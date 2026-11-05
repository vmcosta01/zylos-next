# Playbook: Vendor & Dependency Pivot — Plano B para Crates Críticos

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Protocolo estratégico para substituição de vendors, crates ou serviços externos quando o provider atual se torna inviável (descontinuação, preço abusivo, vulnerabilidade crítica sem patch, mudança de termos)
**Público-alvo:** Tech Lead, Dev Senior, Arquiteto de Software
**Gatilho:** Anúncio de EOL/depreciação, aumento de preço >30%, CVE crítica sem fix em 30 dias, mudança de ToS incompatível, outage recorrente do vendor (>3 incidentes em 90 dias)

---

## 1. Contexto Organizacional

O Zylos-Next depende de vendors e crates externos para funcionalidades críticas:

| Componente | Vendor/Crate Atual | Alternativas Mapeadas | Risco de Lock-in |
|------------|-------------------|----------------------|------------------|
| LLM Embeddings | OpenAI text-embedding-3-small | Ollama nomic-embed, Voyage AI, Cohere | Médio (API compatível via trait) |
| LLM Judge (Eval) | OpenAI GPT-4o-mini | Anthropic Claude Haiku, Gemini Flash, Ollama local | Baixo (LLM-as-judge é agnóstico) |
| Search Provider | Brave Search API | Tavily, SearXNG self-hosted, DuckDuckGo | Baixo (fan-out multi-provider já implementado) |
| WASM Runtime | wasmtime 25.0 | WasmEdge, Wasmer | Alto (API específica, migration custosa) |
| Container Runtime | Docker / Podman | nerdctl, containerd direto | Médio (abstração via trait) |
| Vector Store | SQLite + brute-force | Qdrant, ChromaDB, pgvector | Médio (trait VectorStore já abstrai) |
| Frontend Framework | Leptos (WASM) | Yew, Dioxus, Sycamore | Alto (rewrite completo do cockpit) |
| MCP Protocol | zylos-mobile stdio | HTTP+SSE transport, gRPC | Baixo (spec-compliant, transport swap) |

Missão: garantir que nenhum vendor único seja ponto de falha catastrófica. Toda dependência crítica deve ter plano B documentado, testado e acionável em <72h.

---

## 2. Papéis e Responsabilidades

| Papel | Responsabilidade | Decisão |
|-------|-----------------|---------|
| Tech Lead | Aprovar pivot, definir timeline, comunicar stakeholders | Go/No-go para migração |
| Dev Senior | Implementar adapter/trait abstraction, validar alternativa | Escolha técnica da alternativa |
| Arquiteto | Avaliar impacto arquitetural, definir rollback strategy | Trade-offs de cada alternativa |
| SRE / Ops | Provisionar infra alternativa, monitorar pós-migração | Timing de deploy em produção |
| Product Manager | Comunicar impacto a clientes (se houver mudança visível) | Messaging externo |
| Financeiro | Aprovar custo adicional de vendor alternativo (se aplicável) | Budget cap |

---

## 3. Fluxo de Decisão Estratégica

### 3.1 Quando Iniciar Avaliação de Pivot

Execute este playbook quando QUALQUER condição for verdadeira:

- [ ] Vendor anuncia EOL/depreciação com data <6 meses
- [ ] Aumento de preço >30% sem justificativa técnica aceitável
- [ ] CVE crítica (CVSS ≥9.0) sem patch disponível em 30 dias
- [ ] Mudança de ToS/License incompatível com uso comercial do Zylos
- [ ] Outage recorrente: ≥3 incidentes em 90 dias com SLA violado
- [ ] Vendor adquirido por empresa com histórico anti-consumer
- [ ] Dependência remove feature crítica sem alternativa no mesmo crate
- [ ] Auditoria de segurança identifica risco sistêmico no vendor

### 3.2 Matriz de Decisão

| Cenário | Ação Recomendada | Prazo | Aprovador |
|---------|-----------------|-------|-----------|
| EOL anunciado, >6 meses até deadline | Mapear alternativas, criar spike de validação | 2 sprints | Tech Lead |
| EOL anunciado, <6 meses até deadline | Iniciar migração imediata, paralelo ao trabalho normal | 1 sprint | Tech Lead + PM |
| Preço >30%, alternativa viável existe | Negociar com vendor atual; se falhar, iniciar pivot | 30 dias | Tech Lead + Financeiro |
| CVE crítica sem patch | Implementar workaround temporário + iniciar pivot | 7 dias | Security Engineer + Tech Lead |
| ToS/License change | Legal review em 48h; se incompatível, pivot imediato | 2 semanas | Tech Lead + Legal |
| Outage recorrente | Implementar failover automático + avaliar pivot permanente | 1 sprint | SRE + Tech Lead |
| Vendor adquirido | Monitorar 90 dias; se sinais negativos, pré-migrar | 90 dias | Tech Lead |
| Feature removida | Fork temporário + buscar alternativa definitiva | Variável | Dev Senior + Arquiteto |

### 3.3 Critérios de Entrada e Saída

**Entrada:** Pelo menos uma condição da §3.1 é verdadeira E avaliação inicial confirma que não há mitigação simples (config change, versão nova, workaround).

**Saída:** Pivot concluído quando TODOS forem verdadeiros:
- [ ] Alternativa implementada e passando todos os testes existentes
- [ ] Benchmarks da alternativa dentro de ±20% do baseline original
- [ ] Rollback procedure testada e documentada
- [ ] Documentação atualizada (runbooks, handoffs, ARCHITECTURE.md)
- [ ] Monitoring configurado para nova dependência
- [ ] Post-mortem do motivo do pivot publicado (se foi reativo)

---

## 4. Governança e Escalação

### 4.1 Níveis de Decisão

| Nível | Quem Decide | Exemplo | Tempo Máximo |
|-------|-------------|---------|--------------|
| L1 — Avaliação | Dev Senior | Criar spike, benchmarkar alternativa | 1 sprint |
| L2 — Aprovação | Tech Lead | Autorizar migração em staging | 48 horas |
| L3 — Produção | Tech Lead + SRE | Deploy em produção, descomissionar vendor antigo | 1 semana |
| L4 — Emergencial | Qualquer senior on-call | Hot-swap em caso de outage ativo do vendor | Imediato |

### 4.2 Protocolo de Escalação

1. **Vendor anuncia EOL** → Dev Senior cria issue `dependency-pivot` com timeline e alternativas mapeadas
2. **Spike falha em validar alternativa** → Escalar para Arquiteto; considerar fork ou redesign parcial
3. **Custo da alternativa excede budget** → Tech Lead negocia com Financeiro; se inviável, escalar para CTO
4. **Pivot em produção causa regressão** → Rollback imediato via L4; post-mortem obrigatório em 48h

### 4.3 Comunicação entre Equipes

- **Canal primário:** `#zylos-dependencies` no Slack (ou equivalente)
- **Tracking:** Issues GitHub com label `dependency-pivot` + milestone correspondente
- **Status updates:** Semanal durante migração ativa; mensal durante monitoring pós-pivot
- **Stakeholder updates:** Se pivot afeta UX ou pricing, PM comunica com 2 semanas de antecedência

---

## 5. Procedimento Padrão de Pivot (7 Etapas)

### Etapa 1: Mapeamento de Alternativas (2-5 dias)
- Listar ≥3 alternativas viáveis
- Avaliar cada uma contra critérios: maturidade, comunidade, license, performance, custo, compatibilidade com traits existentes
- Produzir matriz comparativa documentada

### Etapa 2: Spike de Validação (1 sprint)
- Implementar adapter mínimo para alternativa top-ranked
- Rodar subset de testes relevantes
- Benchmark comparativo vs. baseline
- Documento de findings com go/no-go recommendation

### Etapa 3: Trait Abstraction (se não existir)
- Criar ou refinar trait que abstraia vendor específico
- Implementar adapter para vendor atual E alternativa
- Testes de contrato garantem interchangeability
- **Nota:** Zylos já tem traits para VectorStore, LLM Provider, Search Provider; WASM runtime e frontend NÃO têm abstraction layer

### Etapa 4: Migração Completa (1-2 sprints)
- Implementar adapter completo para alternativa
- Migrar todos os call sites
- Atualizar config/env vars
- Testes de integração end-to-end

### Etapa 5: Parallel Run (1-2 semanas)
- Rodar ambos vendors em paralelo (shadow mode)
- Comparar outputs/resultados automaticamente
- Alertar em divergências >threshold definido
- Coletar métricas de performance reais

### Etapa 6: Cutover (dia D)
- Switch definitivo para nova alternativa
- Manter vendor antigo como fallback por 30 dias
- Monitoring intensivo por 7 dias
- Rollback plan pronto e testado

### Etapa 7: Decommission (após 30 dias)
- Remover código/config do vendor antigo
- Cancelar contrato/subscription
- Atualizar documentação (handoffs, runbooks, ARCHITECTURE.md)
- Publicar resumo do pivot em DECISIONS.md como ADR

---

## 6. Ferramentas e Sistemas Necessários

| Ferramenta | Uso | Acesso |
|------------|-----|--------|
| GitHub Issues | Tracking de pivots com label `dependency-pivot` | Todos devs |
| Cargo / crates.io | Avaliar maturidade e downloads de crates Rust | Todos devs |
| Benchmark harness (criterion) | Comparação de performance entre alternativas | Dev Senior |
| Feature flags | Shadow mode / gradual rollout durante parallel run | SRE + Dev Senior |
| Grafana / Axiom | Monitoring comparativo durante parallel run | SRE |
| Notion / Confluence | Documentação de matrizes comparativas e decisões | Todos |
| Slack / Teams | Comunicação de status e alertas | Todos |

---

## 7. Formato e Publicação

- **Formato:** Markdown versionado no repositório (`docs/playbooks/vendor-dependency-pivot.md`)
- **Atualização:** Após cada pivot real, adição de nova dependência crítica, ou quarterly review
- **Review trimestral:** Tech Lead revisa lista de dependências críticas e status dos planos B
- **Onboarding:** Novo dev senior deve ler este playbook e conhecer as alternativas mapeadas na primeira semana

---

## 8. Referências Cruzadas

- **Runbook relacionado:** `zylos-daemon.md` (restart após troca de dependency)
- **Playbook relacionado:** `security-incident-response.md` (se pivot for motivado por CVE)
- **Playbook relacionado:** `capacity-planning.md` (se pivot for motivado por custo/escala)
- **DECISIONS.md:** ADRs de escolhas de vendors/crates anteriores
- **ARCHITECTURE.md:** Traits de abstração existentes e gaps conhecidos
- **HANDOFF_*.md:** Lições aprendidas sobre dependências em cada fase
- **EVALUATION.md:** Métricas de baseline para comparação em pivots

---

**Próxima Revisão Agendada:** Fevereiro de 2027
**Owner:** Tech Lead Zylos-Next