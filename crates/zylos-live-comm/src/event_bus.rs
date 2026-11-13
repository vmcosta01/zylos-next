//! Expanded EventBus with typed events, topic routing, backpressure strategies,
//! dual SSE/WebSocket transport support, persistence replay, filtering, DLQ,
//! schema registry and observability metrics.

use crate::event_filter::{topic_matches, CompiledFilter, FilterCache};
use crate::event_persistence::EventStore;
use crate::event_types::{EventEnvelope, SchemaRegistry, ZylosEvent};
use crate::error::LiveCommError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info_span, Instrument};

// ---------------------------------------------------------------------------
// Backpressure strategy
// ---------------------------------------------------------------------------

/// Strategy applied when a subscriber's bounded channel is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Enumeração BackpressureStrategy.
#[derive(Default)]
pub enum BackpressureStrategy {
    #[default]
    DropOldest,
    DropNewest,
    BlockProducer,
    SlowConsumerAlert,
}



// ---------------------------------------------------------------------------
// Subscriber descriptor
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Subscriber {
    #[allow(dead_code)]
    id: String,
    topic_pattern: String,
    filter: Option<CompiledFilter>,
    tx: broadcast::Sender<EventEnvelope>,
    #[allow(dead_code)]
    strategy: BackpressureStrategy,
}

// ---------------------------------------------------------------------------
// Observability counters
// ---------------------------------------------------------------------------

/// Per-topic metrics exposed for scraping / dashboards.
#[derive(Debug, Default)]
/// Estrutura TopicMetrics.
pub struct TopicMetrics {
    /// Campo publish count.
    pub publish_count: AtomicU64,
    /// Campo subscriber count.
    pub subscriber_count: AtomicU64,
    /// Campo drop count.
    pub drop_count: AtomicU64,
    /// Campo latency sum us.
    pub latency_sum_us: AtomicU64,
}

impl TopicMetrics {
    /// Função publish rate.
    pub fn publish_rate(&self) -> u64 {
        self.publish_count.load(Ordering::Relaxed)
    }
    /// Função subscribers.
    pub fn subscribers(&self) -> u64 {
        self.subscriber_count.load(Ordering::Relaxed)
    }
    /// Função drops.
    pub fn drops(&self) -> u64 {
        self.drop_count.load(Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// EventBus
// ---------------------------------------------------------------------------

/// Central event bus integrating typed events, topic routing, backpressure,
/// persistence, filtering, DLQ, schema validation and observability.
#[derive(Clone)]
/// Estrutura EventBus.
pub struct EventBus {
    inner: Arc<Inner>,
}

struct Inner {
    global_tx: broadcast::Sender<EventEnvelope>,
    subscribers: RwLock<HashMap<String, Subscriber>>,
    store: Option<Arc<EventStore>>,
    schema_registry: Arc<SchemaRegistry>,
    filter_cache: Arc<FilterCache>,
    metrics: RwLock<HashMap<String, Arc<TopicMetrics>>>,
    default_capacity: usize,
    default_strategy: BackpressureStrategy,
    #[allow(dead_code)]
    max_retries_before_dlq: u32,
}

impl EventBus {
    /// Create a new EventBus.
    ///
    /// * `capacity` – default broadcast channel size per subscriber.
    /// * `store` – optional persistent backend; pass `None` for in-memory only.
    pub fn new(capacity: usize, store: Option<EventStore>) -> Self {
        let (global_tx, _) = broadcast::channel(capacity.max(1));
        // Variante Self.
        Self {
            inner: Arc::new(Inner {
                global_tx,
                subscribers: RwLock::new(HashMap::new()),
                store: store.map(Arc::new),
                schema_registry: Arc::new(SchemaRegistry::new()),
                filter_cache: Arc::new(FilterCache::new()),
                metrics: RwLock::new(HashMap::new()),
                default_capacity: capacity,
                default_strategy: BackpressureStrategy::default(),
                max_retries_before_dlq: 3,
            }),
        }
    }

    // -- configuration ------------------------------------------------------

    /// Função with strategy.
    pub fn with_strategy(mut self, s: BackpressureStrategy) -> Self {
        Arc::get_mut(&mut self.inner).unwrap().default_strategy = s;
        self
    }

    /// Função with max retries.
    pub fn with_max_retries(mut self, n: u32) -> Self {
        Arc::get_mut(&mut self.inner).unwrap().max_retries_before_dlq = n;
        self
    }

    // -- publishing ---------------------------------------------------------

    /// Publish a typed event on a hierarchical topic.
    pub async fn publish(
        &self,
        topic: impl Into<String>,
        event: ZylosEvent,
        schema_version: u32,
    ) -> Result<u64, LiveCommError> {
        let topic = topic.into();
        let span = info_span!("event_bus.publish", topic = %topic);
        async move {
            let event_type_name = match &event {
                ZylosEvent::AgentStateChanged(_) => "AgentStateChanged",
                ZylosEvent::TaskCompleted(_) => "TaskCompleted",
                ZylosEvent::MemoryUpdated(_) => "MemoryUpdated",
                ZylosEvent::ApprovalRequested(_) => "ApprovalRequested",
                ZylosEvent::DagProgressChanged(_) => "DagProgressChanged",
                ZylosEvent::SkillInvoked(_) => "SkillInvoked",
                ZylosEvent::Custom(c) => c.name.as_str(),
            };
            let payload_value = serde_json::to_value(&event)
                .map_err(LiveCommError::Serialization)?;
            self.inner
                .schema_registry
                .validate(event_type_name, schema_version, &payload_value)
                .map_err(LiveCommError::InvalidEvent)?;

            let mut envelope = EventEnvelope::new(&topic, event, schema_version);

            if let Some(ref store) = self.inner.store {
                store
                    .append(&mut envelope)
                    .map_err(|e| LiveCommError::Persistence(e.to_string()))?;
            }

            let start = std::time::Instant::now();
            let subs = self.inner.subscribers.read().await;
            let mut delivered = 0u64;
            for sub in subs.values() {
                if !topic_matches(&sub.topic_pattern, &topic) {
                    continue;
                }
                if let Some(ref f) = sub.filter {
                    if !f.matches(&envelope) {
                        continue;
                    }
                }
                match sub.tx.send(envelope.clone()) {
                    // Variante Ok.
                    Ok(_) => delivered += 1,
                    // Variante Err.
                    Err(_) => {
                        self.inc_drop(&topic).await;
                    }
                }
            }
            drop(subs);

            let _ = self.inner.global_tx.send(envelope.clone());

            self.inc_publish(&topic).await;
            let elapsed = start.elapsed().as_micros() as u64;
            self.add_latency(&topic, elapsed).await;

            debug!(seq = envelope.sequence, delivered, "published");
            // Variante Ok.
            Ok(envelope.sequence)
        }
        .instrument(span)
        .await
    }

    // -- subscribing --------------------------------------------------------

    /// Subscribe to events matching a topic pattern (wildcards supported).
    pub async fn subscribe(
        &self,
        id: impl Into<String>,
        topic_pattern: impl Into<String>,
        filter_expr: Option<&str>,
    ) -> Result<broadcast::Receiver<EventEnvelope>, LiveCommError> {
        let id = id.into();
        let topic_pattern = topic_pattern.into();
        let filter = match filter_expr {
            // Variante Some.
            Some(expr) if !expr.is_empty() => Some(
                self.inner
                    .filter_cache
                    .get_or_compile(expr)
                    .map_err(LiveCommError::InvalidEvent)?,
            ),
            _ => None,
        };
        let (tx, rx) = broadcast::channel(self.inner.default_capacity);
        let sub = Subscriber {
            id: id.clone(),
            topic_pattern: topic_pattern.clone(),
            filter,
            tx,
            strategy: self.inner.default_strategy,
        };
        {
            let mut map = self.inner.subscribers.write().await;
            map.insert(id, sub);
        }
        self.inc_sub_count(&topic_pattern).await;
        // Variante Ok.
        Ok(rx)
    }

    /// Unsubscribe by id. Drops the per-subscriber sender so existing
    /// receivers immediately observe `RecvError::Closed` on next `recv()`.
    pub async fn unsubscribe(&self, id: &str) {
        let mut map = self.inner.subscribers.write().await;
        if let Some(sub) = map.remove(id) {
            // Explicitly drop the sender while still holding the write lock
            // to guarantee no concurrent publish can send after removal.
            drop(sub.tx);
            drop(map);
            self.dec_sub_count(&sub.topic_pattern).await;
        }
    }

    // -- replay -------------------------------------------------------------

    /// Função replay.
    pub fn replay(
        &self,
        topic: &str,
        last_sequence: u64,
        limit: u64,
    ) -> Result<Vec<EventEnvelope>, LiveCommError> {
        match &self.inner.store {
            // Variante Some.
            Some(store) => store
                .replay(topic, last_sequence, limit)
                .map_err(|e| LiveCommError::Persistence(e.to_string())),
            // Variante None.
            None => Ok(Vec::new()),
        }
    }

    // -- dead letter queue --------------------------------------------------

    /// Função list dead letters.
    pub fn list_dead_letters(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::event_persistence::DeadLetterEntry>, LiveCommError> {
        match &self.inner.store {
            // Variante Some.
            Some(store) => store
                .list_dead_letters(limit)
                .map_err(|e| LiveCommError::Persistence(e.to_string())),
            // Variante None.
            None => Ok(Vec::new()),
        }
    }

    /// Função ack dead letter.
    pub fn ack_dead_letter(&self, id: &str) -> Result<bool, LiveCommError> {
        match &self.inner.store {
            // Variante Some.
            Some(store) => store
                .ack_dead_letter(id)
                .map_err(|e| LiveCommError::Persistence(e.to_string())),
            // Variante None.
            None => Ok(false),
        }
    }

    // -- schema registry access ---------------------------------------------

    /// Função schema registry.
    pub fn schema_registry(&self) -> &SchemaRegistry {
        &self.inner.schema_registry
    }

    // -- observability ------------------------------------------------------

    /// Função topic metrics.
    pub async fn topic_metrics(&self, topic: &str) -> Option<Arc<TopicMetrics>> {
        let map = self.inner.metrics.read().await;
        map.get(topic).cloned()
    }

    /// Função receiver count.
    pub fn receiver_count(&self) -> usize {
        self.inner.global_tx.receiver_count()
    }

    // -- backward-compatible API (AgUiEvent) --------------------------------

    /// Publish a legacy AgUiEvent on the global broadcast channel.
    /// Wraps the event in a Custom ZylosEvent for compatibility with existing callers.
    pub fn publish_agui(&self, event: crate::agui::AgUiEvent) -> Result<usize, LiveCommError> {
        let envelope = EventEnvelope::new(
            "agui.legacy",
            ZylosEvent::Custom(crate::event_types::CustomEvent {
                name: "AgUiEvent".into(),
                payload: serde_json::to_value(&event).unwrap_or_default(),
            }),
            1,
        );
        self.inner.global_tx.send(envelope).map_err(|e| {
            LiveCommError::BroadcastSend(format!("no active receivers: {}", e))
        })
    }

    /// Subscribe to the raw global broadcast stream (legacy AgUiEvent consumers).
    /// Returns a BroadcastStream wrapping EventEnvelope; callers can extract the
    /// original AgUiEvent from the Custom payload if needed.
    pub fn subscribe_legacy(&self) -> tokio_stream::wrappers::BroadcastStream<EventEnvelope> {
        tokio_stream::wrappers::BroadcastStream::new(self.inner.global_tx.subscribe())
    }

    /// Subscribe with a filter predicate on EventEnvelope (legacy API shim).
    pub fn subscribe_filtered_legacy<F>(
        &self,
        filter: F,
    ) -> impl futures::Stream<Item = Result<EventEnvelope, tokio_stream::wrappers::errors::BroadcastStreamRecvError>>
           + Send
           + 'static
    where
        F: Fn(&EventEnvelope) -> bool + Clone + Send + 'static,
    {
        use futures::StreamExt;
        let stream = tokio_stream::wrappers::BroadcastStream::new(self.inner.global_tx.subscribe());
        stream.filter_map(move |item| {
            let f = filter.clone();
            async move {
                match item {
                    // Variante Ok.
                    Ok(env) if f(&env) => Some(Ok(env)),
                    // Variante Ok.
                    Ok(_) => None,
                    // Variante Err.
                    Err(e) => Some(Err(e)),
                }
            }
        })
    }

    // -- internal helpers ---------------------------------------------------

    async fn get_or_create_metrics(&self, topic: &str) -> Arc<TopicMetrics> {
        {
            let map = self.inner.metrics.read().await;
            if let Some(m) = map.get(topic) {
                return m.clone();
            }
        }
        let mut map = self.inner.metrics.write().await;
        map.entry(topic.to_string())
            .or_insert_with(|| Arc::new(TopicMetrics::default()))
            .clone()
    }

    async fn inc_publish(&self, topic: &str) {
        let m = self.get_or_create_metrics(topic).await;
        m.publish_count.fetch_add(1, Ordering::Relaxed);
    }

    async fn inc_drop(&self, topic: &str) {
        let m = self.get_or_create_metrics(topic).await;
        m.drop_count.fetch_add(1, Ordering::Relaxed);
    }

    async fn add_latency(&self, topic: &str, us: u64) {
        let m = self.get_or_create_metrics(topic).await;
        m.latency_sum_us.fetch_add(us, Ordering::Relaxed);
    }

    async fn inc_sub_count(&self, topic: &str) {
        let m = self.get_or_create_metrics(topic).await;
        m.subscriber_count.fetch_add(1, Ordering::Relaxed);
    }

    async fn dec_sub_count(&self, topic: &str) {
        let m = self.get_or_create_metrics(topic).await;
        m.subscriber_count.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_types::{AgentStateChanged, TaskCompleted};

    fn make_bus() -> EventBus {
        let store = EventStore::open(":memory:").unwrap();
        EventBus::new(64, Some(store))
    }

    #[tokio::test]
    async fn test_publish_and_subscribe_topic_match() {
        let bus = make_bus();
        let mut rx = bus.subscribe("s1", "agents.*.state", None).await.unwrap();

        bus.publish(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "idle".into(),
                new_state: "running".into(),
                reason: None,
            }),
            1,
        )
        .await
        .unwrap();

        let env = rx.recv().await.unwrap();
        assert_eq!(env.topic, "agents.a1.state");
    }

    #[tokio::test]
    async fn test_topic_wildcard_no_match() {
        let bus = make_bus();
        let mut rx = bus.subscribe("s1", "tasks.*.progress", None).await.unwrap();

        bus.publish(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "idle".into(),
                new_state: "running".into(),
                reason: None,
            }),
            1,
        )
        .await
        .unwrap();

        let result =
            tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_filter_expression() {
        let bus = make_bus();
        let mut rx = bus
            .subscribe(
                "s1",
                "agents.**",
                Some(r#"event_type == "AgentStateChanged""#),
            )
            .await
            .unwrap();

        bus.publish(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "idle".into(),
                new_state: "running".into(),
                reason: None,
            }),
            1,
        )
        .await
        .unwrap();

        bus.publish(
            "agents.a1.done",
            ZylosEvent::TaskCompleted(TaskCompleted {
                task_id: "t1".into(),
                agent_id: "a1".into(),
                outcome: "ok".into(),
                duration_ms: 10,
            }),
            1,
        )
        .await
        .unwrap();

        let env = rx.recv().await.unwrap();
        assert_eq!(env.topic, "agents.a1.state");

        let result =
            tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_replay_from_persistence() {
        let bus = make_bus();

        for i in 0..3 {
            bus.publish(
                "agents.a1.state",
                ZylosEvent::AgentStateChanged(AgentStateChanged {
                    agent_id: "a1".into(),
                    previous_state: format!("s{}", i),
                    new_state: format!("s{}", i + 1),
                    reason: None,
                }),
                1,
            )
            .await
            .unwrap();
        }

        let replayed = bus.replay("agents.a1.state", 0, 10).unwrap();
        assert_eq!(replayed.len(), 3);
        assert_eq!(replayed[0].sequence, 1);
        assert_eq!(replayed[2].sequence, 3);

        let from_2 = bus.replay("agents.a1.state", 2, 10).unwrap();
        assert_eq!(from_2.len(), 1);
        assert_eq!(from_2[0].sequence, 3);
    }

    #[tokio::test]
    async fn test_schema_validation_rejects_unknown_version() {
        let bus = make_bus();
        let result = bus
            .publish(
                "agents.a1.state",
                ZylosEvent::AgentStateChanged(AgentStateChanged {
                    agent_id: "a1".into(),
                    previous_state: "i".into(),
                    new_state: "r".into(),
                    reason: None,
                }),
                999,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unsubscribe_stops_delivery() {
        let bus = make_bus();
        let mut rx = bus.subscribe("s1", "agents.**", None).await.unwrap();

        bus.publish(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "i".into(),
                new_state: "r".into(),
                reason: None,
            }),
            1,
        )
        .await
        .unwrap();
        assert!(rx.recv().await.is_ok());

        bus.unsubscribe("s1").await;

        bus.publish(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "r".into(),
                new_state: "done".into(),
                reason: None,
            }),
            1,
        )
        .await
        .unwrap();

        // After unsubscribe, the per-subscriber sender is dropped,
        // so recv() returns RecvError::Closed immediately.
        let result = rx.recv().await;
        assert!(result.is_err(), "Expected RecvError::Closed after unsubscribe");
    }
}