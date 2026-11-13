# POP — WASM Build Troubleshoot (Procedimento Operacional Padrão)

**Versão:** 1.0
**Data:** 2026-11-12
**Playbook Relacionado:** [`pb-e2e-test-setup.md`](../playbooks/pb-e2e-test-setup.md)
**Runbook Relacionado:** `runbooks/rb-cockpit-release.md`
**Público:** Dev Pleno, SRE, Dev Júnior (com supervisão)
**Tempo Estimado:** 15–45 minutos

---

## Objetivo

Diagnosticar e resolver falhas no build WASM do cockpit Zylos (`trunk build` ou `wasm-pack build`), restaurando a capacidade de compilação para target `wasm32-unknown-unknown`.

---

## Pré-Condições

- [ ] Rust toolchain instalada (`rustup show` retorna stable + wasm32-unknown-unknown target)
- [ ] `trunk` instalado (`trunk --version` retorna ≥0.21)
- [ ] Acesso ao repositório `zylos-next` com permissão de escrita em `apps/zylos-cockpit/Cargo.toml`
- [ ] Conexão com internet (para `cargo install` / download de crates)
- [ ] Pelo menos 4GB RAM livre (build WASM release é intensivo)
- [ ] Stakeholders notificados no #dev se build estiver bloqueando release

**Se qualquer pré-condição não for atendida, NÃO iniciar.** Resolver pré-condição primeiro ou escalar para SRE.

---

## Procedimento

### Passo 1: Identificar tipo de erro (3 min)

```bash
cd apps/zylos-cockpit && trunk build 2>&1 | head -50
```

**Output esperado:** Mensagem de erro específica (uuid feature, linking, memory, etc.)
**Se falhar:** Se `trunk` não encontrado, instalar: `cargo install trunk`. Se repositório inacessível, escalar para SRE.

### Passo 2A: Erro UUID feature `js` (5 min)

**Sintoma:** `could not find 'rng' in 'uuid'` ou `specify a source of randomness`

```bash
grep -n "uuid" Cargo.toml
```

**Output esperado:** Linha com `uuid = { version = "1", features = ["v4"] }` (sem `"js"`)

**Correção:**
```bash
sed -i 's/features = \["v4"\]/features = ["v4", "js"]/' Cargo.toml
```

**Validação:**
```bash
grep "uuid" Cargo.toml
```

**Output esperado:** `uuid = { version = "1", features = ["v4", "js"] }`
**Se falhar:** Editar manualmente `Cargo.toml` linha identificada no grep.

### Passo 2B: Erro de linking rust-lld (10 min)

**Sintoma:** `linking with 'rust-lld' failed` ou dependência nativa incompatível

```bash
cargo tree -p zylos-cockpit --target wasm32-unknown-unknown 2>&1 | grep -v wasm | head -20
```

**Output esperado:** Lista de crates não-WASM (sys/native bindings)
**Se output vazio:** Problema não é dependência nativa → ir para Passo 2C.

**Correção:** Identificar crate problemático na lista, verificar se tem feature `wasm` ou `js`:
```bash
grep -r "link =" ../../crates/*/build.rs
```

Adicionar feature WASM-compatível no `Cargo.toml` do crate identificado ou substituir por alternativa WASM-native.

### Passo 2C: Memory allocation error (5 min)

**Sintoma:** `memory allocation failed: out of memory`

```bash
NODE_OPTIONS="--max-old-space-size=8192" trunk build --release
```

**Output esperado:** Build completa sem erro de memória
**Se falhar:** Reduzir otimizações temporariamente: `trunk build --release --no-opt`. Se ainda falhar, escalar para SRE (máquina pode precisar de mais RAM).

### Passo 2D: Timeout no build release (2 min)

**Sintoma:** `timed out after 420.0s` ou similar

**Correção:** Usar profile customizado com LTO thin:
```toml
# Adicionar ao Cargo.toml
[profile.release-lto]
inherits = "release"
lto = "thin"
opt-level = "z"
```

```bash
trunk build --release --profile release-lto
```

**Output esperado:** Build completa dentro do timeout
**Se falhar:** Aumentar timeout do CI/CD para 15min ou executar build localmente sem timeout.

### Passo 3: Validar build pós-correção (5 min)

```bash
trunk build --release && ls -lh dist/*_bg.wasm
```

**Output esperado:** `Finished` sem erros + arquivo `.wasm` listado com tamanho razoável (<20MB uncompressed)
**Se falhar:** Retornar ao Passo 1 com nova mensagem de erro. Se erro persistir após 3 tentativas, escalar para Dev Senior.

---

## Validação Pós-Execução

- [ ] `trunk build --release` exit code 0
- [ ] Arquivo `dist/*_bg.wasm` existe e tem tamanho <20MB uncompressed
- [ ] `gzip -k dist/*_bg.wasm && ls -lh dist/*_bg.wasm.gz` mostra tamanho gzipped <10MB
- [ ] Cockpit carrega no browser: `python3 -m http.server 8080 --directory dist` → abrir `http://localhost:8080`
- [ ] Log de auditoria registrado no TRACKRECORD_FASE10.md (se correção foi aplicada durante Fase 10)

---

## Rollback

**Trigger conditions:** Correção aplicada causou regressão em outros targets (native build falha, testes unitários quebram).

### Passos de Rollback

1. Reverter mudança no `Cargo.toml`:
   ```bash
   git checkout HEAD -- apps/zylos-cockpit/Cargo.toml
   ```
   **Output esperado:** Arquivo restaurado ao estado anterior

2. Verificar build native ainda funciona:
   ```bash
   cargo check --workspace
   ```
   **Output esperado:** `Finished` sem erros

3. Notificar #dev no Slack sobre rollback com link para este POP

### Validação Pós-Rollback

- [ ] `cargo check --workspace` exit code 0
- [ ] Build WASM original reproduz o mesmo erro (confirma que rollback foi efetivo)
- [ ] Ticket criado para investigar causa raiz da incompatibilidade

---

## Quando NÃO Usar Este POP

- ❌ Erro de compilação Rust em target nativo (não WASM) → Use `cargo check --workspace` diretamente, consulte `rust-clippy-compliance` skill
- ❌ Falha em testes E2E (build OK mas testes falham) → Use [`pb-e2e-test-setup.md`](../playbooks/pb-e2e-test-setup.md) §Troubleshooting
- ❌ Problema de deploy/release pipeline → Use [`rb-cockpit-release.md`](./rb-cockpit-release.md)
- ❌ Otimização de tamanho de bundle (build funciona mas é muito grande) → Use [`pot-wasm-size-optimization.md`](../playbooks/pot-wasm-size-optimization.md)
- ❌ Erro em crate específico não relacionado a WASM → Consulte README do crate ou runbook correspondente

---

*Documento gerado via skill `pop-author` v1.0.0. Próxima revisão obrigatória: 2027-05-12 (180 dias) OU após próxima execução com rollback.*