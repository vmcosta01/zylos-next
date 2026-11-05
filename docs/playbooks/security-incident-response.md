# Playbook: Security Incident Response — Zylos-Next

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Protocolo estratégico para detecção, contenção, erradicação e recuperação de incidentes de segurança no sistema Zylos-Next
**Público-alvo:** Tech Lead, Dev Senior, SRE, Security Engineer
**Gatilho:** Detecção de acesso não autorizado, vazamento de credenciais, escape de sandbox, injeção de skill maliciosa, ou qualquer violação dos controles definidos em SECURITY.md

---

## 1. Contexto Organizacional

O Zylos-Next executa código não-confiável em sandboxes (WASM/Docker), gerencia credenciais de múltiplos providers LLM/search, e expõe APIs REST/SSE para cockpit e mobile. Um incidente de segurança pode resultar em:

- **Escape de sandbox:** Código malicioso acessa host filesystem/network
- **Vazamento de credenciais:** API keys de LLM/search expostas via logs, erros ou SSE
- **Injeção de skill maliciosa:** Skill registrada com payload oculto executado por agentes
- **Ataque ao EventBus:** Mensagens forjadas injetadas no broadcast interno
- **Comprometimento do registry:** index.json adulterado para servir skills trojanizadas
- **Negação de serviço:** Exhaustion de fuel/memory em WASM ou spawn massivo de containers

Missão: conter danos em <15 min, erradicar causa raiz em <4h, recuperar serviço em <24h, e publicar post-mortem em <72h.

---

## 2. Papéis e Responsabilidades

| Papel | Responsabilidade | Decisão |
|-------|-----------------|---------|
| Incident Commander (IC) | Coordenar resposta, comunicar stakeholders, autorizar ações destrutivas | Go/No-go para shutdown parcial/total |
| Security Engineer | Analisar vetores de ataque, validar contenção, forense | Classificação de severidade |
| Dev Senior | Implementar hotfixes, rollback, patches de emergência | Timing de deploy |
| SRE / Ops | Executar isolamento de nós, revogação de credenciais, restauração de backups | Execução operacional |
| Tech Lead | Aprovar comunicação externa, coordenar com legal/compliance | Comunicação externa |
| Product Manager | Comunicar impacto a clientes, gerenciar expectativas | Messaging para usuários |

---

## 3. Fluxo de Decisão Estratégica

### 3.1 Classificação de Severidade

| Severidade | Critério | Tempo de Resposta | Exemplo |
|------------|----------|-------------------|---------|
| **P0 — Crítico** | Dados exfiltrados, sandbox escape confirmado, credenciais ativas vazadas | <15 min | WasmSandbox bypass permite leitura de `/etc/passwd` |
| **P1 — Alto** | Serviço degradado por ataque, skill maliciosa detectada pós-execução, credenciais revogadas proativamente | <1h | Skill com `shortcut: skip-validation` executa comando shell |
| **P2 — Médio** | Tentativa de ataque contida, vulnerabilidade explorável mas sem evidência de uso | <4h | CVE em wasmtime sem exploit público conhecido |
| **P3 — Baixo** | Configuração insegura detectada, log com dado sensível (sem exposição externa) | <24h | API key impressa em debug log local |

### 3.2 Matriz de Decisão por Vetor

| Vetor de Ataque | Ação Imediata | Contenção | Erradicação | Recuperação |
|-----------------|---------------|-----------|-------------|-------------|
| Sandbox escape | Shutdown imediato do daemon (`zylos daemon stop`) | Isolar host afetado da rede | Patch do crate sandbox + auditoria de todos os módulos WASM | Restaurar de backup limpo + rotação de todas as credenciais |
| Credencial vazada | Revogar chave no provider imediatamente | Rotacionar todas as chaves do mesmo escopo | Auditar logs para determinar janela de exposição | Reemitir chaves + habilitar alertas de uso anômalo |
| Skill maliciosa | Desativar skill no registry (`active: false`) | Bloquear execução via scheduler | Remover skill + investigar origem (quem registrou?) | Notificar usuários que executaram a skill + scan de outputs |
| EventBus injection | Habilitar validação estrita de envelope no HermesBridge | Rate-limit por source IP/agent ID | Patch de validação + testes de fuzzing | Replay de eventos legítimos se necessário |
| Registry comprometido | Congelar registry (read-only mode) | Rollback para última versão conhecida boa | Verificar integridade de todas as skills via hash | Rebuild do index.json + auditoria manual |
| DoS (fuel/memory exhaustion) | Reduzir `max_concurrent_agents` para 10% do normal | Auto-scaling down + rate-limit agressivo | Identificar fonte (skill específica? agente específico?) | Restaurar limites normais gradualmente com monitoring |

### 3.3 Critérios de Entrada e Saída

**Entrada:** Qualquer alerta de segurança classificado como P0-P3 E confirmação inicial por Security Engineer ou Dev Senior.

**Saída:** Incidente resolvido quando TODOS forem verdadeiros:
- [ ] Causa raiz identificada e documentada
- [ ] Vetor de ataque neutralizado (patch aplicado OU controle compensatório ativo)
- [ ] Credenciais afetadas rotacionadas (se aplicável)
- [ ] Serviço restaurado e métricas estáveis por ≥1h
- [ ] Post-mortem publicado (P0/P1 obrigatório, P2/P3 opcional)

---

## 4. Governança e Escalação

### 4.1 Níveis de Decisão

| Nível | Quem Decide | Exemplo | Tempo Máximo |
|-------|-------------|---------|--------------|
| L1 — Contenção Imediata | Qualquer dev senior on-call | Shutdown de daemon, revogação de chave, desativação de skill | Imediato |
| L2 — Erradicação | Incident Commander + Security Engineer | Deploy de hotfix, rollback de release, notificação a clientes | 4 horas |
| L3 — Comunicação Externa | Tech Lead + Legal/Compliance | Public disclosure, notificação regulatória, comunicado a imprensa | 24 horas |
| L4 — Forense/Investigação | Security Engineer + External Auditor (se necessário) | Análise de core dumps, revisão de logs históricos, contratação de pentest | 72 horas |

### 4.2 Protocolo de Escalação

1. **P0 detectado** → IC assume imediatamente, canal `#zylos-security-incident` criado, todos os seniors notificados
2. **Contenção não alcançada em 30 min** → Escalar para CTO/VP Eng + considerar envolvimento de vendor externo
3. **Dados pessoais/exfiltração confirmada** → Legal/Compliance acionado em <1h, avaliação de obrigações regulatórias (LGPD/GDPR)
4. **Post-mortem atrasado >72h** → Escalar para leadership com justificativa documentada

### 4.3 Comunicação entre Equipes

- **Canal primário durante incidente:** `#zylos-security-incident` (Slack/Teams) — somente fatos, sem especulação
- **War room virtual:** Call aberta para P0; P1+ sob demanda
- **Status updates:** A cada 30 min durante contenção, a cada 2h durante erradicação
- **Stakeholder updates:** Template padronizado (ver §6) enviado por PM/Tech Lead
- **Post-mortem:** Publicado em `docs/postmortems/YYYY-MM-DD-<titulo>.md`, revisado por Security Engineer + Tech Lead

---

## 5. Ferramentas e Sistemas Necessários

| Ferramenta | Uso | Acesso |
|------------|-----|--------|
| `zylos-cli` | Shutdown de daemon, listagem de agents/skills, logs | Todos devs |
| Provider dashboards | Revogação de API keys (OpenAI, Brave, Tavily, etc.) | Security Engineer + SRE |
| Git history / blame | Investigar quem registrou skill maliciosa ou introduziu vulnerabilidade | Todos devs |
| Log aggregation (Axiom/Grafana) | Correlacionar eventos, identificar janela de exposição | SRE + Security |
| Vault / Secrets Manager | Rotação automatizada de credenciais | SRE |
| Container runtime (Docker) | Inspeção de containers suspeitos, coleta de evidências | SRE + Security |
| Wasmtime CLI | Reproduzir execução de WASM suspeito em ambiente isolado | Security Engineer |
| Notion / Confluence | Documentação de incidentes e post-mortems | Todos |

---

## 6. Templates de Comunicação

### 6.1 Alerta Inicial (Interno)

```
🚨 INCIDENTE DE SEGURANÇA — [SEVERIDADE]
Vetor: [sandbox escape / credential leak / malicious skill / ...]
Detectado: [timestamp ISO 8601]
Impacto estimado: [descrição curta]
IC: [@nome]
Canal: #zylos-security-incident
Status: [investigando / contendo / erradicando / recuperando]
Próximo update: [timestamp +30min]
```

### 6.2 Status Update (Interno)

```
📊 UPDATE — [SEVERIDADE] — [HH:MM]
Status atual: [contendo / erradicando / recuperando / resolvido]
Progresso: [o que foi feito desde último update]
Bloqueios: [se houver]
ETA resolução: [estimativa]
Próximo update: [timestamp]
```

### 6.3 Comunicação Externa (Clientes/Stakeholders)

```
Assunto: Aviso de Segurança — Zylos-Next [Data]

Prezados,

Identificamos e resolvemos um incidente de segurança em [data/hora].

O QUE ACONTECEU: [descrição factual, sem detalhes técnicos sensíveis]
IMPACTO: [quais serviços/usuários foram afetados]
AÇÕES TOMADAS: [contenção, erradicação, prevenção]
RECOMENDAÇÕES: [ações que clientes devem tomar, se aplicável]

Para dúvidas: security@zylos.dev

Atenciosamente,
Equipe de Segurança Zylos-Next
```

---

## 7. Forense e Preservação de Evidência

Esta seção define o protocolo para coleta, preservação e cadeia de custódia de evidências digitais durante incidentes de segurança. Conforme TecnetOne e Cutover, a forense é distinta da resposta: enquanto a resposta foca em contenção/erradicação, a forense garante que evidências sejam admissíveis legalmente e auditáveis.

### 7.1 Princípios de Forense Digital

1. **Não alterar a evidência original:** Trabalhar sempre com cópias forenses (imagens bit-a-bit). O original deve ser isolado e preservado.
2. **Cadeia de custódia documentada:** Cada pessoa que manuseia a evidência deve registrar entrada/saída com timestamp, hash e motivo.
3. **Hash de integridade obrigatório:** SHA-256 ou SHA-3 de cada artefato coletado, registrado no audit trail imutável do SOAR.
4. **Ordem de volatilidade:** Coletar evidências mais voláteis primeiro (memória RAM → processos → conexões de rede → disco → logs remotos).
5. **Documentação contemporânea:** Registrar cada ação tomada durante a investigação em tempo real, não retroativamente.

### 7.2 Procedimento de Coleta de Evidência

#### Passo 1: Isolamento do Ativo Comprometido
```bash
# Isolar sem desligar (preserva memória RAM)
# Linux: remover da rede mantendo sessão local ativa
ip link set eth0 down
# OU via firewall: bloquear todo tráfego exceto SSH de investigação
iptables -A INPUT -s <investigator_ip> -p tcp --dport 22 -j ACCEPT
iptables -A INPUT -j DROP
iptables -A OUTPUT -j DROP

# Docker: parar container sem remover volumes
docker stop <container_id>
# NÃO usar docker rm — perde filesystem layer

# WASM sandbox: snapshot do estado antes de kill
zylos agent snapshot <agent-id> --output /evidence/<incident-id>/wasm-snapshot.bin
```

#### Passo 2: Coleta Volátil (Memória + Processos)
```bash
# Dump de memória (Linux)
sudo dd if=/dev/mem of=/evidence/<incident-id>/memory.raw bs=1M
sha256sum /evidence/<incident-id>/memory.raw | tee /evidence/<incident-id>/memory.sha256

# Lista de processos com detalhes
ps auxf > /evidence/<incident-id>/processes.txt
cat /proc/*/status > /evidence/<incident-id>/proc-status.txt 2>/dev/null

# Conexões de rede ativas
ss -tlnp > /evidence/<incident-id>/network-listeners.txt
ss -tnp > /evidence/<incident-id>/network-connections.txt
arp -a > /evidence/<incident-id>/arp-table.txt

# Variáveis de ambiente (podem conter secrets vazadas)
env > /evidence/<incident-id>/environment.txt
```

#### Passo 3: Coleta Persistente (Disco + Logs)
```bash
# Imagem forense do disco (bit-a-bit)
sudo dd if=/dev/sda of=/evidence/<incident-id>/disk-image.dd bs=4M conv=noerror,sync
sha256sum /evidence/<incident-id>/disk-image.dd | tee /evidence/<incident-id>/disk-image.sha256

# Logs do sistema
cp -a /var/log/ /evidence/<incident-id>/system-logs/
journalctl --no-pager > /evidence/<incident-id>/journalctl-full.txt

# Logs do Zylos
cp -a ~/.local/share/zylos/logs/ /evidence/<incident-id>/zylos-logs/

# Configurações e binaries atuais
cp ~/.config/zylos/config.toml /evidence/<incident-id>/zylos-config.toml
sha256sum $(which zylos) > /evidence/<incident-id>/zylos-binary.sha256
```

#### Passo 4: Registro no Audit Trail SOAR
```bash
# Cada artefato coletado deve ser registrado
zylos soar evidence add \
  --incident <incident-id> \
  --file /evidence/<incident-id>/memory.raw \
  --hash $(cat /evidence/<incident-id>/memory.sha256 | awk '{print $1}') \
  --collector "$(whoami)" \
  --description "Memory dump of compromised host"

# Verificar cadeia de custódia
zylos soar evidence chain --incident <incident-id>
```

### 7.3 Requisitos Legais e Compliance

| Regulamentação | Requisito de Evidência | Prazo | Owner |
|---------------|----------------------|-------|-------|
| LGPD (Brasil) | Registro de tratamento de dados pessoais afetados + medidas técnicas | Notificação ANPD em prazo razoável | Legal + Security |
| DORA (UE) | Timeline detalhada + causa raiz + medidas corretivas | Relatório grave em 4h | Compliance |
| ISO 27001 | Evidências de controle de acesso + logs de auditoria | Retenção mínima 1 ano | Security |
| Contratos Enterprise | Direito de auditoria + evidências de recovery | Conforme SLA individual | CS + Legal |

### 7.4 Ferramentas de Forense

| Ferramenta | Uso | Instalação |
|-----------|-----|------------|
| `dd` / `dcfldd` | Imaging forense bit-a-bit | Nativo Linux |
| `volatility3` | Análise de memória RAM | `pip install volatility3` |
| `autopsy` / `sleuthkit` | Análise de imagem de disco | `apt install sleuthkit` |
| `wireshark` / `tcpdump` | Análise de tráfego de rede | `apt install wireshark` |
| `yara` | Detecção de malware por assinatura | `apt install yara` |
| `zylos soar evidence` | Cadeia de custódia e registro no audit trail | Nativo Zylos CLI |

### 7.5 Integração com SOAR

O workflow WF-001 (Breach Confirmado) do SOAR_ARCHITECTURE.md inclui etapa de forense automática:
- Após contenção (§WF-001 step 2), o orquestrador pausa para aprovação humana antes da erradicação
- Durante a pausa, executa automaticamente os passos 1-3 acima via runbook `forensic-evidence-collection`
- Registra todos os hashes no audit trail imutável
- Gera relatório preliminar de evidências para revisão do Security Engineer

---

## 8. Formato e Publicação

- **Formato:** Markdown versionado no repositório (`docs/playbooks/security-incident-response.md`)
- **Atualização:** Após cada incidente real, exercício tabletop, ou mudança significativa na arquitetura
- **Review trimestral:** Security Engineer + Tech Lead revisam vetores, thresholds e templates
- **Exercício tabletop:** Semestral; simular P0 com equipe completa, documentar gaps e corrigir este playbook
- **Onboarding:** Todo novo dev senior deve participar de pelo menos um tabletop nos primeiros 90 dias

---

## 8. Referências Cruzadas

- **Runbook relacionado:** `zylos-daemon.md` (shutdown/restart operacional)
- **Runbook relacionado:** `fase4-sandbox-isolation.md` (auditoria de segurança WASM/Docker)
- **Playbook relacionado:** `capacity-planning.md` (se DoS for causado por pico legítimo vs. ataque)
- **SECURITY.md:** Regras de segurança camada zero (pré-implementação)
- **AGENTS.md:** Regras enforceáveis para agentes (prevenção de injeção)
- **HANDOFF_4.md:** Lições aprendidas sobre sandbox isolation
- **DECISIONS.md:** ADRs relevantes para decisões de segurança

---

**Próxima Revisão Agendada:** Fevereiro de 2027
**Próximo Tabletop Agendado:** Dezembro de 2026
**Owner:** Security Engineer Zylos-Next