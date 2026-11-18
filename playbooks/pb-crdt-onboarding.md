# Playbook: CRDT Onboarding (pb-crdt-onboarding)

## Objetivo
Integrar edição colaborativa de skills via Yjs/CRDT no cockpit.

## Pré-requisitos
- `zylos-auth` crate integrado ao gateway
- WebSocket endpoint `/api/v1/crdt/sync` disponível
- Cockpit com dependência `yjs` e `y-websocket` (via wasm-bindgen ou JS interop)

## Passos

### 1. Conectar ao WebSocket de sync
```javascript
// No cockpit (JS/WASM interop)
import * as Y from 'yjs';
import { WebsocketProvider } from 'y-websocket';

const doc = new Y.Doc();
const provider = new WebsocketProvider(
  'ws://localhost:8080/api/v1/crdt/sync',
  'skill-abc', // document ID
  doc
);

provider.on('status', event => {
  console.log('CRDT connection status:', event.status);
});
```

### 2. Autenticar via token JWT
O provider deve enviar mensagem `Auth { token }` logo após conectar:
```javascript
provider.ws.send(JSON.stringify({
  type: 'Auth',
  data: { token: localStorage.getItem('access_token') }
}));
```

### 3. Bindar ao editor
Use o Y.Text ou Y.Map como source of truth do editor:
```javascript
const ytext = doc.getText('content');
editor.bind(ytext); // adapter específico do editor
```

### 4. Verificar presence
O servidor envia `Awareness` updates com cursor/posição dos outros usuários.
Renderize indicadores visuais (avatars coloridos) baseados nos eventos `PresenceEvent::Joined`.

## Troubleshooting
- **Conexão recusada**: verificar se gateway está rodando e CORS permite WS
- **Auth denied**: token expirado ou inválido → renovar via `/auth/refresh`
- **Updates não chegam**: verificar se `CrdtSyncServer` foi inicializado no AppState

## Referências
- `crates/zylos-auth/src/crdt.rs`
- `pop-crdt-conflict-resolve`
- `pot-crdt-sync-tuning`


## Atualização Fase 11.9 — implementação real (yrs)

> As seções acima descrevem o desenho inicial com `yjs` JS. A implementação entregue usa **yrs (Rust) compilado para wasm32** no cockpit — sem dependência JS.

### Componentes
| Camada | Arquivo | Papel |
|---|---|---|
| Servidor | `apps/zylos-gateway/src/crdt_sync.rs` | WS y-websocket v1, salas por documento, awareness, ping |
| Persistência | `crates/zylos-auth/src/crdt_persistence.rs` | SQLite: log de updates + snapshots versionados, GC 30 dias |
| Cliente (núcleo) | `apps/zylos-cockpit/src/crdt_client.rs` | Réplica yrs + Awareness, sem DOM (testável nativamente) |
| Cliente (UI) | `apps/zylos-cockpit/src/components/collab_panel.rs` | Aba **Colaboração**: textarea, presença, resolução de conflitos |

### Rota correta
O endpoint é `GET /api/v1/crdt/sync/:doc_id` (não `/api/v1/crdt/sync` com nome de sala separado). Root de texto: `content`.

### Passo a passo no cockpit
1. Abrir a aba **Colaboração** (`Tab::Collab`).
2. Informar o **Documento** (ex.: `cockpit-notes`) e o **Nome** exibido aos demais.
3. Clicar **Conectar** — o cliente envia `SyncStep1` + sua presença (`{name,color,cursor}`).
4. Editar no textarea: cada alteração vira um `Update` CRDT (diff prefixo/sufixo, offsets UTF-8 corretos).
5. A lista **Online** mostra pares com cor determinística (`color_for(client_id)`) e cursor/seleção.
6. Para revisar antes de publicar, usar **Editar como rascunho** (ver `runbooks/pop-crdt-conflict-resolve.md`).

### Verificação
```bash
export CARGO_TARGET_DIR=E:/zylos-target/crdt
cargo check -p zylos-cockpit --target wasm32-unknown-unknown
cargo test  -p zylos-cockpit --lib crdt_client
cargo test  -p zylos-gateway --test crdt_multi_client --test crdt_latency_bench -- --nocapture
```

### Observação técnica
`yrs::sync::Awareness::new` não existe em `wasm32-unknown-unknown` (depende de `SystemTime`). O cliente usa `Awareness::with_clock(doc, now_ms)` com `js_sys::Date::now()`.

### Pendências conhecidas
- Autenticação JWT no handshake WS ainda não aplicada pelo servidor (seção 2 acima é desenho).
- Sem teste E2E em navegador; apenas `cargo check` wasm32 + testes nativos da lógica.

## Referências adicionais
- `docs/fase11-reports/crdt-sync-benchmark.md`
- ADR-012 em `docs/DECISIONS_FASE11.md`
