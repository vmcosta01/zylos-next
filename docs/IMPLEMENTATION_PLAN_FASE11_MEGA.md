# Mega-Plano de Implementação — Fase 11 Unificada (A→B→C→D)

**Versão:** 1.0.0
**Data:** 2026-11-12
**Base:** Auditoria consolidada Fases 08/09/10 + Opções A/B/C/D
**Pré-requisito:** Fases 08–10 completas (113/113 tasks, clippy verde, ≥82 docs operacionais)
**Skill:** `implementation-plan-author` v1.0.0

---

## Visão Geral

Este plano transforma o cockpit Zylos de **sistema funcional com gates verdes** em **produto de produção monitorado, integrado e seguro**. Integra 4 estratégias em sequência lógica:

- **Opção A (Fases 11.1–11.3):** Validação de Build Release, E2E tests reais, CI/CD pipeline
- **Opção B (Fases 11.4–11.6):** Deploy em Produção, monitoring, smoke tests
- **Opção C (Fases 11.7–11.10):** AIOps sentinel, integrações Slack/Notion/Linear, CRDT multi-user avançado, analytics dashboard
- **Opção D (Fase 11.11):** Auditoria Final de Segurança (pentest, JWT review, egress validation, threat modeling)

**Duração total estimada:** 14–18 semanas
**Stack:** Rust 1.82+, Axum 0.7, Leptos 0.7, SQLite WAL, wasm-bindgen, Yjs/CRDT, GitHub Actions, Cloudflare Pages, Sentry, Axiom

---

## Compliance Manifesto v3.1 (10 Camadas)

| # | Camada | Conformidade neste Plano |
|---|--------|--------------------------|
| 1 | Conceitos Canônicos (22 regras) | ✅ Vertical slices por fase, gates bloqueantes, atomicidade T11.X.Y, TDD em critérios de aceitação |
| 2 | Princípios Anti-Slop (7 filtros) | ✅ Critérios mensuráveis (<50ms, ≥95% coverage), zero adjetivos subjetivos, Out of Scope explícito por fase |
| 3 | Loop Profissional | ✅ READ→UNDERSTAND→PLAN→IMPLEMENT→TEST→REVIEW→FIX→COMMIT→UPDATE DOCS em cada task |
| 4 | Penalidade | ✅ Gate vermelho = trabalho inválido, não avança à próxima fase |
| 5 | Arquivos Obrigatórios (15 docs) | ✅ Todos mapeados no Apêndice A por fase (PRD, ARCHITECTURE, DESIGN_SYSTEM, AGENTS, RULES, APP_FLOW, IMPLEMENTATION_PLAN, TASKS, DECISIONS, MEMORY, TESTING, SECURITY, EVALUATION, HANDOFF, PROJECT_STRUCTURE) |
| 6 | Templates de Prompts (6 templates) | ✅ Referenciados via skill `review-prompts` em gates de revisão |
| 7 | Matriz de Rastreabilidade | ✅ Cada recomendação da auditoria mapeada para fase/task específica |
| 8 | Integração com Harness | ✅ Gates automatizados via bash/cargo/wasm-pack, CI/CD em Fase 11.3 |
| 9 | Skills Hermes Documentais | ✅ Mapeamento completo no Apêndice C (playbook-author, pop-author, runbook-builder, soar-orchestration, handoff-author, agents-rules-author, etc.) |
| 10 | Protocolo de Evolução + Lições | ✅ Feedback loops em T11.10.3 (analytics auto-improvement) e T11.11.4 (threat model update) |

---

## Fase 11.1: Validação de Build Release

**Objetivo:** Garantir que `trunk build --release` completa sem timeout e produz WASM <5MB gzipped.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.1.1 | Configurar profile `release-lto` com thin LTO + opt-level z + strip | — | Baixo | DECISIONS_FASE11.md (ADR-011) |
| 11.1.2 | Auditar dependências pesadas via `cargo tree --edges features` e desativar features desnecessárias | 11.1.1 | Médio | pot-wasm-dependency-audit.md |
| 11.1.3 | Integrar `wasm-opt -Oz --strip-debug` como pós-build step no Trunk.toml | 11.1.1 | Baixo | pop-wasm-post-build-optimization.md |
| 11.1.4 | Executar `trunk build --release` com profile otimizado e validar tamanho <5MB gzipped | 11.1.2, 11.1.3 | Baixo | TRACKRECORD_FASE11.md |
| 11.1.5 | Code splitting via `leptos::lazy` para componentes pesados (>200KB) | 11.1.4 | Alto | pb-wasm-code-splitting.md |
| 11.1.6 | Validar build release final com métricas: tamanho bruto, gzipped, brotli, initial load | 11.1.5 | Baixo | pot-wasm-size-benchmark.md |

### Entregável Verificável
- `trunk build --release` completa em <5min
- WASM gzipped <5MB
- Brotli <4MB
- Initial load (critical path) <2MB

### Critérios de Aceitação
- [ ] `[profile.release-lto]` configurado em Cargo.toml com `lto = "thin"`, `opt-level = "z"`, `strip = true`
- [ ] `wasm-opt` integrado no Trunk.toml como post-build hook
- [ ] `trunk build --release` exit code 0 em <5min
- [ ] `gzip -c dist/*_bg.wasm | wc -c` < 5242880 (5MB)
- [ ] Code splitting reduz initial load em ≥30% vs monolithic bundle
- [ ] Métricas documentadas em pot-wasm-size-benchmark.md

### Out of Scope
- Source maps para production debugging (adicionar na Fase 11.4)
- CDN configuration (Fase 11.4)
- Service Worker / PWA cache-first (Fase futura)

### Gate de Verificação
```bash
trunk build --release && SIZE=$(gzip -c dist/*_bg.wasm | wc -c) && test "$SIZE" -lt 5242880 && echo "GATE 11.1 VERDE: ${SIZE} bytes gzipped"
```

---

## Fase 11.2: Testes E2E Reais em Browser Headless

**Objetivo:** Configurar e executar suite completa de testes E2E com `wasm-pack test --headless --chrome`, atingindo ≥80% de cobertura de componentes.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.2.1 | Instalar Chrome 120+ e chromedriver compatível no ambiente de desenvolvimento | — | Baixo | pb-e2e-test-setup.md (já criado, validar) |
| 11.2.2 | Criar testes E2E para componentes críticos: ChannelPanel, SkillEditorPanel, SwarmTopologyPanel, AuthLogin | 11.2.1 | Alto | tests/e2e_channels.rs, tests/e2e_skill_editor.rs, tests/e2e_topology.rs, tests/e2e_auth.rs |
| 11.2.3 | Criar testes E2E para workflows completos: login→create channel→send message, create skill→save→validate | 11.2.2 | Alto | tests/e2e_workflows.rs |
| 11.2.4 | Adicionar `#[wasm_bindgen_test(timeout = 300)]` em todos os testes lentos | 11.2.2 | Baixo | — |
| 11.2.5 | Executar suite completa e documentar pass rate, flaky rate, tempo médio | 11.2.3 | Médio | TRACKRECORD_FASE11.md |
| 11.2.6 | Criar testes de regressão para bugs conhecidos (uuid js feature, borrow checker webrtc_relay) | 11.2.2 | Médio | tests/e2e_regression.rs |

### Entregável Verificável
- Suite E2E com ≥30 testes passando
- Pass rate 100% (zero falhas)
- Flaky rate <2%
- Tempo total <5min

### Critérios de Aceitação
- [ ] `wasm-pack test --headless --chrome` exit code 0
- [ ] ≥30 testes E2E cobrindo ≥80% dos componentes cockpit
- [ ] Zero falhas em 3 execuções consecutivas (flaky rate = 0%)
- [ ] Tempo total de execução <5min
- [ ] Testes de regressão para uuid/borrow checker passam
- [ ] Resultados documentados em TRACKRECORD_FASE11.md

### Out of Scope
- Visual regression testing (screenshot comparison)
- Performance benchmarks via E2E (usar Lighthouse na Fase 11.4)
- Cross-browser testing (Firefox/Safari)

### Gate de Verificação
```bash
wasm-pack test --headless --chrome && echo "GATE 11.2 VERDE"
```

---

## Fase 11.3: CI/CD Pipeline com GitHub Actions

**Objetivo:** Automatizar gates de qualidade, build, testes e deploy preview em todo PR e push para main.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.3.1 | Criar workflow `.github/workflows/ci.yml` com jobs: clippy, test, build-wasm, e2e | 11.2 | Médio | .github/workflows/ci.yml |
| 11.3.2 | Configurar cache de cargo registry e target directory para acelerar builds | 11.3.1 | Baixo | .github/workflows/ci.yml |
| 11.3.3 | Adicionar job de size check que falha se WASM gzipped >5MB | 11.3.1 | Baixo | .github/workflows/ci.yml |
| 11.3.4 | Criar workflow `.github/workflows/deploy-preview.yml` para deploy em Cloudflare Pages por branch | 11.3.1 | Médio | .github/workflows/deploy-preview.yml, pb-cloudflare-pages-setup.md |
| 11.3.5 | Criar workflow `.github/workflows/deploy-prod.yml` para deploy em produção via tag | 11.3.4 | Médio | .github/workflows/deploy-prod.yml, rb-production-deploy.md |
| 11.3.6 | Configurar secrets no GitHub (CLOUDFLARE_API_TOKEN, JWT_SECRET, SENTRY_DSN) | 11.3.4, 11.3.5 | Baixo | SECURITY.md (atualizar) |
| 11.3.7 | Adicionar badge de status no README.md do cockpit e gateway | 11.3.1 | Baixo | apps/zylos-cockpit/README.md, apps/zylos-gateway/README.md |
| 11.3.8 | Testar pipeline completo: abrir PR → verificar CI verde → merge → verificar deploy preview | 11.3.5 | Médio | TRACKRECORD_FASE11.md |

### Entregável Verificável
- CI pipeline executa em <10min por PR
- Deploy preview automático em toda branch
- Deploy produção via tag
- Badges de status visíveis nos READMEs

### Critérios de Aceitação
- [ ] `.github/workflows/ci.yml` passa em PR de teste
- [ ] Jobs clippy, test, build-wasm, e2e executam em paralelo
- [ ] Size check job falha se WASM >5MB gzipped
- [ ] Deploy preview URL gerada automaticamente por branch
- [ ] Deploy produção triggerado por tag `v*`
- [ ] Secrets configurados e não expostos em logs
- [ ] Badges renderizam corretamente nos READMEs
- [ ] Pipeline end-to-end testado com PR real

### Out of Scope
- Self-hosted runners (usar GitHub-hosted)
- Matrix testing (múltiplos OS/browser)
- Automatic changelog generation

### Gate de Verificação
```bash
# Validar localmente antes de push
act -j ci # Requer nektos/act para simular GitHub Actions localmente
# OU validar via PR real no GitHub
```

---

## Fase 11.4: Deploy em Produção + Monitoring

**Objetivo:** Deploy do cockpit e gateway em produção com monitoring completo (Sentry, Axiom, health checks).

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.4.1 | Configurar Cloudflare Pages para cockpit estático (custom domain, SSL, caching) | 11.3 | Médio | pb-cloudflare-production-setup.md |
| 11.4.2 | Provisionar VPS/Kubernetes para gateway (2 vCPU, 4GB RAM mínimo) | — | Alto | pb-gateway-infrastructure-setup.md |
| 11.4.3 | Configurar reverse proxy (nginx/Caddy) com TLS termination e WebSocket support | 11.4.2 | Médio | rb-gateway-reverse-proxy.md |
| 11.4.4 | Integrar Sentry no cockpit (WASM panic hook) e gateway (tracing-sentry) | 11.4.1, 11.4.2 | Médio | pb-sentry-integration.md |
| 11.4.5 | Integrar Axiom para logs estruturados do gateway (axiom-rs ou HTTP API) | 11.4.2 | Médio | pb-axiom-logging-setup.md |
| 11.4.6 | Configurar health check endpoints: `/api/v1/health`, `/api/v1/cockpit/health` | 11.4.2 | Baixo | pop-health-check-validation.md |
| 11.4.7 | Configurar alertas automáticos: error rate >1%, latency p95 >2s, health check down | 11.4.4, 11.4.5 | Médio | soar-production-alerts.md |
| 11.4.8 | Gerar source maps para WASM e upload para Sentry | 11.4.4 | Baixo | pop-wasm-source-maps.md |
| 11.4.9 | Smoke test em produção: validar todas as 31 tabs, login flow, WebSocket relay | 11.4.6 | Médio | rb-production-smoke-test.md |
| 11.4.10 | Documentar runbook de incident response para produção | 11.4.7 | Médio | rb-production-incident-response.md |

### Entregável Verificável
- Cockpit acessível via custom domain com SSL
- Gateway rodando em produção com TLS
- Sentry capturando erros do cockpit e gateway
- Axiom recebendo logs estruturados
- Health checks verdes
- Alertas configurados e testados

### Critérios de Aceitação
- [ ] Cockpit carrega em <3s (Lighthouse performance ≥90)
- [ ] Gateway responde health check em <100ms
- [ ] Sentry captura erro de teste intencional (cockpit + gateway)
- [ ] Axiom recebe logs de request/response do gateway
- [ ] Alerta dispara quando health check falha (testar com downtime simulado)
- [ ] Source maps permitem stack trace legível no Sentry
- [ ] Smoke test valida todas as 31 tabs sem erro
- [ ] Runbook de incident response documentado e testado

### Out of Scope
- Multi-region deployment
- Auto-scaling
- Blue/green deploy strategy
- Database backup automation (SQLite WAL já é seguro)

### Gate de Verificação
```bash
curl -f https://prod-url/api/v1/health && curl -f https://prod-url/api/v1/cockpit/health && echo "GATE 11.4 VERDE"
```

---

## Fase 11.5: Performance Optimization & Lighthouse Audit

**Objetivo:** Atingir Lighthouse performance ≥90, accessibility ≥90, best practices ≥95 em produção.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.5.1 | Executar Lighthouse audit em produção e documentar baseline | 11.4 | Baixo | pot-lighthouse-baseline.md |
| 11.5.2 | Otimizar critical rendering path: inline CSS crítico, defer non-critical JS | 11.5.1 | Médio | pop-critical-css-extraction.md |
| 11.5.3 | Adicionar resource hints: preload WASM, prefetch fonts, preconnect CDN | 11.5.1 | Baixo | pop-resource-hints.md |
| 11.5.4 | Implementar image optimization: WebP/AVIF para avatars/icons, lazy loading | 11.5.1 | Médio | pop-image-optimization.md |
| 11.5.5 | Accessibility audit completo (WCAG AA): focus management, aria-labels, keyboard nav, color contrast | 11.5.1 | Alto | rb-accessibility-audit.md |
| 11.5.6 | Corrigir issues de accessibility identificadas (target: score ≥90) | 11.5.5 | Alto | TRACKRECORD_FASE11.md |
| 11.5.7 | Re-executar Lighthouse e validar targets atingidos | 11.5.6 | Baixo | pot-lighthouse-final-report.md |

### Entregável Verificável
- Lighthouse Performance ≥90
- Lighthouse Accessibility ≥90
- Lighthouse Best Practices ≥95
- WCAG AA compliance report

### Critérios de Aceitação
- [ ] Lighthouse Performance score ≥90 em produção
- [ ] Lighthouse Accessibility score ≥90
- [ ] Lighthouse Best Practices score ≥95
- [ ] Zero violations WCAG AA críticas
- [ ] First Contentful Paint <1.5s
- [ ] Largest Contentful Paint <2.5s
- [ ] Cumulative Layout Shift <0.1
- [ ] Relatório final documentado em pot-lighthouse-final-report.md

### Out of Scope
- SEO optimization (cockpit é ferramenta interna)
- PWA/offline support
- Internationalization além de PT-BR/EN-US

### Gate de Verificação
```bash
npx lighthouse https://prod-url --output=json --output-path=lighthouse-report.json && jq '.categories.performance.score >= 0.9 and .categories.accessibility.score >= 0.9' lighthouse-report.json | grep true && echo "GATE 11.5 VERDE"
```

---

## Fase 11.6: Documentação de Usuário Final & Onboarding

**Objetivo:** Criar documentação completa para operadores, devs e admins, incluindo onboarding guiado.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.6.1 | Atualizar USER_GUIDE.md com screenshots reais de produção | 11.4 | Alto | docs/USER_GUIDE.md (atualizar) |
| 11.6.2 | Criar guia de onboarding para novos operadores (journey de 4 semanas) | 11.6.1 | Médio | docs/ONBOARDING_OPERATORS.md |
| 11.6.3 | Criar guia de onboarding para novos desenvolvedores (setup local, first PR) | 11.6.1 | Médio | docs/ONBOARDING_DEVS.md |
| 11.6.4 | Criar FAQ atualizado com top 30 questões (baseado em suporte real) | 11.6.1 | Médio | docs/FAQ.md |
| 11.6.5 | Criar vídeo tutorial de 5min demonstrando workflows principais | 11.6.1 | Alto | docs/TUTORIAL_VIDEO.md (script + link) |
| 11.6.6 | Validar documentação com 3 usuários reais (operador, dev, admin) e coletar feedback | 11.6.4 | Médio | docs/DOC_VALIDATION_REPORT.md |

### Entregável Verificável
- USER_GUIDE.md com screenshots de produção
- 2 guias de onboarding (operadores + devs)
- FAQ com 30 questões
- Script de vídeo tutorial
- Relatório de validação com feedback de 3 usuários

### Critérios de Aceitação
- [ ] USER_GUIDE.md cobre todas as 31 tabs com screenshots
- [ ] Onboarding operadores: novo usuário consegue executar workflow básico em <1h
- [ ] Onboarding devs: novo dev consegue setup local + first PR em <4h
- [ ] FAQ responde ≥80% das questões comuns sem necessidade de suporte
- [ ] Vídeo tutorial script aprovado por Tech Lead
- [ ] Validação com 3 usuários: satisfaction score ≥4/5

### Out of Scope
- Tradução para outros idiomas
- Interactive walkthrough (product tour)
- Chatbot de suporte

### Gate de Verificação
```bash
# Validação manual com 3 usuários
# Score médio ≥4/5 em survey de satisfação
echo "GATE 11.6 VERDE (validação manual)"
```

---

## Fase 11.7: AIOps Sentinel com Alertas Automáticos

**Objetivo:** Implementar sentinelas de monitoramento contínuo que detectam anomalias e disparam alertas/remediação automática.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.7.1 | Criar crate `zylos-sentinel` com anomaly detection engine (baseline + deviation) | 11.4 | Alto | crates/zylos-sentinel/README.md |
| 11.7.2 | Implementar métricas coletadas: CPU, memory, latency, error rate, queue depth | 11.7.1 | Médio | crates/zylos-sentinel/src/metrics.rs |
| 11.7.3 | Implementar alert rules engine com DSL YAML (threshold, trend, composite) | 11.7.1 | Alto | crates/zylos-sentinel/src/rules.rs, pb-sentinel-rules-authoring.md |
| 11.7.4 | Integrar com Axiom para ingestão de métricas em tempo real | 11.7.2, 11.4.5 | Médio | pop-sentinel-axiom-integration.md |
| 11.7.5 | Implementar auto-remediation gated: restart service, scale replicas, notify on-call | 11.7.3 | Alto | soar-sentinel-auto-remediation.md |
| 11.7.6 | Criar dashboard no cockpit tab Sentinel com gráficos de anomalias e alertas | 11.7.4 | Médio | apps/zylos-cockpit/src/components/sentinel_panel.rs (atualizar) |
| 11.7.7 | Testar sentinel com anomalias sintéticas (CPU spike, error burst, latency degradation) | 11.7.5 | Médio | tests/sentinel_anomaly_detection.rs |
| 11.7.8 | Documentar runbook de tuning de thresholds e false positive reduction | 11.7.7 | Médio | rb-sentinel-threshold-tuning.md |

### Entregável Verificável
- Crate `zylos-sentinel` com anomaly detection funcional
- Alert rules engine com DSL YAML
- Auto-remediation gated funcionando
- Dashboard Sentinel atualizado no cockpit
- ≥10 testes de detecção de anomalias passando

### Critérios de Aceitação
- [ ] Sentinel detecta CPU spike >80% em <30s
- [ ] Sentinel detecta error rate >1% em <1min
- [ ] Alert rules carregadas de YAML e hot-reload funcionam
- [ ] Auto-remediation executa ação correta (restart/scale/notify) apenas quando gate aprova
- [ ] Dashboard Sentinel exibe anomalias em tempo real (<5s delay)
- [ ] False positive rate <5% em teste de 24h
- [ ] ≥10 testes unitários/integração passando
- [ ] Runbook de tuning documentado

### Out of Scope
- ML-based anomaly detection (usar statistical methods primeiro)
- Predictive alerting (trend forecasting)
- Multi-tenant sentinel isolation

### Gate de Verificação
```bash
cargo test -p zylos-sentinel && cargo check -p zylos-cockpit && echo "GATE 11.7 VERDE"
```

---

## Fase 11.8: Integrações Slack/Notion/Linear

**Objetivo:** Conectar cockpit a ferramentas externas para notificações, documentação sync e ticket tracking.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.8.1 | Criar crate `zylos-integrations` com adapter trait e HTTP client base | — | Médio | crates/zylos-integrations/README.md |
| 11.8.2 | Implementar Slack adapter: webhook notifications, interactive buttons, thread replies | 11.8.1 | Alto | crates/zylos-integrations/src/slack.rs, pb-slack-integration-setup.md |
| 11.8.3 | Implementar Notion adapter: page create/update, database query, block append | 11.8.1 | Alto | crates/zylos-integrations/src/notion.rs, pb-notion-integration-setup.md |
| 11.8.4 | Implementar Linear adapter: issue create/update, comment, status sync | 11.8.1 | Alto | crates/zylos-integrations/src/linear.rs, pb-linear-integration-setup.md |
| 11.8.5 | Criar notification router no gateway que dispatch para adapters baseado em rules | 11.8.2, 11.8.3, 11.8.4 | Médio | apps/zylos-gateway/src/notification_router.rs |
| 11.8.6 | Adicionar UI no cockpit para configurar integrações (OAuth flow, webhook URLs, channels) | 11.8.5 | Alto | apps/zylos-cockpit/src/components/integrations_panel.rs |
| 11.8.7 | Testar integrações com sandbox/staging environments de cada plataforma | 11.8.6 | Médio | tests/integration_slack.rs, tests/integration_notion.rs, tests/integration_linear.rs |
| 11.8.8 | Documentar SOAR workflow para incident→Slack notify→Linear ticket→Notion doc | 11.8.5 | Médio | soar-incident-notification-workflow.md |

### Entregável Verificável
- Crate `zylos-integrations` com 3 adapters funcionais
- Notification router no gateway
- UI de configuração no cockpit
- ≥15 testes de integração passando
- SOAR workflow documentado

### Critérios de Aceitação
- [ ] Slack notification enviada em <2s após trigger
- [ ] Notion page criada/atualizada com conteúdo correto
- [ ] Linear issue criada com labels/priority corretos
- [ ] Notification router dispatch para adapter correto baseado em rules
- [ ] UI de configuração permite OAuth/webhook setup sem código
- [ ] ≥5 testes por adapter passando (unit + integration)
- [ ] SOAR workflow executa end-to-end em teste
- [ ] Rate limits respeitados (Slack 1msg/s, Notion 3req/s, Linear 1500req/h)

### Out of Scope
- Bidirectional sync (apenas push notifications)
- Custom field mapping (usar defaults)
- Multi-workspace support

### Gate de Verificação
```bash
cargo test -p zylos-integrations && cargo check -p zylos-gateway && cargo check -p zylos-cockpit && echo "GATE 11.8 VERDE"
```

---

## Fase 11.9: CRDT Multi-User Avançado

**Objetivo:** Implementar colaboração em tempo real verdadeira com Yjs/CRDT, presence awareness e conflict resolution visual.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.9.1 | Avaliar CRDT libraries: Yjs vs Automerge vs Diamond-types (benchmark latência, size, DX) | — | Médio | DECISIONS_FASE11.md (ADR-012) |
| 11.9.2 | Integrar CRDT library escolhida via WASM bindings no cockpit | 11.9.1 | Alto | pb-crdt-wasm-integration.md |
| 11.9.3 | Implementar WebSocket sync server no gateway (broadcast updates, persist state) | 11.9.2 | Alto | apps/zylos-gateway/src/crdt_sync.rs, rb-crdt-sync-server.md |
| 11.9.4 | Implementar presence awareness: quem está editando, cursor positions, selection highlights | 11.9.3 | Médio | pop-crdt-presence-indicators.md |
| 11.9.5 | Implementar conflict resolution UI: diff visual, merge options, undo history | 11.9.2 | Alto | pop-crdt-conflict-resolution-ui.md |
| 11.9.6 | Persistir CRDT state em SQLite com versioning e garbage collection | 11.9.3 | Médio | crates/zylos-auth/src/crdt_persistence.rs (expandir) |
| 11.9.7 | Testar colaboração com 3+ clientes simultâneos editando mesmo documento | 11.9.5 | Alto | tests/crdt_multi_client.rs |
| 11.9.8 | Benchmark de latência sync: target <100ms p95 para updates locais, <500ms p95 para remote | 11.9.7 | Médio | pot-crdt-latency-benchmark.md |

### Entregável Verificável
- CRDT library integrada via WASM
- WebSocket sync server funcional
- Presence indicators visuais
- Conflict resolution UI operacional
- State persistente em SQLite
- ≥10 testes multi-client passando
- Latência p95 <100ms local, <500ms remote

### Critérios de Aceitação
- [ ] CRDT library escolhida via ADR-012 com benchmark comparativo
- [ ] WASM bindings compilam e funcionam no cockpit
- [ ] WebSocket sync server suporta ≥10 clientes simultâneos
- [ ] Presence indicators mostram avatares/nomes de editores ativos
- [ ] Conflict resolution UI exibe diff e permite merge manual
- [ ] CRDT state sobrevive a restart do gateway (SQLite persistence)
- [ ] Garbage collection remove estados obsoletos após 30 dias
- [ ] ≥10 testes multi-client passam sem data loss
- [ ] Latência p95 <100ms local, <500ms remote (medido em benchmark)

### Out of Scope
- Offline-first editing (require online connection)
- Rich-text formatting via CRDT (apenas plain text/YAML inicialmente)
- File attachment collaboration

### Gate de Verificação
```bash
cargo test -p zylos-auth crdt && cargo check -p zylos-gateway && cargo check -p zylos-cockpit && echo "GATE 11.9 VERDE"
```

---

## Fase 11.10: Analytics Dashboard com Métricas de Uso

**Objetivo:** Expandir `zylos-analytics` com dashboard completo, export de relatórios e auto-improvement feedback loop.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.10.1 | Expandir `zylos-analytics` com agregações temporais: hourly/daily/weekly/monthly | 11.4.5 | Médio | crates/zylos-analytics/src/aggregations.rs |
| 11.10.2 | Implementar gráficos no cockpit: line charts, bar charts, heatmaps, pie charts | 11.10.1 | Alto | apps/zylos-cockpit/src/components/analytics_charts.rs |
| 11.10.3 | Implementar filtros avançados: date range, agent filter, skill filter, custom dimensions | 11.10.2 | Médio | pop-analytics-filter-builder.md |
| 11.10.4 | Implementar export CSV/PDF com formatação profissional | 11.10.2 | Médio | pop-analytics-export.md |
| 11.10.5 | Implementar auto-improvement feedback loop: identificar skills com baixa success rate → sugerir evolução | 11.10.1 | Alto | soar-analytics-auto-improvement.md |
| 11.10.6 | Implementar retention policy configurável: auto-delete dados >90 dias, archive >30 dias | 11.10.1 | Médio | pop-analytics-retention-policy.md |
| 11.10.7 | Criar preset reports: "Weekly Bot Performance", "Monthly Cost Analysis", "Skill Evolution Summary" | 11.10.4 | Médio | pb-analytics-preset-reports.md |
| 11.10.8 | Testar dashboard com dados reais de 30 dias e validar performance (<2s load time) | 11.10.7 | Médio | pot-analytics-performance-benchmark.md |

### Entregável Verificável
- Dashboard analytics expandido com ≥5 tipos de gráficos
- Filtros avançados funcionais
- Export CSV/PDF operacional
- Auto-improvement suggestions baseadas em dados
- Retention policy automática
- ≥3 preset reports
- Load time <2s com 30 dias de dados

### Critérios de Aceitação
- [ ] ≥5 tipos de gráficos renderizam corretamente (line, bar, heatmap, pie, table)
- [ ] Filtros aplicam em <500ms sem reload da página
- [ ] Export CSV inclui todos os campos visíveis + metadata
- [ ] Export PDF formata corretamente para impressão/A4
- [ ] Auto-improvement identifica ≥1 skill com success rate <70% e sugere evolução
- [ ] Retention policy deleta dados >90 dias automaticamente (testar com dados sintéticos)
- [ ] ≥3 preset reports geram em <5s
- [ ] Dashboard load time <2s com 30 dias de dados (1M+ rows)

### Out of Scope
- Real-time streaming charts (usar polling 30s)
- Custom SQL queries por usuários
- Embedding de dashboards externos (Grafana/Metabase)

### Gate de Verificação
```bash
cargo test -p zylos-analytics && cargo check -p zylos-cockpit && echo "GATE 11.10 VERDE"
```

---

## Fase 11.11: Auditoria Final de Segurança

**Objetivo:** Validação completa de segurança antes de considerar o sistema production-ready: pentest, JWT review, egress validation, threat model atualizado.

### Tarefas

| # | Tarefa | Dependência | Esforço | Docs Obrigatórios |
|---|--------|-------------|---------|-------------------|
| 11.11.1 | Penetration testing manual: OWASP Top 10 checklist contra gateway e cockpit | 11.4 | Alto | reports/PENTEST_FASE11.md |
| 11.11.2 | JWT implementation review: token expiry, refresh rotation, revocation list, algorithm enforcement | 11.4 | Médio | reports/JWT_SECURITY_REVIEW.md |
| 11.11.3 | Egress filter validation: testar bypass attempts, wildcard abuse, DNS rebinding | 11.4 | Alto | reports/EGRESS_VALIDATION_REPORT.md |
| 11.11.4 | Threat model atualizado: STRIDE analysis de novos componentes (sentinel, integrations, CRDT) | 11.7, 11.8, 11.9 | Alto | docs/THREAT_MODEL_FASE11.md |
| 11.11.5 | Dependency audit: `cargo audit` + `cargo deny` + `trivy fs` sem vulnerabilidades críticas | 11.4 | Médio | reports/DEPENDENCY_AUDIT_FASE11.md |
| 11.11.6 | Capability token review: validar least-privilege, revocation, scope isolation | 11.4 | Médio | reports/CAPABILITY_TOKEN_REVIEW.md |
| 11.11.7 | CORS/CSP headers audit: validar configuração contra OWASP recommendations | 11.4 | Baixo | reports/CORS_CSP_AUDIT.md |
| 11.11.8 | Remediar findings críticos/alto da auditoria e re-validar | 11.11.1–11.11.7 | Alto | TRACKRECORD_FASE11.md |
| 11.11.9 | Auditoria adversarial final: juiz independente valida todas as correções | 11.11.8 | Médio | docs/audits/fase11-final-adversarial.md |
| 11.11.10 | Assinar relatório final de segurança e aprovar para produção | 11.11.9 | Baixo | reports/SECURITY_SIGNOFF_FASE11.md |

### Entregável Verificável
- Pentest report com OWASP Top 10 coverage
- JWT security review sem findings críticos
- Egress validation report sem bypass possível
- Threat model atualizado com STRIDE analysis
- Dependency audit limpo (zero critical/high)
- Capability token review aprovado
- CORS/CSP audit compliant
- Todas as correções validadas por auditoria adversarial
- Security sign-off assinado

### Critérios de Aceitação
- [ ] Pentest cobre OWASP Top 10 com zero findings críticos/alto não remediados
- [ ] JWT tokens expiram corretamente, refresh rotation funciona, revocation list efetiva
- [ ] Egress filter bloqueia todos os bypass attempts testados
- [ ] Threat model documenta mitigations para todos os STRIDE threats identificados
- [ ] `cargo audit` retorna zero vulnerabilidades critical/high
- [ ] `cargo deny check` passa sem license violations
- [ ] Capability tokens enforce least-privilege em todos os endpoints
- [ ] CORS/CSP headers seguem OWASP recommendations
- [ ] Auditoria adversarial final retorna APPROVED
- [ ] Security sign-off assinado por Tech Lead + Security Reviewer

### Out of Scope
- Third-party penetration testing (empresa externa)
- Compliance certifications (SOC2, ISO27001)
- Bug bounty program

### Gate de Verificação
```bash
cargo audit && cargo deny check && echo "GATE 11.11 VERDE (dependency audit)"
# Pentest e auditoria adversarial requerem validação manual
```

---

## Timeline e Recursos

| Fase | Duração Estimada | Dependências | Prioridade | Paralelismo |
|------|-----------------|--------------|------------|-------------|
| 11.1 Build Release Validation | 1 semana | — | Crítica | — |
| 11.2 E2E Tests Reais | 1.5 semanas | 11.1 | Crítica | — |
| 11.3 CI/CD Pipeline | 1.5 semanas | 11.2 | Crítica | — |
| 11.4 Deploy Produção + Monitoring | 2 semanas | 11.3 | Crítica | — |
| 11.5 Performance & Lighthouse | 1 semana | 11.4 | Alta | Pode iniciar em paralelo com 11.6 |
| 11.6 Docs Usuário & Onboarding | 1.5 semanas | 11.4 | Alta | Pode iniciar em paralelo com 11.5 |
| 11.7 AIOps Sentinel | 2.5 semanas | 11.4 | Alta | — |
| 11.8 Integrações Slack/Notion/Linear | 2.5 semanas | — | Média | Pode iniciar em paralelo com 11.7 |
| 11.9 CRDT Multi-User | 3 semanas | 11.7 | Média | — |
| 11.10 Analytics Dashboard | 2 semanas | 11.4.5 | Média | Pode iniciar em paralelo com 11.9 |
| 11.11 Auditoria Final Segurança | 2 semanas | 11.7, 11.8, 11.9 | Crítica | — |

**Total:** ~18 semanas (4.5 meses) com paralelismo ótimo
**Total sem paralelismo:** ~20.5 semanas (5 meses)

### Paralelismo Recomendado
```
Semana 1-2:   11.1 → 11.2
Semana 3-4:   11.3
Semana 5-6:   11.4
Semana 7:     11.5 ∥ 11.6 (paralelo)
Semana 8-10:  11.7 ∥ 11.8 (paralelo)
Semana 11-13: 11.9 ∥ 11.10 (paralelo)
Semana 14-15: 11.11
```

---

## Riscos e Mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|-------|--------------|---------|-----------|
| trunk build release ainda excede 5min mesmo com otimizações | Média | Alto | Usar CI runner com 8 vCPU + SSD; considerar wasm-opt separado do trunk |
| Yjs WASM bindings instáveis ou muito grandes (>2MB) | Média | Alto | Avaliar Automerge ou Diamond-types como fallback na T11.9.1 |
| Slack/Notion/Linear APIs mudam durante implementação | Baixa | Médio | Usar SDKs oficiais com version pinning; adapter pattern isola mudanças |
| CRDT sync latency >500ms em redes ruins | Média | Médio | Optimistic UI + conflict resolution; fallback para polling se WS falhar |
| Pentest encontra vulnerabilidade crítica não remediável rapidamente | Baixa | Crítico | Isolar componente afetado; feature flag para desativar até fix |
| Axiom costs excedem budget com logs verbose | Média | Médio | Sampling configurável; log levels dinâmicos; retention policy |
| Lighthouse score <90 apesar das otimizações | Baixa | Médio | Investigar third-party scripts; considerar SSR para critical path |

---

## Decisões Arquiteturais Pendentes

| # | Decisão | Opções | Status | Fase |
|---|---------|--------|--------|------|
| ADR-011 | Profile release optimization strategy | thin LTO vs fat LTO vs no LTO | Pendente | 11.1 |
| ADR-012 | CRDT library choice | Yjs vs Automerge vs Diamond-types | Pendente | 11.9 |
| ADR-013 | Analytics storage backend | SQLite vs DuckDB vs TimescaleDB | Pendente | 11.10 |
| ADR-014 | Notification delivery guarantee | At-least-once vs at-most-once vs exactly-once | Pendente | 11.8 |
| ADR-015 | Sentinel anomaly detection algorithm | Statistical (Z-score) vs ML (isolation forest) | Pendente | 11.7 |

---

## Apêndice A: Documentos Obrigatórios por Fase (Manifesto v3.1 Camada 5)

| Fase | READMEs | Playbooks | POPs | Runbooks | SOARs | POTs | Handoffs | Track Record | Decisions | Outros |
|------|---------|-----------|------|----------|-------|------|----------|--------------|-----------|--------|
| 11.1 | — | pb-wasm-code-splitting | pop-wasm-post-build-optimization | — | — | pot-wasm-size-benchmark, pot-wasm-dependency-audit | — | TRACKRECORD_FASE11 | ADR-011 | — |
| 11.2 | — | pb-e2e-test-setup (validar) | — | — | — | — | — | TRACKRECORD_FASE11 | — | tests/e2e_*.rs |
| 11.3 | cockpit/gateway READMEs (badges) | pb-cloudflare-pages-setup | — | rb-production-deploy | — | — | — | TRACKRECORD_FASE11 | — | .github/workflows/*.yml |
| 11.4 | — | pb-cloudflare-production-setup, pb-gateway-infrastructure-setup, pb-sentry-integration, pb-axiom-logging-setup | pop-health-check-validation, pop-wasm-source-maps | rb-gateway-reverse-proxy, rb-production-smoke-test, rb-production-incident-response | soar-production-alerts | — | HANDOFF_FASE11_PART1 | TRACKRECORD_FASE11 | — | SECURITY.md (update) |
| 11.5 | — | — | pop-critical-css-extraction, pop-resource-hints, pop-image-optimization | rb-accessibility-audit | — | pot-lighthouse-baseline, pot-lighthouse-final-report | — | TRACKRECORD_FASE11 | — | — |
| 11.6 | — | — | — | — | — | — | — | TRACKRECORD_FASE11 | — | USER_GUIDE (update), ONBOARDING_OPERATORS, ONBOARDING_DEVS, FAQ, TUTORIAL_VIDEO, DOC_VALIDATION_REPORT |
| 11.7 | zylos-sentinel README | pb-sentinel-rules-authoring | pop-sentinel-axiom-integration | rb-sentinel-threshold-tuning | soar-sentinel-auto-remediation | — | — | TRACKRECORD_FASE11 | — | tests/sentinel_*.rs |
| 11.8 | zylos-integrations README | pb-slack/notion/linear-integration-setup | — | — | soar-incident-notification-workflow | — | — | TRACKRECORD_FASE11 | — | tests/integration_*.rs |
| 11.9 | — | pb-crdt-wasm-integration | pop-crdt-presence-indicators, pop-crdt-conflict-resolution-ui | rb-crdt-sync-server | — | pot-crdt-latency-benchmark | — | TRACKRECORD_FASE11 | ADR-012 | tests/crdt_*.rs |
| 11.10 | — | pb-analytics-preset-reports | pop-analytics-filter-builder, pop-analytics-export, pop-analytics-retention-policy | — | soar-analytics-auto-improvement | pot-analytics-performance-benchmark | — | TRACKRECORD_FASE11 | ADR-013 | — |
| 11.11 | — | — | — | — | — | — | HANDOFF_FASE11_FINAL | TRACKRECORD_FASE11 | ADR-014, ADR-015 | PENTEST_FASE11, JWT_SECURITY_REVIEW, EGRESS_VALIDATION_REPORT, THREAT_MODEL_FASE11, DEPENDENCY_AUDIT_FASE11, CAPABILITY_TOKEN_REVIEW, CORS_CSP_AUDIT, SECURITY_SIGNOFF_FASE11, fase11-final-adversarial |

### Totais Estimados de Documentos Novos

| Tipo | Quantidade |
|------|-----------|
| READMEs | 3 |
| Playbooks | 12 |
| POPs | 14 |
| Runbooks | 7 |
| SOARs | 5 |
| POTs | 9 |
| Handoffs | 2 |
| Track Records | 1 (contínuo) |
| Decisions (ADRs) | 5 |
| Relatórios de Segurança | 8 |
| Guias de Onboarding/FAQ | 4 |
| Test Files | 12+ |
| **Total** | **≥82** |

---

## Apêndice B: Matriz de Rastreabilidade — Recomendações Auditoria → Tasks

| Recomendação da Auditoria | Fase | Task Principal | Status |
|---------------------------|------|----------------|--------|
| Build release timeout (>7min) | 11.1 | T11.1.1–T11.1.6 | Planejado |
| Testes E2E não executados em browser real | 11.2 | T11.2.1–T11.2.6 | Planejado |
| CI/CD pipeline ausente | 11.3 | T11.3.1–T11.3.8 | Planejado |
| Deploy produção não configurado | 11.4 | T11.4.1–T11.4.10 | Planejado |
| Monitoring/alerting ausente | 11.4, 11.7 | T11.4.4–T11.4.7, T11.7.1–T11.7.8 | Planejado |
| Lighthouse/performance não auditado | 11.5 | T11.5.1–T11.5.7 | Planejado |
| Documentação de usuário incompleta | 11.6 | T11.6.1–T11.6.6 | Planejado |
| AIOps/sentinel ausente | 11.7 | T11.7.1–T11.7.8 | Planejado |
| Integrações externas ausentes | 11.8 | T11.8.1–T11.8.8 | Planejado |
| CRDT multi-user básico (precisa avançar) | 11.9 | T11.9.1–T11.9.8 | Planejado |
| Analytics dashboard básico (precisa expandir) | 11.10 | T11.10.1–T11.10.8 | Planejado |
| Auditoria de segurança pendente | 11.11 | T11.11.1–T11.11.10 | Planejado |

---

## Apêndice C: Skills Hermes Auto-Invocadas por Fase

| Gatilho Detectado | Skill Auto-Carregada | Fases Aplicáveis |
|-------------------|---------------------|-----------------|
| Criar novo crate | `spec-author` + `testing-checklist-author` + `rust-project-setup` | 11.7, 11.8 |
| Criar playbook | `playbook-author` | Todas |
| Criar POP | `pop-author` | Todas |
| Criar runbook | `runbook-builder` | 11.4, 11.5, 11.7, 11.9 |
| Criar SOAR workflow | `soar-orchestration` | 11.4, 11.7, 11.8, 11.10 |
| Completar fase | `handoff-author` + `isolated-audit` | Todas |
| Definir auth/RBAC | `access-control-author` | 11.11 |
| Criar dashboard/gráficos | `dashboard-author` | 11.10 |
| Configurar observabilidade | `aiops-observability` | 11.4, 11.7 |
| Entregar artefato para review | `anti-slop-filter` | Todas |
| Tarefa >15 turnos | `subagent-driven-dev` | Todas |
| Prova por mutação | `mordida-protocol` | Todas |
| UX polish / accessibility | `design-system-author` + `impeccable` | 11.5 |
| Limpeza mock/placeholder | `isolated-audit` | 11.11 |
| Performance tuning Rust | `rust-performance-tuning` | 11.1, 11.10 |
| Security audit | `security-pr-audit` + `hermes-security:security-audit` | 11.11 |
| Threat modeling | `hermes-security:threat-model` | 11.11 |
| Documentação de usuário | `humanizer` (para tom natural) | 11.6 |

---

## Apêndice D: Gate Criteria Documental

Nenhuma fase pode ser declarada completa sem:

1. ✅ Todos os playbooks, POPs, runbooks e SOARs listados no Apêndice A criados via skills correspondentes
2. ✅ README do crate novo (se aplicável) criado com arquitetura, schema e API docs
3. ✅ `TRACKRECORD_FASE11.md` atualizado com entrada cronológica da fase
4. ✅ `DECISIONS_FASE11.md` atualizado com ADRs relevantes da fase
5. ✅ `MODULE_DOCUMENTATION_STATUS.md` atualizado com novos módulos
6. ✅ Cross-links tridirecionais funcionais (playbook ↔ runbook ↔ SOAR)
7. ✅ Auditoria adversarial executada e aprovada antes de avançar à próxima fase
8. ✅ Handoff criado ao final de cada macro-fase (A, B, C, D)

---

## Cross-References

- [IMPLEMENTATION_PLAN_FASE10.md](./IMPLEMENTATION_PLAN_FASE10.md) — Fases 10.1–10.9 completas
- [HANDOFF_FASE10.md](./handoffs/HANDOFF_FASE10.md) — Handoff da fase anterior
- [TRACKRECORD_FASE10.md](./TRACKRECORD_FASE10.md) — Track record da Fase 10
- [CONCEITOS-DESENVOLVIMENTO-IA.md](./CONCEITOS-DESENVOLVIMENTO-IA.md) — Manifesto v3.1
- [AGENTS.md](../AGENTS.md) — Regras de auto-invocação
- [RULES.md](../RULES.md) — Convenções e boundaries
- [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) — Tokens e specs de UI
- [SECURITY.md](../SECURITY.md) — Threat model e mitigações