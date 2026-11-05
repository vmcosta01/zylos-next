# Playbook de Resposta a Incidentes — Zylos-Next

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Escopo:** Detecção, triagem, contenção e recuperação de falhas em produção
**Público:** Tech Lead, SRE, Agente Orquestrador, Devs Pleno/Júnior
**Runbooks Relacionados:** `runbooks/zylos-gateway.md`, `runbooks/zylos-docker.md`

---

## 1. Contexto Organizacional

### Missão do Incident Response
Garantir que qualquer falha no ecossistema Zylos seja detectada, contida e resolvida dentro dos SLAs definidos, minimizando impacto em agentes ativos e preservando integridade de dados.

### Valores Aplicados
- **Prova sobre Promessa:** Nenhum incidente é "resolvido" sem métrica de recuperação validada
- **Segurança Camada Zero:** Contenção antes de investigação; isolamento antes de debug
- **Documentação Viva:** Post-mortem obrigatório em até 24h após resolução

### Papéis no Incidente

| Papel | Responsabilidade | Decisões | Escala Para |
|-------|-----------------|----------|-------------|
| **Incident Commander (IC)** | Coordenação geral, comunicação com stakeholders | Declarar severidade, aprovar rollback | Tech Lead (sev-1 >30min) |
| **SRE / Ops Lead** | Execução técnica de contenção e recuperação | Restart de serviços, failover de DB | IC (mudanças destrutivas) |
| **Agente Orquestrador** | Triagem automática, execução de runbooks de recovery | Retry de tasks, re-spawn de containers | IC (falha recorrente) |
| **Dev Júnior** | Execução guiada de runbooks, coleta de logs | Comandos de diagnóstico pré-aprovados | SRE (desvio do runbook) |
| **Comms Lead** | Atualização de status page, notificação de usuários | Timing de comunicação externa | Product Owner (impacto >1k usuários) |

---

## 2. Fluxo Estratégico de Resposta

### 2.1 Classificação de Severidade

| Severidade | Definição | Tempo de Resposta | Tempo de Resolução | Exemplo |
|------------|-----------|-------------------|--------------------|---------|
| **SEV-1** | Sistema indisponível, perda de dados, breach de segurança | <5 min | <1h | Gateway down, sandbox escape |
| **SEV-2** | Funcionalidade crítica degradada, múltiplos agentes afetados | <15 min | <4h | Memory system lento (>500ms), Docker containers órfãos |
| **SEV-3** | Funcionalidade não-crítica quebrada, workaround disponível | <1h | <24h | OCR falhando para SVG específico, skill discovery lenta |
| **SEV-4** | Bug cosmético, performance sub-ótima, melhoria | <4h | Próximo sprint | Log excessivo, UI desalinhada no cockpit |

### 2.2 Matriz de Decisão Rápida

```
ALERTA RECEBIDO
    ↓
É SEV-1 ou SEV-2? ──SIM──→ Acionar IC imediatamente + Canal #incidentes
    │                           ↓
    NÃO                      Iniciar Runbook de Diagnóstico
    ↓                           ↓
Classificar como SEV-3/4     Contenção funciona? ──NÃO──→ Escalar para SEV-2
    ↓                           ↓
Criar task no Kanban         SIM → Monitorar 15min → Resolver → Post-mortem
    ↓
Atribuir ao responsável
```

### 2.3 Critérios de Escalação Automática

| Condição | Ação Automática | Notificação |
|----------|-----------------|-------------|
| Health check falha 3x consecutivas | Restart do serviço via runbook | Canal #ops-alerts |
| Latência p99 >2x baseline por 5min | Throttle automático + alerta SEV-3 | IC on-call |
| Container crash loop (>3 restarts/min) | Kill + bloqueio de criação nova | SRE + Security |
| Erro de auditoria adversarial em prod | Bloqueio de deploy imediato | Tech Lead |
| Uso de RAM >80% por >10min | Scale-up ou shed de carga não-crítica | Ops Lead |

---

## 3. Governança e Comunicação

### 3.1 Canais de Comunicação

| Canal | Uso | Quem Participa |
|-------|-----|----------------|
| `#incidentes` (Slack/Discord) | Coordenação em tempo real SEV-1/2 | IC, SRE, Tech Lead |
| `#ops-alerts` | Alertas automáticos, SEV-3/4 | Todos os devs |
| Status Page (cockpit/status) | Comunicação externa | Usuários finais |
| Post-mortem doc | Registro permanente | Toda equipe |

### 3.2 Template de Comunicação Externa

```
[STATUS] Incidente #{ID} — {Título Curto}
Severidade: {SEV-N}
Início: {HH:MM UTC}
Status: {Investigando | Contido | Recuperando | Resolvido}
Impacto: {Descrição clara do que está afetado}
Próxima atualização: {HH:MM UTC ou "em X minutos"}
```

### 3.3 Regras de Post-Mortem

1. **Obrigatório** para todo SEV-1 e SEV-2
2. **Prazo:** 24h após resolução
3. **Formato:** 5 Whys + Timeline + Action Items + Owner + Deadline
4. **Blameless:** Foco em sistema/processo, nunca em pessoa
5. **Revisão:** Tech Lead aprova antes de arquivar
6. **Follow-up:** Action items viram tasks no Kanban com prioridade alta

---

## 4. Ferramentas e Recursos

### 4.1 Stack de Observabilidade

| Ferramenta | Uso | Acesso |
|-----------|-----|--------|
| Tracing (tracing-subscriber) | Logs estruturados por agente/sessão | stdout/journald |
| Metrics (prometheus-client) | Latência, throughput, erro rate | `/metrics` endpoint |
| Cockpit Dashboard | Visualização em tempo real | `http://localhost:3000` |
| SQLite Audit Log | Eventos de segurança e estado | `zylos_audit` table |
| Hermes Notifications | Alertas via messaging bots | Connector configurado |

### 4.2 Runbooks de Recuperação Rápida

| Cenário | Runbook | Tempo Estimado |
|---------|---------|----------------|
| Gateway indisponível | `runbooks/zylos-gateway.md` §Troubleshooting | <5min |
| Container órfão/vazamento | `runbooks/zylos-docker.md` §Troubleshooting | <3min |
| Browser headless travado | `runbooks/zylos-browser.md` §Troubleshooting | <5min |
| MCP server não responde | `runbooks/zylos-mobile.md` §Troubleshooting | <3min |
| OCR falhando | `runbooks/zylos-vlm-ocr.md` §Troubleshooting | <5min |
| Sandbox escape suspeito | `specs/SECURITY-MODEL.md` §Incident Response | <10min |

### 4.3 Acessos Necessários

- SSH/bastion para servidores de produção
- Acesso admin ao Cockpit
- Permissão de restart em systemd/docker
- Leitura do SQLite audit log
- API keys de observabilidade (se cloud)

---

## 5. Adaptação por Nível de Senioridade

### Para Seniores
- Foque na **matriz de decisão rápida** e nos **critérios de escalação**
- Revise e atualize este playbook após cada post-mortem
- Valide se os runbooks de recuperação estão atualizados
- Decida quando declarar SEV-1 vs SEV-2 baseado em impacto real

### Para Plenos
- Execute a **triagem inicial** usando a classificação de severidade
- Coordene a comunicação no canal de incidentes durante SEV-3
- Escreva post-mortems para SEV-3 e revise drafts de juniores
- Identifique padrões em incidentes recorrentes e proponha melhorias sistêmicas

### Para Júniores
- Siga os **runbooks de recuperação rápida** passo a passo
- Colete logs e métricas conforme instruído pelo IC/SRE
- Nunca execute comandos fora do runbook sem aprovação explícita
- Reporte qualquer desvio do comportamento esperado imediatamente
- Use o template de comunicação para atualizações de status

---

## 6. Referências Cruzadas

- **Playbook Estratégico Geral:** `PLAYBOOK.md`
- **Security Model:** `specs/SECURITY-MODEL.md` §Incident Response
- **Communication Contract:** `specs/COMMUNICATION-CONTRACT.md` §Error Envelopes
- **Runbooks Operacionais:** `runbooks/README.md`
- **Handoffs:** `HANDOFF_*.md` (lições aprendidas por fase)