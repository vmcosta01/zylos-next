#!/bin/bash
# =============================================================================
# verificar-gate — Hook de Verificação Obrigatória (Conceito #9)
# =============================================================================
# Uso: ./scripts/verificar-gate.sh <spec-file>
# Bloqueia declaração de "pronto" sem artefato verificável.
# Violação = trabalho inválido conforme Manifesto Conceitos-Desenvolvimento-IA v3.0
# =============================================================================

set -euo pipefail

SPEC_FILE="${1:-}"

if [ -z "$SPEC_FILE" ]; then
    echo "❌ BLOCKED: No spec file provided"
    echo "Usage: $0 <spec-file>"
    exit 1
fi

if [ ! -f "$SPEC_FILE" ]; then
    echo "❌ BLOCKED: Spec file not found: $SPEC_FILE"
    exit 1
fi

# Extrair campo verify.cmd do YAML/spec
if ! grep -q 'verify:' "$SPEC_FILE"; then
    echo "❌ BLOCKED: No verify:<cmd> found in $SPEC_FILE"
    echo "   Todo goal contract DEVE conter um comando verify automatizável."
    echo "   Veja specs/GOAL-CONTRACT.md para exemplos."
    exit 1
fi

CMD=$(grep 'cmd:' "$SPEC_FILE" | head -1 | sed 's/.*cmd: *"\{0,1\}\([^"]*\)"\{0,1\}/\1/' | xargs)
TIMEOUT_MS=$(grep 'timeout_ms:' "$SPEC_FILE" | head -1 | awk '{print $2}' | tr -d '"')
EXPECTED_EXIT=$(grep 'expected_exit_code:' "$SPEC_FILE" | head -1 | awk '{print $2}' | tr -d '"')

# Defaults
TIMEOUT_MS="${TIMEOUT_MS:-30000}"
EXPECTED_EXIT="${EXPECTED_EXIT:-0}"

# Converter ms para segundos (com mínimo de 1s)
TIMEOUT_SECS=$(( (TIMEOUT_MS + 999) / 1000 ))
[ "$TIMEOUT_SECS" -lt 1 ] && TIMEOUT_SECS=1

echo "🔍 verificar-gate: Executing verification command..."
echo "   Spec: $SPEC_FILE"
echo "   Cmd: $CMD"
echo "   Timeout: ${TIMEOUT_SECS}s"
echo "   Expected exit: $EXPECTED_EXIT"
echo ""

# Criar diretório de auditoria se não existir
AUDIT_DIR="evals/audits"
mkdir -p "$AUDIT_DIR"

TASK_ID=$(basename "$SPEC_FILE" .md)
AUDIT_LOG="$AUDIT_DIR/${TASK_ID}-verify-$(date +%Y%m%d-%H%M%S).log"

# Executar com timeout
set +e
timeout "${TIMEOUT_SECS}s" bash -c "$CMD" > "$AUDIT_LOG" 2>&1
ACTUAL_EXIT=$?
set -e

echo "" >> "$AUDIT_LOG"
echo "--- Metadata ---" >> "$AUDIT_LOG"
echo "Spec: $SPEC_FILE" >> "$AUDIT_LOG"
echo "Timestamp: $(date -Iseconds)" >> "$AUDIT_LOG"
echo "Expected exit: $EXPECTED_EXIT" >> "$AUDIT_LOG"
echo "Actual exit: $ACTUAL_EXIT" >> "$AUDIT_LOG"

if [ "$ACTUAL_EXIT" -eq 124 ]; then
    echo "❌ BLOCKED: verify command timed out after ${TIMEOUT_SECS}s"
    echo "   Audit log: $AUDIT_LOG"
    exit 1
fi

if [ "$ACTUAL_EXIT" != "$EXPECTED_EXIT" ]; then
    echo "❌ BLOCKED: verify command failed (exit $ACTUAL_EXIT, expected $EXPECTED_EXIT)"
    echo "   Audit log: $AUDIT_LOG"
    echo ""
    echo "   Last 10 lines of output:"
    tail -10 "$AUDIT_LOG" | sed 's/^/   /'
    exit 1
fi

echo "✅ verificar-gate PASSED"
echo "   Audit log: $AUDIT_LOG"
exit 0