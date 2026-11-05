# Runbook — Fase 6: LIVE_COMMUNICATION

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Escopo:** Validação, auditoria e entrega da Fase 6 (Sprints 6.1–6.3)
**Público:** Dev Sênior / Dev Pleno / Dev Júnior

---

## 1. Visão Geral da Fase 6

A Fase 6 implementa a camada de comunicação em tempo real do Zylos-Next:
- **Sprint 6.1:** API Gateway Axum + Auth JWT + Rate Limiting + WebSocket + OpenAPI
- **Sprint 6.2:** Cockpit Leptos WASM (dashboard tabs, i18n, theme, LazyRoster, AG-UI streaming, PWA)
- **Sprint 6.3:** Benchmarks cockpit + Segurança OWASP ASVS L2 + Caching HTTP multi-camada
- **Task 5.3.1 (movida):** Multi-Adapter Export (.cursor/.clinerules/.claude-plugin)

### Critérios de Saída (Master Plan)
- [ ] crate `zylos-live-comm` funcional com todos os componentes
- [ ] 50+ agentes sem lag (lazy-roster funcional)
- [ ] **Dupla Auditoria:** `evals/audits/fase6-audit-adversarial.md` + `evals/audits/fase6-audit-isolated.md` ambos APPROVED
- [ ] AG-UI streaming funcional
- [ ] Segurança OWASP Level 2 compliant
- [ ] Documentação completa

---

## 2. Procedimentos por Nível

### 🟢 Dev Júnior — Verificação Básica

**Objetivo:** Confirmar que tudo compila e testes passam.

```bash
# 1. Compilação cross-crate
cd C:/Users/Paulo/zylos-next
cargo check --workspace 2>&1 | tail -20

# 2. Testes zylos-live-comm (Sprint 6.1 + 6.3)
cargo test -p zylos-live-comm 2>&1 | tail -30

# 3. Testes zylos-skills export (Task 5.3.1)
cargo test -p zylos-skills --lib export 2>&1 | tail -20

# 4. Compilação cockpit WASM (Sprint 6.2)
cd apps/zylos-cockpit && cargo check 2>&1 | tail -20
```

**Checklist Júnior:**
- [ ] `cargo check --workspace` exit 0
- [ ] `cargo test -p zylos-live-comm` → todos passam
- [ ] `cargo test -p zylos-skills --lib export` → 7/7 passam
- [ ] `cargo check` cockpit → exit 0
- [ ] Zero errors, apenas warnings de dead code esperados

**Se falhar:** Reportar ao Dev Pleno com mensagem de erro exata. Não tentar corrigir sozinho.

---

### 🟡 Dev Pleno — Validação Funcional e Conferência de Artefatos

**Objetivo:** Validar que cada módulo cumpre seu contrato e que artefatos existem.

#### 2.1 Conferência de Arquivos Existentes

Verificar que todos os arquivos abaixo existem e não estão vazios:

```
crates/zylos-live-comm/src/
├── auth.rs          # JWT middleware
├── rate_limit.rs    # Token bucket rate limiter
├── ws.rs            # WebSocket endpoint
├── openapi.rs       # OpenAPI 3.0 scaffold (utoipa)
├── security.rs      # CSP/HSTS/audit/sanitization
├── cache.rs         # HTTP caching middleware
├── event_bus.rs     # Broadcast event bus
├── agui.rs          # AG-UI event types
├── routes.rs        # SSE + WS router
├── cockpit.rs       # Cockpit bridge
├── hermes_bridge.rs # Hermes integration
├── skills_api.rs    # Skills REST API
├── error.rs         # Error types
└── lib.rs           # Module declarations

apps/zylos-cockpit/src/
├── main.rs          # Crate root + SW registration
├── app.rs           # Dashboard tabs + state machine
├── i18n.rs          # pt-BR/en-US bundles
├── streaming.rs     # AG-UI SSE client
├── benchmarks.rs    # Performance benchmarks
└── components/
    ├── mod.rs
    ├── agent_card.rs
    ├── theme_toggle.rs
    └── lazy_roster.rs

apps/zylos-cockpit/
├── sw.js            # Service worker
├── manifest.json    # PWA manifest
├── index.html       # Entry point + SW registration
├── style.css        # Design tokens
├── Trunk.toml       # Build config + copy directives
└── Cargo.toml       # Dependencies (leptos, web-sys, js-sys)

crates/zylos-skills/src/
└── export.rs        # Multi-adapter export (.cursor/.cline/.claude)
```

#### 2.2 Validação de Contratos

```bash
# Verificar que security headers estão corretos
grep -c "CONTENT_SECURITY_POLICY\|STRICT_TRANSPORT_SECURITY\|X_CONTENT_TYPE_OPTIONS\|X_FRAME_OPTIONS\|REFERRER_POLICY" crates/zylos-live-comm/src/security.rs
# Esperado: 5

# Verificar que cache key inclui query string
grep "?{}" crates/zylos-live-comm/src/cache.rs
# Esperado: 1 match

# Verificar que streaming tem reconexão
grep -c "Reconnecting\|backoff_ms\|max_reconnect_attempts" apps/zylos-cockpit/src/streaming.rs
# Esperado: ≥3

# Verificar que export suporta 3 formatos
grep -c "CursorRules\|ClineRules\|ClaudePlugin" crates/zylos-skills/src/export.rs
# Esperado: ≥6
```

#### 2.3 Contagem de Testes

```bash
# Contar testes por módulo
cargo test -p zylos-live-comm --lib -- --list 2>&1 | grep ": test$" | wc -l
# Esperado: ≥42 (auth + rate_limit + ws + openapi + security + cache + event_bus + agui + routes)

cargo test -p zylos-skills --lib export -- --list 2>&1 | grep ": test$" | wc -l
# Esperado: 7
```

**Checklist Pleno:**
- [ ] Todos os arquivos listados existem e têm conteúdo substancial
- [ ] Security headers: 5 presentes
- [ ] Cache key: inclui `?{}`
- [ ] Streaming: reconexão automática implementada
- [ ] Export: 3 formatos suportados
- [ ] Testes zylos-live-comm: ≥42
- [ ] Testes export: 7
- [ ] PWA: sw.js + manifest.json + registro no index.html

**Se falhar:** Corrigir o gap específico e re-executar verificação. Documentar correção neste runbook.

---

### 🔴 Dev Sênior — Auditoria Adversarial e Isolada

**Objetivo:** Refutar claims, validar segurança, confirmar performance targets.

#### 3.1 Auditoria Adversarial

Criar `evals/audits/fase6-audit-adversarial.md` seguindo o protocolo do PLAYBOOK.md §4.2:

**Claims a refutar:**
1. "Security headers OWASP ASVS L2 compliant" → Verificar cada header contra checklist ASVS 4.0
2. "Cache hit rate >80%" → Analisar implementação: TTL, evicção, bypass para SSE/WS/auth
3. "AG-UI streaming <100ms latency" → Verificar se há medição real ou apenas placeholder
4. "LazyRoster suporta 500+ agentes" → Verificar se virtualização é real ou apenas paginação
5. "PWA offline-first" → Verificar se SW cobre todos os assets críticos
6. "Multi-adapter export sem perda" → Verificar se todos os campos da skill são preservados
7. "Audit log ring buffer" → Verificar se evicção é thread-safe (Mutex vs RwLock)
8. "Input sanitization previne SQLi/XSS" → Testar edge cases não cobertos pelos testes unitários

**Formato do relatório:**
```markdown
# Auditoria Adversarial — Fase 6

## Claim 1: [claim]
- **Evidência:** [código/arquivo/teste]
- **Refutação tentada:** [o que foi testado]
- **Resultado:** CONFIRMED / REFUTED / PARTIAL
- **Detalhes:** [explicação]

## Veredito Final
- Claims confirmadas: X/8
- Claims refutadas: Y/8
- Claims parciais: Z/8
- **APPROVED / REJECTED**
```

#### 3.2 Auditoria Isolada

Criar `evals/audits/fase6-audit-isolated.md`:

**Procedimento:**
1. Clonar repo em diretório limpo
2. Executar `cargo check --workspace` sem cache
3. Executar `cargo test --workspace` sem cache
4. Verificar que zero dependências externas foram adicionadas fora do master plan
5. Verificar que nenhum arquivo temporário/scratch vazou para o repo
6. Confirmar que `cargo clippy --workspace -- -D warnings` passa (ou tem allows justificados)

**Formato:**
```markdown
# Auditoria Isolada — Fase 6

## Ambiente
- OS: Windows 11
- Rust: [version]
- Data: [timestamp]

## Resultados
- cargo check: PASS/FAIL
- cargo test: X passed, Y failed
- cargo clippy: PASS/FAIL (warnings: [...])
- Dependências novas: [lista ou "nenhuma"]
- Arquivos vazados: [lista ou "nenhum"]

## Veredito
- **APPROVED / REJECTED**
```

#### 3.3 Conferência Final de Runbooks

Verificar que este runbook (`runbooks/fase6-live-communication.md`) contém:
- [ ] Visão geral da fase
- [ ] Procedimentos para 3 níveis (Júnior/Pleno/Sênior)
- [ ] Checklist de validação funcional
- [ ] Template de auditoria adversarial
- [ ] Template de auditoria isolada
- [ ] Critérios de saída alinhados com master plan

---

## 3. Matriz de Rastreabilidade

| Task | Módulo | Testes | Audit Adversarial | Audit Isolada |
|------|--------|--------|-------------------|---------------|
| 6.1.1 | auth.rs, rate_limit.rs | ✅ | §3.1 Claim 2 | §3.2 |
| 6.1.2 | ws.rs, event_bus.rs, routes.rs | ✅ | §3.1 Claim 3 | §3.2 |
| 6.1.3 | openapi.rs | ✅ | §3.1 | §3.2 |
| 6.2.1 | app.rs, i18n.rs, components/ | ✅ (check) | §3.1 Claim 5 | §3.2 |
| 6.2.2 | lazy_roster.rs | ✅ (9 tests) | §3.1 Claim 4 | §3.2 |
| 6.2.3 | streaming.rs | ✅ (5 tests) | §3.1 Claim 3 | §3.2 |
| 6.2.4 | sw.js, manifest.json | ✅ (check) | §3.1 Claim 5 | §3.2 |
| 6.3.1 | benchmarks.rs | ✅ (7 tests) | §3.1 Claim 4 | §3.2 |
| 6.3.2 | security.rs | ✅ (10 tests) | §3.1 Claim 1,8 | §3.2 |
| 6.3.3 | cache.rs | ✅ (11 tests) | §3.1 Claim 2 | §3.2 |
| 5.3.1 | export.rs | ✅ (7 tests) | §3.1 Claim 6 | §3.2 |

---

## 4. Lições Aprendidas (atualizar durante execução)

- `LazyRoster` é componente Leptos (função), não tipo construtível com `::new()` — benchmarks devem medir geração/filtragem, não renderização DOM
- `web_sys::Event::target_dyn_into()` não existe; usar `ev.target().and_then(|t| t.dyn_into::<T>().ok())`
- `DomTokenList::remove()` requer `&Array`; usar `remove_1()` para single-token
- Header customizado `Permissions-Policy` requer `HeaderName::from_static()`, não `.parse()`
- Cache key precisa incluir `?` antes do query string (`uri.query()` retorna sem o `?`)
- `write_file` recusa overwrite se arquivo foi lido com pagination; usar `patch` como fallback