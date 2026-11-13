# Playbook: Skill Evolution Lifecycle (Fase 8)

**Versão:** 1.0
**Data:** 2026-11-11
**Autor:** Implementation Agent (Fase 8)
**Skill:** `playbook-author`
**Módulos:** `zylos-skills::{contract,evolution,miner,trajectory,hierarchy}`

---

## 1. Objetivo

Orquestrar o ciclo completo de evolução de skills: mineração de candidatos a partir de logs → compilação via trajectory → validação de contract formal → registro no evolution graph → promoção por tier hierárquico.

## 2. Gatilhos

- Novo padrão recorrente detectado em gateway logs (>5 ocorrências/dia)
- HANDOFF documentado sem skill correspondente
- Playbook YAML criado sem contract formal
- Skill existente com fitness metric abaixo do threshold por 7 dias consecutivos

## 3. Papéis (RACI)

| Papel | Responsabilidade |
|-------|-----------------|
| SkillMiner Agent | Extrair candidatos de logs automaticamente |
| TrajectoryCompiler | Converter HANDOFF/playbook → SkillContract |
| ContractValidator | Validar (P,O,A,V,F) antes de registro |
| EvolutionGraph Manager | Registrar edges (Extends/Replaces/Repairs/DependsOn) |
| HierarchyClassifier | Classificar tier (Planning/Functional/Atomic) |
| Human Reviewer | Aprovar promoção para tier Planning |

## 4. Fluxo

### 4.1 Mineração (Automático)

```
Gateway Logs → SkillMiner → CandidatePattern[] → EvalFlywheel gate
```

- Regex patterns extraídos de tool call sequences repetidas
- Candidatos com score < 0.3 são descartados automaticamente
- Candidatos aprovados geram draft SkillContract

### 4.2 Compilação via Trajectory

```
HANDOFF.md / playbook.yaml → TrajectoryCompiler → SkillContract draft
```

- Parser extrai preconditions de "Pré-requisitos" section
- Output schema inferido de "Deliverables" section
- Assertions geradas de "Acceptance Criteria"

### 4.3 Validação Formal

```
SkillContract draft → ContractValidator → Pass/Fail + diagnostics
```

Critérios obrigatórios:
- [ ] P: ≥1 precondition avaliável
- [ ] O: JSON Schema draft-07 válido
- [ ] A: ≥1 assertion com validator registrado
- [ ] V: validator retorna resultado em ≤5s
- [ ] F: baseline metric definido

### 4.4 Registro no Evolution Graph

```
Validated Contract → EvolutionGraph.add_node() → Edge classification
```

Edge types:
- **Extends**: nova skill herda de existente (super-set)
- **Replaces**: nova skill substitui obsoleta (backward compat)
- **Repairs**: fix para skill existente (patch version)
- **DependsOn**: dependência funcional entre skills

### 4.5 Classificação Hierárquica

```
Registered Skill → HierarchyClassifier → Tier assignment
```

Tiers:
- **Planning**: orquestra outras skills (ex: `incident-response`)
- **Functional**: executa domínio específico (ex: `web-search`)
- **Atomic**: operação unitária (ex: `http-get`, `file-read`)

Promoção para Planning requer aprovação humana.

## 5. Gates de Qualidade

| Gate | Critério | Bloqueante? |
|------|----------|-------------|
| Miner Score | ≥0.3 confidence | Sim |
| Contract Validation | All (P,O,A,V,F) pass | Sim |
| Evolution Consistency | No cycles in graph | Sim |
| Tier Promotion | Human approval for Planning | Sim |
| Fitness Baseline | Metric defined + measurable | Sim |

## 6. Rollback

Se skill evolvida causa regressão:

1. `EvolutionGraph.retire(skill_id)` — marca como deprecated
2. Reativar predecessor via `EvolutionGraph.revert(edge_id)`
3. Criar incident ticket com tag `skill-evolution-regression`
4. SkillMiner recebe feedback negativo para ajustar scoring

## 7. Métricas de Sucesso

- Tempo médio de mineração → contract: <2h
- Taxa de validação first-pass: >80%
- Skills ativas por tier: Planning ≤20%, Functional 40-60%, Atomic 30-40%
- Fitness metric improvement após evolução: >10% em 30 dias

## 8. Cross-References

- **POPs:** `pop-skill-contract-validation.md`
- **Runbook:** `runbooks/zylos-skills.md` (a criar/atualizar)
- **Handoff:** `docs/handoffs/HANDOFF_FASE8.md` §2.6
- **Código:** `crates/zylos-skills/src/{contract,evolution,miner,trajectory,hierarchy}.rs`
- **SOAR:** `playbooks/soar-guardrail-enforcement.md`

## 9. Histórico

| Versão | Data | Autor | Mudança |
|--------|------|-------|---------|
| 1.0 | 2026-11-11 | Implementation Agent | Criação inicial |