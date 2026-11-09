# Runbook: Build & Distribuição de Instaladores Desktop

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Release tag criada ou build manual solicitado
**Público-alvo:** DevOps / Release Engineer
**Pré-requisitos:** Tauri CLI v2+, Rust toolchain, Node.js 20+, platform-specific toolchains

---

## 1. Objetivo

Gerar instaladores nativos cross-platform para o Zylos Gateway Desktop via Tauri bundler.

## 2. Plataformas e Artefatos

| Plataforma | Formato | Ferramenta | Output |
|-----------|---------|-----------|--------|
| Windows | .exe (NSIS) | tauri build --target x86_64-pc-windows-msvc | `src-tauri/target/release/bundle/nsis/*.exe` |
| Windows | .msi (WiX) | tauri build --target x86_64-pc-windows-msvc | `src-tauri/target/release/bundle/msi/*.msi` |
| macOS | .dmg | tauri build --target aarch64-apple-darwin | `src-tauri/target/release/bundle/dmg/*.dmg` |
| Linux | .AppImage | tauri build --target x86_64-unknown-linux-gnu | `src-tauri/target/release/bundle/appimage/*.AppImage` |
| Linux | .deb | tauri build --target x86_64-unknown-linux-gnu | `src-tauri/target/release/bundle/deb/*.deb` |
| Linux | .rpm | tauri build --target x86_64-unknown-linux-gnu | `src-tauri/target/release/bundle/rpm/*.rpm` |

## 3. Procedimento

### 3.1 Preparação
```bash
cd apps/zylos-desktop
npm install
cargo update -p zylos-desktop-app
```

### 3.2 Build
```bash
npx tauri build
```

### 3.3 Verificação
```bash
# Confirmar artefatos gerados
ls -la src-tauri/target/release/bundle/*/
```

## 4. First-Run Wizard

O wizard é acionado automaticamente na primeira execução quando `~/Zylos/.onboarding_complete` não existe.

### IPC Commands
- `is_first_run` → bool
- `complete_onboarding(gateway_url?)` → ActionResult

### Reset para teste
```bash
rm ~/Zylos/.onboarding_complete
```

## 5. Code Signing (Produção)

| Plataforma | Variável de Ambiente | Formato |
|-----------|---------------------|---------|
| Windows | `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PASSWORD` | PFX/Certificate |
| macOS | `APPLE_SIGNING_IDENTITY` + `APPLE_ID` + `APPLE_PASSWORD` | Developer ID |
| Linux | GPG key via `gpg --sign` | Ascii-armored |

## 6. Validação Final

- [ ] Instalador executa sem warnings de segurança do OS
- [ ] First-run wizard aparece na primeira execução
- [ ] App lança após completar wizard
- [ ] Uninstaller remove app + dados (com confirmação)

## 7. Referências

- Playbook: `playbooks/deploy-release.md`
- Config: `apps/zylos-desktop/src-tauri/tauri.conf.json`
- Docs: `docs/IMPLEMENTATION_PLAN.md` §4.1
