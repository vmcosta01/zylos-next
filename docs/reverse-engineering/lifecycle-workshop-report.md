# Engenharia Reversa: LangSmith Agent Lifecycle Workshop

**Repositório:** https://github.com/langchain-ai/langsmith-agent-lifecycle-workshop
**Data da Análise:** 2026-11-04
**Foco:** Padrões de lifecycle, state machines e orquestração aplicáveis ao Zylos-Next (zylos-bot-system e zylos-soar)

---

## 1. Arquitetura e Stack Principal

O repositório é um workshop educacional enterprise que implementa o ciclo de vida completo de engenharia de IA usando o ecossistema LangChain. A arquitetura é centrada em um sistema de suporte ao cliente multi-agente para um e-commerce fictício (TechHub).

### Stack Tecnológica
- **Linguagem:** Python 3.10+
- **Orquestração:** LangGraph (StateGraph, Command, interrupt)
- **Abstração de Agentes:** LangChain (`create_agent`, `init_chat_model`)
- **Observabilidade:** LangSmith Platform (tracing, evals online/offline)
- **Persistência:** SQLite (dados estruturados), Pickle (vectorstores), MemorySaver (checkpoints em memória)
- **RAG:** HuggingFace Embeddings (padrão) ou OpenAI Embeddings
- **Gerenciamento de Dependências:** uv
- **Modelos LLM:** Anthropic Claude Haiku 4.5 (padrão), OpenAI GPT-5.4, com fallback cross-provider
- **Interface:** Jupyter Lab (desenvolvimento), LangGraph API (produção)

### Estrutura de Diretórios Chave
```
agents/           # Fábricas de agentes reutilizáveis
tools/            # Ferramentas de banco e RAG
evaluators/       # Métricas de avaliação (LLM-as-judge, trace-based)
deployments/      # Configurações de grafos para produção
workshop_modules/ # Notebooks didáticos (Módulos 1-6)
data/             # Dataset sintético TechHub + scripts de geração
config.py         # Configuração centralizada (modelos, paths, gateway)
langgraph.json    # Config de deployment do LangGraph
```

---

## 2. Componentes Principais e Responsabilidades

### Agentes (Padrão Factory Function)
Todos os agentes seguem o padrão `create_<nome>_agent()` com parâmetros opcionais para injeção de dependências:

| Agente | Arquivo | Responsabilidade |
|--------|---------|------------------|
| **DB Agent** | `agents/db_agent.py` | Consultas estruturadas (pedidos, produtos, estoque). Tools rígidas. |
| **SQL Agent** | `agents/sql_agent.py` | Geração flexível de SQL via LLM (evolução do DB Agent). |
| **Docs Agent** | `agents/docs_agent.py` | RAG sobre documentação de produtos e políticas. |
| **Supervisor Agent** | `agents/supervisor_agent.py` | Roteamento e coordenação entre sub-agentes via tool-calling. |
| **Supervisor HITL** | `agents/supervisor_hitl_agent.py` | Classificação de query + verificação de identidade + roteamento. |

### Ferramentas (`tools/database.py`, `tools/documents.py`)
- **Database:** `get_order_status`, `get_order_items`, `get_product_info`, `get_order_item_price`, `get_customer_orders`, `execute_sql` (com whitelist SELECT-only)
- **Documents:** `search_product_docs`, `search_policy_docs` (RAG)
- Conexão DB lazy-loaded via singleton `_db`

### Avaliadores (`evaluators/evaluators.py`)
- **Correctness Evaluator:** LLM-as-Judge comparando output vs reference_output (booleano)
- **Tool Call Counter:** Percorre trace recursivamente contando `run_type == "tool"`

---

## 3. Padrões de Orquestração de Agentes/Workflows

### Supervisor Pattern com Sub-Agentes como Tools
O supervisor não usa routing condicional explícito no grafo. Em vez disso, sub-agentes são embrulhados como `@tool`:

```python
@tool("database_specialist", description="...")
def call_database_specialist(query: str) -> str:
    result = db_agent.invoke({"messages": [{"role": "user", "content": query}]})
    return result["messages"][-1].content
```

Isso permite que o LLM do supervisor decida quando e qual sub-agente chamar via tool-calling nativo, sem edges condicionais hardcoded.

### StateGraph com Custom State Schema
O grafo HITL define schemas separados para input/state/output:

```python
workflow = StateGraph(
    input_schema=MessagesState,      # Entrada: apenas mensagens
    state_schema=IntermediateState,   # Interno: messages + customer_id
    output_schema=MessagesState,      # Saída: apenas mensagens
    context_schema=Context,           # Runtime config (model selection)
)
```

Chaves compartilhadas entre parent e subgraphs fluem automaticamente.

### Command para Routing Explícito
Nós retornam `Command[Literal["next_node_a", "next_node_b"]]` em vez de strings simples:

```python
def query_router(state, runtime) -> Command[Literal["verify_customer", "supervisor_agent"]]:
    if state.get("customer_id"):
        return Command(goto="supervisor_agent")
    classification = classify_query_intent(last_message.content)
    if classification["requires_verification"]:
        return Command(goto="verify_customer")
    return Command(goto="supervisor_agent")
```

### Human-in-the-Loop via `interrupt()`
Coleta de email usa primitiva `interrupt()` que pausa o grafo e espera input externo:

```python
def collect_email(state):
    user_input = interrupt(value="Please provide your email:")
    return Command(update={"messages": [HumanMessage(content=user_input)]}, goto="verify_customer")
```

### Dynamic Prompt Middleware
Injeção condicional de contexto no system prompt via middleware:

```python
@dynamic_prompt
def supervisor_prompt(request: ModelRequest) -> str:
    customer_id = request.state.get("customer_id")
    if customer_id:
        return f"{base_prompt}\n\nCustomer ID: {customer_id}"
    return base_prompt
```

---

## 4. Sistema de Memória e Estado

### Checkpointing Dual-Mode
- **Desenvolvimento:** `MemorySaver()` in-memory passado via `use_checkpointer=True`
- **Produção:** `use_checkpointer=False` — a plataforma LangGraph API gerencia persistência automaticamente

### State Schema Evolution
- Base: `MessagesState` (apenas lista de mensagens com reducers)
- Estendido: `IntermediateState(MessagesState)` adiciona `customer_id: str`
- Context: `Context` dataclass com `model: Literal[...]` para seleção dinâmica em runtime

### Persistência de Identidade
O `customer_id` verificado persiste no checkpoint entre turnos, evitando re-verificação. O estado flui automaticamente para subgrafos via chaves compartilhadas.

---

## 5. Integrações e Conectores

### LLM Gateway Multi-Provider
Suporte a três caminhos de credenciais com precedência definida:
1. API keys diretas (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`)
2. LangSmith LLM Gateway (`LANGSMITH_GATEWAY=true`)
3. Gateway env file (`GATEWAY_ENV_FILE`) para shells interativos

### Fallback Cross-Provider
Cadeia de fallback configurável: modelo principal → modelo do outro provider → modelo pequeno do mesmo provider. Verifica credenciais antes de incluir na cadeia.

### Vectorstore Configurável
Embeddings via HuggingFace (local, zero-cost) ou OpenAI, selecionado por `EMBEDDING_PROVIDER`. Paths adaptativos detectam deployment (`/deps/`) vs local.

### CI/CD Integration
GitHub Actions roda evaluators do Módulo 2 como gate de merge em PRs que tocam `agents/`, `tools/`, `evaluators/`, `deployments/`, `evals/`.

---

## 6. Capacidades de Observabilidade/Debug

### LangSmith Tracing Nativo
Todos os `create_agent` e invocações são automaticamente rastreados. O `count_total_tool_calls_evaluator` percorre a árvore de runs recursivamente.

### Eval-Driven Development
- **Offline:** Dataset baseline com correctness + tool call count
- **Online:** Annotation queues, data flywheel em produção
- **CI:** Regression gate bloqueante em PRs

### Structured Output para Classificação
Uso de `.with_structured_output(QueryClassification)` para decisões de routing tipadas e auditáveis, com campos `reasoning` e `requires_verification`.

### Verify Setup Script
`verify_setup.py` valida configuração de modelos e credenciais antes do workshop, mapeando erros 424/401/403 para fixes específicos.

---

## 7. Pontos Fortes e Fracos

### Fortes
- **Factory Pattern consistente:** Todos os agentes usam `create_*_agent()` com injeção de dependências, facilitando testes e composição
- **Separação dev/prod limpa:** `use_checkpointer` toggle + platform-managed persistence eliminam código condicional em produção
- **Eval como first-class citizen:** Evaluators vivem no repo, CI gate é merge-blocking, não opcional
- **Composição por substituição:** SQL agent substitui DB agent via injeção (`db_agent=sql_agent`) sem mudar o supervisor
- **HITL nativo:** `interrupt()` + `Command` são primitivas do framework, não hacks
- **Multi-provider resilience:** Fallback cross-provider com verificação de credenciais

### Fracos
- **SQL Injection vulnerável:** `tools/database.py` usa f-strings em queries SQL (`f"... WHERE order_id = '{order_id}'"`) — crítico para produção real
- **State acoplado a MessagesState:** Custom states devem estender `MessagesState`, limitando schemas não-message-centric
- **Sub-agentes como tools síncronos:** `db_agent.invoke()` dentro de tool bloqueia; sem streaming ou async entre sub-agentes
- **Singleton DB global:** `_db` module-level não suporta multi-tenancy ou connection pooling
- **Sem error recovery no grafo:** Não há retry/error handling nodes; falhas em tools propagam diretamente
- **Documentação de deployment esparsa:** `langgraph.json` referenciado mas não incluído nos arquivos extraídos

---

## 8. Lições Aplicáveis ao Zylos-Next

### Para zylos-bot-system

1. **Adotar Factory Pattern para Bots:** Cada bot deve ter `create_<bot>_agent()` com injeção de tools, state schema e model config. Isso permite testar bots isoladamente e compor variações (ex: bot com SQL vs bot com API).

2. **State Schema Separation:** Definir `input_schema`, `state_schema`, `output_schema` distintos no StateGraph. O estado interno do bot (session, user_id, flags) não deve vazar para a interface de entrada/saída.

3. **HITL como Primitiva:** Usar `interrupt()` para pausas que exigem input humano (aprovações, clarificações). No Zylos, isso mapeia para SOAR gates e verificações de segurança.

4. **Sub-Agentes via Tool-Calling:** Em vez de routing condicional hardcoded, embrulhar sub-sistemas (SOAR, bot, memory) como tools e deixar o orchestrator decidir via LLM. Mais flexível que edges fixas.

### Para zylos-soar

5. **Command para Routing Tipado:** Usar `Command[Literal[...]]` em vez de strings para transições de estado. Isso dá type safety e IDE support para máquinas de estado complexas.

6. **Dynamic Prompt Middleware:** Injetar contexto de sessão (user_id, threat_level, active_playbook) no system prompt via middleware, não concatenando strings manualmente.

7. **Eval Gate em CI:** Integrar evaluators de correctness e eficiência (tool call count, latency) como merge-blocking gates. O Zylos precisa de regression tests para playbooks e decisões de orquestração.

### Padrões Transversais

8. **Dual-Mode Checkpointing:** Desenvolver com in-memory checkpoints, deployar com platform-managed persistence. Evita código condicional e garante parity dev/prod.

9. **Cross-Provider Fallback:** Configurar cadeias de fallback com verificação de credenciais. Crítico para sistemas soberanos que não podem depender de um único provider.

10. **Structured Output para Decisões:** Classificação de intent, routing e gating devem usar `.with_structured_output()` com campos `reasoning`. Auditabilidade > black-box decisions.

11. **Segurança de Tools:** O padrão SELECT-only whitelist do `execute_sql` é correto, mas a implementação com f-strings é insegura. No Zylos, todas as tools de DB/API devem usar parameterized queries e input validation rigorosa.

12. **Lazy Loading de Recursos:** Conexões DB, vectorstores e clients devem ser lazy-loaded via singleton ou factory. Evita startup cost e facilita testing com mocks.

---

## Referências de Código-Chave

| Padrão | Arquivo | Linha/Função |
|--------|---------|--------------|
| Factory Pattern | `agents/db_agent.py` | `create_db_agent()` |
| HITL interrupt | `agents/supervisor_hitl_agent.py` | `collect_email()` |
| Command routing | `agents/supervisor_hitl_agent.py` | `query_router()` |
| Sub-agent as tool | `agents/supervisor_agent.py` | `call_database_specialist()` |
| Dynamic prompt | `agents/supervisor_agent.py` | `supervisor_prompt()` |
| Custom state | `agents/supervisor_hitl_agent.py` | `IntermediateState` |
| Eval LLM-as-Judge | `evaluators/evaluators.py` | `correctness_evaluator()` |
| Trace-based eval | `evaluators/evaluators.py` | `count_total_tool_calls_evaluator()` |
| Multi-provider config | `config.py` | `_resolve_model_credentials()`, `DEEP_AGENT_FALLBACK_MODELS` |
| Deployment composition | `deployments/supervisor_hitl_sql_agent_graph.py` | Module-level graph instance |