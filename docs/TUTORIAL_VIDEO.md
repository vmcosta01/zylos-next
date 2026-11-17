# Vídeo Tutorial (5 min): Workflows Principais do Cockpit Zylos

**Fase:** 11.6.5 · **Data:** 2026-11-17 · **Versão do roteiro:** 1.0
**Status:** roteiro pronto e **aguardando aprovação do Tech Lead** (critério de aceitação da Fase 11.6).
**Link do vídeo:** _ainda não gravado_. Preencher aqui após a gravação: `<URL>`.

> Todos os comandos, telas e caminhos do roteiro existem no repo em 2026-11-17. As cenas de cockpit usam como referência visual os PNGs reais em `docs/assets/screenshots/` (ver `docs/USER_GUIDE.md` §9).

---

## Ficha técnica

| Item | Valor |
|------|-------|
| Duração alvo | 5:00 (tolerância ±15s) |
| Público | Operadores e devs novos (complementa `docs/ONBOARDING_OPERATORS.md` e `docs/ONBOARDING_DEVS.md`) |
| Resolução | 1920×1080 (cockpit em janela 1280×800 centralizada, zoom 125%) |
| Idioma | Português (BR), legendas PT-BR |
| Ambiente de gravação | Máquina local com gateway (`cargo run -p zylos-gateway`) + cockpit (`trunk serve --port 3001`) |
| Pré-gravação | Gateway já compilado; `ZYLOS_HERMES_TOKEN` exportado; terminal com fonte ≥ 16pt; tema escuro |

---

## Roteiro por cena

### Cena 1 — Abertura (0:00–0:20)
- **Tela:** cockpit com o header "Zylos Cockpit" e as tabs (referência: `docs/assets/screenshots/01-agents.png`).
- **Narração:** "Este é o Cockpit Zylos, a interface web em Rust/WASM que opera o sistema multi-agente Zylos-Next. Em cinco minutos você vai subir o ambiente, entender o painel e executar os três workflows do dia a dia."
- **Texto na tela:** "Zylos Cockpit — 31 tabs · Leptos 0.7 · WASM".

### Cena 2 — Subindo o ambiente (0:20–1:05)
- **Tela:** terminal dividido.
- **Ações e comandos:**
  1. `cargo run -p zylos-gateway` → destacar o log "Gateway configuration loaded" e o endereço `127.0.0.1:3000`.
  2. `curl http://127.0.0.1:3000/api/v1/health` → destacar `"status":"ok"`.
  3. `cd apps/zylos-cockpit && trunk serve --port 3001` → abrir `http://localhost:3001`.
- **Narração:** "O gateway escuta na porta 3000 e só aceita requisições desta máquina. O cockpit roda na 3001. No Windows, o `scripts/zylos-start.sh` faz os dois passos de uma vez."
- **Lower third:** "Dica: `ZYLOS_LISTEN` muda a porta do gateway."

### Cena 3 — Lendo o painel (1:05–1:45)
- **Tela:** tab **Agentes** (ref. `01-agents.png`).
- **Ações:** apontar os 3 cards de métricas (Status, Streams ativos, Versão); usar o filtro "Running"; buscar "Reviewer".
- **Narração:** "No topo ficam a saúde do gateway, os streams ativos e a versão. Abaixo, o roster de agentes com filtro por status: Running, Idle e Blocked."

### Cena 4 — Workflow 1: Triagem e aprovação HITL (1:45–2:40)
- **Tela:** tab **Triagem** (ref. `17-task-triage.png`) e depois a tab de aprovações HITL (ref. `09-hitl.png`).
- **Ações:** mostrar as colunas da triagem; abrir um card pendente; aprovar com justificativa.
- **Narração:** "A Triagem separa o que precisa de você do que pode esperar. Ações de risco viram aprovações HITL: leia o contexto, decida e registre o motivo. Toda decisão fica auditável no ledger de evidências."
- **Lower third:** "Procedimento: `playbooks/pop-approval-decide.md`".
- **Observação de produção:** com gateway sem dados, os painéis mostram erro ou ficam vazios. Gravar com dados de seed ou substituir esta cena por narração sobre os screenshots, sinalizando "ilustrativo".

### Cena 5 — Workflow 2: Monitorar e reagir (2:40–3:35)
- **Tela:** tab **Logs** (ref. `11-logs.png`), depois **SOAR** (ref. `12-soar.png`).
- **Ações:** mostrar o feed "Aguardando eventos..."; na aba SOAR, percorrer Visão Geral → Playbooks → Eventos.
- **Narração:** "Os Logs mostram eventos em tempo real via SSE. Quando o Sentinel detecta anomalia, o SOAR executa o playbook correspondente. Aqui você vê playbooks ativos e isolados e a saúde da validação."
- **Lower third:** "Alertas: `playbooks/pop-sentinel-alert-investigation.md`".

### Cena 6 — Workflow 3: Comunicação em canais (3:35–4:15)
- **Tela:** tab **Canais** (ref. `28-channels.png`).
- **Ações:** selecionar "# Geral", clicar "Entrar", digitar uma mensagem e enviar com Enter.
- **Narração:** "Os Canais conectam humanos e agentes, com histórico persistido em SQLite no gateway. Entre, escreva e envie com Enter."

### Cena 7 — Quando algo dá errado (4:15–4:45)
- **Tela:** parar o gateway (Ctrl+C) → cockpit exibe "Connection lost" (ref. `00-offline-gateway.png`) → subir de novo → clicar "Tentar novamente".
- **Narração:** "Se o gateway cair, o cockpit avisa na hora. Suba o gateway de novo e clique em Tentar novamente. Para produção, siga o `playbooks/pop-gateway-restart.md`."

### Cena 8 — Encerramento (4:45–5:00)
- **Tela:** cartão final com links.
- **Narração:** "Para continuar: o Guia do Usuário tem todas as 31 tabs, o FAQ responde as 30 dúvidas mais comuns e os guias de onboarding levam você do zero à autonomia."
- **Texto na tela:** `docs/USER_GUIDE.md` · `docs/FAQ.md` · `docs/ONBOARDING_OPERATORS.md` · `docs/ONBOARDING_DEVS.md`.

---

## Tabela de tempos (controle de edição)

| # | Cena | Início | Fim | Duração |
|---|------|--------|-----|---------|
| 1 | Abertura | 0:00 | 0:20 | 20s |
| 2 | Subindo o ambiente | 0:20 | 1:05 | 45s |
| 3 | Lendo o painel | 1:05 | 1:45 | 40s |
| 4 | Triagem + HITL | 1:45 | 2:40 | 55s |
| 5 | Monitorar e reagir | 2:40 | 3:35 | 55s |
| 6 | Canais | 3:35 | 4:15 | 40s |
| 7 | Recuperação | 4:15 | 4:45 | 30s |
| 8 | Encerramento | 4:45 | 5:00 | 15s |
| | **Total** | | | **5:00** |

## Checklist de gravação
- [ ] Gateway e cockpit rodando localmente (não usar o preview público, que não tem backend)
- [ ] Dados de seed carregados para as cenas 4–6, ou cenas marcadas como "ilustrativo"
- [ ] Nenhum token, `.zylos-hermes-token` ou segredo visível na tela/terminal
- [ ] Bug das tabs com rótulo cru (`tab.hitl_approval`, etc.) corrigido antes de gravar, ou evitado no enquadramento
- [ ] Legendas PT-BR revisadas
- [ ] Link publicado no topo deste arquivo

## Aprovação

| Papel | Nome | Data | Decisão |
|-------|------|------|---------|
| Tech Lead | | | ☐ Aprovado ☐ Ajustes |
| Autor do roteiro | Hermes Agent | 2026-11-17 | Submetido |
