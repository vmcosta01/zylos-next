# Glossário Zylos-Next v6.0

Termos canônicos usados em specs, código e documentação. Atualizado conforme novos termos são introduzidos.

## Sistemas Core

| Termo | Definição | Crate |
|-------|-----------|-------|
| BOT_SYSTEM | Lifecycle manager, DAG runtime, P2P messaging type-safe entre agentes | `zylos-bot-system` |
| MEMORY_SYSTEM | SQLite WAL Context Graph, VectorStore trait, brief-aware rerank, Memory Agent | `zylos-memory-system` |
| LIVE_COMMUNICATION | AG-UI streaming, event bus SSE/WS, Mobile MCP server | `zylos-live-comm` |
| VERIFICATION_PROOF | Ground enum, Panel adversarial, Guardrail <1ms | `zylos-verification` |
| SANDBOX_ISOLATION | WASM snapshot/resume, Docker per-agent, browser anti-detect | `zylos-sandbox` |
| SKILLS_REGISTRY | index.json discovery <100ms, JSON Schema compilado, eval flywheel | `zylos-skills` |

## Tipos de Memória (MemoryKind)

| Kind | Backend | Descrição |
|------|---------|-----------|
| Working | Context window | Memória efêmera da sessão atual |
| Episodic | SQLite | Eventos e experiências passadas |
| Semantic | Vector store | Conhecimento factual e conceitual |
| Procedural | SKILL.md files | Procedimentos e workflows aprendidos |
| Meta | Reasoning bank | Lições sobre o próprio processo de aprendizado |

## Protocolos

| Termo | Definição |
|-------|-----------|
| Goal-Contract | Contrato tipado com verify:<cmd> obrigatório para toda task |
| WriteGate | Gate preventivo que valida evidence capsule antes de qualquer write externo |
| Recovery-Ladder | Sequência retry→decompose→delegate→escalate para falhas |
| RSI | Rigorous Scientific Iteration: ciclo Hypothesis→Design→Build→Measure→Analyze→Decide→Log |
| HITL | Human-In-The-Loop: aprovação humana obrigatória para operações críticas |
| Evidence Capsule | `{Finding→Evidence→Decision→Disposition}` — prova verificável anexada a toda decisão |

## Estados de Componente (Design System)

Todo componente UI DEVE implementar estes 6 estados:
1. **Default** — estado normal idle
2. **Hover** — feedback visual ao passar mouse/foco
3. **Active/Pressed** — feedback durante interação
4. **Disabled** — ação indisponível (visual distinct, não apenas opacity)
5. **Error** — estado de falha com mensagem acionável
6. **Loading** — indicador de progresso durante operação assíncrona

## Anti-Slop

| Princípio | Significado |
|-----------|-------------|
| Nunca aceite "parece funcionar" | Exige artefato verificável (teste verde, screenshot, log) |
| Spec antes de código | Nenhuma implementação sem spec aprovada correspondente |
| Estados de erro obrigatórios | Todo componente tem 6 estados documentados |
| Documentação antes | Loop profissional enforce docs ANTES de commit |
| Zero confiança em memória | Tudo em disco: MEMORY.md, DECISIONS.md, HANDOFF_*.md |
| Benchmark antes de promoção | Agente só promove com métricas versionadas |
| Out of Scope explícito | Toda spec declara o que NÃO faz |
