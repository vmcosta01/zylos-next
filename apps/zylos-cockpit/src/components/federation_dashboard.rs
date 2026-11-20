//! FederationDashboard — os outros gateways ("nós") que este gateway enxerga, de verdade.
//!
//! Lê `GET /api/v1/federation`: para cada nó configurado em `ZYLOS_FEDERATION_PEERS`, se está no
//! ar, em quanto tempo respondeu e que versão diz ser. Não há relógio híbrido, vetor de clock nem
//! conflitos: o gateway não tem essas peças, então o painel não as mostra.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

/// Um nó, como o gateway devolve.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Peer {
    pub id: String,
    pub url: String,
    /// `online`, `degraded` ou `offline`.
    pub status: String,
    pub latency_ms: Option<u64>,
    pub version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct FederationResp {
    peers: Vec<Peer>,
    checked_at: String,
}

/// Rótulo em palavras (a cor só reforça).
pub fn status_label(s: &str) -> &'static str {
    match s {
        "online" => "No ar",
        "degraded" => "Degradado",
        _ => "Fora do ar",
    }
}

fn status_class(s: &str) -> &'static str {
    match s {
        "online" => "fed-status fed-status-online",
        "degraded" => "fed-status fed-status-degraded",
        _ => "fed-status fed-status-offline",
    }
}

/// Resumo da federação: quantos nós, quantos no ar, latência média dos que responderam.
#[derive(Debug, Clone, PartialEq)]
pub struct Resumo {
    pub total: usize,
    pub online: usize,
    pub latencia_media_ms: Option<u64>,
}

pub fn resumir(peers: &[Peer]) -> Resumo {
    let lat: Vec<u64> = peers.iter().filter_map(|p| p.latency_ms).collect();
    Resumo {
        total: peers.len(),
        online: peers.iter().filter(|p| p.status == "online").count(),
        latencia_media_ms: (!lat.is_empty()).then(|| lat.iter().sum::<u64>() / lat.len() as u64),
    }
}

#[component]
pub fn FederationDashboard() -> impl IntoView {
    let peers = RwSignal::new(Vec::<Peer>::new());
    let checado = RwSignal::new(None::<String>);
    let erro = RwSignal::new(None::<String>);
    let loading = RwSignal::new(true);
    let reload = RwSignal::new(0u32);

    Effect::new(move |_| {
        let _ = reload.get();
        spawn_local(async move {
            loading.set(true);
            match crate::gateway_http::get_json::<FederationResp>("/api/v1/federation").await {
                Ok(r) => {
                    peers.set(r.peers);
                    checado.set(Some(r.checked_at));
                    erro.set(None);
                }
                Err(e) => erro.set(Some(crate::ui_error::report("federação", &e))),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="federation-dashboard">
            <div class="fed-header">
                <h3 class="fed-title"><i class="ph ph-globe-hemisphere-west" aria-hidden="true"></i>" Federação"</h3>
                <button
                    class="fed-refresh-btn"
                    type="button"
                    title="Sondar de novo"
                    on:click=move |_| reload.update(|n| *n += 1)
                >
                    <i class="ph ph-arrows-clockwise" aria-hidden="true"></i>" Sondar"
                </button>
            </div>
            <p class="fed-subtitle">
                "Outros gateways configurados em ZYLOS_FEDERATION_PEERS, sondados pelo /api/v1/health de cada um."
                {move || checado.get().map(|c| format!(" Última sondagem: {}.", c.chars().take(19).collect::<String>().replace('T', " ")))}
            </p>

            {move || erro.get().map(|e| view! { <div class="fed-empty" role="alert">{e}</div> })}

            {move || {
                if loading.get() {
                    view! { <div class="fed-loading"><span class="spinner"></span><span>"Sondando os nós..."</span></div> }.into_any()
                } else if peers.get().is_empty() && erro.get().is_none() {
                    view! {
                        <div class="fed-empty">
                            "Nenhum nó configurado. Defina ZYLOS_FEDERATION_PEERS no gateway com uma lista nome=url separada por vírgula (até 16), por exemplo leste=http://10.0.0.5:3000."
                        </div>
                    }.into_any()
                } else if peers.get().is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    let r = resumir(&peers.get());
                    view! {
                        <div class="fed-summary">
                            <div class="fed-summary-card"><span class="fed-summary-label">"Nós"</span><span class="fed-summary-value">{r.total}</span></div>
                            <div class="fed-summary-card"><span class="fed-summary-label">"No ar"</span><span class="fed-summary-value">{r.online}{" de "}{r.total}</span></div>
                            <div class="fed-summary-card">
                                <span class="fed-summary-label">"Latência média"</span>
                                <span class="fed-summary-value">{r.latencia_media_ms.map(|m| format!("{m} ms")).unwrap_or_else(|| "—".into())}</span>
                            </div>
                        </div>
                        <ul class="fed-node-list">
                            <For
                                each=move || peers.get()
                                key=|p| p.id.clone()
                                children=move |p| {
                                    view! {
                                        <li class="fed-node">
                                            <div class="fed-node-header">
                                                <span class="fed-node-name">{p.id.clone()}</span>
                                                <span class=status_class(&p.status)>{status_label(&p.status)}</span>
                                            </div>
                                            <div class="fed-node-meta">
                                                <span>{p.url.clone()}</span>
                                                <span>{p.latency_ms.map(|m| format!("{m} ms")).unwrap_or_else(|| "sem resposta".into())}</span>
                                                {p.version.clone().map(|v| view! { <span>{format!("versão {v}")}</span> })}
                                            </div>
                                            {p.error.clone().map(|e| view! { <div class="fed-node-error">{e}</div> })}
                                        </li>
                                    }
                                }
                            />
                        </ul>
                    }.into_any()
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(id: &str, status: &str, lat: Option<u64>) -> Peer {
        Peer {
            id: id.into(),
            url: format!("http://{id}"),
            status: status.into(),
            latency_ms: lat,
            version: None,
            error: None,
        }
    }

    #[test]
    fn resposta_do_gateway_le_com_nulos() {
        let r: FederationResp = serde_json::from_str(
            r#"{"self":{"version":"0.1.0"},"checked_at":"2026-11-20T10:00:00Z","peers":[
                {"id":"a","url":"http://a","status":"online","latency_ms":12,"version":"1.0","error":null},
                {"id":"b","url":"http://b","status":"offline","latency_ms":null,"version":null,"error":"tempo esgotado"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(r.peers[0].latency_ms, Some(12));
        assert_eq!(r.peers[1].error.as_deref(), Some("tempo esgotado"));
    }

    #[test]
    fn resumo_conta_so_os_que_responderam_na_latencia() {
        let r = resumir(&[
            peer("a", "online", Some(10)),
            peer("b", "degraded", Some(30)),
            peer("c", "offline", None),
        ]);
        assert_eq!(r.total, 3);
        assert_eq!(r.online, 1);
        assert_eq!(r.latencia_media_ms, Some(20));
        assert_eq!(resumir(&[]).latencia_media_ms, None);
    }

    #[test]
    fn rotulos_em_palavras() {
        assert_eq!(status_label("online"), "No ar");
        assert_eq!(status_label("degraded"), "Degradado");
        assert_eq!(status_label("qualquer"), "Fora do ar");
    }
}
