# PB-Sentry-Integration — Configuração de Error Tracking no Gateway e Cockpit

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.4 T11.4.4
**Público:** Dev Pleno, SRE, Tech Lead
**Tempo Estimado:** 15–30 minutos (setup inicial)

---

## 1. Visão Geral

Este playbook define o procedimento para integrar o Sentry como sistema de error tracking no gateway Zylos (Rust/Axum) e no cockpit (WASM/Leptos), habilitando captura automática de panics, erros HTTP e stack traces em produção.

**Gatilho de ativação:** Primeiro deploy em produção ou após incidente não capturado.

---

## 2. Procedimento de Setup

### Passo 1: Criar projeto no Sentry (Manual)

1. Acesse https://sentry.io → Create Project
2. Plataforma: **Rust** (para gateway) e **JavaScript/WASM** (para cockpit)
3. Nome: `zylos-gateway` e `zylos-cockpit`
4. Copiar DSN gerado para cada projeto

### Passo 2: Configurar DSN no Gateway (Automatizável)

O gateway já lê `SENTRY_DSN` via env var. Configure no ambiente de produção:

```bash
# Linux/macOS
export SENTRY_DSN="https://<key>@<host>/<project_id>"

# Windows PowerShell
$env:SENTRY_DSN = "https://<key>@<host>/<project_id>"

# Docker/Kubernetes
env:
  - name: SENTRY_DSN
    valueFrom:
      secretKeyRef:
        name: zylos-secrets
        key: sentry-dsn
```

**Sem DSN:** O gateway inicia normalmente com log `"Sentry disabled (SENTRY_DSN not set)"`.

### Passo 3: Validar integração local (Automatizável)

```bash
# Iniciar gateway com Sentry
SENTRY_DSN="https://test@sentry.io/test" cargo run -p zylos-gateway

# Verificar log de inicialização
# Esperado: "Sentry initialized for error tracking"

# Testar captura de erro (endpoint que retorna 500)
curl http://localhost:3000/api/v1/nonexistent
# Verificar dashboard do Sentry → novo evento registrado
```

### Passo 4: Integrar cockpit WASM (Futuro)

Para capturar erros no cockpit WASM, adicionar `console_error_panic_hook` + integração manual com Sentry JS SDK:

```rust
// Em apps/zylos-cockpit/src/main.rs
console_error_panic_hook::set_once();
// Futuro: wasm-bindgen bindings para Sentry JS SDK
```

---

## 3. Configurações Aplicadas

| Configuração | Valor | Racional |
|-------------|-------|----------|
| `traces_sample_rate` | 0.1 (10%) | Balanceia custo vs visibilidade em produção |
| `release` | Auto-detected via `sentry::release_name!()` | Vincula eventos a versão específica |
| Panic hook | Automático via feature `panic` | Captura panics Rust não tratados |
| Backtrace | Habilitado via feature `backtrace` | Stack traces completos em eventos |

---

## 4. Troubleshooting

| Sintoma | Causa Provável | Solução |
|---------|---------------|---------|
| Log "Sentry disabled" | `SENTRY_DSN` não configurado ou vazio | Verificar env var no ambiente |
| Eventos não aparecem no dashboard | DSN incorreto ou projeto errado | Validar DSN copiando do Sentry UI |
| Panics não capturados | Feature `panic` não habilitada | Verificar `Cargo.toml`: `features = ["panic"]` |
| Stack traces incompletos | Build sem debug info | Usar `strip = false` em profile release (ou source maps separados) |
| Rate limit excedido | Volume alto de erros | Ajustar `traces_sample_rate` ou filtrar ruído |

---

## 5. Referências

- [`apps/zylos-gateway/src/main.rs`](../apps/zylos-gateway/src/main.rs) — Inicialização do Sentry
- [`apps/zylos-gateway/Cargo.toml`](../apps/zylos-gateway/Cargo.toml) — Dependências `sentry` + `sentry-tracing`
- [Sentry Rust SDK Docs](https://docs.sentry.io/platforms/rust/)
- [`pop-health-check-validation.md`](../runbooks/pop-health-check-validation.md) — Validação de endpoints

---

*Documento gerado conforme skill `playbook-author`. Próxima revisão: após primeiro evento capturado em produção.*