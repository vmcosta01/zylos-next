# Module Documentation Status Tracker — Zylos-Next

**Data:** 2026-11-11 (atualizado pós-Fase 9, Fases 1–4 completas)
**Escopo:** 46 crates + 5 apps (51 módulos totais)
**Critérios:** README.md, `//!` module docs, `///` inline docs em pub items, testes (`#[test]`)

## Legenda

- 🟢 **Completo** — README + `//!` + `///` em ≥80% dos pub items + testes
- 🟡 **Parcial** — Código existe mas falta docs inline ou testes
- 🔴 **Mínimo** — Sem README e/ou sem docs inline e/ou sem testes

## Resumo

| Status | Crates | Apps | Total |
|--------|--------|------|-------|
| 🟢 Completo | 33 | 5 | 38 |
| 🟡 Parcial | 13 | 0 | 13 |
| 🔴 Mínimo | 0 | 0 | 0 |
| **Total** | **46** | **5** | **51** |

## Crates 🟢 Completo (33)

| Crate | LOC | `//!` | `///` | Pub | Tests | README |
|-------|-----|---------|---------|-----|-------|--------|
| zylos-task-durability | — | 18 | 179 | 62 | 39 | ✅ |
| zylos-memory-system | — | 7 | 355 | 160 | 77 | ✅ |
| zylos-verification | — | 8 | 366 | 175 | 149 | ✅ |
| zylos-bot-system | — | 6 | 224 | 118 | 43 | ✅ |
| zylos-vault | — | 3 | 45 | — | 11 | ✅ |
| zylos-templates | — | 4 | 52 | — | 13 | ✅ |
| zylos-a2m | — | 5 | 68 | — | 14 | ✅ |
| zylos-integrations | — | 6 | 85 | — | 11 | ✅ |
| zylos-sandbox | — | 8 | 120 | — | 69 | ✅ |
| zylos-hitl | 1040 | 19 | 85+ | 31 | 17 | ✅ |
| zylos-sentinel | 595 | 15 | 60+ | 19 | 18 | ✅ |
| zylos-triage | 589 | 14 | 55+ | 16 | 19 | ✅ |
| zylos-mobile | 1266 | 4 | 45+ | 26 | 29 | ✅ |
| zylos-swarm-orchestrator | 1257 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-adapters | 1003 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-evidence-ledger | 961 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-goals | 941 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-browser-takeover | 846 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-agent-shield | 809 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-personalities | 807 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-memory-sync | 652 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-browser-persistent | 579 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-ag-ui | 547 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-docker | 478 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-hermes-adapter | 278 | ✅ | ✅ | ✅ | ✅ | ✅ |
| zylos-independent-verifier | 760 | ✅ | ✅ | ✅ | 16 | ✅ |
| zylos-approval | ~450 | ✅ | ✅ | ✅ | 12 | ✅ |
| zylos-egress | ~320 | ✅ | ✅ | ✅ | — | ✅ |
| zylos-audit-log | ~380 | ✅ | ✅ | ✅ | 9 | ✅ |
| zylos-brief | ~420 | ✅ | ✅ | ✅ | — | ✅ |
| zylos-model-config | ~350 | ✅ | ✅ | ✅ | — | ✅ |
| zylos-computer-view | ~520 | ✅ | ✅ | ✅ | 23 | ✅ |
| zylos-spaces | ~480 | ✅ | ✅ | ✅ | — | ✅ |

## Módulos Novos Fase 8 (adicionados aos crates existentes)

| Crate | Módulo Novo | Testes | README Atualizado | Hermes Endpoint |
|-------|-------------|--------|-------------------|-----------------|
| zylos-swarm-orchestrator | hop_count.rs, crash_simulation.rs | 71 | ✅ | — |
| zylos-bot-system | goal_program.rs, autoloop_protocol.rs | 21 | ✅ | — |
| zylos-memory-system | lifecycle.rs, mental_models.rs, context_room.rs, auto_improvement.rs | 41 | ✅ | via memory_api |
| zylos-agent-shield | resource_limiter.rs, network_egress.rs, output_sanitizer.rs, capability_token.rs, host_allocator.rs | 85 | ✅ | hermes_security_api |
| zylos-skills | contract.rs, evolution.rs, miner.rs, trajectory.rs, hierarchy.rs | 88 | ✅ | hermes_skill_evolution_api |
| zylos-personalities | roles.rs, routine_catalog.rs, plan_first_gate.rs, rate_limiter.rs, guardrails.rs | 95 | ✅ | hermes_employee_api, hermes_security_api |
| zylos-desktop | workspace_persistence.rs, agent_awareness.rs, relay_server.rs, mobile_responsive.rs | 83 | ✅ | hermes_desktop_api |
| zylos-gateway | hermes_security_api.rs, hermes_desktop_api.rs, hermes_skill_evolution_api.rs, hermes_employee_api.rs | 4 | ✅ | 28 endpoints novos |

## Documentação Operacional Fase 8

| Documento | Tipo | Path |
|-----------|------|------|
| HANDOFF_FASE8.md | Handoff | docs/handoffs/HANDOFF_FASE8.md |
| pop-guardrail-rule-update.md | POP | playbooks/pop-guardrail-rule-update.md |
| pop-skill-contract-validation.md | POP | playbooks/pop-skill-contract-validation.md |
| pop-relay-token-revoke.md | POP | playbooks/pop-relay-token-revoke.md |
| skill-evolution-lifecycle.md | Playbook | playbooks/skill-evolution-lifecycle.md |
| soar-guardrail-enforcement.md | SOAR | playbooks/soar-guardrail-enforcement.md |
| zylos-desktop-fase8.md | Runbook | runbooks/zylos-desktop-fase8.md |

## Documentação Operacional Fase 9 (Fases 1–4)

| Documento | Tipo | Path |
|-----------|------|------|
| HANDOFF_FASE9.md | Handoff | docs/handoffs/HANDOFF_FASE9.md |
| pb-approval-gate-setup.md | Playbook | playbooks/pb-approval-gate-setup.md |
| pb-egress-rules-management.md | Playbook | playbooks/pb-egress-rules-management.md |
| pb-morning-brief-config.md | Playbook | playbooks/pb-morning-brief-config.md |
| pb-model-tier-strategy.md | Playbook | playbooks/pb-model-tier-strategy.md |
| pb-computer-view-setup.md | Playbook | playbooks/pb-computer-view-setup.md |
| pb-spaces-onboarding.md | Playbook | playbooks/pb-spaces-onboarding.md |
| pop-approval-decide.md | POP | playbooks/pop-approval-decide.md |
| pop-password-masking.md | POP | playbooks/pop-password-masking.md |
| pop-audit-log-export.md | POP | playbooks/pop-audit-log-export.md |
| pop-egress-false-positive.md | POP | playbooks/pop-egress-false-positive.md |
| pop-brief-regenerate.md | POP | playbooks/pop-brief-regenerate.md |
| pop-model-fallback.md | POP | playbooks/pop-model-fallback.md |
| pop-computer-control-emergency.md | POP | playbooks/pop-computer-control-emergency.md |
| pop-space-page-recovery.md | POP | playbooks/pop-space-page-recovery.md |
| rb-zylos-approval.md | Runbook | runbooks/rb-zylos-approval.md |
| rb-zylos-egress.md | Runbook | runbooks/rb-zylos-egress.md |
| rb-zylos-audit-log.md | Runbook | runbooks/rb-zylos-audit-log.md |
| rb-zylos-brief.md | Runbook | runbooks/rb-zylos-brief.md |
| rb-zylos-model-config.md | Runbook | runbooks/rb-zylos-model-config.md |
| rb-zylos-computer-view.md | Runbook | runbooks/rb-zylos-computer-view.md |
| rb-zylos-spaces.md | Runbook | runbooks/rb-zylos-spaces.md |
| soar-approval-timeout.md | SOAR | docs/soar/soar-approval-timeout.md |
| soar-egress-block-response.md | SOAR | docs/soar/soar-egress-block-response.md |
| soar-audit-anomaly.md | SOAR | docs/soar/soar-audit-anomaly.md |
|| soar-computer-control-breach.md | SOAR | docs/soar/soar-computer-control-breach.md |

## Componentes Cockpit (14)

| Componente | Arquivo | Tab | i18n PT-BR | i18n EN-US | CSS | Gateway Endpoint |
|------------|---------|-----|-----------|-----------|-----|-----------------|
| TaskTriagePanel | task_triage_panel.rs | TaskTriage | Triage | Triage | ✅ | /api/v1/tasks |
| ThreadManager | thread_manager.rs | Threads | Threads | Threads | ✅ | — (local) |
| SubagentStatusCards | subagent_status_cards.rs | Subagents | Subagentes | Subagents | ✅ | /api/v1/hermes/bot/* |
| FileManager | file_manager.rs | Files | Arquivos | Files | ✅ | — (local) |
| VoicePanel | voice_panel.rs | Voice | Voz | Voice | ✅ | — (local) |
| EmojiReactions | emoji_reactions.rs | Reactions | Reações | Reactions | ✅ | — (local) |
| AgentAvatarsGrid | agent_avatars.rs | Avatars | Avatares | Avatars | ✅ | — (local) |
| SkinThemesPanel | skin_themes.rs | Skins | Temas | Skins | ✅ | — (local) |
| FleetAuditorPanel | fleet_auditor.rs | Fleet | Frota | Fleet | ✅ | /api/v1/hermes/bot/* |
| FederationDashboard | federation_dashboard.rs | Federation | Federação | Federation | ✅ | — (placeholder) |
| ChannelPanel | channel_panel.rs | Channels | Canais | Channels | ✅ | /api/v1/channels |
| PhoneRelayPanel | phone_relay_panel.rs | PhoneRelay | Phone Relay | Phone Relay | ✅ | /api/v1/computer-view/sessions |
| SkillEditorPanel | skill_editor_panel.rs | SkillEditor | Editor de Skills | Skill Editor | ✅ | /api/v1/hermes/skill-evolution/* |
| SwarmTopologyPanel | swarm_topology_panel.rs | SwarmTopology | Topologia Swarm | Swarm Topology | ✅ | /api/v1/hermes/bot/spawn-crew |

## Documentação Operacional Fase 9 (Fases 1–4)

| Documento | Tipo | Path |
|-----------|------|------|
| HANDOFF_FASE9.md | Handoff | docs/handoffs/HANDOFF_FASE9.md |
| pb-approval-gate-setup.md | Playbook | playbooks/pb-approval-gate-setup.md |
| pb-egress-rules-management.md | Playbook | playbooks/pb-egress-rules-management.md |
| pb-morning-brief-config.md | Playbook | playbooks/pb-morning-brief-config.md |
| pb-model-tier-strategy.md | Playbook | playbooks/pb-model-tier-strategy.md |
| pb-computer-view-setup.md | Playbook | playbooks/pb-computer-view-setup.md |
| pb-spaces-onboarding.md | Playbook | playbooks/pb-spaces-onboarding.md |
| pop-approval-decide.md | POP | playbooks/pop-approval-decide.md |
| pop-password-masking.md | POP | playbooks/pop-password-masking.md |
| pop-audit-log-export.md | POP | playbooks/pop-audit-log-export.md |
| pop-egress-false-positive.md | POP | playbooks/pop-egress-false-positive.md |
| pop-brief-regenerate.md | POP | playbooks/pop-brief-regenerate.md |
| pop-model-fallback.md | POP | playbooks/pop-model-fallback.md |
| pop-computer-control-emergency.md | POP | playbooks/pop-computer-control-emergency.md |
| pop-space-page-recovery.md | POP | playbooks/pop-space-page-recovery.md |
| rb-zylos-approval.md | Runbook | runbooks/rb-zylos-approval.md |
| rb-zylos-egress.md | Runbook | runbooks/rb-zylos-egress.md |
| rb-zylos-audit-log.md | Runbook | runbooks/rb-zylos-audit-log.md |
| rb-zylos-brief.md | Runbook | runbooks/rb-zylos-brief.md |
| rb-zylos-model-config.md | Runbook | runbooks/rb-zylos-model-config.md |
| rb-zylos-computer-view.md | Runbook | runbooks/rb-zylos-computer-view.md |
| rb-zylos-spaces.md | Runbook | runbooks/rb-zylos-spaces.md |
| soar-approval-timeout.md | SOAR | docs/soar/soar-approval-timeout.md |
| soar-egress-block-response.md | SOAR | docs/soar/soar-egress-block-response.md |
| soar-audit-anomaly.md | SOAR | docs/soar/soar-audit-anomaly.md |
| soar-computer-control-breach.md | SOAR | docs/soar/soar-computer-control-breach.md |

## Crates 🟡 Parcial (13)

Código substancial existe; precisa melhorar cobertura de `///` inline docs ou adicionar testes.

| Crate | LOC | `//!` | `///` | Pub | Doc% | Tests | README | Falta |
|-------|-----|---------|---------|-----|------|-------|--------|-------|
| zylos-live-comm | 9274 | 113 | 195 | 334 | 58% | 152 | ✅ | Docs em 42% dos pub items |
| zylos-skills | 6060 | 65 | 71 | 215 | 33% | 120 | ✅ | Docs em 67% dos pub items |
| zylos-soar | 4270 | 40 | 72 | 122 | 59% | 59 | ✅ | Docs em 41% dos pub items |
| zylos-proof | 1880 | 15 | 66 | 60 | 110% | 65 | ✅ | ✅ Docs completas |
| zylos-desktop | 1531 | 31 | 19 | 60 | 32% | 18 | ❌ | README + docs |
| zylos-browser | 1506 | 17 | 21 | 60 | 35% | 17 | ✅ | Docs em 65% dos pub items |
| zylos-router | 1413 | 19 | 13 | 46 | 28% | 19 | ✅ | Docs em 72% dos pub items |
| zylos-vlm-ocr | 711 | 12 | 22 | 27 | 81% | 16 | ✅ | Quase completo |
| zylos-search-mcp | 464 | 7 | 11 | 18 | 61% | 0 | ✅ | Testes |
| zylos-resolver | 383 | 3 | 0 | 31 | 0% | 22 | ✅ | Docs inline |
| zylos-cerebelo | 322 | 4 | 2 | 24 | 8% | 11 | ✅ | Docs inline |
| zylos-learn | 306 | 4 | 5 | 15 | 33% | 8 | ✅ | Docs + testes |
| zylos-panel | 214 | 3 | 0 | 12 | 0% | 19 | ✅ | Docs inline |

## Crates 🔴 Mínimo (0)

Nenhum crate nesta categoria. Todos os 39 crates possuem pelo menos README ou docs inline.

## Apps 🟢 Completo (5)

| App | LOC | Files | Tests | README | Runbook | SOAR | Nota |
|-----|-----|-------|-------|--------|---------|------|------|
| zylos-gateway | 972 | 6 | 36 | ✅ | ✅ | ✅ | API gateway consolidada (Axum) |
| zylos-cockpit | 2264 | 11 | 32 | ✅ | ✅ | ✅ | Dashboard web Leptos WASM |
| zylos-desktop | 302 | 4 | 0 | ✅ | ✅ | ✅ | Tauri v2 + SolidJS shell |
| zylos-cli | ~350 | 1 | 8 | ✅ | ✅ | ✅ | CLI completa (clap + reqwest, subcommands status/agent/task/config/logs) |
| zylos-daemon | ~300 | 1 | 6 | ✅ | ✅ | ✅ | Daemon lifecycle manager (axum health server + supervisor + graceful shutdown) |

## Notas

- 2026-11-07: Todos os 5 apps agora possuem README + runbook operacional + SOAR runbook com cross-links tridirecionais (playbook_ref + runbook_refs).
- 2026-11-07: zylos-gateway, zylos-cockpit e zylos-desktop classificados como 🟢 Completo (código implementado + docs completas).
- 2026-11-07: zylos-cli e zylos-daemon promovidos a 🟢 Completo após implementação real com testes.
- 2026-11-06: Varredura automatizada revelou que 26 crates marcados como "sem docs/code" na verdade têm código substancial (214–9274 LOC). Status reclassificado.
- 2026-11-06: Os 4 crates 🟡 anteriores (hitl, sentinel, triage, mobile) foram completados com inline docs e testes.
- 2026-11-07: zylos-core confirmado com 268 LOC, 14 testes e docs completas (AgentId, CoreError, Result).
- 2026-11-07: 12 crates 🔴 documentados via subagentes paralelos (README + inline docs + cargo check): swarm-orchestrator, adapters, evidence-ledger, goals, browser-takeover, agent-shield, personalities, memory-sync, browser-persistent, ag-ui, docker, hermes-adapter.
- DOC% = (`///` inline docs / pub items) × 100. Meta é ≥80%.

## Ações Prioritárias

1. ~~**Crates 🔴 (13):**~~ ✅ Concluído (12 crates documentados via subagentes + zylos-core já completo)
2. **Crates 🟡 (13):** Completar docs inline nos 13 crates parciais (foco nos com Doc% < 50%)
3. ~~**Apps 🟡 (2):**~~ ✅ Concluído (zylos-cli e zylos-daemon implementados com testes)
4. ~~**zylos-core:**~~ ✅ Concluído (268 LOC, 14 testes, docs completas)
