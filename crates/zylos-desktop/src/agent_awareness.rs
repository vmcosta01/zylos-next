//! # AgentAwarenessState — Estados de consciência do agente para UI (T8.6.2)
//!
//! Define os estados visuais que um agente pode exibir no cockpit,
//! permitindo que o usuário entenda instantaneamente o que cada agente
//! está fazendo sem precisar inspecionar logs.
//!
//! ## Estados
//! - **Idle**: Agente disponível, sem tarefa ativa
//! - **Thinking**: Processando/reasoning antes de agir
//! - **Executing**: Executando uma ação/ferramenta
//! - **WaitingApproval**: Bloqueado aguardando aprovação humana (HITL)
//! - **Error**: Falha recuperável com mensagem de diagnóstico
//! - **Completed**: Tarefa concluída com sucesso
//!
//! ## Integração WebSocket
//! Estados são transmitidos via AG-UI streaming para atualização em tempo real
//! no cockpit Leptos WASM. Cada transição de estado gera um evento `agent_state_changed`.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::SystemTime;

/// Estado de consciência de um agente para exibição na UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AgentAwarenessState {
    /// Agente disponível, sem tarefa ativa.
    #[default]
    Idle,
    /// Processando/reasoning antes de agir.
    Thinking,
    /// Executando uma ação ou ferramenta.
    Executing,
    /// Bloqueado aguardando aprovação humana (HITL gate).
    WaitingApproval,
    /// Falha recuperável com diagnóstico disponível.
    Error,
    /// Tarefa concluída com sucesso.
    Completed,
}

impl fmt::Display for AgentAwarenessState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Thinking => write!(f, "thinking"),
            Self::Executing => write!(f, "executing"),
            Self::WaitingApproval => write!(f, "waiting_approval"),
            Self::Error => write!(f, "error"),
            Self::Completed => write!(f, "completed"),
        }
    }
}

impl AgentAwarenessState {
    /// Parseia estado de string (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "idle" => Some(Self::Idle),
            "thinking" | "reasoning" => Some(Self::Thinking),
            "executing" | "running" | "working" => Some(Self::Executing),
            "waiting_approval" | "waiting" | "blocked" => Some(Self::WaitingApproval),
            "error" | "failed" | "failure" => Some(Self::Error),
            "completed" | "done" | "success" => Some(Self::Completed),
            _ => None,
        }
    }

    /// Retorna a cor CSS recomendada para este estado (para uso no cockpit).
    pub fn css_color(&self) -> &'static str {
        match self {
            Self::Idle => "var(--color-muted)",
            Self::Thinking => "var(--color-warning)",
            Self::Executing => "var(--color-accent)",
            Self::WaitingApproval => "var(--color-info)",
            Self::Error => "var(--color-error)",
            Self::Completed => "var(--color-success)",
        }
    }

    /// Retorna ícone/emoji recomendado para este estado.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Idle => "💤",
            Self::Thinking => "🤔",
            Self::Executing => "⚡",
            Self::WaitingApproval => "✋",
            Self::Error => "❌",
            Self::Completed => "✅",
        }
    }

    /// Retorna label acessível para screen readers.
    pub fn accessibility_label(&self) -> &'static str {
        match self {
            Self::Idle => "Agent is idle and available",
            Self::Thinking => "Agent is thinking",
            Self::Executing => "Agent is executing a task",
            Self::WaitingApproval => "Agent is waiting for human approval",
            Self::Error => "Agent encountered an error",
            Self::Completed => "Agent completed successfully",
        }
    }

    /// Verifica se o estado indica que o agente está ativo (não idle/completed).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Thinking | Self::Executing | Self::WaitingApproval
        )
    }

    /// Verifica se o estado requer atenção do usuário.
    pub fn requires_attention(&self) -> bool {
        matches!(self, Self::WaitingApproval | Self::Error)
    }

    /// Retorna todos os estados como slice ordenado.
    pub fn all() -> &'static [AgentAwarenessState] {
        &[
            AgentAwarenessState::Idle,
            AgentAwarenessState::Thinking,
            AgentAwarenessState::Executing,
            AgentAwarenessState::WaitingApproval,
            AgentAwarenessState::Error,
            AgentAwarenessState::Completed,
        ]
    }
}

/// Evento de mudança de estado de um agente.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentStateChangedEvent {
    /// ID do agente.
    pub agent_id: String,
    /// Estado anterior.
    pub previous_state: AgentAwarenessState,
    /// Novo estado.
    pub new_state: AgentAwarenessState,
    /// Mensagem contextual opcional (ex: descrição da tarefa, erro).
    pub message: Option<String>,
    /// Timestamp Unix epoch em milissegundos.
    pub timestamp_ms: u64,
}

impl AgentStateChangedEvent {
    /// Cria um novo evento de mudança de estado.
    pub fn new(
        agent_id: impl Into<String>,
        previous: AgentAwarenessState,
        new: AgentAwarenessState,
        message: Option<String>,
    ) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            agent_id: agent_id.into(),
            previous_state: previous,
            new_state: new,
            message,
            timestamp_ms,
        }
    }

    /// Verifica se houve mudança real de estado.
    pub fn is_actual_change(&self) -> bool {
        self.previous_state != self.new_state
    }
}

/// Tracker de estados de múltiplos agentes com histórico.
pub struct AgentAwarenessTracker {
    /// Estado atual por agent_id.
    states: std::collections::HashMap<String, AgentAwarenessState>,
    /// Histórico recente de eventos (ring buffer).
    history: Vec<AgentStateChangedEvent>,
    /// Tamanho máximo do histórico.
    max_history: usize,
}

impl Default for AgentAwarenessTracker {
    fn default() -> Self {
        Self::new(100)
    }
}

impl AgentAwarenessTracker {
    /// Cria um tracker com tamanho de histórico especificado.
    pub fn new(max_history: usize) -> Self {
        Self {
            states: std::collections::HashMap::new(),
            history: Vec::new(),
            max_history,
        }
    }

    /// Atualiza o estado de um agente e retorna o evento gerado.
    pub fn update_state(
        &mut self,
        agent_id: impl Into<String>,
        new_state: AgentAwarenessState,
        message: Option<String>,
    ) -> AgentStateChangedEvent {
        let agent_id = agent_id.into();
        let previous = self.states.get(&agent_id).copied().unwrap_or_default();

        let event = AgentStateChangedEvent::new(agent_id.clone(), previous, new_state, message);

        self.states.insert(agent_id, new_state);

        // Adicionar ao histórico (ring buffer)
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(event.clone());

        event
    }

    /// Retorna o estado atual de um agente.
    pub fn get_state(&self, agent_id: &str) -> AgentAwarenessState {
        self.states.get(agent_id).copied().unwrap_or_default()
    }

    /// Retorna todos os agentes com seus estados atuais.
    pub fn all_states(&self) -> Vec<(String, AgentAwarenessState)> {
        self.states
            .iter()
            .map(|(id, state)| (id.clone(), *state))
            .collect()
    }

    /// Retorna agentes em um estado específico.
    pub fn agents_in_state(&self, state: AgentAwarenessState) -> Vec<String> {
        self.states
            .iter()
            .filter(|(_, s)| **s == state)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Retorna agentes que requerem atenção.
    pub fn agents_requiring_attention(&self) -> Vec<(String, AgentAwarenessState)> {
        self.states
            .iter()
            .filter(|(_, s)| s.requires_attention())
            .map(|(id, s)| (id.clone(), *s))
            .collect()
    }

    /// Retorna as últimas N entradas do histórico.
    pub fn recent_history(&self, limit: usize) -> &[AgentStateChangedEvent] {
        let start = self.history.len().saturating_sub(limit);
        &self.history[start..]
    }

    /// Retorna estatísticas dos estados.
    pub fn stats(&self) -> AwarenessStats {
        let mut counts = std::collections::HashMap::new();
        for state in AgentAwarenessState::all() {
            counts.insert(*state, 0usize);
        }
        for state in self.states.values() {
            *counts.entry(*state).or_insert(0) += 1;
        }

        AwarenessStats {
            total_agents: self.states.len(),
            state_counts: counts,
            attention_required: self.agents_requiring_attention().len(),
            history_size: self.history.len(),
        }
    }

    /// Remove um agente do tracker.
    pub fn remove_agent(&mut self, agent_id: &str) -> bool {
        self.states.remove(agent_id).is_some()
    }

    /// Limpa todo o tracker.
    pub fn clear(&mut self) {
        self.states.clear();
        self.history.clear();
    }
}

/// Estatísticas de awareness dos agentes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AwarenessStats {
    pub total_agents: usize,
    pub state_counts: std::collections::HashMap<AgentAwarenessState, usize>,
    pub attention_required: usize,
    pub history_size: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state_is_idle() {
        assert_eq!(AgentAwarenessState::default(), AgentAwarenessState::Idle);
    }

    #[test]
    fn test_state_display() {
        assert_eq!(format!("{}", AgentAwarenessState::Idle), "idle");
        assert_eq!(format!("{}", AgentAwarenessState::Thinking), "thinking");
        assert_eq!(format!("{}", AgentAwarenessState::Executing), "executing");
        assert_eq!(
            format!("{}", AgentAwarenessState::WaitingApproval),
            "waiting_approval"
        );
        assert_eq!(format!("{}", AgentAwarenessState::Error), "error");
        assert_eq!(format!("{}", AgentAwarenessState::Completed), "completed");
    }

    #[test]
    fn test_state_from_str() {
        assert_eq!(
            AgentAwarenessState::parse("idle"),
            Some(AgentAwarenessState::Idle)
        );
        assert_eq!(
            AgentAwarenessState::parse("THINKING"),
            Some(AgentAwarenessState::Thinking)
        );
        assert_eq!(
            AgentAwarenessState::parse("reasoning"),
            Some(AgentAwarenessState::Thinking)
        );
        assert_eq!(
            AgentAwarenessState::parse("executing"),
            Some(AgentAwarenessState::Executing)
        );
        assert_eq!(
            AgentAwarenessState::parse("running"),
            Some(AgentAwarenessState::Executing)
        );
        assert_eq!(
            AgentAwarenessState::parse("working"),
            Some(AgentAwarenessState::Executing)
        );
        assert_eq!(
            AgentAwarenessState::parse("waiting_approval"),
            Some(AgentAwarenessState::WaitingApproval)
        );
        assert_eq!(
            AgentAwarenessState::parse("blocked"),
            Some(AgentAwarenessState::WaitingApproval)
        );
        assert_eq!(
            AgentAwarenessState::parse("error"),
            Some(AgentAwarenessState::Error)
        );
        assert_eq!(
            AgentAwarenessState::parse("failed"),
            Some(AgentAwarenessState::Error)
        );
        assert_eq!(
            AgentAwarenessState::parse("completed"),
            Some(AgentAwarenessState::Completed)
        );
        assert_eq!(
            AgentAwarenessState::parse("done"),
            Some(AgentAwarenessState::Completed)
        );
        assert_eq!(AgentAwarenessState::parse("unknown"), None);
        assert_eq!(AgentAwarenessState::parse(""), None);
    }

    #[test]
    fn test_is_active() {
        assert!(!AgentAwarenessState::Idle.is_active());
        assert!(AgentAwarenessState::Thinking.is_active());
        assert!(AgentAwarenessState::Executing.is_active());
        assert!(AgentAwarenessState::WaitingApproval.is_active());
        assert!(!AgentAwarenessState::Error.is_active());
        assert!(!AgentAwarenessState::Completed.is_active());
    }

    #[test]
    fn test_requires_attention() {
        assert!(!AgentAwarenessState::Idle.requires_attention());
        assert!(!AgentAwarenessState::Thinking.requires_attention());
        assert!(!AgentAwarenessState::Executing.requires_attention());
        assert!(AgentAwarenessState::WaitingApproval.requires_attention());
        assert!(AgentAwarenessState::Error.requires_attention());
        assert!(!AgentAwarenessState::Completed.requires_attention());
    }

    #[test]
    fn test_css_colors_non_empty() {
        for state in AgentAwarenessState::all() {
            assert!(!state.css_color().is_empty());
        }
    }

    #[test]
    fn test_icons_non_empty() {
        for state in AgentAwarenessState::all() {
            assert!(!state.icon().is_empty());
        }
    }

    #[test]
    fn test_accessibility_labels_non_empty() {
        for state in AgentAwarenessState::all() {
            assert!(!state.accessibility_label().is_empty());
        }
    }

    #[test]
    fn test_state_serialization_roundtrip() {
        for state in AgentAwarenessState::all() {
            let json = serde_json::to_string(state).unwrap();
            let restored: AgentAwarenessState = serde_json::from_str(&json).unwrap();
            assert_eq!(*state, restored);
        }
    }

    #[test]
    fn test_state_changed_event_creation() {
        let event = AgentStateChangedEvent::new(
            "agent-1",
            AgentAwarenessState::Idle,
            AgentAwarenessState::Thinking,
            Some("Processing user request".into()),
        );

        assert_eq!(event.agent_id, "agent-1");
        assert_eq!(event.previous_state, AgentAwarenessState::Idle);
        assert_eq!(event.new_state, AgentAwarenessState::Thinking);
        assert_eq!(event.message, Some("Processing user request".into()));
        assert!(event.timestamp_ms > 0);
        assert!(event.is_actual_change());
    }

    #[test]
    fn test_state_changed_event_no_change() {
        let event = AgentStateChangedEvent::new(
            "agent-1",
            AgentAwarenessState::Idle,
            AgentAwarenessState::Idle,
            None,
        );
        assert!(!event.is_actual_change());
    }

    #[test]
    fn test_tracker_update_and_get() {
        let mut tracker = AgentAwarenessTracker::new(50);

        tracker.update_state("a1", AgentAwarenessState::Thinking, None);
        assert_eq!(tracker.get_state("a1"), AgentAwarenessState::Thinking);

        tracker.update_state(
            "a1",
            AgentAwarenessState::Executing,
            Some("Running tests".into()),
        );
        assert_eq!(tracker.get_state("a1"), AgentAwarenessState::Executing);
    }

    #[test]
    fn test_tracker_unknown_agent_returns_idle() {
        let tracker = AgentAwarenessTracker::new(10);
        assert_eq!(tracker.get_state("nonexistent"), AgentAwarenessState::Idle);
    }

    #[test]
    fn test_tracker_agents_in_state() {
        let mut tracker = AgentAwarenessTracker::new(50);

        tracker.update_state("a1", AgentAwarenessState::Thinking, None);
        tracker.update_state("a2", AgentAwarenessState::Executing, None);
        tracker.update_state("a3", AgentAwarenessState::Thinking, None);
        tracker.update_state("a4", AgentAwarenessState::Idle, None);

        let thinking = tracker.agents_in_state(AgentAwarenessState::Thinking);
        assert_eq!(thinking.len(), 2);
        assert!(thinking.contains(&"a1".to_string()));
        assert!(thinking.contains(&"a3".to_string()));

        let idle = tracker.agents_in_state(AgentAwarenessState::Idle);
        assert_eq!(idle.len(), 1);
    }

    #[test]
    fn test_tracker_agents_requiring_attention() {
        let mut tracker = AgentAwarenessTracker::new(50);

        tracker.update_state("a1", AgentAwarenessState::WaitingApproval, None);
        tracker.update_state("a2", AgentAwarenessState::Error, Some("Timeout".into()));
        tracker.update_state("a3", AgentAwarenessState::Executing, None);
        tracker.update_state("a4", AgentAwarenessState::Idle, None);

        let attention = tracker.agents_requiring_attention();
        assert_eq!(attention.len(), 2);
    }

    #[test]
    fn test_tracker_history_ring_buffer() {
        let mut tracker = AgentAwarenessTracker::new(3);

        tracker.update_state("a1", AgentAwarenessState::Thinking, None);
        tracker.update_state("a1", AgentAwarenessState::Executing, None);
        tracker.update_state("a1", AgentAwarenessState::Completed, None);
        tracker.update_state("a1", AgentAwarenessState::Idle, None);

        // History should only keep last 3
        let recent = tracker.recent_history(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].new_state, AgentAwarenessState::Executing);
        assert_eq!(recent[2].new_state, AgentAwarenessState::Idle);
    }

    #[test]
    fn test_tracker_stats() {
        let mut tracker = AgentAwarenessTracker::new(50);

        tracker.update_state("a1", AgentAwarenessState::Thinking, None);
        tracker.update_state("a2", AgentAwarenessState::Error, None);
        tracker.update_state("a3", AgentAwarenessState::Idle, None);

        let stats = tracker.stats();
        assert_eq!(stats.total_agents, 3);
        assert_eq!(stats.attention_required, 1);
        assert_eq!(stats.history_size, 3);
        assert_eq!(
            *stats
                .state_counts
                .get(&AgentAwarenessState::Thinking)
                .unwrap(),
            1
        );
        assert_eq!(
            *stats.state_counts.get(&AgentAwarenessState::Error).unwrap(),
            1
        );
        assert_eq!(
            *stats.state_counts.get(&AgentAwarenessState::Idle).unwrap(),
            1
        );
    }

    #[test]
    fn test_tracker_remove_agent() {
        let mut tracker = AgentAwarenessTracker::new(50);
        tracker.update_state("a1", AgentAwarenessState::Thinking, None);

        assert!(tracker.remove_agent("a1"));
        assert_eq!(tracker.get_state("a1"), AgentAwarenessState::Idle);
        assert!(!tracker.remove_agent("a1")); // Already removed
    }

    #[test]
    fn test_tracker_clear() {
        let mut tracker = AgentAwarenessTracker::new(50);
        tracker.update_state("a1", AgentAwarenessState::Thinking, None);
        tracker.update_state("a2", AgentAwarenessState::Executing, None);

        tracker.clear();

        assert_eq!(tracker.all_states().len(), 0);
        assert_eq!(tracker.recent_history(10).len(), 0);
    }

    #[test]
    fn test_tracker_all_states() {
        let mut tracker = AgentAwarenessTracker::new(50);
        tracker.update_state("x", AgentAwarenessState::Idle, None);
        tracker.update_state("y", AgentAwarenessState::Completed, None);

        let all = tracker.all_states();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_event_serialization() {
        let event = AgentStateChangedEvent::new(
            "agent-ser",
            AgentAwarenessState::Idle,
            AgentAwarenessState::Executing,
            Some("Deploy started".into()),
        );

        let json = serde_json::to_string(&event).unwrap();
        let restored: AgentStateChangedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event.agent_id, restored.agent_id);
        assert_eq!(event.previous_state, restored.previous_state);
        assert_eq!(event.new_state, restored.new_state);
        assert_eq!(event.message, restored.message);
    }

    #[test]
    fn test_awareness_stats_serialization() {
        let stats = AwarenessStats {
            total_agents: 5,
            state_counts: std::collections::HashMap::from([
                (AgentAwarenessState::Idle, 2),
                (AgentAwarenessState::Executing, 3),
            ]),
            attention_required: 0,
            history_size: 10,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let restored: AwarenessStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, restored);
    }

    #[test]
    fn test_all_states_count() {
        assert_eq!(AgentAwarenessState::all().len(), 6);
    }
}
