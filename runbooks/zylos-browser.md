# Runbook — zylos-browser (Browser Headless Anti-Detect + WebMCP)

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Crate:** `crates/zylos-browser`
**Dependências:** `headless_chrome`, `serde`, `tokio`, `sha2`, `rand`
**Fase:** 4 (SANDBOX_ISOLATION) → 7 (END_TO_END_INTEGRATION)

---

## 1. Visão Geral

O `zylos-browser` fornece automação de browser headless com fingerprint determinístico, bloqueio de trackers e compliance WebMCP. É a camada de interação web segura para agentes que precisam navegar sites reais.

**Módulos:**
- `headless.rs` — Gerenciamento de sessão Chrome headless via CDP
- `fingerprint.rs` — Fingerprint determinístico por agente (canvas, WebGL, fonts)
- `context.rs` — BrowserContext isolado com cookies/storage separados
- `webmcp.rs` — Implementação do protocolo WebMCP (tools/list, tools/call)
- `webmcp_eval.rs` — Suite de avaliação de compliance WebMCP (8 checks)
- `error.rs` — Tipos de erro unificados
- `lib.rs` — Re-exports públicos

**Integração:** Usado pelo `zylos-sandbox` como backend de browser e pelo `zylos-live-comm` para scraping seguro.

---

## 2. Nível Senior — Arquitetura e Decisões

### 2.1 Por que fingerprint determinístico?
Browsers headless são detectados por:
- Canvas/WebGL hashes únicos por sessão
- Font enumeration inconsistente
- Navigator properties contraditórias
- Timing anomalies

Nosso fingerprint é **determinístico por agente ID**: mesmo agente sempre gera mesma impressão, permitindo:
- Sessões persistentes reconhecíveis por sites
- Detecção de comprometimento (fingerprint mudou = sandbox violada)
- Auditoria forense (correlacionar ações ao agente)

### 2.2 WebMCP Compliance
Implementa o protocolo WebMCP spec-compliant:
- `tools/list` → enumera capabilities disponíveis
- `tools/call` → executa tool com validação de input
- Sem `eval()`, `Function()`, ou `innerHTML` direto
- Polyfill seguro para browsers sem suporte nativo

Eval suite valida 8 checks com ≥95% pass rate.

### 2.3 Tracker Blocking
Lista de bloqueio embutida cobre:
- Analytics (GA, Mixpanel, Amplitude)
- Ads (DoubleClick, Taboola, Outbrain)
- Social widgets (FB pixel, Twitter button)
- Fingerprinters (FingerprintJS, CreepJS)

Bloqueio via CDP Network.setRequestInterception, não por extensão.

### 2.4 Trade-offs
| Aspecto | Nossa Abordagem | Alternativa Rejeitada |
|---------|----------------|----------------------|
| Driver | headless_chrome (CDP direto) | Selenium/WebDriver (overhead 3x) |
| Fingerprint | Determinístico por seed | Randomizado por sessão (indetectável mas não auditável) |
| Blocking | CDP interception | uBlock Origin extension (não funciona em headless puro) |
| Isolation | BrowserContext separado | Profile directory (mais pesado, filesystem leak risk) |

---

## 3. Nível Pleno — Operação e Integração

### 3.1 Build e testes
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-browser
# 7 testes: fingerprint_deterministic, context_isolation, tracker_blocking, webmcp_tools_list, webmcp_tools_call, webmcp_no_eval, session_lifecycle
```

### 3.2 API pública
```rust
use zylos_browser::{BrowserManager, AgentFingerprint, WebMcpServer};

// Criar manager com fingerprint determinístico
let fp = AgentFingerprint::from_agent_id("agent-42");
let manager = BrowserManager::new(fp)?;

// Navegar com tracker blocking ativo
let page = manager.navigate("https://example.com").await?;

// Extrair conteúdo
let html = page.content().await?;

// WebMCP server para expor tools ao agente
let mcp = WebMcpServer::new(manager.clone());
let tools = mcp.list_tools();
let result = mcp.call_tool("screenshot", params).await?;

// Cleanup
manager.close().await?;
```

### 3.3 Configuração
```rust
BrowserConfig {
    headless: true,
    block_trackers: true,
    viewport: (1920, 1080),
    user_agent: Option<String>,  // override opcional
    timeout_ms: 30_000,
}
```

### 3.4 Checklist operacional
- [ ] Chrome/Chromium instalado e acessível via PATH
- [ ] Fingerprint gerado antes de qualquer navegação
- [ ] Tracker blocking habilitado em produção
- [ ] WebMCP eval suite passando ≥95%
- [ ] Sessions fechadas explicitamente após uso (no leak)
- [ ] Logs de navegação capturados para auditoria

### 3.5 Troubleshooting
| Sintoma | Causa provável | Solução |
|---------|---------------|---------|
| `Chrome not found` | Binário não no PATH | Instalar Chromium ou setar `CHROME_PATH` env |
| `Timeout navigating` | Site lento ou bloqueando headless | Aumentar timeout ou usar proxy |
| `Fingerprint mismatch` | Sandbox violada ou seed corrompida | Investigar comprometimento, regenerar seed |
| `WebMCP eval <95%` | Polyfill desatualizado | Atualizar `webmcp.rs` conforme spec |
| `Memory leak` | Session não fechada | Verificar `drop()` ou `close()` em todos os paths |

---

## 4. Nível Júnior — Execução Guiada

### 4.1 Verificar se compila
```bash
cd C:/Users/Paulo/zylos-next
cargo check -p zylos-browser
```
**Esperado:** `Finished` sem erros.

### 4.2 Rodar testes
```bash
cargo test -p zylos-browser
```
**Esperado:** `test result: ok. 7 passed`.

### 4.3 Verificar Chrome disponível
```bash
google-chrome --version || chromium --version || chrome.exe --version
```
**Esperado:** Versão do Chrome/Chromium. Se falhar, instalar navegador.

### 4.4 Validar fingerprint determinístico
```bash
cargo test -p zylos-browser fingerprint_deterministic -- --nocapture
```
**Esperado:** Teste passa, mostrando que mesmo agent_id gera mesmo hash.

### 4.5 Quando escalar
- Erro de compilação → verificar dependências (`headless_chrome`, `sha2`)
- Teste falha → ler mensagem, comparar com expected no código
- Chrome não encontrado → instalar ou setar `CHROME_PATH`
- WebMCP eval baixo → verificar spec version em `specs/COMMUNICATION-CONTRACT.md`
- Leak de memória → profiler com `cargo flamegraph` ou valgrind

---

## 5. Referências Cruzadas

- **Código fonte:** `crates/zylos-browser/src/`
- **Runbook Fase 4:** `runbooks/fase4-sandbox-isolation.md`
- **Runbook Browser Anti-Detect:** `runbooks/browser-headless-anti-detect.md`
- **Runbook WebMCP Compliance:** `runbooks/webmcp-spec-compliance.md`
- **Spec comunicação:** `specs/COMMUNICATION-CONTRACT.md` §WebMCP
- **Security model:** `specs/SECURITY-MODEL.md` §Browser Boundaries