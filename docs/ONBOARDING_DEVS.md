# Onboarding de Desenvolvedores — Setup Local + First PR

**Fase:** 11.6.3 · **Data:** 2026-11-17 · **Versão:** 1.0
**Meta de aceitação:** novo dev faz setup local + abre o first PR em **< 4 horas**.
**Complementa (não substitui):** `playbooks/onboarding-devs.md` (jornada cultural de 4 semanas, papéis de mentor/buddy), `docs/DEVELOPMENT.md` (loop profissional), `docs/CONTRIBUTING.md` (padrões de código), `docs/runbooks/dev-onboarding-offboarding.md`.

> Todos os comandos abaixo foram conferidos contra o repo em 2026-11-17: `Cargo.toml` (workspace), `rust-toolchain.toml`, `apps/zylos-cockpit/Trunk.toml`, `.github/workflows/ci.yml`.

---

## Orçamento de tempo (< 4h)

| Bloco | Tempo | Entrega |
|-------|-------|---------|
| 1. Toolchain | 30 min | `rustc`, `cargo`, alvo `wasm32-unknown-unknown`, `trunk` |
| 2. Build do workspace | 45–60 min (1º build frio) | `cargo check` verde |
| 3. Rodar gateway + cockpit | 20 min | Cockpit em `localhost` falando com o gateway |
| 4. Gates locais do CI | 30 min | fmt + clippy + testes do crate tocado |
| 5. First PR | 60 min | PR aberto com CI rodando |

---

## 1. Toolchain (30 min)

```bash
# Rust: o repo fixa o canal em rust-toolchain.toml
#   channel = "stable"; components = clippy, rustfmt, rust-analyzer; targets = wasm32-unknown-unknown
# Basta ter o rustup instalado: o primeiro `cargo` dentro do repo instala o resto.
rustup show            # dentro do repo: confirma toolchain + target wasm32

# Cockpit (Leptos/WASM)
cargo install trunk    # usado em apps/zylos-cockpit (Trunk.toml) e no job wasm-build do CI
cargo install wasm-pack  # opcional: só para rodar os testes E2E do cockpit localmente
```

- **MSRV:** `rust-version = "1.82"` em `Cargo.toml`; o CI tem o job `MSRV Check (Rust 1.82)`.
- **Linux:** o CI instala `libgtk-3-dev libglib2.0-dev libappindicator3-dev librsvg2-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev protobuf-compiler` (dependências do app desktop/Tauri). Instale-as, ou exclua os crates desktop como o CI faz (ver §2).
- **Windows:** use git-bash; os scripts `scripts/zylos-start.sh` / `scripts/zylos-stop.sh` são escritos para esse ambiente (usam `.exe` e `taskkill`).

## 2. Build do workspace (45–60 min)

```bash
git clone https://github.com/vmcosta01/zylos-next.git && cd zylos-next

# Mesmo comando do job "Check" do CI (exclui o app desktop/Tauri):
cargo check --workspace --all-targets --exclude zylos-desktop --exclude zylos-desktop-app
```

O workspace tem 56 membros (`[workspace].members` em `Cargo.toml`): crates em `crates/`, apps em `apps/` (`zylos-gateway`, `zylos-cockpit`, `zylos-cli`, `zylos-daemon`, `zylos-desktop/src-tauri`) e `tests/zylos-e2e`. Mapa conceitual: `PROJECT_STRUCTURE.md`.

> Dica de tempo: durante o primeiro build, leia `AGENTS.md`, `RULES.md` e `docs/ARCHITECTURE.md`.

## 3. Rodar gateway + cockpit (20 min)

```bash
# Terminal 1 — gateway (default 127.0.0.1:3000; override: ZYLOS_LISTEN; config: ZYLOS_CONFIG=<arquivo.toml>)
cargo run -p zylos-gateway
curl http://127.0.0.1:3000/api/v1/health      # {"status":"ok","service":"zylos-gateway",...}

# Terminal 2 — cockpit (Trunk.toml define porta 3000; use outra para não colidir com o gateway)
cd apps/zylos-cockpit
trunk serve --port 3001
# abrir http://localhost:3001
```

- Rotas `/api/v1/hermes/*` exigem `ZYLOS_HERMES_TOKEN` (Bearer). Sem ele, respondem pedindo para defini-lo (`apps/zylos-gateway/src/hermes_api.rs`).
- O gateway recusa `Origin` que não seja desta máquina (`apps/zylos-gateway/src/local_guard.rs`).
- Atalho (Windows/git-bash, após `cargo build -p zylos-gateway`): `scripts/zylos-start.sh` / `scripts/zylos-stop.sh`. O start lê `.zylos-hermes-token` da raiz — crie esse arquivo localmente (não commitar).
- Sem gateway, o cockpit mostra o banner "Connection lost" — ver `docs/assets/screenshots/00-offline-gateway.png` e `docs/USER_GUIDE.md` §1.

## 4. Gates locais = jobs do CI (30 min)

O PR só fica verde se passar em `.github/workflows/ci.yml`. Rode localmente o equivalente, focando no crate que você alterou (`-p <crate>`) para economizar tempo:

| Job CI | Comando local |
|--------|---------------|
| Rustfmt | `cargo fmt --all -- --check` |
| Clippy | `cargo clippy -p <crate> --all-targets` (CI: `--workspace ... --exclude zylos-desktop --exclude zylos-desktop-app`) |
| Test Suite | `cargo test -p <crate>` (CI: `cargo test --workspace --all-features --exclude zylos-desktop --exclude zylos-desktop-app`) |
| Documentation | `cargo doc -p <crate> --no-deps` (CI usa `RUSTDOCFLAGS=-Dwarnings`) |
| WASM Build (Cockpit) | `cd apps/zylos-cockpit && trunk build --release` |
| WASM size check | o CI falha se o `.wasm` gzipped passar de 5MB |
| E2E (Chrome headless) | `cd apps/zylos-cockpit && wasm-pack test --headless --chrome` (CI define `WASM_BINDGEN_TEST_TIMEOUT=300`) |
| Security Audit | `cargo audit` (não bloqueante no CI) |

Testes E2E do cockpit: `apps/zylos-cockpit/tests/e2e_*.rs`; E2E do workspace: `tests/zylos-e2e`.

## 5. First PR (60 min)

### Escolha da tarefa
Siga a progressão de `playbooks/onboarding-devs.md` §4.2: comece por **fix em docs** ou **teste unitário faltante**. Boas candidatas reais, levantadas nos screenshots do USER_GUIDE:
- Adicionar as chaves i18n faltantes (`tab.goals`, `tab.personalities`, `tab.swarm_dag`, `tab.sentinel`, `tab.evidence_ledger`, `tab.hitl_approval`) em `apps/zylos-cockpit/src/i18n.rs`. Hoje elas aparecem cruas nas tabs.

### Fluxo
```bash
git checkout -b fix/<descricao-curta>        # convenção: feat/, fix/, refactor/, docs/ (docs/DEVELOPMENT.md)
# ... editar ...
cargo fmt --all
cargo clippy -p <crate> --all-targets
cargo test -p <crate>
git commit -s -m "fix(cockpit): adiciona chaves i18n das tabs goals/sentinel/..."   # conventional commits + DCO
git push -u origin fix/<descricao-curta>
gh pr create --fill --base master
```

### O que acontece no PR
- `CI` (`.github/workflows/ci.yml`) roda em `pull_request` para `main`/`master`.
- `Deploy Preview` (`.github/workflows/deploy-preview.yml`) builda o cockpit e publica no Cloudflare Pages (projeto `zylos-cockpit-preview`, branch = nome da sua branch) e comenta a URL no PR. Abra o preview para validar mudanças visuais (sem gateway, o preview mostra o estado offline).
- Review: juiz ≠ autor (`docs/DEVELOPMENT.md`). Merge só com CI verde.

### Checklist do first PR
- [ ] Branch com prefixo correto
- [ ] Commit no padrão `<type>(<scope>): <descrição>`, com `-s`
- [ ] `cargo fmt --all -- --check` limpo
- [ ] Clippy e testes do crate verdes
- [ ] Docs atualizadas se o comportamento mudou (loop READ → … → UPDATE DOCS)
- [ ] Tempo total registrado: ____ h (meta < 4h)

---

## Troubleshooting de setup

| Sintoma | Causa provável | Ação |
|---------|----------------|------|
| `cargo check` falha em `webkit2gtk`/`gtk` (Linux) | Deps do desktop ausentes | Instale os pacotes da §1 ou use `--exclude zylos-desktop --exclude zylos-desktop-app` |
| `protoc` não encontrado | `protobuf-compiler` ausente | Instale-o (o CI instala) |
| `trunk serve` diz que a porta está em uso | `Trunk.toml` usa 3000, a mesma do gateway | `trunk serve --port 3001` |
| Cockpit em "Connection lost" | Gateway parado ou Origin não-local | Suba o gateway; acesse via `localhost` |
| Rotas `/hermes/*` recusadas | Sem `ZYLOS_HERMES_TOKEN` | Exporte a variável antes de subir o gateway |
| Build WASM lento/estoura | Profile release pesado | Ver `runbooks/pop-wasm-build-troubleshoot.md` |
| Teste E2E timeout | Default de 20s do runner | `WASM_BINDGEN_TEST_TIMEOUT=300` |

## Referências
- `playbooks/onboarding-devs.md` — jornada de 4 semanas, papéis, gates de competência
- `docs/DEVELOPMENT.md` — loop profissional e política git
- `docs/CONTRIBUTING.md` — padrões de código e review
- `apps/zylos-cockpit/README.md`, `runbooks/zylos-cockpit.md`, `runbooks/zylos-gateway.md`
- `docs/FAQ.md` — seção "Desenvolvimento"
