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