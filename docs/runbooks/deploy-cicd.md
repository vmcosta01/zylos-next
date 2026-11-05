# Runbook: Deploy & CI/CD — Pipeline de Build, Test e Release

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Guia operacional completo para build, teste, deploy e rollback do Zylos-Next via CI/CD
**Público-alvo:** Dev Pleno, Dev Junior, SRE, Tech Lead
**Gatilho:** Novo PR merged, release tag criada, deploy falhou, necessidade de rollback, configuração de pipeline alterada
**Playbook relacionado:** `stakeholder-communication.md` §6.4 (Changelog template)

---

## 1. Visão Geral

O pipeline CI/CD do Zylos-Next cobre:

- **Build:** Compilação Rust (CLI + daemon) e WASM (cockpit) em Linux/macOS/Windows
- **Test:** Unit tests, integration tests, E2E tests, fuzz testing, security scan
- **Release:** Publicação no GitHub Releases, crates.io, npm (cockpit), Docker Hub
- **Deploy:** Staging automático em PR merge, production manual via tag
- **Rollback:** Revert para versão anterior via CLI ou registry

### Arquitetura do Pipeline

```
PR Opened → Lint + Unit Tests + Security Scan
     ↓
PR Merged → Integration Tests + WASM Build + Staging Deploy
     ↓
Tag Created → Full Test Suite + Production Deploy + Changelog Publish
```

---

## 2. Pré-requisitos

| Requisito | Versão Mínima | Verificação |
|-----------|---------------|-------------|
| Rust toolchain | 1.85+ | `rustc --version` |
| wasm-pack | 0.13+ | `wasm-pack --version` |
| Node.js | 20+ | `node --version` |
| Docker | 24+ | `docker --version` |
| Git | 2.40+ | `git --version` |
| GitHub CLI | 2.40+ | `gh --version` |

### Acessos Necessários

- **GitHub:** Write access ao repositório `zylos-next`
- **Docker Hub:** Push access à org `zylosdev`
- **crates.io:** API token em `~/.cargo/credentials`
- **npm:** Auth token em `~/.npmrc`
- **Staging Server:** SSH key ou kubeconfig para cluster de staging
- **Production Server:** Acesso via deployment bot (não direto)

---

## 3. Fluxo de Deploy

### 3.1 Deploy de Staging (Automático)

**Gatilho:** Merge na branch `main`

**Passos executados pelo CI:**
1. Checkout código + setup toolchain
2. `cargo test --all-features` (unit + integration)
3. `cd apps/zylos-cockpit && trunk build --release`
4. `docker build -t zylosdev/zylos-staging:$SHA .`
5. `docker push zylosdev/zylos-staging:$SHA`
6. `kubectl set image deployment/zylos-staging zylos=zylosdev/zylos-staging:$SHA`
7. Health check: `curl https://staging.zylos.dev/health`
8. Notificar Slack: ✅ Staging deploy successful / ❌ Failed

**Validação:**
```bash
curl https://staging.zylos.dev/health
# Esperado: {"status":"ok","version":"X.Y.Z-sha"}
```

### 3.2 Deploy de Production (Manual via Tag)

**Gatilho:** Tag `vX.Y.Z` criada e pushada

**Passos executados pelo CI:**
1. Todos os passos de staging + E2E tests + fuzz testing
2. `cargo publish --dry-run` (validar crate)
3. `npm publish --dry-run` (validar pacote cockpit)
4. Build de artefatos binários (Linux/macOS/Windows)
5. Upload para GitHub Releases
6. Publicar changelog no blog/docs
7. Notificar Slack + Email: Production release vX.Y.Z

**Validação:**
```bash
gh release view vX.Y.Z --json tagName,publishedAt
# Esperado: tag e timestamp presentes
curl https://api.zylos.dev/health
# Esperado: {"status":"ok","version":"X.Y.Z"}
```

### 3.3 Hotfix (Branch de Emergência)

**Gatilho:** Bug crítico em production

**Fluxo:**
```bash
git checkout -b hotfix/X.Y.Z+1 v.X.Y.Z
# Fix + commit + push
gh pr create --base main --head hotfix/X.Y.Z+1 --label hotfix
# Após merge: criar nova tag patch
git tag vX.Y.Z+1
git push origin vX.Y.Z+1
```

---

## 4. Troubleshooting

### 4.1 Build falha com "linker error"

**Causa:** Dependência nativa faltando (OpenSSL, pkg-config).

**Solução:**
```bash
# Ubuntu/Debian
sudo apt install libssl-dev pkg-config

# macOS
brew install openssl pkg-config

# Windows (MSVC)
vcpkg install openssl:x64-windows-static
```

### 4.2 WASM build timeout (>10min)

**Causa:** Cache frio ou dependências pesadas.

**Solução:**
```bash
# Limpar cache corrompido
cargo clean
rm -rf apps/zylos-cockpit/dist/

# Rebuild com cache
trunk build --release
```

Se persistir: verificar memória disponível (WASM link precisa de ~4GB).

### 4.3 Docker push falha com "unauthorized"

**Causa:** Token expirado ou sem permissão.

**Solução:**
```bash
echo "$DOCKER_HUB_TOKEN" | docker login -u "$DOCKER_HUB_USER" --password-stdin
docker push zylosdev/zylos-staging:$SHA
```

### 4.4 Health check falha pós-deploy

**Diagnóstico:**
```bash
# Verificar logs do container
kubectl logs deployment/zylos-staging --tail=100

# Verificar eventos
kubectl get events --sort-by=.lastTimestamp | tail -20

# Testar endpoint internamente
kubectl exec deployment/zylos-staging -- curl localhost:9847/health
```

**Causas comuns:**
- Config map desatualizado → `kubectl apply -f k8s/configmap.yaml`
- Secret faltando → verificar `kubectl get secrets`
- Porta errada → confirmar `containerPort` no deployment

### 4.5 Rollback necessário

```bash
# Via CLI (recomendado)
zylos rollback <skill> <previous-version>

# Via Kubernetes
kubectl rollout undo deployment/zylos-production

# Via GitHub Releases (binário)
gh release download vX.Y.Z-1 --pattern 'zylos-linux-x86_64.tar.gz'
tar xzf zylos-linux-x86_64.tar.gz
cp zylos ~/.local/bin/zylos
zylos daemon restart
```

**Validação pós-rollback:**
```bash
zylos --version  # Deve mostrar versão anterior
curl https://api.zylos.dev/health  # Status ok
```

---

## 5. Caminhos Alternativos e Contingência

| Cenário | Ação Alternativa |
|---------|-----------------|
| CI completamente down | Build local + upload manual para GitHub Releases |
| Docker Hub indisponível | Usar GHCR (`ghcr.io/zylosdev/zylos`) como mirror |
| Staging cluster down | Deploy em VM temporária via SSH |
| crates.io rate limit | Aguardar 1h ou usar proxy registry |
| Tag acidental em branch errada | Deletar tag: `git push origin :refs/tags/vX.Y.Z` |

---

## 6. Critério de Validação Final

Após qualquer deploy ou rollback, TODOS devem passar:

- [ ] `curl https://api.zylos.dev/health` retorna `{"status":"ok"}`
- [ ] `zylos --version` mostra versão esperada
- [ ] `zylos doctor` retorna ✓ em todos os checks
- [ ] Cockpit carrega sem erros no console
- [ ] Métricas de error rate < 0.1% nos últimos 5min
- [ ] Changelog publicado (se release)

Se algum falhar após 2 tentativas: iniciar rollback e escalar para SRE on-call.

---

## 7. Protocolo Pós-execução

1. Registrar deploy em `~/.local/share/zylos/logs/deploy.log`
2. Se foi release: publicar changelog no blog e notificar stakeholders (playbook §6.4)
3. Se foi hotfix: criar post-mortem em <48h
4. Se foi rollback: abrir issue investigando causa raiz
5. Revisar pipeline trimestralmente: remover stages desnecessários, adicionar novos checks

---

## 8. Referências Cruzadas

- **Playbook:** `stakeholder-communication.md` §6.4 (Changelog), §6.5 (Feature Announcement)
- **Runbook relacionado:** `zylos-cli.md` (comandos de deploy/rollback)
- **Runbook relacionado:** `monitoring-observability.md` (métricas pós-deploy)
- **HANDOFF relevante:** `HANDOFF_6.md` (Multi-adapter Export)
- **Configuração CI:** `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- **Kubernetes manifests:** `k8s/production/`, `k8s/staging/`

---

**Próxima Revisão Agendada:** Dezembro de 2026
**Owner:** SRE / Tech Lead Zylos-Next