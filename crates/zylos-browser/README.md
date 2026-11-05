# zylos-browser

Anti-detect browser context manager with WebMCP compliance for Zylos agents.

## Architecture

### Modules

- **`error`** — `BrowserError` enum and `Result` type alias for browser operations
- **`fingerprint`** — `BrowserFingerprint` struct for deterministic per-agent fingerprinting
- **`context`** — `BrowserContext`, `BrowserContextConfig`, `PermissionDecision`, `InboxEntry` for per-agent isolation and permission management
- **`webmcp`** — `WebMcpCompliance`, `McpToolRegistration` for WebMCP spec-compliant tool registration
- **`headless`** — `HeadlessBrowser`, `HeadlessConfig`, `PageState`, `NavigationResult` for headless browser automation
- **`webmcp_eval`** — `WebMcpEvaluator`, `ComplianceReport`, `ComplianceCheckResult`, `SecurePolyfill`, `CheckSeverity` for evaluating WebMCP compliance

### Key Exports

```rust
pub use error::{BrowserError, Result};
pub use fingerprint::BrowserFingerprint;
pub use context::{BrowserContext, BrowserContextConfig, PermissionDecision, InboxEntry};
pub use webmcp::{WebMcpCompliance, McpToolRegistration};
pub use headless::{HeadlessBrowser, HeadlessConfig, PageState, NavigationResult};
pub use webmcp_eval::{WebMcpEvaluator, ComplianceReport, ComplianceCheckResult, SecurePolyfill, CheckSeverity};
```

## Usage

```rust
use zylos_browser::{BrowserContext, BrowserContextConfig, HeadlessBrowser, HeadlessConfig};

// Create an isolated browser context for an agent
let config = BrowserContextConfig::default();
let ctx = BrowserContext::new(config);

// Launch a headless browser session
let headless_config = HeadlessConfig::default();
let browser = HeadlessBrowser::new(headless_config).await?;
```

## Dependencies

| Crate | Features | Purpose |
|-------|----------|---------|
| `tokio` | full | Async runtime |
| `serde` | derive | Serialization |
| `serde_json` | — | JSON handling |
| `tracing` | — | Structured logging |
| `uuid` | v4, serde | Unique identifiers |
| `chrono` | serde | Timestamps |
| `thiserror` | — | Error types |
| `sha2` | — | Fingerprint hashing |
| `hex` | — | Hash encoding |
| `rand` | — | Randomization |

## Testing

```bash
cargo test -p zylos-browser
```

Dev dependencies: `tempfile` for temporary directory fixtures in integration tests.

## Related Crates

- None (standalone crate with no internal workspace dependencies)