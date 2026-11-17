# Lighthouse Baseline — Zylos Cockpit (T11.5.1)

**Data da coleta:** 2026-11-17T12:51:05Z
**Ferramenta:** Lighthouse 12.8.2, preset `desktop` (RTT 40ms, 10 Mbps, CPU 1x)
**URL:** `https://fix-ci-trigger-master.zylos-cockpit-preview.pages.dev/` (Cloudflare Pages preview — produção com custom domain ainda não existe, ver T11.4.1)
**Relatório bruto:** [`lighthouse-baseline.json`](./lighthouse-baseline.json)
**POT:** [`playbooks/pot-lighthouse-baseline.md`](../../playbooks/pot-lighthouse-baseline.md)

## Scores

| Categoria | Score | Meta 11.5 |
|---|---|---|
| Performance | **80** | ≥90 ❌ |
| Accessibility | **94** | ≥90 ✅ |
| Best Practices | **96** | ≥95 ✅ |
| SEO | 45 | fora de escopo |

## Métricas

| Métrica | Valor | Meta |
|---|---|---|
| First Contentful Paint | 1.9 s | <1.5s ❌ |
| Largest Contentful Paint | 2.2 s | <2.5s ✅ |
| Total Blocking Time | 30 ms | — |
| Cumulative Layout Shift | 0 | <0.1 ✅ |
| Speed Index | 1.9 s | — |
| Time to Interactive | 2.2 s | — |

## Rede (transfer size)

| Recurso | Bytes | Observação |
|---|---|---|
| `/` (HTML) | 1 457 | — |
| `design-tokens-*.css` | 3 738 | render-blocking |
| `style-*.css` | 8 462 | render-blocking |
| `zylos-cockpit-*.js` | 8 773 | glue wasm-bindgen |
| `zylos-cockpit-*_bg.wasm` | 1 562 563 | ~1.5 MB comprimido |
| `/manifest.json` | 1 449 | **servido como `text/html`** (fallback SPA) |
| `/favicon.ico` | 1 445 | **servido como `text/html`** |
| `http://localhost:3000/api/v1/health` | 0 | `ERR_CONNECTION_REFUSED` |
| `…pages.devhttp//localhost:3000/api/v1/events` | 0 | URL malformada (origin + endpoint absoluto) |

## Auditorias reprovadas

| Auditoria | Causa raiz identificada |
|---|---|
| `first-contentful-paint` (0.35) | Nada é pintado antes de baixar 2 CSS bloqueantes + instanciar o WASM de 1.5 MB; `#app` vazio no HTML. |
| `render-blocking-insight` | `design-tokens.css` e `style.css` como `<link rel=stylesheet>` no `<head>`. |
| `network-dependency-tree-insight` | Cadeia HTML → manifest (903 ms); sem `preload` do WASM/JS. |
| `largest-contentful-paint-element` | LCP é o `<p>` do `error-banner` com "Connection lost: JsValue(TypeError: Failed to fetch…)" — o gateway não está acessível no preview. |
| `color-contrast` | `.cta-retry` — `#fff` sobre `--accent-blue #3B82F6` = **3.67:1** (exige 4.5:1). `apps/zylos-cockpit/style.css` (`:root` do cockpit e regra `.cta-primary, .cta-retry`). |
| `errors-in-console` | (1) `net::ERR_CONNECTION_REFUSED` para `localhost:3000`; (2) `Manifest: Line: 1, column: 1, Syntax error` (manifest = HTML); (3) `The script has an unsupported MIME type ('text/html')` — `/sw.js` não existe no `dist`. |
| `valid-source-maps` | WASM sem source map (Best Practices; tratado em `pop-wasm-source-maps.md`). |
| `meta-description` | ausente no `index.html`. |
| `robots-txt` | `/robots.txt` cai no fallback SPA e retorna HTML (30 erros de parsing). |
| `is-crawlable` | header `x-robots-tag: noindex` — **padrão do Cloudflare Pages em deploys de preview**, não configurável pelo projeto; some no domínio de produção. |

## Diagnóstico de causa raiz — assets ausentes

`Trunk.toml` declara `[[copy]] from = "sw.js"` e `from = "manifest.json"`, mas o **Trunk 0.22 ignora a seção `[[copy]]`** (não faz parte do schema de configuração). O jeito suportado é `<link data-trunk rel="copy-file" href="…">` no `index.html`. Além disso, `manifest.json` referencia `/icon-192.png` e `/icon-512.png`, que não existem no repositório.

## Plano derivado

1. Inline do CSS crítico + carregamento não-bloqueante do restante (T11.5.2).
2. `preload` de WASM/JS via Trunk, `preconnect` ao gateway (T11.5.3).
3. Ícones PNG/WebP reais, `copy-file` de manifest/sw/robots/favicon (T11.5.4).
4. Contraste `.cta-retry`, falhas de rede silenciosas, meta description (T11.5.5/11.5.6).
