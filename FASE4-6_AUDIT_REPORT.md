# Relatório de Auditoria: Fases 4-6 do zylos-next

**Data:** 2026-11-03
**Master Plan:** zylos-next-master-plan.md (linhas 1130-1934)
**Crates Auditados:** zylos-sandbox, zylos-skills, zylos-live-comm, zylos-mobile

---

## Resumo Executivo

| Crate | Fase | Tasks Planejadas | Implementado | Gaps Críticos |
|-------|------|-----------------|--------------|---------------|
| zylos-sandbox | 4 | 6 tasks (4.1.1–4.2.3) | ⚠️ ~40% | Host functions, manifest, pooling, overlayfs, TLS matching, CAPTCHA |
| zylos-skills | 5 | N/A | ❌ **NÃO EXISTE** | Crate não encontrado no workspace |
| zylos-live-comm | 6 | 9 tasks (6.1.1–6.3.3) | ✅ ~70% | Frontend Leptos, MFA/WebAuthn, Merkle audit, ABAC, pentest contínuo |
| zylos-mobile | 6 | Componente mobile | ✅ MCP server | Documentação, testes, features mobile-specific |

---

## Gaps por Prioridade

### 🔴 CRÍTICO (bloqueia entrega da fase)

1. **zylos-skills NÃO EXISTE** — Crate listado na auditoria não existe no workspace.
   - Verificar se foi renomeado, movido, ou fundido em outro crate
   - Master Plan Fase 5 referencia: registry.rs, schema.rs, eval.rs, debt.rs, export.rs, glossary.rs, scheduler.rs
   - Possíveis candidatos: zylos-soar (tem registry.rs), zylos-bot-system

2. **WASM Frontend (Leptos 0.7) — Task 6.2.1** — 48h estimadas, não implementado
   - Component library, dashboard persistente, tema dark/light, i18n, PWA, streaming integration
   - Sem este componente, Cockpit visual não funciona

3. **Host Functions Registry — Task 4.1.1** — Macro `#[host_function]` e audit log ausentes
   - WASM modules não podem chamar funções Rust do host sem capability validation

4. **Module Manifest Schema — Task 4.1.1** — TOML/YAML declarando capabilities ausente
   - Sem manifest, capability-based security é manual e error-prone

### 🟡 ALTO (degrada significativamente a entrega)

5. **MFA/WebAuthn/FIDO2 — Task 6.3.2** — Apenas JWT RS256 implementado
   - OWASP ASVS Level 2 exige MFA para operações sensíveis

6. **Merkle Tree Audit Log — Task 6.3.2** — Audit log imutável ausente
   - Compliance e forensics dependem de integridade verificável

7. **ABAC Hybrid Policy Engine — Task 6.3.2** — Apenas RBAC 5-tier
   - Attribute-based policies (tenant_id, resource_owner, time_of_day) não suportadas

8. **Docker OverlayFS/AppArmor/SELinux — Task 4.1.3** — Filesystem hardening ausente
   - Containers usam bind mounts simples, sem isolation reforçada

9. **Image Digest Pinning — Task 4.1.3** — Imagens por tag, não SHA256
   - Supply chain attack vector aberto

10. **Sandbox Pooling — Task 4.1.1** — Cold-start optimization ausente
    - Target <5ms com cache; atual provavelmente >50ms

### 🟢 MÉDIO (melhora qualidade/compliance)

11. **Continuous Pentest Pipeline — Task 6.3.2** — ZAP/Burp/Semgrep/gitleaks não integrados
12. **STRIDE Threat Model Document — Task 6.3.2** — Attack surface mapping ausente
13. **TLS Fingerprint Matching — Task 4.2.1** — JA3/JA4 evasion não implementado
14. **Mouse/Keyboard Humanization — Task 4.2.1** — Behavioral analysis evasion ausente
15. **CAPTCHA Solving Pipeline — Task 4.2.1** — 2captcha/hCaptcha integration ausente
16. **Secret Injection Segura — Task 4.1.3** — Vault/encrypted env vars ausente
17. **Artifact Extraction + Malware Scan — Task 4.1.3** — Post-execution handling ausente
18. **Compiled Artifact Cache — Task 4.1.1** — Module::deserialize para cold-start ausente
19. **Benchmark Suite Consolidado — Task 6.3.1** — cargo bench + playwright + k6 incompleto
20. **Lazy-Roster 50+ Agentes — Task 6.2.2** — Virtualização de lista ausente

---

## Recomendações

1. **Localizar ou criar zylos-skills** — Verificar se funcionalidade foi distribuída em outros crates (zylos-soar, zylos-bot-system)
2. **Priorizar WASM Frontend** — Task 6.2.1 é prerequisite para demonstração do Cockpit
3. **Implementar host functions + manifest** — Foundation para capability-based security real
4. **Adicionar MFA antes de production** — JWT-only é insuficiente para OWASP Level 2
5. **Criar docs/COCKPIT.md e docs/SECURITY.md** — Task 6.3.3 exige documentação completa

---

## Arquivos Gerados

- `crates/zylos-sandbox/README.md` — Descrição, features, API pública, status vs plano, gaps
- `crates/zylos-live-comm/README.md` — Descrição, features, API pública, status vs plano, gaps
- `crates/zylos-mobile/README.md` — Descrição, features, API pública, status vs plano, gaps
- Este relatório consolidado
