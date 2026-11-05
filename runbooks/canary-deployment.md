# Runbook de Canary Deployment — Zylos-Next

> **Versão:** 1.0.0 | **Última revisão:** 2026-11-04 | **Classificação:** Interno
> **Frequência:** A cada release | **Proprietário:** Release Engineering

---

## 1. Objetivo

Executar deploy progressivo (canary) dos componentes Zylos-Next seguindo a estratégia
5% → 25% → 50% → 100%, com métricas de health validadas em cada estágio, rollback
automático em caso de anomalias, e gates de aprovação entre fases.

---

## 2. Escopo

### 2.1 Componentes Elegíveis

| Componente | Deploy Type | Canary Strategy |
|---|---|---|
| `zylos-gateway` | Axum HTTP/SSE | Traffic split via load balancer |
| `zylos-cockpit` | Leptos WASM bundle | Asset versioning + feature flag |
| `zylos-sandbox` | wasmtime + bollard Docker | Container fleet split |
| `zylos-triage` | Stateless classifier | Traffic split |
| `zylos-sentinel` | Health scanner | Rolling update |
| `zylos-live-comm` | AG-UI streaming | Connection pool split |
| `zylos-soar` | Orchestration | Workflow version routing |

### 2.2 Fora do Escopo

- `zylos-memory-system`: migrations seguem runbook específico de schema migration.
- Mudanças de infraestrutura (K8s, rede, storage).
- Deploy de branches feature (usar preview environments).

---

## 3. Pré-requisitos

### 3.1 Validação Pré-Deploy

- [ ] CI/CD pipeline verde (todos os testes unitários, integração e E2E).
- [ ] Schema migration validada em staging (se aplicável).
- [ ] Feature flags configuradas para rollback rápido.
- [ ] Dashboard de métricas de canary disponível e acessível.
- [ ] `zylos-sentinel` configurado com alertas para o novo deploy.
- [ ] On-call SRE informado e disponível para monitoramento.

### 3.2 Validação de Schema Migration

Quando o release inclui mudanças no schema do `zylos-memory-system`:

```bash
# Validar migration em ambiente de staging
zylos-memory migrate validate --source=<staging-db> --migration=<file.sql>

# Verificar backward compatibility
zylos-memory migrate check-backward-compat --migration=<file.sql>

# Dry-run da migration em shadow mode
zylos-memory migrate dry-run --source=<production-read-replica> --migration=<file.sql>

# Confirmar que não há breaking changes
zylos-memory migrate assert-safe --migration=<file.sql>
```

> ** SÊNIOR:** Revisa o diff de migration e aprova a execução. Valida que não há
> DROP COLUMN, ALTER TYPE incompatível, ou mudança de constraints que possa causar
> deadlock em produção.

> ** PLENO:** Executa as validações acima. Reporta resultados ao Sênior.

> ** JÚNIOR:** Garante que o arquivo de migration está no repositório e versionado.

---

## 4. Estratégia de Canary — 5% → 25% → 50% → 1
### 4.1 Visão Geral

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Canary 5%  │───▶│ Canary 25%  │───▶│ Canary 50%  │───▶│  Full 100%  │
│  (15 min)   │    │  (15 min)   │    │  (15 min)   │    │  (complete) │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
     │                  │                  │                  │
  [Auto]            [Auto+Human]       [Human Gate]       [Human Gate]
```

### 4.2 Métricas de Health por Estágio

| Métrica | 5% Canary | 25% Canary | 50% Canary | 100% Deploy |
|---|---|---|---|---|
| HTTP 5xx rate | < 0.1% | < 0.1% | < 0.05% | < 0.05% |
| Latência p99 | < 1.5x baseline | < 1.3x baseline | < 1.2x baseline | < 1.2x baseline |
| Error rate (triaz) | < 1% | < 0.5% | < 0.5% | < 0.3% |
| Memory usage | < 120% baseline | < 110% baseline | < 110% baseline | < 105% baseline |
| CPU usage | < 150% baseline | < 130% baseline | < 120% baseline | < 110% baseline |
| Sandbox crash rate | 0% | 0% | 0% | 0% |
| SOAR workflow timeout | < 1% | < 0.5% | < 0.5% | < 0.3% |
| AG-UI stream drops | < 0.1% | < 0.05% | < 0.05% | < 0.01% |

### 4.3 Rollback Automático — Triggers

O rollback é executado automaticamente quando **qualquer** trigger é acionado:

| Trigger | Limiar | Ação |
|---|---|---|
| HTTP 5xx rate | > 1% por 2 min | Rollback imediato |
| Latência p99 | > 3x baseline por 3 min | Rollback imediato |
| OOM kills | ≥ 1 em qualquer pod | Rollback imediato |
| Sandbox panic | Qualquer panic | Rollback imediato |
| Error rate (triage) | > 5% por 5 min | Rollback imediato |
| Health endpoint | Falha por 30s contínuos | Rollback imediato |
| Memory leak | > 500MB crescimento em 10 min | Rollback imediato |

```bash
# Status do canary em tempo real
zylos-canary status --deployment=<deploy-id>

# Forçar rollback manual
zylos-canary rollback --deployment=<deploy-id> --reason="<motivo>"

# Abortar canary sem rollback (manter versão estável)
zylos-canary abort --deployment=<deploy-id>
```

---

## 5. Procedimento

### 5.1 Estágio 1 — Canary 5% (Automático)

> ** SÊNIOR:** Inicia o canary deploy. Monitora os primeiros 5 minutos. Aprova
> progressão para 25% se todas as métricas estão dentro do esperado.

> ** PLENO:** Configura o traffic split e valida que o `zylos-sentinel` está
> monitorando o canary. Ajusta alertas se necessário.

> ** JÚNIOR:** Acompanha o dashboard de métricas. Reporta anomalias ao canal
> de deploy. Registra horários de cada evento.

```bash
# === zylos-gateway: Canary 5% ===
zylos-deploy canary start \
  --component=zylos-gateway \
  --image=<new-image-tag> \
  --traffic-split=5 \
  --duration=15m \
  --auto-rollback=true \
  --metrics-endpoint=https://metrics.zylos.internal/canary

# Monitorar health do gateway canary
watch -n 5 'zylos-canary metrics --component=zylos-gateway --stage=5pct'

# Verificar que SSE streams estão ativos no canary
zylos-gateway sse-status --canary --expect=active

# === zylos-cockpit: Canary 5% ===
zylos-deploy canary start \
  --component=zylos-cockpit \
  --wasm-bundle=<new-bundle-hash> \
  --traffic-split=5 \
  --feature-flag=cockpit-canary-v<version> \
  --duration=15m

# Validar que o WASM bundle carrega corretamente
curl -sf https://cockpit.zylos.internal/assets/<new-bundle-hash>/cockpit.wasm
```

### 5.2 Estágio 2 — Canary 25% (Auto + Human Gate)

> ** SÊNIOR:** Revisa métricas dos primeiros 15 minutos. Verifica logs de erro.
> Decide se aprova progressão para 50%. Documenta decisão no canal de deploy.

> ** PLENO:** Executa progressão se aprovada. Monitora métricas em tempo real.
> Prepara rollback plan caso necessário.

> ** JÚNIOR:** Atualiza ticket de deploy. Monitora canais de suporte para
> reports de usuários.

```bash
# === Aprovação para 25% ===
# SÊNIOR executa:
zylos-canary promote --deployment=<deploy-id> --stage=25pct --approver=<nome>

# === zylos-gateway: Promover para 25% ===
zylos-deploy canary promote \
  --component=zylos-gateway \
  --traffic-split=25 \
  --duration=15m

# === zylos-cockpit: Promover para 25% ===
zylos-deploy canary promote \
  --component=zylos-cockpit \
  --traffic-split=25 \
  --duration=15m

# Validação de métricas agregadas
zylos-canary report --deployment=<deploy-id> --stage=25pct --output=table
```

### 5.3 Estágio 3 — Canary 50% (Human Gate Obrigatório)

> ** SÊNIOR:** Gate obrigatório. Revisa relatório completo de métricas.
> Valida que não há regressão em fluxos SOAR. Confirma com o Product Owner
> se a experiência do usuário está intacta. **Dois SÊNIORes devem aprovar.**

> ** PLENO:** Gera relatório de métricas. Prepara rollback plan.
> Executa progressão apenas após aprovação dupla.

> ** JÚNIOR:** Monitora métricas. Acompanha canais de suporte intensamente.
> Reporta qualquer anomalia imediatamente.

```bash
# === GATE: Aprovação dupla obrigatória ===
# SÊNIOR 1:
zylos-canary gate-approve --deployment=<deploy-id> --stage=50pct --approver=<sênior1>
# SÊNIOR 2:
zylos-canary gate-approve --deployment=<deploy-id> --stage=50pct --approver=<sênior2>

# === zylos-gateway: Promover para 50% ===
zylos-deploy canary promote \
  --component=zylos-gateway \
  --traffic-split=50 \
  --duration=15m \
  --require-dual-approval=true

# === zylos-cockpit: Promover para 50% ===
zylos-deploy canary promote \
  --component=zylos-cockpit \
  --traffic-split=50 \
  --duration=15m

# Validação profunda — executar smoke tests contra canary
zylos-test smoke --target=canary --component=zylos-gateway --verbose
zylos-test smoke --target=canary --component=zylos-cockpit --verbose
zylos-soar test --target=canary --runbook=regression --timeout=300
```

### 5.4 Estágio 4 — Full Deploy 100% (Human Gate)

> ** SÊNIOR:** Gate final. Revisa todas as métricas acumuladas. Confirma que
> a versão canary é estável por 30+ minutos em 50%. Aprova rollout completo.
> Comunica stakeholders.

> ** PLENO:** Executa rollout completo. Monitora métricas de produção.
> Remove versão antiga após confirmação de estabilidade.

> ** JÚNIOR:** Monitora. Remove feature flags obsoletas. Atualiza documentação.

```bash
# === GATE FINAL: Aprovação para 100% ===
zylos-canary gate-approve --deployment=<deploy-id> --stage=100pct --approver=<sênior>

# === Full rollout zylos-gateway ===
zylos-deploy canary complete \
  --component=zylos-gateway \
  --cleanup-old-version=true

# === Full rollout zylos-cockpit ===
zylos-deploy canary complete \
  --component=zylos-cockpit \
  --cleanup-old-bundle=true

# === Cleanup pós-deploy ===
zylos-canary cleanup --deployment=<deploy-id>
zylos-feature-flag remove --flag=cockpit-canary-v<version>

# Validação final
zylos-sentinel full-health-check --cluster=production --verbose
```

---

## 6. Rollback

### 6.1 Rollback Automático

Acionado pelos triggers definidos na seção 4.3. O sistema:

1. Para de enviar tráfego para o canary.
2. Restaura 100% do tráfego para a versão estável.
3. Gera alerta P2 via `notification-router`.
4. Cria ticket de incidente automaticamente.

### 6.2 Rollback Manual

```bash
# Rollback de componente específico
zylos-canary rollback --component=zylos-gateway --reason="<motivo>"

# Rollback de todos os componentes do deployment
zylos-canary rollback-all --deployment=<deploy-id> --reason="<motivo>"

# Rollback de schema migration (se aplicável)
zylos-memory migrate rollback --target=<previous-version> --force
```

> ** SÊNIOR:** Decide rollback manual. Comunica stakeholders. Inicia post-mortem.

> ** PLENO:** Executa rollback. Valida que a versão estável está operando.

> ** JÚNIOR:** Monitora métricas pós-rollback. Atualiza ticket.

---

## 7. Verificação

### 7.1 Checklist Pós-Deploy

- [ ] Todas as métricas dentro dos targets por 30 minutos contínuos.
- [ ] Zero alertas P1/P2 no `zylos-sentinel`.
- [ ] Smoke tests E2E passando em produção.
- [ ] Feature flags removidas (se aplicável).
- [ ] Versão antiga limpa e removida.
- [ ] Documentação atualizada.
- [ ] Change log atualizado.
- [ ] Comunicação de release enviada.

### 7.2 Comandos de Verificação

```bash
# Verificar todas as métricas do deployment
zylos-canary report --deployment=<deploy-id> --full

# Validar health de todos os componentes
zylos-sentinel full-health-check --cluster=production

# Executar smoke tests E2E
zylos-test e2e --environment=production --suite=regression

# Validar integridade do memory-system (se migration foi aplicada)
zylos-memory integrity-check --quick
```

---

## 8. Referências

- `zylos-gateway` — Documentação de deployment e configuration
- `zylos-cockpit` — Documentação de WASM bundle versioning
- `zylos-sentinel` — Configuração de alertas e health checks
- `notification-router` — Configuração de alertas de deploy
- Feature flag service — Documentação de flags e toggles
- Schema migration runbook — `/runbooks/schema-migration.md`
- Incident response playbook — `/playbooks/incident-response-soar.md`