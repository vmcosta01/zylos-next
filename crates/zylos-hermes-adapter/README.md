# zylos-hermes-adapter

Bridge entre zylos-next e o ecossistema Hermes. Mapeia 65 perfis Hermes para agentes zylos e expõe skills, memory e HITL via MCP HTTP.

## Quick Start

```rust
use zylos_hermes_adapter::{HermesAdapter, HermesProfileMapping};

let mut adapter = HermesAdapter::new(
    "http://localhost:8080".into(),
    "bearer-token".into(),
);
adapter.load_default_profiles();
adapter.validate_profile_count()?;
let config = adapter.expose_skills_via_mcp().await?;
let metrics = adapter.integration_metrics();
```

## Types

| Tipo | Descrição |
|------|-----------|
| `HermesAdapter` | Adapter principal; gerencia profiles, endpoint MCP e auth |
| `HermesProfileMapping` | Mapeamento profile Hermes ↔ agente zylos (tenant, capabilities, model, flags) |
| `McpServerConfig` | Configuração do servidor MCP: endpoint, auth header, lista de tools |
| `McpTool` | Definição de tool MCP: nome, descrição, JSON Schema de input |
| `IntegrationMetrics` | Métricas agregadas: total profiles, approval count, equivalence count, unique tenants |
| `HermesAdapterError` | Enum de erros (ver abaixo) |

## Error Variants

| Variante | Causa |
|----------|-------|
| `NoProfilesRegistered` | Operação requer ≥1 profile mas nenhum foi registrado |
| `InsufficientProfiles { expected, actual }` | Menos de 65 profiles registrados |
| `McpConnectionFailed(msg)` | Conexão HTTP ao endpoint MCP falhou |

## Gaps

- `expose_skills_via_mcp` retorna config estática; não há runtime MCP server neste crate
- Profiles padrão são hardcoded; sem carregamento dinâmico de arquivo/config
- `McpConnectionFailed` nunca é produzido internamente (reservado para uso futuro)
- Sem validação de capabilities por tenant
