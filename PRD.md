# Product Requirements Document — Zylos-Next v6.0

## Visão do Produto
Zylos-Next é um **runtime soberano de sistemas multi-agente escrito em Rust**, projetado para orquestrar, executar e auditar agentes autônomos com performance de sistemas embarcados (<30MB RAM por agente idle), isolamento forte (WASM + Docker por agente) e verificação adversarial de resultados.

**Problema a resolver:** Ecossistemas de agentes existentes (Hermes, CrewAI, LangGraph, OpenHands) dependem de runtimes pesados (Python/Node), não oferecem isolamento por agente, não possuem verificação adversarial nativa, e exigem dependências runtime que comprometem soberania técnica. O Zylos-Next resolve esses 4 problemas em um único binário Rust.

## Target Users Específicos

### Persona 1: Victor Costa — Arquiteto de Sistemas Multi-Agente
- **Dor:** Não consegue escalar além de 5 agentes simultâneos sem travar o Hermes; perdeu trabalho por falta de verificação adversarial; não tem cockpit visual unificado.
- **Comportamento atual:** Usa Claude Code + Hermes Desktop + 4 perfis ativos; precisa de 6 sistemas core (BOT, MEMORY, LIVE_COMM, VERIFICATION, SANDBOX, SKILLS) para escalar.

### Persona 2: Equipe de Engenharia de Agentes (3-10 pessoas)
- **Dor:** Não consegue auditar decisões de agentes em produção; não tem métricas versionadas; não pode fazer rollback granular de workflows.
- **Comportamento atual:** Usa Grafana + Loki + scripts ad-hoc; quer observabilidade nativa e eval flywheel.

## Features Core

1. **BOT_SYSTEM**: Lifecycle manager + DAG runtime + P2P messaging type-safe + shadow-git + recovery-ladder
2. **MEMORY_SYSTEM**: SQLite WAL Context Graph + VectorStore trait + 4-scope isolation + brief-aware rerank
3. **LIVE_COMMUNICATION**: AG-UI streaming + event bus SSE/WS + Mobile MCP server
4. **VERIFICATION_PROOF**: Ground enum + Panel adversarial + Guardrail <1ms + Tree-of-Thoughts
5. **SANDBOX_ISOLATION**: WASM snapshot/resume + Docker per-agent + Browser anti-detect + OCR sidecar
6. **SKILLS_REGISTRY**: index.json discovery <100ms + JSON Schema compilado + Eval flywheel + export para .cursor/.claude

Referência de fluxo de usuário: [APP_FLOW.md](./APP_FLOW.md)

## Research Summary
- **2.686 repositórios filtrados** de um catálogo de 6.539 entradas → nenhum oferece implementação Rust completa dos 6 sistemas core
- **63 papers arXiv com metadados reais** → validaram padrões de VectorStore, Context Graph, 4-scope isolation
- **7 links de alto valor** → OpenFANG (137k LOC Rust), Rig (VectorStore trait), AutoAgents (Ractor)
- **5 versões de relatório comparativo (v1-v5)** + **v6** consolidando a decisão "construir tudo em Rust"
- **Manifesto Conceitos-Desenvolvimento-IA v3.0** → 22 conceitos canônicos obrigatórios

## Definition of Done (DoD)
- [ ] Todos os 6 sistemas core implementados em crates Rust separados
- [ ] <30MB RAM por agente idle (medido via criterion + /proc)
- [ ] <50ms TTFD (Time-To-First-Decision) em workflows de 100 nós
- [ ] Guardrail rejeita falso-sucesso em <1ms (p95)
- [ ] WASM snapshot/resume funcional em <100ms
- [ ] SKILLS_REGISTRY descobre skills em <100ms
- [ ] 50+ agentes simultâneos sem lag no cockpit
- [ ] Todos os 22 conceitos do manifesto implementados e auditados
- [ ] 15 documentos obrigatórios do manifesto aprovados por auditoria adversarial
- [ ] CI/CD verde com coverage >80% e security audit limpo
- [ ] Cockpit web (Leptos WASM) funcional com lazy-roster

## Out of Scope (v6.0)
- [ ] Integração runtime com plugins Hermes existentes (apenas migration adapters)
- [ ] Suporte a múltiplas LLMs no core (adapters externos)
- [ ] Mobile apps nativos iOS/Android (apenas MCP server para apps existentes)
- [ ] GUI desktop nativa (apenas web via Leptos WASM)
- [ ] Multi-tenancy completo (apenas roles/permissões server-side)
- [ ] Kubernetes operator (binário standalone)
- [ ] Integração com cloud providers específicos (AWS/GCP/Azure SDKs)

## Métricas de Sucesso
- **Latência:** p95 <50ms para decisão em DAG de 100 nós
- **Memória:** <30MB por agente idle, <500MB com 50 agentes ativos
- **Throughput:** 2400 tasks/sec (baseline OpenFANG)
- **Confiabilidade:** 99.9% de workflows completos com rollback automático
- **Segurança:** 0 vulnerabilidades críticas em cargo-audit
- **Qualidade:** Coverage >80%, complexidade ciclomática <15 por função

## Requisitos Funcionais
- Binário único `zylos-gateway` unificando os 6 sistemas
- CLI `zylos-cli` para administração
- Daemon compartilhado `zylos-daemon` para lifecycle
- Cockpit web em Leptos WASM
- SQLite WAL como storage principal
- A2A + MCP + AG-UI como protocolos de comunicação

## Requisitos de Performance
- Cold start <180ms (baseline OpenFANG)
- Hot path <1ms para operações de memória
- WASM snapshot <100ms
- AG-UI streaming com <50ms de latência

## Requisitos de Segurança
- Sandboxing WASM obrigatório para tools de terceiros
- Docker por agente para isolamento de rede/filesystem
- Browser fingerprint no engine anti-detect
- Secrets via environment variables (nunca hardcoded)
- Authorization verificada server-side em toda request

## Requisitos de Observabilidade
- Tracing estruturado via OpenTelemetry
- Métricas Prometheus nativas
- Logs JSON com correlation IDs
- Cockpit com métricas em tempo real

## Referências
- [Master Plan v3.0](./IMPLEMENTATION_PLAN.md)
- [Arquitetura](./ARCHITECTURE.md)
- [Manifesto Conceitos-Desenvolvimento-IA](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md)
