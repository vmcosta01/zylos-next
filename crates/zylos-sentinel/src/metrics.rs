//! Coleta de métricas do Sentinel (T11.7.2).
//!
//! Cinco métricas canônicas, todas com chave estável usada pelo
//! [`AlertRulesEngine`](crate::alert_rules::AlertRulesEngine):
//!
//! | Métrica            | Chave                | Fonte                                       |
//! |--------------------|----------------------|---------------------------------------------|
//! | CPU                | `cpu_percent`        | [`SystemProbe`] (procfs no Linux)           |
//! | Memória            | `memory_percent`     | [`SystemProbe`] (procfs no Linux)           |
//! | Latência (p95)     | `latency_p95_ms`     | [`MetricsCollector::record_request`]        |
//! | Taxa de erro       | `error_rate_percent` | [`MetricsCollector::record_request`]        |
//! | Profundidade de fila | `queue_depth`      | [`MetricsCollector::set_queue_depth`]       |
//!
//! Também fornece detecção estatística de anomalias por baseline + desvio
//! (Z-score, ADR-015 — métodos estatísticos antes de ML) e um histórico em
//! ring buffer para os gráficos do cockpit.
//!
//! Tudo aqui é síncrono e livre de I/O (exceto [`ProcfsProbe`]), para que o
//! gateway possa embrulhar em `Mutex` e os testes sejam determinísticos.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Métricas canônicas coletadas pelo Sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    /// Uso de CPU do host (0–100).
    CpuPercent,
    /// Uso de memória do host (0–100).
    MemoryPercent,
    /// Latência p95 das requisições na janela (ms).
    LatencyP95Ms,
    /// Percentual de respostas com erro (5xx) na janela (0–100).
    ErrorRatePercent,
    /// Requisições em voo / itens enfileirados.
    QueueDepth,
}

impl MetricKind {
    /// Todas as métricas, em ordem estável (usada pelo dashboard).
    pub const ALL: [MetricKind; 5] = [
        MetricKind::CpuPercent,
        MetricKind::MemoryPercent,
        MetricKind::LatencyP95Ms,
        MetricKind::ErrorRatePercent,
        MetricKind::QueueDepth,
    ];

    /// Chave usada no mapa de métricas avaliado pelas regras YAML.
    pub fn key(&self) -> &'static str {
        match self {
            Self::CpuPercent => "cpu_percent",
            Self::MemoryPercent => "memory_percent",
            Self::LatencyP95Ms => "latency_p95_ms",
            Self::ErrorRatePercent => "error_rate_percent",
            Self::QueueDepth => "queue_depth",
        }
    }

    /// Converte a chave textual de volta para o enum.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.key() == key)
    }
}

/// Fotografia das métricas em um instante. Campos `None` = fonte indisponível
/// (ex.: CPU em host sem procfs) — nunca inventamos valor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetricSnapshot {
    /// Timestamp Unix em milissegundos.
    pub timestamp_ms: u64,
    /// CPU (%).
    pub cpu_percent: Option<f64>,
    /// Memória (%).
    pub memory_percent: Option<f64>,
    /// Latência p95 (ms) na janela.
    pub latency_p95_ms: Option<f64>,
    /// Taxa de erro (%) na janela.
    pub error_rate_percent: Option<f64>,
    /// Profundidade de fila.
    pub queue_depth: Option<f64>,
    /// Quantidade de requisições observadas na janela.
    pub request_count: u64,
}

impl MetricSnapshot {
    /// Valor de uma métrica específica.
    pub fn get(&self, kind: MetricKind) -> Option<f64> {
        match kind {
            MetricKind::CpuPercent => self.cpu_percent,
            MetricKind::MemoryPercent => self.memory_percent,
            MetricKind::LatencyP95Ms => self.latency_p95_ms,
            MetricKind::ErrorRatePercent => self.error_rate_percent,
            MetricKind::QueueDepth => self.queue_depth,
        }
    }

    /// Mapa `chave → valor` (apenas métricas presentes) para o rules engine.
    pub fn to_map(&self) -> HashMap<String, f64> {
        MetricKind::ALL
            .into_iter()
            .filter_map(|k| self.get(k).map(|v| (k.key().to_string(), v)))
            .collect()
    }
}

/// Fonte de métricas de sistema (CPU/memória).
pub trait SystemProbe: Send {
    /// Uso de CPU (%) desde a última chamada, se disponível.
    fn cpu_percent(&mut self) -> Option<f64>;
    /// Uso de memória (%), se disponível.
    fn memory_percent(&mut self) -> Option<f64>;
}

/// Probe fixa — para testes e hosts sem fonte de sistema.
#[derive(Debug, Clone, Default)]
pub struct StaticProbe {
    /// Valor de CPU devolvido.
    pub cpu: Option<f64>,
    /// Valor de memória devolvido.
    pub memory: Option<f64>,
}

impl SystemProbe for StaticProbe {
    fn cpu_percent(&mut self) -> Option<f64> {
        self.cpu
    }
    fn memory_percent(&mut self) -> Option<f64> {
        self.memory
    }
}

/// Contadores agregados da linha `cpu` de `/proc/stat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    /// Ticks ociosos (idle + iowait).
    pub idle: u64,
    /// Ticks totais.
    pub total: u64,
}

/// Faz parse da primeira linha `cpu ...` de `/proc/stat`.
pub fn parse_proc_stat(content: &str) -> Option<CpuTimes> {
    let line = content.lines().find(|l| l.starts_with("cpu "))?;
    let vals: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    if vals.len() < 4 {
        return None;
    }
    let idle = vals[3] + vals.get(4).copied().unwrap_or(0);
    let total = vals.iter().sum();
    Some(CpuTimes { idle, total })
}

/// Uso de CPU (%) entre duas leituras de `/proc/stat`.
pub fn cpu_percent_between(prev: CpuTimes, cur: CpuTimes) -> Option<f64> {
    let total = cur.total.checked_sub(prev.total)?;
    let idle = cur.idle.checked_sub(prev.idle)?;
    if total == 0 {
        return None;
    }
    Some(((total - idle.min(total)) as f64 / total as f64) * 100.0)
}

/// Faz parse de `/proc/meminfo` e devolve uso de memória (%).
pub fn parse_meminfo_percent(content: &str) -> Option<f64> {
    let field = |name: &str| -> Option<f64> {
        content
            .lines()
            .find(|l| l.starts_with(name))?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    };
    let total = field("MemTotal:")?;
    let available = field("MemAvailable:")?;
    if total <= 0.0 {
        return None;
    }
    Some(((total - available) / total * 100.0).clamp(0.0, 100.0))
}

/// Probe baseada em procfs (Linux — produção em container). Em outros SOs
/// devolve `None` (a métrica fica ausente, não zerada).
#[derive(Debug, Default)]
pub struct ProcfsProbe {
    prev_cpu: Option<CpuTimes>,
}

impl SystemProbe for ProcfsProbe {
    fn cpu_percent(&mut self) -> Option<f64> {
        let content = std::fs::read_to_string("/proc/stat").ok()?;
        let cur = parse_proc_stat(&content)?;
        let prev = self.prev_cpu.replace(cur)?;
        cpu_percent_between(prev, cur)
    }

    fn memory_percent(&mut self) -> Option<f64> {
        let content = std::fs::read_to_string("/proc/meminfo").ok()?;
        parse_meminfo_percent(&content)
    }
}

/// Percentil `p` (0–100) por nearest-rank. Lista vazia → `None`.
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

/// Coletor de métricas de requisição + sistema.
///
/// Requisições são acumuladas na janela corrente; [`MetricsCollector::snapshot`]
/// fecha a janela e calcula p95 e taxa de erro.
pub struct MetricsCollector {
    probe: Box<dyn SystemProbe>,
    latencies: Vec<f64>,
    errors: u64,
    queue_depth: f64,
    max_samples: usize,
}

impl std::fmt::Debug for MetricsCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsCollector")
            .field("pending_requests", &self.latencies.len())
            .field("errors", &self.errors)
            .field("queue_depth", &self.queue_depth)
            .finish()
    }
}

impl MetricsCollector {
    /// Cria coletor com a probe informada.
    pub fn new(probe: Box<dyn SystemProbe>) -> Self {
        Self {
            probe,
            latencies: Vec::new(),
            errors: 0,
            queue_depth: 0.0,
            max_samples: 10_000,
        }
    }

    /// Registra uma requisição concluída.
    pub fn record_request(&mut self, latency_ms: f64, is_error: bool) {
        if self.latencies.len() < self.max_samples {
            self.latencies.push(latency_ms.max(0.0));
            if is_error {
                self.errors += 1;
            }
        }
    }

    /// Define a profundidade de fila atual.
    pub fn set_queue_depth(&mut self, depth: f64) {
        self.queue_depth = depth.max(0.0);
    }

    /// Fecha a janela e devolve a fotografia das métricas.
    pub fn snapshot(&mut self, timestamp_ms: u64) -> MetricSnapshot {
        let count = self.latencies.len() as u64;
        let (latency, error_rate) = if count == 0 {
            (None, None)
        } else {
            (
                percentile(&self.latencies, 95.0),
                Some(self.errors as f64 / count as f64 * 100.0),
            )
        };
        self.latencies.clear();
        self.errors = 0;
        MetricSnapshot {
            timestamp_ms,
            cpu_percent: self.probe.cpu_percent(),
            memory_percent: self.probe.memory_percent(),
            latency_p95_ms: latency,
            error_rate_percent: error_rate,
            queue_depth: Some(self.queue_depth),
            request_count: count,
        }
    }
}

/// Anomalia estatística detectada (baseline + desvio).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anomaly {
    /// Métrica afetada.
    pub metric: MetricKind,
    /// Valor observado.
    pub value: f64,
    /// Média do baseline.
    pub baseline_mean: f64,
    /// Desvio padrão do baseline.
    pub baseline_stddev: f64,
    /// Z-score do valor observado.
    pub z_score: f64,
    /// Timestamp (ms).
    pub timestamp_ms: u64,
}

/// Detector Z-score por métrica com janela deslizante de baseline.
#[derive(Debug, Clone)]
pub struct BaselineDetector {
    window: usize,
    min_samples: usize,
    z_threshold: f64,
    /// Desvio mínimo absoluto para evitar alarmes em séries quase constantes.
    min_stddev: f64,
    series: HashMap<MetricKind, VecDeque<f64>>,
}

impl Default for BaselineDetector {
    fn default() -> Self {
        Self::new(60, 10, 3.0)
    }
}

impl BaselineDetector {
    /// `window` amostras de baseline, `min_samples` antes de alertar, limiar `z_threshold`.
    pub fn new(window: usize, min_samples: usize, z_threshold: f64) -> Self {
        Self {
            window: window.max(2),
            min_samples: min_samples.max(2),
            z_threshold,
            min_stddev: 0.5,
            series: HashMap::new(),
        }
    }

    /// Observa todas as métricas presentes no snapshot.
    pub fn observe_snapshot(&mut self, snap: &MetricSnapshot) -> Vec<Anomaly> {
        MetricKind::ALL
            .into_iter()
            .filter_map(|k| {
                snap.get(k)
                    .and_then(|v| self.observe(k, v, snap.timestamp_ms))
            })
            .collect()
    }

    /// Observa um valor; devolve anomalia se |z| ≥ limiar. Valores anômalos
    /// não entram no baseline (evita "aprender" o incidente).
    pub fn observe(
        &mut self,
        metric: MetricKind,
        value: f64,
        timestamp_ms: u64,
    ) -> Option<Anomaly> {
        let series = self.series.entry(metric).or_default();
        let mut result = None;
        if series.len() >= self.min_samples {
            let n = series.len() as f64;
            let mean = series.iter().sum::<f64>() / n;
            let var = series.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
            let stddev = var.sqrt().max(self.min_stddev);
            let z = (value - mean) / stddev;
            if z.abs() >= self.z_threshold {
                result = Some(Anomaly {
                    metric,
                    value,
                    baseline_mean: mean,
                    baseline_stddev: stddev,
                    z_score: z,
                    timestamp_ms,
                });
            }
        }
        if result.is_none() {
            series.push_back(value);
            while series.len() > self.window {
                series.pop_front();
            }
        }
        result
    }
}

/// Ponto de série temporal para gráficos.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MetricPoint {
    /// Timestamp (ms).
    pub t: u64,
    /// Valor.
    pub v: f64,
    /// Se o ponto foi marcado como anomalia.
    pub anomaly: bool,
}

/// Histórico em ring buffer por métrica (alimenta o dashboard do cockpit).
#[derive(Debug, Clone)]
pub struct MetricHistory {
    capacity: usize,
    series: HashMap<MetricKind, VecDeque<MetricPoint>>,
}

impl MetricHistory {
    /// Cria histórico com `capacity` pontos por métrica.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            series: HashMap::new(),
        }
    }

    /// Adiciona snapshot, marcando pontos anômalos.
    pub fn push(&mut self, snap: &MetricSnapshot, anomalies: &[Anomaly]) {
        for kind in MetricKind::ALL {
            let Some(v) = snap.get(kind) else { continue };
            let anomaly = anomalies.iter().any(|a| a.metric == kind);
            let s = self.series.entry(kind).or_default();
            s.push_back(MetricPoint {
                t: snap.timestamp_ms,
                v,
                anomaly,
            });
            while s.len() > self.capacity {
                s.pop_front();
            }
        }
    }

    /// Série de uma métrica (mais antiga → mais nova).
    pub fn series(&self, kind: MetricKind) -> Vec<MetricPoint> {
        self.series
            .get(&kind)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Mapa `chave → série` serializável.
    pub fn to_map(&self) -> HashMap<String, Vec<MetricPoint>> {
        MetricKind::ALL
            .into_iter()
            .map(|k| (k.key().to_string(), self.series(k)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collector(cpu: Option<f64>, mem: Option<f64>) -> MetricsCollector {
        MetricsCollector::new(Box::new(StaticProbe { cpu, memory: mem }))
    }

    #[test]
    fn metric_keys_roundtrip() {
        for k in MetricKind::ALL {
            assert_eq!(MetricKind::from_key(k.key()), Some(k));
        }
        assert_eq!(MetricKind::from_key("nope"), None);
    }

    #[test]
    fn percentile_nearest_rank() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&v, 95.0), Some(95.0));
        assert_eq!(percentile(&v, 100.0), Some(100.0));
        assert_eq!(percentile(&[], 95.0), None);
        assert_eq!(percentile(&[7.0], 50.0), Some(7.0));
    }

    #[test]
    fn snapshot_computes_latency_and_error_rate() {
        let mut c = collector(Some(42.0), Some(60.0));
        for i in 0..99 {
            c.record_request(10.0 + i as f64, false);
        }
        c.record_request(500.0, true);
        c.set_queue_depth(3.0);
        let s = c.snapshot(1_000);
        assert_eq!(s.request_count, 100);
        assert_eq!(s.error_rate_percent, Some(1.0));
        assert_eq!(s.latency_p95_ms, Some(104.0));
        assert_eq!(s.cpu_percent, Some(42.0));
        assert_eq!(s.queue_depth, Some(3.0));
        let m = s.to_map();
        assert_eq!(m.len(), 5);
        assert_eq!(m["error_rate_percent"], 1.0);
    }

    #[test]
    fn snapshot_resets_window_and_omits_unknown() {
        let mut c = collector(None, None);
        c.record_request(5.0, true);
        let _ = c.snapshot(1);
        let s = c.snapshot(2);
        assert_eq!(s.request_count, 0);
        assert_eq!(s.latency_p95_ms, None);
        assert_eq!(s.error_rate_percent, None);
        assert_eq!(s.cpu_percent, None);
        // Apenas queue_depth presente.
        assert_eq!(s.to_map().len(), 1);
    }

    #[test]
    fn procfs_parsers() {
        let a = parse_proc_stat("cpu  100 0 100 700 100 0 0 0\ncpu0 1 2 3 4").unwrap();
        assert_eq!(
            a,
            CpuTimes {
                idle: 800,
                total: 1000
            }
        );
        let b = parse_proc_stat("cpu  200 0 200 750 150 0 0 0").unwrap();
        // delta total=300, idle=100 → 66.67%
        let pct = cpu_percent_between(a, b).unwrap();
        assert!((pct - 66.666).abs() < 0.01, "{pct}");
        assert!(parse_proc_stat("garbage").is_none());
        let mem = parse_meminfo_percent("MemTotal: 1000 kB\nMemFree: 1 kB\nMemAvailable: 250 kB\n");
        assert_eq!(mem, Some(75.0));
        assert!(parse_meminfo_percent("MemTotal: 1000 kB").is_none());
    }

    #[test]
    fn baseline_detector_flags_spike_not_noise() {
        let mut d = BaselineDetector::new(30, 10, 3.0);
        for i in 0..20u64 {
            let noise = if i % 2 == 0 { 1.0 } else { -1.0 };
            assert!(d.observe(MetricKind::CpuPercent, 30.0 + noise, i).is_none());
        }
        let a = d.observe(MetricKind::CpuPercent, 92.0, 21).expect("spike");
        assert!(a.z_score > 3.0);
        assert!((a.baseline_mean - 30.0).abs() < 0.5);
        // Spike não contamina baseline: próximo normal continua normal.
        assert!(d.observe(MetricKind::CpuPercent, 31.0, 22).is_none());
    }

    #[test]
    fn baseline_detector_needs_min_samples() {
        let mut d = BaselineDetector::new(30, 10, 3.0);
        for i in 0..5 {
            assert!(d.observe(MetricKind::LatencyP95Ms, 10.0, i).is_none());
        }
        assert!(d.observe(MetricKind::LatencyP95Ms, 10_000.0, 6).is_none());
    }

    #[test]
    fn history_ring_buffer_marks_anomalies() {
        let mut h = MetricHistory::new(3);
        let mut d = BaselineDetector::new(10, 2, 3.0);
        for t in 0..5u64 {
            let snap = MetricSnapshot {
                timestamp_ms: t,
                cpu_percent: Some(if t == 4 { 99.0 } else { 10.0 }),
                ..Default::default()
            };
            let anomalies = d.observe_snapshot(&snap);
            h.push(&snap, &anomalies);
        }
        let s = h.series(MetricKind::CpuPercent);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].t, 2);
        assert!(s[2].anomaly);
        assert!(!s[1].anomaly);
        assert!(h.series(MetricKind::QueueDepth).is_empty());
    }
}
