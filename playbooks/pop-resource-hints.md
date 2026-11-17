# POP-Resource-Hints — Preload, Preconnect e Cache do Cockpit

**Versão:** 1.0.0
**Data:** 2026-11-17
**Fase:** 11.5 (Performance & Lighthouse) — T11.5.3
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.5

---

## Propósito

Encurtar a cadeia de dependências de rede (baseline: HTML → manifest 903 ms) e garantir que o WASM comece a baixar o mais cedo possível.

## Hints aplicados

| Hint | Recurso | Como | Observação |
|---|---|---|---|
| `modulepreload` | `zylos-cockpit-*.js` | automático pelo Trunk 0.22 (`data-trunk rel="rust"`) | conferir no `dist/index.html` |
| `preload as=fetch type=application/wasm crossorigin` | `zylos-cockpit-*_bg.wasm` | automático pelo Trunk 0.22 | WASM é o maior recurso (~1.5 MB gz) |
| `preconnect` | gateway | snippet inline lê `<meta name="zylos-gateway-url">` e injeta `<link rel=preconnect>` só se configurado | evita preconnect inútil para `localhost` |
| fontes | — | **nenhuma webfont é baixada** (`Inter`/`JetBrains Mono` só se instaladas localmente; fallback `system-ui`) | não há o que fazer `prefetch`; adicionar `preload` só se uma webfont for introduzida |
| CDN | — | todos os assets são same-origin (Cloudflare Pages) | não há CDN de terceiros para `preconnect` |

## Cache (`apps/zylos-cockpit/_headers`, Cloudflare Pages)

- `*.wasm`, `*.js` → `Cache-Control: public, max-age=31536000, immutable` (nomes com hash do Trunk).
- `/sw.js` → `no-cache` + `Content-Type: application/javascript`.
- `/manifest.json` → `application/manifest+json`, 1h.
- `/icons/*` → 7 dias.

O arquivo é copiado para o `dist` por `<link data-trunk rel="copy-file" href="_headers">`.

## Verificação

```bash
cd apps/zylos-cockpit && trunk build --release
grep -oE '<link rel="(modulepreload|preload)"[^>]*>' dist/index.html
test -f dist/_headers && echo OK
# Em preview/produção:
curl -sI https://<host>/<arquivo>_bg.wasm | grep -iE 'content-type|cache-control'
```

## Rollback

Remover a linha `copy-file` do `_headers`; os hints do Trunk não são configuráveis individualmente (são parte do loader).
