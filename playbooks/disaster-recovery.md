# Playbook de Disaster Recovery — Zylos-Next

> **Versão:** 1.0.0 | **Última revisão:** 2026-11-04 | **Classificação:** Confidencial
> **Ciclo de teste:** Anual obrigatório | **Proprietário:** SRE Lead

---

## 1. Declaração Formal

Este documento estabelece a estratégia de recuperação de desastres (DR) para a plataforma
Zylos-Next, garantindo continuidade operacional, preservação de dados e conformidade
regulatória em cenários de falha catastrófica. A recuperação prioriza a integridade dos
dados do `zylos-memory-system` (SQLite WAL + petgraph) acima de disponibilidade transiente.

A ativação deste playbook é autoridade exclusiva do **On-Call SRE Sênior** ou do
**Incident Commander** designado.

---

## 2. Objetivo

- Restaurar todos os componentes da plataforma Zylos-Next dentro dos limites de RTO/RPO definidos.
- Garantir zero perda de dados para componentes stateless (gateway, cockpit, sandbox).
- Limitar perda de dados do memory-system a no máximo 1 minuto de transações.
- Cumprir obrigações de comunicação regulatória sob a LGPD (Lei 13.709/2018).
- Documentar lições aprendidas em post-mortem obrigatório em até 72h após recuperação.

---

## 3. Escopo

### 3.1 Componentes Cobertos

| Componente | Tipo | RTO | RPO | Estratégia de Backup |
|---|---|---|---|---|
| `zylos-gateway` | Stateless (Axum HTTP/SSE) | < 5 min | 0 | Redeploy from image + config restore |
| `zylos-cockpit` | Stateless (Leptos WASM) | < 10 min | 0 | Redeploy from WASM bundle |
| `zylos-memory-system` | Stateful (SQLite WAL + petgraph) | < 30 min | < 1 min | Streaming replication + WAL archive |
| `zylos-sandbox` | Stateless (wasmtime + bollard Docker) | < 15 min | 0 | Container image + volume snapshot |
| `zylos-soar` | Orchestration state | < 20 min | < 5 min | Event log replay from gateway |
| `zylos-triage` | Stateless classifier | < 10 min | 0 | Redeploy from image |
| `zylos-sentinel` | Health scanner | < 10 min | 0 | Redeploy from image |
| `zylos-live-comm` | AG-UI streaming | < 10 min | 0 | Redeploy from image |

### 3.2 Fora do Escopo

- Dados de terceiros não replicados (logs de clientes não persistidos no memory-system).
- Ambiente de desenvolvimento local dos desenvolvedores.
- Certificados SSL/TLS de terceiros (responsabilidade do provedor).

---

## 4. Pré-requisitos

### 4.1 Infraestrutura

- Backup diário do `zylos-memory-system` (full + WAL streaming para storage imutável).
- Imagens de container versionadas publicadas no registry interno.
- Rede DR isolada pré-provisionada com conectividade VPN.
- Sistema de DNS secundário configurado e testado.

### 4.2 Acessos e Credenciais

- [ ] Credenciais do storage imutável (read-only para recovery).
- [ ] Acesso SSH ao cluster de recovery (chave YubiKey hardware).
- [ ] Token de deploy com escopo restrito ao namespace DR.
- [ ] Contato do DPO para comunicação LGPD.

### 4.3 Validação Pré-ativação

- [ ] `zylos-sentinel` reportou falha confirmada em múltiplos probes.
- [ ] Tentativa de failover automático falhou ou foi descartada.
- [ ] Incident Commander designado e sala de guerra ativa.

---

## 5. Procedimento de Recuperação

### 5.1 Fase 1 — Avaliação e Ativação (T+0 a T+5min)

> **🔴 SÊNIOR (Incident Commander):** Autoriza ativação do playbook. Confirma severidade
> e decide entre failover parcial vs. recovery total. Notifica stakeholders executivos.

> **🟡 PLENO:** Valida alertas do `zylos-sentinel` e confirma que não é falso positivo.
> Verifica integridade do último backup do `zylos-memory-system` (checksum SHA-256).

> **🟢 JÚNIOR:** Abre ticket de incidente no sistema de tracking. Registra timeline
> de eventos. Monitora canais de comunicação para atualizações.

```bash
# Verificar status dos componentes via zylos-sentinel
zylos-sentinel health-check --cluster=production --output=json

# Validar último backup do memory-system
zylos-memory backup verify --latest --checksum
```

### 5.2 Fase 2 — Recuperação do Memory-System (T+5min a T+30min)

Este é o componente crítico com maior RTO. Deve ser recuperado **antes** dos demais.

> **🔴 SÊNIOR:** Decide ponto de recuperação (PITR — Point-In-Time Recovery). Avalia
> se há necessidade de restaurar de full backup + replay de WAL ou apenas do último
> snapshot consistente.

> **🟡 PLENO:** Executa a restauração do SQLite. Monitora replay de WAL em tempo real.
> Valida integridade do grafo petgraph após restauração.

> **🟢 JÚNIOR:** Monitora progresso. Atualiza ticket com timestamps de cada etapa.

```bash
# Restaurar memory-system do backup mais recente
zylos-memory restore --source=immutability-storage --timestamp=<PITR>

# Validar integridade do grafo
zylos-memory graph-validate --check-edges --check-vertices --checksum

# Confirmar WAL replay completo
zylos-memory wal-status --expect=synced
```

### 5.3 Fase 3 — Recuperação dos Componentes Stateless (T+15min a T+30min)

> **🔴 SÊNIOR:** Coordena ordem de deploy. Garante que configurações sensíveis
> (secrets, tokens) são restauradas do vault.

> **🟡 PLENO:** Executa deploy sequencial: gateway → cockpit → sandbox → triage →
> sentinel → live-comm. Valida health endpoint de cada um antes do próximo.

> **🟢 JÚNIOR:** Monitora logs de startup. Reporta qualquer erro no canal de guerra.

```bash
# Recuperação sequencial dos componentes
# 1. Gateway (Axum HTTP/SSE) — RTO < 5min
zylos-deploy restore --component=zylos-gateway --from=registry --tag=<stable>
curl -sf https://gateway.zylos.internal/healthz

# 2. Cockpit (Leptos WASM) — RTO < 10min
zylos-deploy restore --component=zylos-cockpit --from=registry --tag=<stable>
curl -sf https://cockpit.zylos.internal/healthz

# 3. Sandbox (wasmtime + bollard) — RTO < 15min
zylos-deploy restore --component=zylos-sandbox --from=registry --tag=<stable>
zylos-sandbox health-check --all-runtimes

# 4. Triage — RTO < 10min
zylos-deploy restore --component=zylos-triage --from=registry --tag=<stable>

# 5. Sentinel — RTO < 10min
zylos-deploy restore --component=zylos-sentinel --from=registry --tag=<stable>

# 6. Live Comm (AG-UI) — RTO < 10min
zylos-deploy restore --component=zylos-live-comm --from=registry --tag=<stable>
```

### 5.4 Fase 4 — Validação End-to-End (T+30min a T+45min)

> **🔴 SÊNIOR:** Executa smoke test completo. Valida que fluxos SOAR (`zylos-soar`)
> operam normalmente. Confirma que `zylos-live-comm` está streamando corretamente.

> **🟡 PLENO:** Executa suite de integração. Verifica que `zylos-triage` classifica
> mensagens corretamente (dois estágios).

> **🟢 JÚNIOR:** Monitora métricas de latência e erro. Reporta anomalias.

---

## 6. Verificação de Integridade de Dados

### 6.1 Checklist de Integridade

```bash
# Validação completa do memory-system
zylos-memory integrity-check --full --report=json

# Verificar consistência do grafo petgraph
zylos-memory graph-validate --deep --report=json

# Comparar contagem de registros com pré-desastre
zylos-memory stats compare --baseline=<pre-disaster-snapshot>

# Validar checksums de artefatos críticos
zylos-deploy verify-checksums --all-components
```

### 6.2 Critérios de Aceitação

- [ ] Zero diferenças em dados críticos (agentes, workflows, memórias persistentes).
- [ ] Grafo petgraph consistente (sem edges órfãos, sem vértices isolados anômalos).
- [ ] Todos os health endpoints respondendo 200.
- [ ] Latência p99 dentro de 2x do baseline normal.
- [ ] Taxa de erro < 0.1% em janela de 15 minutos.

---

## 7. Comunicação Regulatória (LGPD)

### 7.1 Gatilhos de Notificação

Conforme Art. 48 da LGPD, a ANPD e os titulares devem ser notificados quando houver
risco ou dano relevante aos dados pessoais:

- Vazamento confirmado de dados pessoais armazenados no `zylos-memory-system`.
- Indisponibilidade prolongada que impeça exercício de direitos do titular.
- Comprometimento de logs de auditoria que rastreiam acesso a dados pessoais.

### 7.2 Timeline de Comunicação

| Tempo | Ação | Responsável |
|---|---|---|
| T+0 | Notificação interna ao DPO | Incident Commander |
| T+2h | Avaliação de impacto pelo DPO | DPO + Jurídico |
| T+24h | Notificação à ANPD (se aplicável) | DPO |
| T+72h | Comunicação aos titulares (se aplicável) | DPO + Comunicação |
| T+7dias | Relatório preliminar à diretoria | Incident Commander |

### 7.3 Conteúdo da Notificação (Art. 48, §1º)

- Natureza dos dados pessoais afetados.
- Informações sobre os titulares envolvidos.
- Medidas técnicas utilizadas para proteção dos dados.
- Riscos relacionados ao incidente.
- Medidas adotadas para reverter ou mitigar os efeitos.

---

## 8. Rollback

Se a recuperação falhar ou introduzir inconsistências:

```bash
# Abortar recovery e reverter para último estado conhecido
zylos-recovery abort --reason="<motivo>"

# Restaurar de backup anterior (se PITR falhou)
zylos-memory restore --source=immutability-storage --timestamp=<T-24h>

# Redeploy de versão estável anterior
zylos-deploy rollback --component=<component> --to=<last-stable-tag>
```

> **🔴 SÊNIOR:** Decide quando abortar e qual fallback executar. Comunica stakeholders.

> **🟡 PLENO:** Executa comandos de rollback. Valida estado resultante.

> **🟢 JÚNIOR:** Documenta o que falhou. Atualiza timeline do incidente.

---

## 9. Verificação Pós-Recovery

### 9.1 Smoke Tests Automatizados

```bash
# Executar suite completa de smoke tests
zylos-test smoke --environment=dr-recovered --fail-fast

# Validar fluxos SOAR end-to-end
zylos-soar test --runbook=health-verification --verbose

# Testar classificação do triage com dataset conhecido
zylos-triage validate --dataset=regression --expect-accuracy=0.95
```

### 9.2 Critérios de Encerramento

- [ ] Todos os smoke tests passando.
- [ ] Latência e throughput normalizados por 30 minutos contínuos.
- [ ] Zero alertas críticos no `zylos-sentinel` por 15 minutos.
- [ ] Incident Commander declara recuperação concluída.
- [ ] Comunicação de encerramento enviada a todos os stakeholders.

---

## 10. Teste Anual Obrigatório

### 10.1 Requisitos

- Teste completo de DR deve ser executado **anualmente** no máximo a cada 12 meses.
- Teste parcial (tabletop exercise) a cada 6 meses.
- Resultados documentados e apresentados à diretoria.
- Falhas identificadas devem gerar action items com deadline.

### 10.2 Metodologia

1. **Tabletop Exercise:** Simulação teórica com todos os stakeholders.
2. **Teste Parcial:** Failover de componente individual em janela de manutenção.
3. **Teste Completo:** Recovery total em ambiente isolado com dados de produção.

### 10.3 Métricas do Teste

| Métrica | Target |
|---|---|
| Tempo total de recuperação | ≤ soma dos RTOs |
| Dados perdidos | ≤ RPO definido |
| Tempo de comunicação LGPD | ≤ 24h |
| Smoke tests passando | 100% |

---

## 11. Referências

- LGPD — Lei 13.709/2018, Art. 46–48 (Segurança e Boas Práticas)
- ANPD — Guia de Boas Práticas em Segurança de Dados
- NIST SP 800-34 Rev. 1 — Contingency Planning Guide
- `zylos-sentinel` — Documentação de health scanning
- `zylos-memory-system` — Documentação de backup e replication
- Post-mortem templates em `/playbooks/templates/post-mortem.md`
- Notification-router skill para procedimentos de escalação