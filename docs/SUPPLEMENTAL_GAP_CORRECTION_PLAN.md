# Plano Suplementar de Correção de Gaps — Zylos-Next v6.0

**Data:** 2026-11-04
**Autor:** Hermes Agent (default profile)
**Base:** Master Plan v6 (58 semanas) + Gap Analysis Runbooks/Playbooks + Module Documentation Status + Análise REA de crates
**Objetivo:** Detalhar com precisão cirúrgica tudo que NÃO está coberto pelo master plan original, focando em implementação real de código, documentação de módulos e artefatos operacionais faltantes.

---

## 0. Diagnóstico Real do Codebase (2026-11-04)

### 0.1 Crates: Estado Real vs. Documentado

O `MODULE_DOCUMENTATION_STATUS.md` classificou 15 crates como 🔴 "sem docs/code". A análise profunda revelou que **7 desses crates têm implementação real**, mas foram classificados erroneamente porque o tracker só verificou `lib.rs` e não submódulos:

| Crate | Status no Tracker | Status Real | Arquivos .rs | Linhas Totais | Pub Items | Testes | Ação Necessária |
|-------|-------------------|-------------|--------------|---------------|-----------|--------|-----------------|
| zylos-proof | 🔴 Sem docs/code | ✅ Implementado | 4 | 1.884 | 39 | 69 | Adicionar README + module docs |
| zylos-cerebelo | 🔴 Sem docs/code | ✅ Implementado | 1 | 322 | 7 | 12 | Adicionar README + inline docs |
| zylos-learn | 🔴 Sem docs/code | ✅ Implementado | 1 | 306 | 7 | 9 | Adicionar README + inline docs |
| zylos-panel | 🔴 Sem docs/code | ✅ Implementado | 1 | 214 | 10 | 20 | Adicionar README + inline docs |
| zylos-resolver | 🔴 Sem docs/code | ✅ Implementado | 1 | 383 | 15 | 23 | Adicionar README + inline docs |
| zylos-router | 🔴 Sem docs/code | ✅ Implementado | 1 | 368 | 9 | 16 | Adicionar README + inline docs |
| zylos-vlm-ocr | 🔴 Sem docs/code | ⚠️ Stub com estrutura | 5 | 186 | 4 | 2 | Implementar lógica real nos 5 arquivos vazios |
| zylos-browser | 🔴 Sem docs/code | ⚠️ Parcial (18L) | 1 | 18 | ~2 | 0 | Expandir para implementação completa |
| zylos-docker | 🔴 Sem docs/code | ⚠️ Parcial (12L) | 1 | 12 | ~1 | 0 | Expandir para implementação completa |
| zylos-live-comm | 🔴 Sem docs/code | ⚠️ Parcial (40L) | 1 | 40 | ~3 | 0 | Expandir para implementação completa |
| zylos-sandbox | 🔴 Sem docs/code | ⚠️ Parcial (30L) | 1 | 30 | ~2 | 0 | Expandir para implementação completa |
| zylos-search-mcp | 🔴 Sem docs/code | ⚠️ Parcial (12L) | 1 | 12 | ~1 | 0 | Expandir para implementação completa |
| zylos-skills | 🔴 Sem docs/code | ⚠️ Parcial (25L) | 1 | 25 | ~2 | 0 | Expandir para implementação completa |
| zylos-soar | 🔴 Sem docs/code | ⚠️ Parcial (27L) | 1 | 27 | ~2 | 0 | Expandir para implementação completa |
| zylos-core | 🔴 Sem docs/code | ❌ Stub real (1L) | 1 | 1 | 0 | 0 | Criar do zero — é o crate fundamental |

### 0.2 Apps: Estado Real

| App | Status | Detalhes | Ação |
|-----|--------|----------|------|
| zylos-gateway | ✅ Implementado | 126L main.rs, Cargo.toml presente | Adicionar README + docs |
| zylos-cockpit | ⚠️ Parcial | 18L main.rs | Expandir + adicionar README |
| zylos-daemon | ⚠️ Parcial | 5L main.rs, sem Cargo.toml | Criar Cargo.toml + expandir |
| zylos-cli | ❌ Stub | Sem main.rs, sem Cargo.toml | Criar do zero |

### 0.3 Implicação para o Planejamento

O master plan assume que as fases 1-6 constroem crates do zero. Na realidade:
- **7 crates já têm implementação substancial** (proof, cerebelo, learn, panel, resolver, router, vlm-ocr) → precisam de documentação e testes, não de reimplementação
- **7 crates são parciais** (browser, docker, live-comm, sandbox, search-mcp, skills, soar) → precisam de expansão significativa
- **1 crate é stub total** (core) → precisa ser criado do zero e é bloqueante para todos os outros
- **4 apps precisam de trabalho** → gateway tem base, cockpit/daemon/cli precisam de criação/expansão

---

## 1. Fase S0: Correção de Documentação de Crates Existentes (Semana 0-1)

**Duração:** 5 dias úteis
**Dependências:** Nenhuma
**Bloqueia:** Todas as fases subsequentes (documentação correta é pré-requisito para desenvolvimento)

### Task S0.1: Atualizar MODULE_DOCUMENTATION_STATUS.md com dados reais

**Arquivo alvo:** `C:/Users/Paulo/zylos-next/docs/MODULE_DOCUMENTATION_STATUS.md`

**Ações exatas:**
1. Reclassificar zylos-proof, zylos-cerebelo, zylos-learn, zylos-panel, zylos-resolver, zylos-router de 🔴 para 🟢 Completo (têm código real, pub items e testes)
2. Reclassificar zylos-vlm-ocr de 🔴 para 🟡 Parcial (tem estrutura de 5 arquivos mas lógica vazia)
3. Reclassificar zylos-browser, zylos-docker, zylos-live-comm, zylos-sandbox, zylos-search-mcp, zylos-skills, zylos-soar de 🔴 para 🟡 Parcial
4. Manter zylos-core como 🔴 Stub
5. Atualizar resumo estatístico com números corrigidos
6. Adicionar coluna "Arquivos .rs" e "Linhas Totais" à tabela principal

**Critério de conclusão:** Tracker reflete estado real do codebase; nenhum crate classificado como 🔴 que tenha >10 linhas de código real.

### Task S0.2: Criar READMEs para os 7 crates implementados

**Arquivos a criar:**
- `C:/Users/Paulo/zylos-next/crates/zylos-proof/README.md`
- `C:/Users/Paulo/zylos-next/crates/zylos-cerebelo/README.md`
- `C:/Users/Paulo/zylos-next/crates/zylos-learn/README.md`
- `C:/Users/Paulo/zylos-next/crates/zylos-panel/README.md`
- `C:/Users/Paulo/zylos-next/crates/zylos-resolver/README.md`
- `C:/Users/Paulo/zylos-next/crates/zylos-router/README.md`
- `C:/Users/Paulo/zylos-next/crates/zylos-vlm-ocr/README.md`

**Template obrigatório por README:**
```markdown
# zylos-{nome}

{Descrição de 2-3 linhas do propósito do crate, extraída dos //! comments em lib.rs}

## Arquitetura

{Lista de módulos/submódulos com descrição de responsabilidade de cada um}

## Tipos Públicos

{Tabela com todos os pub structs/enums/traits/fns extraídos do código real}

## Uso

{Exemplo mínimo de uso baseado nos testes existentes}

## Dependências

{Lista de deps do Cargo.toml com justificativa de cada uma}

## Testes

{Instruções para rodar testes + cobertura atual}

## Relação com Master Plan

{Referência à fase e task do master plan que este crate atende}
```

**Critério de conclusão:** Todos os 7 READMEs criados com conteúdo extraído do código real (não inventado); `cargo doc --workspace` passa sem warnings.

### Task S0.3: Adicionar inline docs aos crates implementados que estão abaixo do threshold

**Crates alvo:** zylos-cerebelo (0 /// docs), zylos-learn (0), zylos-panel (0), zylos-resolver (0), zylos-router (0)

**Ações exatas por crate:**
1. Ler cada `pub fn`, `pub struct`, `pub enum`, `pub trait` no lib.rs
2. Adicionar `///` doc comment com: descrição, parâmetros, retorno, erros, exemplo mínimo
3. Target: ≥50 inline docs por crate (threshold do doc score 🟢)

**Critério de conclusão:** Cada crate atinge ≥50 `///` comments; `cargo doc --no-deps` gera documentação HTML sem warnings.

### Task S0.4: Criar READMEs para as 4 apps

**Arquivos a criar:**
- `C:/Users/Paulo/zylos-next/apps/zylos-gateway/README.md`
- `C:/Users/Paulo/zylos-next/apps/zylos-cockpit/README.md`
- `C:/Users/Paulo/zylos-next/apps/zylos-daemon/README.md`
- `C:/Users/Paulo/zylos-next/apps/zylos-cli/README.md`

**Conteúdo obrigatório:** Propósito da app, como buildar (`cargo build -p zylos-{app}`), como rodar, variáveis de ambiente, endpoints/comandos disponíveis, relação com crates.

**Critério de conclusão:** 4 READMEs criados; cada app tem instruções de build/run verificadas.

---

## 2. Fase S1: Implementação de zylos-core (Semana 1-2)

**Duração:** 5 dias úteis
**Dependências:** S0 concluída
**Bloqueia:** Expansão de todos os crates parciais (eles dependem de tipos core)

### Contexto

`zylos-core` é o único crate verdadeiramente vazio (1 linha). Ele é a fundação tipológica do sistema — todos os outros crates importam tipos dele. O master plan não tem task explícita para criar zylos-core porque assume que ele existe.

### Task S1.1: Definir tipos fundamentais em zylos-core

**Arquivo alvo:** `C:/Users/Paulo/zylos-next/crates/zylos-core/src/lib.rs`

**Tipos a implementar (extraídos das dependências observadas nos outros crates):**

```rust
// Identificadores tipados
pub type AgentId = uuid::Uuid;
pub type TaskId = uuid::Uuid;
pub type SkillId = String; // slug format: "category/name"
pub type MemoryId = uuid::Uuid;
pub type EvidenceId = uuid::Uuid;

// Enums compartilhados
pub enum AgentStatus { Idle, Running, Blocked, Failed, Completed }
pub enum TaskStatus { Todo, Ready, Running, Blocked, Review, Done, Archived }
pub enum Priority { Low, Normal, High, Critical }
pub enum GroundLevel { Assumed, Observed, Confirmed, Verified, Proven }

// Traits fundamentais
pub trait Serializable: serde::Serialize + for<'de> serde::Deserialize<'de> {}
pub trait Identifiable { fn id(&self) -> &str; }
pub trait Timestamped { fn created_at(&self) -> &chrono::DateTime<chrono::Utc>; }

// Error types
pub enum ZylosError {
    NotFound(String),
    ValidationError(String),
    SerializationError(serde_json::Error),
    DatabaseError(sqlx::Error),
    IoError(std::io::Error),
    AuthError(String),
    Timeout(String),
}

pub type ZylosResult<T> = Result<T, ZylosError>;
```

**Critério de conclusão:** `cargo build -p zylos-core` passa; todos os tipos são re-exportados e documentados; ≥30 testes unitários cobrindo serialização, display, conversões.

### Task S1.2: Migrar crates existentes para usar zylos-core

**Crates alvo:** zylos-proof, zylos-cerebelo, zylos-learn, zylos-panel, zylos-resolver, zylos-router

**Ação exata por crate:**
1. Adicionar `zylos-core = { path = "../zylos-core" }` ao Cargo.toml
2. Substituir definições locais duplicadas (Ground, AgentId, etc.) por `use zylos_core::*`
3. Rodar `cargo test -p zylos-{crate}` para confirmar zero regressões

**Critério de conclusão:** Zero definições duplicadas de tipos core em qualquer crate; workspace compila sem warnings.

---

## 3. Fase S2: Expansão dos 7 Crates Parciais (Semana 2-4)

**Duração:** 10 dias úteis
**Dependências:** S1 concluída (tipos core disponíveis)
**Bloqueia:** Integração entre sistemas

### Task S2.1: zylos-browser — Anti-detect browser context manager

**Estado atual:** 18L com module docs apenas
**Target:** ~400L com implementação real

**Módulos a implementar:**
- `browser_profile.rs` — Criação/isolamento de perfis Chrome/Firefox por agente
- `fingerprint.rs` — Rotação de fingerprint (canvas, WebGL, fonts, timezone)
- `proxy_pool.rs` — Pool de proxies rotativos com health check
- `webmcp_bridge.rs` — Integração com WebMCP protocol para tool use via browser
- `session_manager.rs` — Lifecycle de sessões (create, pause, resume, destroy)

**Dependências:** headless_chrome ou chromiumoxide, reqwest, serde_json
**Testes mínimos:** 20 (unit + integration com mock server)
**Critério de conclusão:** `cargo test -p zylos-browser` passa; README documenta API pública.

### Task S2.2: zylos-docker — Sandbox isolation via containers

**Estado atual:** 12L com module docs apenas
**Target:** ~350L

**Módulos a implementar:**
- `container_lifecycle.rs` — Create/start/stop/remove com labels por agente
- `filesystem_isolation.rs` — Mounts read-only, tmpfs, network policies
- `resource_limits.rs` — CPU/memory/pid limits via cgroups v2
- `image_registry.rs` — Pull/cache de imagens base seguras
- `health_monitor.rs` — Watchdog de containers órfãos/zumbis

**Dependências:** bollard (Docker API async), tokio
**Testes mínimos:** 15 (requer Docker daemon disponível em CI)
**Critério de conclusão:** Container sandbox funcional com isolamento verificável.

### Task S2.3: zylos-live-comm — AG-UI streaming e EventBus

**Estado atual:** 40L com module docs
**Target:** ~600L

**Módulos a implementar:**
- `event_bus.rs` — Pub/sub in-process com backpressure e ordering guarantees
- `agui_protocol.rs` — Serialização/deserialização AG-UI messages (text, tool_call, state_delta)
- `sse_transport.rs` — Server-Sent Events transport layer com reconnect
- `cockpit_api.rs` — REST endpoints para cockpit frontend
- `hermes_bridge.rs` — Bridge bidirecional com Hermes messaging

**Dependências:** axum, tokio-stream, serde, uuid
**Testes mínimos:** 30 (event ordering, reconnect, backpressure)
**Critério de conclusão:** Streaming end-to-end funcional entre daemon e cockpit.

### Task S2.4: zylos-sandbox — WASM snapshot/resume + OCR sidecar

**Estado atual:** 30L com module docs
**Target:** ~500L

**Módulos a implementar:**
- `wasm_runtime.rs` — Wasmtime/Wasmer runtime com snapshot/restore
- `snapshot_store.rs` — Persistência de snapshots em disco com versioning
- `ocr_sidecar.rs` — Processo sidecar para VLM-OCR com IPC via Unix socket
- `capability_gates.rs` — Whitelist de syscalls/imports permitidos
- `audit_log.rs` — Log imutável de todas as operações do sandbox

**Dependências:** wasmtime, tokio, serde
**Testes mínimos:** 25 (snapshot roundtrip, capability enforcement)
**Critério de conclusão:** WASM agent executa, pausa e restaura estado corretamente.

### Task S2.5: zylos-search-mcp — Parallel search fan-out

**Estado atual:** 12L com module docs
**Target:** ~300L

**Módulos a implementar:**
- `fan_out.rs` — Dispatch paralelo para N providers com timeout individual
- `result_merger.rs` — Dedup, scoring unificado, ranking
- `provider_registry.rs` — Plugin system para search providers (web, code, docs)
- `cache_layer.rs` — Cache LRU de resultados com TTL
- `mcp_server.rs` — MCP protocol server implementation

**Dependências:** reqwest, mcp-sdk, dashmap
**Testes mínimos:** 20 (dedup correctness, timeout handling, cache eviction)
**Critério de conclusão:** MCP server funcional com ≥2 providers integrados.

### Task S2.6: zylos-skills — SKILLS_REGISTRY soberano

**Estado atual:** 25L com module docs
**Target:** ~450L

**Módulos a implementar:**
- `registry.rs` — Index.json loading, validation, hot-reload via filesystem watch
- `skill_loader.rs` — Dynamic loading de skills com sandboxing
- `dependency_resolver.rs` — Topological sort de skill dependencies
- `version_manager.rs` — Semantic versioning + rollback
- `capability_checker.rs` — Verificação de permissões antes de load

**Dependências:** notify (fs watch), semver, serde_json
**Testes mínimos:** 25 (hot-reload, dependency cycles, version conflicts)
**Critério de conclusão:** Skills carregam dinamicamente sem restart do daemon.

### Task S2.7: zylos-soar — Security Orchestration engine

**Estado atual:** 27L com module docs
**Target:** ~550L

**Módulos a implementar:**
- `event_intake.rs` — Normalização de eventos de segurança (ZylosSecurityEvent schema)
- `triage_engine.rs` — Regras de auto-triage com scoring de severidade
- `playbook_executor.rs` — State machine YAML-driven playbook execution
- `audit_trail.rs` — Append-only log com HMAC-SHA256 integrity
- `runbook_invoker.rs` — Invocação de runbooks como subprocessos estruturados

**Dependências:** serde_yaml, sha2, chrono, tokio
**Testes mínimos:** 30 (triage rules, playbook state transitions, audit integrity)
**Critério de conclusão:** Playbook YAML executa end-to-end com audit trail verificável.

---

## 4. Fase S3: Apps — CLI, Cockpit, Daemon (Semana 4-6)

**Duração:** 10 dias úteis
**Dependências:** S2 concluída (crates parciais expandidos)

### Task S3.1: zylos-cli — Command-line interface completa

**Estado atual:** ❌ Sem código
**Target:** ~800L

**Subcomandos a implementar:**
- `zylos agent {list,create,delete,status}` — Gestão de agentes
- `zylos task {submit,cancel,retry,history}` — Gestão de tarefas
- `zylos skill {install,remove,list,validate}` — Gestão de skills
- `zylos soar {test-run,status,events}` — Operações SOAR
- `zylos daemon {start,stop,health,logs}` — Controle do daemon
- `zylos config {show,set,validate}` — Configuração local

**Dependências:** clap v4, tokio, reqwest (para comunicar com daemon via REST)
**Testes mínimos:** 40 (argument parsing, error messages, integration com daemon mock)
**Critério de conclusão:** `zylos --help` mostra todos os subcomandos; cada comando tem teste de integração.

### Task S3.2: zylos-cockpit — Frontend WASM/PWA expansão

**Estado atual:** 18L main.rs
**Target:** ~1.200L + assets HTML/CSS/JS

**Componentes a implementar:**
- Dashboard de agentes em tempo real (SSE consumer)
- Visualizador de task graph com estados
- Console de logs filtráveis por agente/tarefa
- Painel SOAR com timeline de eventos
- Theme switcher (dark/light/system)
- PWA manifest + service worker para offline

**Dependências:** leptos ou yew (WASM framework), wasm-bindgen
**Testes mínimos:** 20 (component rendering, SSE reconnection, theme persistence)
**Critério de conclusão:** Cockpit serve via gateway; dashboard mostra dados reais do daemon.

### Task S3.3: zylos-daemon — Background process robusto

**Estado atual:** 5L main.rs, sem Cargo.toml
**Target:** ~600L + Cargo.toml

**Componentes a implementar:**
- Graceful shutdown com drain de tasks em andamento
- Health check endpoint (`/health`, `/ready`)
- Metrics exporter (Prometheus format)
- Log rotation e structured logging (tracing + JSON)
- PID file management e single-instance lock
- Signal handling (SIGTERM, SIGHUP para reload)

**Dependências:** axum, tracing, tracing-subscriber, tokio-graceful-shutdown
**Testes mínimos:** 25 (shutdown drain, health endpoints, signal handling)
**Critério de conclusão:** Daemon roda como serviço systemd; health check retorna 200; metrics scrapeáveis.

### Task S3.4: zylos-gateway — Documentação e hardening

**Estado atual:** ✅ Implementado (126L)
**Target:** Adicionar README + inline docs + testes de integração

**Ações:**
1. Criar README.md com arquitetura, endpoints, variáveis de ambiente
2. Adicionar `///` docs em todos os pub items
3. Adicionar testes de integração para cada endpoint REST
4. Documentar autenticação e rate limiting

**Critério de conclusão:** Gateway 100% documentado; ≥15 testes de integração passando.

---

## 5. Fase S4: Artefatos Operacionais Faltantes (Semana 6-7)

**Duração:** 5 dias úteis
**Dependências:** S0-S3 concluídas
**Fonte:** GAPS_ANALYSIS_RUNBOOKS_PLAYBOOKS.md §4

### Task S4.1: Playbook de Disaster Recovery standalone

**Arquivo a criar:** `C:/Users/Paulo/zylos-next/docs/playbooks/disaster-recovery.md`

**Conteúdo obrigatório:**
- Declaração formal de desastre (quem autoriza, critérios)
- RTO/RPO por componente (daemon, cockpit, gateway, DB, storage)
- Sequência de recuperação passo a passo com validação em cada etapa
- Procedimento de integridade de dados (checksums, replay de WAL)
- Comunicação regulatória (LGPD/DORA templates)
- Teste de recuperação completo (simulação anual obrigatória)

**Critério de conclusão:** Playbook aprovado em tabletop exercise; tempo de recuperação medido ≤ RTO definido.

### Task S4.2: Runbook de Canary Deployment para zylos-daemon

**Arquivo a criar:** `C:/Users/Paulo/zylos-next/docs/runbooks/canary-deployment.md`

**Conteúdo obrigatório:**
- Estratégia de canary (5% → 25% → 50% → 100%)
- Métricas de health check por fase (error rate, latency p99, task completion rate)
- Rollback automático triggers e procedimento manual
- Validação de compatibilidade de schema DB antes de cada fase
- Comunicação e approval gates entre fases

**Critério de conclusão:** Runbook testado em staging; rollback executado com sucesso em simulação.

### Task S4.3: Playbook de Incident Response SOAR

**Arquivo a criar:** `C:/Users/Paulo/zylos-next/docs/playbooks/incident-response-soar.md`

**Conteúdo obrigatório:**
- Classificação de severidade (P1-P4) com SLAs de resposta
- Triage automatizado via zylos-soar (regras YAML mapeadas)
- Escalação humana automática quando triage score < threshold
- Forensics checklist (preservação de logs, snapshots, evidências)
- Post-mortem template com 5 Whys obrigatórios
- Integração com Hermes notification-router para alertas

**Critério de conclusão:** Playbook integrado ao zylos-soar; teste end-to-end com evento simulado P2 passa.

### Task S4.4: Runbook de Backup e Restore de Dados

**Arquivo a criar:** `C:/Users/Paulo/zylos-next/docs/runbooks/backup-restore.md`

**Conteúdo obrigatório:**
- Schedule de backups (incremental diário, full semanal)
- Ferramentas e comandos exatos por componente (SQLite WAL, file storage, config)
- Verificação de integridade pós-backup (checksums, restore test)
- Procedimento de restore parcial vs. completo
- Retenção e purga (LGPD compliance)
- Criptografia at-rest e in-transit dos backups

**Critério de conclusão:** Restore completo testado em ambiente isolado; dados íntegros verificados.

### Task S4.5: Documentação de API REST do Gateway e Cockpit

**Arquivos a criar:**
- `C:/Users/Paulo/zylos-next/docs/api/gateway-openapi.yaml` — OpenAPI 3.0 spec
- `C:/Users/Paulo/zylos-next/docs/api/cockpit-api.md` — Endpoints internos cockpit

**Conteúdo obrigatório:**
- Todos os endpoints com request/response schemas
- Autenticação e autorização (JWT, API keys, scopes)
- Rate limiting e quotas
- Error codes padronizados
- Exemplos de curl/httpie para cada endpoint
- Versioning strategy

**Critério de conclusão:** OpenAPI spec valida contra swagger-cli; ≥90% dos endpoints têm exemplo funcional.

---

## 6. Cronograma Consolidado

| Fase | Duração | Semana | Tasks | Bloqueia |
|------|---------|--------|-------|----------|
| S0 | 5 dias | 0-1 | S0.1-S0.4 | Todas |
| S1 | 5 dias | 1-2 | S1.1-S1.2 | S2, S3 |
| S2 | 10 dias | 2-4 | S2.1-S2.7 | S3 |
| S3 | 10 dias | 4-6 | S3.1-S3.4 | S4 |
| S4 | 5 dias | 6-7 | S4.1-S4.5 | Release v6.0 |
| **Total** | **35 dias úteis** | **7 semanas** | **23 tasks** | |

---

## 7. Matriz de Dependências

```
S0.1 ──→ S0.2 ──→ S0.3 ──→ S0.4
                              │
                              ▼
                         S1.1 ──→ S1.2
                                   │
                    ┌──────────────┼──────────────┐
                    ▼              ▼              ▼
               S2.1-S2.3     S2.4-S2.5      S2.6-S2.7
                    │              │              │
                    └──────────────┼──────────────┘
                                   ▼
                         S3.1 ──→ S3.2 ──→ S3.3 ──→ S3.4
                                                        │
                                                        ▼
                                              S4.1 ──→ S4.2 ──→ S4.3
                                                        │         │
                                                        ▼         ▼
                                                   S4.4 ──→ S4.5
```

**Caminho crítico:** S0 → S1.1 → S1.2 → S2.3 (live-comm) → S3.2 (cockpit) → S4.3 (incident response)

---

## 8. Riscos e Mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|-------|---------------|---------|-----------|
| zylos-core tem tipos insuficientes para crates existentes | Alta | Alto | Extrair tipos REAIS dos crates implementados antes de definir core (S1.1) |
| WASM runtime incompatível com Windows dev environment | Média | Médio | Usar Docker como fallback para sandbox tests; CI Linux-first |
| Docker não disponível na máquina de desenvolvimento | Média | Alto | zylos-docker tem feature flag `docker-unavailable`; testes mock por padrão |
| Cockpit WASM bundle > 5MB | Baixa | Médio | Code-splitting lazy loading; medir bundle size em CI |
| Playbooks não testáveis sem infra real | Alta | Médio | Simular todos os componentes com mocks; tabletop exercises mensais |

---

## 9. Fase S5: Sentinel AIOps + HITL Approval (Fase 7C Original) — Semana 7-9

**Duração:** 10 dias úteis (200h estimadas)
**Dependências:** S3 concluída (daemon + cockpit funcionais), zylos-live-comm com AG-UI
**Fonte:** Master Plan §Fase 7C (linhas 2060-2245)
**Nota:** Esta fase corresponde à Fase 7C do plano original, que não foi coberta pelas fases S0-S4.

### Task S5.1: HITL Approval Cards via AG-UI (Task 7C.1.1)

**Estado atual:** ❌ Sem código
**Target:** ~800L

**Subtasks:**
- Implementar approval card schema compatível com AG-UI protocol
- Implementar storage server-side de approvals pendentes/histórico em SQLite
- Implementar secret masking que nunca trunca conteúdo em aprovação de inbox
- Implementar contexto headless: ferramentas gated excluídas automaticamente
- Implementar aprovação expirada → erro claro, não execução tardia
- Implementar ações semanticamente equivalentes reutilizam aprovação via blake3 hash
- Implementar four-tier decision: safety > team > personal > auto > default
- Implementar execution authority propagada via task-local em chamadas aninhadas
- Implementar multi-decision step resolve todas as chamadas sem deadlock
- Implementar abandon desbloqueia conversa após retries exauridos

**Dependências:** zylos-live-comm (AG-UI), zylos-memory-system (SQLite)
**Testes mínimos:** 30 (approval lifecycle, equivalence hashing, tier hierarchy, timeout handling)
**Critério de conclusão:** Approval cards renderizam no cockpit; storage persistente; equivalence hash funcional.

### Task S5.2: Task Durability via SQL Leases (Task 7C.1.2)

**Estado atual:** ❌ Sem código
**Target:** ~1.000L

**Subtasks:**
- Implementar `TaskWorker` com lease SQL (`lease_id UUID` + `lease_until TIMESTAMPTZ`)
- Implementar compare-and-swap para single-owner execution
- Implementar state machine completa: `queued | running | waiting_approval | waiting_input | scheduled | paused | succeeded | failed | cancelled`
- Implementar maintenance loop (60s): recover publications, reclaim accepted ideas, refresh stale
- Implementar **database-as-clock**: `lease_until = now() + make_interval(secs => lease_ms/1000)` em SQL, NUNCA `Utc::now()` no Rust
- Implementar heartbeat por task: `tokio::time::interval(lease_ms/3)` renovando lease; CAS failure → abort
- Implementar `LostLeaseError`: reverte task para `queued` atomicamente
- Implementar trio `guard()/checkpoint()/event()` dentro de `TaskContext`
- Implementar `run()` com registro em tabela `runs`; cleanup on success/failure
- Implementar `answer()` preservando histórico completo de Q&A
- Implementar `select ... for update skip locked` (zero contention multi-replica)
- Implementar `pg_notify` inside transaction (fires on commit never before)
- Implementar exponential backoff: `2^failures * 60s`, max 60min, jitter ±10%
- Implementar circuit breaker: auto-pause após 5 falhas consecutivas
- Implementar 100-task active cap enforced na criação

**Dependências:** zylos-memory-system (Postgres/SQLite)
**Testes mínimos:** 40 (lease steal, CAS race, maintenance recovery, heartbeat, LostLeaseError, skip_locked, pg_notify, backoff, circuit breaker, 100-cap)
**Critério de conclusão:** Lease durability funcional; maintenance loop recupera órfãos <120s; zero corrupção em concorrência.

### Task S5.3: Outcome Uncertainty Tri-State (Task 7C.1.3)

**Estado atual:** ❌ Sem código
**Target:** ~200L

**Subtasks:**
- Definir enum `Outcome { Succeeded, Failed, OutcomeUnknown }`
- Classificar network error/5xx em writes → OutcomeUnknown
- Retry policy: OutcomeUnknown → confirmação humana, não auto-retry
- Audit log: toda transição de outcome registrada com timestamp
- Notification dedup key para evitar alertas duplicados

**Dependências:** Task S5.2
**Testes mínimos:** 10 (5xx→unknown, timeout→unknown, 4xx→failed, 200→succeeded, dedup)
**Critério de conclusão:** Write com erro de rede nunca assume sucesso nem falha; OutcomeUnknown força confirmação humana.

### Task S5.4: Two-Stage Triage Engine (Task 7C.2.1)

**Estado atual:** ❌ Sem código
**Target:** ~400L

**Subtasks:**
- Stage 1: rules-only para safe negatives (bulk, auto-reply, promotions)
- Stage 2: model fallback com prompt estruturado + Zod parsing
- Campo `decided_by: "rule:<id>" | "model:<name>"` para auditoria
- Framing pattern: wrappers fixos para instruções agendadas
- Initiator tracking: todo turn registra causa (person/routine/responsibility/memory/handoff)
- Eval measurement contra corpus de casos rotulados

**Dependências:** Nenhuma (pode rodar em paralelo com S5.1-S5.3)
**Testes mínimos:** 15 (rule match→skip model, model failure→throw, decided_by populated, framing correctness)
**Critério de conclusão:** Rules nunca dizem "needs_you"; model failure → exceção; todo verdict tem `decided_by`.

### Task S5.5: Equivalence-Based Approval Hashing (Task 7C.2.2)

**Estado atual:** ❌ Sem código
**Target:** ~700L

**Subtasks:**
- Implementar blake3 hash de `[actor_id, bot_id, tool_ref, effect, scope, stable(target), stable(args)]`
- Trait `ApprovalDigest` com implementações blake3/sha256 intercambiáveis
- `stable()` stripa campos voláteis e ordena keys recursivamente (depth limit 6)
- Four-tier decision via `decide_action`: safety > team_rule > personal_rule > auto_review > default
- 7 mensagens de recusa distintas: DECLINED_BY_PERSON, ALREADY_WAITING, WITHDRAWN_APPROVALS_OFF, etc.
- Continuation capture via `tokio::task_local!`
- `carry_on()` multi-decision step: último decidido roda continuação
- `withdraw_allowed()`: fecha pedidos elegíveis quando allow-rule adicionada
- `abandon()`: salva resultado sintético para desbloquear conversa
- `is_permanent(error)`: previne retry infinito de falhas estruturais
- `public_approval()`: stripa campos internos para UX clara
- Escalation como ferramenta ungated (safe exit sempre disponível)
- Policy engine como WASM sandbox (wasmtime 25.0)

**Dependências:** Task S5.1 (HITL Approval)
**Testes mínimos:** 35 (same-action reuse, tier override, refusal messages, carry_on, withdraw selective, abandon, permanent error, WASM policy load)
**Critério de conclusão:** Retry reutiliza aprovação via blake3; safety tier sempre vence; escalation funciona sem grants; policy WASM carrega regras customizadas.

### Task S5.6: Sentinel Health Scanner / AIOps (Task 7C.2.3)

**Estado atual:** ❌ Sem código
**Target:** ~350L

**Subtasks:**
- Detectors como pure functions sobre stores read-only
- Normalized signatures para dedup (hash de tool + error normalizado)
- Output separado (fora do data dir principal)
- Scheduling via tokio interval (120s default)
- Privacy-first: hash PII/IDs, strip sensitive fields
- Issue upsert por signature em JSONL append-only
- Team incident reporting com rate limit (5/hour mute)

**Dependências:** zylos-live-comm
**Testes mínimos:** 15 (detector pure function, signature dedup, privacy stripping, rate limiting, JSONL format)
**Critério de conclusão:** Detectors são pure functions; signatures agrupam erros equivalentes; output sem PII; rate limit previne storm.

### Task S5.7: Dupla Auditoria da Fase 7C

**Arquivos a criar:**
- `C:/Users/Paulo/zylos-next/evals/audits/fase7c-audit-adversarial.md`
- `C:/Users/Paulo/zylos-next/evals/audits/fase7c-audit-isolated.md`

**Conteúdo obrigatório:**
- Verificação independente de cada task S5.1-S5.6 contra critérios de aceite
- Testes de integração E2E com SOAR gateway
- Validação de security boundaries (approval bypass, lease stealing, PII leakage)
- Performance benchmarks (approval latency, lease renewal throughput, triage speed)
- Assinatura formal: APPROVED / REJECTED com justificativa

**Critério de conclusão:** Ambos os audits retornam APPROVED; integração E2E com SOAR gateway verificada.

---

## 10. Cronograma Consolidado Atualizado

| Fase | Duração | Semana | Tasks | Bloqueia |
|------|---------|--------|-------|----------|
| S0 | 5 dias | 0-1 | S0.1-S0.4 | Todas |
| S1 | 5 dias | 1-2 | S1.1-S1.2 | S2, S3, S5 |
| S2 | 10 dias | 2-4 | S2.1-S2.7 | S3, S5 |
| S3 | 10 dias | 4-6 | S3.1-S3.4 | S4, S5 |
| S4 | 5 dias | 6-7 | S4.1-S4.5 | Release v6.0 |
| S5 | 10 dias | 7-9 | S5.1-S5.7 | Release v6.0 |
| **Total** | **45 dias úteis** | **9 semanas** | **30 tasks** | |

---

## 11. Matriz de Dependências Atualizada

```
S0.1 ──→ S0.2 ──→ S0.3 ──→ S0.4
                              │
                              ▼
                         S1.1 ──→ S1.2
                                   │
                    ┌──────────────┼──────────────┐
                    ▼              ▼              ▼
               S2.1-S2.3     S2.4-S2.5      S2.6-S2.7
                    │              │              │
                    └──────────────┼──────────────┘
                                   ▼
                         S3.1 ──→ S3.2 ──→ S3.3 ──→ S3.4
                                                        │
                                          ┌─────────────┼─────────────┐
                                          ▼             ▼             ▼
                                     S4.1-S4.5    S5.1 ──→ S5.2   S5.4
                                                     │         │       │
                                                     ▼         ▼       │
                                                  S5.3      S5.5 ←─────┘
                                                     │         │
                                                     ▼         ▼
                                                  S5.6 ──→ S5.7
```

**Caminho crítico atualizado:** S0 → S1.1 → S1.2 → S2.3 (live-comm) → S3.2 (cockpit) → S5.1 (HITL) → S5.2 (leases) → S5.5 (equivalence) → S5.7 (audit)

---

## 12. Riscos e Mitigações Atualizados

| Risco | Probabilidade | Impacto | Mitigação |
|-------|---------------|---------|-----------|
| zylos-core tem tipos insuficientes para crates existentes | Alta | Alto | Extrair tipos REAIS dos crates implementados antes de definir core (S1.1) |
| WASM runtime incompatível com Windows dev environment | Média | Médio | Usar Docker como fallback para sandbox tests; CI Linux-first |
| Docker não disponível na máquina de desenvolvimento | Média | Alto | zylos-docker tem feature flag `docker-unavailable`; testes mock por padrão |
| Cockpit WASM bundle > 5MB | Baixa | Médio | Code-splitting lazy loading; medir bundle size em CI |
| Playbooks não testáveis sem infra real | Alta | Médio | Simular todos os componentes com mocks; tabletop exercises mensais |
| SQL leases complexos demais para SQLite (limitações de FOR UPDATE) | Média | Alto | Feature flag `postgres-leases` vs `sqlite-simple-leases`; SQLite usa polling como fallback |
| Blake3 dependency adiciona surface area de segurança | Baixa | Médio | Audit de blake3 crate; fallback SHA-256 via trait ApprovalDigest |
| WASM policy engine (wasmtime) aumenta binary size significativamente | Média | Médio | Feature flag `wasm-policy`; compilar apenas em release builds |

---

## 13. Definição de Done Global Atualizada

O plano suplementar é considerado completo quando:
1. ✅ Todos os 30 tasks têm critério de conclusão verificado
2. ✅ `cargo build --workspace` passa sem errors
3. ✅ `cargo test --workspace` passa com ≥80% coverage nos crates implementados
4. ✅ `cargo doc --workspace` gera sem warnings
5. ✅ Todos os READMEs e docs de módulo existem e refletem código real
6. ✅ Todos os runbooks e playbooks foram testados em simulação
7. ✅ OpenAPI spec valida e cobre ≥90% dos endpoints
8. ✅ Zero definições duplicadas de tipos core em qualquer crate
9. ✅ HITL approval cards funcionais via AG-UI com storage server-side
10. ✅ Task durability via SQL leases com maintenance loop e circuit breaker
11. ✅ Outcome uncertainty tri-state implementado
12. ✅ Two-stage triage com decided_by auditável
13. ✅ Equivalence-based approval hashing funcional
14. ✅ Sentinel health scanner com detectors pure functions
15. ✅ Dupla auditoria (adversarial + isolated) ambos APPROVED
16. ✅ Integração E2E com SOAR gateway verificada

---

*Documento gerado automaticamente por Hermes Agent baseado em análise REA do codebase real. Última atualização: 2026-11-04T00:30:00-03:00*