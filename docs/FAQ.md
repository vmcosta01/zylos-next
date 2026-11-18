# FAQ — Zylos-Next

> **Versão 2.0 (Fase 11.6.4, 2026-11-17).** A Parte A traz as **30 perguntas mais frequentes**, levantadas nas dúvidas reais desta fase (setup, preview sem gateway, CI, deploy). Cada resposta cita arquivo, comando ou endpoint que **existe no repo** (conferido com `grep` em 2026-11-17). A Parte B é o FAQ legado (v1); veja o aviso no início dela.

## Parte A — Top 30 (verificadas)

### Uso do Cockpit

**A1. Abri o cockpit e só aparece "Connection lost: … Failed to fetch". Está quebrado?**
Não. O cockpit é WASM estático e, ao abrir, chama `GET http://localhost:3000/api/v1/health` (`fetch_health()` em `apps/zylos-cockpit/src/app.rs`). Sem gateway nesse endereço, mostra o banner e o botão "Tentar novamente". Suba o gateway (`cargo run -p zylos-gateway`) e clique no botão. Screenshot: `docs/assets/screenshots/00-offline-gateway.png`.

**A2. Por que o preview do Cloudflare Pages não mostra dados?**
O preview (`https://fix-ci-trigger-master.zylos-cockpit-preview.pages.dev`) só publica o frontend. O gateway ainda não está deployado, e mesmo um gateway local recusa `Origin` que não seja da máquina local (`apps/zylos-gateway/src/local_guard.rs`). O preview serve para validar UI, não operação.

**A3. Quantas tabs existem e onde estão definidas?**
31, no `enum Tab` de `apps/zylos-cockpit/src/app.rs`. Descrição e screenshot de cada uma: `docs/USER_GUIDE.md` §4.

**A4. Algumas tabs aparecem como `tab.goals`, `tab.sentinel`… Por quê?**
Faltam as traduções dessas chaves em `apps/zylos-cockpit/src/i18n.rs`. As 6 afetadas são `tab.goals`, `tab.personalities`, `tab.swarm_dag`, `tab.sentinel`, `tab.evidence_ledger` e `tab.hitl_approval`. É um bug conhecido e serve como boa tarefa de first PR (`docs/ONBOARDING_DEVS.md` §5).

**A5. Os números de Frota, Federação e Subagentes são reais?**
Não. Esses painéis (e Threads, Arquivos, Canais, Avatares, Phone Relay, SOAR) renderizam **dados de demonstração embutidos no componente**. Painéis ligados à API (Tarefas, Memória, Sentinel, Ledger, HITL, Segurança, Topologia etc.) mostram erro de fetch quando não há gateway. A legenda de cada screenshot no USER_GUIDE informa qual é o caso.

**A6. Como troco tema claro/escuro?**
Pelo botão 🌙 no header (`ThemeToggle`, `apps/zylos-cockpit/src/components/theme_toggle.rs`). A preferência fica salva em `localStorage`. A tab "Temas" (`skin_themes.rs`) oferece skins adicionais.

**A7. Existe tela de login?**
O componente existe (`apps/zylos-cockpit/src/components/auth_login.rs`) e o gateway expõe `/api/v1/auth/login` e `/api/v1/auth/register`. Porém o componente **não está montado** em `App`, então hoje o cockpit abre direto no dashboard.

**A8. Os atalhos `Ctrl+K` e `1-9` funcionam?**
Ainda não. Não há handler global de teclado. Só `channel_panel.rs` e `chat_panel.rs` tratam `keydown` (Enter para enviar). Ver aviso em `docs/USER_GUIDE.md` §6.

**A9. Em que idioma está a interface?**
Português do Brasil por padrão (`provide_i18n(Locale::PtBr)` em `app.rs`). Também existe um dicionário em inglês em `i18n.rs`.

### Gateway e operação

**A10. Como subo o gateway e em que porta ele escuta?**
`cargo run -p zylos-gateway`. O padrão é `127.0.0.1:3000` (`default_listen_addr()` em `apps/zylos-gateway/src/config.rs`), com override via `ZYLOS_LISTEN=0.0.0.0:3000`.

**A11. Quais variáveis de ambiente o gateway lê de fato?**
`ZYLOS_CONFIG` (arquivo TOML), `ZYLOS_LISTEN`, `ZYLOS_SHUTDOWN_TIMEOUT_SECS`, `ZYLOS_EVENT_BUS_CAPACITY`, `ZYLOS_SKILLS_PATH`, `ZYLOS_SOAR_AUDIT_KEY`, `ZYLOS_HERMES_TOKEN`, `SENTRY_DSN`, `AXIOM_TOKEN`, `AXIOM_DATASET` e `AXIOM_ENDPOINT` (todas aparecem via `std::env::var` em `apps/zylos-gateway/src/`). A precedência está documentada em `config.rs`: CLI > env > TOML > defaults.

**A12. Quais endpoints de health existem?**
`/gateway/health`, `/gateway/ready`, `/gateway/live` (raiz) e `/api/v1/health` e `/api/v1/cockpit/health` (definidos em `apps/zylos-gateway/src/lib.rs`). Procedimento de validação: `runbooks/pop-health-check-validation.md`.

**A13. Por que as rotas `/api/v1/hermes/*` retornam erro pedindo token?**
Elas são protegidas por Bearer token. Defina `ZYLOS_HERMES_TOKEN` antes de subir o gateway (`apps/zylos-gateway/src/hermes_api.rs`). O `scripts/zylos-start.sh` lê esse valor do arquivo `.zylos-hermes-token` na raiz, que você cria localmente e não deve commitar.

**A14. Recebo erro "o cabeçalho Origin não é desta máquina". Como resolvo?**
É o middleware `so_desta_maquina` (`apps/zylos-gateway/src/local_guard.rs`), que aceita só origens locais (`localhost`/loopback). Acesse o cockpit por `http://localhost:<porta>`. Para acesso remoto, use reverse proxy conforme `runbooks/zylos-gateway.md`.

**A15. Onde ficam os dados persistentes?**
Em SQLite, sob `data/`. Por exemplo, os canais usam `sqlite://data/channels.db` (`apps/zylos-gateway/src/lib.rs`). Backup e restore: `runbooks/backup-restore.md` e `playbooks/pop-backup-restore.md`.

**A16. Como reinicio o gateway com segurança?**
Siga `playbooks/pop-gateway-restart.md`. O shutdown gracioso respeita `ZYLOS_SHUTDOWN_TIMEOUT_SECS` (padrão 5s). No Windows/git-bash, `scripts/zylos-stop.sh` e `scripts/zylos-start.sh` fazem o ciclo completo.

**A17. Como ativo Sentry e Axiom?**
Defina `SENTRY_DSN`. Sem ela, o log mostra "Sentry disabled (SENTRY_DSN not set)" (`apps/zylos-gateway/src/main.rs`). Para Axiom, defina `AXIOM_TOKEN` e `AXIOM_DATASET` (`apps/zylos-gateway/src/axiom_logging.rs`). Os playbooks são `playbooks/pb-sentry-integration.md` e `playbooks/pb-axiom-logging-setup.md`.

**A18. Qual variável define a porta do gateway no `docker-compose.yml`?**
O gateway lê apenas `ZYLOS_LISTEN` (`apps/zylos-gateway/src/config.rs`); `ZYLOS_HOST`/`ZYLOS_PORT` não existem. O `docker-compose.yml` e o `Dockerfile` definem `ZYLOS_LISTEN=0.0.0.0:8080`, coerente com `EXPOSE 8080` e o healthcheck `http://localhost:8080/api/v1/health`; o compose publica a porta como `9090:8080` (acesse `http://localhost:9090`).

### Incidentes e procedimentos

**A19. O que faço quando um alerta do Sentinel dispara?**
Siga `playbooks/pop-sentinel-alert-investigation.md`. Ajuste de thresholds: `runbooks/rb-sentinel-threshold-tuning.md`. Visão geral: `runbooks/sentinel-aiops.md`.

**A20. Qual é o fluxo de resposta a incidente em produção?**
`runbooks/rb-production-incident-response.md`, com alertas automatizados em `docs/soar/soar-production-alerts.md` e escalonamento em `docs/playbooks/on-call-playbook.md`.

**A21. Como decido uma aprovação HITL corretamente?**
Siga `playbooks/pop-approval-decide.md`. Detalhes do serviço estão em `runbooks/rb-zylos-approval.md`, e o timeout de aprovação tem SOAR próprio em `docs/soar/soar-approval-timeout.md`.

**A22. Como revogo um token comprometido?**
`runbooks/pop-token-revoke.md`. Tokens do relay mobile seguem `playbooks/pop-relay-token-revoke.md`. Em caso de brecha de autenticação, use `docs/soar/soar-auth-breach.md`.

**A23. Uma regra de egress bloqueou uma chamada legítima. E agora?**
Siga `playbooks/pop-egress-false-positive.md`. A gestão de regras fica em `playbooks/pb-egress-rules-management.md`, e o runbook do serviço é `runbooks/rb-zylos-egress.md`.

**A24. Como paro o controle remoto / phone relay em emergência?**
Use `playbooks/pop-relay-emergency-stop.md` e `playbooks/pop-computer-control-emergency.md`.

### Desenvolvimento, CI e deploy

**A25. Qual versão do Rust devo usar?**
Use stable, fixado em `rust-toolchain.toml` com clippy, rustfmt, rust-analyzer e o target `wasm32-unknown-unknown`. O MSRV declarado é `rust-version = "1.82"` em `Cargo.toml`, verificado pelo job "MSRV Check (Rust 1.82)" do CI.

**A26. Como rodo o cockpit localmente?**
`cd apps/zylos-cockpit && trunk serve --port 3001`. O `Trunk.toml` define a porta 3000, que colide com o gateway, por isso a flag. Build de produção: `trunk build --release`.

**A27. Quais checagens o CI faz no meu PR?**
`.github/workflows/ci.yml` roda check, rustfmt, clippy, test, docs, coverage, security-audit (não bloqueante), MSRV, build-release, WASM build, WASM size check (< 5MB gzipped) e E2E com `wasm-pack test --headless --chrome`. Os crates `zylos-desktop`/`zylos-desktop-app` ficam fora dos jobs Rust. Equivalentes locais: `docs/ONBOARDING_DEVS.md` §4.

**A28. Os testes E2E do cockpit estouram timeout. O que faço?**
Defina `WASM_BINDGEN_TEST_TIMEOUT=300`, como o CI faz (o default do runner é 20s). Os testes ficam em `apps/zylos-cockpit/tests/e2e_*.rs`. Problemas de build WASM: `runbooks/pop-wasm-build-troubleshoot.md`.

**A29. Como meu PR ganha um preview?**
`.github/workflows/deploy-preview.yml` builda o cockpit e publica no Cloudflare Pages (`--project-name=zylos-cockpit-preview --branch=<sua-branch>`), depois comenta a URL no PR. Setup: `playbooks/pb-cloudflare-pages-setup.md`.

**A30. Como sai uma release de produção?**
Fazendo push de uma tag `v*`. `.github/workflows/deploy-prod.yml` valida (clippy, testes, WASM < 5MB), publica no projeto Pages `zylos-cockpit` (branch `main`) e gera release notes. Procedimento operacional: `runbooks/rb-cockpit-release.md`.

---

## Parte B — FAQ legado (v1)

> **Aviso (2026-11-17):** as respostas abaixo vêm da v1 deste FAQ e **não foram todas verificadas**. Em busca no código (`apps/`, `crates/`) **não foram encontrados**: backend PostgreSQL configurável (`[storage] backend`), `[memory] consolidation_cron`, flags `--check`/`--validate-skills-only`, endpoint `/metrics`, `[rate_limit]`, `[webhooks]`, `[mcp.servers]`, feature `dhat-heap`, OpenTelemetry e imagem `ghcr.io`. Também não existe `CHANGELOG.md` na raiz. Em caso de conflito, vale a Parte A.


## Gerais

### 1. O que é Zylos-Next?

Zylos-Next é a reescrita nativa em Rust da plataforma de orquestração de agentes Hermes v1.
Oferece 3× menos consumo de RAM (~30MB/agente idle), verificação formal de claims,
sandbox WASM/Docker, e dashboard embutido via Leptos SSR.

### 2. Por que migrar do Hermes v1?

Performance (binário único sem runtime Node.js), segurança (sandbox isolation, audit trail
tamper-evident), verificação (5-level Ground proofs), e memória persistente (SQLite com
consolidação e decay). Veja [MIGRATION.md](MIGRATION.md) para detalhes completos.

### 3. Qual a versão mínima do Rust necessária?

Rust 1.82 stable (MSRV). Edition 2021. O MSRV é testado em CI e não será alterado sem
aviso prévio via changelog e release notes.

### 4. Zylos-Next é compatível com skills Hermes v1?

Sim. Skills SKILL.md padrão são auto-migradas no primeiro startup. Legacy YAML é convertido
automaticamente. Inline JS handlers requerem reescrita parcial para Rust ou WASM.
Veja a Skill Compatibility Matrix em [MIGRATION.md](MIGRATION.md).

### 5. Posso rodar Hermes v1 e Zylos-Next simultaneamente?

Sim. Use portas diferentes e traffic shifting gradual (Canary → Beta → Gamma → Full).
Veja Operação Paralela em [MIGRATION.md](MIGRATION.md) para estratégia detalhada.

### 6. Qual licença o projeto usa?

Dual license: MIT OR Apache-2.0. Você pode escolher qualquer uma das duas.
Contribuições são aceitas sob os mesmos termos dual-license.

### 7. Onde encontro o changelog?

No arquivo `CHANGELOG.md` na raiz do repositório. Releases seguem Semantic Versioning.
Breaking changes são documentados em MIGRATION.md com guia de migração específico.

## Instalação e Setup

### 8. Como instalar Zylos-Next?

Build from source: `cargo build --release -p zylos-gateway`.
Ou Docker: `docker pull ghcr.io/nousresearch/zylos-gateway:latest`.
Veja [README.md](README.md) para quick start completo em menos de 5 minutos.

### 9. Quais são as dependências externas?

Zero para operação básica. SQLite é bundled via rusqlite (feature `bundled`).
Docker 24+ apenas se sandbox for habilitado. OpenSSL é necessário apenas para TLS nativo.

### 10. Como configurar o gateway?

TOML file + environment variables. Variáveis de ambiente fazem override sobre config.toml.
Prefixo: `ZYLOS_`. Exemplo: `ZYLOS_LISTEN=0.0.0.0:3000`.
Veja [DEPLOYMENT.md](DEPLOYMENT.md) para lista completa de variáveis.

### 11. Onde ficam os dados persistentes?

Diretório `data/` contém SQLite databases: memory.db, agents.db, audit.db.
Configure backups regulares deste diretório. O path é configurável via `[storage] data_dir`.

### 12. Como habilitar TLS?

Use reverse proxy (nginx/Caddy) terminando TLS, ou configure certificado nativo via
`[server.tls]` no config.toml. Para produção, recomenda-se reverse proxy com certbot.

### 13. Posso usar PostgreSQL ao invés de SQLite?

Sim. Configure `[storage] backend = "postgres"` e forneça connection string.
SQLite é o default para deployments single-node. PostgreSQL é recomendado para clusters.

### 14. Como atualizar Zylos-Next?

Pare o serviço, substitua o binário, rode migrações automáticas no startup.
Rollback: restaure binário anterior e backup do data/. Sem migração manual entre minor versions.

## Arquitetura

### 15. Quantos crates compõem o workspace?

23 crates sob `crates/` mais 4 apps (gateway, cockpit, cli, daemon).
Veja crate map completo em [README.md](README.md).

### 16. O que é o sistema de Ground levels?

5 níveis de evidência para claims: Assumed → Observed → Confirmed → Verified → Proven.
Cada claim deve declarar seu ground level. Claims sem evidência suficiente são rejeitadas
pelo verification pipeline antes de chegar ao agente consumidor.

### 17. Como funciona o HITL (Human-in-the-Loop)?

Approval cards com equivalence hashing (SHA-256) permitem reuse de aprovações para requests
semanticamente equivalentes. 4 tiers disponíveis: auto-approve, single-reviewer,
dual-reviewer, e panel review para decisões críticas.

### 18. O que é o Sentinel?

Sistema AIOps com detectores pure-function que analisam métricas e logs em tempo real.
Integrado ao SOAR para resposta automatizada a incidentes. Detectores são stateless
e testáveis isoladamente sem dependências externas.

### 19. Como funciona a consolidação de memória?

Job noturno (cron configurável) que deduplica entradas, aplica decay temporal,
detecta contradições, e atribui ground levels automaticamente. Reduz storage e melhora
qualidade de contexto. Configurável via `[memory] consolidation_cron`.

### 20. O que é o Cerebelo?

Orquestrador de DAGs longos com recovery estruturado automático. Gerencia workflows
multi-step com checkpointing e retry inteligente. Usa shadow-git para audit trail
de cada execução individual.

### 21. Como funciona o sandbox?

Três modos: `none` (sem isolamento), `wasm` (WASM runtime isolado), `docker` (container).
Skills não confiáveis devem usar `wasm` ou `docker`. Modo configurável por skill.

### 22. O que é o AG-UI Protocol?

Protocolo de comunicação entre agentes baseado em SSE (Server-Sent Events).
Backward compatible com Hermes v1. Campos obrigatórios: decided_by, equivalence_hash,
initiator, ground_level, sentinel_findings.

### 23. Como funciona o triage two-stage?

Primeiro stage: regras determinísticas (fast path, <1ms). Segundo stage: modelo de
classificação para casos ambíguos. Todo triage produz audit trail com decided_by obrigatório.

### 24. O que são playbooks e runbooks?

Playbooks: estratégias de alto nível (ex: security-incident-response.yaml).
Runbooks: procedimentos operacionais passo-a-passo executáveis por agentes.
Playbooks referenciam runbooks via binding matrix documentada.

## Operação

### 25. Como monitorar health do gateway?

`GET /gateway/health` (liveness) e `GET /gateway/ready` (readiness).
Prometheus metrics em `/metrics`. Veja health probes em [DEPLOYMENT.md](DEPLOYMENT.md).

### 26. Como fazer backup?

Backup do diretório `data/` (SQLite files). Use `.backup` command do sqlite3 para hot backup
sem downtime. Restore copiando arquivos de volta. Frequência recomendada: diária para prod.

### 27. Como escalar horizontalmente?

Stateless gateway permite múltiplas réplicas. SQLite pode ser substituído por PostgreSQL
para shared state. Use HPA no Kubernetes baseado em CPU/memory thresholds.

### 28. Qual o consumo de memória esperado?

~30MB por agente idle, ~200MB base overhead. Para 100 agentes: ~3.2GB total.
Compare com ~10GB equivalente em Hermes v1. Memory profiling via feature flag `dhat-heap`.

### 29. Como reportar bugs ou vulnerabilidades?

Bugs: GitHub Issues com template. Vulnerabilidades: security@nousresearch.com (não pública).
Inclua steps to reproduce, logs relevantes, versão do gateway, e ambiente de execução.

### 30. Como contribuir com o projeto?

Leia [CONTRIBUTING.md](CONTRIBUTING.md). Setup de dev, conventional commits, testing
requirements, e review process estão documentados lá. PRs de todos os tamanhos bem-vindos!

## Segurança

### 31. Como funciona o audit trail?

Shadow-git com SHA-256 chains. Cada evento de aprovação, skill execution, e memory mutation
é registrado com hash encadeado. Tamper-evident por design: alteração invalida cadeia.

### 32. Skills podem acessar o filesystem do host?

Apenas se sandbox mode for `none`. Com `wasm` ou `docker`, acesso é restrito ao sandbox.
Skills que precisam de filesystem access devem declarar `scope: filesystem` e passar HITL.

### 33. Como gerenciar secrets?

Secrets nunca em config.toml. Use environment variables ou secret manager (Vault, AWS SM).
O gateway suporta env var interpolation: `${ENV_VAR}` em valores TOML.

### 34. O sandbox WASM é seguro contra escapes?

Sim. Runtime WASM isolado com capability-based security. Sem acesso direto a syscalls.
Network access é opt-in via capability tokens. Docker oferece isolamento adicional.

### 35. Como funciona rate limiting?

Configurável por endpoint via `[rate_limit]` no config.toml. Default: 100 req/s global,
5 req/hr para incident reporting. HTTP 429 com Retry-After header quando excedido.

## Integrações

### 36. Zylos-Next integra com LLM providers?

Sim. Suporte nativo para OpenAI, Anthropic, Google Gemini, e qualquer provider compatível.
Configuração via `[llm]` section. Model fallback chain suportado para resiliência.

### 37. Posso usar MCP servers com Zylos-Next?

Sim. zylos-search-mcp implementa Model Context Protocol. Adicione servers via
`[mcp.servers]` no config.toml. Servers rodam como subprocessos isolados.

### 38. Como integrar com sistemas externos?

Via skills customizadas ou webhooks outbound. Skills podem fazer HTTP, gRPC, ou publicar
em message queues. Webhooks configuráveis via `[webhooks]` no config.toml.

### 39. Zylos-Next suporta observabilidade distribuída?

Sim. Tracing OpenTelemetry nativo. Export para Jaeger, Zipkin, ou OTLP collectors.
Correlation IDs propagados via AG-UI protocol entre agentes.

### 40. Como integrar com CI/CD?

GitHub Actions workflow incluso. Build, test, lint, e Docker push automáticos.
Release tags disparam publicação. Veja `.github/workflows/` para exemplos.

## Troubleshooting Rápido

| Sintoma | Causa Provável | Solução |
|---------|---------------|---------|
| Gateway won't start | Port ocupada ou config inválida | `ss -tlnp \| grep 3000`; `--check` |
| Health check fails | Subsystem init incompleto | Aguarde 10s; verifique logs |
| Skills não carregam | Path errado ou formato inválido | `--validate-skills-only` |
| Alta memória | Muitos agentes ou EventBus oversized | Reduza capacity; profile dhat |
| Approval cards lentos | Equivalence hash collision | Review hash strategy |
| Memory contradictions | Dados legacy inconsistentes | Review manual; adjust decay |
| SSE disconnects | Proxy timeout ou buffer overflow | Increase proxy buffer |
| Migration fails | Schema incompatível | Check migration-report.json |
| Sandbox errors | Docker not running ou permissions | Verify Docker daemon |
| Triage false positives | Rules desatualizadas | Update rules; suppress playbook |
| LLM timeout | Provider overload ou network | Configure fallback model |
| Audit chain broken | Disk corruption ou tampering | Restore from backup |

## Links Úteis

- [README.md](README.md) — Quick start e crate map
- [ARCHITECTURE.md](ARCHITECTURE.md) — Diagramas C4 e ADRs
- [DEPLOYMENT.md](DEPLOYMENT.md) — Docker, K8s, bare metal
- [MIGRATION.md](MIGRATION.md) — Guia Hermes v1 → Zylos-Next
- [CONTRIBUTING.md](CONTRIBUTING.md) — Guidelines de contribuição
- [OpenAPI Spec](api/gateway-openapi.yaml) — API reference completa
- [Playbooks](../playbooks/) — Strategic playbooks
- [GitHub Repository](https://github.com/nousresearch/zylos-next) — Source code
- [Discord Community](https://discord.gg/nousresearch) — Community support
