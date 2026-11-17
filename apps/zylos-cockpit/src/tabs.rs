//! Abas do cockpit: enum, ordem de navegação, chaves i18n e mapeamento de atalhos.
//!
//! Módulo puro (sem Leptos) para ser testável em `cargo test --lib`.

/// Aba ativa do dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Agents,
    Tasks,
    Memory,
    Goals,
    Personalities,
    SwarmDag,
    Sentinel,
    EvidenceLedger,
    HitlApproval,
    Skills,
    Logs,
    Soar,
    Security,
    Awareness,
    SkillEvolution,
    EmployeeGovernance,
    TaskTriage,
    Threads,
    Subagents,
    Files,
    Voice,
    Reactions,
    Avatars,
    Skins,
    Fleet,
    Federation,
    Chat,
    Channels,
    PhoneRelay,
    Integrations,
    SwarmTopology,
    SkillEditor,
    Analytics,
    Collab,
}

impl Tab {
    /// Todas as abas, na ordem exibida na barra de navegação.
    pub const ALL: [Tab; 34] = [
        Tab::Agents,
        Tab::Tasks,
        Tab::Memory,
        Tab::Goals,
        Tab::Personalities,
        Tab::SwarmDag,
        Tab::Sentinel,
        Tab::EvidenceLedger,
        Tab::HitlApproval,
        Tab::Skills,
        Tab::Logs,
        Tab::Soar,
        Tab::Security,
        Tab::Awareness,
        Tab::SkillEvolution,
        Tab::EmployeeGovernance,
        Tab::TaskTriage,
        Tab::Threads,
        Tab::Subagents,
        Tab::Files,
        Tab::Voice,
        Tab::Reactions,
        Tab::Avatars,
        Tab::Skins,
        Tab::Fleet,
        Tab::Federation,
        Tab::Chat,
        Tab::Channels,
        Tab::PhoneRelay,
        Tab::Integrations,
        Tab::SwarmTopology,
        Tab::SkillEditor,
        Tab::Analytics,
        Tab::Collab,
    ];

    pub fn css_class(&self) -> &'static str {
        match self {
            Tab::Agents => "tab-agents",
            Tab::Tasks => "tab-tasks",
            Tab::Memory => "tab-memory",
            Tab::Goals => "tab-goals",
            Tab::Personalities => "tab-personalities",
            Tab::SwarmDag => "tab-swarm-dag",
            Tab::Sentinel => "tab-sentinel",
            Tab::EvidenceLedger => "tab-evidence-ledger",
            Tab::HitlApproval => "tab-hitl",
            Tab::Skills => "tab-skills",
            Tab::Logs => "tab-logs",
            Tab::Soar => "tab-soar",
            Tab::Security => "tab-security",
            Tab::Awareness => "tab-awareness",
            Tab::SkillEvolution => "tab-skill-evolution",
            Tab::EmployeeGovernance => "tab-employee-governance",
            Tab::TaskTriage => "tab-task-triage",
            Tab::Threads => "tab-threads",
            Tab::Subagents => "tab-subagents",
            Tab::Files => "tab-files",
            Tab::Voice => "tab-voice",
            Tab::Reactions => "tab-reactions",
            Tab::Avatars => "tab-avatars",
            Tab::Skins => "tab-skins",
            Tab::Fleet => "tab-fleet",
            Tab::Federation => "tab-federation",
            Tab::Chat => "tab-chat",
            Tab::Channels => "tab-channels",
            Tab::PhoneRelay => "tab-phone-relay",
            Tab::Integrations => "tab-integrations",
            Tab::SwarmTopology => "tab-swarm-topology",
            Tab::SkillEditor => "tab-skill-editor",
            Tab::Analytics => "tab-analytics",
            Tab::Collab => "tab-collab",
        }
    }

    pub fn i18n_key(&self) -> &'static str {
        match self {
            Tab::Agents => "tab.agents",
            Tab::Tasks => "tab.tasks",
            Tab::Memory => "tab.memory",
            Tab::Goals => "tab.goals",
            Tab::Personalities => "tab.personalities",
            Tab::SwarmDag => "tab.swarm_dag",
            Tab::Sentinel => "tab.sentinel",
            Tab::EvidenceLedger => "tab.evidence_ledger",
            Tab::HitlApproval => "tab.hitl_approval",
            Tab::Skills => "tab.skills",
            Tab::Logs => "tab.logs",
            Tab::Soar => "tab.soar",
            Tab::Security => "tab.security",
            Tab::Awareness => "tab.awareness",
            Tab::SkillEvolution => "tab.skill_evolution",
            Tab::EmployeeGovernance => "tab.employee_governance",
            Tab::TaskTriage => "tab.task_triage",
            Tab::Threads => "tab.threads",
            Tab::Subagents => "tab.subagents",
            Tab::Files => "tab.files",
            Tab::Voice => "tab.voice",
            Tab::Reactions => "tab.reactions",
            Tab::Avatars => "tab.avatars",
            Tab::Skins => "tab.skins",
            Tab::Fleet => "tab.fleet",
            Tab::Federation => "tab.federation",
            Tab::Chat => "tab.chat",
            Tab::Channels => "tab.channels",
            Tab::PhoneRelay => "tab.phone_relay",
            Tab::Integrations => "tab.integrations",
            Tab::SwarmTopology => "tab.swarm_topology",
            Tab::SkillEditor => "tab.skill_editor",
            Tab::Analytics => "tab.analytics",
            Tab::Collab => "tab.collab",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tem_todas_as_abas_sem_repeticao() {
        let mut keys: Vec<_> = Tab::ALL.iter().map(|t| t.i18n_key()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), Tab::ALL.len());
        let mut css: Vec<_> = Tab::ALL.iter().map(|t| t.css_class()).collect();
        css.sort();
        css.dedup();
        assert_eq!(css.len(), Tab::ALL.len());
    }
}
