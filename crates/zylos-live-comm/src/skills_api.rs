//! Skills Registry API — REST endpoints for skill discovery and evaluation
//!
//! Task 5.3.2: GET /skills, GET /skills/:id, POST /eval/run
//! Uses zylos-skills crate for registry, schema validation, and eval flywheel.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use zylos_skills::{EvalFlywheel, EvalReport, SkillEntry, SkillRegistry, SkillValidator};

/// Shared state for skills API handlers
#[derive(Clone)]
/// Estrutura SkillsApiState.
pub struct SkillsApiState {
    /// Campo registry.
    pub registry: Arc<RwLock<SkillRegistry>>,
    /// Campo validator.
    pub validator: Arc<SkillValidator>,
    /// Campo eval flywheel.
    pub eval_flywheel: Arc<RwLock<EvalFlywheel>>,
}

/// Response for skill listing
#[derive(Serialize, serde::Deserialize)]
/// Estrutura SkillsListResponse.
pub struct SkillsListResponse {
    /// Campo total.
    pub total: usize,
    /// Campo skills.
    pub skills: Vec<SkillEntry>,
}

/// Response for single skill lookup
#[derive(Serialize, serde::Deserialize)]
/// Estrutura SkillDetailResponse.
pub struct SkillDetailResponse {
    /// Campo skill.
    pub skill: SkillEntry,
}

/// Request body for eval run
#[derive(Deserialize)]
/// Estrutura EvalRunRequest.
pub struct EvalRunRequest {
    /// Campo test case id.
    pub test_case_id: String,
    /// Campo scores.
    pub scores: Vec<EvalScoreInput>,
}

/// Individual score input for eval
#[derive(Deserialize)]
/// Estrutura EvalScoreInput.
pub struct EvalScoreInput {
    /// Campo criterion.
    pub criterion: String,
    /// Campo score.
    pub score: f64,
    /// Campo reasoning.
    pub reasoning: String,
}

/// Error response
#[derive(Serialize, serde::Deserialize)]
/// Estrutura ApiError.
pub struct ApiError {
    /// Campo error.
    pub error: String,
    /// Campo code.
    pub code: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.code.as_str() {
            "NOT_FOUND" => StatusCode::NOT_FOUND,
            "VALIDATION_ERROR" => StatusCode::BAD_REQUEST,
            "EVAL_FAILED" => StatusCode::UNPROCESSABLE_ENTITY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

/// Build the Skills Registry API router
pub fn create_skills_router(state: SkillsApiState) -> Router {
    Router::new()
        .route("/skills", get(list_skills))
        .route("/skills/:id", get(get_skill))
        .route("/eval/run", post(run_eval))
        .with_state(state)
}

/// GET /skills — List all registered skills
async fn list_skills(
    State(state): State<SkillsApiState>,
) -> Result<Json<SkillsListResponse>, ApiError> {
    let registry = state.registry.read().await;
    let skills: Vec<SkillEntry> = registry.list_all().into_iter().cloned().collect();

    // Variante Ok.
    Ok(Json(SkillsListResponse {
        total: skills.len(),
        skills,
    }))
}

/// GET /skills/:id — Get a specific skill by name
async fn get_skill(
    State(state): State<SkillsApiState>,
    Path(id): Path<String>,
) -> Result<Json<SkillDetailResponse>, ApiError> {
    let registry = state.registry.read().await;

    match registry.get(&id) {
        // Variante Ok.
        Ok(skill) => Ok(Json(SkillDetailResponse {
            skill: skill.clone(),
        })),
        // Variante Err.
        Err(_) => Err(ApiError {
            error: format!("Skill '{}' not found", id),
            code: "NOT_FOUND".to_string(),
        }),
    }
}

/// POST /eval/run — Run an evaluation against the flywheel
async fn run_eval(
    State(state): State<SkillsApiState>,
    Json(request): Json<EvalRunRequest>,
) -> Result<Json<EvalReport>, ApiError> {
    let scores: Vec<zylos_skills::CriterionScore> = request
        .scores
        .into_iter()
        .map(|s| zylos_skills::CriterionScore {
            criterion: s.criterion,
            score: s.score,
            reasoning: s.reasoning,
            duration_ms: 0.0,
        })
        .collect();

    let mut flywheel = state.eval_flywheel.write().await;

    match flywheel.evaluate_mock(&request.test_case_id, scores) {
        // Variante Ok.
        Ok(report) => Ok(Json(report)),
        // Variante Err.
        Err(e) => Err(ApiError {
            error: e.to_string(),
            code: "EVAL_FAILED".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use std::path::Path;
    use tower::ServiceExt;
    use zylos_skills::{EvalFlywheel, SkillEntry, SkillRegistry, SkillValidator};

    fn test_state() -> SkillsApiState {
        let mut registry = SkillRegistry::empty(Path::new("."));
        registry.add(SkillEntry {
            name: "test-skill".to_string(),
            description: "A test skill for API verification".to_string(),
            version: "1.0.0".to_string(),
            path: "test-skill/SKILL.md".to_string(),
            category: Some("devops".to_string()),
            tags: vec!["test".to_string()],
            depends_on: vec![],
            active: true,
            digest: "sha256:test".to_string(),
            trigger_phrases: vec![],
            usage: zylos_skills::UsageStats {
                last_used: 0,
                use_count: 0,
                success_rate: 0.0,
                avg_latency_ms: 0.0,
                success_count: 0,
                failure_count: 0,
            },
        });

        let mut flywheel = EvalFlywheel::empty();
        flywheel.add_test_case(zylos_skills::EvalTestCase {
            id: "tc-001".to_string(),
            skill_name: "test-skill".to_string(),
            input: "test input".to_string(),
            expected_output: Some("expected".to_string()),
            context: std::collections::HashMap::new(),
            criteria_override: vec![],
            tags: vec![],
        });

        // Variante SkillsApiState.
        SkillsApiState {
            registry: Arc::new(RwLock::new(registry)),
            validator: Arc::new(SkillValidator::default_schema().unwrap()),
            eval_flywheel: Arc::new(RwLock::new(flywheel)),
        }
    }

    #[tokio::test]
    async fn test_list_skills_returns_200() {
        let state = test_state();
        let app = create_skills_router(state);

        let request = Request::builder()
            .uri("/skills")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let result: SkillsListResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.skills[0].name, "test-skill");
    }

    #[tokio::test]
    async fn test_get_skill_found() {
        let state = test_state();
        let app = create_skills_router(state);

        let request = Request::builder()
            .uri("/skills/test-skill")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let result: SkillDetailResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.skill.name, "test-skill");
    }

    #[tokio::test]
    async fn test_get_skill_not_found() {
        let state = test_state();
        let app = create_skills_router(state);

        let request = Request::builder()
            .uri("/skills/nonexistent")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let error: ApiError = serde_json::from_slice(&body).unwrap();
        assert_eq!(error.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn test_eval_run_success() {
        let state = test_state();
        let app = create_skills_router(state);

        let payload = serde_json::json!({
            "test_case_id": "tc-001",
            "scores": [
                {"criterion": "correctness", "score": 0.9, "reasoning": "correct"},
                {"criterion": "completeness", "score": 0.8, "reasoning": "complete"},
                {"criterion": "clarity", "score": 0.85, "reasoning": "clear"},
                {"criterion": "efficiency", "score": 0.7, "reasoning": "efficient"}
            ]
        });

        let request = Request::builder()
            .method("POST")
            .uri("/eval/run")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let report: EvalReport = serde_json::from_slice(&body).unwrap();
        assert_eq!(report.test_case_id, "tc-001");
        assert_eq!(report.verdict, "pass");
    }

    #[tokio::test]
    async fn test_eval_run_not_found() {
        let state = test_state();
        let app = create_skills_router(state);

        let payload = serde_json::json!({
            "test_case_id": "nonexistent",
            "scores": []
        });

        let request = Request::builder()
            .method("POST")
            .uri("/eval/run")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_list_skills_empty_registry() {
        let registry = SkillRegistry::empty(Path::new("."));
        let state = SkillsApiState {
            registry: Arc::new(RwLock::new(registry)),
            validator: Arc::new(SkillValidator::default_schema().unwrap()),
            eval_flywheel: Arc::new(RwLock::new(EvalFlywheel::empty())),
        };
        let app = create_skills_router(state);

        let request = Request::builder()
            .uri("/skills")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let result: SkillsListResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.total, 0);
    }
}
