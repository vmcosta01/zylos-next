# POP — Health Check Validation (Procedimento Operacional Padrão)

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.4 T11.4.6
**Runbook Relacionado:** `rb-production-smoke-test.md`
**Público:** Dev Pleno, SRE, Agente Automatizado
**Tempo Estimado:** 5–10 minutos

---

## Objetivo

Validar que os endpoints de health check do gateway Zylos respondem corretamente, confirmando disponibilidade do serviço e listagem de endpoints do cockpit.

---

## Pré-Condições

- [ ] Gateway rodando localmente (`cargo run -p zylos-gateway`) ou em produção
- [ ] Acesso à porta 3000 (local) ou URL de produção
- [ ] `curl` disponível no ambiente

---

## Procedimento

### Passo 1: Validar `/api/v1/health` (1 min)

```bash
curl -s http://localhost:3000/api/v1/health | python3 -m json.tool
```

**Output esperado:**
```json
{
    "status": "ok",
    "service": "zylos-gateway",
    "version": "0.1.0"
}
```

**Se falhar:** Verificar se gateway está rodando (`tasklist | grep zylos-gateway`). Se não estiver, iniciar com `cargo run -p zylos-gateway`.

### Passo 2: Validar `/api/v1/cockpit/health` (1 min)

```bash
curl -s http://localhost:3000/api/v1/cockpit/health | python3 -m json.tool
```

**Output esperado:**
```json
{
    "status": "ok",
    "component": "cockpit",
    "endpoints_available": [
        "/api/v1/channels",
        "/api/v1/computer-view/sessions",
        "/api/v1/hermes/skills",
        "/api/v1/goals",
        "/api/v1/memory",
        "/api/v1/personalities",
        "/api/v1/sentinel",
        "/api/v1/evidence",
        "/api/v1/phase2/status"
    ]
}
```

**Se falhar:** Verificar logs do gateway para erros de inicialização de subsistemas.

### Passo 3: Validar `/gateway/health` (legado) (1 min)

```bash
curl -s http://localhost:3000/gateway/health | python3 -m json.tool
```

**Output esperado:**
```json
{
    "service": "zylos-gateway",
    "status": "healthy",
    "version": "0.1.0"
}
```

### Passo 4: Validar readiness probe `/gateway/ready` (1 min)

```bash
curl -s http://localhost:3000/gateway/ready | python3 -m json.tool
```

**Output esperado:** `{"status": "ready"}` (após inicialização completa) ou `{"status": "not_ready"}` (durante startup).

### Passo 5: Validar liveness probe `/gateway/live` (30s)

```bash
curl -s http://localhost:3000/gateway/live | python3 -m json.tool
```

**Output esperado:** `{"status": "alive"}`

---

## Validação Pós-Execução

- [ ] Todos os 5 endpoints retornam HTTP 200
- [ ] `/api/v1/health` retorna `"status": "ok"`
- [ ] `/api/v1/cockpit/health` lista todos os 9 endpoints disponíveis
- [ ] `/gateway/ready` retorna `"ready"` após startup completo
- [ ] Nenhum erro nos logs do gateway durante validação

---

## Rollback

**Trigger:** Endpoints de health check causam erro 500 ou panic no gateway.

1. Reverter mudanças em `apps/zylos-gateway/src/lib.rs`:
 ```bash
 git checkout HEAD -- apps/zylos-gateway/src/lib.rs
 ```
2. Reiniciar gateway: `cargo run -p zylos-gateway`
3. Validar que endpoints legados (`/gateway/health`, `/gateway/ready`, `/gateway/live`) ainda funcionam

---

## Quando NÃO Usar Este POP

- ❌ Gateway não está rodando → Iniciar primeiro com `cargo run -p zylos-gateway`
- ❌ Problema de CORS em browser → Usar `curl` ou desativar CORS temporariamente para debug
- ❌ Endpoint retorna 404 → Verificar se rota foi registrada corretamente em `lib.rs`
- ❌ Produção com auth habilitado → Adicionar header `Authorization: Bearer <token>` nas requisições

---

## Referências

- [`apps/zylos-gateway/src/lib.rs`](../apps/zylos-gateway/src/lib.rs) — Implementação dos endpoints
- [`playbooks/pb-cloudflare-pages-setup.md`](../playbooks/pb-cloudflare-pages-setup.md) — Setup de deploy
- [`runbooks/rb-cockpit-release.md`](../runbooks/rb-cockpit-release.md) — Pipeline de release

---

*Documento gerado conforme skill `pop-author`. Próxima revisão: após alteração nos endpoints de health check.*