# RB-Cockpit-Release — Runbook de Release do Cockpit Zylos

**Versão:** 1.0.0
**Data:** 2026-11-12
**Playbook Relacionado:** [`pb-e2e-test-setup.md`](../playbooks/pb-e2e-test-setup.md)
**POPs Relacionados:** [`pop-wasm-build-troubleshoot.md`](./pop-wasm-build-troubleshoot.md)
**SOAR Workflow:** N/A (pipeline manual/CI)
**Público:** Dev Pleno, SRE, Tech Lead
**Tempo Estimado:** 30–90 minutos (dependendo do gate)

---

## 1. Visão Geral

Este runbook documenta o pipeline completo de release do cockpit Zylos: build local, validação em preview, QA staging e deploy em produção. Cobre gates obrigatórios entre fases, procedimento de rollback e métricas de sucesso.

**Módulos envolvidos:** `zylos-cockpit` (WASM frontend), `zylos-gateway` (backend Axum), `trunk` (bundler WASM).

**Integrações:** GitHub Actions (CI/CD), Cloudflare Pages / S3 (hosting estático), Sentry (error tracking).

---

## 2. Nível Senior (Tech Lead / Arquiteto / SRE Senior)

### Decisões de Arquitetura

| Decisão | Racional | Alternativa Rejeitada |
|---------|----------|----------------------|
| Pipeline LOCAL→PREVIEW→QA→PROD | Gates bloqueantes previnem regressões em produção | Deploy direto (risco inaceitável) |
| `trunk build --release` como artefato único | Reprodutibilidade: mesmo commit = mesmo hash WASM | Build ad-hoc por dev (inconsistência) |
| Rollback via git revert + redeploy | Simplicidade: não requer infraestrutura de versionamento de artefatos | Blue/green deploy (complexidade excessiva para cockpit) |
| Health check `/api/v1/cockpit/health` | Validação end-to-end pós-deploy (não apenas HTTP 200) | Apenas status code (falso positivo) |

### Modelo de Segurança

- **Tokens JWT:** Bearer token obrigatório em todos os endpoints protegidos
- **CORS:** Whitelist explícita de origens permitidas (`localhost:*`, domínio de produção)
- **CSP Headers:** `script-src 'self' 'wasm-unsafe-eval'` para permitir WASM execution
- **Secrets:** Nunca em código-fonte; injetados via env vars no CI/CD

### Benchmarks e Targets

| Métrica | Target | Medição |
|---------|--------|---------|
| Build time (dev) | <30s | `time trunk build` |
| Build time (release) | <5min | `time trunk build --release` |
| WASM size (gzipped) | <5MB | `gzip -c dist/*_bg.wasm \| wc -c` |
| Clippy warnings | 0 | `cargo clippy --workspace -- -D warnings` |
| Test pass rate | 100% | `cargo test --workspace` |
| Lighthouse performance | ≥90 | Chrome DevTools audit |
| WCAG AA score | ≥90 | axe-core CLI |

### Trade-offs

| Opção | Pró | Contra | Escolha |
|-------|-----|--------|---------|
| LTO fat vs thin | Melhor otimização (-10% size) | Build 2x mais lento | Thin (balanceado) |
| `opt-level = "z"` vs `"s"` | Menor binário | Pior performance hot paths | `"z"` (cockpit é I/O-bound) |
| Strip symbols | -20% size | Debugging difícil em prod | `strip = true` + source maps separados |

---

## 3. Nível Pleno (Dev Pleno / On-call Engineer)

### Pipeline Completo

#### Fase 1: LOCAL (Desenvolvimento)

```bash
# Gate de qualidade
cargo clippy --workspace -- -D warnings
cargo test --workspace

# Build dev (rápido)
cd apps/zylos-cockpit && trunk build

# Testar localmente
python3 -m http.server 8080 --directory dist
# Abrir http://localhost:8080
```

**Gate LOCAL:** Zero erros clippy + zero falhas em testes + cockpit carrega no browser.
**Se falhar:** Corrigir warnings/testes antes de prosseguir. Consultar `pop-wasm-build-troubleshoot.md` se build falhar.

#### Fase 2: PREVIEW (Branch Deploy)

```bash
# Build release otimizado
trunk build --release

# Validar tamanho do WASM
ls -lh dist/*_bg.wasm
gzip -k dist/*_bg.wasm && ls -lh dist/*_bg.wasm.gz

# Deploy para preview URL (CI/CD ou manual)
# Exemplo Cloudflare Pages:
wrangler pages deploy dist --project-name zylos-cockpit-preview
```

**Gate PREVIEW:** Build release completa sem timeout + tamanho WASM <10MB gzipped + preview URL acessível.
**Se falhar:** Verificar `pop-wasm-build-troubleshoot.md` §Passo 2D (timeout) ou `pot-wasm-size-optimization.md` (tamanho).

#### Fase 3: QA (Staging)

```bash
# Testes E2E em browser headless
wasm-pack test --headless --chrome

# Accessibility audit (WCAG AA)
npx @axe-core/cli http://staging-url --stdout

# Performance profiling (Chrome DevTools → Performance tab)
# Gravar 30s de uso normal, verificar: FPS > 30, memory < 200MB
```

**Gate QA:** E2E tests passam + WCAG AA score ≥ 90 + performance metrics dentro dos limites.
**Se falhar:** Bloquear release, criar ticket com evidências (screenshots, logs, traces).

#### Fase 4: PROD (Production)

```bash
# Tag release
git tag v0.X.Y
git push origin v0.X.Y

# Deploy production (CI/CD automático via tag)
# OU manual: wrangler pages deploy dist --project-name zylos-cockpit-prod

# Smoke test em produção
curl https://prod-url/api/v1/health
curl https://prod-url/api/v1/cockpit/health
```

**Gate PROD:** Health checks verdes + nenhuma regressão reportada em 24h.
**Se falhar:** Acionar rollback imediatamente (§Rollback abaixo).

### Troubleshooting Table

| Sintoma | Causa Provável | Solução | Tempo |
|---------|---------------|---------|-------|
| Build release timeout (>7min) | LTO fat + máquina lenta | Usar `--profile release-lto` (thin LTO) | 5min |
| WASM >10MB gzipped | Dependências pesadas / debug info | `wasm-opt -Oz --strip-debug` | 10min |
| E2E tests flaky | Race condition em async | Adicionar `#[wasm_bindgen_test(timeout = 300)]` | 15min |
| Health check falha pós-deploy | Gateway não reiniciou / CORS errado | Verificar logs gateway + CORS config | 10min |
| Cockpit branco em produção | WASM não carregou / CSP bloqueia | Verificar console browser + CSP headers | 5min |

### Checklist Operacional

- [ ] Todos os gates anteriores verdes antes de iniciar próxima fase
- [ ] Branch protegida: merge apenas via PR aprovado
- [ ] Secrets injetados via CI/CD (nunca hardcoded)
- [ ] Notificação automática no #dev após cada deploy
- [ ] Monitoramento ativo (Sentry + health checks) por 24h pós-deploy

---

## 4. Nível Júnior (Dev Júnior / Estagiário / Agente Automatizado)

### Passo a Passo: Release Completo

#### Passo 1: Verificar pré-condições (5 min)
```bash
git status  # Deve estar em branch main limpa
cargo clippy --workspace -- -D warnings  # Exit code 0
cargo test --workspace  # Exit code 0
```
**Output esperado:** Sem mudanças pendentes, zero warnings, zero falhas.
**Se falhar:** NÃO prosseguir. Corrigir problemas primeiro ou escalar para Dev Pleno.

#### Passo 2: Build release (5–35 min)
```bash
cd apps/zylos-cockpit && trunk build --release
```
**Output esperado:** `Finished` sem erros, `dist/` populado.
**Se falhar:** Consultar `pop-wasm-build-troubleshoot.md`. Se timeout, usar `--profile release-lto`.

#### Passo 3: Validar tamanho (2 min)
```bash
gzip -k dist/*_bg.wasm && ls -lh dist/*_bg.wasm.gz
```
**Output esperado:** Tamanho gzipped <10MB (ideal <5MB).
**Se falhar:** Consultar `pot-wasm-size-optimization.md`. Se >15MB, bloquear release e investigar.

#### Passo 4: Deploy preview (5 min)
```bash
wrangler pages deploy dist --project-name zylos-cockpit-preview
```
**Output esperado:** URL de preview retornada pelo Wrangler.
**Se falhar:** Verificar autenticação Wrangler (`wrangler login`) ou permissões Cloudflare.

#### Passo 5: Validar preview (10 min)
1. Abrir URL de preview no browser
2. Verificar todas as 31 tabs carregam sem erro
3. Testar login/logout flow
4. Verificar console do browser sem erros críticos

**Output esperado:** Cockpit funcional, sem erros no console.
**Se falhar:** Documentar erro com screenshot, criar ticket, NÃO prosseguir para QA.

#### Passo 6: Executar testes E2E (5–10 min)
```bash
wasm-pack test --headless --chrome
```
**Output esperado:** `test result: ok. X passed; 0 failed`.
**Se falhar:** Classificar severidade conforme `pb-e2e-test-setup.md` §2 Senior. SEV-1/2 = bloquear release.

#### Passo 7: Deploy produção (após aprovação Tech Lead)
```bash
git tag v0.X.Y && git push origin v0.X.Y
# CI/CD faz deploy automaticamente
```
**Output esperado:** Tag criada, deploy iniciado no CI/CD.
**Se falhar:** Verificar logs do CI/CD. Se deploy parcial, acionar rollback.

#### Passo 8: Smoke test pós-deploy (5 min)
```bash
curl https://prod-url/api/v1/health
curl https://prod-url/api/v1/cockpit/health
```
**Output esperado:** Ambos retornam `{"status": "ok"}` com HTTP 200.
**Se falhar:** ACIONAR ROLLBACK IMEDIATAMENTE (§Rollback).

### Regras de Quando NÃO Agir e Escalar Imediatamente

- ❌ **NÃO** faça deploy em produção sem aprovação explícita do Tech Lead
- ❌ **NÃO** ignore falhas em gates (LOCAL/PREVIEW/QA) — eles existem por razão
- ❌ **NÃO** modifique código durante release window sem criar nova branch
- ❌ **NÃO** desabilite gates de CI/CD para "acelerar" release
- ✅ **SEMPRE** notifique #dev antes e após cada deploy
- ✅ **SEMPRE** monitore Sentry por 24h após deploy em produção

### Glossário

| Termo | Definição |
|-------|-----------|
| `trunk` | Bundler WASM para projetos Leptos/Rust |
| `wrangler` | CLI da Cloudflare para deploy de Pages/Workers |
| `LTO` | Link-Time Optimization (otimização cross-crate) |
| `CSP` | Content Security Policy (header HTTP de segurança) |
| `Smoke test` | Teste rápido pós-deploy para validar funcionalidade básica |
| `Gate` | Ponto de verificação obrigatório entre fases do pipeline |

---

## 5. Referências Cruzadas

### Playbooks Estratégicos
- [`pb-e2e-test-setup.md`](../playbooks/pb-e2e-test-setup.md) — Configuração e execução de testes E2E

### POPs Operacionais
- [`pop-wasm-build-troubleshoot.md`](./pop-wasm-build-troubleshoot.md) — Diagnóstico de falhas WASM build
- [`pot-wasm-size-optimization.md`](../playbooks/pot-wasm-size-optimization.md) — Otimização de tamanho WASM

### Dashboards de Monitoramento
- Cockpit tab **Sentinel** — Anomalias em tempo real
- Cockpit tab **Fleet** — Métricas de bots
- Sentry dashboard — Error tracking pós-deploy

### Canais de Comunicação
- Slack: `#dev` (deploy notifications), `#infra` (SRE on-call)
- PagerDuty: Tech Lead (SEV-1 pós-deploy)

### Revisão
- **Última revisão:** 2026-11-12
- **Próximo review obrigatório:** 2027-05-12 (180 dias) OU após próximo incidente SEV-1/SEV-2 relacionado a release

---

*Documento gerado via skill `runbook-builder` v1.0.0. Para questões, consultar Tech Lead ou SRE on-call.*