# zylos-agent-shield

Security layer for zylos-gateway: PII detection, credential masking, outbound content filtering, and comprehensive audit logging. Middleware-style API that scans text payloads, redacts sensitive data, and persists security events to SQLite WAL.

## Quick Start

```rust
use zylos_agent_shield::{AgentShield, ShieldConfig, RedactionStrategy};

// Default config detects email, phone, SSN, credit cards, API keys,
// bearer tokens, passwords, AWS keys, private keys. IP detection off by default.
let config = ShieldConfig::default();

// With persistent audit log
let shield = AgentShield::new(config, Some("shield-audit.db")).await?;

// Without persistence (scan-only mode)
let shield = AgentShield::new(config, None).await?;

// Scan text for PII/credentials
let result = shield.scan("Contact user@example.com", "outbound-email");
assert!(result.has_findings);
println!("{}", result.redacted_text); // "[REDACTED:email]"

// Block outbound content with policy violations
match shield.check_outbound(text, "destination")? {
    Some(scan) => eprintln!("Blocked: {} findings", scan.findings.len()),
    None => { /* allowed */ }
}

// Query audit log
let entries = shield.query_audit(None, Some(Severity::High), 50).await?;
```

## Types

| Type | Description |
|------|-------------|
| `SensitiveType` | Enum: `Email`, `PhoneNumber`, `Ssn`, `CreditCard`, `ApiKey`, `BearerToken`, `Password`, `AwsAccessKey`, `PrivateKey`, `IpAddress`, `Custom` |
| `Finding` | Single detection: byte offsets, matched text (masked), confidence 0.0–1.0 |
| `ScanResult` | Full scan output: findings list, redacted text, `has_findings` flag |
| `ShieldConfig` | Toggle each detector + custom regex patterns + redaction strategy |
| `RedactionStrategy` | Enum: `Mask` (`[REDACTED:<type>]`), `Hash` (SHA-256 prefix), `Remove`, `Fixed(String)` |
| `CustomPattern` | User-defined regex with name and label |
| `AuditEntry` | Security event record with severity, fingerprint, actor, target |
| `AuditEventType` | Enum: `PiiDetected`, `CredentialBlocked`, `OutboundFiltered`, `PolicyViolation`, `ScanCompleted`, `ConfigChanged` |
| `Severity` | Enum: `Info`, `Warning`, `High`, `Critical` |
| `ShieldStats` | Aggregate counts: total events, critical/high events, outbound blocked |
| `AgentShield` | Main scanner + audit logger; cloneable, optionally backed by SQLite WAL |

## Error Variants (`ShieldError`)

| Variant | Cause |
|---------|-------|
| `Database(sqlx::Error)` | SQLite audit DB failure |
| `Regex(regex::Error)` | Custom pattern failed to compile |
| `Serialization(serde_json::Error)` | JSON encode/decode failure |
| `Io(std::io::Error)` | Filesystem error |
| `Internal(String)` | Logic error (e.g. query_audit/stats called without DB) |

## Gaps

- Phone number regex has low confidence (0.7) and high false-positive rate on formatted numbers.
- IP address detection disabled by default due to false positives; no subnet/CIDR awareness.
- `check_outbound` blocks on *any* finding — no severity threshold or allowlist mechanism.
- Audit persistence in `check_outbound` is fire-and-forget (`tokio::spawn`); failures are silently lost.
- No streaming / chunked scanning — entire text must fit in memory.
- Custom patterns always get confidence 0.8; no per-pattern confidence override.
- `row_to_audit_entry` falls back to `ScanCompleted` / `Info` on unrecognized strings — silent data loss on corrupted rows.
- No rate limiting or backpressure on audit writes under high throughput.
- Fingerprint uses SHA-256 of raw text; identical content at different positions produces same fingerprint.

## License

Part of the zylos-gateway project.
