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

## Metas (revistas em 2026-11-20)

| Onde | Meta | Medido | Situação |
|---|---|---|---|
| Desktop | LCP < 2 s | 0,8 s | cumprida |
| Mobile simulado (4G lento + CPU 4x) | LCP <= 4 s | 3,5 a 3,9 s | cumprida, com pouca folga |
| Mobile simulado | LCP < 2 s (meta original) | 3,5 a 3,9 s | **não alcançável** neste framework (ver Conclusão) |

A meta original "LCP < 2 s" (PRD, TESTING, EVALUATION, IMPLEMENTATION_PLAN, runbook) valia para
tudo e foi dividida em duas. É uma decisão de produto: para voltar à meta única basta reverter o
commit que trouxe esta tabela. Rodar a medição com `lighthouse --preset=mobile` (padrão: Slow 4G,
CPU 4x) contra o `dist/` servido com brotli, em todo PR.

## Por que o relatório de 2026-11-18 diz 1,3 s

`docs/fase11-reports/lighthouse-mobile-2026-11-18.md` anuncia LCP de 1,275 s e performance 0,99.
Não é uma medição que contradiga esta, e o próprio relatório tem dois problemas:

- O **JSON salvo ao lado dele** (mesmo Lighthouse 12.8.2, mesmo estrangulamento) diz performance
  **0,87**, LCP **1,39 s** e TBT 481 ms: o resumo em texto não bate com o arquivo que o sustenta (e a
  mensagem do commit, "mediana 81", não bate com nenhum dos dois).
- O elemento do LCP nesse JSON é o `<p>` do **banner de erro estático** ("Connection lost") que o
  `index.html` desenha antes do wasm carregar, medido **sem gateway**. Isso mede a velocidade do HTML
  estático, não o momento em que o cockpit fica usável. O número desta página mede o app montado.

Os dois números são verdadeiros para o que medem; só o desta página responde "quando o painel
aparece".

## SEO 63 é esperado, não é defeito

O cockpit é um painel privado e o `robots.txt` (`User-agent: *` / `Disallow: /`) bloqueia a
indexação de propósito. Medido em 2026-11-20 com Lighthouse 13.6 sobre o mesmo `dist/`, mudando
**só** o `robots.txt`:

| `robots.txt` | SEO | Auditorias que falham |
|---|---|---|
| `Disallow: /` (o do repositório) | **63** | só `is-crawlable` |
| `Allow: /` | **100** | nenhuma |

Ou seja, tudo o mais de SEO (título, descrição, viewport, links) já passa; o 63 é exatamente o preço
de não aparecer no Google. **Não mude o `robots.txt` para subir a nota**: liberar a indexação de um
painel interno é decisão do Victor, não de métrica. Se um dia houver uma página pública (landing),
ela deve ser outro site com o próprio `robots.txt`.

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
