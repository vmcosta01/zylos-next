//! Runtime do Sentinel no gateway (T11.7.2 / T11.7.4 / T11.7.5 / T11.7.6).
//!
//! - Middleware [`track_requests`] mede latência, 5xx e requisições em voo
//!   (fila) de todas as rotas.
//! - [`SentinelRuntime::tick`] (a cada `SENTINEL_TICK_SECS`, padrão 5 s) fecha a
//!   janela de métricas, roda detecção Z-score + regras YAML, propõe
//!   remediações no gate HITL e envia tudo ao Axiom (dataset
//!   `AXIOM_SENTINEL_DATASET`, fallback `AXIOM_DATASET`).
//! - Eventos de remediação são encaminhados ao `SoarOrchestrator`.
//!
//! Rotas (sob `/api/v1`):
//! - `GET  /sentinel/metrics` — snapshot atual, séries (ring buffer), anomalias, alertas
//! - `GET  /sentinel/remediations` — propostas + auditoria + modo do gate
//! - `POST /sentinel/remediations/:id/approve` — `{ "approver": "..." }`
//! - `POST /sentinel/remediations/:id/reject` — `{ "approver": "...", "reason": "..." }`
//!
//! Segurança: o modo do gate vem de `SENTINEL_REMEDIATION_MODE` com padrão
//! `dry_run`; o executor padrão ([`LogOnlyExecutor`]) não toca infraestrutura —
//! restart/scale reais exigem plugar um executor explícito.

use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use zylos_sentinel::alert_rules::{AlertRulesEngine, TriggeredAlert};
use zylos_sentinel::metrics::{
    Anomaly, BaselineDetector, MetricHistory, MetricSnapshot, MetricsCollector, ProcfsProbe,
    SystemProbe,
};
use zylos_sentinel::remediation::{
    GateConfig, GateMode, PendingRemediation, RemediationAction, RemediationAuditEntry,
    RemediationExecutor, RemediationGate, SoarRemediationEvent,
};

/// Regras padrão (usadas quando `SENTINEL_RULES_PATH` não está definido).
pub const DEFAULT_RULES_YAML: &str = r#"
- name: cpu-spike
  description: "CPU acima de 80%"
  metric: cpu_percent
  operator: ">"
  threshold: 80.0
  window_minutes: 1
  severity: SEV-2
  cooldown_minutes: 5
  remediation:
    action: restart-gateway
    gated: true
- name: memory-critical
  description: "Memória acima de 90%"
  metric: memory_percent
  operator: ">"
  threshold: 90.0
  severity: SEV-2
  cooldown_minutes: 10
  remediation:
    action: notify-oncall
    gated: false
    max_auto_executions: 2
- name: error-rate-high
  description: "Taxa de erro 5xx acima de 1%"
  metric: error_rate_percent
  operator: ">"
  threshold: 1.0
  window_minutes: 1
  severity: SEV-1
  cooldown_minutes: 5
  remediation:
    action: notify-oncall:pagerduty
    gated: false
    max_auto_executions: 2
- name: latency-degraded
  description: "Latência p95 acima de 1500 ms"
  metric: latency_p95_ms
  operator: ">"
  threshold: 1500.0
  severity: SEV-3
  cooldown_minutes: 15
  remediation:
    action: scale:gateway:3
    gated: true
- name: queue-backlog
  description: "Mais de 200 requisições em voo"
  metric: queue_depth
  operator: ">"
  threshold: 200.0
  severity: SEV-3
  cooldown_minutes: 15
  remediation:
    action: scale:gateway:4
    gated: true
"#;

const MAX_RECENT: usize = 100;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Destino das métricas/eventos do Sentinel (Axiom em produção).
pub trait MetricsSink: Send + Sync {
    /// Envia um lote de eventos JSON. Não deve bloquear.
    fn push(&self, events: Vec<Value>);
}

/// Sink que descarta (Axiom não configurado).
#[derive(Debug, Default)]
pub struct NoopSink;
impl MetricsSink for NoopSink {
    fn push(&self, _events: Vec<Value>) {}
}

/// Sink que guarda eventos em memória (testes).
#[derive(Debug, Default)]
pub struct MemorySink(pub Mutex<Vec<Value>>);
impl MetricsSink for MemorySink {
    fn push(&self, events: Vec<Value>) {
        if let Ok(mut v) = self.0.lock() {
            v.extend(events);
        }
    }
}

/// Sink HTTP para a ingest API do Axiom (`POST {endpoint}/{dataset}/ingest`).
pub struct AxiomMetricsSink {
    tx: tokio::sync::mpsc::Sender<Vec<Value>>,
}

impl AxiomMetricsSink {
    /// Cria a partir das env vars (`AXIOM_TOKEN`, `AXIOM_SENTINEL_DATASET` ou
    /// `AXIOM_DATASET`, `AXIOM_ENDPOINT`). `None` se não configurado.
    pub fn from_env() -> Option<Self> {
        let mut cfg = crate::axiom_logging::AxiomConfig::from_env()?;
        if let Ok(ds) = std::env::var("AXIOM_SENTINEL_DATASET") {
            if !ds.is_empty() {
                cfg.dataset = ds;
            }
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .ok()?;
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<Value>>(256);
        let url = format!("{}/{}/ingest", cfg.endpoint, cfg.dataset);
        let token = cfg.token.clone();
        tokio::spawn(async move {
            while let Some(batch) = rx.recv().await {
                if batch.is_empty() {
                    continue;
                }
                match client
                    .post(&url)
                    .bearer_auth(&token)
                    .json(&batch)
                    .send()
                    .await
                {
                    Ok(r) if r.status().is_success() => {}
                    Ok(r) => warn!(status = %r.status(), "Axiom sentinel ingest falhou"),
                    Err(e) => warn!(error = %e, "Axiom sentinel ingest erro de rede"),
                }
            }
        });
        info!(dataset = %cfg.dataset, "Sentinel → Axiom ingest habilitado");
        Some(Self { tx })
    }
}

impl MetricsSink for AxiomMetricsSink {
    fn push(&self, events: Vec<Value>) {
        if self.tx.try_send(events).is_err() {
            warn!("buffer Axiom do Sentinel cheio; lote descartado");
        }
    }
}

/// Converte um tick em eventos Axiom (um por snapshot/anomalia/alerta/remediação).
pub fn axiom_events(
    snap: &MetricSnapshot,
    anomalies: &[Anomaly],
    alerts: &[TriggeredAlert],
    remediations: &[PendingRemediation],
) -> Vec<Value> {
    let time = (snap.timestamp_ms / 1000).to_string();
    let mut out = Vec::with_capacity(1 + anomalies.len() + alerts.len() + remediations.len());
    out.push(json!({
        "_time": time, "service": "zylos-gateway", "kind": "sentinel_metric",
        "cpu_percent": snap.cpu_percent, "memory_percent": snap.memory_percent,
        "latency_p95_ms": snap.latency_p95_ms, "error_rate_percent": snap.error_rate_percent,
        "queue_depth": snap.queue_depth, "request_count": snap.request_count,
    }));
    for a in anomalies {
        out.push(json!({
            "_time": time, "service": "zylos-gateway", "kind": "sentinel_anomaly",
            "metric": a.metric, "value": a.value, "z_score": a.z_score,
            "baseline_mean": a.baseline_mean, "baseline_stddev": a.baseline_stddev,
        }));
    }
    for a in alerts {
        out.push(json!({
            "_time": time, "service": "zylos-gateway", "kind": "sentinel_alert",
            "rule_name": a.rule_name, "severity": a.severity, "value": a.current_value,
            "threshold": a.threshold, "confirmed": Value::Null,
        }));
    }
    for r in remediations {
        out.push(json!({
            "_time": time, "service": "zylos-gateway", "kind": "sentinel_remediation",
            "remediation_id": r.id, "rule_name": r.rule_name, "action": r.action.label(),
            "status": r.status,
        }));
    }
    out
}

/// Executor padrão: apenas registra no log. Nunca toca infraestrutura.
#[derive(Debug, Default)]
pub struct LogOnlyExecutor;
impl RemediationExecutor for LogOnlyExecutor {
    fn execute(&self, action: &RemediationAction) -> Result<String, String> {
        info!(action = %action.label(), "remediação aprovada (LogOnlyExecutor — sem efeito em infra)");
        Ok(format!(
            "registrado (LogOnlyExecutor): {}; configure um executor de infra para efeito real",
            action.label()
        ))
    }
}

/// Destino dos eventos SOAR (abstrai o orquestrador para testes).
pub trait SoarForwarder: Send + Sync {
    /// Encaminha eventos de remediação.
    fn forward(&self, events: Vec<SoarRemediationEvent>);
}

/// Encaminha ao `SoarOrchestrator` real (assíncrono, fire-and-forget).
pub struct OrchestratorForwarder(pub Arc<zylos_soar::orchestrator::SoarOrchestrator>);

/// Converte evento do Sentinel para o tipo do SOAR.
pub fn to_soar_event(ev: SoarRemediationEvent) -> zylos_soar::ZylosSecurityEvent {
    let severity: zylos_soar::Severity =
        serde_json::from_value(Value::String(ev.severity)).unwrap_or(zylos_soar::Severity::Medium);
    zylos_soar::ZylosSecurityEvent::new(
        ev.source,
        severity,
        zylos_soar::EventCategory::Anomaly,
        ev.title,
        ev.description,
        ev.affected_assets,
        ev.raw_payload,
    )
}

impl SoarForwarder for OrchestratorForwarder {
    fn forward(&self, events: Vec<SoarRemediationEvent>) {
        for ev in events {
            let orch = self.0.clone();
            let event = to_soar_event(ev);
            tokio::spawn(async move {
                if let Err(e) = orch.process_event(event).await {
                    warn!(error = %e, "SOAR falhou ao processar evento do Sentinel");
                }
            });
        }
    }
}

/// Encaminhador que guarda eventos (testes / SOAR ausente).
#[derive(Debug, Default)]
pub struct MemoryForwarder(pub Mutex<Vec<SoarRemediationEvent>>);
impl SoarForwarder for MemoryForwarder {
    fn forward(&self, events: Vec<SoarRemediationEvent>) {
        if let Ok(mut v) = self.0.lock() {
            v.extend(events);
        }
    }
}

struct Inner {
    collector: MetricsCollector,
    detector: BaselineDetector,
    history: MetricHistory,
    engine: AlertRulesEngine,
    gate: RemediationGate,
    last: Option<MetricSnapshot>,
    anomalies: VecDeque<Anomaly>,
    alerts: VecDeque<TriggeredAlert>,
}

/// Estado compartilhado do Sentinel no gateway.
pub struct SentinelRuntime {
    inner: Mutex<Inner>,
    in_flight: AtomicI64,
    sink: Box<dyn MetricsSink>,
    soar: Box<dyn SoarForwarder>,
    /// Barramento SSE: cada alerta disparado vira um aviso `SentinelAlert` no cockpit.
    bus: Option<zylos_live_comm::EventBus>,
}

/// Handle compartilhado.
pub type SentinelRuntimeHandle = Arc<SentinelRuntime>;

impl SentinelRuntime {
    /// Construtor explícito (testes).
    pub fn new(
        probe: Box<dyn SystemProbe>,
        engine: AlertRulesEngine,
        gate: RemediationGate,
        sink: Box<dyn MetricsSink>,
        soar: Box<dyn SoarForwarder>,
    ) -> Self {
        Self {
            inner: Mutex::new(Inner {
                collector: MetricsCollector::new(probe),
                detector: BaselineDetector::default(),
                history: MetricHistory::new(120),
                engine,
                gate,
                last: None,
                anomalies: VecDeque::new(),
                alerts: VecDeque::new(),
            }),
            in_flight: AtomicI64::new(0),
            sink,
            soar,
            bus: None,
        }
    }

    /// Liga o barramento de eventos para publicar `SentinelAlert` a cada alerta disparado.
    pub fn with_event_bus(mut self, bus: zylos_live_comm::EventBus) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Publica um aviso no SSE (ignora silenciosamente fora de um runtime tokio, como em testes sync).
    fn publish_alerts(&self, messages: Vec<String>) {
        let Some(bus) = self.bus.clone() else { return };
        if messages.is_empty() {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        handle.spawn(async move {
            for message in messages {
                let event = zylos_live_comm::ZylosEvent::Custom(zylos_live_comm::CustomEvent {
                    name: "SentinelAlert".to_string(),
                    payload: json!({ "message": message }),
                });
                if let Err(e) = bus.publish("sentinel.alert", event, 1).await {
                    warn!(error = %e, "falha ao publicar SentinelAlert");
                }
            }
        });
    }

    /// Constrói a partir do ambiente (gateway real).
    pub fn from_env(soar: Box<dyn SoarForwarder>) -> Self {
        let engine = std::env::var("SENTINEL_RULES_PATH")
            .ok()
            .and_then(|p| match std::fs::read_to_string(&p) {
                Ok(c) => match AlertRulesEngine::load_from_yaml(&c) {
                    Ok(e) => Some(e),
                    Err(err) => {
                        warn!(path = %p, error = %err, "regras do Sentinel inválidas; usando padrão");
                        None
                    }
                },
                Err(err) => {
                    warn!(path = %p, error = %err, "falha lendo regras do Sentinel; usando padrão");
                    None
                }
            })
            .unwrap_or_else(|| {
                AlertRulesEngine::load_from_yaml(DEFAULT_RULES_YAML).unwrap_or_default()
            });
        let mode =
            GateMode::from_env_value(std::env::var("SENTINEL_REMEDIATION_MODE").ok().as_deref());
        info!(
            ?mode,
            rules = engine.rule_count(),
            "Sentinel runtime inicializado"
        );
        let gate = RemediationGate::new(
            GateConfig {
                mode,
                ..GateConfig::default()
            },
            Box::new(LogOnlyExecutor),
        );
        let sink: Box<dyn MetricsSink> = match AxiomMetricsSink::from_env() {
            Some(s) => Box::new(s),
            None => Box::new(NoopSink),
        };
        Self::new(Box::new(ProcfsProbe::default()), engine, gate, sink, soar)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Registra requisição concluída.
    pub fn record_request(&self, latency_ms: f64, is_error: bool) {
        self.lock().collector.record_request(latency_ms, is_error);
    }

    /// Executa um ciclo de coleta/detecção. Retorna `(anomalias, alertas, propostas)`.
    pub fn tick(&self, timestamp_ms: u64) -> (usize, usize, usize) {
        let now_secs = timestamp_ms / 1000;
        let depth = self.in_flight.load(Ordering::Relaxed).max(0) as f64;
        let (events, soar_events, counts, alert_messages) = {
            let mut g = self.lock();
            let inner = &mut *g;
            inner.collector.set_queue_depth(depth);
            let snap = inner.collector.snapshot(timestamp_ms);
            let anomalies = inner.detector.observe_snapshot(&snap);
            inner.history.push(&snap, &anomalies);
            let alerts = inner.engine.evaluate(&snap.to_map());
            let mut proposals = Vec::new();
            for a in &alerts {
                match inner.gate.propose(&inner.engine, a, now_secs) {
                    Ok(Some(p)) => proposals.push(p),
                    Ok(None) => {}
                    Err(e) => warn!(rule = %a.rule_name, error = %e, "remediação inválida"),
                }
            }
            let events = axiom_events(&snap, &anomalies, &alerts, &proposals);
            for a in &anomalies {
                inner.anomalies.push_back(a.clone());
            }
            for a in &alerts {
                inner.alerts.push_back(a.clone());
            }
            while inner.anomalies.len() > MAX_RECENT {
                inner.anomalies.pop_front();
            }
            while inner.alerts.len() > MAX_RECENT {
                inner.alerts.pop_front();
            }
            inner.last = Some(snap);
            let counts = (anomalies.len(), alerts.len(), proposals.len());
            let alert_messages: Vec<String> = alerts
                .iter()
                .map(|a| {
                    format!(
                        "{}: valor {:.1}, limite {:.1}",
                        a.rule_name, a.current_value, a.threshold
                    )
                })
                .collect();
            (
                events,
                inner.gate.drain_soar_events(),
                counts,
                alert_messages,
            )
        };
        self.publish_alerts(alert_messages);
        self.sink.push(events);
        if !soar_events.is_empty() {
            self.soar.forward(soar_events);
        }
        counts
    }

    /// Payload de `GET /sentinel/metrics`.
    pub fn metrics_view(&self) -> Value {
        let g = self.lock();
        json!({
            "current": g.last,
            "series": g.history.to_map(),
            "anomalies": g.anomalies.iter().rev().take(20).collect::<Vec<_>>(),
            "alerts": g.alerts.iter().rev().take(20).collect::<Vec<_>>(),
            "remediation_mode": g.gate.mode(),
            "pending_remediations": g.gate.proposals().iter()
                .filter(|p| p.status == zylos_sentinel::remediation::RemediationStatus::PendingApproval)
                .count(),
            "rules": g.engine.rule_count(),
        })
    }

    /// Payload de `GET /sentinel/remediations`.
    pub fn remediations_view(&self) -> RemediationsView {
        let g = self.lock();
        RemediationsView {
            mode: g.gate.mode(),
            proposals: g.gate.proposals().to_vec(),
            audit: g.gate.audit().to_vec(),
        }
    }

    fn decide(
        &self,
        id: &str,
        body: &DecisionBody,
        approve: bool,
    ) -> Result<PendingRemediation, String> {
        let now = now_ms() / 1000;
        let (res, soar_events) = {
            let mut g = self.lock();
            let res = if approve {
                g.gate.approve(id, &body.approver, now)
            } else {
                g.gate.reject(
                    id,
                    &body.approver,
                    body.reason.as_deref().unwrap_or(""),
                    now,
                )
            };
            (res, g.gate.drain_soar_events())
        };
        if !soar_events.is_empty() {
            self.soar.forward(soar_events);
        }
        res
    }

    /// Spawna o loop periódico de ticks.
    pub fn spawn_loop(self: &Arc<Self>, every: Duration) {
        let rt = Arc::clone(self);
        tokio::spawn(async move {
            let mut iv = tokio::time::interval(every);
            loop {
                iv.tick().await;
                rt.tick(now_ms());
            }
        });
    }
}

/// Resposta de `GET /sentinel/remediations`.
#[derive(Debug, Serialize)]
pub struct RemediationsView {
    /// Modo do gate.
    pub mode: GateMode,
    /// Propostas.
    pub proposals: Vec<PendingRemediation>,
    /// Auditoria.
    pub audit: Vec<RemediationAuditEntry>,
}

/// Corpo de aprovação/rejeição.
#[derive(Debug, Deserialize)]
pub struct DecisionBody {
    /// Quem decide (obrigatório).
    pub approver: String,
    /// Motivo (rejeição).
    #[serde(default)]
    pub reason: Option<String>,
}

/// Middleware que alimenta o coletor (latência, 5xx, fila).
pub async fn track_requests(
    State(rt): State<SentinelRuntimeHandle>,
    req: Request,
    next: Next,
) -> Response {
    rt.in_flight.fetch_add(1, Ordering::Relaxed);
    let start = Instant::now();
    let resp = next.run(req).await;
    rt.in_flight.fetch_sub(1, Ordering::Relaxed);
    rt.record_request(
        start.elapsed().as_secs_f64() * 1000.0,
        resp.status().is_server_error(),
    );
    resp
}

async fn get_metrics(State(rt): State<SentinelRuntimeHandle>) -> Json<Value> {
    Json(rt.metrics_view())
}

async fn get_remediations(State(rt): State<SentinelRuntimeHandle>) -> Json<RemediationsView> {
    Json(rt.remediations_view())
}

fn decision_response(r: Result<PendingRemediation, String>) -> Response {
    match r {
        Ok(p) => Json(p).into_response(),
        Err(e) if e.contains("não encontrada") => {
            (StatusCode::NOT_FOUND, Json(json!({ "error": e }))).into_response()
        }
        Err(e) if e.contains("obrigatório") => {
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response()
        }
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e }))).into_response(),
    }
}

async fn approve(
    State(rt): State<SentinelRuntimeHandle>,
    Path(id): Path<String>,
    Json(body): Json<DecisionBody>,
) -> Response {
    decision_response(rt.decide(&id, &body, true))
}

async fn reject(
    State(rt): State<SentinelRuntimeHandle>,
    Path(id): Path<String>,
    Json(body): Json<DecisionBody>,
) -> Response {
    decision_response(rt.decide(&id, &body, false))
}

/// Router do runtime do Sentinel.
pub fn create_sentinel_runtime_router(rt: SentinelRuntimeHandle) -> Router {
    Router::new()
        .route("/sentinel/metrics", get(get_metrics))
        .route("/sentinel/remediations", get(get_remediations))
        .route("/sentinel/remediations/:id/approve", post(approve))
        .route("/sentinel/remediations/:id/reject", post(reject))
        .with_state(rt)
}

/// Intervalo do tick (`SENTINEL_TICK_SECS`, padrão 5 s, mínimo 1 s).
pub fn tick_interval_from_env() -> Duration {
    let secs = std::env::var("SENTINEL_TICK_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5)
        .max(1);
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use tower::ServiceExt;
    use zylos_sentinel::metrics::StaticProbe;
    use zylos_sentinel::remediation::{RecordingExecutor, RemediationStatus};

    struct SharedSink(Arc<MemorySink>);
    impl MetricsSink for SharedSink {
        fn push(&self, e: Vec<Value>) {
            self.0.push(e)
        }
    }
    struct SharedFwd(Arc<MemoryForwarder>);
    impl SoarForwarder for SharedFwd {
        fn forward(&self, e: Vec<SoarRemediationEvent>) {
            self.0.forward(e)
        }
    }

    fn runtime(
        cpu: f64,
        mode: GateMode,
    ) -> (SentinelRuntimeHandle, Arc<MemorySink>, Arc<MemoryForwarder>) {
        let sink = Arc::new(MemorySink::default());
        let fwd = Arc::new(MemoryForwarder::default());
        let rt = SentinelRuntime::new(
            Box::new(StaticProbe {
                cpu: Some(cpu),
                memory: Some(40.0),
            }),
            AlertRulesEngine::load_from_yaml(DEFAULT_RULES_YAML).unwrap(),
            RemediationGate::new(
                GateConfig {
                    mode,
                    approval_ttl_secs: u64::MAX / 4,
                },
                Box::new(RecordingExecutor::default()),
            ),
            Box::new(SharedSink(sink.clone())),
            Box::new(SharedFwd(fwd.clone())),
        );
        (Arc::new(rt), sink, fwd)
    }

    #[test]
    fn default_rules_parse() {
        let e = AlertRulesEngine::load_from_yaml(DEFAULT_RULES_YAML).unwrap();
        assert_eq!(e.rule_count(), 5);
    }

    #[test]
    fn tick_sends_axiom_events_and_soar() {
        let (rt, sink, fwd) = runtime(95.0, GateMode::RequireApproval);
        for _ in 0..50 {
            rt.record_request(10.0, false);
        }
        let (_, alerts, proposals) = rt.tick(10_000);
        assert_eq!(alerts, 1);
        assert_eq!(proposals, 1);
        let ev = sink.0.lock().unwrap().clone();
        assert!(ev
            .iter()
            .any(|e| e["kind"] == "sentinel_metric" && e["cpu_percent"] == 95.0));
        assert!(ev
            .iter()
            .any(|e| e["kind"] == "sentinel_alert" && e["rule_name"] == "cpu-spike"));
        assert!(ev
            .iter()
            .any(|e| e["kind"] == "sentinel_remediation" && e["status"] == "pending_approval"));
        let soar = fwd.0.lock().unwrap().clone();
        assert_eq!(soar.len(), 1);
        let converted = to_soar_event(soar[0].clone());
        assert_eq!(converted.severity, zylos_soar::Severity::High);
        assert_eq!(converted.category, zylos_soar::EventCategory::Anomaly);
    }

    #[tokio::test]
    async fn http_approve_flow_requires_approver() {
        let (rt, _, fwd) = runtime(95.0, GateMode::RequireApproval);
        rt.tick(now_ms());
        let app = create_sentinel_runtime_router(rt.clone());

        let resp = app
            .clone()
            .oneshot(
                HttpRequest::get("/sentinel/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["pending_remediations"], 1);
        assert_eq!(v["series"]["cpu_percent"].as_array().unwrap().len(), 1);

        let id = rt.remediations_view().proposals[0].id.clone();
        let bad = HttpRequest::post(format!("/sentinel/remediations/{id}/approve"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"approver":"  "}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(bad).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        let ok = HttpRequest::post(format!("/sentinel/remediations/{id}/approve"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"approver":"sre"}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(ok).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            rt.remediations_view().proposals[0].status,
            RemediationStatus::Executed
        );

        let again = HttpRequest::post(format!("/sentinel/remediations/{id}/approve"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"approver":"sre"}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(again).await.unwrap().status(),
            StatusCode::CONFLICT
        );

        let missing = HttpRequest::post("/sentinel/remediations/rem-999/reject")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"approver":"sre"}"#))
            .unwrap();
        assert_eq!(
            app.oneshot(missing).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            fwd.0.lock().unwrap().len(),
            2,
            "proposta + execução enviadas ao SOAR"
        );
    }

    #[test]
    fn dry_run_default_never_executes() {
        let (rt, _, _) = runtime(95.0, GateMode::DryRun);
        rt.tick(1_000);
        let v = rt.remediations_view();
        assert_eq!(v.proposals[0].status, RemediationStatus::DryRun);
        assert!(rt
            .decide(
                &v.proposals[0].id,
                &DecisionBody {
                    approver: "x".into(),
                    reason: None
                },
                true
            )
            .is_err());
    }

    #[tokio::test]
    async fn middleware_records_errors_and_latency() {
        let (rt, _, _) = runtime(10.0, GateMode::DryRun);
        let app = Router::new()
            .route("/ok", get(|| async { "ok" }))
            .route("/boom", get(|| async { StatusCode::INTERNAL_SERVER_ERROR }))
            .layer(axum::middleware::from_fn_with_state(
                rt.clone(),
                track_requests,
            ));
        for path in ["/ok", "/ok", "/ok", "/boom"] {
            let _ = app
                .clone()
                .oneshot(HttpRequest::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
        }
        rt.tick(5_000);
        let v = rt.metrics_view();
        assert_eq!(v["current"]["request_count"], 4);
        assert_eq!(v["current"]["error_rate_percent"], 25.0);
        assert_eq!(v["current"]["queue_depth"], 0.0);
    }
}
