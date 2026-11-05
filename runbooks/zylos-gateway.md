# Runbook — zylos-gateway (API Gateway Consolidada)

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Crate:** `apps/zylos-gateway`
**Dependências:** `zylos-live-comm`, `zylos-skills`
**Fase:** 6 (LIVE_COMMUNICATION) → 7 (END_TO_END_INTEGRATION)

---

## 1. Visão Geral

O `zylos-gateway` é o ponto de entrada HTTP único do Zylos-Next. Consolida rotas de todos os crates em um servidor Axum com CORS, tracing estruturado e health checks hierárquicos.

**Rotas expostas:**
- `GET /gateway/health` → health check do gateway
- `GET /api/v1/health` → health check do cockpit
- `GET /api/v1/events` → AG-UI SSE streaming
- `GET /api/v1/skills` → lista de skills
- `GET /api/v1/skills/:id` → detalhe de skill
- `POST /api/v1/eval/run` → execução de eval flywheel

**Porta padrão:** `0.0.0.0:3000` (override via env `ZYLOS_LISTEN`)

---

## 2. Nível Senior — Arquitetura e Decisões

### 2.1 Por que um gateway consolidado?
- **Single entry point:** Cockpit, bots externos e Hermes bridge acessam um único host/port
- **Middleware compartilhado:** CORS, tracing, rate limiting aplicados uma vez
- **Deploy simplificado:** Um binário, um container, um health check

### 2.2 State management
`AppState` é `Clone` + `Arc<RwLock<...>>` para compartilhamento seguro entre handlers async. SkillRegistry e EvalFlywheel usam `RwLock` porque leituras são frequentes e escritas raras.

### 2.3 Segurança
- CORS permite qualquer origem (`Any`) — aceitável para dev/staging; em produção, restringir para domínios específicos
- Tracing JSON estruturado para ingestão por Loki/Datadog
- Sem auth no gateway — auth é responsabilidade do `zylos-live-comm` (JWT middleware)

### 2.4 Benchmarks esperados
- Health check: <1ms p99
- Skills list (100 skills): <10ms p99
- SSE connection setup: <5ms

---

## 3. Nível Pleno — Operação e Integração

### 3.1 Build e execução
```bash
cd C:/Users/Paulo/zylos-next
cargo build -p zylos-gateway --release
./target/release/zylos-gateway
```

### 3.2 Variáveis de ambiente
| Variável | Default | Descrição |
|----------|---------|-----------|
| `ZYLOS_LISTEN` | `0.0.0.0:3000` | Endereço de escuta |
| `RUST_LOG` | `info,tower_http=debug` | Filtro de tracing |

### 3.3 Health checks
```bash
# Gateway health
curl http://localhost:3000/gateway/health

# Cockpit health (nested)
curl http://localhost:3000/api/v1/health

# Skills API
curl http://localhost:3000/api/v1/skills

# SSE stream
curl -N -H "Accept: text/event-stream" http://localhost:3000/api/v1/events
```

### 3.4 Testes
```bash
cargo test -p zylos-gateway
# 5 testes: gateway_health, cockpit_health_nested, skills_endpoint, sse_endpoint, cors_headers
```

### 3.5 Checklist de deploy
- [ ] `ZYLOS_LISTEN` configurado para porta não-conflitante
- [ ] `RUST_LOG` ajustado para nível desejado
- [ ] Health check respondendo 200
- [ ] CORS headers presentes em respostas
- [ ] SSE stream estabelecendo conexão

---

## 4. Nível Júnior — Execução Guiada

### 4.1 Verificar se compila
```bash
cd C:/Users/Paulo/zylos-next
cargo check -p zylos-gateway
```
**Esperado:** `Finished` sem erros.

### 4.2 Rodar testes
```bash
cargo test -p zylos-gateway
```
**Esperado:** `test result: ok. 5 passed`.

### 4.3 Iniciar servidor
```bash
cargo run -p zylos-gateway
```
**Esperado:** Log `zylos-gateway listening on 0.0.0.0:3000`.

### 4.4 Validar health check
Abra outro terminal:
```bash
curl http://localhost:3000/gateway/health
```
**Esperado:** `{"service":"zylos-gateway","status":"healthy","version":"..."}`

### 4.5 Quando escalar
- Erro de compilação → verificar dependências no Cargo.toml
- Teste falhando → ler mensagem de erro, comparar com expected no código
- Servidor não inicia → verificar se porta 3000 está em uso (`netstat -ano | grep 3000`)
- Health check retorna 404 → verificar se rota foi registrada no router

---

## 5. Referências Cruzadas

- **Código fonte:** `apps/zylos-gateway/src/main.rs`
- **Runbook Fase 6:** `runbooks/fase6-live-communication.md`
- **HANDOFF_6:** `HANDOFF_6.md` §2.1
- **Spec comunicação:** `specs/COMMUNICATION-CONTRACT.md`
- **Security model:** `specs/SECURITY-MODEL.md`