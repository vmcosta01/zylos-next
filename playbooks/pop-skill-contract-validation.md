# POP-Skill-Contract-Validation — Validação de Skill Contracts (T8.4.1)

**Versão:** 1.0
**Data:** 2026-11-11
**Autor:** Implementation Agent (Fase 8)
**Skill:** `pop-author`
**Módulo:** `zylos-skills::contract`

---

## 1. Objetivo

Procedimento operacional para validar SkillContracts formais (P,O,A,V,F) antes de deploy em produção, garantindo que preconditions, output schemas e fitness metrics estejam corretos.

## 2. Pré-requisitos

- Acesso ao gateway API (`http://localhost:9876`)
- Token com permissão `skills:validate`
- SkillContract YAML/JSON definido conforme schema `(P,O,A,V,F)`

## 3. Procedimento

### 3.1 Validar Contract Localmente

```bash
cargo test -p zylos-skills contract -- --test-threads=1
```

### 3.2 Validar via API

```bash
curl -X POST http://localhost:9876/api/v1/skills/contracts/validate \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d @skill_contract.json
```

### 3.3 Verificar Evolution Graph Consistency

```bash
curl http://localhost:9876/api/v1/skills/evolution/graph \
  -H "Authorization: Bearer $TOKEN" | jq '.edges[] | select(.type == "Extends")'
```

### 3.4 Run Trajectory Compiler (HANDOFF → Contract)

```bash
curl -X POST http://localhost:9876/api/v1/skills/trajectory/compile \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"source": "handoff", "path": "docs/handoffs/HANDOFF_FASE8.md"}'
```

## 4. Critérios de Aceitação

- [ ] Todas as preconditions (P) são avaliáveis como boolean
- [ ] Output schema (O) é válido JSON Schema draft-07
- [ ] Assertions (A) têm pelo menos um validator registrado
- [ ] Validators (V) retornam `ValidatorResult` dentro de 5s
- [ ] Fitness metrics (F) têm baseline definido

## 5. Rollback

Se validação falhar em produção:

1. Reverter skill para versão anterior via evolution graph: `POST /api/v1/skills/evolution/revert`
2. Notificar owner da skill via Slack/Notion
3. Criar issue com tag `skill-contract-regression`

## 6. Cross-References

- **Runbook:** `runbooks/zylos-skills.md` (a criar)
- **Handoff:** `docs/handoffs/HANDOFF_FASE8.md` §2.6
- **Código:** `crates/zylos-skills/src/contract.rs`, `evolution.rs`, `trajectory.rs`
- **Playbook relacionado:** `playbooks/workflow-orchestration.md`

## 7. Histórico

| Versão | Data | Autor | Mudança |
|--------|------|-------|---------|
| 1.0 | 2026-11-11 | Implementation Agent | Criação inicial |