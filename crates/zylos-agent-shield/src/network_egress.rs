//! # NetworkEgressFilter — Controle de tráfego outbound por agente (T8.3.3)
//!
//! Filtra requisições HTTP outbound baseando-se em whitelist por agente.
//! Implementado como módulo reutilizável que pode ser integrado como:
//! - Middleware Axum no gateway
//! - Proxy sidecar standalone
//! - Check direto em código de agente
//!
//! ## Configuração
//! ```json
//! {
//!   "egress_rules": {
//!     "agent-1": {
//!       "allowed_hosts": ["api.example.com", "github.com"],
//!       "allowed_ports": [443, 80],
//!       "deny_all": false
//!     },
//!     "*": {
//!       "allowed_hosts": ["localhost"],
//!       "allowed_ports": [8080],
//!       "deny_all": false
//!     }
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Regra de egress para um agente ou padrão wildcard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub struct EgressRule {
 /// Hosts permitidos (domínio ou IP). Wildcard `*` permite todos.
 pub allowed_hosts: Vec<String>,
 /// Portas permitidas. Lista vazia = todas as portas permitidas.
 pub allowed_ports: Vec<u16>,
 /// Se true, bloqueia todo o tráfego independente das outras regras.
 pub deny_all: bool,
}


/// Configuração completa de egress filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressConfig {
 /// Regras por agent_id. Chave `"*"` é o fallback default.
 pub egress_rules: HashMap<String, EgressRule>,
 /// Se true, loga tentativas bloqueadas (recomendado em produção).
 pub log_blocked: bool,
 /// Se true, permite DNS resolution para hosts na whitelist.
 pub allow_dns_resolution: bool,
}

impl Default for EgressConfig {
 fn default() -> Self {
 let mut rules = HashMap::new();
 // Default: permitir localhost e portas comuns de dev
 rules.insert(
 "*".into(),
 EgressRule {
 allowed_hosts: vec!["localhost".into(), "127.0.0.1".into()],
 allowed_ports: vec![80, 443, 8080, 8443],
 deny_all: false,
 },
 );
 Self {
 egress_rules: rules,
 log_blocked: true,
 allow_dns_resolution: true,
 }
 }
}

/// Resultado de uma verificação de egress.
#[derive(Debug, Clone, PartialEq)]
pub enum EgressDecision {
 /// Tráfego permitido.
 Allowed,
 /// Tráfego bloqueado com razão.
 Blocked { reason: String },
}

/// Filtro de egress network com suporte a regras por agente.
#[derive(Clone)]
pub struct NetworkEgressFilter {
 config: Arc<RwLock<EgressConfig>>,
}

impl NetworkEgressFilter {
 /// Cria um novo filtro com a configuração fornecida.
 pub fn new(config: EgressConfig) -> Self {
 Self {
 config: Arc::new(RwLock::new(config)),
 }
 }

 /// Cria um filtro com configuração default.
 pub fn default_config() -> Self {
 Self::new(EgressConfig::default())
 }

 /// Atualiza a configuração em runtime (hot-reload).
 pub async fn update_config(&self, config: EgressConfig) {
 let mut guard = self.config.write().await;
 *guard = config;
 debug!("NetworkEgressFilter: config updated");
 }

 /// Verifica se uma conexão outbound é permitida para um agente específico.
 ///
 /// # Arguments
 /// * `agent_id` - ID do agente tentando a conexão
 /// * `host` - Hostname ou IP de destino
 /// * `port` - Porta de destino
 pub async fn check(&self, agent_id: &str, host: &str, port: u16) -> EgressDecision {
 let config = self.config.read().await;

 // 1. Buscar regra específica do agente, depois fallback para "*"
 let rule = config
 .egress_rules
 .get(agent_id)
 .or_else(|| config.egress_rules.get("*"));

 match rule {
 None => {
 // Sem regra definida → permitir por default (fail-open)
 debug!(agent = agent_id, host = host, port = port, "No egress rule found, allowing");
 EgressDecision::Allowed
 }
 Some(rule) => {
 // 2. Verificar deny_all
 if rule.deny_all {
 let reason = format!("Agent '{}' has deny_all=true", agent_id);
 if config.log_blocked {
 warn!(agent = agent_id, host = host, port = port, %reason, "Egress blocked");
 }
 return EgressDecision::Blocked { reason };
 }

 // 3. Verificar host na whitelist
 let host_allowed = rule.allowed_hosts.iter().any(|allowed| {
 if allowed == "*" {
 true
 } else if allowed.starts_with("*.") {
 // Wildcard de subdomínio: *.example.com matches sub.example.com
 let suffix = &allowed[1..]; // ".example.com"
 host.ends_with(suffix) || host == &allowed[2..]
 } else {
 host == allowed
 }
 });

 if !host_allowed {
 let reason = format!(
 "Host '{}' not in whitelist for agent '{}'",
 host, agent_id
 );
 if config.log_blocked {
 warn!(agent = agent_id, host = host, port = port, %reason, "Egress blocked");
 }
 return EgressDecision::Blocked { reason };
 }

 // 4. Verificar porta na whitelist (lista vazia = todas permitidas)
 if !rule.allowed_ports.is_empty() && !rule.allowed_ports.contains(&port) {
 let reason = format!(
 "Port {} not in allowed ports {:?} for agent '{}'",
 port, rule.allowed_ports, agent_id
 );
 if config.log_blocked {
 warn!(agent = agent_id, host = host, port = port, %reason, "Egress blocked");
 }
 return EgressDecision::Blocked { reason };
 }

 EgressDecision::Allowed
 }
 }
 }

 /// Verifica múltiplas conexões em batch.
 pub async fn check_batch(
 &self,
 agent_id: &str,
 targets: &[(String, u16)],
 ) -> Vec<(String, u16, EgressDecision)> {
 let mut results = Vec::with_capacity(targets.len());
 for (host, port) in targets {
 let decision = self.check(agent_id, host, *port).await;
 results.push((host.clone(), *port, decision));
 }
 results
 }

 /// Retorna a regra efetiva para um agente (específica ou wildcard).
 pub async fn effective_rule(&self, agent_id: &str) -> Option<EgressRule> {
 let config = self.config.read().await;
 config
 .egress_rules
 .get(agent_id)
 .or_else(|| config.egress_rules.get("*"))
 .cloned()
 }

 /// Adiciona ou atualiza uma regra para um agente específico.
 pub async fn set_rule(&self, agent_id: impl Into<String>, rule: EgressRule) {
 let mut config = self.config.write().await;
 config.egress_rules.insert(agent_id.into(), rule);
 }

 /// Remove uma regra específica (não remove o wildcard).
 pub async fn remove_rule(&self, agent_id: &str) -> bool {
 let mut config = self.config.write().await;
 config.egress_rules.remove(agent_id).is_some()
 }

 /// Retorna o número de regras configuradas.
 pub async fn rule_count(&self) -> usize {
 let config = self.config.read().await;
 config.egress_rules.len()
 }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
 use super::*;

 #[tokio::test]
 async fn test_default_allows_localhost() {
 let filter = NetworkEgressFilter::default_config();

 let result = filter.check("any-agent", "localhost", 8080).await;
 assert_eq!(result, EgressDecision::Allowed);

 let result = filter.check("any-agent", "127.0.0.1", 443).await;
 assert_eq!(result, EgressDecision::Allowed);
 }

 #[tokio::test]
 async fn test_default_blocks_external() {
 let filter = NetworkEgressFilter::default_config();

 let result = filter.check("any-agent", "evil.com", 443).await;
 assert!(matches!(result, EgressDecision::Blocked { .. }));
 }

 #[tokio::test]
 async fn test_agent_specific_rule() {
 let mut rules = HashMap::new();
 rules.insert(
 "web-agent".into(),
 EgressRule {
 allowed_hosts: vec!["api.example.com".into(), "cdn.example.com".into()],
 allowed_ports: vec![443],
 deny_all: false,
 },
 );
 rules.insert(
 "*".into(),
 EgressRule {
 allowed_hosts: vec!["localhost".into()],
 allowed_ports: vec![8080],
 deny_all: false,
 },
 );

 let filter = NetworkEgressFilter::new(EgressConfig {
 egress_rules: rules,
 log_blocked: false,
 allow_dns_resolution: true,
 });

 // web-agent pode acessar api.example.com:443
 let r1 = filter.check("web-agent", "api.example.com", 443).await;
 assert_eq!(r1, EgressDecision::Allowed);

 // web-agent NÃO pode acessar evil.com
 let r2 = filter.check("web-agent", "evil.com", 443).await;
 assert!(matches!(r2, EgressDecision::Blocked { .. }));

 // web-agent NÃO pode acessar api.example.com:80 (porta não permitida)
 let r3 = filter.check("web-agent", "api.example.com", 80).await;
 assert!(matches!(r3, EgressDecision::Blocked { .. }));

 // Outro agente usa regra wildcard
 let r4 = filter.check("other-agent", "localhost", 8080).await;
 assert_eq!(r4, EgressDecision::Allowed);

 let r5 = filter.check("other-agent", "api.example.com", 443).await;
 assert!(matches!(r5, EgressDecision::Blocked { .. }));
 }

 #[tokio::test]
 async fn test_deny_all() {
 let mut rules = HashMap::new();
 rules.insert(
 "restricted-agent".into(),
 EgressRule {
 allowed_hosts: vec!["*".into()], // Mesmo com wildcard...
 allowed_ports: vec![],
 deny_all: true, // ...deny_all bloqueia tudo
 },
 );

 let filter = NetworkEgressFilter::new(EgressConfig {
 egress_rules: rules,
 log_blocked: false,
 allow_dns_resolution: true,
 });

 let result = filter
 .check("restricted-agent", "anything.com", 443)
 .await;
 assert!(matches!(result, EgressDecision::Blocked { .. }));
 if let EgressDecision::Blocked { reason } = result {
 assert!(reason.contains("deny_all"));
 }
 }

 #[tokio::test]
 async fn test_wildcard_host() {
 let mut rules = HashMap::new();
 rules.insert(
 "*".into(),
 EgressRule {
 allowed_hosts: vec!["*".into()],
 allowed_ports: vec![443, 80],
 deny_all: false,
 },
 );

 let filter = NetworkEgressFilter::new(EgressConfig {
 egress_rules: rules,
 log_blocked: false,
 allow_dns_resolution: true,
 });

 // Qualquer host deve ser permitido
 assert_eq!(
 filter.check("any", "example.com", 443).await,
 EgressDecision::Allowed
 );
 assert_eq!(
 filter.check("any", "sub.domain.org", 80).await,
 EgressDecision::Allowed
 );

 // Porta não permitida
 assert!(matches!(
 filter.check("any", "example.com", 22).await,
 EgressDecision::Blocked { .. }
 ));
 }

 #[tokio::test]
 async fn test_subdomain_wildcard() {
 let mut rules = HashMap::new();
 rules.insert(
 "*".into(),
 EgressRule {
 allowed_hosts: vec!["*.example.com".into()],
 allowed_ports: vec![],
 deny_all: false,
 },
 );

 let filter = NetworkEgressFilter::new(EgressConfig {
 egress_rules: rules,
 log_blocked: false,
 allow_dns_resolution: true,
 });

 // Subdomínios devem ser permitidos
 assert_eq!(
 filter.check("a", "api.example.com", 443).await,
 EgressDecision::Allowed
 );
 assert_eq!(
 filter.check("a", "deep.sub.example.com", 443).await,
 EgressDecision::Allowed
 );

 // Domínio raiz também deve ser permitido
 assert_eq!(
 filter.check("a", "example.com", 443).await,
 EgressDecision::Allowed
 );

 // Domínio diferente deve ser bloqueado
 assert!(matches!(
 filter.check("a", "evil.com", 443).await,
 EgressDecision::Blocked { .. }
 ));
 }

 #[tokio::test]
 async fn test_empty_ports_means_all_allowed() {
 let mut rules = HashMap::new();
 rules.insert(
 "*".into(),
 EgressRule {
 allowed_hosts: vec!["example.com".into()],
 allowed_ports: vec![], // Vazio = todas as portas
 deny_all: false,
 },
 );

 let filter = NetworkEgressFilter::new(EgressConfig {
 egress_rules: rules,
 log_blocked: false,
 allow_dns_resolution: true,
 });

 assert_eq!(
 filter.check("a", "example.com", 22).await,
 EgressDecision::Allowed
 );
 assert_eq!(
 filter.check("a", "example.com", 9999).await,
 EgressDecision::Allowed
 );
 }

 #[tokio::test]
 async fn test_no_rule_fallback_open() {
 // Config sem nenhuma regra → fail-open
 let filter = NetworkEgressFilter::new(EgressConfig {
 egress_rules: HashMap::new(),
 log_blocked: false,
 allow_dns_resolution: true,
 });

 let result = filter.check("any-agent", "anything.com", 443).await;
 assert_eq!(result, EgressDecision::Allowed);
 }

 #[tokio::test]
 async fn test_check_batch() {
 let filter = NetworkEgressFilter::default_config();

 let targets = vec![
 ("localhost".to_string(), 8080u16),
 ("evil.com".to_string(), 443u16),
 ("127.0.0.1".to_string(), 443u16),
 ];

 let results = filter.check_batch("test-agent", &targets).await;
 assert_eq!(results.len(), 3);
 assert_eq!(results[0].2, EgressDecision::Allowed);
 assert!(matches!(results[1].2, EgressDecision::Blocked { .. }));
 assert_eq!(results[2].2, EgressDecision::Allowed);
 }

 #[tokio::test]
 async fn test_set_and_remove_rule() {
 let filter = NetworkEgressFilter::default_config();

 // Adicionar regra customizada
 filter
 .set_rule(
 "custom-agent",
 EgressRule {
 allowed_hosts: vec!["custom.api.com".into()],
 allowed_ports: vec![443],
 deny_all: false,
 },
 )
 .await;

 let r1 = filter.check("custom-agent", "custom.api.com", 443).await;
 assert_eq!(r1, EgressDecision::Allowed);

 let r2 = filter.check("custom-agent", "other.com", 443).await;
 assert!(matches!(r2, EgressDecision::Blocked { .. }));

 // Remover regra → volta para wildcard
 assert!(filter.remove_rule("custom-agent").await);
 let r3 = filter.check("custom-agent", "custom.api.com", 443).await;
 // Agora usa wildcard default que só permite localhost
 assert!(matches!(r3, EgressDecision::Blocked { .. }));
 }

 #[tokio::test]
 async fn test_effective_rule() {
 let filter = NetworkEgressFilter::default_config();

 // Agente sem regra específica → retorna wildcard
 let rule = filter.effective_rule("unknown-agent").await;
 assert!(rule.is_some());
 assert!(rule.unwrap().allowed_hosts.contains(&"localhost".to_string()));

 // Adicionar regra específica
 filter
 .set_rule(
 "special",
 EgressRule {
 allowed_hosts: vec!["special.com".into()],
 allowed_ports: vec![443],
 deny_all: false,
 },
 )
 .await;

 let rule = filter.effective_rule("special").await;
 assert!(rule.is_some());
 assert_eq!(rule.unwrap().allowed_hosts, vec!["special.com"]);
 }

 #[tokio::test]
 async fn test_update_config_hot_reload() {
 let filter = NetworkEgressFilter::default_config();

 // Inicialmente evil.com é bloqueado
 assert!(matches!(
 filter.check("a", "evil.com", 443).await,
 EgressDecision::Blocked { .. }
 ));

 // Hot-reload com nova config que permite tudo
 filter
 .update_config(EgressConfig {
 egress_rules: HashMap::from([(
 "*".into(),
 EgressRule {
 allowed_hosts: vec!["*".into()],
 allowed_ports: vec![],
 deny_all: false,
 },
 )]),
 log_blocked: false,
 allow_dns_resolution: true,
 })
 .await;

 // Agora evil.com deve ser permitido
 assert_eq!(
 filter.check("a", "evil.com", 443).await,
 EgressDecision::Allowed
 );
 }

 #[tokio::test]
 async fn test_rule_count() {
 let filter = NetworkEgressFilter::default_config();
 assert_eq!(filter.rule_count().await, 1); // Apenas wildcard

 filter
 .set_rule("a", EgressRule::default())
 .await;
 assert_eq!(filter.rule_count().await, 2);

 filter.remove_rule("a").await;
 assert_eq!(filter.rule_count().await, 1);
 }

 #[test]
 fn test_egress_config_serialization() {
 let config = EgressConfig::default();
 let json = serde_json::to_string_pretty(&config).unwrap();
 let restored: EgressConfig = serde_json::from_str(&json).unwrap();
 assert_eq!(restored.egress_rules.len(), config.egress_rules.len());
 }

 #[test]
 fn test_egress_rule_serialization() {
 let rule = EgressRule {
 allowed_hosts: vec!["api.com".into(), "*.cdn.com".into()],
 allowed_ports: vec![443, 80],
 deny_all: false,
 };
 let json = serde_json::to_string(&rule).unwrap();
 let restored: EgressRule = serde_json::from_str(&json).unwrap();
 assert_eq!(rule, restored);
 }
}