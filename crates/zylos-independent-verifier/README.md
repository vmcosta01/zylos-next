# zylos-independent-verifier

Verificação adversarial de outputs de agentes **sem contexto compartilhado**.

## Visão Geral

Este crate implementa um verificador independente que valida outputs de agentes usando apenas:
- O output bruto (bytes/string)
- Metadados declarativos (hash esperado, tamanho máximo, schema)
- Regras de validação determinísticas

**Princípio fundamental:** o verificador NUNCA acessa o estado interno do agente, logs, ou histórico de execução. A verificação é puramente baseada no artefato produzido.

## Arquitetura

```
VerificationRequest {
    output: Vec<u8>,           // Output bruto do agente
    expected_hash: Option<String>,  // SHA-256 esperado (opcional)
    max_size_bytes: usize,     // Limite de tamanho
    schema: Option<Value>,     // JSON Schema para validação estrutural
}

verify_output(request) → Result<VerificationResult, VerifyError>
```

## Funcionalidades

- **Hash verification**: SHA-256 do output vs hash esperado
- **Size limits**: Rejeita outputs acima do limite configurado
- **Schema validation**: Validação contra JSON Schema draft-07
- **Adversarial mode**: Assume que o output pode ser malicioso; nenhuma confiança implícita
- **Zero shared state**: Sem acesso a EventBus, stores, ou estado de outros crates

## Uso

```rust
use zylos_independent_verifier::{verify_output, VerificationRequest};

let request = VerificationRequest {
    output: b"resultado do agente".to_vec(),
    expected_hash: Some("abc123...".into()),
    max_size_bytes: 1024 * 1024, // 1MB
    schema: None,
};

match verify_output(&request) {
    Ok(result) => println!("Válido: {:?}", result),
    Err(e) => eprintln!("Falha: {}", e),
}
```

## Testes

16 testes cobrindo:
- Hash match/mismatch
- Size limit enforcement
- Schema validation (valid/invalid)
- Edge cases (empty output, oversized, malformed hash)

```bash
cargo test -p zylos-independent-verifier
```

## Integração

- **Fase 8**: T8.1.4 — Adversarial verification without shared context
- **Hermes Bridge**: `/api/v1/hermes/security/sanitizer/sanitize` (proxy)
- **SOAR Workflow**: `soar-guardrail-enforcement.md` Step 2 (Contain)

## Cross-References

- [HANDOFF_FASE8.md](../../docs/handoffs/HANDOFF_FASE8.md) §2.3
- [POP-Skill-Contract-Validation](../../playbooks/pop-skill-contract-validation.md)
- [SOAR Guardrail Enforcement](../../playbooks/soar-guardrail-enforcement.md)