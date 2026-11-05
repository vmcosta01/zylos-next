# Engenharia Reversa: browser-use

**Repositório:** https://github.com/browser-use/browser-use
**Versão analisada:** 0.13.10
**Data da análise:** 2026-11-04
**Analisador:** Hermes Agent (subagente REA)

---

## 1. Arquitetura e Stack Principal

### Stack Tecnológica
- **Linguagem:** Python 3.11+ (async-first com `anyio` e `asyncio`)
- **Modelo de dados:** Pydantic v2 (`BaseModel`, `ConfigDict`, validators) para todos os schemas, estado e configuração
- **Event Bus:** `bubus` (event-driven architecture interna para comunicação entre componentes)
- **Browser Automation:** CDP direto via `cdp-use` + Playwright como fallback; suporte a cloud browsers proprietários
- **LLM Providers:** OpenAI, Anthropic, Google GenAI, Groq, Ollama, AWS Bedrock, OCI, LiteLLM, OpenRouter, Mistral, Cerebras, Vercel — cada um com adapter próprio em `browser_use/llm/<provider>/`
- **CLI:** Click + Rich + InquirerPy
- **Telemetria:** PostHog (anonymized, opt-out via `ANONYMIZED_TELEMETRY=False`)
- **Observabilidade:** Laminar (`lmnr`) opcional com decorators no-op quando ausente
- **Sandbox remota:** HTTP/SSE streaming com `cloudpickle` para serialização de funções
- **MCP:** Model Context Protocol server via `mcp` SDK (stdio transport)
- **Build:** Hatchling; dependências gerenciadas via uv

### Padrão Arquitetural Core
Arquitetura **Agent Loop + Event Bus + Watchdogs**:
- O `Agent` é um loop async que alterna entre LLM call → action execution → state update
- O `BrowserSession` é o hub central de estado do navegador, conectado via CDP WebSocket
- **Watchdogs** são componentes reativos que escutam eventos no `EventBus` e executam lógica de monitoramento/correção automaticamente (crash recovery, captcha detection, DOM consistency, popup handling, screenshot capture, HAR recording, security checks)
- A comunicação entre Agent ↔ Browser ↔ Watchdogs é **exclusivamente via eventos**, nunca por chamadas diretas

---

## 2. Componentes Principais e Responsabilidades

| Módulo | Arquivo-chave | Responsabilidade |
|--------|--------------|------------------|
| **Agent** | `agent/service.py` (4188 linhas) | Loop principal de execução; orquestra LLM, tools, browser, memory, planning, judge, loop detection |
| **AgentState** | `agent/views.py` | Estado serializável do agente: step count, failures, plan, loop detector, message manager state, file system state |
| **MessageManager** | `agent/message_manager/service.py` | Gerencia histórico de mensagens, compaction automática, sensitive data redaction, history windowing |
| **Tools/Registry** | `tools/service.py` (2327 linhas), `tools/registry/` | Registro dinâmico de actions; execução com timeout global (180s); JS injection para search/find_elements |
| **BrowserSession** | `browser/session.py` | Conexão CDP, session pool, reconnect logic, focus management, event bus owner |
| **Watchdogs** | `browser/watchdogs/*.py` (15 watchdogs) | Monitoramento reativo: crash, captcha, DOM, popups, downloads, screenshots, HAR, security, permissions, storage state, about:blank, default action, recording, local browser |
| **DOM Service** | `dom/service.py`, `dom/serializer/` | Extração e serialização de DOM para LLM; clickable elements indexing; paint order; markdown extraction |
| **Sandbox** | `sandbox/sandbox.py` | Execução remota de funções Python via SSE; cloudpickle serialization; import extraction automática |
| **Skills** | `skills/service.py` | Fetch/cache/exec de skills via Browser Use API; validação de parâmetros contra schema Pydantic; cookie injection |
| **MCP Server** | `mcp/server.py` (1294 linhas) | Exposição de capacidades via Model Context Protocol; agent mode + direct browser control + content extraction |
| **LLM Adapters** | `llm/<provider>/` | Abstração multi-provider com interface `BaseChatModel`; auto-timeout por modelo; gateway support |
| **Telemetry** | `telemetry/service.py` | Product telemetry anonymizada via PostHog; device fingerprinting |
| **FileSystem** | `filesystem/file_system.py` | Sistema de arquivos virtualizado para o agente (leitura/escrita de arquivos durante tarefas) |
| **Judge** | `agent/judge.py` | LLM-as-judge para verificação de conclusão de tarefa; verdict + reasoning + failure classification |

---

## 3. Padrões de Orquestração de Agentes/Workflows

### Agent Loop (service.py)
```
while not done and steps < max_steps:
    1. maybe_compact_messages()      # Compactação periódica do histórico
    2. get_browser_state_summary()   # Snapshot do DOM + screenshot
    3. prepare_step_state()          # Monta contexto para LLM
    4. llm.ainvoke(messages)         # Chamada ao modelo
    5. validate_output()             # Parsing + validação Pydantic
    6. execute_actions()             # Executa lista de ActionModels via Tools
    7. update_state()                # Atualiza AgentState, history, loop detector
    8. check_termination()           # Done action / max_failures / judge
```

### Planning System
- **Plan generation:** LLM gera `list[PlanItem]` com status `pending | current | done | skipped`
- **Replan on stall:** Após N falhas consecutivas (`planning_replan_on_stall=3`), injeta nudge para replanejamento
- **Exploration limit:** Se agente passa N steps sem plano (`planning_exploration_limit=5`), força geração de plano
- **Flash mode:** Desativa planning e thinking para modelos rápidos (BU2)

### Loop Detection (ActionLoopDetector)
- Hash normalizado por ação (search: tokens sorteados; click: element index; navigate: URL completa)
- Janela deslizante de 20 ações; detecção de repetição em thresholds escalonados (5, 8, 12)
- Page fingerprinting via SHA-256 do DOM text para detectar estagnação
- **Soft enforcement:** apenas injeta nudge messages no contexto do LLM, nunca bloqueia ações

### Multi-Agent / Follow-up Tasks
- Suporte a `add_new_task()` para follow-up requests dentro da mesma sessão
- Tags `<initial_user_request>` e `<follow_up_user_request>` no prompt para delimitar contexto
- Callbacks externos: `register_should_stop_callback`, `register_external_agent_status_raise_error_callback`

### Sandbox Execution Pattern
- Decorator `@sandbox` extrai source code da função + imports via AST parsing
- Serializa com `cloudpickle` + envia para executor remoto via SSE
- Stream de eventos tipados: `browser_created → instance_ready → log* → result/error → stream_complete`

---

## 4. Sistema de Memória e Estado

### Camadas de Memória
1. **Short-term (read_state):** Conteúdo extraído de ações individuais, incluído apenas no próximo step (`include_extracted_content_only_once=True`)
2. **Long-term memory:** Campo `long_term_memory` em `ActionResult`, sempre incluído no contexto futuro
3. **Compacted memory:** Resumo LLM-gerado do histórico antigo, acionado por:
   - Cadência: a cada N steps (`compact_every_n_steps=25`)
   - Tamanho: mínimo de chars/tokens (`trigger_char_count=40000` ≈ 10k tokens)
   - Preserva primeiro item + últimos K items (`keep_last_items=6`)
   - Summary máximo de 6000 chars
4. **Agent Brain (por step):** `thinking`, `evaluation_previous_goal`, `memory`, `next_goal` — campos estruturados na saída do LLM

### State Management
- `AgentState`: Pydantic model serializável com pause/resume support
- `MessageManagerState`: Histórico de mensagens + compacted memory + read state + compaction metadata
- `FileSystemState`: Estado do sistema de arquivos virtual
- `ActionLoopDetector`: Rolling window de action hashes + page fingerprints
- **Checkpoint-friendly:** Todo estado é serializável para permitir pause/resume e cloud execution

### Sensitive Data Handling
- Dicionário `{domain: {key: value}}` ou `{key: value}` passado ao agente
- Redaction automática em logs e mensagens via `redact_sensitive_string()`
- Detecção de chave correspondente a valores sensíveis no texto (`_detect_sensitive_key_name`)
- Filtragem antes de compaction e envio ao LLM

---

## 5. Integrações e Conectores

### LLM Providers (15 adapters)
OpenAI, Anthropic, Google GenAI, Groq, Ollama, AWS Bedrock, Azure, OCI, LiteLLM, OpenRouter, Mistral, Cerebras, DeepSeek, Vercel, Browser Use (BU2 gateway)

### Browser Backends
- Local Chrome/Chromium via CDP
- Cloud browsers via Browser Use API (stealth, residential proxies, CAPTCHA solving)
- Remote CDP connection
- Profile sync (cookies transfer)

### External Integrations
- **Gmail:** `integrations/gmail/` — actions + service para email automation
- **Anthropic Toolsets:** `integrations/anthropic/` — Bash + BrowserUse toolsets compatíveis com Claude SDK
- **OpenAI Toolsets:** `integrations/openai/` — integração similar para OpenAI SDK
- **Claude Toolsets:** `integrations/toolsets_for_claude/` — 31 browser actions + Bash
- **MCP Server:** Exposição completa via Model Context Protocol (stdio)
- **Skills API:** Fetch/exec de skills hospedadas na Browser Use Cloud

### File & Document Processing
- PDF generation via ReportLab (`save_as_pdf` action)
- DOCX reading via python-docx
- Markdown extraction via markdownify
- PYPDF para leitura de PDFs
- Image processing via Pillow

---

## 6. Capacidades de Observabilidade/Debug

### Tracing
- **Laminar (lmnr):** Decorators `@observe` e `@observe_debug` com fallback no-op automático
- Span types: `DEFAULT`, `LLM`, `TOOL`
- Debug mode ativado por `LMNR_LOGGING_LEVEL=debug` ou `BROWSER_USE_DEBUG=1`
- Tags obrigatórias: `observe`, `observe_debug`

### Logging Estruturado
- Emoji-coded log output: 💡 Thinking, 👍/⚠️/❔ Eval, 🧠 Memory, 🎯 Next goal
- Color-coded terminal output (green/red/blue)
- Message formatting com terminal width awareness
- Watchdog logging com parent/grandparent event chain tracing

### Telemetria
- PostHog anonymized telemetry (opt-out)
- Device fingerprinting via hardware hash + persisted UUID
- Eventos: agent creation, step execution, task completion, MCP server usage

### Debug Artifacts
- GIF generation do histórico de execução (`generate_gif`)
- Conversation saving (`save_conversation_path`)
- Screenshot capture por step (watchdog)
- HAR recording (watchdog)
- Video recording (`video_recorder.py`)
- Demo mode com visual highlights

### Watchdog Diagnostics
- Circuit breaker para CDP WebSocket morto
- Reconnection wait com timeout configurável
- Event history tracking com parent/grandparent chain
- Timing measurement por handler execution

---

## 7. Pontos Fortes e Fracos

### Pontos Fortes
1. **Event-driven architecture madura:** EventBus + Watchdogs desacopla completamente monitoramento da lógica de negócio; 15 watchdogs especializados cobrem crash, captcha, DOM, popups, security, downloads, etc.
2. **Memory compaction automática:** Sistema de compactação por cadência + tamanho com preservação seletiva (first + last K items); critical para sessões longas
3. **Loop detection sofisticado:** Hash normalizado por tipo de ação + page fingerprinting + nudges escalonados; soft enforcement evita false positives
4. **Multi-provider LLM abstraction:** 15 adapters com interface uniforme; auto-timeout por modelo; gateway support para routing
5. **Sandbox com SSE streaming:** Execução remota type-safe com eventos tipados; cloudpickle + AST import extraction
6. **Sensitive data handling integrado:** Redaction automática em logs, mensagens e compaction; domain-scoped credentials
7. **MCP compliance:** Exposição padrão via Model Context Protocol permite integração com qualquer MCP client
8. **Planning com replan automático:** Detecção de stall + exploration limit força replanejamento proativo
9. **Judge system:** LLM-as-judge com verdict estruturado + failure classification + impossible task detection
10. **Watchdog circuit breaker:** Previne hangs em CDP WebSocket morto com reconnection logic

### Pontos Fracos
1. **Monolítico:** `agent/service.py` tem 4188 linhas; `tools/service.py` tem 2327 linhas; alta complexidade cognitiva
2. **Python-only:** Não há sandboxing WASM/nativo; sandbox é apenas remote HTTP execution
3. **Sem HITL nativo:** Não há mecanismo de human-in-the-loop para aprovação de ações críticas; apenas callbacks externos
4. **Sem verification proof:** Judge é LLM-based (probabilístico), não há prova criptográfica ou determinística de execução
5. **Skills são cloud-dependent:** SkillService requer API key e conexão à Browser Use Cloud; sem registry local/offline
6. **Telemetria always-on por default:** PostHog ativo salvo opt-out explícito; preocupação para plataformas soberanas
7. **Sem SOAR/orchestration multi-agente:** Agent é single-task; não há DAG, workflow engine ou coordenação entre agentes
8. **State serialização limitada:** AgentState é Pydantic model mas não há persistence layer integrada (checkpoint manual)
9. **DOM serialization cost:** `max_clickable_elements_length=40000` chars pode exceder context window em páginas complexas
10. **Cloudpickle dependency:** Sandbox depende de cloudpickle para serialização; incompatível com ambientes restritos/WASM

---

## 8. Lições Aplicáveis ao Zylos-Next

### Para BOT_SYSTEM
- **Adotar event-driven architecture:** Substituir polling por EventBus + Watchdogs; cada watchdog é um componente independente com lifecycle próprio
- **Loop detection como primitiva:** Implementar ActionLoopDetector no core do bot system; hash normalizado por ação + page/state fingerprinting
- **Agent Brain estruturado:** Exigir `thinking`, `evaluation_previous_goal`, `memory`, `next_goal` como campos obrigatórios na saída do LLM; melhora observabilidade e debugging
- **Planning com replan automático:** Implementar `planning_replan_on_stall` e `planning_exploration_limit` como guards contra agentes perdidos

### Para MEMORY_SYSTEM
- **Memory compaction por cadência + tamanho:** Adotar o padrão de compactação periódica (N steps) com floor de tamanho; preservar first + last K items
- **Separação short-term vs long-term:** Read state (efêmero, next-step only) vs long-term memory (persistente); evitar poluição de contexto
- **Sensitive data redaction integrada:** Redaction automática em todos os pontos de saída (logs, mensagens, compaction, storage); domain-scoped credentials
- **State serializável:** Todo estado de memória deve ser serializável para checkpoint/resume; usar schemas tipados (equivalente a Pydantic em Rust: serde + schemars)

### Para LIVE_COMMUNICATION
- **SSE streaming com eventos tipados:** Adotar padrão SSEEvent com discriminated union (Rust: enum com serde tag); type guards para cada evento
- **Watchdog pattern para health monitoring:** Circuit breaker + reconnection logic para conexões WebSocket/CDP; event history com parent chain
- **Logging estruturado com emojis/codes:** Facilita parsing humano e machine-readable; color-coded para CLI

### Para VERIFICATION_PROOF
- **Judge como camada complementar, não substituta:** LLM-as-judge é útil para triagem mas insuficiente para prova; Zylos deve manter verification-proof determinística e usar LLM judge apenas como sinal adicional
- **Structured verdict:** Adotar `JudgementResult` com `verdict`, `reasoning`, `failure_reason`, `impossible_task`, `reached_captcha` como schema fixo
- **Ground truth support:** Permitir injeção de ground truth para validação automatizada

### Para SANDBOX_ISOLATION
- **Sandbox além de HTTP:** Zylos já tem WASM/Docker; browser-use mostra que sandbox puramente remoto é limitado; manter isolamento local como primário
- **Import extraction via AST:** Para sandboxes que aceitam código dinâmico, extrair dependências automaticamente (em Rust: parse syn/quote para identificar imports)
- **Streaming de execução:** SSE events para feedback em tempo real; não bloquear caller até conclusão

### Para SKILLS_REGISTRY
- **Skills com schema validation:** Cada skill deve ter schema Pydantic-equivalente (JSON Schema / schemars) para validação de parâmetros antes da execução
- **Wildcard com limite:** `*` limitado a N skills para evitar tool overload no LLM; paginação com safety limit
- **Cookie/credential injection:** Skills devem declarar requisitos de autenticação explicitamente; runtime injeta credenciais do contexto seguro
- **Offline-first:** Diferente de browser-use, Zylos deve suportar registry local sem dependência cloud

### Para SOAR / Orquestração
- **browser-use NÃO tem SOAR:** Esta é uma lacuna explícita; Zylos deve construir orquestração multi-agente (DAG, parallel execution, conditional branching) como diferencial
- **Follow-up task pattern:** Suporte a tarefas subsequentes na mesma sessão com tags de delimitação de contexto
- **External stop/status callbacks:** Hooks para controle externo do agente (pause, resume, abort)

### Padrões Transversais a Adotar
1. **Timeout por modelo/provider:** Auto-detect timeout baseado no provider (Gemini: 75-90s, Groq: 30s, Claude/O3: 90s)
2. **Action timeout global:** Guard contra hangs com fallback configurável via env var
3. **Demo mode:** Modo visual para desenvolvimento/debugging com highlights de elementos
4. **Conversation persistence:** Save/load de conversas para replay e debugging
5. **Device fingerprinting:** Identificação anônima de instância para telemetria e rate limiting
6. **No-op decorators:** Observabilidade opcional sem custo quando desativada (Rust: feature flags + cfg attributes)

### Anti-padrões a Evitar
1. **God classes:** `agent/service.py` com 4188 linhas é insustentável; decompor em módulos menores
2. **Cloud dependency for core features:** Skills, browsers e telemetria não devem ser obrigatoriamente cloud-bound
3. **Probabilistic-only verification:** LLM judge sem prova determinística é insuficiente para sistemas soberanos
4. **Always-on telemetry:** Plataformas soberanas devem ter telemetria opt-in, não opt-out
5. **Implicit state mutation:** Todo state change deve ser explícito e serializável

---

## Apêndice: Estrutura de Diretórios Analisada

```
browser_use/
├── actor/              # Mouse/page/element interaction primitives
├── agent/              # Core agent loop, views, prompts, judge, message_manager
│   ├── message_manager/ # History management, compaction, sensitive data
│   └── system_prompts/ # Template prompts por modelo/use-case
├── beta/               # Experimental features
├── browser/            # BrowserSession, profiles, cloud, watchdogs
│   ├── cloud/          # Cloud browser integration
│   └── watchdogs/      # 15 watchdog components
├── cli.py              # CLI entry point
├── config.py           # Configuration management
├── controller/         # Legacy alias for tools
├── dom/                # DOM extraction, serialization, markdown
│   ├── playground/     # DOM experimentation scripts
│   └── serializer/     # Clickable elements, HTML, paint order
├── filesystem/         # Virtual file system for agent
├── integrations/       # Gmail, Anthropic, OpenAI, Claude toolsets
├── llm/                # 15 provider adapters + base classes
├── mcp/                # Model Context Protocol server/client
├── observability.py    # lmnr integration + no-op fallbacks
├── sandbox/            # Remote execution via SSE + cloudpickle
├── screenshots/        # Screenshot utilities
├── skills/             # Skill service, views, installation
├── sync/               # Sync wrappers for async APIs
├── telemetry/          # PostHog anonymized telemetry
├── tokens/             # Token counting and cost estimation
└── tools/              # Action registry, execution, JS templates
    ├── extraction/     # Content extraction tools
    └── registry/       # Dynamic action registration
```

---

*Relatório gerado por engenharia reversa estática do código-fonte. Não foram executados testes dinâmicos ou benchmarks.*