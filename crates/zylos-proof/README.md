# zylos-proof

Engine de prova determinística com 5 níveis de ground, cápsulas de evidência content-addressable, verificação adversarial SANTA e motor de honestidade zero-LLM. Portado do CORTEX proof engine para Rust.

## Status vs Master Plan (Fase 3 + 
| Task | Descrição | Status |
|------|-----------|--------|
| 3.2.2 | Panel Adversarial SANTA | ✅ Implementado (stub determinístico) |
| 7B.1.1 | Honesty Motor (peso_de/leitura_bool/decide_delivery/build_report/checar_completude) | ✅ Implementado |
| 7B.1.2 | EvidenceCapsule content-addressable | ✅ Implementado |
| 7B.1.3 | Three-phase verification (integrity/sufficiency/contradiction) | ✅ Implementado |
| 7B.1.4 | Transitive trust composition | ✅ Implementado |
| 7B.1.5 | Structured rebuttal (bidirectional refutation) | ✅ Implementado |

## Features Implementadas

- **Ground (5 níveis)**: Assumed → Observed → Confirmed → Verified → Proven, com `#[repr(u8)]` e conversão u8
- **Evidence**: SHA-256 content hash, verify_integrity, source tracking
- **ProofBuilder**: builder ergonômico com observe/confirm/verify helpers, chain_hash determinístico
- **VerificationGate**: min_ground + min_independent_sources + accept_llm_as_primary filtering
- **EvidenceCapsule**: content-addressable (SHA-256 digest canônico), TrustLevel (Untrusted/Internal/Verified/External), Completeness (Partial/Verified/Contradicted), ProofStatus lifecycle
- **Three-phase verify()**: integridade estrutural → suficiência de evidência → detecção de contradições
- **compose()**: transitive trust propagation (min para independentes, max para corroborativos), circular composition detection
- **refute()**: structured rebuttal bidirecional com RebuttalType (FactualError/LogicalFlaw/OutdatedEvidence/ScopeViolation)
- **SANTA**: 3-juiz stub determinístico (structural/sufficiency/consistency perspectives), ≥2/3 approval threshold, contested/refuted outcomes
- **Honesty Motor**: peso_de (clamp 1-5 + suspeito detection), leitura_bool PT/EN, normalizar_lista (MAX_ITENS=1000), campo_ex (panic-safe getter), decide_delivery (achado negativo bloqueante), build_report (truncamento honesto), checar_completude (screenshot manifest validation)

## API Pública

```rust
// Core types
pub enum Ground { Assumed, Observed, Confirmed, Verified, Proven }
pub struct Evidence { description, ground, content_hash, source, timestamp }
pub struct ProofResult { passed, required_ground, achieved_ground, evidence_chain, chain_hash }
pub struct VerificationGate { name, min_ground, accept_llm_primary, min_independent_sources }

// Builder
pub struct ProofBuilder { ... }
impl ProofBuilder {
    pub fn new(required_ground) -> Self;
    pub fn observe/confirm/verify(self, desc, content, source) -> Self;
    pub fn build(self) -> ProofResult;
}

// Evidence Capsules
pub struct EvidenceCapsule { finding, evidence_type, digest, completeness, trust_boundary, ground, refutes, refuted_by, status }
pub fn verify(capsules: &[EvidenceCapsule]) -> VerifyResult;
pub fn compose(capsules: &[EvidenceCapsule]) -> Result<EvidenceCapsule, ComposeError>;
pub fn refute(target: &mut EvidenceCapsule, counter: EvidenceCapsule, rebuttal_type) -> EvidenceCapsule;

// SANTA
pub fn santa_verify(capsules: &[EvidenceCapsule], config: &SantaConfig) -> SantaResult;

// Honesty
pub fn peso_de(stakes: f64) -> (u8, bool);
pub fn leitura_bool(valor: &str) -> BoolLeitura;
pub fn decide_delivery(ok, achado_negativo_aterrado, motivo) -> DeliveryDecision;
pub fn build_report(titulo, conteudo, max_chars, total_itens, itens_aterrados, custo) -> HonestReport;
pub fn checar_completude(manifesto, bytes) -> CompletudeResult;
```

## Gaps Críticos

1. **SANTA é stub determinístico**: juízes avaliam regras estruturais, não LLMs/oracles independentes reais
2. **Ground divergente entre crates**: zylos-proof usa 5 níveis (Assumed→Proven), zylos-verification usa 3 níveis (Crido/Observado/Confirmado) — unificação pendente
3. **Integração Proof ↔ EvidenceCapsule**: os dois sistemas coexistem mas não há adapter automático entre ProofResult e EvidenceCapsule
4. **Meta-analysis**: presente apenas como stub em zylos-verification/rsi.rs, não em zylos-proof

## Exemplo de Uso

```rust
use zylos_proof::{ProofBuilder, Ground, EvidenceCapsule, EvidenceType, Completeness, TrustLevel, santa_verify, SantaConfig};

// Prova simples
let proof = ProofBuilder::new(Ground::Confirmed)
    .observe("file exists", b"/src/lib.rs", "file_check")
    .confirm("tests pass", b"42/42", "test_runner")
    .build();
assert!(proof.passed);

// SANTA verification
let capsules = vec![
    EvidenceCapsule::new("check_a", EvidenceType::ToolOutput, "runner", Completeness::Verified, TrustLevel::External, Ground::Confirmed),
    EvidenceCapsule::new("check_b", EvidenceType::Artifact, "auditor", Completeness::Verified, TrustLevel::Verified, Ground::Verified),
];
let result = santa_verify(&capsules, &SantaConfig::default());
assert!(result.approved);
```