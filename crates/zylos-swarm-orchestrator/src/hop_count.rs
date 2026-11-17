//! # Hop Count Circuit Breaker
//!
//! Previne loops infinitos em DAGs multi-agente limitando a profundidade
//! máxima de spawns aninhados. Quando o contador atinge o limite configurado,
//! operações subsequentes retornam `Err(CircuitOpen)` sem panic.
//!
//! ## Configuração
//! O limite é configurável via `rules.json`:
//! ```json
//! { "hop_count_max": 8 }
//! ```
//! Default conservador: 8 níveis.
//!
//! ## Thread Safety
//! Usa `AtomicU8` para contagem lock-free segura em contextos async Tokio.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Erro retornado quando o circuit breaker abre (limite de hops excedido).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitOpen {
    /// Limite configurado que foi excedido.
    pub max_hops: u8,
    /// Valor atual do contador no momento da abertura.
    pub current: u8,
}

impl std::fmt::Display for CircuitOpen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Hop count circuit breaker open: {} >= max {}",
            self.current, self.max_hops
        )
    }
}

impl std::error::Error for CircuitOpen {}

/// Guard atômico que rastreia e limita a profundidade de spawns aninhados.
///
/// Clone-safe: todas as clones compartilham o mesmo contador atômico via `Arc`.
#[derive(Debug, Clone)]
pub struct HopCountGuard {
    inner: Arc<HopCountInner>,
}

#[derive(Debug)]
struct HopCountInner {
    current: AtomicU8,
    max: u8,
}

impl HopCountGuard {
    /// Cria um novo guard com o limite especificado.
    ///
    /// # Panics
    /// Panics se `max` for 0 (limite inválido).
    pub fn new(max: u8) -> Self {
        assert!(max > 0, "HopCountGuard max must be > 0");
        Self {
            inner: Arc::new(HopCountInner {
                current: AtomicU8::new(0),
                max,
            }),
        }
    }

    /// Cria um guard com o limite default de 8 hops.
    pub fn default_limit() -> Self {
        Self::new(8)
    }

    /// Tenta entrar em um novo nível de spawn.
    ///
    /// Retorna `Ok(previous_count)` se dentro do limite, ou
    /// `Err(CircuitOpen)` se o limite foi atingido/excedido.
    ///
    /// O contador é incrementado atomicamente ANTES da verificação,
    /// garantindo que nenhum outro thread possa passar simultaneamente.
    pub fn enter(&self) -> Result<u8, CircuitOpen> {
        let prev = self.inner.current.fetch_add(1, Ordering::SeqCst);
        if prev >= self.inner.max {
            // Rollback: decrementamos porque não vamos prosseguir
            self.inner.current.fetch_sub(1, Ordering::SeqCst);
            Err(CircuitOpen {
                max_hops: self.inner.max,
                current: prev + 1,
            })
        } else {
            Ok(prev)
        }
    }

    /// Libera um nível de spawn (decrementa o contador).
    ///
    /// Deve ser chamado em drop/exit de cada spawn aninhado para
    /// garantir que o contador reflita a profundidade real.
    /// Saturating sub previne underflow em cenários de erro.
    pub fn exit(&self) {
        self.inner
            .current
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                Some(v.saturating_sub(1))
            })
            .ok();
    }

    /// Retorna o valor atual do contador.
    pub fn current(&self) -> u8 {
        self.inner.current.load(Ordering::SeqCst)
    }

    /// Retorna o limite máximo configurado.
    pub fn max(&self) -> u8 {
        self.inner.max
    }

    /// Reseta o contador para zero. Útil para testes.
    pub fn reset(&self) {
        self.inner.current.store(0, Ordering::SeqCst);
    }

    /// Verifica se o circuit breaker está aberto sem modificar o contador.
    pub fn is_open(&self) -> bool {
        self.inner.current.load(Ordering::SeqCst) >= self.inner.max
    }
}

impl Drop for HopCountInner {
    fn drop(&mut self) {
        // Safety: reset on drop to prevent stale state in tests
        self.current.store(0, Ordering::SeqCst);
    }
}

/// RAII guard que automaticamente chama `exit()` ao sair do escopo.
///
/// Uso típico:
/// ```ignore
/// let _guard = hop_count.enter_guard()?;
/// // ... trabalho aninhado ...
/// // exit() chamado automaticamente no drop
/// ```
pub struct HopCountEnterGuard {
    parent: HopCountGuard,
}

impl HopCountGuard {
    /// Entra em um novo nível e retorna um RAII guard que libera automaticamente.
    pub fn enter_guard(&self) -> Result<HopCountEnterGuard, CircuitOpen> {
        self.enter()?;
        Ok(HopCountEnterGuard {
            parent: self.clone(),
        })
    }
}

impl Drop for HopCountEnterGuard {
    fn drop(&mut self) {
        self.parent.exit();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_enter_exit() {
        let guard = HopCountGuard::new(3);
        assert_eq!(guard.current(), 0);

        guard.enter().unwrap();
        assert_eq!(guard.current(), 1);

        guard.enter().unwrap();
        assert_eq!(guard.current(), 2);

        guard.exit();
        assert_eq!(guard.current(), 1);

        guard.exit();
        assert_eq!(guard.current(), 0);
    }

    #[test]
    fn test_circuit_opens_at_limit() {
        let guard = HopCountGuard::new(3);

        // Preencher até o limite
        guard.enter().unwrap(); // 0 -> 1
        guard.enter().unwrap(); // 1 -> 2
        guard.enter().unwrap(); // 2 -> 3

        // Próximo deve falhar
        let err = guard.enter().unwrap_err();
        assert_eq!(err.max_hops, 3);
        assert!(err.current >= 3);

        // Contador não deve ter sido incrementado além do limite
        assert_eq!(guard.current(), 3);
    }

    #[test]
    fn test_circuit_opens_prevents_further_entries() {
        let guard = HopCountGuard::new(2);

        guard.enter().unwrap();
        guard.enter().unwrap();

        // Múltiplas tentativas devem todas falhar
        assert!(guard.enter().is_err());
        assert!(guard.enter().is_err());
        assert!(guard.enter().is_err());

        // Contador estável no limite
        assert_eq!(guard.current(), 2);
    }

    #[test]
    fn test_exit_saturates_at_zero() {
        let guard = HopCountGuard::new(5);

        // Exit sem enter não deve underflow
        guard.exit();
        guard.exit();
        assert_eq!(guard.current(), 0);
    }

    #[test]
    fn test_raii_guard_auto_exits() {
        let guard = HopCountGuard::new(5);

        {
            let _g1 = guard.enter_guard().unwrap();
            assert_eq!(guard.current(), 1);

            {
                let _g2 = guard.enter_guard().unwrap();
                assert_eq!(guard.current(), 2);
            }
            // _g2 dropped, should be back to 1
            assert_eq!(guard.current(), 1);
        }
        // _g1 dropped, should be back to 0
        assert_eq!(guard.current(), 0);
    }

    #[test]
    fn test_raii_guard_exits_on_error_path() {
        let guard = HopCountGuard::new(3);

        let result: Result<(), Box<dyn std::error::Error>> = (|| {
            let _g = guard.enter_guard()?;
            assert_eq!(guard.current(), 1);
            // Simular erro early return
            Err("simulated error")?;
            #[allow(unreachable_code)]
            Ok(())
        })();

        assert!(result.is_err());
        // Guard deve ter sido liberado apesar do erro
        assert_eq!(guard.current(), 0);
    }

    #[test]
    fn test_clone_shares_counter() {
        let guard = HopCountGuard::new(5);
        let cloned = guard.clone();

        guard.enter().unwrap();
        assert_eq!(cloned.current(), 1);

        cloned.enter().unwrap();
        assert_eq!(guard.current(), 2);

        guard.exit();
        assert_eq!(cloned.current(), 1);
    }

    #[test]
    fn test_reset() {
        let guard = HopCountGuard::new(5);
        guard.enter().unwrap();
        guard.enter().unwrap();
        assert_eq!(guard.current(), 2);

        guard.reset();
        assert_eq!(guard.current(), 0);
    }

    #[test]
    fn test_is_open() {
        let guard = HopCountGuard::new(2);
        assert!(!guard.is_open());

        guard.enter().unwrap();
        assert!(!guard.is_open());

        guard.enter().unwrap();
        assert!(guard.is_open());
    }

    #[test]
    fn test_default_limit() {
        let guard = HopCountGuard::default_limit();
        assert_eq!(guard.max(), 8);
    }

    #[tokio::test]
    async fn test_concurrent_enter_exit() {
        let guard = HopCountGuard::new(100);
        let mut handles = Vec::new();

        for _ in 0..50 {
            let g = guard.clone();
            handles.push(tokio::spawn(async move {
                let _enter = g.enter_guard().unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                // _enter drops here, calling exit()
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // Após todos os spawns completarem, contador deve estar em 0
        assert_eq!(guard.current(), 0);
    }

    #[test]
    fn test_recovery_after_circuit_open() {
        let guard = HopCountGuard::new(2);

        // Esgotar limite
        guard.enter().unwrap();
        guard.enter().unwrap();
        assert!(guard.enter().is_err());

        // Liberar um slot
        guard.exit();
        assert_eq!(guard.current(), 1);

        // Agora deve funcionar novamente
        guard.enter().unwrap();
        assert_eq!(guard.current(), 2);
    }

    #[test]
    fn test_circuit_open_display() {
        let err = CircuitOpen {
            max_hops: 8,
            current: 9,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("8"));
        assert!(msg.contains("9"));
    }
}
