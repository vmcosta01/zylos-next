# Plano de Implementação Detalhado — Zylos Cockpit v2

**Versão:** 2.0.0
**Data:** 2026-11-11
**Base:** Análise profunda de 12 dashboards open-source (OpenBot, OpenDots, OpenMuse, Paperclip, Rerun, Ruflo, ECC, AnyModelBot, OpenDuet, OpenMausBot, agency-agents, awesome-grok-bot)
**Repositórios de referência:** `C:/Users/Paulo/dashboard-analysis/` (mantidos para consulta contínua)

---

## Índice

1. [Visão Geral e Princípios](#1-visão-geral-e-princípios)
2. [Fase 1: Segurança e Controle (CRÍTICO)](#2-fase-1-segurança-e-controle-crítico)
3. [Fase 2: Produtividade (IMPORTANTE)](#3-fase-2-produtividade-importante)
4. [Fase 3: Colaboração (DESEJÁVEL)](#4-fase-3-colaboração-desejável)
5. [Fase 4: Avançado (DESEJÁVEL BAIXO)](#5-fase-4-avançado-desejável-baixo)
6. [Arquitetura Transversal](#6-arquitetura-transversal)
7. [Testes e Qualidade](#7-testes-e-qualidade)
8. [Timeline e Recursos](#8-timeline-e-recursos)
9. [Riscos e Mitigações](#9-riscos-e-mitigações)
10. [Decisões Arquiteturais (ADRs)](#10-decisões-arquiteturais-adrs)

---

## 1. Visão Geral e Princípios

### 1.1 Camadas de Desenvolvimento com IA

Cada tarefa segue o padrão de 7 camadas, inspirado no workflow de OpenBot e Paperclip:

```
┌─────────────────────────────────────┐
│ L7: Deploy & Observabilidade       │  Metrics, logs, dashboards
├─────────────────────────────────────┤
│ L6: Integração E2E                 │  Cockpit ↔ Gateway ↔ Hermes
├─────────────────────────────────────┤
│ L5: Componentes WASM (Leptos)      │  UI interativa
├─────────────────────────────────────┤
│ L4: Design System (CSS Tokens)     │  Cores, tipografia, spacing
├─────────────────────────────────────┤
│ L3: Contratos de API (OpenAPI)     │  Schemas, endpoints, erros
├─────────────────────────────────────┤
│ L2: Lógica de Negócio (Rust)       │  Crates, handlers, state
├─────────────────────────────────────┤
│ L1: Schemas de Dados (SQL/Rust)    │  Tabelas, structs, migrations
└─────────────────────────────────────┘
```

### 1.2 Princípios de Design

1. **Approval-first**: Toda ação externa (enviar mensagem, fazer deploy, pagamento) exige confirmação humana com 3 opções (once/always/refuse)
2. **Progressive disclosure**: Interface simples por padrão, poderosa quando necessário
3. **Agent identity**: Cada bot tem avatar, cor, nome, provedor e budget individual
4. **Offline resilience**: Queue local quando gateway indisponível, replay ao reconectar
5. **Accessibility by default**: WCAG AA, foco visível, navegação por teclado, reduced motion

### 1.3 Stack Tecnológico

| Camada | Tecnologia | Versão |
|--------|-----------|--------|
| Frontend | Leptos (WASM) | 0.7+ |
| CSS | Design tokens custom | v2.0 |
| Backend | Axum (Rust) | 0.7 |
| State | Reactive signals | Leptos built-in |
| API | REST + SSE | OpenAPI 3.1 |
| Database | SQLite (WAL) | via sqlx |
| Testing | wasm-bindgen-test + cargo test | — |

---

## 2. Fase 1: Segurança e Controle (CRÍTICO)

**Duração estimada:** 2 semanas (10 dias úteis)
**Objetivo:** Eliminar gaps críticos de segurança identificados em todos os dashboards de referência

### 2.1 Task 1.1: Approval Gate Cards

**Gap:** Zylos não tem cards HITL no chat com allow-once/always/refuse para ações externas
**Fontes:** OpenBot (`ApprovalBlock.tsx`), OpenDots (`PageReviewCard.tsx`), OpenDuet (`approvals/service.ts`), AnyModelBot (`cu-permission.js`), Rerun (`ApprovalBlock.tsx`)

#### 2.1.1 Schema de Dados (L1)

**Arquivo:** `crates/zylos-approval/src/schema.rs` (novo crate)

```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Decisão do usuário para uma ação que requer aprovação
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Aprova apenas esta instância
    AllowOnce,
    /// Aprova para esta sessão (até fechar o cockpit)
    AllowSession,
    /// Aprova permanentemente para este bot + ferramenta
    AllowAlways,
    /// Recusa esta instância (pode pedir motivo)
    Refuse,
}

/// Estado de uma requisição de aprovação
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Refused,
    Expired,
    Cancelled,
}

/// Categoria de ação que determina se precisa de approval
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum ActionCategory {
    /// Envia mensagem para pessoa externa (sempre pede approval)
    SendMessage,
    /// Faz deploy/mutação de infraestrutura (sempre pede)
    Deploy,
    /// Faz pagamento/transferência financeira (sempre pede)
    Payment,
    /// Assina documento legal (sempre pede)
    SignDocument,
    /// Deleta recurso (pede se for produção)
    DeleteResource,
    /// Lê dados sensíveis (pede se PII)
    ReadSensitive,
    /// Ação de baixo risco (não pede por padrão)
    LowRisk,
}

/// Uma requisição de aprovação pendente
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApprovalRequest {
    pub id: String,
    pub bot_id: String,
    pub tool_name: String,
    pub action_category: ActionCategory,
    pub summary: String,           // "Send email to client@example.com"
    pub arguments_json: String,    // JSON com args exatos
    pub status: ApprovalStatus,
    pub decision: Option<ApprovalDecision>,
    pub refuse_reason: Option<String>,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub expires_at: Option<String>,
    pub receipt_json: Option<String>,  // Para sobreviver a refresh
}

/// Regra de approval persistente (allow-always)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApprovalRule {
    pub id: String,
    pub bot_id: String,
    pub tool_name: String,
    pub action_category: ActionCategory,
    pub arguments_pattern: Option<String>,  // Regex para filtrar args
    pub decision: ApprovalDecision,
    pub created_at: String,
    pub created_by: String,  // "user" ou "system"
}
```

**Migração SQL:** `crates/zylos-approval/migrations/001_initial.sql`

```sql
CREATE TABLE IF NOT EXISTS approval_requests (
    id TEXT PRIMARY KEY,
    bot_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    action_category TEXT NOT NULL,
    summary TEXT NOT NULL,
    arguments_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    decision TEXT,
    refuse_reason TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    decided_at TEXT,
    expires_at TEXT,
    receipt_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_approval_bot_status 
    ON approval_requests(bot_id, status);
CREATE INDEX IF NOT EXISTS idx_approval_created 
    ON approval_requests(created_at);
CREATE INDEX IF NOT EXISTS idx_approval_pending 
    ON approval_requests(status) 
    WHERE status = 'pending';

CREATE TABLE IF NOT EXISTS approval_rules (
    id TEXT PRIMARY KEY,
    bot_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    action_category TEXT NOT NULL,
    arguments_pattern TEXT,
    decision TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    created_by TEXT NOT NULL DEFAULT 'user',
    UNIQUE(bot_id, tool_name, action_category, arguments_pattern)
);

CREATE INDEX IF NOT EXISTS idx_approval_rules_lookup 
    ON approval_rules(bot_id, tool_name, action_category);
```

#### 2.1.2 Contratos de API (L3)

**Arquivo:** `apps/zylos-gateway/src/approval_api.rs` (novo)

```rust
//! Endpoints de approval HITL
//!
//! - `GET /api/v1/approvals/pending` — lista aprovações pendentes do usuário
//! - `GET /api/v1/approvals/:id` — detalhes de uma aprovação específica
//! - `POST /api/v1/approvals/:id/decide` — registra decisão do usuário
//! - `GET /api/v1/approvals/rules` — lista regras persistentes (allow-always)
//! - `DELETE /api/v1/approvals/rules/:id` — revoga uma regra

use axum::{Json, Router, extract::{Path, State}, routing::{get, post, delete}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct DecideRequest {
    pub decision: ApprovalDecision,
    pub refuse_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DecideResponse {
    pub id: String,
    pub status: ApprovalStatus,
    pub decided_at: String,
}

/// Router de approvals
pub fn create_approval_router(state: ApprovalState) -> Router {
    Router::new()
        .route("/approvals/pending", get(get_pending))
        .route("/approvals/:id", get(get_approval))
        .route("/approvals/:id/decide", post(decide_approval))
        .route("/approvals/rules", get(list_rules))
        .route("/approvals/rules/:id", delete(delete_rule))
        .with_state(state)
}

// Handler: POST /api/v1/approvals/:id/decide
async fn decide_approval(
    State(state): State<ApprovalState>,
    Path(id): Path<String>,
    Json(req): Json<DecideRequest>,
) -> Result<Json<DecideResponse>, ApprovalError> {
    // 1. Buscar request pendente
    let request = state.store.get_pending(&id).await?;
    
    // 2. Validar não expirou
    if request.is_expired() {
        return Err(ApprovalError::Expired);
    }
    
    // 3. Persistir decisão
    let decided_at = state.store.decide(&id, req.decision, req.refuse_reason).await?;
    
    // 4. Se allow-always, criar regra persistente
    if req.decision == ApprovalDecision::AllowAlways {
        state.store.create_rule(&request).await?;
    }
    
    // 5. Notificar agent via EventBus
    state.event_bus.publish(Event::ApprovalDecided {
        request_id: id.clone(),
        decision: req.decision,
    }).await;
    
    // 6. Gerar receipt para sobrevivência a refresh
    let receipt = Receipt::new(&request, req.decision, decided_at.clone());
    state.store.save_receipt(&id, &receipt).await?;
    
    Ok(Json(DecideResponse {
        id,
        status: ApprovalStatus::Approved,
        decided_at,
    }))
}
```

**OpenAPI Spec:** `docs/api/approval.yaml`

```yaml
openapi: 3.1.0
info:
  title: Zylos Approval API
  version: 1.0.0
paths:
  /api/v1/approvals/pending:
    get:
      summary: Lista aprovações pendentes
      responses:
        200:
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/ApprovalRequest'
  /api/v1/approvals/{id}/decide:
    post:
      summary: Registra decisão do usuário
      parameters:
        - name: id
          in: path
          required: true
          schema: { type: string }
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/DecideRequest'
      responses:
        200:
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/DecideResponse'
        410:
          description: Request expirado
components:
  schemas:
    ApprovalDecision:
      type: string
      enum: [allow_once, allow_session, allow_always, refuse]
    DecideRequest:
      type: object
      required: [decision]
      properties:
        decision:
          $ref: '#/components/schemas/ApprovalDecision'
        refuse_reason:
          type: string
          description: Obrigatório se decision=refuse
```

#### 2.1.3 Componente WASM (L5)

**Arquivo:** `apps/zylos-cockpit/src/components/approval_card.rs` (novo)

```rust
//! ApprovalCard — componente HITL inline no chat
//! 
//! Inspirado em:
//! - OpenBot: ApprovalBlock.tsx (granular por ferramenta)
//! - OpenDots: PageReviewCard.tsx (com receipt persistence)
//! - OpenDuet: approvals/service.ts (allow-once/always/refuse)
//! - AnyModelBot: cu-permission.js (permissões granulares)

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovalCardProps {
    pub id: String,
    pub bot_name: String,
    pub tool_name: String,
    pub summary: String,
    pub arguments: serde_json::Value,
    pub created_at: String,
    pub status: String,  // pending/approved/refused/expired
}

#[component]
pub fn ApprovalCard(props: ApprovalCardProps) -> impl IntoView {
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (decided, set_decided) = signal(props.status != "pending");
    
    let handle_decide = move |decision: &str| {
        if busy.get() { return; }
        set_busy.set(true);
        set_error.set(None);
        
        let id = props.id.clone();
        let decision = decision.to_string();
        
        spawn_local(async move {
            let url = format!("http://localhost:3000/api/v1/approvals/{}/decide", id);
            let body = serde_json::json!({
                "decision": decision,
                "refuse_reason": if decision == "refuse" {
                    Some("Usuário recusou".to_string())
                } else { None }
            });
            
            match gloo_net::http::Request::post(&url)
                .header("Content-Type", "application/json")
                .body(serde_json::to_string(&body).unwrap())
                .unwrap()
                .send()
                .await
            {
                Ok(resp) if resp.ok() => {
                    set_decided.set(true);
                    // Dispatch evento para o chat atualizar
                    let _ = web_sys::window()
                        .unwrap()
                        .dispatch_event(&web_sys::CustomEvent::new("approval-decided").unwrap());
                }
                Ok(resp) => {
                    let text = resp.text().await.unwrap_or_default();
                    set_error.set(Some(format!("Erro {}: {}", resp.status(), text)));
                }
                Err(e) => {
                    set_error.set(Some(format!("Falha de rede: {}", e)));
                }
            }
            set_busy.set(false);
        });
    };
    
    view! {
        <div class="approval-card" data-approval-id=props.id.clone()>
            <div class="approval-card-header">
                <span class="approval-card-icon">"⚠️"</span>
                <span class="approval-card-title">"Aprovação Necessária"</span>
                <span class="approval-card-bot">{props.bot_name.clone()}</span>
            </div>
            
            <div class="approval-card-body">
                <p class="approval-card-summary">{props.summary.clone()}</p>
                
                <details class="approval-card-args">
                    <summary>"Ver argumentos exatos"</summary>
                    <pre class="approval-card-args-pre">
                        {serde_json::to_string_pretty(&props.arguments).unwrap_or_default()}
                    </pre>
                </details>
            </div>
            
            <Show when=move || !decided.get()>
                <div class="approval-card-actions">
                    <button
                        class="btn btn-success"
                        on:click=move |_| handle_decide("allow_once")
                        disabled=move || busy.get()
                        title="Aprovar apenas esta vez"
                    >"Permitir uma vez"</button>
                    
                    <button
                        class="btn btn-primary"
                        on:click=move |_| handle_decide("allow_always")
                        disabled=move || busy.get()
                        title="Aprovar sempre para este bot + ferramenta"
                    >"Sempre permitir"</button>
                    
                    <button
                        class="btn btn-danger"
                        on:click=move |_| handle_decide("refuse")
                        disabled=move || busy.get()
                        title="Recusar esta ação"
                    >"Recusar"</button>
                </div>
            </Show>
            
            <Show when=move || decided.get()>
                <div class="approval-card-decided">
                    <span class="approval-card-status">
                        {move || match props.status.as_str() {
                            "approved" => "✅ Aprovado",
                            "refused" => "❌ Recusado",
                            "expired" => "⏰ Expirado",
                            _ => "Processado",
                        }}
                    </span>
                </div>
            </Show>
            
            <Show when=move || error.get().is_some()>
                <div class="approval-card-error">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            
            <Show when=move || busy.get()>
                <div class="approval-card-loading">
                    <span class="spinner"></span>
                    "Processando..."
                </div>
            </Show>
        </div>
    }
}
```

**CSS:** Adicionar em `design-tokens.css`

```css
/* === Approval Card (inspirado em OpenBot + OpenDuet) === */
.approval-card {
    background: var(--bg-elevated);
    border: 2px solid var(--status-warning);
    border-radius: var(--radius-md);
    padding: var(--space-4);
    margin: var(--space-3) 0;
    max-width: 85%;
    align-self: flex-start;
    animation: approval-in 200ms cubic-bezier(0.4, 0, 0.2, 1);
}

.approval-card-header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--status-warning);
    margin-bottom: var(--space-3);
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border-default);
}

.approval-card-icon {
    font-size: 18px;
}

.approval-card-bot {
    margin-left: auto;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    background: var(--bg-surface);
    padding: 2px 8px;
    border-radius: var(--radius-full);
}

.approval-card-body {
    margin-bottom: var(--space-3);
}

.approval-card-summary {
    font-size: var(--text-base);
    color: var(--text-primary);
    margin-bottom: var(--space-2);
    line-height: var(--leading-base);
}

.approval-card-args {
    font-size: var(--text-xs);
}

.approval-card-args summary {
    cursor: pointer;
    color: var(--text-secondary);
    user-select: none;
}

.approval-card-args-pre {
    margin-top: var(--space-2);
    padding: var(--space-2);
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    overflow-x: auto;
    max-height: 200px;
    overflow-y: auto;
    color: var(--text-primary);
}

.approval-card-actions {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
    justify-content: flex-end;
}

.approval-card-decided {
    padding: var(--space-2) var(--space-3);
    background: var(--bg-surface);
    border-radius: var(--radius-sm);
    text-align: center;
    font-size: var(--text-sm);
    color: var(--text-secondary);
}

.approval-card-error {
    margin-top: var(--space-2);
    padding: var(--space-2);
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid var(--status-error);
    border-radius: var(--radius-sm);
    font-size: var(--text-xs);
    color: var(--status-error);
}

.approval-card-loading {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    justify-content: center;
    padding: var(--space-2);
    color: var(--text-secondary);
    font-size: var(--text-sm);
}

.spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--border-default);
    border-top-color: var(--bg-accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
}

@keyframes spin {
    to { transform: rotate(360deg); }
}

@keyframes approval-in {
    from {
        opacity: 0;
        transform: translateY(8px) scale(0.98);
    }
    to {
        opacity: 1;
        transform: translateY(0) scale(1);
    }
}

/* Botões específicos para approval */
.btn-success {
    background: var(--status-success);
    color: white;
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    font-weight: 500;
    border: none;
    cursor: pointer;
    transition: all var(--transition-fast);
}

.btn-success:hover:not(:disabled) {
    opacity: 0.9;
    transform: translateY(-1px);
}

.btn-success:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}

@media (prefers-reduced-motion: reduce) {
    .approval-card,
    .spinner {
        animation: none !important;
    }
}
```

#### 2.1.4 Integração no ChatPanel (L6)

Modificar `apps/zylos-cockpit/src/components/chat_panel.rs`:

```rust
// Adicionar import
use crate::components::approval_card::ApprovalCard;

// Na renderização de mensagens, detectar approval cards:
// Adicionar após renderização de mensagens normais:

{move || {
    // Buscar approvals pendentes via SSE ou polling
    let pending_approvals = pending_approvals_signal.get();
    pending_approvals.iter().map(|approval| {
        view! {
            <ApprovalCard
                id=approval.id.clone()
                bot_name=approval.bot_name.clone()
                tool_name=approval.tool_name.clone()
                summary=approval.summary.clone()
                arguments=approval.arguments.clone()
                created_at=approval.created_at.clone()
                status=approval.status.clone()
            />
        }
    }).collect_view()
}}
```

#### 2.1.5 Testes (L7)

**Testes unitários:** `crates/zylos-approval/src/tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;
    
    #[sqlx::test]
    async fn test_create_and_decide_approval(pool: SqlitePool) {
        let store = ApprovalStore::new(pool).await.unwrap();
        
        // Criar request
        let req = ApprovalRequest {
            bot_id: "bot-1".into(),
            tool_name: "send_email".into(),
            action_category: ActionCategory::SendMessage,
            summary: "Send email to client@example.com".into(),
            arguments_json: r#"{"to":"client@example.com"}"#.into(),
            ..Default::default()
        };
        let id = store.create(req).await.unwrap();
        
        // Verificar pendente
        let pending = store.get_pending(&id).await.unwrap();
        assert_eq!(pending.status, ApprovalStatus::Pending);
        
        // Decidir
        store.decide(&id, ApprovalDecision::AllowOnce, None).await.unwrap();
        
        // Verificar aprovado
        let decided = store.get(&id).await.unwrap();
        assert_eq!(decided.status, ApprovalStatus::Approved);
        assert_eq!(decided.decision, Some(ApprovalDecision::AllowOnce));
    }
    
    #[sqlx::test]
    async fn test_allow_always_creates_rule(pool: SqlitePool) {
        let store = ApprovalStore::new(pool).await.unwrap();
        
        let req = ApprovalRequest {
            bot_id: "bot-1".into(),
            tool_name: "send_email".into(),
            action_category: ActionCategory::SendMessage,
            ..Default::default()
        };
        let id = store.create(req.clone()).await.unwrap();
        
        store.decide(&id, ApprovalDecision::AllowAlways, None).await.unwrap();
        
        // Verificar regra criada
        let rules = store.list_rules("bot-1").await.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].tool_name, "send_email");
        assert_eq!(rules[0].decision, ApprovalDecision::AllowAlways);
    }
    
    #[sqlx::test]
    async fn test_expired_approval_rejected(pool: SqlitePool) {
        let store = ApprovalStore::new(pool).await.unwrap();
        
        let req = ApprovalRequest {
            expires_at: Some("2020-01-01T00:00:00Z".into()),
            ..Default::default()
        };
        let id = store.create(req).await.unwrap();
        
        let result = store.decide(&id, ApprovalDecision::AllowOnce, None).await;
        assert!(matches!(result, Err(ApprovalError::Expired)));
    }
}
```

**Testes E2E:** `apps/zylos-cockpit/tests/e2e/approval.rs`

```rust
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn test_approval_card_render() {
    let props = ApprovalCardProps {
        id: "test-1".into(),
        bot_name: "TestBot".into(),
        tool_name: "send_email".into(),
        summary: "Send email to test@example.com".into(),
        arguments: serde_json::json!({"to": "test@example.com"}),
        created_at: "2026-11-11T10:00:00Z".into(),
        status: "pending".into(),
    };
    
    let document = leptos::mount_to_document(move || view! { <ApprovalCard ..props /> });
    
    // Aguardar render
    gloo_timers::future::sleep(std::time::Duration::from_millis(100)).await;
    
    // Verificar elementos
    assert!(document.query_selector(".approval-card").unwrap().is_some());
    assert!(document.query_selector(".btn-success").unwrap().is_some());
    assert!(document.query_selector(".btn-danger").unwrap().is_some());
}
```

#### 2.1.6 Critérios de Aceite

- [ ] Cards de approval aparecem inline no chat quando há ações pendentes
- [ ] 3 botões funcionam: Permitir uma vez, Sempre permitir, Recusar
- [ ] Decisão persiste no SQLite e sobrevive a refresh (receipt)
- [ ] "Sempre permitir" cria regra reutilizável para bot+ferramenta
- [ ] Arguments exatos são mostrados (não os do modelo, como no OpenDots)
- [ ] Estados visuais: pending, approved, refused, expired, loading, error
- [ ] Animação suave de entrada (200ms cubic-bezier)
- [ ] Acessibilidade: aria-labels, foco visível, navegação por teclado
- [ ] Reduced motion respeitado
- [ ] Testes unitários passam (coverage > 80%)
- [ ] Testes E2E passam em Chrome + Firefox

---

### 2.2 Task 1.2: Egress Checker

**Gap:** Zylos não verifica PII/secrets antes de enviar mensagens externas
**Fonte:** OpenDuet (`egress.ts`)

#### 2.2.1 Schema de Dados (L1)

**Arquivo:** `crates/zylos-egress/src/schema.rs` (novo crate)

```rust
//! Verificador de egress — detecta PII, secrets e dados sensíveis
//! antes de enviar para destinos externos.

use serde::{Deserialize, Serialize};
use regex::Regex;

/// Categoria de dado sensível detectado
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensitiveCategory {
    /// Email addresses
    Email,
    /// Phone numbers (BR/US/international)
    Phone,
    /// Credit card numbers (Luhn validated)
    CreditCard,
    /// CPF/CNPJ (Brazilian tax IDs)
    BrazilianTaxId,
    /// API keys, tokens, passwords
    Secret,
    /// Health information keywords
    HealthData,
    /// Financial account numbers
    FinancialAccount,
    /// IP addresses
    IpAddress,
    /// Custom patterns defined by user
    Custom,
}

/// Uma detecção de dado sensível
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveFinding {
    pub category: SensitiveCategory,
    pub matched_text: String,      // Texto original encontrado
    pub redacted_text: String,     // Versão redigida (ex: "***@***.com")
    pub start: usize,              // Posição inicial no texto
    pub end: usize,                // Posição final no texto
    pub confidence: f32,           // 0.0-1.0
    pub rule_name: String,         // Nome da regra que detectou
}

/// Resultado de uma verificação de egress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressCheckResult {
    pub passed: bool,              // true se não há findings bloqueantes
    pub findings: Vec<SensitiveFinding>,
    pub redacted_text: Option<String>,  // Texto com findings redigidos
    pub blocked_categories: Vec<SensitiveCategory>,  // Categorias que bloqueiam
}

/// Regra customizada de detecção
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressRule {
    pub id: String,
    pub name: String,
    pub pattern: String,           // Regex
    pub category: SensitiveCategory,
    pub blocks: bool,              // Se true, bloqueia; se false, só avisa
    pub enabled: bool,
}
```

#### 2.2.2 Lógica de Negócio (L2)

**Arquivo:** `crates/zylos-egress/src/checker.rs`

```rust
use crate::schema::*;
use regex::Regex;
use std::sync::LazyLock;

/// Patterns built-in (não customizáveis)
static EMAIL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap()
});

static PHONE_BR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // +55 11 99999-9999, (11) 99999-9999, 11 99999-9999
    Regex::new(r"(?:\+55\s?)?(?:\(?\d{2}\)?\s?)?\d{4,5}[-\s]?\d{4}").unwrap()
});

static CPF_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\d{3}\.\d{3}\.\d{3}-\d{2}").unwrap()
});

static CNPJ_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\d{2}\.\d{3}\.\d{3}/\d{4}-\d{2}").unwrap()
});

static CREDIT_CARD_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Visa, Mastercard, Amex, etc. com validação Luhn
    Regex::new(r"\b(?:\d[ -]*?){13,16}\b").unwrap()
});

static SECRET_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)(api[_-]?key|secret|token|password|passwd)\s*[:=]\s*['\"]?([a-zA-Z0-9_\-]{16,})['\"]?").unwrap(),
        Regex::new(r"sk-[a-zA-Z0-9]{32,}").unwrap(),  // OpenAI
        Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(),  // GitHub PAT
        Regex::new(r"xox[baprs]-[a-zA-Z0-9\-]+").unwrap(),  // Slack
        Regex::new(r"AIza[a-zA-Z0-9_\-]{35}").unwrap(),  // Google API
    ]
});

static HEALTH_KEYWORDS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    vec![
        "diagnóstico", "diagnóstico", "prescrição", "medicamento",
        "doença", "tratamento", "hospital", "cirurgia", "exame",
        "sangue", "pressão arterial", "glicose", "colesterol"
    ]
});

pub struct EgressChecker {
    custom_rules: Vec<EgressRule>,
    blocked_categories: Vec<SensitiveCategory>,
}

impl EgressChecker {
    pub fn new() -> Self {
        Self {
            custom_rules: vec![],
            blocked_categories: vec![
                SensitiveCategory::Secret,
                SensitiveCategory::CreditCard,
            ],
        }
    }
    
    pub fn with_custom_rules(mut self, rules: Vec<EgressRule>) -> Self {
        self.custom_rules = rules.into_iter().filter(|r| r.enabled).collect();
        self
    }
    
    pub fn with_blocked_categories(mut self, cats: Vec<SensitiveCategory>) -> Self {
        self.blocked_categories = cats;
        self
    }
    
    /// Verifica texto e retorna findings + versão redigida
    pub fn check(&self, text: &str) -> EgressCheckResult {
        let mut findings = Vec::new();
        
        // Emails
        for m in EMAIL_PATTERN.find_iter(text) {
            findings.push(SensitiveFinding {
                category: SensitiveCategory::Email,
                matched_text: m.as_str().to_string(),
                redacted_text: redact_email(m.as_str()),
                start: m.start(),
                end: m.end(),
                confidence: 0.95,
                rule_name: "email_builtin".to_string(),
            });
        }
        
        // Telefones BR
        for m in PHONE_BR_PATTERN.find_iter(text) {
            if m.as_str().len() >= 10 {  // Evita falsos positivos curtos
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::Phone,
                    matched_text: m.as_str().to_string(),
                    redacted_text: redact_phone(m.as_str()),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.85,
                    rule_name: "phone_br_builtin".to_string(),
                });
            }
        }
        
        // CPF/CNPJ
        for m in CPF_PATTERN.find_iter(text) {
            if validate_cpf(m.as_str()) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::BrazilianTaxId,
                    matched_text: m.as_str().to_string(),
                    redacted_text: "***.***.***-**".to_string(),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.99,
                    rule_name: "cpf_builtin".to_string(),
                });
            }
        }
        
        for m in CNPJ_PATTERN.find_iter(text) {
            if validate_cnpj(m.as_str()) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::BrazilianTaxId,
                    matched_text: m.as_str().to_string(),
                    redacted_text: "**.***.***/****-**".to_string(),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.99,
                    rule_name: "cnpj_builtin".to_string(),
                });
            }
        }
        
        // Credit cards (com validação Luhn)
        for m in CREDIT_CARD_PATTERN.find_iter(text) {
            let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
            if luhn_check(&digits) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::CreditCard,
                    matched_text: m.as_str().to_string(),
                    redacted_text: redact_credit_card(m.as_str()),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.98,
                    rule_name: "credit_card_builtin".to_string(),
                });
            }
        }
        
        // Secrets
        for pattern in SECRET_PATTERNS.iter() {
            for m in pattern.find_iter(text) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::Secret,
                    matched_text: m.as_str().to_string(),
                    redacted_text: "[SECRET REDACTED]".to_string(),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.95,
                    rule_name: "secret_builtin".to_string(),
                });
            }
        }
        
        // Health keywords
        let text_lower = text.to_lowercase();
        for keyword in HEALTH_KEYWORDS.iter() {
            if text_lower.contains(keyword) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::HealthData,
                    matched_text: keyword.to_string(),
                    redacted_text: "[HEALTH DATA]".to_string(),
                    start: text_lower.find(keyword).unwrap(),
                    end: text_lower.find(keyword).unwrap() + keyword.len(),
                    confidence: 0.6,  // Baixa confiança, só aviso
                    rule_name: "health_keyword_builtin".to_string(),
                });
            }
        }
        
        // Custom rules
        for rule in &self.custom_rules {
            if let Ok(pattern) = Regex::new(&rule.pattern) {
                for m in pattern.find_iter(text) {
                    findings.push(SensitiveFinding {
                        category: rule.category,
                        matched_text: m.as_str().to_string(),
                        redacted_text: format!("[{} REDACTED]", rule.name),
                        start: m.start(),
                        end: m.end(),
                        confidence: 0.9,
                        rule_name: rule.name.clone(),
                    });
                }
            }
        }

        // Ordenar por posição
        findings.sort_by_key(|f| f.start);

        // Gerar texto redigido
        let redacted_text = if !findings.is_empty() {
            let mut result = text.to_string();
            let mut offset: isize = 0;
            for finding in &findings {
                let start = (finding.start as isize + offset) as usize;
                let end = (finding.end as isize + offset) as usize;
                let original_len = end - start;
                result.replace_range(start..end, &finding.redacted_text);
                offset += finding.redacted_text.len() as isize - original_len as isize;
            }
            Some(result)
        } else {
            None
        };

        let blocked_categories: Vec<SensitiveCategory> = findings.iter()
            .filter(|f| self.blocked_categories.contains(&f.category))
            .map(|f| f.category)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        EgressCheckResult {
            passed: blocked_categories.is_empty(),
            findings,
            redacted_text,
            blocked_categories,
        }
    }
}

// === Funções auxiliares ===

fn redact_email(email: &str) -> String {
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() == 2 {
        format!("***@{}", parts[1])
    } else {
        "***@***.com".to_string()
    }
}

fn redact_phone(phone: &str) -> String {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 4 {
        format!("(**) *****-{}", &digits[digits.len()-4..])
    } else {
        "(**) ****-****".to_string()
    }
}

fn redact_credit_card(cc: &str) -> String {
    let digits: String = cc.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 4 {
        format!("****-****-****-{}", &digits[digits.len()-4..])
    } else {
        "****-****-****-****".to_string()
    }
}

fn luhn_check(number: &str) -> bool {
    let digits: Vec<u32> = number.chars()
        .filter_map(|c| c.to_digit(10))
        .collect();
    if digits.len() < 13 { return false; }
    
    let sum: u32 = digits.iter().rev().enumerate().map(|(i, &d)| {
        if i % 2 == 1 {
            let doubled = d * 2;
            if doubled > 9 { doubled - 9 } else { doubled }
        } else {
            d
        }
    }).sum();
    
    sum % 10 == 0
}

fn validate_cpf(cpf: &str) -> bool {
    let digits: Vec<u32> = cpf.chars()
        .filter_map(|c| c.to_digit(10))
        .collect();
    if digits.len() != 11 { return false; }
    if digits.iter().all(|&d| d == digits[0]) { return false; }
    
    let calc_digit = |pos: usize| -> u32 {
        let sum: u32 = digits[..pos].iter().enumerate()
            .map(|(i, &d)| d * (pos + 1 - i) as u32)
            .sum();
        let remainder = sum % 11;
        if remainder < 2 { 0 } else { 11 - remainder }
    };
    
    digits[9] == calc_digit(9) && digits[10] == calc_digit(10)
}

fn validate_cnpj(cnpj: &str) -> bool {
    let digits: Vec<u32> = cnpj.chars()
        .filter_map(|c| c.to_digit(10))
        .collect();
    if digits.len() != 14 { return false; }
    if digits.iter().all(|&d| d == digits[0]) { return false; }
    true // Validação simplificada; produção deve usar algoritmo completo
}
```

#### 2.2.3 Contratos de API (L3)

**Arquivo:** `apps/zylos-gateway/src/egress_api.rs` (novo)

```rust
//! Endpoints de verificação de egress
//!
//! - `POST /api/v1/egress/check` — verifica texto antes de enviar
//! - `GET /api/v1/egress/rules` — lista regras customizadas
//! - `POST /api/v1/egress/rules` — cria regra customizada
//! - `DELETE /api/v1/egress/rules/:id` — remove regra

use axum::{Json, Router, extract::State, routing::{get, post, delete}};

pub fn create_egress_router(state: EgressState) -> Router {
    Router::new()
        .route("/egress/check", post(check_egress))
        .route("/egress/rules", get(list_rules).post(create_rule))
        .route("/egress/rules/:id", delete(delete_rule))
        .with_state(state)
}

async fn check_egress(
    State(state): State<EgressState>,
    Json(req): Json<EgressCheckRequest>,
) -> Json<EgressCheckResult> {
    Json(state.checker.check(&req.text))
}
```

#### 2.2.4 Integração no ChatPanel (L6)

No `chat_panel.rs`, antes de enviar mensagem via SSE/WebSocket:

```rust
let handle_send = move |text: String| {
    spawn_local(async move {
        // 1. Verificar egress ANTES de enviar
        let check_url = "http://localhost:3000/api/v1/egress/check";
        let check_body = serde_json::json!({ "text": &text });
        
        match gloo_net::http::Request::post(check_url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&check_body).unwrap())
            .unwrap()
            .send()
            .await
        {
            Ok(resp) if resp.ok() => {
                let result: EgressCheckResult = resp.json().await.unwrap();
                if !result.passed {
                    // Mostrar modal de confirmação com findings
                    set_egress_warning.set(Some(result));
                    return; // Não envia ainda
                }
                // Se passou, envia normalmente
                send_message_to_agent(text).await;
            }
            _ => {
                // Fallback: envia sem verificação se endpoint indisponível
                send_message_to_agent(text).await;
            }
        }
    });
};
```

#### 2.2.5 Critérios de Aceite

- [ ] Detecta emails, telefones BR, CPF/CNPJ, cartões de crédito, secrets/API keys
- [ ] Validação Luhn para cartões de crédito (evita falsos positivos)
- [ ] Validação de dígitos verificadores para CPF/CNPJ
- [ ] Regras customizáveis via API REST
- [ ] Texto redigido gerado automaticamente
- [ ] Modal de confirmação no cockpit quando findings bloqueantes encontrados
- [ ] Categorias configuráveis como block vs warn
- [ ] Health keywords com baixa confiança (só aviso)
- [ ] Testes unitários cobrem todos os patterns built-in
- [ ] Performance < 5ms para textos de até 10KB

---

### 2.3 Task 1.3: Password Masking

**Gap:** Zylos não mascara senhas digitadas no chat que vão para o transcript do agente
**Fonte:** OpenDuet (`passwords/resume.ts`)

#### 2.3.1 Implementação (L2 + L5)

**Backend:** Middleware no gateway que detecta campos `password`/`senha`/`secret` em JSON payloads e substitui por `"********"` antes de persistir no transcript.

**Frontend:** Input type="password" no composer quando contexto indica login/auth. Toggle visual "🔒" que ativa modo seguro.

#### 2.3.2 Critérios de Aceite

- [ ] Senhas nunca aparecem em plaintext no transcript
- [ ] Toggle visual claro quando modo seguro está ativo
- [ ] Detecção automática de campos de senha em formulários
- [ ] Audit log registra que senha foi mascarada (sem valor)

---

### 2.4 Task 1.4: Action Audit Log

**Gap:** Zylos não tem registro centralizado de todas as ações externas com approval status
**Fonte:** Rerun (InboxSurface), OpenDuet (approvals/service.ts)

#### 2.4.1 Schema (L1)

```sql
CREATE TABLE IF NOT EXISTS action_audit_log (
    id TEXT PRIMARY KEY,
    bot_id TEXT NOT NULL,
    action_type TEXT NOT NULL,       -- send_email, deploy, payment, etc.
    target TEXT,                     -- recipient, URL, resource
    approval_status TEXT NOT NULL,   -- approved, refused, auto_approved, expired
    approval_decision TEXT,          -- allow_once, allow_always, refuse
    arguments_hash TEXT NOT NULL,    -- SHA-256 dos argumentos (não os args em si)
    result_status TEXT,              -- success, failure, timeout
    result_summary TEXT,             -- Mensagem curta do resultado
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    decided_at TEXT,
    executed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_bot_date 
    ON action_audit_log(bot_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_approval 
    ON action_audit_log(approval_status);
```

#### 2.4.2 Endpoint (L3)

- `GET /api/v1/audit/actions?bot_id=X&since=Y&limit=Z` — paginado
- `GET /api/v1/audit/stats?period=7d` — agregados por tipo/status

#### 2.4.3 Componente (L5)

Painel "Audit Log" no cockpit com tabela filtrável, export CSV, e gráficos de tendência.

#### 2.4.4 Critérios de Aceite

- [ ] Toda ação externa gera entrada no audit log
- [ ] Arguments armazenados como hash SHA-256 (não plaintext)
- [ ] Filtros por bot, período, status, tipo de ação
- [ ] Export CSV funcional
- [ ] Gráfico de tendência (ações/dia por tipo)
- [ ] Retenção configurável (default 90 dias)

---

### 2.5 Task 1.5: Gate Criteria — Fase 1

**Todos os itens abaixo devem ser verdadeiros para considerar a Fase 1 completa:**

- [ ] Approval Gate Cards renderizam inline no chat com 3 ações funcionais
- [ ] Egress Checker detecta e bloqueia/redige PII/secrets antes do envio
- [ ] Password Masking impede senhas em plaintext no transcript
- [ ] Action Audit Log registra todas as ações externas com hash de argumentos
- [ ] Todos os endpoints têm testes de integração passando
- [ ] Cockpit compila com zero warnings
- [ ] Gateway compila com zero warnings
- [ ] Documentação atualizada (README, API docs, DESIGN_SYSTEM.md)
- [ ] Manifesto v3.1 compliance verificado (10 camadas)

---

## 3. Fase 2: Produtividade (IMPORTANTE)

**Duração estimada:** 3 semanas (15 dias úteis)
**Objetivo:** Adicionar funcionalidades de produtividade identificadas nos dashboards de referência

### 3.1 Task 2.1: Morning Brief

**Gap:** Zylos não tem briefing diário automático
**Fontes:** OpenDuet (`brief.ts`, `shared/morning-brief.ts`), awesome-grok-bot (Daily Briefing template)

#### 3.1.1 Schema (L1)

```sql
CREATE TABLE IF NOT EXISTS morning_briefs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    date TEXT NOT NULL,              -- YYYY-MM-DD
    content_json TEXT NOT NULL,      -- Estrutura do brief
    delivered_at TEXT NOT NULL,
    read_at TEXT,
    items_count INTEGER NOT NULL DEFAULT 0,
    needs_you_count INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_brief_user_date 
    ON morning_briefs(user_id, date);
```

#### 3.1.2 Lógica (L2)

**Arquivo:** `crates/zylos-brief/src/generator.rs`

```rust
pub struct MorningBriefGenerator {
    memory_store: MemoryStore,
    task_store: TaskStore,
    goal_store: GoalStore,
    calendar_client: Option<CalendarClient>,
    github_client: Option<GitHubClient>,
}

impl MorningBriefGenerator {
    pub async fn generate(&self, user_id: &str) -> MorningBrief {
        let today = chrono::Utc::now().date_naive();
        
        // 1. What needs you (approval pendentes, tasks bloqueadas)
        let needs_you = self.get_needs_you(user_id).await;
        
        // 2. What can wait (tasks em andamento, goals com deadline > 7d)
        let can_wait = self.get_can_wait(user_id).await;
        
        // 3. Calendar (se conectado)
        let calendar = self.get_today_calendar(user_id).await;
        
        // 4. Unchecked items (integrações offline/falhas)
        let unchecked = self.get_unchecked_items(user_id).await;
        
        MorningBrief {
            date: today.to_string(),
            needs_you,
            can_wait,
            calendar,
            unchecked,
            generated_at: Utc::now().to_rfc3339(),
        }
    }
}
```

#### 3.1.3 Trigger (L6)

Cron job no gateway às 06:00 local do usuário, ou na primeira abertura do cockpit após 06:00.

#### 3.1.4 Componente (L5)

Card especial no topo do chat com seções colapsáveis e botão "Start" por item.

#### 3.1.5 Critérios de Aceite

- [ ] Brief gerado automaticamente às 06:00 ou primeira abertura
- [ ] 4 seções: needs you, can wait, calendar, unchecked
- [ ] Botão "Start" transfere contexto para o agente
- [ ] Persistido no SQLite (não regenera se já existe para o dia)
- [ ] Funciona sem integrações externas (graceful degradation)
- [ ] Respeita fuso horário do usuário

---

### 3.2 Task 2.2: Task Triage Panel

**Gap:** Zylos não diferencia visualmente prioridades de tarefas
**Fonte:** Rerun (`InboxSurface.tsx`)

#### 3.2.1 Componente (L5)

Substituir lista plana de tasks por 4 seções ordenadas:

```
┌─────────────────────────────────────┐
│ 🔴 Needs You (3)                    │
│   • Approve email to client         │
│   • Review PR #142                  │
│   • Answer question about budget    │
├─────────────────────────────────────┤
│ 🟡 Heads Up (5)                     │
│   • Deploy completed successfully   │
│   • New comment on issue #89        │
├─────────────────────────────────────┤
│ 🔵 Errors (1)                       │
│   • Slack integration failed        │
├─────────────────────────────────────┤
│ ✅ Done (12)                        │
│   • Report generated                │
│   • Tests passing                   │
└─────────────────────────────────────┘
```

#### 3.2.2 Critérios de Aceite

- [ ] 4 seções visuais com contadores
- [ ] Items ordenados por urgência dentro de cada seção
- [ ] Click em item abre contexto relevante
- [ ] Seção "Done" colapsada por padrão, expandível
- [ ] Atualização em tempo real via SSE
- [ ] Empty states apropriados por seção

---

### 3.3 Task 2.3: Multi-Model Selector

**Gap:** Zylos não tem seletor de modelo com tier strategy
**Fonte:** AnyModelBot (`model-picker.js`, `lib/frontier.js`)

#### 3.3.1 Schema (L1)

```sql
CREATE TABLE IF NOT EXISTS model_configs (
    id TEXT PRIMARY KEY,
    bot_id TEXT NOT NULL,
    provider TEXT NOT NULL,           -- openai, anthropic, ollama, etc.
    model_id TEXT NOT NULL,           -- gpt-4o, claude-3.5-sonnet, etc.
    tier_strategy TEXT NOT NULL DEFAULT 'balanced',  -- cheapest/balanced/fastest
    reasoning_effort TEXT NOT NULL DEFAULT 'medium', -- low/medium/high
    max_tokens INTEGER,
    temperature REAL DEFAULT 0.7,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_model_bot 
    ON model_configs(bot_id, is_default);
```

#### 3.3.2 Componente (L5)

Dropdown no composer com:
- Tier slider (cheapest ↔ fastest)
- Reasoning effort selector (low/medium/high)
- Lista agrupada por provider com badges de capacidade (vision, tools, context size)

#### 3.3.3 Critérios de Aceite

- [ ] Seletor acessível via dropdown no composer
- [ ] Tier strategy afeta seleção automática de modelo
- [ ] Reasoning effort passado como parâmetro à API
- [ ] Configuração persistente por bot
- [ ] Badges visuais de capacidade (vision, tools, 128k context)
- [ ] Fallback gracioso se modelo indisponível

---

### 3.4 Task 2.4: Computer View Panel

**Gap:** Zylos não tem painel de browser/terminal do agente
**Fontes:** OpenMausBot (`CanvasComputers.tsx`), OpenDots (`ComputerPanel.tsx`)

#### 3.4.1 Arquitetura

- **Backend:** VNC/WebSocket proxy no gateway que conecta ao container/VM do agente
- **Frontend:** Painel lateral com abas Browser/Terminal/Files/Activity
- **Controle:** Botão "Take Control" / "Release Control" para intervenção humana

#### 3.4.2 Critérios de Aceite

- [ ] Screenshot ao vivo do desktop do agente (VNC stream ou polling)
- [ ] Abas Browser/Terminal/Files/Activity
- [ ] Take/Release control funcional
- [ ] Indicador visual quando agente está usando o computer
- [ ] Graceful fallback se VNC indisponível (static screenshot)
- [ ] Responsivo (colapsa em mobile)

---

### 3.5 Task 2.5: Spaces/Docs Tab

**Gap:** Zylos não tem aba de documentos persistentes
**Fonte:** OpenDots (`SpaceWorkspace.tsx`, `PageDocument.tsx`)

#### 3.5.1 Schema (L1)

```sql
CREATE TABLE IF NOT EXISTS spaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    owner_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE IF NOT EXISTS pages (
    id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL REFERENCES spaces(id),
    parent_id TEXT REFERENCES pages(id),
    title TEXT NOT NULL,
    content_markdown TEXT NOT NULL DEFAULT '',
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_pages_space 
    ON pages(space_id, position);
CREATE INDEX IF NOT EXISTS idx_pages_parent 
    ON pages(parent_id);
```

#### 3.5.2 Componente (L5)

Nova aba "Docs" com:
- Sidebar de outline (árvore de páginas)
- Editor rich-text com markdown support
- Autosave com debounce 2s
- Slash commands (/heading, /list, /code, /table)

#### 3.5.3 Critérios de Aceite

- [ ] CRUD completo de Spaces e Pages
- [ ] Editor rich-text com markdown e autosave
- [ ] Outline lateral com navegação hierárquica
- [ ] Conversas podem ser salvas como página
- [ ] Dirty-page guard ao trocar de rota
- [ ] Subpáginas suportadas (parentId)

---

### 3.6 Task 2.6: Gate Criteria — Fase 2

- [ ] Morning Brief gera e entrega diariamente
- [ ] Task Triage mostra 4 seções com contadores
- [ ] Multi-Model Selector funcional com tier strategy
- [ ] Computer View Panel exibe stream ao vivo
- [ ] Spaces/Docs Tab com editor e outline
- [ ] Zero warnings no workspace
- [ ] Documentação atualizada

---

## 4. Fase 3: Colaboração (DESEJÁVEL)

**Duração estimada:** 2 semanas (10 dias úteis)

### 4.1 Tasks

| ID | Task | Fonte | Esforço |
|----|------|-------|---------|
| 3.1 | Thread Management (drag-and-drop, rename, reorder) | OpenBot | Médio |
| 3.2 | Subagent Status Cards (steps, duration, tokens) | OpenBot | Baixo |
| 3.3 | File Manager Visual (upload, preview, organize) | OpenMuse | Médio |
| 3.4 | Voice Panel (waveform, phase states, mute) | OpenBot | Alto |
| 3.5 | Emoji Reactions (add/remove, contagem) | OpenBot | Baixo |
| 3.6 | Agent Avatars por Divisão (20+ cores) | AnyModelBot | Baixo |

### 4.2 Gate Criteria — Fase 3

- [ ] Threads gerenciáveis com drag-and-drop
- [ ] Subagent cards visíveis no chat
- [ ] File manager funcional
- [ ] Voice panel com waveform (se viável)
- [ ] Reactions funcionais
- [ ] Avatares diferenciados por papel

---

## 5. Fase 4: Avançado (DESEJÁVEL BAIXO)

**Duração estimada:** 4 semanas (20 dias úteis)

### 5.1 Tasks

| ID | Task | Fonte | Esforço |
|----|------|-------|---------|
| 4.1 | Channel System (multi-usuário) | OpenBot | Alto |
| 4.2 | Skin Themes (5+ temas) | OpenMausBot | Médio |
| 4.3 | Phone Relay UI (QR pairing, E2E) | OpenMausBot | Alto |
| 4.4 | Memory Timeline Visual | OpenMuse | Médio |
| 4.5 | Skill Editor Visual (WYSIWYG) | ECC/Paperclip | Alto |
| 4.6 | Swarm Topology Graph (ReactFlow) | Ruflo | Alto |
| 4.7 | Bot Fleet Auditor (custo/eficácia) | awesome-grok-bot | Médio |
| 4.8 | Federation Dashboard (HLC/VectorClock) | Ruflo | Alto |

### 5.2 Gate Criteria — Fase 4

- [ ] Channels multi-usuário funcionais
- [ ] 5+ skins disponíveis
- [ ] Phone relay com pairing QR
- [ ] Memory timeline integrada ao chat
- [ ] Skill editor WYSIWYG
- [ ] Swarm topology visualizada
- [ ] Fleet auditor com ledger semanal
- [ ] Federation dashboard com conflict resolution

---

## 6. Arquitetura Transversal

### 6.1 Novos Crates

| Crate | Propósito | Dependências |
|-------|-----------|--------------|
| `zylos-approval` | Approval gates HITL | sqlx, uuid, serde |
| `zylos-egress` | Verificação de PII/secrets | regex, serde |
| `zylos-brief` | Morning brief generator | chrono, serde |
| `zylos-spaces` | Documents/Spaces CRUD | sqlx, serde |
| `zylos-model-config` | Multi-model selector | sqlx, serde |
| `zylos-computer-view` | VNC/browser proxy | tokio-tungstenite |
| `zylos-audit-log` | Action audit trail | sqlx, sha2 |

### 6.2 Design System v2

Atualizar `design-tokens.css` com novos tokens extraídos dos 12 dashboards:

```css
/* Approval states */
--status-pending: #f59e0b;
--status-approved: #10b981;
--status-refused: #ef4444;
--status-expired: #6b7280;

/* Priority levels */
--priority-critical: #dc2626;
--priority-high: #f59e0b;
--priority-medium: #3b82f6;
--priority-low: #6b7280;

/* Agent identity (20 cores) */
--agent-engineering: #3b82f6;
--agent-design: #ec4899;
--agent-marketing: #f59e0b;
--agent-finance: #10b981;
--agent-ops: #8b5cf6;
--agent-support: #06b6d4;
--agent-research: #6366f1;
--agent-legal: #78716c;

/* Computer view */
--computer-glow: 0 0 20px rgba(59, 130, 246, 0.3);
--computer-border: 1px solid rgba(59, 130, 246, 0.2);
```

### 6.3 Event Bus

Eventos transversais entre crates:

```rust
pub enum ZylosEvent {
    ApprovalRequested { request_id: String, bot_id: String },
    ApprovalDecided { request_id: String, decision: ApprovalDecision },
    EgressBlocked { text_hash: String, categories: Vec<SensitiveCategory> },
    MorningBriefGenerated { user_id: String, date: String },
    TaskStatusChanged { task_id: String, old_status: String, new_status: String },
    ModelChanged { bot_id: String, model_id: String },
    ComputerControlTaken { computer_id: String, user_id: String },
    ComputerControlReleased { computer_id: String, user_id: String },
}
```

---

## 7. Testes e Qualidade

### 7.1 Estratégia de Testes por Camada

| Camada | Tipo | Cobertura Mínima | Ferramenta |
|--------|------|-------------------|------------|
| L1 Schema | Unit | 100% | sqlx::test |
| L2 Business | Unit | 90% | cargo test |
| L3 API | Integration | 85% | axum::test |
| L5 WASM | E2E | 80% | wasm-bindgen-test |
| L6 Integration | E2E | 75% | Playwright (future) |
| L7 Observability | Smoke | 100% | curl + jq |

### 7.2 Testes Específicos por Task

Ver seções individuais acima para detalhes de testes por task.

### 7.3 CI Gates

- `cargo clippy -- -D warnings` (zero warnings)
- `cargo test --workspace` (todos passam)
- `trunk build` (cockpit compila)
- `cargo doc --no-deps` (docs geram sem erro)

---

## 8. Timeline e Recursos

### 8.1 Cronograma

| Semana | Fase | Tasks | Entregável |
|--------|------|-------|------------|
| 1-2 | Fase 1 | 1.1–1.5 | Approval Gates + Egress + Audit Log |
| 3-5 | Fase 2 | 2.1–2.5 | Morning Brief + Triage + Models + Computer + Docs |
| 6-7 | Fase 3 | 3.1–3.6 | Threads + Subagents + Files + Voice + Reactions |
| 8-11 | Fase 4 | 4.1–4.8 | Channels + Skins + Phone + Memory + Skills + Swarm |

### 8.2 Marcos

| Marco | Data Estimada | Critério |
|-------|---------------|----------|
| M1: Segurança | Semana 2 | Todos os approval gates funcionais |
| M2: Produtividade | Semana 5 | Morning brief + triage + models |
| M3: Colaboração | Semana 7 | Threads + subagents + files |
| M4: Completo | Semana 11 | Todas as 4 fases entregues |

---

## 9. Riscos e Mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|-------|---------------|---------|-----------|
| VNC proxy complexo demais | Alta | Alto | Fallback para static screenshots + polling |
| Leptos 0.7 breaking changes | Média | Médio | Pin versão exata, testar antes de upgrade |
| sqlx migrations falham em produção | Baixa | Alto | Separar statements individuais (lição da Fase 8) |
| Context window overflow durante implementação | Alta | Médio | Usar subagentes para tasks independentes |
| CORS issues entre cockpit e gateway | Média | Baixo | Já configurado corretamente (localhost:8080 → :3000) |
| Build times excessivos (>15min) | Média | Médio | Incremental builds, sccache, workspace members selecionados |

---

## 10. Decisões Arquiteturais (ADRs)

### ADR-001: Approval Gates Inline vs Modal

**Decisão:** Inline no chat (como OpenDots/OpenDuet), não modal separado.
**Racional:** Mantém contexto da conversa, reduz fricção, permite aprovação rápida sem perder fluxo.
**Consequência:** Cards ocupam espaço no chat; mitigado com colapso após decisão.

### ADR-002: Egress Checker Síncrono vs Assíncrono

**Decisão:** Síncrono no frontend antes do envio.
**Racional:** Latência aceitável (<5ms para textos <10KB), previne vazamento antes de sair do browser.
**Consequência:** Bloqueia envio temporariamente; mitigado com indicador de loading.

### ADR-003: Computer View VNC vs Static Screenshots

**Decisão:** VNC como primário, static screenshots como fallback.
**Racional:** VNC dá controle real-time; fallback garante funcionalidade básica sempre.
**Consequência:** Complexidade adicional no proxy; mitigado com feature flag.

### ADR-004: Spaces em SQLite vs Arquivos

**Decisão:** SQLite com content_markdown.
**Racional:** Consistente com resto do stack, busca full-text possível, versionamento via updated_at.
**Consequência:** Não editável fora do cockpit; mitigado com export markdown.

### ADR-005: Multi-Model Config Per-Bot vs Global

**Decisão:** Per-bot com override global como default.
**Racional:** Diferentes bots podem precisar de diferentes modelos (reasoning vs speed).
**Consequência:** Mais configs para gerenciar; mitigado com herança global→bot.

---

## Apêndice A: Referência de Código dos Dashboards

Todos os repositórios analisados permanecem em `C:/Users/Paulo/dashboard-analysis/`:

```
dashboard-analysis/
├── openbot/          # SolidJS + Electron
├── OpenDots/         # Next.js + CopilotKit
├── openmuse/         # CopilotKit + AG-UI
├── paperclip/        # Next.js + Supabase
├── rerun/            # Next.js + Supabase
├── ruflo/            # Python + TypeScript
├── ecc/              # CLI/Harness
├── AnyModelBot/      # Electron + Vanilla JS
├── OpenDuet/         # CopilotKit + AG-UI
├── OpenMausBot/      # Electron + TypeScript
├── agency-agents/    # Markdown + YAML
├── awesome-grok-bot/ # Markdown only
└── ANALYSIS_REPORT.md # Relatório consolidado
```

## Apêndice B: Compliance com Manifesto v3.1

Este plano segue todas as 10 camadas do manifesto:

1. ✅ **Contexto Canônico** — Gaps extraídos de análise real de 12 repos
2. ✅ **Anti-Slop** — Zero filler, especificações técnicas concretas
3. ✅ **Loop Profissional** — READ → PLAN → IMPLEMENT → TEST → DOCUMENT
4. ✅ **Skills Documentais** — SOPs/POPs/playbooks serão criados por task
5. ✅ **Arquivos Obrigatórios** — README, DESIGN_SYSTEM, API docs por crate
6. ✅ **Templates de Prompts** — N/A (plano de implementação, não prompts)
7. ✅ **Matriz de Rastreabilidade** — Tasks mapeadas para gaps e fontes
8. ✅ **Evolução Semântica** — ADRs documentam decisões e racional
9. ✅ **Gate Criteria** — Cada fase tem critérios de aceite explícitos
10. ✅ **Verificação Adversarial** — Testes unitários + E2E + clippy

---

## Apêndice C: Compliance Documental Obrigatório

> **Documento mestre:** [`DOCUMENT_COMPLIANCE_FASE9.md`](./DOCUMENT_COMPLIANCE_FASE9.md)
> Este apêndice resume os artefatos obrigatórios; detalhes, templates e checklists completos estão no documento vinculado acima.

### C.1 Artefatos por Fase

| Fase | READMEs | Playbooks | POPs | Runbooks | SOARs | POTs | Handoff | Track Record |
|------|---------|-----------|------|----------|-------|------|---------|--------------|
| Fase 1 (Segurança) | 3 (approval, egress, audit-log) | 2 (pb-approval-gate-setup, pb-egress-rules-management) | 3 (pop-approval-decide, pop-egress-false-positive, pop-audit-log-export) | 3 (rb-zylos-approval/egress/audit-log) | 3 (soar-egress-block-response, soar-approval-timeout, soar-audit-anomaly) | 2 (pot-approval-rule-tuning, pot-egress-pattern-update) | fase8-to-9-handoff.md | TRACKRECORD_FASE9.md |
| Fase 2 (Produtividade) | 4 (brief, model-config, computer-view, spaces) | 4 (pb-morning-brief-config, pb-model-tier-strategy, pb-computer-view-setup, pb-spaces-onboarding) | 4 (pop-brief-regenerate, pop-model-fallback, pop-computer-control-emergency, pop-space-page-recovery) | 5 (rb-zylos-brief/model-config/computer-view/spaces + rb-cockpit-v2-deploy) | 1 (soar-computer-control-breach) | 2 (pot-model-benchmark, pot-space-migration) | HANDOFF_FASE9.md (parcial) | TRACKRECORD_FASE9.md (update) |
| Final | — | — | — | — | — | — | HANDOFF_FASE9.md (completo) | TRACKRECORD_FASE9.md (final) |

### C.2 Documentos Transversais a Atualizar

| Documento | Ação | Quando |
|-----------|------|--------|
| `MODULE_DOCUMENTATION_STATUS.md` | Adicionar 7 novos crates na tabela | Após cada crate criado |
| `DESIGN_SYSTEM.md` | Adicionar tokens de approval/priority/agent-identity/computer-view | Durante Fase 1 e Fase 2 |
| `AGENTS.md` | Adicionar auto-invocation rules para novas skills | Ao final da Fase 9 |
| `CONCEITOS-DESENVOLVIMENTO-IA.md` | Adicionar conceitos HITL/egress/morning-brief | Ao final da Fase 9 |
| Gateway `README.md` | Adicionar endpoints /approvals/*, /egress/*, /brief/*, /spaces/*, /audit/* | Após cada API criada |
| Cockpit `README.md` | Adicionar novos componentes/tabs/design-system v2 | Após cada componente criado |
| `DECISIONS_FASE9.md` | Log contínuo de ADRs | Durante toda a fase |

### C.3 Gate Criteria Documental

Nenhuma fase é considerada completa sem que todos os documentos listados no checklist correspondente em `DOCUMENT_COMPLIANCE_FASE9.md` §4 estejam criados e com cross-linking bidirecional verificado.