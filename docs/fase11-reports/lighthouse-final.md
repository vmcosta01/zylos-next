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

---

## Rodada de correção mobile (feat/f11-perf-fix, 2026-11-17)

**Build:** `trunk build --release` com `data-cargo-profile="wasm-release"` (novo `[profile.wasm-release]` na raiz: `opt-level="z"`, `lto=true`, `codegen-units=1`, `panic="abort"`, `strip="debuginfo"`) + `data-wasm-opt="z"`. O log do trunk (`-v`) confirma `cargo build ... --profile wasm-release` e `wasm-opt ... -Oz` rodando (binaryen version_131). Tempo: cargo 12m00s + wasm-opt ~9 s (`-j 4`).
**Outras mudanças:** `leptos` só com `csr` (`ssr`/`hydrate` viraram features opt-in do crate); CSS via `<link data-trunk rel="inline" type="css">` (inline real: o `dist/index.html` passa a ter os `<style>` e nenhum `<link rel="stylesheet">`).
**Lighthouse:** 12.8.2, localhost:8099, Chrome headless. Relatórios: [`lighthouse-perf-fix-mobile.json`](./lighthouse-perf-fix-mobile.json) e [`lighthouse-perf-fix-desktop.json`](./lighthouse-perf-fix-desktop.json) (rodada "brotli").

### Tamanhos (medidos)

| Artefato | Antes (fd3d13c) | Depois | Δ |
|---|---|---|---|
| `*_bg.wasm` bruto | 5.27 MB | **1,648,397 B (1.65 MB)** | −69 % |
| `*_bg.wasm` gzip -9 | 1.51 MB (1,514,638 B transferidos) | **619,438 B** | −59 % |
| `*_bg.wasm` brotli q11 | — | **455,952 B** | — |
| JS glue (bruto / gzip / br) | 8.5 KB transferidos | 51,374 / 8,753 / 7,483 B | ≈ |
| CSS bloqueante | 2 folhas (style 7.9 KB + tokens 3.6 KB transferidos) | **0** (inline; `index.html` 63,759 B bruto / 10,129 B br) | — |
| Total transferido (Lighthouse) | 1,502 KiB | 628 KiB (gzip) / **465 KiB** (brotli) | −58 % / −69 % |

### Scores (mobile = Moto G Power, Slow 4G, CPU 4x)

| Rodada | Servidor | Perf | FCP | LCP | TBT | SI |
|---|---|---|---|---|---|---|
| Antes (T11.5.7) | http-server gzip | 71 | 1.0 s | 9.4 s | 240 ms | 1.0 s |
| Depois #1 | http-server `-g` | **80** | 0.9 s | 5.0 s | 140 ms | 0.9 s |
| Depois #2 | http-server `-g` | 77 | 0.7 s | 4.9 s | 260 ms | 0.7 s |
| Depois #3 | http-server `-g` | 77 | 0.6 s | 4.9 s | 280 ms | 0.6 s |
| Depois #4 | node estático com `.br` pré-comprimido | 74 | 0.7 s | **4.1 s** | 510 ms | 1.1 s |
| Depois #5 | idem | 61 | 0.7 s | 4.0 s | 1,510 ms | 1.4 s |

Desktop: **99** nas três rodadas (FCP 0.2 s, LCP 0.8–1.0 s, TBT 0–40 ms). A11y 100 e Best Practices 96 (inalterados).

> O TBT oscila muito entre rodadas (140 → 1,510 ms) com o mesmo `dist`: a máquina de medição rodava builds cargo/Chrome concorrentes, e a #5 é outlier de ruído local. O LCP é estável (4.0–5.0 s).

### Gate mobile ≥90: **ainda REPROVADO** (melhor rodada: 80)

Diagnóstico das novas rodadas:
- **Render-blocking zerado.** O download caiu ~3x e o LCP foi de 9.4 s para 4.0–5.0 s.
- O elemento LCP continua sendo `div.cockpit-root > main.cockpit-main > div.error-banner > p` ("Connection lost: JsValue(TypeError: Failed to fetch …"): é o banner de erro que o app mostra porque **não há gateway em localhost:3000** durante o teste. Fases: TTFB ~0.5 s e *render delay* ~3.5–4.4 s (download + compile + boot do WASM + fetch que falha). O FCP (splash) sai em 0.6–0.9 s, mas o splash é menor que o banner, então não vira o LCP.
- Os `ERR_CONNECTION_REFUSED` também seguram Best Practices em 96.

Próximos passos (não feitos aqui, porque exigem mexer em `src/` do cockpit, que outro agente está editando):
1. Medir com o gateway rodando (ou com mock): o LCP atual é um artefato do erro de conexão, e a stack trace longa dentro do `<p>` gera um bloco de texto grande.
2. Não renderizar o `JsValue` cru (stack do wasm) no banner; com uma mensagem curta, o bloco fica menor que o splash.
3. Fazer do splash/app-shell o elemento LCP: título/skeleton do `#app-splash` com área ≥ à do primeiro conteúdo do app, para o LCP coincidir com o FCP (~0.7 s).
4. Code-splitting/lazy: o Leptos 0.7 não divide o WASM nativamente; avaliar adiar `yrs`/CRDT e os painéis secundários.
5. Em produção, o Cloudflare Pages entrega Brotli automaticamente, então os números "brotli" acima são os representativos.
