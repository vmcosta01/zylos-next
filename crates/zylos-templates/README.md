# zylos-templates

Blueprints compartilháveis para agentes IA com schema YAML, registry versionado e detecção de conflitos semver.

O crate fornece um schema tipado (`AgentTemplate`) para definir papéis de agentes, um registry em memória com persistência em disco para templates built-in e da comunidade, verificação de integridade via SHA-256 e instalação segura com bloqueio automático de upgrades breaking (major bump).

## Instalação

Adicione ao `Cargo.toml`:

```toml
[dependencies]
zylos-templates = { path = "../zylos-templates" }

# Com suporte a registry remoto (feature gate)
zylos-templates = { path = "../zylos-templates", features = ["registry"] }
```

### Dependências transitivas

| Crate      | Uso                                      |
|------------|------------------------------------------|
| `semver`   | Versionamento semântico de templates     |
| `serde`    | Serialização/deserialização YAML         |
| `sha2`     | Hash de integridade do conteúdo          |
| `chrono`   | Timestamp de instanciação (`DateTime<Utc>`) |
| `thiserror`| Tipos de erro ergonômicos                |
| `tracing`  | Logs estruturados no registry            |

## Uso rápido

### Carregar templates built-in e buscar por slug

```rust
use zylos_templates::{TemplateRegistry, AgentTemplate};

let mut registry = TemplateRegistry::new("./templates");
registry.load_builtins(); // 15 templates: engineering, ops, research, creative

let reviewer = registry.get("code-reviewer")?;
println!("{} v{}", reviewer.display_name, reviewer.version);
// Code Reviewer v1.0.0
```

### Instanciar um agente a partir de um template

```rust
let agent = reviewer.instantiate("agent-001");

assert_eq!(agent.agent_id, "agent-001");
assert_eq!(agent.hitl_tier, 2);        // override do template
assert_eq!(agent.sandbox_mode, "docker");
assert_eq!(agent.proof_level, "confirmed");
```

Templates sem `config_overrides` usam padrões seguros: `hitl_tier = 1`, `sandbox_mode = "none"`, `proof_level = "basic"`.

### Instalar template da comunidade com detecção de conflito

```rust
let yaml = std::fs::read("custom-agent.yaml")?;
let installed = registry.install_from_bytes(&yaml)?;

// Upgrade minor (1.0.0 → 1.1.0): OK
// Upgrade major (1.x → 2.0.0): Err(TemplateError::VersionConflict)
```

### Preview sem instalar

```rust
let preview = TemplateRegistry::preview(yaml_bytes)?;
println!("Preview: {} v{}", preview.slug, preview.version);
```

### Verificar integridade

```rust
let hash = reviewer.content_hash(); // SHA-256 hex (64 chars), determinístico
```

## Schema do template YAML

```yaml
slug: code-reviewer           # identificador único ou @namespace/name
version: "1.0.0"              # semver obrigatório
display_name: Code Reviewer
description: Reviews PRs for security, performance, and style compliance
personality: senior-backend-engineer

skills:
  - github-pr-review
  - security-scan
  - performance-analysis

tools:
  - github
  - sonarqube
  - semgrep

config_overrides:             # opcional — defaults aplicados na instanciação
  hitl_tier: 2                # 1 (auto) → 4 (humano obrigatório)
  sandbox_mode: docker        # none | docker | vm
  proof_level: confirmed      # basic | confirmed | verified

tags:
  - engineering
  - security
  - quality
```

Todos os campos exceto `slug`, `version`, `display_name`, `description` e `personality` possuem default via `#[serde(default)]` e podem ser omitidos.

## Registry

### API pública

| Método                              | Descrição                                           |
|-------------------------------------|-----------------------------------------------------|
| `TemplateRegistry::new(dir)`        | Cria registry com diretório de instalação           |
| `load_builtins()`                   | Carrega 15 templates built-in                       |
| `register(template)`                | Registra template manualmente                       |
| `get(slug) -> Result<&AgentTemplate>`| Busca por slug ou `Err(NotFound)`                  |
| `list() -> Vec<&str>`               | Lista todos os slugs registrados                    |
| `count() -> usize`                  | Quantidade de templates                             |
| `install_from_bytes(yaml)`          | Instala com verificação de conflito + persistência  |
| `TemplateRegistry::preview(yaml)`   | Parse sem instalar (método associado)               |

### Templates built-in (15)

**Engineering:** `code-reviewer`, `test-writer`, `refactor-agent`, `doc-writer`
**Ops:** `incident-responder`, `infra-provisioner`, `ci-debugger`, `capacity-planner`
**Research:** `literature-reviewer`, `market-researcher`, `data-analyst`, `patent-searcher`
**Creative:** `content-writer`, `ui-designer`, `video-script-writer`

## Versionamento e conflitos

O crate segue [Semantic Versioning](https://semver.org/) para templates:

- **Minor/patch upgrade** (1.0.0 → 1.1.0): instalado automaticamente
- **Major upgrade** (1.x → 2.0.0): bloqueado com `TemplateError::VersionConflict`
- A verificação usa `is_breaking_upgrade_from(old_version)` que compara apenas o componente major

Para forçar um upgrade breaking, remova o template existente antes de reinstalar.

## Tipos de erro

```rust
pub enum TemplateError {
    NotFound(String),                          // slug não encontrado
    VersionConflict { installed, requested },  // major bump bloqueado
    InvalidYaml(String),                       // YAML malformado
    Io(std::io::Error),                        // erro de filesystem
    RegistryFetch(String),                     // feature "registry" apenas
}
```

## CLI

O crate expõe a lógica usada pelo CLI do Zylos para gerenciamento de templates:

```bash
# Listar templates disponíveis
zylos template list

# Instalar da comunidade
zylos template install ./my-agent.yaml

# Preview sem instalar
zylos template preview ./my-agent.yaml

# Verificar hash de integridade
zylos template hash code-reviewer
```

A integração CLI usa `TemplateRegistry::install_from_bytes()` internamente, herdando a detecção automática de conflitos.

## Testes

```bash
cargo test -p zylos-templates
```

Cobertura dos testes unitários (12 testes):

- Contagem de built-ins (15 templates)
- Busca por slug existente e inexistente
- Instanciação com overrides e com defaults
- Roundtrip YAML (serialize → deserialize)
- Instalação com persistência em disco
- Detecção de conflito de versão (minor OK, major bloqueado)
- Preview sem instalação
- Hash de conteúdo determinístico (SHA-256)
- Listagem completa de slugs
- Rejeição de YAML inválido
- Lógica de breaking upgrade

## Licença

Este crate é parte do projeto Zylos e está licenciado sob os mesmos termos do repositório raiz. Consulte o arquivo `LICENSE` na raiz do workspace.