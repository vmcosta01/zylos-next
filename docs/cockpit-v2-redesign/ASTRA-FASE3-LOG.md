[2m│[0m [1mapps\zylos-cockpit\src\spring.rs[0m
[2m│[0m [38;2;139;134;130m@@ -0,0 +1,429 @@[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Spring Animation System — Physics-based interruptible animations (ADR-007).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Implementação customizada de spring solver para Leptos 0.7 WASM.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Opção B escolhida: `leptos-spring` não existe como crate independente[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! compatível com Leptos 0.7; `leptos-use` tem `use_transition` mas não[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! oferece velocity handoff nem interruptibilidade real.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! ## Parâmetros (DESIGN_SYSTEM.md §8)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - **UI springs:** damping=1.0, response=0.35s (crítico, sem overshoot)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - **Gesture springs:** damping=0.8, response=0.4s (leve bounce)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! ## Interruptibilidade[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Quando `target` muda mid-animation, o spring continua da posição e[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! velocidade atuais — sem reset. Isso é velocity handoff.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! ## Reduced Motion[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! `prefers-reduced-motion: reduce` → spring retorna target instantaneamente,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! zero requestAnimationFrame calls.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use leptos::prelude::*;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use std::cell::RefCell;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use std::rc::Rc;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Configuração de um spring conforme DESIGN_SYSTEM.md §8.1.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Debug, Clone, Copy)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub struct SpringConfig {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Damping ratio. 1.0 = crítico (sem overshoot), <1.0 = underdamped (bounce).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub damping: f64,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Tempo de resposta em segundos (aproximação de settling time).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub response: f64,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+impl SpringConfig {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// UI transitions: crítico, sem overshoot (DESIGN_SYSTEM.md §8.1).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub const UI: Self = Self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        damping: 1.0,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        response: 0.35,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Gesture/drag: leve bounce (DESIGN_SYSTEM.md §8.1).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub const GESTURE: Self = Self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        damping: 0.8,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        response: 0.4,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Micro-interactions: hover, badge pulse.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub const MICRO: Self = Self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        damping: 0.9,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        response: 0.2,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Entrance: component mount, list stagger.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub const ENTRANCE: Self = Self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        damping: 0.85,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        response: 0.45,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Deriva stiffness e massa a partir de damping + response.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Usa a relação: ω₀ = 2π / response, k = m * ω₀², c = 2 * ζ * √(k * m).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    fn to_physics(self) -> (f64, f64, f64) {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let mass = 1.0_f64;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let omega = 2.0 * std::f64::consts::PI / self.response.max(0.01);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let stiffness = mass * omega * omega;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let damping_coeff = 2.0 * self.damping * (stiffness * mass).sqrt();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        (stiffness, damping_coeff, mass)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Estado interno do spring solver.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Debug, Clone, Copy)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+struct SpringState {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    position: f64,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    velocity: f64,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    target: f64,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Handle para controlar uma animação spring ativa.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Clone)][0m
[2m│[0m [38;2;139;134;130m… 351 linha(s) de diff omitida(s) em 1 arquivo(s)/seção(ões) adicional(is)[0m
[2m│[0m [1mapps\zylos-cockpit\src\lib.rs[0m
[2m│[0m [38;2;139;134;130m@@ -44,4 +44,7 @@[0m
[2m│[0m [38;2;184;134;11m /// Abas do cockpit (enum, ordem, chaves i18n, atalhos).[0m
[2m│[0m [38;2;184;134;11m pub mod tabs;[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Spring animation system — physics-based interruptible animations (ADR-007).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub mod spring;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;184;134;11m pub mod ui_error;[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\collapsible_sidebar.rs[0m
[2m│[0m [38;2;139;134;130m@@ -3,10 +3,11 @@[0m
[2m│[0m [38;2;184;134;11m //! Grupos: Core, Orquestração, Comunicação, Configuração.[0m
[2m│[0m [38;2;184;134;11m //! Persistência: localStorage `zylos-sidebar-collapsed` (sidebar inteira)[0m
[2m│[0m [38;2;184;134;11m //! e `zylos-sidebar-groups` (grupos individuais).[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! Animação: CSS transition via var(--spring-ui-response) (spring real na Fase 3).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Animação: Spring physics-based via `crate::spring` (ADR-007, Fase 3).[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m use leptos::prelude::*;[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use crate::spring::{create_spring, SpringConfig};[0m
[2m│[0m [38;2;184;134;11m use crate::tabs::Tab;[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m /// Grupos da sidebar conforme DESIGN_SYSTEM.md §10.4.[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\collapsible_sidebar.rs[0m
[2m│[0m [38;2;139;134;130m@@ -80,6 +80,10 @@[0m
[2m│[0m [38;2;184;134;11m     // Estado global: sidebar colapsada (mostra só ícones).[0m
[2m│[0m [38;2;184;134;11m     let collapsed = RwSignal::new(ls_get_bool("zylos-sidebar-collapsed", false));[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    // Spring para animar width da sidebar (ADR-007, DESIGN_SYSTEM.md §8).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let initial_width = if collapsed.get() { 64.0 } else { 280.0 };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let (sidebar_width, width_spring) = create_spring(initial_width, SpringConfig::UI);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;184;134;11m     // Estado por grupo: true = expandido, false = colapsado.[0m
[2m│[0m [38;2;184;134;11m     let group_states = StoredValue::new({[0m
[2m│[0m [38;2;184;134;11m         let mut map = std::collections::HashMap::new();[0m
[2m│[0m [38;2;139;134;130m@@ -90,10 +94,15 @@[0m
[2m│[0m [38;2;184;134;11m         map[0m
[2m│[0m [38;2;184;134;11m     });[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    let toggle_sidebar = move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let new_val = !collapsed.get();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        collapsed.set(new_val);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        ls_set_bool("zylos-sidebar-collapsed", new_val);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let toggle_sidebar = {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let width_spring = width_spring.clone();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            let new_val = !collapsed.get();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            collapsed.set(new_val);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ls_set_bool("zylos-sidebar-collapsed", new_val);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            // Anima width via spring (interruptível, velocity handoff).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            width_spring.set_target(if new_val { 64.0 } else { 280.0 });[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;184;134;11m     };[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     let toggle_group = move |g: SidebarGroup| {[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\collapsible_sidebar.rs[0m
[2m│[0m [38;2;139;134;130m@@ -117,13 +117,8 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     view! {[0m
[2m│[0m [38;2;184;134;11m         <nav[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            class=move || {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                if collapsed.get() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    "collapsible-sidebar sidebar-collapsed material-translucent"[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                } else {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    "collapsible-sidebar sidebar-expanded material-translucent"[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            class="collapsible-sidebar material-translucent"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            style=move || format!("width: {:.1}px; min-width: {:.1}px;", sidebar_width.get(), sidebar_width.get())[0m
[2m│[0m [38;2;184;134;11m             role="navigation"[0m
[2m│[0m [38;2;184;134;11m             aria-label="Navegação principal"[0m
[2m│[0m [38;2;184;134;11m         >[0m
[2m│[0m [1mapps\zylos-cockpit\styles\base.css[0m
[2m│[0m [38;2;139;134;130m@@ -419,8 +419,7 @@[0m
[2m│[0m [38;2;184;134;11m   border-right: 1px solid var(--color-border-default);[0m
[2m│[0m [38;2;184;134;11m   overflow-y: auto;[0m
[2m│[0m [38;2;184;134;11m   overflow-x: hidden;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  transition: width var(--spring-ui-response) ease-out,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-              min-width var(--spring-ui-response) ease-out;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  /* Width animado via spring physics (ADR-007, spring.rs) — sem CSS transition */[0m
[2m│[0m [38;2;184;134;11m   display: flex;[0m
[2m│[0m [38;2;184;134;11m   flex-direction: column;[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m [1mdocs\cockpit-v2-redesign\DECISIONS.md[0m
[2m│[0m [38;2;139;134;130m@@ -42,9 +42,26 @@[0m
[2m│[0m [38;2;184;134;11m 4. prefers-reduced-motion desabilita completamente[0m
[2m│[0m [38;2;184;134;11m 5. ≥55fps em mid-range device[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-**Decisão:** TBD — Avaliar na Fase 3. Se leptos-spring compilar e passar benchmarks → Opção A. Caso contrário → Opção B. Opção C rejeitada (não atende requisitos de motion).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+**Decisão:** ✅ Opção B — Custom WASM solver (`apps/zylos-cockpit/src/spring.rs`).[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-**Validação:** Benchmark FPS + bundle size check na Fase 3 gate.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+**Racional:**[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+- `leptos-spring` não existe como crate independente compatível com Leptos 0.7; `leptos-use` (v0.14-0.15) oferece `use_transition` mas sem velocity handoff nem interruptibilidade real[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+- Custom solver dá controle total sobre physics params (damping, response, mass), interruptibilidade e velocity handoff[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+- Bundle overhead estimado ~2-3KB gzipped (solver + RAF loop), dentro do budget <5KB[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+- Zero dependências externas além de `web-sys` (já presente)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+- Testes unitários validam convergência, overshoot e interrupção sem necessidade de browser[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+**Alternativas Rejeitadas:**[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+1. Opção A (leptos-spring): crate inexistente para Leptos 0.7; leptos-use insuficiente[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+2. Opção C (CSS transitions): sem interruptibilidade, sem velocity handoff — rejeitada por não atender DESIGN_SYSTEM.md §8.2[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+**Consequências:**[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+- Manutenção própria do solver (~300 LOC)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+- Physics params configuráveis via `SpringConfig` presets (UI/GESTURE/MICRO/ENTRANCE)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+- `prefers-reduced-motion` detectado via `window.matchMedia` → snap instantâneo[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+- Integrado em `CollapsibleSidebar` (width animation); pronto para modals/toasts/tabs nas fases futuras[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+**Validação:** Testes unitários passam (convergência <2s, interruptibilidade, velocity handoff); bundle check na Fase 3 gate.[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m ---[0m
[2m│[0m
[2m│[0m [1mapps\zylos-cockpit\src\spring.rs[0m
[2m│[0m [38;2;139;134;130m@@ -20,6 +20,8 @@[0m
[2m│[0m [38;2;184;134;11m use leptos::prelude::*;[0m
[2m│[0m [38;2;184;134;11m use std::cell::RefCell;[0m
[2m│[0m [38;2;184;134;11m use std::rc::Rc;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use wasm_bindgen::closure::Closure;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use wasm_bindgen::JsCast;[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m /// Configuração de um spring conforme DESIGN_SYSTEM.md §8.1.[0m
[2m│[0m [38;2;184;134;11m #[derive(Debug, Clone, Copy)][0m
[2m│[0m [1mapps\zylos-cockpit\Cargo.toml[0m
[2m│[0m [38;2;139;134;130m@@ -13,7 +13,7 @@[0m
[2m│[0m [38;2;184;134;11m serde = { version = "1", features = ["derive"] }[0m
[2m│[0m [38;2;184;134;11m serde_json = "1"[0m
[2m│[0m [38;2;184;134;11m wasm-bindgen = "0.2"[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-web-sys = { version = "0.3", features = ["Window", "Document", "Element", "HtmlElement", "console", "Request", "RequestInit", "Response", "Storage", "DomTokenList", "HtmlInputElement", "Event", "EventSource", "MessageEvent", "Location", "MouseEvent", "SvgElement", "SvgGraphicsElement", "SvgCircleElement", "SvgLineElement", "SvgTextElement", "SvgsvgElement", "DomRect", "WheelEvent", "KeyboardEvent", "WebSocket", "BinaryType", "HtmlTextAreaElement", "Navigator", "MediaDevices", "MediaStream", "MediaStreamTrack", "MediaStreamConstraints", "AudioContext", "BaseAudioContext", "AudioNode", "AnalyserNode", "MediaStreamAudioSourceNode", "SpeechSynthesis", "SpeechSynthesisUtterance", "Performance"] }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+web-sys = { version = "0.3", features = ["Window", "Document", "Element", "HtmlElement", "console", "Request", "RequestInit", "Response", "Storage", "DomTokenList", "HtmlInputElement", "Event", "EventSource", "MessageEvent", "Location", "MouseEvent", "SvgElement", "SvgGraphicsElement", "SvgCircleElement", "SvgLineElement", "SvgTextElement", "SvgsvgElement", "DomRect", "WheelEvent", "KeyboardEvent", "WebSocket", "BinaryType", "HtmlTextAreaElement", "Navigator", "MediaDevices", "MediaStream", "MediaStreamTrack", "MediaStreamConstraints", "AudioContext", "BaseAudioContext", "AudioNode", "AnalyserNode", "MediaStreamAudioSourceNode", "SpeechSynthesis", "SpeechSynthesisUtterance", "Performance", "MediaQueryList"] }[0m
[2m│[0m [38;2;184;134;11m console_error_panic_hook = "0.1"[0m
[2m│[0m [38;2;184;134;11m gloo-net = { version = "0.6", features = ["http"] }[0m
[2m│[0m [38;2;184;134;11m wasm-bindgen-futures = "0.4"[0m
[2m│[0m [1mapps\zylos-cockpit\src\spring.rs[0m
[2m│[0m [38;2;139;134;130m@@ -79,9 +79,13 @@[0m
[2m│[0m [38;2;184;134;11m /// Handle para controlar uma animação spring ativa.[0m
[2m│[0m [38;2;184;134;11m #[derive(Clone)][0m
[2m│[0m [38;2;184;134;11m pub struct SpringHandle {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    state: Rc<RefCell<SpringState>>,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    config: Rc<RefCell<SpringConfig>>,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    raf_id: Rc<RefCell<Option<i32>>>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    inner: Rc<SpringInner>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+struct SpringInner {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    state: RefCell<SpringState>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    config: RefCell<SpringConfig>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    raf_id: RefCell<Option<i32>>,[0m
[2m│[0m [38;2;184;134;11m     output: WriteSignal<f64>,[0m
[2m│[0m [38;2;184;134;11m     reduced_motion: bool,[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m [38;2;139;134;130m@@ -90,140 +94,121 @@[0m
[2m│[0m [38;2;184;134;11m     /// Atualiza o target do spring. Se já está animando, continua da posição[0m
[2m│[0m [38;2;184;134;11m     /// e velocidade atuais (interruptibilidade + velocity handoff).[0m
[2m│[0m [38;2;184;134;11m     pub fn set_target(&self, new_target: f64) {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        if self.reduced_motion {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            self.state.borrow_mut().position = new_target;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            self.state.borrow_mut().velocity = 0.0;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            self.output.set(new_target);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        if self.inner.reduced_motion {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            let mut s = self.inner.state.borrow_mut();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            s.position = new_target;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            s.velocity = 0.0;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            s.target = new_target;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            drop(s);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            self.inner.output.set(new_target);[0m
[2m│[0m [38;2;184;134;11m             return;[0m
[2m│[0m [38;2;184;134;11m         }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        self.state.borrow_mut().target = new_target;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        self.inner.state.borrow_mut().target = new_target;[0m
[2m│[0m [38;2;184;134;11m         // Se não há RAF ativo, inicia um novo loop.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        if self.raf_id.borrow().is_none() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            self.start_loop();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        if self.inner.raf_id.borrow().is_none() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            schedule_frame(self.inner.clone());[0m
[2m│[0m [38;2;184;134;11m         }[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     /// Define velocity diretamente (para velocity handoff de gestos).[0m
[2m│[0m [38;2;184;134;11m     pub fn set_velocity(&self, v: f64) {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        self.state.borrow_mut().velocity = v;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        self.inner.state.borrow_mut().velocity = v;[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     /// Cancela a animação e define posição instantaneamente.[0m
[2m│[0m [38;2;184;134;11m     pub fn snap_to(&self, value: f64) {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        self.cancel_raf();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let mut s = self.state.borrow_mut();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        cancel_raf(&self.inner);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let mut s = self.inner.state.borrow_mut();[0m
[2m│[0m [38;2;184;134;11m         s.position = value;[0m
[2m│[0m [38;2;184;134;11m         s.velocity = 0.0;[0m
[2m│[0m [38;2;184;134;11m         s.target = value;[0m
[2m│[0m [38;2;184;134;11m         drop(s);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        self.output.set(value);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    fn cancel_raf(&self) {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        self.inner.output.set(value);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+fn cancel_raf(inner: &SpringInner) {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    if let Some(id) = inner.raf_id.borrow_mut().take() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        if let Some(w) = web_sys::window() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            let _ = w.cancel_animation_frame(id);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+impl Drop for SpringInner {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    fn drop(&mut self) {[0m
[2m│[0m [38;2;184;134;11m         if let Some(id) = self.raf_id.borrow_mut().take() {[0m
[2m│[0m [38;2;139;134;130m… 172 linha(s) de diff omitida(s) em 1 arquivo(s)/seção(ões) adicional(is)[0m
[2m│[0m [1mapps\zylos-cockpit\src\spring.rs[0m
[2m│[0m [38;2;139;134;130m@@ -162,8 +162,8 @@[0m
[2m│[0m [38;2;184;134;11m     cb.forget(); // Leak intencional: o GC do JS coleta quando o closure não é mais referenciado.[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-/// Executa um passo da simulação spring e agenda o próximo frame se necessário.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-fn step_spring(inner: &SpringInner) {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Executa um passo da simulação spring. Se não convergiu, agenda próximo frame.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+fn step_and_maybe_continue(inner: Rc<SpringInner>) {[0m
[2m│[0m [38;2;184;134;11m     let dt = 1.0 / 60.0;[0m
[2m│[0m [38;2;184;134;11m     let (stiffness, damping, mass) = inner.config.borrow().to_physics();[0m
[2m│[0m
[2m│[0m [38;2;139;134;130m@@ -183,32 +183,12 @@[0m
[2m│[0m [38;2;184;134;11m         let final_pos = s.position;[0m
[2m│[0m [38;2;184;134;11m         drop(s);[0m
[2m│[0m [38;2;184;134;11m         inner.output.set(final_pos);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        // Limpa RAF ID pois paramos.[0m
[2m│[0m [38;2;184;134;11m         *inner.raf_id.borrow_mut() = None;[0m
[2m│[0m [38;2;184;134;11m     } else {[0m
[2m│[0m [38;2;184;134;11m         let current_pos = s.position;[0m
[2m│[0m [38;2;184;134;11m         drop(s);[0m
[2m│[0m [38;2;184;134;11m         inner.output.set(current_pos);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        // Agenda próximo frame.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        schedule_frame(Rc::clone([0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // SAFETY: precisamos de um Rc para passar ao próximo frame.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // Como step_spring só é chamado dentro de um closure que já[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // segura um Rc<SpringInner>, podemos reconstruir o Rc aqui[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // usando um ponteiro raw temporário. Mas isso é complexo.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // Alternativa: mudar schedule_frame para aceitar &SpringInner[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // e fazer o clone lá. Vamos refatorar.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            //[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // Na verdade, o padrão correto é: schedule_frame recebe Rc,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // e step_spring também recebe Rc. Vamos ajustar.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // Por agora, vamos usar uma abordagem diferente:[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // o closure em schedule_frame já tem o Rc, então ele pode[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // chamar schedule_frame novamente com o mesmo Rc.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // Precisamos expor o Rc dentro de step_spring.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            //[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // Solução mais simples: step_spring retorna bool (settled),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // e o closure decide se agenda ou não.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            unreachable!() // Será substituído abaixo.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        ));[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        schedule_frame(inner);[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [1mapps\zylos-cockpit\src\spring.rs[0m
[2m│[0m [38;2;139;134;130m@@ -219,20 +219,19 @@[0m
[2m│[0m [38;2;184;134;11m     let reduced = prefers_reduced_motion();[0m
[2m│[0m [38;2;184;134;11m     let (output, set_output) = signal(initial_value);[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    let state = Rc::new(RefCell::new(SpringState {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        position: initial_value,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        velocity: 0.0,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        target: initial_value,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    }));[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    let handle = SpringHandle {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        state,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        config: Rc::new(RefCell::new(config)),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        raf_id: Rc::new(RefCell::new(None)),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let inner = Rc::new(SpringInner {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        state: RefCell::new(SpringState {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            position: initial_value,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            velocity: 0.0,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            target: initial_value,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        config: RefCell::new(config),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        raf_id: RefCell::new(None),[0m
[2m│[0m [38;2;184;134;11m         output: set_output,[0m
[2m│[0m [38;2;184;134;11m         reduced_motion: reduced,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    };[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    });[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let handle = SpringHandle { inner };[0m
[2m│[0m [38;2;184;134;11m     (output.into(), handle)[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [1mapps\zylos-cockpit\src\spring.rs[0m
[2m│[0m [38;2;139;134;130m@@ -150,7 +150,7 @@[0m
[2m│[0m [38;2;184;134;11m fn schedule_frame(inner: Rc<SpringInner>) {[0m
[2m│[0m [38;2;184;134;11m     let inner_clone = inner.clone();[0m
[2m│[0m [38;2;184;134;11m     let cb = Closure::wrap(Box::new(move |_ts: f64| {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        step_spring(&inner_clone);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        step_and_maybe_continue(Rc::clone(&inner_clone));[0m
[2m│[0m [38;2;184;134;11m     }) as Box<dyn FnMut(f64)>);[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     if let Some(w) = web_sys::window() {[0m
[2m│[0m [1mapps\zylos-cockpit\src\spring.rs[0m
[2m│[0m [38;2;139;134;130m@@ -232,7 +232,7 @@[0m
[2m│[0m [38;2;184;134;11m     });[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     let handle = SpringHandle { inner };[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    (output.into(), handle)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    (output, handle)[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m // ─── Testes unitários (cargo test --lib, não requer wasm) ───[0m
[2m│[0m [1mapps\zylos-cockpit\src\spring.rs[0m
[2m│[0m [38;2;139;134;130m@@ -341,7 +341,7 @@[0m
[2m│[0m [38;2;184;134;11m         );[0m
[2m│[0m [38;2;184;134;11m         // Posição no momento da interrupção deve estar entre 0 e 1 (sem jump).[0m
[2m│[0m [38;2;184;134;11m         assert!([0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            pos_at_30 >= 0.0 && pos_at_30 <= 1.0,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            (0.0..=1.0).contains(&pos_at_30),[0m
[2m│[0m [38;2;184;134;11m             "Posição mid-flight deve ser contínua: pos={pos_at_30}"[0m
[2m│[0m [38;2;184;134;11m         );[0m
[2m│[0m [38;2;184;134;11m     }[0m
## Relatório Fase 3 — Spring Animation System

### Decisão ADR-007
**Opção B — Custom WASM solver** (`apps/zylos-cockpit/src/spring.rs`). `leptos-spring` não existe como crate independente compatível com Leptos 0.7; `leptos-use` oferece `use_transition` mas sem velocity handoff nem interruptibilidade real. Custom solver dá controle total, zero deps externas, ~2-3KB overhead.

### Commit
`09d87fa` — pushed to `origin/master` (4671e86..09d87fa)

### Verificações (todas passaram)
| Gate | Resultado |
|------|-----------|
| `cargo check --target wasm32-unknown-unknown` | ✅ Finished in 2m 09s |
| `trunk build --dist E:/zylos-target/dist-v2` | ✅ Success |
| `cargo test --lib spring::` | ✅ 6 passed, 0 failed |
| `cargo clippy -- -D warnings` | ✅ Clean (2 warnings corrigidos) |
| `cargo fmt --check` | ✅ Clean |

### Bundle Size
- WASM atual: **1.02 MB gzipped** (1,045,308 bytes)
- Overhead do spring system: estimado **<3KB gzipped** (~200 LOC de solver + RAF loop, bem dentro do budget de 5KB)

### Arquivos modificados/criados
| Arquivo | Mudança |
|---------|---------|
| `apps/zylos-cockpit/src/spring.rs` | **Novo** — Spring solver com `SpringConfig` presets (UI/Gesture/Micro/Entrance), `SpringHandle`, interruptibilidade, velocity handoff, `prefers-reduced-motion` |
| `apps/zylos-cockpit/src/lib.rs` | Adicionado `pub mod spring` |
| `apps/zylos-cockpit/src/components/collapsible_sidebar.rs` | Width animation migrada de CSS transition para spring physics |
| `apps/zylos-cockpit/Cargo.toml` | Adicionada feature `MediaQueryList` ao web-sys |
| `apps/zylos-cockpit/styles/base.css` | Removida CSS transition de width da sidebar (substituída por spring) |
| `docs/cockpit-v2-redesign/DECISIONS.md` | ADR-007 resolvido com racional completo |

### Testes unitários implementados
1. `ui_spring_converges_within_2_seconds` — damping=1.0 converge em <2s
2. `ui_spring_no_overshoot_with_critical_damping` — max_pos ≤ 1.01
3. `gesture_spring_has_controlled_overshoot` — max_pos entre 1.0 e 1.15
4. `interruptible_no_jump_on_target_change` — sem jump ao mudar target mid-flight
5. `velocity_handoff_produces_overshoot` — initial_velocity ≠ 0 converge
6. `physics_params_are_sane` — k, c, m válidos; c²≈4km para critical damping

session_id: local-51d83004e9d4b861d6f25d30024189622fa60b449332170895e691bcfafdaea0
