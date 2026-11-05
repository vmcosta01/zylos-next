# Engenharia Reversa: LangChainJS

**Data:** 2026-11-03
**Repositório:** https://github.com/langchain-ai/langchainjs
**Versão analisada:** HEAD (monorepo pnpm + Turborepo)
**Objetivo:** Extrair padrões de arquitetura, orquestração, memória e observabilidade aplicáveis ao Zylos-Next (Rust).

---

## 1. Arquitetura e Stack Principal

### Stack
- **Linguagem:** TypeScript (ES2022, ESM-first com suporte CJS)
- **Gerenciador:** pnpm v10.14.0 workspaces
- **Build:** Turborepo (`turbo.json`) + tsdown (bundler baseado em Rolldown/esbuild)
- **Lint/Format:** oxlint + oxfmt (substitutos Rust-based de ESLint/Prettier)
- **Runtime support:** Node.js 20/22/24, Deno, Bun, Cloudflare Workers, Vercel Edge, Browser
- **Schema validation:** Zod v3 e v4 (interop layer em `libs/langchain-core/src/utils/types/zod.ts`)
- **Testes:** Jest + standard tests suite (`@langchain/standard-tests`)

### Estrutura do Monorepo
```
libs/
├── langchain-core/        # Abstrações base: Runnable, callbacks, messages, tools
├── langchain/             # Orquestração de alto nível: agents, prompts, chains
├── langchain-textsplitters/
├── langchain-mcp-adapters/
├── langchain-classic/     # Compatibilidade retroativa
└── providers/             # 30+ integrações first-party
    ├── langchain-openai/
    ├── langchain-anthropic/
    ├── langchain-google-genai/
    └── ...
internal/
├── build/                 # Utilitários de build compartilhados
└── tsconfig/              # Configuração TS base
```

### Padrão de Camadas
O projeto segue uma separação rígida entre **core** (interfaces estáveis, zero dependências externas) e **implementação** (providers, agentes). O pacote `langchain` depende de `@langchain/core`, mas nunca o inverso. Providers dependem apenas de `@langchain/core`.

---

## 2. Componentes Principais e Responsabilidades

### `@langchain/core` (libs/langchain-core/src/)
| Módulo | Responsabilidade |
|--------|-----------------|
| `runnables/base.ts` (3566 linhas) | Classe `Runnable`: unidade composável universal com `.invoke()`, `.stream()`, `.batch()`, `.pipe()` |
| `messages/` | Tipos de mensagem padronizados (Human, AI, Tool, System) com serialização |
| `tools/` | Interface `Tool` + validação Zod de input/output |
| `callbacks/` | Sistema de eventos distribuídos para tracing/logging |
| `tracers/` | Implementações concretas: LangSmith, console, event stream, log stream |
| `language_models/` | Abstrações `BaseChatModel`, `BaseLLM` com streaming nativo |
| `vectorstores.ts` | Interface `VectorStore` para RAG |
| `retrievers/` | Interface `BaseRetriever` |
| `prompts/` | Templates de prompt com variáveis tipadas |
| `output_parsers/` | Parsing estruturado de saída LLM |

### `langchain` (libs/langchain/src/)
| Módulo | Responsabilidade |
|--------|-----------------|
| `agents/ReactAgent.ts` (1643 linhas) | Implementação ReAct sobre StateGraph do LangGraph |
| `agents/state.ts` | `StateManager`: compartilha estado entre nós de middleware |
| `agents/middleware/` | 20+ middlewares: retry, fallback, PII redaction, HITL, summarization, tool limits |
| `agents/nodes/` | Nós do grafo: AgentNode, ToolNode, Before/After hooks |
| `chat_models/` | Wrappers de modelos de chat |
| `tools/` | Ferramentas built-in (search, code exec, etc.) |

---

## 3. Padrões de Orquestração de Agentes/Workflows

### Runnable como Unidade Universal
Tudo é `Runnable<Input, Output, Config>`:
```typescript
// Composição via pipe
const chain = prompt.pipe(model).pipe(outputParser);

// Cada Runnable tem:
// - invoke(input, config?) → output
// - stream(input, config?) → AsyncGenerator<chunk>
// - batch(inputs[], config?) → outputs[]
// - withConfig(config) → novo Runnable com config merged
```

### StateGraph (LangGraph Integration)
O `ReactAgent` é construído sobre `StateGraph` do `@langchain/langgraph`:
- **Nós:** `model_request` → `tools` → `END` (ciclo ReAct)
- **Estado tipado:** `createAgentState()` gera annotation Zod com mensagens + estado customizado + middleware states
- **Middleware pipeline:** Antes/depois de cada nó, middlewares podem interceptar, modificar estado, ou redirecionar fluxo (`jumpTo`)
- **Command pattern:** Nós podem retornar `Command` para controle explícito de roteamento

### Middleware Architecture
```
libs/langchain/src/agents/middleware/
├── modelRetry.ts          # Retry com backoff em falhas de modelo
├── modelFallback.ts       # Fallback para modelo alternativo
├── modelCallLimit.ts      # Limite de chamadas por run
├── toolRetry.ts           # Retry de tool calls
├── toolCallLimit.ts       # Limite de tool calls
├── toolError.ts           # Tratamento de erros de ferramenta
├── hitl.ts                # Human-in-the-loop approval gates
├── piiRedaction.ts        # Redação de PII antes do modelo
├── summarization.ts       # Sumarização automática de contexto
├── dynamicSystemPrompt.ts # System prompt dinâmico por estado
├── contextEditing.ts      # Edição de contexto pré-modelo
├── llmToolSelector.ts     # Seleção dinâmica de ferramentas via LLM
└── todoListMiddleware.ts  # Tracking de tarefas do agente
```

Cada middleware implementa hooks (`beforeModel`, `afterModel`, `beforeTool`, `afterTool`) e mantém estado isolado via `StateManager`.

### Streaming Nativo
- `IterableReadableStream` wrapper sobre async generators
- `EventStreamCallbackHandler` emite `StreamEvent` tipados
- Suporte a `stream_mode`: values, updates, events, debug

---

## 4. Sistema de Memória e Estado

### Memória Legacy (BaseMemory)
```typescript
abstract class BaseMemory {
  abstract loadMemoryVariables(values: InputValues): Promise<MemoryVariables>;
  abstract saveContext(inputValues: InputValues, outputValues: OutputValues): Promise<void>;
}
```
Interface simples key-value. Usada em chains clássicas, sendo substituída por LangGraph checkpointer.

### Estado Moderno (LangGraph Checkpointing)
- `BaseCheckpointSaver` interface para persistência de estado do grafo
- `BaseStore` para memória de longo prazo cross-thread
- Estado é imutável por nó; reducers definem merge semantics
- `GetStateOptions` permite inspeção histórica de checkpoints

### StateManager (Agent-level)
```typescript
class StateManager {
  #nodes: Map<string, AgentNode[]>;
  addNode(middleware: AnyAgentMiddleware, node: AgentNode): void;
  getState(name: string): Record<string, unknown>;
}
```
Compartilha estado entre nós de um mesmo middleware group. Isola `jumpTo` (controle de fluxo interno) da visão externa.

### Chat History
- `ChatHistory` runnable injeta histórico de mensagens no prompt
- Serialização via `getBufferString()` ou transformadores customizados
- Window/buffer strategies em `langchain-textsplitters`

---

## 5. Integrações e Conectores

### Provider Pattern
Cada provider (`libs/providers/langchain-*`) exporta:
- `ChatModel` (ex: `ChatOpenAI`, `ChatAnthropic`)
- `Embeddings` (opcional)
- `Tools` específicos do provedor
- Types e utils próprios

### Adapter Architecture
- `@langchain/mcp-adapters`: Bridge para Model Context Protocol servers
- Conversores normalizam formatos de mensagem entre providers
- `decisions/` em OpenAI provider: routing logic para diferentes API versions

### Vector Stores & Retrievers
30+ integrações: Pinecone, Qdrant, Weaviate, pgvector, Redis, MongoDB Atlas, Neo4j, Chroma, etc.
Todas implementam `VectorStore` interface com `similaritySearch`, `addDocuments`, `delete`.

### Tool Ecosystem
- `ClientTool` vs `ServerTool`: distinção para execução local vs remota
- Validação Zod em runtime
- `wrapToolCall()` adiciona tracing e error handling automático

---

## 6. Capacidades de Observabilidade/Debug

### Callback System (libs/langchain-core/src/callbacks/)
Hierarquia de handlers herdáveis:
```
CallbackManager
├── handlers: BaseCallbackHandler[]      (local ao run atual)
└── inheritableHandlers: BaseCallbackHandler[]  (propagados para child runs)
```

Eventos cobertos: `handleLLMStart`, `handleLLMNewToken`, `handleLLMEnd`, `handleChainStart`, `handleChainEnd`, `handleToolStart`, `handleToolEnd`, `handleRetrieverStart`, `handleRetrieverEnd`.

### Tracers Concretos
| Tracer | Uso |
|--------|-----|
| `LangChainTracer` | Envia traces para LangSmith API |
| `ConsoleCallbackHandler` | Debug local no terminal |
| `LogStreamCallbackHandler` | Stream estruturado de logs (RunLog/RunLogPatch) |
| `EventStreamCallbackHandler` | Stream de eventos tipados para consumers |
| `RootListenersTracer` | Listeners no root run para métricas globais |
| `RunCollector` | Coleta runs completos para avaliação post-hoc |

### Trace Policy
`TracePolicy` controla quais runs são traceados (sampling, filtering por metadata/tags). Configurável via `TRACE_POLICY_KEY` no config.

### AsyncLocalStorage Context
Propagação automática de contexto via `AsyncLocalStorageProviderSingleton`: callbacks herdam config sem passagem explícita de parâmetros.

### Mermaid Graph Visualization
`Graph.drawMermaid()` e `drawMermaidImage()` geram visualizações do grafo de runnables para debug visual.

---

## 7. Pontos Fortes e Fracos

### Fortes
1. **Composabilidade extrema:** `Runnable.pipe()` permite composição declarativa de pipelines complexos
2. **Streaming first-class:** Não é afterthought; toda a cadeia suporta streaming nativo
3. **Middleware architecture:** Extensibilidade horizontal sem modificar core (retry, PII, HITL como plugins)
4. **Type safety profunda:** Zod schemas em runtime + TypeScript types em compile time
5. **Multi-runtime:** Funciona em Node, Deno, Bun, Workers, Browser sem adaptação
6. **Observabilidade integrada:** Tracing não é opcional; está woven into every Runnable
7. **Ecossistema massivo:** 30+ providers first-party, vector stores, tools
8. **Separação core/impl:** Estabilidade de interface com evolução independente

### Fracos
1. **Complexidade acidental:** `base.ts` com 3566 linhas; over-abstraction dificulta debugging
2. **Overhead de abstrações:** Múltiplas camadas de wrapping impactam performance cold-start
3. **Dual Zod support:** Manter compatibilidade v3+v4 adiciona complexidade e bundle size
4. **Memória legacy confusa:** `BaseMemory` coexiste com LangGraph checkpointing sem migração clara
5. **Error handling inconsistente:** Alguns middlewares swallow errors, outros rethrow; difícil prever comportamento
6. **Bundle size:** Monorepo grande com muitas dependências transitivas
7. **Documentação fragmentada:** Core docs vs LangGraph docs vs provider docs em lugares diferentes
8. **Testing surface area:** Standard tests são bons, mas integration tests exigem múltiplas API keys

---

## 8. Lições Aplicáveis ao Zylos-Next (Rust)

### Adotar
1. **Trait-based composability:** Equivalente Rust de `Runnable` seria um trait `Invocable<I, O>` com métodos `invoke`, `stream`, `batch`. Usar generics + trait objects para composição.
2. **Middleware como layers:** Tower-style middleware pattern já é idiomatic em Rust. Adaptar a arquitetura de middleware do LangChain (hooks before/after) para Axum/Tower layers.
3. **Structured tracing:** Implementar equivalente do callback system usando `tracing` crate com spans hierárquicos. LangSmith ≈ OpenTelemetry exporter.
4. **State machine explícita:** StateGraph do LangGraph é essencialmente uma FSM tipada. Em Rust, usar enums + match para estados do agente com transições verificadas em compile time.
5. **Schema validation em runtime:** Usar `serde` + `schemars` para validação equivalente ao Zod. Derive macros reduzem boilerplate.
6. **Streaming via async iterators:** `async-stream` crate ou `futures::Stream` trait como equivalente de `IterableReadableStream`.
7. **Provider abstraction:** Trait `ChatModel` com associated types para request/response. Feature flags para providers opcionais.

### Evitar
1. **Over-abstraction prematura:** Não replicar 3566-line base class. Preferir composition over inheritance (Rust natural).
2. **Dual schema version support:** Escolher uma versão de schema/validation library e manter. Breaking changes são aceitáveis em ecossistema jovem.
3. **Legacy memory patterns:** Começar direto com checkpointing/state persistence. Não implementar key-value memory simples que será descartado.
4. **Callback handler inheritance:** Rust não tem herança. Usar visitor pattern ou channel-based event distribution.
5. **Dynamic dispatch excessivo:** Preferir static dispatch com generics onde possível. `dyn Trait` apenas quando necessário.
6. **Monorepo complexity inicial:** Começar como single crate com feature flags. Split em workspace apenas quando houver 3+ consumidores independentes.

### Adaptações Específicas Rust
- **Zod interop → serde + schemars:** Generate JSON Schema from Rust types, validate at boundaries
- **AsyncLocalStorage → task_local! ou extensions:** Propagar contexto via tokio task locals ou axum extensions
- **Middleware hooks → Tower layers:** Before/after model/tool hooks mapeiam para layer::Service impl
- **StateGraph → petgraph + enum states:** Grafo de estados com tipos Rust nativos
- **LangSmith → OpenTelemetry + Grafana/Jaeger:** Stack aberta equivalente
- **pnpm workspaces → Cargo workspace:** Mesma semântica, tooling nativo

---

## Apêndice: Arquivos-Chave Inspecionados

| Arquivo | Linhas | Conteúdo |
|---------|--------|----------|
| `libs/langchain-core/src/runnables/base.ts` | 3566 | Runnable base class, composição, streaming |
| `libs/langchain/src/agents/ReactAgent.ts` | 1643 | Agente ReAct sobre StateGraph |
| `libs/langchain/src/agents/state.ts` | 55 | StateManager para middleware state sharing |
| `libs/langchain-core/src/callbacks/manager.ts` | 1571 | Callback hierarchy e propagação |
| `libs/langchain-core/src/memory.ts` | ~80 | BaseMemory legacy interface |
| `libs/langchain-core/src/agents.ts` | ~30 | AgentAction/AgentFinish/AgentStep types |
| `libs/langchain-core/src/runnables/graph.ts` | ~60 | Graph visualization (Mermaid) |
| `AGENTS.md` | ~150 | Guidelines de desenvolvimento e estrutura |
| `package.json` | ~80 | Scripts, deps, overrides de segurança |