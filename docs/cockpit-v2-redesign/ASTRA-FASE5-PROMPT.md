# Zylos Cockpit v2.0 — Fase 5: VNC Stream + Fallback (ADR-003)

**Role:** Senior Frontend Engineer (Leptos 0.7 / Rust WASM)
**Model:** GPT-6 Astra (via ghostcli)
**Starting commit:** `d090cc2` (Fase 1-4 complete)

## Context
Fases 1-4 do redesign do Cockpit v2.0 estão completas e commitadas no master:
- Fase 1 (`47e8608`): design system unificado (ADR-008)
- Fase 2 (`4671e86`): layout shell translúcido + sidebar colapsável
- Fase 3 (`09d87fa`): spring animation system (ADR-007)
- Fase 4 (`c959c37`): inline approval cards reais (ADR-001)

Agora você implementa a **Fase 5: VNC Stream + Fallback (ADR-003)**.

**Leia ANTES de começar:**
1. `docs/cockpit-v2-redesign/IMPLEMENTATION_PLAN.md` — Seção "Fase 5" com tarefas atômicas
2. `docs/cockpit-v2-redesign/DESIGN_SYSTEM.md` — Seções relevantes
3. `docs/cockpit-v2-redesign/APP_FLOW.md` — Seção AF-07 (Phone Relay VNC)
4. `docs/cockpit-v2-redesign/DECISIONS.md` — ADR-003 (VNC Primário + Screenshot Fallback)

## Problema Atual
O arquivo `apps/zylos-cockpit/src/components/phone_relay_panel.rs` tem uma div estática na linha ~241:
```html
<div class="relay-screen relay-inactive">
    <p>"Nenhuma sessão de relay ativa."</p>
</div>
```
A ADR-003 decidiu que esta área deve mostrar um **stream VNC real via WebSocket** com **screenshot fallback** automático.

## Endpoints Backend EXISTENTES (não criar novos, exceto screenshot)
- `GET /api/v1/computer-view/sessions` — lista sessões ativas
- `POST /api/v1/computer-view/sessions` — cria nova sessão
- `GET /api/v1/computer-view/sessions/:id` — detalhes da sessão
- `DELETE /api/v1/computer-view/sessions/:id` — termina sessão
- `POST /api/v1/computer-view/sessions/:id/take-control` — adquire lease de controle
- `DELETE /api/v1/computer-view/sessions/:id/take-control` — libera lease
- `GET /ws/computer-view/{session_id}` — **WebSocket para stream VNC** (webrtc_relay.rs)

## Endpoint Backend A CRIAR
- `GET /api/v1/computer-view/sessions/:id/screenshot` — retorna último frame como PNG/JPEG (fallback quando WS falha)
  - Implementar em `apps/zylos-gateway/src/computer_view_api.rs`
  - Se não houver frame disponível, retornar 404
  - Content-Type: image/png ou image/jpeg

## Fase 5 — O que implementar

### 5.1 Componente VncCanvas
- Conecta via WebSocket a `ws://gateway/ws/computer-view/{session_id}`
- Renderiza frames binários em `<canvas>` element
- Suporta JPEG frames (decode via `image` crate ou JS ImageBitmap)
- Spring entrance animation ao conectar (Fase 3)

### 5.2 Quality Monitor + Adaptive Scaling
- Track frame intervals (tempo entre frames recebidos)
- Se avg interval > 66ms (<15 FPS), reduzir resolução solicitada
- Enviar mensagem WS para servidor ajustar qualidade

### 5.3 ScreenshotFallback Component
- Quando WS desconecta ou timeout de 3s sem frames
- Faz GET `/api/v1/computer-view/sessions/:id/screenshot`
- Mostra imagem estática com overlay "Conexão perdida — tentando reconectar..."
- Auto-retry a cada 5s

### 5.4 Take Control / Release Control
- Botão "Assumir Controle" → POST `/api/v1/computer-view/sessions/:id/take-control`
- Mostra badge "Controlando" quando lease ativo
- Auto-release após 5 min de inatividade (timer client-side)
- Botão "Liberar Controle" → DELETE `/api/v1/computer-view/sessions/:id/take-control`

### 5.5 QR Code para Pareamento (Empty State)
- Quando nenhuma sessão existe, mostra QR code para pareamento
- Usa endpoint existente `POST /api/v1/hermes/desktop/relay/pair-code`
- QR code em SVG (já existe no código atual)

### 5.6 Manual Screenshot Download
- Botão "Capturar Tela" → captura frame atual do canvas como PNG
- Download via Blob URL

### 5.7 Integração com Phone Relay Panel existente
- Substituir `<div class="relay-screen relay-inactive">` por `<VncStream session_id=... />`
- Manter a seção de dispositivos pareados intacta
- Manter o fluxo de pareamento existente

## Arquivos a modificar/criar
- `apps/zylos-cockpit/src/components/phone_relay_panel.rs` — integrar VNC stream
- `apps/zylos-cockpit/src/components/vnc_canvas.rs` — **novo**: componente VNC
- `apps/zylos-cockpit/src/components/mod.rs` — exportar novo componente
- `apps/zylos-gateway/src/computer_view_api.rs` — adicionar endpoint screenshot
- `apps/zylos-cockpit/styles/base.css` — classes CSS para VNC canvas/fallback

## Regras
- **CARGO_TARGET_DIR=E:/zylos-target/cockpit-v2** sempre
- **Shell bash** no Windows (POSIX syntax)
- **Foreground timeout ~420s** → builds longos em background com log + polling
- **Commit atômico** ao final: `feat(cockpit-v2): Fase 5 — VNC stream real + screenshot fallback (ADR-003)`
- **Push** após commit: `GIT_TERMINAL_PROMPT=0 git -c credential.helper= -c credential.helper='!gh auth git-credential' push origin master`
- **Zero mocks** — usar API real; se nenhuma sessão ativa, mostrar Empty state honesto
- **Bundle budget** — adicionar <10KB ao WASM gzipped (sem crates pesadas de imagem)

## Verificação obrigatória (gate)
1. `cargo check -p zylos-gateway` — compila
2. `cargo check -p zylos-cockpit --target wasm32-unknown-unknown` — compila
3. `cargo test -p zylos-gateway --lib computer_view` — testes passam
4. `cargo test -p zylos-cockpit --lib` — testes passam
5. `cargo clippy -p zylos-gateway -p zylos-cockpit --all-targets -- -D warnings` — limpo
6. `cargo fmt --all --check` — formatado
7. Grep para stubs: zero `todo!()` ou `unimplemented!()` nos arquivos modificados

## Out of Scope (NÃO fazer nesta fase)
- Command palette (Fase 6)
- AgentCard redesign (Fase 7)
- Performance optimization final (Fase 8)
- WebRTC peer-to-peer (usar apenas WebSocket relay existente)
- Audio streaming

## Entrega
Ao completar, escrever relatório curto com:
- Commit hash
- Resultado de cada verificação (test result lines)
- Bundle size delta vs commit anterior
- Lista de arquivos criados/modificados
- Nota sobre o endpoint screenshot criado no backend

**Comece lendo `IMPLEMENTATION_PLAN.md` seção Fase 5 e o arquivo atual `phone_relay_panel.rs`.**