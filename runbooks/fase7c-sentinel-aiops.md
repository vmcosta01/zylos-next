# Fase 7C: Sentinel AIOps — Runbook Zylos-Next

## Visão Geral

O Sentinel AIOps system (`zylos-sentinel`) implementa health scanning com pure-function detectors, normalized incident signatures para deduplication, privacy-first output sanitization, JSONL append-only logging, rate-limited incident reporting (5 incidents/hour per signature), e EventBus integration para broadcasting.

**Módulos e tipos principais** (de `crates/zylos-sentinel/src/lib.rs`):
- `Severity` enum — Healthy, Warning, Critical
- `HealthCheck` struct — detector_id, target, severity, reason, detail, check_latency_ms
- `IncidentSignature` — SHA-256 normalized hash (detector:target:reason, first 16 hex chars)
- `Incident` struct — signature, detector_id, target, severity, reason, detail, first_seen, occurrence_count
- `IncidentRateLimiter` — min_interval based, tracks last_reported per signature
- `sanitize_for_output()` — Strips Bearer tokens, API keys, emails, passwords from text
- `run_health_scan()` — Pure-function detector execution

**Design principles** (do doc comment `lib.rs:1-15`):
- Pure-function health detectors producing normalized, privacy-safe signatures
- Separate output directory for scan results
- Tokio interval scheduling (120s default)
- Privacy-first hashing of PII/IDs
- JSONL append-only logging for audit trail
- Rate limit: 5 incidents/hour per signature
- EventBus integration for incident broadcasting

**Critério de Saída:** Health scans rodam a cada 120s; incidents são deduplicated por signature; PII nunca aparece em logs; rate limiter previne alert storms.

## Pré-requisitos

- Fases 0-7B completas
- Dependências: `tokio`, `serde`, `sha2`, `hex`, `chrono`, `tracing`
- EventBus disponível (de `zylos-live-comm`) para broadcasting
- Output directory configurado para scan results
- Detectors registrados como pure functions

### Setup

```bash
cargo build -p zylos-sentinel
cargo test -p zylos-sentinel
```

## Para Dev Senior

### Decisões Arquiteturais

1. **Pure-Function Detectors**: Cada detector é uma função pura `Fn() -> HealthCheck`. Sem side effects, sem I/O interno (I/O é feito no setup, detector apenas avalia). Isso permite testing determinístico e composição.

2. **Normalized Incident Signatures**: `IncidentSignature::new(detector_id, target, reason)` computa SHA-256 hash dos três campos concatenados, pega primeiros 16 hex chars. Mesma causa raiz → mesma assinatura → supressão de duplicatas. Variable data (timestamps, IPs, request IDs) é stripped antes do hash.

3. **Privacy-First Sanitization**: `sanitize_for_output()` é aplicado ANTES de qualquer logging ou reporting. Strip patterns: Bearer tokens, API keys (`api_key=`, `apikey=`, `token=`, `secret=`, `password=`), email addresses. Substitui por `[REDACTED]` ou `[EMAIL_REDACTED]`.

4. **JSONL Append-Only Logging**: Audit trail em formato JSONL (um JSON por linha). Append-only garante integridade. Cada entrada tem timestamp, detector_id, severity, signature. Facilita parsing por ferramentas externas.

5. **Rate-Limited Incident Reporting**: `IncidentRateLimiter` enforces min_interval entre reports da mesma signature. Default: 5 incidents/hour (12 min interval). Previne alert storms durante cascading failures. `reset()` após recovery permite re-report imediato.

6. **EventBus Integration**: Incidents são broadcast via EventBus para consumers (cockpit, notification services, SOAR). Consumers subscribe ao topic `sentinel.incidents.*`.

7. **Tokio Interval Scheduling**: Scan loop usa `tokio::time::interval(120s)`. Configurable. Graceful shutdown via CancellationToken.

### Extension Points

- **Novos detectors**: Implemente `Fn() -> HealthCheck` e registre no scan loop
- **Custom sanitization rules**: Estenda `sanitize_for_output()` com novos patterns
- **Novos severity levels**: Adicione ao enum (cuidado: breaking change)
- **Custom rate limit strategies**: Implemente sliding window ou adaptive rate limiting
- **External integrations**: Conecte incidents a PagerDuty, Slack, OpsGenie via webhook
- **Historical analysis**: Query JSONL logs para trend detection e anomaly scoring

### Performance Tuning

- Health scan completo <1s para 50 detectors
- Signature computation <10μs (SHA-256 de strings curtas)
- Sanitization <100μs por KB de texto
- Rate limiter lookup O(1) via HashMap
- JSONL write <1ms (buffered, fsync opcional)
- Parallelize detectors independentes via `tokio::spawn`

## Para Dev Pleno

### Writing a Health Detector

```rust
use zylos_sentinel::{HealthCheck, Severity};

fn check_disk_usage() -> HealthCheck {
    let start = std::time::Instant::now();
    
    // Pure evaluation — I/O done externally or cached
    let usage_percent = get_cached_disk_usage(); // Pre-fetched
    
    let (severity, reason) = if usage_percent > 95 {
        (Severity::Critical, "disk_usage_critical")
    } else if usage_percent > 85 {
        (Severity::Warning, "disk_usage_high")
    } else {
        (Severity::Healthy, "disk_usage_normal")
    };
    
    HealthCheck {
        detector_id: "disk_usage".into(),
        target: "/data".into(),
        severity,
        reason: reason.into(),
        detail: Some(format!("{:.1}% used", usage_percent)),
        check_latency_ms: start.elapsed().as_millis() as u64,
    }
}
```

### Running Health Scans

```rust
use zylos_sentinel::run_health_scan;

let detectors: Vec<(String, fn() -> HealthCheck)> = vec![
    ("disk_usage".into(), check_disk_usage),
    ("http_endpoint".into(), check_api_health),
    ("process_alive".into(), check_worker_process),
];

let checks = run_health_scan(&detectors);

for check in &checks {
    if check.severity != Severity::Healthy {
        tracing::warn!(
            detector = %check.detector_id,
            target = %check.target,
            severity = %check.severity,
            reason = %check.reason,
            "Health check failed"
        );
    }
}
```

### Incident Rate Limiting

```rust
use zylos_sentinel::{IncidentRateLimiter, IncidentSignature};
use std::time::Duration;

let mut limiter = IncidentRateLimiter::new(Duration::from_secs(720)); // 12 min = 5/hr

let sig = IncidentSignature::new("http_endpoint", "api.example.com", "connection_timeout");

if limiter.should_report(&sig) {
    // Report incident via EventBus
    bus.publish(incident_event).await?;
} else {
    tracing::debug!(signature = %sig.0, "Incident suppressed by rate limiter");
}

// After recovery, reset to allow immediate re-report
limiter.reset(&sig);
```

### Privacy Sanitization

```rust
use zylos_sentinel::sanitize_for_output;

let raw_log = "User admin@example.com sent Bearer eyJhbGciOi... with api_key=sk_live_abc123";
let safe = sanitize_for_output(raw_log);
// "User [EMAIL_REDACTED] sent Bearer [REDACTED] with api_key=[REDACTED]"

// ALWAYS sanitize before logging
tracing::info!("{}", sanitize_for_output(&response_body));
```

### JSONL Logging

```rust
use std::fs::OpenOptions;
use std::io::Write;

let file = OpenOptions::new()
    .create(true)
    .append(true)
    .open("sentinel/incidents.jsonl")?;

let entry = serde_json::json!({
    "timestamp": chrono::Utc::now().to_rfc3339(),
    "detector_id": "http_endpoint",
    "target": "api.example.com",
    "severity": "critical",
    "signature": "a1b2c3d4e5f67890",
    "reason": "connection_timeout",
    "occurrence_count": 3,
});

writeln!(file, "{}", entry.to_string())?;
```

## Para Dev Junior

### Passo a Passo: Adicionando um Novo Detector

1. **Defina a função detector**: `fn check_xxx() -> HealthCheck`
2. **Seja puro**: Não faça I/O dentro do detector; use dados pré-carregados ou cached
3. **Meça latência**: Sempre popule `check_latency_ms`
4. **Normalize targets**: Remova variable data (IPs, timestamps) do target string
5. **Registre no scan loop**: Adicione ao vetor de detectors
6. **Teste**: Mock inputs, verifique todos os severity levels

### Pitfalls Comuns

- **Nunca logue dados não sanitizados**: Sempre passe por `sanitize_for_output()` primeiro
- **Não inclua timestamps na signature**: Eles mudam a cada scan, quebrando dedup
- **Não faça I/O pesado dentro do detector**: Move para setup phase ou cache
- **Não ignore rate limiter**: Alert storms pioram incidentes
- **Não esqueça de resetar após recovery**: Senão o próximo incidente real fica suprimido
- **Não misture severity levels**: Healthy ≠ Warning ≠ Critical; seja consistente

### Debugging Tips

- JSONL logs são grep-friendly: `grep '"severity":"critical"' sentinel/incidents.jsonl`
- `IncidentRateLimiter.tracked_count()` mostra quantas signatures estão sendo rastreadas
- Health check latency ajuda a identificar detectors lentos
- Use `RUST_LOG=zylos_sentinel=debug` para ver scan cycles e rate limit decisions
- Sanitization é idempotente: aplicar múltiplas vezes é seguro

## Troubleshooting

| Erro | Causa | Recuperação |
|------|-------|-------------|
| Incidents duplicados | Signature não normalizada corretamente | Verifique que variable data é stripped antes do hash |
| Alert storm | Rate limiter desabilitado ou interval muito curto | Configure min_interval adequado (default 12min) |
| PII em logs | `sanitize_for_output()` não chamado | Adicione sanitization em todo log point |
| Detector timeout | I/O pesado dentro do detector | Move I/O para setup, use cache |
| JSONL corruption | Write não atômico | Use buffered writer com fsync, rotate files |
| False positives | Threshold muito sensível | Ajuste thresholds, adicione hysteresis |
| Missed incidents | Scan interval muito longo | Reduza interval ou adicione event-driven triggers |
| Memory growth | Rate limiter HashMap sem cleanup | Implemente TTL eviction ou periodic pruning |

## Referências

- **Source**: `crates/zylos-sentinel/src/lib.rs` (596 lines, single-module crate)
- **Key types**: `Severity`, `HealthCheck`, `IncidentSignature`, `Incident`, `IncidentRateLimiter`, `sanitize_for_output()`, `run_health_scan()`
- **Master Plan**: Fase 7C sentinel/AIOps section
- **Inspirações**: OpenDuet `sentinel.ts`, `health-check.ts`, `incident-rate-limiter.ts`
- **Integration**: EventBus de `zylos-live-comm` para broadcasting