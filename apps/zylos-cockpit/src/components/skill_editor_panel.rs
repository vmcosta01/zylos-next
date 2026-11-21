//! Skill Editor Panel — Editor visual de skills integrado ao Hermes Agent (Fase 10.4).
//!
//! Conecta-se ao registro do gateway, `/api/v1/skills/contracts`, para listar e salvar skills.
//! Preview YAML com syntax highlighting básico e validação em tempo real.
//! Undo/redo com stack de estados.
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_FASE10.md](../../docs/IMPLEMENTATION_PLAN_FASE10.md) §10.4

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Fetch JSON genérico via gloo-net com retry.
async fn fetch_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    use gloo_net::http::Request;
    let mut last_err = String::new();
    for attempt in 0..3 {
        match crate::auth::authed(Request::get(url)).send().await {
            Ok(resp) if resp.ok() => {
                return resp.json::<T>().await.map_err(|e| format!("{:?}", e));
            }
            Ok(resp) => last_err = format!("HTTP {}", resp.status()),
            Err(e) => last_err = format!("{:?}", e),
        }
        if attempt < 2 {
            gloo_timers::future::TimeoutFuture::new(500 * (attempt + 1)).await;
        }
    }
    Err(last_err)
}

/// Prefixo do `id` de uma skill criada aqui e ainda não salva no gateway (o nome é editável).
const PREFIXO_NOVA: &str = "nova:";

/// Primeiro nome `nova-skill`, `nova-skill-2`, ... que ainda não existe na lista (sem diferenciar
/// maiúsculas: o gateway recusa nomes que só diferem nelas).
fn nome_livre(skills: &[SkillDef]) -> String {
    let existe = |n: &str| skills.iter().any(|s| s.name.eq_ignore_ascii_case(n));
    let mut n = 1;
    loop {
        let c = if n == 1 {
            "nova-skill".to_string()
        } else {
            format!("nova-skill-{n}")
        };
        if !existe(&c) {
            return c;
        }
        n += 1;
    }
}

/// Tipo de step dentro de uma skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Action,
    Condition,
    Validate,
    Transform,
}

impl StepType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Action => "Action",
            Self::Condition => "Condition",
            Self::Validate => "Validate",
            Self::Transform => "Transform",
        }
    }

    #[allow(dead_code)]
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Action => "step-type-action",
            Self::Condition => "step-type-condition",
            Self::Validate => "step-type-validate",
            Self::Transform => "step-type-transform",
        }
    }
}

/// Step individual de uma skill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillStep {
    pub id: String,
    pub step_type: StepType,
    pub content: String,
    pub params: Vec<(String, String)>,
}

/// Definição completa de uma skill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub triggers: Vec<String>,
    pub steps: Vec<SkillStep>,
    pub validators: Vec<String>,
}

/// Contrato simplificado retornado pela API Hermes.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HermesContract {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    category: String,
}

/// Token types para syntax highlighting YAML.
#[derive(Debug, Clone, PartialEq)]
enum YamlToken {
    Key(String),
    StringLit(String),
    Number(String),
    Bool(String),
    Comment(String),
    Dash,
    Colon,
    Plain(String),
}

/// Parser YAML simples para syntax highlighting.
fn tokenize_yaml(yaml: &str) -> Vec<Vec<YamlToken>> {
    yaml.lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                return vec![YamlToken::Comment(line.to_string())];
            }
            let mut tokens = Vec::new();
            let indent = line.len() - line.trim_start().len();
            if indent > 0 {
                tokens.push(YamlToken::Plain(" ".repeat(indent)));
            }
            let rest = trimmed;
            if let Some(after_dash) = rest.strip_prefix("- ") {
                tokens.push(YamlToken::Dash);
                tokens.push(YamlToken::Plain(" ".to_string()));
                if let Some(colon_pos) = after_dash.find(':') {
                    let key = &after_dash[..colon_pos];
                    tokens.push(YamlToken::Key(key.to_string()));
                    tokens.push(YamlToken::Colon);
                    let val = after_dash[colon_pos + 1..].trim();
                    if !val.is_empty() {
                        tokens.push(classify_value(val));
                    }
                } else {
                    tokens.push(classify_value(after_dash));
                }
            } else if let Some(colon_pos) = rest.find(':') {
                let key = &rest[..colon_pos];
                tokens.push(YamlToken::Key(key.to_string()));
                tokens.push(YamlToken::Colon);
                let val = rest[colon_pos + 1..].trim();
                if !val.is_empty() {
                    tokens.push(classify_value(val));
                }
            } else if !rest.is_empty() {
                tokens.push(classify_value(rest));
            }
            tokens
        })
        .collect()
}

fn classify_value(val: &str) -> YamlToken {
    if val.starts_with('"') || val.starts_with('\'') {
        YamlToken::StringLit(val.to_string())
    } else if val == "true" || val == "false" {
        YamlToken::Bool(val.to_string())
    } else if val.parse::<f64>().is_ok() {
        YamlToken::Number(val.to_string())
    } else {
        YamlToken::Plain(val.to_string())
    }
}

/// Gera HTML com syntax highlighting para preview YAML.
fn render_highlighted_yaml(yaml: &str) -> String {
    let tokens = tokenize_yaml(yaml);
    let mut html = String::new();
    for line_tokens in tokens {
        for token in line_tokens {
            match token {
                YamlToken::Key(k) => html.push_str(&format!(
                    "<span class=\"yaml-key\">{}</span>",
                    html_escape(&k)
                )),
                YamlToken::StringLit(s) => html.push_str(&format!(
                    "<span class=\"yaml-string\">{}</span>",
                    html_escape(&s)
                )),
                YamlToken::Number(n) => html.push_str(&format!(
                    "<span class=\"yaml-number\">{}</span>",
                    html_escape(&n)
                )),
                YamlToken::Bool(b) => html.push_str(&format!(
                    "<span class=\"yaml-bool\">{}</span>",
                    html_escape(&b)
                )),
                YamlToken::Comment(c) => html.push_str(&format!(
                    "<span class=\"yaml-comment\">{}</span>",
                    html_escape(&c)
                )),
                YamlToken::Dash => html.push_str("<span class=\"yaml-dash\">-</span>"),
                YamlToken::Colon => html.push_str("<span class=\"yaml-colon\">:</span>"),
                YamlToken::Plain(p) => html.push_str(&html_escape(&p)),
            }
        }
        html.push('\n');
    }
    html
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Gera preview YAML de uma skill.
fn generate_yaml_preview(skill: &SkillDef) -> String {
    let mut out = String::new();
    out.push_str(&format!("name: {}\n", skill.name));
    out.push_str(&format!("description: {}\n", skill.description));
    out.push_str(&format!("category: {}\n", skill.category));
    if !skill.triggers.is_empty() {
        out.push_str("triggers:\n");
        for t in &skill.triggers {
            out.push_str(&format!("  - {}\n", t));
        }
    }
    if !skill.steps.is_empty() {
        out.push_str("steps:\n");
        for s in &skill.steps {
            out.push_str(&format!("  - id: {}\n", s.id));
            out.push_str(&format!("    type: {:?}\n", s.step_type));
            out.push_str("    content: |\n");
            // Toda linha do conteúdo recua 6 espaços (antes só a primeira: o resto virava YAML quebrado).
            if s.content.is_empty() {
                out.push_str("      \n");
            }
            for linha in s.content.lines() {
                out.push_str(&format!("      {linha}\n"));
            }
            if !s.params.is_empty() {
                out.push_str("    params:\n");
                for (k, v) in &s.params {
                    out.push_str(&format!("      {}: {}\n", k, v));
                }
            }
        }
    }
    if !skill.validators.is_empty() {
        out.push_str("validators:\n");
        for v in &skill.validators {
            out.push_str(&format!("  - {}\n", v));
        }
    }
    out
}

/// O que o editor sabe reler do YAML que ele mesmo gera (nome, descrição e categoria vêm da lista).
#[derive(Debug, Clone, PartialEq)]
struct YamlLido {
    triggers: Vec<String>,
    steps: Vec<SkillStep>,
    validators: Vec<String>,
}

fn tipo_do_passo(t: &str) -> Option<StepType> {
    match t {
        "Action" => Some(StepType::Action),
        "Condition" => Some(StepType::Condition),
        "Validate" => Some(StepType::Validate),
        "Transform" => Some(StepType::Transform),
        _ => None,
    }
}

/// Lê de volta o YAML gerado por [`generate_yaml_preview`]. **Recusa** (Err) qualquer coisa fora
/// desse formato em vez de adivinhar: quem chama então desliga o Save daquela skill, porque salvar
/// uma versão relida pela metade apagaria os passos originais.
fn parse_skill_yaml(yaml: &str) -> Result<YamlLido, String> {
    #[derive(PartialEq)]
    enum Secao {
        Topo,
        Triggers,
        Steps,
        Validators,
    }
    let mut secao = Secao::Topo;
    let mut lido = YamlLido {
        triggers: vec![],
        steps: vec![],
        validators: vec![],
    };
    // Estado dentro de um passo: lendo o bloco `content: |` ou os `params:`.
    let (mut em_content, mut em_params) = (false, false);
    let mut conteudo: Vec<String> = vec![];
    let fecha_conteudo = |passo: &mut SkillStep, linhas: &mut Vec<String>| {
        passo.content = linhas.join("\n");
        linhas.clear();
    };
    for (n, linha) in yaml.lines().enumerate() {
        let erro = |m: &str| format!("linha {}: {m}", n + 1);
        if linha.trim().is_empty() && !em_content {
            continue;
        }
        if em_content {
            if let Some(texto) = linha.strip_prefix("      ") {
                conteudo.push(texto.to_string());
                continue;
            }
            em_content = false;
            if let Some(p) = lido.steps.last_mut() {
                fecha_conteudo(p, &mut conteudo);
            }
        }
        if !linha.starts_with(' ') {
            em_params = false;
            secao = match linha {
                "triggers:" => Secao::Triggers,
                "steps:" => Secao::Steps,
                "validators:" => Secao::Validators,
                l if l.starts_with("name:")
                    || l.starts_with("description:")
                    || l.starts_with("category:") =>
                {
                    Secao::Topo
                }
                _ => return Err(erro("campo desconhecido")),
            };
            continue;
        }
        match secao {
            Secao::Topo => return Err(erro("recuo inesperado")),
            Secao::Triggers => lido.triggers.push(
                linha
                    .strip_prefix("  - ")
                    .ok_or_else(|| erro("trigger mal formado"))?
                    .to_string(),
            ),
            Secao::Validators => lido.validators.push(
                linha
                    .strip_prefix("  - ")
                    .ok_or_else(|| erro("validator mal formado"))?
                    .to_string(),
            ),
            Secao::Steps => {
                if let Some(id) = linha.strip_prefix("  - id: ") {
                    em_params = false;
                    lido.steps.push(SkillStep {
                        id: id.to_string(),
                        step_type: StepType::Action,
                        content: String::new(),
                        params: vec![],
                    });
                    continue;
                }
                let passo = lido.steps.last_mut().ok_or_else(|| erro("passo sem id"))?;
                if let Some(t) = linha.strip_prefix("    type: ") {
                    passo.step_type =
                        tipo_do_passo(t).ok_or_else(|| erro("tipo de passo desconhecido"))?;
                    em_params = false;
                } else if linha == "    content: |" {
                    em_content = true;
                    em_params = false;
                } else if linha == "    params:" {
                    em_params = true;
                } else if let (true, Some(kv)) = (em_params, linha.strip_prefix("      ")) {
                    let (k, v) = kv
                        .split_once(": ")
                        .ok_or_else(|| erro("parâmetro mal formado"))?;
                    passo.params.push((k.to_string(), v.to_string()));
                } else {
                    return Err(erro("linha de passo desconhecida"));
                }
            }
        }
    }
    if em_content {
        if let Some(p) = lido.steps.last_mut() {
            fecha_conteudo(p, &mut conteudo);
        }
    }
    Ok(lido)
}

/// Validação básica do YAML gerado.
fn validate_yaml_preview(yaml: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if yaml.trim().is_empty() {
        errors.push("YAML vazio".to_string());
        return errors;
    }
    if !yaml.contains("name:") {
        errors.push("L1: campo 'name' obrigatório".to_string());
    }
    for (i, line) in yaml.lines().enumerate() {
        if line.contains('\t') {
            errors.push(format!("L{}: tabulação não permitida, use espaços", i + 1));
        }
    }
    errors
}

/// Componente principal do Skill Editor Panel.
#[component]
pub fn SkillEditorPanel() -> impl IntoView {
    let skills = RwSignal::new(Vec::<SkillDef>::new());
    let selected_idx = RwSignal::new(None::<usize>);
    let preview_yaml = RwSignal::new(String::new());
    let highlighted_html = RwSignal::new(String::new());
    let validation_errors = RwSignal::new(Vec::<String>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let undo_stack = RwSignal::new(Vec::<SkillDef>::new());
    let redo_stack = RwSignal::new(Vec::<SkillDef>::new());
    let save_status = RwSignal::new(None::<String>);
    // Skills salvas cujos passos já foram relidos do gateway (só elas podem ser salvas de novo) e
    // as que não puderam ser relidas, com o motivo (Save desligado para não apagar os passos).
    let carregadas = RwSignal::new(std::collections::HashSet::<String>::new());
    let bloqueadas = RwSignal::new(std::collections::HashMap::<String, String>::new());

    // Carregar skills do Hermes
    spawn_local(async move {
        let url = crate::gateway_http::api_url("/api/v1/skills/contracts").unwrap_or_default();
        match fetch_json::<serde_json::Value>(&url).await {
            Ok(val) => {
                if let Some(arr) = val.get("contracts").and_then(|v| v.as_array()) {
                    let mapped: Vec<SkillDef> = arr
                        .iter()
                        .filter_map(|c| {
                            let contract: HermesContract =
                                serde_json::from_value(c.clone()).ok()?;
                            Some(SkillDef {
                                id: contract.name.clone(),
                                name: contract.name,
                                description: contract.description,
                                category: contract.category,
                                triggers: vec![],
                                steps: vec![],
                                validators: vec![],
                            })
                        })
                        .collect();
                    // Lista vazia não é erro: o botão "+ Nova skill" cria a primeira.
                    skills.set(mapped);
                } else {
                    error_msg.set(Some(
                        "O gateway respondeu sem a lista de skills (campo `contracts`): atualize o gateway."
                            .into(),
                    ));
                }
            }
            Err(e) => {
                error_msg.set(Some(format!("Gateway indisponível: {}", e)));
            }
        }
        loading.set(false);
    });

    // Atualizar preview quando seleção muda
    Effect::new(move |_| {
        if let Some(idx) = selected_idx.get() {
            let current_skills = skills.get();
            if let Some(skill) = current_skills.get(idx) {
                let yaml = generate_yaml_preview(skill);
                preview_yaml.set(yaml.clone());
                highlighted_html.set(render_highlighted_yaml(&yaml));
                validation_errors.set(validate_yaml_preview(&yaml));
            }
        }
    });

    // Ao selecionar uma skill já salva, lê o YAML dela no gateway e recupera os passos.
    Effect::new(move |_| {
        let Some(idx) = selected_idx.get() else {
            return;
        };
        let Some(skill) = skills.with_untracked(|v| v.get(idx).cloned()) else {
            return;
        };
        if skill.id.starts_with(PREFIXO_NOVA)
            || carregadas.with_untracked(|c| c.contains(&skill.name))
            || bloqueadas.with_untracked(|b| b.contains_key(&skill.name))
        {
            return;
        }
        let nome = skill.name;
        spawn_local(async move {
            let url = crate::gateway_http::api_url(&format!(
                "/api/v1/skills/contracts/{}",
                nome.replace(' ', "%20")
            ))
            .unwrap_or_default();
            let lido = match fetch_json::<serde_json::Value>(&url).await {
                Ok(v) => match v.get("yaml").and_then(|y| y.as_str()) {
                    Some(y) => parse_skill_yaml(y),
                    None => Err("a resposta não tem o campo yaml".to_string()),
                },
                Err(e) => Err(format!("não consegui ler a skill no gateway ({e})")),
            };
            match lido {
                Ok(l) => {
                    skills.update(|v| {
                        if let Some(s) = v.iter_mut().find(|s| s.name == nome) {
                            s.triggers = l.triggers;
                            s.steps = l.steps;
                            s.validators = l.validators;
                        }
                    });
                    carregadas.update(|c| {
                        c.insert(nome);
                    });
                }
                Err(motivo) => bloqueadas.update(|b| {
                    b.insert(nome, motivo);
                }),
            }
        });
    });

    let nova_skill = move || {
        let nome = nome_livre(&skills.get_untracked());
        let idx = skills.get_untracked().len();
        skills.update(|v| {
            v.push(SkillDef {
                id: format!("{PREFIXO_NOVA}{nome}"),
                name: nome,
                description: String::new(),
                category: String::new(),
                triggers: vec![],
                steps: vec![],
                validators: vec![],
            })
        });
        selected_idx.set(Some(idx));
        error_msg.set(None);
    };

    // O nome só é editável enquanto a skill ainda não foi para o gateway (uma skill salva é
    // identificada pelo nome; renomear criaria outra).
    let nome_editavel = Memo::new(move |_| {
        selected_idx
            .get()
            .and_then(|i| skills.with(|v| v.get(i).map(|s| s.id.starts_with(PREFIXO_NOVA))))
            .unwrap_or(false)
    });
    let renomear = move |novo: String| {
        if let Some(idx) = selected_idx.get_untracked() {
            skills.update(|v| {
                if let Some(s) = v.get_mut(idx).filter(|s| s.id.starts_with(PREFIXO_NOVA)) {
                    s.name = novo;
                }
            });
        }
    };

    let add_step = move || {
        if let Some(idx) = selected_idx.get_untracked() {
            let current = skills.get_untracked();
            if let Some(mut skill) = current.get(idx).cloned() {
                undo_stack.update(|stack| {
                    stack.push(skill.clone());
                    if stack.len() > 20 {
                        stack.remove(0);
                    }
                });
                redo_stack.set(vec![]);
                let new_step = SkillStep {
                    id: format!("step-{}", skill.steps.len() + 1),
                    step_type: StepType::Action,
                    content: "new action".into(),
                    params: vec![],
                };
                skill.steps.push(new_step);
                let mut updated = current;
                updated[idx] = skill.clone();
                skills.set(updated);
                let yaml = generate_yaml_preview(&skill);
                preview_yaml.set(yaml.clone());
                highlighted_html.set(render_highlighted_yaml(&yaml));
            }
        }
    };

    let undo = move || {
        if let Some(idx) = selected_idx.get_untracked() {
            let stack = undo_stack.get_untracked();
            if let Some(prev) = stack.last() {
                let current = skills.get_untracked();
                if let Some(current_skill) = current.get(idx) {
                    redo_stack.update(|rs| rs.push(current_skill.clone()));
                }
                let mut updated = current;
                updated[idx] = prev.clone();
                skills.set(updated);
                let yaml = generate_yaml_preview(prev);
                preview_yaml.set(yaml.clone());
                highlighted_html.set(render_highlighted_yaml(&yaml));
                undo_stack.update(|s| {
                    s.pop();
                });
            }
        }
    };

    let redo = move || {
        if let Some(idx) = selected_idx.get_untracked() {
            let rstack = redo_stack.get_untracked();
            if let Some(next) = rstack.last() {
                let current = skills.get_untracked();
                if let Some(current_skill) = current.get(idx) {
                    undo_stack.update(|us| us.push(current_skill.clone()));
                }
                let mut updated = current;
                updated[idx] = next.clone();
                skills.set(updated);
                let yaml = generate_yaml_preview(next);
                preview_yaml.set(yaml.clone());
                highlighted_html.set(render_highlighted_yaml(&yaml));
                validation_errors.set(validate_yaml_preview(&yaml));
                redo_stack.update(|s| {
                    s.pop();
                });
            }
        }
    };

    let save_to_backend = move || {
        if let Some(idx) = selected_idx.get_untracked() {
            let current = skills.get_untracked();
            if let Some(skill) = current.get(idx).cloned() {
                let status = save_status;
                let salva_do_gateway = !skill.id.starts_with(PREFIXO_NOVA);
                // Salvar uma skill cujos passos não foram relidos gravaria um YAML sem eles e
                // apagaria os originais: só salva depois de relê-los com sucesso.
                if salva_do_gateway && !carregadas.get_untracked().contains(&skill.name) {
                    let motivo = bloqueadas.get_untracked().get(&skill.name).cloned();
                    status.set(Some(match motivo {
                        Some(m) => format!(
                            "Save desligado: os passos desta skill não puderam ser lidos ({m})"
                        ),
                        None => "Aguarde: os passos desta skill ainda estão carregando".into(),
                    }));
                    spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(4000).await;
                        status.set(None);
                    });
                    return;
                }
                spawn_local(async move {
                    let yaml = generate_yaml_preview(&skill);

                    // Validação local antes de enviar
                    let errs = validate_yaml_preview(&yaml);
                    if !errs.is_empty() {
                        status.set(Some(format!("Validação falhou: {}", errs[0])));
                        gloo_timers::future::TimeoutFuture::new(3000).await;
                        status.set(None);
                        return;
                    }

                    // POST para o endpoint de contratos (não /validate)
                    let post_url = match crate::gateway_http::api_url("/api/v1/skills/contracts") {
                        Ok(u) => u,
                        Err(e) => {
                            status.set(Some(e));
                            return;
                        }
                    };
                    match crate::auth::authed(gloo_net::http::Request::post(&post_url))
                        .header("Content-Type", "application/json")
                        .body(
                            serde_json::json!({
                                "name": skill.name,
                                "description": skill.description,
                                "category": skill.category,
                                "yaml": yaml
                            })
                            .to_string(),
                        )
                        .unwrap_or_else(|_| panic!("invalid body"))
                        .send()
                        .await
                    {
                        Ok(resp) if resp.ok() => {
                            status.set(Some("Salvo com sucesso".into()));
                            // Skill nova que acabou de ir para o gateway passa a ser uma skill
                            // salva comum: o nome deixa de ser editável e os passos já são os atuais.
                            if !salva_do_gateway {
                                skills.update(|v| {
                                    if let Some(s) = v.iter_mut().find(|s| s.id == skill.id) {
                                        s.id = s.name.clone();
                                    }
                                });
                                carregadas.update(|c| {
                                    c.insert(skill.name.clone());
                                });
                            }
                        }
                        Ok(resp) => status.set(Some(format!("Erro HTTP {}", resp.status()))),
                        Err(e) => status.set(Some(format!("Falha de conexão: {:?}", e))),
                    }
                    gloo_timers::future::TimeoutFuture::new(3000).await;
                    status.set(None);
                });
            }
        }
    };

    view! {
        <div class="skill-editor-panel">
            <div class="skill-list-pane">
                <div class="skill-list-header">
                    {"Skills "}
                    <button class="skill-toolbar-btn" on:click=move |_| nova_skill()>{"+ Nova skill"}</button>
                </div>
                <div class="skill-list-items">
                    <For
                        each=move || {
                            let s = skills.get();
                            s.into_iter().enumerate().collect::<Vec<_>>()
                        }
                        key=|(idx, s)| (*idx, s.name.clone())
                        children=move |(idx, skill)| {
                            let idx_copy = idx;
                            let is_selected = move || selected_idx.get() == Some(idx_copy);
                            let item_class = move || {
                                if is_selected() {
                                    "skill-list-item skill-list-item-active"
                                } else {
                                    "skill-list-item"
                                }
                            };
                            view! {
                                <div
                                    class=item_class
                                    on:click=move |_| selected_idx.set(Some(idx_copy))
                                >
                                    <span class="skill-list-item-name">{skill.name.clone()}</span>
                                    <span class="skill-list-item-category">{skill.category.clone()}</span>
                                </div>
                            }
                        }
                    />
                </div>
            </div>

            <div class="skill-editor-pane">
                <div class="skill-toolbar">
                    {move || nome_editavel.get().then(|| view! {
                        <input
                            class="skill-name-input"
                            type="text"
                            aria-label="Nome da skill"
                            maxlength="100"
                            prop:value=move || {
                                selected_idx
                                    .get()
                                    .and_then(|i| skills.with(|v| v.get(i).map(|s| s.name.clone())))
                                    .unwrap_or_default()
                            }
                            on:input=move |ev| renomear(event_target_value(&ev))
                        />
                    })}
                    <button class="skill-toolbar-btn" on:click=move |_| add_step()>{"+ Add Step"}</button>
                    <button class="skill-toolbar-btn" on:click=move |_| undo()>{"↩ Undo"}</button>
                    <button class="skill-toolbar-btn" on:click=move |_| redo()>{"↪ Redo"}</button>
                    <button class="skill-toolbar-btn skill-toolbar-btn-primary" on:click=move |_| save_to_backend()><i class="ph ph-floppy-disk" aria-hidden="true"></i>" Save to Backend"</button>
                    {move || {
                        if let Some(msg) = save_status.get() {
                            view! { <span class="save-status">{msg}</span> }.into_any()
                        } else {
                            ().into_any()
                        }
                    }}
                </div>

                {move || {
                    if loading.get() {
                        view! {
                            <div class="skill-editor-empty">
                                <span class="spinner"></span>
                                <span>{"Carregando skills..."}</span>
                            </div>
                        }.into_any()
                    } else if let Some(err) = error_msg.get() {
                        view! {
                            <div class="skill-editor-empty">
                                <span class="error-icon"><i class="ph ph-warning" aria-hidden="true"></i></span>
                                <span>{err}</span>
                            </div>
                        }.into_any()
                    } else if selected_idx.get().is_none() {
                        view! {
                            <div class="skill-editor-empty">
                                {"Selecione uma skill na lista à esquerda ou crie uma com + Nova skill"}
                            </div>
                        }.into_any()
                    } else {
                        let errs = validation_errors;
                        view! {
                            <div class="skill-steps-area">
                                <For
                                    each=move || {
                                        let idx = selected_idx.get().unwrap_or(0);
                                        let s = skills.get();
                                        s.get(idx).map(|sk| sk.steps.clone()).unwrap_or_default()
                                    }
                                    key=|step| step.id.clone()
                                    children=move |step| {
                                        let stype = step.step_type.clone();
                                        let scontent = step.content.clone();
                                        view! {
                                            <div class="skill-step">
                                                <div class="skill-step-header">
                                                    <span class="step-type-badge"
                                                        class:step-type-action=matches!(stype, StepType::Action)
                                                        class:step-type-condition=matches!(stype, StepType::Condition)
                                                        class:step-type-validate=matches!(stype, StepType::Validate)>
                                                        {stype.label()}
                                                    </span>
                                                    <span class="step-id">{step.id.clone()}</span>
                                                </div>
                                                <div class="step-content-preview">{scontent}</div>
                                            </div>
                                        }
                                    }
                                />
                                {move || {
                                    let errors = errs.get();
                                    if !errors.is_empty() {
                                        view! {
                                            <div class="validation-errors">
                                                <For
                                                    each=move || errors.clone()
                                                    key=|e| e.clone()
                                                    children=move |err| {
                                                        view! { <div class="validation-error">{err}</div> }
                                                    }
                                                />
                                            </div>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }
                                }}
                            </div>
                        }.into_any()
                    }
                }}
            </div>

            <div class="skill-preview-pane">
                <div class="skill-preview-header">{"YAML Preview"}</div>
                <div class="skill-preview-content" inner_html=move || highlighted_html.get()>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(nome: &str) -> SkillDef {
        SkillDef {
            id: nome.into(),
            name: nome.into(),
            description: String::new(),
            category: String::new(),
            triggers: vec![],
            steps: vec![],
            validators: vec![],
        }
    }

    fn skill_completa() -> SkillDef {
        SkillDef {
            id: "deploy".into(),
            name: "deploy".into(),
            description: "Publica".into(),
            category: "ci".into(),
            triggers: vec!["on_push".into(), "manual".into()],
            steps: vec![
                SkillStep {
                    id: "s1".into(),
                    step_type: StepType::Action,
                    content: "echo um\n\necho tres".into(),
                    params: vec![
                        ("timeout".into(), "30".into()),
                        ("url".into(), "a: b".into()),
                    ],
                },
                SkillStep {
                    id: "s2".into(),
                    step_type: StepType::Condition,
                    content: String::new(),
                    params: vec![],
                },
                SkillStep {
                    id: "s3".into(),
                    step_type: StepType::Transform,
                    content: "uma linha".into(),
                    params: vec![],
                },
            ],
            validators: vec!["exit_code == 0".into()],
        }
    }

    #[test]
    fn o_yaml_gerado_volta_igual_ao_que_foi() {
        let original = skill_completa();
        let lido = parse_skill_yaml(&generate_yaml_preview(&original)).unwrap();
        assert_eq!(lido.triggers, original.triggers);
        assert_eq!(lido.steps, original.steps);
        assert_eq!(lido.validators, original.validators);
    }

    #[test]
    fn skill_sem_passos_nem_extras_tambem_volta() {
        let mut s = skill_completa();
        s.triggers.clear();
        s.steps.clear();
        s.validators.clear();
        let lido = parse_skill_yaml(&generate_yaml_preview(&s)).unwrap();
        assert!(lido.triggers.is_empty() && lido.steps.is_empty() && lido.validators.is_empty());
    }

    #[test]
    fn yaml_fora_do_formato_do_editor_e_recusado_em_vez_de_adivinhado() {
        for ruim in [
            "name: x\nfoo: bar\n",                             // campo desconhecido
            "name: x\nsteps:\n  - id: s1\n    type: Magica\n", // tipo desconhecido
            "name: x\nsteps:\n    type: Action\n",             // passo sem id
            "name: x\nsteps:\n  - id: s1\n    extra: 1\n",     // linha de passo desconhecida
            "name: x\ntriggers:\n  nao-e-lista\n",             // trigger mal formado
            "name: x\n  recuado: 1\n",                         // recuo no topo
        ] {
            assert!(parse_skill_yaml(ruim).is_err(), "devia recusar: {ruim:?}");
        }
    }

    #[test]
    fn nome_livre_pula_os_que_ja_existem_sem_diferenciar_maiusculas() {
        assert_eq!(nome_livre(&[]), "nova-skill");
        assert_eq!(nome_livre(&[def("deploy")]), "nova-skill");
        assert_eq!(nome_livre(&[def("nova-skill")]), "nova-skill-2");
        assert_eq!(
            nome_livre(&[def("Nova-Skill"), def("nova-skill-2")]),
            "nova-skill-3"
        );
    }

    #[test]
    fn test_generate_yaml_preview_basic() {
        let skill = SkillDef {
            id: "test".into(),
            name: "test-skill".into(),
            description: "A test".into(),
            category: "testing".into(),
            triggers: vec!["on_test".into()],
            steps: vec![SkillStep {
                id: "s1".into(),
                step_type: StepType::Action,
                content: "echo hello".into(),
                params: vec![],
            }],
            validators: vec![],
        };
        let yaml = generate_yaml_preview(&skill);
        assert!(yaml.contains("name: test-skill"));
        assert!(yaml.contains("triggers:"));
        assert!(yaml.contains("steps:"));
    }

    #[test]
    fn test_validate_yaml_empty() {
        let errors = validate_yaml_preview("");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validate_yaml_missing_name() {
        let errors = validate_yaml_preview("description: test\n");
        assert!(errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn test_validate_yaml_with_tabs() {
        let errors = validate_yaml_preview("name: test\n\tdescription: bad\n");
        assert!(errors.iter().any(|e| e.contains("tabulação")));
    }

    #[test]
    fn test_validate_yaml_valid() {
        let errors = validate_yaml_preview("name: test\ndescription: ok\n");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_step_type_labels() {
        assert_eq!(StepType::Action.label(), "Action");
        assert_eq!(StepType::Condition.label(), "Condition");
        assert_eq!(StepType::Validate.label(), "Validate");
        assert_eq!(StepType::Transform.label(), "Transform");
    }

    #[test]
    fn test_tokenize_yaml_key_value() {
        let tokens = tokenize_yaml("name: test-skill");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0][0], YamlToken::Key(k) if k == "name"));
    }

    #[test]
    fn test_render_highlighted_yaml() {
        let html = render_highlighted_yaml("name: test\n");
        assert!(html.contains("yaml-key"));
        assert!(html.contains("test"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a&b"), "a&amp;b");
    }
}
