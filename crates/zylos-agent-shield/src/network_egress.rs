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
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Regra de egress para um agente ou padrão wildcard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
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
        let decision = evaluate(&config, agent_id, host, port);
        if let EgressDecision::Blocked { reason } = &decision {
            if config.log_blocked {
                warn!(agent = agent_id, host = host, port = port, %reason, "Egress blocked");
            }
        } else {
            debug!(agent = agent_id, host = host, port = port, "Egress allowed");
        }
        decision
    }

    /// Verifica uma URL completa (`scheme://host[:port]/...`).
    ///
    /// Rejeita userinfo (`user@host`), esquemas fora de http/https e hosts
    /// malformados. Deve ser chamado **a cada salto de redirect** (o cliente
    /// HTTP precisa ter redirects automáticos desabilitados ou usar uma
    /// policy que invoque este método).
    pub async fn check_url(&self, agent_id: &str, url: &str) -> EgressDecision {
        match parse_url_target(url) {
            Ok((host, port)) => self.check(agent_id, &host, port).await,
            Err(reason) => {
                let config = self.config.read().await;
                if config.log_blocked {
                    warn!(agent = agent_id, url = url, %reason, "Egress blocked (url)");
                }
                EgressDecision::Blocked { reason }
            }
        }
    }

    /// Valida um redirect: o destino é tratado como uma nova requisição.
    pub async fn check_redirect(&self, agent_id: &str, location: &str) -> EgressDecision {
        self.check_url(agent_id, location).await
    }

    /// Valida os IPs efetivamente resolvidos para `host` (anti DNS-rebinding).
    ///
    /// O chamador deve conectar **exatamente** a um dos IPs validados aqui
    /// (IP pinning). Qualquer IP interno (loopback, privado, link-local,
    /// metadata) bloqueia a conexão, a menos que o IP esteja listado
    /// explicitamente na whitelist do agente.
    pub async fn check_resolved(
        &self,
        agent_id: &str,
        host: &str,
        port: u16,
        resolved: &[IpAddr],
    ) -> EgressDecision {
        let first = self.check(agent_id, host, port).await;
        if first != EgressDecision::Allowed {
            return first;
        }
        if resolved.is_empty() {
            return EgressDecision::Blocked {
                reason: format!("Host '{}' resolved to no addresses", host),
            };
        }
        let config = self.config.read().await;
        let rule = match lookup_rule(&config, agent_id) {
            Some(r) => r,
            None => {
                return EgressDecision::Blocked {
                    reason: "no egress rule".into(),
                }
            }
        };
        for ip in resolved {
            let ip = canonical_ip(*ip);
            if is_internal_ip(&ip) && !explicitly_allows_ip(rule, &ip) {
                let reason = format!(
                    "Host '{}' resolved to internal address {} (possible DNS rebinding)",
                    host, ip
                );
                if config.log_blocked {
                    warn!(agent = agent_id, host = host, port = port, %reason, "Egress blocked");
                }
                return EgressDecision::Blocked { reason };
            }
        }
        EgressDecision::Allowed
    }

    /// Resolve `host` via DNS (tokio) e aplica [`Self::check_resolved`].
    /// Retorna a decisão e os IPs validados (para pinning).
    pub async fn check_with_dns(
        &self,
        agent_id: &str,
        host: &str,
        port: u16,
    ) -> (EgressDecision, Vec<IpAddr>) {
        let pre = self.check(agent_id, host, port).await;
        if pre != EgressDecision::Allowed {
            return (pre, vec![]);
        }
        let allow_dns = self.config.read().await.allow_dns_resolution;
        let target = match normalize_host(host) {
            Ok(HostKind::Ip(ip)) => return (EgressDecision::Allowed, vec![ip]),
            Ok(HostKind::Name(n)) => n,
            Err(reason) => return (EgressDecision::Blocked { reason }, vec![]),
        };
        if !allow_dns {
            return (
                EgressDecision::Blocked {
                    reason: "DNS resolution disabled by config".into(),
                },
                vec![],
            );
        }
        let ips: Vec<IpAddr> = match tokio::net::lookup_host((target.as_str(), port)).await {
            Ok(it) => it.map(|sa| sa.ip()).collect(),
            Err(e) => {
                return (
                    EgressDecision::Blocked {
                        reason: format!("DNS resolution failed for '{}': {}", target, e),
                    },
                    vec![],
                )
            }
        };
        let d = self.check_resolved(agent_id, host, port, &ips).await;
        if d == EgressDecision::Allowed {
            (d, ips)
        } else {
            (d, vec![])
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
// Normalização / avaliação (hardening F11.11.3)
// ---------------------------------------------------------------------------

/// Host normalizado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKind {
    /// Endereço IP literal (qualquer codificação aceita por `inet_aton`).
    Ip(IpAddr),
    /// Nome DNS normalizado (minúsculo, sem ponto final).
    Name(String),
}

fn lookup_rule<'a>(config: &'a EgressConfig, agent_id: &str) -> Option<&'a EgressRule> {
    config
        .egress_rules
        .get(agent_id)
        .or_else(|| config.egress_rules.get("*"))
}

/// Converte IPv4-mapped / IPv4-compatible IPv6 para IPv4.
fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return IpAddr::V4(v4);
            }
            let seg = v6.segments();
            // ::a.b.c.d (IPv4-compatible, deprecated) — exceto :: e ::1
            if seg[..6].iter().all(|s| *s == 0) && !(seg[6] == 0 && seg[7] <= 1) {
                let o = v6.octets();
                return IpAddr::V4(Ipv4Addr::new(o[12], o[13], o[14], o[15]));
            }
            IpAddr::V6(v6)
        }
        v4 => v4,
    }
}

/// IP que nunca deve ser alcançado via wildcard (SSRF): loopback, privado,
/// link-local (inclui metadata 169.254.169.254), CGNAT, unspecified,
/// broadcast, multicast, ULA IPv6, documentação, etc.
pub fn is_internal_ip(ip: &IpAddr) -> bool {
    match canonical_ip(*ip) {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || o[0] == 0
                || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64.0.0/10 CGNAT
                || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0.0/24
                || (o[0] == 198 && (o[1] & 0xfe) == 18) // 198.18.0.0/15
                || o[0] >= 240 // reservado
        }
        IpAddr::V6(v6) => {
            let seg = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg[0] & 0xfe00) == 0xfc00 // ULA fc00::/7
                || (seg[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
                || (seg[0] & 0xffc0) == 0xfec0 // site-local (deprecated)
                || (seg[0] == 0x2001 && seg[1] == 0x0db8) // documentação
                || seg[0] == 0x0064 && seg[1] == 0xff9b // NAT64 (pode mapear p/ interno)
                || seg[0] == 0x2002 // 6to4 embute IPv4 arbitrário
        }
    }
}

/// Parse IPv4 no estilo `inet_aton` (decimal/hex/octal, 1–4 partes).
/// Retorna `None` se não for um literal numérico.
fn parse_ipv4_loose(s: &str) -> Option<Ipv4Addr> {
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() > 4 {
        return None;
    }
    let mut nums = Vec::with_capacity(parts.len());
    for p in &parts {
        if p.is_empty() {
            return None;
        }
        let v = if let Some(h) = p.strip_prefix("0x").or_else(|| p.strip_prefix("0X")) {
            if h.is_empty() {
                0
            } else {
                u64::from_str_radix(h, 16).ok()?
            }
        } else if p.len() > 1 && p.starts_with('0') {
            u64::from_str_radix(&p[1..], 8).ok()?
        } else {
            p.parse::<u64>().ok()?
        };
        nums.push(v);
    }
    let n = nums.len();
    let mut acc: u64 = 0;
    for (i, v) in nums.iter().enumerate() {
        if i < n - 1 {
            if *v > 255 {
                return None;
            }
            acc |= v << (8 * (3 - i));
        } else {
            let bits = 8 * (4 - i);
            if bits < 64 && *v >= (1u64 << bits) {
                return None;
            }
            acc |= v;
        }
    }
    u32::try_from(acc).ok().map(Ipv4Addr::from)
}

/// Normaliza um host recebido de um agente.
///
/// - trim, minúsculo, remove ponto final (`example.com.`)
/// - rejeita userinfo (`@`), barras, `\`, `?`, `#`, `%`, espaços, controle
/// - IPv6 com ou sem colchetes; IPv4-mapped vira IPv4
/// - IPv4 em decimal/hex/octal/curto (`2130706433`, `0x7f.1`, `0177.0.0.1`, `127.1`)
pub fn normalize_host(raw: &str) -> Result<HostKind, String> {
    let h = raw.trim();
    if h.is_empty() {
        return Err("empty host".into());
    }
    if h.chars()
        .any(|c| c.is_control() || c.is_whitespace() || "@/\\?#%".contains(c))
    {
        return Err(format!("Host '{}' contains forbidden characters", raw));
    }
    let h = h.to_ascii_lowercase();
    if let Some(inner) = h.strip_prefix('[') {
        let inner = inner
            .strip_suffix(']')
            .ok_or_else(|| format!("Host '{}' has unbalanced brackets", raw))?;
        let v6: Ipv6Addr = inner
            .parse()
            .map_err(|_| format!("Host '{}' is not a valid IPv6 literal", raw))?;
        return Ok(HostKind::Ip(canonical_ip(IpAddr::V6(v6))));
    }
    if h.contains(':') {
        let v6: Ipv6Addr = h
            .parse()
            .map_err(|_| format!("Host '{}' is not a valid IPv6 literal", raw))?;
        return Ok(HostKind::Ip(canonical_ip(IpAddr::V6(v6))));
    }
    let h = h.strip_suffix('.').unwrap_or(&h).to_string();
    if h.is_empty() || h.contains("..") || h.starts_with('.') {
        return Err(format!("Host '{}' is malformed", raw));
    }
    if let Some(v4) = parse_ipv4_loose(&h) {
        return Ok(HostKind::Ip(IpAddr::V4(v4)));
    }
    // Última label puramente numérica/hex que não parseou = IP malformado.
    let last = h.rsplit('.').next().unwrap_or("");
    if !last.is_empty()
        && (last.chars().all(|c| c.is_ascii_digit()) || last.starts_with("0x"))
    {
        return Err(format!("Host '{}' looks like a malformed IP literal", raw));
    }
    if !h
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
    {
        // Bloqueia unicode/IDN não convertido (homógrafos); exija punycode.
        return Err(format!("Host '{}' must be ASCII (use punycode)", raw));
    }
    Ok(HostKind::Name(h))
}

/// Nomes que sempre apontam para a própria máquina / rede interna.
fn is_internal_name(name: &str) -> bool {
    name == "localhost"
        || name.ends_with(".localhost")
        || name.ends_with(".local")
        || name.ends_with(".internal")
        || name == "metadata"
        || name == "metadata.google.internal"
}

fn explicitly_allows_ip(rule: &EgressRule, ip: &IpAddr) -> bool {
    rule.allowed_hosts
        .iter()
        .any(|a| matches!(normalize_host(a), Ok(HostKind::Ip(x)) if x == *ip))
}

fn pattern_matches(pattern: &str, host: &HostKind) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        let suffix = suffix.trim_end_matches('.').to_ascii_lowercase();
        return match host {
            HostKind::Name(n) => n == &suffix || n.ends_with(&format!(".{}", suffix)),
            HostKind::Ip(_) => false,
        };
    }
    match normalize_host(pattern) {
        Ok(p) => &p == host,
        Err(_) => false,
    }
}

/// Avaliação pura (sem lock) — fail-closed.
fn evaluate(config: &EgressConfig, agent_id: &str, host: &str, port: u16) -> EgressDecision {
    let Some(rule) = lookup_rule(config, agent_id) else {
        // F11.11.3: fail-closed — sem regra = bloqueado.
        return EgressDecision::Blocked {
            reason: format!("No egress rule for agent '{}' (fail-closed)", agent_id),
        };
    };
    if rule.deny_all {
        return EgressDecision::Blocked {
            reason: format!("Agent '{}' has deny_all=true", agent_id),
        };
    }
    if port == 0 {
        return EgressDecision::Blocked {
            reason: "Port 0 is not allowed".into(),
        };
    }
    let target = match normalize_host(host) {
        Ok(t) => t,
        Err(reason) => return EgressDecision::Blocked { reason },
    };

    // Destinos internos só passam com entrada EXATA (não via "*" ou "*.x").
    let internal = match &target {
        HostKind::Ip(ip) => is_internal_ip(ip),
        HostKind::Name(n) => is_internal_name(n),
    };
    let host_allowed = rule.allowed_hosts.iter().any(|allowed| {
        let exact = allowed != "*" && !allowed.starts_with("*.");
        if internal && !exact {
            return false;
        }
        pattern_matches(allowed, &target)
    });
    if !host_allowed {
        let reason = if internal {
            format!(
                "Host '{}' is internal/loopback and not explicitly whitelisted for agent '{}'",
                host, agent_id
            )
        } else {
            format!("Host '{}' not in whitelist for agent '{}'", host, agent_id)
        };
        return EgressDecision::Blocked { reason };
    }

    if !rule.allowed_ports.is_empty() && !rule.allowed_ports.contains(&port) {
        return EgressDecision::Blocked {
            reason: format!(
                "Port {} not in allowed ports {:?} for agent '{}'",
                port, rule.allowed_ports, agent_id
            ),
        };
    }
    EgressDecision::Allowed
}

/// Extrai (host, porta) de uma URL http(s). Rejeita userinfo e outros esquemas.
pub fn parse_url_target(url: &str) -> Result<(String, u16), String> {
    let u = url.trim();
    let (scheme, rest) = u
        .split_once("://")
        .ok_or_else(|| format!("URL '{}' has no scheme", url))?;
    let default_port = match scheme.to_ascii_lowercase().as_str() {
        "https" => 443,
        "http" => 80,
        other => return Err(format!("Scheme '{}' not allowed for egress", other)),
    };
    // authority termina no primeiro '/', '?', '#' ou '\\' (browsers tratam '\\' como '/')
    let end = rest
        .find(['/', '?', '#', '\\'])
        .unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.contains('@') {
        return Err(format!("URL '{}' contains userinfo (user@host) — rejected", url));
    }
    if authority.is_empty() {
        return Err(format!("URL '{}' has empty host", url));
    }
    let (host, port) = if let Some(after) = authority.strip_prefix('[') {
        let close = after
            .find(']')
            .ok_or_else(|| format!("URL '{}' has unbalanced IPv6 brackets", url))?;
        let host = &authority[..close + 2];
        let tail = &after[close + 1..];
        let port = match tail.strip_prefix(':') {
            Some(p) => p
                .parse::<u16>()
                .map_err(|_| format!("URL '{}' has invalid port", url))?,
            None if tail.is_empty() => default_port,
            None => return Err(format!("URL '{}' has garbage after IPv6 host", url)),
        };
        (host.to_string(), port)
    } else if let Some((h, p)) = authority.rsplit_once(':') {
        let port = if p.is_empty() {
            default_port
        } else {
            p.parse::<u16>()
                .map_err(|_| format!("URL '{}' has invalid port", url))?
        };
        (h.to_string(), port)
    } else {
        (authority.to_string(), default_port)
    };
    // valida o host já aqui (erros mais claros)
    normalize_host(&host)?;
    Ok((host, port))
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

        let result = filter.check("restricted-agent", "anything.com", 443).await;
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
    async fn test_no_rule_fallback_closed() {
        // F11.11.3: config sem nenhuma regra → fail-closed
        let filter = NetworkEgressFilter::new(EgressConfig {
            egress_rules: HashMap::new(),
            log_blocked: false,
            allow_dns_resolution: true,
        });

        let result = filter.check("any-agent", "anything.com", 443).await;
        assert!(matches!(result, EgressDecision::Blocked { .. }));
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
        assert!(rule
            .unwrap()
            .allowed_hosts
            .contains(&"localhost".to_string()));

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

        filter.set_rule("a", EgressRule::default()).await;
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

    // -----------------------------------------------------------------
    // F11.11.3 — tentativas de bypass
    // -----------------------------------------------------------------

    fn filter_with(hosts: &[&str], ports: &[u16]) -> NetworkEgressFilter {
        NetworkEgressFilter::new(EgressConfig {
            egress_rules: HashMap::from([(
                "*".to_string(),
                EgressRule {
                    allowed_hosts: hosts.iter().map(|h| h.to_string()).collect(),
                    allowed_ports: ports.to_vec(),
                    deny_all: false,
                },
            )]),
            log_blocked: false,
            allow_dns_resolution: true,
        })
    }

    fn blocked(d: &EgressDecision) -> bool {
        matches!(d, EgressDecision::Blocked { .. })
    }

    #[tokio::test]
    async fn bypass_wildcard_star_does_not_reach_internal_ips() {
        let f = filter_with(&["*"], &[]);
        for h in [
            "127.0.0.1",
            "localhost",
            "LOCALHOST",
            "localhost.",
            "foo.localhost",
            "169.254.169.254",
            "metadata.google.internal",
            "10.0.0.1",
            "172.16.5.4",
            "192.168.1.1",
            "100.64.0.1",
            "0.0.0.0",
            "0",
            "[::1]",
            "::1",
            "[::]",
            "[::ffff:127.0.0.1]",
            "[::ffff:7f00:1]",
            "[::ffff:169.254.169.254]",
            "[fe80::1]",
            "[fd00::1]",
            "[::127.0.0.1]",
        ] {
            assert!(blocked(&f.check("a", h, 80).await), "should block {h}");
        }
        assert_eq!(f.check("a", "example.com", 80).await, EgressDecision::Allowed);
        assert_eq!(f.check("a", "8.8.8.8", 53).await, EgressDecision::Allowed);
    }

    #[tokio::test]
    async fn bypass_alternate_ipv4_encodings() {
        let f = filter_with(&["*"], &[]);
        for h in [
            "2130706433",      // decimal 127.0.0.1
            "0x7f000001",      // hex
            "0x7f.0.0.1",      // hex parcial
            "0177.0.0.1",      // octal
            "017700000001",    // octal inteiro
            "127.1",           // forma curta
            "127.0.1",         // forma curta 3 partes
            "2852039166",      // decimal 169.254.169.254
            "0xA9FEA9FE",      // hex 169.254.169.254
            "0251.0376.0251.0376", // octal metadata
            "127.0.0.1.",      // ponto final
        ] {
            assert!(blocked(&f.check("a", h, 80).await), "should block {h}");
        }
    }

    #[test]
    fn normalize_host_decodes_encodings() {
        let lo = HostKind::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(normalize_host("2130706433").unwrap(), lo);
        assert_eq!(normalize_host("0x7f000001").unwrap(), lo);
        assert_eq!(normalize_host("0177.0.0.1").unwrap(), lo);
        assert_eq!(normalize_host("127.1").unwrap(), lo);
        assert_eq!(normalize_host("[::ffff:127.0.0.1]").unwrap(), lo);
        assert_eq!(
            normalize_host("API.Example.COM.").unwrap(),
            HostKind::Name("api.example.com".into())
        );
        assert!(normalize_host("256.0.0.1").is_err());
        assert!(normalize_host("1.2.3.4.5").is_err());
        assert!(normalize_host("[::1").is_err());
    }

    #[tokio::test]
    async fn bypass_exact_whitelist_case_and_trailing_dot_still_match() {
        let f = filter_with(&["api.example.com"], &[443]);
        assert_eq!(f.check("a", "API.EXAMPLE.COM", 443).await, EgressDecision::Allowed);
        assert_eq!(f.check("a", "api.example.com.", 443).await, EgressDecision::Allowed);
        assert!(blocked(&f.check("a", "api.example.com.evil.com", 443).await));
        assert!(blocked(&f.check("a", "evilapi.example.com", 443).await));
    }

    #[tokio::test]
    async fn bypass_subdomain_wildcard_suffix_confusion() {
        let f = filter_with(&["*.example.com"], &[]);
        // antes: ends_with(".example.com") ok, mas checar sufixos similares
        assert!(blocked(&f.check("a", "evilexample.com", 443).await));
        assert!(blocked(&f.check("a", "example.com.evil.com", 443).await));
        assert_eq!(f.check("a", "A.EXAMPLE.COM.", 443).await, EgressDecision::Allowed);
        // wildcard de domínio nunca casa IP
        assert!(blocked(&f.check("a", "93.184.216.34", 443).await));
    }

    #[tokio::test]
    async fn bypass_userinfo_and_forbidden_chars_in_host() {
        let f = filter_with(&["*"], &[]);
        for h in [
            "example.com@127.0.0.1",
            "user:pass@example.com",
            "example.com/../",
            "example.com\\@127.0.0.1",
            "example.com#@127.0.0.1",
            "example.com?x",
            "exa mple.com",
            "example.com%00",
            "exa\u{0}mple.com",
            "",
            "..",
            "ex..ample.com",
        ] {
            assert!(blocked(&f.check("a", h, 443).await), "should block {h:?}");
        }
    }

    #[tokio::test]
    async fn bypass_unicode_homograph_rejected() {
        let f = filter_with(&["*"], &[]);
        // 'е' cirílico
        assert!(blocked(&f.check("a", "\u{0435}xample.com", 443).await));
        // fullwidth digits / dots
        assert!(blocked(&f.check("a", "１２７.０.０.１", 443).await));
    }

    #[tokio::test]
    async fn explicit_internal_whitelist_still_works() {
        let f = filter_with(&["127.0.0.1", "[::1]"], &[8080]);
        assert_eq!(f.check("a", "127.0.0.1", 8080).await, EgressDecision::Allowed);
        // encodings do mesmo IP explicitamente permitido também casam (normalizados)
        assert_eq!(f.check("a", "2130706433", 8080).await, EgressDecision::Allowed);
        assert_eq!(f.check("a", "::1", 8080).await, EgressDecision::Allowed);
        assert!(blocked(&f.check("a", "127.0.0.2", 8080).await));
    }

    #[tokio::test]
    async fn port_zero_blocked() {
        let f = filter_with(&["*"], &[]);
        assert!(blocked(&f.check("a", "example.com", 0).await));
    }

    #[tokio::test]
    async fn check_url_parsing_and_userinfo() {
        let f = filter_with(&["api.example.com"], &[443]);
        assert_eq!(
            f.check_url("a", "https://api.example.com/v1/x?y=1").await,
            EgressDecision::Allowed
        );
        assert_eq!(
            f.check_url("a", "HTTPS://API.example.com:443/").await,
            EgressDecision::Allowed
        );
        for u in [
            "https://api.example.com@127.0.0.1/",
            "https://user:pw@api.example.com/",
            "https://api.example.com:80/",
            "http://api.example.com/", // porta 80 não permitida
            "file:///etc/passwd",
            "gopher://api.example.com/",
            "ftp://api.example.com/",
            "https://api.example.com:99999/",
            "https://127.0.0.1\\@api.example.com/",
            "api.example.com",
            "https:///x",
        ] {
            assert!(blocked(&f.check_url("a", u).await), "should block {u}");
        }
    }

    #[tokio::test]
    async fn check_url_ipv6_forms() {
        let f = filter_with(&["*"], &[]);
        assert!(blocked(&f.check_url("a", "http://[::1]:8080/").await));
        assert!(blocked(&f.check_url("a", "http://[::ffff:10.0.0.1]/").await));
        assert!(blocked(&f.check_url("a", "http://[::1/").await));
        assert_eq!(
            f.check_url("a", "http://[2606:4700:4700::1111]/").await,
            EgressDecision::Allowed
        );
    }

    #[tokio::test]
    async fn redirect_to_internal_is_blocked() {
        let f = filter_with(&["*"], &[]);
        assert_eq!(
            f.check_url("a", "https://example.com/start").await,
            EgressDecision::Allowed
        );
        for loc in [
            "http://169.254.169.254/latest/meta-data/",
            "http://localhost:6379/",
            "http://0x7f000001/",
            "http://[::ffff:127.0.0.1]/",
            "file:///etc/passwd",
        ] {
            assert!(blocked(&f.check_redirect("a", loc).await), "should block {loc}");
        }
    }

    #[tokio::test]
    async fn dns_rebinding_resolved_internal_ip_blocked() {
        let f = filter_with(&["*.attacker.com"], &[]);
        // nome público que resolve para IP interno (rebinding / nip.io-like)
        let d = f
            .check_resolved(
                "a",
                "rebind.attacker.com",
                80,
                &[IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
            )
            .await;
        assert!(blocked(&d));
        let d = f
            .check_resolved(
                "a",
                "rebind.attacker.com",
                80,
                &["::ffff:169.254.169.254".parse().unwrap()],
            )
            .await;
        assert!(blocked(&d));
        // qualquer IP interno no conjunto bloqueia (round-robin)
        let d = f
            .check_resolved(
                "a",
                "rr.attacker.com",
                80,
                &[
                    IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
                    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                ],
            )
            .await;
        assert!(blocked(&d));
        let d = f
            .check_resolved(
                "a",
                "ok.attacker.com",
                80,
                &[IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
            )
            .await;
        assert_eq!(d, EgressDecision::Allowed);
        assert!(blocked(&f.check_resolved("a", "ok.attacker.com", 80, &[]).await));
    }

    #[tokio::test]
    async fn dns_check_with_literal_ip_skips_resolution() {
        let f = filter_with(&["*"], &[]);
        let (d, ips) = f.check_with_dns("a", "8.8.8.8", 53).await;
        assert_eq!(d, EgressDecision::Allowed);
        assert_eq!(ips, vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))]);
        let (d, _) = f.check_with_dns("a", "0x7f000001", 80).await;
        assert!(blocked(&d));
    }

    #[test]
    fn internal_ip_classification() {
        for ip in [
            "127.0.0.1", "10.1.2.3", "172.31.0.1", "192.168.0.1", "169.254.169.254",
            "100.100.100.200", "0.0.0.0", "255.255.255.255", "224.0.0.1", "::1", "::",
            "fe80::1", "fc00::1", "::ffff:192.168.1.1", "64:ff9b::7f00:1", "2002:7f00:1::",
        ] {
            let ip: IpAddr = ip.parse().unwrap();
            assert!(is_internal_ip(&ip), "{ip} should be internal");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111", "172.32.0.1"] {
            let ip: IpAddr = ip.parse().unwrap();
            assert!(!is_internal_ip(&ip), "{ip} should be public");
        }
    }
}
