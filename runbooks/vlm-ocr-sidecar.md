# Runbook & Playbook: VLM-OCR Sidecar (SVG Parser + Visual Verification)

**Sistema:** VLM-OCR Sidecar — Task 4.3.1
**Crate:** `zylos-vlm-ocr`
**Módulos:** `svg`, `parser`, `formula_extractor`
**Última Atualização:** 31 de outubro de 2026
**Status Operacional:** ✅ SVG Parser 17/17 testes passando

---

## 🎯 Visão Geral (Todos os Níveis)

O VLM-OCR Sidecar é o módulo de verificação visual do Zylos-Next responsável por extrair texto, fórmulas matemáticas e estrutura semântica de documentos visuais (SVG, screenshots, PDFs renderizados). Ele opera como sidecar isolado que alimenta o Proof Engine da Fase 3 com ground observado de fontes visuais. O parser SVG atual suporta extração de nós de texto, paths, fórmulas LaTeX/MathML e preservação de atributos de estilo para validação adversarial.

---

## 👨‍💻 NÍVEL SENIOR — Arquitetura e Decisões

### Responsabilidades
- Validar que o parser SVG extrai corretamente fórmulas aninhadas em `<text>` com atributos `font-style="italic"` ou classes CSS matemáticas
- Garantir que raw string literals em testes não quebram com caracteres especiais (`#`, `"`)
- Definir contrato de integração entre VLM-OCR output e Ground::Observado no Proof Engine
- Auditar que nenhum dado visual vaza para logs ou resultados serializados sem sanitização
- Validar overhead do parser (<50ms para SVGs <100KB)

### Procedimentos

#### S-VLM.1: Validação de Extração de Fórmulas
```rust
use zylos_vlm_ocr::svg::{parse_svg, SvgFormula};

let svg = r##"<svg><text font-style="italic">E = mc²</text></svg>"##;
let doc = parse_svg(svg)?;

assert_eq!(doc.formulas.len(), 1);
assert_eq!(doc.formulas[0].latex, "E = mc²");
assert_eq!(doc.formulas[0].source_style, Some("italic".to_string()));
```
**Critério Senior:** Fórmulas são extraídas com metadados de estilo preservados. Falsos positivos (texto comum detectado como fórmula) <1%.

#### S-VLM.2: Auditoria de Raw String Safety
```rust
// Testes devem usar r##"..."## quando o conteúdo contém #"
let svg_with_hash = r##"<svg><text>Value is #42</text></svg>"##;
let doc = parse_svg(svg_with_hash)?;
assert_eq!(doc.texts[0].content, "Value is #42");
```
**Critério Senior:** Zero panics em parsing de SVGs com caracteres especiais. Testes cobrem edge cases de escaping.

#### S-VLM.3: Integração com Ground::Observado
```rust
use zylos_verification::ground::{Ground, Observado};
use zylos_vlm_ocr::svg::parse_svg;

let doc = parse_svg(svg_bytes)?;
let ground = Ground::Observado(Observado::new(
    doc.to_evidence(),
    Source::VlmOcrSidecar { svg_hash: sha256(svg_bytes) },
));
// Ground deve ser aceito pelo Proof Engine sem conversão implícita
let proof = Proof::builder().ground(ground).build()?;
assert!(proof.verify()?);
```
**Critério Senior:** Output do VLM-OCR é diretamente consumível como Ground::Observado. Hash do SVG original é preservado para auditoria.

#### S-VLM.4: Benchmark de Throughput
```bash
cd C:/Users/Paulo/zylos-next
cargo bench -p zylos-vlm-ocr -- svg_parse_100kb
```
**Critério Senior:** Parse de SVG <100KB em <50ms. Parse de SVG >1MB em <500ms. Memory peak <2x tamanho do input.

### Decisões Documentadas
| Decisão | Justificativa | Alternativa Rejeitada |
|---------|--------------|----------------------|
| Parser SVG custom em Rust | Controle total sobre extração de fórmulas; sem dependência de binários externos | resvg (focado em render, não extração), usvg (perde metadados de texto) |
| Raw strings `r##"..."##` em testes | Evita escaping frágil de `#` e `"` em SVGs reais | Escaping manual (`\#`, `\"`) — ilegível e propenso a erros |
| font_style como Option<String> | Preserva informação de estilo sem assumir formato específico | Heurística baseada em substring ("italic" in content) — falso positivo alto |
| Sidecar isolado do Proof Engine | Separação de responsabilidades; VLM-OCR pode evoluir independentemente | Acoplamento direto ao verification crate (breaking changes frequentes) |
| Evidence com hash SHA-256 | Integridade verificável sem armazenar SVG completo em proofs | Referência por path (frágil, não portável) |

---

## 👷 NÍVEL PLENO — Implementação e Validação

### Responsabilidades
- Executar testes unitários e validar cobertura de todos os módulos do VLM-OCR
- Configurar cenários de integração VLM-OCR → Proof Engine → Panel Adversarial
- Documentar padrões de adição de novos formatos (PNG OCR, PDF text layer)
- Implementar benchmarks e otimizar hot paths de parsing
- Gerenciar fixtures de teste (SVGs reais de papers, diagrams, formulas)

### Procedimentos

#### P-VLM.1: Rodar Suite Completa de Testes
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-vlm-ocr --lib
```
**Esperado:** 17/17 testes passando. Verificar contagem contra spec da Task 4.3.1.

#### P-VLM.2: Cenário de Integração — SVG → Proof → Panel
```rust
use zylos_vlm_ocr::svg::parse_svg;
use zylos_verification::{Proof, Panel, Ground, Observado};

// 1. Extrair conteúdo visual
let svg = std::fs::read_to_string("fixtures/diagram_with_formula.svg")?;
let doc = parse_svg(&svg)?;

// 2. Criar proof com ground observado
let ground = Ground::Observado(Observado::new(doc.to_evidence(), source));
let proof = Proof::builder()
    .ground(ground)
    .claim("Diagram shows E=mc² derivation")
    .build()?;

// 3. Submeter ao panel adversarial
let panel = Panel::new(judges);
let deliberation = panel.deliberar(proof).await?;
assert!(!deliberation.refutada, "Visual evidence should survive adversarial review");
```

#### P-VLM.3: Adicionar Novo Formato de Input
```rust
// Exemplo: adicionar suporte a PNG via tesseract-ocr
// 1. Criar crates/zylos-vlm-ocr/src/png.rs
// 2. Implementar trait VisualExtractor
// 3. Adicionar testes em tests/png_tests.rs
// 4. Atualizar lib.rs para expor módulo condicionalmente (feature flag)
```
**Critério Pleno:** Novos formatos seguem o mesmo contrato de output (`SvgDocument`-like struct + `to_evidence()` method). Feature flags previnem dependências obrigatórias.

#### P-VLM.4: Benchmark e Otimização
```bash
cargo bench -p zylos-vlm-ocr
cargo flamegraph --bench vlm_ocr --output flame-vlm.svg
```
**Critério Pleno:** Hot paths identificados no flamegraph são otimizados. Allocs desnecessárias removidas. Zero clones de strings grandes.

### Checklist de Validação
- [ ] `cargo test -p zylos-vlm-ocr --lib` passa 17/17
- [ ] Fórmulas extraídas com metadados de estilo preservados
- [ ] Raw strings em testes cobrem edge cases (`#`, `"`, `\n`)
- [ ] Output integra com Ground::Observado sem conversão implícita
- [ ] Parse <50ms para SVGs <100KB
- [ ] Zero panics em inputs malformados (retorna Error, não panic)
- [ ] Clippy zero warnings
- [ ] Fixtures de teste versionados em `tests/fixtures/`

---

## 🌱 NÍVEL JÚNIOR — Execução Guiada

### Responsabilidades
- Rodar testes e reportar resultados
- Seguir exemplos de uso sem modificar
- Reportar erros com output completo
- Verificar pré-requisitos (Rust toolchain)

### Procedimentos Passo-a-Passo

#### J-VLM.1: Verificar Pré-requisitos
```bash
# Rust toolchain atualizado
rustc --version  # deve ser >= 1.75

# Crate compila
cd C:/Users/Paulo/zylos-next
cargo check -p zylos-vlm-ocr
```
✅ Ambos OK → prossiga
❌ Falha → reporte ao Pleno com mensagem de erro

#### J-VLM.2: Compilar o Crate
```bash
cargo build -p zylos-vlm-ocr
```
✅ `Finished` → prossiga
❌ Erro → copie output completo e reporte ao Pleno

#### J-VLM.3: Rodar Testes
```bash
cargo test -p zylos-vlm-ocr --lib 2>&1 | tee test-vlm-ocr.log
```
✅ `test result: ok. 17 passed` → sucesso
❌ Falha → anote nome do teste + mensagem de erro completa

#### J-VLM.4: Verificar Exemplo Básico
Leia `crates/zylos-vlm-ocr/examples/svg_demo.rs` (se existir) e execute:
```bash
cargo run -p zylos-vlm-ocr --example svg_demo
```
Se não existir exemplo, reporte ao Pleno como gap de documentação.

### Glossário para Júniores
| Termo | Significado |
|-------|------------|
| **VLM** | Vision Language Model — modelo que entende imagens e extrai significado |
| **OCR** | Optical Character Recognition — extração de texto de imagens |
| **Sidecar** | Processo auxiliar que roda isolado do sistema principal |
| **SVG** | Scalable Vector Graphics — formato XML para gráficos vetoriais |
| **Raw String** | String literal em Rust (`r#"..."#`) que não interpreta escape sequences |
| **Ground::Observado** | Nível de certeza mais alto: fato visto diretamente por sensor/parser |
| **Evidence** | Dados estruturados que suportam uma claim; inclui hash do input original |
| **Fixture** | Arquivo de teste fixo usado como input conhecido para validação |
| **Feature Flag** | Configuração Cargo que habilita/desabilita funcionalidades opcionais |
| **Flamegraph** | Visualização de perfil de performance mostrando onde o tempo é gasto |

### Quando Escalar
- Teste falha intermitentemente → **Pleno** (possível race condition ou fixture corrompido)
- Parse demora >50ms para SVG pequeno → **Senior** (performance regression)
- Fórmula extraída incorretamente → **Pleno** (bug na heurística de detecção)
- Raw string causa erro de compilação → **Pleno** (escaping incorreto)
- Output não integra com Ground::Observado → **Senior imediatamente** (contrato quebrado)
- Panic em input malformado → **Senior imediatamente** (violação de robustez)

---

## 🔗 Referências Cruzadas
- **Master Plan:** `zylos-next-master-plan.md` Task 4.3.1 (VLM OCR Sidecar)
- **Pré-requisito:** `runbooks/fase3-verification-proof.md` (Ground Enum, Proof Engine)
- **Relacionado:** `runbooks/fase4-sandbox-isolation.md` (isolamento de execução)
- **Auditorias:** `evals/audits/vlm-ocr-audit.md` (pendente)
- **Manifesto:** Conceitos #3 (Verification First), #8 (Deterministic Execution), #11 (Ground Truth Hierarchy)

---

## 📊 Métricas de Qualidade do VLM-OCR

| Métrica | Target | Atual | Status |
|---------|--------|-------|--------|
| Testes unitários | 17 | 17 | ✅ |
| Fórmulas extraídas corretamente | >99% | TBD | ⏳ Eval pendente |
| Parse latency (<100KB) | <50ms | TBD | ⏳ Bench pendente |
| Parse latency (>1MB) | <500ms | TBD | ⏳ Bench pendente |
| Falsos positivos (texto→fórmula) | <1% | TBD | ⏳ Eval pendente |
| Panics em input malformado | 0 | 0 | ✅ |
| Integração Ground::Observado | ✅ | ✅ | ✅ |
| Clippy warnings | 0 | TBD | ⏳ Check pendente |