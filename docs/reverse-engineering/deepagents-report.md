# Engenharia Reversa: langchain-ai/deepagents

**Data:** 2026-11-04
**Repositório:** https://github.com/langchain-ai/deepagents
**Versão analisada:** 0.7.32 (main branch)
**Licença:** MIT

---

## 1. Arquitetura e Stack Principal

O Deep Agents não é um runtime independente, mas um **harness opinativo** construído sobre três camadas distintas:

1.  **LangGraph (Runtime):** Fornece a execução durável baseada em grafos, gerenciamento de estado (`StateGraph`), checkpoints, streaming e interrupções (human-in-the-loop).
2.  **LangChain `create_agent` (Abstração):** Monta o loop básico de agente (modelo + ferramentas + middleware).
3.  **Deep Agents (Harness):** Adiciona a camada opinativa via `create_deep_agent()`, configurando middlewares padrão, backends de filesystem/memória, subagentes, skills e perfis de provedor.

### Stack Tecnológica
*   **Linguagem:** Python 3.11+ (com suporte a 3.14) e TypeScript/JavaScript (via `deepagents.js`).
*   **Dependências Core:** `langchain>=1.4.7`, `langchain-core>=1.7.3`, `langgraph` (implícito via langchain), `langsmith>=0.14.16`.
*   **Provedores Nativos:** Anthropic (`langchain-anthropic`), Google GenAI (`langchain-google-genai`), AWS Bedrock (opcional).
*   **Ferramentas de Dev:** `uv` (gerenciador de pacotes), `ruff` (linter/formatter), `pytest` (testes), `pre-commit`, `ty` (type checker).
*   **Build:** Setuptools com `pyproject.toml`.

### Modelo de Execução
A construção ocorre em duas fases:
1.  **Construção:** `create_deep_agent()` resolve o modelo, seleciona o backend, monta a pilha de middlewares, compõe o system prompt e delega ao `langchain.agents.create_agent()`.
2.  **Execução:** O LangGraph dirige o loop. Middlewares interceptam antes/depois das chamadas ao modelo e ferramentas para injetar contexto, gerenciar histórico ou impor permissões.

---

## 2. Componentes Principais e Responsabilidades

| Componente | Localização | Responsabilidade |
| :--- | :--- | :--- |
| `create_deep_agent` | `libs/deepagents/deepagents/graph.py` | Ponto de entrada; monta middleware stack, backends e subagentes. |
| `DeepAgentState` | `libs/deepagents/deepagents/graph.py` | Estende `AgentState` com `DeltaChannel` para crescimento linear do histórico. |
| `MemoryMiddleware` | `middleware/memory.py` | Carrega arquivos `AGENTS.md`, injeta no system prompt e gerencia aprendizado persistente. |
| `SummarizationMiddleware` | `middleware/summarization.py` | Compactação automática de contexto baseada em tokens/fração; offload de mídia para backend. |
| `SubAgentMiddleware` | `middleware/subagents.py` | Delegação de tarefas via tool `task`; isolamento de contexto e estado privado. |
| `FilesystemMiddleware` | `middleware/filesystem.py` | Ferramentas de leitura/escrita/edição com permissões granulares. |
| Backends | `backends/` | Abstração de storage (local, sandbox, remoto) para arquivos, memória e shell. |
| Profiles | `profiles/` | Ajustes específicos por provedor (Anthropic, Bedrock) e harness (subagentes padrão). |
| Skills | `.agents/skills/` | Comportamentos reutilizáveis carregados sob demanda (ex: `textual-screenshot`). |

---

## 3. Padrões de Orquestração de Agentes/Workflows

### Subagentes e Delegação
O sistema utiliza um padrão de **delegação via ferramenta**:
*   O agente principal possui uma ferramenta `task` que aceita `agent_name`, `description` e `input`.
*   Subagentes podem ser **declarativos** (configurados via dicionário) ou **compilados** (`CompiledStateGraph` do LangGraph).
*   Suporte a **AsyncSubAgents** para execução assíncrona/remota.
*   **Forked Subagents** (beta): permitem bifurcação de estado isolado.
*   Cada subagente tem sua própria pilha de middlewares independente.

### Middleware Stack Ordenado
A ordem de execução é crítica e segue o padrão:
1.  **Base Scaffolding:** Planejamento, filesystem, subagentes, sumarização.
2.  **Caller Middleware:** Comportamentos customizados da aplicação.
3.  **Profile/Tail Middleware:** Ajustes de provedor, exclusão de ferramentas, cache de prompt, injeção final de memória e aprovação humana.

### Human-in-the-Loop (HITL)
Implementado nativamente via recursos de interrupção do LangGraph. Permite aprovar, editar ou rejeitar chamadas de ferramentas antes da execução, essencial para operações de filesystem ou shell.

---

## 4. Sistema de Memória e Estado

### Memória Persistente (AGENTS.md)
*   Baseado na especificação aberta `AGENTS.md`.
*   Carregado via `MemoryMiddleware` de múltiplas fontes (`~/.deepagents/AGENTS.md`, `./.deepagents/AGENTS.md`).
*   Injetado dinamicamente no system prompt a cada turno.
*   **Auto-aprendizado:** O agente é instruído a usar `edit_file` para atualizar sua própria memória com feedbacks, preferências e padrões descobertos.
*   Comentários HTML são removidos antes da injeção para permitir anotações internas.

### Gerenciamento de Contexto (Sumarização)
*   **Trigger Automático:** Configurable por `tokens`, `messages` ou `fraction` da janela de contexto (padrão: 85% da capacidade).
*   **Offload de Mídia:** Imagens/áudio/vídeo em mensagens antigas são movidos para `/conversation_history/media/` e substituídos por tags de referência XML, mantendo o histórico textual leve.
*   **Compactação Manual:** Ferramenta `compact_conversation` disponível para o agente ou usuário forçar resumo quando elegível (>50% do trigger).
*   **Truncamento de Args:** Otimização pré-sumarização que corta argumentos grandes de ferramentas antigas (`write_file`, `execute`) antes de compactar o resto.

### Estado do Grafo
*   `DeepAgentState` usa `DeltaChannel` para evitar crescimento quadrático do checkpoint em threads longas.
*   Campos privados (`PrivateStateAttr`) impedem vazamento de estado interno para subagentes.
*   Middlewares podem contribuir com campos tipados via `state_schema`.

---

## 5. Integrações e Conectores

### Modelos e Provedores
*   **Model-Agnostic:** Qualquer modelo compatível com LangChain ChatModel.
*   **Perfis de Provedor:** `ProviderProfile` ajusta comportamento para Anthropic (cache de prompt), Bedrock, Google GenAI, etc.
*   **Open-Weight:** Suporte a Ollama, vLLM, llama.cpp via integrações LangChain.

### Filesystem e Sandbox
*   Backends plugáveis: local, sandboxed (Docker/E2B), remoto.
*   Permissões de path aplicadas via `FilesystemPermission`.
*   Shell access condicionado à disponibilidade do backend.

### MCP (Model Context Protocol)
*   Suporte nativo a servidores MCP como fonte de ferramentas.
*   Documentação dedicada em `openwiki/integrations/mcp.md`.

### Observabilidade Externa
*   Integração profunda com **LangSmith** para tracing, avaliação e monitoramento.
*   Traces estruturados com `TracePolicy` configurável por middleware.

---

## 6. Capacidades de Observabilidade e Debug

### Tracing Nativo
*   Todos os middlewares suportam `TracePolicy` para controlar inputs/outputs nos traces.
*   Eventos de sumarização, memória e subagentes são rastreados.
*   Tags de cache de prompt visíveis nos traces Anthropic.

### Avaliação e Benchmarks
*   **Benchmarks:** Targets `bench` (walltime) e `bench-memory` em `libs/deepagents`, `libs/code`, `libs/partners/quickjs`.
*   **CodSpeed:** Dashboard integrado para regressão de performance (threshold global ~10%).
*   **Evals Suite:** Scripts em `.github/scripts/evals/` para agregação, análise de falhas e validação de grafos.
*   **Harbor Integration:** Validação de limites de recursos em ambientes de teste.

### Debugging de Middleware
*   Logs estruturados via `logging` padrão Python.
*   Warnings tratados como erros em testes (`filterwarnings = ["error"]`).
*   Labels de CI (`ci:allow-warnings`) para bypass temporário em emergências.

---

## 7. Pontos Fortes e Fracos

### ✅ Pontos Fortes
1.  **Arquitetura em Camadas Clara:** Separação explícita entre runtime (LangGraph), abstração (LangChain) e harness (Deep Agents). Facilita substituição de peças.
2.  **Gerenciamento de Contexto Sofisticado:** Sumarização automática com offload de mídia, truncamento seletivo de args e triggers adaptativos por perfil de modelo.
3.  **Memória Auto-Gerida:** Padrão AGENTS.md com auto-aprendizado via `edit_file` reduz necessidade de RAG complexo para conhecimento procedural.
4.  **Extensibilidade via Middleware:** Stack ordenado permite injeção de comportamento sem fork; middlewares privados evitam poluição de estado.
5.  **Produção-Ready:** Checkpointing, streaming, HITL, tracing LangSmith e benchmarks contínuos integrados desde o design.
6.  **Multi-Linguagem:** Paridade Python/TypeScript planejada.

### ⚠️ Pontos Fracos / Riscos
1.  **Acoplamento ao Ecossistema LangChain:** Dependência pesada de `langchain-core`, `langchain-agents` e versões específicas. Breaking changes upstream impactam diretamente.
2.  **Complexidade de Middleware:** Ordem importa; debugar interações entre 10+ middlewares requer conhecimento profundo da stack interna.
3.  **Overhead de Abstração:** Três camadas adicionam latência e superfície de erro vs. implementação direta no LangGraph.
4.  **Documentação Distribuída:** Docs divididos entre README, ARCHITECTURE.md, DEVELOPMENT.md, openwiki/ e site externo. Curva de aprendizado inicial alta.
5.  **Features Beta Instáveis:** Forked subagents e alguns backends ainda em beta; API pode mudar.
6.  **Segurança "Trust the LLM":** Modelo de segurança delega contenção ao sandbox/backend, não ao prompt. Requer infraestrutura robusta em produção.

---

## 8. Lições Aplicáveis ao Zylos-Next

### 🎯 Adotar Imediatamente
1.  **Padrão AGENTS.md para Memória:** Implementar carregamento de arquivos markdown locais como memória persistente no `zylos-memory-system`. Simples, portátil e editável pelo próprio agente.
2.  **Sumarização com Offload de Mídia:** No `zylos-live-comm` ou `zylos-cockpit`, adotar padrão de mover payloads grandes (imagens, logs) para storage e substituir por referências antes de compactar contexto.
3.  **Middleware Stack Ordenado:** Estruturar `zylos-soar` e `zylos-gateway` com hooks explícitos (before_model, after_tool, etc.) em vez de lógica monolítica. Permitir plugins/user-hooks no meio da stack.
4.  **Estado Privado em Subagentes:** Garantir que `zylos-bot-system` isole estado interno de workers/subagentes para evitar vazamento de contexto sensível.

### 🔬 Adaptar com Cautela
5.  **Delegação via Tool (não grafo hardcoded):** Considerar expor subagentes do Zylos como ferramentas chamáveis pelo orquestrador, em vez de nós fixos no grafo SOAR. Mais flexível para agentes dinâmicos.
6.  **Perfis de Provedor:** Criar mecanismo similar aos `ProviderProfile` no `zylos-gateway` para ajustar prompts/tool-surface por modelo (Claude vs GPT vs local).
7.  **Benchmarks Contínuos:** Integrar benchmarks de walltime e memória no CI do Zylos-Next (já existe infra Rust/Criterion). Adotar threshold de regressão automático.

### ⚠️ Evitar / Mitigar
8.  **Dependência Excessiva de Frameworks:** Zylos é Rust puro; evitar portar abstrações Python desnecessárias. Extrair *padrões*, não código.
9.  **Segurança Delegada:** Zylos-sandbox já isola execução; não confiar apenas em prompts ou permissões de middleware. Manter boundaries rígidos no nível WASM/Docker.
10.  **Complexidade Prematura de Sumarização:** Começar com triggers simples (token count fixo) antes de implementar frações adaptativas por modelo. Medir uso real primeiro.

### 💡 Insights Estratégicos
*   **"Batteries-Included" vs Minimalismo:** Deep Agents prova que há valor em entregar filesystem, memória e subagentes *por padrão*. Zylos-Next deve considerar quais capacidades são "core" vs opcionais.
*   **Auto-Correção como Feature:** O padrão de "agente atualiza própria memória quando erra" é mais eficaz que RAG passivo. Priorizar no roadmap do `zylos-memory-system`.
*   **Observabilidade como Cidadã de Primeira Classe:** Tracing não é addon; está embutido em cada middleware. Zylos deve instrumentar `zylos-proof` e `zylos-verification` com mesma profundidade desde o início.

---

## Referências

*   Repositório: https://github.com/langchain-ai/deepagents
*   Arquitetura: `libs/ARCHITECTURE.md`
*   Desenvolvimento: `libs/DEVELOPMENT.md`
*   Diretrizes Globais: `AGENTS.md`
*   SDK Entry: `libs/deepagents/deepagents/__init__.py`
*   Middleware Memória: `libs/deepagents/deepagents/middleware/memory.py`
*   Middleware Sumarização: `libs/deepagents/deepagents/middleware/summarization.py`
*   Middleware Subagentes: `libs/deepagents/deepagents/middleware/subagents.py`
*   PyProject: `libs/deepagents/pyproject.toml`