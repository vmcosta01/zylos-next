# HANDOFF_4 — SANDBOX_ISOLATION → Fase 5

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Fase Anterior:** Fase 3 (Live Communication)
**Próxima Fase:** Fase 5 (Skills Registry)

---

## 1. Resumo Executivo

A Fase 4 implementou o sistema de isolamento e execução segura do Zylos-Next, cobrindo:
- **WasmSandbox:** Execução determinística via wasmtime 25.0 com memory/fuel limits e zero network/fs access
- **DockerSandbox:** Containers isolados com cgroups, network mode none, auto-remove e cleanup garantido
- **SearchMCP:** Servidor MCP de busca paralela com fan-out multi-provider, deduplicação e merge de resultados
- **Sprint 4.2.1:** Browser headless anti-detect (7/7 testes)
- **Sprint 4.2.2:** WebMCP spec compliance (6/6 testes)
- **Sprint 4.2.3:** Parallel Search MCP (8/8 testes)

Todos os critérios de saída foram atendidos. 21/21 testes core + 21 testes sprint passando, clippy limpo. Dupla auditoria aprovada.

---

## 2. Artefatos Entregues

### 2.1 Crates e Módulos

| Crate/Módulo | Arquivos | Testes | Status |
|--------------|----------|--------|--------|
| zylos-sandbox (wasm) | wasm_sandbox.rs, config.rs, error.rs | 5 passed | ✅ |
| zylos-sandbox (docker) | docker_sandbox.rs, cleanup.rs, cgroups.rs | 8 passed | ✅ |
| zylos-search-mcp | search_engine.rs, provider.rs, dedup.rs, merge.rs | 8 passed | ✅ |
| zylos-browser | anti_detect.rs, fingerprint.rs, stealth.rs | 7 passed | ✅ |

### 2.2 Documentação

| Documento | Caminho | Status |
|-----------|---------|--------|
| Runbook Fase 4 | `runbooks/fase4-sandbox-isolation.md` | ✅ Completo (3 tiers) |
| Runbook Sprint 4.2.1 | `runbooks/browser-headless-anti-detect.md` | ✅ 7/7 testes |
| Runbook Sprint 4.2.2 | `runbooks/webmcp-spec-compliance.md` | ✅ 6/6 testes |
| Runbook Sprint 4.2.3 | `runbooks/parallel-search-mcp.md` | ✅ 8/8 testes |
| Auditoria Adversarial | `evals/audits/fase4-audit-adversarial.md` | ✅ APPROVED |
| Auditoria Isolada | `evals/audits/fase4-audit-isolated.md` | ✅ APPROVED |
| HANDOFF_4 | `HANDOFF_4.md` | ✅ Este documento |

### 2.3 Métricas Alcançadas

| Métrica | Target | Obtido | Status |
|---------|--------|--------|--------|
| Testes WasmSandbox | ≥5 | 5 | ✅ |
| Testes DockerSandbox | ≥8 | 8 | ✅ |
| Testes SearchMCP | ≥8 | 8 | ✅ |
| Wasm cold start | <10ms | Medido em bench | ✅ |
| Docker spawn | <500ms | Medido em bench | ✅ |
| Security violations | 0 escapes | 0 escapes | ✅ |
| Container órfãos | 0 | Auto-remove OK | ✅ |
| Clippy -D warnings | 0 errors | 0 errors | ✅ |

---

## 3. Claims Parciais (Não-Blockers)

1. **WasmSandbox fuel exhaustion granularity:** Fuel limit previne loops infinitos mas não distingue entre código legítimo pesado e malicioso; ajuste fino requer profiling por skill.
2. **DockerSandbox cgroups v2 only:** Hosts com cgroups v1 apenas não são suportados; documentado como requisito mínimo.

Nenhuma claim foi REFUTED.

---

## 4. Lições Aprendidas

- `wasmtime::Store::set_fuel()` deve ser chamado ANTES de cada invocação; fuel não é resetado automaticamente entre calls
- Docker `auto_remove: true` falha silenciosamente se container estiver em estado paused; adicionar health check pré-cleanup
- SearchMCP fan-out precisa de timeout individual por provider; um provider lento não pode bloquear todos os resultados
- Deduplicação de resultados de busca deve usar hash normalizado (lowercase, trim whitespace) para evitar falsos negativos
- Browser anti-detect: `navigator.webdriver = false` é detectável via prototype chain; usar Object.defineProperty override
- WebMCP spec exige `tools/list` antes de qualquer `tools/call`; clientes que pulam listagem devem receber erro claro

---

## 5. Gate de Entrada para Fase 5

Conforme PLAYBOOK.md §3.1, validação pré-Fase 5:

- [x] Todos os documentos obrigatórios da Fase 4 estão APPROVED
- [x] HANDOFF_4.md existe com 5 seções completas
- [x] Benchmarks da Fase 4 passam nos targets definidos
- [x] Auditoria adversarial (dupla) está verde
- [x] Runbook da Fase 4 existe com 3 tiers (Sênior/Pleno/Júnior)
- [x] Runbooks de sprint 4.2.x existem e estão completos
- [x] MEMORY.md reflete estado atual e lições aprendidas

**Status: ✅ GATE APROVADO — Fase 5 pode iniciar.**

---

## 6. Recomendações para Fase 5

1. **Skills Registry** deve validar skills contra sandbox capabilities antes de registro (skill que requer network não pode rodar em WasmSandbox sem allow_network)
2. **Eval Flywheel** deve executar skills dentro de sandbox apropriado (WASM para deterministic, Docker para SO completo)
3. **Debt Tracking** deve rastrear shortcuts relacionados a sandbox configuration (ex: `shortcut: skip-sandbox` é debt crítico de segurança)
4. **Scheduler Rotinas** precisa de timeout enforcement via sandbox fuel/docker kill
5. **Skills API** deve expor campo `sandbox_type` por skill para clients escolherem runtime adequado

---

**Assinatura:** Hermes Agent (default profile)
**Data:** 2026-11-02