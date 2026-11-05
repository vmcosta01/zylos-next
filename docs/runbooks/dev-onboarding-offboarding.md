# Runbook: Onboarding e Offboarding de Desenvolvedores

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Procedimento padronizado para provisionamento de ambiente, acessos e ferramentas para novos devs, e revogação completa no desligamento
**Público-alvo:** Tech Lead, Dev Senior, SRE, RH/People Ops
**Gatilho:** Novo dev contratado ou transferido para o time Zylos-Next (onboarding), ou desligamento/transferência de dev do time (offboarding)
**Playbook relacionado:** `stakeholder-communication.md` (comunicação interna de mudanças de equipe)

---

## 1. Visão Geral

Este runbook garante que todo desenvolvedor que entra no time Zylos-Next tenha ambiente funcional em <4h e que todo dev que sai tenha acessos revogados em <24h, sem resíduos de credenciais ou permissões órfãs.

**Por que importa:** Onboarding lento = dias perdidos de produtividade. Offboarding incompleto = risco de segurança (acessos residuais, secrets vazadas, contas órfãs). Ambos são requisitos de compliance (LGPD, ISO 27001, SOC2).

---

## 2. Pré-requisitos

### Para Onboarding
- [ ] Contrato assinado e data de início confirmada
- [ ] Conta corporativa criada (email, Slack/Teams)
- [ ] Acesso ao repositório GitHub/GitLab aprovado pelo Tech Lead
- [ ] Máquina/laptop provisionada pelo IT (ou VM/cloud workstation)
- [ ] Buddy/mentor designado pelo Tech Lead

### Para Offboarding
- [ ] Data de desligamento confirmada pelo RH
- [ ] Lista de sistemas e acessos do dev obtida via `zylos access audit <dev-email>`
- [ ] Knowledge transfer agendado com Tech Lead
- [ ] Tickets de revogação criados em todos os sistemas relevantes

---

## 3. Onboarding — Passo a Passo

### 3.1 Dia 0: Provisionamento de Acessos (Antes do Primeiro Dia)

```bash
# 1. Adicionar ao time no GitHub
gh api orgs/zylosdev/teams/engineering/memberships/<username> \
  -X PUT -f role=member

# 2. Provisionar acesso ao Grafana/Axiom (se SRE)
# Via painel admin ou API do provider

# 3. Criar conta no Sentry (error tracking)
# Convite via email corporativo

# 4. Adicionar ao canal #zylos-devs no Slack
/slack invite @<username> #zylos-devs
```

**Validação:** Dev consegue clonar repo, acessar dashboard de métricas e ver canal no Slack.

### 3.2 Dia 1: Setup de Ambiente Local

```bash
# 1. Clonar repositório
git clone git@github.com:zylosdev/zylos-next.git
cd zylos-next

# 2. Instalar toolchain Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install stable
rustup component add clippy rustfmt

# 3. Instalar dependências do sistema
# Linux: sudo apt install build-essential pkg-config libssl-dev
# macOS: brew install openssl pkg-config
# Windows: vcpkg install openssl:x64-windows-static

# 4. Build inicial + testes
cargo build --all-features
cargo test --all-features

# 5. Instalar trunk (WASM frontend)
cargo install trunk wasm-bindgen-cli

# 6. Validar cockpit local
cd apps/zylos-cockpit && trunk serve
# Abrir http://localhost:8080 e confirmar que carrega
```

**Validação:** `cargo test` passa 100%, cockpit abre sem erros no console.

### 3.3 Dia 1-2: Configuração de Ferramentas

| Ferramenta | Ação | Validação |
|-----------|------|-----------|
| Git | Configurar nome/email + GPG signing | `git config user.signingkey <KEY_ID>` |
| IDE | Importar settings do repositório (`.vscode/`, `.idea/`) | LSP funciona, formatação automática |
| Docker | Login no registry interno | `docker pull zylosdev/zylos-staging:latest` |
| CLI Zylos | Instalar binário + configurar | `zylos status` retorna connected |
| Secrets | Receber credenciais via vault (NUNCA por email/chat) | Acesso confirmado no vault |

### 3.4 Semana 1: Primeiras Tarefas

1. **Ler documentação obrigatória:**
   - `MASTER_PLAN.md` (visão geral)
   - `PLAYBOOK.md` (processos do time)
   - Este runbook + `security-incident-response.md`
   - `HANDOFF_6.md` (estado atual do projeto)

2. **Completar tarefa de onboarding:** Issue etiquetada `good-first-issue` ou `onboarding` no board

3. **Primeiro deploy supervisionado:** Fazer merge de PR simples com revisão do buddy

4. **Reunião de alinhamento:** 1:1 com Tech Lead para dúvidas e feedback inicial

**Validação final de onboarding:**
- [ ] Dev fez merge de pelo menos 1 PR
- [ ] Dev executou `zylos doctor` sem erros
- [ ] Dev conhece o processo de escalação (`security-incident-response.md` §4)
- [ ] Buddy confirma que dev está operacional

---

## 4. Offboarding — Passo a Passo

### 4.1 Último Dia: Revogação de Acessos

Execute na ordem. Não pule etapas.

```bash
# 1. Remover do time GitHub
gh api orgs/zylosdev/teams/engineering/memberships/<username> -X DELETE

# 2. Revogar tokens pessoais
gh api users/<username>/tokens -X DELETE  # ou via UI

# 3. Remover do Grafana/Axiom/Sentry
# Via painel admin de cada ferramenta

# 4. Revogar acesso ao vault de secrets
# HashiCorp Vault: vault token revoke <token>
# AWS SM: remover IAM policy attachment

# 5. Remover do Slack/Teams
# Admin panel → Members → Remove

# 6. Desprovisionar máquina/VM
# IT ticket ou script automatizado

# 7. Verificar ausência de acessos residuais
zylos access audit <dev-email>
# Esperado: zero resultados
```

### 4.2 Knowledge Transfer (Última Semana)

| Item | Ação | Owner |
|------|------|-------|
| PRs abertos | Reassignar ou fechar com comentário | Tech Lead |
| Issues assignadas | Reassignar para outro dev | Tech Lead |
| Documentação pessoal | Migrar para docs compartilhadas | Dev + Buddy |
| Credenciais locais | Confirmar que não há secrets em texto plano | Dev + Security |
| Contatos externos | Transferir relacionamentos com vendors/clientes | PM/CS |

### 4.3 Validação Final de Offboarding

- [ ] `zylos access audit <dev-email>` retorna zero resultados
- [ ] Nenhum PR ou issue aberto sob responsabilidade do dev
- [ ] Secrets rotacionadas se dev tinha acesso a produção
- [ ] Máquina/VM desprovisionada ou reimageada
- [ ] Email de despedida enviado ao time (opcional, cultural)
- [ ] Ticket de offboarding fechado no sistema de RH

---

## 5. Caminhos Alternativos e Contingência

| Cenário | Ação Alternativa |
|---------|-----------------|
| Dev precisa de acesso urgente antes do setup completo | Provisionar VM temporária com imagem pré-configurada |
| Máquina não chegou a tempo | Cloud workstation (GitHub Codespaces / Gitpod) |
| Dev sai abruptamente (sem notice period) | Executar §4.1 imediatamente; knowledge transfer via código/docs |
| Access audit mostra acessos residuais após revogação | Escalar para Security Engineer; possível bug no sistema de IAM |
| Dev era único owner de recurso crítico | Antes do offboarding: adicionar co-owner + documentar |

---

## 6. Critério de Validação Final

**Onboarding completo quando:**
- [ ] Todos os checks da §3.4 passam
- [ ] Dev confirma que ambiente está funcional
- [ ] Tech Lead aprova onboarding no board

**Offboarding completo quando:**
- [ ] Todos os checks da §4.3 passam
- [ ] Security Engineer confirma zero acessos residuais
- [ ] Ticket de offboarding fechado

---

## 7. Protocolo Pós-execução

1. **Onboarding:** Coletar feedback do novo dev após 2 semanas; melhorar este runbook se necessário
2. **Offboarding:** Registrar lições aprendidas se houve gap no processo
3. **Auditoria trimestral:** Revisar lista de devs ativos vs. acessos provisionados
4. **Atualizar este runbook:** Sempre que nova ferramenta for adotada ou removida do stack

---

## 8. Referências Cruzadas

- **Runbooks relacionados:** `zylos-cli.md` (setup da CLI), `deploy-cicd.md` (primeiro deploy)
- **Playbooks relacionados:** `stakeholder-communication.md` (comunicação de mudança de equipe), `security-incident-response.md` (se offboarding for por causa de segurança)
- **SOAR:** `soar/SOAR_ARCHITECTURE.md` (offboarding pode ser trigger para WF de revogação automática)
- **Gap Analysis:** `GAPS_ANALYSIS_RUNBOOKS_PLAYBOOKS.md` §4.5 (origem deste documento)
- **Referências externas:** OpServices (runbook de onboarding/offboarding como tipo essencial)