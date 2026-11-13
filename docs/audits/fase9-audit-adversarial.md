# Auditoria Adversarial — Zylos Cockpit v2 (Fases 1–4)

**Data:** 2026-11-12
**Auditor:** Juiz Adversarial (subagente)
**Escopo:** Compilação, testes, gateway, documentação, CSS, i18n, integridade de componentes, conformidade com Manifesto v3.1

---

## Veredito: CHANGES_REQUIRED

A compilação do workspace passa, mas os **testes do cockpit falham** com 4 erros de compilação. A documentação de módulos não lista os componentes do cockpit individualmente. Esses dois pontos bloqueiam a aprovação.

---

## Findings Críticos (bloqueiam aprovação)

### C1. Testes `e2e_components` não compilam — módulo `components` não é público na lib

- **Arquivo:** `apps/zylos-cockpit/tests/e2e_components.rs` (linhas 44, 64, 82, 106)
- **Erro:** `cannot find 'components' in 'zylos_cockpit'` (E0433 × 4)
- **Causa:** O `lib.rs` do cockpit não re-exporta o módulo `components` como `pub mod components`. Os testes de integração tentam acessar `zylos_cockpit::components::*` mas o caminho não existe.
- **Impacto:** `cargo test -p zylos-cockpit --tests` **falha**. Zero testes e2e executam.
- **Correção:** Adicionar `pub mod components;` no `lib.rs` ou mover os tipos usados nos testes para um módulo público dedicado.

### C2. MODULE_DOCUMENTATION_STATUS.md não lista os 14 componentes do cockpit

- **Arquivo:** `docs/MODULE_DOCUMENTATION_STATUS.md`
- **Problema:** O documento lista o app `zylos-cockpit` como uma linha única ("Dashboard web Leptos WASM") mas **não enumera os 14 componentes individuais** (channel_panel, phone_relay_panel, skill_editor_panel, swarm_topology_panel, chat_panel, fleet_auditor, subagent_status_cards, emoji_reactions, agent_avatars, voice_panel, file_manager, task_triage, thread_manager, skin_themes).
- **Critério não atendido:** "MODULE_DOCUMENTATION_STATUS.md lista 14 componentes cockpit?" → **NÃO** (0 menções diretas aos nomes dos componentes).
- **Correção:** Adicionar seção dedicada listando cada componente com status de docs, LOC e testes.

---

## Warnings (não bloqueiam mas devem ser documentados)

### W1. 8 warnings de código morto/no-unused no cockpit

| Arquivo | Warning | Tipo |
|---------|---------|------|
| `components/mod.rs:31` | unused import `AgentAvatar` | unused_imports |
| `components/emoji_reactions.rs:34` | unused variable `message_id` | unused_variables |
| `components/fleet_auditor.rs:115` | unused variable `sorted_by_cost` | unused_variables |
| `components/fleet_auditor.rs:121` | unused variable `sorted_by_effectiveness` | unused_variables |
| `components/chat_panel.rs:43` | method `css_class` never used | dead_code |
| `components/subagent_status_cards.rs:26` | method `as_str` never used | dead_code |
| `components/agent_avatars.rs:145` | function `from_label` never used | dead_code |
| `components/fleet_auditor.rs:36` | method `cost_per_action` never used | dead_code |

**Recomendação:** Prefixar com `_` ou remover antes do merge.

### W2. Future incompatibility em dependência transitiva

- `proc-macro-error2 v2.0.1` será rejeitado por versão futura do Rust.
- Não bloqueia hoje, mas deve ser rastreado.

### W3. HANDOFF_FASE9.md lista tasks agregadas, não 8/8 explícitas da Fase 4

- O documento contém 18 menções a "Task" mas as tasks da Fase 4 estão agrupadas (ex: "Tasks 3.1–3.6" em uma linha, "Task 4.1" a "Task 4.8" listadas mas sem seções individuais detalhadas).
- O critério "reflete 8/8 tasks da Fase 4" é **parcialmente atendido**: todas as 8 estão mencionadas, mas sem estrutura uniforme de descrição/status.

### W4. TRACKRECORD_FASE9.md tem entradas cronológicas mas apenas das tasks de crates

- As 4 tasks novas de componentes cockpit (Channel, PhoneRelay, SkillEditor, SwarmTopology) **não têm entrada dedicada** no trackrecord. As últimas entradas são de Computer View, Model Config e Spaces (tasks de crate, não de componente UI).
- O critério "tem entrada cronológica das 4 tasks novas" → **PARCIAL** (faltam as 4 tasks de UI da Fase 4).

---

## Itens Aprovados ✅

| # | Critério | Status | Evidência |
|---|----------|--------|-----------|
| 1 | `cargo check --workspace` zero erros | ✅ PASS | exit_code 0, apenas warnings |
| 3 | `cargo check -p zylos-gateway` passa | ✅ PASS | exit_code 0 |
| 3b | Routers `channels_api` e `computer_view_api` integrados no lib.rs | ✅ PASS | Linhas 4, 5, 127, 128 confirmadas |
| 5 | CSS para os 4 painéis novos | ✅ PASS | `.channel-panel` (L445), `.phone-relay-panel` (L472), `.skill-editor-panel` (L507), `.swarm-topology-panel` (L535) |
| 6 | i18n PT-BR e EN-US para as 4 tabs | ✅ PASS | `tab.channels`, `tab.phone_relay`, `tab.skill_editor`, `tab.swarm_topology` presentes em ambos os idiomas |
| 7 | Integridade dos 4 componentes | ✅ PASS | Todos registrados em `mod.rs` (pub mod + pub use), `app.rs` (enum Tab, css_class, i18n_key, render match, TabButton nav) |

---

## Métricas

| Métrica | Valor |
|---------|-------|
| Workspace compila | ✅ Sim (0 erros, ~10 warnings) |
| Testes cockpit (`--tests`) | ❌ Falha (4 erros E0433, timeout após 300s) |
| Componentes cockpit registrados | 14 (confirmado via mod.rs: 28 linhas = 14 pares mod+use) |
| Componentes documentados individualmente | 0 (gap crítico) |
| Tasks Fase 4 no HANDOFF | 8 mencionadas, estrutura parcial |
| Tasks Fase 4 no TRACKRECORD | 0 entradas dedicadas (apenas crates) |
| CSS painéis novos | 4/4 presentes |
| i18n keys novas | 4/4 em PT-BR + EN-US |
| Gateway routers integrados | 2/2 confirmados |
| Docs operacionais Fase 9 | 29 documentos (playbooks, POPs, runbooks, SOARs) |
| Apps 🟢 Completo | 5/5 |
| Crates 🟢 Completo | 33/46 |
| Crates 🟡 Parcial | 13/46 |

---

## Conformidade Manifesto v3.1

| Camada | Status | Nota |
|--------|--------|------|
| Código funcional | ⚠️ Parcial | Compila mas testes e2e quebrados |
| Testes automatizados | ❌ Falho | e2e_components não compila |
| Documentação inline | ✅ OK | `//!` e `///` presentes nos crates novos |
| README por módulo | ✅ OK | Todos os crates novos têm README |
| Playbooks/POPs/SOARs | ✅ OK | 29 docs operacionais listados |
| Handoff estruturado | ⚠️ Parcial | Tasks agrupadas, falta detalhe individual |
| Trackrecord cronológico | ⚠️ Parcial | Faltam entradas das 4 tasks de UI |
| Anti-slop | ✅ OK | Código real, sem placeholders inventados |
| Loop profissional | ⚠️ Parcial | READ→UPDATE_DOCS incompleto (componentes não documentados) |
| Verificação adversarial | ✅ Este documento | Primeira passagem completa |

---

## Plano de Correção Obrigatório

1. **[CRÍTICO]** Expor `pub mod components;` no `lib.rs` do cockpit OU criar módulo de tipos públicos para testes → re-executar `cargo test -p zylos-cockpit --tests` até passar
2. **[CRÍTICO]** Adicionar seção "Componentes Cockpit (14)" no `MODULE_DOCUMENTATION_STATUS.md` com nome, LOC, status de docs e testes de cada componente
3. **[WARNING]** Adicionar 4 entradas cronológicas no `TRACKRECORD_FASE9.md` para ChannelPanel, PhoneRelayPanel, SkillEditorPanel, SwarmTopologyPanel
4. **[WARNING]** Resolver os 8 warnings de dead_code/unused (prefixar `_` ou remover)
5. **[WARNING]** Detalhar as 8 tasks da Fase 4 individualmente no HANDOFF_FASE9.md

**Re-auditoria recomendada após correções dos itens C1 e C2.**