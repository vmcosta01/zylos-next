//! # CapabilityToken — Tokens de capacidade por agente (T8.3.5)
//!
//! Sistema de tokens granulares que definem permissões explícitas por agente.
//! Cada token especifica quais operações um agente pode realizar:
//! - `filesystem:read:/data` — leitura em diretório específico
//! - `network:egress:api.example.com` — acesso a host específico
//! - `exec:docker:run` — execução de comando Docker
//! - `memory:write:agent-*` — escrita em memória com padrão de escopo
//!
//! ## Revogação
//! Tokens podem ser revogados via `revoke()` ou broadcast via EventBus
//! para invalidação imediata em todos os nós.
//!
//! ## Validação
//! Cada operação deve ser validada contra o token do agente antes de executar.
//! Tokens expirados ou revogados são rejeitados imediatamente.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Erros do sistema de capability tokens.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TokenError {
    #[error("Token não encontrado: {0}")]
    NotFound(String),
    #[error("Token expirado")]
    Expired,
    #[error("Token revogado")]
    Revoked,
    #[error("Capacidade não permitida: {0}")]
    CapabilityDenied(String),
    #[error("Token inválido: {0}")]
    Invalid(String),
}

/// Uma capacidade individual concedida a um agente.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Capability {
    /// Domínio da capacidade (ex: "filesystem", "network", "exec", "memory").
    pub domain: String,
    /// Ação permitida (ex: "read", "write", "egress", "run").
    pub action: String,
    /// Recurso alvo (path, host, comando, pattern). Wildcard `*` permite tudo no domínio.
    pub resource: String,
}

impl Capability {
    /// Cria uma nova capacidade.
    pub fn new(
        domain: impl Into<String>,
        action: impl Into<String>,
        resource: impl Into<String>,
    ) -> Self {
        Self {
            domain: domain.into(),
            action: action.into(),
            resource: resource.into(),
        }
    }

    /// Verifica se esta capacidade autoriza uma operação específica.
    ///
    /// Regras de least-privilege (F11.11.6):
    /// - recursos com bytes de controle ou segmentos `..` são sempre negados
    ///   (impede `/data/../etc/passwd` sob `filesystem:read:/data/*`);
    /// - `filesystem`: wildcard de prefixo respeita fronteira de path —
    ///   `/data*` autoriza `/data` e `/data/x`, mas NÃO `/database`;
    /// - `network`: wildcard de prefixo (`api.example.com*`) é negado (permitiria
    ///   `api.example.com.evil.net`); use `*.example.com`, que casa somente
    ///   subdomínios com fronteira de ponto (case-insensitive);
    /// - demais domínios: prefixo literal (`agent-*` → `agent-123`).
    pub fn authorizes(&self, domain: &str, action: &str, resource: &str) -> bool {
        if self.domain != domain || self.action != action {
            return false;
        }
        if !Self::resource_is_safe(resource) {
            return false;
        }
        // Wildcard resource matches everything
        if self.resource == "*" {
            return true;
        }
        if domain == "network" {
            return Self::host_matches(&self.resource, resource);
        }
        // Prefix wildcard: "agent-*" matches "agent-123"
        if let Some(prefix) = self.resource.strip_suffix('*') {
            if !resource.starts_with(prefix) {
                return false;
            }
            if domain == "filesystem" && !prefix.is_empty() && !prefix.ends_with('/') {
                // Fronteira de path: o próximo caractere deve ser separador ou fim.
                let rest = &resource[prefix.len()..];
                return rest.is_empty() || rest.starts_with('/');
            }
            return true;
        }
        // Exact match
        self.resource == resource
    }

    /// Rejeita recursos com bytes de controle ou travessia de diretório.
    fn resource_is_safe(resource: &str) -> bool {
        if resource.chars().any(|c| c.is_control()) {
            return false;
        }
        !resource.split(['/', '\\']).any(|seg| seg == "..")
    }

    /// Casamento de host para o domínio `network`.
    fn host_matches(pattern: &str, host: &str) -> bool {
        let pattern = pattern.to_ascii_lowercase();
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        if host.is_empty() {
            return false;
        }
        if let Some(suffix) = pattern.strip_prefix("*.") {
            if suffix.is_empty() || suffix.contains('*') {
                return false;
            }
            return host.len() > suffix.len() + 1
                && host.ends_with(suffix)
                && host.as_bytes()[host.len() - suffix.len() - 1] == b'.';
        }
        if pattern.contains('*') {
            // Wildcards fora da forma `*.suffix` não são aceitos para rede.
            return false;
        }
        pattern == host
    }
}

/// Um token de capacidade emitido para um agente.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityToken {
    pub id: String,
    /// ID do agente proprietário.
    pub agent_id: String,
    /// Lista de capacidades concedidas.
    pub capabilities: Vec<Capability>,
    /// Timestamp de emissão (unix epoch seconds).
    pub issued_at: u64,
    /// TTL em segundos (None = sem expiração).
    pub ttl_secs: Option<u64>,
    /// Se true, o token foi revogado.
    pub revoked: bool,
    /// Hash SHA-256 do token para verificação rápida.
    pub fingerprint: String,
    /// Metadata arbitrária.
    pub metadata: serde_json::Value,
}

impl CapabilityToken {
    /// Cria um novo token com ID auto-gerado.
    pub fn new(agent_id: impl Into<String>, capabilities: Vec<Capability>) -> Self {
        let agent_id = agent_id.into();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let id = Uuid::new_v4().to_string();
        let fingerprint = Self::compute_fingerprint(&id, &agent_id);
        Self {
            id,
            agent_id,
            capabilities,
            issued_at: now,
            ttl_secs: Some(3600), // 1 hora default
            revoked: false,
            fingerprint,
            metadata: serde_json::Value::Null,
        }
    }

    /// Builder: define TTL customizado.
    pub fn with_ttl(mut self, secs: u64) -> Self {
        self.ttl_secs = Some(secs);
        self
    }

    /// Builder: sem expiração.
    pub fn without_expiry(mut self) -> Self {
        self.ttl_secs = None;
        self
    }

    /// Builder: anexa metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Verifica se o token está expirado.
    pub fn is_expired(&self) -> bool {
        match self.ttl_secs {
            None => false,
            Some(ttl) => {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                now > self.issued_at.saturating_add(ttl)
            }
        }
    }

    /// Verifica se o token autoriza uma operação específica.
    pub fn authorizes(&self, domain: &str, action: &str, resource: &str) -> Result<(), TokenError> {
        if self.revoked {
            return Err(TokenError::Revoked);
        }
        if self.is_expired() {
            return Err(TokenError::Expired);
        }
        if self
            .capabilities
            .iter()
            .any(|cap| cap.authorizes(domain, action, resource))
        {
            Ok(())
        } else {
            Err(TokenError::CapabilityDenied(format!(
                "{}:{}:{}",
                domain, action, resource
            )))
        }
    }

    /// Computa fingerprint SHA-256 do token.
    fn compute_fingerprint(token_id: &str, agent_id: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token_id.as_bytes());
        hasher.update(b":");
        hasher.update(agent_id.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Store de capability tokens com suporte a revogação e lookup rápido.
pub struct CapabilityTokenStore {
    /// Tokens ativos indexados por token_id.
    tokens: Arc<RwLock<std::collections::HashMap<String, CapabilityToken>>>,
    /// Index secundário: agent_id → set de token_ids.
    agent_index: Arc<RwLock<std::collections::HashMap<String, HashSet<String>>>>,
    /// Set de fingerprints revogados (para rejeição rápida).
    revoked_fingerprints: Arc<RwLock<HashSet<String>>>,
}

impl CapabilityTokenStore {
    /// Cria um novo store vazio.
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(std::collections::HashMap::new())),
            agent_index: Arc::new(RwLock::new(std::collections::HashMap::new())),
            revoked_fingerprints: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Emite um novo token para um agente.
    pub async fn issue(&self, token: CapabilityToken) -> String {
        let token_id = token.id.clone();
        let agent_id = token.agent_id.clone();
        let fingerprint = token.fingerprint.clone();

        let mut tokens = self.tokens.write().await;
        tokens.insert(token_id.clone(), token);

        let mut index = self.agent_index.write().await;
        index
            .entry(agent_id)
            .or_insert_with(HashSet::new)
            .insert(token_id.clone());

        tracing::info!(
            token_id = %token_id,
            fingerprint = %fingerprint,
            "CapabilityToken issued"
        );
        token_id
    }

    /// Busca um token por ID.
    pub async fn get(&self, token_id: &str) -> Option<CapabilityToken> {
        let tokens = self.tokens.read().await;
        tokens.get(token_id).cloned()
    }

    /// Valida um token e verifica autorização para uma operação.
    pub async fn validate(
        &self,
        token_id: &str,
        domain: &str,
        action: &str,
        resource: &str,
    ) -> Result<CapabilityToken, TokenError> {
        let token = self
            .get(token_id)
            .await
            .ok_or_else(|| TokenError::NotFound(token_id.to_string()))?;

        // Check if fingerprint is in revoked set
        {
            let revoked = self.revoked_fingerprints.read().await;
            if revoked.contains(&token.fingerprint) {
                return Err(TokenError::Revoked);
            }
        }

        token.authorizes(domain, action, resource)?;
        Ok(token)
    }

    /// Valida um token exigindo que ele pertença ao agente chamador
    /// (isolamento de escopo: um token vazado não pode ser usado por outro agente).
    pub async fn validate_for_agent(
        &self,
        token_id: &str,
        agent_id: &str,
        domain: &str,
        action: &str,
        resource: &str,
    ) -> Result<CapabilityToken, TokenError> {
        let token = self.validate(token_id, domain, action, resource).await?;
        if token.agent_id != agent_id {
            return Err(TokenError::CapabilityDenied(format!(
                "token pertence a outro agente: {}",
                token.agent_id
            )));
        }
        Ok(token)
    }

    /// Revoga um token por ID.
    pub async fn revoke(&self, token_id: &str) -> Result<(), TokenError> {
        let mut tokens = self.tokens.write().await;
        if let Some(token) = tokens.get_mut(token_id) {
            token.revoked = true;

            let mut revoked = self.revoked_fingerprints.write().await;
            revoked.insert(token.fingerprint.clone());

            tracing::warn!(token_id = %token_id, "CapabilityToken revoked");
            Ok(())
        } else {
            Err(TokenError::NotFound(token_id.to_string()))
        }
    }

    /// Revoga todos os tokens de um agente.
    pub async fn revoke_all_for_agent(&self, agent_id: &str) -> usize {
        let index = self.agent_index.read().await;
        let token_ids: Vec<String> = index
            .get(agent_id)
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default();
        drop(index);

        let mut count = 0;
        for token_id in token_ids {
            if self.revoke(&token_id).await.is_ok() {
                count += 1;
            }
        }
        count
    }

    /// Lista todos os tokens de um agente.
    pub async fn list_for_agent(&self, agent_id: &str) -> Vec<CapabilityToken> {
        let index = self.agent_index.read().await;
        let token_ids = match index.get(agent_id) {
            Some(ids) => ids.clone(),
            None => return vec![],
        };
        drop(index);

        let tokens = self.tokens.read().await;
        token_ids
            .iter()
            .filter_map(|id| tokens.get(id).cloned())
            .collect()
    }

    /// Remove tokens expirados (garbage collection).
    pub async fn gc_expired(&self) -> usize {
        let mut tokens = self.tokens.write().await;
        let mut index = self.agent_index.write().await;

        let expired_ids: Vec<String> = tokens
            .iter()
            .filter(|(_, t)| t.is_expired())
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired_ids.len();
        for id in &expired_ids {
            if let Some(token) = tokens.remove(id) {
                if let Some(agent_tokens) = index.get_mut(&token.agent_id) {
                    agent_tokens.remove(id);
                }
            }
        }

        if count > 0 {
            tracing::debug!(removed = count, "GC: expired tokens removed");
        }
        count
    }

    /// Retorna contagem total de tokens ativos.
    pub async fn count(&self) -> usize {
        self.tokens.read().await.len()
    }
}

impl Default for CapabilityTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_authorizes_exact() {
        let cap = Capability::new("filesystem", "read", "/data/reports");
        assert!(cap.authorizes("filesystem", "read", "/data/reports"));
        assert!(!cap.authorizes("filesystem", "write", "/data/reports"));
        assert!(!cap.authorizes("network", "read", "/data/reports"));
        assert!(!cap.authorizes("filesystem", "read", "/other/path"));
    }

    #[test]
    fn test_capability_authorizes_wildcard() {
        let cap = Capability::new("filesystem", "read", "*");
        assert!(cap.authorizes("filesystem", "read", "/any/path"));
        assert!(cap.authorizes("filesystem", "read", "/another/path"));
        assert!(!cap.authorizes("filesystem", "write", "/any/path"));
    }

    #[test]
    fn test_capability_authorizes_prefix_wildcard() {
        let cap = Capability::new("memory", "write", "agent-*");
        assert!(cap.authorizes("memory", "write", "agent-123"));
        assert!(cap.authorizes("memory", "write", "agent-abc"));
        assert!(!cap.authorizes("memory", "write", "team-123"));
        assert!(!cap.authorizes("memory", "read", "agent-123"));
    }

    #[test]
    fn test_token_creation() {
        let token = CapabilityToken::new(
            "agent-1",
            vec![Capability::new("filesystem", "read", "/data")],
        );
        assert_eq!(token.agent_id, "agent-1");
        assert!(!token.revoked);
        assert!(!token.fingerprint.is_empty());
        assert_eq!(token.capabilities.len(), 1);
    }

    #[test]
    fn test_token_not_expired() {
        let token = CapabilityToken::new("a", vec![]).with_ttl(3600);
        assert!(!token.is_expired());
    }

    #[test]
    fn test_token_expired() {
        let mut token = CapabilityToken::new("a", vec![]);
        token.ttl_secs = Some(0); // Expirou imediatamente
        token.issued_at = 0; // Epoch
        assert!(token.is_expired());
    }

    #[test]
    fn test_token_no_expiry() {
        let token = CapabilityToken::new("a", vec![]).without_expiry();
        assert!(!token.is_expired());
    }

    #[test]
    fn test_token_authorizes_valid() {
        let token = CapabilityToken::new(
            "agent-1",
            vec![
                Capability::new("filesystem", "read", "/data"),
                Capability::new("network", "egress", "api.example.com"),
            ],
        );
        assert!(token.authorizes("filesystem", "read", "/data").is_ok());
        assert!(token
            .authorizes("network", "egress", "api.example.com")
            .is_ok());
    }

    #[test]
    fn test_token_authorizes_denied() {
        let token = CapabilityToken::new(
            "agent-1",
            vec![Capability::new("filesystem", "read", "/data")],
        );
        let result = token.authorizes("filesystem", "write", "/data");
        assert!(matches!(result, Err(TokenError::CapabilityDenied(_))));
    }

    #[test]
    fn test_token_authorizes_revoked() {
        let mut token =
            CapabilityToken::new("agent-1", vec![Capability::new("filesystem", "read", "*")]);
        token.revoked = true;
        assert_eq!(
            token.authorizes("filesystem", "read", "/anything"),
            Err(TokenError::Revoked)
        );
    }

    #[test]
    fn test_token_authorizes_expired() {
        let mut token = CapabilityToken::new("a", vec![Capability::new("fs", "read", "*")]);
        token.ttl_secs = Some(0);
        token.issued_at = 0;
        assert_eq!(
            token.authorizes("fs", "read", "/x"),
            Err(TokenError::Expired)
        );
    }

    #[tokio::test]
    async fn test_store_issue_and_get() {
        let store = CapabilityTokenStore::new();
        let token = CapabilityToken::new(
            "agent-1",
            vec![Capability::new("filesystem", "read", "/data")],
        );
        let id = store.issue(token).await;

        let retrieved = store.get(&id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().agent_id, "agent-1");
    }

    #[tokio::test]
    async fn test_store_validate_success() {
        let store = CapabilityTokenStore::new();
        let token = CapabilityToken::new(
            "agent-1",
            vec![Capability::new("network", "egress", "api.example.com")],
        );
        let id = store.issue(token).await;

        let result = store
            .validate(&id, "network", "egress", "api.example.com")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_store_validate_denied() {
        let store = CapabilityTokenStore::new();
        let token = CapabilityToken::new(
            "agent-1",
            vec![Capability::new("network", "egress", "api.example.com")],
        );
        let id = store.issue(token).await;

        let result = store.validate(&id, "network", "egress", "evil.com").await;
        assert!(matches!(result, Err(TokenError::CapabilityDenied(_))));
    }

    #[tokio::test]
    async fn test_store_revoke() {
        let store = CapabilityTokenStore::new();
        let token =
            CapabilityToken::new("agent-1", vec![Capability::new("filesystem", "read", "*")]);
        let id = store.issue(token).await;

        // Before revocation: should work
        assert!(store
            .validate(&id, "filesystem", "read", "/x")
            .await
            .is_ok());

        // Revoke
        store.revoke(&id).await.unwrap();

        // After revocation: should fail
        let result = store.validate(&id, "filesystem", "read", "/x").await;
        assert_eq!(result, Err(TokenError::Revoked));
    }

    #[tokio::test]
    async fn test_store_revoke_all_for_agent() {
        let store = CapabilityTokenStore::new();

        let t1 = CapabilityToken::new("agent-x", vec![Capability::new("fs", "read", "*")]);
        let t2 = CapabilityToken::new("agent-x", vec![Capability::new("net", "egress", "*")]);
        let t3 = CapabilityToken::new("agent-y", vec![Capability::new("fs", "read", "*")]);

        let id1 = store.issue(t1).await;
        let id2 = store.issue(t2).await;
        let _id3 = store.issue(t3).await;

        let revoked_count = store.revoke_all_for_agent("agent-x").await;
        assert_eq!(revoked_count, 2);

        // agent-x tokens revoked
        assert_eq!(
            store.validate(&id1, "fs", "read", "/x").await,
            Err(TokenError::Revoked)
        );
        assert_eq!(
            store.validate(&id2, "net", "egress", "x").await,
            Err(TokenError::Revoked)
        );

        // agent-y token still valid
        assert_eq!(store.count().await, 3); // Still in store, just revoked
    }

    #[tokio::test]
    async fn test_store_list_for_agent() {
        let store = CapabilityTokenStore::new();

        store
            .issue(CapabilityToken::new(
                "agent-a",
                vec![Capability::new("fs", "read", "/a")],
            ))
            .await;
        store
            .issue(CapabilityToken::new(
                "agent-a",
                vec![Capability::new("fs", "write", "/a")],
            ))
            .await;
        store
            .issue(CapabilityToken::new(
                "agent-b",
                vec![Capability::new("net", "egress", "*")],
            ))
            .await;

        let list_a = store.list_for_agent("agent-a").await;
        assert_eq!(list_a.len(), 2);

        let list_b = store.list_for_agent("agent-b").await;
        assert_eq!(list_b.len(), 1);

        let list_c = store.list_for_agent("nonexistent").await;
        assert!(list_c.is_empty());
    }

    #[tokio::test]
    async fn test_store_gc_expired() {
        let store = CapabilityTokenStore::new();

        // Token expirado
        let mut expired = CapabilityToken::new("agent-gc", vec![]);
        expired.ttl_secs = Some(0);
        expired.issued_at = 0;
        store.issue(expired).await;

        // Token válido
        store
            .issue(CapabilityToken::new("agent-gc", vec![]).with_ttl(3600))
            .await;

        assert_eq!(store.count().await, 2);

        let removed = store.gc_expired().await;
        assert_eq!(removed, 1);
        assert_eq!(store.count().await, 1);
    }

    #[tokio::test]
    async fn test_store_validate_not_found() {
        let store = CapabilityTokenStore::new();
        let result = store.validate("nonexistent-id", "fs", "read", "/x").await;
        assert!(matches!(result, Err(TokenError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_store_revoke_not_found() {
        let store = CapabilityTokenStore::new();
        let result = store.revoke("nonexistent-id").await;
        assert_eq!(result, Err(TokenError::NotFound("nonexistent-id".into())));
    }

    #[test]
    fn test_token_serialization_roundtrip() {
        let token = CapabilityToken::new(
            "agent-ser",
            vec![
                Capability::new("filesystem", "read", "/data"),
                Capability::new("network", "egress", "*.example.com"),
            ],
        )
        .with_ttl(7200)
        .with_metadata(serde_json::json!({"env": "production"}));

        let json = serde_json::to_string(&token).unwrap();
        let restored: CapabilityToken = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.agent_id, token.agent_id);
        assert_eq!(restored.capabilities.len(), 2);
        assert_eq!(restored.ttl_secs, Some(7200));
        assert_eq!(restored.metadata["env"], "production");
    }

    #[test]
    fn test_capability_serialization_roundtrip() {
        let cap = Capability::new("exec", "docker", "run");
        let json = serde_json::to_string(&cap).unwrap();
        let restored: Capability = serde_json::from_str(&json).unwrap();
        assert_eq!(cap, restored);
    }

    // ---- F11.11.6 least-privilege hardening ----
    #[test]
    fn f11_fs_prefix_respects_path_boundary() {
        let cap = Capability::new("filesystem", "read", "/data*");
        assert!(cap.authorizes("filesystem", "read", "/data"));
        assert!(cap.authorizes("filesystem", "read", "/data/x.txt"));
        assert!(!cap.authorizes("filesystem", "read", "/database"));
        assert!(!cap.authorizes("filesystem", "read", "/data-secret/x"));
    }

    #[test]
    fn f11_fs_traversal_denied() {
        let cap = Capability::new("filesystem", "read", "/data/*");
        assert!(cap.authorizes("filesystem", "read", "/data/a/b"));
        assert!(!cap.authorizes("filesystem", "read", "/data/../etc/passwd"));
        assert!(!cap.authorizes("filesystem", "read", "/data/a/../../etc"));
        assert!(!cap.authorizes("filesystem", "read", "/data\\..\\x"));
        assert!(!cap.authorizes("filesystem", "read", "/data/a\0"));
        let any = Capability::new("filesystem", "read", "*");
        assert!(!any.authorizes("filesystem", "read", "/x/../y"));
    }

    #[test]
    fn f11_network_prefix_wildcard_denied() {
        let cap = Capability::new("network", "egress", "api.example.com*");
        assert!(!cap.authorizes("network", "egress", "api.example.com.evil.net"));
        assert!(!cap.authorizes("network", "egress", "api.example.com"));
    }

    #[test]
    fn f11_network_suffix_wildcard_dot_boundary() {
        let cap = Capability::new("network", "egress", "*.example.com");
        assert!(cap.authorizes("network", "egress", "api.example.com"));
        assert!(cap.authorizes("network", "egress", "API.Example.COM."));
        assert!(!cap.authorizes("network", "egress", "example.com"));
        assert!(!cap.authorizes("network", "egress", "evilexample.com"));
        assert!(!cap.authorizes("network", "egress", "example.com.evil.net"));
        let exact = Capability::new("network", "egress", "api.example.com");
        assert!(exact.authorizes("network", "egress", "Api.Example.com"));
        assert!(!exact.authorizes("network", "egress", "api.example.com.evil.net"));
    }

    #[test]
    fn f11_ttl_overflow_does_not_panic_and_expires_logically() {
        let mut t = CapabilityToken::new("a", vec![]).with_ttl(u64::MAX);
        t.issued_at = u64::MAX - 1;
        assert!(!t.is_expired());
    }

    #[tokio::test]
    async fn f11_validate_for_agent_rejects_foreign_agent() {
        let store = CapabilityTokenStore::new();
        let id = store
            .issue(CapabilityToken::new(
                "agent-a",
                vec![Capability::new("exec", "docker", "run")],
            ))
            .await;
        assert!(store
            .validate_for_agent(&id, "agent-a", "exec", "docker", "run")
            .await
            .is_ok());
        assert!(matches!(
            store
                .validate_for_agent(&id, "agent-b", "exec", "docker", "run")
                .await,
            Err(TokenError::CapabilityDenied(_))
        ));
        store.revoke(&id).await.unwrap();
        assert_eq!(
            store
                .validate_for_agent(&id, "agent-a", "exec", "docker", "run")
                .await
                .unwrap_err(),
            TokenError::Revoked
        );
    }

}
