# Module Documentation Status Tracker — Zylos-Next

**Data:** 2026-11-04
**Escopo:** 23 crates + 4 apps
**Critérios:** README.md, `//!` module docs em lib.rs/main.rs, `///` inline docs em pub items, testes (`#[test]`/`#[cfg(test)]`)

## Legenda

- ✅ Presente / ≥1
- ❌ Ausente / 0
- ⚠️ Parcial (existe mas insuficiente)

## Crates (23)

| Crate | README | `//!` Module Docs | `///` Inline Docs | Pub Items | Tests | Doc Score |
|-------|--------|-------------------|-------------------|-----------|-------|-----------|
| zylos-task-durability | ✅ | ✅ 18 | ✅ 179 | 62 | 39 | 🟢 Completo |
| zylos-memory-system | ✅ | ✅ 7 | ✅ 355 | 160 | 77 | 🟢 Completo |
| zylos-verification | ✅ | ✅ 8 | ✅ 366 | 175 | 149 | 🟢 Completo |
| zylos-bot-system | ✅ | ✅ 6 | ✅ 224 | 118 | 43 | 🟢 Completo |
| zylos-hitl | ✅ | ✅ 19 | ⚠️ 19 | 31 | 17 | 🟡 Inline docs baixo |
| zylos-sentinel | ✅ | ✅ 15 | ⚠️ 41 | 19 | 18 | 🟡 Inline docs baixo |
| zylos-triage | ✅ | ✅ 14 | ⚠️ 36 | 16 | 19 | 🟡 Inline docs baixo |
| zylos-mobile | ✅ | ✅ 4 | ⚠️ 29 | 26 | 3 | 🟡 Poucos testes |
| zylos-live-comm | ✅ | ❌ 0 | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-skills | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-sandbox | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-router | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-panel | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-learn | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-cerebelo | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-resolver | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-proof | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-soar | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-vlm-ocr | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-search-mcp | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-browser | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-docker | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-core | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |

## Apps (4)

| App | README | `//!` Module Docs | `///` Inline Docs | Pub Items | Tests | Doc Score |
|-----|--------|-------------------|-------------------|-----------|-------|-----------|
| zylos-gateway | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-cockpit | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-daemon | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |
| zylos-cli | ❌ | ❌ | ❌ 0 | 0 | 0 | 🔴 Sem docs/code |

## Resumo Estatístico

| Métrica | Valor | % |
|---------|-------|---|
| Crates com README | 9/23 | 39% |
| Crates com `//!` module docs | 8/23 | 35% |
| Crates com `///` inline docs (>0) | 8/23 | 35% |
| Crates com testes (>0) | 8/23 | 35% |
| Crates 🟢 completos | 4/23 | 17% |
| Crates 🟡 parciais | 4/23 | 17% |
| Crates 🔴 sem docs/code | 15/23 | 65% |
| Apps com README | 0/4 | 0% |
| Apps com qualquer doc | 0/4 | 0% |

## Notas

- Os 15 crates marcados como 🔴 podem ser stubs/placeholders criados no workspace Cargo mas sem código-fonte implementado ainda. Verificar se há `src/lib.rs` ou `src/main.rs` antes de exigir documentação.
- Os 4 crates 🟡 (hitl, sentinel, triage, mobile) têm estrutura mas precisam de mais inline docs e testes.
- `zylos-live-comm` tem README e lib.rs mas zero docs inline — provável placeholder.
- **Doc Score** é calculado como: README (1pt) + module docs ≥5 (1pt) + inline docs ≥50 (1pt) + tests ≥10 (1pt). 🟢=4pts, 🟡=2-3pts, 🔴=0-1pt.

## Ações Prioritárias

1. **Verificar stubs:** Confirmar quais dos 15 crates 🔴 são placeholders vs. crates que deveriam ter código
2. **Completar 🟡:** Adicionar inline docs e testes aos 4 crates parciais
3. **READMEs para apps:** Criar README.md para gateway, cockpit, daemon, cli
4. **Module docs:** Adicionar `//!` aos crates que têm código mas faltam module docs