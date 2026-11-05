# Runbook: WebMCP Spec Compliance (Task 4.2.2)

**Sistema:** SANDBOX_ISOLATION — Fase 4, Sprint 4.2
**Crate:** `zylos-browser`
**Módulo:** `webmcp_eval`
**Última Atualização:** 30 de outubro de 2026
**Status Operacional:** ✅ 6/6 testes passando (eval suite + polyfill generation)

---

## 1. Visão Geral

O módulo `webmcp_eval` implementa um eval suite automatizado para validação de compliance com a especificação WebMCP, incluindo geração de polyfills seguros (sem `eval()`, `Function()`, `innerHTML`) e verificação de escopo page-lifetime, bloqueio de origens e rejeição cross-origin.

### Componentes Principais
- `WebMcpEvaluator`: orquestrador do eval suite com 8 checks independentes
- `ComplianceReport`: relatório estruturado com score percentual e severidade por check
- `SecurePolyfill`: código JS gerado e validado contra padrões perigosos
- `CheckSeverity`: enumeração Error/Warning/Info para classificação de falhas

---

## 2. Pré-requisitos

```bash
cd C:/Users/Paulo/zylos-next
cargo build -p zylos-browser
```

Nenhuma dependência externa necessária. O eval suite é puramente determinístico.

---

## 3. Verificação de Saúde

### 3.1 Testes Unitários
```bash
cargo test -p zylos-browser --lib webmcp_eval
```
**Esperado:** 6 testes passando em <1s.

### 3.2 Eval Suite Completo
```rust
use zylos_browser::WebMcpEvaluator;

let mut evaluator = WebMcpEvaluator::new();
let report = evaluator.run_eval_suite();
assert!(report.compliant); // score >= 95%
assert_eq!(report.total_checks, 8);
```

### 3.3 Polyfill Seguro
```rust
use zylos_browser::{WebMcpEvaluator, McpToolRegistration};

let evaluator = WebMcpEvaluator::new();
let tools = vec![McpToolRegistration {
    name: "browser.navigate".to_string(),
    origin: "https://app.example.com".to_string(),
    requires_gesture: false,
    allowed_patterns: vec!["*".to_string()],
}];
let polyfill = evaluator.generate_secure_polyfill(&tools).unwrap();
assert!(polyfill.validated);
assert!(!polyfill.code.contains("eval("));
```

---

## 4. Procedimentos Operacionais

### 4.1 Rodar Eval Suite e Interpretar Relatório
```rust
let mut evaluator = WebMcpEvaluator::new();
let report = evaluator.run_eval_suite();

println!("Score: {:.1}%", report.score_percent);
println!("Passed: {}/{}", report.passed, report.total_checks);

for result in &report.results {
    if !result.passed {
        eprintln!("[{:?}] {}: {}", result.severity, result.check_name, result.message);
    }
}
```

### 4.2 Gerar Polyfill para Conjunto de Tools
```rust
let tools = vec![
    McpToolRegistration {
        name: "browser.click".to_string(),
        origin: "https://app.example.com".to_string(),
        requires_gesture: true,
        allowed_patterns: vec!["button.*".to_string()],
    },
];

match evaluator.generate_secure_polyfill(&tools) {
    Ok(polyfill) => {
        println!("Polyfill gerado ({} bytes)", polyfill.code.len());
        println!("Tools registradas: {:?}", polyfill.registered_tools);
    }
    Err(e) => eprintln!("Falha na geração: {}", e),
}
```

### 4.3 Validar Código JS Externo
```rust
let code = "document.querySelector('#btn').addEventListener('click', handler);";
match evaluator.compliance().validate_polyfill(code) {
    Ok(_) => println!("Código seguro"),
    Err(e) => eprintln!("Código rejeitado: {}", e),
}
```

### 4.4 Bloquear Origem Maliciosa
```rust
evaluator.compliance_mut().block_origin("https://evil.example.com");
```

---

## 5. Checks do Eval Suite

| # | Check Name | Severidade | Descrição |
|---|-----------|------------|-----------|
| 1 | `valid_tool_registration` | Error | Registro válido deve ser aceito |
| 2 | `blocked_origin_enforcement` | Error | Origem bloqueada rejeita registro e invocação |
| 3 | `page_lifetime_scope_clearing` | Error | `clear_origin()` remove registros da origem |
| 4 | `cross_origin_invocation_rejection` | Error | Same-origin permitido, cross-origin rejeitado |
| 5 | `polyfill_security_validation` | Error | Código seguro aceito; `eval`/`Function`/`innerHTML` rejeitados |
| 6 | `gesture_requirement_registration` | Warning | Tools com e sem gesture requirement registram corretamente |
| 7 | `duplicate_registration_handling` | Info | Registro duplicado é idempotente |
| 8 | `empty_origin_handling` | Warning | Origem vazia não causa crash |

**Threshold de Compliance:** ≥95% (mínimo 7/8 checks passando).

---

## 6. Troubleshooting

| Sintoma | Causa Provável | Ação |
|---------|---------------|------|
| Score < 95% | Check de segurança falhando | Verificar se `validate_polyfill` está rejeitando padrões corretos |
| Polyfill rejeita tool válida | Nome da tool contém padrão proibido (`eval(`, etc.) | Renomear tool ou ajustar validação |
| Cross-origin check falha | `check_invocation` retornando `true` para origem diferente | Verificar lógica de comparação de origem em `WebMcpCompliance` |
| Page-lifetime clearing não funciona | `clear_origin` não decrementa contador | Verificar implementação de `registration_count()` |
| Duplicate registration rejeitada | Implementação mudou para reject ao invés de idempotent | Atualizar check ou restaurar comportamento idempotente |

---

## 7. Métricas de Referência

- **Eval suite completo:** <5ms
- **Geração de polyfill (10 tools):** <2ms
- **Validação de código JS:** <1ms por chamada
- **Memória do evaluator:** <500KB

---

## 8. Checklist de Validação Pós-Deploy

- [ ] `cargo test -p zylos-browser --lib webmcp_eval` passa com 0 warnings
- [ ] Eval suite retorna score ≥95%
- [ ] Polyfill gerado não contém `eval(`, `Function(`, `innerHTML`, `document.write`
- [ ] Blocked origin rejeita registro e invocação
- [ ] Cross-origin invocation corretamente rejeitada
- [ ] Page-lifetime clearing funcional
- [ ] Tool names com padrões perigosos são rejeitadas na geração