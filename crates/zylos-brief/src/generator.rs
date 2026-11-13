//! MorningBriefGenerator — lógica de agregação de múltiplas fontes de dados
//!
//! Gera o morning brief consultando:
//! - Approval requests pendentes (needs_you)
//! - Tasks em andamento (can_wait)
//! - Calendar events (calendar) — se integrado
//! - Integration health (unchecked) — falhas recentes
//!
//! O generator é stateless; recebe stores como dependência e retorna
//! um MorningBrief completo. A persistência é responsabilidade do caller.

use crate::schema::*;
use sqlx::SqlitePool;

/// Generator de morning briefs
pub struct MorningBriefGenerator {
    pool: SqlitePool,
}

impl MorningBriefGenerator {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Gera morning brief completo para um usuário e data
    pub async fn generate(&self, user_id: &str, date: &str) -> Result<MorningBrief, sqlx::Error> {
        let mut brief = MorningBrief::empty(user_id, date);

        // Seção 1: Needs You — approvals pendentes + tasks bloqueadas
        let needs_you_items = self.collect_needs_you(user_id).await?;
        if !needs_you_items.is_empty() {
            brief.sections[0].items = needs_you_items;
            brief.sections[0].summary = None; // Será preenchido por finalize()
        }

        // Seção 2: Can Wait — tasks em andamento, goals com deadline > 7d
        let can_wait_items = self.collect_can_wait(user_id).await?;
        if !can_wait_items.is_empty() {
            brief.sections[1].items = can_wait_items;
            brief.sections[1].summary = None;
        }

        // Seção 3: Calendar — placeholder para integração futura
        // Por agora, mantém summary padrão "Sem eventos agendados"
        // TODO: integrar com Google Calendar / Outlook via MCP connector

        // Seção 4: Unchecked — integrações com falhas recentes
        let unchecked_items = self.collect_unchecked().await?;
        if !unchecked_items.is_empty() {
            brief.sections[3].items = unchecked_items;
            brief.sections[3].summary = None;
        }

        brief.finalize();

        tracing::info!(
            user_id = %user_id,
            date = %date,
            total_items = brief.total_items,
            needs_you = brief.needs_you_count,
            "Morning brief generated"
        );

        Ok(brief)
    }

    /// Coleta itens que requerem atenção imediata
    async fn collect_needs_you(&self, _user_id: &str) -> Result<Vec<BriefItem>, sqlx::Error> {
        let mut items = Vec::new();

        // Approvals pendentes (da tabela approval_requests)
        let approvals = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT id, bot_id, tool_name, summary FROM approval_requests WHERE status = 'pending' ORDER BY created_at ASC LIMIT 10"
        )
        .fetch_all(&self.pool)
        .await?;

        for (id, bot_id, tool_name, summary) in approvals {
            items.push(BriefItem {
                id: format!("approval-{}", id),
                title: summary,
                description: Some(format!("Bot: {} | Tool: {}", bot_id, tool_name)),
                priority: "critical".into(),
                source: "approval".into(),
                action_url: Some(format!("/chat?approval={}", id)),
                metadata: None,
            });
        }

        // Tasks bloqueadas (se houver tabela de tasks no futuro)
        // Por agora, apenas approvals são coletados

        Ok(items)
    }

    /// Coleta itens que podem esperar
    async fn collect_can_wait(&self, _user_id: &str) -> Result<Vec<BriefItem>, sqlx::Error> {
        let mut items = Vec::new();

        // Goals ativos com deadline > 7 dias (da tabela goals se existir)
        // Placeholder: sem integração direta ainda
        // TODO: consultar zylos-goals store quando disponível

        // Audit log: ações recentes que não foram verificadas
        let recent_actions = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, bot_id, action_type FROM action_audit_log WHERE created_at > datetime('now', '-24 hours') AND result_status IS NULL ORDER BY created_at DESC LIMIT 5"
        )
        .fetch_all(&self.pool)
        .await?;

        for (id, bot_id, action_type) in recent_actions {
            items.push(BriefItem {
                id: format!("audit-{}", id),
                title: format!("Ação pendente de verificação: {}", action_type),
                description: Some(format!("Bot: {}", bot_id)),
                priority: "low".into(),
                source: "task".into(),
                action_url: Some("/audit".into()),
                metadata: None,
            });
        }

        Ok(items)
    }

    /// Coleta integrações com problemas recentes
    async fn collect_unchecked(&self) -> Result<Vec<BriefItem>, sqlx::Error> {
        let mut items = Vec::new();

        // Verificar se há erros recentes no audit log
        let errors = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, bot_id, action_type FROM action_audit_log WHERE result_status = 'failure' AND created_at > datetime('now', '-24 hours') ORDER BY created_at DESC LIMIT 5"
        )
        .fetch_all(&self.pool)
        .await?;

        for (id, bot_id, action_type) in errors {
            items.push(BriefItem {
                id: format!("error-{}", id),
                title: format!("Falha em {}: verificar logs", action_type),
                description: Some(format!("Bot: {}", bot_id)),
                priority: "high".into(),
                source: "integration".into(),
                action_url: Some("/audit".into()),
                metadata: None,
            });
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_generator() -> MorningBriefGenerator {
        let pool = SqlitePool::connect(":memory:").await.unwrap();

        // Criar tabelas necessárias (migrations inline)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS approval_requests (
                id TEXT PRIMARY KEY, bot_id TEXT NOT NULL, tool_name TEXT NOT NULL,
                action_category TEXT NOT NULL, summary TEXT NOT NULL, arguments_json TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending', decision TEXT, refuse_reason TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                decided_at TEXT, expires_at TEXT, receipt_json TEXT
            )"
        ).execute(&pool).await.unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS action_audit_log (
                id TEXT PRIMARY KEY, bot_id TEXT NOT NULL, action_type TEXT NOT NULL,
                target TEXT, approval_status TEXT NOT NULL, approval_decision TEXT,
                arguments_hash TEXT NOT NULL, result_status TEXT, result_summary TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                decided_at TEXT, executed_at TEXT
            )"
        ).execute(&pool).await.unwrap();

        MorningBriefGenerator::new(pool)
    }

    #[tokio::test]
    async fn test_generate_empty_brief() {
        let gen = setup_test_generator().await;
        let brief = gen.generate("user-1", "2026-11-11").await.unwrap();

        assert_eq!(brief.user_id, "user-1");
        assert_eq!(brief.date, "2026-11-11");
        assert_eq!(brief.sections.len(), 4);
        assert_eq!(brief.total_items, 0);
        assert_eq!(brief.needs_you_count, 0);
    }

    #[tokio::test]
    async fn test_generate_with_pending_approvals() {
        let gen = setup_test_generator().await;

        // Inserir approval pendente
        sqlx::query(
            "INSERT INTO approval_requests (id, bot_id, tool_name, action_category, summary, arguments_json, status) VALUES ('appr-1', 'bot-1', 'send_email', 'send_message', 'Enviar email para cliente', '{}', 'pending')"
        ).execute(&gen.pool).await.unwrap();

        let brief = gen.generate("user-1", "2026-11-11").await.unwrap();

        assert_eq!(brief.needs_you_count, 1);
        assert_eq!(brief.sections[0].items.len(), 1);
        assert_eq!(brief.sections[0].items[0].source, "approval");
        assert!(brief.sections[0].items[0].title.contains("email"));
    }

    #[tokio::test]
    async fn test_generate_with_failed_actions() {
        let gen = setup_test_generator().await;

        // Inserir ação com falha
        sqlx::query(
            "INSERT INTO action_audit_log (id, bot_id, action_type, approval_status, arguments_hash, result_status) VALUES ('audit-1', 'bot-1', 'deploy', 'approved', 'abc123', 'failure')"
        ).execute(&gen.pool).await.unwrap();

        let brief = gen.generate("user-1", "2026-11-11").await.unwrap();

        let unchecked = &brief.sections[3];
        assert_eq!(unchecked.items.len(), 1);
        assert_eq!(unchecked.items[0].source, "integration");
        assert!(unchecked.items[0].priority == "high");
    }

    #[tokio::test]
    async fn test_generate_finalize_updates_summaries() {
        let gen = setup_test_generator().await;

        // Inserir 2 approvals pendentes
        for i in 1..=2 {
            sqlx::query(&format!(
                "INSERT INTO approval_requests (id, bot_id, tool_name, action_category, summary, arguments_json, status) VALUES ('appr-{}', 'bot-1', 'tool_{}', 'send_message', 'Ação {}', '{{}}', 'pending')",
                i, i, i
            )).execute(&gen.pool).await.unwrap();
        }

        let brief = gen.generate("user-1", "2026-11-11").await.unwrap();

        assert_eq!(brief.needs_you_count, 2);
        assert_eq!(brief.total_items, 2);
        assert!(brief.sections[0].summary.as_ref().unwrap().contains("2"));
    }

    #[tokio::test]
    async fn test_generate_excludes_decided_approvals() {
        let gen = setup_test_generator().await;

        // Inserir approval já decidido
        sqlx::query(
            "INSERT INTO approval_requests (id, bot_id, tool_name, action_category, summary, arguments_json, status, decision) VALUES ('appr-decided', 'bot-1', 'send_email', 'send_message', 'Email aprovado', '{}', 'approved', 'allow_once')"
        ).execute(&gen.pool).await.unwrap();

        let brief = gen.generate("user-1", "2026-11-11").await.unwrap();

        assert_eq!(brief.needs_you_count, 0);
        assert_eq!(brief.sections[0].items.len(), 0);
    }
}