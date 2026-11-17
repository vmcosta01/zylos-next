//! Benchmark T11.10.8 — 30 dias de dados sintéticos.
//!
//! Uso:
//! ```text
//! cargo run --release -p zylos-analytics --example analytics_bench -- [eventos_por_dia] [arquivo.db]
//! ```
//! Padrão: 34 000 eventos/dia × 30 dias ≈ 1,02 M linhas em banco SQLite
//! temporário (arquivo, WAL). Imprime tabela Markdown com tempos medidos.
//!
//! Variáveis: `KEEP=1` mantém o diretório temporário; `REUSE=1` reaproveita
//! o banco informado (pula geração/ingestão e retenção — mede o load com
//! processo novo); `NO_RETENTION=1` pula a etapa de retenção (que altera o banco).

use std::time::Instant;
use zylos_analytics::{
    feedback::analyze,
    synthetic::{generate, SyntheticConfig},
    AnalyticsStore, FeedbackConfig, Filter, Granularity, GroupBy, PresetReport, RetentionPolicy,
    DAY_SECS,
};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let per_day: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(34_000);
    let dir = std::env::temp_dir().join(format!("zylos-analytics-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let path = args
        .get(2)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dir.join("bench.db"));
    // Reusa o banco se `REUSE=1` e o arquivo existir (pula geração/ingestão).
    let reuse = std::env::var("REUSE").is_ok_and(|v| v == "1") && path.exists();
    if !reuse {
        let _ = std::fs::remove_file(&path);
    }

    // "agora" fixo = 2026-11-17 00:00 UTC (reprodutível)
    let now: i64 = 1_794_873_600;
    let cfg = SyntheticConfig {
        days: 30,
        events_per_day: per_day,
        ..SyntheticConfig::default()
    };

    let mut rows: Vec<(String, f64, String)> = Vec::new();
    if !reuse {
        let t = Instant::now();
        let events = generate(&cfg, now);
        rows.push((
            "gerar eventos sintéticos".into(),
            ms(t),
            format!("{} eventos", events.len()),
        ));
        let mut store = AnalyticsStore::open(&path).expect("open");
        let t = Instant::now();
        for chunk in events.chunks(50_000) {
            store.insert_batch(chunk).expect("insert");
        }
        rows.push((
            "ingestão (insert_batch, 50k/tx)".into(),
            ms(t),
            String::new(),
        ));
        let t = Instant::now();
        store.optimize().expect("optimize");
        rows.push((
            "optimize (wal_checkpoint + ANALYZE)".into(),
            ms(t),
            String::new(),
        ));
        // fecha a conexão: o load abaixo mede uma conexão nova
    }

    // Reabre para medir o "load" de dashboard a frio (conexão nova).
    let t_load = Instant::now();
    let mut parts = Vec::new();
    let t = Instant::now();
    let store = AnalyticsStore::open(&path).expect("reopen");
    parts.push(format!("open {:.0}", ms(t)));
    let last30 = Filter::all().range(now - 30 * DAY_SECS, now);
    let t = Instant::now();
    let total = store.count(&last30).expect("count");
    parts.push(format!("count {:.0}", ms(t)));
    let t = Instant::now();
    let summary = store.summary(&last30).expect("summary");
    parts.push(format!("summary {:.0}", ms(t)));
    let t = Instant::now();
    let daily = store.aggregate(&last30, Granularity::Daily).expect("daily");
    parts.push(format!("daily {:.0}", ms(t)));
    let t = Instant::now();
    let by_skill = store.breakdown(&last30, GroupBy::Skill).expect("skill");
    let by_agent = store.breakdown(&last30, GroupBy::Agent).expect("agent");
    parts.push(format!("breakdowns {:.0}", ms(t)));
    let t = Instant::now();
    let heat = store.heatmap(&last30).expect("heatmap");
    parts.push(format!("heatmap {:.0}", ms(t)));
    let t = Instant::now();
    let recent = store.query_events(&last30, 100, 0).expect("recent");
    parts.push(format!("eventos {:.0}", ms(t)));
    let load_ms = ms(t_load);
    rows.push((
        "**load do dashboard** (open + count + summary + daily + 2 breakdowns + heatmap + 100 eventos)"
            .into(),
        load_ms,
        format!(
            "{total} linhas · {} dias · {} skills · {} agentes · {} células · {} eventos · parciais ms: {}",
            daily.len(),
            by_skill.len(),
            by_agent.len(),
            heat.len(),
            recent.len(),
            parts.join(", ")
        ),
    ));

    let mut time = |name: &str, f: &mut dyn FnMut() -> String| {
        let t = Instant::now();
        let note = f();
        rows.push((name.to_string(), ms(t), note));
    };
    for g in [
        Granularity::Hourly,
        Granularity::Daily,
        Granularity::Weekly,
        Granularity::Monthly,
    ] {
        time(&format!("aggregate {g:?} (30d, rollup)"), &mut || {
            format!("{} buckets", store.aggregate(&last30, g).unwrap().len())
        });
    }
    time("filtro agente+skill+7d (rollup)", &mut || {
        let f = Filter::all()
            .range(now - 7 * DAY_SECS, now)
            .agent("hermes")
            .skill("pdf-extract");
        format!(
            "{} buckets",
            store.aggregate(&f, Granularity::Hourly).unwrap().len()
        )
    });
    time(
        "filtro por dimensão channel=slack (eventos brutos)",
        &mut || {
            let f = last30.clone().dim("channel", "slack");
            format!(
                "{} buckets",
                store.aggregate(&f, Granularity::Daily).unwrap().len()
            )
        },
    );
    time("filtro latência > 5000 ms (eventos brutos)", &mut || {
        let mut f = last30.clone();
        f.min_latency_ms = Some(5_000);
        format!("{} eventos", store.count(&f).unwrap())
    });
    time("predicado customizado (in-memory, 7d)", &mut || {
        let f = Filter::all().range(now - 7 * DAY_SECS, now);
        let b = store
            .aggregate_with_predicate(&f, Granularity::Daily, |e| {
                !e.success && e.latency_ms > 3_000
            })
            .unwrap();
        format!("{} buckets", b.len())
    });
    time("feedback loop (analyze)", &mut || {
        let r = analyze(&store, now, &FeedbackConfig::default()).unwrap();
        let low: Vec<_> = r
            .recommendations
            .iter()
            .filter(|x| x.success_rate < 0.70)
            .map(|x| x.skill.clone())
            .collect();
        format!(
            "{} recomendações · <70%: {:?}",
            r.recommendations.len(),
            low
        )
    });
    for p in PresetReport::ALL {
        time(&format!("preset {} (gerar + PDF)", p.id()), &mut || {
            let r = p.generate(&store, now, &Filter::all()).unwrap();
            let pdf = zylos_analytics::export::report_to_pdf(&r);
            format!("{} seções · PDF {} bytes", r.sections.len(), pdf.len())
        });
    }
    time("export CSV (7d, eventos brutos)", &mut || {
        let f = Filter::all().range(now - 7 * DAY_SECS, now);
        let mut ev = Vec::new();
        store.for_each_event(&f, |e| ev.push(e)).unwrap();
        let csv = zylos_analytics::export::events_to_csv(&ev, &f, now);
        format!("{} linhas · {} bytes", ev.len(), csv.len())
    });
    drop(time);

    // Retenção: +1 dia simulado à frente com política 29d → remove o 1º dia
    let mut store = store;
    if reuse || std::env::var("NO_RETENTION").is_ok_and(|v| v == "1") {
        // não muta um banco reutilizado (nem quando NO_RETENTION=1)
        print_rows(&rows, total, summary.success_rate, &path, load_ms);
        return;
    }
    let t = Instant::now();
    let rep = store
        .apply_retention(
            &RetentionPolicy {
                archive_after_days: Some(7),
                delete_after_days: 29,
            },
            now,
        )
        .unwrap();
    rows.push((
        "retenção (archive >7d, delete >29d)".into(),
        ms(t),
        format!("{rep:?}"),
    ));

    print_rows(&rows, total, summary.success_rate, &path, load_ms);
    drop(store);
    if std::env::var("KEEP").is_err() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn print_rows(
    rows: &[(String, f64, String)],
    total: u64,
    success_rate: f64,
    path: &std::path::Path,
    load_ms: f64,
) {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    println!("| Operação | Tempo (ms) | Observação |\n|---|---:|---|");
    for (n, t, o) in rows {
        println!("| {n} | {t:.1} | {o} |");
    }
    println!(
        "\nlinhas={total} · sucesso médio={:.1}% · arquivo={:.1} MiB",
        success_rate * 100.0,
        size as f64 / 1_048_576.0
    );
    println!(
        "RESULTADO: load {:.1} ms → {}",
        load_ms,
        if load_ms < 2_000.0 {
            "PASS (<2s)"
        } else {
            "FAIL (>=2s)"
        }
    );
}
