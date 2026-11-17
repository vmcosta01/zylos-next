# POT-Lighthouse-Baseline — Procedimento de Auditoria Lighthouse do Cockpit

**Versão:** 1.0.0
**Data:** 2026-11-17
**Fase:** 11.5 — T11.5.1
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.5
**Resultado:** [`docs/fase11-reports/lighthouse-baseline.md`](../docs/fase11-reports/lighthouse-baseline.md) · JSON bruto [`lighthouse-baseline.json`](../docs/fase11-reports/lighthouse-baseline.json)

---

## Propósito

Padronizar como medir Performance / Accessibility / Best Practices / SEO do cockpit, para que baseline (T11.5.1) e relatório final (T11.5.7) sejam comparáveis.

## Pré-requisitos

- Node ≥ 18 (`npx`), Chrome/Chromium instalado.
- Para medição local: `trunk` 0.22 e target `wasm32-unknown-unknown`.

## Parâmetros fixos (não alterar entre medições)

| Parâmetro | Valor |
|---|---|
| Versão | `lighthouse@12` (baseline: 12.8.2) |
| Preset principal | `--preset=desktop` (RTT 40ms, 10 Mbps, CPU 1x) |
| Preset secundário | padrão mobile (Moto G Power, 4G lento, CPU 4x) — informativo |
| Chrome | `--chrome-flags="--headless=new"` |
| Execuções | 1 por alvo; repetir 3x e usar a mediana se a variação de Performance for >5 pts |

## Procedimento A — URL publicada (preview/produção)

```bash
URL=https://<branch>.zylos-cockpit-preview.pages.dev/
npx -y lighthouse@12 "$URL" --quiet --chrome-flags="--headless=new" \
  --preset=desktop --output=json --output-path=docs/fase11-reports/lighthouse-<nome>.json
```

## Procedimento B — build local

```bash
export CARGO_TARGET_DIR=E:/zylos-target/web   # disco com espaço
cd apps/zylos-cockpit && trunk build --release
npx -y http-server dist -p 8089 -g -c-1 --silent &    # -g serve .gz pré-comprimidos se existirem
npx -y lighthouse@12 http://localhost:8089/ --quiet --chrome-flags="--headless=new" \
  --preset=desktop --output=json --output-path=../../docs/fase11-reports/lighthouse-local.json
```

Observação: `http-server` **não** aplica compressão dinâmica; o WASM local trafega sem gzip/brotli (≈4–5 MB vs ≈1.5 MB no Cloudflare). Pré-comprimir (`gzip -k -9 dist/*.wasm dist/*.js`) e usar `-g` para aproximar o cenário de produção.

## Extração dos números

```bash
jq '{perf:.categories.performance.score, a11y:.categories.accessibility.score,
     bp:.categories["best-practices"].score, seo:.categories.seo.score,
     fcp:.audits["first-contentful-paint"].displayValue,
     lcp:.audits["largest-contentful-paint"].displayValue,
     cls:.audits["cumulative-layout-shift"].displayValue,
     tbt:.audits["total-blocking-time"].displayValue}' lighthouse-<nome>.json
# Auditorias reprovadas:
jq -r '.audits[] | select(.score!=null and .score<0.9 and .scoreDisplayMode!="informative") | "\(.id) \(.score)"' lighthouse-<nome>.json
```

## Gate 11.5

```bash
jq '.categories.performance.score >= 0.9 and .categories.accessibility.score >= 0.9' lighthouse-<nome>.json | grep true && echo "GATE 11.5 VERDE"
```

## Interpretação conhecida (não são bugs do cockpit)

- **`is-crawlable` / SEO no preview:** Cloudflare Pages adiciona `x-robots-tag: noindex` em todo deploy de *preview* (`*.pages.dev` de branch). Não é configurável no projeto e desaparece no domínio de produção. SEO está fora de escopo (§11.5 Out of Scope); mesmo assim o cockpit publica `robots.txt` com `Disallow: /` por ser ferramenta interna.
- **LCP = banner de erro:** sem gateway acessível, o maior elemento é o `error-banner`. Medir com gateway configurado (meta `zylos-gateway-url`) quando a produção existir.
