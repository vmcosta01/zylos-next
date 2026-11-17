//! Login/Register UI component for the cockpit.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
}

/// Login form component with tenant-aware authentication.
#[component]
pub fn LoginForm() -> impl IntoView {
    let user_id = RwSignal::new(String::new());
    let tenant_id = RwSignal::new(String::from("tenant-default"));
    let name = RwSignal::new(String::new());
    // F11.11.8: emissão de token exige a credencial de bootstrap do gateway.
    let credential = RwSignal::new(String::new());
    let error_msg = RwSignal::new(None::<String>);
    let is_loading = RwSignal::new(false);
    let token = RwSignal::new(None::<String>);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        error_msg.set(None);
        is_loading.set(true);

        let uid = user_id.get();
        let tid = tenant_id.get();
        let n = name.get();
        let cred = credential.get();

        spawn_local(async move {
            let body = serde_json::json!({
                "user_id": uid,
                "tenant_id": tid,
                "roles": ["editor"],
                "name": if n.is_empty() { None } else { Some(n) },
            });

            let req = gloo_net::http::Request::post("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .header("Authorization", &format!("Bearer {}", cred))
                .body(body.to_string());

            let resp = match req {
                Ok(r) => r.send().await,
                Err(e) => {
                    error_msg.set(Some(format!("Failed to build request: {}", e)));
                    is_loading.set(false);
                    return;
                }
            };

            match resp {
                Ok(r) if r.ok() => {
                    if let Ok(auth) = r.json::<AuthResponse>().await {
                        token.set(Some(auth.access_token.clone()));
                        // Persist token for subsequent API calls
                        if let Some(storage) =
                            web_sys::window().and_then(|w| w.local_storage().ok().flatten())
                        {
                            let _ = storage.set_item("access_token", &auth.access_token);
                            let _ = storage.set_item("tenant_id", &tid);
                        }
                        is_loading.set(false);
                    } else {
                        error_msg.set(Some("Failed to parse auth response".into()));
                        is_loading.set(false);
                    }
                }
                Ok(r) => {
                    error_msg.set(Some(format!("Login failed: HTTP {}", r.status())));
                    is_loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(format!("Network error: {}", e)));
                    is_loading.set(false);
                }
            }
        });
    };

    view! {
        <div class="auth-container">
            <h2>"Zylos Login"</h2>
            {move || {
                if let Some(t) = token.get() {
                    view! {
                        <div class="auth-success">
                            <p>"✓ Authenticated successfully"</p>
                            <p class="token-preview">{format!("Token: {}...", &t[..20.min(t.len())])}</p>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <form on:submit=on_submit>
                            <label>"User ID"</label>
                            <input
                                type="text"
                                prop:value=move || user_id.get()
                                on:input=move |ev| user_id.set(event_target_value(&ev))
                                placeholder="user-001"
                                required=true
                            />

                            <label>"Tenant"</label>
                            <input
                                type="text"
                                prop:value=move || tenant_id.get()
                                on:input=move |ev| tenant_id.set(event_target_value(&ev))
                                placeholder="tenant-default"
                                required=true
                            />

                            <label>"Access credential"</label>
                            <input
                                type="password"
                                autocomplete="off"
                                prop:value=move || credential.get()
                                on:input=move |ev| credential.set(event_target_value(&ev))
                                required=true
                            />

                            <label>"Display Name (optional)"</label>
                            <input
                                type="text"
                                prop:value=move || name.get()
                                on:input=move |ev| name.set(event_target_value(&ev))
                                placeholder="Alice"
                            />

                            <button type="submit" disabled=move || is_loading.get()>
                                {move || if is_loading.get() { "Signing in..." } else { "Sign In" }}
                            </button>

                            {move || {
                                if let Some(err) = error_msg.get() {
                                    view! { <p class="auth-error">{err}</p> }.into_any()
                                } else {
                                    ().into_any()
                                }
                            }}
                        </form>
                    }.into_any()
                }
            }}
        </div>
    }
}

/// Presence indicator showing who is currently editing a document.
#[allow(dead_code)]
#[component]
pub fn PresenceIndicator(doc_id: String) -> impl IntoView {
    let users = RwSignal::new(Vec::<(String, String, String)>::new());

    // Poll presence endpoint periodically
    let did = doc_id.clone();
    spawn_local(async move {
        loop {
            if let Some(token) = web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.get_item("access_token").ok().flatten())
            {
                let url = format!("/api/v1/presence/active?doc_id={}", did);
                if let Ok(resp) = gloo_net::http::Request::get(&url)
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    if resp.ok() {
                        if let Ok(list) = resp.json::<Vec<(u64, String, String, String)>>().await {
                            users.set(
                                list.into_iter()
                                    .map(|(_, uid, name, color)| (uid, name, color))
                                    .collect(),
                            );
                        }
                    }
                }
            }
            gloo_timers::future::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    view! {
        <div class="presence-bar">
            <span class="presence-label">"Editing now:"</span>
            {move || {
                let u = users.get();
                if u.is_empty() {
                    view! { <span class="presence-empty">"No one else"</span> }.into_any()
                } else {
                    u.iter().map(|(uid, name, color)| {
                        let c = color.clone();
                        let n = name.clone();
                        view! {
                            <span class="presence-avatar" style=format!("background-color:{}", c) title=uid.clone()>
                                {n.chars().next().unwrap_or('?').to_uppercase().to_string()}
                            </span>
                        }
                    }).collect_view().into_any()
                }
            }}
        </div>
    }
}

/// Conflict resolution UI for CRDT merge conflicts.
#[component]
pub fn ConflictResolver(_doc_id: String) -> impl IntoView {
    let has_conflict = RwSignal::new(false);
    let resolved = RwSignal::new(false);

    let on_resolve = move |_| {
        resolved.set(true);
        has_conflict.set(false);
        // In production: send corrected state via CrdtMessage::SyncStep2
    };

    view! {
        <div class="conflict-panel" class:hidden=move || !has_conflict.get()>
            <div class="conflict-header">
                <span>"⚠ Merge conflict detected"</span>
                <button on:click=on_resolve disabled=move || resolved.get()>
                    {move || if resolved.get() { "✓ Resolved" } else { "Accept Auto-Merge" }}
                </button>
            </div>
            <p class="conflict-hint">
                "Multiple editors modified overlapping regions. Review changes above."
            </p>
        </div>
    }
}
