# RB-zylos-egress: Runbook Operacional

**Version:** 1.0.0
**Crate:** [zylos-egress](../crates/zylos-egress/)
**Last Updated:** 2026-11-11
**Related Playbook:** [pb-egress-rules-management.md](../playbooks/pb-egress-rules-management.md)
**Related POP:** [pop-egress-false-positive.md](../playbooks/pop-egress-false-positive.md)
**Related SOAR:** [soar-egress-block-response.md](../docs/soar/soar-egress-block-response.md)

## Deploy

### Pré-requisitos
- Rust 1.82+ instalado
- Crate registrado no workspace (`Cargo.toml` members)
- Dependência `regex = "1"` disponível

### Build
```bash
cargo build -p zylos-egress --release
```

### Integração ao Gateway
Adicionar em `apps/zylos-gateway/src/lib.rs`:
```rust
use zylos_egress::EgressChecker;

// No build_router():
let egress_checker = EgressChecker::new();
let egress_router = create_egress_router(egress_checker);
app.merge(egress_router)
```

### Health Check
```bash
curl -sf -X POST http://localhost:3000/api/v1/egress/check \
  -H "Content-Type: application/json" \
  -d '{"text": "teste seguro"}' | jq '.passed'
# Esperado: true
```

## Troubleshooting

### Erro 500 em /egress/check
**Causa:** Regex customizada inválida ou panic no checker
**Solução:** Verificar logs do gateway para stack trace. Se regex custom, validar pattern com ferramenta externa antes de deploy.

### Falso positivo bloqueando mensagens legítimas
**Causa:** Pattern built-in muito amplo ou regra custom mal configurada
**Solução:** Seguir [pop-egress-false-positive.md](../playbooks/pop-egress-false-positive.md). Para built-ins, abrir issue com caso de teste. Para custom, ajustar regex via API.

### Performance degradada (>50ms por check)
**Causa:** Muitas regras customizadas ou regex complexas sem anchors
**Solução:** 
1. Contar regras ativas: `GET /egress/rules | jq 'length'`
2. Se >50 regras, consolidar ou remover inativas
3. Adicionar `\b` boundaries nas regex custom para evitar backtracking

### Cockpit não mostra warning de egress
**Causa:** Frontend não está chamando `/egress/check` antes do envio
**Solução:** Verificar Network tab no DevTools. Se request não aparece, bug no ChatPanel — verificar integração em `chat_panel.rs`.

## Performance Tuning

- **LazyLock patterns:** Todos os patterns built-in são compilados uma vez na primeira chamada. Cold start ~2ms, warm calls <1ms.
- **Regex optimization:** Usar `\b` word boundaries e evitar `.*` greedy em regras custom.
- **Batch checking:** Para verificação de múltiplos textos, chamar `/egress/check` uma vez com texto concatenado (separado por `\n---\n`) é mais eficiente que N chamadas individuais.

## Monitoring

| Métrica | Query/Source | Alerta Se |
|---------|-------------|-----------|
| Check latency p99 | Gateway metrics / Prometheus | >50ms |
| Block rate | `findings.blocked_categories.len() > 0` / total checks | >30% (possível falso positivo massivo) |
| Custom rule count | `GET /egress/rules \| jq 'length'` | >100 regras |
| Health keyword hits | Filter findings by category=health_data | Spike súbito (possível contexto médico legítimo) |

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.2
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [regex crate docs](https://docs.rs/regex/latest/regex/)