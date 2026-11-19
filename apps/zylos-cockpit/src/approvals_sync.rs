//! Sinal compartilhado entre o stream SSE (bin) e os componentes HITL (lib).
//!
//! O stream sobe o contador a cada evento de aprovação (pedida, resolvida, expirada);
//! o painel HITL e as aprovações inline no chat refazem o `GET /api/v1/approvals` quando ele muda.

use leptos::prelude::*;

/// Contador de eventos de aprovação, fornecido por contexto.
#[derive(Clone, Copy)]
pub struct ApprovalsTick(pub RwSignal<u32>);

/// Lê o contador, se o stream o tiver fornecido (testes e previews não têm).
pub fn use_approvals_tick() -> Option<RwSignal<u32>> {
    use_context::<ApprovalsTick>().map(|t| t.0)
}

/// Sobe o contador em 1 (volta a zero ao estourar `u32`).
pub fn bump(tick: RwSignal<u32>) {
    tick.update(|t| *t = t.wrapping_add(1));
}

/// Tipo de uma entrada do log de atividade (filtro da aba Activity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    Approval,
    Alert,
    Agent,
}

/// Uma linha da linha do tempo: o que o gateway avisou pelo SSE.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityEntry {
    pub kind: ActivityKind,
    pub text: String,
    /// Instante em RFC 3339 (UTC); a tela formata no fuso local.
    pub at: String,
}

/// Quantas entradas o log guarda (as mais antigas saem primeiro).
pub const ACTIVITY_MAX: usize = 200;

/// Log de atividade fornecido por contexto (em memória; recarregar a página limpa).
#[derive(Clone, Copy)]
pub struct ActivityLog(pub RwSignal<Vec<ActivityEntry>>);

/// Lê o log, se o stream o tiver fornecido.
pub fn use_activity_log() -> Option<RwSignal<Vec<ActivityEntry>>> {
    use_context::<ActivityLog>().map(|l| l.0)
}

/// Põe a entrada no topo e corta o excesso.
pub fn record_activity(log: RwSignal<Vec<ActivityEntry>>, entry: ActivityEntry) {
    log.update(|v| {
        v.insert(0, entry);
        v.truncate(ACTIVITY_MAX);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_incrementa_e_da_a_volta() {
        let s = RwSignal::new(u32::MAX);
        bump(s);
        assert_eq!(s.get_untracked(), 0);
        bump(s);
        assert_eq!(s.get_untracked(), 1);
    }

    #[test]
    fn sem_contexto_devolve_none() {
        assert!(use_approvals_tick().is_none());
    }

    #[test]
    fn log_guarda_o_mais_recente_primeiro_e_respeita_o_limite() {
        let log = RwSignal::new(Vec::new());
        for i in 0..(ACTIVITY_MAX + 5) {
            record_activity(
                log,
                ActivityEntry {
                    kind: ActivityKind::Alert,
                    text: format!("evento {i}"),
                    at: "2026-11-19T12:00:00Z".into(),
                },
            );
        }
        let v = log.get_untracked();
        assert_eq!(v.len(), ACTIVITY_MAX);
        assert_eq!(v[0].text, format!("evento {}", ACTIVITY_MAX + 4));
    }
}
