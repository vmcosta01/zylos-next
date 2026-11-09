# zylos-ag-ui

Adapter do protocolo AG-UI (Agent-User Interaction) para zylos-gateway. Faz a ponte entre eventos internos do orquestrador zylos e o formato de streaming SSE usado por frontends compatíveis com AG-UI.

## Quick Start

```rust
use zylos_ag_ui::{AgUiEvent, encode_event, ag_ui_stream, OrchestratorEvent};
use tokio::sync::mpsc;

let (tx, rx) = mpsc::channel::<OrchestratorEvent>(64);
let stream = ag_ui_stream(rx);

let evt = AgUiEvent::text_start("msg-1", "assistant");
let sse = encode_event(&evt)?;
```

## Types

### Eventos AG-UI (`AgUiEvent`)

| Variante | Struct | Descrição |
|----------|--------|-----------|
| `TEXT_MESSAGE_START` | `TextMessageStart` | Início de mensagem de texto streamada |
| `TEXT_MESSAGE_CONTENT` | `TextMessageContent` | Delta incremental de texto |
| `TEXT_MESSAGE_END` | `TextMessageEnd` | Fim da mensagem de texto |
| `TOOL_CALL_START` | `ToolCallStart` | Início de invocação de ferramenta |
| `TOOL_CALL_ARGS` | `ToolCallArgs` | Argumentos JSON streamados da ferramenta |
| `TOOL_CALL_END` | `ToolCallEnd` | Conclusão da ferramenta com resultado opcional |
| `STATE_SNAPSHOT` | `StateSnapshot` | Snapshot completo de estado |
| `STATE_DELTA` | `StateDelta` | Atualização incremental via JSON Patch (RFC 6902) |
| `RUN_STARTED` | `RunStarted` | Início de um run de agente |
| `RUN_FINISHED` | `RunFinished` | Run concluído com sucesso |
| `RUN_ERROR` | `RunError` | Run falhou com código e mensagem |
| `STEP_STARTED` | `StepStarted` | Início de step discreto dentro de um run |
| `STEP_FINISHED` | `StepFinished` | Step concluído |
| `CUSTOM` | `CustomEvent` | Extensão vendor-specific fora do spec core |

### Tipos auxiliares

| Tipo | Descrição |
|------|-----------|
| `JsonPatchOp` | Operação JSON Patch: `Add`, `Remove`, `Replace` |
| `OrchestratorEvent` | Evento interno do orquestrador zylos (convertível para `AgUiEvent` via `From`) |

### Funções

| Função | Descrição |
|--------|-----------|
| `encode_event(&AgUiEvent) -> Result<SseEvent>` | Serializa evento AG-UI para frame SSE axum |
| `ag_ui_stream(mpsc::Receiver<OrchestratorEvent>) -> impl Stream` | Cria stream SSE a partir de canal de eventos do orquestrador |

### Construtores convenience (`AgUiEvent::*`)

`text_start`, `text_delta`, `text_end`, `run_started`, `run_finished`, `run_error`, `tool_start`, `tool_args`, `tool_end`, `state_snapshot`, `step_started`, `step_finished`, `custom`.

## Error Variants

Este crate não define enum de erro próprio. A única falha possível é `serde_json::Error` retornado por `encode_event()`. O stream `ag_ui_stream` loga e pula eventos malformados ao invés de terminar o stream.

## Gaps

- Sem validação de schema dos payloads `input`/`result` nos eventos de run/step
- Sem suporte a `parent_run_id` em conversões do orquestrador (hardcoded `None`)
- Sem rate limiting ou backpressure no stream SSE
- Sem reconexão automática ou replay de eventos perdidos
- `CUSTOM` events não têm registro centralizado de nomes vendor
