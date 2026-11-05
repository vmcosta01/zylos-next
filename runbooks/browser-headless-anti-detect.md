# Runbook: Browser Headless Anti-Detect (Task 4.2.1)

**Sistema:** SANDBOX_ISOLATION — Fase 4, Sprint 4.2
**Crate:** `zylos-browser`
**Módulo:** `headless`
**Última Atualização:** 30 de outubro de 2026
**Status Operacional:** ✅ 7/7 testes passando

---

## 1. Visão Geral

O módulo `headless` implementa um browser headless anti-detect com fingerprint determinístico no engine, inbox persistente de permissões (allow/ask/deny), bloqueio de trackers e user-agent identificável como bot legítimo (`ZylosBot/`).

### Componentes Principais
- `HeadlessBrowser`: instância principal com lifecycle gerenciado
- `HeadlessConfig`: configuração de seed, permissions policy, tracker blocking
- `PageState`: estado serializável da página atual
- `NavigationResult`: resultado tipado de navegação (success/blocked/error)

---

## 2. Pré-requisitos

```bash
cd C:/Users/Paulo/zylos-next
cargo build -p zylos-browser
```

Nenhuma dependência externa (Chrome/Firefox) é necessária — o browser é simulado internamente para fins de sandbox e compliance testing.

---

## 3. Verificação de Saúde

### 3.1 Testes Unitários
```bash
cargo test -p zylos-browser --lib headless
```
**Esperado:** 7 testes passando em <1s.

### 3.2 Fingerprint Determinístico
```rust
use zylos_browser::{HeadlessBrowser, HeadlessConfig};

let config = HeadlessConfig { seed: 42, ..Default::default() };
let b1 = HeadlessBrowser::new(config.clone());
let b2 = HeadlessBrowser::new(config);
assert_eq!(b1.fingerprint(), b2.fingerprint());
```

### 3.3 User-Agent Contém Identificador
Todo request deve incluir `ZylosBot/` no user-agent para transparência ética.

---

## 4. Procedimentos Operacionais

### 4.1 Criar Instância com Seed Específico
```rust
let config = HeadlessConfig {
    seed: 12345,
    block_trackers: true,
    permission_policy: PermissionPolicy::Ask,
    ..Default::default()
};
let browser = HeadlessBrowser::new(config);
```

### 4.2 Navegar e Verificar Resultado
```rust
let result = browser.navigate("https://example.com").await?;
match result {
    NavigationResult::Success(page) => println!("OK: {}", page.title),
    NavigationResult::Blocked(reason) => eprintln!("Bloqueado: {}", reason),
    NavigationResult::Error(e) => eprintln!("Erro: {}", e),
}
```

### 4.3 Gerenciar Permissões via Inbox
```rust
// Verificar inbox pendente
let pending = browser.permission_inbox().pending();
for entry in pending {
    browser.permission_inbox().decide(entry.id, PermissionDecision::Allow);
}
```

### 4.4 Fechar Página e Liberar Recursos
```rust
browser.close_page()?;
```

---

## 5. Troubleshooting

| Sintoma | Causa Provável | Ação |
|---------|---------------|------|
| Fingerprint diferente entre instâncias | Seed diferente ou config mutada | Verificar `HeadlessConfig.seed` e clonagem |
| Navegação falha com `InvalidScheme` | URL sem `https://` ou `http://` | Adicionar scheme válido |
| Tracker não bloqueado | `block_trackers: false` na config | Habilitar flag ou verificar lista de domínios |
| Permission inbox vazia mas deveria ter entrada | Policy setada como `AutoAllow` | Mudar para `Ask` ou `AutoDeny` para testar fluxo |

---

## 6. Métricas de Referência

- **Criação de instância:** <5ms
- **Navegação simulada:** <10ms
- **Fingerprint hash:** estável entre reinicializações com mesmo seed
- **Memória por instância:** <2MB

---

## 7. Checklist de Validação Pós-Deploy

- [ ] `cargo test -p zylos-browser --lib` passa com 0 warnings
- [ ] Fingerprint consistente com seed fixo
- [ ] User-agent contém `ZylosBot/`
- [ ] Trackers bloqueados quando habilitado
- [ ] Inbox de permissões funcional (allow/ask/deny)
- [ ] Close libera recursos sem leak