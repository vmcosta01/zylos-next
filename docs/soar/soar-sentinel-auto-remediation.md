# SOAR — Auto-Remediação Gated do Sentinel

**Versão:** 1.0.0
**Data:** 2026-11-17
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.7 T11.7.5
**POP Relacionado:** [`pop-sentinel-axiom-integration.md`](../../playbooks/pop-sentinel-axiom-integration.md)
**Runbook Relacionado:** [`rb-sentinel-threshold-tuning.md`](../../runbooks/rb-sentinel-threshold-tuning.md)
**SOAR Relacionado:** [`soar-production-alerts.md`](./soar-production-alerts.md)
**Público:** SRE, Tech Lead, On-call
**Trigger:** Alerta disparado por regra YAML do Sentinel com bloco `remediation`

---

## 1. Visão Geral

Quando uma regra do Sentinel dispara e possui `remediation`, o `RemediationGate` (`crates/zylos-sentinel/src/remediation.rs`) cria uma **proposta** (`rem-<n>`). O que acontece com ela depende do modo do gate — e **nenhuma ação destrutiva (restart/scale) é executada sem aprovação humana explícita**, em nenhum modo.

Cada transição (proposta, execução, rejeição, expiração) gera um `SoarRemediationEvent` que o gateway converte (`to_soar_event`) em `zylos_soar::ZylosSecurityEvent` (`category = Anomaly`, `source = "zylos-sentinel"`) e envia ao `SoarOrchestrator::process_event` (`apps/zylos-gateway/src/sentinel_runtime.rs`, `OrchestratorForwarder`). A triagem SOAR roteia `High/Anomaly` para *enrich and assign*; `Low` vira *log only*.

## 2. Ações Suportadas

| String YAML | Ação | Destrutiva? |
|-------------|------|-------------|
| `restart-<serviço>` / `restart:<serviço>` | `RestartService` | **Sim** |
| `scale:<serviço>:<réplicas>` | `ScaleReplicas` | **Sim** |
| `notify-oncall` / `notify-oncall:<canal>` | `NotifyOncall` | Não |

## 3. Modos do Gate (`SENTINEL_REMEDIATION_MODE`)

| Modo | Notificação não-gated | Notificação `gated: true` | Restart / Scale |
|------|-----------------------|---------------------------|-----------------|
| `dry_run` (**padrão**, também para valor inválido) | registra "executaria…" | registra | registra — nunca executa nem aceita aprovação |
| `require_approval` | pendente → HITL | pendente → HITL | pendente → HITL |
| `auto` | executa automaticamente até `max_auto_executions` | pendente → HITL | pendente → HITL |

Regras adicionais:
- Proposta pendente expira após `approval_ttl_secs` (padrão 15 min) → status `expired`.
- Aprovador vazio → `400`; proposta inexistente → `404`; já decidida/expirada → `409`.
- Executor padrão do gateway é `LogOnlyExecutor`: mesmo aprovado, **não toca infraestrutura** até que um executor de infra seja plugado explicitamente em código (revisão obrigatória).

## 4. Fluxo

```
alerta ─▶ propose()
            ├─ dry_run ──────────────▶ status dry_run ──▶ SOAR (log)
            ├─ auto + notify não-gated ─▶ execute ─▶ executed/failed ─▶ SOAR
            └─ demais ───────────────▶ pending_approval ─▶ SOAR (enrich & assign)
                                             │
             cockpit (aba Sentinel) / API ◀──┘
             POST /api/v1/sentinel/remediations/:id/approve {"approver": "..."}
             POST /api/v1/sentinel/remediations/:id/reject  {"approver": "...", "reason": "..."}
                                             │
                                  executed | failed | rejected | expired ─▶ SOAR + auditoria + Axiom
```

## 5. Procedimento On-call (HITL)

1. Receber o evento SOAR / notificação (`Sentinel: <regra> → <ação>`).
2. Abrir cockpit → aba **Sentinel** → seção *Remediações (HITL)*; conferir gráfico da métrica (ponto vermelho = anomalia Z-score, linha tracejada = limite).
3. Validar no Axiom (ver POP §5 Passo 6) se não é falso positivo.
4. Informar o nome no campo *Aprovador* e clicar **Aprovar** ou **Rejeitar**. Via API:
   ```bash
   curl -X POST http://localhost:3000/api/v1/sentinel/remediations/rem-3/approve \
     -H 'content-type: application/json' -d '{"approver":"oncall-sre"}'
   ```
5. Se rejeitado por falso positivo, abrir ciclo de tuning ([`rb-sentinel-threshold-tuning.md`](../../runbooks/rb-sentinel-threshold-tuning.md)).

## 6. Auditoria

`GET /api/v1/sentinel/remediations` retorna `mode`, `proposals` e `audit` (`proposed`, `dry_run`, `approved`, `rejected`, `executed`, `failed`, `expired`, com ator e epoch). Os mesmos estados chegam ao Axiom como `kind = sentinel_remediation`.

## 7. Promoção de Modo (dry_run → require_approval → auto)

| De → Para | Pré-requisito |
|-----------|---------------|
| `dry_run` → `require_approval` | ≥7 dias em dry-run com FP rate <5% por regra (consulta Axiom) e on-call treinado neste SOAR |
| `require_approval` → `auto` | ≥14 dias sem aprovação indevida; apenas notificações não-gated passam a ser automáticas |

Rollback imediato: `SENTINEL_REMEDIATION_MODE=dry_run` + restart do gateway.

## 8. Verificação

```bash
cargo test -p zylos-sentinel                       # gate: dry-run padrão, destrutivas exigem HITL, TTL, limites
cargo test -p zylos-gateway --lib sentinel_runtime # HTTP approve/reject, encaminhamento SOAR, dry-run
```
