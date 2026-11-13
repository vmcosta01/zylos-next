# POP: Resetar Layout da Topologia do Swarm

**Versão:** 1.0
**Data:** 12 de novembro de 2026
**Componente:** `zylos-cockpit` / `swarm_topology_panel.rs`
**Acionamento:** Usuário clica em "↻ Resetar Layout" no painel de topologia, ou operador executa via console.
**Rollback:** Posições customizadas são perdidas permanentemente; não há undo. Confirme antes de executar programaticamente.

---

## 1. Contexto

O painel de topologia do swarm (Fase 10.5) permite drag-and-drop de nós SVG com persistência das posições customizadas no `localStorage` do navegador sob a chave `zylos-topology-positions`. Quando o layout fica desorganizado ou após mudanças estruturais no swarm, é necessário restaurar o layout circular determinístico padrão.

## 2. Pré-condições

- [ ] Cockpit carregado no navegador (página `/cockpit` ou rota equivalente)
- [ ] Painel de topologia visível e renderizado
- [ ] Console do navegador acessível (F12 → Console) se execução for via JS

## 3. Procedimento

### 3.1 Via UI (recomendado)

1. No painel de topologia, localize o botão **"↻ Resetar Layout"** no canto superior direito.
2. Clique no botão.
3. O painel imediatamente:
   - Remove `zylos-topology-positions` do `localStorage`
   - Reseta zoom/pan para padrão (scale=1, translate=0,0)
   - Recalcula layout circular determinístico
4. Verifique que os nós retornaram às posições originais.

### 3.2 Via Console do Navegador

```javascript
// Remove posições salvas
localStorage.removeItem('zylos-topology-positions');
// Recarrega a página para aplicar
location.reload();
```

### 3.3 Via Código Rust (programático)

```rust
use zylos_cockpit::components::swarm_topology_panel::reset_topology_layout;

// Limpa localStorage e reseta
reset_topology_layout();
```

## 4. Verificação Pós-Execução

- [ ] Nós estão em layout circular (orchestrators no centro, gateways intermediários, agents na periferia)
- [ ] Zoom está em 100% (sem escala aplicada)
- [ ] Pan está centralizado (translate = 0,0)
- [ ] `localStorage.getItem('zylos-topology-positions')` retorna `null`
- [ ] Drag-and-drop continua funcional após reset

## 5. Rollback

**Não há rollback automático.** As posições customizadas são permanentemente removidas. Se precisar preservar um layout customizado antes do reset:

```javascript
// Backup antes do reset
const backup = localStorage.getItem('zylos-topology-positions');
console.log('Backup:', backup);
// Após reset, restaurar:
// localStorage.setItem('zylos-topology-positions', backup);
// location.reload();
```

## 6. Troubleshooting

| Sintoma | Causa Provável | Ação |
|---------|---------------|------|
| Botão não aparece | Versão antiga do cockpit sem Fase 10.5 | Atualizar build do cockpit |
| Layout não muda após clique | localStorage bloqueado pelo navegador | Verificar configurações de privacidade |
| Nós saltam durante polling pós-reset | Custom positions ainda no signal | Aguardar próximo ciclo de polling (≤5s) |
| Zoom não reseta | Transform signal não atualizado | Recarregar página como fallback |

## 7. Referências

- **Código-fonte:** `apps/zylos-cockpit/src/components/swarm_topology_panel.rs`
- **Chave localStorage:** `zylos-topology-positions`
- **Função pública:** `reset_topology_layout()`
- **Polling interval:** 5 segundos (`POLL_INTERVAL_SECS`)
- **Fase:** 10.5 — Swarm Topology Interativo