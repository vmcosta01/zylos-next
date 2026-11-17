# Fase 11.11 — Auditoria final de segurança (índice)

Branch `feat/f11-security`. Data: 2026-11-17. Seção do plano: `docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md` "## Fase 11.11".

| Tarefa | Relatório | Status |
|---|---|---|
| 11.11.1 OWASP Top 10 | [11.11.1-owasp-top10.md](11.11.1-owasp-top10.md) | Revisão estática feita; pentest dinâmico pendente |
| 11.11.2 JWT | [11.11.2-jwt-review.md](11.11.2-jwt-review.md) | Corrigido (25fe4a5, 35d4770) |
| 11.11.3 Egress | [11.11.3-egress-validation.md](11.11.3-egress-validation.md) | Filtro corrigido (1406acd); não integrado |
| 11.11.4 STRIDE | [11.11.4-stride-threat-model.md](11.11.4-stride-threat-model.md) | Feito |
| 11.11.5 Dependências | [11.11.5-dependency-audit.md](11.11.5-dependency-audit.md) | 48→12 vulns, 4→0 críticas; licenças ok; gate vermelho |
| 11.11.6 Capability tokens | [11.11.6-capability-tokens.md](11.11.6-capability-tokens.md) | Corrigido (5d18743); não integrado nos endpoints |
| 11.11.7 CORS/CSP | [11.11.7-cors-csp-headers.md](11.11.7-cors-csp-headers.md) | Gateway ok (35d4770); cockpit: recomendação |
| 11.11.8 Remediação | [11.11.8-remediation.md](11.11.8-remediation.md) | 3/3 críticos corrigidos; 4 altos abertos |
| 11.11.9 Adversarial | [11.11.9-adversarial-audit.md](11.11.9-adversarial-audit.md) | NÃO APROVADO p/ exposição; condicional p/ loopback |
| 11.11.10 Sign-off | — | Não assinado |

Saídas brutas: `reports/raw/cargo-audit-fase11.txt`, `reports/raw/cargo-audit-after.txt`, `reports/raw/cargo-deny-fase11.txt`, `reports/raw/cargo-deny-after.txt`, `reports/raw/cargo-deny-after-sem-denytoml.txt`.

Nota: os nomes de arquivo exigidos pelo plano (`reports/PENTEST_FASE11.md`, `reports/JWT_SECURITY_REVIEW.md`, …) correspondem aos relatórios acima; este diretório consolida todos.
