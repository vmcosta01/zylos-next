# PB-E2E-Test-Setup — Configuração e Execução de Testes E2E Browser Headless

**Versão:** 1.0.0
**Data:** 2026-11-12
**Playbook Relacionado:** [`IMPLEMENTATION_PLAN_FASE10.md`](../docs/IMPLEMENTATION_PLAN_FASE10.md) §10.6
**Runbooks Relacionados:** [`rb-cockpit-release.md`](../runbooks/rb-cockpit-release.md), [`pop-wasm-build-troubleshoot.md`](../runbooks/pop-wasm-build-troubleshoot.md)
**SOAR Workflow:** N/A (procedimento manual)
**Público:** Dev Pleno, QA Engineer, SRE
**Tempo Estimado:** 30–60 minutos (setup inicial), 5–10 minutos (execução)

---

## 1. Visão Geral

Este playbook define o procedimento estratégico para configurar e executar testes E2E do cockpit Zylos em browser headless usando `wasm-pack test --headless --chrome`. Cobre instalação de dependências, validação de ambiente, execução de testes e interpretação de resultados.

**Gatilho de ativação:** Gate da Fase 10.6 (`wasm-pack test --headless --chrome`) ou validação pré-release.

**SLA/RTO/RPO:** N/A (ferramenta de qualidade, não serviço crítico).

**Runbooks referenciados:**
- [`rb-cockpit-release.md`](../runbooks/rb-cockpit-release.md) — Pipeline de release completo
- [`pop-wasm-build-troubleshoot.md`](../runbooks/pop-wasm-build-troubleshoot.md) — Diagnóstico de falhas WASM

---

## 2. Nível Senior (Tech Lead / Arquiteto / SRE Senior)

### Matriz RACI

| Atividade | Responsible | Accountable | Consulted | Informed |
|-----------|------------|-------------|-----------|----------|
| Setup ambiente E2E | Dev Pleno | Tech Lead | SRE | PM |
| Execução testes headless | QA Engineer | Tech Lead | Dev Pleno | PM |
| Interpretação resultados | QA Engineer | Tech Lead | Dev Senior | All |
| Decisão gate verde/vermelho | Tech Lead | Tech Lead | QA Engineer | PM |
| Atualização deste playbook | Dev Senior | Tech Lead | QA Engineer | All |

### Critérios de Severidade

| Severidade | Condição | Ação |
|-----------|----------|------|
| SEV-1 | Testes E2E falham em >50% dos casos | Bloquear release, investigar imediatamente |
| SEV-2 | Testes E2E falham em 10–50% dos casos | Investigar antes do próximo deploy |
| SEV-3 | Testes E2E flaky (<10% falha intermitente) | Documentar, criar ticket de fix |
| SEV-4 | Warnings de teste sem falha | Monitorar, priorizar no backlog |

### Gates de Decisão

| Gate | Condição Objetiva | Ação se Falhar |
|------|------------------|----------------|
| G1: Ambiente válido | `chromedriver --version` retorna versão compatível com Chrome instalado | Resolver pré-condição, NÃO prosseguir |
| G2: Build WASM OK | `trunk build` exit code 0 | Verificar `pop-wasm-build-troubleshoot.md` |
| G3: Testes passam | `wasm-pack test --headless --chrome` exit code 0 | Investigar falhas, bloquear release se SEV-1/SEV-2 |
| G4: Cobertura mínima | ≥80% dos componentes têm pelo menos 1 teste E2E | Criar testes faltantes antes do release |

### Trade-offs Documentados

| Decisão | Racional | Alternativa Rejeitada |
|---------|----------|----------------------|
| Chrome headless (não Firefox) | Maior compatibilidade com wasm-bindgen-test, melhor suporte CI | Firefox headless (menos estável com WASM) |
| `wasm-pack test` (não Playwright) | Nativo ao ecossistema Rust/WASM, sem dependência Node.js pesada | Playwright (mais flexível mas overhead maior) |
| Headless por padrão | CI/CD friendly, sem display server necessário | Browser visível (apenas para debug local) |

### Referências

- ADR-006: Migrations SQL individuais (lição aprendida Fase 8)
- Post-mortem: Timeout trunk build (>7min) documentado em HANDOFF_FASE10.md §10.6

---

## 3. Nível Pleno (Dev Pleno / On-call Engineer)

### Fluxograma de Decisão

```
START
 │
 ├─ Chrome instalado? ─── NÃO → Instalar Chrome/Chromium → RETORNAR
 │ SIM
 ├─ wasm-pack instalado? ─── NÃO → cargo install wasm-pack → RETORNAR
 │ SIM
 ├─ chromedriver compatível? ─── NÃO → Atualizar chromedriver → RETORNAR
 │ SIM
 ├─ trunk build OK? ─── NÃO → pop-wasm-build-troubleshoot.md → RETORNAR
 │ SIM
 ├─ wasm-pack test --headless --chrome → EXIT CODE?
 │ 0 → GATE VERDE ✅ → END
 │ ≠0 → Classificar severidade (SEV-1/2/3/4)
 │ SEV-1/2 → BLOQUEAR RELEASE → Investigar → RETORNAR
 │ SEV-3/4 → Documentar → Criar ticket → END
```

### Tabela de Escalação

| Condição | Quem Contatar | Canal | Timeout |
|----------|--------------|-------|---------|
| Chrome/chromedriver incompatível | SRE | Slack #infra | 30min |
| Build WASM falha repetidamente | Dev Senior | Slack #dev | 1h |
| Testes E2E SEV-1 | Tech Lead | PagerDuty | 15min |
| Testes E2E SEV-2 | Dev Senior | Slack #dev | 2h |
| Testes flaky recorrentes | QA Engineer | Jira ticket | 24h |

### Checklist de Comunicação

- [ ] Notificar #dev no Slack antes de iniciar setup (aviso de manutenção)
- [ ] Se SEV-1/SEV-2: notificar Tech Lead via PagerDuty IMEDIATAMENTE
- [ ] Se gate vermelho: atualizar status no Kanban board
- [ ] Após resolução: postar resumo no #dev com link para este playbook

### Runbooks Acionáveis por Fase

| Fase | Runbook | Quando Usar |
|------|---------|-------------|
| Pré-execução | `pop-wasm-build-troubleshoot.md` | Build WASM falha |
| Execução | Este playbook (§4 Júnior) | Setup e execução normal |
| Pós-falha | `rb-cockpit-release.md` §Rollback | Release bloqueado por testes |
| Otimização | `pot-wasm-size-optimization.md` | Bundle muito grande |

### Métricas de Sucesso

| Métrica | Target | Medição |
|---------|--------|---------|
| Pass rate testes E2E | 100% | `wasm-pack test` exit code 0 |
| Tempo execução suite completa | <5min | Timer no CI |
| Flaky rate | <2% | Histórico de 30 dias |
| Cobertura componentes | ≥80% | Contagem manual vs total |

---

## 4. Nível Júnior (Dev Júnior / Estagiário / Agente Automatizado)

### Passo a Passo de Triagem Inicial

#### Passo 1: Verificar Chrome (2 min)
```bash
google-chrome --version || chromium-browser --version
```
**Output esperado:** `Google Chrome 120.x.x.x` ou `Chromium 120.x.x.x`
**Se falhar:** Instalar Chrome: `sudo apt install chromium-browser` (Linux) ou download oficial. Vá para § Rollback se instalação falhar.

#### Passo 2: Verificar wasm-pack (1 min)
```bash
wasm-pack --version
```
**Output esperado:** `wasm-pack 0.13.x`
**Se falhar:** `cargo install wasm-pack`. Aguardar compilação (~2min).

#### Passo 3: Verificar chromedriver (2 min)
```bash
chromedriver --version
```
**Output esperado:** `ChromeDriver 120.x.x.x` (versão DEVE coincidir com Chrome do Passo 1)
**Se falhar:** 
- Linux: `sudo apt install chromium-chromedriver`
- macOS: `brew install chromedriver`
- Windows: `npm install -g chromedriver`
**Se versão incompatível:** Atualizar Chrome OU chromedriver para match.

#### Passo 4: Build WASM (5–35 min)
```bash
cd apps/zylos-cockpit && trunk build
```
**Output esperado:** `Finished` sem erros, `dist/` populado
**Se falhar:** Consultar [`pop-wasm-build-troubleshoot.md`](../runbooks/pop-wasm-build-troubleshoot.md). NÃO prosseguir sem build bem-sucedido.

#### Passo 5: Executar Testes E2E (5–10 min)
```bash
wasm-pack test --headless --chrome
```
**Output esperado:** `test result: ok. X passed; 0 failed`
**Se falhar:** Ler mensagem de erro, classificar severidade (§2 Senior), escalar conforme tabela (§3 Pleno).

### Regras de Quando NÃO Agir e Escalar Imediatamente

- ❌ **NÃO** tente corrigir falhas de teste SEV-1 sozinho → Escalar para Tech Lead via PagerDuty
- ❌ **NÃO** modifique código do cockpit para "fazer teste passar" sem approval → Criar ticket primeiro
- ❌ **NÃO** ignore warnings de compatibilidade Chrome/chromedriver → Versões mismatch causam falhas silenciosas
- ❌ **NÃO** execute testes em branch não-mergeada sem avisar no #dev → Pode conflitar com CI de outros devs

### Glossário

| Termo | Definição |
|-------|-----------|
| `wasm-pack` | Ferramenta Rust para build/test de crates WASM |
| `headless` | Modo browser sem interface gráfica (para CI/CD) |
| `chromedriver` | Driver que permite automação do Chrome via WebDriver protocol |
| `SEV-1/2/3/4` | Classificação de severidade (1=crítico, 4=cosmético) |
| `flaky` | Teste que passa/falha intermitentemente sem mudança de código |
| `gate` | Ponto de verificação obrigatório antes de prosseguir |

---

## 5. Referências Cruzadas

### Runbooks Operacionais
- [`rb-cockpit-release.md`](../runbooks/rb-cockpit-release.md) — Pipeline LOCAL→PREVIEW→QA→PROD
- [`pop-wasm-build-troubleshoot.md`](../runbooks/pop-wasm-build-troubleshoot.md) — Diagnóstico de falhas WASM build

### Dashboards de Monitoramento
- Cockpit tab **Sentinel** — Anomalias em tempo real
- Cockpit tab **Fleet** — Métricas de bots (inclui testes)

### Canais de Comunicação
- Slack: `#dev` (geral), `#infra` (SRE), `#qa` (testes)
- PagerDuty: Tech Lead on-call (SEV-1/SEV-2)
- Jira: Board "Cockpit QA" para tickets de testes flaky

### Revisão
- **Última revisão:** 2026-11-12
- **Próximo review obrigatório:** 2027-05-12 (180 dias) OU após próximo incidente SEV-1/SEV-2 relacionado a testes E2E

---

*Documento gerado via skill `playbook-author` v1.0.0. Para questões, consultar Tech Lead ou skill `testing-checklist-author`.*