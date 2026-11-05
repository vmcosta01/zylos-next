# Engenharia Reversa: Macro (macro-inc/macro)
## Relatório Comparativo para Zylos-Next

**Data:** 2026-11-04
**Fonte:** Repositório público `macro-inc/macro` (GitHub) + Documentação oficial
**Análise:** Estrutural, Arquitetural e Comparativa

---

## 1. Arquitetura e Stack Principal

### Stack Tecnológica
- **Backend:** Rust (167 crates em `crates/`, 42 serviços em `services/`)
- **Frontend:** SolidJS + Tauri (desktop/mobile) + Web (Vite)
- **Colaboração Real-time:** CRDTs via Cloudflare Durable Objects + Loro (`packages/loro-mirror`)
- **Infraestrutura:** Pulumi (IaC), Docker Compose (local), Nix (dev shell reproducível)
- **Banco de Dados:** PostgreSQL (sqlx), Redis, OpenSearch, Kafka
- **Auth:** FusionAuth (local/dev), Google/GitHub SSO (prod)
- **Storage:** S3/LocalStack + CloudFront (signed URLs)
- **Observabilidade:** OpenTelemetry (OTLP) → Grafana Tempo/Loki/Prometheus ou Jaeger/Datadog

### Padrão Arquitetural: Hexagonal (Ports & Adapters)
Os serviços Rust seguem layout hexagonal estrito, enforced por regras ast-grep:
- **`src/domain/`**: Lógica pura, zero dependência de infra (sem AWS, redis, reqwest, axum, kafka). Regra `CS-51`.
- **`src/inbound/`**: Adapters de entrada (handlers Axum, listeners Kafka, tools MCP). Não executam SQL direto (`CS-53`).
- **`src/outbound/`**: Adapters de saída (repositórios DB, clientes API). Implementam traits definidas em domain.
- **Dependências apontam para dentro:** Domain nunca importa inbound/outbound (`CS-52`).

### Organização do Monorepo
```
macro/
├── apps/web/          # SolidJS client (browser, Tauri, mobile)
├── apps/docs/         # docs.macro.com
├── services/          # 42 serviços deployáveis (Rust + alguns workers)
├── crates/            # 167 bibliotecas Rust (domínio, modelos, DB clients)
├── packages/          # TypeScript compartilhado (collaboration, lexical-core)
├── infra/             # Pulumi + Docker Compose local
├── nix/               # Dev shell pinado
└── tooling/           # Scripts e code generators
```

---

## 2. Componentes Principais e Responsabilidades

| Bloco | Descrição Técnica | Crate/Serviço Provável |
|-------|-------------------|------------------------|
| **Email** | Inbox unificada multi-conta, sync Gmail, parsing de anexos PDF para busca | `services/email-*`, `crates/email*` |
| **Messages** | Channels + DMs com threads permissionados, grafo bidirecional | `services/chat-*`, `crates/chat*` |
| **Tasks** | Issues leves acopladas a channels/email, bidirectional links | `services/task-*`, `crates/task*` |
| **Docs** | Editor Markdown CRDT-based, version control, agent-native editing | `services/sync_service`, `packages/loro-mirror` |
| **Canvas** | Board 2D com @links embeddables | `apps/web` (frontend-heavy) |
| **Agents** | Runtime de agentes com memória unificada, MCP server, tool surface | `crates/agent*`, session actor |
| **Calls** | Gravação, transcrição, log na memória da equipe | `services/call-*` |
| **CRM** | Objetos Company/Contact com agregação de email/messages | `services/crm-*`, `crates/crm*` |
| **Search** | OpenSearch-backed, indexa emails, docs, attachments, messages | `services/search-*`, `crates/search*` |
| **Auth** | Passwordless (email code), SSO, API tokens, CLA bot | FusionAuth + `services/auth-*` |

### Serviços Auxiliares (Docker-built)
- `sync_service`: Cloudflare Worker para CRDT sync (Durable Objects)
- `lexical_service`: Processamento de rich text (Lexical editor)
- `websocket_service`: Real-time WebSocket gateway

---

## 3. Padrões de Orquestração de Agentes/Workflows

### Modelo de Agente
- **Agentes como peers no CRDT:** Agentes editam documentos como colaboradores humanos no mesmo sistema CRDT, não via API separada. Conflitos resolvidos nativamente pelo protocolo.
- **Session Actor:** Rastreia turnos de agente com spans OTLP (`span.gen_ai.operation.name="invoke_agent"`). Propaga contexto via `params._meta["macro.dev/trace-context"]` para correlacionar chamadas de modelo + backend no mesmo trace.
- **MCP Server Nativo:** Surface de ferramentas com ~100% cobertura da UI, sem rate limits. Endpoint HTTP: `https://mcp-server.macro.com/mcp`. Compatível com Claude Code, Codex, qualquer cliente MCP.
- **Automações Cron:** Jobs periódicos (ex: atualização diária de docs de team context) rodam como agentes que leem channels e escrevem em docs, com detecção de conflitos via CRDT.

### Orquestração Implícita
- **Não há workflow engine explícito** (tipo Temporal/Airflow). A orquestração emerge de:
  - Eventos Kafka entre serviços
  - CRDT sync para estado compartilhado
  - Cron jobs como triggers
  - MCP tools como interface de ação
- **Bidirectional Graph:** @mentions criam links bidirecionais persistentes. Um agente pode navegar de task → email → message → CRM record seguindo arestas do grafo, não queries ad-hoc.

---

## 4. Sistema de Memória e Estado

### Memória Unificada de Equipe
- **Escopo:** Team-level, não user-level. Sintetiza conversas, DMs, emails enviados/recebidos, tasks, calls.
- **Refresh:** Cron job noturno que faz síntese em um único pass (não incremental por fonte).
- **Formato:** Markdown puro, exportável. Atualizável via comando natural ("remember X").
- **Disponibilidade:** Via MCP para agentes externos, ou model picker (OpenAI/Anthropic/Google) para portabilidade.

### Estado Persistente
- **PostgreSQL:** Fonte primária para entidades estruturadas (users, teams, channels, tasks, CRM records, permissions).
- **CRDT State (Cloudflare):** Documentos colaborativos. Local state em volume Docker `sync_state` para dev.
- **OpenSearch:** Índice de busca full-text (emails, docs, attachments, messages).
- **Kafka:** Event bus para propagação assíncrona entre serviços.
- **Redis:** Cache e pub/sub de curto prazo.
- **Grafo Bidirecional:** Links @mention armazenados como arestas, não apenas texto. Permite travessia reversa (de um CRM record ver todas as mensagens que o mencionaram).

### Permissões Baseadas em Canal
- Acesso herdado por menção em canal: entrar no canal = ganhar acesso a tudo @mencionado nele.
- Threads têm permissão individual (shareable via link).
- Entity access mutations passam exclusivamente por `entity_access` crate (`CS-27`).

---

## 5. Integrações e Conectores

### Nativas
| Integração | Tipo | Notas |
|------------|------|-------|
| Gmail / Google Workspace | OAuth + Sync | Multi-account, inbox unificada |
| GitHub | OAuth + Webhooks | PRs linkados a tasks, embed em channels |
| Stripe | API | Billing/subscriptions |
| CloudFront | Signed URLs | Document storage proxy |
| FusionAuth | Auth provider | Passwordless + SSO |
| Pipedream Connect | Workflow automation | Origens configuráveis via env |

### MCP (Model Context Protocol)
- **Server HTTP público:** `https://mcp-server.macro.com/mcp`
- **Cobertura:** Quase 100% das ações da UI (criar task, buscar email, editar doc, gerenciar CRM).
- **Sem rate limits.**
- **Memória exposta:** Agentes externos podem ler/atualizar team memory via MCP.

### Extensibilidade
- **Blocks modulares:** Cada superfície (email, chat, docs) é um bloco independente mas compartilha backend e grafo.
- **Custom properties:** Entidades suportam propriedades customizadas (CRM, tasks).
- **API/SDK:** Criação de tasks e outras ações via API externa.

---

## 6. Capacidades de Observabilidade/Debug

### Tracing Distribuído
- **OTLP nativo:** Todos os serviços Rust exportam spans e eventos `tracing` via OTLP.
- **Browser-to-backend:** Frontend exporta spans via proxy, propaga `traceparent`. Um trace cobre browser → proxy → múltiplos serviços.
- **Agent correlation:** Session actor injeta contexto em `params._meta["macro.dev/trace-context"]`. Runtime in-process restaura contexto para que chamadas de modelo e backend apareçam no mesmo trace.
- **Backends suportados:** Grafana Tempo (padrão local), Jaeger, Datadog. Toggle via `--traces lgtm|jaeger|datadog|off`.

### Logs
- **Loki:** Agregação de logs de serviço.
- **Rotação local:** 20 MB/arquivo, 3 arquivos/container.
- **Console logs visíveis** no terminal do `just run_local`.

### Debug Browser para Agentes
- **Chromium headless** opt-in (`--with-chrome`): DevTools protocol em `localhost:9222`.
- **VNC live:** `localhost:6080/vnc.html` para assistir agente navegando.
- **MCP chrome-devtools:** Configurado em `.mcp.json` para agentes dirigirem o app.

### Ferramentas de Desenvolvimento
- **`just check`:** Gate único (fmt, lint, code rules, PII scan) escopado a mudanças vs `origin/main`.
- **`just doctor-local`:** Preflight check de Docker, toolchain, portas.
- **Seed scenarios:** `just seed-scenario apply` cria mundo realista (users, teams, channels, perms) com login links por persona.
- **Init snapshots:** Cache content-addressed de DB migrations/Kafka topics/FusionAuth kickstart. Cold start subsequente restaura snapshot em vez de reinicializar.
- **Timings:** `MACRO_LOCAL_TIMINGS` gera JSONL com duração de cada stage de bring-up.

---

## 7. Pontos Fortes e Fracos

### Fortes
1. **Arquitetura hexagonal enforced por tooling:** Regras ast-grep (`CS-51`, `CS-52`, `CS-53`) impedem vazamento de infra para domain. Não é só convenção, é CI gate.
2. **Memória unificada real:** Team memory sintetizada de todas as fontes (email, chat, tasks, calls) em um pass. Muito superior a chatbots que só lembram do próprio chat.
3. **CRDT-native agents:** Agentes como peers no documento, não bolt-on. Conflitos resolvidos pelo protocolo, não por locks ou retries.
4. **MCP first-class:** Surface completa, sem rate limits, documentado como produto. Agentes externos são cidadãos de primeira classe.
5. **DevEx excepcional:** Nix shell, init snapshots, seed scenarios, debug browser, tracing out-of-box. Onboarding de contribuidor leva minutos, não dias.
6. **Grafo bidirecional:** @links são arestas persistidas, não texto. Permite navegação reversa e auditoria completa de contexto.
7. **Monorepo bem organizado:** 167 crates + 42 serviços coesos, com regras claras de boundary e reuse (`CS-23`, `CS-26`, `CS-71`).

### Fracos / Limitações
1. **Complexidade operacional:** 42 serviços + Postgres + Redis + Kafka + OpenSearch + Cloudflare Workers + FusionAuth. Self-hosting pesado para times pequenos.
2. **Dependência de Cloudflare:** Sync de docs depende de Durable Objects. Self-hosting requer substituir essa camada (não trivial).
3. **Sem workflow engine explícito:** Orquestração emerge de eventos + cron + CRDT. Workflows complexos multi-step não têm primitiva dedicada (state machine, retry policy, compensation).
4. **AGPLv3:** Licença copyleft forte. Comercialização de derivados exige licença separada.
5. **Documentação de agentes limitada:** Docs públicos de `/product/agents` bloqueados via web_extract. Detalhes de runtime, tool schema e memory synthesis não são totalmente transparentes.
6. **Frontend SolidJS:** Ecossistema menor que React/Vue. Hiring pool reduzido.
7. **Estado local frágil:** LocalStack perde recursos temporários em restart; requer reprovisionamento manual (`cargo x localstack-provision`).

---

## 8. Lições Aplicáveis ao Zylos-Next

### Adotar Imediatamente

| Lição Macro | Aplicação Zylos-Next | Crate Alvo |
|-------------|---------------------|------------|
| **Regras ast-grep como CI gate** | Enforçar boundaries hexagonais em `zylos-soar`, `zylos-memory-system`, `zylos-bot-system`. Impedir que domain importe wasmtime/bollard/reqwest. | Novo: `tooling/ast-grep-rules/` |
| **Tracing OTLP end-to-end** | Instrumentar `zylos-daemon` → `zylos-sandbox` → `zylos-verification` com spans propagados. Correlacionar agent turn → sandbox exec → proof verification no mesmo trace. | `zylos-daemon`, todos os crates |
| **MCP server nativo** | Expor `zylos-skills`, `zylos-memory-system`, `zylos-hitl` via MCP HTTP. Permitir que Claude/Codex usem Zylos como tool surface. | Novo: `zylos-mcp-server` |
| **Seed scenarios para teste** | Criar `seed/scenarios/` com mundos sintéticos (agents, skills, memory states) para E2E testing do cockpit e daemon. | `zylos-cli seed-scenario` |
| **Init snapshots para dev** | Cache de DB migrations + Kafka topics + sandbox images para acelerar `zylos dev up`. | `zylos-cli stack` |
| **Memory synthesis cron** | Adaptar padrão de síntese noturna team-wide para `zylos-memory-system`: consolidar signals de SOAR, HITL, verification em memória estruturada. | `zylos-memory-system` |

### Adaptar com Cautela

| Padrão Macro | Consideração Zylos-Next |
|--------------|------------------------|
| **CRDT para estado compartilhado** | Útil para `zylos-live-comm` e cockpit colaborativo, mas overkill para estado de agente/sandbox. Avaliar Yjs/Loro apenas para superfícies multi-user. |
| **42 microsserviços** | Zylos tem 14 crates. Manter monolítico modular até que escala force split. Premature decomposition mata produtividade. |
| **Cloudflare Durable Objects** | Zylos usa WASM/Docker sandbox. Não replicar dependência de vendor. Se precisar de atores persistentes, usar Actix/axum + SQLite embedded ou FoundationDB. |
| **Bidirectional graph como DB edges** | Poderoso mas complexo. Zylos pode começar com foreign keys + materialized views. Migrar para grafo só se traversal reversa virar gargalo. |

### Evitar

| Anti-padrão Macro | Motivo Zylos-Next |
|-------------------|-------------------|
| **Orquestração implícita via eventos** | Zylos já tem `zylos-soar` como workflow engine explícito. Manter state machines declarativas, não emergentes. |
| **AGPLv3 para plataforma agent** | Zylos deve ser MIT/Apache para maximizar adoção e integração comercial. |
| **Dependência de SaaS para core** | Sync, auth, storage devem ser self-hostable sem vendor lock-in. Macro depende de Cloudflare + FusionAuth; Zylos deve rodar em bare metal. |

### Gap Analysis: O Que Macro Tem Que Zylos Precisa Construir

1. **Team Memory Synthesis:** `zylos-memory-system` atual é likely event-store. Falta a camada de síntese periódica que consolida sinais brutos em memória acionável.
2. **MCP Server:** Zylos expõe skills internamente, mas não via protocolo MCP padronizado. Priorizar `zylos-mcp-server` para interoperabilidade.
3. **Agent-as-Peer em Docs:** Se Zylos tiver documentação colaborativa (cockpit notes, runbooks), considerar CRDT + agent editing. Se for só dashboard, skip.
4. **Debug Browser para Agentes:** `zylos-browser` existe, mas falta VNC/live-view integration para observar agentes navegando. Adicionar CDP endpoint + noVNC.
5. **CI Gates com ast-grep:** Zylos provavelmente usa clippy/fmt. Adicionar regras customizadas para enforçar arquitetura (ex: `zylos-sandbox` não importa `zylos-cockpit`).

---

## Apêndice: Referências Chave

- **Repositório:** https://github.com/macro-inc/macro
- **Licença:** AGPLv3
- **Arquitetura Rust:** `.agents/rules/rust/architecture.md` (CS-23, CS-25, CS-27, CS-30, CS-31, CS-51, CS-52, CS-53)
- **Coding Rules:** `CODING.md` (CS-24, CS-26, CS-28, CS-29, CS-32–35, CS-71)
- **Style Guide:** `docs/development/style-guide.md`
- **Running Locally:** `docs/development/running-locally.md`
- **Contributing:** `CONTRIBUTING.md`
- **MCP Setup:** https://docs.macro.com/AI/mcp/overview
- **Agent Recipes:** https://docs.macro.com/AI/recipes

---

*Relatório gerado via engenharia reversa de fontes públicas. Análise de código-fonte completo bloqueada por timeout de clone; conclusões baseadas em README, docs de desenvolvimento, regras de agente e documentação pública.*