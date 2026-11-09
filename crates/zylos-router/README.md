# zylos-router

**Fase:** 7B.2.1 — Padrões CORTEX
**Status:** ✅ Implementado (~100% do plano)

## Descrição

Roteamento aterrado com coleira (leash), alvos sagrados e proposta de mudança. Garante que agentes não desviem do escopo definido e nunca inventem capacidades.

## Tipos Principais

- `Capability` — Capacidade registrada com keywords, embedding e track-record
- `RouteResult` — Resultado de roteamento com score e tipo de match
- `ChangeProposal` — Proposta de mudança com risco e aprovação humana obrigatória
- `LeashConfig` — Configuração de distância máxima e alvos sagrados
- `RiskLevel` — Low/Medium/High/Critical

## Features Implementadas

- **GroundedRouter**: roteamento com keyword + semantic matching, fallback para `general-purpose` quando score=0
- **is_real()**: garantia dura que nunca inventa capacidade — verifica se capability_id está registrado
- **propose_change()**: com requires_human=true sempre, valida alvos sagrados e bloqueia capacidades inventadas
- **Alvos sagrados**: freios/verificadores com gate reforçado via `sacred_targets` HashSet
- **Desempate por track-record**: medido (feature flag opt-in via `use_track_record_tiebreak`)
- **route_com_proposta()**: com cluster fallbacks + dedup persistente, gera ChangeProposal quando threshold atingido
- **Channel Registry**: adapters para múltiplos canais de comunicação (Telegram, Slack, Discord, etc.)
- **Leash validation**: distância coseno máxima entre origem e alvo
- **Risk level inference**: Low/Medium/High/Critical baseado em keywords do alvo

## API Pública

```rust
pub struct GroundedRouter {
    config: LeashConfig,
    capabilities: Vec<Capability>,
    change_log: Vec<ChangeProposal>,
    fallback_clusters: HashMap<String, FallbackClusterEntry>,
    registered_ids: HashSet<String>,
}

impl GroundedRouter {
    pub fn new(config: LeashConfig) -> Self;
    pub fn add_capability(&mut self, cap: Capability);
    pub fn is_real(&self, capability_id: &str) -> bool;
    pub fn check_leash(&self, origin: &[f64], target: &[f64]) -> Result<f64, RouterError>;
    pub fn route(&self, query: &str) -> RouteResult;
    pub fn route_com_proposta(&mut self, query: &str) -> (RouteResult, Option<ChangeProposal>);
    pub fn propose_change(&mut self, original: &str, proposed: &str, justification: impl Into<String>, distance: f64) -> Result<ChangeProposal, RouterError>;
    pub fn change_log(&self) -> &[ChangeProposal];
    pub fn fallback_clusters(&self) -> &HashMap<String, FallbackClusterEntry>;
}

pub struct ChannelRegistry {
    adapters: HashMap<String, Box<dyn ChannelAdapter>>,
}

impl ChannelRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, name: &str, adapter: Box<dyn ChannelAdapter>);
    pub async fn send(&self, channel: &str, session_id: &str, message: OutboundMessage) -> Result<(), ChannelError>;
    pub async fn receive(&self, channel: &str) -> Result<Option<InboundMessage>, ChannelError>;
}
```

## Gaps vs Master Plan

Nenhum gap crítico — todas as features do plano foram implementadas.

## Runbook SOAR

[`cortex-patterns.runbook.yaml`](../../docs/soar/runbooks/cortex-patterns.runbook.yaml)

## Fonte Original

`~/.claude/skills/orquestrar/scripts/router.mjs` (140 linhas)
