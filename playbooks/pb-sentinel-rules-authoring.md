# PB-Sentinel-Rules-Authoring — Guia de Criação de Regras de Alerta YAML

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.7 T11.7.3
**Público:** Dev Pleno, SRE, Tech Lead
**Tempo Estimado:** 15–30 minutos por regra

---

## 1. Visão Geral

Este playbook define como criar, validar e manter regras de alerta para o motor AIOps Sentinel do Zylos-Next. As regras são definidas em YAML e carregadas pelo `AlertRulesEngine` do crate `zylos-sentinel`.

**Gatilho de ativação:** Nova métrica disponível, mudança de threshold, ou incidente recorrente sem cobertura de alerta.

---

## 2. Formato YAML da Regra

```yaml
name: unique-rule-id          # Obrigatório. snake-case, único no arquivo.
description: "Human-readable" # Opcional. Aparece em notificações.
metric: metric_key            # Obrigatório. Deve existir no mapa de métricas.
operator: ">"                 # Obrigatório. >, >=, <, <=, ==, !=
threshold: 1.0                # Obrigatório. Valor numérico para comparação.
window_minutes: 5             # Opcional. Default: 5. Janela de avaliação.
severity: SEV-2               # Obrigatório. SEV-1, SEV-2, SEV-3, SEV-4.
cooldown_minutes: 15          # Opcional. Default: 15. Deduplicação temporal.
notify:                       # Opcional. Lista de targets de notificação.
 - slack: "#channel-name"
 - sentry: true
 - pagerduty: "service-key"
 - webhook: "https://..."
remediation:                  # Opcional. Ação automática pós-alerta.
 action: restart-gateway     # Identificador da ação.
 gated: true                 # Se true, requer approval humano.
 max_auto_executions: 2      # Máx. execuções automáticas no cooldown.
```

### Campos Obrigatórios

| Campo | Tipo | Descrição |
|-------|------|-----------|
| `name` | string | ID único da regra. snake-case recomendado. |
| `metric` | string | Chave exata no mapa de métricas fornecido ao `evaluate()`. |
| `operator` | enum | Um de: `>`, `>=`, `<`, `<=`, `==`, `!=` |
| `threshold` | f64 | Valor numérico para comparação com a métrica. |
| `severity` | enum | `SEV-1` (crítico), `SEV-2` (alto), `SEV-3` (médio), `SEV-4` (baixo). |

### Campos Opcionais

| Campo | Default | Descrição |
|-------|---------|-----------|
| `description` | `""` | Texto exibido em notificações e dashboards. |
| `window_minutes` | `5` | Janela temporal de avaliação da métrica. |
| `cooldown_minutes` | `15` | Período mínimo entre alertas duplicados da mesma regra. |
| `notify` | `[]` | Lista de canais de notificação. |
| `remediation` | `None` | Configuração de ação automática. |

---

## 3. Exemplos Práticos

### Error Rate Alto
```yaml
name: high-error-rate
description: "Taxa de erros HTTP 5xx excede 1%"
metric: error_rate_percent
operator: ">"
threshold: 1.0
window_minutes: 5
severity: SEV-2
cooldown_minutes: 15
notify:
 - slack: "#alerts-zylos"
 - sentry: true
remediation:
 action: notify-oncall
 gated: true
```

### Latência P95 Degradada
```yaml
name: high-latency-p95
description: "Latência P95 excede 2 segundos"
metric: latency_p95_ms
operator: ">"
threshold: 2000.0
severity: SEV-3
cooldown_minutes: 30
notify:
 - slack: "#alerts-zylos"
```

### Health Check Down
```yaml
name: health-check-down
description: "Gateway não responde health check"
metric: health_check_failures
operator: ">="
threshold: 3.0
window_minutes: 2
severity: SEV-1
cooldown_minutes: 10
notify:
 - slack: "#alerts-critical"
 - pagerduty: "zylos-gateway"
remediation:
 action: restart-gateway
 gated: true
 max_auto_executions: 2
```

### Uso de Memória Crítico
```yaml
name: memory-critical
description: "Uso de memória excede 90%"
metric: memory_usage_percent
operator: ">"
threshold: 90.0
severity: SEV-2
cooldown_minutes: 10
notify:
 - slack: "#alerts-zylos"
remediation:
 action: scale-replicas
 gated: false
 max_auto_executions: 3
```

---

## 4. Validação de Regras

### Validar sintaxe YAML localmente
```bash
# Usando Python
python3 -c "import yaml; yaml.safe_load(open('rules.yaml'))"

# Ou usando o próprio crate (teste unitário)
cargo test -p zylos-sentinel -- alert_rules
```

### Verificar nomes únicos
O `AlertRulesEngine::load_from_yaml()` rejeita automaticamente regras com nomes duplicados, retornando erro descritivo.

### Testar avaliação
```rust
let mut engine = AlertRulesEngine::load_from_yaml(yaml_content).unwrap();
let mut metrics = HashMap::new();
metrics.insert("error_rate_percent".to_string(), 2.5);
let alerts = engine.evaluate(&metrics);
assert_eq!(alerts.len(), 1);
```

---

## 5. Boas Práticas

| Prática | Racional |
|---------|----------|
| Nomes descritivos em snake-case | Facilita busca e correlação em logs |
| Cooldown ≥ window_minutes | Previne alertas duplicados dentro da mesma janela |
| SEV-1 apenas para indisponibilidade total | Evita fadiga de alerta |
| `gated: true` para ações destrutivas | Previne restarts acidentais em produção |
| `max_auto_executions` limitado | Evita loops de restart infinito |
| Descrição clara e acionável | On-call entende o problema sem contexto extra |
| Métricas normalizadas (%, ms, count) | Thresholds comparáveis entre regras |

---

## 6. Troubleshooting

| Sintoma | Causa Provável | Solução |
|---------|---------------|---------|
| `Failed to parse alert rules YAML` | Sintaxe YAML inválida | Validar com `python3 -c "import yaml; ..."` |
| `Duplicate alert rule name` | Duas regras com mesmo `name` | Renomear uma das regras |
| Alerta nunca dispara | Métrica key incorreta ou threshold muito alto | Verificar key exata no mapa de métricas |
| Alerta dispara mas não repete | Cooldown muito longo | Reduzir `cooldown_minutes` |
| Remediation não executa | `gated: true` sem approval | Aprovar manualmente ou mudar para `gated: false` |

---

## 7. Referências

- [`crates/zylos-sentinel/src/alert_rules.rs`](../crates/zylos-sentinel/src/alert_rules.rs) — Implementação do engine
- [`docs/soar/soar-production-alerts.md`](../docs/soar/soar-production-alerts.md) — Regras de produção pré-definidas
- [`runbooks/rb-production-incident-response.md`](../runbooks/rb-production-incident-response.md) — Resposta a incidentes
- [`playbooks/pb-sentry-integration.md`](../playbooks/pb-sentry-integration.md) — Integração Sentry
- [`playbooks/pb-axiom-logging-setup.md`](../playbooks/pb-axiom-logging-setup.md) — Fonte de métricas via Axiom

---

*Documento gerado conforme skill `playbook-author`. Próxima revisão: após adição de novas métricas ao sentinel.*