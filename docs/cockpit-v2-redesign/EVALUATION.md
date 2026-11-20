# Evaluation — Zylos Cockpit v2.0 Redesign

**Versão:** 2.0.0
**Data:** 2026-11-18
**Status:** Métricas e Benchmarks Definidos
**Conceito Manifesto:** #22 (Evaluation/Benchmark de agentes)
**Depende de:** [PRD.md](./PRD.md), [TESTING.md](./TESTING.md)

---

## 1. Métricas de UX

| Métrica | Baseline (v0.1.0) | Target (v2.0) | Método | Frequência |
|---------|-------------------|---------------|--------|------------|
| Apple Design Principles Score | 2.2/10 | ≥8.0/10 | Auditoria manual 17 princípios | Cada release |
| Lighthouse Mobile Performance | ~45 | ≥90 | Lighthouse CI | Todo PR |
| Lighthouse Accessibility | ~60 | ≥90 | Lighthouse CI + axe-core | Todo PR |
| LCP (Largest Contentful Paint) | ~4s | desktop <2s; mobile simulado <=4s (a meta <2s no mobile não é alcançável: `PERFORMANCE_WASM.md`) | Lighthouse (desktop e mobile Slow 4G + CPU 4x) | Todo PR |
| TBT (Total Blocking Time) | ~800ms | <300ms | Lighthouse | Todo PR |
| CLS (Cumulative Layout Shift) | ~0.25 | <0.1 | Lighthouse | Todo PR |
| Tempo médio aprovação HITL | 12s | <3s | Telemetria custom | Contínuo |
| Taxa abandono aprovação | ~35% | <10% | Analytics sessão | Semanal |
| FPS animações spring | N/A | ≥55fps | Chrome DevTools Perf | Cada fase |
| Bundle WASM gzipped | ~1.2MB | <800KB | trunk build --profile wasm-release | Todo PR |

---

## 2. Benchmarks de Performance WASM

### 2.1 Bundle Size Budget

| Componente | Budget Gzipped | Medição |
|-----------|----------------|---------|
| Core WASM | <400KB | wasm-opt -Oz |
| CSS total | <50KB | gzip |
| Assets (fonts, icons) | <200KB | brotli |
| Spring solver (se custom) | <5KB | wasm-opt |
| Ammonia sanitizer | <15KB | wasm-opt |
| **Total** | **<800KB** | CI gate fail se exceder |

### 2.2 Runtime Benchmarks

| Operação | Target | Método |
|----------|--------|--------|
| Command palette open | <100ms | Performance.now() keydown→visible |
| Fuzzy search 500 items | <50ms | Benchmark.js WASM |
| AgentCard hover animation frame | <16ms | Chrome Perf monitor |
| SSE event → UI update | <100ms | Timestamp diff |
| VNC frame decode + render | <33ms (30fps) | requestAnimationFrame timing |
| Inline approval optimistic update | <50ms | User perception threshold |
| Sidebar collapse/expand | 300-400ms | Spring response time |
| Page transition | 300-400ms | Spring entrance/exit |

### 2.3 Memory Benchmarks

| Cenário | Target | Método |
|---------|--------|--------|
| Idle memory footprint | <50MB | Chrome Task Manager |
| Após 1h sessão ativa | <100MB | Heap snapshot diff |
| SSE buffer (100 eventos) | <5MB | Object size estimation |
| VNC canvas (1080p) | <10MB | Canvas allocation tracking |
| GC pause max | <10ms | Chrome Perf GC events |

---

## 3. Dataset de Teste Visual

### 3.1 Screenshots de Referência (9 Repos Pares)

| Repo | Tela Referência | Elemento Chave a Comparar | Status |
|------|----------------|---------------------------|--------|
| Linear | Dashboard overview | Card density, typography hierarchy | Capturado |
| Vercel Dashboard | Deployment list | Status badges, filter pills | Capturado |
| Arc Browser | Sidebar grouped nav | Collapsible groups, translucency | Capturado |
| OpenDuet | Inline approval card | Card layout, action buttons | Capturado |
| OpenDots | PageReviewCard | Receipt persistence, hash display | Capturado |
| OpenMuse | VNC fallback | Screenshot placeholder, controls | Capturado |
| OpenMausBot | VNC stream live | Canvas rendering, latency indicator | Capturado |
| ECC CLI | Toast notifications | Stack behavior, variants | Capturado |
| Ruflo | Bulk approval panel | Multi-select, sidebar integration | Capturado |

### 3.2 Visual Regression Baselines

| Viewport | Telas Cobertas | Threshold Diff |
|----------|---------------|----------------|
| 375px (mobile) | Todas 11 telas | 0.01% pixel change |
| 768px (tablet) | Todas 11 telas | 0.01% pixel change |
| 1440px (desktop) | Todas 11 telas | 0.01% pixel change |

### 3.3 Apple Design Principles Audit Rubric

| # | Princípio | Peso | Critério v2.0 | Score Atual |
|---|-----------|------|---------------|-------------|
| 1 | Springs/Animações fluidas | 10% | Physics-based, interruptíveis, velocity handoff | 0/10 |
| 2 | Materiais translúcidos | 8% | backdrop-filter blur+saturate em header/sidebar | 1/10 |
| 3 | Manipulação direta | 6% | Drag, swipe, gestures (futuro) | 1/10 |
| 4 | Feedback imediato | 10% | :active states, optimistic updates <50ms | 3/10 |
| 5 | Hierarquia visual clara | 8% | Typography scale, spacing consistency | 4/10 |
| 6 | Densidade informacional | 6% | Cards compactos sem perda de clareza | 3/10 |
| 7 | Sidebar organizada | 6% | Grupos colapsáveis, badges, search | 3/10 |
| 8 | Command palette | 6% | Cmd+K omnibar com fuzzy search | 0/10 |
| 9 | Estados completos | 10% | 6 estados por componente documentado | 2/10 |
| 10 | Reduced motion | 4% | prefers-reduced-motion respeitado | 7/10 |
| 11 | Reduced transparency | 4% | prefers-reduced-transparency respeitado | 0/10 |
| 12 | Contrast preferences | 4% | prefers-contrast suportado | 0/10 |
| 13 | Acessibilidade keyboard | 6% | Tab order lógico, focus visível | 3/10 |
| 14 | Screen reader support | 4% | ARIA labels, live regions | 2/10 |
| 15 | Consistência visual | 6% | Single design system, zero conflitos | 2/10 |
| 16 | Error recovery graceful | 4% | Error boundaries, retry, fallbacks | 3/10 |
| 17 | Performance percebida | 4% | Skeleton loading, progressive enhancement | 3/10 |
| **Total** | | **100%** | **Target: ≥8.0/10** | **2.2/10** |

---

## 4. Avaliação Comparativa vs Pares

| Capacidade | Zylos v2.0 Target | Linear | Vercel | OpenDuet | OpenMausBot |
|-----------|-------------------|--------|--------|----------|-------------|
| Inline approvals | ✅ ADR-001 | ❌ | ❌ | ✅ | ❌ |
| VNC stream real | ✅ ADR-003 | ❌ | ❌ | ❌ | ✅ |
| Spring animations | ✅ ADR-007 | ✅ | ✅ | ❌ | ❌ |
| Translucent materials | ✅ ADR-010 | ✅ | ✅ | ❌ | ❌ |
| Command palette | ✅ ADR-009 | ✅ | ✅ | ❌ | ❌ |
| 5-level proofs | ✅ Único | ❌ | ❌ | ❌ | ❌ |
| Triple sandbox | ✅ Único | ❌ | ❌ | ❌ | ❌ |
| AIOps sentinel | ✅ Único | ❌ | ❌ | ❌ | ❌ |
| 338+ skills registry | ✅ Único | ❌ | ❌ | ❌ | ❌ |

**Conclusão:** Zylos v2.0 target = frontend equivalente aos melhores dashboards (Linear/Vercel) + capacidades backend únicas que nenhum par possui.

---

## 5. Processo de Avaliação Contínua

### 5.1 Por PR
- Lighthouse CI automático (fail se <90 qualquer categoria)
- Bundle size check (fail se >800KB gzipped)
- Visual regression diff (review manual se >threshold)
- Unit tests wasm-bindgen-test (fail se qualquer vermelho)

### 5.2 Por Fase
- Apple Design audit parcial (princípios relevantes à fase)
- Performance benchmark das features da fase
- Accessibility scan axe-core
- Manual QA dos 6 estados obrigatórios

### 5.3 Por Release
- Apple Design audit completo (17 princípios)
- Comparação visual contra screenshots dos 9 pares
- User testing com persona "Ana SRE" (mínimo 3 sessões)
- Telemetria de métricas reais (HITL time, abandonment rate)

---

## 6. Compliance com Manifesto

| Requisito | Status |
|-----------|--------|
| Métricas de UX mensuráveis | ✅ Seção 1 |
| Benchmarks performance WASM | ✅ Seção 2 |
| Dataset teste visual (9 pares) | ✅ Seção 3 |
| Avaliação comparativa | ✅ Seção 4 |
| Processo contínuo definido | ✅ Seção 5 |
| Cross-link com TESTING.md | ✅ Métricas alinhadas |

---

## 7. Changelog

| Versão | Data | Mudança |
|--------|------|---------|
| 2.0.0 | 2026-11-18 | Initial v2.0 evaluation plan; UX metrics; WASM benchmarks; visual dataset; Apple audit rubric |