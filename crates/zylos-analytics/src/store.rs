//! Armazenamento SQLite de eventos de analytics.
//!
//! Schema:
//! - `events` — eventos "quentes" (últimos `archive_after_days`).
//! - `events_archive` — eventos arquivados pela política de retenção (mesmo schema).
//! - `all_events` — view `UNION ALL` das duas tabelas (consultas brutas).
//! - `rollup_hourly` — pré-agregação por (hora, agente, skill), mantida na
//!   inserção. Responde às consultas do dashboard sem varrer a tabela bruta;
//!   é o que mantém o load < 2s com 1M+ eventos (ver
//!   `docs/fase11-reports/analytics-benchmark.md`).
//! - `rollup_dim_hourly` — pré-agregação por (dimensão, valor, hora, agente,
//!   skill). Responde filtros de **uma** dimensão customizada (`dim.k = v`)
//!   sem `json_extract` sobre a tabela bruta. Bancos criados antes desta
//!   tabela são migrados (backfill idempotente) em [`AnalyticsStore::open`].
//!
//! `rusqlite::Connection` é `Send` mas não `Sync`; o gateway envolve o store
//! num `Mutex`.

use crate::event::AnalyticsEvent;
use crate::filter::Filter;
use crate::Result;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts          INTEGER NOT NULL,
    agent       TEXT NOT NULL,
    skill       TEXT NOT NULL,
    success     INTEGER NOT NULL,
    latency_ms  INTEGER NOT NULL,
    cost_usd    REAL NOT NULL DEFAULT 0,
    tokens      INTEGER NOT NULL DEFAULT 0,
    dims        TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_events_ts ON events(ts);
CREATE INDEX IF NOT EXISTS idx_events_skill_ts ON events(skill, ts);
CREATE INDEX IF NOT EXISTS idx_events_agent_ts ON events(agent, ts);

CREATE TABLE IF NOT EXISTS events_archive (
    id          INTEGER PRIMARY KEY,
    ts          INTEGER NOT NULL,
    agent       TEXT NOT NULL,
    skill       TEXT NOT NULL,
    success     INTEGER NOT NULL,
    latency_ms  INTEGER NOT NULL,
    cost_usd    REAL NOT NULL DEFAULT 0,
    tokens      INTEGER NOT NULL DEFAULT 0,
    dims        TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_archive_ts ON events_archive(ts);

CREATE VIEW IF NOT EXISTS all_events AS
    SELECT id, ts, agent, skill, success, latency_ms, cost_usd, tokens, dims FROM events
    UNION ALL
    SELECT id, ts, agent, skill, success, latency_ms, cost_usd, tokens, dims FROM events_archive;

CREATE TABLE IF NOT EXISTS rollup_hourly (
    hour        INTEGER NOT NULL,
    agent       TEXT NOT NULL,
    skill       TEXT NOT NULL,
    n           INTEGER NOT NULL,
    ok          INTEGER NOT NULL,
    lat_sum     INTEGER NOT NULL,
    lat_max     INTEGER NOT NULL,
    cost        REAL NOT NULL,
    tokens      INTEGER NOT NULL,
    PRIMARY KEY (hour, agent, skill)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS idx_rollup_skill ON rollup_hourly(skill, hour);
CREATE INDEX IF NOT EXISTS idx_rollup_agent ON rollup_hourly(agent, hour);

CREATE TABLE IF NOT EXISTS rollup_dim_hourly (
    dkey        TEXT NOT NULL,
    dval        TEXT NOT NULL,
    hour        INTEGER NOT NULL,
    agent       TEXT NOT NULL,
    skill       TEXT NOT NULL,
    n           INTEGER NOT NULL,
    ok          INTEGER NOT NULL,
    lat_sum     INTEGER NOT NULL,
    lat_max     INTEGER NOT NULL,
    cost        REAL NOT NULL,
    tokens      INTEGER NOT NULL,
    PRIMARY KEY (dkey, dval, hour, agent, skill)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

const UPSERT_ROLLUP: &str =
    "INSERT INTO rollup_hourly (hour, agent, skill, n, ok, lat_sum, lat_max, cost, tokens)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
     ON CONFLICT(hour, agent, skill) DO UPDATE SET
        n = n + excluded.n,
        ok = ok + excluded.ok,
        lat_sum = lat_sum + excluded.lat_sum,
        lat_max = MAX(lat_max, excluded.lat_max),
        cost = cost + excluded.cost,
        tokens = tokens + excluded.tokens";

const UPSERT_DIM_ROLLUP: &str =
    "INSERT INTO rollup_dim_hourly (dkey, dval, hour, agent, skill, n, ok, lat_sum, lat_max, cost, tokens)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
     ON CONFLICT(dkey, dval, hour, agent, skill) DO UPDATE SET
        n = n + excluded.n,
        ok = ok + excluded.ok,
        lat_sum = lat_sum + excluded.lat_sum,
        lat_max = MAX(lat_max, excluded.lat_max),
        cost = cost + excluded.cost,
        tokens = tokens + excluded.tokens";

/// Chave em `meta` que marca o backfill de `rollup_dim_hourly` como feito.
const META_DIM_ROLLUP: &str = "schema.rollup_dim_hourly.v1";

/// Backfill de `rollup_dim_hourly` a partir dos eventos brutos (quentes +
/// arquivo). Só valores-texto entram — o mesmo que `insert_batch` produz.
const BACKFILL_DIM_ROLLUP: &str = "
    DELETE FROM rollup_dim_hourly;
    INSERT INTO rollup_dim_hourly (dkey, dval, hour, agent, skill, n, ok, lat_sum, lat_max, cost, tokens)
    SELECT j.key, j.value, e.ts - (e.ts % 3600), e.agent, e.skill,
           COUNT(*), SUM(e.success), SUM(e.latency_ms), MAX(e.latency_ms),
           SUM(e.cost_usd), SUM(e.tokens)
    FROM all_events AS e, json_each(e.dims) AS j
    WHERE e.dims <> '{}' AND j.type = 'text'
    GROUP BY 1, 2, 3, 4, 5;";

/// Fonte física escolhida para responder um [`Filter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// `rollup_hourly` (período/agente/skill alinhados à hora).
    Rollup,
    /// `rollup_dim_hourly` (idem + exatamente uma dimensão).
    DimRollup,
    /// `all_events` (eventos brutos).
    Raw,
}

impl Source {
    pub(crate) fn of(f: &Filter) -> Self {
        if f.rollup_compatible() {
            Self::Rollup
        } else if f.dim_rollup_compatible() {
            Self::DimRollup
        } else {
            Self::Raw
        }
    }

    pub(crate) fn table(self) -> &'static str {
        match self {
            Self::Rollup => "rollup_hourly",
            Self::DimRollup => "rollup_dim_hourly",
            Self::Raw => "all_events",
        }
    }

    pub(crate) fn ts_col(self) -> &'static str {
        match self {
            Self::Raw => "ts",
            _ => "hour",
        }
    }

    pub(crate) fn is_rollup(self) -> bool {
        self != Self::Raw
    }

    /// `WHERE` parametrizado para esta fonte.
    pub(crate) fn where_sql(self, f: &Filter) -> Result<(String, Vec<rusqlite::types::Value>)> {
        let (mut w, mut p) = f.to_sql(self.ts_col(), self.is_rollup())?;
        if self == Self::DimRollup {
            if let Some((k, v)) = f.dimensions.iter().next() {
                w.push_str(if w.is_empty() { " WHERE " } else { " AND " });
                w.push_str("dkey = ? AND dval = ?");
                p.push(rusqlite::types::Value::Text(k.clone()));
                p.push(rusqlite::types::Value::Text(v.clone()));
            }
        }
        Ok((w, p))
    }
}

/// Store SQLite de eventos.
pub struct AnalyticsStore {
    pub(crate) conn: Connection,
}

#[derive(Default)]
struct RollAcc {
    n: i64,
    ok: i64,
    lat_sum: i64,
    lat_max: i64,
    cost: f64,
    tokens: i64,
}

impl AnalyticsStore {
    /// Abre (ou cria) o banco em `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        // 64 MiB de page cache + 256 MiB mmap: o rollup e os índices ficam
        // quentes em memória (o benchmark T11.10.8 depende disso).
        conn.pragma_update(None, "cache_size", -65_536)?;
        conn.pragma_update(None, "mmap_size", 268_435_456i64)?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        Self::init(conn)
    }

    /// Banco em memória (testes e demo).
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Self::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Migrações idempotentes para bancos criados por versões anteriores.
    fn migrate(conn: &mut Connection) -> Result<()> {
        let done: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [META_DIM_ROLLUP],
                |r| r.get(0),
            )
            .optional()?;
        if done.is_none() {
            let tx = conn.transaction()?;
            tx.execute_batch(BACKFILL_DIM_ROLLUP)?;
            tx.execute(
                "INSERT INTO meta(key, value) VALUES (?1, '1')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [META_DIM_ROLLUP],
            )?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Manutenção após cargas grandes: checkpoint do WAL (trunca o arquivo
    /// `-wal`) e `ANALYZE`/`PRAGMA optimize` para o planner escolher os índices.
    pub fn optimize(&self) -> Result<()> {
        self.conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(()),
                other => Err(other),
            })?;
        self.conn.execute_batch("ANALYZE; PRAGMA optimize;")?;
        Ok(())
    }

    /// Insere um evento; retorna o id.
    pub fn insert(&mut self, e: &AnalyticsEvent) -> Result<i64> {
        self.insert_batch(std::slice::from_ref(e))?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Insere vários eventos em uma transação, atualizando o rollup horário.
    /// Retorna quantos foram inseridos. Falha atômica se algum for inválido.
    pub fn insert_batch(&mut self, events: &[AnalyticsEvent]) -> Result<usize> {
        for e in events {
            e.validate()?;
        }
        let tx = self.conn.transaction()?;
        let mut roll: HashMap<(i64, &str, &str), RollAcc> = HashMap::new();
        let mut droll: HashMap<(&str, &str, i64, &str, &str), RollAcc> = HashMap::new();
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO events (ts, agent, skill, success, latency_ms, cost_usd, tokens, dims)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for e in events {
                let dims = if e.dimensions.is_empty() {
                    "{}".to_string()
                } else {
                    serde_json::to_string(&e.dimensions)?
                };
                stmt.execute(params![
                    e.ts,
                    e.agent,
                    e.skill,
                    e.success as i64,
                    e.latency_ms as i64,
                    e.cost_usd,
                    e.tokens as i64,
                    dims
                ])?;
                let hour = e.ts - e.ts.rem_euclid(3600);
                let acc = roll
                    .entry((hour, e.agent.as_str(), e.skill.as_str()))
                    .or_default();
                acc.n += 1;
                acc.ok += e.success as i64;
                acc.lat_sum += e.latency_ms as i64;
                acc.lat_max = acc.lat_max.max(e.latency_ms as i64);
                acc.cost += e.cost_usd;
                acc.tokens += e.tokens as i64;
                for (k, v) in &e.dimensions {
                    let d = droll
                        .entry((
                            k.as_str(),
                            v.as_str(),
                            hour,
                            e.agent.as_str(),
                            e.skill.as_str(),
                        ))
                        .or_default();
                    d.n += 1;
                    d.ok += e.success as i64;
                    d.lat_sum += e.latency_ms as i64;
                    d.lat_max = d.lat_max.max(e.latency_ms as i64);
                    d.cost += e.cost_usd;
                    d.tokens += e.tokens as i64;
                }
            }
            let mut up = tx.prepare_cached(UPSERT_ROLLUP)?;
            for ((hour, agent, skill), a) in &roll {
                up.execute(params![
                    hour, agent, skill, a.n, a.ok, a.lat_sum, a.lat_max, a.cost, a.tokens
                ])?;
            }
            let mut dup = tx.prepare_cached(UPSERT_DIM_ROLLUP)?;
            for ((k, v, hour, agent, skill), a) in &droll {
                dup.execute(params![
                    k, v, hour, agent, skill, a.n, a.ok, a.lat_sum, a.lat_max, a.cost, a.tokens
                ])?;
            }
        }
        tx.commit()?;
        Ok(events.len())
    }

    /// Conta eventos (quentes + arquivados) que satisfazem o filtro.
    pub fn count(&self, f: &Filter) -> Result<u64> {
        let src = Source::of(f);
        if src.is_rollup() {
            let (w, p) = src.where_sql(f)?;
            let sql = format!("SELECT COALESCE(SUM(n),0) FROM {}{w}", src.table());
            let n: i64 = self
                .conn
                .query_row(&sql, params_from_iter(p), |r| r.get(0))?;
            return Ok(n as u64);
        }
        let (w, p) = f.to_sql("ts", false)?;
        let sql = format!("SELECT COUNT(*) FROM all_events{w}");
        let n: i64 = self
            .conn
            .query_row(&sql, params_from_iter(p), |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Lista eventos (mais recentes primeiro) com paginação.
    ///
    /// Consulta cada tabela (quente e arquivo) com `ORDER BY ts DESC LIMIT`
    /// próprio — usa `idx_events_ts`/`idx_archive_ts` em vez de ordenar a view
    /// `all_events` inteira — e intercala o resultado.
    pub fn query_events(
        &self,
        f: &Filter,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AnalyticsEvent>> {
        let (w, p) = f.to_sql("ts", false)?;
        let take = (limit + offset) as i64;
        let sql = format!(
            "SELECT * FROM (SELECT id, ts, agent, skill, success, latency_ms, cost_usd, tokens, dims
                FROM events{w} ORDER BY ts DESC, id DESC LIMIT ?)
             UNION ALL
             SELECT * FROM (SELECT id, ts, agent, skill, success, latency_ms, cost_usd, tokens, dims
                FROM events_archive{w} ORDER BY ts DESC, id DESC LIMIT ?)
             ORDER BY 2 DESC, 1 DESC LIMIT ? OFFSET ?"
        );
        let mut all = p.clone();
        all.push(rusqlite::types::Value::Integer(take));
        all.extend(p);
        all.push(rusqlite::types::Value::Integer(take));
        all.push(rusqlite::types::Value::Integer(limit as i64));
        all.push(rusqlite::types::Value::Integer(offset as i64));
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(all), row_to_event)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Itera todos os eventos do filtro em ordem cronológica, chamando `visit`.
    pub fn for_each_event(&self, f: &Filter, mut visit: impl FnMut(AnalyticsEvent)) -> Result<()> {
        let (w, p) = f.to_sql("ts", false)?;
        let sql = format!(
            "SELECT id, ts, agent, skill, success, latency_ms, cost_usd, tokens, dims
             FROM all_events{w} ORDER BY ts ASC, id ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(p), row_to_event)?;
        for r in rows {
            visit(r?);
        }
        Ok(())
    }

    /// Lista eventos que satisfazem filtro **e** um predicado customizado.
    pub fn query_events_with(
        &self,
        f: &Filter,
        pred: impl Fn(&AnalyticsEvent) -> bool,
        limit: usize,
    ) -> Result<Vec<AnalyticsEvent>> {
        let mut out = Vec::new();
        self.for_each_event(f, |e| {
            if out.len() < limit && pred(&e) {
                out.push(e);
            }
        })?;
        Ok(out)
    }

    /// Agentes distintos (para dropdowns de filtro).
    pub fn distinct_agents(&self) -> Result<Vec<String>> {
        self.distinct("agent")
    }

    /// Skills distintas (para dropdowns de filtro).
    pub fn distinct_skills(&self) -> Result<Vec<String>> {
        self.distinct("skill")
    }

    fn distinct(&self, col: &str) -> Result<Vec<String>> {
        let sql = format!("SELECT DISTINCT {col} FROM rollup_hourly ORDER BY {col}");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut v = Vec::new();
        for r in rows {
            v.push(r?);
        }
        Ok(v)
    }

    /// Quantidade de eventos nas tabelas `(quentes, arquivados)`.
    pub fn table_counts(&self) -> Result<(u64, u64)> {
        let hot: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?;
        let arch: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM events_archive", [], |r| r.get(0))?;
        Ok((hot as u64, arch as u64))
    }

    /// Lê um valor da tabela `meta`.
    pub fn meta_get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    /// Grava um valor na tabela `meta`.
    pub fn meta_set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

fn row_to_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<AnalyticsEvent> {
    let dims: String = r.get(8)?;
    let dimensions: BTreeMap<String, String> = serde_json::from_str(&dims).unwrap_or_default();
    Ok(AnalyticsEvent {
        id: Some(r.get(0)?),
        ts: r.get(1)?,
        agent: r.get(2)?,
        skill: r.get(3)?,
        success: r.get::<_, i64>(4)? != 0,
        latency_ms: r.get::<_, i64>(5)?.max(0) as u64,
        cost_usd: r.get(6)?,
        tokens: r.get::<_, i64>(7)?.max(0) as u64,
        dimensions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> AnalyticsStore {
        AnalyticsStore::open_in_memory().unwrap()
    }

    #[test]
    fn insert_and_count() {
        let mut s = store();
        let evs = vec![
            AnalyticsEvent::new(3600, "a1", "s1", true, 10),
            AnalyticsEvent::new(3700, "a1", "s2", false, 20).with_dim("channel", "slack"),
            AnalyticsEvent::new(7200, "a2", "s1", true, 30),
        ];
        assert_eq!(s.insert_batch(&evs).unwrap(), 3);
        assert_eq!(s.count(&Filter::all()).unwrap(), 3);
        assert_eq!(s.count(&Filter::all().agent("a1")).unwrap(), 2);
        assert_eq!(s.count(&Filter::all().dim("channel", "slack")).unwrap(), 1);
        let ff = Filter {
            success: Some(false),
            ..Filter::default()
        };
        assert_eq!(s.count(&ff).unwrap(), 1);
        assert_eq!(s.distinct_skills().unwrap(), vec!["s1", "s2"]);
    }

    #[test]
    fn invalid_batch_is_atomic() {
        let mut s = store();
        let evs = vec![
            AnalyticsEvent::new(1, "a", "s", true, 1),
            AnalyticsEvent::new(2, "", "s", true, 1),
        ];
        assert!(s.insert_batch(&evs).is_err());
        assert_eq!(s.count(&Filter::all()).unwrap(), 0);
    }

    #[test]
    fn query_events_roundtrip_and_predicate() {
        let mut s = store();
        s.insert(&AnalyticsEvent::new(100, "a", "s", true, 5).with_dim("model", "haiku"))
            .unwrap();
        s.insert(&AnalyticsEvent::new(200, "a", "s", true, 500))
            .unwrap();
        let all = s.query_events(&Filter::all(), 10, 0).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].ts, 200);
        assert_eq!(
            all[1].dimensions.get("model").map(String::as_str),
            Some("haiku")
        );
        let slow = s
            .query_events_with(&Filter::all(), |e| e.latency_ms > 100, 10)
            .unwrap();
        assert_eq!(slow.len(), 1);
    }

    #[test]
    fn migration_backfills_dim_rollup_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("old.db");
        {
            let mut s = AnalyticsStore::open(&p).unwrap();
            s.insert_batch(&[
                AnalyticsEvent::new(3600, "a", "s", true, 10).with_dim("channel", "slack"),
                AnalyticsEvent::new(3700, "a", "s", false, 30).with_dim("channel", "slack"),
                AnalyticsEvent::new(3800, "b", "s", true, 20).with_dim("channel", "api"),
                AnalyticsEvent::new(3900, "b", "s", true, 20),
            ])
            .unwrap();
            // simula banco de versão anterior: sem a tabela nem o marcador
            s.conn
                .execute_batch(
                    "DROP TABLE rollup_dim_hourly;
                     DELETE FROM meta WHERE key = 'schema.rollup_dim_hourly.v1';",
                )
                .unwrap();
        }
        for _ in 0..2 {
            let s = AnalyticsStore::open(&p).unwrap();
            let slack = Filter::all().dim("channel", "slack");
            assert_eq!(s.count(&slack).unwrap(), 2);
            let b = s.summary(&slack).unwrap();
            assert_eq!((b.count, b.success, b.max_latency_ms), (2, 1, 30));
            assert_eq!(s.count(&Filter::all().dim("channel", "api")).unwrap(), 1);
        }
    }

    #[test]
    fn file_store_persists() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.db");
        {
            let mut s = AnalyticsStore::open(&p).unwrap();
            s.insert(&AnalyticsEvent::new(1, "a", "s", true, 1))
                .unwrap();
            s.meta_set("k", "v").unwrap();
        }
        let s = AnalyticsStore::open(&p).unwrap();
        assert_eq!(s.count(&Filter::all()).unwrap(), 1);
        assert_eq!(s.meta_get("k").unwrap().as_deref(), Some("v"));
    }
}
