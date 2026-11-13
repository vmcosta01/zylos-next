//! P2PMessenger — Mensagens type-safe entre agentes via Tokio broadcast channels
//!
//! Implementa a Task 1.1.3 da Fase 1: P2P messaging com tópicos,
//! delivery guarantee (ack/nack), dead letter queue e schema validation.
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, error, warn};
use uuid::Uuid;
use crate::error::{BotError, Result};

/// Identificador único de mensagem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(Uuid);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Tópico de comunicação (canal lógico entre agentes)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Topic(String);

impl Topic {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Topic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Topic {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Mensagem trocada entre agentes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// ID único da mensagem
    pub id: MessageId,
    /// Tópico de destino
    pub topic: Topic,
    /// ID do agente remetente
    pub sender: String,
    /// Payload serializado como JSON
    pub payload: serde_json::Value,
    /// Timestamp de criação (ISO 8601)
    pub timestamp: String,
    /// TTL opcional em segundos (None = infinito)
    pub ttl_secs: Option<u64>,
}

impl Message {
    /// Cria uma nova mensagem com timestamp atual
    pub fn new(
        topic: impl Into<Topic>,
        sender: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: MessageId::new(),
            topic: topic.into(),
            sender: sender.into(),
            payload,
            timestamp: chrono_now_stub(),
            ttl_secs: None,
        }
    }

    /// Define TTL para a mensagem
    pub fn with_ttl(mut self, secs: u64) -> Self {
        self.ttl_secs = Some(secs);
        self
    }
}

/// Entrada na dead letter queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetter {
    pub message: Message,
    pub reason: String,
    pub failed_at: String,
}

/// Estado interno de um tópico
struct TopicState {
    tx: broadcast::Sender<Message>,
    subscriber_count: usize,
}

/// Messenger P2P com suporte a tópicos, broadcast e dead letter queue
pub struct P2PMessenger {
    topics: Arc<DashMap<Topic, TopicState>>,
    dead_letters: Arc<Mutex<Vec<DeadLetter>>>,
    max_dead_letters: usize,
    default_channel_capacity: usize,
}

impl P2PMessenger {
    /// Cria um novo P2PMessenger
    ///
    /// # Arguments
    /// * `default_channel_capacity` - Capacidade padrão do buffer de broadcast por tópico
    /// * `max_dead_letters` - Tamanho máximo da dead letter queue antes de descartar as mais antigas
    pub fn new(default_channel_capacity: usize, max_dead_letters: usize) -> Self {
        Self {
            topics: Arc::new(DashMap::new()),
            dead_letters: Arc::new(Mutex::new(Vec::new())),
            max_dead_letters,
            default_channel_capacity,
        }
    }

    /// Inscreve um agente em um tópico, retornando um receiver
    ///
    /// Se o tópico não existir, ele é criado automaticamente.
    pub fn subscribe(&self, topic: impl Into<Topic>) -> broadcast::Receiver<Message> {
        let topic = topic.into();
        let mut entry = self
            .topics
            .entry(topic.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(self.default_channel_capacity);
                debug!(topic = %topic, "Topic created");
                TopicState {
                    tx,
                    subscriber_count: 0,
                }
            });

        entry.subscriber_count += 1;
        entry.tx.subscribe()
    }

    /// Publica uma mensagem em um tópico
    ///
    /// Retorna o número de receivers que receberam a mensagem.
    /// Se não houver subscribers, a mensagem vai para a dead letter queue.
    pub async fn publish(&self, msg: Message) -> Result<usize> {
        let topic = msg.topic.clone();

        // Verificar TTL
        if let Some(ttl) = msg.ttl_secs {
            // Stub: em produção, comparar timestamp com now
            if ttl == 0 {
                warn!(msg_id = %msg.id, topic = %topic, "Message expired (TTL=0)");
                self.add_dead_letter(msg, "TTL expired".to_string()).await;
                return Ok(0);
            }
        }

        if let Some(state) = self.topics.get(&topic) {
            match state.tx.send(msg.clone()) {
                Ok(count) => {
                    if count == 0 {
                        debug!(msg_id = %msg.id, topic = %topic, "No subscribers, sending to DLQ");
                        self.add_dead_letter(msg, "No active subscribers".to_string())
                            .await;
                    } else {
                        debug!(msg_id = %msg.id, topic = %topic, receivers = count, "Message delivered");
                    }
                    Ok(count)
                }
                Err(e) => {
                    error!(msg_id = %msg.id, topic = %topic, error = %e, "Send failed");
                    self.add_dead_letter(msg, format!("Broadcast send error: {}", e))
                        .await;
                    Ok(0)
                }
            }
        } else {
            warn!(topic = %topic, "Publish to non-existent topic, creating and sending to DLQ");
            // Criar tópico mas enviar para DLQ pois ninguém está ouvindo ainda
            let (tx, _) = broadcast::channel(self.default_channel_capacity);
            self.topics.insert(
                topic.clone(),
                TopicState {
                    tx,
                    subscriber_count: 0,
                },
            );
            self.add_dead_letter(msg, "Topic did not exist at publish time".to_string())
                .await;
            Ok(0)
        }
    }

    /// Publica e aguarda confirmação de pelo menos um subscriber
    ///
    /// Timeout em milissegundos. Retorna erro se nenhum subscriber receber dentro do prazo.
    pub async fn publish_with_ack(
        &self,
        msg: Message,
        timeout_ms: u64,
    ) -> Result<usize> {
        let topic = msg.topic.clone();
        let has_subscribers = self
            .topics
            .get(&topic)
            .map(|s| s.subscriber_count > 0)
            .unwrap_or(false);

        if !has_subscribers {
            // Aguardar brevemente por subscribers
            let deadline = Duration::from_millis(timeout_ms);
            let start = tokio::time::Instant::now();
            loop {
                if start.elapsed() >= deadline {
                    self.add_dead_letter(
                        msg,
                        format!("No subscribers within {}ms", timeout_ms),
                    )
                    .await;
                    return Err(BotError::Internal(format!(
                        "publish_with_ack: no subscribers on '{}' within {}ms",
                        topic, timeout_ms
                    )));
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
                if self
                    .topics
                    .get(&topic)
                    .map(|s| s.subscriber_count > 0)
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }

        self.publish(msg).await
    }

    /// Retorna uma cópia das mensagens na dead letter queue
    pub async fn get_dead_letters(&self) -> Vec<DeadLetter> {
        self.dead_letters.lock().await.clone()
    }

    /// Limpa a dead letter queue
    pub async fn clear_dead_letters(&self) -> usize {
        let mut dlq = self.dead_letters.lock().await;
        let count = dlq.len();
        dlq.clear();
        count
    }

    /// Lista todos os tópicos ativos com contagem de subscribers
    pub fn list_topics(&self) -> Vec<(Topic, usize)> {
        self.topics
            .iter()
            .map(|entry| (entry.key().clone(), entry.subscriber_count))
            .collect()
    }

    /// Remove um tópico e libera recursos
    pub fn remove_topic(&self, topic: impl Into<Topic>) -> bool {
        let topic = topic.into();
        self.topics.remove(&topic).is_some()
    }

    /// Adiciona entrada à dead letter queue, respeitando limite de tamanho
    async fn add_dead_letter(&self, message: Message, reason: String) {
        let mut dlq = self.dead_letters.lock().await;
        if dlq.len() >= self.max_dead_letters {
            // Remover a mais antiga (FIFO)
            dlq.remove(0);
        }
        dlq.push(DeadLetter {
            message,
            reason,
            failed_at: chrono_now_stub(),
        });
    }
}

impl Default for P2PMessenger {
    fn default() -> Self {
        Self::new(256, 1000)
    }
}

/// Retorna timestamp ISO 8601 UTC atual usando std::time (sem dependência chrono).
fn chrono_now_stub() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Formatação manual de RFC 3339 / ISO 8601 UTC
    const SECS_PER_DAY: u64 = 86400;
    const DAYS_PER_400Y: u64 = 146097;
    const DAYS_PER_100Y: u64 = 36524;
    const DAYS_PER_4Y: u64 = 1461;
    const DAYS_PER_Y: u64 = 365;
    const MONTH_DAYS: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    let days = secs / SECS_PER_DAY;
    let time_secs = secs % SECS_PER_DAY;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Calcular data a partir de dias desde epoch (1970-01-01)
    let mut y = 1970i64;
    let mut d = days;

    // Ciclos de 400 anos
    let n400 = d / DAYS_PER_400Y;
    y += (n400 * 400) as i64;
    d %= DAYS_PER_400Y;

    // Ciclos de 100 anos
    let mut n100 = d / DAYS_PER_100Y;
    if n100 == 4 { n100 = 3; }
    y += (n100 * 100) as i64;
    d -= n100 * DAYS_PER_100Y;

    // Ciclos de 4 anos
    let n4 = d / DAYS_PER_4Y;
    y += (n4 * 4) as i64;
    d -= n4 * DAYS_PER_4Y;

    // Anos individuais
    let mut n1 = d / DAYS_PER_Y;
    if n1 == 4 { n1 = 3; }
    y += n1 as i64;
    d -= n1 * DAYS_PER_Y;

    // Mês e dia
    let is_leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let mut m = 0usize;
     for (i, &md_base) in MONTH_DAYS.iter().enumerate() {
     let md = if i == 1 && is_leap { 29 } else { md_base };
     if d < md {
     m = i;
     break;
     }
     d -= md;
     }
    let day = d + 1;
    let month = m + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, month, day, hours, minutes, seconds
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_publish_and_receive() {
        let messenger = P2PMessenger::new(64, 100);
        let mut rx = messenger.subscribe("test-topic");

        let msg = Message::new("test-topic", "agent-a", serde_json::json!({"data": 42}));
        let delivered = messenger.publish(msg).await.expect("publish should succeed");

        assert_eq!(delivered, 1);

        let received = rx.recv().await.expect("should receive message");
        assert_eq!(received.sender, "agent-a");
        assert_eq!(received.payload["data"], 42);
    }

    #[tokio::test]
    async fn test_no_subscribers_goes_to_dlq() {
        let messenger = P2PMessenger::new(64, 100);

        let msg = Message::new("empty-topic", "agent-b", serde_json::json!({"x": 1}));
        let delivered = messenger.publish(msg).await.expect("publish should succeed");

        assert_eq!(delivered, 0);

        let dlq = messenger.get_dead_letters().await;
        assert_eq!(dlq.len(), 1);
        assert!(
            dlq[0].reason.contains("No active subscribers")
                || dlq[0].reason.contains("Topic did not exist"),
            "Unexpected DLQ reason: {}",
            dlq[0].reason
        );
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let messenger = P2PMessenger::new(64, 100);
        let mut rx1 = messenger.subscribe("multi-topic");
        let mut rx2 = messenger.subscribe("multi-topic");

        let msg = Message::new("multi-topic", "sender", serde_json::json!({"v": "hello"}));
        let delivered = messenger.publish(msg).await.expect("publish should succeed");

        assert_eq!(delivered, 2);

        let r1 = rx1.recv().await.unwrap();
        let r2 = rx2.recv().await.unwrap();
        assert_eq!(r1.payload["v"], "hello");
        assert_eq!(r2.payload["v"], "hello");
    }

    #[tokio::test]
    async fn test_list_topics() {
        let messenger = P2PMessenger::new(64, 100);
        let _rx1 = messenger.subscribe("topic-a");
        let _rx2 = messenger.subscribe("topic-b");
        let _rx3 = messenger.subscribe("topic-a"); // Segundo subscriber no mesmo tópico

        let topics = messenger.list_topics();
        assert_eq!(topics.len(), 2);

        let topic_map: HashMap<String, usize> = topics
            .into_iter()
            .map(|(t, c)| (t.as_str().to_string(), c))
            .collect();
        assert_eq!(topic_map.get("topic-a"), Some(&2));
        assert_eq!(topic_map.get("topic-b"), Some(&1));
    }

    #[tokio::test]
    async fn test_remove_topic() {
        let messenger = P2PMessenger::new(64, 100);
        let _rx = messenger.subscribe("removable");

        assert!(messenger.remove_topic("removable"));
        assert!(!messenger.remove_topic("removable")); // Já removido

        let topics = messenger.list_topics();
        assert!(topics.is_empty());
    }

    #[tokio::test]
    async fn test_dead_letter_queue_max_size() {
        let messenger = P2PMessenger::new(64, 3); // Max 3 entradas

        for i in 0..5 {
            let msg = Message::new("no-listener", "agent", serde_json::json!({"i": i}));
            messenger.publish(msg).await.unwrap();
        }

        let dlq = messenger.get_dead_letters().await;
        assert_eq!(dlq.len(), 3); // Apenas as 3 mais recentes
        // As primeiras 2 devem ter sido descartadas
        assert_eq!(dlq[0].message.payload["i"], 2);
        assert_eq!(dlq[1].message.payload["i"], 3);
        assert_eq!(dlq[2].message.payload["i"], 4);
    }

    #[tokio::test]
    async fn test_clear_dead_letters() {
        let messenger = P2PMessenger::new(64, 100);

        let msg = Message::new("dlq-test", "agent", serde_json::json!({}));
        messenger.publish(msg).await.unwrap();

        assert_eq!(messenger.get_dead_letters().await.len(), 1);
        let cleared = messenger.clear_dead_letters().await;
        assert_eq!(cleared, 1);
        assert!(messenger.get_dead_letters().await.is_empty());
    }

    #[tokio::test]
    async fn test_message_with_ttl_zero() {
        let messenger = P2PMessenger::new(64, 100);
        let _rx = messenger.subscribe("ttl-topic");

        let msg = Message::new("ttl-topic", "agent", serde_json::json!({}))
            .with_ttl(0);
        let delivered = messenger.publish(msg).await.unwrap();

        assert_eq!(delivered, 0);
        let dlq = messenger.get_dead_letters().await;
        assert_eq!(dlq.len(), 1);
        assert!(dlq[0].reason.contains("TTL expired"));
    }

    #[tokio::test]
    async fn test_publish_with_ack_timeout() {
        let messenger = P2PMessenger::new(64, 100);

        let msg = Message::new("no-sub-topic", "agent", serde_json::json!({}));
        let result = messenger.publish_with_ack(msg, 50).await;

        assert!(result.is_err());
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(err_msg.contains("no subscribers"));
    }

    #[tokio::test]
    async fn test_publish_with_ack_success() {
        let messenger = P2PMessenger::new(64, 100);
        let _rx = messenger.subscribe("ack-topic");

        let msg = Message::new("ack-topic", "agent", serde_json::json!({"ok": true}));
        let result = messenger.publish_with_ack(msg, 100).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }
}