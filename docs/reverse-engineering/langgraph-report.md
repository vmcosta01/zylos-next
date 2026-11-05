# Engenharia Reversa: LangGraph

**Data:** 2026-11-04
**Repositório Analisado:** [langchain-ai/langgraph](https://github.com/langchain-ai/langgraph)
**Objetivo:** Extrair padrões de arquitetura, orquestração durável e gestão de estado para informar o design do Zylos-Next (petgraph + SQLite WAL).

---

## 1. Arquitetura e Stack Principal

LangGraph é um runtime de orquestração de baixo nível inspirado no modelo **Pregel** (Google) e **Apache Beam**, com interface pública inspirada em NetworkX. Diferente de frameworks de alto nível, ele não abstrai prompts ou arquitetura de agentes; foca exclusivamente em execução durável, streaming e controle de fluxo stateful.

### Stack Tecnológica
- **Linguagem:** Python 3.9+ / TypeScript (monorepo com `libs/langgraph`, `libs/sdk-py`, `libs/sdk-js`).
- **Runtime Core:** Implementação própria do loop Pregel (`pregel/main.py` com ~4500 linhas), usando message passing e super-steps discretos.
- **Persistência:** Abstrações pluggáveis via `BaseCheckpointSaver`. Implementações oficiais: Memory (dev), SQLite (aiosqlite), PostgreSQL (psycopg).
- **Serialização:** `JsonPlusSerializer` customizado com suporte a tipos Python nativos, dataclasses, Pydantic e criptografia opcional (`EncryptedSerializer`).
- **Streaming:** Pipeline de transformadores (`StreamTransformer`) que projeta eventos brutos do grafo em streams ergonômicos (values, updates, messages, lifecycle, debug).
- **Dependências Chave:** `langchain-core` (Runnables, callbacks), `pydantic` (validação de estado), `typing_extensions`.

### Modelo de Execução (Pregel)
O grafo executa em **super-steps** discretos:
1. Nós começam inativos; ativam ao receber mensagem em canal de entrada.
2. Nó ativo executa função, envia atualizações via canais.
3. Fim do super-step: nós sem mensagens votam `halt`.
4. Termina quando todos inativos e sem mensagens em trânsito.

Isso garante determinismo e permite checkpointing consistente entre super-steps.

---

## 2. Componentes Principais e Responsabilidades

| Componente | Caminho | Responsabilidade |
|---|---|---|
| `StateGraph` | `libs/langgraph/langgraph/graph/state.py` | Builder do grafo: define schema de estado, nós, arestas, branches. Compila para `CompiledStateGraph`. |
| `Pregel` | `libs/langgraph/langgraph/pregel/main.py` | Runtime executável: loop de super-steps, leitura/escrita de canais, checkpointing, streaming, retry, timeout. |
| `BaseChannel` | `libs/langgraph/langgraph/channels/` | Primitivas de comunicação entre nós. Tipos: `LastValue`, `BinaryOperatorAggregate` (reducers), `Topic` (pub/sub), `EphemeralValue`, `DeltaChannel` (beta), `NamedBarrierValue`. |
| `BaseCheckpointSaver` | `libs/checkpoint/langgraph/checkpoint/base/__init__.py` | Interface de persistência: `get`, `put`, `list`, `put_writes`. Suporta `thread_id` como chave primária. |
| `BaseStore` | `libs/checkpoint/langgraph/store/base/__init__.py` | Memória de longo prazo cross-thread: namespaces hierárquicos, busca vetorial opcional, operações Get/Put/Search/List. |
| `StreamTransformer` | `libs/langgraph/langgraph/stream/` | Transformadores de stream: `ValuesTransformer`, `UpdatesTransformer`, `MessagesTransformer`, `LifecycleTransformer`, `SubgraphTransformer`, `DebugTransformer`. |
| `ManagedValue` | `libs/langgraph/langgraph/managed/` | Valores gerenciados pelo runtime (ex: caches, contadores) com lifecycle atrelado ao grafo. |
| SDK | `libs/sdk-py/`, `libs/sdk-js/` | Clientes HTTP/WebSocket para LangGraph Platform (deploy remoto). |
| Prebuilt | `libs/prebuilt/` | Agentes pré-construídos (`create_react_agent`), `ToolNode`, validadores. |

---

## 3. Padrões de Orquestração de Agentes/Workflows

### StateGraph e Compilação
- `StateGraph` é um builder genérico (`Generic[StateT, ContextT, InputT, OutputT]`).
- Schema de estado definido via `TypedDict` ou `Pydantic BaseModel`.
- Cada chave do estado pode ter um **reducer** anotado (`Annotated[list, reducer]`) para agregar writes concorrentes.
- `.compile()` valida estrutura (nós órfãos, ciclos), injeta checkpointer/store/cache, retorna `CompiledStateGraph` executável.
- Suporta schemas separados de input/output para encapsulamento.

### Checkpoints e Durabilidade
- Checkpoint = snapshot imutável com: `id` (UUID6 monotônico), `ts`, `channel_values`, `channel_versions`, `versions_seen`, `pending_sends`.
- `CheckpointMetadata` rastreia origem (`input`/`loop`/`update`/`fork`), step, parents, run_id.
- **DeltaChannel** (beta): armazena apenas deltas entre snapshots, com frequência configurável (`snapshot_frequency`) e limite de super-steps (`DELTA_MAX_SUPERSTEPS_SINCE_SNAPSHOT=5000`). Reduz custo de I/O para estados grandes.
- Writes são ordenados por `(task_path, task_id, idx)` para garantir consistência em reducers sensíveis a ordem.
- Time-travel: qualquer checkpoint pode ser usado como ponto de partida para nova execução.

### Streaming
- Stream bruto do Pregel é projetado via transformadores encadeáveis.
- Modos: `values` (estado completo), `updates` (deltas), `messages` (LLM tokens/tool calls), `events` (lifecycle), `debug` (tasks/writes), `custom` (user-defined).
- `StreamMux` multiplexa streams de subgrafos.
- Suporte a `astream_events(version="v3")` compatível com LangChain callbacks.

### Human-in-the-Loop
- `interrupt()`: pausa execução, serializa estado no checkpoint, retorna `Interrupt` ao caller.
- `Command(resume=value)`: retoma de um interrupt específico.
- Breakpoints configuráveis via `compile(breakpoints=[...])`.
- `update_state()`: modifica estado manualmente em qualquer checkpoint (cria fork).

### Subgrafos e Composição
- Grafos podem ser aninhados como nós de um grafo pai.
- Namespace de checkpoint isolado (`checkpoint_ns`).
- Streams de subgrafos propagados via `SubgraphTransformer`.

---

## 4. Sistema de Memória e Estado

### Memória de Curto Prazo (Thread-scoped)
- Gerenciada automaticamente pelo checkpointer via `thread_id`.
- Estado inclui histórico de mensagens, artefatos, arquivos uploadados.
- Técnicas de gestão: trimming de mensagens, resumo, filtragem por relevância.
- Lida no início de cada super-step; escrita ao fim.

### Memória de Longo Prazo (Cross-thread)
- `BaseStore`: KV store com namespaces hierárquicos (`("users", "profiles")`).
- Tipos de memória suportados:
  - **Semântica:** fatos sobre usuário/domínio (profile JSON ou coleção de documentos).
  - **Episódica:** experiências passadas (few-shot examples, trajetórias).
  - **Procedural:** instruções/skills (system prompt dinâmico).
- Busca: filtro por conteúdo + busca vetorial opcional (via `Embeddings`).
- Escrita: síncrona (hot path) ou assíncrona (background task).
- `Item` e `SearchItem` com metadados temporais e score de relevância.

### Canais como Primitivas de Estado
- `LastValue`: valor mais recente (default).
- `BinaryOperatorAggregate`: aplica reducer customizado (`(old, new) -> merged`).
- `Topic`: pub/sub fan-out para múltiplos consumidores.
- `EphemeralValue`: vive apenas um super-step.
- `NamedBarrierValue`: sincronização explícita entre nós paralelos.
- `DeltaChannel`: armazenamento incremental (beta).

---

## 5. Integrações e Conectores

- **LangChain Core:** Runnables, callbacks, embeddings, modelos de chat.
- **LangSmith:** Tracing nativo via callbacks, avaliação, deployment.
- **Checkpointer Backends:** Memory, SQLite (sync/async), PostgreSQL (sync/async), Redis (cache).
- **Store Backends:** Memory, SQLite, PostgreSQL.
- **SDK:** Clientes Python/JS para LangGraph Platform (REST + WebSocket).
- **CLI:** `langgraph-cli` para dev server, build, deploy.
- **Deep Agents:** Harness de alto nível sobre LangGraph (planning, subagents, filesystem tools).
- **MCP:** Docs conectáveis a agentes via MCP server.

---

## 6. Capacidades de Observabilidade e Debug

- **LangSmith Tracing:** Callbacks automáticos capturam spans de nós, tarefas, tool calls, LLM tokens.
- **Debug Stream:** Modo `stream_mode="debug"` expõe tasks, writes, reads por super-step.
- **Visualization:** `draw_graph()` gera representações visuais do grafo (Mermaid/Graphviz).
- **Metadata em Runtime:** `config["metadata"]` expõe `langgraph_step`, `langgraph_node`, `langgraph_triggers`, `langgraph_path`, `langgraph_checkpoint_ns`.
- **Recursion Limit:** Contador de steps com handling proativo (`RemainingSteps`) ou reativo (`GraphRecursionError`).
- **Error Handlers:** Nós de erro customizáveis por nó ou globais (`set_node_defaults(error_handler=...)`).
- **State Inspection:** `get_state()`, `get_state_history()`, `update_state()` para debugging interativo.

---

## 7. Pontos Fortes e Fracos

### Fortes
- **Durabilidade real:** Checkpointing entre super-steps garante recuperação exata após falha.
- **Flexibilidade extrema:** Mix de lógica determinística e agêntica no mesmo grafo.
- **Abstrações bem definidas:** Channel/Reducer/Checkpoint/Store são extensíveis sem fork.
- **Streaming de primeira classe:** Transformadores composáveis, não afterthought.
- **Human-in-the-loop nativo:** Interrupt/resume/update_state como primitivas do runtime.
- **Ecossistema maduro:** SDK, CLI, LangSmith, prebuilts, documentação extensa.
- **DeltaChannel:** Otimização de I/O para estados grandes (ainda beta).

### Fracos
- **Complexidade interna:** `pregel/main.py` tem ~4500 linhas; curva de aprendizado íngreme para contribuidores.
- **Acoplamento a langchain-core:** Dependência de Runnables/callbacks dificulta uso standalone.
- **Performance Python:** Loop Pregel em Python puro; overhead significativo vs runtime compilado.
- **DeltaChannel beta:** API instável, não recomendada para produção ainda.
- **Documentação fragmentada:** Conceitos espalhados entre docs site, README, docstrings.
- **Overhead de serialização:** JsonPlusSerializer é flexível mas lento para estados grandes.
- **Multi-tenancy limitada:** Thread isolation depende do backend; sem namespace enforcement nativo.

---

## 8. Lições Aplicáveis ao Zylos-Next

### Adotar Diretamente
1. **Modelo Pregel com Super-steps:** Zylos-Next já usa petgraph; implementar execução por super-steps discretos habilita checkpointing consistente e paralelismo seguro.
2. **Channels com Reducers:** Em vez de estado global mutável, usar canais tipados com reducers (`BinaryOperatorAggregate`) para agregar writes concorrentes deterministicamente.
3. **Checkpoint Tuple Estruturada:** Adotar esquema similar: `{id, ts, channel_values, channel_versions, versions_seen, pending_writes}`. SQLite WAL é ideal para isso.
4. **Interrupt/Resume como Primitiva:** Implementar `interrupt()` que serializa estado e `Command(resume=...)` que retoma. Essencial para human-in-the-loop e approval workflows.
5. **Streaming via Transformadores:** Separar stream bruto do runtime de projeções ergonômicas. Zylos pode oferecer modos `values`, `updates`, `events` sem acoplar consumers ao core.
6. **Store Cross-thread:** Memória de longo prazo deve ser separada do checkpointer, com namespaces hierárquicos e busca vetorial opcional.

### Adaptar
7. **DeltaChannel Simplificado:** Zylos pode implementar versão estável de delta storage desde o início, dado que SQLite WAL já suporta append-only eficiente. Evitar complexidade beta do LangGraph.
8. **Serialização Binária:** Em vez de JSON, usar MessagePack/CBOR ou protobuf para checkpoints. SQLite armazena blobs eficientemente; reduz overhead vs JsonPlusSerializer.
9. **Grafo como Configuração Serializável:** LangGraph compila grafos em runtime. Zylos poderia suportar definição declarativa (YAML/TOML) de grafos para hot-reload sem recompilação Rust.
10. **Observabilidade Nativa:** Integrar tracing estruturado (OpenTelemetry) diretamente no loop Pregel, não como callback externo. Rust tem vantagem de performance aqui.

### Evitar
11. **Dependência de Framework Externo:** LangGraph depende de langchain-core. Zylos deve manter core independente; integrações com LLM frameworks devem ser adaptadores opcionais.
12. **Complexidade Prematura:** `pregel/main.py` mostra o custo de feature creep. Zylos deve começar com subset mínimo (StateGraph + Checkpoint + Interrupt) e expandir sob demanda.
13. **Python como Runtime Crítico:** Para workloads de alta vazão, considerar implementar o loop Pregel em Rust (petgraph já está lá). Python/JS podem ser bindings para definição de grafos, não execução.

### Prioridades para Zylos-Next
1. Implementar `CheckpointSaver` sobre SQLite WAL com schema derivado de `Checkpoint`/`CheckpointTuple`.
2. Definir trait `Channel` em Rust com implementações `LastValue`, `Aggregate`, `Topic`.
3. Implementar loop Pregel mínimo em Rust com suporte a super-steps e checkpointing.
4. Adicionar primitiva `interrupt`/`resume` antes de streaming avançado.
5. Projetar `Store` separado com namespaces e índice vetorial opcional (usando sqlite-vss ou similar).

---

## Referências

- Repositório: https://github.com/langchain-ai/langgraph
- Documentação: https://langchain-ai.github.io/langgraph/
- Paper Pregel: https://research.google/pubs/pub37252/
- Apache Beam: https://beam.apache.org/
- CoALA (Memory Types): https://arxiv.org/pdf/2309.02427