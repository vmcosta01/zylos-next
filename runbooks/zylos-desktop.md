# Runbook: Zylos Desktop — Tauri App, IPC e Gateway Embed

**Versão:** 1.0
**Data:** 2026-11-07
**Autor:** Hermes Agent
**Gatilho:** App não inicia, gateway embed falha, IPC timeout, tray desaparece, ou onboarding bloqueia
**Público-alvo:** Support / DevOps / End User
**Pré-requisitos:** Zylos Desktop instalado, acesso ao sistema de arquivos do usuário

---

## 1. Arquitetura

O Zylos Desktop é uma aplicação nativa construída com Tauri v2 (Rust backend) e SolidJS (TypeScript frontend):
- **Gateway Embed:** O binário `zylos-gateway` é empacotado junto e iniciado automaticamente como processo filho
- **IPC Bridge:** 10 comandos Tauri (`get_status`, `send_prompt`, `list_skills`, etc.) conectam o frontend ao backend Rust
- **System Tray:** Menu contextual com opções de mostrar/ocultar janela, status do gateway e sair
- **Deep Links:** Protocolo `zylos://` para abrir chats e aprovações HITL diretamente
- **Onboarding:** Wizard de primeira execução que configura gateway URL e cria marker `.onboarding_complete`
- **Config:** `~/Zylos/desktop-config.json` com gateway URL e agents root

## 2. Verificação de Saúde

```bash
# Verificar se o app está rodando (Windows)
tasklist | grep -i zylos-desktop

# Verificar se o gateway embed está acessível
curl -sf http://localhost:3000/gateway/health

# Verificar config
cat ~/Zylos/desktop-config.json | python3 -m json.tool

# Verificar marker de onboarding
test -f ~/Zylos/.onboarding_complete && echo "Onboarding completo" || echo "Onboarding pendente"
```

No app:
- Barra de status mostra "Connected" e contagem de agentes ativos
- System tray visível com ícone do Zylos
- Chat panel responde a prompts

## 3. Troubleshooting

### 3.1 App não inicia / crash imediato
```bash
# Logs do Tauri (Windows)
type %APPDATA%\com.zylos.desktop\logs\*.log

# Logs do Tauri (macOS)
tail -100 ~/Library/Logs/com.zylos.desktop/*.log

# Verificar se porta 3000 está em uso (gateway embed)
netstat -ano | grep :3000

# Reinstalar
# Windows: Settings > Apps > Zylos > Uninstall, depois reinstalar .msi
# macOS: arrastar Zylos.app para Lixeira, depois reinstalar .dmg
```

### 3.2 Gateway embed não inicia
```bash
# Verificar se o binário existe
ls ./resources/zylos-gateway*  # dentro do bundle do app

# Tentar iniciar manualmente
./resources/zylos-gateway --config ~/Zylos/gateway.toml

# Se falhar, verificar logs do gateway
tail -50 ~/Zylos/logs/gateway.log
```

### 3.3 IPC timeout / commands falham
```bash
# Reiniciar o app completamente
taskkill /F /IM zylos-desktop-app.exe  # Windows
pkill -9 zylos-desktop-app             # macOS/Linux

# Limpar cache do Tauri
rm -rf %APPDATA%/com.zylos.desktop/cache  # Windows
rm -rf ~/Library/Caches/com.zylos.desktop  # macOS

# Verificar se o gateway está respondendo
curl -sf http://localhost:3000/gateway/health
```

### 3.4 Onboarding bloqueia / loop infinito
```bash
# Resetar onboarding
rm ~/Zylos/.onboarding_complete

# Resetar config para padrão
echo '{"gateway_url":"http://localhost:3000","agents_root":"~/Zylos/agents"}' > ~/Zylos/desktop-config.json

# Reiniciar o app
```

### 3.5 Deep links não funcionam
```bash
# Windows: verificar registro
reg query HKEY_CLASSES_ROOT\zylos

# macOS: verificar URL scheme
plutil -p /Applications/Zylos.app/Contents/Info.plist | grep -A2 CFBundleURLSchemes

# Testar deep link
start zylos://chat/test-agent  # Windows
open zylos://chat/test-agent   # macOS
```

## 4. Configuração

Arquivo: `~/Zylos/desktop-config.json`

```json
{
  "gateway_url": "http://localhost:3000",
  "agents_root": "~/Zylos/agents"
}
```

Override via env: `ZYLOS_GATEWAY_URL`.

## 5. Validação Final

- [ ] App inicia sem crash
- [ ] Gateway embed responde em `/gateway/health`
- [ ] IPC `get_status` retorna healthy + agent count
- [ ] System tray visível e funcional
- [ ] Deep link `zylos://chat/test` abre chat correto
- [ ] Onboarding completa sem loop

## 6. Referências

- Playbook: `playbooks/deploy-release.md` (para updates de desktop)
- SOAR: `docs/soar/runbooks/zylos-desktop.runbook.yaml`
- Docs: `docs/IMPLEMENTATION_PLAN.md` §Fase 2.1
- Installer: `runbooks/desktop-installer-build.md`