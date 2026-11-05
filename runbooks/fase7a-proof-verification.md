# Fase 7A: Proof Verification — Runbook Zylos-Next

## Visão Geral

O proof verification engine (`zylos-proof`) implementa provas determinísticas com 5 níveis de ground (Assumed→Proven), evidence capsules com hashing SHA-256, SANTA adversarial verification (3 juízes independentes, ≥2/3 concordância), e honesty scoring para detectar overclaiming.

**Módulos principais** (de `crates/zylos-proof/src/lib.rs`):
- Root: `Ground` enum (Assumed=0, Observed=1, Confirmed=2, Verified=3, Proven=4), `Evidence` struct (description, ground, content_hash, source, timestamp), `ProofResult` (passed, required_ground, achieved_ground, evidence_chain, chain_hash, failure_reason), `ProofBuilder`
- `evidence_capsule` — EvidenceCapsule com EvidenceType, Completeness, TrustLevel, ProofStatus, VerifyResult
- `honesty` — Honesty scoring para detectar overclaiming
- `santa` — SantaConfig, JudgeVote, SantaResult, santa_verify()

**Crates relacionados:**
- `zylos-verification` — Ground adapter, guardrail, judge, finding_validator, goal_contract, RSI, screenshot_verification, ToT reasoning, proof module

**Critério de Saída:** Guardrail rejeita falso-sucesso em <1ms; SANTA verifica com ≥2/3 concordância; evidence capsules têm integridade verificável via SHA-256.

## Pré-requisitos

- Fases 0-6 completas
- Dependências: `serde`, `sha2`, `chrono`, `hex`
- `zylos-core` para tipos compartilhados

### Setup

```bash
cargo build -p zylos-proof
cargo build -p zylos-verification
cargo test -p zylos-proof
cargo test -p zylos-verification

# Benchmarks
cargo bench -p zylos-verification
```

## Para Dev Senior

### Decisões Arquiteturais

1. **Ground Enum Hierárquico**: 5 níveis estritos (`Assumed < Observed < Confirmed < Verified < Proven`). Um claim só é aceito se o ground for >= ao mínimo exigido pelo gate. `#[repr(u8)]` para storage eficiente. Comparação via `PartialOrd`/`Ord` derivados.

2. **Evidence com SHA-256 Integrity**: Cada `Evidence` carrega `content_hash` computado via SHA-256. `verify_integrity()` re-computa hash e compara. `ProofResult::compute_chain_hash()` gera hash composto de toda a cadeia para auditoria.

3. **SANTA Adversarial Verification**: 3 juízes independentes avaliam evidence capsules de perspectivas diferentes: structural integrity, evidence sufficiency, consistency. Requer ≥2/3 concordância (`min_approvals: 2`). Desacordo resulta em `ProofStatus::Contested`. Stub determinístico para testing; produção substitui por LLMs diferentes ou auditores humanos.

4. **Evidence Capsules**: `EvidenceCapsule` encapsula `{Finding→Evidence→Decision→Disposition}` com `EvidenceType`, `Completeness`, `TrustLevel`. `verify()` valida integridade estrutural e retorna `VerifyResult` com confidence score, missing evidence list, e contradictions.

5. **Honesty Scoring**: Detecta overclaiming comparando ground claimed vs ground achieved. Score baixo indica agente que frequentemente afirma mais do que pode provar. Integrado com agent-track para gate de promoção.

6. **ProofBuilder Ergonomic**: Builder pattern para construir provas fluentemente: `.observe()`, `.confirm()`, `.verify()`, `.build()`. Cada método adiciona evidência com ground apropriado.

### Extension Points

- **Novos ground levels**: Adicione ao enum (cuidado: breaking change para serialization)
- **Custom SANTA judges**: Implemente evaluators reais (LLM, human auditor) substituindo stubs
- **Novos evidence types**: Estenda `EvidenceType` enum
- **Custom honesty metrics**: Adicione dimensões ao scoring (ex: calibration, specificity)
- **Chain-of-custody**: Estenda `Evidence` com provenance chain para forensic audit

### Performance Tuning

- SHA-256 hashing <1μs para evidências típicas (<1KB)
- SANTA verification <10ms (3 judges paralelos)
- Guardrail check <1ms (pure function, no I/O)
- Chain hash computation linear no número de evidências
- Cache hash computations para evidências imutáveis

## Para Dev Pleno

### Building a Proof

```rust
use zylos_proof::{ProofBuilder, Ground, Evidence, ProofResult};

let proof = ProofBuilder::new(Ground::Confirmed)
    .observe("File exists", b"ls -la output", "file_check")
    .confirm("Tests pass", b"cargo test output: 42 passed", "test_runner")
    .confirm("Hash matches", b"sha256sum: abc123...", "hash_check")
    .build();

assert!(proof.passed);
assert_eq!(proof.achieved_ground, Ground::Confirmed);
println!("Chain hash: {}", proof.chain_hash);
```

### Verifying Evidence Integrity

```rust
use zylos_proof::Evidence;

let evidence = Evidence::new(
    "API response",
    Ground::Observed,
    b"{"status": "ok"}",
    "http_client",
);

// Later, verify integrity
let original_content = b"{"status": "ok"}";
assert!(evidence.verify_integrity(original_content));

// Tampered content fails
assert!(!evidence.verify_integrity(b"{"status": "error"}"));
```

### SANTA Verification

```rust
use zylos_proof::santa::{santa_verify, SantaConfig};
use zylos_proof::evidence_capsule::{EvidenceCapsule, EvidenceType, Completeness, TrustLevel};
use zylos_proof::Ground;

let capsules = vec![
    EvidenceCapsule::new(
        "check_a",
        EvidenceType::ToolOutput,
        "runner_1",
        Completeness::Verified,
        TrustLevel::External,
        Ground::Confirmed,
    ),
    EvidenceCapsule::new(
        "check_b",
        EvidenceType::Artifact,
        "auditor",
        Completeness::Verified,
        TrustLevel::Verified,
        Ground::Verified,
    ),
];

let config = SantaConfig::default(); // 3 judges, min 2 approvals
let result = santa_verify(&capsules, &config);

println!("Approved: {}", result.approved);
println!("Status: {:?}", result.status);
for vote in &result.votes {
    println!("Judge {}: approved={}, confidence={:.2}, rationale={}", 
        vote.judge_id, vote.approved, vote.confidence, vote.rationale);
}
```

### Checking Weak Evidence

```rust
use zylos_proof::{ProofResult, Ground};

let proof: ProofResult = /* ... */;

// Find evidence below required ground
let weak = proof.weak_evidence();
for ev in &weak {
    eprintln!("Weak evidence: {} (ground: {}, required: {})", 
        ev.description, ev.ground, proof.required_ground);
}
```

## Para Dev Junior

### Passo a Passo: Criando uma Prova

1. **Determine o ground mínimo necessário**: Consulte gate requirements (ex: deploy precisa Confirmed)
2. **Colete evidências**: Cada evidência precisa de description, content bytes, source
3. **Use ProofBuilder**: `.observe()` para dados brutos, `.confirm()` para testes/hashes, `.verify()` para multi-source
4. **Build e verifique**: `.build()` retorna `ProofResult` com `passed` boolean
5. **Se falhou**: Cheque `failure_reason` e `weak_evidence()` para entender o que falta

### Pitfalls Comuns

- **Não use Ground::Assumed em produção**: Assumed é apenas para drafts/exploração
- **Não pule hash verification**: Sempre chame `verify_integrity()` antes de confiar em evidência
- **Não ignore contested status**: SANTA contested significa desacordo real — investigue
- **Não misture sources não independentes**: Verified requer fontes verdadeiramente independentes
- **Não forge timestamps**: Use `chrono::Utc::now()` sempre; nunca hardcode

### Debugging Tips

- `ProofResult::weak_evidence()` lista exatamente quais evidências falharam
- SANTA votes incluem rationale detalhado de cada juiz
- Chain hash permite verificar que nenhuma evidência foi alterada post-hoc
- Use `RUST_LOG=zylos_proof=debug` para ver verification traces
- Honesty score history revela padrões de overclaiming

## Troubleshooting

| Erro | Causa | Recuperação |
|------|-------|-------------|
| Proof `passed: false` | Ground achieved < required | Adicione mais evidências de ground superior |
| `verify_integrity()` returns false | Content foi alterado após criação | Re-colete evidência, investigue tampering |
| SANTA `status: Contested` | Juízes discordam (1-2 split) | Investigue rationale de cada juiz, adicione evidências |
| SANTA `status: Refuted` | Todos juízes rejeitaram | Revisite premissas, colete evidências melhores |
| `chain_hash` mismatch | Evidência adicionada/removida após hash | Reconstrua prova do zero |
| Honesty score baixo | Agente overclaims frequentemente | Revise training, adicione guardrails |
| Guardrail false positive | Threshold muito agressivo | Ajuste threshold, adicione exceções documentadas |

## Referências

- **Source**: `crates/zylos-proof/src/` (3 módulos: lib.rs, evidence_capsule.rs, honesty.rs, santa.rs)
- **Related**: `crates/zylos-verification/src/` (11 módulos + benches)
- **Key files**: `zylos-proof/src/lib.rs:1-200` (Ground, Evidence, ProofResult, ProofBuilder), `santa.rs:1-175` (SANTA verification), `evidence_capsule.rs`, `honesty.rs`
- **Verification modules**: `zylos-verification/src/guardrail.rs`, `judge.rs`, `finding_validator.rs`, `goal_contract.rs`
- **Benchmarks**: `crates/zylos-verification/benches/verification.rs`
- **Master Plan**: Linhas 189-192 (Fase 3 VERIFICATION_PROOF overview)