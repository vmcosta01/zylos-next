# zylos-sentinel

**Fase:** 7C.2.3 — Padrões Externos (Sentinel Health Scanner)
**Status:** ✅ Maduro (75%+ do plano)

## Descrição

AIOps health scanner com detectors pure functions, normalized signatures, privacy-first output e rate-limited incident reporting. Implementado como standalone crate na Fase 7C com 21 testes passando.

## Tipos Principais

- `Severity` — Healthy/Warning/Critical
- `Observation` — Input imutável para detectors (tool, error_message, timestamp, metadata)
- `DetectorFinding` — Output de detector com signature_input normalizada
- `IncidentReport` — Relatório rate-limited com signature dedup e PII stripped
- `ScanResult` — Resultado agregado de scan periódico

## Implementado

- ✅ Pure-function detectors: http_5xx, timeout, repeated_failures — zero side effects
- ✅ Normalized signatures: SHA-256 truncado (128-bit) de `tool|stripped_error`
- ✅ Privacy-first PII stripping: emails, UUIDs, IPv4, hex tokens ≥16 chars → redacted
- ✅ JSONL append-only logging
- ✅ Token bucket rate limiter: 5 incidents/hour per signature
- ✅ EventBus integration
- ✅ 21 testes passando

## Runbook SOAR

[`cortex-patterns.runbook.yaml`](../../docs/soar/runbooks/cortex-patterns.runbook.yaml) (integrado)

## Fonte

OpenMausBot `server/sentinel/`
