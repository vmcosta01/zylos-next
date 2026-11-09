# Module Documentation Status Tracker — Zylos-Next

**Data:** 2026-11-06 (varredura automatizada)
**Escopo:** 39 crates + 5 apps (44 módulos totais)
**Critérios:** README.md, `//!` module docs, `///` inline docs em pub items, testes (`#[test]`)

## Legenda

- 🟢 **Completo** — README + `//!` + `///` em ≥80% dos pub items + testes
- 🟡 **Parcial** — Código existe mas falta docs inline ou testes
- 🔴 **Mínimo** — Sem README e/ou sem docs inline e/ou sem testes

## Resumo

| Status | Crates | Apps | Total |
|--------|--------|------|-------|
| 🟢 Completo | 26 | 5 | 31 |
| 🟡 Parcial | 13 | 0 | 13 |
| 🔴 Mínimo | 0 | 0 | 0 |
| **Total** | **39** | **5** | **44** |

## Crates 🟢 Completo (26)

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
