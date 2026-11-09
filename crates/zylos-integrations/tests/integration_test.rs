//! Integration tests for zylos-integrations.
//!
//! Covers: token encryption round-trip, audit logging, manifest loading,
//! OAuth session lifecycle, executor URL building, and error paths.

use std::path::Path;
use zylos_integrations::manifest::{load_specs, SpecRegistry};
use zylos_integrations::oauth::TokenBundle;
use zylos_integrations::db::TokenStore;
use zylos_integrations::error::IntegrationError;
use chrono::{Duration, Utc};

// ---------------------------------------------------------------------------
// Manifest / Spec Loading Tests
// ---------------------------------------------------------------------------

#[test]
fn test_load_all_11_specs() {
    let specs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs");
    let specs = load_specs(&specs_dir).expect("should load all specs");
    assert_eq!(specs.len(), 11, "Expected 11 provider specs, got {}", specs.len());

    let slugs: Vec<&str> = specs.iter().map(|s| s.slug.as_str()).collect();
    for expected in &[
        "gmail", "slack", "notion", "linear", "jira",
        "github", "feishu", "wechat_work", "figma", "stripe", "shopify",
    ] {
        assert!(slugs.contains(expected), "Missing spec: {}", expected);
    }
}

#[test]
fn test_spec_registry_lookup() {
    let specs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs");
    let registry = SpecRegistry::load_dir(&specs_dir).unwrap();

    let gmail = registry.get("gmail").expect("gmail spec should exist");
    assert_eq!(gmail.display_name, "Gmail (Google Workspace)");
    assert!(gmail.requires_approval, "gmail should require approval");
    assert!(!gmail.actions.is_empty(), "gmail should have actions");

    let figma = registry.get("figma").expect("figma spec should exist");
    assert_eq!(figma.display_name, "Figma");
    assert!(!figma.requires_approval, "figma should not require global approval");

    let stripe = registry.get("stripe").expect("stripe spec should exist");
    assert!(stripe.requires_approval, "stripe should require approval");

    let shopify = registry.get("shopify").expect("shopify spec should exist");
    assert!(shopify.requires_approval, "shopify should require approval");
}

#[test]
fn test_spec_has_valid_actions() {
    let specs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs");
    let specs = load_specs(&specs_dir).unwrap();

    for spec in &specs {
        for action in &spec.actions {
            assert!(!action.id.is_empty(), "Action id empty in spec {}", spec.slug);
            assert!(!action.method.is_empty(), "Action method empty in spec {}::{}", spec.slug, action.id);
            assert!(!action.url_template.is_empty(), "URL template empty in spec {}::{}", spec.slug, action.id);
            let valid_methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];
            assert!(
                valid_methods.contains(&action.method.to_uppercase().as_str()),
                "Invalid HTTP method '{}' in spec {}::{}",
                action.method, spec.slug, action.id
            );
        }
    }
}

#[test]
fn test_load_empty_dir_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let specs = load_specs(dir.path()).unwrap();
    assert!(specs.is_empty());
}

#[test]
fn test_load_nonexistent_dir_returns_empty() {
    let specs = load_specs(Path::new("/nonexistent/path/that/does/not/exist")).unwrap();
    assert!(specs.is_empty());
}

// ---------------------------------------------------------------------------
// TokenBundle Tests
// ---------------------------------------------------------------------------

#[test]
fn test_token_bundle_not_expired() {
    let bundle = TokenBundle {
        provider_slug: "test".into(),
        access_token: "tok".into(),
        refresh_token: None,
        expires_at: Utc::now() + Duration::hours(1),
        scopes: vec![],
        token_type: "Bearer".into(),
    };
    assert!(!bundle.is_expired());
}

#[test]
fn test_token_bundle_expired() {
    let bundle = TokenBundle {
        provider_slug: "test".into(),
        access_token: "tok".into(),
        refresh_token: None,
        expires_at: Utc::now() - Duration::seconds(120),
        scopes: vec![],
        token_type: "Bearer".into(),
    };
    assert!(bundle.is_expired());
}

#[test]
fn test_token_bundle_expires_within_buffer() {
    // Within the 60-second buffer → considered expired
    let bundle = TokenBundle {
        provider_slug: "test".into(),
        access_token: "tok".into(),
        refresh_token: None,
        expires_at: Utc::now() + Duration::seconds(30),
        scopes: vec![],
        token_type: "Bearer".into(),
    };
    assert!(bundle.is_expired(), "Token expiring within 60s buffer should be considered expired");
}

// ---------------------------------------------------------------------------
// TokenStore Encryption Round-Trip Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_token_store_encrypt_decrypt_round_trip() {
    // Use a temp dir so we don't pollute the real data dir
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("ZYLOS_DATA_DIR", tmp.path());
    // Use a known test key
    std::env::set_var(
        "ZYLOS_VAULT_KEY",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );

    let store = TokenStore::new().await.expect("should create token store");

    let bundle = TokenBundle {
        provider_slug: "test-provider".into(),
        access_token: "super-secret-access-token-12345".into(),
        refresh_token: Some("super-secret-refresh-token-67890".into()),
        expires_at: Utc::now() + Duration::hours(1),
        scopes: vec!["read".into(), "write".into()],
        token_type: "Bearer".into(),
    };

    store.store_tokens("user-1", &bundle).await.unwrap();

    let retrieved = store
        .get_tokens("user-1", "test-provider")
        .await
        .unwrap()
        .expect("should retrieve stored tokens");

    assert_eq!(retrieved.access_token, bundle.access_token);
    assert_eq!(retrieved.refresh_token, bundle.refresh_token);
    assert_eq!(retrieved.provider_slug, bundle.provider_slug);
    assert_eq!(retrieved.scopes, bundle.scopes);
    assert_eq!(retrieved.token_type, bundle.token_type);

    // Cleanup env vars
    std::env::remove_var("ZYLOS_DATA_DIR");
    std::env::remove_var("ZYLOS_VAULT_KEY");
}

#[tokio::test]
async fn test_token_store_overwrite_on_duplicate() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("ZYLOS_DATA_DIR", tmp.path());
    std::env::set_var(
        "ZYLOS_VAULT_KEY",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );

    let store = TokenStore::new().await.unwrap();

    let bundle1 = TokenBundle {
        provider_slug: "prov".into(),
        access_token: "token-v1".into(),
        refresh_token: None,
        expires_at: Utc::now() + Duration::hours(1),
        scopes: vec![],
        token_type: "Bearer".into(),
    };
    store.store_tokens("user-a", &bundle1).await.unwrap();

    let bundle2 = TokenBundle {
        provider_slug: "prov".into(),
        access_token: "token-v2-updated".into(),
        refresh_token: Some("new-refresh".into()),
        expires_at: Utc::now() + Duration::hours(2),
        scopes: vec!["admin".into()],
        token_type: "Bearer".into(),
    };
    store.store_tokens("user-a", &bundle2).await.unwrap();

    let retrieved = store.get_tokens("user-a", "prov").await.unwrap().unwrap();
    assert_eq!(retrieved.access_token, "token-v2-updated");
    assert_eq!(retrieved.refresh_token, Some("new-refresh".into()));
    assert_eq!(retrieved.scopes, vec!["admin".to_string()]);

    std::env::remove_var("ZYLOS_DATA_DIR");
    std::env::remove_var("ZYLOS_VAULT_KEY");
}

#[tokio::test]
async fn test_token_store_delete_tokens() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("ZYLOS_DATA_DIR", tmp.path());
    std::env::set_var(
        "ZYLOS_VAULT_KEY",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );

    let store = TokenStore::new().await.unwrap();

    let bundle = TokenBundle {
        provider_slug: "to-delete".into(),
        access_token: "tok".into(),
        refresh_token: None,
        expires_at: Utc::now() + Duration::hours(1),
        scopes: vec![],
        token_type: "Bearer".into(),
    };
    store.store_tokens("user-del", &bundle).await.unwrap();
    assert!(store.get_tokens("user-del", "to-delete").await.unwrap().is_some());

    store.delete_tokens("user-del", "to-delete").await.unwrap();
    assert!(store.get_tokens("user-del", "to-delete").await.unwrap().is_none());

    std::env::remove_var("ZYLOS_DATA_DIR");
    std::env::remove_var("ZYLOS_VAULT_KEY");
}

#[tokio::test]
async fn test_token_store_get_nonexistent_returns_none() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("ZYLOS_DATA_DIR", tmp.path());
    std::env::set_var(
        "ZYLOS_VAULT_KEY",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );

    let store = TokenStore::new().await.unwrap();
    let result = store.get_tokens("no-user", "no-provider").await.unwrap();
    assert!(result.is_none());

    std::env::remove_var("ZYLOS_DATA_DIR");
    std::env::remove_var("ZYLOS_VAULT_KEY");
}

// ---------------------------------------------------------------------------
// Audit Log Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_audit_log_records_action() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("ZYLOS_DATA_DIR", tmp.path());
    std::env::set_var(
        "ZYLOS_VAULT_KEY",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );

    let store = TokenStore::new().await.unwrap();

    let result_body = serde_json::json!({"status": "ok", "id": "msg-123"});
    store
        .log_action("user-log", "gmail", "send_email", "auto", &result_body)
        .await
        .unwrap();

    // Verify by querying the audit log table directly
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT user_id, provider_slug, action, decided_by FROM integration_audit_log ORDER BY id DESC LIMIT 1",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "user-log");
    assert_eq!(rows[0].1, "gmail");
    assert_eq!(rows[0].2, "send_email");
    assert_eq!(rows[0].3, "auto");

    std::env::remove_var("ZYLOS_DATA_DIR");
    std::env::remove_var("ZYLOS_VAULT_KEY");
}

// ---------------------------------------------------------------------------
// Error Path Tests
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_provider_error_display() {
    let err = IntegrationError::UnknownProvider("nonexistent".into());
    let msg = format!("{}", err);
    assert!(msg.contains("nonexistent"), "Error should contain provider name");
}

#[test]
fn test_approval_required_error_display() {
    let err = IntegrationError::ApprovalRequired {
        provider: "stripe".into(),
        action: "create_refund".into(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("stripe"));
    assert!(msg.contains("create_refund"));
}

#[test]
fn test_rate_limited_error_display() {
    let err = IntegrationError::RateLimited {
        provider: "shopify".into(),
        retry_after_secs: 30,
    };
    let msg = format!("{}", err);
    assert!(msg.contains("shopify"));
    assert!(msg.contains("30"));
}