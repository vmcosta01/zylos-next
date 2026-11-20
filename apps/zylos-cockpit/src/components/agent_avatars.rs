//! AgentAvatars — avatares visuais para agentes com cores por divisão
//!
//! Inspirado em: OpenBot agent cards, agency-agents role badges.
//! Fornece avatares coloridos baseados na divisão/função do agente,
//! com mais de 20 cores distintas para diferenciação visual clara.
//!
//! ## Divisões e Cores (CSS custom properties em `styles/design-tokens.css`)
//! - Engineering: var(--role-engineering)
//! - Security: var(--role-security)
//! - DevOps: var(--role-devops)
//! - Data: var(--role-data)
//! - Design: var(--role-design)
//! - Product: var(--role-product)
//! - QA: var(--role-qa)
//! - Research: var(--role-research)
//! - Support: var(--role-support)
//! - Finance: var(--role-finance)
//! - Legal: var(--role-legal)
//! - HR: var(--role-hr)
//! - Marketing: var(--role-marketing)
//! - Operations: var(--role-operations)
//! - Architecture: var(--role-architecture)
//! - Testing: var(--role-testing)
//! - Documentation: var(--role-documentation)
//! - Integration: var(--role-integration)
//! - Monitoring: var(--role-monitoring)
//! - Compliance: var(--role-compliance)
//! - Default: var(--role-custom)
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §4.6

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Divisão/função do agente que determina a cor do avatar
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentDivision {
    Engineering,
    Security,
    DevOps,
    Data,
    Design,
    Product,
    QA,
    Research,
    Support,
    Finance,
    Legal,
    HR,
    Marketing,
    Operations,
    Architecture,
    Testing,
    Documentation,
    Integration,
    Monitoring,
    Compliance,
    Custom(String),
}

impl AgentDivision {
    /// Retorna o nome da CSS custom property associada à divisão.
    /// O valor real da cor vive em `styles/design-tokens.css` (ADR-008).
    pub fn color_var(&self) -> &'static str {
        match self {
            Self::Engineering => "--role-engineering",
            Self::Security => "--role-security",
            Self::DevOps => "--role-devops",
            Self::Data => "--role-data",
            Self::Design => "--role-design",
            Self::Product => "--role-product",
            Self::QA => "--role-qa",
            Self::Research => "--role-research",
            Self::Support => "--role-support",
            Self::Finance => "--role-finance",
            Self::Legal => "--role-legal",
            Self::HR => "--role-hr",
            Self::Marketing => "--role-marketing",
            Self::Operations => "--role-operations",
            Self::Architecture => "--role-architecture",
            Self::Testing => "--role-testing",
            Self::Documentation => "--role-documentation",
            Self::Integration => "--role-integration",
            Self::Monitoring => "--role-monitoring",
            Self::Compliance => "--role-compliance",
            Self::Custom(_) => "--role-custom",
        }
    }

    /// Retorna a referência CSS `var(--role-*)` para uso em inline styles.
    pub fn color(&self) -> String {
        format!("var({})", self.color_var())
    }

    /// Retorna as iniciais da divisão para exibição no avatar
    pub fn initials(&self) -> &'static str {
        match self {
            Self::Engineering => "EN",
            Self::Security => "SE",
            Self::DevOps => "DO",
            Self::Data => "DA",
            Self::Design => "DE",
            Self::Product => "PR",
            Self::QA => "QA",
            Self::Research => "RE",
            Self::Support => "SU",
            Self::Finance => "FI",
            Self::Legal => "LE",
            Self::HR => "HR",
            Self::Marketing => "MK",
            Self::Operations => "OP",
            Self::Architecture => "AR",
            Self::Testing => "TE",
            Self::Documentation => "DC",
            Self::Integration => "IN",
            Self::Monitoring => "MO",
            Self::Compliance => "CO",
            Self::Custom(_) => "??",
        }
    }

    /// Retorna o label legível da divisão
    pub fn label(&self) -> String {
        match self {
            Self::Engineering => "Engenharia".into(),
            Self::Security => "Segurança".into(),
            Self::DevOps => "DevOps".into(),
            Self::Data => "Dados".into(),
            Self::Design => "Design".into(),
            Self::Product => "Produto".into(),
            Self::QA => "QA".into(),
            Self::Research => "Pesquisa".into(),
            Self::Support => "Suporte".into(),
            Self::Finance => "Finanças".into(),
            Self::Legal => "Legal".into(),
            Self::HR => "RH".into(),
            Self::Marketing => "Marketing".into(),
            Self::Operations => "Operações".into(),
            Self::Architecture => "Arquitetura".into(),
            Self::Testing => "Testes".into(),
            Self::Documentation => "Documentação".into(),
            Self::Integration => "Integração".into(),
            Self::Monitoring => "Monitoramento".into(),
            Self::Compliance => "Compliance".into(),
            Self::Custom(name) => name.clone(),
        }
    }

    /// Parse de string para divisão (case-insensitive)
    #[allow(dead_code)]
    pub fn from_label(label: &str) -> Self {
        match label.to_lowercase().as_str() {
            "engineering" | "engenharia" | "eng" => Self::Engineering,
            "security" | "segurança" | "sec" => Self::Security,
            "devops" | "dev-ops" | "do" => Self::DevOps,
            "data" | "dados" => Self::Data,
            "design" | "ux" | "ui" => Self::Design,
            "product" | "produto" | "pm" => Self::Product,
            "qa" | "quality" => Self::QA,
            "research" | "pesquisa" => Self::Research,
            "support" | "suporte" => Self::Support,
            "finance" | "finanças" => Self::Finance,
            "legal" | "jurídico" => Self::Legal,
            "hr" | "rh" | "people" => Self::HR,
            "marketing" | "mkt" => Self::Marketing,
            "operations" | "operações" | "ops" => Self::Operations,
            "architecture" | "arquitetura" | "arch" => Self::Architecture,
            "testing" | "testes" | "test" => Self::Testing,
            "documentation" | "documentação" | "docs" => Self::Documentation,
            "integration" | "integração" => Self::Integration,
            "monitoring" | "monitoramento" | "mon" => Self::Monitoring,
            "compliance" => Self::Compliance,
            other => Self::Custom(other.to_string()),
        }
    }
}

/// Informações completas de um agente para exibição
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAvatarInfo {
    pub id: String,
    pub name: String,
    pub division: AgentDivision,
    pub status: String, // online, offline, busy, away
    pub current_task: Option<String>,
}

/// Componente de avatar individual
#[component]
pub fn AgentAvatar(
    #[prop(into)] agent: AgentAvatarInfo,
    #[prop(default = 48)] size: u32,
) -> impl IntoView {
    let color = agent.division.color().to_string();
    let initials = agent.division.initials().to_string();
    let name = agent.name.clone();
    let division_label = agent.division.label();
    let status = agent.status.clone();
    let task = agent.current_task.clone();

    let status_class = match status.as_str() {
        "online" => "avatar-status-online",
        "busy" => "avatar-status-busy",
        "away" => "avatar-status-away",
        _ => "avatar-status-offline",
    };

    let style = format!(
        "width: {}px; height: {}px; background-color: {}; font-size: {}px;",
        size,
        size,
        color,
        size / 3
    );

    view! {
        <div class="agent-avatar-wrapper" title=format!("{} ({})", name, division_label)>
            <div class="agent-avatar" style=style>
                <span class="agent-avatar-initials">{initials}</span>
            </div>
            <div class=format!("agent-avatar-status {}", status_class)></div>
            {task.map(|t| view! {
                <div class="agent-avatar-task">{t}</div>
            })}
        </div>
    }
}

/// Grid de avatares de todos os agentes
#[component]
pub fn AgentAvatarsGrid() -> impl IntoView {
    let agents = RwSignal::new(vec![
        AgentAvatarInfo {
            id: "agent-1".into(),
            name: "Code Builder".into(),
            division: AgentDivision::Engineering,
            status: "online".into(),
            current_task: Some("Implementando approval gates".into()),
        },
        AgentAvatarInfo {
            id: "agent-2".into(),
            name: "Shield Guard".into(),
            division: AgentDivision::Security,
            status: "online".into(),
            current_task: Some("Verificando egress rules".into()),
        },
        AgentAvatarInfo {
            id: "agent-3".into(),
            name: "Pipeline Runner".into(),
            division: AgentDivision::DevOps,
            status: "busy".into(),
            current_task: Some("Deploy staging".into()),
        },
        AgentAvatarInfo {
            id: "agent-4".into(),
            name: "Data Analyst".into(),
            division: AgentDivision::Data,
            status: "online".into(),
            current_task: None,
        },
        AgentAvatarInfo {
            id: "agent-5".into(),
            name: "UI Designer".into(),
            division: AgentDivision::Design,
            status: "away".into(),
            current_task: None,
        },
        AgentAvatarInfo {
            id: "agent-6".into(),
            name: "Product Owner".into(),
            division: AgentDivision::Product,
            status: "online".into(),
            current_task: Some("Revisando backlog".into()),
        },
        AgentAvatarInfo {
            id: "agent-7".into(),
            name: "Test Runner".into(),
            division: AgentDivision::QA,
            status: "busy".into(),
            current_task: Some("Executando E2E tests".into()),
        },
        AgentAvatarInfo {
            id: "agent-8".into(),
            name: "Deep Researcher".into(),
            division: AgentDivision::Research,
            status: "online".into(),
            current_task: Some("Analisando competitors".into()),
        },
        AgentAvatarInfo {
            id: "agent-9".into(),
            name: "Help Desk".into(),
            division: AgentDivision::Support,
            status: "offline".into(),
            current_task: None,
        },
        AgentAvatarInfo {
            id: "agent-10".into(),
            name: "Budget Tracker".into(),
            division: AgentDivision::Finance,
            status: "online".into(),
            current_task: None,
        },
        AgentAvatarInfo {
            id: "agent-11".into(),
            name: "Contract Reviewer".into(),
            division: AgentDivision::Legal,
            status: "away".into(),
            current_task: None,
        },
        AgentAvatarInfo {
            id: "agent-12".into(),
            name: "People Ops".into(),
            division: AgentDivision::HR,
            status: "online".into(),
            current_task: None,
        },
    ]);

    view! {
        <div class="agent-avatars-panel">
            <div class="agent-avatars-header">
                <h3 class="agent-avatars-title"><i class="ph ph-users-three" aria-hidden="true"></i>" Equipe de Agentes"</h3>
                <span class="agent-avatars-count">
                    {move || format!("{} agentes", agents.get().len())}
                </span>
            </div>

            <div class="agent-avatars-grid">
                <For
                    each=move || agents.get()
                    key=|a| a.id.clone()
                    children=move |agent| {
                        view! {
                            <AgentAvatar agent=agent size=56 />
                        }
                    }
                />
            </div>

            // Legenda de divisões
            <div class="agent-division-legend">
                <span class="legend-item"><span class="legend-dot" style="background: var(--role-engineering)"></span>"Engenharia"</span>
                <span class="legend-item"><span class="legend-dot" style="background: var(--role-security)"></span>"Segurança"</span>
                <span class="legend-item"><span class="legend-dot" style="background: var(--role-devops)"></span>"DevOps"</span>
                <span class="legend-item"><span class="legend-dot" style="background: var(--role-data)"></span>"Dados"</span>
                <span class="legend-item"><span class="legend-dot" style="background: var(--role-design)"></span>"Design"</span>
                <span class="legend-item"><span class="legend-dot" style="background: var(--role-product)"></span>"Produto"</span>
                <span class="legend-item"><span class="legend-dot" style="background: var(--role-qa)"></span>"QA"</span>
                <span class="legend-item"><span class="legend-dot" style="background: var(--role-research)"></span>"Pesquisa"</span>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_divisions_have_unique_colors() {
        let divisions = vec![
            AgentDivision::Engineering,
            AgentDivision::Security,
            AgentDivision::DevOps,
            AgentDivision::Data,
            AgentDivision::Design,
            AgentDivision::Product,
            AgentDivision::QA,
            AgentDivision::Research,
            AgentDivision::Support,
            AgentDivision::Finance,
            AgentDivision::Legal,
            AgentDivision::HR,
            AgentDivision::Marketing,
            AgentDivision::Operations,
            AgentDivision::Architecture,
            AgentDivision::Testing,
            AgentDivision::Documentation,
            AgentDivision::Integration,
            AgentDivision::Monitoring,
            AgentDivision::Compliance,
        ];

        let colors: Vec<String> = divisions.iter().map(|d| d.color()).collect();
        let unique: std::collections::HashSet<&String> = colors.iter().collect();
        assert_eq!(
            colors.len(),
            unique.len(),
            "All divisions must have unique colors"
        );
        assert_eq!(colors.len(), 20, "Must have exactly 20 divisions");
    }

    #[test]
    fn test_from_label_case_insensitive() {
        assert_eq!(
            AgentDivision::from_label("ENGINEERING"),
            AgentDivision::Engineering
        );
        assert_eq!(
            AgentDivision::from_label("segurança"),
            AgentDivision::Security
        );
        assert_eq!(AgentDivision::from_label("DevOps"), AgentDivision::DevOps);
        assert_eq!(
            AgentDivision::from_label("unknown"),
            AgentDivision::Custom("unknown".into())
        );
    }

    #[test]
    fn test_initials_are_two_chars() {
        let divisions = vec![
            AgentDivision::Engineering,
            AgentDivision::Security,
            AgentDivision::DevOps,
            AgentDivision::QA,
            AgentDivision::HR,
        ];
        for d in divisions {
            assert_eq!(
                d.initials().len(),
                2,
                "Initials must be exactly 2 chars for {:?}",
                d
            );
        }
    }

    #[test]
    fn test_agent_avatar_info_serialization() {
        let agent = AgentAvatarInfo {
            id: "test-1".into(),
            name: "Test Agent".into(),
            division: AgentDivision::Engineering,
            status: "online".into(),
            current_task: Some("Testing".into()),
        };
        let json = serde_json::to_string(&agent).unwrap();
        assert!(json.contains("Test Agent"));
        assert!(json.contains("online"));
    }
}
