# Runbook — zylos-core (Tipos Compartilhados e Traits)

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Crate:** `crates/zylos-core`
**Dependências:** `serde`, `uuid`, `chrono`
**Fase:** 0 (FOUNDATION) → Todas as fases

---

## 1. Visão Geral

O `zylos-core` é o crate foundational do Zylos-Next. Define tipos compartilhados, traits de interface e utilitários usados por todos os outros crates. Não contém lógica de negócio — apenas contratos e abstrações.

**Conteúdo:**
- Tipos de ID (`AgentId`, `SkillId`, `SessionId`) baseados em UUID v7
- Enums compartilhados (`AgentStatus`, `SkillCategory`, `SandboxType`)
- Traits de interface (`Executable`, `Auditable`, `Serializable`)
- Error types comuns (`CoreError`)
- Feature flags para conditional compilation

**Regra:** Nenhum crate deve definir seus próprios tipos de ID ou enums que já existem no core. Importe de `zylos_core::`.

---

## 2. Nível Senior — Arquitetura e Decisões

### 2.1 Por que um crate core separado?
- **Evita circular dependencies:** Crates A e B podem ambos depender de Core sem depender um do outro
- **Single source of truth:** Mudanças em tipos compartilhados são feitas em um lugar
- **Feature gating:** Consumidores podem habilitar apenas o que precisam (`serde`, `sqlx`, `wasm`)
- **Versionamento estável:** Core muda menos que crates de feature, reduzindo churn de dependência

### 2.2 UUID v7 para IDs
UUID v7 é time-sortable e collision-resistant:
- Ordenação temporal natural (sem índice separado por timestamp)
- Compatível com SQLite TEXT primary keys
- Geração distribuída sem coordenação
- Legível em logs (formato padrão UUID)

### 2.3 Design de Traits
Traits no core são **object-safe** quando possível:
```rust
// ✅ Object-safe — pode usar dyn Executable
pub trait Executable {
    fn execute(&self, ctx: &Context) -> Result<Output>;
}

// ❌ Não object-safe — requer generics
pub trait Serializable<T> {
    fn serialize(&self) -> Vec<u8>;
}
```

Quando object-safety não é possível, documente explicitamente e forneça wrapper enum se necessário.

### 2.4 Feature Flags
| Feature | Default | Descrição |
|---------|---------|-----------|
| `serde` | ✅ | Derive Serialize/Deserialize em todos os tipos |
| `sqlx` | ❌ | Implementações FromRow/Type para SQLx |
| `wasm` | ❌ | Bindings WASM-compatíveis (no std) |
| `openapi` | ❌ | Schemas OpenAPI 3.0 via schemars |

---

## 3. Nível Pleno — Operação e Integração

### 3.1 Adicionar novo tipo compartilhado
1. Defina o tipo em módulo apropriado (`types/`, `traits/`, `error.rs`)
2. Derive macros obrigatórias: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`
3. Derive `Serialize`/`Deserialize` atrás da feature `serde`
4. Adicione testes unitários no mesmo arquivo (`#[cfg(test)] mod tests`)
5. Re-export no `lib.rs` com doc comment público

### 3.2 Breaking changes
Core é usado por TODOS os crates. Breaking changes requerem:
1. Deprecation warning por pelo menos 1 fase antes da remoção
2. Migration guide no CHANGELOG.md
3. Atualização simultânea de todos os consumidores no workspace
4. Bump de versão minor (semver)

### 3.3 Build e testes
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-core --all-features
# Testes devem passar com E sem features opcionais
cargo test -p zylos-core --no-default-features
```

### 3.4 Checklist ao modificar core
- [ ] Todos os consumers compilam após mudança (`cargo check --workspace`)
- [ ] Testes passam com todas as combinações de features
- [ ] Doc comments atualizados
- [ ] CHANGELOG.md atualizado se breaking change
- [ ] No runtime dependencies adicionadas (core é zero-dep exceto serde/uuid)

---

## 4. Nível Júnior — Execução Guiada

### 4.1 Verificar se compila
```bash
cd C:/Users/Paulo/zylos-next
cargo check -p zylos-core --all-features
```
**Esperado:** `Finished` sem erros.

### 4.2 Rodar testes
```bash
cargo test -p zylos-core --all-features
```
**Esperado:** Todos passam.

### 4.3 Usar tipo do core em outro crate
```rust
// No Cargo.toml do seu crate:
// zylos-core = { path = "../zylos-core", features = ["serde"] }

use zylos_core::{AgentId, AgentStatus};

let id = AgentId::new(); // UUID v7
let status = AgentStatus::Running;
```

### 4.4 Quando escalar
- Erro de compilação → verificar se feature necessária está habilitada
- Tipo não encontrado → verificar re-export no `lib.rs`
- Breaking change detectado → ler CHANGELOG.md para migration guide
- Circular dependency → mover tipo para core, não criar import entre crates siblings

---

## 5. Referências Cruzadas

- **Código fonte:** `crates/zylos-core/src/lib.rs`
- **Runbook Fase 0:** `runbooks/fase0-foundation.md`
- **ADR-001:** Rust como linguagem única
- **Workspace Cargo.toml:** `Cargo.toml` (raiz) §workspace.dependencies