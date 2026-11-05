# HANDOFF_3: Skills Registry & Eval Engine

**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Fase:** 3 — Agent Orchestration & Skills Registry
**Antecessor:** HANDOFF_2.md
**Sucessor:** HANDOFF_4.md

---

## 1. O Que Foi Entregue

### 1.1 Skills Registry (`crates/zylos-skills/`)
- **SkillManifest:** Schema YAML/JSON para definição de skills (nome, versão, input/output schema, triggers, shortcuts)
- **SkillLoader:** Carrega skills de `~/.zylos/skills/` ou registry remoto, valida contra schema
- **SkillIndex:** Index invertido por trigger/capability para lookup eficiente em runtime
- **Registry API:** Endpoints REST para CRUD de skills no daemon (`/api/v1/skills/*`)
- **Export Adapters:** Geração automática de `.cursorrules`, `.claude/settings.json`, `.windsurfrules` a partir do manifest

### 1.2 Eval Engine (`crates/zylos-eval/`)
- **EvalCase:** Estrutura de caso de teste com input, expected_output, context e criteria_override
- **CriterionScore:** Score por critério (accuracy, safety, completeness, latency) com reasoning do juiz LLM
- **EvalReport:** Relatório completo com overall_score ponderado, verdict (pass/fail/needs_review) e timing
- **LLM Judge Integration:** Suporte a OpenAI, Anthropic e Ollama como provedores de julgamento
- **CLI Integration:** `zylos skill eval <name>` executa suite de avaliação e gera report JSON/HTML

### 1.3 Test Suite
- 47 testes unitários cobrindo parsing de manifest, validação de schema, scoring e edge cases
- 8 testes de integração com registry real (in-memory + filesystem)
- Benchmarks de load time (<5ms para 100 skills indexadas)

---

## 2. Decisões Técnicas

| Decisão | Racional | Alternativa Rejeitada |
|---------|----------|----------------------|
| Manifest em YAML (não TOML) | Compatibilidade com ecossistema MCP/Claude; parsers maduros | TOML (menos suporte em tooling externo) |
| Eval via LLM-as-judge (não regex) | Skills produzem output livre; critérios semânticos exigem compreensão | Heurísticas fixas (falso positivo alto) |
| Index invertido em memória | Lookup O(1) por trigger; skills são poucas (<1000 típico) | SQLite FTS (overhead desnecessário) |
| Export adapters como pós-processamento | Não acopla formato externo ao core; fácil adicionar novos targets | Gerar durante build (rigidez) |

---

## 3. Gaps Conhecidos e Dívida Técnica

- **Skill versioning:** Registry atual é flat; não suporta múltiplas versões simultâneas da mesma skill
- **Eval caching:** Cada eval chama LLM judge do zero; deveria cachear scores para inputs idênticos
- **Remote registry auth:** Token-based apenas; falta OAuth/device flow para CI/CD
- **Shortcut validation:** `shortcut: skip-validation` existe mas não tem auditoria de uso
- **i18n em manifests:** Campos description/name são en-only; falta suporte a localização

---

## 4. Lições Aprendidas

1. **Schema validation cedo:** Validar manifest no load previne panics em runtime. Bugs de tipo (input_schema errado) eram silenciosos antes da validação estrita.
2. **LLM judge precisa de rubrica explícita:** Sem critérios definidos no EvalCase, o juiz inventa padrões inconsistentes. Sempre passar criteria_override ou defaults globais.
3. **Export adapters quebram com campos opcionais:** Adapters assumiam campos obrigatórios que eram Option<T>. Agora todos usam serde(default).
4. **Index rebuild é caro:** Evitar rebuild completo em hot path. Usar incremental add/remove.

---

## 5. Próximos Passos (Fase 4+)

- Implementar versioning no registry (semver + alias `stable`/`latest`)
- Adicionar cache de eval results (SQLite local, TTL configurável)
- Criar `zylos skill audit` para detectar shortcuts perigosos
- Integrar eval engine no CI (fail PR se overall_score < threshold)
- Documentar formato de manifest em docs/specs/skill-manifest.md

---

## 6. Referências

- **Código:** `crates/zylos-skills/src/`, `crates/zylos-eval/src/`
- **Tests:** `crates/zylos-skills/tests/`, `crates/zylos-eval/tests/`
- **ADR relevante:** ADR-018 (Skills Registry Architecture)
- **Runbook relacionado:** `zylos-cli.md` §4.1 (comandos de skill)