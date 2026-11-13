# POT-WASM-Size-Optimization — Otimização de Tamanho do Bundle WASM

**Versão:** 1.0.0
**Data:** 2026-11-12
**Fase:** 10.6 (Qualidade e Estabilidade)
**Skill:** `rust-performance-tuning`

---

## Propósito

Reduzir o tamanho do bundle WASM do cockpit Zylos para <5MB gzipped através de otimizações de compilação, tree-shaking e pós-processamento.

## Diagnóstico Inicial

### 1. Medir Tamanho Atual

```bash
cd apps/zylos-cockpit

# Build release
trunk build --release

# Tamanho bruto
ls -lh dist/*_bg.wasm

# Tamanho gzipped (métrica real de deploy)
gzip -k dist/*_bg.wasm
ls -lh dist/*_bg.wasm.gz

# Análise de composição (requer wasm-tools)
cargo install wasm-tools
wasm-tools print dist/*_bg.wasm | wc -l
```

### 2. Identificar Maiores Contribuidores

```bash
# Instalar twiggy (analisador de tamanho WASM)
cargo install twiggy

# Top 20 maiores itens
twiggy top -n 20 dist/*_bg.wasm

# Árvore de dependências por tamanho
twiggy dominators dist/*_bg.wasm

# Caminhos de retenção (por que algo não foi removido)
twiggy paths dist/*_bg.wasm --function-name "nome_da_funcao"
```

## Estratégias de Otimização

### Estratégia 1: Profile Release Otimizado

Adicionar ao `apps/zylos-cockpit/Cargo.toml`:

```toml
[profile.release]
opt-level = "z"          # Otimizar para tamanho (não velocidade)
lto = true               # Link-Time Optimization completo
codegen-units = 1        # Unidade única = melhor otimização
strip = true             # Remover símbolos de debug
panic = "abort"          # Abort em vez de unwind (menor binário)
incremental = false      # Desabilitar incremental (melhor LTO)
debug = false            # Sem debug info
```

**Impacto esperado:** -30% a -50% no tamanho final.

### Estratégia 2: Feature Flags Seletivas

Auditar dependências pesadas e desativar features desnecessárias:

```toml
# Exemplo: serde sem derive macros se possível
serde = { version = "1", default-features = false, features = ["alloc"] }

# Leptos apenas com CSR (sem SSR/hydrate se não usado)
leptos = { version = "0.7", default-features = false, features = ["csr"] }

# web-sys apenas com APIs usadas
web-sys = { version = "0.3", features = [
    "Window", "Document", "Element", "HtmlElement",
    "MouseEvent", "WheelEvent", "KeyboardEvent",
    "WebSocket", "MessageEvent", "CloseEvent",
    "LocalStorage", "Storage",
    "CanvasRenderingContext2d", "HtmlCanvasElement",
    "SvgElement", "SvgsvgElement"
]}
```

**Verificação:**
```bash
# Listar todas as features habilitadas
cargo tree -p zylos-cockpit --edges features --target wasm32-unknown-unknown
```

### Estratégia 3: wasm-opt Pós-Processamento

```bash
# Instalar binaryen (contém wasm-opt)
# Linux: apt install binaryen
# macOS: brew install binaryen
# Windows: choco install binaryen

# Otimização agressiva de tamanho
wasm-opt -Oz --strip-debug --strip-producers \
  dist/*_bg.wasm -o dist/*_bg_opt.wasm

# Comparar tamanhos
ls -lh dist/*_bg.wasm dist/*_bg_opt.wasm

# Substituir original
mv dist/*_bg_opt.wasm dist/*_bg.wasm
```

**Flags úteis:**
- `-Oz`: Otimização máxima de tamanho
- `-Os`: Balanceado tamanho/velocidade
- `--strip-debug`: Remove DWARF debug info
- `--strip-producers`: Remove seção "producers"
- `--vacuum`: Remove código morto adicional
- `--gufa`: Global Unreachable Function Analysis (remove funções inalcançáveis)

### Estratégia 4: Tree-Shaking Manual

Identificar e remover código não utilizado:

```bash
# Funções exportadas mas não chamadas pelo JS
wasm-objdump -x dist/*_bg.wasm | grep "Export"

# Imports não utilizados
wasm-objdump -x dist/*_bg.wasm | grep "Import"

# Seções customizadas removíveis
wasm-objdump -h dist/*_bg.wasm
```

### Estratégia 5: Code Splitting (Lazy Loading)

Para bundles >10MB, considerar divisão em chunks:

```rust
// Usar leptos::lazy para carregar componentes sob demanda
use leptos::lazy::Lazy;

#[component]
pub fn App() -> impl IntoView {
    let heavy_component = Lazy::new(|| {
        // Carregado apenas quando renderizado
        import("./heavy_component.js")
    });
    
    view! {
        <Suspense fallback=|| view! { <Spinner /> }>
            {heavy_component}
        </Suspense>
    }
}
```

**Nota:** Requer configuração de bundler (Trunk suporta via `Trunk.toml`).

## Checklist de Otimização

- [ ] `[profile.release]` configurado com `opt-level = "z"` + `lto = true` + `strip = true`
- [ ] Features desnecessárias desativadas em `Cargo.toml`
- [ ] `wasm-opt -Oz` executado como pós-build step
- [ ] Debug info removida (`strip = true` ou `wasm-opt --strip-debug`)
- [ ] Código morto eliminado (`twiggy paths` analisado)
- [ ] Dependências pesadas auditadas (`cargo tree --edges features`)
- [ ] Tamanho gzipped medido e documentado

## Targets de Tamanho

| Categoria | Target | Status Atual |
|-----------|--------|-------------|
| Bruto (uncompressed) | <15MB | ❓ Medir |
| Gzipped | <5MB | ❓ Medir |
| Brotli | <4MB | ❓ Medir |
| Initial load (critical path) | <2MB | ❓ Medir |

## Automação no CI/CD

Adicionar ao pipeline de release:

```yaml
# .github/workflows/release.yml (exemplo)
- name: Build WASM Release
  run: trunk build --release
  
- name: Optimize with wasm-opt
  run: |
    wasm-opt -Oz --strip-debug dist/*_bg.wasm -o dist/*_bg_opt.wasm
    mv dist/*_bg_opt.wasm dist/*_bg.wasm
    
- name: Check Size Budget
  run: |
    SIZE=$(gzip -c dist/*_bg.wasm | wc -c)
    BUDGET=5242880  # 5MB em bytes
    if [ "$SIZE" -gt "$BUDGET" ]; then
      echo "❌ WASM size $(($SIZE/1024))KB exceeds budget $(($BUDGET/1024))KB"
      exit 1
    fi
    echo "✅ WASM size $(($SIZE/1024))KB within budget"
```

## Cross-References

- [pb-e2e-test-setup.md](./pb-e2e-test-setup.md)
- [pop-wasm-build-troubleshoot.md](../runbooks/pop-wasm-build-troubleshoot.md)
- [rb-cockpit-release.md](../runbooks/rb-cockpit-release.md)
- [IMPLEMENTATION_PLAN_FASE10.md](../docs/IMPLEMENTATION_PLAN_FASE10.md) §10.6.7

---

**Última atualização:** 2026-11-12