# Relatório de Engenharia Reversa: Morphic AI Search Engine

**Data:** 2026-11-04
**Repositório Analisado:** [miurla/morphic](https://github.com/miurla/morphic) (v1.9.0)
**Objetivo:** Análise profunda de arquitetura para extrair lições aplicáveis ao Zylos-Next

---

## 1. Arquitetura e Stack Principal

### Stack Tecnológico
- **Runtime:** Next.js 16.2.6 (App Router) + React 19.3.1 + TypeScript 5.x
- **AI Core:** Vercel AI SDK v7 (`ai` package) com `ToolLoopAgent`
- **Banco de Dados:** PostgreSQL 17 + Drizzle ORM 0.45.2 + RLS nativo
- **Cache/Fila:** Redis (Upstash ou local) para cache SearXNG e rate-limiting
- **Auth:** Better Auth (built-in) ou Supabase Auth (alternativo)
- **Observabilidade:** OpenTelemetry + Langfuse (tracing) + PostHog (analytics)
- **UI:** Tailwind CSS 4 + shadcn/ui (Radix primitives) + Generative UI via `@json-render/react`
- **Storage:** AWS S3 (via `@aws-sdk/client-s3`) para uploads de arquivos
- **Search Providers:** Tavily, SearXNG, Brave, Exa, Firecrawl (arquitetura pluginável)
- **Deploy:** Docker Compose (PostgreSQL + Redis + SearXNG + App) ou Vercel

### Padrões Arquiteturais
- **Server-first:** API Routes como orquestradores principais; RSC para estado server-side
- **Streaming nativo:** SSE via `toUIMessageStreamResponse()` da AI SDK
- **Monorepo single-app:** Tudo em um único repositório Next.js (sem packages separados)
- **Schema-driven:** Zod para validação runtime, Drizzle para schema DB, JSON configs para modelos

---

## 2. Componentes Principais e Responsabilidades

### Camada de Orquestração (`app/api/chat/route.ts`)
- Validação de auth, rate-limiting (guest/adaptive/overall), usage budget enforcement
- Seleção dinâmica de modelo baseada em cookie `searchMode` (quick/adaptive)
- Delegação para `createChatStreamResponse` (usuários autenticados) ou `createEphemeralChatStreamResponse` (guests)
- Analytics não-bloqueante via `after()` hook do Next.js
- Refund de usage em caso de erro/abort

### Agente Researcher (`lib/agents/researcher.ts`)
- Implementado como `ToolLoopAgent` da AI SDK (não é um framework customizado)
- Dois modos de operação:
  - **Quick Mode:** maxSteps=20, tools=[search, fetch, editDraft], prompt otimizado
  - **Adaptive Mode:** maxSteps=50, tools=[search, fetch, todoWrite, editDraft], prompt completo
- Tools injetados via factory functions (`createSearchTool`, `createFetchTool`, etc.)
- Citation label allocator garante labels únicos e sequenciais entre tool calls paralelas
- Telemetry integrado via OTel context propagation

### Ferramentas do Agente (`lib/tools/`)
| Tool | Responsabilidade |
|------|-----------------|
| `search.ts` | Busca multi-provider com fallback automático, streaming de resultados, projection model-facing (trim content, remove metadata) |
| `fetch.ts` | Recuperação de conteúdo de URLs específicas |
| `draft.ts` | Edição incremental de documentos via find/replace exato, versionamento por sessão |
| `todo.ts` | Lista de tarefas session-scoped para tracking de progresso em adaptive mode |
| `question.ts` | Clarificação ativa com o usuário |
| `dynamic.ts` | Tools runtime-defined (MCP e outros) |

### Streaming Pipeline (`lib/streaming/`)
- `create-chat-stream-response.ts`: Orquestra preparação de mensagens, truncamento de contexto, draft attachment, agent execution, persistência pós-stream
- Helpers especializados: `compactHistoricalMessages`, `capHistoricalAttachments`, `trimColdStartHistory`, `summarizeCarriedContext`, `persistStreamResults`
- Error handling granular: distingue tool failure vs stream error vs empty response vs cancellation

### Persistência (`lib/db/schema.ts`)
- Tabelas: `chats`, `messages`, `parts` (normalizada por tipo), `notes`, `files`, `feedback`, `usageGrants`, `usageEvents`
- RLS policies em todas as tabelas via `current_setting('app.current_user_id')`
- Full-text search via `tsvector` gerado automaticamente nas colunas `text_text` e `reasoning_text`
- Parts table usa discriminated union pattern: cada tipo de parte tem colunas específicas (`tool_search_input`, `tool_fetch_output`, etc.)

---

## 3. Padrões de Orquestração de Agentes/Workflows

### ToolLoopAgent Pattern
O Morphic **não** implementa um framework de agentes customizado. Usa `ToolLoopAgent` da Vercel AI SDK diretamente:

```typescript
const agent = new ToolLoopAgent({
  model: getModel(model),
  instructions: systemPrompt,
  tools,
  activeTools: activeToolsList,
  stopWhen: isStepCount(maxSteps),
  telemetry: { isEnabled, functionId, includeRuntimeContext }
})
```

### Workflow Implícito
Não há DAG explícito ou state machine. O workflow emerge do prompt + tools disponíveis:
1. Usuário envia mensagem → API route valida e seleciona modo
2. Mensagens históricas são preparadas (truncate, compact, attach draft)
3. Agent recebe mensagens + tools e decide autonomamente qual tool chamar
4. Loop continua até `maxSteps` ou agente produzir resposta final
5. Stream é persistido assincronamente via `onEnd` callback

### Search Mode Bifurcation
- **Quick:** Força `type: 'optimized'` no search tool via wrapper, limita tools, prompt mais curto
- **Adaptive:** Permite todos os tools, inclui todoWrite para planning, prompt com instruções de pesquisa profunda

### Draft Versioning
- Draft é reconstruído do histórico a cada turn (`buildDraftVersions`)
- Edit tool opera sobre snapshot imutável do início do turn
- Versões são incrementais; find/replace falho retorna lista de edits não-aplicados para retry

---

## 4. Sistema de Memória e Estado

### Memória de Conversa
- **Persistente:** PostgreSQL (chats/messages/parts) com RLS por usuário
- **Ephemeral:** Guests usam `createEphemeralChatStreamResponse` sem persistência
- **Context Window Management:** Truncamento adaptativo baseado em token estimates, cold-start history trimming, compaction de mensagens históricas

### Estado de Sessão
- **Todos:** Session-scoped via closure na factory `createTodoTools()` — não persiste entre requests
- **Draft:** Reconstruído do histórico de mensagens a cada request (stateless server)
- **Citation Labels:** Allocator stateful por request, seed derivado do histórico

### Memória de Longo Prazo
- **Notes:** Tabela dedicada para anotações do usuário vinculadas a chats/mensagens
- **Files:** Library de arquivos uploadados com metadata e object key no S3
- **Usage Grants:** Sistema de créditos/budget persistente com idempotency keys

### Ausências Notáveis
- ❌ Sem vector store / embeddings / RAG
- ❌ Sem memória semântica entre sessões
- ❌ Sem knowledge graph ou entity extraction
- ❌ Sem summarization automática de conversas antigas (apenas truncamento)

---

## 5. Integrações e Conectores

### AI Providers
- OpenAI, Anthropic, Google, Ollama, Vercel AI Gateway, OpenAI-compatible
- Configuração via `public/config/models.json` (id, providerId, enabled, toolCallType)
- Provider options pass-through (ex: `promptCacheKey` para OpenAI)

### Search Providers (Plugin Architecture)
```
lib/tools/search/providers/
├── base.ts          # Interface comum
├── tavily.ts        # Default
├── searxng.ts       # Self-hosted
├── brave.ts         # Multimedia support
├── exa.ts           # Neural search
├── firecrawl.ts     # Web scraping
└── index.ts         # Factory + DEFAULT_PROVIDER
```
- Fallback automático: se provider dedicado falha com erro recuperável, tenta optimized provider
- Advanced search via rota interna `/api/advanced-search` (SearXNG only)

### Autenticação
- Dual-mode: Better Auth (built-in, zero-config) ou Supabase Auth
- Anonymous mode para dev/personal use (compartilhado, inseguro)
- OAuth support via `lib/auth/oauth-config.ts`

### Storage
- AWS S3 para file uploads com presigned URLs
- Upload route com validação de limites por guest/user

### MCP (Model Context Protocol)
- Next.js 16 built-in MCP server em `/_next/mcp`
- `.mcp.json` para Next DevTools MCP externo
- Dynamic tools suportam MCP runtime-defined

---

## 6. Capacidades de Observabilidade/Debug

### Tracing (Langfuse + OTel)
- Root span `research` envolve agent stream + title generation
- Spans propagam via OTel context (AI SDK integration automática)
- Metadata rico: chatId, userId, modelId, searchMode, trigger
- Error classification: phase (preparation/generation), stage (prepare-messages/build-agent/start-stream), cancelled flag
- Force-flush antes de serverless function exit

### Performance Logging
- `perfLog`/`perfTime` helpers com timestamps relativos
- Contadores resetáveis por request (dev only)
- Summary log ao final de cada request: chat type, total time

### Analytics (PostHog)
- Eventos: chat, account, adaptive limit, chat limit
- Conversation turn calculation, query shape derivation
- Guest attribution via client-provided distinct ID
- Non-blocking via async IIFE

### Usage Budget System
- Enforcement modes: on/shadow/off
- Spend/refund/limit_reached events persistidos em `usage_events`
- Idempotency via `usage_attempt_id`
- Refund handler executado em `after()` hook ou on error

### Error Handling
- Public error serialization (não expõe internals)
- Tool failure vs stream error distinction
- Empty response detection e refund
- Stream error diagnostics com phase/stage tracking

---

## 7. Pontos Fortes e Fracos

### ✅ Pontos Fortes
1. **Simplicidade arquitetural:** Um único agente + tools, sem over-engineering de frameworks
2. **Streaming first-class:** Toda a pipeline é desenhada para streaming desde o início
3. **Generative UI real:** Não é apenas markdown — componentes React renderizados via spec JSON streamed
4. **RLS nativo no PostgreSQL:** Segurança no nível do banco, não apenas na aplicação
5. **Search provider abstraction limpa:** Interface comum + factory + fallback automático
6. **Draft versioning robusto:** Find/replace exato com retry feedback, reconstrução stateless
7. **Observabilidade production-grade:** OTel + Langfuse + PostHog integrados organicamente
8. **Usage budget enforcement:** Sistema completo de créditos com idempotency e refund
9. **Context window management sofisticado:** Truncamento adaptativo, cold-start trimming, compaction
10. **Type safety end-to-end:** Zod schemas → Drizzle types → TypeScript types alinhados

### ❌ Pontos Fracos
1. **Sem memória semântica:** Apenas histórico cronológico truncado, sem embeddings/RAG
2. **Single-agent architecture:** Não há especialização, delegação entre agentes, ou parallelismo
3. **Stateless sessions:** Todos/drafts reconstruídos a cada request, sem cache de estado intermediário
4. **No sandboxing:** Code execution não existe; fetch tool acessa URLs diretamente sem isolamento
5. **No verification layer:** Respostas não são verificadas contra fontes ou ground truth
6. **Coupling com Vercel AI SDK:** Migração para outro framework seria rewrite significativo
7. **Limited HITL:** Apenas clarification questions; sem approval gates ou human review workflows
8. **No multi-modal reasoning:** File uploads existem mas não há vision/audio processing no agente
9. **Search-only focus:** Arquitetura não generaliza para outros domínios sem modificação pesada
10. **Monolithic deployment:** Single Next.js app; scaling requer replicar tudo

---

## 8. Lições Aplicáveis ao Zylos-Next

### 🎯 Adotar Diretamente

| Lição Morphic | Aplicação no Zylos-Next | Crate Alvo |
|---------------|------------------------|------------|
| **Tool output projection** (`toModelOutput`) | Trim tool results antes de enviar ao LLM; remover metadata, duplicatas, conteúdo UI-only | `zylos-soar` |
| **Citation label allocator** | Alocador thread-safe de labels sequenciais para citações em buscas paralelas | `zylos-search-mcp` |
| **Error classification (phase/stage)** | Classificar erros de agente por fase (prep/generation/tool) e estágio para debugging | `zylos-verification` |
| **Usage budget + idempotency** | Sistema de créditos/budget com attempt IDs e refund handlers | `zylos-gateway` |
| **Draft versioning via find/replace** | Edição incremental de artefatos com retry feedback e versionamento | `zylos-skills` |
| **RLS patterns no PostgreSQL** | Policies declarativas usando session variables para multi-tenancy | `zylos-memory-system` |
| **OTel span propagation** | Propagar trace context entre crates Rust via OTel baggage | `zylos-live-comm` |
| **Stream error diagnostics** | Metadata estruturado em spans de erro: phase, stage, cancelled flag | `zylos-cockpit` |

### ⚠️ Adaptar com Cautela

| Padrão Morphic | Considerações para Zylos-Next |
|----------------|-------------------------------|
| **ToolLoopAgent (single loop)** | Zylos já tem SOAR multi-fase; usar loop simples apenas dentro de fases individuais, não como orquestrador global |
| **Session-scoped state via closures** | Em Rust, usar Arc<Mutex<>> ou channels; evitar state mutável compartilhado sem sincronização |
| **Generative UI via JSON spec** | Cockpit Leptos pode consumir specs similares, mas precisa de schema validation rigorosa (serde) |
| **Anonymous mode** | Acceptable para dev; em produção, sempre exigir auth. Zylos bot-system já tem identity — estender para guests |
| **Search fallback chain** | Útil para `zylos-search-mcp`, mas implementar como strategy pattern trait-based, não hardcoded |

### 🚫 Evitar / Melhorar

| Limitação Morphic | Como Zylos-Next Deve Diferir |
|-------------------|------------------------------|
| **Sem memória semântica** | `zylos-memory-system` já prevê embeddings; implementar RAG desde o início |
| **Single agent, no delegation** | Zylos SOAR já orquestra múltiplos agentes; manter essa vantagem |
| **No sandboxing** | `zylos-sandbox` (WASM/Docker) já resolve isso; nunca executar código untrusted fora do sandbox |
| **No verification** | `zylos-verification` + `zylos-proof` são diferenciais; manter verification como gate obrigatório |
| **No HITL workflows** | `zylos-hitl` já implementa approval gates; expandir para além de clarification questions |
| **Stateless reconstruction** | Cache de estado intermediário em `zylos-memory-system` para evitar rebuilds custosos |
| **Framework coupling** | Zylos é Rust-native; abstrações devem ser trait-based, não dependentes de um único ecossistema |

### 💡 Insights Estratégicos

1. **Simplicidade vence:** Morphic prova que um único agente bem-instrumentado + tools sólidos > framework complexo. Zylos deve resistir à tentação de over-engineer o SOAR.

2. **Streaming é infraestrutura, não feature:** Todo o pipeline do Morphic assume streaming. Zylos-live-comm deve tratar streaming como primitive, não como adaptação posterior.

3. **Observabilidade não é afterthought:** Langfuse/OTel estão integrados no core do Morphic, não adicionados depois. Zylos deve instrumentar desde o crate level.

4. **Generative UI é o diferencial real:** A capacidade de renderizar componentes dinâmicos (não apenas texto) é o que torna Morphic único. Zylos-cockpit deve investir em specs declarativas para UI gerada por agentes.

5. **Usage enforcement é produto:** O sistema de budget/credits do Morphic é production-ready. Zylos-gateway deve ter isso como feature de plataforma, não como hack.

6. **Draft/edit pattern é universal:** O padrão find/replace com versionamento funciona para documentos, código, configs. Zylos-skills pode adotar como primitive para edição de artefatos.

7. **Search provider abstraction escala:** A interface pluginável do Morphic permite adicionar providers sem modificar o core. Zylos-search-mcp deve seguir o mesmo padrão trait-based.

---

## Apêndice: Estrutura de Arquivos Chave

```
morphic/
├── app/
│   ├── api/chat/route.ts              # Entry point principal
│   ├── api/search/route.ts            # Search standalone
│   └── search/[id]/page.tsx           # Share page
├── lib/
│   ├── agents/
│   │   ├── researcher.ts              # ToolLoopAgent factory
│   │   └── prompts/search-mode-prompts.ts
│   ├── streaming/
│   │   ├── create-chat-stream-response.ts  # Core orchestration
│   │   └── helpers/                   # 20+ helpers especializados
│   ├── tools/
│   │   ├── search.ts                  # Multi-provider search
│   │   ├── search/providers/          # Plugin implementations
│   │   ├── draft.ts                   # Document editing
│   │   ├── todo.ts                    # Task tracking
│   │   └── fetch.ts                   # URL retrieval
│   ├── db/
│   │   ├── schema.ts                  # Drizzle schema + RLS
│   │   └── actions.ts                 # DB operations
│   └── auth/                          # Better Auth + Supabase
├── components/
│   ├── artifact/                      # Generative UI renderers
│   └── chat.tsx                       # Main chat interface
└── instrumentation.ts                 # OTel + Langfuse setup
```

---

*Relatório gerado via engenharia reversa estática do repositório miurla/morphic @ v1.9.0. Análise complementar com REA tools (binary analysis, runtime inspection) não foi necessária dado que o projeto é open-source TypeScript/Next.js com código-fonte completo disponível.*