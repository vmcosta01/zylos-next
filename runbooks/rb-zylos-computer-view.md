# RB-zylos-computer-view: Runbook Operacional

**Version:** 1.0.0
**Crate:** [zylos-computer-view](../crates/zylos-computer-view/)
**Last Updated:** 2026-11-11
**Related Playbook:** [pb-computer-view-setup.md](../playbooks/pb-computer-view-setup.md)
**Related POP:** [pop-computer-control-emergency.md](../playbooks/pop-computer-control-emergency.md)
**Related SOAR:** [soar-computer-control-breach.md](../docs/soar/soar-computer-control-breach.md)

## Deploy

### Pré-requisitos
- Rust 1.82+ instalado
- Crate registrado no workspace (`Cargo.toml` members)
- Dependências `dashmap = "6"`, `base64 = "0.22"` disponíveis

### Build
```bash
cargo build -p zylos-computer-view --release
```

### Integração ao Gateway
Adicionar em `apps/zylos-gateway/src/lib.rs`:
```rust
use zylos_computer_view::ComputerSessionManager;

// No build_router():
let session_manager = ComputerSessionManager::new(5); // 5 min lease default
let computer_router = create_computer_router(session_manager.clone());
app.merge(computer_router)

// Background task para expiry automático
tokio::spawn(async move {
    loop {
        session_manager.expire_leases();
        session_manager.cleanup_stale_sessions();
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
});
```

### Health Check
```bash
curl -sf "http://localhost:3000/api/v1/computer/sessions" | jq 'length'
# Esperado: número inteiro (0 ou mais)
# Falha: HTTP 500 ou connection refused
```

## Troubleshooting

### Erro 500 em /computer/sessions
**Causa:** Router não integrado no gateway ou panic no SessionManager
**Solução:** Verificar logs do gateway. Se router não integrado, adicionar `.merge(computer_router)` no `build_router()`.

### Take control retorna 409 Conflict
**Causa:** Outro usuário já tem lease ativo e não expirado
**Solução:** Verificar `control_lease.user_id` e `expires_at`. Aguardar expiry ou usar release forçado via POP de controle emergencial.

### Sessão fica presa em "connecting"
**Causa:** VNC endpoint inacessível ou proxy não iniciou conexão
**Solução:** Verificar se VNC server está rodando no endpoint configurado. Testar conectividade: `nc -zv localhost 5900`. Se VNC indisponível, usar screenshot fallback.

### Lease não expira automaticamente
**Causa:** Background task de expiry parou ou não foi iniciada
**Solução:** Verificar logs do gateway para mensagens "Control leases expired". Se ausente, reiniciar gateway para recriar background task.

### Screenshot retorna placeholder sempre
**Causa:** Proxy VNC não implementou captura de screenshot real
**Solução:** Comportamento esperado na versão atual. Proxy VNC real deve ser implementado para fornecer screenshots dinâmicos. Placeholder é fallback funcional.

### DashMap contention sob alta carga
**Causa:** Muitas sessões ativas (>1000) com operações concorrentes
**Solução:** DashMap v6 usa sharding interno. Se performance degradar, considerar particionamento por bot_id ou migração para Redis.

## Monitoring

| Métrica | Query/Source | Alerta Se |
|---------|-------------|-----------|
| Active sessions | `session_manager.active_count()` | >50 (possível leak de sessões) |
| Orphaned sessions | Sessions sem heartbeat >5min | >5 (cleanup pode ter falhado) |
| Control leases active | Count sessions with non-null lease | >10 (muitos usuários com controle) |
| Lease expiry rate | `expire_leases()` return value | Spike súbito (possível clock drift) |
| Screenshot fallback rate | % of requests returning placeholder | >80% (VNC provavelmente down) |

## Performance Tuning

- **DashMap sharding:** Default 16 shards é suficiente para <1000 sessões. Aumentar via `DashMap::with_capacity(n)` se necessário.
- **Cleanup interval:** 30s é balanceado. Reduzir para 10s se sessões órfãs acumulam rápido.
- **Lease duration:** 5 min default evita locks prolongados. Aumentar apenas para use cases de debugging extendido.

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.4
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [dashmap crate docs](https://docs.rs/dashmap/latest/dashmap/)
- [OpenMausBot CanvasComputers.tsx](https://github.com/milind-soni/OpenMausBot/blob/main/src/components/CanvasComputers.tsx)