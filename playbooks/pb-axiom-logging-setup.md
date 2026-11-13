# PB-Axiom-Logging-Setup — Configuração de Logs Estruturados no Gateway

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.4 T11.4.5
**Público:** Dev Pleno, SRE, Tech Lead
**Tempo Estimado:** 15–30 minutos (setup inicial)

---

## 1. Visão Geral

Este playbook define o procedimento para integrar o Axiom como sistema de logs estruturados no gateway Zylos, habilitando ingestão automática de logs HTTP, métricas de latência e eventos de erro via API HTTP com batching assíncrono.

**Gatilho de ativação:** Primeiro deploy em produção ou necessidade de observabilidade centralizada.

**Arquitetura:** O módulo `axiom_logging` envia logs em batches de até 100 entries a cada 5 segundos via worker tokio em background. Se o buffer (capacidade 1000) encher, logs são descartados com warning local.

---

## 2. Procedimento de Setup

### Passo 1: Criar dataset no Axiom (Manual)

1. Acesse https://app.axiom.co → Datasets → Create Dataset
2. Nome: `zylos-gateway-logs`
3. Schema: auto-detect (JSON)
4. Copiar **API Token** (Settings → API Tokens → Generate)
5. Anotar nome exato do dataset

### Passo 2: Configurar env vars no Gateway (Automatizável)

```bash
# Linux/macOS
export AXIOM_TOKEN="xapt-xxxxxxxxxxxx"
export AXIOM_DATASET="zylos-gateway-logs"
# Opcional: endpoint customizado (default: https://api.axiom.co/v1/datasets)
export AXIOM_ENDPOINT="https://api.axiom.co/v1/datasets"

# Windows PowerShell
$env:AXIOM_TOKEN = "xapt-xxxxxxxxxxxx"
$env:AXIOM_DATASET = "zylos-gateway-logs"

# Docker/Kubernetes
env:
  - name: AXIOM_TOKEN
    valueFrom:
      secretKeyRef:
        name: zylos-secrets
        key: axiom-token
  - name: AXIOM_DATASET
    value: "zylos-gateway-logs"
```

**Sem env vars:** O módulo `AxiomClient::new()` retorna `None` e o gateway opera apenas com logs locais via tracing.

### Passo 3: Inicializar cliente no main.rs (Já implementado)

O gateway já inicializa o `AxiomClient` automaticamente quando as env vars estão presentes. Adicione ao `main.rs`:

```rust
// Em apps/zylos-gateway/src/main.rs, após init do Sentry
let axiom_client = zylos_gateway::axiom_logging::AxiomClient::new();
if axiom_client.is_some() {
    info!("Axiom structured logging enabled");
}
```

### Passo 4: Validar ingestão local (Automatizável)

```bash
# Iniciar gateway com Axiom configurado
AXIOM_TOKEN="xapt-test" AXIOM_DATASET="zylos-gateway-logs" cargo run -p zylos-gateway

# Verificar log de inicialização
# Esperado: "Axiom logging initialized" + "Axiom structured logging enabled"

# Gerar tráfego para produzir logs
curl http://localhost:3000/api/v1/health
curl http://localhost:3000/api/v1/cockpit/health

# Verificar dashboard do Axiom → dataset → Explore
# Logs devem aparecer em até 5 segundos (batch interval)
```

---

## 3. Formato dos Logs

Cada entry enviado ao Axiom tem a seguinte estrutura JSON:

```json
{
  "_time": "1700000000",
  "level": "INFO",
  "message": "Request completed",
  "service": "zylos-gateway",
  "request_id": "uuid-v4",
  "method": "GET",
  "path": "/api/v1/health",
  "status": 200,
  "latency_ms": 12
}
```

| Campo | Tipo | Descrição |
|-------|------|-----------|
| `_time` | string (epoch) | Timestamp Unix seconds |
| `level` | string | INFO, WARN, ERROR |
| `message` | string | Mensagem descritiva |
| `service` | string | Sempre "zylos-gateway" |
| `request_id` | string? | UUID da requisição (opcional) |
| `method` | string? | Método HTTP (opcional) |
| `path` | string? | Caminho da rota (opcional) |
| `status` | u16? | Status code HTTP (opcional) |
| `latency_ms` | u64? | Latência em milissegundos (opcional) |

---

## 4. Configurações de Performance

| Parâmetro | Valor | Racional |
|-----------|-------|----------|
| Buffer capacity | 1000 entries | Previne bloqueio sob pico de tráfego |
| Batch size | 100 entries | Balanceia throughput vs latência |
| Flush interval | 5 segundos | Latência aceitável para logs não-críticos |
| Request timeout | 10 segundos | Previne hang em caso de rede lenta |
| Drop policy | Warn + discard | Nunca bloqueia thread principal |

---

## 5. Troubleshooting

| Sintoma | Causa Provável | Solução |
|---------|---------------|---------|
| Log "Axiom logging initialized" não aparece | Env vars não configuradas | Verificar `AXIOM_TOKEN` e `AXIOM_DATASET` |
| Warning "Axiom log buffer full" | Pico de tráfego excede buffer | Aumentar capacidade ou reduzir verbosidade |
| Warning "Axiom ingest failed" | Token inválido ou dataset errado | Validar token e nome do dataset no Axiom UI |
| Warning "Axiom ingest request failed" | Problema de rede/DNS | Verificar conectividade e firewall |
| Logs não aparecem no Axiom | Batch ainda não flushou | Aguardar até 5 segundos (flush interval) |
| Campos opcionais ausentes | Middleware não instrumentado | Adicionar middleware Axum que chama `send_log` |

---

## 6. Integração Futura: Middleware Axum

Para capturar automaticamente todas as requisições HTTP, adicionar middleware ao router:

```rust
// Em lib.rs, antes das camadas CORS/trace
.layer(axum::middleware::from_fn_with_state(
    axiom_client.clone(),
    axiom_middleware::log_request,
))
```

Middleware extrai method, path, status, latency e envia via `axiom_client.send_log()`.

---

## 7. Referências

- [`apps/zylos-gateway/src/axiom_logging.rs`](../apps/zylos-gateway/src/axiom_logging.rs) — Implementação do cliente Axiom
- [`apps/zylos-gateway/Cargo.toml`](../apps/zylos-gateway/Cargo.toml) — Dependência `reqwest` com feature `json`
- [Axiom Ingest API Docs](https://www.axiom.co/docs/restapi/endpoints/ingest)
- [`pb-sentry-integration.md`](../playbooks/pb-sentry-integration.md) — Error tracking complementar

---

*Documento gerado conforme skill `playbook-author`. Próxima revisão: após primeiro batch de logs ingerido em produção.*