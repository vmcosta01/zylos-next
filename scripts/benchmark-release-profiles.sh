#!/bin/bash
# Benchmark das 4 opções de otimização do build release (ADR-011)
# Executar em: cd apps/zylos-cockpit && bash ../../scripts/benchmark-release-profiles.sh

set -e

COCKPIT_DIR="$(cd "$(dirname "$0")/../apps/zylos-cockpit" && pwd)"
RESULTS_FILE="$COCKPIT_DIR/../../docs/BENCHMARK_RELEASE_PROFILES.md"
CARGO_TOML="$COCKPIT_DIR/Cargo.toml"

echo "# Benchmark Release Profiles — ADR-011" > "$RESULTS_FILE"
echo "" >> "$RESULTS_FILE"
echo "**Data:** $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$RESULTS_FILE"
echo "**Máquina:** $(uname -a)" >> "$RESULTS_FILE"
echo "**Rust:** $(rustc --version)" >> "$RESULTS_FILE"
echo "**Trunk:** $(trunk --version 2>/dev/null || echo 'N/A')" >> "$RESULTS_FILE"
echo "" >> "$RESULTS_FILE"
echo "| Opção | Build Time (s) | WASM Bruto (MB) | WASM Gzipped (MB) | WASM Brotli (MB) | Status |" >> "$RESULTS_FILE"
echo "|-------|---------------|-----------------|-------------------|------------------|--------|" >> "$RESULTS_FILE"

cd "$COCKPIT_DIR"

# Backup do Cargo.toml original
cp Cargo.toml Cargo.toml.bak

cleanup() {
    cp Cargo.toml.bak Cargo.toml
    rm -f Cargo.toml.bak
}
trap cleanup EXIT

run_benchmark() {
    local name="$1"
    local profile_config="$2"
    
    echo "=== Benchmarking: $name ==="
    
    # Aplicar config do profile
    if [ -n "$profile_config" ]; then
        echo "" >> Cargo.toml
        echo "$profile_config" >> Cargo.toml
    fi
    
    # Limpar cache anterior
    rm -rf dist/
    
    # Medir tempo de build
    local start_time=$(date +%s%N)
    if trunk build --release 2>&1 | tee /tmp/trunk-build-$name.log; then
        local end_time=$(date +%s%N)
        local elapsed=$(( (end_time - start_time) / 1000000000 ))
        
        # Medir tamanhos
        local wasm_file=$(ls dist/*_bg.wasm 2>/dev/null | head -1)
        if [ -n "$wasm_file" ] && [ -f "$wasm_file" ]; then
            local raw_bytes=$(wc -c < "$wasm_file")
            local raw_mb=$(echo "scale=2; $raw_bytes / 1048576" | bc)
            
            gzip -k -f "$wasm_file"
            local gz_bytes=$(wc -c < "${wasm_file}.gz")
            local gz_mb=$(echo "scale=2; $gz_bytes / 1048576" | bc)
            rm -f "${wasm_file}.gz"
            
            # Brotli (se disponível)
            local br_mb="N/A"
            if command -v brotli &>/dev/null; then
                brotli -k -f "$wasm_file"
                local br_bytes=$(wc -c < "${wasm_file}.br")
                br_mb=$(echo "scale=2; $br_bytes / 1048576" | bc)
                rm -f "${wasm_file}.br"
            fi
            
            echo "| $name | ${elapsed}s | ${raw_mb} | ${gz_mb} | ${br_mb} | ✅ OK |" >> "$RESULTS_FILE"
            echo "  → Build: ${elapsed}s | Raw: ${raw_mb}MB | Gzip: ${gz_mb}MB | Brotli: ${br_mb}MB"
        else
            echo "| $name | ${elapsed}s | N/A | N/A | N/A | ❌ No WASM |" >> "$RESULTS_FILE"
            echo "  → Build completou mas nenhum WASM encontrado"
        fi
    else
        echo "| $name | N/A | N/A | N/A | N/A | ❌ Build Failed |" >> "$RESULTS_FILE"
        echo "  → Build falhou (ver /tmp/trunk-build-$name.log)"
    fi
    
    # Restaurar Cargo.toml
    cp Cargo.toml.bak Cargo.toml
}

# Opção 1: Thin LTO + opt-level z + strip
run_benchmark "Thin LTO + z + strip" '[profile.release]
opt-level = "z"
lto = "thin"
strip = true'

# Opção 2: Fat LTO + opt-level z + strip
run_benchmark "Fat LTO + z + strip" '[profile.release]
opt-level = "z"
lto = true
strip = true'

# Opção 3: No LTO + opt-level s
run_benchmark "No LTO + s" '[profile.release]
opt-level = "s"
lto = false
strip = true'

# Opção 4: Default (baseline atual sem overrides)
run_benchmark "Default (baseline)" ""

echo "" >> "$RESULTS_FILE"
echo "## Notas" >> "$RESULTS_FILE"
echo "- Builds executados sequencialmente na mesma máquina" >> "$RESULTS_FILE"
echo "- Cache de cargo limpo entre cada opção" >> "$RESULTS_FILE"
echo "- Brotli: $(command -v brotli &>/dev/null && echo 'disponível' || echo 'não instalado, valores N/A')" >> "$RESULTS_FILE"
echo "" >> "$RESULTS_FILE"
echo "## Recomendação" >> "$RESULTS_FILE"
echo "" >> "$RESULTS_FILE"
echo "*(Preencher após análise dos resultados)*" >> "$RESULTS_FILE"

echo ""
echo "=== Resultados salvos em: $RESULTS_FILE ==="
cat "$RESULTS_FILE"