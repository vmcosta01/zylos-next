//! AgentsPanel — agentes reais do swarm via `GET /api/v1/agents`.
//!
//! Sem agentes registrados mostra estado vazio honesto; em erro, mensagem
//! curta de [`crate::ui_error::report`]. Nada de dados de exemplo.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

use super::{AgentInfo, AgentStatus, EmptyState, LazyRoster};

/// Agente como devolvido pelo gateway.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GatewayAgent {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub task_count: usize,
    #[serde(default)]
    pub last_active: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentsResponse {
    agents: Vec<GatewayAgent>,
}

/// Mapeia o status textual do gateway.
pub fn parse_status(s: &str) -> AgentStatus {
    match s {
        "running" => AgentStatus::Running,
        "blocked" => AgentStatus::Blocked,
        "archived" => AgentStatus::Archived,
        _ => AgentStatus::Idle,
    }
}

/// `2026-11-17T23:40:05.123+00:00` → `2026-11-17 23:40` (UTC).
pub fn short_timestamp(iso: &str) -> String {
    if iso.len() >= 16 && iso.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &iso[..10], &iso[11..16])
    } else if iso.is_empty() {
        "—".to_string()
    } else {
        iso.to_string()
    }
}

/// Converte para o modelo do card.
pub fn to_agent_info(a: GatewayAgent) -> AgentInfo {
    AgentInfo {
        status: parse_status(&a.status),
        last_active: short_timestamp(&a.last_active),
        id: a.id,
        name: a.name,
        task_count: a.task_count,
        activity: Vec::new(), // API não fornece sparkline data ainda
    }
}

async fn fetch_agents() -> Result<Vec<AgentInfo>, String> {
    use gloo_net::http::Request;
    let base = crate::gateway_config::gateway_base()
        .ok_or("Gateway não configurado (meta zylos-gateway-url)")?;
    let url = crate::gateway_config::join_url(&base, "/api/v1/agents");
    let resp = crate::auth::authed(Request::get(&url))
        .send()
        .await
        .map_err(|e| format!("{e:?}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let r: AgentsResponse = resp.json().await.map_err(|e| format!("{e:?}"))?;
    Ok(r.agents.into_iter().map(to_agent_info).collect())
}

/// Aba Agentes.
#[component]
pub fn AgentsPanel() -> impl IntoView {
    // None = carregando; Some(Ok) = dados; Some(Err) = mensagem amigável.
    let data = RwSignal::new(None::<Result<Vec<AgentInfo>, String>>);
    let load = move || {
        spawn_local(async move {
            let r = fetch_agents()
                .await
                .map_err(|e| crate::ui_error::report("agents", &e));
            data.set(Some(r));
        });
    };
    Effect::new(move |_| load());

    view! {
        {move || match data.get() {
            None => view! { <div class="roster-loading">"Carregando agentes..."</div> }.into_any(),
            Some(Err(msg)) => view! {
                <div class="error-banner" role="alert">
                    <p>{msg}</p>
                    <button class="cta-retry" on:click=move |_| load()>"Tentar novamente"</button>
                </div>
            }.into_any(),
            Some(Ok(agents)) if agents.is_empty() => view! {
                <EmptyState
                    icon=crate::components::empty_state::icons::ROBOT
                    title="Nenhum agente registrado".to_string()
                    description="Agentes aparecerão aqui quando registrados no swarm.".to_string()
                />
            }.into_any(),
            Some(Ok(agents)) => view! { <LazyRoster agents=agents max_visible=20 /> }.into_any(),
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converte_agente_do_gateway() {
        let a: GatewayAgent = serde_json::from_str(
            r#"{"id":"a","name":"N","status":"running","task_count":4,
                "last_active":"2026-11-17T23:40:05.1+00:00","extra":1}"#,
        )
        .unwrap();
        let i = to_agent_info(a);
        assert_eq!(i.status, AgentStatus::Running);
        assert_eq!(i.task_count, 4);
        assert_eq!(i.last_active, "2026-11-17 23:40");
    }

    #[test]
    fn status_e_data_desconhecidos() {
        assert_eq!(parse_status("xyz"), AgentStatus::Idle);
        assert_eq!(parse_status("archived"), AgentStatus::Archived);
        assert_eq!(short_timestamp(""), "—");
    }
}
