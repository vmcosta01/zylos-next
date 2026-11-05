# Runbook & Playbook: Fase 3 — VERIFICATION_PROOF (Ground Enum + Proof Engine + Panel Adversarial)

**Sistema:** VERIFICATION_PROOF — Fase 3
**Crate:** `zylos-verification`
**Módulos:** `ground`, `proof_engine`, `judge`, `panel`, `finding_validator`, `tot_reasoning`, `screenshot_verification`
**Última Atualização:** 31 de outubro de 2026
**Status Operacional:** ✅ Crate funcional, auditoria dupla pendente

---

## 🎯 Visão Geral (Todos os Níveis)

A Fase 3 implementa o sistema de verificação e prova do Zylos-Next: enumeração de níveis de ground (Observado/Confirmado/Crido), engine de proofs com verify/compose/refute, panel adversarial com ≥3 juízes independentes, validador de achados com oráculos isolados, módulo ToT reasoning (BFS/DFS) e verificação de screenshots. Este crate garante que nenhuma afirmação de agente é aceita sem prova verificável e auditável.

---

## 👨‍💻 NÍVEL SENIOR — Arquitetura e Decisões

### Responsabilidades
- Validar que o Ground Enum previne mistura de níveis incompatíveis em compile-time
- Garantir que o Proof Engine opera em <1ms para verify/compose/refute
- Auditar independência dos juízes no Panel Adversarial (nenhum juiz influencia outro)
- Definir critérios de falso-sucesso e thresholds de deliberação
- Validar overhead do ToT reasoning (<50% quando ativado)

### Procedimentos

#### S3.1: Auditoria de Type Safety do Ground Enum
```rust
use zylos_verification::ground::{Ground, Observado, Confirmado, Crido};

// Compile-time: conversão implícita entre níveis deve falhar
// let obs: Observado = Observado::new(data);
// let conf: Confirmado = obs; // ERRO DE COMPILAÇÃO ESPERADO
// Conversão explícita requer justificativa auditável
let conf = obs.promote_to_confirmed(Justificativa::CrossReference(vec![source_a, source_b]))?;
```
**Critério Senior:** Zero conversões implícitas passam pelo compilador. Toda promoção/rebaixamento gera log de auditoria.

#### S3.2: Benchmark do Proof Engine
```bash
cd C:/Users/Paulo/zylos-next
cargo bench -p zylos-verification -- proof_engine_verify_1k_proofs
```
**Critério Senior:** verify() <1ms, compose() <1ms, refute() <1ms. Se exceder, investigar alocações ou cloning desnecessário.

#### S3.3: Validação de Independência no Panel
```rust
use zylos_verification::panel::{Panel, Judge};

let panel = Panel::new(vec![
    Box::new(LLMJudge::new(model_a)),
    Box::new(DeterministicJudge::new(rules)),
    Box::new(CompositeJudge::new(vec![judge_x, judge_y])),
]);

// Cada juiz delibera sem acesso às refutações dos outros
// Refutação soberana combina resultados de forma determinística
let deliberation = panel.deliberar(claim).await?;
assert_eq!(deliberation.juizes_independentes, 3);
```
**Critério Senior:** Juízes executam em paralelo sem shared state. Refutação soberana é determinística e reproduzível.

#### S3.4: Threshold de Falso-Sucesso no Guardrail
```rust
use zylos_verification::guardrail::GuardrailShortCircuit;

let guardrail = GuardrailShortCircuit::new(config);
// Testar com casos conhecidos de falso-sucesso
for case in false_positive_cases {
    let result = guardrail.check(case);
    assert!(!result.is_success(), "Falso-sucesso detectado: {:?}", case);
}
```
**Critério Senior:** Taxa de falso-negativo <1%. Detector opera em <1ms sem LLM.

### Decisões Documentadas
| Decisão | Justificativa | Alternativa Rejeitada |
|---------|--------------|----------------------|
| Ground como enum com tipos distintos | Compile-time safety; impossível misturar níveis acidentalmente | String tags (runtime error only) |
| Proof como struct imutável | Proofs são fatos históricos; mutabilidade invalida auditoria | Mutable proof com versioning (complexidade desnecessária) |
| Panel com ≥3 juízes | Consenso mínimo para evitar viés individual; par = deadlock possível | 2 juízes (empate frequente), 5+ (latência excessiva) |
| ToT como feature flag | Overhead significativo; apenas para tarefas complexas | Sempre ativo (performance inaceitável em casos simples) |
| Screenshot verification com magic bytes | Detecta arquivos corrompidos/falsos antes de processamento caro | Apenas checksum (não detecta formato inválido) |

---

## 👷 NÍVEL PLENO — Implementação e Validação

### Responsabilidades
- Executar testes unitários e validar cobertura de todos os módulos
- Configurar cenários de integração proof + panel + finding-validator
- Documentar padrões de criação de juízes customizados
- Implementar benchmarks e otimizar hot paths

### Procedimentos

#### P3.1: Rodar Suite Completa de Testes
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-verification --lib
```
**Esperado:** Todos os testes passando. Verificar contagem contra Master Plan (Ground: 5+, Proof: 8+, Panel: 6+, Finding: 4+, ToT: 5+, Screenshot: 4+).

#### P3.2: Cenário de Integração — Prova com Deliberação
```rust
use zylos_verification::{Proof, Panel, LLMJudge, DeterministicJudge, CompositeJudge};

// 1. Criar proof com ground observado
let proof = Proof::builder()
    .ground(Ground::Observado(data))
    .evidence(vec![source_a, source_b])
    .timestamp(Utc::now())
    .build()?;

// 2. Verificar proof
assert!(proof.verify()?);

// 3. Submeter ao panel adversarial
let panel = Panel::new(vec![
    Box::new(LLMJudge::new("gpt-4")),
    Box::new(DeterministicJudge::new(rules)),
    Box::new(CompositeJudge::new(vec![judge_x])),
]);
let deliberation = panel.deliberar(proof.clone()).await?;

// 4. Verificar refutação soberana
if deliberation.refutada {
    eprintln!("Refutado por: {:?}", deliberation.refutacao_so Soberana);
} else {
    println!("Prova aceita com acordo de {}/{} juízes", 
             deliberation.acordo_count, deliberation.total_juizes);
}
```

#### P3.3: Validar Finding-Validator com Oráculos Isolados
```rust
use zylos_verification::finding_validator::FindingValidator;

let validator = FindingValidator::new();
validator.register_finding("achado-1", oracle_1)?;
validator.register_finding("achado-2", oracle_2)?;

// Validação cruzada detecta inconsistências
let cross_check = validator.cross_validate()?;
if !cross_check.consistente {
    eprintln!("Inconsistência: {:?}", cross_check.conflitos);
}
```

#### P3.4: Benchmark e Otimização
```bash
cargo bench -p zylos-verification
cargo flamegraph --bin zylos-verification-bench --output flame.svg
```
**Critério Pleno:** Proof <1ms, Panel <5s, ToT overhead <50%. Hot paths identificados no flamegraph são otimizados.

### Checklist de Validação
- [ ] `cargo test -p zylos-verification --lib` passa todos os testes
- [ ] Ground Enum previne conversões implícitas em compile-time
- [ ] Proof Engine verify/compose/refute <1ms
- [ ] Panel com 3 juízes delibera em <5s
- [ ] Guardrail rejeita falso-sucesso em <1ms
- [ ] ToT reasoning overhead <50% quando ativado
- [ ] Screenshot verification detecta inválidos >99%
- [ ] Clippy zero warnings

---

## 🌱 NÍVEL JÚNIOR — Execução Guiada

### Responsabilidades
- Rodar testes e reportar resultados
- Seguir exemplos de uso sem modificar
- Reportar erros com output completo

### Procedimentos Passo-a-Passo

#### J3.1: Compilar o Crate
```bash
cd C:/Users/Paulo/zylos-next
cargo build -p zylos-verification
```
✅ `Finished` → prossiga
❌ Erro → copie output completo e reporte ao Pleno

#### J3.2: Rodar Testes
```bash
cargo test -p zylos-verification --lib 2>&1 | tee test-fase3.log
```
✅ `test result: ok. X passed` → sucesso
❌ Falha → anote nome do teste + mensagem de erro

#### J3.3: Verificar Exemplo Básico
Leia `crates/zylos-verification/examples/proof_demo.rs` (se existir) e execute:
```bash
cargo run -p zylos-verification --example proof_demo
```
Se não existir exemplo, reporte ao Pleno como gap de documentação.

### Glossário para Júniores
| Termo | Significado |
|-------|------------|
| **Ground** | Nível de certeza de um fato: Observado (visto diretamente), Confirmado (verificado por fonte externa), Crido (inferido sem prova direta) |
| **Proof** | Estrutura imutável que encapsula ground, evidências e timestamp; pode ser verificada, composta ou refutada |
| **Panel Adversarial** | Conjunto de ≥3 juízes independentes que deliberam sobre uma claim; decisão final é a refutação soberana |
| **Juiz** | Componente que tenta refutar uma claim; pode ser LLM-based, determinístico (regras fixas) ou composto |
| **Refutação Soberana** | Decisão final do panel combinando resultados de todos os juízes de forma determinística |
| **ToT Reasoning** | Tree-of-Thoughts: exploração BFS/DFS de caminhos de raciocínio com avaliação estruturada |
| **Guardrail Short-Circuit** | Detector rápido (<1ms) que rejeita falso-sucesso sem chamar LLM |
| **Finding-Validator** | Valida achados individualmente com oráculos isolados e faz validação cruzada entre achados |

### Quando Escalar
- Teste falha intermitentemente → **Pleno** (possível race condition em panel paralelo)
- Proof verify demora >1ms → **Senior** (performance regression)
- Juízes compartilham estado → **Senior imediatamente** (violação de independência)
- Ground conversion compila quando não deveria → **Senior imediatamente** (type safety quebrada)

---

## 🔗 Referências Cruzadas
- **Master Plan:** `zylos-next-master-plan.md` linhas 807-974 (Fase 3)
- **Pré-requisito:** `runbooks/fase2-memory-system.md`
- **Próxima Fase:** `runbooks/fase4-sandbox-isolation.md`
- **Auditorias:** `evals/audits/fase3-audit.md`, `evals/audits/fase3-audit-v2.md` (pendentes)
- **Manifesto:** Conceitos #3 (Verification First), #6 (Adversarial by Default), #11 (Ground Truth Hierarchy)