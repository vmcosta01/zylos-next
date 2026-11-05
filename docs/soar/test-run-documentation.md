# SOAR: Test-Run (Dry-Run) de Runbooks

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Documentação técnica do comando `zylos soar test-run` para validação automatizada de runbooks sem efeitos colaterais
**Público-alvo:** Devs pleno/senior, SREs
**Gatilho:** Após modificar um runbook YAML, antes de deploy em produção, ou como parte do CI/CD
**Gap resolvido:** §4.2 da análise `GAPS_ANALYSIS_RUNBOOKS_PLAYBOOKS.md`

---

## 1. Visão Geral

O comando `zylos soar test-run` executa um runbook YAML em **modo dry-run**: valida pré-condições, simula cada passo sem executar comandos destrutivos, verifica outputs esperados contra mocks, e reporta se o runbook está estruturalmente correto e executável.

### Por que importa

Conforme Cutover e Belago, runbooks desatualizados são passivos operacionais. O teste automatizado garante que:
- Comandos ainda existem no PATH após mudanças de infra
- Outputs esperados ainda correspondem à realidade do sistema
- Steps de rollback são válidos e executáveis
- Pré-condições detectam configurações quebradas antes do incidente real

---

## 2. Uso

```bash
# Dry-run básico (sem efeitos colaterais)
zylos soar test-run ./docs/soar/runbooks/zylos-daemon.runbook.yaml

# Dry-run com contexto de incidente simulado
zylos soar test-run ./docs/soar/runbooks/deploy-cicd.runbook.yaml \
  --context '{"trigger": "pr_merged_to_main", "sha": "abc1234"}'

# Dry-run verboso (mostra cada step simulado)
zylos soar test-run ./docs/soar/runbooks/monitoring-observability.runbook.yaml --verbose

# Validar todos os runbooks de um diretório
zylos soar test-run ./docs/soar/runbooks/ --all

# Integrar com CI (exit code não-zero se falhar)
zylos soar test-run ./docs/soar/runbooks/ --all --ci
```

---

## 3. O Que é Validado

| Validação | Descrição | Falha = |
|-----------|-----------|---------|
| **Schema compliance** | YAML conforms to `runbook-schema.yaml` | Erro de parse |
| **Preconditions executable** | Cada `command` em preconditions existe e roda | Warning ou Error |
| **Steps reachable** | Grafo de steps é conexo; nenhum step órfão ou inalcançável | Error |
| **Commands exist** | Binários referenciados estão no PATH | Warning |
| **Timeouts reasonable** | Nenhum timeout > 1h ou < 1s | Warning |
| **Rollback valid** | Rollback steps são executáveis e não dependem de state criado por steps falhos | Error |
| **Validation checks sound** | Checks finais usam comandos que produzem output comparável | Warning |
| **No destructive ops in dry-run** | Steps marcados como `execute` são simulados; apenas `validate` roda de verdade | Info |

---

## 4. Output

### Modo Normal
```
✅ zylos-daemon.runbook.yaml — PASSED (12/12 checks)
   Preconditions: 3/3 OK
   Steps: 7/7 reachable, 0 orphaned
   Commands: 5/5 found in PATH
   Rollback: 3/3 valid
   Validation: 4/4 sound
```

### Modo Verbose
```
🔍 zylos-daemon.runbook.yaml
  [PRE] ✅ Daemon binary exists → which zylos → /usr/local/bin/zylos
  [PRE] ✅ Config file valid → zylos config validate → exit_code:0
  [PRE] ⚠️ Log directory writable → skipped (non-critical)
  [STEP] check_process → SIMULATED (pgrep -f zylos-daemon)
  [STEP] health_check → SIMULATED (curl ...)
  [STEP] start_daemon → SIMULATED (zylos daemon start)
  [STEP] restart_daemon → SIMULATED (human_approval_required=true)
  [STEP] collect_diagnostics → SIMULATED
  [STEP] escalate → SIMULATED (notify)
  [STEP] validate_final → SIMULATED
  [ROLLBACK] force_kill → VALID (pkill -9 -f zylos-daemon)
  [ROLLBACK] restore_binary_backup → VALID (cp ...)
  [ROLLBACK] clean_socket → VALID (rm -f ...)
  [VALIDATE] Process alive → SOUND (pgrep)
  [VALIDATE] Health OK → SOUND (curl + json match)
  [VALIDATE] CLI list agents → SOUND (zylos agent list)
  [VALIDATE] No panics → SOUND (grep -c)

✅ PASSED — 12/12 checks, 0 errors, 1 warning
```

### Modo CI
```json
{
  "runbook": "zylos-daemon.runbook.yaml",
  "status": "passed",
  "checks_total": 12,
  "checks_passed": 12,
  "warnings": 1,
  "errors": 0,
  "duration_ms": 45
}
```

---

## 5. Integração com CI/CD

Adicionar ao pipeline `deploy-cicd.md` como gate pré-deploy:

```yaml
# .github/workflows/ci.yml (exemplo)
- name: Validate SOAR runbooks
  run: |
    zylos soar test-run ./docs/soar/runbooks/ --all --ci
    if [ $? -ne 0 ]; then
      echo "❌ Runbook validation failed. Fix before merging."
      exit 1
    fi
```

---

## 6. Limitações Conhecidas

- **Não testa lógica condicional complexa:** Dry-run simula paths lineares; branches condicionais baseados em output real não são exercitados.
- **Não substitui tabletop humano:** Cenários de estresse, comunicação entre equipes e tomada de decisão sob pressão requerem simulação humana.
- **Mocks são estáticos:** Outputs simulados são hardcoded; não refletem estado real do sistema.
- **Human approval steps são pulados:** Em dry-run, steps com `human_approval_required: true` são marcados como SIMULATED mas nunca bloqueiam.

---

## 7. Referências Cruzadas

- **Schema:** `docs/soar/runbook-schema.yaml`
- **Runbooks executáveis:** `docs/soar/runbooks/*.runbook.yaml`
- **Gap Analysis:** `docs/GAPS_ANALYSIS_RUNBOOKS_PLAYBOOKS.md` §4.2
- **SOAR Architecture:** `docs/soar/SOAR_ARCHITECTURE.md` §2.4 (Runbook Executor)
- **Deploy CI/CD:** `docs/runbooks/deploy-cicd.md` §3.1 (staging deploy gate)

---

**Próxima Revisão Agendada:** Dezembro de 2026
**Owner:** SRE Zylos-Next