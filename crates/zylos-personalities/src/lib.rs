//! # zylos-personalities
//!
//! Personality-driven agent identities with division taxonomy for zylos-gateway.
//!
//! Provides a structured system for defining agent personalities organized by
//! functional divisions (Engineering, Research, Operations, Creative, Security).
//! Each personality includes behavioral traits, communication style, expertise
//! domains, and collaboration preferences that shape how agents interact.
//!
//! ## Employee Roles (T8.5.1)
//! O módulo `roles` define 8 papéis de negócio padronizados para AI Employees,
//! cada um mapeado para um personality preset e conjunto de rotinas padrão.

/// Módulo guardrails — GuardrailEnforcer com regras declarativas por role/recurso (T8.5.5).
pub mod guardrails;
/// Módulo plan_first_gate — Gate de aprovação humana antes de execução (T8.5.3).
pub mod plan_first_gate;
/// Módulo rate_limiter — EmployeeRateLimiter com token bucket por role/employee (T8.5.4).
pub mod rate_limiter;
pub mod roles;
/// Módulo routine_catalog — Catálogo indexado de rotinas por employee role (T8.5.2).
pub mod routine_catalog;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::Row;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Core Types
// ---------------------------------------------------------------------------

/// Functional division that groups related personalities.
///
/// Divisions organize agent personalities into logical categories reflecting
/// organizational roles. Each personality belongs to exactly one division.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Division {
    /// Software engineering and systems development.
    Engineering,
    /// Research, analysis, and knowledge synthesis.
    Research,
    /// Infrastructure, deployment, and operational workflows.
    Operations,
    /// Design, UX, and creative problem-solving.
    Creative,
    /// Security auditing, threat modeling, and compliance.
    Security,
    /// Project management, coordination, and planning.
    Management,
    /// User support, onboarding, and assistance.
    Support,
    /// User-defined division for custom personalities.
    Custom,
}

impl std::fmt::Display for Division {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Engineering => write!(f, "engineering"),
            Self::Research => write!(f, "research"),
            Self::Operations => write!(f, "operations"),
            Self::Creative => write!(f, "creative"),
            Self::Security => write!(f, "security"),
            Self::Management => write!(f, "management"),
            Self::Support => write!(f, "support"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl Division {
    /// Alias para `from_str` — evita warning clippy `should_implement_trait`.
    pub fn parse(s: &str) -> Result<Self, String> {
        <Self as FromStr>::from_str(s)
    }
}

impl FromStr for Division {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "engineering" | "eng" => Ok(Self::Engineering),
            "research" => Ok(Self::Research),
            "operations" | "ops" => Ok(Self::Operations),
            "creative" => Ok(Self::Creative),
            "security" | "sec" => Ok(Self::Security),
            "management" | "mgmt" => Ok(Self::Management),
            "support" => Ok(Self::Support),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("Unknown division: {}", s)),
        }
    }
}

/// Communication style preference for an agent personality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationStyle {
    /// Tone: formal, casual, technical, friendly, authoritative.
    pub tone: String,
    /// Verbosity: terse, concise, detailed, verbose.
    pub verbosity: String,
    /// Whether to use emojis or informal markers.
    pub use_emojis: bool,
    /// Preferred language for responses.
    pub language: String,
    /// Whether to proactively offer suggestions.
    pub proactive: bool,
    /// Response format preference: prose, bullet_points, structured, mixed.
    pub format: String,
}

impl Default for CommunicationStyle {
    fn default() -> Self {
        Self {
            tone: "professional".to_string(),
            verbosity: "concise".to_string(),
            use_emojis: false,
            language: "en".to_string(),
            proactive: true,
            format: "mixed".to_string(),
        }
    }
}

/// Behavioral trait that shapes agent decision-making.
///
/// Traits are weighted characteristics that influence how an agent approaches
/// tasks, communicates, and makes decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralTrait {
    /// Trait identifier (e.g. "precision", "pragmatism").
    pub name: String,
    /// Human-readable description of the trait's effect.
    pub description: String,
    /// Weight 0.0–1.0 indicating how strongly this trait influences behavior.
    pub weight: f64,
}

/// Expertise domain with proficiency level.
///
/// Represents a specific area of knowledge or skill that an agent possesses,
/// used to frame LLM context and route tasks appropriately.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertiseDomain {
    /// Domain name (e.g. "Rust", "System Design", "Threat Modeling").
    pub domain: String,
    /// Proficiency level: beginner, intermediate, advanced, expert.
    pub proficiency: String,
    /// Years of equivalent experience (for LLM context framing).
    pub years_experience: Option<u32>,
}

/// Collaboration preference for multi-agent workflows.
///
/// Defines how an agent prefers to work within teams, including role,
/// team size, and conflict resolution strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationPreference {
    /// Preferred team size: solo, small (2-4), medium (5-10), large (10+).
    pub preferred_team_size: String,
    /// Role in team: leader, contributor, reviewer, mentor, specialist.
    pub role: String,
    /// Whether to defer to other agents on their expertise.
    pub defer_to_experts: bool,
    /// Conflict resolution style: consensus, authority, data_driven, escalate.
    pub conflict_resolution: String,
}

impl Default for CollaborationPreference {
    fn default() -> Self {
        Self {
            preferred_team_size: "small".to_string(),
            role: "contributor".to_string(),
            defer_to_experts: true,
            conflict_resolution: "data_driven".to_string(),
        }
    }
}

/// A complete agent personality profile.
///
/// Contains all configuration needed to define an agent's identity: division,
/// communication style, behavioral traits, expertise domains, and collaboration
/// preferences. Stored in SQLite via [`PersonalityRegistry`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityProfile {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// URL-safe slug for lookups (e.g. "senior-engineer").
    pub slug: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Functional division this personality belongs to.
    pub division: Division,
    /// Free-text description of the personality's purpose and behavior.
    pub description: String,
    /// System prompt fragment injected into LLM context.
    pub system_prompt_fragment: String,
    /// Communication style configuration.
    pub communication_style: CommunicationStyle,
    /// Weighted behavioral traits influencing decision-making.
    pub behavioral_traits: Vec<BehavioralTrait>,
    /// Expertise domains with proficiency levels.
    pub expertise_domains: Vec<ExpertiseDomain>,
    /// Collaboration preferences for multi-agent workflows.
    pub collaboration: CollaborationPreference,
    /// Tags for filtering and grouping.
    pub tags: Vec<String>,
    /// Whether this personality is active/available.
    pub active: bool,
    /// Schema version for migration tracking.
    pub version: u32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl PersonalityProfile {
    /// Creates a new personality with auto-generated ID and timestamps.
    pub fn new(
        slug: impl Into<String>,
        display_name: impl Into<String>,
        division: Division,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            slug: slug.into(),
            display_name: display_name.into(),
            division,
            description: String::new(),
            system_prompt_fragment: String::new(),
            communication_style: CommunicationStyle::default(),
            behavioral_traits: Vec::new(),
            expertise_domains: Vec::new(),
            collaboration: CollaborationPreference::default(),
            tags: Vec::new(),
            active: true,
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    /// Computes a content fingerprint for change detection.
    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.slug.as_bytes());
        hasher.update(self.display_name.as_bytes());
        hasher.update(self.division.to_string().as_bytes());
        hasher.update(self.system_prompt_fragment.as_bytes());
        hasher.update(
            serde_json::to_string(&self.behavioral_traits)
                .unwrap_or_default()
                .as_bytes(),
        );
        hex::encode(hasher.finalize())
    }
}

/// Query filter for personality lookups.
///
/// All fields are optional; omitted filters are not applied. Combine multiple
/// filters to narrow results. Use with [`PersonalityRegistry::query`].
#[derive(Debug, Clone, Default)]
pub struct PersonalityQuery {
    /// Filter by functional division.
    pub division: Option<Division>,
    /// Filter by tag substring match.
    pub tag: Option<String>,
    /// When true, only return active personalities.
    pub active_only: bool,
    /// Substring search across display_name, description, and slug.
    pub search: Option<String>,
    /// Maximum number of results to return.
    pub limit: Option<usize>,
    /// Number of results to skip (for pagination).
    pub offset: Option<usize>,
}

/// Summary statistics about the personality registry.
///
/// Returned by [`PersonalityRegistry::stats`] to provide an overview of
/// registered personalities, their divisions, and tag distribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityStats {
    /// Total number of registered profiles (active + inactive).
    pub total_profiles: usize,
    /// Number of currently active profiles.
    pub active_profiles: usize,
    /// Profile count grouped by division name.
    pub division_counts: HashMap<String, usize>,
    /// Profile count grouped by tag.
    pub tag_counts: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors that can occur when working with personality profiles.
#[derive(Debug, thiserror::Error)]
pub enum PersonalityError {
    /// SQLite database error.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    /// JSON serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// YAML parsing error (e.g. loading profiles from files).
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    /// Requested personality was not found.
    #[error("Not found: {0}")]
    NotFound(String),
    /// A personality with the same slug already exists.
    #[error("Duplicate slug: {0}")]
    DuplicateSlug(String),
    /// Filesystem I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Catch-all for internal errors.
    #[error("Internal: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Personality Registry
// ---------------------------------------------------------------------------

/// `:memory:` cria um banco separado por conexão; com um pool de várias
/// conexões as tabelas da migração só existiriam em uma delas ("no such
/// table"). Para memória usa uma única conexão que nunca é reciclada.
async fn connect_sqlite_pool(
    path: &str,
    options: SqliteConnectOptions,
) -> Result<SqlitePool, sqlx::Error> {
    let p = path.trim_start_matches("sqlite:").trim_start_matches("//");
    let in_memory = p == ":memory:" || p.starts_with(":memory:?") || p.contains("mode=memory");
    if in_memory {
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await
    } else {
        SqlitePool::connect_with(options).await
    }
}

/// SQLite WAL-backed registry for agent personality profiles.
#[derive(Clone)]
pub struct PersonalityRegistry {
    pool: SqlitePool,
}

impl PersonalityRegistry {
    /// Opens or creates the personality registry at the given path.
    pub async fn open(path: &str) -> Result<Self, PersonalityError> {
        let options = SqliteConnectOptions::from_str(path)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = connect_sqlite_pool(path, options).await?;
        let registry = Self { pool };
        registry.run_migrations().await?;
        Ok(registry)
    }

    async fn run_migrations(&self) -> Result<(), PersonalityError> {
        sqlx::query("CREATE TABLE IF NOT EXISTS personality_profiles (id TEXT PRIMARY KEY, slug TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL, division TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', system_prompt_fragment TEXT NOT NULL DEFAULT '', communication_style TEXT NOT NULL DEFAULT '{}', behavioral_traits TEXT NOT NULL DEFAULT '[]', expertise_domains TEXT NOT NULL DEFAULT '[]', collaboration TEXT NOT NULL DEFAULT '{}', tags TEXT NOT NULL DEFAULT '[]', active INTEGER NOT NULL DEFAULT 1, version INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)")
            .execute(&self.pool).await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_personality_division ON personality_profiles(division)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_personality_slug ON personality_profiles(slug)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_personality_active ON personality_profiles(active)",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Inserts a new personality profile. Returns error if slug already exists.
    pub async fn create(&self, profile: &PersonalityProfile) -> Result<(), PersonalityError> {
        let comm_json = serde_json::to_string(&profile.communication_style)?;
        let traits_json = serde_json::to_string(&profile.behavioral_traits)?;
        let expertise_json = serde_json::to_string(&profile.expertise_domains)?;
        let collab_json = serde_json::to_string(&profile.collaboration)?;
        let tags_json = serde_json::to_string(&profile.tags)?;

        sqlx::query(
            r#"INSERT INTO personality_profiles
                (id, slug, display_name, division, description, system_prompt_fragment,
                 communication_style, behavioral_traits, expertise_domains, collaboration,
                 tags, active, version, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&profile.id)
        .bind(&profile.slug)
        .bind(&profile.display_name)
        .bind(profile.division.to_string())
        .bind(&profile.description)
        .bind(&profile.system_prompt_fragment)
        .bind(&comm_json)
        .bind(&traits_json)
        .bind(&expertise_json)
        .bind(&collab_json)
        .bind(&tags_json)
        .bind(profile.active as i32)
        .bind(profile.version as i32)
        .bind(profile.created_at.to_rfc3339())
        .bind(profile.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.message().contains("UNIQUE constraint failed") {
                    return PersonalityError::DuplicateSlug(profile.slug.clone());
                }
            }
            PersonalityError::Database(e)
        })?;
        Ok(())
    }

    /// Retrieves a personality by slug.
    pub async fn get_by_slug(
        &self,
        slug: &str,
    ) -> Result<Option<PersonalityProfile>, PersonalityError> {
        let row = sqlx::query("SELECT * FROM personality_profiles WHERE slug = ?")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_profile(&r))),
            None => Ok(None),
        }
    }

    /// Retrieves a personality by ID.
    pub async fn get_by_id(
        &self,
        id: &str,
    ) -> Result<Option<PersonalityProfile>, PersonalityError> {
        let row = sqlx::query("SELECT * FROM personality_profiles WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_profile(&r))),
            None => Ok(None),
        }
    }

    /// Updates an existing personality profile. Bumps version and updated_at.
    pub async fn update(&self, profile: &PersonalityProfile) -> Result<bool, PersonalityError> {
        let comm_json = serde_json::to_string(&profile.communication_style)?;
        let traits_json = serde_json::to_string(&profile.behavioral_traits)?;
        let expertise_json = serde_json::to_string(&profile.expertise_domains)?;
        let collab_json = serde_json::to_string(&profile.collaboration)?;
        let tags_json = serde_json::to_string(&profile.tags)?;

        let result = sqlx::query(
            r#"UPDATE personality_profiles SET
                display_name = ?, division = ?, description = ?, system_prompt_fragment = ?,
                communication_style = ?, behavioral_traits = ?, expertise_domains = ?,
                collaboration = ?, tags = ?, active = ?, version = ?, updated_at = ?
            WHERE id = ?"#,
        )
        .bind(&profile.display_name)
        .bind(profile.division.to_string())
        .bind(&profile.description)
        .bind(&profile.system_prompt_fragment)
        .bind(&comm_json)
        .bind(&traits_json)
        .bind(&expertise_json)
        .bind(&collab_json)
        .bind(&tags_json)
        .bind(profile.active as i32)
        .bind((profile.version + 1) as i32)
        .bind(Utc::now().to_rfc3339())
        .bind(&profile.id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Deletes a personality by slug.
    pub async fn delete(&self, slug: &str) -> Result<bool, PersonalityError> {
        let result = sqlx::query("DELETE FROM personality_profiles WHERE slug = ?")
            .bind(slug)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Queries personalities with optional filters.
    pub async fn query(
        &self,
        q: &PersonalityQuery,
    ) -> Result<Vec<PersonalityProfile>, PersonalityError> {
        let mut sql = String::from("SELECT * FROM personality_profiles WHERE 1=1");
        let mut conditions = Vec::new();

        if q.division.is_some() {
            conditions.push("division = ?");
        }
        if q.active_only {
            conditions.push("active = 1");
        }
        if q.tag.is_some() {
            conditions.push("tags LIKE ?");
        }
        if q.search.is_some() {
            conditions.push("(display_name LIKE ? OR description LIKE ? OR slug LIKE ?)");
        }

        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY display_name ASC");
        if q.limit.is_some() {
            sql.push_str(" LIMIT ?");
        }
        if q.offset.is_some() {
            sql.push_str(" OFFSET ?");
        }

        let mut query = sqlx::query(&sql);
        if let Some(ref div) = q.division {
            query = query.bind(div.to_string());
        }
        if let Some(ref tag) = q.tag {
            query = query.bind(format!("%{}%", tag));
        }
        if let Some(ref search) = q.search {
            let pattern = format!("%{}%", search);
            query = query
                .bind(pattern.clone())
                .bind(pattern.clone())
                .bind(pattern);
        }
        if let Some(limit) = q.limit {
            query = query.bind(limit as i64);
        }
        if let Some(offset) = q.offset {
            query = query.bind(offset as i64);
        }

        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows.iter().map(row_to_profile).collect())
    }

    /// Lists all available divisions with profile counts.
    pub async fn list_divisions(&self) -> Result<HashMap<String, usize>, PersonalityError> {
        let rows = sqlx::query(
            "SELECT division, COUNT(*) as cnt FROM personality_profiles GROUP BY division",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| (r.get("division"), r.get::<i64, _>("cnt") as usize))
            .collect())
    }

    /// Returns aggregate statistics.
    pub async fn stats(&self) -> Result<PersonalityStats, PersonalityError> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM personality_profiles")
            .fetch_one(&self.pool)
            .await?;
        let active: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM personality_profiles WHERE active = 1")
                .fetch_one(&self.pool)
                .await?;
        let division_counts = self.list_divisions().await?;

        // Aggregate tag counts
        let all_tags: Vec<String> = sqlx::query_scalar("SELECT tags FROM personality_profiles")
            .fetch_all(&self.pool)
            .await?;
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for tags_json in all_tags {
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&tags_json) {
                for tag in tags {
                    *tag_counts.entry(tag).or_insert(0) += 1;
                }
            }
        }

        Ok(PersonalityStats {
            total_profiles: total as usize,
            active_profiles: active as usize,
            division_counts,
            tag_counts,
        })
    }

    /// Loads personality profiles from YAML files in a directory.
    /// Each file should contain a single PersonalityProfile in YAML format.
    pub async fn load_from_dir(&self, dir: &std::path::Path) -> Result<usize, PersonalityError> {
        let mut loaded = 0;
        if !dir.exists() {
            return Ok(0);
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml")
                || path.extension().and_then(|e| e.to_str()) == Some("yml")
            {
                let content = std::fs::read_to_string(&path)?;
                let profile: PersonalityProfile = serde_yaml::from_str(&content)?;
                match self.create(&profile).await {
                    Ok(()) => loaded += 1,
                    Err(PersonalityError::DuplicateSlug(_)) => {
                        tracing::debug!("Skipping duplicate personality: {}", profile.slug);
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(loaded)
    }

    /// Seeds the registry with built-in personality profiles if empty.
    pub async fn seed_defaults(&self) -> Result<usize, PersonalityError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM personality_profiles")
            .fetch_one(&self.pool)
            .await?;
        if count > 0 {
            return Ok(0);
        }

        let defaults = default_personalities();
        let mut seeded = 0;
        for p in &defaults {
            self.create(p).await?;
            seeded += 1;
        }
        Ok(seeded)
    }
}

// ---------------------------------------------------------------------------
// Row Mapping
// ---------------------------------------------------------------------------

fn row_to_profile(row: &sqlx::sqlite::SqliteRow) -> PersonalityProfile {
    let comm_str: String = row.get("communication_style");
    let traits_str: String = row.get("behavioral_traits");
    let expertise_str: String = row.get("expertise_domains");
    let collab_str: String = row.get("collaboration");
    let tags_str: String = row.get("tags");
    let division_str: String = row.get("division");

    PersonalityProfile {
        id: row.get("id"),
        slug: row.get("slug"),
        display_name: row.get("display_name"),
        division: division_str.parse().unwrap_or(Division::Custom),
        description: row.get("description"),
        system_prompt_fragment: row.get("system_prompt_fragment"),
        communication_style: serde_json::from_str(&comm_str).unwrap_or_default(),
        behavioral_traits: serde_json::from_str(&traits_str).unwrap_or_default(),
        expertise_domains: serde_json::from_str(&expertise_str).unwrap_or_default(),
        collaboration: serde_json::from_str(&collab_str).unwrap_or_default(),
        tags: serde_json::from_str(&tags_str).unwrap_or_default(),
        active: row.get::<i32, _>("active") != 0,
        version: row.get::<i32, _>("version") as u32,
        created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
            .unwrap_or_else(|_| Utc::now().into())
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
            .unwrap_or_else(|_| Utc::now().into())
            .with_timezone(&Utc),
    }
}

// ---------------------------------------------------------------------------
// Built-in Personalities
// ---------------------------------------------------------------------------

fn default_personalities() -> Vec<PersonalityProfile> {
    vec![
        {
            let mut p = PersonalityProfile::new(
                "senior-engineer",
                "Senior Engineer",
                Division::Engineering,
            );
            p.description =
                "Experienced software engineer focused on clean code, testing, and architecture."
                    .into();
            p.system_prompt_fragment = "You are a senior software engineer. Prioritize correctness, readability, and testability. Suggest improvements and flag potential issues proactively.".into();
            p.communication_style.tone = "technical".into();
            p.communication_style.verbosity = "concise".into();
            p.expertise_domains = vec![
                ExpertiseDomain {
                    domain: "Rust".into(),
                    proficiency: "expert".into(),
                    years_experience: Some(8),
                },
                ExpertiseDomain {
                    domain: "System Design".into(),
                    proficiency: "advanced".into(),
                    years_experience: Some(10),
                },
                ExpertiseDomain {
                    domain: "Testing".into(),
                    proficiency: "expert".into(),
                    years_experience: Some(8),
                },
            ];
            p.behavioral_traits = vec![
                BehavioralTrait {
                    name: "precision".into(),
                    description: "Prefers exact specifications over ambiguity".into(),
                    weight: 0.9,
                },
                BehavioralTrait {
                    name: "pragmatism".into(),
                    description: "Balances ideal solutions with practical constraints".into(),
                    weight: 0.8,
                },
            ];
            p.collaboration.role = "contributor".into();
            p.tags = vec!["backend".into(), "systems".into(), "code-review".into()];
            p
        },
        {
            let mut p =
                PersonalityProfile::new("research-analyst", "Research Analyst", Division::Research);
            p.description =
                "Deep research specialist who synthesizes information from multiple sources."
                    .into();
            p.system_prompt_fragment = "You are a research analyst. Provide thorough, well-cited analysis. Distinguish between established facts and speculation. Quantify when possible.".into();
            p.communication_style.tone = "formal".into();
            p.communication_style.verbosity = "detailed".into();
            p.communication_style.format = "structured".into();
            p.expertise_domains = vec![
                ExpertiseDomain {
                    domain: "Literature Review".into(),
                    proficiency: "expert".into(),
                    years_experience: Some(6),
                },
                ExpertiseDomain {
                    domain: "Data Analysis".into(),
                    proficiency: "advanced".into(),
                    years_experience: Some(5),
                },
            ];
            p.behavioral_traits = vec![
                BehavioralTrait {
                    name: "thoroughness".into(),
                    description: "Explores all angles before concluding".into(),
                    weight: 0.95,
                },
                BehavioralTrait {
                    name: "skepticism".into(),
                    description: "Questions assumptions and verifies claims".into(),
                    weight: 0.85,
                },
            ];
            p.tags = vec!["analysis".into(), "synthesis".into(), "citations".into()];
            p
        },
        {
            let mut p = PersonalityProfile::new(
                "devops-specialist",
                "DevOps Specialist",
                Division::Operations,
            );
            p.description =
                "Infrastructure and deployment expert focused on reliability and automation."
                    .into();
            p.system_prompt_fragment = "You are a DevOps specialist. Focus on automation, observability, and reliability. Prefer declarative configurations. Always consider rollback strategies.".into();
            p.communication_style.tone = "technical".into();
            p.communication_style.verbosity = "concise".into();
            p.expertise_domains = vec![
                ExpertiseDomain {
                    domain: "Kubernetes".into(),
                    proficiency: "expert".into(),
                    years_experience: Some(7),
                },
                ExpertiseDomain {
                    domain: "CI/CD".into(),
                    proficiency: "expert".into(),
                    years_experience: Some(8),
                },
                ExpertiseDomain {
                    domain: "Terraform".into(),
                    proficiency: "advanced".into(),
                    years_experience: Some(5),
                },
            ];
            p.collaboration.role = "specialist".into();
            p.tags = vec![
                "infrastructure".into(),
                "deployment".into(),
                "monitoring".into(),
            ];
            p
        },
        {
            let mut p =
                PersonalityProfile::new("security-auditor", "Security Auditor", Division::Security);
            p.description = "Security-focused agent that identifies vulnerabilities and enforces best practices.".into();
            p.system_prompt_fragment = "You are a security auditor. Assume adversarial conditions. Flag all potential vulnerabilities, even low-severity ones. Reference CVEs and OWASP categories when applicable.".into();
            p.communication_style.tone = "authoritative".into();
            p.communication_style.verbosity = "detailed".into();
            p.expertise_domains = vec![
                ExpertiseDomain {
                    domain: "Application Security".into(),
                    proficiency: "expert".into(),
                    years_experience: Some(10),
                },
                ExpertiseDomain {
                    domain: "Threat Modeling".into(),
                    proficiency: "advanced".into(),
                    years_experience: Some(8),
                },
                ExpertiseDomain {
                    domain: "Cryptography".into(),
                    proficiency: "intermediate".into(),
                    years_experience: Some(4),
                },
            ];
            p.behavioral_traits = vec![
                BehavioralTrait {
                    name: "paranoia".into(),
                    description: "Assumes worst-case scenarios by default".into(),
                    weight: 0.9,
                },
                BehavioralTrait {
                    name: "compliance".into(),
                    description: "References standards and regulations".into(),
                    weight: 0.7,
                },
            ];
            p.collaboration.conflict_resolution = "escalate".into();
            p.tags = vec!["security".into(), "audit".into(), "compliance".into()];
            p
        },
        {
            let mut p = PersonalityProfile::new(
                "creative-designer",
                "Creative Designer",
                Division::Creative,
            );
            p.description =
                "UX/UI and creative problem solver with strong visual design sensibility.".into();
            p.system_prompt_fragment = "You are a creative designer. Think visually and empathetically. Propose multiple alternatives. Consider accessibility and user experience in every suggestion.".into();
            p.communication_style.tone = "friendly".into();
            p.communication_style.use_emojis = true;
            p.communication_style.format = "mixed".into();
            p.expertise_domains = vec![
                ExpertiseDomain {
                    domain: "UI/UX Design".into(),
                    proficiency: "expert".into(),
                    years_experience: Some(7),
                },
                ExpertiseDomain {
                    domain: "CSS/Tailwind".into(),
                    proficiency: "advanced".into(),
                    years_experience: Some(5),
                },
                ExpertiseDomain {
                    domain: "Accessibility".into(),
                    proficiency: "advanced".into(),
                    years_experience: Some(4),
                },
            ];
            p.tags = vec!["design".into(), "ux".into(), "frontend".into()];
            p
        },
        {
            let mut p =
                PersonalityProfile::new("project-manager", "Project Manager", Division::Management);
            p.description = "Organized coordinator who tracks progress, manages risks, and keeps teams aligned.".into();
            p.system_prompt_fragment = "You are a project manager. Focus on clarity, deadlines, and accountability. Break complex work into actionable items. Track dependencies and blockers proactively.".into();
            p.communication_style.tone = "professional".into();
            p.communication_style.verbosity = "concise".into();
            p.communication_style.proactive = true;
            p.expertise_domains = vec![
                ExpertiseDomain {
                    domain: "Agile/Scrum".into(),
                    proficiency: "expert".into(),
                    years_experience: Some(8),
                },
                ExpertiseDomain {
                    domain: "Risk Management".into(),
                    proficiency: "advanced".into(),
                    years_experience: Some(6),
                },
            ];
            p.collaboration.role = "leader".into();
            p.collaboration.preferred_team_size = "medium".into();
            p.tags = vec!["planning".into(), "coordination".into(), "tracking".into()];
            p
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_division_roundtrip() {
        for div in [
            Division::Engineering,
            Division::Research,
            Division::Operations,
            Division::Creative,
            Division::Security,
            Division::Management,
            Division::Support,
            Division::Custom,
        ] {
            let s = div.to_string();
            let parsed = Division::parse(&s).unwrap();
            assert_eq!(div, parsed);
        }
    }

    #[test]
    fn test_personality_fingerprint_deterministic() {
        let p = PersonalityProfile::new("test", "Test", Division::Engineering);
        let fp1 = p.fingerprint();
        let fp2 = p.fingerprint();
        assert_eq!(fp1, fp2);
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        let profile = PersonalityProfile::new("test-eng", "Test Engineer", Division::Engineering);
        registry.create(&profile).await.unwrap();

        let retrieved = registry.get_by_slug("test-eng").await.unwrap().unwrap();
        assert_eq!(retrieved.display_name, "Test Engineer");
        assert_eq!(retrieved.division, Division::Engineering);
    }

    #[tokio::test]
    async fn test_duplicate_slug_rejected() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        let p1 = PersonalityProfile::new("dup", "First", Division::Engineering);
        let p2 = PersonalityProfile::new("dup", "Second", Division::Research);
        registry.create(&p1).await.unwrap();
        let err = registry.create(&p2).await.unwrap_err();
        assert!(matches!(err, PersonalityError::DuplicateSlug(_)));
    }

    #[tokio::test]
    async fn test_query_by_division() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        registry
            .create(&PersonalityProfile::new(
                "eng1",
                "Eng 1",
                Division::Engineering,
            ))
            .await
            .unwrap();
        registry
            .create(&PersonalityProfile::new(
                "eng2",
                "Eng 2",
                Division::Engineering,
            ))
            .await
            .unwrap();
        registry
            .create(&PersonalityProfile::new(
                "res1",
                "Res 1",
                Division::Research,
            ))
            .await
            .unwrap();

        let eng = registry
            .query(&PersonalityQuery {
                division: Some(Division::Engineering),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(eng.len(), 2);
    }

    #[tokio::test]
    async fn test_seed_defaults() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        let seeded = registry.seed_defaults().await.unwrap();
        assert!(seeded > 0);

        let stats = registry.stats().await.unwrap();
        assert_eq!(stats.total_profiles, seeded);
        assert!(stats.division_counts.contains_key("engineering"));
    }

    #[tokio::test]
    async fn test_seed_idempotent() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        let first = registry.seed_defaults().await.unwrap();
        let second = registry.seed_defaults().await.unwrap();
        assert!(first > 0);
        assert_eq!(second, 0);
    }

    #[tokio::test]
    async fn test_update_bumps_version() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        let mut profile =
            PersonalityProfile::new("ver-test", "Version Test", Division::Engineering);
        registry.create(&profile).await.unwrap();

        profile.description = "Updated description".into();
        registry.update(&profile).await.unwrap();

        let updated = registry.get_by_slug("ver-test").await.unwrap().unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.description, "Updated description");
    }

    #[tokio::test]
    async fn test_delete() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        registry
            .create(&PersonalityProfile::new(
                "del-me",
                "Delete Me",
                Division::Custom,
            ))
            .await
            .unwrap();
        let deleted = registry.delete("del-me").await.unwrap();
        assert!(deleted);
        assert!(registry.get_by_slug("del-me").await.unwrap().is_none());
    }

    #[test]
    fn test_division_from_str_aliases() {
        assert_eq!(Division::parse("eng").unwrap(), Division::Engineering);
        assert_eq!(Division::parse("ops").unwrap(), Division::Operations);
        assert_eq!(Division::parse("sec").unwrap(), Division::Security);
        assert_eq!(Division::parse("mgmt").unwrap(), Division::Management);
        assert!(Division::parse("nonexistent").is_err());
    }

    #[test]
    fn test_personality_profile_serialization_roundtrip() {
        let mut p = PersonalityProfile::new("serde-test", "Serde Test", Division::Research);
        p.description = "Testing serialization".into();
        p.tags = vec!["test".into(), "serde".into()];
        p.expertise_domains = vec![ExpertiseDomain {
            domain: "Testing".into(),
            proficiency: "expert".into(),
            years_experience: Some(5),
        }];

        let json = serde_json::to_string(&p).unwrap();
        let deserialized: PersonalityProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.slug, "serde-test");
        assert_eq!(deserialized.division, Division::Research);
        assert_eq!(deserialized.tags.len(), 2);
        assert_eq!(deserialized.expertise_domains[0].domain, "Testing");
    }

    #[test]
    fn test_fingerprint_changes_on_content_change() {
        let mut p = PersonalityProfile::new("fp-change", "FP Change", Division::Engineering);
        let fp1 = p.fingerprint();
        p.system_prompt_fragment = "Changed prompt".into();
        let fp2 = p.fingerprint();
        assert_ne!(fp1, fp2);
    }

    #[tokio::test]
    async fn test_query_with_search_and_limit() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        registry
            .create(&PersonalityProfile::new(
                "alpha-one",
                "Alpha One",
                Division::Engineering,
            ))
            .await
            .unwrap();
        registry
            .create(&PersonalityProfile::new(
                "alpha-two",
                "Alpha Two",
                Division::Research,
            ))
            .await
            .unwrap();
        registry
            .create(&PersonalityProfile::new(
                "beta-one",
                "Beta One",
                Division::Engineering,
            ))
            .await
            .unwrap();

        let results = registry
            .query(&PersonalityQuery {
                search: Some("alpha".into()),
                limit: Some(1),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].slug.contains("alpha"));
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        let profile = PersonalityProfile::new("id-lookup", "ID Lookup", Division::Creative);
        let id = profile.id.clone();
        registry.create(&profile).await.unwrap();

        let retrieved = registry.get_by_id(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.slug, "id-lookup");
        assert_eq!(retrieved.division, Division::Creative);
    }

    #[tokio::test]
    async fn test_list_divisions_counts() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        registry
            .create(&PersonalityProfile::new("e1", "E1", Division::Engineering))
            .await
            .unwrap();
        registry
            .create(&PersonalityProfile::new("e2", "E2", Division::Engineering))
            .await
            .unwrap();
        registry
            .create(&PersonalityProfile::new("r1", "R1", Division::Research))
            .await
            .unwrap();

        let divisions = registry.list_divisions().await.unwrap();
        assert_eq!(divisions.get("engineering"), Some(&2));
        assert_eq!(divisions.get("research"), Some(&1));
    }

    #[tokio::test]
    async fn test_stats_tag_counts() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        let mut p1 = PersonalityProfile::new("tag1", "Tag1", Division::Engineering);
        p1.tags = vec!["rust".into(), "backend".into()];
        let mut p2 = PersonalityProfile::new("tag2", "Tag2", Division::Engineering);
        p2.tags = vec!["rust".into(), "frontend".into()];
        registry.create(&p1).await.unwrap();
        registry.create(&p2).await.unwrap();

        let stats = registry.stats().await.unwrap();
        assert_eq!(stats.total_profiles, 2);
        assert_eq!(stats.tag_counts.get("rust"), Some(&2));
        assert_eq!(stats.tag_counts.get("backend"), Some(&1));
        assert_eq!(stats.tag_counts.get("frontend"), Some(&1));
    }

    #[tokio::test]
    async fn test_update_nonexistent_returns_false() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        let profile = PersonalityProfile::new("ghost", "Ghost", Division::Custom);
        let updated = registry.update(&profile).await.unwrap();
        assert!(!updated);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_returns_false() {
        let registry = PersonalityRegistry::open(":memory:").await.unwrap();
        let deleted = registry.delete("does-not-exist").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_communication_style_default() {
        let style = CommunicationStyle::default();
        assert_eq!(style.tone, "professional");
        assert_eq!(style.verbosity, "concise");
        assert!(!style.use_emojis);
        assert_eq!(style.language, "en");
        assert!(style.proactive);
        assert_eq!(style.format, "mixed");
    }

    #[tokio::test]
    async fn test_collaboration_preference_default() {
        let collab = CollaborationPreference::default();
        assert_eq!(collab.preferred_team_size, "small");
        assert_eq!(collab.role, "contributor");
        assert!(collab.defer_to_experts);
        assert_eq!(collab.conflict_resolution, "data_driven");
    }
}

#[cfg(test)]
mod memory_pool_regression {
    /// Regressão: `:memory:` com pool multi-conexão perdia as tabelas.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn memory_pool_keeps_schema_across_concurrent_queries() {
        let opts = <sqlx::sqlite::SqliteConnectOptions as std::str::FromStr>::from_str(":memory:")
            .unwrap();
        let pool = super::connect_sqlite_pool(":memory:", opts).await.unwrap();
        sqlx::query("CREATE TABLE t (x INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        let mut hs = Vec::new();
        for _ in 0..32 {
            let p = pool.clone();
            hs.push(tokio::spawn(async move {
                sqlx::query("SELECT count(*) FROM t").execute(&p).await
            }));
        }
        for h in hs {
            h.await
                .unwrap()
                .expect("tabela deve existir em todas as consultas");
        }
    }
}
