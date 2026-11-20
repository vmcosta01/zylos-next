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
    let initials = match &agent.division {
        // Agente real (divisão personalizada): as iniciais do nome, não "??".
        AgentDivision::Custom(_) => iniciais_do_nome(&agent.name),
        outra => outra.initials().to_string(),
    };
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
    let agents = RwSignal::new(Vec::<AgentAvatarInfo>::new());
    let loading = RwSignal::new(true);
    let erro = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            loading.set(true);
            match carregar_agentes().await {
                Ok(lista) => {
                    agents.set(lista);
                    erro.set(None);
                }
                Err(e) => erro.set(Some(crate::ui_error::report("avatares", &e))),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="agent-avatars-panel">
            <div class="agent-avatars-header">
                <h3 class="agent-avatars-title"><i class="ph ph-users-three" aria-hidden="true"></i>" Equipe de Agentes"</h3>
                <span class="agent-avatars-count">
                    {move || format!("{} agentes", agents.get().len())}
                </span>
            </div>

            {move || erro.get().map(|e| view! { <div class="agent-avatars-empty" role="alert">{e}</div> })}
            {move || (loading.get()).then(|| view! { <div class="agent-avatars-empty">"Carregando agentes..."</div> })}
            {move || (!loading.get() && erro.get().is_none() && agents.get().is_empty()).then(|| view! {
                <div class="agent-avatars-empty">"Nenhum agente: crie um bot na pasta de bots do gateway ou registre um agente no swarm."</div>
            })}

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

        </div>
    }
}

/// Até duas letras do nome, em maiúsculas (`Bot Lento` -> `BL`, `vigia` -> `VI`).
pub fn iniciais_do_nome(nome: &str) -> String {
    let palavras: Vec<&str> = nome.split_whitespace().collect();
    let letras: String = if palavras.len() >= 2 {
        palavras
            .iter()
            .take(2)
            .filter_map(|p| p.chars().next())
            .collect()
    } else {
        nome.chars().take(2).collect()
    };
    letras.to_uppercase()
}

#[derive(Debug, Deserialize)]
struct BotsResp {
    bots: Vec<BotLinha>,
}

#[derive(Debug, Deserialize)]
struct BotLinha {
    nome: String,
    provedor: Option<String>,
    #[serde(default)]
    nome_exibicao: Option<String>,
    problema: Option<String>,
    #[serde(default)]
    estado: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SwarmResp {
    agents: Vec<SwarmLinha>,
}

#[derive(Debug, Deserialize)]
struct SwarmLinha {
    id: String,
    name: String,
    status: String,
}

/// Os agentes de verdade: bots da pasta de bots (sem os que têm problema) e agentes registrados
/// no swarm. O estado "ocupado" só vem de `estado = trabalhando` (bot) ou `running` (swarm).
fn agentes_reais(bots: BotsResp, swarm: Option<SwarmResp>) -> Vec<AgentAvatarInfo> {
    let mut lista: Vec<AgentAvatarInfo> = bots
        .bots
        .into_iter()
        .filter(|b| b.problema.is_none())
        .map(|b| {
            let trabalhando = b.estado.as_deref() == Some("trabalhando");
            AgentAvatarInfo {
                id: format!("bot-{}", b.nome),
                name: b.nome_exibicao.unwrap_or(b.nome),
                division: AgentDivision::Custom(b.provedor.unwrap_or_else(|| "bot".into())),
                status: if trabalhando { "busy" } else { "online" }.into(),
                current_task: trabalhando.then(|| "Respondendo uma pergunta".to_string()),
            }
        })
        .collect();
    for a in swarm.map(|s| s.agents).unwrap_or_default() {
        let ocupado = a.status == "running";
        lista.push(AgentAvatarInfo {
            id: format!("swarm-{}", a.id),
            name: a.name,
            division: AgentDivision::Custom("swarm".into()),
            status: match a.status.as_str() {
                "running" => "busy",
                "archived" => "offline",
                _ => "online",
            }
            .into(),
            current_task: ocupado.then(|| "Executando uma tarefa".to_string()),
        });
    }
    lista
}

async fn carregar_agentes() -> Result<Vec<AgentAvatarInfo>, String> {
    let bots: BotsResp = crate::gateway_http::get_json("/api/v1/bots").await?;
    // O swarm é opcional: sem ele (ou com erro) a lista só tem os bots.
    let swarm: Option<SwarmResp> = crate::gateway_http::get_json("/api/v1/agents").await.ok();
    Ok(agentes_reais(bots, swarm))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iniciais_do_nome_de_um_ou_mais_palavras() {
        assert_eq!(iniciais_do_nome("Bot Lento"), "BL");
        assert_eq!(iniciais_do_nome("vigia"), "VI");
        assert_eq!(iniciais_do_nome("x"), "X");
        assert_eq!(iniciais_do_nome("  Agente   alfa  "), "AA");
    }

    #[test]
    fn agentes_reais_vem_de_bots_e_swarm_sem_inventar_tarefa() {
        let bots: BotsResp = serde_json::from_str(
            r#"{"bots":[
                {"nome":"vigia","provedor":"nvidia","nome_exibicao":"Vigia da Noite","problema":null,"estado":"trabalhando"},
                {"nome":"arq","provedor":"openai","problema":null,"estado":"ocioso"},
                {"nome":"quebrado","provedor":null,"problema":"falta ALMA.md"}
            ]}"#,
        )
        .unwrap();
        let swarm: SwarmResp = serde_json::from_str(
            r#"{"agents":[{"id":"a1","name":"Agente A","status":"running"},{"id":"a2","name":"Agente B","status":"idle"}]}"#,
        )
        .unwrap();
        let l = agentes_reais(bots, Some(swarm));
        let nomes: Vec<_> = l.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(nomes, ["Vigia da Noite", "arq", "Agente A", "Agente B"]);
        assert_eq!(l[0].status, "busy");
        assert_eq!(
            l[0].current_task.as_deref(),
            Some("Respondendo uma pergunta")
        );
        assert_eq!(l[1].status, "online");
        assert_eq!(l[1].current_task, None, "ocioso: nenhuma tarefa inventada");
        assert_eq!(l[2].status, "busy");
        assert_eq!(l[3].current_task, None);
        // Sem swarm (rota indisponível): só os bots.
        let so_bots: BotsResp =
            serde_json::from_str(r#"{"bots":[{"nome":"a","provedor":"x","problema":null}]}"#)
                .unwrap();
        assert_eq!(agentes_reais(so_bots, None).len(), 1);
    }

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
