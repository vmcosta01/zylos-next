# RB-Accessibility-Audit — Auditoria de Acessibilidade (WCAG 2.1 AA) do Cockpit

**Versão:** 1.0.0
**Data:** 2026-11-17
**Fase:** 11.5 — T11.5.5 (auditoria) / T11.5.6 (correções)
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.5
**Relacionados:** [`pot-lighthouse-baseline.md`](../playbooks/pot-lighthouse-baseline.md), [`docs/fase11-reports/lighthouse-final.md`](../docs/fase11-reports/lighthouse-final.md)
**Público:** Dev frontend, QA
**Tempo Estimado:** 30–60 min por rodada

---

## 1. Escopo e método

| Camada | Ferramenta | Cobertura |
|---|---|---|
| Automática | Lighthouse 12 (axe-core embutido), categoria Accessibility | DOM renderizado da tela inicial (estado sem gateway = `error-banner`) |
| Estática | `grep` no código Leptos (`apps/zylos-cockpit/src`) | todos os `<input>`, `<textarea>`, `<select>`, `<button>`, `on:click` em elementos não-interativos |
| Manual | teclado (Tab/Shift+Tab/Enter/Espaço) | barra de abas, tema, retry |
| Contraste | cálculo WCAG (luminância relativa) | tokens `--accent-blue`, `--fg-*`, botões CTA |

Limite honesto: o Lighthouse só audita a tela renderizada no carregamento. As 31 abas e os formulários internos foram auditados **estaticamente** (código), não com axe em runtime — ficou registrado como pendência (§6).

## 2. Comandos

```bash
# Automático (build local servido em :8089)
npx -y lighthouse@12 http://localhost:8089/ --only-categories=accessibility \
  --chrome-flags="--headless=new" --preset=desktop --output=json --output-path=a11y.json
jq -r '.audits[] | select(.score==0) | .id' a11y.json

# Estático — campos sem rótulo acessível
grep -rn "<input\|<textarea\|<select" apps/zylos-cockpit/src | grep -v "aria-label\|id="
# Estático — clique em elemento não interativo (precisa role + tabindex + teclado)
grep -rnE "<(div|span|li)[^>]*on:click" apps/zylos-cockpit/src
```

## 3. Achados (rodada 1 — 2026-11-17)

| # | Critério WCAG | Severidade | Onde | Achado | Status |
|---|---|---|---|---|---|
| A1 | 1.4.3 Contraste (AA) | **Crítico** (axe *serious*) | `apps/zylos-cockpit/style.css` `.cta-primary, .cta-retry` | `#fff` sobre `#3B82F6` = **3.67:1** (< 4.5:1) | ✅ corrigido: fundo `#2563EB` = 5.17:1 |
| A2 | 1.4.3 Contraste | Médio | tema claro `.tab-btn.active`, `.cta-secondary` | `#3B82F6` sobre `#FFFFFF` = 3.68:1 | ✅ corrigido: `#1D4ED8` = 6.70:1 |
| A3 | 1.3.1 / 4.1.2 Nome acessível | Alto | `components/auth_login.rs` | 3 `<label>` sem `for` → inputs sem nome | ✅ `for`/`id` (`auth-user-id`, `auth-tenant`, `auth-name`) |
| A4 | 4.1.2 Nome acessível | Alto | `components/lazy_roster.rs` busca | só `placeholder` | ✅ `aria-label="Buscar agentes"` |
| A5 | 4.1.2 Nome acessível | Alto | `components/channel_panel.rs` input | só `placeholder` | ✅ `aria-label="Mensagem do canal"` |
| A6 | 4.1.2 Nome acessível | Alto | `components/chat_panel.rs` textarea | só `placeholder` | ✅ `aria-label="Mensagem para o bot"` |
| A7 | 1.3.1 / 4.1.2 Estado | Médio | `app.rs` `TabButton` | aba ativa só indicada por cor/borda | ✅ `aria-current="page"` na aba ativa |
| A8 | 2.4.1 / 1.3.1 Landmark | Baixo | `app.rs` `<nav class="cockpit-tabs">` | `nav` sem rótulo | ✅ `aria-label="Abas do cockpit"` |
| A9 | 4.1.3 Mensagens de status | Médio | `app.rs` `error-banner` | erro de conexão não anunciado | ✅ `role="alert"` |
| A10 | 2.4.7 Foco visível | — | `style.css:150`, `design-tokens.css` `:focus-visible` | outline 2px já presente | ✅ conforme (sem mudança) |
| A11 | 2.3.3 Movimento | — | `design-tokens.css` | `prefers-reduced-motion` já respeitado | ✅ conforme |
| A12 | 2.1.1 Teclado | — | todo `src/` | nenhum `on:click` em `div/span/li` (grep vazio) | ✅ conforme |
| A13 | 3.1.1 Idioma | — | `index.html` | `lang="pt-BR"` | ✅ conforme |
| A14 | 1.1.1 Alternativas | — | `src/` | nenhum `<img>`; ícones são texto/emoji | ✅ conforme (regra em `pop-image-optimization.md`) |
| A15 | 2.4.3 Ordem de foco / gestão de foco | Médio | troca de abas | ao trocar de aba o foco permanece no botão (aceitável, padrão de tabs simples); não há modais com *focus trap* na tela inicial | ⚠️ pendente revisão manual por aba |
| A16 | 4.1.2 Padrão ARIA de tabs | Baixo | barra de abas | não usa `role=tablist/tab` + setas; usa botões + `aria-current` (válido, menos idiomático) | ⚠️ aceito; evolução opcional |

## 4. Cálculo de contraste usado

`L = 0.2126 R + 0.7152 G + 0.0722 B` (canais linearizados), `ratio = (L1 + 0.05) / (L2 + 0.05)`.

| Par | Ratio |
|---|---|
| `#FFFFFF` / `#3B82F6` | 3.68 (Lighthouse reportou 3.67) |
| `#FFFFFF` / `#2563EB` | 5.17 |
| `#1D4ED8` / `#FFFFFF` | 6.70 |
| `#9CA3AF` (`--fg-secondary`) / `#0A0E1A` | 7.6 |

## 5. Critério de aceite

- Lighthouse Accessibility ≥ 90 **e** zero auditorias axe com impacto *critical/serious* reprovadas.
- Ver números finais em `docs/fase11-reports/lighthouse-final.md`.

## 6. Pendências

- Rodar axe em runtime em cada uma das 31 abas (requer gateway ativo; combinar com `rb-production-smoke-test.md`).
- Teste com leitor de tela real (NVDA/VoiceOver) — não executado nesta rodada.
- Revisar gestão de foco nos painéis com diálogos (HITL approval, skill editor).
