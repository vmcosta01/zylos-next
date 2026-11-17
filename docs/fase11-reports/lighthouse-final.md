# Lighthouse Final — Zylos Cockpit (T11.5.7)

**Coleta:** 2026-11-17T17:06Z (desktop) / 17:07Z (mobile) — Lighthouse 12.8.2
**Build:** `trunk build --release` (Trunk 0.22.1, profile release do workspace: opt-level 3, lto, cg-units 1)
**Servidor:** `npx http-server dist -p 8089 -s -g -b` (localhost, gzip pré-comprimido)
**Relatórios brutos:** [`lighthouse-final.json`](./lighthouse-final.json) (desktop), [`lighthouse-final-mobile.json`](./lighthouse-final-mobile.json) (mobile, throttling padrão Moto G Power / Slow 4G)
**Baseline:** [`lighthouse-baseline.md`](./lighthouse-baseline.md) (somente desktop, Cloudflare Pages preview)

> ⚠️ O ambiente difere do baseline: o final roda em localhost (sem RTT real da CDN) e o baseline no preview do Cloudflare Pages. Parte do ganho de FCP vem do ambiente, não só das otimizações. O baseline não teve rodada mobile, então mobile não tem comparação direta.

## Scores

| Categoria | Baseline (desktop) | Final desktop | Final mobile | Meta |
|---|---|---|---|---|
| Performance | 80 | **93** ✅ | **71** ❌ | ≥90 |
| Accessibility | 94 | **100** ✅ | **100** ✅ | ≥90 |
| Best Practices | 96 | 96 | 96 | ≥95 |
| SEO | 45 | 63 | 63 | fora de escopo (`robots.txt: Disallow: /` intencional, ferramenta interna) |

## Métricas

| Métrica | Baseline desktop | Final desktop | Final mobile |
|---|---|---|---|
| FCP | 1.9 s | 0.3 s | 1.0 s |
| LCP | 2.2 s | 1.8 s | 9.4 s |
| TBT | 30 ms | 50 ms | 240 ms |
| CLS | 0 | 0 | 0 |
| Speed Index | 1.9 s | 0.4 s | 1.0 s |
| TTI | 2.2 s | 1.8 s | 9.4 s |

## O que mudou (T11.5.2–T11.5.6)

- **11.5.2:** shell crítico inline (`<style>` + `#app-splash`) no `index.html`, pintado antes do WASM. ⚠️ `data-inline` em `rel="css"` **não teve efeito no Trunk 0.22.1**: `design-tokens-*.css` e `style-*.css` ainda saem como `<link rel="stylesheet">` bloqueantes (Lighthouse: render-blocking ≈150 ms de economia estimada).
- **11.5.3:** o Trunk injeta automaticamente `modulepreload` do JS + `preload` do WASM; `preconnect` ao gateway só quando a meta `zylos-gateway-url` é preenchida; `_headers` com cache imutável para `.wasm`/`.js`.
- **11.5.4:** `icons/` (SVG + PNG + WebP 192/512, favicon-32); `manifest.json`, `sw.js`, `robots.txt`, `_headers` agora copiados para o `dist` via `copy-file`/`copy-dir` (antes caíam no fallback SPA como `text/html`).
- **11.5.5/11.5.6:** `runbooks/rb-accessibility-audit.md`; contraste do `.cta-primary/.cta-retry` → `#2563EB` (5.17:1); `lang="pt-BR"`, meta description, aria; `src/gateway_config.rs` resolve a URL do gateway (evita chamada fixa a `localhost:3000` em produção).

## Gate T11.5.7

- **Desktop: APROVADO** — perf 93, a11y 100.
- **Mobile: REPROVADO em performance (71).** Causa: LCP/TTI de 9.4 s, dominado pelo download + compilação do `*_bg.wasm` (5.27 MB bruto / 1.51 MB gzip) em Slow 4G + CPU 4x. O elemento LCP é o `<p>` renderizado pelo app depois do boot do WASM, não o splash.
- A rodada de correção mobile não foi feita nesta sessão: cada build release leva ~27 min e os ganhos exigem mudanças estruturais (abaixo).

## Pendências para mobile ≥90

1. Reduzir o WASM: o `[profile.release]` do crate (`opt-level="z"`, `lto="thin"`) é **ignorado** (o Cargo só lê profiles da raiz do workspace); usar um profile dedicado (ex. `[profile.wasm-release]` com `opt-level="z"`) via `data-cargo-profile`, e conferir se o `wasm-opt` está rodando de fato (nada no log do trunk mostra isso).
2. Servir com Brotli em produção (Cloudflare faz isso por padrão; o localhost só fez gzip).
3. Fazer o splash ser o elemento LCP (texto maior/mais visível dentro do shell crítico) para que o LCP não espere o WASM.
4. Inline real do CSS (passo pós-build, ou mover o CSS crítico para o `<style>`), já que o `data-inline` do Trunk não funcionou.
5. Ruído do console: `ERR_CONNECTION_REFUSED` (o gateway não está rodando no teste local) custa ponto em Best Practices.
