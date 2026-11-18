# POP: CRDT Conflict Resolution (pop-crdt-conflict-resolve)

## Escopo
Resolver conflitos visuais de merge em edição colaborativa de skills via CRDT.

## Gatilho
- Usuário reporta conteúdo duplicado ou perdido após sessão colaborativa
- Indicador de conflito renderizado no cockpit (merge conflict UI)
- Divergência detectada entre estado local e servidor

## Procedimento

### 1. Identificar documento afetado
```bash
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
 http://localhost:8080/api/v1/crdt/documents?active=true
```

### 2. Coletar state vectors dos clientes
Solicitar aos usuários envolvidos que exportem seu estado local (Yjs encodeStateAsUpdate):
```javascript
// No console do cockpit
const update = Y.encodeStateAsUpdate(ydoc);
console.log(btoa(String.fromCharCode(...update)));
```

### 3. Comparar updates divergentes
Usar `Y.diffUpdates` ou aplicar ambos em um Doc temporário para visualizar diferenças:
```javascript
const merged = new Y.Doc();
Y.applyUpdate(merged, updateA);
Y.applyUpdate(merged, updateB);
// Verificar conteúdo resultante
```

### 4. Resolver manualmente ou aceitar merge automático
- **Se CRDT resolveu corretamente**: apenas confirmar e notificar usuários
- **Se há perda de dados**: restaurar da versão anterior (audit log / snapshot) e re-aplicar updates
- **Se há duplicação**: remover entradas duplicadas via operação delete no Y.Text/Y.Map

### 5. Broadcast do estado corrigido
```rust
server.broadcast_update(&doc_id, CrdtMessage::SyncStep2(corrected_state));
```

### 6. Notificar usuários
Exibir toast no cockpit: "Conflito resolvido — revisão automática aplicada" ou "Ação manual necessária".

## Prevenção
- Usar TTL curto para awareness updates (30s stale timeout)
- Habilitar snapshots periódicos a cada N updates
- Renderizar indicador visual quando >2 usuários editam mesma região

## Referências
- `crates/zylos-auth/src/crdt.rs`
- `pb-crdt-onboarding`
- `pot-crdt-sync-tuning`


## Atualização Fase 11.9 — UI de resolução no cockpit (T11.9.5)

Em edição ao vivo o CRDT (yrs/YATA) converge sozinho: não há "conflito" no sentido de perda. O fluxo abaixo é para quem quer **revisar** a divergência antes de publicar.

### Procedimento no cockpit (aba Colaboração)
1. Clicar **Editar como rascunho**: o documento é bifurcado (`CrdtClient::begin_draft`). Edições locais vão só para o rascunho; updates remotos continuam chegando ao documento principal.
2. A seção **Local × remoto** mostra o diff por linha:
   - `+` (verde) — linha só no rascunho local;
   - `~` (azul) — linha só no remoto;
   - sem marcação — igual nos dois.
3. Escolher uma ação:
   | Botão | Método | Efeito |
   |---|---|---|
   | **Mesclar (CRDT)** | `merge_draft()` | Aplica as operações do rascunho no principal; os dois lados são preservados, ordem determinística |
   | **Manter local** | `keep_local()` | Reescreve o texto principal com o rascunho (sobrescreve edições remotas concorrentes) |
   | **Aceitar remoto** | `accept_remote()` | Descarta o rascunho |
4. O resultado é enviado como `Update` e retransmitido a todos os clientes da sala.

### Quando usar cada opção
- Edições em regiões diferentes → **Mesclar**.
- Reescrita completa intencional → **Manter local** (avisar os demais: sobrescreve).
- Rascunho obsoleto → **Aceitar remoto**.

### Recuperação via servidor (perda real de dados)
```bash
curl http://localhost:3000/api/v1/crdt/documents/<doc_id>/versions
curl http://localhost:3000/api/v1/crdt/documents/<doc_id>/versions/<versao>
curl -X POST http://localhost:3000/api/v1/crdt/documents/<doc_id>/compact
```
Cole o texto da versão desejada em modo rascunho e use **Manter local**.

### Evidência
`crdt_client_draft_merge_keeps_both_sides` e `crdt_client_keep_local_and_accept_remote` (`apps/zylos-cockpit/src/crdt_client.rs`); `crdt_same_position_conflict_is_deterministic` (`apps/zylos-gateway/tests/crdt_multi_client.rs`).
