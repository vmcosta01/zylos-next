# Engenharia Reversa: QwenPaw (AgentScope)
**Data:** 04 de Novembro de 2026
**Analista:** Hermes Agent (Subagente de Pesquisa)
**Fonte:** https://github.com/agentscope-ai/QwenPaw

## Resumo Executivo
QwenPaw é uma plataforma de assistente pessoal de IA ("Personal AI Assistant") desenvolvida pela equipe AgentScope (Alibaba). Diferente de frameworks de orquestração pura, ele é um **produto final** focado em UX, memória persistente e onipresença (multi-canal). Sua arquitetura é monolítica-modular em Python, com forte ênfase em segurança (sandboxing), memória de três camadas e extensibilidade via Skills/MCP. Para o Zylos-Next, as principais lições residem na maturidade do sistema de memória (ReMe), na abordagem de segurança em camadas e no protocolo de comunicação entre agentes (ACP).

---

## 1. Arquitetura e Stack Principal

*   **Linguagem Core:** Python 3.11–3.13 (backend, runtime de agentes, CLI).
*   **Frontend/UI:** Console web (React/Vite implícito pelo build), TUI, e Desktop App via **Tauri** (Rust + WebView).
*   **Runtime Local:** `qwenpaw local` para inferência sem nuvem; suporte a Ollama/LM Studio.
*   **Modelos:** Família QwenPaw-Flash (2B, 4B, 9B, 35B-A3B, 27B) treinados especificamente para tarefas de agente.
*   **Infraestrutura:** Docker (`docker-compose.yml`), scripts de deploy dedicados.
*   **Design System:** "Orange with purpose" (#FF7F16), tokens CSS semânticos, Lucide React para ícones. Foco extremo em acessibilidade e estados de interação.

### Estrutura de Diretórios Chave (`src/qwenpaw/`)
A organização revela uma separação clara entre *runtime*, *lógica de agente* e *infraestrutura*:
*   `agents/`: Lógica cognitiva (ReAct, ACP, skills, memory, prompts).
*   `loop/`: Motor de execução (compiler, catalog, gates).
*   `memory/`: Integração com sistema de memória persistente.
*   `sandbox/`, `security/`, `governance/`: Camadas de isolamento e controle.
*   `harnesses/`: Adaptadores para diferentes ambientes de execução.
*   `providers/`, `drivers/`: Abstrações para LLMs e ferramentas externas.
*   `channel_protocol/`: Unificação de mensagens multi-plataforma.

---

## 2. Componentes Principais e Responsabilidades

| Componente | Responsabilidade | Arquivos/Diretórios Chave |
| :--- | :--- | :--- |
| **React Agent** | Implementação do loop ReAct (Reasoning + Acting) com suporte a sub-agentes. | `agents/react_agent.py`, `agents/prompt_builder.py` |
| **Loop Engine** | Compilação e execução de workflows; suporta gates condicionais. | `loop/compiler.py`, `loop/gates/`, `loop/catalog.py` |
| **Skill System** | Registro, descoberta e execução de habilidades modulares. | `agents/skill_system/`, `agents/skills/` |
| **Security Layer** | Tool Guard, File Guard, Skill Scanner, Access Policy. Bloqueio pré-execução. | `security/`, `governance/` |
| **Channel Protocol** | Adaptação unificada para DingTalk, Discord, Telegram, iMessage, etc. | `channel_protocol/`, `drivers/` |
| **Local Models** | Runtime otimizado para modelos QwenPaw-Flash locais. | `local_models/`, `providers/` |
| **Observability** | Rastreamento de tokens, estatísticas de agentes, logs estruturados. | `observability/`, `token_usage/`, `agent_stats/` |
| **Tauri Shell** | Wrapper desktop nativo com acesso a filesystem e sistema. | `tauri/` |

---

## 3. Padrões de Orquestração de Agentes/Workflows

*   **ReAct como Padrão Base:** O `react_agent.py` sugere que o loop padrão é Reasoning-Action iterativo, não grafos DAG estáticos complexos por padrão.
*   **Sub-Agentes Dinâmicos:** Capacidade de spawnar agentes independentes com memória e skills próprios durante a execução (`fork_project.py`).
*   **ACP (Agent Communication Protocol):** Diretório `agents/acp/` indica suporte nativo a protocolo padronizado para comunicação cross-sistema, permitindo orquestração distribuída.
*   **Loop Compiler:** `loop/compiler.py` sugere que workflows podem ser definidos declarativamente e compilados para execução eficiente, possivelmente com otimizações de estado.
*   **Gates Condicionais:** `loop/gates/` permite pontos de decisão humana ou automática dentro do fluxo, similar ao HITL do Zylos mas integrado ao loop engine.
*   **Routing Chat Model:** `routing_chat_model.py` indica roteamento inteligente de requisições para diferentes modelos baseado em complexidade/custo.

---

## 4. Sistema de Memória e Estado

Arquitetura de **três camadas** documentada explicitamente:

1.  **Live Working Context:** Memória de curto prazo da sessão atual.
2.  **Full Verbatim History:** Histórico completo imutável para auditoria e recuperação.
3.  **Personal Knowledge Base (ReMe):** Memória de longo prazo auto-evolutiva. Conversas e recursos são convertidos continuamente em Markdown legível, editável e buscável.

### Detalhes Técnicos
*   **Integração ReMe:** `memory/` atua como ponte para o projeto [ReMe](https://github.com/agentscope-ai/ReMe).
*   **Checkpoints:** `checkpoints/` sugere persistência de estado intermediário para recuperação de falhas.
*   **Backup:** `backup/` dedicado indica que a memória é tratada como dado crítico de usuário.
*   **Agent Memory vs Global Memory:** Separação entre `agents/memory/` (contexto do agente) e `memory/` (base global), permitindo isolamento e compartilhamento seletivo.

---

## 5. Integrações e Conectores

*   **Multi-Canal Nativo:** DingTalk, Lark, WeChat, Discord, Telegram, iMessage, QQ. Não é adapter genérico; há drivers específicos.
*   **MCP (Model Context Protocol):** Suporte explícito para integração com ecossistema MCP.
*   **Marketplace de Plugins:** `market/` e `plugins/` indicam sistema de distribuição e instalação dinâmica.
*   **Hub:** `hub/` provavelmente gerencia registro/descoberta de componentes remotos.
*   **Browser Automation:** `browser/` dedicado para web scraping/automação como ferramenta nativa.
*   **Document Processing:** Suporte a PDF, Word, Excel, PowerPoint mencionado no README.

---

## 6. Capacidades de Observabilidade/Debug

*   **Token Usage Tracking:** `token_usage/` para monitoramento de custo/consumo.
*   **Agent Stats:** `agent_stats/` para métricas de desempenho por agente.
*   **Observability Module:** `observability/` dedicado sugere integração com OpenTelemetry ou similar.
*   **Design Quality Gates:** O `design.md` define gates rigorosos de verificação de UI (estados de erro, loading, acessibilidade) que funcionam como observabilidade de UX.
*   **E2E Tests:** Diretório `e2e/` separado indica testes de integração completos.

---

## 7. Pontos Fortes e Fracos

### Forças
*   **Memória de Longo Prazo Real:** Integração ReMe vai além de RAG simples; é uma base de conhecimento viva.
*   **Segurança em Profundidade:** Sandbox kernel-level + guards + scanner é mais robusto que maioria dos frameworks.
*   **UX First:** Design system maduro e focus em "personal assistant" real, não apenas demo técnica.
*   **Onipresença:** Suporte a 7+ canais de mensagem nativos é diferencial massivo para adoção.
*   **Modelos Locais Treinados:** QwenPaw-Flash otimizado para agentes reduz dependência de API.
*   **Tauri Desktop:** Performance nativa com footprint menor que Electron.

### Fraquezas / Riscos
*   **Stack Python:** Performance ceiling para alta concorrência comparado a Rust/Go. GIL pode limitar paralelismo real em CPU-bound.
*   **Monólito Modular:** Apesar da separação, é um único repositório/package. Deploy independente de componentes é difícil.
*   **Complexidade de Canais:** Manter 7+ integrações de chat ativas é burden operacional alto.
*   **Dependência Alibaba:** Forte acoplamento ao ecossistema Qwen/AgentScope pode limitar portabilidade.
*   **Telemetria:** Coleta de dados anônima obrigatória no init pode ser barreira para usuários privacy-first.

---

## 8. Lições Aplicáveis ao Zylos-Next

| Dimensão Zylos | Lição do QwenPaw | Ação Recomendada |
| :--- | :--- | :--- |
| **Memory System** | Três camadas (working/history/KB) + ReMe como KB evolutiva. | Implementar camada de KB similar ao ReMe no `zylos-memory-system`. Separar claramente working memory de knowledge base. |
| **Sandbox** | Kernel-level sandbox + Tool/File Guards + Skill Scanner. | Reforçar `zylos-sandbox` com guards pré-execução. Adicionar scanner estático de skills antes do load. |
| **SOAR / Orchestration** | Loop compiler + gates + ACP nativo. | Considerar compiler para workflows SOAR em vez de interpretação runtime. Adotar ACP ou similar para cross-agent comm. |
| **HITL** | Gates integrados ao loop engine, não bolt-on. | Integrar HITL gates diretamente no `zylos-soar` execution path. |
| **Bot System / Live Comm** | Channel protocol unificado + drivers nativos. | Criar abstração `channel_protocol` no `zylos-live-comm` para suportar múltiplos backends sem reescrita. |
| **Skills Registry** | Marketplace + skill_system modular + scanner de segurança. | Adicionar verificação de segurança no `zylos-skills` registry. Considerar formato declarativo para skills. |
| **Verification / Proof** | Design quality gates + E2E separado. | Estender `zylos-verification` para incluir gates de UX/integração, não apenas lógica. |
| **Desktop / Cockpit** | Tauri > Electron para performance. | Avaliar Tauri para `zylos-cockpit` se Leptos WASM mostrar limitações de shell nativo. |
| **Modelos Locais** | Modelos treinados para agente (não generic chat). | Fine-tunar modelos pequenos para tool-use específico do Zylos. |
| **Segurança** | Governance module separado. | Criar crate `zylos-governance` para políticas de acesso centralizadas. |

### Prioridades Imediatas para Zylos-Next
1.  **Adotar arquitetura de memória de 3 camadas** no `zylos-memory-system` (atualmente focado em working memory).
2.  **Implementar Skill Scanner** no `zylos-skills` para prevenção de supply chain attacks.
3.  **Definir Channel Protocol abstraction** no `zylos-live-comm` antes de adicionar mais canais.
4.  **Estudar ACP** como candidato para substituir/extender comunicação interna entre crates Zylos.

---

## Notas Metodológicas
Análise baseada em extração web do repositório GitHub (README, design.md, estrutura de diretórios src/qwenpaw). Não foi realizado clone completo ou execução de código devido ao tamanho do repositório e restrições de ambiente. Recomenda-se validação pontual via clone local para detalhes de implementação de `loop/compiler.py` e `agents/acp/`.