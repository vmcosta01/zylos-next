//! # zylos-personalities
//!
//! Personality-driven agent identities with division taxonomy for zylos-gateway.
//!
//! Provides a structured system for defining agent personalities organized by
//! functional divisions (Engineering, Research, Operations, Creative, Security).
//! Each personality includes behavioral traits, communication style, expertise
//! domains, and collaboration preferences that shape how agents interact.

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Division {
    Engineering,
    Research,
    Operations,
    Creative,
    Security,
    Management,
    Support,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralTrait {
    pub name: String,
    pub description: String,
    /// Weight 0.0–1.0 indicating how strongly this trait influences behavior.
    pub weight: f64,
}

/// Expertise domain with proficiency level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertiseDomain {
    pub domain: String,
    /// Proficiency: beginner, intermediate, advanced, expert.
    pub proficiency: String,
    /// Years of equivalent experience (for LLM context framing).
    pub years_experience: Option<u32>,
}

/// Collaboration preference for multi-agent workflows.
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityProfile {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub division: Division,
    pub description: String,
    pub system_prompt_fragment: String,
    pub communication_style: CommunicationStyle,
    pub behavioral_traits: Vec<BehavioralTrait>,
    pub expertise_domains: Vec<ExpertiseDomain>,
    pub collaboration: CollaborationPreference,
    /// Tags for filtering and grouping.
    pub tags: Vec<String>,
    /// Whether this personality is active/available.
    pub active: bool,
    /// Version for migration tracking.
    pub version: u32,
    pub created_at: DateTime<Utc>,
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
        hasher.update(serde_json::to_string(&self.behavioral_traits).unwrap_or_default().as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Query filter for personality lookups.
#[derive(Debug, Clone, Default)]
pub struct PersonalityQuery {
    pub division: Option<Division>,
    pub tag: Option<String>,
    pub active_only: bool,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Summary statistics about the personality registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityStats {
    pub total_profiles: usize,
    pub active_profiles: usize,
    pub division_counts: HashMap<String, usize>,
    pub tag_counts: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum PersonalityError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Duplicate slug: {0}")]
    DuplicateSlug(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Internal: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Personality Registry
// ---------------------------------------------------------------------------

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
        let pool = SqlitePool::connect_with(options).await?;
        let registry = Self { pool };
        registry.run_migrations().await?;
        Ok(registry)
    }

    async fn run_migrations(&self) -> Result<(), PersonalityError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS personality_profiles (
                id TEXT PRIMARY KEY,
                slug TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                division TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                system_prompt_fragment TEXT NOT NULL DEFAULT '',
                communication_style TEXT NOT NULL DEFAULT '{}',
                behavioral_traits TEXT NOT NULL DEFAULT '[]',
                expertise_domains TEXT NOT NULL DEFAULT '[]',
                collaboration TEXT NOT NULL DEFAULT '{}',
                tags TEXT NOT NULL DEFAULT '[]',
                active INTEGER NOT NULL DEFAULT 1,
                version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_personality_division ON personality_profiles(division);
            CREATE INDEX IF NOT EXISTS idx_personality_slug ON personality_profiles(slug);
            CREATE INDEX IF NOT EXISTS idx_personality_active ON personality_profiles(active);
            "#,
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
    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<PersonalityProfile>, PersonalityError> {
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
    pub async fn get_by_id(&self, id: &str) -> Result<Option<PersonalityProfile>, PersonalityError> {
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
    pub async fn query(&self, q: &PersonalityQuery) -> Result<Vec<PersonalityProfile>, PersonalityError> {
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
            query = query.bind(pattern.clone()).bind(pattern.clone()).bind(pattern);
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
        division: Division::from_str(&division_str).unwrap_or(Division::Custom),
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
            let mut p = PersonalityProfile::new("senior-engineer", "Senior Engineer", Division::Engineering);
            p.description = "Experienced software engineer focused on clean code, testing, and architecture.".into();
            p.system_prompt_fragment = "You are a senior software engineer. Prioritize correctness, readability, and testability. Suggest improvements and flag potential issues proactively.".into();
            p.communication_style.tone = "technical".into();
            p.communication_style.verbosity = "concise".into();
            p.expertise_domains = vec![
                ExpertiseDomain { domain: "Rust".into(), proficiency: "expert".into(), years_experience: Some(8) },
                ExpertiseDomain { domain: "System Design".into(), proficiency: "advanced".into(), years_experience: Some(10) },
                ExpertiseDomain { domain: "Testing".into(), proficiency: "expert".into(), years_experience: Some(8) },
            ];
            p.behavioral_traits = vec![
                BehavioralTrait { name: "precision".into(), description: "Prefers exact specifications over ambiguity".into(), weight: 0.9 },
                BehavioralTrait { name: "pragmatism".into(), description: "Balances ideal solutions with practical constraints".into(), weight: 0.8 },
            ];
            p.collaboration.role = "contributor".into();
            p.tags = vec!["backend".into(), "systems".into(), "code-review".into()];
            p
        },
        {
            let mut p = PersonalityProfile::new("research-analyst", "Research Analyst", Division::Research);
            p.description = "Deep research specialist who synthesizes information from multiple sources.".into();
            p.system_prompt_fragment = "You are a research analyst. Provide thorough, well-cited analysis. Distinguish between established facts and speculation. Quantify when possible.".into();
            p.communication_style.tone = "formal".into();
            p.communication_style.verbosity = "detailed".into();
            p.communication_style.format = "structured".into();
            p.expertise_domains = vec![
                ExpertiseDomain { domain: "Literature Review".into(), proficiency: "expert".into(), years_experience: Some(6) },
                ExpertiseDomain { domain: "Data Analysis".into(), proficiency: "advanced".into(), years_experience: Some(5) },
            ];
            p.behavioral_traits = vec![
                BehavioralTrait { name: "thoroughness".into(), description: "Explores all angles before concluding".into(), weight: 0.95 },
                BehavioralTrait { name: "skepticism".into(), description: "Questions assumptions and verifies claims".into(), weight: 0.85 },
            ];
            p.tags = vec!["analysis".into(), "synthesis".into(), "citations".into()];
            p
        },
        {
            let mut p = PersonalityProfile::new("devops-specialist", "DevOps Specialist", Division::Operations);
            p.description = "Infrastructure and deployment expert focused on reliability and automation.".into();
            p.system_prompt_fragment = "You are a DevOps specialist. Focus on automation, observability, and reliability. Prefer declarative configurations. Always consider rollback strategies.".into();
            p.communication_style.tone = "technical".into();
            p.communication_style.verbosity = "concise".into();
            p.expertise_domains = vec![
                ExpertiseDomain { domain: "Kubernetes".into(), proficiency: "expert".into(), years_experience: Some(7) },
                ExpertiseDomain { domain: "CI/CD".into(), proficiency: "expert".into(), years_experience: Some(8) },
                ExpertiseDomain { domain: "Terraform".into(), proficiency: "advanced".into(), years_experience: Some(5) },
            ];
            p.collaboration.role = "specialist".into();
            p.tags = vec!["infrastructure".into(), "deployment".into(), "monitoring".into()];
            p
        },
        {
            let mut p = PersonalityProfile::new("security-auditor", "Security Auditor", Division::Security);
            p.description = "Security-focused agent that identifies vulnerabilities and enforces best practices.".into();
            p.system_prompt_fragment = "You are a security auditor. Assume adversarial conditions. Flag all potential vulnerabilities, even low-severity ones. Reference CVEs and OWASP categories when applicable.".into();
            p.communication_style.tone = "authoritative".into();
            p.communication_style.verbosity = "detailed".into();
            p.expertise_domains = vec![
                ExpertiseDomain { domain: "Application Security".into(), proficiency: "expert".into(), years_experience: Some(10) },
                ExpertiseDomain { domain: "Threat Modeling".into(), proficiency: "advanced".into(), years_experience: Some(8) },
                ExpertiseDomain { domain: "Cryptography".into(), proficiency: "intermediate".into(), years_experience: Some(4) },
            ];
            p.behavioral_traits = vec![
                BehavioralTrait { name: "paranoia".into(), description: "Assumes worst-case scenarios by default".into(), weight: 0.9 },
                BehavioralTrait { name: "compliance".into(), description: "References standards and regulations".into(), weight: 0.7 },
            ];
            p.collaboration.conflict_resolution = "escalate".into();
            p.tags = vec!["security".into(), "audit".into(), "compliance".into()];
            p
        },
        {
            let mut p = PersonalityProfile::new("creative-designer", "Creative Designer", Division::Creative);
            p.description = "UX/UI and creative problem solver with strong visual design sensibility.".into();
            p.system_prompt_fragment = "You are a creative designer. Think visually and empathetically. Propose multiple alternatives. Consider accessibility and user experience in every suggestion.".into();
            p.communication_style.tone = "friendly".into();
            p.communication_style.use_emojis = true;
            p.communication_style.format = "mixed".into();
            p.expertise_domains = vec![
                ExpertiseDomain { domain: "UI/UX Design".into(), proficiency: "expert".into(), years_experience: Some(7) },
                ExpertiseDomain { domain: "CSS/Tailwind".into(), proficiency: "advanced".into(), years_experience: Some(5) },
                ExpertiseDomain { domain: "Accessibility".into(), proficiency: "advanced".into(), years_experience: Some(4) },
            ];
            p.tags = vec!["design".into(), "ux".into(), "frontend".into()];
            p
        },
        {
            let mut p = PersonalityProfile::new("project-manager", "Project Manager", Division::Management);
            p.description = "Organized coordinator who tracks progress, manages risks, and keeps teams aligned.".into();
            p.system_prompt_fragment = "You are a project manager. Focus on clarity, deadlines, and accountability. Break complex work into actionable items. Track dependencies and blockers proactively.".into();
            p.communication_style.tone = "professional".into();
            p.communication_style.verbosity = "concise".into();
            p.communication_style.proactive = true;
            p.expertise_domains = vec![
                ExpertiseDomain { domain: "Agile/Scrum".into(), proficiency: "expert".into(), years_experience: Some(8) },
                ExpertiseDomain { domain: "Risk Management".into(), proficiency: "advanced".into(), years_experience: Some(6) },
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
            let parsed = Division::from_str(&s).unwrap();
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
        registry.create(&PersonalityProfile::new("eng1", "Eng 1", Division::Engineering)).await.unwrap();
        registry.create(&PersonalityProfile::new("eng2", "Eng 2", Division::Engineering)).await.unwrap();
        registry.create(&PersonalityProfile::new("res1", "Res 1", Division::Research)).await.unwrap();

        let eng = registry.query(&PersonalityQuery {
            division: Some(Division::Engineering),
            ..Default::default()
        }).await.unwrap();
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
        let mut profile = PersonalityProfile::new("ver-test", "Version Test", Division::Engineering);
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
        registry.create(&PersonalityProfile::new("del-me", "Delete Me", Division::Custom)).await.unwrap();
        let deleted = registry.delete("del-me").await.unwrap();
        assert!(deleted);
        assert!(registry.get_by_slug("del-me").await.unwrap().is_none());
    }
}