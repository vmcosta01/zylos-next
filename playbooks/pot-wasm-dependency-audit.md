# POT — WASM Dependency Audit (Otimização de Tamanho)

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.1 T11.1.2
**Público:** Dev Pleno, Arquiteto
**Tempo Estimado:** 30–60 minutos para aplicar otimizações

---

## 1. Visão Geral

Auditoria das dependências do cockpit Zylos compiladas para `wasm32-unknown-unknown`, identificando oportunidades de redução de bundle size via desativação de features desnecessárias e eliminação de duplicatas.

**Baseline atual:** 5.02 MB bruto / 1.44 MB gzipped (ADR-011)

---

## 2. Dependências Principais (por impacto estimado)

| Crate | Versão | Features Ativas | Impacto Estimado | Otimizável? |
|-------|--------|----------------|-----------------|-------------|
| `leptos` | 0.7.8 | csr, hydrate, ssr, hydration, default | Alto (~40% bundle) | ⚠️ Parcial |
| `web-sys` | 0.3.107 | 25 features individuais | Médio (~15% bundle) | ✅ Sim |
| `gloo-net` | 0.6.0 | default, http, eventsource, websocket, json, serde | Médio (~10% bundle) | ✅ Sim |
| `serde_json` | 1.0.154 | default, std, alloc | Baixo (~5% bundle) | ❌ Não |
| `uuid` | 1.x | v4, js | Baixo (~3% bundle) | ❌ Não |
| `js-sys` | 0.3.107 | default, std, unsafe-eval | Baixo (~5% bundle) | ❌ Não |
| `console_error_panic_hook` | 0.1.7 | default | Mínimo (~1% bundle) | ❌ Não |

---

## 3. Oportunidades de Otimização Identificadas

### 3.1 Leptos: Remover feature `ssr` em builds WASM-only

**Problema:** A feature `ssr` (server-side rendering) inclui código de renderização HTML no servidor que nunca é executado em ambiente browser-only. O cockpit é puramente CSR (client-side rendering).

**Impacto estimado:** -200 a -400 KB bruto (-50 a -100 KB gzipped)

**Risco:** Médio — `leptos_router` pode depender de `ssr` internamente. Requer teste.

**Ação:**
```toml
# Em apps/zylos-cockpit/Cargo.toml
leptos = { version = "0.7", default-features = false, features = ["csr"] }
leptos_router = { version = "0.7", default-features = false }
```

**Validação:** `trunk build --release` + `wasm-pack test --headless --chrome` devem passar.

### 3.2 Gloo-net: Desativar features não utilizadas

**Problema:** `gloo-net` ativa por default: `eventsource`, `websocket`, `json`, `http`. O cockpit usa apenas `http` para fetch REST. EventSource e WebSocket são usados indiretamente pelo `leptos_router` (não pelo cockpit diretamente).

**Impacto estimado:** -50 a -100 KB bruto (-15 a -30 KB gzipped)

**Risco:** Alto — `leptos_router` depende de `gloo-net` com features default. Mudar pode quebrar roteamento.

**Ação:** Testar primeiro se `leptos_router` funciona sem eventsource/websocket:
```toml
gloo-net = { version = "0.6", default-features = false, features = ["http", "json"] }
```

**Validação:** Navegação entre tabs do cockpit deve funcionar.

### 3.3 Web-sys: Reduzir features SVG se não usadas

**Problema:** 6 features SVG ativas (`SvgElement`, `SvgGraphicsElement`, `SvgCircleElement`, `SvgLineElement`, `SvgTextElement`, `SvgsvgElement`). Usadas apenas pelo `SwarmTopologyPanel`. Se este componente for lazy-loaded ou removido, essas features podem ser desativadas.

**Impacto estimado:** -30 a -60 KB bruto (-10 a -20 KB gzipped)

**Risco:** Baixo — apenas afeta SwarmTopologyPanel.

**Ação:** Manter por enquanto; reconsiderar após code splitting (T11.1.5).

### 3.4 Duplicatas de `convert_case`

**Problema:** Duas versões de `convert_case` (v0.6.0 e v0.7.1) incluídas via proc-macros do leptos. Não é otimizável diretamente (dependência transitiva de macros).

**Impacto:** Mínimo (proc-macros rodam em compile-time, não em runtime WASM).

**Ação:** Nenhuma. Monitorar updates do leptos que unifiquem versões.

---

## 4. Dependências NÃO Otimizáveis

| Crate | Razão |
|-------|-------|
| `serde` + `serde_json` | Essenciais para serialização gateway↔cockpit |
| `uuid` com feature `js` | Obrigatória para geração de IDs em WASM (ADR-011 fix) |
| `js-sys` | Binding fundamental para JS interop |
| `console_error_panic_hook` | Debug essencial em desenvolvimento |
| `wasm-bindgen` | Infraestrutura WASM obrigatória |
| `gloo-timers` | Usado para delays async no cockpit |

---

## 5. Recomendações Priorizadas

| Prioridade | Ação | Impacto Esperado | Esforço | Risco |
|-----------|------|-----------------|---------|-------|
| P1 | Remover `ssr` feature do leptos | -50 a -100 KB gzipped | Baixo | Médio |
| P2 | Desativar eventsource/websocket no gloo-net | -15 a -30 KB gzipped | Baixo | Alto |
| P3 | Code splitting via `leptos::lazy` (T11.1.5) | -30% initial load | Alto | Baixo |
| P4 | Reduzir web-sys SVG features | -10 a -20 KB gzipped | Baixo | Baixo |

**Nota:** Com baseline de 1.44 MB gzipped (71% abaixo do target de 5MB), todas as otimizações acima são **opcionais**. O bundle já atende os requisitos com folga significativa.

---

## 6. Metodologia de Auditoria

```bash
# Listar dependências diretas com features
cargo tree --target wasm32-unknown-unknown -e features --depth 1

# Verificar árvore de uma crate específica
cargo tree --target wasm32-unknown-unknown -e features -i <crate-name>

# Encontrar duplicatas
cargo tree --target wasm32-unknown-unknown --duplicates

# Medir tamanho real (requer trunk build --release)
gzip -c dist/*_bg.wasm | wc -c
```

---

## 7. Referências

- [ADR-011: Profile Release Optimization Strategy](../docs/DECISIONS_FASE11.md#adr-011-profile-release-optimization-strategy)
- [BENCHMARK_RELEASE_PROFILES.md](../docs/BENCHMARK_RELEASE_PROFILES.md)
- [Leptos 0.7 Feature Flags](https://docs.rs/leptos/0.7/leptos/)
- [Gloo-net Features](https://docs.rs/gloo-net/0.6/gloo_net/)

---

*Documento gerado conforme skill `rust-performance-tuning`. Próxima revisão: após aplicação de P1/P2 ou upgrade do leptos.*