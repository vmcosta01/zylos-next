//! Tipos fundamentais para durabilidade de tarefas

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Identificador único de checkpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub u64);

impl CheckpointId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Estado serializável de uma tarefa
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    /// Versão do schema de estado
    pub version: u32,
    /// Dados opacos da tarefa (JSON)
    pub data: Vec<u8>,
    /// Timestamp do checkpoint
    pub timestamp: SystemTime,
    /// ID sequencial do checkpoint
    pub checkpoint_id: CheckpointId,
    /// Hash SHA-256 dos dados para integridade
    pub data_hash: [u8; 32],
}

/// Um checkpoint completo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub task_id: String,
    pub state: TaskState,
    pub metadata: CheckpointMetadata,
}

/// Metadados do checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    /// Nome do worker que criou
    pub worker_profile: String,
    /// Turno em que foi criado
    pub turn_number: u64,
    /// Descrição legível
    pub description: String,
}

/// Resultado de uma operação de recovery
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryOutcome {
    /// Recuperação bem-sucedida
    Recovered { checkpoint_id: CheckpointId },
    /// Nenhum checkpoint disponível
    NoCheckpoint,
    /// Checkpoint corrompido ou hash inválido
    Corrupted { reason: String },
    /// Erro de I/O durante recovery
    IoError { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_id_equality() {
        let a = CheckpointId::new(42);
        let b = CheckpointId::new(42);
        let c = CheckpointId::new(43);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_task_state_serialization() {
        let state = TaskState {
            version: 1,
            data: b"hello world".to_vec(),
            timestamp: SystemTime::UNIX_EPOCH,
            checkpoint_id: CheckpointId::new(1),
            data_hash: [0u8; 32],
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: TaskState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.data, b"hello world");
    }
}
