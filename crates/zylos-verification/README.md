# zylos-verification

Motor de verificação estruturada para agentes Zylos. Implementa GoalContracts com validação em três fases, protocolo RSI (Reproducible Scientific Investigation), painel adversarial multi-juiz, guardrails sub-milissegundo e engine de provas com cadeia de evidência.

## Status vs Master Plan (Fase 3)

| Task | Descrição | Status |
|------|-----------|--------|
| 3.1.1 | GoalContract + RSI | ✅ Implementado |
| 3.1.2 | ProofEngine + Guardrail | ✅ Implementado |
| 3.2.1 | Judge multi-strategy | ⚠️ Parcial — AdversarialPanel existe mas sem integração LLM/oracle real |
| 3.2.2 | Panel Adversarial SANTA | ❌ Ausente neste crate (implementado em zylos-proof) |
| 3.2.3 | Finding-Validator oracle cross-validation | ❌ Ausente |
| 3.3.1 | ToT beam-search | ❌ Ausente |
| 3.3.2 | ScreenshotVerification | ❌ Ausente neste crate (checar_completude em zylos-proof/honesty) |
| 3.3.3 | Benchmarks | ❌ Ausente |

## Features Implementadas

- **GoalContract**: validação schema/preconditions/dry-run, parsing de DESIGN.md (`[CMD]`/`[FILE]`/`[API]`), templates Handlebars (code_review/deploy), versionamento com digest SHA-256, rollback
- **RSI Protocol**: state machine 7 fases (Hypothesis→Log), phase gates estritos, blinded evaluation com lift pós-Analyze, pre-registration HITL enforcement, StatisticalGuardrail (Bonferroni/Holm), meta-analysis DerSimonian-Laird stub
- **Ground**: enum Crido/Observado/Confirmado com promote/demote explícito, ordering, serde
- **Guardrail**: detecção sub-ms de false-success (empty_output, placeholder_heavy, error_in_success, truncated_json), check_as_proof integration
- **Proof Engine**: Evidence chain com ground levels, compose (min-ground), refute, verify, weakest_evidence_ground
- **AdversarialPanel**: multi-judge verdict collection, veto_on_refute, min_agreement consensus, to_proof conversion

## API Pública

```rust
// GoalContract
pub struct GoalContract { ... }
impl GoalContract {
    pub fn validate_schema(&self) -> Result<()>;
    pub fn evaluate_preconditions(&self) -> Result<()>;
    pub fn verify_dry_run(&self) -> Result<()>;
    pub fn from_design_md(md: &str) -> Result<Self>;
    pub fn to_design_md(&self) -> String;
    pub fn from_template(tmpl: &str, ctx: &Value) -> Result<Self>;
}

// RSI
pub struct Experiment { ... }
impl Experiment {
    pub fn new(hypothesis, design_version, dataset_hash, code_commit, env) -> Self;
    pub fn advance_phase(&mut self) -> Result<RsiPhase>;
    pub fn pass_gate(&mut self);
    pub fn decide(&mut self) -> Result<DecisionOutcome>;
    pub fn lift_blind(&mut self) -> Result<&str>;
}

// Proof
pub struct Proof { ... }
impl Proof {
    pub fn new(claim, ground) -> Self;
    pub fn add_evidence(&mut self, evidence);
    pub fn verify(&mut self) -> ProofStatus;
    pub fn compose(a, b, claim) -> Proof;
    pub fn refute(&mut self, counter_evidence);
}

// Guardrail
pub struct Guardrail { ... }
impl Guardrail {
    pub fn check(&self, output, claimed_status) -> GuardrailResult;
    pub fn check_as_proof(&self, output, status, desc) -> Proof;
}

// Panel
pub struct AdversarialPanel { ... }
impl AdversarialPanel {
    pub fn add_verdict(&mut self, verdict);
    pub fn deliberate(&self) -> PanelResult;
    pub fn to_proof(&self, claim) -> Proof;
}
```

## Gaps Críticos

1. **Finding-Validator (Task 3.2.3)**: oracle cross-validation não implementado
2. **ToT beam-search (Task 3.3.1)**: tree-of-thought search ausente
3. **ScreenshotVerification (Task 3.3.2)**: lógica de verificação visual ausente neste crate (parcial em zylos-proof honesty)
4. **Juiz LLM/oracle real**: AdversarialPanel coleta verdicts mas não tem integração com LLM ou oracle externo — é um framework de consolidação apenas
5. **Benchmarks (Task 3.3.3)**: nenhum benchmark de latência ou acurácia

## Exemplo de Uso

```rust
use zylos_verification::{GoalContract, Guardrail, Proof, Ground};

// Verificar output de agente
let g = Guardrail::new();
let proof = g.check_as_proof("42 tests passed", "success", "tests ok");
assert!(proof.is_valid());

// GoalContract a partir de DESIGN.md
let contract = GoalContract::from_design_md(r#"
## Acceptance Criteria
- [CMD] cargo test → exit 0
- [FILE] target/release/app exists
"#)?;
contract.validate_schema()?;
```