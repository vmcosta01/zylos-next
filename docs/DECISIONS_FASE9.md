# Decisões Arquiteturais — Fase 9 (Cockpit v2)

**Data Início:** 2026-11-11
**Base:** Análise profunda de 12 dashboards open-source + IMPLEMENTATION_PLAN_DETAILED.md

---

## ADR-001: Approval Gates Inline no Chat vs Modal Separado

**Data:** 2026-11-11
**Status:** Aceito
**Contexto:** Como renderizar cards de aprovação HITL no cockpit — inline no fluxo de chat ou como modal/painel separado?

**Decisão:** Inline no chat, como OpenDots (`PageReviewCard.tsx`) e OpenDuet (`approvals/service.ts`).

**Racional:**
- Mantém contexto conversacional — usuário vê a ação no fluxo da conversa
- Reduz fricção — aprovação rápida sem trocar de tela
- Consistente com padrão de 5/12 dashboards analisados (OpenBot, OpenDots, OpenDuet, AnyModelBot, Rerun)
- Receipt persistence permite sobrevivência a refresh sem perder estado

**Alternativas Consideradas:**
1. Modal centralizado (Paperclip) — perde contexto, requer navegação extra
2. Painel lateral dedicado (Ruflo) — bom para bulk approval, ruim para ações individuais
3. Notificação toast (ECC CLI) — inadequado para UI web, fácil de perder

**Consequências:**
- Cards ocupam espaço vertical no chat → mitigado com colapso após decisão
- Scroll pode pular cards se muitos pendentes → mitigado com sticky header de pending count
- CSS deve ser responsivo para mobile → incluído em design-tokens.css

---

## ADR-002: Egress Checker Síncrono no Frontend

**Data:** 2026-11-11
**Status:** Aceito
**Contexto:** Onde executar verificação de PII/secrets antes de enviar mensagens — frontend (browser) ou backend (gateway)?

**Decisão:** Síncrono no frontend (Leptos WASM) antes do envio, com fallback para backend se endpoint indisponível.

**Racional:**
- Previne vazamento antes de sair do browser — defesa em profundidade
- Latência aceitável: <5ms para textos <10KB (regex patterns compilados)
- Não depende de round-trip de rede para verificação básica
- Backend verifica novamente como segunda linha de defesa

**Alternativas Consideradas:**
1. Apenas backend — PII já saiu do browser, vulnerável a interceptação
2. Web Worker assíncrono — complexidade adicional, não melhora UX perceptivelmente
3. Wasm module separado — overhead de serialização desnecessário para regex simples

**Consequências:**
- Bloqueia envio temporariamente durante verificação → mitigado com spinner <100ms
- Patterns devem ser atualizados via API → rules customizáveis em `zylos-egress`
- Falso positivo bloqueia envio legítimo → POP de false-positive + ajuste de regra

---

## ADR-003: Computer View VNC Primário + Static Screenshot Fallback

**Data:** 2026-11-11
**Status:** Aceito
**Contexto:** Como renderizar desktop/browser do agente no cockpit — stream ao vivo ou screenshots estáticos?

**Decisão:** VNC/WebSocket stream como primário, static screenshots como fallback automático.

**Racional:**
- VNC dá controle real-time e visibilidade completa (OpenMausBot, OpenDots)
- Fallback garante funcionalidade básica quando VNC indisponível
- Feature flag permite desativar VNC em ambientes restritos
- Progressive enhancement: funciona sempre, melhor quando possível

**Alternativas Consideradas:**
1. Apenas screenshots (OpenMuse) — sem interatividade, delay de polling
2. iframe embed direto — problemas de segurança (X-Frame-Options), sem controle
3. WebRTC — complexo demais para MVP, NAT traversal issues

**Consequências:**
- Proxy WebSocket necessário no gateway → crate `zylos-computer-view`
- Bandwidth pode ser alto para streams contínuos → mitigado com quality scaling
- Take/Release control requer lease management → timeout automático de 5min

---

## ADR-004: Spaces/Documents em SQLite vs Arquivos Markdown

**Data:** 2026-11-11
**Status:** Aceito
**Contexto:** Onde persistir documentos/páginas do Spaces — banco de dados ou filesystem?

**Decisão:** SQLite com coluna `content_markdown`, não arquivos no filesystem.

**Racional:**
- Consistente com resto do stack Zylos (todos os crates usam SQLite)
- Busca full-text possível via FTS5 extension futura
- Versionamento via `updated_at` + audit log
- Transações ACID para operações multi-página (move, rename hierárquico)
- Backup único (sqlite3 .backup) vs múltiplos arquivos

**Alternativas Consideradas:**
1. Arquivos markdown no disco (agency-agents) — sem busca, sem transações, backup complexo
2. PostgreSQL/Supabase (Paperclip, Rerun) — dependência externa pesada, overkill para single-user
3. CRDT/Yjs para colaboração — prematuro, adicionar quando multi-user for necessário

**Consequências:**
- Não editável fora do cockpit → mitigado com export markdown via API
- Tamanho limitado por row (~1GB teórico, ~10MB prático) → validar max content size
- Migração de docs legados requer import tool → POT `pot-space-migration.md`

---

## ADR-005: Multi-Model Config Per-Bot com Herança Global

**Data:** 2026-11-11
**Status:** Aceito
**Contexto:** Configuração de modelo deve ser global (todos os bots) ou por-bot individual?

**Decisão:** Per-bot com override global como default. Hierarquia: global → bot → conversation.

**Racional:**
- Diferentes bots têm diferentes necessidades (reasoning agent vs fast triage)
- AnyModelBot prova que tier strategy por-conversa é valioso
- Herança reduz configuração repetitiva
- Conversation-level override permite ajuste ad-hoc sem mudar config persistente

**Alternativas Consideradas:**
1. Apenas global — inflexível, força mesmo modelo para todos os casos
2. Apenas per-bot — configuração excessiva para setups simples
3. Per-user (multi-tenant) — prematuro, adicionar quando auth multi-user existir

**Consequências:**
- 3 níveis de resolução de config → lógica clara de fallback necessária
- UI deve mostrar qual nível está ativo (badge "global"/"bot"/"session")
- Quota/billing tracking deve considerar nível de override

---

## ADR-006: Migrations SQL com Statements Individuais

**Data:** 2026-11-11 (herdado da Fase 8)
**Status:** Aceito (lição aprendida)
**Contexto:** sqlx SQLite driver ignora silenciosamente tudo após primeiro statement em string multi-statement.

**Decisão:** Sempre separar CREATE TABLE, CREATE INDEX, etc. em chamadas `sqlx::query().execute()` individuais.

**Racional:**
- Lição dura da Fase 8: 4 endpoints retornavam HTTP 500 porque tabelas nunca foram criadas
- sqlx SQLite não suporta multi-statement strings (limitação do driver, não do SQLite)
- Statements individuais são idempotentes com IF NOT EXISTS
- Debugging mais fácil (erro aponta para statement específico)

**Consequências:**
- Código de migration mais verboso (6 statements = 6 chamadas)
- Wrap em função `run_migrations()` para encapsular complexidade
- Nunca usar `include_str!("migration.sql")` diretamente com sqlx SQLite

---

## Log de Decisões Futuras

_Decisões adicionais serão registradas aqui conforme a implementação avança._

| # | Data | Título | Status |
|---|------|--------|--------|
| 001 | 2026-11-11 | Approval Gates Inline vs Modal | ✅ Aceito |
| 002 | 2026-11-11 | Egress Checker Síncrono Frontend | ✅ Aceito |
| 003 | 2026-11-11 | Computer View VNC + Fallback | ✅ Aceito |
| 004 | 2026-11-11 | Spaces em SQLite vs Arquivos | ✅ Aceito |
| 005 | 2026-11-11 | Multi-Model Per-Bot + Herança | ✅ Aceito |
| 006 | 2026-11-11 | Migrations Statements Individuais | ✅ Aceito (Fase 8) |