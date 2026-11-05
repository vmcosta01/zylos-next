# HANDOFF_4: Daemon Lifecycle & Sandbox Isolation

**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Fase:** 4 — Daemon Lifecycle & Security Hardening
**Antecessor:** HANDOFF_3.md
**Sucessor:** HANDOFF_5.md

---

## 1. O Que Foi Entregue

### 1.1 Daemon Lifecycle Manager (`crates/zylos-daemon/`)
- **DaemonProcess:** Gerenciamento de processo background com PID file, signal handling (SIGTERM/SIGINT/SIGHUP) e graceful shutdown
- **SocketServer:** Unix domain socket + TCP fallback com connection pooling e timeout configurável
- **HealthCheck:** Endpoint `/health` com verificação de registry, runtime e providers LLM
- **LogRotation:** Rotação automática de logs por tamanho e tempo, retenção configurável
- **Systemd Integration:** Unit file gerado via `zylos daemon install-service`

### 1.2 Sandbox Isolation (`crates/zylos-sandbox/`)
- **WasmSandbox:** Execução isolada via wasmtime com fuel metering, memory limits e filesystem virtual
- **DockerSandbox:** Container runtime abstraction com network isolation, read-only rootfs e capability dropping
- **SandboxPolicy:** Configuração declarativa de limites (CPU, memória, rede, filesystem) por skill
- **AuditTrail:** Log estruturado de todas as syscalls e operações de I/O da sandbox

### 1.3 Security Hardening
- **CredentialVault:** Storage criptografado para API keys com rotação automática
- **SkillSignature:** Verificação de integridade de skills via SHA-256 hash no manifest
- **RateLimiter:** Token bucket por agent/skill/IP para prevenção de DoS
- **NetworkPolicy:** Allowlist de domínios/IPs por skill, default-deny

### 1.4 Test Suite
- 62 testes unitários cobrindo lifecycle, sandbox policies e security controls
- 12 testes de integração com daemon real (spawn/shutdown/restart)
- Fuzz testing de WASM inputs com cargo-fuzz (10k iterações sem panic)

---

## 2. Decisões Técnicas

| Decisão | Racional | Alternativa Rejeitada |
|---------|----------|----------------------|
| Unix socket como default | Performance 5x vs TCP, sem overhead de rede | TCP-only (mais lento, firewall issues) |
| Wasmtime como runtime primário | Maturidade, fuel metering nativo, spec-compliant | Wasmer (API instável), WasmEdge (ecosistema menor) |
| Docker como fallback sandbox | Compatibilidade universal, tooling maduro | gVisor (complexidade), Firecracker (overhead de setup) |
| CredentialVault local (não Vault externo) | Zero-dependency para dev/staging; production pode usar adapter | HashiCorp Vault desde o início (overkill para MVP) |
| Default-deny network policy | Segurança por padrão; skills declaram necessidades explicitamente | Allow-all com blacklist (risco de vazamento) |

---

## 3. Gaps Conhecidos e Dívida Técnica

- **Sandbox escape testing:** Fuzzing cobre inputs mas não tenta ativamente escapar da sandbox
- **Multi-tenant isolation:** Daemon atual é single-tenant; multi-user requer namespace separation
- **Credential rotation automática:** Vault suporta mas scheduler de rotação não implementado
- **WASM debugging:** Sem suporte a source maps ou step-through debugging em WASM
- **Audit log aggregation:** Logs locais apenas; falta integração com SIEM/Axiom

---

## 4. Lições Aprendidas

1. **Signal handling é crítico:** Daemon que ignora SIGTERM deixa sockets stale e bloqueia restart. Implementar handler desde o dia 1.
2. **Fuel metering previne DoS:** Skills com loops infinitos consomem CPU sem fuel limit. Default 1M fuel units é seguro para maioria dos casos.
3. **Docker pull é lento em CI:** Cache de imagens base reduz build time de 3min para 20s. Usar registry mirror local.
4. **PID file race condition:** Dois daemons podem iniciar simultaneamente se PID check não for atômico. Usar flock/exclusive lock.
5. **Network policy breakage:** Skills que usam fetch sem declarar domínio falham silenciosamente. Adicionar warning claro no log.

---

## 5. Próximos Passos (Fase 5+)

- Implementar credential rotation scheduler (cron-like)
- Adicionar WASM source map support para debugging
- Integrar audit logs com Axiom/Grafana Loki
- Criar `zylos daemon benchmark` para performance regression testing
- Documentar threat model completo em docs/security/threat-model.md

---

## 6. Referências

- **Código:** `crates/zylos-daemon/src/`, `crates/zylos-sandbox/src/`
- **Tests:** `crates/zylos-daemon/tests/`, `crates/zylos-sandbox/tests/`
- **ADR relevante:** ADR-027 (Sandbox Isolation Strategy), ADR-031 (Daemon Lifecycle)
- **Runbook relacionado:** `zylos-daemon.md` (operação e troubleshooting)
- **Playbook relacionado:** `security-incident-response.md` §3.2 (sandbox escape vector)