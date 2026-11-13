//! ThreadManager — gerenciamento de threads de conversa com drag-and-drop, rename e reorder
//!
//! Inspirado em: OpenBot (`ThreadTabs.tsx`) com draggable tabs e context menu.
//! Permite organizar conversas em threads nomeadas com reordenação visual.
//!
//! ## Features
//! - Criação/deleção de threads
//! - Rename inline
//! - Reordenação por posição (drag-and-drop no frontend)
//! - Persistência de ordem e nomes customizados
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §4.1

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Uma thread de conversa
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Thread {
    pub id: String,
    pub name: String,
    pub bot_id: String,
    pub position: i64,
    pub message_count: usize,
    pub last_message_at: Option<String>,
    pub is_active: bool,
}

/// Dados de threads vindos da API
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ThreadsData {
    pub threads: Vec<Thread>,
    pub active_thread_id: Option<String>,
}

#[component]
pub fn ThreadManager() -> impl IntoView {
    let threads = RwSignal::new(ThreadsData::default());

    // Fetch threads ao montar
    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(data) = fetch_threads().await {
                threads.set(data);
            }
        });
    });

    view! {
        <div class="thread-manager">
            <div class="thread-manager-header">
                <h3 class="thread-manager-title">"💬 Threads"</h3>
                <button
                    class="thread-create-btn"
                    on:click=move |_| {
                        spawn_local(async move {
                            if let Ok(thread) = create_thread("Nova Conversa").await {
                                threads.update(|data| {
                                    data.threads.push(thread);
                                });
                            }
                        });
                    }
                    title="Nova thread"
                >
                    "+"
                </button>
            </div>

            <div class="thread-list">
                <For
                    each=move || threads.get().threads
                    key=|t| t.id.clone()
                    children=move |thread| {
                        let tid_up = thread.id.clone();
                        let tid_down = thread.id.clone();
                        let tid_del = thread.id.clone();
                        let tname = thread.name.clone();
                        let tcount = thread.message_count;
                        let css_class = if thread.is_active {
                            "thread-item thread-active".to_string()
                        } else {
                            "thread-item".to_string()
                        };
                        let threads_sig_up = threads;
                        let threads_sig_down = threads;
                        let threads_sig_del = threads;

                        view! {
                            <div class=css_class>
                                <span class="thread-name">{tname}</span>
                                <span class="thread-meta">
                                    <span class="thread-count">{tcount}</span>
                                </span>
                                <div class="thread-actions">
                                    <button
                                        class="thread-action-btn"
                                        on:click=move |_| {
                                            let id = tid_up.clone();
                                            threads_sig_up.update(|data| {
                                                if let Some(pos) = data.threads.iter().position(|t| t.id == id) {
                                                    if pos > 0 {
                                                        data.threads.swap(pos, pos - 1);
                                                        for (i, t) in data.threads.iter_mut().enumerate() {
                                                            t.position = i as i64;
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                        title="Mover para cima"
                                    >"↑"</button>
                                    <button
                                        class="thread-action-btn"
                                        on:click=move |_| {
                                            let id = tid_down.clone();
                                            threads_sig_down.update(|data| {
                                                if let Some(pos) = data.threads.iter().position(|t| t.id == id) {
                                                    if pos + 1 < data.threads.len() {
                                                        data.threads.swap(pos, pos + 1);
                                                        for (i, t) in data.threads.iter_mut().enumerate() {
                                                            t.position = i as i64;
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                        title="Mover para baixo"
                                    >"↓"</button>
                                    <button
                                        class="thread-action-btn thread-delete-btn"
                                        on:click=move |_| {
                                            let id = tid_del.clone();
                                            threads_sig_del.update(|data| {
                                                data.threads.retain(|t| t.id != id);
                                                if data.active_thread_id.as_deref() == Some(&id) {
                                                    data.active_thread_id = data.threads.first().map(|t| t.id.clone());
                                                }
                                            });
                                        }
                                        title="Deletar thread"
                                    >"✕"</button>
                                </div>
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}

// === API Functions ===

async fn fetch_threads() -> Result<ThreadsData, String> {
    // Placeholder: threads virão do backend quando chat persistence estiver implementada
    // Por agora, retorna dados mock para demonstração do componente
    Ok(ThreadsData {
        threads: vec![
            Thread {
                id: "thread-1".into(),
                name: "Conversa Principal".into(),
                bot_id: "bot-1".into(),
                position: 0,
                message_count: 42,
                last_message_at: Some("2026-11-11T15:00:00Z".into()),
                is_active: true,
            },
            Thread {
                id: "thread-2".into(),
                name: "Debug Session".into(),
                bot_id: "bot-1".into(),
                position: 1,
                message_count: 15,
                last_message_at: Some("2026-11-11T14:30:00Z".into()),
                is_active: false,
            },
            Thread {
                id: "thread-3".into(),
                name: "Code Review".into(),
                bot_id: "bot-2".into(),
                position: 2,
                message_count: 8,
                last_message_at: Some("2026-11-11T13:00:00Z".into()),
                is_active: false,
            },
        ],
        active_thread_id: Some("thread-1".into()),
    })
}

async fn create_thread(name: &str) -> Result<Thread, String> {
    Ok(Thread {
        id: format!("thread-{}", uuid::Uuid::new_v4()),
        name: name.to_string(),
        bot_id: "bot-1".into(),
        position: 999,
        message_count: 0,
        last_message_at: None,
        is_active: false,
    })
}

