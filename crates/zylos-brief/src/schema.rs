//! Schema de dados para Morning Brief Generator
//!
//! Tipos serializáveis para persistência SQLite e comunicação via API REST.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Um item individual dentro de uma seção do brief
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,           // critical, high, medium, low
    pub source: String,             // approval, task, goal, calendar, integration
    pub action_url: Option<String>, // URL para ação direta no cockpit
    pub metadata: Option<serde_json::Value>,
}

/// Uma seção do morning brief
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefSection {
    pub name: String, // needs_you, can_wait, calendar, unchecked
    pub icon: String, // emoji ou ícone visual
    pub items: Vec<BriefItem>,
    pub summary: Option<String>, // Resumo textual da seção
}

/// Morning Brief completo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorningBrief {
    pub id: String,
    pub user_id: String,
    pub date: String, // YYYY-MM-DD
    pub sections: Vec<BriefSection>,
    pub generated_at: String, // ISO 8601
    pub total_items: usize,
    pub needs_you_count: usize,
}

impl MorningBrief {
    /// Cria brief vazio com todas as seções padrão
    pub fn empty(user_id: &str, date: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            date: date.to_string(),
            sections: vec![
                BriefSection {
                    name: "needs_you".into(),
                    icon: "🔴".into(),
                    items: vec![],
                    summary: Some("Nenhuma ação urgente pendente".into()),
                },
                BriefSection {
                    name: "can_wait".into(),
                    icon: "🟡".into(),
                    items: vec![],
                    summary: Some("Tarefas em andamento sob controle".into()),
                },
                BriefSection {
                    name: "calendar".into(),
                    icon: "📅".into(),
                    items: vec![],
                    summary: Some("Sem eventos agendados hoje".into()),
                },
                BriefSection {
                    name: "unchecked".into(),
                    icon: "⚠️".into(),
                    items: vec![],
                    summary: Some("Todas as integrações funcionando".into()),
                },
            ],
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_items: 0,
            needs_you_count: 0,
        }
    }

    /// Atualiza contadores após popular seções
    pub fn finalize(&mut self) {
        self.total_items = self.sections.iter().map(|s| s.items.len()).sum();
        self.needs_you_count = self
            .sections
            .iter()
            .find(|s| s.name == "needs_you")
            .map(|s| s.items.len())
            .unwrap_or(0);

        // Atualizar summaries automáticos
        for section in &mut self.sections {
            if section.items.is_empty() {
                continue; // Manter summary padrão de vazio
            }
            section.summary = match section.name.as_str() {
                "needs_you" => Some(format!(
                    "{} itens requerem sua atenção",
                    section.items.len()
                )),
                "can_wait" => Some(format!("{} tarefas em andamento", section.items.len())),
                "calendar" => Some(format!("{} eventos hoje", section.items.len())),
                "unchecked" => Some(format!("{} integrações com problemas", section.items.len())),
                _ => None,
            };
        }
    }
}

/// Entrada persistida no SQLite (JSON serializado)
#[derive(Debug, Clone, FromRow)]
pub struct BriefRow {
    pub id: String,
    pub user_id: String,
    pub date: String,
    pub content_json: String,
    pub delivered_at: String,
    pub read_at: Option<String>,
    pub items_count: i64,
    pub needs_you_count: i64,
}

/// Request para regeneração forçada
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegenerateRequest {
    pub user_id: Option<String>,
    pub force: bool, // Se true, substitui brief existente do dia
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_brief_has_all_sections() {
        let brief = MorningBrief::empty("user-1", "2026-11-11");
        assert_eq!(brief.sections.len(), 4);
        assert_eq!(brief.sections[0].name, "needs_you");
        assert_eq!(brief.sections[1].name, "can_wait");
        assert_eq!(brief.sections[2].name, "calendar");
        assert_eq!(brief.sections[3].name, "unchecked");
        assert_eq!(brief.total_items, 0);
        assert_eq!(brief.needs_you_count, 0);
    }

    #[test]
    fn test_finalize_updates_counts() {
        let mut brief = MorningBrief::empty("user-1", "2026-11-11");
        brief.sections[0].items.push(BriefItem {
            id: "1".into(),
            title: "Approve email".into(),
            description: None,
            priority: "critical".into(),
            source: "approval".into(),
            action_url: None,
            metadata: None,
        });
        brief.sections[1].items.push(BriefItem {
            id: "2".into(),
            title: "Deploy staging".into(),
            description: None,
            priority: "medium".into(),
            source: "task".into(),
            action_url: None,
            metadata: None,
        });
        brief.finalize();

        assert_eq!(brief.total_items, 2);
        assert_eq!(brief.needs_you_count, 1);
        assert_eq!(
            brief.sections[0].summary.as_deref(),
            Some("1 itens requerem sua atenção")
        );
        assert_eq!(
            brief.sections[1].summary.as_deref(),
            Some("1 tarefas em andamento")
        );
    }

    #[test]
    fn test_brief_serialization_roundtrip() {
        let brief = MorningBrief::empty("user-1", "2026-11-11");
        let json = serde_json::to_string(&brief).unwrap();
        let parsed: MorningBrief = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.user_id, "user-1");
        assert_eq!(parsed.date, "2026-11-11");
        assert_eq!(parsed.sections.len(), 4);
    }
}
