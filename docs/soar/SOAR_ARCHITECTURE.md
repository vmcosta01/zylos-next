# SOAR Zylos — Security Orchestration, Automation and Response

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Arquitetura, workflows e integração do SOAR com runbooks e playbooks existentes
**Público-alvo:** Devs pleno/senior, SREs e analistas de segurança
**Gatilho de execução:** Novo incidente de segurança, alerta de anomalia, ou necessidade de automação de resposta

---

## 1. Visão Geral

O SOAR Zylos é a camada de orquestração que conecta **detecção** (monitoring/observability) → **decisão** (playbooks) → **execução** (runbooks) → **auditoria** (logs imutáveis). Ele elimina a lacuna entre documentação estratégica e ação técnica, garantindo que incidentes de segurança sejam respondidos de forma consistente, rápida e rastreável.

### 1.1 Posicionamento na Arquitetura Zylos

```
┌─────────────────────────────────────────────────────┐
│                  DETECÇÃO                            │
│  Monitoring · AIOps · Alertas · Anomaly Detection    │
└──────────────────────┬──────────────────────────────┘
                       │ Evento/Alerta
                       ▼
┌─────────────────────────────────────────────────────┐
│                   SOAR ZYLOS                         │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ Triage Auto │→ │ Playbook     │→ │ Runbook    │ │
│  │ (Classifica)│  │ (Decide)     │  │ (Executa)  │ │
│  └─────────────┘  └──────────────┘  └────────────┘ │
│         │                │                 │        │
│         ▼                ▼                 ▼        │
│  ┌─────────────────────────────────────────────────┐│
│  │          AUDITORIA IMUTÁVEL                      ││
│  │  Timeline · Ações · Decisões · Timestamps        ││
│  └─────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│              PÓS-INCIDENTE                           │
│  Postmortem · Métricas · Melhoria Contínua           │
└─────────────────────────────────────────────────────┘
```

### 1.2 Princípios de Design

1.  **Human-in-the-loop para decisões críticas:** Automação total apenas para triagem e contenção de baixo risco. Isolamento de produção, exclusão de dados e comunicação externa exigem aprovação humana.
2.  **Playbooks governam, runbooks executam:** O SOAR nunca executa lógica de negócio diretamente; ele invoca runbooks existentes via interface padronizada.
3.  **Auditoria como dado primário:** Cada ação gera um evento imutável antes da execução, não depois. Logs são append-only e assinados.
4.  **Fail-safe over fail-fast:** Em caso de dúvida ou erro, o SOAR pausa e escala para humano, nunca tenta "adivinhar" a próxima etapa.
5.  **Integração nativa com ecossistema Zylos:** Usa `zylos daemon` como barramento de eventos, `zylos agent` como executores, e `zylos cockpit` como dashboard de incidentes.

---

## 2. Componentes do SOAR Zylos

### 2.1 Event Intake & Normalization

Recebe alertas de múltiplas fontes e normaliza para um schema comum (`ZylosSecurityEvent`).

**Fontes suportadas:**
- Prometheus/Grafana alerts (via webhook)
- Zylos daemon health checks
- OSINT feeds (CVEs, threat intel)
- Logs estruturados (via syslog/fluentd)
- Integrações futuras: SIEM externo, EDR, cloud providers

**Schema normalizado:**
```rust
pub struct ZylosSecurityEvent {
    pub id: Uuid,
    pub source: String,           // "prometheus", "daemon", "osint"
    pub severity: Severity,       // Critical, High, Medium, Low, Info
    pub category: EventCategory,  // Breach, Anomaly, Vulnerability, Compliance
    pub title: String,
    pub description: String,
    pub affected_assets: Vec<String>,
    pub raw_payload: serde_json::Value,
    pub received_at: DateTime<Utc>,
}
```

### 2.2 Auto-Triage Engine

Classifica eventos automaticamente usando regras + heurísticas leves (sem LLM nesta camada para garantir determinismo).

**Regras de triagem:**
| Severidade | Categoria | Ação Automática | Playbook Acionado |
|-----------|-----------|-----------------|-------------------|
| Critical | Breach | Isolar ativo + notificar on-call + abrir incidente P1 | `security-incident-response` |
| High | Anomaly | Enriquecer contexto + atribuir para análise em 15min | `security-incident-response` |
| Medium | Vulnerability | Criar ticket + vincular ao `vendor-dependency-pivot` se aplicável | `vendor-dependency-pivot` |
| Low | Compliance | Log + métrica + revisão semanal | Nenhum (batch processing) |
| Any | False Positive Detectado | Suprimir + ajustar regra + log de aprendizado | Nenhum |

### 2.3 Playbook Orchestrator

Interpreta playbooks como **máquinas de estado executáveis**, não documentos estáticos.

**Formato de playbook executável (YAML):**
```yaml
name: security-incident-response
version: "1.0"
triggers:
  - event.category == "Breach" && event.severity == "Critical"
roles:
  incident_commander: "on-call-security-lead"
  executor: "zylos-agent-soar"
  approver: "security-manager"
states:
  - name: triage
    auto: true
    runbook: null  # usa auto-triage engine
    next:
      - condition: "triage.confirmed_breach == true"
        to: containment
      - condition: "triage.false_positive == true"
        to: closed
  - name: containment
    auto: false  # requer aprovação humana
    approval_required: true
    runbook: "isolate-compromised-host"
    timeout_minutes: 10
    escalation: "incident-commander"
    next:
      - to: eradication
  - name: eradication
    auto: true
    runbook: "remove-malware-and-restore"
    next:
      - to: recovery
  - name: recovery
    auto: true
    runbook: "validate-system-integrity"
    next:
      - to: post_incident
  - name: post_incident
    auto: true
    actions:
      - generate_postmortem
      - update_metrics
      - notify_stakeholders  # usa playbook stakeholder-communication
    terminal: true
```

### 2.4 Runbook Executor

Interface padronizada para invocar qualquer runbook existente como uma **unidade atômica executável**.

**Contrato de execução:**
```rust
pub trait SoarRunnable {
    /// Executa o runbook e retorna resultado estruturado
    async fn execute(&self, context: IncidentContext) -> Result<RunbookResult>;

    /// Valida pré-condições antes da execução
    async fn validate_preconditions(&self, context: &IncidentContext) -> Result<()>;

    /// Retorna rollback procedure se execução falhar
    fn rollback(&self) -> Option<Box<dyn SoarRunnable>>;
}
```

**Mapeamento runbooks existentes → SOAR:**
| Runbook Existente | Tipo SOAR | Gatilho Automático |
|------------------|-----------|-------------------|
| `zylos-cli.md` | Recovery | CLI inoperante durante incidente |
| `zylos-daemon.md` | Recovery | Daemon crash ou health check falho |
| `zylos-cockpit.md` | Communication | Dashboard indisponível durante crise |
| `deploy-cicd.md` | Containment/Rollback | Deploy malicioso ou comprometido detectado |
| `monitoring-observability.md` | Detection | Próprio SOAR consome métricas deste runbook |
| `memory-consolidation.runbook.yaml` | Maintenance | Memory decay threshold ou schedule |
| `dag-execution.runbook.yaml` | Orchestration | Bot task queued ou checkpoint stale |
| `verification-proof.runbook.yaml` | Verification | Proof request ou panel vote |
| `sandbox-management.runbook.yaml` | Security | Sandbox spawn ou health degraded |
| `skills-registry.runbook.yaml` | Maintenance | Skill registered ou eval failed |
| `live-comm-gateway.runbook.yaml` | Recovery | Gateway health fail ou WS disconnect |
| `mobile-mcp.runbook.yaml` | Sync | Mobile device connected ou sync failed |
| `cortex-patterns.runbook.yaml` | Verification | Cortex verification ou cerebelo stale |
| `hitl-approval.runbook.yaml` | Approval | Approval pending ou TTL expiring |
| `task-durability.runbook.yaml` | Recovery | Lease expired ou circuit breaker tripped |

### 2.5 Audit Trail & Evidence Store

Registro imutável de todas as ações do SOAR. Requisito para compliance (LGPD, DORA, ISO 27001).

**Campos obrigatórios por evento de auditoria:**
- Timestamp UTC com precisão de ms
- ID do incidente correlacionado
- Ator (humano ou agente automatizado)
- Ação executada (com parâmetros sanitizados)
- Resultado (sucesso/falha/timeout)
- Hash do estado anterior e posterior (para integridade)
- Assinatura digital do evento (HMAC-SHA256 com chave rotativa)

**Storage:** Append-only log no `zylos daemon` + export periódico para cold storage (S3/GCS com WORM).

---

### 2.6 Novos Componentes — Fases 7B/7C

Os seguintes componentes foram integrados ao ecossistema SOAR nas Fases 7B e 7C:

#### Event Bus (zylos-live-comm)
Barramento de eventos interno baseado em AG-UI protocol. Publica eventos `ApprovalRequested`, `TaskCompleted`, `HealthDegraded` para subscribers SOAR. WebSocket gateway em `localhost:8765`.

#### Gateway (zylos-live-comm)
HTTP/WebSocket gateway para comunicação em tempo real. Health checks em `/health`, drain graceful de conexões, integração com HITL approval cards.

#### Mobile MCP (zylos-mobile)
Model Context Protocol para dispositivos móveis. Multi-device sync com resolução de conflitos latest-writer, registro automático de dispositivos.

#### HITL Approval Engine (zylos-hitl + zylos-task-durability::approval_hash)
Human-in-the-loop via AG-UI protocol. Storage server-side por referência (never by value), CAS claim atômico, TTL 60min, equivalence-based hashing (blake3 em `zylos-task-durability::approval_hash`), four-tier decision (safety > team > personal > auto > default), 7 mensagens de recusa distintas. Validation probe task-local implementado. AG-UI protocol bindings pendentes (Task 7C.1.1).

#### Sentinel Health Scanner (zylos-sentinel)
AIOps health scanner standalone crate com detectors pure functions (http_5xx, timeout, repeated_failures), normalized signatures (SHA-256 truncado 128-bit), privacy-first PII stripping (emails, UUIDs, IPv4, hex tokens), JSONL append-only logging, token bucket rate limiter 5 incidents/hour. 21 testes passando.

#### Triage Engine (zylos-soar::triage_message)
Two-stage message triage movido para `zylos-soar`. Stage 1 rules-only (safe negatives apenas, NEVER produz NeedsYou), Stage 2 model fallback. `classify_message()` + `route_message()` com campo `decided_by` auditável (`"rule:<id>"` | `"model:<name>"`), framing pattern detection, initiator tracking (Person/Routine/Responsibility/Memory/Handoff/System). 20 testes passando. Legacy `zylos-triage` crate mantido como referência histórica.

#### Proof Engine (zylos-proof)
Motor de honestidade determinística. Ground enum 5 níveis, Evidence struct, ProofBuilder, VerificationGate. Zero LLM calls.

#### Resolver DAG (zylos-resolver)
Fronteira BFS sobre DAG com checkpoint por onda (JSON atômico), paralelismo opt-in, glue proofs entre sub-objetivos, resume anti-forja.

#### Panel Adversarial (zylos-panel)
Painel adversarial determinístico com guardrails, detector de falso-sucesso, filtro de auto-julgamento, aggregate_votes onde 1 refutação aterrada derruba maioria, lentes canônicas (funcionalidade, estrutura, performance, segurança, simplicidade, testes).

#### Router Aterrado (zylos-router)
Roteamento com coleira (leash), alvos sagrados, proposta de mudança com requires_human=true, fallback general-purpose, is_real() garantia dura.

#### Cerebelo (zylos-cerebelo)
Checkpoint/rollback com shadow-git, audit chain SHA-256 encadeada, test-ratchet, rollback escopado a arquivos tracked.

#### Learn Loop (zylos-learn)
Loop Aprender→Persistir→Aplicar com classificação de desfechos, assinatura normalizada, gate de recorrência ≥3×, witness anti-regressão determinístico.

#### Task Durability (zylos-task-durability)
SQL leases com database-as-clock, SELECT FOR UPDATE SKIP LOCKED, LostLeaseError, heartbeat lease_ms/3, maintenance loop 60s, circuit breaker 5 falhas, 100-task active cap, pg_notify inside transaction.

## 3. Workflows Pré-Construídos

### 3.1 WF-001: Resposta a Breach Confirmado

```
Alerta Crítico → Auto-Triage → Confirma Breach → [APROVAÇÃO HUMANA]
    → Isolar Host (runbook) → Remover Malware (runbook)
    → Validar Integridade (runbook) → Gerar Postmortem
    → Notificar Stakeholders (playbook) → Fechar Incidente
```

**SLA:** Contenção em <15min após aprovação. Resolução completa em <4h.

### 3.2 WF-002: CVE Crítica em Dependência Direta

```
OSINT Feed → Match com Cargo.toml → Classificar Severidade
    → Se CVSS >= 9.0: Bloquear deploy + criar ticket urgente
    → Acionar playbook vendor-dependency-pivot
    → Monitorar patch availability a cada 30min
    → Quando disponível: executar deploy-cicd runbook (hotfix branch)
    → Validar + Notificar
```

**SLA:** Avaliação em <5min. Hotfix deployado em <2h se patch existir.

### 3.3 WF-003: Anomalia de Comportamento de Agente

```
Monitoring detecta desvio >3σ no consumo de recursos de agente
    → Auto-Triage: classificar como Anomaly/High
    → Coletar métricas dos últimos 30min (runbook monitoring)
    → Se padrão conhecido: aplicar correção automática
    → Se desconhecido: escalar para analista + preservar estado
    → Post-análise: atualizar regras de triagem ou baseline
```

**SLA:** Detecção em <2min. Diagnóstico inicial em <10min.

---

## 4. Integração com Ferramentas Existentes

### 4.1 Zylos Daemon

O daemon atua como **barramento de eventos interno** do SOAR:
- Recebe eventos normalizados via endpoint `/soar/events`
- Distribui para subscribers (orchestrator, audit store, cockpit)
- Garante entrega at-least-once com deduplicação por event ID

### 4.2 Zylos Cockpit

Dashboard dedicado no cockpit para operações SOAR:
- **Incident Timeline:** Visualização temporal de todos os eventos e ações
- **Active Incidents:** Lista filtrável por severidade, status, assignee
- **Runbook Execution Status:** Tempo real de cada etapa em andamento
- **Audit Explorer:** Busca e export de logs de auditoria
- **Playbook Editor:** Interface visual para criar/editar playbooks YAML

### 4.3 Zylos CLI

Comandos SOAR expostos via CLI para operadores:
```bash
zylos soar incidents list --status active
zylos soar incidents show <id>
zylos soar approve <incident-id> --action containment
zylos soar reject <incident-id> --reason "false positive"
zylos soar audit search --from 2026-11-01 --category breach
zylos soar playbook validate ./path/to/playbook.yaml
zylos soar test-run <playbook-name> --dry-run
```

---

## 5. Segurança do Próprio SOAR

O SOAR é alvo prioritário para atacantes. Proteções obrigatórias:

1.  **Assinatura de Playbooks:** Todo playbook YAML deve ser assinado com chave GPG do security team. Playbooks não assinados são rejeitados.
2.  **RBAC Granular:** Permissões separadas para criar playbooks, aprovar ações, executar runbooks e acessar audit logs.
3.  **Rate Limiting:** Máximo de 10 execuções automáticas/minuto por categoria. Excedente pausa e escala.
4.  **Kill Switch Global:** Comando `zylos soar halt` para parar toda automação imediatamente. Requer autenticação multifator.
5.  **Isolamento de Rede:** O orchestrator roda em namespace/container separado, sem acesso direto à internet. Apenas o event intake tem entrada externa.
6.  **Secrets Management:** Credenciais de runbooks armazenadas em vault (HashiCorp/AWS Secrets Manager), nunca em texto plano ou variáveis de ambiente.

---

## 6. Roadmap de Implementação

### Fase 1: Fundação (4 semanas)
- [ ] Schema `ZylosSecurityEvent` e event intake via daemon
- [ ] Auto-Triage Engine com regras básicas
- [ ] Audit Trail append-only com HMAC
- [ ] Integração CLI básica (`soar incidents list/show`)

### Fase 2: Orquestração (6 semanas)
- [ ] Playbook Orchestrator com máquina de estados
- [ ] Adapter para runbooks existentes (`SoarRunnable` trait)
- [ ] Aprovação humana via CLI + cockpit
- [ ] WF-001 (Breach) e WF-002 (CVE) funcionais

### Fase 3: Visibilidade e Refinamento (4 semanas)
- [ ] Dashboard SOAR no cockpit
- [ ] WF-003 (Anomalia de Agente)
- [ ] Export de audit logs para cold storage
- [ ] Testes de caos e validação de SLAs

### Fase 4: AIOps e Evolução (contínuo)
- [ ] Integração com LLM para enriquecimento de contexto (não para decisão)
- [ ] Aprendizado de regras de triagem baseado em feedback humano
- [ ] Correlação automática entre incidentes relacionados
- [ ] Benchmark contra frameworks NIST/MITRE ATT&CK

---

## 7. Critérios de Validação do SOAR

Após implementação de cada fase, confirme:

- [ ] Eventos são normalizados e persistidos em <500ms
- [ ] Auto-triage classifica corretamente >95% dos eventos de teste
- [ ] Playbooks executam na ordem definida, respeitando gates humanos
- [ ] Audit trail contém 100% das ações com timestamps e hashes válidos
- [ ] Kill switch paralisa automação em <2s
- [ ] Runbooks existentes funcionam via SOAR sem modificação de código
- [ ] Cockpit exibe timeline e status em tempo real (<3s de latência)
- [ ] Teste de penetração no próprio SOAR não encontra vulnerabilidades críticas

---

## 8. Referências Cruzadas

- **Runbooks executáveis:** [`zylos-cli.md`](./runbooks/zylos-cli.runbook.yaml) · [`zylos-daemon.md`](./runbooks/zylos-daemon.runbook.yaml) · [`deploy-cicd.md`](./runbooks/deploy-cicd.runbook.yaml) · [`monitoring-observability.md`](./runbooks/monitoring-observability.runbook.yaml)
- **Runbooks SOAR Fases 7B/7C:** [`memory-consolidation.runbook.yaml`](./runbooks/memory-consolidation.runbook.yaml) · [`dag-execution.runbook.yaml`](./runbooks/dag-execution.runbook.yaml) · [`verification-proof.runbook.yaml`](./runbooks/verification-proof.runbook.yaml) · [`sandbox-management.runbook.yaml`](./runbooks/sandbox-management.runbook.yaml) · [`skills-registry.runbook.yaml`](./runbooks/skills-registry.runbook.yaml) · [`live-comm-gateway.runbook.yaml`](./runbooks/live-comm-gateway.runbook.yaml) · [`mobile-mcp.runbook.yaml`](./runbooks/mobile-mcp.runbook.yaml) · [`cortex-patterns.runbook.yaml`](./runbooks/cortex-patterns.runbook.yaml) · [`hitl-approval.runbook.yaml`](./runbooks/hitl-approval.runbook.yaml) · [`task-durability.runbook.yaml`](./runbooks/task-durability.runbook.yaml)
- **Playbooks governados:** [`security-incident-response.md`](../playbooks/security-incident-response.md) · [`vendor-dependency-pivot.md`](../playbooks/vendor-dependency-pivot.md) · [`stakeholder-communication.md`](../playbooks/stakeholder-communication.md)
- **Handoffs arquiteturais:** [`HANDOFF_4.md`](../handoffs/HANDOFF_4.md) (Daemon Lifecycle) · [`HANDOFF_5.md`](../handoffs/HANDOFF_5.md) (Live Communication) · [`HANDOFF_6.md`](../handoffs/HANDOFF_6.md) (Fase 6→7B) · [`HANDOFF_7.md`](../handoffs/HANDOFF_7.md) (Fase 7B→7C) · [`fase6-to-7b-handoff.md`](../handoffs/fase6-to-7b-handoff.md) · [`fase7b-to-7c-handoff.md`](../handoffs/fase7b-to-7c-handoff.md)
- **Referências externas:** Cutover (Runbook vs Playbook) · Belago (SOAR no SOC) · Elastic (AIOps for Observability) · TecnetOne (Definições de Runbook/Playbook de Segurança)