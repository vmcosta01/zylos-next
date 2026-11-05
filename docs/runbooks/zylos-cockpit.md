# Runbook: zylos-cockpit — Operação e Troubleshooting

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Gatilho:** Cockpit não carrega, dashboard vazio, streaming falha, tema/i18n quebrado, ou PWA offline não funciona
**Público-alvo:** Devs pleno e junior

---

## 1. Descrição e Contexto

O `zylos-cockpit` é o frontend Leptos WASM do Zylos-Next (ADR-042). Ele roda no browser como CSR, consome a API REST/SSE do `zylos-live-comm`, e exibe dashboards de agentes, tasks, memória, skills e logs. Falhas aqui bloqueiam a visibilidade do sistema mas NÃO afetam execução de agentes (daemon continua operando).

Este runbook cobre diagnóstico, recuperação e validação para problemas comuns no cockpit.

---

## 2. Pré-requisitos

- Browser moderno com suporte a WASM (Chrome 90+, Firefox 89+, Safari 15+)
- Daemon rodando e acessível (`zylos daemon status` → "running")
- Arquivos estáticos servidos (via `trunk serve` em dev ou nginx/caddy em prod)
- Console do browser aberto (F12 → Console) para inspeção de erros WASM
- Acesso ao log do daemon para correlacionar eventos SSE

---

## 3. Diagnóstico Rápido (Primeiros 60 segundos)

Execute na ordem. Pare quando identificar a causa raiz.

```bash
# 1. Verificar se arquivos estáticos são servidos
curl -s http://localhost:8080/ | head -5
# Esperado: HTML com <div id="app"> e script .js
# Se 404/erro: servidor não está rodando → vá para §4.1

# 2. Verificar health endpoint do backend
curl -s http://localhost:9847/health
# Esperado: {"status":"ok","active_streams":N,"version":"X.Y.Z"}
# Se timeout: daemon down → runbook zylos-daemon.md §4.1
# Se erro CORS: configuração de headers → vá para §4.5

# 3. Abrir console do browser e verificar erros WASM
# F12 → Console → procurar por "panic", "unreachable", "wasm"
# Se panic hook disparou: → vá para §4.3
# Se fetch error: → vá para §4.4

# 4. Verificar service worker (PWA)
# F12 → Application → Service Workers
# Se "activated and running": OK
# Se "redundant" ou ausente: → vá para §4.6
```

---

## 4. Procedimentos de Recuperação

### 4.1 Servidor de arquivos estáticos não está rodando

**Ambiente de desenvolvimento:**
```bash
cd ~/zylos-next/apps/zylos-cockpit
trunk serve --port 8080
# Esperado: "Serving at http://127.0.0.1:8080"
```

**Ambiente de produção:**
```bash
# Verificar se nginx/caddy está ativo
systemctl status nginx   # ou caddy
# Se inativo:
systemctl start nginx
# Verificar config
nginx -t
```

**Validação:** `curl http://localhost:8080/` retorna HTML válido.

### 4.2 Dashboard vazio ou Loading infinito

```bash
# Testar endpoint de dados diretamente
curl -s http://localhost:9847/api/v1/agents | jq '.[0]'
# Se JSON válido: problema é no frontend → limpar cache do browser
# Se vazio: nenhum agente registrado → criar agente via CLI
zylos agent create --name test-agent --skill echo

# Se endpoint retorna erro: verificar logs do daemon
tail -30 ~/.local/share/zylos/logs/daemon.log | grep -i "cockpit\|api\|error"
```

**Validação:** Dashboard exibe pelo menos um agente após refresh.

### 4.3 Panic WASM no console do browser

```text
# Exemplo de erro no console:
panicked at 'called `Option::unwrap()` on a `None` value', src/app.rs:142
```

1. Copiar stack trace completo do console
2. Identificar arquivo e linha (ex: `src/app.rs:142`)
3. Abrir código-fonte correspondente no repositório
4. Criar issue com tag `cockpit-bug` + stack trace + passos de reprodução
5. Workaround: hard refresh (Ctrl+Shift+R) ou limpar localStorage

**Nota:** Panics WASM são bugs de código. Não há recovery em runtime; requer fix e rebuild.

### 4.4 Fetch/SSE errors no console

```bash
# Verificar se daemon expõe CORS headers corretos
curl -v http://localhost:9847/health 2>&1 | grep -i "access-control"
# Esperado: Access-Control-Allow-Origin: * (ou domínio específico)

# Se ausente: adicionar no config do gateway
# [cors]
# allowed_origins = ["http://localhost:8080"]

# Para SSE especificamente:
curl -N http://localhost:9847/api/v1/stream/events
# Esperado: stream contínuo de eventos AG-UI
# Se desconecta: verificar timeout do proxy/reverse proxy
```

**Validação:** Console sem fetch errors; SSE stream ativo visível na aba Network.

### 4.5 Tema ou i18n não aplica

```bash
# Limpar localStorage (tema e locale são persistidos lá)
# F12 → Application → Local Storage → localhost:8080 → Clear All
# Ou via console JS:
# localStorage.clear(); location.reload();

# Verificar se i18n files estão sendo servidos
curl -s http://localhost:8080/i18n/pt-BR.json | jq '.tab.agents'
# Esperado: string traduzida
# Se 404: arquivos de tradução não copiados para dist/
```

**Validação:** Após reload, tema e idioma aplicam corretamente.

### 4.6 PWA offline não funciona

```bash
# Verificar se service worker está registrado
# F12 → Application → Service Workers
# Se ausente: trunk não gerou sw.js corretamente

# Rebuild PWA assets
cd ~/zylos-next/apps/zylos-cockpit
trunk build --release
# Verificar que dist/sw.js e dist/manifest.json existem
ls -la dist/sw.js dist/manifest.json

# Forçar atualização do SW
# F12 → Application → Service Workers → Unregister → Reload
```

**Validação:** Após desligar servidor, cockpit ainda carrega (cache offline).

---

## 5. Caminhos Alternativos e Contingência

| Cenário | Ação Alternativa |
|---------|-----------------|
| Cockpit completamente inoperante | Usar CLI diretamente (`zylos agent list`, `zylos skill eval`) |
| WASM não carrega em browser antigo | Atualizar browser ou usar fallback HTML (se implementado) |
| SSE desconecta frequentemente | Aumentar `keepalive_interval` no config do gateway |
| Build WASM falha | Limpar cache: `cargo clean && trunk build` |
| Performance ruim (>3s load) | Habilitar `opt-level = "z"` + LTO no Cargo.toml release profile |

---

## 6. Critério de Validação Final

Após qualquer procedimento de recuperação, TODOS devem passar:

- [ ] `curl http://localhost:8080/` retorna HTML com `<div id="app">`
- [ ] Console do browser sem panics ou fetch errors
- [ ] Dashboard exibe dados reais (não Loading/Empty/Error)
- [ ] Aba Agents lista pelo menos um agente
- [ ] Theme toggle funciona (light ↔ dark)
- [ ] Locale switch funciona (pt-BR ↔ en-US)
- [ ] SSE stream ativo na aba Network (EventStream type)

Se algum falhar após 2 tentativas: escalar com screenshot do console + network tab.

---

## 7. Protocolo Pós-execução

1. Registrar incidente em `~/.local/share/zylos/logs/ops.log`
2. Se bug novo: criar issue com tag `cockpit-bug` + screenshot + console log
3. Se configuração: atualizar este runbook e `Trunk.toml.example`
4. Testar PWA offline mensalmente: desligar servidor e validar cache
5. Medir LCP/FCP trimestralmente com Lighthouse; target <2s

---

## 8. Referências Cruzadas

- **Runbook relacionado:** `zylos-daemon.md` (backend do cockpit)
- **Runbook relacionado:** `fase6-live-communication.md` (SSE/AG-UI details)
- **Arquitetura:** `docs/architecture/adr-042-leptos-wasm.md`
- **Design System:** `DESIGN_SYSTEM.md` (cores, spacing, componentes)
- **APP_FLOW:** `APP_FLOW.md` (6 estados obrigatórios do dashboard)