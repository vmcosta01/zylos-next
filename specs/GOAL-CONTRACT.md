# Goal Contract — Zylos-Next v6.0

## Visão Geral
Este documento define o contrato de verificação obrigatório para toda task do Zylos-Next. Nenhuma task pode ser declarada "pronta" sem um comando `verify:<cmd>` executável que prove o resultado. Violação = trabalho inválido (Conceito #5, #9).

## Estrutura do Goal Contract

Toda spec técnica DEVE conter:

```yaml
goal_contract:
  id: "<uuid>"
  title: "<descrição curta>"
  context: "<por que esta task existe>"
  files_relevant: ["path/to/file1.rs", "path/to/file2.rs"]
  constraints:
    - "<constraint técnica ou de negócio>"
  acceptance_criteria:
    - "<critério testável e mensurável>"
  expected_tests:
    - "test_<function>_<scenario>"
  out_of_scope:
    - "<o que NÃO faz parte desta task>"
  verify:
    cmd: "<comando shell que prova o resultado>"
    timeout_ms: 30000
    expected_exit_code: 0
```

## Exemplos de verify:<cmd>

### Crate compila sem warnings
```yaml
verify:
  cmd: "cargo check --workspace --all-targets 2>&1 | grep -c 'warning:' | xargs test 0 -eq"
  timeout_ms: 120000
  expected_exit_code: 0
```

### Testes passam
```yaml
verify:
  cmd: "cargo test --workspace --all-features -- --quiet 2>&1 | tail -1 | grep -q 'test result: ok'"
  timeout_ms: 300000
  expected_exit_code: 0
```

### Benchmark dentro do target
```yaml
verify:
  cmd: "cargo bench --bench bot_bench -- --noplot 2>&1 | grep 'agent_spawn' | awk '{print $NF}' | awk '{if ($1 < 10.0) exit 0; else exit 1}'"
  timeout_ms: 180000
  expected_exit_code: 0
```

### Schema JSON válido
```yaml
verify:
  cmd: "python3 -c \"import jsonschema, json; jsonschema.validate(json.load(open('specs/schemas/agent-config.json')), json.load(open('specs/schemas/meta-schema.json')))\""
  timeout_ms: 5000
  expected_exit_code: 0
```

### Documento existe e tem seções obrigatórias
```yaml
verify:
  cmd: "grep -q '## Out of Scope' PRD.md && grep -q '## Target Users' PRD.md && grep -q '## Definition of Done' PRD.md"
  timeout_ms: 1000
  expected_exit_code: 0
```

### Security audit limpo
```yaml
verify:
  cmd: "cargo audit 2>&1 | grep -c 'Crate:.*severity:.*high\\|critical' | xargs test 0 -eq"
  timeout_ms: 60000
  expected_exit_code: 0
```

## Hook verificar-gate

O hook `verificar-gate` é executado automaticamente antes de qualquer declaração de conclusão. Ele:

1. Extrai o campo `verify.cmd` da spec da task
2. Executa o comando com timeout definido
3. Compara exit code com `expected_exit_code`
4. **BLOQUEIA** a conclusão se o comando falhar ou não existir
5. Registra o resultado em `evals/audits/<task-id>-verify.log`

### Implementação (pre-commit hook)
```bash
#!/bin/bash
# .git/hooks/pre-push ou CI gate
SPEC_FILE="$1"
if ! grep -q 'verify:' "$SPEC_FILE"; then
  echo "❌ BLOCKED: No verify:<cmd> found in $SPEC_FILE"
  exit 1
fi
CMD=$(grep 'cmd:' "$SPEC_FILE" | head -1 | sed 's/.*cmd: *"\(.*\)"/\1/')
TIMEOUT=$(grep 'timeout_ms:' "$SPEC_FILE" | head -1 | awk '{print $2}')
EXPECTED=$(grep 'expected_exit_code:' "$SPEC_FILE" | head -1 | awk '{print $2}')
timeout "${TIMEOUT:-30000}ms" bash -c "$CMD"
ACTUAL=$?
if [ "$ACTUAL" != "${EXPECTED:-0}" ]; then
  echo "❌ BLOCKED: verify command failed (exit $ACTUAL, expected ${EXPECTED:-0})"
  exit 1
fi
echo "✅ verify-gate passed"
```

## Anti-Patterns (Proibidos)

| Anti-Pattern | Por que é proibido | Alternativa correta |
|---|---|---|
| `verify: "manual review"` | Não é automatizável nem reproduzível | Escrever teste ou script |
| `verify: "looks good"` | Subjetivo, não verificável | Métrica mensurável + cmd |
| `verify: "TODO"` | Placeholder = trabalho incompleto | Implementar antes de declarar pronto |
| Sem campo `verify` | Viola Conceito #9 | Adicionar cmd obrigatório |
| `verify.cmd` que sempre passa | Teste inútil | Cmd deve falhar se resultado estiver errado |

## Referências
- [Manifesto Conceito #5](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md) — Spec Antes de Código
- [Manifesto Conceito #9](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md) — Code Review Gate
- [DESIGN.md](./DESIGN.md) para schemas tipados
- [TESTING.md](../TESTING.md) para estratégia de testes