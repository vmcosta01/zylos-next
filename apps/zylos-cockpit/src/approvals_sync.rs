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
}
