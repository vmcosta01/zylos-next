# Onboarding de Operadores — Cockpit Zylos (Jornada de 4 Semanas)

**Fase:** 11.6.2 · **Data:** 2026-11-17 · **Versão:** 1.0
**Público:** novos operadores (on-call, SRE, analistas de aprovação HITL)
**Pré-leitura:** [USER_GUIDE.md](USER_GUIDE.md) (com screenshots reais), [FAQ.md](FAQ.md)

> **Meta de aceitação (plano Fase 11.6):** um operador novo executa o **workflow básico em < 1 hora** (Dia 1, abaixo).
> Todos os arquivos citados existem no repositório; caminhos relativos à raiz do repo.

---

## 0. Antes do Dia 1 (responsável: mentor)

- [ ] Acesso de leitura ao repositório e ao canal de incidentes
- [ ] Máquina com Rust stable (o repo fixa via `rust-toolchain.toml`) e `trunk` instalado, **ou** acesso a um ambiente com o gateway já rodando
- [ ] Mentor designado + 1 sessão de shadowing on-call agendada para a Semana 3

**Importante:** o gateway só aceita requisições desta máquina (`apps/zylos-gateway/src/local_guard.rs`, middleware `so_desta_maquina`). O operador opera o cockpit **na mesma máquina** do gateway, ou via túnel/reverse proxy configurado pela infra (ver `runbooks/zylos-gateway.md`).

---

## Semana 1 — Fundamentos e workflow básico

### Dia 1: workflow básico em < 1h (cronometrado)

| Min | Passo | Comando / ação | Resultado esperado |
|-----|-------|----------------|--------------------|
| 0–15 | Subir gateway | `cargo run -p zylos-gateway` (raiz do repo) | Log "Gateway configuration loaded"; escuta `127.0.0.1:3000` |
| 15–20 | Health | `curl http://127.0.0.1:3000/api/v1/health` | `{"status":"ok","service":"zylos-gateway",...}` |
| 20–25 | Liveness/readiness | `curl http://127.0.0.1:3000/gateway/live` e `/gateway/ready` | `{"status":"alive"}` / `{"status":"ready"}` |
| 25–35 | Subir cockpit | `cd apps/zylos-cockpit && trunk serve --port 3001` | Cockpit em `http://localhost:3001` |
| 35–45 | Tour de tabs | Visite **Agentes, Logs, SOAR, Triagem, Canais** | Header mostra Status `ok` e versão do gateway |
| 45–55 | Simular falha | Pare o gateway (Ctrl+C) e clique **"Tentar novamente"** | Banner "Connection lost" (igual a `docs/assets/screenshots/00-offline-gateway.png`) |
| 55–60 | Recuperar | Suba o gateway de novo → **"Tentar novamente"** | Dashboard volta |

> Atalho Windows/git-bash: `scripts/zylos-start.sh` sobe gateway (`./target/debug/zylos-gateway.exe`) + cockpit (porta 3001) e `scripts/zylos-stop.sh` derruba ambos. Requer build prévio (`cargo build -p zylos-gateway`) e o arquivo `.zylos-hermes-token` na raiz (exporta `ZYLOS_HERMES_TOKEN`, que habilita as rotas `/api/v1/hermes/*`).

**Gate do Dia 1:** operador demonstra subir, verificar health, reconhecer o estado offline e recuperar, sem ajuda.

### Dias 2–5: leitura dirigida

| Dia | Ler | Objetivo |
|-----|-----|----------|
| 2 | [USER_GUIDE.md](USER_GUIDE.md) §3–§5 | Conhecer as 31 tabs e o que é dado real vs demo |
| 3 | `runbooks/zylos-gateway.md`, `runbooks/zylos-cockpit.md` | Operação dos dois componentes |
| 4 | `runbooks/pop-health-check-validation.md`, `docs/runbooks/operational-runbook.md` | Rotina de verificação |
| 5 | `playbooks/pop-gateway-restart.md` | Primeiro procedimento de recuperação |

**Gate Semana 1:** quiz de 10 perguntas tiradas do [FAQ.md](FAQ.md) (≥ 8 acertos) + executar `playbooks/pop-gateway-restart.md` em ambiente local.

---

## Semana 2 — Operação diária (HITL, Triagem, Evidências)

| Tema | Tab no cockpit | Procedimento |
|------|----------------|--------------|
| Decidir aprovações HITL | HitlApproval (`tab.hitl_approval`) | `playbooks/pop-approval-decide.md`, `runbooks/rb-zylos-approval.md` |
| Triagem de inbox | Triagem | `playbooks/false-positive-suppression.yaml` para falsos positivos |
| Consultar o ledger de evidências | EvidenceLedger | `playbooks/pop-evidence-ledger-query.md`, `runbooks/evidence-ledger.md` |
| Exportar audit log | — | `playbooks/pop-audit-log-export.md`, `runbooks/rb-zylos-audit-log.md` |
| Canais | Canais | `runbooks/rb-zylos-channels.md`, `docs/pop-channel-recovery.md` |

**Exercícios:**
1. Liste canais via API: `curl http://127.0.0.1:3000/api/v1/channels`.
2. Consulte estatísticas: `curl http://127.0.0.1:3000/api/v1/evidence/stats` e `/api/v1/sentinel/stats`.
3. Registre em um ticket de treino uma decisão HITL simulada seguindo `playbooks/pop-approval-decide.md` (com justificativa).

**Gate Semana 2:** operador decide 5 aprovações simuladas com justificativa auditável; mentor revisa.

---

## Semana 3 — Monitoramento, alertas e incidentes

| Tema | Referência |
|------|-----------|
| Sentinel (AIOps) | `runbooks/sentinel-aiops.md`, `playbooks/pop-sentinel-alert-investigation.md`, `runbooks/rb-sentinel-threshold-tuning.md` |
| Alertas de produção (SOAR) | `docs/soar/soar-production-alerts.md`, `docs/soar/SOAR_ARCHITECTURE.md` |
| Resposta a incidente | `runbooks/rb-production-incident-response.md`, `playbooks/incident-response.md` |
| On-call | `docs/playbooks/on-call-playbook.md` |
| Observabilidade | `docs/runbooks/monitoring-observability.md`, `playbooks/pb-axiom-logging-setup.md`, `playbooks/pb-sentry-integration.md` |
| Egress / segurança | `playbooks/pop-egress-false-positive.md`, `runbooks/rb-zylos-egress.md` |

**Exercícios:**
1. **Tabletop:** mentor descreve "gateway caiu às 03h". Operador conduz `runbooks/rb-production-incident-response.md` em voz alta: detecção → contenção (`playbooks/pop-gateway-restart.md`) → comunicação (`docs/playbooks/stakeholder-communication.md`) → pós-mortem.
2. **Shadowing:** acompanhar 1 turno on-call real.

**Gate Semana 3:** tabletop concluído sem pular etapas; operador sabe quando escalar (matriz de `docs/playbooks/on-call-playbook.md`).

---

## Semana 4 — Emergências, backup e autonomia

| Tema | Referência |
|------|-----------|
| Backup/restore | `runbooks/backup-restore.md`, `playbooks/pop-backup-restore.md` |
| Disaster recovery | `playbooks/disaster-recovery.md`, `docs/playbooks/disaster-recovery.md` |
| Revogar tokens | `runbooks/pop-token-revoke.md`, `playbooks/pop-relay-token-revoke.md` |
| Parada de emergência | `playbooks/pop-relay-emergency-stop.md`, `playbooks/pop-computer-control-emergency.md` |
| Fallback de modelo | `playbooks/pop-model-fallback.md` |
| Incidente de segurança | `docs/playbooks/security-incident-response.md`, `playbooks/security-incident-response.yaml` |

**Exercícios:**
1. Backup e restore em ambiente local seguindo `runbooks/backup-restore.md` (os SQLite ficam em `data/`, ex.: `data/channels.db`).
2. Executar `playbooks/pop-relay-emergency-stop.md` em modo simulado.

**Gate final (graduação):** operador conduz sozinho um turno de acompanhamento com mentor apenas observando; zero passos de runbook pulados.

---

## Checklist de graduação

- [ ] Workflow básico do Dia 1 em < 1h (tempo registrado: ____ min)
- [ ] Quiz FAQ ≥ 8/10
- [ ] 5 decisões HITL revisadas
- [ ] Tabletop de incidente concluído
- [ ] Backup/restore executado
- [ ] 1 turno solo acompanhado

## Feedback (alimenta a Fase 11.6.6)

Ao final, o operador registra onde travou e qual doc estava errada/ausente. Esses relatos são insumo para `docs/DOC_VALIDATION_REPORT.md` (Fase 11.6.6, ainda não executada).

## Limitações conhecidas (2026-11-17)

- O gateway de produção ainda não está deployado; o preview do cockpit roda sem backend (ver USER_GUIDE §1).
- Tela de login (`auth_login.rs`) existe mas não está montada no `App`.
- 6 tabs mostram rótulos crus de i18n (`tab.goals` etc.).
