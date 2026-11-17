# POP-Critical-CSS-Extraction — Caminho Crítico de Renderização do Cockpit

**Versão:** 1.0.0
**Data:** 2026-11-17
**Fase:** 11.5 (Performance & Lighthouse) — T11.5.2
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.5

---

## Propósito

Eliminar recursos *render-blocking* do `apps/zylos-cockpit/index.html` para reduzir o First Contentful Paint (baseline 1.9s, meta <1.5s).

## Contexto (baseline T11.5.1)

- `design-tokens.css` (3.7 KB gz) e `style.css` (8.5 KB gz) eram `<link rel="stylesheet">` bloqueantes.
- `#app` vazio: nada era pintado até o WASM (~1.5 MB gz) baixar, compilar e montar o Leptos.
- O JS gerado pelo Trunk já é `type="module"` (adiado por padrão) — não há JS síncrono a adiar além do snippet de service worker, que roda só no evento `load`.

## Procedimento

1. **Inline das folhas de estilo pelo Trunk** — no `index.html`:
   ```html
   <link data-trunk rel="css" href="design-tokens.css" data-inline />
   <link data-trunk rel="css" href="style.css" data-inline />
   ```
   O Trunk injeta o conteúdo em `<style>` no `dist/index.html`. Custo: ~50 KB a mais no HTML não comprimido (~12 KB gz), compensado pela remoção de 2 round-trips bloqueantes.
2. **Shell crítico estático** — bloco `<style>` mínimo + `<div id="app-splash">` com o título e "Carregando…". É o primeiro conteúdo pintado (FCP) antes do WASM instanciar.
3. **Esconder o splash quando o Leptos monta**: `body:has(.cockpit-root) #app-splash { display: none; }` — sem JS adicional. O splash tem `aria-hidden="true"` para não duplicar o título para leitores de tela.
4. **Manter cores do splash idênticas aos tokens** (`#0A0E1A`, `#F9FAFB`, borda `#374151`) para CLS = 0 e sem "flash" de cor.

## Por que inline total (e não extração "critical" parcial)

Ferramentas como `critical`/`penthouse` exigem renderizar a página; para um app CSR em que todo o DOM vem do WASM, o "above the fold" real não existe no HTML. O CSS total comprimido (~12 KB) cabe na janela inicial de congestionamento TCP (~14 KB), então inline total tem o mesmo custo de rede de um CSS crítico parcial e é mais simples de manter.

## Verificação

```bash
cd apps/zylos-cockpit && trunk build --release
grep -c '<link rel="stylesheet"' dist/index.html   # esperado: 0
grep -c '<style' dist/index.html                   # esperado: >= 3
# Lighthouse: auditoria render-blocking-insight deve passar (score 1)
```

## Rollback

Remover `data-inline` dos dois `<link data-trunk rel="css">` — o Trunk volta a emitir folhas externas com hash.

## Riscos

- Mudanças de CSS invalidam o cache do HTML inteiro (HTML não tem hash). Aceitável: o HTML já é servido `no-cache` pelo Cloudflare Pages.
- `:has()` requer Chrome 105+/Safari 15.4+/Firefox 121+. Em navegadores antigos o splash permanece acima do app (degradação visual, sem perda de função).
