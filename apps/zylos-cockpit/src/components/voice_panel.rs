//! VoicePanel — voz real no navegador.
//!
//! - Microfone: `navigator.mediaDevices.getUserMedia({audio: true})`
//! - Waveform: `AudioContext` + `AnalyserNode` (barras a partir do espectro real)
//! - Transcrição: `SpeechRecognition` / `webkitSpeechRecognition` (via `js_sys::Reflect`,
//!   a API não é estável no `web-sys`)
//! - Resposta: opcionalmente envia a transcrição para `POST /api/v1/bots/:nome/perguntar`
//!   e lê a resposta com `speechSynthesis`
//!
//! Sem suporte a reconhecimento de voz, o painel mostra
//! [`UNSUPPORTED_MSG`] e não inicia nada.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};

use crate::ui_error::user_message;

/// Mensagem quando o navegador não tem SpeechRecognition.
pub const UNSUPPORTED_MSG: &str = "Seu navegador não suporta reconhecimento de voz";

/// Número de barras do waveform.
pub const BAR_COUNT: usize = 24;

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

    /// Valor para `SpeechSynthesisUtterance.rate`.
    pub fn rate(&self) -> f32 {
        match self {
            Self::Slow => 0.75,
            Self::Normal => 1.0,
            Self::Fast => 1.5,
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

/// Reduz o espectro (`getByteFrequencyData`) a `n` barras (média de cada faixa).
pub fn bars_from_spectrum(spectrum: &[u8], n: usize) -> Vec<u8> {
    if n == 0 || spectrum.is_empty() {
        return vec![0; n];
    }
    let chunk = spectrum.len().div_ceil(n);
    (0..n)
        .map(|i| {
            let s =
                &spectrum[(i * chunk).min(spectrum.len())..((i + 1) * chunk).min(spectrum.len())];
            if s.is_empty() {
                0
            } else {
                (s.iter().map(|&v| v as u32).sum::<u32>() / s.len() as u32) as u8
            }
        })
        .collect()
}

#[derive(Deserialize)]
struct BotAnswer {
    resposta: String,
}

fn get_prop(target: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(target, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

/// Construtor `SpeechRecognition`/`webkitSpeechRecognition`, se existir.
fn recognition_ctor() -> Option<js_sys::Function> {
    let w: JsValue = web_sys::window()?.into();
    ["SpeechRecognition", "webkitSpeechRecognition"]
        .iter()
        .map(|k| get_prop(&w, k))
        .find(|v| v.is_function())
        .map(|v| v.unchecked_into())
}

/// Texto acumulado de um `SpeechRecognitionEvent`.
fn transcript_of(event: &JsValue) -> String {
    let results = get_prop(event, "results");
    let len = get_prop(&results, "length").as_f64().unwrap_or(0.0) as u32;
    let mut out = String::new();
    for i in 0..len {
        let alt = js_sys::Reflect::get_u32(&results, i)
            .and_then(|r| js_sys::Reflect::get_u32(&r, 0))
            .unwrap_or(JsValue::UNDEFINED);
        if let Some(t) = get_prop(&alt, "transcript").as_string() {
            out.push_str(&t);
        }
    }
    out.trim().to_string()
}

/// Recursos de uma escuta em andamento (liberados em [`VoiceSession::stop`]).
#[derive(Default)]
struct VoiceSession {
    recognition: Option<JsValue>,
    stream: Option<web_sys::MediaStream>,
    audio: Option<web_sys::AudioContext>,
    running: Rc<Cell<bool>>,
    callbacks: Vec<Closure<dyn FnMut(JsValue)>>,
}

impl VoiceSession {
    fn stop(&mut self) {
        self.running.set(false);
        if let Some(r) = self.recognition.take() {
            if let Ok(f) = get_prop(&r, "abort").dyn_into::<js_sys::Function>() {
                let _ = f.call0(&r);
            }
        }
        if let Some(s) = self.stream.take() {
            for t in s.get_tracks().iter() {
                if let Ok(t) = t.dyn_into::<web_sys::MediaStreamTrack>() {
                    t.stop();
                }
            }
        }
        if let Some(a) = self.audio.take() {
            let _ = a.close();
        }
        self.callbacks.clear();
    }
}

fn speak(text: &str, rate: f32) -> Result<web_sys::SpeechSynthesisUtterance, String> {
    let synth = web_sys::window()
        .ok_or("sem window")?
        .speech_synthesis()
        .map_err(|_| "speechSynthesis indisponível".to_string())?;
    let u = web_sys::SpeechSynthesisUtterance::new_with_text(text)
        .map_err(|_| "falha ao criar fala".to_string())?;
    u.set_lang("pt-BR");
    u.set_rate(rate);
    synth.cancel();
    synth.speak(&u);
    Ok(u)
}

fn cancel_speech() {
    if let Some(s) = web_sys::window().and_then(|w| w.speech_synthesis().ok()) {
        s.cancel();
    }
}

/// Liga microfone + analisador e anima `bars` até `running` virar `false`.
async fn start_waveform(
    session: Rc<RefCell<VoiceSession>>,
    bars: RwSignal<Vec<u8>>,
) -> Result<(), String> {
    let window = web_sys::window().ok_or("sem window")?;
    let devices = window
        .navigator()
        .media_devices()
        .map_err(|_| "microfone indisponível neste navegador".to_string())?;
    let constraints = web_sys::MediaStreamConstraints::new();
    constraints.set_audio(&JsValue::TRUE);
    let promise = devices
        .get_user_media_with_constraints(&constraints)
        .map_err(|_| "microfone indisponível".to_string())?;
    let stream: web_sys::MediaStream = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|_| "permissão de microfone negada".to_string())?
        .unchecked_into();
    let audio =
        web_sys::AudioContext::new().map_err(|_| "AudioContext indisponível".to_string())?;
    let source = audio
        .create_media_stream_source(&stream)
        .map_err(|_| "falha ao ler o microfone".to_string())?;
    let analyser = audio
        .create_analyser()
        .map_err(|_| "AnalyserNode indisponível".to_string())?;
    analyser.set_fft_size(256);
    source
        .connect_with_audio_node(&analyser)
        .map_err(|_| "falha ao conectar o analisador".to_string())?;

    let running = {
        let mut s = session.borrow_mut();
        if !s.running.get() {
            // Parado enquanto pedia permissão: libera já.
            for t in stream.get_tracks().iter() {
                if let Ok(t) = t.dyn_into::<web_sys::MediaStreamTrack>() {
                    t.stop();
                }
            }
            let _ = audio.close();
            return Ok(());
        }
        s.stream = Some(stream);
        s.audio = Some(audio);
        s.running.clone()
    };

    let mut buf = vec![0u8; analyser.frequency_bin_count() as usize];
    #[allow(clippy::type_complexity)]
    let frame: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let frame2 = frame.clone();
    *frame2.borrow_mut() = Some(Closure::new(move || {
        if !running.get() {
            bars.set(vec![0; BAR_COUNT]);
            frame.borrow_mut().take();
            return;
        }
        analyser.get_byte_frequency_data(&mut buf);
        bars.set(bars_from_spectrum(&buf, BAR_COUNT));
        if let (Some(w), Some(cb)) = (web_sys::window(), frame.borrow().as_ref()) {
            let _ = w.request_animation_frame(cb.as_ref().unchecked_ref());
        }
    }));
    if let Some(cb) = frame2.borrow().as_ref() {
        let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
    }
    Ok(())
}

#[component]
pub fn VoicePanel() -> impl IntoView {
    let supported = recognition_ctor().is_some();
    let phase = RwSignal::new(VoicePhase::Idle);
    let muted = RwSignal::new(false);
    let speed = RwSignal::new(VoiceSpeed::Normal);
    let transcript = RwSignal::new(String::new());
    let answer = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let bot = RwSignal::new(String::new());
    let bars = RwSignal::new(vec![0u8; BAR_COUNT]);
    let session: Rc<RefCell<VoiceSession>> = Rc::default();
    let session = StoredValue::new_local(session);
    // Mantém a utterance viva até terminar (o navegador pode coletá-la antes do `onend`).
    let utterance = StoredValue::new_local(
        None::<(
            web_sys::SpeechSynthesisUtterance,
            Closure<dyn FnMut(JsValue)>,
        )>,
    );

    let stop_all = move || {
        session.with_value(|s| s.borrow_mut().stop());
        cancel_speech();
    };
    on_cleanup(stop_all);

    let fail = move |e: String| {
        session.with_value(|s| s.borrow_mut().stop());
        error.set(Some(user_message(&e)));
        phase.set(VoicePhase::Error);
    };

    let respond = move |text: String| {
        let name = bot.get_untracked().trim().to_string();
        if text.is_empty() || name.is_empty() {
            phase.set(VoicePhase::Idle);
            return;
        }
        phase.set(VoicePhase::Processing);
        spawn_local(async move {
            let path = format!("/api/v1/bots/{}/perguntar", crate::auth::url_encode(&name));
            let body = serde_json::json!({ "pergunta": text });
            match crate::gateway_http::post_json::<_, BotAnswer>(&path, &body).await {
                Ok(r) => {
                    answer.set(r.resposta.clone());
                    if muted.get_untracked() || phase.get_untracked() != VoicePhase::Processing {
                        phase.set(VoicePhase::Idle);
                        return;
                    }
                    match speak(&r.resposta, speed.get_untracked().rate()) {
                        Ok(u) => {
                            phase.set(VoicePhase::Speaking);
                            let on_end = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                                if phase.get_untracked() == VoicePhase::Speaking {
                                    phase.set(VoicePhase::Idle);
                                }
                            });
                            u.set_onend(Some(on_end.as_ref().unchecked_ref()));
                            utterance.set_value(Some((u, on_end)));
                        }
                        Err(e) => fail(e),
                    }
                }
                Err(e) => fail(e),
            }
        });
    };

    let start_listening = move || {
        let Some(ctor) = recognition_ctor() else {
            error.set(Some(UNSUPPORTED_MSG.to_string()));
            return;
        };
        stop_all();
        error.set(None);
        transcript.set(String::new());
        answer.set(String::new());
        let rec = match js_sys::Reflect::construct(&ctor, &js_sys::Array::new()) {
            Ok(r) => r,
            Err(_) => return fail(UNSUPPORTED_MSG.to_string()),
        };
        let set = |k: &str, v: JsValue| {
            let _ = js_sys::Reflect::set(&rec, &JsValue::from_str(k), &v);
        };
        set("lang", JsValue::from_str("pt-BR"));
        set("interimResults", JsValue::TRUE);
        set("continuous", JsValue::FALSE);

        let on_result = Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
            transcript.set(transcript_of(&ev));
        });
        let on_error = Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
            let code = get_prop(&ev, "error").as_string().unwrap_or_default();
            if code != "aborted" && code != "no-speech" {
                fail(format!("reconhecimento de voz: {code}"));
            }
        });
        let on_end = Closure::<dyn FnMut(JsValue)>::new(move |_| {
            session.with_value(|s| s.borrow_mut().stop());
            if phase.get_untracked() == VoicePhase::Listening {
                respond(transcript.get_untracked());
            }
        });
        set("onresult", on_result.as_ref().clone());
        set("onerror", on_error.as_ref().clone());
        set("onend", on_end.as_ref().clone());

        if let Ok(f) = get_prop(&rec, "start").dyn_into::<js_sys::Function>() {
            if f.call0(&rec).is_err() {
                return fail("não foi possível iniciar o reconhecimento".into());
            }
        }
        let shared = session.get_value();
        {
            let mut s = shared.borrow_mut();
            s.running = Rc::new(Cell::new(true));
            s.recognition = Some(rec);
            s.callbacks = vec![on_result, on_error, on_end];
        }
        phase.set(VoicePhase::Listening);
        spawn_local(async move {
            if let Err(e) = start_waveform(shared, bars).await {
                web_sys::console::warn_1(&format!("voz: waveform: {e}").into());
                error.set(Some(user_message(&e)));
            }
        });
    };

    let interrupt = move || {
        stop_all();
        phase.set(VoicePhase::Idle);
    };

    view! {
        <div class="voice-panel">
            <div class="voice-panel-header">
                <h3 class="voice-panel-title"><i class="ph ph-microphone" aria-hidden="true"></i>" Painel de Voz"</h3>
            </div>

            {(!supported).then(|| view! {
                <div class="voice-unsupported" role="alert">{UNSUPPORTED_MSG}</div>
            })}
            {move || error.get().map(|e| view! { <div class="voice-error" role="alert">{e}</div> })}

            <div class=move || format!("voice-waveform {}", phase.get().css_class())>
                {move || bars.get().into_iter().map(|v| {
                    let h = 4 + (v as u32 * 60 / 255);
                    view! { <div class="waveform-bar" style=format!("height:{h}px")></div> }
                }).collect_view()}
            </div>

            <div class="voice-phase-indicator">
                <span class="voice-phase-icon">{move || phase.get().icon()}</span>
                <span class="voice-phase-label">{move || phase.get().label()}</span>
            </div>

            <label class="voice-bot">
                "Bot (opcional): "
                <input
                    type="text"
                    placeholder="nome do bot para responder"
                    prop:value=move || bot.get()
                    on:input=move |ev| bot.set(event_target_value(&ev))
                />
            </label>

            <div class="voice-transcript">
                {move || {
                    let t = transcript.get();
                    if t.is_empty() {
                        view! { <span class="voice-transcript-empty">"Nenhuma transcrição ainda."</span> }.into_any()
                    } else {
                        view! { <span class="voice-transcript-text">{t}</span> }.into_any()
                    }
                }}
            </div>
            {move || {
                let a = answer.get();
                (!a.is_empty()).then(|| view! { <div class="voice-answer">{a}</div> })
            }}

            <div class="voice-controls">
                <button
                    class="voice-btn voice-btn-primary"
                    on:click=move |_| start_listening()
                    disabled=move || !supported || matches!(phase.get(), VoicePhase::Listening | VoicePhase::Processing)
                    title="Iniciar escuta"
                >
                    <i class="ph ph-microphone" aria-hidden="true"></i>" Ouvir"
                </button>
                <button
                    class="voice-btn voice-btn-danger"
                    on:click=move |_| interrupt()
                    disabled=move || matches!(phase.get(), VoicePhase::Idle | VoicePhase::Error)
                    title="Interromper"
                >
                    "⏹ Parar"
                </button>
                <button
                    class="voice-btn"
                    on:click=move |_| {
                        muted.update(|m| *m = !*m);
                        if muted.get_untracked() {
                            cancel_speech();
                        }
                    }
                    title="Mudo"
                >
                    {move || if muted.get() { "Ativar som" } else { "Mudo" }}
                </button>
                <button class="voice-btn" on:click=move |_| speed.update(|s| *s = s.next()) title="Velocidade">
                    {move || speed.get().label().to_string()}
                </button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bars_from_spectrum_averages() {
        let spec: Vec<u8> = (0..8).map(|i| i * 10).collect();
        assert_eq!(bars_from_spectrum(&spec, 4), vec![5, 25, 45, 65]);
        assert_eq!(bars_from_spectrum(&[], 3), vec![0, 0, 0]);
        assert_eq!(bars_from_spectrum(&[255; 5], 8).len(), 8);
    }

    #[test]
    fn test_speed_rate_cycle() {
        assert_eq!(VoiceSpeed::Normal.rate(), 1.0);
        assert_eq!(VoiceSpeed::Fast.next(), VoiceSpeed::Slow);
        assert_eq!(VoiceSpeed::Slow.rate(), 0.75);
    }

    #[test]
    fn test_phase_labels() {
        assert_eq!(VoicePhase::Listening.label(), "Ouvindo...");
        assert_eq!(VoicePhase::Error.css_class(), "voice-phase-error");
    }
}
