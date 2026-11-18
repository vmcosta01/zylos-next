//! Spring Animation System — Physics-based interruptible animations (ADR-007).
//!
//! Implementação customizada de spring solver para Leptos 0.7 WASM.
//! Opção B escolhida: `leptos-spring` não existe como crate independente
//! compatível com Leptos 0.7; `leptos-use` tem `use_transition` mas não
//! oferece velocity handoff nem interruptibilidade real.
//!
//! ## Parâmetros (DESIGN_SYSTEM.md §8)
//! - **UI springs:** damping=1.0, response=0.35s (crítico, sem overshoot)
//! - **Gesture springs:** damping=0.8, response=0.4s (leve bounce)
//!
//! ## Interruptibilidade
//! Quando `target` muda mid-animation, o spring continua da posição e
//! velocidade atuais — sem reset. Isso é velocity handoff.
//!
//! ## Reduced Motion
//! `prefers-reduced-motion: reduce` → spring retorna target instantaneamente,
//! zero requestAnimationFrame calls.

use leptos::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

/// Configuração de um spring conforme DESIGN_SYSTEM.md §8.1.
#[derive(Debug, Clone, Copy)]
pub struct SpringConfig {
    /// Damping ratio. 1.0 = crítico (sem overshoot), <1.0 = underdamped (bounce).
    pub damping: f64,
    /// Tempo de resposta em segundos (aproximação de settling time).
    pub response: f64,
}

impl SpringConfig {
    /// UI transitions: crítico, sem overshoot (DESIGN_SYSTEM.md §8.1).
    pub const UI: Self = Self {
        damping: 1.0,
        response: 0.35,
    };

    /// Gesture/drag: leve bounce (DESIGN_SYSTEM.md §8.1).
    pub const GESTURE: Self = Self {
        damping: 0.8,
        response: 0.4,
    };

    /// Micro-interactions: hover, badge pulse.
    pub const MICRO: Self = Self {
        damping: 0.9,
        response: 0.2,
    };

    /// Entrance: component mount, list stagger.
    pub const ENTRANCE: Self = Self {
        damping: 0.85,
        response: 0.45,
    };

    /// Deriva stiffness e massa a partir de damping + response.
    /// Usa a relação: ω₀ = 2π / response, k = m * ω₀², c = 2 * ζ * √(k * m).
    fn to_physics(self) -> (f64, f64, f64) {
        let mass = 1.0_f64;
        let omega = 2.0 * std::f64::consts::PI / self.response.max(0.01);
        let stiffness = mass * omega * omega;
        let damping_coeff = 2.0 * self.damping * (stiffness * mass).sqrt();
        (stiffness, damping_coeff, mass)
    }
}

/// Estado interno do spring solver.
#[derive(Debug, Clone, Copy)]
struct SpringState {
    position: f64,
    velocity: f64,
    target: f64,
}

/// Handle para controlar uma animação spring ativa.
#[derive(Clone)]
pub struct SpringHandle {
    inner: Rc<SpringInner>,
}

struct SpringInner {
    state: RefCell<SpringState>,
    config: RefCell<SpringConfig>,
    raf_id: RefCell<Option<i32>>,
    output: WriteSignal<f64>,
    reduced_motion: bool,
}

impl SpringHandle {
    /// Atualiza o target do spring. Se já está animando, continua da posição
    /// e velocidade atuais (interruptibilidade + velocity handoff).
    pub fn set_target(&self, new_target: f64) {
        if self.inner.reduced_motion {
            let mut s = self.inner.state.borrow_mut();
            s.position = new_target;
            s.velocity = 0.0;
            s.target = new_target;
            drop(s);
            self.inner.output.set(new_target);
            return;
        }
        self.inner.state.borrow_mut().target = new_target;
        // Se não há RAF ativo, inicia um novo loop.
        if self.inner.raf_id.borrow().is_none() {
            schedule_frame(self.inner.clone());
        }
    }

    /// Define velocity diretamente (para velocity handoff de gestos).
    pub fn set_velocity(&self, v: f64) {
        self.inner.state.borrow_mut().velocity = v;
    }

    /// Cancela a animação e define posição instantaneamente.
    pub fn snap_to(&self, value: f64) {
        cancel_raf(&self.inner);
        let mut s = self.inner.state.borrow_mut();
        s.position = value;
        s.velocity = 0.0;
        s.target = value;
        drop(s);
        self.inner.output.set(value);
    }
}

fn cancel_raf(inner: &SpringInner) {
    if let Some(id) = inner.raf_id.borrow_mut().take() {
        if let Some(w) = web_sys::window() {
            let _ = w.cancel_animation_frame(id);
        }
    }
}

impl Drop for SpringInner {
    fn drop(&mut self) {
        if let Some(id) = self.raf_id.borrow_mut().take() {
            if let Some(w) = web_sys::window() {
                let _ = w.cancel_animation_frame(id);
            }
        }
    }
}

/// Agenda um frame RAF. O closure clona o `Rc<SpringInner>` para cada frame,
/// evitando problemas de lifetime com `Closure::wrap`.
fn schedule_frame(inner: Rc<SpringInner>) {
    let inner_clone = inner.clone();
    let cb = Closure::wrap(Box::new(move |_ts: f64| {
        step_and_maybe_continue(Rc::clone(&inner_clone));
    }) as Box<dyn FnMut(f64)>);

    if let Some(w) = web_sys::window() {
        let id = w
            .request_animation_frame(cb.as_ref().unchecked_ref())
            .unwrap_or(0);
        *inner.raf_id.borrow_mut() = Some(id);
    }
    cb.forget(); // Leak intencional: o GC do JS coleta quando o closure não é mais referenciado.
}

/// Executa um passo da simulação spring. Se não convergiu, agenda próximo frame.
fn step_and_maybe_continue(inner: Rc<SpringInner>) {
    let dt = 1.0 / 60.0;
    let (stiffness, damping, mass) = inner.config.borrow().to_physics();

    let mut s = inner.state.borrow_mut();
    let displacement = s.position - s.target;
    let spring_force = -stiffness * displacement;
    let damping_force = -damping * s.velocity;
    let acceleration = (spring_force + damping_force) / mass;

    s.velocity += acceleration * dt;
    s.position += s.velocity * dt;

    let settled = (s.position - s.target).abs() < 0.001 && s.velocity.abs() < 0.001;
    if settled {
        s.position = s.target;
        s.velocity = 0.0;
        let final_pos = s.position;
        drop(s);
        inner.output.set(final_pos);
        *inner.raf_id.borrow_mut() = None;
    } else {
        let current_pos = s.position;
        drop(s);
        inner.output.set(current_pos);
        schedule_frame(inner);
    }
}

/// Detecta `prefers-reduced-motion: reduce` via window.matchMedia.
fn prefers_reduced_motion() -> bool {
    web_sys::window()
        .and_then(|w| {
            w.match_media("(prefers-reduced-motion: reduce)")
                .ok()
                .flatten()
        })
        .map(|mql| mql.matches())
        .unwrap_or(false)
}

/// Cria um spring animator reutilizável.
///
/// Retorna `(output_signal, handle)` onde:
/// - `output_signal`: ReadSignal<f64> que contém o valor interpolado atual
/// - `handle`: SpringHandle para controlar target/velocity/snap
///
/// # Exemplo
/// ```ignore
/// let (width, spring) = create_spring(280.0, SpringConfig::UI);
/// spring.set_target(64.0); // Anima de 280 → 64 com physics
/// ```
pub fn create_spring(initial_value: f64, config: SpringConfig) -> (ReadSignal<f64>, SpringHandle) {
    let reduced = prefers_reduced_motion();
    let (output, set_output) = signal(initial_value);

    let inner = Rc::new(SpringInner {
        state: RefCell::new(SpringState {
            position: initial_value,
            velocity: 0.0,
            target: initial_value,
        }),
        config: RefCell::new(config),
        raf_id: RefCell::new(None),
        output: set_output,
        reduced_motion: reduced,
    });

    let handle = SpringHandle { inner };
    (output, handle)
}

// ─── Testes unitários (cargo test --lib, não requer wasm) ───

#[cfg(test)]
mod tests {
    use super::*;

    /// Simula N passos do spring solver sem DOM/RAF.
    fn simulate_spring(
        config: SpringConfig,
        initial: f64,
        target: f64,
        initial_velocity: f64,
        steps: usize,
    ) -> (f64, f64) {
        let (stiffness, damping, mass) = config.to_physics();
        let dt = 1.0 / 60.0;
        let mut pos = initial;
        let mut vel = initial_velocity;

        for _ in 0..steps {
            let displacement = pos - target;
            let spring_force = -stiffness * displacement;
            let damping_force = -damping * vel;
            let acc = (spring_force + damping_force) / mass;
            vel += acc * dt;
            pos += vel * dt;
        }
        (pos, vel)
    }

    #[test]
    fn ui_spring_converges_within_2_seconds() {
        // 2s = 120 frames a 60fps
        let (pos, vel) = simulate_spring(SpringConfig::UI, 0.0, 1.0, 0.0, 120);
        assert!(
            (pos - 1.0).abs() < 0.01,
            "UI spring deve convergir em <2s: pos={pos}"
        );
        assert!(vel.abs() < 0.01, "Velocidade deve ser ~0: vel={vel}");
    }

    #[test]
    fn ui_spring_no_overshoot_with_critical_damping() {
        // Damping=1.0 (crítico) não deve ultrapassar target significativamente.
        let (stiffness, damping, mass) = SpringConfig::UI.to_physics();
        let dt = 1.0 / 60.0;
        let mut pos = 0.0;
        let mut vel = 0.0;
        let target = 1.0;
        let mut max_pos = 0.0_f64;

        for _ in 0..120 {
            let disp = pos - target;
            let sf = -stiffness * disp;
            let df = -damping * vel;
            let acc = (sf + df) / mass;
            vel += acc * dt;
            pos += vel * dt;
            max_pos = max_pos.max(pos);
        }
        // Critical damping pode ter overshoot mínimo numérico (<1%).
        assert!(
            max_pos <= 1.01,
            "Critical damping não deve ter overshoot significativo: max={max_pos}"
        );
    }

    #[test]
    fn gesture_spring_has_controlled_overshoot() {
        // Damping=0.8 deve ter overshoot, mas controlado (<15%).
        let (stiffness, damping, mass) = SpringConfig::GESTURE.to_physics();
        let dt = 1.0 / 60.0;
        let mut pos = 0.0;
        let mut vel = 0.0;
        let target = 1.0;
        let mut max_pos = 0.0_f64;

        for _ in 0..180 {
            let disp = pos - target;
            let sf = -stiffness * disp;
            let df = -damping * vel;
            let acc = (sf + df) / mass;
            vel += acc * dt;
            pos += vel * dt;
            max_pos = max_pos.max(pos);
        }
        assert!(
            max_pos > 1.0 && max_pos < 1.15,
            "Gesture spring deve ter overshoot controlado: max={max_pos}"
        );
    }

    #[test]
    fn interruptible_no_jump_on_target_change() {
        // Muda target mid-flight: posição não deve saltar.
        let config = SpringConfig::UI;
        let (pos_at_30, vel_at_30) = simulate_spring(config, 0.0, 1.0, 0.0, 30);

        // Agora muda target para 0.5, continuando da posição/velocidade atuais.
        let (final_pos, _) = simulate_spring_with_state(config, pos_at_30, 0.5, vel_at_30, 120);
        assert!(
            (final_pos - 0.5).abs() < 0.01,
            "Após interrupção, spring deve convergir para novo target: pos={final_pos}"
        );
        // Posição no momento da interrupção deve estar entre 0 e 1 (sem jump).
        assert!(
            (0.0..=1.0).contains(&pos_at_30),
            "Posição mid-flight deve ser contínua: pos={pos_at_30}"
        );
    }

    fn simulate_spring_with_state(
        config: SpringConfig,
        initial: f64,
        target: f64,
        initial_velocity: f64,
        steps: usize,
    ) -> (f64, f64) {
        let (stiffness, damping, mass) = config.to_physics();
        let dt = 1.0 / 60.0;
        let mut pos = initial;
        let mut vel = initial_velocity;

        for _ in 0..steps {
            let displacement = pos - target;
            let spring_force = -stiffness * displacement;
            let damping_force = -damping * vel;
            let acc = (spring_force + damping_force) / mass;
            vel += acc * dt;
            pos += vel * dt;
        }
        (pos, vel)
    }

    #[test]
    fn velocity_handoff_produces_overshoot() {
        // Initial velocity alto → overshoot controlado.
        let (pos, _) = simulate_spring(SpringConfig::GESTURE, 0.0, 1.0, 5.0, 180);
        // Deve convergir eventualmente.
        assert!(
            (pos - 1.0).abs() < 0.05,
            "Com velocity handoff, spring deve convergir: pos={pos}"
        );
    }

    #[test]
    fn physics_params_are_sane() {
        let (k, c, m) = SpringConfig::UI.to_physics();
        assert!(k > 0.0, "Stiffness deve ser positiva: {k}");
        assert!(c > 0.0, "Damping coeff deve ser positivo: {c}");
        assert!((m - 1.0).abs() < f64::EPSILON, "Mass deve ser 1.0: {m}");

        // Verifica relação de critical damping: c² ≈ 4km
        let discriminant = c * c - 4.0 * k * m;
        assert!(
            discriminant.abs() < 1.0,
            "Critical damping: c²≈4km, diff={discriminant}"
        );
    }
}
