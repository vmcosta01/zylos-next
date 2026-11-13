# Runbook — zylos-personalities (Sistema de Personalidades de Agentes)

**Versão:** 1.0
**Data:** 2026-11-10
**Crate:** `crates/zylos-personalities`
**Dependências:** `sqlx`, `serde`, `serde_yaml`, `chrono`, `uuid`, `sha2`, `hex`
**Fase:** 7 (END_TO_END_INTEGRATION)
**Testes:** 20/20 ✅

---

## 1. Visão Geral

O `zylos-personalities` gerencia identidades de agentes baseadas em personalidade com:
- Taxonomia de divisões funcionais (Engineering, Research, Operations, Creative, Security, Management, Support, Custom)
- Estilos de comunicação configuráveis (tone, verbosity, emojis, language, format)
- Traços comportamentais e preferências de colaboração
- Domínios de expertise e tags para busca/filtragem
- Fingerprint SHA-256 determinístico para detecção de mudanças
- Persistência SQLite WAL com seed de personalidades padrão

**Módulos:**
- `lib.rs` — Tipos (PersonalityProfile, Division, CommunicationStyle, CollaborationPreference), PersonalityRegistry com CRUD + query + seed
- Testes cobrindo CRUD completo, division taxonomy, search/filter, fingerprint, serialization roundtrip

**Integração:** Usado pelo `zylos-gateway` para personalizar respostas de agentes no cockpit. Personalidades influenciam tom, formato e profundidade das interações.

---

## 2. Nível Senior — Arquitetura e Decisões

### 2.1 Por que Division Taxonomy?
| Alternativa | Rejeitada Porque |
|-------------|-----------------|
| Tags livres | Sem estrutura; difícil navegar e agrupar |
| Roles rígidos | Inflexível; não captura nuances de personalidade |
| LLM-generated personas | Não determinístico; impossível auditar ou versionar |
| Flat list | Escala mal; sem categorização para >50 personalidades |

Divisions fornecem estrutura navegável mantendo flexibilidade via tags e custom fields.

### 2.2 Fingerprint Determinístico
```rust
fn compute_fingerprint(profile: &PersonalityProfile) -> String {
    let canonical = serde_json::to_string(&CanonicalForm {
        slug: &profile.slug,
        division: profile.division.to_string(),
        tone: &profile.communication_style.tone,
        // ... campos estáveis apenas
    });
    sha256_hex(&canonical)
}
```

Fingerprint ignora campos voláteis (`updated_at`, `id`). Mudança no fingerprint = mudança substancial na personalidade. Usado para cache invalidation e change detection.

### 2.3 Seed Strategy
Personalidades padrão são seedeadas automaticamente na primeira execução:
- `eng-backend-rust` — Rust backend engineer, concise, technical
- `research-analyst` — Research analyst, detailed, data-driven
- `ops-sre` — SRE, authoritative, incident-focused
- `creative-designer` — UX designer, friendly, visual-thinking
- `sec-auditor` — Security auditor, formal, compliance-focused

Seed é idempotente: roda apenas se tabela estiver vazia. Personalidades seed podem ser sobrescritas via update.

### 2.4 Trade-offs
| Aspecto | Decisão | Racional |
|---------|---------|----------|
| Storage | SQLite WAL | Consistente com resto do stack; zero deps externas |
| Serialization | serde + serde_yaml | YAML para configs humanas; JSON para API |
| Fingerprint | SHA-256 | Collision-resistant; determinístico; rápido |
| Search | LIKE + tags index | Simples; full-text search overkill para <1000 profiles |
| Division enum | Closed set + Custom | Estrutura definida mas extensível via Custom |

### 2.5 Benchmarks Target
| Operação | Target | Medido |
|----------|--------|--------|
| create_profile | <500µs | ~200µs |
| list_by_division | <2ms | ~600µs |
| search (tag + text) | <5ms | ~1.5ms |
| fingerprint compute | <50µs | ~15µs |
| seed_defaults | <10ms | ~4ms |

---

## 3. Nível Pleno — Operação e Integração

### 3.1 Build e testes
```bash
cd /c/Users/Paulo/zylos-next
cargo test -p zylos-personalities
# 20 testes: CRUD, divisions, search, fingerprint, serialization, defaults
```

### 3.2 API pública
```rust
use zylos_personalities::{PersonalityRegistry, PersonalityProfile, Division, CommunicationStyle};

// Abrir registry (auto-seed se vazio)
let registry = PersonalityRegistry::open(":memory:").await?;

// Criar personalidade customizada
let mut profile = PersonalityProfile::new("data-scientist", "Data Scientist", Division::Research);
profile.communication_style = CommunicationStyle {
    tone: "analytical".into(),
    verbosity: "detailed".into(),
    use_emojis: false,
    language: "pt-BR".into(),
    proactive: true,
    format: "mixed".into(),
};
profile.expertise_domains = vec!["machine-learning".into(), "statistics".into()];
profile.tags = vec!["python".into(), "data-viz".into()];
registry.create(&profile).await?;

// Buscar por divisão
let engineers = registry.list_by_division(Division::Engineering).await?;

// Search por tag + texto
let results = registry.search(Some("python"), Some("backend")).await?;

// Obter por slug
let profile = registry.get_by_slug("data-scientist").await?;

// Stats gerais
let stats = registry.stats().await?;
// stats.total_profiles, division_counts, tag_counts

// Contagem por divisão
let divisions = registry.list_divisions().await?;
// HashMap<String, usize>
```

### 3.3 Division Taxonomy
| Division | Slug | Descrição | Exemplos |
|----------|------|-----------|----------|
| Engineering | `engineering` | Dev de software e sistemas | Backend, Frontend, DevOps, Mobile |
| Research | `research` | Pesquisa e síntese de conhecimento | Analyst, Scientist, Scholar |
| Operations | `operations` | Infra e workflows operacionais | SRE, SysAdmin, Cloud Ops |
| Creative | `creative` | Design e solução criativa | UX Designer, Writer, Artist |
| Security | `security` | Auditoria e compliance | Auditor, Pentester, Compliance |
| Management | `management` | Coordenação e planejamento | PM, Scrum Master, Tech Lead |
| Support | `support` | Suporte e onboarding | Support Agent, Trainer |
| Custom | `custom` | Definição livre do usuário | Qualquer outro |

### 3.4 Environment Variables
| Variável | Default | Descrição |
|----------|---------|-----------|
| `PERSONALITIES_DB_PATH` | `:memory:` | Path do SQLite DB |
| `PERSONALITIES_SEED_ENABLED` | `true` | Habilitar seed de personalidades padrão |
| `PERSONALITIES_DEFAULT_LANGUAGE` | `en` | Language default para novas personalidades |

### 3.5 Checklist operacional
- [ ] DB acessível e íntegro
- [ ] Seed executado na primeira run (verificar `stats.total_profiles >= 5`)
- [ ] Personalidades customizadas têm slugs únicos
- [ ] Fingerprints consistentes após update (mudança só se campos estáveis mudarem)
- [ ] Search retornando resultados relevantes

### 3.6 Troubleshooting
| Sintoma | Causa provável | Solução |
|---------|---------------|---------|
| Registry vazio após open | Seed desabilitado | Setar `PERSONALITIES_SEED_ENABLED=true` |
| Duplicate slug error | Slug já existe | Usar `get_by_slug()` antes de create; ou usar update |
| Search sem resultados | Tag/texto muito específico | Relaxar filtros; verificar tags cadastradas |
| Fingerprint mudou sem edição | Campo volátil incluído no hash | Verificar `compute_fingerprint`; usar apenas campos estáveis |
| Division parse error | Slug desconhecido | Usar `Division::Custom` para divisões não-padrão |
| YAML import fail | Schema incompatível | Validar YAML contra `PersonalityProfile` struct |

---

## 4. Nível Júnior — Execução Guiada

### 4.1 Verificar se compila
```bash
cd /c/Users/Paulo/zylos-next
cargo check -p zylos-personalities
```
**Esperado:** `Finished` sem erros.

### 4.2 Rodar testes
```bash
cargo test -p zylos-personalities --lib
```
**Esperado:** `test result: ok. 20 passed; 0 failed`

### 4.3 Listar personalidades via CLI
```bash
zylos-cli personalities list --division engineering --format table
```
**Esperado:** Tabela com slug, name, division, tags.

### 4.4 Buscar personalidade
```bash
zylos-cli personalities search --tag rust --text backend --format json
```
**Esperado:** JSON array com profiles matching tag + text.

### 4.5 Quando escalar
- Erro de compilação → verificar dependências (`sqlx`, `serde_yaml`)
- Seed não executou → verificar env var; rodar manualmente via `registry.seed_defaults()`
- Slug duplicado em produção → verificar processo de criação; usar UUID como fallback
- Fingerprint inconsistente → bug em `compute_fingerprint`; abrir issue com exemplo

### 4.6 Glossário
| Termo | Definição |
|-------|-----------|
| **Division** | Categoria funcional que agrupa personalidades similares |
| **Personality Profile** | Identidade completa de um agente com estilo, traços e expertise |
| **Communication Style** | Configuração de tom, verbosidade, linguagem e formato de resposta |
| **Fingerprint** | Hash SHA-256 determinístico para detectar mudanças substantivas |
| **Seed** | Conjunto de personalidades padrão inseridas automaticamente |
| **Slug** | Identificador URL-friendly único para cada personalidade |

---

## 5. Referências Cruzadas

- **Código fonte:** `crates/zylos-personalities/src/lib.rs`
- **Playbook:** `playbooks/onboarding-devs.md` (personalidades para novos devs)
- **Runbook relacionado:** `zylos-swarm-orchestrator.md` (agentes com personalidade assigned)
- **SOAR workflow:** `docs/soar/runbooks/personality-matching.runbook.yaml`
- **HANDOFF_7:** `HANDOFF_7.md`