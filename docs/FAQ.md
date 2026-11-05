# FAQ — Zylos-Next

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
