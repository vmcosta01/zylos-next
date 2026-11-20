//! Estado ao vivo dos agentes (`trabalhando`), vindo do SSE do gateway.
//!
//! O stream (bin) grava aqui cada `AgentStateChanged`; a lateral (lib) lê. O gateway publica
//! `running` quando o bot começa a responder e `idle` quando termina; `error` vem de falha.
//! A leitura periódica de `GET /api/v1/bots` reconcilia o mapa (ver `chat_panel`), então um
//! evento perdido numa queda do SSE não deixa o bot preso em "trabalhando".

use std::collections::HashMap;

use leptos::prelude::*;

/// Nome do bot → está respondendo agora. Fornecido por contexto.
#[derive(Clone, Copy)]
pub struct AgentStates(pub RwSignal<HashMap<String, bool>>);

/// Lê o mapa, se o stream o tiver fornecido (testes e previews não têm).
pub fn use_agent_states() -> Option<RwSignal<HashMap<String, bool>>> {
    use_context::<AgentStates>().map(|s| s.0)
}

/// `(bot, trabalhando)` de um evento `AgentStateChanged`; `None` para qualquer outro evento.
pub fn parse_agent_state(raw: &str) -> Option<(String, bool)> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    if v.get("event_type")?.as_str()? != "AgentStateChanged" {
        return None;
    }
    let d = v.get("data")?;
    let agent = d.get("agent_id")?.as_str()?.to_string();
    let novo = d.get("new_state")?.as_str()?;
    Some((agent, novo == "running"))
}

/// Aplica o evento ao mapa, se for de estado de agente.
pub fn apply_event(states: RwSignal<HashMap<String, bool>>, raw: &str) {
    if let Some((agent, working)) = parse_agent_state(raw) {
        states.update(|m| {
            m.insert(agent, working);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(novo: &str) -> String {
        format!(
            r#"{{"event_type":"AgentStateChanged","data":{{"agent_id":"vigia","previous_state":"x","new_state":"{novo}","reason":null}}}}"#
        )
    }

    #[test]
    fn running_e_trabalhando_e_o_resto_nao() {
        assert_eq!(
            parse_agent_state(&ev("running")),
            Some(("vigia".into(), true))
        );
        assert_eq!(
            parse_agent_state(&ev("idle")),
            Some(("vigia".into(), false))
        );
        assert_eq!(
            parse_agent_state(&ev("error")),
            Some(("vigia".into(), false))
        );
    }

    #[test]
    fn outros_eventos_e_lixo_sao_ignorados() {
        assert_eq!(
            parse_agent_state(r#"{"event_type":"ApprovalRequested","data":{}}"#),
            None
        );
        assert_eq!(
            parse_agent_state(r#"{"event_type":"AgentStateChanged","data":{}}"#),
            None
        );
        assert_eq!(parse_agent_state("não é json"), None);
    }
}
