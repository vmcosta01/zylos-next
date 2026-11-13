//! # HostAllocatorLimiter — Limite de heap do processo host (T8.3.6)
//!
//! Previne DoS por exaustão de memória limitando o uso de heap do processo
//! gateway/daemon. Configura-se via variável de ambiente ou API.
//!
//! ## Mecanismo
//! Usa um contador atômico global para rastrear alocações. Quando o limite
//! é atingido, novas alocações retornam erro em vez de causar OOM kill.
//!
//! ## Configuração
//! ```bash
//! ZYLOS_HOST_MEMORY_LIMIT_MB=1024  # 1 GB
//! ```
//!
//! ## Limitações
//! Este é um soft limit cooperativo: depende dos componentes chamarem
//! `check_allocation()` antes de alocações grandes. Não substitui cgroups
//! ou Docker memory limits (hard limits).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Erro retornado quando o limite de memória é excedido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryLimitExceeded {
 /// Limite configurado em bytes.
 pub limit_bytes: u64,
 /// Uso atual em bytes.
 pub current_bytes: u64,
 /// Tamanho da alocação tentada em bytes.
 pub requested_bytes: u64,
}

impl std::fmt::Display for MemoryLimitExceeded {
 fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
 write!(
 f,
 "Host memory limit exceeded: {} bytes requested, {} / {} bytes used",
 self.requested_bytes, self.current_bytes, self.limit_bytes
 )
 }
}

impl std::error::Error for MemoryLimitExceeded {}

/// Estado atual do limitador de memória.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryStatus {
 /// Limite configurado em bytes (0 = ilimitado).
 pub limit_bytes: u64,
 /// Uso atual rastreado em bytes.
 pub current_bytes: u64,
 /// Pico de uso observado em bytes.
 pub peak_bytes: u64,
 /// Número de alocações permitidas.
 pub allocations_allowed: u64,
 /// Número de alocações rejeitadas.
 pub allocations_rejected: u64,
 /// Se o limitador está ativo.
 pub is_active: bool,
 /// Percentual de uso (0.0 - 1.0).
 pub usage_ratio: f64,
}

/// Limitador de memória do processo host.
///
/// Thread-safe via contadores atômicos. Clone-safe (compartilha estado via Arc).
#[derive(Clone)]
pub struct HostAllocatorLimiter {
 inner: Arc<HostAllocatorInner>,
}

struct HostAllocatorInner {
 /// Limite em bytes (0 = ilimitado).
 limit_bytes: AtomicU64,
 /// Uso atual em bytes.
 current_bytes: AtomicU64,
 /// Pico de uso em bytes.
 peak_bytes: AtomicU64,
 /// Contador de alocações permitidas.
 allocations_allowed: AtomicU64,
 /// Contador de alocações rejeitadas.
 allocations_rejected: AtomicU64,
}

impl HostAllocatorLimiter {
 /// Cria um novo limitador com o limite especificado em bytes.
 ///
 /// Use 0 para desativar o limite (ilimitado).
 pub fn new(limit_bytes: u64) -> Self {
 Self {
 inner: Arc::new(HostAllocatorInner {
 limit_bytes: AtomicU64::new(limit_bytes),
 current_bytes: AtomicU64::new(0),
 peak_bytes: AtomicU64::new(0),
 allocations_allowed: AtomicU64::new(0),
 allocations_rejected: AtomicU64::new(0),
 }),
 }
 }

 /// Cria um limitador a partir da variável de ambiente `ZYLOS_HOST_MEMORY_LIMIT_MB`.
 ///
 /// Retorna limitador desativado (limit=0) se a variável não estiver definida
 /// ou contiver valor inválido.
 pub fn from_env() -> Self {
 let limit_mb = std::env::var("ZYLOS_HOST_MEMORY_LIMIT_MB")
 .ok()
 .and_then(|v| v.parse::<u64>().ok())
 .unwrap_or(0);

 let limit_bytes = limit_mb * 1024 * 1024;
 tracing::info!(
 limit_mb = limit_mb,
 limit_bytes = limit_bytes,
 "HostAllocatorLimiter initialized from env"
 );
 Self::new(limit_bytes)
 }

 /// Cria um limitador desativado (sem limite).
 pub fn unlimited() -> Self {
 Self::new(0)
 }

 /// Verifica se uma alocação de `bytes` é permitida.
 ///
 /// Se permitida, incrementa o contador de uso e retorna Ok.
 /// Se rejeitada, retorna Err(MemoryLimitExceeded) sem modificar contadores.
 pub fn check_allocation(&self, bytes: u64) -> Result<(), MemoryLimitExceeded> {
 let limit = self.inner.limit_bytes.load(Ordering::Relaxed);

 // Limite 0 = ilimitado
 if limit == 0 {
 self.inner.allocations_allowed.fetch_add(1, Ordering::Relaxed);
 return Ok(());
 }

 let current = self.inner.current_bytes.load(Ordering::Relaxed);
 let new_total = current + bytes;

 if new_total > limit {
 self.inner.allocations_rejected.fetch_add(1, Ordering::Relaxed);
 Err(MemoryLimitExceeded {
 limit_bytes: limit,
 current_bytes: current,
 requested_bytes: bytes,
 })
 } else {
 // CAS loop para atualização segura
 loop {
 match self.inner.current_bytes.compare_exchange_weak(
 current,
 new_total,
 Ordering::SeqCst,
 Ordering::Relaxed,
 ) {
 Ok(_) => break,
 Err(actual) => {
 // Outro thread modificou; recalcular
 let new_actual = actual + bytes;
 if new_actual > limit {
 self.inner.allocations_rejected.fetch_add(1, Ordering::Relaxed);
 return Err(MemoryLimitExceeded {
 limit_bytes: limit,
 current_bytes: actual,
 requested_bytes: bytes,
 });
 }
 // Continuar CAS com valor atualizado
 continue;
 }
 }
 }

 // Atualizar pico se necessário
 self.update_peak(new_total);
 self.inner.allocations_allowed.fetch_add(1, Ordering::Relaxed);
 Ok(())
 }
 }

 /// Libera `bytes` do contador de uso (após dealocação).
 ///
 /// Saturating sub previne underflow em cenários de erro.
 pub fn release(&self, bytes: u64) {
 self.inner
 .current_bytes
 .try_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
 Some(v.saturating_sub(bytes))
 })
 .ok();
 }

 /// Define um novo limite em runtime.
 pub fn set_limit(&self, limit_bytes: u64) {
 self.inner.limit_bytes.store(limit_bytes, Ordering::SeqCst);
 tracing::info!(limit_bytes = limit_bytes, "HostAllocatorLimiter: limit updated");
 }

 /// Retorna o limite atual em bytes.
 pub fn limit(&self) -> u64 {
 self.inner.limit_bytes.load(Ordering::Relaxed)
 }

 /// Retorna o uso atual em bytes.
 pub fn current_usage(&self) -> u64 {
 self.inner.current_bytes.load(Ordering::Relaxed)
 }

 /// Retorna o status completo do limitador.
 pub fn status(&self) -> MemoryStatus {
 let limit = self.inner.limit_bytes.load(Ordering::Relaxed);
 let current = self.inner.current_bytes.load(Ordering::Relaxed);
 let peak = self.inner.peak_bytes.load(Ordering::Relaxed);
 let allowed = self.inner.allocations_allowed.load(Ordering::Relaxed);
 let rejected = self.inner.allocations_rejected.load(Ordering::Relaxed);

 MemoryStatus {
 limit_bytes: limit,
 current_bytes: current,
 peak_bytes: peak,
 allocations_allowed: allowed,
 allocations_rejected: rejected,
 is_active: limit > 0,
 usage_ratio: if limit > 0 {
 current as f64 / limit as f64
 } else {
 0.0
 },
 }
 }

 /// Reseta todos os contadores (para testes).
 pub fn reset(&self) {
 self.inner.current_bytes.store(0, Ordering::SeqCst);
 self.inner.peak_bytes.store(0, Ordering::SeqCst);
 self.inner.allocations_allowed.store(0, Ordering::SeqCst);
 self.inner.allocations_rejected.store(0, Ordering::SeqCst);
 }

 /// Atualiza o pico de uso se o valor atual for maior.
 fn update_peak(&self, current: u64) {
 loop {
 let peak = self.inner.peak_bytes.load(Ordering::Relaxed);
 if current <= peak {
 break;
 }
 if self
 .inner
 .peak_bytes
 .compare_exchange_weak(peak, current, Ordering::SeqCst, Ordering::Relaxed)
 .is_ok()
 {
 break;
 }
 }
 }
}

impl Default for HostAllocatorLimiter {
 fn default() -> Self {
 Self::unlimited()
 }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn test_unlimited_allows_everything() {
 let limiter = HostAllocatorLimiter::unlimited();
 assert!(limiter.check_allocation(1024 * 1024 * 1024).is_ok());
 assert!(limiter.check_allocation(u64::MAX / 2).is_ok());

 let status = limiter.status();
 assert!(!status.is_active);
 assert_eq!(status.limit_bytes, 0);
 }

 #[test]
 fn test_within_limit() {
 let limiter = HostAllocatorLimiter::new(1000);

 assert!(limiter.check_allocation(500).is_ok());
 assert_eq!(limiter.current_usage(), 500);

 assert!(limiter.check_allocation(400).is_ok());
 assert_eq!(limiter.current_usage(), 900);
 }

 #[test]
 fn test_exceeds_limit() {
 let limiter = HostAllocatorLimiter::new(1000);

 assert!(limiter.check_allocation(600).is_ok());
 let result = limiter.check_allocation(500);
 assert!(result.is_err());

 let err = result.unwrap_err();
 assert_eq!(err.limit_bytes, 1000);
 assert_eq!(err.current_bytes, 600);
 assert_eq!(err.requested_bytes, 500);

 // Uso não deve ter sido incrementado após rejeição
 assert_eq!(limiter.current_usage(), 600);
 }

 #[test]
 fn test_release_frees_space() {
 let limiter = HostAllocatorLimiter::new(1000);

 limiter.check_allocation(800).unwrap();
 assert_eq!(limiter.current_usage(), 800);

 limiter.release(300);
 assert_eq!(limiter.current_usage(), 500);

 // Agora deve aceitar mais
 assert!(limiter.check_allocation(400).is_ok());
 assert_eq!(limiter.current_usage(), 900);
 }

 #[test]
 fn test_release_saturates_at_zero() {
 let limiter = HostAllocatorLimiter::new(1000);

 // Release sem alocação prévia não deve underflow
 limiter.release(100);
 limiter.release(999);
 assert_eq!(limiter.current_usage(), 0);
 }

 #[test]
 fn test_peak_tracking() {
 let limiter = HostAllocatorLimiter::new(10000);

 limiter.check_allocation(500).unwrap();
 limiter.check_allocation(300).unwrap(); // total: 800
 limiter.release(400); // total: 400
 limiter.check_allocation(200).unwrap(); // total: 600

 let status = limiter.status();
 assert_eq!(status.peak_bytes, 800);
 assert_eq!(status.current_bytes, 600);
 }

 #[test]
 fn test_set_limit_runtime() {
 let limiter = HostAllocatorLimiter::new(1000);
 assert_eq!(limiter.limit(), 1000);

 limiter.set_limit(2000);
 assert_eq!(limiter.limit(), 2000);

 // Desativar limite
 limiter.set_limit(0);
 assert!(limiter.check_allocation(u64::MAX / 2).is_ok());
 }

 #[test]
 fn test_status_reporting() {
 let limiter = HostAllocatorLimiter::new(1000);

 limiter.check_allocation(300).unwrap();
 limiter.check_allocation(200).unwrap();
 // Tentar alocação que será rejeitada
 let _ = limiter.check_allocation(600);

 let status = limiter.status();
 assert!(status.is_active);
 assert_eq!(status.limit_bytes, 1000);
 assert_eq!(status.current_bytes, 500);
 assert_eq!(status.allocations_allowed, 2);
 assert_eq!(status.allocations_rejected, 1);
 assert!((status.usage_ratio - 0.5).abs() < f64::EPSILON);
 }

 #[test]
 fn test_reset() {
 let limiter = HostAllocatorLimiter::new(1000);
 limiter.check_allocation(500).unwrap();
 let _ = limiter.check_allocation(600); // rejected

 limiter.reset();

 let status = limiter.status();
 assert_eq!(status.current_bytes, 0);
 assert_eq!(status.peak_bytes, 0);
 assert_eq!(status.allocations_allowed, 0);
 assert_eq!(status.allocations_rejected, 0);
 }

 #[test]
 fn test_memory_status_serialization() {
 let status = MemoryStatus {
 limit_bytes: 1024,
 current_bytes: 512,
 peak_bytes: 768,
 allocations_allowed: 10,
 allocations_rejected: 2,
 is_active: true,
 usage_ratio: 0.5,
 };
 let json = serde_json::to_string(&status).unwrap();
 let restored: MemoryStatus = serde_json::from_str(&json).unwrap();
 assert_eq!(status, restored);
 }

 #[test]
 fn test_memory_limit_exceeded_display() {
 let err = MemoryLimitExceeded {
 limit_bytes: 1000,
 current_bytes: 800,
 requested_bytes: 300,
 };
 let msg = format!("{}", err);
 assert!(msg.contains("1000"));
 assert!(msg.contains("800"));
 assert!(msg.contains("300"));
 }

 #[test]
 fn test_clone_shares_state() {
 let limiter = HostAllocatorLimiter::new(1000);
 let cloned = limiter.clone();

 limiter.check_allocation(400).unwrap();
 assert_eq!(cloned.current_usage(), 400);

 cloned.check_allocation(300).unwrap();
 assert_eq!(limiter.current_usage(), 700);

 limiter.release(200);
 assert_eq!(cloned.current_usage(), 500);
 }

 #[tokio::test]
 async fn test_concurrent_allocations() {
 let limiter = Arc::new(HostAllocatorLimiter::new(10_000));
 let mut handles = Vec::new();

 // Spawnar 20 tasks que cada uma aloca 100 bytes
 for _ in 0..20 {
 let l = limiter.clone();
 handles.push(tokio::spawn(async move {
 // Cada task tenta alocar 100 bytes
 let _ = l.check_allocation(100);
 tokio::time::sleep(std::time::Duration::from_millis(1)).await;
 l.release(100);
 }));
 }

 for h in handles {
 h.await.unwrap();
 }

 // Após todas as tasks completarem, uso deve estar em 0
 assert_eq!(limiter.current_usage(), 0);

 let status = limiter.status();
 // Todas as 20 alocações devem ter sido permitidas (100 * 20 = 2000 < 10000)
 assert_eq!(status.allocations_allowed, 20);
 assert_eq!(status.allocations_rejected, 0);
 }

 #[test]
 fn test_from_env_missing() {
 // Sem variável de ambiente → ilimitado
 std::env::remove_var("ZYLOS_HOST_MEMORY_LIMIT_MB");
 let limiter = HostAllocatorLimiter::from_env();
 assert_eq!(limiter.limit(), 0);
 }

 #[test]
 fn test_exact_limit_boundary() {
 let limiter = HostAllocatorLimiter::new(1000);

 // Exatamente no limite deve ser permitido
 assert!(limiter.check_allocation(1000).is_ok());
 assert_eq!(limiter.current_usage(), 1000);

 // Mais 1 byte deve ser rejeitado
 assert!(limiter.check_allocation(1).is_err());
 }

 #[test]
 fn test_zero_allocation_always_allowed() {
 let limiter = HostAllocatorLimiter::new(100);
 assert!(limiter.check_allocation(0).is_ok());

 // Mesmo com limite cheio, alocação de 0 bytes deve passar
 limiter.check_allocation(100).unwrap();
 assert!(limiter.check_allocation(0).is_ok());
 }
}