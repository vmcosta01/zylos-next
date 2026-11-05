# Architecture Decision Records — Zylos-Next v6.0

## ADR-001: Rust como Linguagem Única do Runtime
- **Status:** Aprovado
- **Data:** 2026-10-30
- **Contexto:** Ecossistemas existentes (Python/Node) consomem >200MB RAM por agente idle; necessidade de <30MB e safety guarantees em produção.
- **Decisão:** Rust 1.82+ para todos os 6 sistemas core e apps.
- **Consequências:** Curva de aprendizado maior; performance 5-10x melhor que Python; memory safety sem GC; toolchain madura (cargo, clippy, rustfmt).
- **Alternativas Rejeitadas:** Go (sem generics robustos para DAGs), Zig (ecossistema imaturo), C++ (memory safety insuficiente).

## ADR-002: SQLite WAL como Storage Principal
- **Status:** Aprovado
- **Data:** 2026-10-30
- **Contexto:** Necessidade de ACID + portabilidade + zero-config para até 50 agentes simultâneos.
- **Decisão:** rusqlite 0.32 com modo WAL ativado via pragma.
- **Consequências:** Single-writer limit (~1000 writes/sec); suficiente para escala alvo; backup = cópia de arquivo; migrações via SQL puro.
- **Alternativas Rejeitadas:** PostgreSQL (overhead operacional), DuckDB (OLAP não OLTP), Redis (persistência frágil).

## ADR-003: WASM (wasmtime) para Sandboxing de Tools
- **Status:** Aprovado
- **Data:** 2026-10-30
- **Contexto:** Tools de terceiros não podem comprometer o host; isolamento forte obrigatório.
- **Decisão:** wasmtime 25.0 com snapshot/resume e capability-based permissions.
- **Consequências:** Overhead ~10ms cold start; isolamento de memória/filesystem/rede garantido; ecosystem WASI crescente.
- **Alternativas Rejeitadas:** Docker apenas (muito pesado por tool), gVisor (Linux-only), Firecracker (VM overhead excessivo).

## ADR-004: Leptos para Cockpit Web (WASM Frontend)
- **Status:** Aprovado
- **Data:** 2026-10-30
- **Contexto:** Frontend deve rodar no browser sem Node.js build step; SSR nativo desejável.
- **Decisão:** Leptos 0.7 com sinais reativos e SSR.
- **Consequências:** 20% mais rápido que Yew; fine-grained reactivity; mesma linguagem do backend (Rust).
- **Alternativas Rejeitadas:** Yew (mais lento, hooks model), Dioxus (menos maduro), React/WASM (bundle gigante).

## ADR-005: petgraph + SQLite para Context Graph
- **Status:** Aprovado
- **Data:** 2026-10-30
- **Contexto:** Memory system precisa de grafo persistente com queries eficientes e rerank.
- **Decisão:** petgraph 0.6 em memória + persistência via SQLite WAL; F1-score target 0.800.
- **Consequências:** Grafo carregado sob demanda; queries <1ms p95; serialização customizada para nós/arestas.
- **Alternativas Rejeitadas:** Neo4j (dependência externa pesada), Apache AGE (PostgreSQL lock-in), in-memory only (perda de dados).

## ADR-006: Tokio JoinSet + CancellationToken para Lifecycle
- **Status:** Aprovado
- **Data:** 2026-10-30
- **Contexto:** Agentes devem ter spawn/kill/suspend graceful sem leaks.
- **Decisão:** Tokio JoinSet para gerenciar tarefas concorrentes + CancellationToken hierárquico.
- **Consequências:** Shutdown ordenado; cancellation safety documentada por função; zero goroutine leaks.
- **Alternativas Rejeitadas:** async-std (menos adoption), smol (features insuficientes), OS threads (overhead 100x).

## ADR-007: Axum + Tower para API Gateway
- **Status:** Aprovado
- **Data:** 2026-10-30
- **Contexto:** REST + WebSocket + SSE em único binário; middleware chain flexível.
- **Decisão:** Axum 0.7 com Tower 0.5 service layer.
- **Consequências:** Type-safe routing; middleware composável (auth, rate-limit, tracing); HTTP/2 nativo.
- **Alternativas Rejeitadas:** Actix-web (actor model desnecessário), Warp (filters complexos), Rocket (sync-first legacy).

## Referências
- [Manifesto Conceito #7](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md) — Documentos Vivos
- [ARCHITECTURE.md](./ARCHITECTURE.md) para detalhes técnicos de cada decisão