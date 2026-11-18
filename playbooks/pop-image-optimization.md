# POP-Image-Optimization — Ícones, WebP e Lazy Loading do Cockpit

**Versão:** 1.0.0
**Data:** 2026-11-17
**Fase:** 11.5 (Performance & Lighthouse) — T11.5.4
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.5

---

## Propósito

Corrigir os assets de imagem quebrados do baseline e padronizar formatos leves.

## Diagnóstico (baseline T11.5.1)

- `manifest.json` apontava para `/icon-192.png` e `/icon-512.png`, **inexistentes** no repositório.
- `/favicon.ico`, `/manifest.json` e `/sw.js` retornavam o fallback SPA (`text/html`) porque o `[[copy]]` do `Trunk.toml` não é suportado pelo Trunk 0.22 → erros "Manifest: Syntax error" e "unsupported MIME type ('text/html')" no console.
- O cockpit **não usa `<img>`** em nenhum componente (`grep -rn "<img" apps/zylos-cockpit/src` vazio): avatares são iniciais/emoji em CSS. Não há imagens de conteúdo para converter ou fazer lazy-load hoje.

## Procedimento aplicado

1. Ícones em `apps/zylos-cockpit/icons/`:
   - `icon.svg` (vetorial, favicon moderno, 202 B)
   - `icon-192.png` / `icon-512.png` (PWA / apple-touch-icon)
   - `icon-192.webp` / `icon-512.webp` (WebP lossless)
   - `favicon-32.png`
   Gerados com ffmpeg (sem dependência nova):
   ```bash
   ffmpeg -f lavfi -i "color=c=0x0f1117:s=512x512" -vf "drawbox=..." -frames:v 1 icon-512.png
   ffmpeg -i icon-512.png -c:v libwebp -lossless 1 icon-512.webp
   ```
2. `manifest.json` lista PNG (exigido pelo Chrome para instalabilidade), WebP e SVG.
3. `index.html` copia `icons/` com `<link data-trunk rel="copy-dir" href="icons">` e declara `<link rel="icon" type="image/svg+xml">` + PNG 32px + `apple-touch-icon`.
4. AVIF: o ffmpeg local não tem encoder AVIF (`libaom`/`libsvtav1` ausentes); para ícones de <3 KB o ganho seria irrelevante. Não aplicado.

## Regra para novas imagens (avatares, screenshots)

```html
<picture>
  <source srcset="/img/x.avif" type="image/avif">
  <source srcset="/img/x.webp" type="image/webp">
  <img src="/img/x.png" alt="Descrição" width="64" height="64" loading="lazy" decoding="async">
</picture>
```
- Sempre `width`/`height` explícitos (CLS).
- `loading="lazy"` em tudo fora do primeiro viewport; nunca no elemento LCP.
- `alt` obrigatório (vazio `alt=""` só para decorativas).

## Verificação

```bash
cd apps/zylos-cockpit && trunk build --release
ls dist/icons dist/manifest.json dist/sw.js dist/robots.txt
python -c "import json;[print(i['src']) for i in json.load(open('dist/manifest.json'))['icons']]"
```
