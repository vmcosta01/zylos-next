# Segurança Camada Zero — Zylos-Next v6.0

## Princípio Fundamental
Segurança é definida **ANTES** de qualquer código. Nenhuma feature pode ser implementada sem que seus requisitos de segurança estejam documentados e aprovados neste arquivo. Violação = trabalho inválido.

## Threat Model

### Assets Críticos
| Asset | Sensibilidade | Proteção Requerida |
|-------|--------------|-------------------|
| Agent System Prompts | Alta | Encryption at rest, access logging |
| Memory Context Graph | Alta | Scope isolation, audit trail |
| API Keys / Tokens | Crítica | Env vars only, never logged |
| User PII | Crítica | Data minimization, retention policy |
| Workflow State | Média | Shadow-git integrity checks |
| Verification Proofs | Alta | Immutable storage, tamper detection |

### Attack Vectors
1. **Prompt Injection**: Malicious input in agent context → Guardrail validation obrigatória
2. **Sandbox Escape**: WASM/Docker breakout → Capability-based permissions, no raw host access
3. **Memory Poisoning**: Corrupted context graph entries → Integrity hashes, version rollback
4. **Authorization Bypass**: Direct object reference attacks → Server-side ownership verification
5. **Supply Chain**: Malicious skill/crate → Schema validation + cargo-audit CI gate
6. **Data Exfiltration**: Agent leaking secrets → Output sanitization, network egress control

## Rules de Acesso (RBAC Enforcement)

### Matriz de Roles e Permissões

| Role | Read | Write | Admin | Deploy | Audit |
|------|------|-------|-------|--------|-------|
| viewer | ✅ | ❌ | ❌ | ❌ | ❌ |
| editor | ✅ | ✅ | ❌ | ❌ | ❌ |
| admin | ✅ | ✅ | ✅ | ✅ | ✅ |
| auditor | ✅ | ❌ | ❌ | ❌ | ✅ |

### Regras de Acesso Server-Side (Obrigatórias)

1. **Todo endpoint DEVE validar role antes de processar** — middleware `require_role()` em Axum
2. **Cross-tenant isolation enforced via tenant_id em todo query** — nunca confiar em session apenas
3. **API keys scoped por role + expiry** — rotação automática a cada 90 dias
4. **Audit log imutável para toda operação write/admin** — append-only via SQLite WAL
5. **Rate limiting por role**: viewer=100/min, editor=500/min, admin=unlimited (com alert)
6. **Permission cache TTL ≤ 60s** — invalidation broadcast via event bus on role change
7. **Deny-by-default**: endpoints sem annotation explícita retornam 403

### Enforcement Técnico

```rust
// Middleware obrigatório em todo handler protegido
#[require_role(Role::Editor)]
async fn create_agent(...) -> Result<Response> { ... }

// Validação compile-time via macro
role_check!(user, Role::Admin, "deploy requires admin");
```

## Secrets Policy

### Gestão de Secrets

1. **Zero secrets em código-fonte** — `.env.example` apenas com placeholders; CI falha se detectar secret real via `gitleaks`
2. **Secrets via vault externo** — HashiCorp Vault ou AWS Secrets Manager; nunca env vars em produção
3. **Secret rotation automática** — máximo 90 dias; zero-downtime via dual-key durante transição
4. **Secret scoping por ambiente** — dev/staging/prod com namespaces isolados no vault
5. **Access logging para todo secret retrieval** — quem acessou qual secret quando; alert on anomalia
6. **Encryption at rest** — AES-256-GCM para secrets em DB; key management via KMS
7. **Secret injection em runtime apenas** — nunca persistir decrypted secret em disco/log/memory dump
8. **Pre-commit hook `gitleaks detect`** — bloqueia commit com pattern de secret (AWS key, JWT, password)
9. **CI scan obrigatório** — `cargo-audit` + `trivy` + `gitleaks` em todo PR; fail on HIGH/CRITICAL
10. **Incident response**: secret leak → revoke immediate + rotate dependent keys + post-mortem em 24h

### Key Management

- **Master keys**: gerenciadas via KMS (AWS/Azure/GCP); never exportable
- **Data encryption keys (DEK)**: geradas por envelope encryption; encrypted by master key
- **Key derivation**: HKDF-SHA256 para context-specific keys (por tenant, por agent)
- **Key versioning**: suporta múltiplas versões ativas durante rotação; deprecated keys read-only por 30 dias

## Regras Enforceáveis

### Secrets Management
- **NUNCA** hardcode secrets em código fonte
- **SEMPRE** usar environment variables via `.env.example` como template
- **SEMPRE** validar ausência de secrets via `gitleaks` pre-commit hook
- Rotação automática: tokens com TTL máximo de 24h para agentes efêmeros
- Logging: REDACT todos os campos sensíveis automaticamente

### Authorization & Authentication
- **SEMPRE** verificar authorization server-side (frontend é apenas UX)
- **SEMPRE** validar ownership antes de qualquer operação mutável
- Roles: `admin`, `operator`, `viewer` — enforced por middleware Axum
- Session tokens: JWT signed com HS256, expiry 1h, refresh token rotation
- Rate limiting: 100 req/min por user, 1000 req/min por IP

### Sandboxing Obrigatório
- Tools de terceiros: **SEMPRE** executar em WASM sandbox (wasmtime)
- Agentes com acesso a rede/filesystem: **SEMPRE** isolar via Docker container
- Browser automation: fingerprint engine anti-detect obrigatório
- Capability grants: whitelist explícita por agente (princípio do menor privilégio)
- Resource limits: CPU/memory caps via cgroups (Docker) ou WASM linear memory limit

### Input Validation
- **SEMPRE** validar inputs externos com serde + jsonschema ANTES de processar
- String lengths: max limits definidos por campo (ex: prompt 8KB, name 64 chars)
- Unicode normalization: NFC obrigatório para evitar homoglyph attacks
- SQL injection: rusqlite prepared statements apenas (zero string interpolation)
- Path traversal: canonical path validation + chroot/jail quando aplicável

### Data Protection
- Encryption at rest: SQLite WAL com SEE extension (AES-256) para dados sensíveis
- Encryption in transit: TLS 1.3 obrigatório para todas as conexões externas
- Retention policy: logs retidos 30 dias, memory context 90 dias, proofs indefinidamente
- Right to deletion: API endpoint `/api/v1/gdpr/delete-user` com cascade completo
- Backup encryption: AES-256-GCM para snapshots de banco

### Observability de Segurança
- Audit log: toda operação mutável registrada com timestamp, user, resource, outcome
- Anomaly detection: baseline de comportamento por agente, alertas em desvios >3σ
- Failed auth tracking: lockout após 5 falhas consecutivas (exponential backoff)
- Secret access logging: todo read de secret gera evento de auditoria
- Compliance reports: export mensal em formato SOC2/ISO27001

## Pipeline de Deploy Seguro
```
LOCAL → PREVIEW → QA → PROD
```

### LOCAL
- Dev environment com secrets mock (.env.local)
- Hot reload ativo, debug symbols enabled
- Testes unitários + integration rodando localmente

### PREVIEW
- Branch-specific deployment automático
- Secrets reais via vault (HashiCorp/AWS Secrets Manager)
- Smoke tests + security scan (cargo-audit + trivy)
- Approval manual necessário para promover

### QA
- Staging environment idêntico ao prod
- Load testing (k6) com baseline de performance
- Penetration testing automatizado (OWASP ZAP)
- Sign-off de security team obrigatório

### PROD
- Blue-green deployment com rollback automático
- Canary releases (5% → 25% → 100%)
- Monitoring ativo (Prometheus + Grafana alerts)
- Incident response playbook documentado

## Multiusuário (Conceito #13)
- Tenant isolation: cada organização tem namespace separado no SQLite
- Row-level security: queries filtradas por tenant_id automaticamente
- Cross-tenant access: bloqueado por default, requer approval explícito
- Audit trail: log de acessos cross-tenant (mesmo que negados)
- Data residency: opção de pinning geográfico para compliance regional

## Incident Response
1. **Detection**: Alerta automático ou report manual
2. **Containment**: Isolar agente/sistema comprometido (<5 min)
3. **Eradication**: Remover root cause + patch de segurança
4. **Recovery**: Restore de backup validado + verify integrity
5. **Lessons**: Post-mortem documentado em `DECISIONS.md`

## Compliance Checklist
- [ ] OWASP Top 10 mitigations documentadas
- [ ] GDPR data processing register mantido
- [ ] SOC2 Type II controls implemented
- [ ] PCI-DSS scope assessment (se aplicável)
- [ ] Third-party vendor security review

## Referências
- [Manifesto Conceito #12](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md)
- [AGENTS.md](./AGENTS.md) para regras de código seguro
- [ARCHITECTURE.md](./ARCHITECTURE.md) para threat model técnico