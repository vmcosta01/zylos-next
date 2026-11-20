//! Módulo wasm "extras": os painéis secundários do cockpit, carregados sob demanda.
//!
//! O app principal (`zylos-cockpit`) só traz o shell, o chat, o Inbox, a Activity e as aprovações.
//! Ao abrir uma aba secundária, `ExtraHost` carrega `extras.js` + `extras_bg.wasm` (uma vez) e chama
//! `mount_extra`, que monta o painel num elemento da página com um runtime Leptos próprio.
//! Contextos (tema, toast, i18n) são recriados aqui; o estilo é o mesmo CSS global da página.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use zylos_cockpit::components::analytics_panel::AnalyticsPanel;
use zylos_cockpit::components::{
    provide_theme, provide_toast_store, AgentAvatarsGrid, AgentAwarenessPanel, ChannelPanel,
    CollabPanel, EmployeeGovernancePanel, EvidenceLedgerPanel, FederationDashboard, FileManager,
    FleetAuditorPanel, GoalsPanel, IntegrationsPanel, MemoryTimeline, PersonalitiesPanel,
    PhoneRelayPanel, SecurityPanel, SentinelPanel, SkillEditorPanel, SkillEvolutionPanel,
    SkinThemesPanel, SoarDashboard, SubagentStatusCards, SwarmDagPanel, SwarmTopologyPanel,
    TaskTriagePanel, TasksDag, ThreadManager, ToastContainer, VoicePanel,
};
use zylos_cockpit::i18n::{provide_i18n, Locale};

/// Painel de uma aba secundária (a chave é `Tab::css_class()`).
fn panel_for(key: &str) -> AnyView {
    match key {
        "tab-tasks" => view! {
            <div class="tab-panel tab-panel-tasks">
                <TasksDag />
            </div>
        }
        .into_any(),
        "tab-memory" => view! {
            <div class="tab-panel tab-panel-memory">
                <MemoryTimeline />
            </div>
        }
        .into_any(),
        "tab-goals" => view! {
            <div class="tab-panel tab-panel-goals">
                <GoalsPanel />
            </div>
        }
        .into_any(),
        "tab-personalities" => view! {
            <div class="tab-panel tab-panel-personalities">
                <PersonalitiesPanel />
            </div>
        }
        .into_any(),
        "tab-swarm-dag" => view! {
            <div class="tab-panel tab-panel-swarm-dag">
                <SwarmDagPanel />
            </div>
        }
        .into_any(),
        "tab-sentinel" => view! {
            <div class="tab-panel tab-panel-sentinel">
                <SentinelPanel />
            </div>
        }
        .into_any(),
        "tab-evidence-ledger" => view! {
            <div class="tab-panel tab-panel-evidence-ledger">
                <EvidenceLedgerPanel />
            </div>
        }
        .into_any(),
        "tab-soar" => view! {
            <div class="tab-panel tab-panel-soar">
                <SoarDashboard />
            </div>
        }
        .into_any(),
        "tab-security" => view! {
            <div class="tab-panel tab-panel-security">
                <SecurityPanel />
            </div>
        }
        .into_any(),
        "tab-awareness" => view! {
            <div class="tab-panel tab-panel-awareness">
                <AgentAwarenessPanel />
            </div>
        }
        .into_any(),
        "tab-skill-evolution" => view! {
            <div class="tab-panel tab-panel-skill-evolution">
                <SkillEvolutionPanel />
            </div>
        }
        .into_any(),
        "tab-employee-governance" => view! {
        <div class="tab-panel tab-panel-employee-governance">
        <EmployeeGovernancePanel />
        </div>
        }
        .into_any(),
        "tab-task-triage" => view! {
        <div class="tab-panel tab-panel-task-triage">
        <TaskTriagePanel />
        </div>
        }
        .into_any(),
        "tab-threads" => view! {
        <div class="tab-panel tab-panel-threads">
        <ThreadManager />
        </div>
        }
        .into_any(),
        "tab-subagents" => view! {
        <div class="tab-panel tab-panel-subagents">
        <SubagentStatusCards />
        </div>
        }
        .into_any(),
        "tab-files" => view! {
        <div class="tab-panel tab-panel-files">
        <FileManager />
        </div>
        }
        .into_any(),
        "tab-voice" => view! {
        <div class="tab-panel tab-panel-voice">
        <VoicePanel />
        </div>
        }
        .into_any(),
        "tab-reactions" => view! {
        <div class="tab-panel tab-panel-reactions">
        <p class="panel-subtitle">"Reaja às mensagens reais dos canais (persistidas no gateway)."</p>
        <ChannelPanel />
        </div>
        }
        .into_any(),
        "tab-avatars" => view! {
        <div class="tab-panel tab-panel-avatars">
        <AgentAvatarsGrid />
        </div>
        }
        .into_any(),
        "tab-skins" => view! {
        <div class="tab-panel tab-panel-skins">
        <SkinThemesPanel />
        </div>
        }
        .into_any(),
        "tab-fleet" => view! {
        <div class="tab-panel tab-panel-fleet">
        <FleetAuditorPanel />
        </div>
        }
        .into_any(),
        "tab-federation" => view! {
        <div class="tab-panel tab-panel-federation">
        <FederationDashboard />
        </div>
        }
        .into_any(),
        "tab-channels" => view! {
        <div class="tab-panel tab-panel-channels h-full">
        <ChannelPanel />
        </div>
        }
        .into_any(),
        "tab-phone-relay" => view! {
        <div class="tab-panel tab-panel-phone-relay">
        <PhoneRelayPanel />
        </div>
        }
        .into_any(),
        "tab-integrations" => view! {
        <div class="tab-panel tab-panel-integrations">
        <IntegrationsPanel />
        </div>
        }
        .into_any(),
        "tab-swarm-topology" => view! {
        <div class="tab-panel tab-panel-swarm-topology">
        <SwarmTopologyPanel />
        </div>
        }
        .into_any(),
        "tab-skill-editor" => view! {
        <div class="tab-panel tab-panel-skill-editor">
        <SkillEditorPanel />
        </div>
        }
        .into_any(),
        "tab-analytics" => view! {
        <div class="tab-panel tab-panel-analytics">
        <AnalyticsPanel />
        </div>
        }
        .into_any(),
        "tab-collab" => view! {
        <div class="tab-panel tab-panel-collab">
        <CollabPanel />
        </div>
        }
        .into_any(),
        other => view! { <p class="muted">{format!("Aba desconhecida: {other}")}</p> }.into_any(),
    }
}

/// Instância montada; `free()` no JS desmonta o painel.
#[wasm_bindgen]
pub struct ExtraHandle {
    _mounted: Box<dyn std::any::Any>,
}

/// Monta o painel da aba `key` dentro de `parent`.
#[wasm_bindgen]
pub fn mount_extra(key: &str, parent: web_sys::HtmlElement) -> ExtraHandle {
    console_error_panic_hook::set_once();
    let key = key.to_string();
    let handle = leptos::mount::mount_to(parent, move || {
        let _ = provide_theme();
        let _ = provide_toast_store();
        let _ = provide_i18n(Locale::PtBr);
        view! {
            {panel_for(&key)}
            <ToastContainer />
        }
    });
    ExtraHandle {
        _mounted: Box::new(handle),
    }
}

fn main() {}
