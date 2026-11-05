# Playbook: Disaster Recovery — Recuperação de Desastres e Perda Total

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Protocolo estratégico para declaração de desastre, coordenação de recuperação, validação de integridade e comunicação regulatória em cenários de perda total ou parcial de infraestrutura
**Público-alvo:** Tech Lead, SRE, CTO/VP Eng, Security Engineer, Legal/Compliance
**Gatilho:** Perda de datacenter/região cloud, corrupção massiva de dados, ransomware confirmado, falha catastrófica de storage, ou qualquer evento que torne o serviço inoperante por >1h sem perspectiva de restauração via incident response padrão
**Playbooks relacionados:** `security-incident-response.md` (para breaches), `stakeholder-communication.md` (para comunicação externa)

---

## 1. Contexto Organizacional

O Zylos-Next é um sistema de orquestração de agentes IA com os seguintes componentes críticos:

| Componente | RTO | RPO | Dependência Crítica |
|-----------|-----|-----|-------------------|
| zylos-daemon | 15min | 0 (stateless) | Config + binary |
| SQLite DB (registry/skills) | 1h | 5min | WAL + backups |
| Vector Store (embeddings) | 2h | 1h | Index files |
| Cockpit (WASM frontend) | 30min | 0 (static assets) | CDN/build artifacts |
| SSE Gateway | 15min | 0 (stateless) | Daemon health |
| Audit Logs | 4h | 0 (append-only) | Cold storage |

**Missão DR:** Restaurar serviço funcional dentro dos RTOs definidos, garantir zero perda de dados além do RPO aceitável, e produzir evidências auditáveis de integridade pós-recuperação.

**Diferença chave vs. Incident Response:** IR lida com degradação parcial ou breach contido. DR lida com **perda total ou indisponibilidade prolongada** onde procedimentos operacionais normais não são suficientes.

---

## 2. Papéis e Responsabilidades

| Papel | Responsabilidade | Decisão |
|-------|-----------------|---------|
| **DR Coordinator** (Tech Lead ou SRE Senior) | Declarar desastre, coordenar timeline, aprovar cada fase | Go/No-go entre fases |
| **Infra Lead** | Executar recuperação de infra (provisionamento, restore) | Timing técnico de cada componente |
| **Data Integrity Owner** | Validar integridade de DB, vector store e audit logs | Aceitação ou rejeição de restore |
| **Security Engineer** | Verificar ausência de comprometimento antes do restore | Clean bill of health |
| **Communications Lead** | Notificar stakeholders internos/externos por timeline definida | Timing e conteúdo de comunicações |
| **Legal/Compliance** | Avaliar obrigações regulatórias (LGPD, DORA, contratos enterprise) | Notificação a autoridades |
| **Executive Sponsor** (CTO/VP) | Aprovar budget emergencial, decisões de arquitetura sob pressão | Escalação final |

---

## 3. Fluxo de Decisão Estratégica

### 3.1 Quando Declarar Desastre

Declare DR quando QUALQUER condição for verdadeira E procedimentos de IR falharem ou forem insuficientes:

- [ ] Datacenter/região cloud totalmente indisponível por >30min sem ETA de restauração
- [ ] Corrupção confirmada de >50% dos dados do registry/vector store
- [ ] Ransomware ou wiper confirmado com criptografia/deleção em massa
- [ ] Backup mais recente também corrompido ou inacessível
- [ ] SLA enterprise violado sem perspectiva de resolução em <2h
- [ ] Infraestrutura crítica (DNS, certificates, secrets vault) comprometida irrecuperavelmente

### 3.2 Matriz de Decisão por Cenário

| Cenário | Fase Inicial | RTO Target | Comunicar Em | Aprovador |
|---------|-------------|------------|-------------|-----------|
| Região cloud down | F1: Failover para região secundária | 30min | Imediato | DR Coordinator |
| Corrupção de DB | F2: Restore de backup + validação | 2h | 15min após confirmação | Data Integrity Owner |
| Ransomware confirmado | F3: Isolar → Forense → Restore limpo | 4h | Imediato + legal | Security Eng + Executive |
| Secrets vault perdido | F4: Regenerar credenciais + rotacionar | 1h | Após rotação completa | Security Eng |
| Perda total (multi-falha) | F5: Rebuild from scratch + restore seletivo | 8h | Contínuo a cada 30min | Executive Sponsor |

### 3.3 Critérios de Entrada e Saída

**Entrada:** Declaração formal de desastre pelo DR Coordinator + notificação inicial enviada.

**Saída:** Serviço declarado recuperado quando TODOS forem verdadeiros:
- [ ] Todos os componentes dentro do RTO definido
- [ ] Dados restaurados dentro do RPO definido
- [ ] Validação de integridade aprovada pelo Data Integrity Owner
- [ ] Security Engineer confirma ambiente limpo
- [ ] Comunicação de resolução enviada a todos os stakeholders
- [ ] Post-mortem agendado para <72h

---

## 4. Governança e Escalação

### 4.1 Níveis de Decisão DR

| Nível | Quem Decide | Exemplo | Tempo Máximo |
|-------|-------------|---------|--------------|
| L1 — Failover automático | SRE on-call | Switch para região secundária (pré-configurado) | Imediato |
| L2 — Restore de backup | DR Coordinator + Infra Lead | Escolher ponto de restore, validar integridade | 30min |
| L3 — Rebuild parcial | Tech Lead + Security Eng | Reconstruir componente com nova arquitetura | 2h |
| L4 — Rebuild total + pivot | Executive Sponsor + Tech Lead | Mudar provider, aceitar perda parcial, comunicar clientes | 1h após declaração |

### 4.2 Protocolo de Escalação DR

1. **Fase estourou RTO em >50%** → Escalar para próximo nível + notificar Executive Sponsor
2. **Dados restaurados falham validação de integridade** → Parar restore, acionar Data Integrity Owner + Security Eng
3. **Novo comprometimento detectado durante recovery** → Pausar recovery, voltar para IR playbook, reassess
4. **Comunicação regulatória necessária** → Legal/Compliance assume timing; nunca comunicar sem aprovação legal

### 4.3 War Room e Comunicação Interna

- **Canal primário:** `#zylos-dr-war-room` (Slack/Teams) — acesso restrito a participantes DR
- **Bridge de voz:** Call permanente aberta durante F3-F5
- **Status updates:** A cada 30min no war room, mesmo se "sem novidades"
- **Decisões documentadas:** Toda decisão L2+ registrada em thread dedicada com timestamp e aprovador

---

## 5. Ferramentas e Sistemas Necessários

| Ferramenta | Uso | Acesso |
|------------|-----|--------|
| Terraform / Pulumi | Provisionamento de infra em nova região/provider | Infra Lead, SRE |
| Restic / BorgBackup | Restore de backups criptografados | Infra Lead |
| `sqlite3` + `integrity_check` | Validação de DB restaurado | Data Integrity Owner |
| `zylos-cli` | Health checks e validação funcional | Todos |
| Grafana / Prometheus | Monitoramento pós-restore | SRE |
| Cold Storage (S3/GCS WORM) | Backups imutáveis e audit logs | Infra Lead |
| HashiCorp Vault / AWS SM | Rotação de secrets pós-desastre | Security Eng |
| Slack / Email / StatusPage | Comunicação interna e externa | Communications Lead |
| Jira / Linear | Tracking de tarefas DR | Todos |

---

## 6. Fases de Recuperação

### F1: Failover para Região Secundária
**Runbooks acionados:** `deploy-cicd.md` (§3.1 staging deploy adaptado), `monitoring-observability.md` (§3.1 health queries)
**Validação:** Health check OK em nova região + métricas baseline atingidas em <15min

### F2: Restore de Backup + Validação
**Runbooks acionados:** `zylos-daemon.md` (§4.1 start pós-restore), `monitoring-observability.md` (§3.2 log queries para verificar integridade)
**Validação:** `PRAGMA integrity_check` retorna `ok` + amostra aleatória de 100 registros válida

### F3: Isolar → Forense → Restore Limpo (Ransomware)
**Runbooks acionados:** `security-incident-response.md` (contenção), novo runbook `forensic-evidence-collection.md` (gap §4.4), `deploy-cicd.md` (§3.3 hotfix clean build)
**Validação:** Scan de malware limpo + hash comparison de binaries contra known-good + Security Eng sign-off

### F4: Regenerar Credenciais + Rotacionar
**Runbooks acionados:** `zylos-daemon.md` (restart com novas creds), `deploy-cicd.md` (deploy com novos secrets)
**Validação:** Todos os serviços autenticam com sucesso + zero credenciais antigas ativas

### F5: Rebuild From Scratch + Restore Seletivo
**Runbooks acionados:** Todos os runbooks em sequência + `dev-onboarding-offboarding.md` (para provisionar acessos emergenciais)
**Validação:** Full system test suite passa + UAT com cliente enterprise (se aplicável)

---

## 7. Comunicação Regulatória e Compliance

### 7.1 LGPD (Brasil)
- **Notificação à ANPD:** Obrigatória em caso de vazamento de dados pessoais dentro de prazo razoável
- **Conteúdo mínimo:** Natureza dos dados afetados, medidas técnicas adotadas, riscos aos titulares, medidas de mitigação
- **Owner:** Legal/Compliance + Security Engineer

### 7.2 DORA (UE - Digital Operational Resilience Act)
- **Relatório de incidente grave:** Dentro de 4h da classificação como grave
- **Conteúdo:** Timeline, impacto operacional, causa raiz preliminar, medidas corretivas
- **Owner:** Compliance + Tech Lead

### 7.3 Contratos Enterprise
- **SLA breach notification:** Conforme contrato individual (geralmente <24h)
- **Direito de auditoria:** Cliente pode solicitar evidências de recovery
- **Owner:** Customer Success + Legal

---

## 8. Pós-Desastre

1. **Post-mortem obrigatório** em <72h com todos os participantes DR
2. **Atualizar este playbook** com lições aprendidas
3. **Revisar RTOs/RPOs** com base em performance real da recuperação
4. **Teste de DR agendado** em <90 dias para validar correções
5. **Auditoria de compliance** se houve notificação regulatória
6. **Revisão de custos** com Financeiro (DR geralmente gera custo emergencial significativo)

---

## 9. Referências Cruzadas

- **Runbooks executáveis:** `deploy-cicd.md` · `zylos-daemon.md` · `monitoring-observability.md` · `zylos-cli.md` · `zylos-cockpit.md`
- **Playbooks relacionados:** `security-incident-response.md` · `stakeholder-communication.md` · `capacity-planning.md`
- **SOAR:** `soar/SOAR_ARCHITECTURE.md` (WF-001 breach containment pode escalar para DR)
- **Gap Analysis:** `GAPS_ANALYSIS_RUNBOOKS_PLAYBOOKS.md` §4.3 (origem deste documento)
- **Referências externas:** Cutover (DR playbooks), Belago (cyber recovery), TecnetOne (runbook/playbook segurança)

---

**Próxima Revisão Agendada:** Fevereiro de 2027
**Owner:** Tech Lead / SRE Zylos-Next
**Teste de DR Agendado:** Trimestralmente (próximo: Janeiro de 2027)