# Runbook & Playbook: Fase 1 — BOT_SYSTEM (Lifecycle + DAG + P2P)

**Sistema:** BOT_SYSTEM — Fase 1
**Crate:** `zylos-bot-system`
**Módulos:** `lifecycle`, `dag_executor`, `p2p_messenger`
**Última Atualização:** 31 de outubro de 2026
**Status Operacional:** ✅ 21/21 testes passando

---

## 🎯 Visão Geral (Todos os Níveis)

A Fase 1 implementa o sistema de orquestração de bots do Zylos-Next: gerenciamento de ciclo de vida de agentes (spawn/kill/shutdown), execução de tarefas via DAG com ordenação topológica e detecção de ciclos, e comunicação P2P entre agentes com pub/sub, DLQ e TTL. Este crate é a base sobre a qual todos os agentes especializados operam.

---

## 👨‍💻 NÍVEL SENIOR — Arquitetura e Decisões

### Responsabilidades
- Validar que o modelo de concorrência escala para 100+ agentes simultâneos
- Garantir que o DAG executor previne deadlocks em grafos complexos
- Auditar isolamento de mensagens P2P (nenhum agente lê fila de outro)
- Definir contratos de retry/backoff para tolerância a falhas transitórias

### Procedimentos

#### S1.1: Benchmark de Concorrência de Agentes
```bash
cd C:/Users/Paulo/zylos-next
cargo bench -p zylos-bot-system -- lifecycle_spawn_100_agents
```
**Critério Senior:** Spawn de 100 agentes <500ms total. Shutdown graceful <2s. Se exceder, investigar contention no `LifecycleManager`.

#### S1.2: Auditoria de Isolamento P2P
Verificar que `P2PMessenger` usa canais separados por subscriber ID:
```rust
// Em p2p_messenger.rs — cada subscribe() cria mpsc::channel independente
// Nenhum broadcast global; apenas pub/sub explícito por tópico
```
**Por que importa:** Vazamento de mensagens entre agentes viola princípio #7 (Zero Trust Messaging) do manifesto.

#### S1.3: Validação de Detecção de Ciclos no DAG
```rust
use zylos_bot_system::DagExecutor;

let mut dag = DagExecutor::new();
dag.add_task("a", vec!["b"]);
dag.add_task("b", vec!["c"]);
dag.add_task("c", vec!["a"]); // ciclo!
assert!(dag.validate().is_err()); // deve rejeitar
```
**Critério Senior:** Ciclos detectados em O(V+E). Mensagem de erro lista nós envolvidos para debugging.

### Decisões Documentadas
| Decisão | Justificativa | Alternativa Rejeitada |
|---------|--------------|----------------------|
| Tokio JoinSet para spawn | Structured concurrency; cancel propagation automática | std::thread (sem async, overhead alto) |
| mpsc por subscriber | Isolamento garantido; sem lock contention | broadcast channel (vazamento possível) |
| DLQ com TTL | Mensagens órfãs não acumulam indefinidamente | Retry infinito (memory leak) |
| Topological sort Kahn | Determinístico; detecta ciclos durante sort | DFS-based (não determinístico em ordem) |

---

## 👷 NÍVEL PLENO — Implementação e Validação

### Responsabilidades
- Executar testes unitários e validar cobertura
- Configurar cenários de integração multi-agente
- Documentar padrões de uso do DAG executor
- Implementar novos providers de messenger se necessário

### Procedimentos

#### P1.1: Rodar Suite Completa de Testes
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-bot-system --lib
```
**Esperado:** 21 testes passando em <2s.

#### P1.2: Cenário de Integração — Pipeline de 3 Agentes
```rust
use zylos_bot_system::{LifecycleManager, DagExecutor, P2PMessenger};

// 1. Criar lifecycle manager
let lm = LifecycleManager::new();
let agent_a = lm.spawn("agent-a").await?;
let agent_b = lm.spawn("agent-b").await?;
let agent_c = lm.spawn("agent-c").await?;

// 2. Configurar DAG: A → B → C
let mut dag = DagExecutor::new();
dag.add_task("process-a", vec![]);
dag.add_task("process-b", vec!["process-a"]);
dag.add_task("process-c", vec!["process-b"]);

// 3. Executar com mensageiro compartilhado
let messenger = P2PMessenger::new();
dag.execute(&messenger).await?;

// 4. Shutdown graceful
lm.shutdown_all().await?;
```

#### P1.3: Verificar DLQ e TTL
```rust
let messenger = P2PMessenger::new();
messenger.publish("topic-x", payload).await;
// Sem subscriber → mensagem vai para DLQ
tokio::time::sleep(Duration::from_secs(60)).await; // TTL expira
assert_eq!(messenger.dlq_len(), 0); // limpo automaticamente
```

### Checklist de Validação
- [ ] `cargo test -p zylos-bot-system --lib` passa 21/21
- [ ] Spawn de 10 agentes <100ms
- [ ] DAG com ciclo rejeitado com mensagem clara
- [ ] DLQ limpa após TTL
- [ ] Shutdown graceful não deixa handles pendentes
- [ ] Clippy zero warnings

---

## 🌱 NÍVEL JÚNIOR — Execução Guiada

### Responsabilidades
- Rodar testes e reportar resultados
- Seguir exemplos de uso sem modificar
- Reportar erros com output completo

### Procedimentos Passo-a-Passo

#### J1.1: Compilar o Crate
```bash
cd C:/Users/Paulo/zylos-next
cargo build -p zylos-bot-system
```
✅ `Finished` → prossiga
❌ Erro → copie output completo e reporte ao Pleno

#### J1.2: Rodar Testes
```bash
cargo test -p zylos-bot-system --lib 2>&1 | tee test-fase1.log
```
✅ `test result: ok. 21 passed` → sucesso
❌ Falha → anote nome do teste + mensagem de erro

#### J1.3: Verificar Exemplo Básico
Leia `crates/zylos-bot-system/examples/simple_pipeline.rs` (se existir) e execute:
```bash
cargo run -p zylos-bot-system --example simple_pipeline
```
Se não existir exemplo, reporte ao Pleno como gap de documentação.

### Glossário para Júniores
| Termo | Significado |
|-------|------------|
| **DAG** | Directed Acyclic Graph — grafo direcionado sem ciclos; usado para ordenar tarefas dependentes |
| **JoinSet** | Estrutura do Tokio que gerencia múltiplas tasks async; cancela todas se uma falhar |
| **DLQ** | Dead Letter Queue — fila para mensagens que não puderam ser entregues |
| **TTL** | Time To Live — tempo máximo que uma mensagem/resource vive antes de ser descartado |
| **Pub/Sub** | Publish/Subscribe — padrão onde publicadores enviam mensagens sem saber quem são os assinantes |
| **Graceful shutdown** | Desligamento ordenado: termina tarefas em andamento antes de encerrar processo |

### Quando Escalar
- Teste falha intermitentemente → **Pleno** (possível race condition)
- Spawn demora >1s para 10 agentes → **Senior** (performance regression)
- Mensagem aparece em subscriber errado → **Senior imediatamente** (violação de isolamento)

---

## 🔗 Referências Cruzadas
- **Master Plan:** `zylos-next-master-plan.md` linhas 200-350 (Fase 1)
- **Pré-requisito:** `runbooks/fase0-foundation.md`
- **Próxima Fase:** `runbooks/fase2-memory-system.md`
- **Manifesto:** Conceitos #7 (Zero Trust Messaging), #12 (Structured Concurrency)