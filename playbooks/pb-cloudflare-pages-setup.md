# PB-Cloudflare-Pages-Setup — Configuração de Deploy Preview e Produção

**Versão:** 1.0.0
**Data:** 2026-11-12
**Playbook Relacionado:** [`IMPLEMENTATION_PLAN_FASE11_MEGA.md`](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.3
**Runbooks Relacionados:** [`rb-cockpit-release.md`](../runbooks/rb-cockpit-release.md)
**Público:** Dev Pleno, SRE, Tech Lead
**Tempo Estimado:** 15–30 minutos (setup inicial)

---

## 1. Visão Geral

Este playbook define o procedimento para configurar o Cloudflare Pages como hosting do cockpit Zylos, habilitando deploy preview automático por PR e deploy em produção via tags git.

**Gatilho de ativação:** Primeira configuração de CI/CD ou troca de conta Cloudflare.

**Pré-requisitos:**
- Conta Cloudflare (free tier suficiente)
- Acesso admin ao repositório GitHub `zylos-next`
- Wrangler CLI instalado localmente (`npm install -g wrangler`)

---

## 2. Procedimento de Setup

### Passo 1: Criar projeto no Cloudflare Pages (Manual)

1. Acesse https://dash.cloudflare.com → Pages → Create a project
2. Selecione "Direct upload" (não conectar ao GitHub diretamente — usamos Actions)
3. Nome do projeto preview: `zylos-cockpit-preview`
4. Nome do projeto produção: `zylos-cockpit`
5. Framework preset: "None" (WASM estático)
6. Build command: deixar vazio (build feito no CI)
7. Output directory: `dist`

### Passo 2: Gerar API Token (Manual)

1. Cloudflare Dashboard → My Profile → API Tokens → Create Token
2. Template: "Edit Cloudflare Workers"
3. Permissions:
 - Account > Cloudflare Pages > Edit
 - Account > Cloudflare D1 > Read (futuro)
4. Account Resources: Include > All accounts
5. Copiar token gerado → salvar em local seguro

### Passo 3: Obter Account ID (Manual)

1. Cloudflare Dashboard → Overview (qualquer domínio)
2. Sidebar direita → Account ID
3. Copiar valor

### Passo 4: Configurar Secrets no GitHub (Manual)

1. GitHub → Repositório `zylos-next` → Settings → Secrets and variables → Actions
2. Adicionar:
 - `CLOUDFLARE_API_TOKEN` = token do Passo 2
 - `CLOUDFLARE_ACCOUNT_ID` = ID do Passo 3
3. Para deploy em produção com approval:
 - Settings → Environments → New environment → `production`
 - Protection rules: Required reviewers (adicionar Tech Lead)

### Passo 5: Validar configuração local (Automatizável)

```bash
wrangler login
wrangler pages project list
# Deve listar: zylos-cockpit-preview, zylos-cockpit
```

---

## 3. Workflows Configurados

| Workflow | Trigger | Projeto Cloudflare | Purpose |
|----------|---------|-------------------|---------|
| `deploy-preview.yml` | PR open/sync/reopen | `zylos-cockpit-preview` | Preview URL por PR |
| `deploy-prod.yml` | Tag `v*` push | `zylos-cockpit` | Deploy produção + GitHub Release |
| `ci.yml` (jobs WASM) | Push/PR main | N/A | Build + size check + E2E |

---

## 4. Rollback

Se deploy em produção causar regressão:

1. Identificar tag anterior estável: `git tag --sort=-version:refname | head -5`
2. Re-deploy manual da tag anterior:
 ```bash
 git checkout v0.X.Y-1
 cd apps/zylos-cockpit && trunk build --release
 wrangler pages deploy dist --project-name=zylos-cockpit --branch=main
 ```
3. Ou reverter tag no GitHub e criar nova tag corrigida

---

## 5. Troubleshooting

| Sintoma | Causa Provável | Solução |
|---------|---------------|---------|
| Deploy falha com 403 | Token sem permissão Pages > Edit | Regenerar token com permissão correta |
| Preview URL não aparece no PR | Secret `CLOUDFLARE_API_TOKEN` não configurado | Verificar Settings → Secrets |
| Build WASM falha no CI | wasm-opt incompatível | Wrapper C# é local-only; CI usa binaryen nativo |
| Size check falha (>5MB) | Regressão de bundle | Investigar dependências novas, considerar code splitting |
| E2E falha no CI mas passa local | Chrome version mismatch | `browser-actions/setup-chrome@v1` usa stable; verificar compatibilidade |

---

## 6. Referências

- [Cloudflare Pages Direct Upload Docs](https://developers.cloudflare.com/pages/platform/direct-upload/)
- [Wrangler CLI Reference](https://developers.cloudflare.com/workers/wrangler/)
- [`ci.yml`](../.github/workflows/ci.yml) — Jobs WASM build + size check + E2E
- [`deploy-preview.yml`](../.github/workflows/deploy-preview.yml) — Preview deploy workflow
- [`deploy-prod.yml`](../.github/workflows/deploy-prod.yml) — Production deploy workflow

---

*Documento gerado conforme skill `playbook-author`. Próxima revisão: após primeiro deploy em produção bem-sucedido.*