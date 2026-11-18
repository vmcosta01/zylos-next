# Security — Zylos Cockpit v2.0 Frontend

**Versão:** 2.0.0
**Data:** 2026-11-18
**Status:** Contrato de Segurança Frontend
**Conceito Manifesto:** #12 (Segurança como camada zero)
**Depende de:** [PRD.md](./PRD.md), [ARCHITECTURE.md](./ARCHITECTURE.md)

---

## 1. Threat Model Frontend

| # | Ameaça | Vetor | Impacto | Mitigação |
|---|--------|-------|---------|-----------|
| S1 | XSS em approval cards | HTML malicioso em description field | Execução de script no contexto do usuário | Ammonia sanitization + CSP |
| S2 | Token theft via localStorage | XSS ou extensão maliciosa | Session hijacking | Auth token em httpOnly cookie apenas |
| S3 | PII leakage via chat/egress | Usuário cola dados sensíveis | Vazamento para LLM/serviços externos | Egress checker síncrono (ADR-002) |
| S4 | CSRF em ações state-changing | Link malicioso clickado | Aprovação/rejeição não-autorizada | SameSite cookies + origin validation |
| S5 | Clickjacking | iframe embedding | UI spoofing | X-Frame-Options: DENY |
| S6 | VNC stream interception | MITM em WebSocket | Screen capture não-autorizado | WSS obrigatório; cert pinning futuro |
| S7 | Dependency supply chain | Crate/npm comprometido | Código malicioso em bundle | cargo-audit + deny.toml + SRI |
| S8 | DOM clobbering | IDs globais colidindo | Logic bypass | Namespace isolation; evitar window.* |

---

## 2. Input Validation (Client-Side)

### 2.1 Regras Gerais
- **Nunca confiar em validação client-side como única defesa** — server-side é autoridade
- Validar ANTES de enviar para reduzir round-trips desnecessários
- Feedback imediato ao usuário (inline validation, não apenas on submit)
- Sanitizar TODO conteúdo renderizado como HTML

### 2.2 Campos Específicos

| Campo | Validação | Sanitização | Erro UX |
|-------|-----------|-------------|---------|
| Search query | Max 500 chars; regex válido | Escape HTML entities | Warning inline se regex inválido |
| Approval decision | Enum: approve/reject/escalate | N/A | Disabled button se estado inválido |
| Settings form fields | Type-specific (email, URL, number) | Trim whitespace | Inline error abaixo do campo |
| Command palette input | Max 200 chars | Strip null bytes | Silent truncation |
| VNC session ID | UUID format validation | N/A | Error state + mensagem clara |

### 2.3 Egress Checker (ADR-002)
```rust
// Pseudo-code: egress check antes de qualquer POST/WS send
fn check_egress(text: &str) -> Result<(), EgressViolation> {
    // Patterns compilados em build time
    static PATTERNS: &[Regex] = &[
        PII_EMAIL, PII_CPF, PII_PHONE, SECRET_API_KEY, SECRET_BEARER
    ];
    
    for pattern in PATTERNS {
        if let Some(m) = pattern.find(text) {
            return Err(EgressViolation {
                kind: pattern.kind(),
                position: m.range(),
                redacted: redact(text, m.range())
            });
        }
    }
    Ok(())
}
```

- Latência alvo: <5ms para textos <10KB
- Falso positivo: POP de ajuste de regra + whitelist contextual
- Backend re-verifica como segunda linha de defesa

---

## 3. HTML Sanitization (XSS Prevention)

### 3.1 Biblioteca: ammonia (WASM)
- Whitelist-based sanitizer
- Zero unsafe code
- Bundle overhead: ~15KB gzipped
- Configuração customizada para approval cards

### 3.2 Política de Sanitization

```rust
let policy = ammonia::Builder::default()
    .tags(&["p", "br", "strong", "em", "code", "pre", "ul", "ol", "li"])
    .tag_attributes("a", &["href"])
    .link_rel(Some("noopener noreferrer"))
    .url_schemes(&["https"])  // Bloqueia javascript:, data:, etc.
    .strip_comments(true)
    .build();

let safe_html = policy.clean(untrusted_html);
```

### 3.3 Onde Aplicar
- **InlineApprovalCard.description** — SEMPRE sanitizar
- **AgentCard.notes** — se renderizar markdown/html
- **Toast messages** — se contiver conteúdo dinâmico
- **Memory graph labels** — se user-generated
- **NUNCA** usar `dangerouslySetInnerHTML` ou equivalente Leptos sem sanitization

---

## 4. Content Security Policy (CSP)

### 4.1 Headers (Configurados no Gateway Axum)

```
Content-Security-Policy: 
  default-src 'self';
  script-src 'self' 'wasm-unsafe-eval';
  style-src 'self' 'unsafe-inline' https://fonts.googleapis.com;
  font-src 'self' https://fonts.gstatic.com;
  img-src 'self' data: blob:;
  connect-src 'self' ws: wss:;
  frame-ancestors 'none';
  base-uri 'self';
  form-action 'self';
  upgrade-insecure-requests;
```

### 4.2 Justificativas

| Diretiva | Valor | Razão |
|----------|-------|-------|
| script-src | 'wasm-unsafe-eval' | WASM instantiation requer eval-like behavior |
| style-src | 'unsafe-inline' | CSS variables dinâmicas via style attribute |
| font-src | fonts.gstatic.com | Inter font via Google Fonts CDN |
| img-src | data: blob: | Screenshots VNC, QR codes gerados localmente |
| connect-src | ws: wss: | WebSocket para VNC e SSE |
| frame-ancestors | 'none' | Previne clickjacking |

### 4.3 Report-Only Mode
- Inicialmente deploy com `Content-Security-Policy-Report-Only`
- Monitorar violations por 1 semana
- Promover para enforcement após zero violations legítimas

---

## 5. Token Storage

### 5.1 Política

| Tipo de Token | Armazenamento | Razão |
|---------------|--------------|-------|
| Auth JWT/session | httpOnly cookie (Secure, SameSite=Strict) | Inacessível via JS; previne XSS theft |
| CSRF token | Cookie não-httpOnly + header duplo | Double-submit cookie pattern |
| User preferences | localStorage | Não-sensível; sobrevivência a refresh OK |
| Sidebar state | localStorage | UX preference; não-sensível |
| Approval receipts | localStorage | Cache de UI; TTL 24h; não-authoritative |
| VNC lease token | Memory only (Signal) | Short-lived; não persistir |

### 5.2 Regras Absolutas
- **NUNCA** armazenar auth tokens em localStorage
- **NUNCA** logar tokens em console.log
- **NUNCA** passar tokens via URL query params
- Cookies devem ter: `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/api`

---

## 6. WebSocket Security (VNC)

### 6.1 Connection Hardening
- **WSS obrigatório** em produção; WS apenas em dev local
- Origin validation no gateway: reject conexões de origins desconhecidos
- Rate limiting: max 5 connections/session/minuto
- Frame size limit: 4MB max; rejeitar frames maiores
- Idle timeout: 5min sem atividade → close connection

### 6.2 Lease Management
- Lease token: cryptographically random UUID
- Auto-release: 5min inatividade OU explicit release
- Single-writer: apenas um lease ativo por session
- Lease revocation: admin pode forçar release via API

### 6.3 Binary Frame Validation
- Magic bytes check: JPEG frames devem começar com `FF D8 FF`
- Dimension validation: width/height dentro de limites razoáveis (max 4K)
- Corrupt frame handling: skip + log, nunca crash

---

## 7. Dependency Security

### 7.1 Rust Crates
```toml
# deny.toml
[advisories]
db-path = "~/.cargo/advisory-db"
vulnerability = "deny"
unmaintained = "warn"
yanked = "deny"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "ISC"]
copyleft = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-git = []
```

### 7.2 CI Pipeline
- `cargo audit` em todo PR
- `cargo deny check` em todo PR
- Dependabot/Renovate para updates automáticos
- Review manual obrigatório para novas dependências

### 7.3 WASM-Specific
- Verificar crates WASM-compatible não usam syscalls
- Auditar bindings web-sys/gloo-net para surface area mínima
- Preferir crates com `#![forbid(unsafe_code)]`

---

## 8. Secure Development Practices

### 8.1 Code Review Checklist
- [ ] Todo HTML dinâmico passa por ammonia?
- [ ] Tokens sensíveis fora de localStorage?
- [ ] CSP headers configurados corretamente?
- [ ] Input validation presente antes de network calls?
- [ ] Error messages não vazam stack traces/internals?
- [ ] Secrets fora de código fonte (.env.example apenas)?
- [ ] Dependencies auditadas?

### 8.2 Pre-commit Hooks
```bash
#!/bin/bash
# .git/hooks/pre-commit
grep -rn 'localStorage.setItem.*token\|localStorage.setItem.*key\|localStorage.setItem.*secret' src/ && exit 1
grep -rn 'dangerouslySetInnerHTML\|set_inner_html_unchecked' src/ && exit 1
grep -rn '#[0-9a-fA-F]\{6\}' src/ && echo "WARNING: Hardcoded color detected" 
cargo audit --quiet || exit 1
```

### 8.3 Incident Response
1. Detectar via AIOps sentinel ou report externo
2. Conter: feature flag disable se aplicável
3. Investigar: logs, scope, affected users
4. Remediar: patch + advisory
5. Comunicar: changelog + security advisory se necessário

---

## 9. Compliance com Manifesto

| Requisito | Status |
|-----------|--------|
| Segurança definida ANTES do código | ✅ Este documento |
| Validação de input client-side | ✅ Seção 2 |
| Sanitização HTML (XSS) | ✅ Seção 3 |
| CSP headers | ✅ Seção 4 |
| Token storage seguro | ✅ Seção 5 |
| .env.example (não secrets em código) | ✅ Já existe no repo |
| Camada zero, não afterthought | ✅ Integrado em todas as fases |

---

## 10. Changelog

| Versão | Data | Mudança |
|--------|------|---------|
| 2.0.0 | 2026-11-18 | Initial v2.0 frontend security; threat model; CSP; token policy; egress checker |