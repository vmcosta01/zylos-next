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