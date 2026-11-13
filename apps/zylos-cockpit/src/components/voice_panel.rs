//! VoicePanel — painel de voz com waveform, phase states e controles de mute/interrupt
//!
//! Inspirado em: OpenBot (`VoicePanel.tsx`) com visualização de waveform e estados de fase.
//! Permite interação por voz com o agente, exibindo estado da conversa em tempo real.
//!
//! ## Features
//! - Visualização de waveform (placeholder CSS animation)
//! - Estados de fase: idle, listening, processing, speaking
//! - Controles: mute/unmute, interrupt, speed cycling
//! - Indicador visual de atividade de áudio
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §4.4

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Estado da sessão de voz
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoicePhase {
    Idle,
    Listening,
    Processing,
    Speaking,
    Error,
}

impl VoicePhase {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Aguardando",
            Self::Listening => "Ouvindo...",
            Self::Processing => "Processando...",
            Self::Speaking => "Falando...",
            Self::Error => "Erro",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Idle => "voice-phase-idle",
            Self::Listening => "voice-phase-listening",
            Self::Processing => "voice-phase-processing",
            Self::Speaking => "voice-phase-speaking",
            Self::Error => "voice-phase-error",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Idle => "🎙️",
            Self::Listening => "👂",
            Self::Processing => "⏳",
            Self::Speaking => "🔊",
            Self::Error => "⚠️",
        }
    }
}

/// Velocidade de reprodução de voz
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceSpeed {
    Slow,   // 0.75x
    Normal, // 1.0x
    Fast,   // 1.5x
}

impl VoiceSpeed {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Slow => "0.75x",
            Self::Normal => "1.0x",
            Self::Fast => "1.5x",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Slow => Self::Normal,
            Self::Normal => Self::Fast,
            Self::Fast => Self::Slow,
        }
    }
}

#[component]
pub fn VoicePanel() -> impl IntoView {
    let phase = RwSignal::new(VoicePhase::Idle);
    let muted = RwSignal::new(false);
    let speed = RwSignal::new(VoiceSpeed::Normal);
    let transcript = RwSignal::new(String::new());

    let toggle_mute = move || {
        muted.update(|m| *m = !*m);
    };

    let cycle_speed = move || {
        speed.update(|s| *s = s.next());
    };

    let start_listening = move || {
        phase.set(VoicePhase::Listening);
        transcript.set("".into());
        // Placeholder: iniciar Web Audio API / SpeechRecognition
        // Simular transição para processing após 2s
        spawn_local(async move {
            gloo_timers::future::sleep(std::time::Duration::from_secs(2)).await;
            phase.set(VoicePhase::Processing);
            gloo_timers::future::sleep(std::time::Duration::from_secs(1)).await;
            phase.set(VoicePhase::Speaking);
            transcript.set("Resposta simulada do agente via voz.".into());
            gloo_timers::future::sleep(std::time::Duration::from_secs(3)).await;
            phase.set(VoicePhase::Idle);
        });
    };

    let interrupt = move || {
        phase.set(VoicePhase::Idle);
        transcript.set("".into());
    };

    view! {
        <div class="voice-panel">
            <div class="voice-panel-header">
                <h3 class="voice-panel-title">"🎙️ Painel de Voz"</h3>
            </div>

            // Waveform visualization (CSS animation placeholder)
            <div class=format!("voice-waveform {}", phase.get().css_class())>
                <div class="waveform-bar"></div>
                <div class="waveform-bar"></div>
                <div class="waveform-bar"></div>
                <div class="waveform-bar"></div>
                <div class="waveform-bar"></div>
                <div class="waveform-bar"></div>
                <div class="waveform-bar"></div>
            </div>

            // Phase indicator
            <div class="voice-phase-indicator">
                <span class="voice-phase-icon">{phase.get().icon()}</span>
                <span class="voice-phase-label">{phase.get().label()}</span>
            </div>

            // Transcript
            <div class="voice-transcript">
                {move || {
                    let t = transcript.get();
                    if t.is_empty() {
                        view! { <span class="voice-transcript-placeholder">"Transcrição aparecerá aqui..."</span> }.into_any()
                    } else {
                        view! { <span class="voice-transcript-text">{t}</span> }.into_any()
                    }
                }}
            </div>

            // Controls
            <div class="voice-controls">
                <button
                    class="voice-btn voice-btn-primary"
                    on:click=move |_| start_listening()
                    disabled=move || phase.get() != VoicePhase::Idle
                    title="Iniciar escuta"
                >
                    "🎙️ Ouvir"
                </button>

                <button
                    class="voice-btn voice-btn-danger"
                    on:click=move |_| interrupt()
                    disabled=move || phase.get() == VoicePhase::Idle
                    title="Interromper"
                >
                    "⏹ Parar"
                </button>

                <button
                    class="voice-btn"
                    on:click=move |_| toggle_mute()
                    title="Mudo"
                >
                    {move || if muted.get() { "🔇 Ativar som" } else { "🔊 Mudo" }}
                </button>

                <button
                    class="voice-btn"
                    on:click=move |_| cycle_speed()
                    title="Velocidade"
                >
                    {move || format!("⚡ {}", speed.get().label())}
                </button>
            </div>
        </div>
    }
}