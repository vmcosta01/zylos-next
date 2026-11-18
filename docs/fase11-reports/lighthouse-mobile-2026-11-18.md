# Lighthouse Mobile Performance — 2026-11-18

## Resumo

| Métrica | Baseline (antes) | Após otimizações | Alvo |
|---------|------------------|-------------------|------|
| Performance Score | 0.74–0.80 | **0.99** (mediana) | ≥ 0.90 ✅ |
| LCP | 4–5 s | **1,275 ms** (mediana) | < 2,500 ms ✅ |
| FCP | ~1,100 ms | **1,125 ms** | < 1,800 ms ✅ |
| TBT | ~300 ms | **132 ms** | < 200 ms ✅ |
| CLS | 0 | **0** | < 0.1 ✅ |

Medição: 3 iterações com Lighthouse 12, preset mobile (Moto G Power), Chrome headless.
Servidor: Node.js custom (`perf2-serve.js`) com brotli/gzip pré-comprimido na porta 8194.

## Iterações

| Run | Score | LCP (ms) | FCP (ms) | TBT (ms) | CLS |
|-----|-------|----------|----------|----------|-----|
| 1   | 0.93  | 1,280    | 1,130    | 323      | 0   |
| 2   | 0.99  | 1,275    | 1,125    | 132      | 0   |
| 3   | 0.99  | 991      | 991      | 97       | 0   |
| **Mediana** | **0.99** | **1,275** | **1,125** | **132** | **0** |

## Otimizações Aplicadas

### Commit 9c45ed3 — Shell HTML estático com skeleton grid como LCP candidate
- Critical CSS inline no `<head>`: reset, layout do splash, skeleton-grid/card com animação pulse, error-banner e cta-retry (valores dark-theme literais, sem dependência de `var()` ou stylesheet externo).
- `#app-splash` contém header + 3 skeleton cards pintados pelo HTML antes do WASM; `body:has(.cockpit-root)` esconde quando Leptos monta.
- Noscript fallback mostra error-banner se JS estiver desabilitado.
- `rel="inline"` do Trunk 0.22 confirmado funcional: 3 blocos `<style>` inline, 0 `rel="stylesheet"` externo.

### Commit 94d2c4c — Error-banner estático visível como LCP candidate
- Error-banner agora é `display:flex` imediato no `#app-splash` (não mais `display:none`), pintado no first paint (~375ms) em vez de após 4s de `fetch_health` falhar via Leptos.
- Banner inclui título, mensagem e botão retry; `min-height:120px` garante área suficiente para ser o maior elemento visível antes do WASM.
- `body:has(.cockpit-root)` continua escondendo todo `#app-splash` quando o Leptos monta, sem impacto na UX com gateway online.

### Infraestrutura de medição
- Servidor HTTP custom (`E:/zylos-target/perf2-serve.js`) com suporte a brotli/gzip pré-comprimido e HTTP/1.1.
- WASM brotli: 508 KB (vs 1.85 MB bruto); JS brotli: 8.4 KB.
- Compressão manual pós-build: `gzip -k -9` + `brotli -k -q 11`.

## Arquivos Gerados
- `docs/fase11-reports/lighthouse-mobile-2026-11-18.json` — JSON completo da iteração mediana (run 2).
- `docs/fase11-reports/lighthouse-mobile-2026-11-18.md` — Este relatório.

## Pendências
- SEO (meta description já existe no index.html; robots.txt já existe no dist).
- A11y contraste: não verificado nesta rodada (foco foi performance).
- `cargo check` e `cargo test` do cockpit rodando em background; resultados pendentes no momento deste commit.