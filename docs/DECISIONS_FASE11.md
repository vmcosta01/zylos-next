# DECISIONS — Fase 11 Unificada (A→B→C→D)

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Mestre:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](./IMPLEMENTATION_PLAN_FASE11_MEGA.md)
**Track Record:** [TRACKRECORD_FASE11.md](./TRACKRECORD_FASE11.md)

---

## Índice de ADRs

| # | Título | Status | Fase | Data Decisão |
|---|--------|--------|------|--------------|
| ADR-011 | Profile Release Optimization Strategy | 🟡 Pendente | 11.1 | — |
| ADR-012 | CRDT Library Choice | 🟡 Pendente | 11.9 | — |
| ADR-013 | Analytics Storage Backend | 🟡 Pendente | 11.10 | — |
| ADR-014 | Notification Delivery Guarantee | 🟡 Pendente | 11.8 | — |
| ADR-015 | Sentinel Anomaly Detection Algorithm | 🟡 Pendente | 11.7 | — |

---

## Template de ADR

```markdown
### ADR-XXX: [Título]

**Status:** 🟡 Pendente | 🔵 Proposto | ✅ Aceito | ❌ Rejeitado | 🔄 Substituído por ADR-YYY
**Data:** YYYY-MM-DD
**Fase:** 11.X
**Decisor(es):** [Tech Lead, Arquiteto, SRE, etc.]

#### Contexto
[Descrever o problema ou situação que motiva a decisão. Incluir constraints técnicas, requisitos de negócio e alternativas consideradas.]

#### Decisão
[Descrição clara e concisa da decisão tomada. Incluir configuração específica, library escolhida, padrão adotado, etc.]

#### Consequências
- **Positivas:** [Listar benefícios diretos da decisão]
- **Negativas:** [Listar trade-offs, riscos ou custos aceitos]
- **Neutras:** [Mudanças que não impactam positivamente nem negativamente]

#### Alternativas Consideradas
| Alternativa | Prós | Contras | Motivo da Rejeição |
|-------------|------|---------|-------------------|
| [Opção A] | ... | ... | ... |
| [Opção B] | ... | ... | ... |

#### Validação
[Como verificar que a decisão está funcionando conforme esperado. Métricas, testes, benchmarks.]

#### Referências
- [Links para docs, RFCs, benchmarks, issues relacionadas]
```

---

## ADR-011: Profile Release Optimization Strategy

**Status:** ✅ Aceito
**Data:** 2026-11-12
**Fase:** 11.1
**Tasks Relacionadas:** T11.1.1, T11.1.3, T11.1.6
**Decisor(es):** Hermes Agent (dados empíricos)

### Contexto
O build release do cockpit (`trunk build --release`) precisava ser avaliado quanto a tamanho do WASM e tempo de build para definir a estratégia de otimização ideal. O wasm-opt v131 bundled no trunk 0.22.1 falhava com Rust 1.99 por não passar flags de bulk memory e nontrapping float-to-int automaticamente.

### Decisão
**Manter profile Default do workspace root** (`opt-level = 3`, `lto = true`, `codegen-units = 1`, `strip = true`). Nenhuma mudança necessária.

### Consequências
- **Positivas:** WASM gzipped = 1.44 MB (71% abaixo do target de 5MB); build time ~53s; zero risco de regressão; fat LTO já ativo
- **Negativas:** `opt-level = 3` prioriza performance sobre tamanho (mas 1.44MB já é excelente); build poderia ser ~20s mais rápido com thin LTO
- **Neutras:** Warning sobre `target_features` custom section missing é esperado com `strip = true` e não afeta funcionalidade

### Alternativas Consideradas
| Alternativa | Prós | Contras | Motivo da Rejeição |
|-------------|------|---------|-------------------|
| Thin LTO + opt-level z + strip | Build ~20s mais rápido | Binário maior; ganho marginal vs default | 1.44MB gzipped já atende target com folga |
| Fat LTO + opt-level z + strip | Menor binário teórico | Build 8-10min; wasm-opt falha sem wrapper | Complexidade desnecessária dado resultado atual |
| No LTO + opt-level s | Build mais rápido (2-3min) | Binário 6-8MB gzipped; pior perf runtime | Regressão inaceitável em tamanho e performance |
| Code splitting (lazy loading) | Initial load menor | Implementação complexa; total ainda ~8MB | Prematuro antes de validar necessidade real |

### Validação
- ✅ `trunk build --release` completa em ~53s com exit code 0
- ✅ WASM bruto: 5.02 MB | Gzipped: 1.44 MB | Ratio: 28.7%
- ✅ Target de <5MB gzipped atingido com 71% de margem
- ✅ Wrapper wasm-opt C# criado para corrigir incompatibilidade Rust 1.99 + wasm-opt v131

### Referências
- [BENCHMARK_RELEASE_PROFILES.md](./BENCHMARK_RELEASE_PROFILES.md) — Dados completos do benchmark
- [Cargo.toml](../Cargo.toml) linhas 124-128 — Profile release atual do workspace
- [Trunk.toml](../apps/zylos-cockpit/Trunk.toml) — Configuração do trunk com wasm_opt_args documentado

---

## ADR-012: CRDT Library Choice

**Status:** 🟡 Pendente
**Fase:** 11.9
**Tasks Relacionadas:** T11.9.1, T11.9.2

### Contexto
Precisamos de colaboração em tempo real verdadeira para edição simultânea de skills/spaces no cockpit. A escolha da library CRDT impacta latência sync, tamanho do WASM bundle, DX e maturidade do ecossistema.

### Opções em Avaliação

| Library | WASM Size | Latência Sync | Maturidade | DX Rust | Community |
|---------|-----------|---------------|------------|---------|-----------|
| Yjs | ~800KB | <50ms p95 | Alta (5+ anos) | Bindings oficiais | Grande (ProseMirror/TipTap) |
| Automerge | ~1.2MB | <100ms p95 | Média (3 anos) | Crate nativo Rust | Média |
| Diamond-types | ~400KB | <30ms p95 | Baixa (1 ano) | Crate nativo Rust | Pequena |

### Critérios de Decisão
- WASM size <1.5MB (para não impactar initial load)
- Latência sync p95 <100ms local, <500ms remote
- Bindings WASM estáveis e mantidos
- Suporte a presence awareness e undo history
- Licença compatível (MIT/Apache-2.0)

### Validação Planejada
- Implementar PoC com cada library (edit text simples)
- Benchmark latência com 2, 5, 10 clientes simultâneos
- Medir WASM size final com bindings
- Avaliar documentação e exemplos disponíveis

---

## ADR-013: Analytics Storage Backend

**Status:** 🟡 Pendente
**Fase:** 11.10
**Tasks Relacionadas:** T11.10.1, T11.10.8

### Contexto
O crate `zylos-analytics` atualmente usa SQLite para persistência de métricas. Com expansão para agregações temporais e dashboard completo, precisamos avaliar se SQLite escala adequadamente ou se migrar para backend especializado é necessário.

### Opções em Avaliação

| Backend | Query Perf (1M rows) | Setup Complexity | Ops Overhead | Cost |
|---------|---------------------|------------------|--------------|------|
| SQLite (atual) | ~2-5s aggregations | Zero (já integrado) | Zero | Zero |
| DuckDB | ~200-500ms aggregations | Baixa (embedded) | Zero | Zero |
| TimescaleDB | ~100-300ms aggregations | Alta (Postgres extension) | Média (backup, tuning) | Médio |
| ClickHouse | ~50-150ms aggregations | Alta (standalone) | Alta (cluster ops) | Alto |

### Critérios de Decisão
- Dashboard load time <2s com 30 dias de dados (~1M rows)
- Agregações complexas (GROUP BY hourly/daily) <1s
- Zero infraestrutura adicional (preferência por embedded)
- Compatível com stack Rust existente (sqlx/sqlite ou duckdb-rs)

### Validação Planejada
- Gerar dataset sintético de 1M rows (30 dias, 10 agents, 50 skills)
- Benchmark queries de agregação em cada backend
- Medir memória/CPU durante queries pesadas
- Avaliar maturidade dos crates Rust (duckdb-rs, clickhouse-rs)

---

## ADR-014: Notification Delivery Guarantee

**Status:** 🟡 Pendente
**Fase:** 11.8
**Tasks Relacionadas:** T11.8.5, T11.8.7

### Contexto
O notification router no gateway precisa dispatch notificações para Slack/Notion/Linear. Precisamos definir garantia de entrega que balanceie confiabilidade, complexidade e tolerância a duplicatas.

### Opções em Avaliação

| Garantia | Implementação | Duplicatas Possíveis? | Mensagens Perdidas? | Complexidade |
|----------|--------------|----------------------|--------------------|--------------|
| At-most-once | Fire-and-forget | Não | Sim (se falhar) | Baixa |
| At-least-once | Retry queue + ack | Sim (se ack falhar após send) | Não | Média |
| Exactly-once | Idempotency keys + dedup | Não | Não | Alta |

### Critérios de Decisão
- Notificações críticas (alertas SEV-1) NÃO podem ser perdidas
- Duplicatas aceitáveis se raras (<0.1%) e idempotentes no destino
- Implementação simples o suficiente para manter em Rust puro
- Rate limits das APIs externas respeitados

### Validação Planejada
- Testar comportamento sob falha de rede (simular timeout/API down)
- Verificar se Slack/Notion/Linear suportam idempotency keys nativamente
- Medir overhead de retry queue em memória/CPU
- Validar que duplicatas não causam efeitos colaterais indesejados

---

## ADR-015: Sentinel Anomaly Detection Algorithm

**Status:** 🟡 Pendente
**Fase:** 11.7
**Tasks Relacionadas:** T11.7.1, T11.7.3, T11.7.7

### Contexto
O sentinel precisa detectar anomalias em métricas de sistema (CPU, memory, latency, error rate). Precisamos escolher algoritmo que balanceie precisão, false positives, complexidade computacional e explicabilidade.

### Opções em Avaliação

| Algoritmo | Precisão | False Positive Rate | CPU Overhead | Explicabilidade | Tuning Difficulty |
|-----------|----------|--------------------|--------------|-----------------|-------------------|
| Z-score (statistical) | Média | ~5-10% | Muito baixo | Alta (threshold claro) | Baixa |
| EWMA (exponential moving avg) | Boa | ~3-7% | Baixo | Média | Média |
| Isolation Forest (ML) | Alta | ~1-3% | Médio | Baixa (black box) | Alta |
| Prophet (time series) | Alta | ~2-5% | Alto | Média | Alta |

### Critérios de Decisão
- Detecção de anomalia em <30s após ocorrência
- False positive rate <5% em operação normal
- CPU overhead <1% em produção
- Explicabilidade alta para operadores entenderem alertas
- Hot-reload de thresholds sem restart

### Validação Planejada
- Gerar dataset sintético com anomalias conhecidas (spike, drift, outlier)
- Medir detection delay, precision, recall para cada algoritmo
- Medir CPU/memory overhead em load normal
- Testar hot-reload de thresholds
- Avaliar facilidade de tuning por operador não-ML

---

## Histórico de Revisões

| Data | Versão | Autor | Mudanças |
|------|--------|-------|----------|
| 2026-11-12 | 1.0.0 | Hermes Agent | Criação inicial com template + 5 ADRs pendentes |

---

*Este documento é atualizado conforme decisões são tomadas. Última atualização: 2026-11-12.*