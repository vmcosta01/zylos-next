[2m│[0m [1mapps\zylos-cockpit\src\app.rs[0m
[2m│[0m [38;2;139;134;130m@@ -99,13 +99,16 @@[0m
[2m│[0m [38;2;184;134;11m     });[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     view! {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        <a href="#main-content" class="skip-link">"Pular para o conteúdo"</a>[0m
[2m│[0m [38;2;184;134;11m         <div class="shell-layout">[0m
[2m│[0m [38;2;184;134;11m             <TranslucentHeader version=version.get() gateway_status=gateway_status.get() />[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m             <div class="shell-body">[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <CollapsibleSidebar active_tab=active_tab set_active_tab=set_active_tab />[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <main class="shell-main" role="main">[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <aside class="shell-sidebar-wrapper">[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    <CollapsibleSidebar active_tab=active_tab set_active_tab=set_active_tab />[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                </aside>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <main id="main-content" class="shell-main" role="main">[0m
[2m│[0m [38;2;184;134;11m                     {move || match &*state.read() {[0m
[2m│[0m [38;2;184;134;11m                         DashboardState::Loading => view! {[0m
[2m│[0m [38;2;184;134;11m                             <div class="skeleton-grid">[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -350,7 +350,7 @@[0m
[2m│[0m [38;2;184;134;11m             <div class="vnc-controls">[0m
[2m│[0m [38;2;184;134;11m                 <Show when=move || controlling.get()[0m
[2m│[0m [38;2;184;134;11m                       fallback=move || view! {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                          <button class="cta-primary" on:click=move |_: web_sys::MouseEvent| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                          <button class="cta-primary" aria-label="Assumir controle do computador remoto" on:click=move |_: web_sys::MouseEvent| {[0m
[2m│[0m [38;2;184;134;11m                               let sid = active_session.get().unwrap_or_default();[0m
[2m│[0m [38;2;184;134;11m                               let ctrl = controlling;[0m
[2m│[0m [38;2;184;134;11m                               let err = error_msg;[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -372,7 +372,7 @@[0m
[2m│[0m [38;2;184;134;11m                               {"Assumir Controle"}[0m
[2m│[0m [38;2;184;134;11m                           </button>[0m
[2m│[0m [38;2;184;134;11m                       }>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    <button class="cta-secondary" on:click=move |_: web_sys::MouseEvent| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    <button class="cta-secondary" aria-label="Liberar controle do computador remoto" on:click=move |_: web_sys::MouseEvent| {[0m
[2m│[0m [38;2;184;134;11m                         let sid = active_session.get().unwrap_or_default();[0m
[2m│[0m [38;2;184;134;11m                         let ctrl = controlling;[0m
[2m│[0m [38;2;184;134;11m                         let err = error_msg;[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -392,7 +392,7 @@[0m
[2m│[0m [38;2;184;134;11m                         {"Liberar Controle"}[0m
[2m│[0m [38;2;184;134;11m                     </button>[0m
[2m│[0m [38;2;184;134;11m                 </Show>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <button class="cta-secondary" on:click=capture_screenshot>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <button class="cta-secondary" aria-label="Capturar screenshot da sessão remota" on:click=capture_screenshot>[0m
[2m│[0m [38;2;184;134;11m                     {"📷 Capturar Tela"}[0m
[2m│[0m [38;2;184;134;11m                 </button>[0m
[2m│[0m [38;2;184;134;11m             </div>[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\hitl_approval_panel.rs[0m
[2m│[0m [38;2;139;134;130m@@ -320,13 +320,13 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m                         view! {[0m
[2m│[0m [38;2;184;134;11m                             <>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                <button class="cta-primary approval-btn-approve" on:click=approve_handler>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                <button class="cta-primary approval-btn-approve" aria-label="Aprovar esta ação" on:click=approve_handler>[0m
[2m│[0m [38;2;184;134;11m                                     "✓ Aprovar"[0m
[2m│[0m [38;2;184;134;11m                                 </button>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                <button class="cta-secondary approval-btn-reject" on:click=reject_handler>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                <button class="cta-secondary approval-btn-reject" aria-label="Recusar esta ação" on:click=reject_handler>[0m
[2m│[0m [38;2;184;134;11m                                     "✕ Recusar"[0m
[2m│[0m [38;2;184;134;11m                                 </button>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                <button class="cta-secondary approval-btn-escalate" on:click=escalate_handler>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                <button class="cta-secondary approval-btn-escalate" aria-label="Escalar para revisão humana" on:click=escalate_handler>[0m
[2m│[0m [38;2;184;134;11m                                     "⚠ Escalar"[0m
[2m│[0m [38;2;184;134;11m                                 </button>[0m
[2m│[0m [38;2;184;134;11m                             </>[0m
[2m│[0m [1mapps\zylos-cockpit\styles\base.css[0m
[2m│[0m [38;2;139;134;130m@@ -27,6 +27,31 @@[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m #app:empty { display: none; }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/* ── Skip-to-Content Link (WCAG AA) ── */[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.skip-link {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  position: absolute;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  top: -40px;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  left: 0;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  z-index: 10000;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  padding: var(--space-2) var(--space-4);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  background: var(--color-accent-primary);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  color: #fff;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  font-weight: 600;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  text-decoration: none;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  border-radius: 0 0 var(--radius-md) 0;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  transition: top var(--transition-fast);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.skip-link:focus {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  top: 0;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  outline: 2px solid #fff;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  outline-offset: 2px;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/* ── Sidebar Wrapper Landmark ── */[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.shell-sidebar-wrapper {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  flex-shrink: 0;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m /* ── Focus States (WCAG AA) ── */[0m
[2m│[0m [38;2;184;134;11m :focus-visible {[0m
[2m│[0m [1mapps\zylos-cockpit\Cargo.toml[0m
[2m│[0m [38;2;139;134;130m@@ -32,3 +32,10 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m [dev-dependencies][0m
[2m│[0m [38;2;184;134;11m wasm-bindgen-test = "0.3.80"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[profile.release][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+opt-level = "z"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+lto = true[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+codegen-units = 1[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+panic = "abort"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+strip = true[0m
[2m│[0m [1mapps\zylos-cockpit\Trunk.toml[0m
[2m│[0m [38;2;139;134;130m@@ -6,6 +6,11 @@[0m
[2m│[0m [38;2;184;134;11m stage = "pre_build"[0m
[2m│[0m [38;2;184;134;11m command = "rustup"[0m
[2m│[0m [38;2;184;134;11m command_arguments = ["target", "add", "wasm32-unknown-unknown"][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[[hooks]][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+stage = "post_build"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+command = "wasm-opt"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+command_arguments = ["-Oz", "--strip-debug", "--strip-producers", "$WASM_FILE", "-o", "$WASM_FILE"][0m
[2m│[0m
[2m│[0m [38;2;184;134;11m [tools][0m
[2m│[0m
[2m│[0m [1mapps\zylos-cockpit\Cargo.toml[0m
[2m│[0m [38;2;139;134;130m@@ -32,10 +32,3 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m [dev-dependencies][0m
[2m│[0m [38;2;184;134;11m wasm-bindgen-test = "0.3.80"[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-[profile.release][0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-opt-level = "z"[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-lto = true[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-codegen-units = 1[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-panic = "abort"[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-strip = true[0m
## Relatório — Fase 8: Acessibilidade + Performance

**Commit:** `3df9fc7` (pushed to `origin/master`)

### Verificações
1. `cargo check -p zylos-cockpit --target wasm32-unknown-unknown` — OK (compila limpo)
2. `cargo test -p zylos-cockpit --lib` — OK (144 testes passaram, 0 falhas)
3. `cargo clippy -p zylos-cockpit --all-targets` — OK (zero warnings próprios; apenas future-incompat de dep externa)
4. Grep ARIA landmarks — todos presentes (`<nav>`, `<header>`, `aria-live`, `role="dialog"`, `skip-link`)
5. Grep `skip-link` CSS — presente em `base.css:32` e `base.css:45`
6. Bundle size — não medido via `trunk build --release` nesta sessão (wasm-opt hook adicionado no Trunk.toml para builds futuros; perfil `wasm-release` com `opt-level="z"` + LTO já existia no workspace root)

### ARIA Landmarks Adicionados/Confirmados
- `<nav aria-label="Navegação principal">` — `collapsible_sidebar.rs:119` (já existia)
- `<header role="banner">` — `translucent_header.rs:67` (já existia)
- `<aside class="shell-sidebar-wrapper">` — `app.rs:105` (novo wrapper semântico)
- `<main id="main-content" role="main">` — `app.rs:108` (id adicionado para skip-link)
- `role="dialog" aria-modal="true" aria-label="Command Palette"` — `command_palette.rs:447-449` (já existia)
- `aria-live="polite"` no ToastContainer — `toast_container.rs:233` (já existia)
- Skip-to-content link — `app.rs:102` (novo)

### ARIA Labels Adicionados
- VNC "Assumir Controle" — `vnc_canvas.rs:353`
- VNC "Liberar Controle" — `vnc_canvas.rs:375`
- VNC "Capturar Screenshot" — `vnc_canvas.rs:395`
- HITL "Aprovar esta ação" — `hitl_approval_panel.rs:323`
- HITL "Recusar esta ação" — `hitl_approval_panel.rs:326`
- HITL "Escalar para revisão humana" — `hitl_approval_panel.rs:329`
- Sidebar toggle, theme toggle, toast close, command palette — já existiam

### prefers-reduced-motion
Já implementado em `spring.rs:196-205` (`prefers_reduced_motion()` via `window.matchMedia`) e cacheado no `create_spring()` (linha 219). Springs retornam target instantaneamente quando reduce está ativo — zero RAF calls. Media query CSS correspondente já existe em `base.css:516`.

### Performance
- `[profile.wasm-release]` com `opt-level="z"`, LTO, `codegen-units=1`, `panic="abort"`, `strip="debuginfo"` já configurado no workspace root (`Cargo.toml:130-136`)
- Hook `wasm-opt -Oz --strip-debug --strip-producers` adicionado como post_build no `Trunk.toml`
- Perfil duplicado removido do crate-level Cargo.toml (era ignorado pelo Cargo)

session_id: local-0d7e9787e7b26ea6752a4c7c7562c397b1ccc6d9d9984a28422564f50bfdb0fb
