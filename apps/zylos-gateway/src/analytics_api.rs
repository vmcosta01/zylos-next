//! Analytics Dashboard API (Fase 11.10).
//!
//! Rotas (montadas sob `/api/v1`):
//!
//! | Método | Rota | Descrição |
//! |---|---|---|
//! | GET  | `/analytics/summary` | Totais do filtro (count, sucesso, latência, custo) |
//! | GET  | `/analytics/timeseries?granularity=hourly\|daily\|weekly\|monthly[&group_by=agent\|skill]` | Série temporal |
//! | GET  | `/analytics/breakdown?by=agent\|skill` | Agregado por agente/skill |
//! | GET  | `/analytics/heatmap` | Matriz 7×24 (dia da semana × hora UTC) |
//! | GET  | `/analytics/events?limit=&offset=` | Eventos brutos paginados |
//! | POST | `/analytics/events` | Ingestão (`AnalyticsEvent` ou lista) |
//! | GET  | `/analytics/dimensions` | Agentes e skills conhecidos (para filtros) |
//! | GET  | `/analytics/export?kind=events\|timeseries\|breakdown&format=csv` | Export CSV com metadados |
//! | GET  | `/analytics/reports` | Lista de relatórios preset |
//! | GET  | `/analytics/reports/:id?format=json\|csv\|pdf` | Gera relatório preset |
//! | GET  | `/analytics/recommendations` | Feedback loop de auto-melhoria |
//! | GET  | `/analytics/retention` | Política vigente + última execução |
//! | POST | `/analytics/retention` | Aplica retenção (corpo opcional: `RetentionPolicy`) |
//!
//! Filtros comuns (query string): `since`, `until` (epoch s, `until`
//! exclusivo), `agents=a,b`, `skills=x,y`, `success=true|false`,
//! `min_latency_ms`, `max_latency_ms`, `dim.<chave>=<valor>`.
//!
//! O `AnalyticsStore` (rusqlite) não é `Sync`; fica atrás de
//! `Arc<Mutex<_>>` e toda consulta roda em `spawn_blocking`.

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use zylos_analytics::{
    export, feedback, AnalyticsError, AnalyticsEvent, AnalyticsStore, FeedbackConfig, Filter,
    Granularity, GroupBy, PresetReport, RetentionPolicy,
};

/// Handle compartilhado do store de analytics.
#[derive(Clone)]
pub struct AnalyticsHandle {
    store: Arc<Mutex<AnalyticsStore>>,
    retention: Arc<Mutex<RetentionPolicy>>,
}

impl AnalyticsHandle {
    /// Envolve um store existente.
    pub fn new(store: AnalyticsStore) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            retention: Arc::new(Mutex::new(RetentionPolicy::default())),
        }
    }

    /// Store em memória (testes / fallback).
    pub fn in_memory() -> Self {
        Self::new(AnalyticsStore::open_in_memory().expect("sqlite em memória"))
    }

    /// Abre `ZYLOS_ANALYTICS_DB` (padrão `data/analytics.db`); cai para
    /// memória se o arquivo não puder ser aberto. Política de retenção lida
    /// de `ZYLOS_ANALYTICS_RETENTION_DAYS` (padrão 90).
    pub fn from_env() -> Self {
        let path =
            std::env::var("ZYLOS_ANALYTICS_DB").unwrap_or_else(|_| "data/analytics.db".to_string());
        if let Some(parent) = std::path::Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let h = match AnalyticsStore::open(&path) {
            Ok(s) => Self::new(s),
            Err(e) => {
                tracing::warn!(error = %e, path, "analytics store indisponível; usando memória");
                Self::in_memory()
            }
        };
        if let Some(days) = std::env::var("ZYLOS_ANALYTICS_RETENTION_DAYS")
            .ok()
            .and_then(|d| d.parse::<i64>().ok())
        {
            let mut p = RetentionPolicy {
                delete_after_days: days,
                ..RetentionPolicy::default()
            };
            if p.archive_after_days.is_some_and(|a| a >= days) {
                p.archive_after_days = None;
            }
            if p.validate().is_ok() {
                *h.retention.lock().expect("lock") = p;
            }
        }
        h
    }

    async fn run<T, F>(&self, f: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&mut AnalyticsStore) -> zylos_analytics::Result<T> + Send + 'static,
    {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = store
                .lock()
                .map_err(|_| ApiError::internal("lock envenenado"))?;
            f(&mut guard).map_err(ApiError::from)
        })
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    }
}

/// Erro HTTP da API.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad(m: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: m.into(),
        }
    }
    fn internal(m: &str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: m.to_string(),
        }
    }
}

impl From<AnalyticsError> for ApiError {
    fn from(e: AnalyticsError) -> Self {
        match e {
            AnalyticsError::Invalid(m) => Self::bad(m),
            other => Self::internal(&other.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

type Params = HashMap<String, String>;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn parse_num<T: std::str::FromStr>(p: &Params, k: &str) -> Result<Option<T>, ApiError> {
    match p.get(k).map(|s| s.trim()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(v) => v
            .parse()
            .map(Some)
            .map_err(|_| ApiError::bad(format!("parâmetro inválido: {k}={v}"))),
    }
}

fn list(p: &Params, k: &str) -> Vec<String> {
    p.get(k)
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Constrói [`Filter`] a partir da query string.
pub fn filter_from_params(p: &Params) -> Result<Filter, ApiError> {
    let mut f = Filter {
        since: parse_num(p, "since")?,
        until: parse_num(p, "until")?,
        agents: list(p, "agents"),
        skills: list(p, "skills"),
        success: parse_num(p, "success")?,
        min_latency_ms: parse_num(p, "min_latency_ms")?,
        max_latency_ms: parse_num(p, "max_latency_ms")?,
        ..Filter::default()
    };
    for (k, v) in p {
        if let Some(dim) = k.strip_prefix("dim.") {
            f.dimensions.insert(dim.to_string(), v.clone());
        }
    }
    f.validate()?;
    Ok(f)
}

fn granularity(p: &Params) -> Result<Granularity, ApiError> {
    Ok(Granularity::parse(
        p.get("granularity").map(String::as_str).unwrap_or("daily"),
    )?)
}

fn filter_json(f: &Filter) -> Value {
    json!({ "filter": f, "description": f.describe() })
}

async fn summary(
    State(h): State<AnalyticsHandle>,
    Query(p): Query<Params>,
) -> Result<Json<Value>, ApiError> {
    let f = filter_from_params(&p)?;
    let fc = f.clone();
    let s = h.run(move |s| s.summary(&fc)).await?;
    Ok(Json(json!({ "summary": s, "meta": filter_json(&f) })))
}

async fn timeseries(
    State(h): State<AnalyticsHandle>,
    Query(p): Query<Params>,
) -> Result<Json<Value>, ApiError> {
    let f = filter_from_params(&p)?;
    let g = granularity(&p)?;
    let by = p.get("group_by").map(|s| GroupBy::parse(s)).transpose()?;
    let fc = f.clone();
    let data = h
        .run(move |s| match by {
            None => Ok(json!({ "series": s.aggregate(&fc, g)? })),
            Some(by) => Ok(json!({ "groups": s.aggregate_by(&fc, g, by)? })),
        })
        .await?;
    Ok(Json(
        json!({ "granularity": g, "data": data, "meta": filter_json(&f) }),
    ))
}

async fn breakdown(
    State(h): State<AnalyticsHandle>,
    Query(p): Query<Params>,
) -> Result<Json<Value>, ApiError> {
    let f = filter_from_params(&p)?;
    let by = GroupBy::parse(p.get("by").map(String::as_str).unwrap_or("skill"))?;
    let fc = f.clone();
    let groups = h.run(move |s| s.breakdown(&fc, by)).await?;
    Ok(Json(
        json!({ "by": by, "groups": groups, "meta": filter_json(&f) }),
    ))
}

async fn heatmap(
    State(h): State<AnalyticsHandle>,
    Query(p): Query<Params>,
) -> Result<Json<Value>, ApiError> {
    let f = filter_from_params(&p)?;
    let fc = f.clone();
    let cells = h.run(move |s| s.heatmap(&fc)).await?;
    Ok(Json(json!({ "cells": cells, "meta": filter_json(&f) })))
}

async fn list_events(
    State(h): State<AnalyticsHandle>,
    Query(p): Query<Params>,
) -> Result<Json<Value>, ApiError> {
    let f = filter_from_params(&p)?;
    let limit = parse_num::<usize>(&p, "limit")?.unwrap_or(100).min(1_000);
    let offset = parse_num::<usize>(&p, "offset")?.unwrap_or(0);
    let fc = f.clone();
    let (total, events) = h
        .run(move |s| Ok((s.count(&fc)?, s.query_events(&fc, limit, offset)?)))
        .await?;
    Ok(Json(json!({
        "total": total, "limit": limit, "offset": offset,
        "events": events, "meta": filter_json(&f)
    })))
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Ingest {
    One(AnalyticsEvent),
    Many(Vec<AnalyticsEvent>),
}

async fn ingest(
    State(h): State<AnalyticsHandle>,
    Json(body): Json<Ingest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let events = match body {
        Ingest::One(e) => vec![e],
        Ingest::Many(v) => v,
    };
    if events.len() > 50_000 {
        return Err(ApiError::bad("lote máximo: 50000 eventos"));
    }
    let n = h.run(move |s| s.insert_batch(&events)).await?;
    Ok((StatusCode::CREATED, Json(json!({ "inserted": n }))))
}

async fn dimensions(State(h): State<AnalyticsHandle>) -> Result<Json<Value>, ApiError> {
    let (agents, skills) = h
        .run(|s| Ok((s.distinct_agents()?, s.distinct_skills()?)))
        .await?;
    Ok(Json(json!({ "agents": agents, "skills": skills })))
}

fn attachment(body: Vec<u8>, mime: &'static str, filename: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response()
}

async fn export_data(
    State(h): State<AnalyticsHandle>,
    Query(p): Query<Params>,
) -> Result<Response, ApiError> {
    let f = filter_from_params(&p)?;
    let format = p.get("format").map(String::as_str).unwrap_or("csv");
    if format != "csv" {
        return Err(ApiError::bad("export de dados brutos suporta format=csv; use /analytics/reports/:id?format=pdf para PDF"));
    }
    let kind = p.get("kind").cloned().unwrap_or_else(|| "events".into());
    let g = granularity(&p)?;
    let by = GroupBy::parse(p.get("by").map(String::as_str).unwrap_or("skill"))?;
    let limit = parse_num::<usize>(&p, "limit")?
        .unwrap_or(100_000)
        .min(1_000_000);
    let ts = now();
    let k = kind.clone();
    let csv = h
        .run(move |s| match k.as_str() {
            "events" => {
                let ev = s.query_events(&f, limit, 0)?;
                Ok(export::events_to_csv(&ev, &f, ts))
            }
            "timeseries" => Ok(export::buckets_to_csv(
                &format!("Série temporal ({g:?})"),
                &s.aggregate(&f, g)?,
                &f,
                ts,
            )),
            "breakdown" => Ok(export::groups_to_csv(
                &format!("Breakdown por {by:?}"),
                by.column(),
                &s.breakdown(&f, by)?,
                &f,
                ts,
            )),
            other => Err(AnalyticsError::Invalid(format!(
                "kind inválido: {other} (events|timeseries|breakdown)"
            ))),
        })
        .await?;
    Ok(attachment(
        csv.into_bytes(),
        "text/csv; charset=utf-8",
        &format!("zylos-analytics-{kind}.csv"),
    ))
}

async fn list_reports() -> Json<Value> {
    Json(json!({
        "reports": PresetReport::ALL.iter().map(|p| json!({
            "id": p.id(), "title": p.title(),
            "formats": ["json", "csv", "pdf"],
        })).collect::<Vec<_>>()
    }))
}

async fn get_report(
    State(h): State<AnalyticsHandle>,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> Result<Response, ApiError> {
    let preset = PresetReport::parse(&id)?;
    let mut base = filter_from_params(&p)?;
    base.since = None;
    base.until = None;
    let at = parse_num::<i64>(&p, "now")?.unwrap_or_else(now);
    let report = h.run(move |s| preset.generate(s, at, &base)).await?;
    Ok(
        match p.get("format").map(String::as_str).unwrap_or("json") {
            "pdf" => attachment(
                export::report_to_pdf(&report),
                "application/pdf",
                &format!("{}.pdf", preset.id()),
            ),
            "csv" => attachment(
                export::report_to_csv(&report).into_bytes(),
                "text/csv; charset=utf-8",
                &format!("{}.csv", preset.id()),
            ),
            "json" => Json(report).into_response(),
            other => return Err(ApiError::bad(format!("format inválido: {other}"))),
        },
    )
}

async fn recommendations(
    State(h): State<AnalyticsHandle>,
    Query(p): Query<Params>,
) -> Result<Json<Value>, ApiError> {
    let mut cfg = FeedbackConfig::default();
    if let Some(v) = parse_num(&p, "window_days")? {
        cfg.window_days = v;
    }
    if let Some(v) = parse_num(&p, "min_success_rate")? {
        cfg.min_success_rate = v;
    }
    if let Some(v) = parse_num(&p, "min_samples")? {
        cfg.min_samples = v;
    }
    let at = parse_num::<i64>(&p, "now")?.unwrap_or_else(now);
    let report = h.run(move |s| feedback::analyze(s, at, &cfg)).await?;
    Ok(Json(
        serde_json::to_value(report).map_err(|e| ApiError::internal(&e.to_string()))?,
    ))
}

async fn get_retention(State(h): State<AnalyticsHandle>) -> Result<Json<Value>, ApiError> {
    let policy = *h.retention.lock().map_err(|_| ApiError::internal("lock"))?;
    let (last, counts) = h
        .run(|s| Ok((s.meta_get("retention.last_run")?, s.table_counts()?)))
        .await?;
    let last: Option<Value> = last.and_then(|l| serde_json::from_str(&l).ok());
    Ok(Json(json!({
        "policy": policy, "last_run": last,
        "hot_events": counts.0, "archived_events": counts.1
    })))
}

async fn post_retention(
    State(h): State<AnalyticsHandle>,
    body: Option<Json<RetentionPolicy>>,
) -> Result<Json<Value>, ApiError> {
    let policy = match body {
        Some(Json(p)) => {
            p.validate()?;
            *h.retention.lock().map_err(|_| ApiError::internal("lock"))? = p;
            p
        }
        None => *h.retention.lock().map_err(|_| ApiError::internal("lock"))?,
    };
    let at = now();
    let report = h.run(move |s| s.apply_retention(&policy, at)).await?;
    Ok(Json(json!({ "policy": policy, "report": report })))
}

/// Router de analytics (estado já resolvido → `Router<()>`).
pub fn create_analytics_router(handle: AnalyticsHandle) -> Router {
    Router::new()
        .route("/analytics/summary", get(summary))
        .route("/analytics/timeseries", get(timeseries))
        .route("/analytics/breakdown", get(breakdown))
        .route("/analytics/heatmap", get(heatmap))
        .route("/analytics/events", get(list_events).post(ingest))
        .route("/analytics/dimensions", get(dimensions))
        .route("/analytics/export", get(export_data))
        .route("/analytics/reports", get(list_reports))
        .route("/analytics/reports/:id", get(get_report))
        .route("/analytics/recommendations", get(recommendations))
        .route(
            "/analytics/retention",
            get(get_retention).post(post_retention),
        )
        .with_state(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;
    use zylos_analytics::synthetic::{generate, SyntheticConfig};

    fn seeded() -> (Router, i64) {
        let at = now();
        let mut s = AnalyticsStore::open_in_memory().unwrap();
        let cfg = SyntheticConfig {
            days: 14,
            events_per_day: 300,
            ..SyntheticConfig::default()
        };
        s.insert_batch(&generate(&cfg, at)).unwrap();
        (create_analytics_router(AnalyticsHandle::new(s)), at)
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Vec<u8>, String) {
        let mut req = Request::builder().method(method).uri(uri);
        let body = match body {
            Some(v) => {
                req = req.header("content-type", "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = resp.status();
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default();
        let bytes = to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec();
        (status, bytes, ct)
    }

    fn j(b: &[u8]) -> Value {
        serde_json::from_slice(b).unwrap()
    }

    #[tokio::test]
    async fn summary_timeseries_breakdown_heatmap() {
        let (app, _) = seeded();
        let (st, b, _) = call(&app, "GET", "/analytics/summary", None).await;
        assert_eq!(st, StatusCode::OK);
        assert!(j(&b)["summary"]["count"].as_u64().unwrap() > 3_000);

        let (st, b, _) = call(
            &app,
            "GET",
            "/analytics/timeseries?granularity=daily&skills=pdf-extract",
            None,
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert!(j(&b)["data"]["series"].as_array().unwrap().len() >= 14);

        let (st, b, _) = call(
            &app,
            "GET",
            "/analytics/timeseries?granularity=weekly&group_by=agent",
            None,
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(j(&b)["data"]["groups"].as_object().unwrap().len(), 6);

        let (_, b, _) = call(&app, "GET", "/analytics/breakdown?by=skill", None).await;
        assert_eq!(j(&b)["groups"].as_array().unwrap().len(), 10);

        let (_, b, _) = call(&app, "GET", "/analytics/heatmap?dim.channel=slack", None).await;
        assert_eq!(j(&b)["cells"].as_array().unwrap().len(), 168);
    }

    #[tokio::test]
    async fn invalid_params_are_400() {
        let (app, _) = seeded();
        for uri in [
            "/analytics/summary?since=abc",
            "/analytics/timeseries?granularity=yearly",
            "/analytics/breakdown?by=planet",
            "/analytics/heatmap?dim.bad%20key=1",
            "/analytics/reports/nope",
            "/analytics/export?kind=zzz",
        ] {
            let (st, _, _) = call(&app, "GET", uri, None).await;
            assert_eq!(st, StatusCode::BAD_REQUEST, "{uri}");
        }
    }

    #[tokio::test]
    async fn ingest_and_list_events() {
        let app = create_analytics_router(AnalyticsHandle::in_memory());
        let ev = json!([
            {"ts": 1_794_787_200, "agent": "a", "skill": "s", "success": true, "latency_ms": 10},
            {"ts": 1_794_787_260, "agent": "a", "skill": "s", "success": false, "latency_ms": 20}
        ]);
        let (st, b, _) = call(&app, "POST", "/analytics/events", Some(ev)).await;
        assert_eq!(st, StatusCode::CREATED, "{}", String::from_utf8_lossy(&b));
        assert_eq!(j(&b)["inserted"], 2);
        let (_, b, _) = call(&app, "GET", "/analytics/events?success=false", None).await;
        let v = j(&b);
        assert_eq!(v["total"], 1);
        assert_eq!(v["events"][0]["latency_ms"], 20);
        let (_, b, _) = call(&app, "GET", "/analytics/dimensions", None).await;
        assert_eq!(j(&b)["skills"], json!(["s"]));
    }

    #[tokio::test]
    async fn export_csv_and_reports_pdf() {
        let (app, _) = seeded();
        let (st, b, ct) = call(
            &app,
            "GET",
            "/analytics/export?kind=events&agents=hermes&limit=50",
            None,
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert!(ct.starts_with("text/csv"));
        let csv = String::from_utf8(b).unwrap();
        assert!(csv.contains("agentes [hermes]"));
        assert!(csv.contains("id,timestamp,agent,skill,success,latency_ms"));

        let (_, b, _) = call(&app, "GET", "/analytics/reports", None).await;
        assert_eq!(j(&b)["reports"].as_array().unwrap().len(), 3);
        for id in [
            "weekly_bot_performance",
            "monthly_cost_analysis",
            "skill_evolution_summary",
        ] {
            let (st, b, ct) = call(
                &app,
                "GET",
                &format!("/analytics/reports/{id}?format=pdf"),
                None,
            )
            .await;
            assert_eq!(st, StatusCode::OK);
            assert_eq!(ct, "application/pdf");
            assert!(b.starts_with(b"%PDF-1.4"));
            let (st, b, _) = call(&app, "GET", &format!("/analytics/reports/{id}"), None).await;
            assert_eq!(st, StatusCode::OK);
            assert_eq!(j(&b)["id"], id);
        }
    }

    #[tokio::test]
    async fn recommendations_flag_low_success_skill() {
        let (app, _) = seeded();
        let (st, b, _) = call(&app, "GET", "/analytics/recommendations", None).await;
        assert_eq!(st, StatusCode::OK);
        let v = j(&b);
        let recs = v["recommendations"].as_array().unwrap();
        assert!(recs
            .iter()
            .any(|r| r["skill"] == "pdf-extract" && r["success_rate"].as_f64().unwrap() < 0.70));
    }

    #[tokio::test]
    async fn retention_endpoint_deletes_old() {
        let app = create_analytics_router(AnalyticsHandle::in_memory());
        let at = now();
        let ev = json!([
            {"ts": at - 100 * 86_400, "agent": "a", "skill": "s", "success": true, "latency_ms": 1},
            {"ts": at - 3_600, "agent": "a", "skill": "s", "success": true, "latency_ms": 1}
        ]);
        call(&app, "POST", "/analytics/events", Some(ev)).await;
        let (st, b, _) = call(&app, "POST", "/analytics/retention", None).await;
        assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
        assert_eq!(j(&b)["report"]["deleted"], 1);
        let (_, b, _) = call(&app, "GET", "/analytics/retention", None).await;
        let v = j(&b);
        assert_eq!(v["policy"]["delete_after_days"], 90);
        assert_eq!(v["hot_events"], 1);
        let bad = json!({"archive_after_days": 100, "delete_after_days": 90});
        let (st, _, _) = call(&app, "POST", "/analytics/retention", Some(bad)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
    }
}
