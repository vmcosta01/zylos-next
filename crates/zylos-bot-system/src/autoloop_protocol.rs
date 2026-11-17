//! # AutoloopProtocol — Protocolo de comunicação assíncrona para orquestração autônoma
//!
//! T8.1.3: Implementa o enum `AutoloopMessage` com variantes Ask/Notice/Resolve/Later/Events
//! para suporte a resolução diferida entre agentes, inspirado em agent-autoloop.
//!
//! ## Semântica das Variantes
//! - **Ask**: Agente precisa de decisão humana ou de outro agente; bloqueia até resposta.
//! - **Notice**: Notificação unidirecional; não requer resposta.
//! - **Resolve**: Resultado final de uma tarefa/programa; fecha o ciclo.
//! - **Later**: Agenda re-execução após delay; retorna imediatamente.
//! - **Events**: Batch de eventos para processamento assíncrono.
//!
//! ## Integração P2P
//! Mensagens são serializadas via serde e transportadas pelo P2PMessenger
//! em tópicos dedicados (`autoloop.<program_id>` ou `autoloop.agent.<agent_id>`).

use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Identificador único de mensagem do protocolo Autoloop.
pub type AutoloopMsgId = String;

/// Pergunta estruturada que requer resposta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AskQuery {
    /// Título curto da pergunta (exibido em UI/cards).
    pub title: String,
    /// Descrição detalhada do contexto.
    pub text: String,
    /// Opções disponíveis para resposta (vazio = resposta livre).
    pub options: Vec<String>,
    /// Chave única para correlacionar resposta.
    pub key: String,
    /// Timeout opcional para resposta (None = infinito).
    pub timeout_secs: Option<u64>,
}

/// Evento para processamento assíncrono em batch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoloopEvent {
    /// Tipo do evento (ex: "file_changed", "metric_updated", "error_detected").
    pub event_type: String,
    /// Payload arbitrário do evento.
    pub payload: serde_json::Value,
    /// Timestamp ISO 8601 do evento.
    pub timestamp: String,
}

/// Resultado de uma execução de programa/tarefa.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolveResult {
    /// Se a execução foi bem-sucedida.
    pub success: bool,
    /// Métrica resultante (se aplicável).
    pub metric: Option<f64>,
    /// Resumo legível do resultado.
    pub summary: String,
    /// Artefatos produzidos (paths, URLs, IDs).
    pub artifacts: Vec<String>,
    /// Erro detalhado (se success=false).
    pub error: Option<String>,
}

/// Definição de re-execução diferida.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeferredTask {
    /// Delay antes da re-execução.
    pub delay: Duration,
    /// ID do programa/tarefa a re-executar.
    pub target_id: String,
    /// Contexto adicional para a re-execução.
    pub context: serde_json::Value,
    /// Número máximo de retries (0 = infinito).
    pub max_retries: u32,
    /// Contador atual de retries.
    pub retry_count: u32,
}

/// Mensagem do protocolo Autoloop para comunicação assíncrona entre agentes.
///
/// Serializável via serde para transporte sobre P2PMessenger ou persistência.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutoloopMessage {
    /// Agente precisa de decisão/resposta; bloqueia até resolução.
    Ask {
        id: AutoloopMsgId,
        sender: String,
        query: AskQuery,
    },
    /// Notificação unidirecional; não requer resposta.
    Notice {
        id: AutoloopMsgId,
        sender: String,
        message: String,
        metadata: serde_json::Value,
    },
    /// Resultado final de execução; fecha ciclo de iteração.
    Resolve {
        id: AutoloopMsgId,
        sender: String,
        result: ResolveResult,
    },
    /// Agenda re-execução após delay; retorna imediatamente.
    Later {
        id: AutoloopMsgId,
        sender: String,
        deferred: DeferredTask,
    },
    /// Batch de eventos para processamento assíncrono.
    Events {
        id: AutoloopMsgId,
        sender: String,
        events: Vec<AutoloopEvent>,
    },
}

impl AutoloopMessage {
    /// Cria uma mensagem Ask com ID auto-gerado.
    pub fn ask(sender: impl Into<String>, query: AskQuery) -> Self {
        Self::Ask {
            id: Uuid::new_v4().to_string(),
            sender: sender.into(),
            query,
        }
    }

    /// Cria uma mensagem Notice com ID auto-gerado.
    pub fn notice(
        sender: impl Into<String>,
        message: impl Into<String>,
        metadata: serde_json::Value,
    ) -> Self {
        Self::Notice {
            id: Uuid::new_v4().to_string(),
            sender: sender.into(),
            message: message.into(),
            metadata,
        }
    }

    /// Cria uma mensagem Resolve com ID auto-gerado.
    pub fn resolve(sender: impl Into<String>, result: ResolveResult) -> Self {
        Self::Resolve {
            id: Uuid::new_v4().to_string(),
            sender: sender.into(),
            result,
        }
    }

    /// Cria uma mensagem Later com ID auto-gerado.
    pub fn later(sender: impl Into<String>, deferred: DeferredTask) -> Self {
        Self::Later {
            id: Uuid::new_v4().to_string(),
            sender: sender.into(),
            deferred,
        }
    }

    /// Cria uma mensagem Events com ID auto-gerado.
    pub fn events(sender: impl Into<String>, events: Vec<AutoloopEvent>) -> Self {
        Self::Events {
            id: Uuid::new_v4().to_string(),
            sender: sender.into(),
            events,
        }
    }

    /// Retorna o ID da mensagem.
    pub fn id(&self) -> &str {
        match self {
            Self::Ask { id, .. } => id,
            Self::Notice { id, .. } => id,
            Self::Resolve { id, .. } => id,
            Self::Later { id, .. } => id,
            Self::Events { id, .. } => id,
        }
    }

    /// Retorna o sender da mensagem.
    pub fn sender(&self) -> &str {
        match self {
            Self::Ask { sender, .. } => sender,
            Self::Notice { sender, .. } => sender,
            Self::Resolve { sender, .. } => sender,
            Self::Later { sender, .. } => sender,
            Self::Events { sender, .. } => sender,
        }
    }

    /// Retorna o tópico P2P recomendado para esta mensagem.
    pub fn p2p_topic(&self, program_id: Option<&str>) -> String {
        match program_id {
            Some(pid) => format!("autoloop.{}", pid),
            None => format!("autoloop.agent.{}", self.sender()),
        }
    }

    /// Verifica se a mensagem requer resposta (bloqueante).
    pub fn requires_response(&self) -> bool {
        matches!(self, Self::Ask { .. })
    }

    /// Serializa para JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Desserializa de JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Handler trait para processamento de mensagens Autoloop.
///
/// Implementações definem como cada variante é tratada pelo agente receptor.
pub trait AutoloopHandler: Send + Sync {
    /// Processa uma mensagem Ask; deve retornar resposta ou erro.
    fn handle_ask(
        &self,
        query: &AskQuery,
    ) -> impl std::future::Future<Output = Result<serde_json::Value, String>> + Send;

    /// Processa uma mensagem Notice; fire-and-forget.
    fn handle_notice(&self, message: &str, metadata: &serde_json::Value);

    /// Processa uma mensagem Resolve; atualiza estado do programa.
    fn handle_resolve(&self, result: &ResolveResult);

    /// Processa uma mensagem Later; agenda re-execução.
    fn handle_later(&self, deferred: &DeferredTask);

    /// Processa um batch de eventos.
    fn handle_events(&self, events: &[AutoloopEvent]);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ask_creation() {
        let msg = AutoloopMessage::ask(
            "agent-1",
            AskQuery {
                title: "Como proceder?".into(),
                text: "Dois caminhos possíveis.".into(),
                options: vec!["A".into(), "B".into()],
                key: "decision_1".into(),
                timeout_secs: Some(300),
            },
        );

        assert!(msg.requires_response());
        assert_eq!(msg.sender(), "agent-1");
        assert!(!msg.id().is_empty());

        if let AutoloopMessage::Ask { query, .. } = &msg {
            assert_eq!(query.title, "Como proceder?");
            assert_eq!(query.options.len(), 2);
            assert_eq!(query.timeout_secs, Some(300));
        } else {
            panic!("Expected Ask variant");
        }
    }

    #[test]
    fn test_notice_creation() {
        let msg = AutoloopMessage::notice(
            "agent-2",
            "Build concluído com sucesso",
            serde_json::json!({"duration_ms": 4500}),
        );

        assert!(!msg.requires_response());
        assert_eq!(msg.sender(), "agent-2");

        if let AutoloopMessage::Notice {
            message, metadata, ..
        } = &msg
        {
            assert_eq!(message, "Build concluído com sucesso");
            assert_eq!(metadata["duration_ms"], 4500);
        } else {
            panic!("Expected Notice variant");
        }
    }

    #[test]
    fn test_resolve_creation() {
        let msg = AutoloopMessage::resolve(
            "evaluator",
            ResolveResult {
                success: true,
                metric: Some(87.5),
                summary: "Coverage increased to 87.5%".into(),
                artifacts: vec!["reports/coverage.html".into()],
                error: None,
            },
        );

        assert!(!msg.requires_response());

        if let AutoloopMessage::Resolve { result, .. } = &msg {
            assert!(result.success);
            assert_eq!(result.metric, Some(87.5));
            assert_eq!(result.artifacts.len(), 1);
            assert!(result.error.is_none());
        } else {
            panic!("Expected Resolve variant");
        }
    }

    #[test]
    fn test_later_creation() {
        let msg = AutoloopMessage::later(
            "scheduler",
            DeferredTask {
                delay: Duration::from_secs(3600),
                target_id: "prog-abc".into(),
                context: serde_json::json!({"iteration": 5}),
                max_retries: 3,
                retry_count: 0,
            },
        );

        assert!(!msg.requires_response());

        if let AutoloopMessage::Later { deferred, .. } = &msg {
            assert_eq!(deferred.delay, Duration::from_secs(3600));
            assert_eq!(deferred.target_id, "prog-abc");
            assert_eq!(deferred.max_retries, 3);
        } else {
            panic!("Expected Later variant");
        }
    }

    #[test]
    fn test_events_creation() {
        let events = vec![
            AutoloopEvent {
                event_type: "file_changed".into(),
                payload: serde_json::json!({"path": "src/lib.rs"}),
                timestamp: "2026-11-11T00:00:00Z".into(),
            },
            AutoloopEvent {
                event_type: "metric_updated".into(),
                payload: serde_json::json!({"key": "coverage", "value": 90.0}),
                timestamp: "2026-11-11T00:01:00Z".into(),
            },
        ];

        let msg = AutoloopMessage::events("watcher", events);
        assert!(!msg.requires_response());

        if let AutoloopMessage::Events { events, .. } = &msg {
            assert_eq!(events.len(), 2);
            assert_eq!(events[0].event_type, "file_changed");
            assert_eq!(events[1].event_type, "metric_updated");
        } else {
            panic!("Expected Events variant");
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let original = AutoloopMessage::ask(
            "agent-x",
            AskQuery {
                title: "Test".into(),
                text: "Context".into(),
                options: vec!["Y".into(), "N".into()],
                key: "k1".into(),
                timeout_secs: None,
            },
        );

        let json = original.to_json().unwrap();
        let restored = AutoloopMessage::from_json(&json).unwrap();

        assert_eq!(original, restored);
    }

    #[test]
    fn test_p2p_topic_with_program_id() {
        let msg = AutoloopMessage::notice("a1", "test", serde_json::Value::Null);
        assert_eq!(msg.p2p_topic(Some("prog-123")), "autoloop.prog-123");
    }

    #[test]
    fn test_p2p_topic_without_program_id() {
        let msg = AutoloopMessage::notice("agent-alpha", "test", serde_json::Value::Null);
        assert_eq!(msg.p2p_topic(None), "autoloop.agent.agent-alpha");
    }

    #[test]
    fn test_requires_response_only_for_ask() {
        let ask = AutoloopMessage::ask(
            "a",
            AskQuery {
                title: "T".into(),
                text: "D".into(),
                options: vec![],
                key: "k".into(),
                timeout_secs: None,
            },
        );
        let notice = AutoloopMessage::notice("a", "m", serde_json::Value::Null);
        let resolve = AutoloopMessage::resolve(
            "a",
            ResolveResult {
                success: true,
                metric: None,
                summary: "ok".into(),
                artifacts: vec![],
                error: None,
            },
        );
        let later = AutoloopMessage::later(
            "a",
            DeferredTask {
                delay: Duration::from_secs(60),
                target_id: "t".into(),
                context: serde_json::Value::Null,
                max_retries: 0,
                retry_count: 0,
            },
        );
        let events = AutoloopMessage::events("a", vec![]);

        assert!(ask.requires_response());
        assert!(!notice.requires_response());
        assert!(!resolve.requires_response());
        assert!(!later.requires_response());
        assert!(!events.requires_response());
    }

    #[test]
    fn test_resolve_failure() {
        let msg = AutoloopMessage::resolve(
            "evaluator",
            ResolveResult {
                success: false,
                metric: None,
                summary: "Tests failed".into(),
                artifacts: vec![],
                error: Some("3 tests failed in module X".into()),
            },
        );

        if let AutoloopMessage::Resolve { result, .. } = &msg {
            assert!(!result.success);
            assert!(result.error.is_some());
            assert!(result.metric.is_none());
        } else {
            panic!("Expected Resolve variant");
        }
    }

    #[test]
    fn test_deferred_task_retry_tracking() {
        let deferred = DeferredTask {
            delay: Duration::from_secs(300),
            target_id: "prog-xyz".into(),
            context: serde_json::json!({"attempt": 2}),
            max_retries: 5,
            retry_count: 2,
        };

        assert_eq!(deferred.retry_count, 2);
        assert_eq!(deferred.max_retries, 5);
        assert!(deferred.retry_count < deferred.max_retries);
    }
}
