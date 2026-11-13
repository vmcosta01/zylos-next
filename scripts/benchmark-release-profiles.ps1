# Benchmark das 4 opções de otimização do build release (ADR-011)
# Executar em PowerShell: cd C:\Users\Paulo\zylos-next && .\scripts\benchmark-release-profiles.ps1
# NOTA: Profiles DEVEM estar no workspace root Cargo.toml, não no crate Cargo.toml

$ErrorActionPreference = "Continue"

$ROOT_DIR = "C:\Users\Paulo\zylos-next"
$COCKPIT_DIR = "$ROOT_DIR\apps\zylos-cockpit"
$RESULTS_FILE = "$ROOT_DIR\docs\BENCHMARK_RELEASE_PROFILES.md"
$WORKSPACE_TOML = "$ROOT_DIR\Cargo.toml"

# Backup do workspace Cargo.toml original
Copy-Item $WORKSPACE_TOML "$WORKSPACE_TOML.bak" -Force

function Run-Benchmark {
    param(
        [string]$Name,
        [string]$ProfileBlock
    )

    Write-Host "=== Benchmarking: $Name ===" -ForegroundColor Cyan

    # Substituir [profile.release] no workspace root
    $originalContent = Get-Content "$WORKSPACE_TOML.bak" -Raw
    if ($ProfileBlock) {
        # Remover profile existente e adicionar novo
        $withoutProfile = $originalContent -replace '(?s)\[profile\.release\].*$', ''
        $newContent = $withoutProfile.TrimEnd() + "`n`n$ProfileBlock`n"
        Set-Content $WORKSPACE_TOML $newContent -NoNewline
    } else {
        # Baseline: usar profile original do backup
        Copy-Item "$WORKSPACE_TOML.bak" $WORKSPACE_TOML -Force
    }

    # Limpar dist anterior
    if (Test-Path "$COCKPIT_DIR\dist") {
        Remove-Item "$COCKPIT_DIR\dist" -Recurse -Force
    }

    # Medir tempo de build
    $startTime = Get-Date
    Set-Location $COCKPIT_DIR

    $buildOutput = & trunk build --release 2>&1 | Out-String
    $endTime = Get-Date
    $elapsed = [math]::Round(($endTime - $startTime).TotalSeconds, 1)

    Write-Host "  Build output (last 5 lines):" -ForegroundColor Gray
    $buildOutput -split "`n" | Select-Object -Last 5 | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }

    # Verificar se WASM foi gerado (não confiar apenas em exit code)
    $wasmFile = Get-ChildItem "$COCKPIT_DIR\dist\*_bg.wasm" -ErrorAction SilentlyContinue | Select-Object -First 1

    if ($wasmFile -and (Test-Path $wasmFile.FullName)) {
        $rawBytes = $wasmFile.Length
        $rawMB = [math]::Round($rawBytes / 1MB, 2)

        # Comprimir com gzip via .NET
        $gzFile = "$($wasmFile.FullName).gz"
        if (Test-Path $gzFile) { Remove-Item $gzFile -Force }
        $inputStream = [System.IO.File]::OpenRead($wasmFile.FullName)
        $outputStream = [System.IO.File]::Create($gzFile)
        $gzipStream = New-Object System.IO.Compression.GzipStream($outputStream, [System.IO.Compression.CompressionMode]::Compress)
        $inputStream.CopyTo($gzipStream)
        $gzipStream.Close()
        $outputStream.Close()
        $inputStream.Close()
        $gzBytes = (Get-Item $gzFile).Length
        $gzMB = [math]::Round($gzBytes / 1MB, 2)
        Remove-Item $gzFile -Force

        # Brotli (se disponível)
        $brMB = "N/A"
        if (Get-Command brotli -ErrorAction SilentlyContinue) {
            $brFile = "$($wasmFile.FullName).br"
            & brotli -k -f $wasmFile.FullName 2>$null
            if (Test-Path $brFile) {
                $brBytes = (Get-Item $brFile).Length
                $brMB = [math]::Round($brBytes / 1MB, 2)
                Remove-Item $brFile -Force
            }
        }

        Write-Host "  OK: Build ${elapsed}s | Raw: ${rawMB}MB | Gzip: ${gzMB}MB | Brotli: ${brMB}MB" -ForegroundColor Green
        return @{ Name=$Name; Time=$elapsed; Raw=$rawMB; Gzip=$gzMB; Brotli=$brMB; Status="OK" }
    } else {
        Write-Host "  FAIL: No WASM produced" -ForegroundColor Red
        return @{ Name=$Name; Time="N/A"; Raw="N/A"; Gzip="N/A"; Brotli="N/A"; Status="FAIL" }
    }
}

# Inicializar relatório
$results = @()

# Opção 1: Thin LTO + opt-level z + strip
$r1 = Run-Benchmark -Name "Thin LTO + z + strip" -ProfileBlock @"
[profile.release]
opt-level = "z"
lto = "thin"
codegen-units = 1
strip = true
"@
$results += $r1

# Opção 2: Fat LTO + opt-level z + strip
$r2 = Run-Benchmark -Name "Fat LTO + z + strip" -ProfileBlock @"
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
"@
$results += $r2

# Opção 3: No LTO + opt-level s + strip
$r3 = Run-Benchmark -Name "No LTO + s" -ProfileBlock @"
[profile.release]
opt-level = "s"
lto = false
codegen-units = 16
strip = true
"@
$results += $r3

# Opção 4: Default baseline (opt-level 3, fat LTO, strip)
$r4 = Run-Benchmark -Name "Default (baseline)" -ProfileBlock ""
$results += $r4

# Restaurar Cargo.toml original
Copy-Item "$WORKSPACE_TOML.bak" $WORKSPACE_TOML -Force
Remove-Item "$WORKSPACE_TOML.bak" -Force
Set-Location $ROOT_DIR

# Gerar relatório markdown
$report = @"
# Benchmark Release Profiles - ADR-011

**Data:** $(Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
**Maquina:** $env:COMPUTERNAME
**Rust:** $(rustc --version)
**Trunk:** $(trunk --version 2>$null)

| Opcao | Build Time (s) | WASM Bruto (MB) | WASM Gzipped (MB) | WASM Brotli (MB) | Status |
|-------|---------------|-----------------|-------------------|------------------|--------|
"@

foreach ($r in $results) {
    $report += "`n| $($r.Name) | $($r.Time) | $($r.Raw) | $($r.Gzip) | $($r.Brotli) | $($r.Status) |"
}

$report += @"

## Notas
- Profiles aplicados no workspace root Cargo.toml (nao no crate)
- Builds executados sequencialmente na mesma maquina
- Cache de cargo mantido entre builds (apenas dist/ limpo)
- Brotli: $(if (Get-Command brotli -ErrorAction SilentlyContinue) { "disponivel" } else { "nao instalado" })

## Recomendacao
*(Preencher apos analise dos resultados)*
"@

$report | Out-File $RESULTS_FILE -Encoding UTF8

Write-Host "`n=== Resultados salvos em: $RESULTS_FILE ===" -ForegroundColor Green
Get-Content $RESULTS_FILE