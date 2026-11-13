# POP — WASM Source Maps Upload para Sentry

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.4 T11.4.8
**Playbook Relacionado:** [`pb-sentry-integration.md`](../playbooks/pb-sentry-integration.md)
**Público:** Dev Pleno, SRE
**Tempo Estimado:** 10–15 minutos

---

## Objetivo

Gerar e fazer upload de source maps do WASM do cockpit para o Sentry, permitindo stack traces legíveis em erros de produção. Sem source maps, os erros no Sentry mostram apenas endereços de memória (`wasm-function[1234]`), impossibilitando debugging eficiente.

---

## Pré-Condições

- [ ] `trunk build --release` completou com sucesso
- [ ] Sentry CLI instalado (`npm install -g @sentry/cli` ou `cargo install sentry-cli`)
- [ ] Token de autenticação Sentry configurado (`SENTRY_AUTH_TOKEN`)
- [ ] Projeto `zylos-cockpit` criado no Sentry

---

## Procedimento

### Passo 1: Gerar source maps no build WASM (2 min)

O trunk já gera `.wasm.map` quando debug info está disponível. Verificar:

```bash
ls -la apps/zylos-cockpit/dist/*.wasm.map
```

**Se não existir:** Adicionar ao `Trunk.toml`:
```toml
[build]
# Manter debug info para source maps
minify = false
```

E rebuildar: `trunk build --release`

### Passo 2: Validar arquivo .wasm.map (1 min)

```bash
head -c 200 apps/zylos-cockpit/dist/*.wasm.map | python3 -m json.tool
```

**Output esperado:** JSON válido com campos `version`, `sources`, `mappings`.

### Passo 3: Upload via Sentry CLI (3 min)

```bash
sentry-cli sourcemaps upload \
  --org zylos \
  --project zylos-cockpit \
  --release $(git describe --tags --always) \
  apps/zylos-cockpit/dist/
```

**Output esperado:** `Uploaded X source map(s)` sem erros.

**Se falhar com auth error:** Verificar `SENTRY_AUTH_TOKEN`:
```bash
export SENTRY_AUTH_TOKEN="sntrys_..."
```

### Passo 4: Validar upload no dashboard Sentry (2 min)

1. Acesse https://sentry.io → Project `zylos-cockpit` → Releases
2. Verificar release correspondente ao tag/commit
3. Confirmar "Source Maps" count > 0

### Passo 5: Testar stack trace legível (3 min)

1. Provocar erro intencional no cockpit (ex: acessar rota inexistente)
2. Verificar evento no Sentry → Stack Trace deve mostrar nomes de funções Rust, não `wasm-function[N]`

---

## Integração com CI/CD

Adicionar ao workflow `deploy-prod.yml` após build WASM:

```yaml
- name: Upload source maps to Sentry
  env:
    SENTRY_AUTH_TOKEN: ${{ secrets.SENTRY_AUTH_TOKEN }}
  run: |
    npm install -g @sentry/cli
    sentry-cli sourcemaps upload \
      --org zylos \
      --project zylos-cockpit \
      --release ${{ github.ref_name }} \
      apps/zylos-cockpit/dist/
```

---

## Rollback

Se upload corromper source maps existentes:

1. Sentry Dashboard → Releases → Selecionar release → Delete Artifacts
2. Re-upload da versão correta
3. Validar stack trace novamente

---

## Troubleshooting

| Sintoma | Causa Provável | Solução |
|---------|---------------|---------|
| `.wasm.map` não gerado | `strip = true` removeu debug info | Usar `strip = "debuginfo"` ou desativar temporariamente |
| Upload falha com 401 | Token inválido ou expirado | Regenerar token em Sentry → Settings → Auth Tokens |
| Upload falha com 409 | Release já existe com source maps diferentes | Usar `--overwrite` flag ou deletar release anterior |
| Stack trace ainda ilegível | Source map version mismatch | Verificar se release tag corresponde ao build exato |
| Arquivo muito grande (>50MB) | Debug info completa incluída | Usar `wasm-opt --strip-debug` no .wasm final, manter .map separado |

---

## Referências

- [`apps/zylos-cockpit/Trunk.toml`](../apps/zylos-cockpit/Trunk.toml) — Configuração de build WASM
- [`playbooks/pb-sentry-integration.md`](../playbooks/pb-sentry-integration.md) — Setup do Sentry
- [Sentry WASM Source Maps Docs](https://docs.sentry.io/platforms/rust/sourcemaps/)
- [`pop-wasm-build-troubleshoot.md`](./pop-wasm-build-troubleshoot.md) — Diagnóstico de build WASM

---

*Documento gerado conforme skill `pop-author`. Próxima revisão: após primeiro stack trace legível capturado em produção.*