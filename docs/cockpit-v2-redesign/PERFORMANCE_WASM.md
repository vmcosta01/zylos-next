# Desempenho do wasm do cockpit: o que pesa e o que dá para ganhar

Medido em 2026-11-20, build `release` (perfil `wasm-release`, `opt-level = "z"`, `wasm-opt -Oz`),
servido com brotli (como o Cloudflare Pages).

## Números

| Cenário | Performance | FCP | LCP | TBT | CLS |
|---|---|---|---|---|---|
| Lighthouse **desktop** | 99 | 0,6 s | **0,8 s** | 0 ms | 0 |
| Lighthouse **mobile** (4G lento + CPU 4x, simulado) | 84 a 91 | 1,1 a 3,0 s | **3,5 a 3,9 s** | 40 a 50 ms | 0 |

O módulo principal tem 1,02 MB bruto (cerca de 333 KB com brotli). A meta "LCP < 2 s" dos
documentos (PRD, TESTING, EVALUATION) é para 3G/mobile: **no desktop já se cumpre; no mobile
simulado não**.

## O que pesa (código do módulo principal, antes do `wasm-opt`, por crate dono da função)

| Parte | Fatia do código |
|---|---|
| Leptos: `tachys` 29,1% + `reactive_graph` 11,6% + `leptos` 1,7% | **cerca de 42%** |
| Código do próprio cockpit (`zylos_cockpit`, 293 funções) | 18,6% |
| `core` / `alloc` / `hashbrown` / `std` | cerca de 15% |
| `serde_json` + `serde_core` | 5,4% |
| `js_sys` + `web_sys` + `wasm_bindgen` | cerca de 7% |

Reproduzir: `cargo build -p zylos-cockpit --bin zylos-cockpit --target wasm32-unknown-unknown
--profile wasm-release` e `node scripts/wasm-size.mjs target/wasm32-unknown-unknown/wasm-release/zylos-cockpit.wasm`
(o `.wasm` do `dist/` não tem nomes: o `wasm-opt` os remove). Sem instalar ferramentas.

## O que foi tentado

- `opt-level = "s"` no lugar de `"z"`: wasm **maior** (1,22 MB contra 1,01 MB). Mantido `"z"`.
- Dependências diretas do cockpit já são mínimas (chrono, gloo, js-sys, leptos, serde, serde_json,
  uuid, wasm-bindgen, web-sys, yrs); `yrs` só entra pela lib e é podado no módulo principal.
- Dois scripts bloqueantes no `<head>` foram removidos (PR #11).

## Conclusão

Mesmo movendo **todo** o código do cockpit para fora do módulo principal, o ganho seria de no
máximo cerca de 19% do código; o piso é o próprio Leptos (cerca de 42%), que não sai sem trocar de
framework. Chegar a LCP < 2 s no mobile com 4x de CPU lenta **não é alcançável** só com ajustes
de build ou divisão de código.

**Recomendação (decisão do Victor):** manter "LCP < 2 s" como meta de **desktop** (hoje 0,8 s) e
tratar o mobile simulado com uma meta realista (por exemplo LCP <= 4 s), medindo em todo PR. Se o
mobile for prioridade de verdade, o caminho é outro: casca estática com o primeiro conteúdo no
HTML (SSR/pré-render do Chat) ou outro framework, o que é uma decisão de arquitetura.
