//! # AutoImprovementLoop — Aprendizado automático a partir de padrões de acesso (T8.2.4)
//!
//! Background task Tokio que monitora padrões de query no ContextGraph e
//! MentalModelStore, identificando oportunidades de consolidação e refresh.
//!
//! ## Arquitetura
//! - Coleta métricas de acesso via `AccessTracker` (lock-free counters)
//! - Analisa padrões periodicamente em background task
//! - Emite eventos via callback quando oportunidades são detectadas
//! - Zero LLM calls: análise puramente estatística/determinística
//!
//! ## Integração
//! O caller (gateway ou daemon) é responsável por spawnar a background task
//! e consumir os eventos emitidos via `ImprovementEvent`.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, info};

/// Evento emitido quando uma oportunidade de melhoria é detectada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImprovementEvent {
    /// Grupo de entradas candidatas a consolidação detectado.
    ConsolidationCandidate {
        /// IDs das entradas similares.
        entry_ids: Vec<String>,
        /// Similaridade média do grupo.
        avg_similarity: f64,
        /// Razão da detecção.
        reason: String,
    },
    /// Entradas stale que precisam de refresh de embedding/relevância.
    StaleEntries {
        /// Número de entradas stale detectadas.
        count: usize,
        /// Idade média das entradas stale.
        avg_age_secs: u64,
    },
    /// Padrão de acesso frequente detectado (hot path).
    HotAccessPattern {
        /// Escopo do padrão (ex: "agent:user-1", "global").
        scope: String,
        /// Contagem de acessos na janela.
        access_count: u64,
        /// Sugestão: criar cache dedicado ou mental model.
        suggestion: String,
    },
    /// Relatório periódico de saúde da memória.
    HealthReport {
        /// Total de entradas no sistema.
        total_entries: u64,
        /// Entradas acessadas na última janela.
        active_entries: u64,
        /// Taxa de acerto do cache (%).
        cache_hit_rate: f64,
        /// Sugestões pendentes não resolvidas.
        pending_suggestions: usize,
    },
}

/// Tracker lock-free de acessos a entradas de memória.
pub struct AccessTracker {
    /// Contadores de acesso por entry_id.
    access_counts: Arc<DashMap<String, AtomicU64>>,
    /// Timestamps de último acesso por entry_id.
    last_access: Arc<DashMap<String, Instant>>,
    /// Contador global de queries.
    total_queries: AtomicU64,
    /// Contador de cache hits.
    cache_hits: AtomicU64,
}

impl Default for AccessTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessTracker {
    pub fn new() -> Self {
        Self {
            access_counts: Arc::new(DashMap::new()),
            last_access: Arc::new(DashMap::new()),
            total_queries: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
        }
    }

    /// Registra um acesso a uma entrada de memória.
    pub fn record_access(&self, entry_id: &str) {
        self.access_counts
            .entry(entry_id.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
        self.last_access
            .insert(entry_id.to_string(), Instant::now());
        self.total_queries.fetch_add(1, Ordering::Relaxed);
    }

    /// Registra um cache hit.
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Retorna contagem de acessos para uma entrada.
    pub fn get_access_count(&self, entry_id: &str) -> u64 {
        self.access_counts
            .get(entry_id)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Retorna as N entradas mais acessadas.
    pub fn top_accessed(&self, n: usize) -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = self
            .access_counts
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().load(Ordering::Relaxed)))
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.1));
        entries.truncate(n);
        entries
    }

    /// Retorna entradas que não foram acessadas há mais que `threshold`.
    pub fn stale_entries(&self, threshold: Duration) -> Vec<(String, Duration)> {
        let now = Instant::now();
        self.last_access
            .iter()
            .filter_map(|entry| {
                let age = now.duration_since(*entry.value());
                if age > threshold {
                    Some((entry.key().clone(), age))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Retorna taxa de cache hit (0.0-1.0).
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.total_queries.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let hits = self.cache_hits.load(Ordering::Relaxed);
        hits as f64 / total as f64
    }

    /// Retorna contagem total de entradas rastreadas.
    pub fn tracked_entry_count(&self) -> usize {
        self.access_counts.len()
    }

    /// Reseta todos os contadores (para testes).
    pub fn reset(&self) {
        self.access_counts.clear();
        self.last_access.clear();
        self.total_queries.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
    }
}

/// Configuração do AutoImprovementLoop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoImprovementConfig {
    /// Intervalo entre ciclos de análise.
    pub analysis_interval: Duration,
    /// Limiar de acessos para considerar entrada como "hot".
    pub hot_access_threshold: u64,
    /// Idade para considerar entrada como stale.
    pub stale_threshold: Duration,
    /// Se true, emite HealthReport periodicamente.
    pub emit_health_reports: bool,
    /// Tamanho da janela para detecção de padrões.
    pub pattern_window: Duration,
}

impl Default for AutoImprovementConfig {
    fn default() -> Self {
        Self {
            analysis_interval: Duration::from_secs(300), // 5 minutos
            hot_access_threshold: 50,
            stale_threshold: Duration::from_secs(7 * 24 * 60 * 60), // 7 dias
            emit_health_reports: true,
            pattern_window: Duration::from_secs(3600), // 1 hora
        }
    }
}

/// Loop de auto-melhoria que analisa padrões de acesso em background.
pub struct AutoImprovementLoop {
    config: AutoImprovementConfig,
    tracker: Arc<AccessTracker>,
    event_tx: broadcast::Sender<ImprovementEvent>,
}

impl AutoImprovementLoop {
    /// Cria um novo loop com configuração e tracker compartilhado.
    pub fn new(config: AutoImprovementConfig, tracker: Arc<AccessTracker>) -> Self {
        let (event_tx, _) = broadcast::channel(64);
        Self {
            config,
            tracker,
            event_tx,
        }
    }

    /// Retorna um receiver para eventos de melhoria.
    pub fn subscribe(&self) -> broadcast::Receiver<ImprovementEvent> {
        self.event_tx.subscribe()
    }

    /// Retorna referência ao tracker de acessos.
    pub fn tracker(&self) -> &Arc<AccessTracker> {
        &self.tracker
    }

    /// Executa um ciclo único de análise (para testes e uso manual).
    pub fn run_analysis_cycle(&self) -> Vec<ImprovementEvent> {
        let mut events = Vec::new();

        // 1. Detectar hot access patterns
        let top = self.tracker.top_accessed(10);
        for (entry_id, count) in &top {
            if *count >= self.config.hot_access_threshold {
                events.push(ImprovementEvent::HotAccessPattern {
                    scope: entry_id.clone(),
                    access_count: *count,
                    suggestion: format!(
                        "Entry '{}' accessed {} times — consider caching or creating a mental model",
                        entry_id, count
                    ),
                });
            }
        }

        // 2. Detectar entradas stale
        let stale = self.tracker.stale_entries(self.config.stale_threshold);
        if !stale.is_empty() {
            let avg_age: u64 =
                stale.iter().map(|(_, age)| age.as_secs()).sum::<u64>() / stale.len() as u64;
            events.push(ImprovementEvent::StaleEntries {
                count: stale.len(),
                avg_age_secs: avg_age,
            });
        }

        // 3. Health report
        if self.config.emit_health_reports {
            let total = self.tracker.tracked_entry_count() as u64;
            let active = self
                .tracker
                .access_counts
                .iter()
                .filter(|e| e.value().load(Ordering::Relaxed) > 0)
                .count() as u64;
            events.push(ImprovementEvent::HealthReport {
                total_entries: total,
                active_entries: active,
                cache_hit_rate: self.tracker.cache_hit_rate(),
                pending_suggestions: events.len(),
            });
        }

        // Emitir eventos
        for event in &events {
            if let Err(e) = self.event_tx.send(event.clone()) {
                debug!("No subscribers for improvement event: {}", e);
            }
        }

        events
    }

    /// Inicia o loop de análise em background.
    ///
    /// Retorna um JoinHandle que pode ser abortado para parar o loop.
    pub fn start_background(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            info!(
                interval_secs = this.config.analysis_interval.as_secs(),
                "AutoImprovementLoop started"
            );
            let mut interval = tokio::time::interval(this.config.analysis_interval);
            loop {
                interval.tick().await;
                let events = this.run_analysis_cycle();
                if !events.is_empty() {
                    info!(
                        event_count = events.len(),
                        "AutoImprovementLoop cycle completed"
                    );
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_tracker_record_and_query() {
        let tracker = AccessTracker::new();

        tracker.record_access("entry-1");
        tracker.record_access("entry-1");
        tracker.record_access("entry-2");

        assert_eq!(tracker.get_access_count("entry-1"), 2);
        assert_eq!(tracker.get_access_count("entry-2"), 1);
        assert_eq!(tracker.get_access_count("nonexistent"), 0);
    }

    #[test]
    fn test_access_tracker_top_accessed() {
        let tracker = AccessTracker::new();

        for _ in 0..10 {
            tracker.record_access("hot");
        }
        for _ in 0..5 {
            tracker.record_access("warm");
        }
        tracker.record_access("cold");

        let top = tracker.top_accessed(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "hot");
        assert_eq!(top[0].1, 10);
        assert_eq!(top[1].0, "warm");
        assert_eq!(top[1].1, 5);
    }

    #[test]
    fn test_access_tracker_stale_detection() {
        let tracker = AccessTracker::new();

        // Registrar acesso "antigo" (simulado manipulando last_access diretamente)
        tracker.record_access("old-entry");
        // Substituir timestamp por um antigo
        tracker.last_access.insert(
            "old-entry".to_string(),
            Instant::now() - Duration::from_secs(100),
        );

        tracker.record_access("new-entry");

        let stale = tracker.stale_entries(Duration::from_secs(50));
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].0, "old-entry");
    }

    #[test]
    fn test_cache_hit_rate() {
        let tracker = AccessTracker::new();

        // Sem queries → rate = 0
        assert!((tracker.cache_hit_rate() - 0.0).abs() < f64::EPSILON);

        tracker.record_access("e1");
        tracker.record_access("e2");
        tracker.record_cache_hit();

        // 1 hit / 2 queries = 0.5
        assert!((tracker.cache_hit_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tracked_entry_count() {
        let tracker = AccessTracker::new();
        assert_eq!(tracker.tracked_entry_count(), 0);

        tracker.record_access("a");
        tracker.record_access("b");
        tracker.record_access("a"); // mesmo entry

        assert_eq!(tracker.tracked_entry_count(), 2);
    }

    #[test]
    fn test_reset() {
        let tracker = AccessTracker::new();
        tracker.record_access("x");
        tracker.record_cache_hit();

        tracker.reset();

        assert_eq!(tracker.tracked_entry_count(), 0);
        assert_eq!(tracker.get_access_count("x"), 0);
        assert!((tracker.cache_hit_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_auto_improvement_hot_pattern_detection() {
        let config = AutoImprovementConfig {
            hot_access_threshold: 5,
            emit_health_reports: false,
            ..Default::default()
        };
        let tracker = Arc::new(AccessTracker::new());
        let loop_instance = AutoImprovementLoop::new(config, tracker.clone());

        // Simular acessos acima do limiar
        for _ in 0..10 {
            tracker.record_access("frequent-entry");
        }

        let events = loop_instance.run_analysis_cycle();

        let hot_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, ImprovementEvent::HotAccessPattern { .. }))
            .collect();
        assert!(!hot_events.is_empty());

        if let ImprovementEvent::HotAccessPattern {
            scope,
            access_count,
            ..
        } = &hot_events[0]
        {
            assert_eq!(scope, "frequent-entry");
            assert_eq!(*access_count, 10);
        }
    }

    #[tokio::test]
    async fn test_auto_improvement_stale_detection() {
        let config = AutoImprovementConfig {
            stale_threshold: Duration::from_secs(10),
            emit_health_reports: false,
            ..Default::default()
        };
        let tracker = Arc::new(AccessTracker::new());
        let loop_instance = AutoImprovementLoop::new(config, tracker.clone());

        // Simular entrada antiga
        tracker.record_access("old-entry");
        tracker.last_access.insert(
            "old-entry".to_string(),
            Instant::now() - Duration::from_secs(60),
        );

        let events = loop_instance.run_analysis_cycle();

        let stale_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, ImprovementEvent::StaleEntries { .. }))
            .collect();
        assert!(!stale_events.is_empty());

        if let ImprovementEvent::StaleEntries { count, .. } = &stale_events[0] {
            assert_eq!(*count, 1);
        }
    }

    #[tokio::test]
    async fn test_auto_improvement_health_report() {
        let config = AutoImprovementConfig {
            emit_health_reports: true,
            hot_access_threshold: 1000, // alto para não gerar hot events
            stale_threshold: Duration::from_secs(999999), // alto para não gerar stale
            ..Default::default()
        };
        let tracker = Arc::new(AccessTracker::new());
        let loop_instance = AutoImprovementLoop::new(config, tracker.clone());

        tracker.record_access("e1");
        tracker.record_access("e2");
        tracker.record_cache_hit();

        let events = loop_instance.run_analysis_cycle();

        let health_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, ImprovementEvent::HealthReport { .. }))
            .collect();
        assert_eq!(health_events.len(), 1);

        if let ImprovementEvent::HealthReport {
            total_entries,
            active_entries,
            cache_hit_rate,
            ..
        } = &health_events[0]
        {
            assert_eq!(*total_entries, 2);
            assert_eq!(*active_entries, 2);
            assert!((*cache_hit_rate - 0.5).abs() < f64::EPSILON);
        }
    }

    #[tokio::test]
    async fn test_event_broadcast_subscription() {
        let config = AutoImprovementConfig {
            hot_access_threshold: 1,
            emit_health_reports: true,
            ..Default::default()
        };
        let tracker = Arc::new(AccessTracker::new());
        let loop_instance = AutoImprovementLoop::new(config, tracker.clone());

        let mut rx = loop_instance.subscribe();

        tracker.record_access("test-entry");

        let events = loop_instance.run_analysis_cycle();
        assert!(!events.is_empty());

        // Verificar que eventos foram broadcastados
        let received = rx.try_recv();
        assert!(received.is_ok());
    }

    #[test]
    fn test_improvement_event_serialization() {
        let event = ImprovementEvent::ConsolidationCandidate {
            entry_ids: vec!["e1".into(), "e2".into()],
            avg_similarity: 0.95,
            reason: "High cosine similarity detected".into(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let restored: ImprovementEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, restored);
    }

    #[test]
    fn test_auto_improvement_config_default() {
        let config = AutoImprovementConfig::default();
        assert_eq!(config.analysis_interval, Duration::from_secs(300));
        assert_eq!(config.hot_access_threshold, 50);
        assert!(config.emit_health_reports);
    }

    #[tokio::test]
    async fn test_no_events_when_no_activity() {
        let config = AutoImprovementConfig {
            hot_access_threshold: 100,
            stale_threshold: Duration::from_secs(999999),
            emit_health_reports: false,
            ..Default::default()
        };
        let tracker = Arc::new(AccessTracker::new());
        let loop_instance = AutoImprovementLoop::new(config, tracker);

        let events = loop_instance.run_analysis_cycle();
        // Sem activity e sem health report → sem eventos
        assert!(events.is_empty());
    }
}
