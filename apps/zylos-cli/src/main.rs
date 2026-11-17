//! zylos-cli — Command-line interface for Zylos-Next administration.
//!
//! Provides subcommands for managing agents, tasks, configuration, and logs
//! via the zylos-gateway REST API.
//!
//! # Quick Start
//!
//! ```bash
//! zylos status
//! zylos agent list
//! zylos task submit spec.yaml
//! zylos config show
//! zylos logs --tail 50
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::Value;
use tracing_subscriber::EnvFilter;

/// Zylos-Next CLI — manage agents, tasks, and system health from the terminal.
#[derive(Parser)]
#[command(name = "zylos", version, about = "Zylos-Next CLI")]
struct Cli {
    /// Gateway URL (overrides ZYLOS_GATEWAY_URL)
    #[arg(
        long,
        global = true,
        env = "ZYLOS_GATEWAY_URL",
        default_value = "http://localhost:3000"
    )]
    gateway_url: String,

    /// Output format: json or table
    #[arg(long, global = true, default_value = "table")]
    format: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check gateway and subsystem health
    Status,
    /// Manage agents
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Manage tasks
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Show or validate configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage SOAR playbooks and events
    Soar {
        #[command(subcommand)]
        action: SoarAction,
    },
    /// Perguntar a um bot (o zylos pensa sozinho)
    Bot {
        #[command(subcommand)]
        action: BotAction,
    },
    /// Provedores de IA: quais existem, se a chave está pronta, quais modelos oferecem
    Provedores {
        #[command(subcommand)]
        action: ProvedoresAction,
    },
    /// Stream or tail logs
    Logs {
        /// Number of lines to display
        #[arg(long, default_value = "20")]
        tail: usize,
    },
}

#[derive(Subcommand)]
enum AgentAction {
    /// List all registered agents
    List,
    /// Start an agent by ID
    Start { id: String },
    /// Stop an agent gracefully
    Stop { id: String },
    /// Inspect agent details
    Inspect { id: String },
}

#[derive(Subcommand)]
enum TaskAction {
    /// Submit a new task from a YAML/JSON spec file
    Submit { spec_path: String },
    /// Query task status by ID
    Status { id: String },
    /// Cancel a pending or running task
    Cancel { id: String },
}

#[derive(Subcommand)]
enum SoarAction {
    /// List available SOAR playbooks
    ListPlaybooks,
    /// Submit a SOAR event from a JSON/YAML file
    SubmitEvent { event_path: String },
}

#[derive(Subcommand)]
enum BotAction {
    /// Faz uma pergunta ao bot e imprime a resposta
    Perguntar { nome: String, pergunta: String },
}

#[derive(Subcommand)]
enum ProvedoresAction {
    /// Lista os provedores e se a chave de cada um está pronta
    Listar,
    /// Lista os modelos que o provedor oferece (pergunta a ele)
    Modelos { nome: String },
}

/// O gateway espera o provedor por até 120 s; a CLI espera um pouco mais, para
/// receber o 504 explicado em vez de cortar antes.
const TEMPO_BOT: std::time::Duration = std::time::Duration::from_secs(130);

#[derive(Subcommand)]
enum ConfigAction {
    /// Display current effective configuration
    Show,
    /// Validate a config file without applying it
    Validate { path: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .init();

    let cli = Cli::parse();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    match cli.command {
        Commands::Status => cmd_status(&client, &cli.gateway_url).await?,
        Commands::Agent { action } => match action {
            AgentAction::List => cmd_agent_list(&client, &cli.gateway_url).await?,
            AgentAction::Start { id } => cmd_agent_start(&client, &cli.gateway_url, &id).await?,
            AgentAction::Stop { id } => cmd_agent_stop(&client, &cli.gateway_url, &id).await?,
            AgentAction::Inspect { id } => {
                cmd_agent_inspect(&client, &cli.gateway_url, &id).await?
            }
        },
        Commands::Task { action } => match action {
            TaskAction::Submit { spec_path } => {
                cmd_task_submit(&client, &cli.gateway_url, &spec_path).await?
            }
            TaskAction::Status { id } => cmd_task_status(&client, &cli.gateway_url, &id).await?,
            TaskAction::Cancel { id } => cmd_task_cancel(&client, &cli.gateway_url, &id).await?,
        },
        Commands::Config { action } => match action {
            ConfigAction::Show => cmd_config_show(&client, &cli.gateway_url).await?,
            ConfigAction::Validate { path } => cmd_config_validate(&path)?,
        },
        Commands::Soar { action } => match action {
            SoarAction::ListPlaybooks => cmd_soar_list_playbooks(&client, &cli.gateway_url).await?,
            SoarAction::SubmitEvent { event_path } => {
                cmd_soar_submit_event(&client, &cli.gateway_url, &event_path).await?
            }
        },
        Commands::Bot { action } => match action {
            BotAction::Perguntar { nome, pergunta } => {
                cmd_bot_perguntar(&cli.gateway_url, &nome, &pergunta).await?
            }
        },
        Commands::Provedores { action } => {
            let (caminho, formatar): (String, fn(&Value) -> String) = match action {
                ProvedoresAction::Listar => ("provedores".into(), texto_dos_provedores),
                ProvedoresAction::Modelos { nome } => {
                    (format!("provedores/{nome}/modelos"), texto_dos_modelos)
                }
            };
            cmd_provedores(&cli.gateway_url, &caminho, formatar).await?
        }
        Commands::Logs { tail } => cmd_logs(&client, &cli.gateway_url, tail).await?,
    }

    Ok(())
}

async fn cmd_provedores(base: &str, caminho: &str, formatar: fn(&Value) -> String) -> Result<()> {
    let client = reqwest::Client::builder().timeout(TEMPO_BOT).build()?;
    let url = format!("{}/api/v1/{}", base.trim_end_matches('/'), caminho);
    let resp = client
        .get(&url)
        .send()
        .await
        .context("não deu para falar com o gateway")?;
    let ok = resp.status().is_success();
    let txt = resp.text().await.context("resposta do gateway ilegível")?;
    match serde_json::from_str::<Value>(&txt) {
        Ok(v) if ok => println!("{}", formatar(&v)),
        _ => {
            eprintln!("{}", texto_do_bot(false, &txt).unwrap_err());
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Uma linha por provedor: nome, língua, chave pronta ou o problema.
fn texto_dos_provedores(v: &Value) -> String {
    let mut linhas = Vec::new();
    for p in v["provedores"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let estado = if p["chave_pronta"] == true {
            "chave pronta".to_string()
        } else {
            format!(
                "sem chave: {}",
                p["problema_da_chave"].as_str().unwrap_or("?")
            )
        };
        linhas.push(format!(
            "{:<12} {:<20} {}",
            p["nome"].as_str().unwrap_or("?"),
            p["lingua"].as_str().unwrap_or("?"),
            estado
        ));
    }
    linhas.join("\n")
}

/// Um modelo por linha, com o total no fim.
fn texto_dos_modelos(v: &Value) -> String {
    let ms: Vec<&str> = v["modelos"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    format!(
        "{}\n({} modelos em {})",
        ms.join("\n"),
        ms.len(),
        v["provedor"].as_str().unwrap_or("?")
    )
}

async fn cmd_bot_perguntar(base: &str, nome: &str, pergunta: &str) -> Result<()> {
    let client = reqwest::Client::builder().timeout(TEMPO_BOT).build()?;
    let url = format!(
        "{}/api/v1/bots/{}/perguntar",
        base.trim_end_matches('/'),
        nome
    );
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "pergunta": pergunta }))
        .send()
        .await
        .context("não deu para falar com o gateway")?;
    let ok = resp.status().is_success();
    let txt = resp.text().await.context("resposta do gateway ilegível")?;
    match texto_do_bot(ok, &txt) {
        Ok(s) => println!("{s}"),
        Err(s) => {
            eprintln!("{s}");
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Sucesso → o texto da resposta; erro → o `detail` do gateway (ou o texto cru).
fn texto_do_bot(ok: bool, txt: &str) -> std::result::Result<String, String> {
    let v: Option<Value> = serde_json::from_str(txt).ok();
    if ok {
        match v.as_ref().and_then(|v| v["resposta"].as_str()) {
            Some(r) => Ok(r.to_string()),
            None => Err(format!("resposta inesperada do gateway: {txt}")),
        }
    } else {
        Err(v
            .as_ref()
            .and_then(|v| v["detail"].as_str())
            .map(str::to_string)
            .unwrap_or_else(|| txt.trim().to_string()))
    }
}

// ---------------------------------------------------------------------------
// Command implementations
// ---------------------------------------------------------------------------

async fn cmd_status(client: &reqwest::Client, base: &str) -> Result<()> {
    let resp = client
        .get(format!("{base}/gateway/health"))
        .send()
        .await
        .context("Failed to reach gateway")?;

    if !resp.status().is_success() {
        println!("Gateway unhealthy: HTTP {}", resp.status());
        std::process::exit(1);
    }

    let body: Value = resp
        .json()
        .await
        .context("Invalid JSON from /gateway/health")?;
    print_json_or_table(&body, "gateway_health");

    // Phase 2 subsystems
    if let Ok(resp) = client
        .get(format!("{base}/api/v1/phase2/status"))
        .send()
        .await
    {
        if resp.status().is_success() {
            let subs: Value = resp.json().await.unwrap_or_default();
            print_json_or_table(&subs, "subsystems");
        }
    }

    Ok(())
}

async fn cmd_agent_list(client: &reqwest::Client, base: &str) -> Result<()> {
    let resp = client
        .get(format!("{base}/api/v1/agents/count"))
        .send()
        .await
        .context("Failed to fetch agent count")?;

    if resp.status().is_success() {
        let body: Value = resp.json().await?;
        print_json_or_table(&body, "agents");
    } else {
        println!("Agent listing unavailable: HTTP {}", resp.status());
    }
    Ok(())
}

async fn cmd_agent_start(client: &reqwest::Client, base: &str, id: &str) -> Result<()> {
    let resp = client
        .post(format!("{base}/api/v1/agents/{id}/start"))
        .send()
        .await
        .context("Failed to start agent")?;

    handle_response(resp, "agent started").await
}

async fn cmd_agent_stop(client: &reqwest::Client, base: &str, id: &str) -> Result<()> {
    let resp = client
        .post(format!("{base}/api/v1/agents/{id}/stop"))
        .send()
        .await
        .context("Failed to stop agent")?;

    handle_response(resp, "agent stopped").await
}

async fn cmd_agent_inspect(client: &reqwest::Client, base: &str, id: &str) -> Result<()> {
    let resp = client
        .get(format!("{base}/api/v1/agents/{id}"))
        .send()
        .await
        .context("Failed to inspect agent")?;

    if resp.status().is_success() {
        let body: Value = resp.json().await?;
        print_json_or_table(&body, "agent_detail");
    } else {
        println!("Agent not found or error: HTTP {}", resp.status());
    }
    Ok(())
}

async fn cmd_task_submit(client: &reqwest::Client, base: &str, spec_path: &str) -> Result<()> {
    let content = std::fs::read_to_string(spec_path)
        .with_context(|| format!("Cannot read spec file: {spec_path}"))?;

    let payload: Value = serde_json::from_str(&content)
        .or_else(|_| serde_yaml::from_str(&content))
        .context("Spec must be valid JSON or YAML")?;

    // A pergunta acontece sempre: mesmo que a spec já traga allow_partial, o
    // primeiro envio vai sem ele.
    let payload = sem_parte_valida(&payload);
    let resp = enviar_dag(client, base, &payload).await?;

    // O gateway recusa (422) um DAG com etapas que não podem rodar, sem executar
    // nada, e diz o que poderia rodar. Aqui a pessoa decide: só com "s" a parte
    // válida é reenviada; qualquer outra resposta (ou nenhuma) não executa nada.
    if resp.status() == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
        let texto = resp.text().await.unwrap_or_default();
        let corpo: Value = serde_json::from_str(&texto).unwrap_or(Value::Null);
        if let Some((resumo, ha_parte_valida)) = resumo_dag_invalido(&corpo) {
            println!("{resumo}");
            if ha_parte_valida {
                print!("Executar só a parte válida? (s/N) ");
                std::io::Write::flush(&mut std::io::stdout()).ok();
                let mut linha = String::new();
                std::io::stdin().read_line(&mut linha).ok();
                if resposta_e_sim(&linha) {
                    let resp = enviar_dag(client, base, &com_parte_valida(&payload)).await?;
                    return handle_response(resp, "task submitted (parte válida)").await;
                }
            }
            println!("Nada foi executado.");
        } else {
            // outro 422 (ex.: spec com campo faltando): mostra o texto do gateway
            eprintln!("Error (422 Unprocessable Entity): {texto}");
        }
        std::process::exit(1);
    }

    handle_response(resp, "task submitted").await
}

async fn enviar_dag(
    client: &reqwest::Client,
    base: &str,
    payload: &Value,
) -> Result<reqwest::Response> {
    client
        .post(format!("{base}/api/v1/orchestrate"))
        .json(payload)
        .send()
        .await
        .context("Failed to submit task")
}

/// Texto para a pessoa quando o gateway recusou o DAG por etapas inválidas, e se
/// há alguma etapa que poderia rodar. `None` se a resposta não é esse diagnóstico.
fn resumo_dag_invalido(corpo: &Value) -> Option<(String, bool)> {
    let fora = corpo.get("fora")?.as_array()?;
    let executaveis = corpo.get("executaveis")?.as_array()?;
    let mut texto = String::from("O pedido tem etapas que não podem rodar (nada foi executado):\n");
    for f in fora {
        let id = f.get("node_id").and_then(Value::as_str).unwrap_or("?");
        let motivo = f
            .get("motivo")
            .and_then(Value::as_str)
            .unwrap_or("motivo não informado");
        texto.push_str(&format!("  - {id}: {motivo}\n"));
    }
    let nomes: Vec<&str> = executaveis.iter().filter_map(Value::as_str).collect();
    if nomes.is_empty() {
        texto.push_str("Nenhuma etapa poderia rodar.");
    } else {
        texto.push_str(&format!(
            "Poderiam rodar, nesta ordem: {}",
            nomes.join(", ")
        ));
    }
    Some((texto, !nomes.is_empty()))
}

/// Só "s"/"sim" (ou "y"/"yes") confirmam; o padrão é não.
fn resposta_e_sim(linha: &str) -> bool {
    matches!(
        linha.trim().to_lowercase().as_str(),
        "s" | "sim" | "y" | "yes"
    )
}

/// O mesmo pedido sem a marcação de parte válida.
fn sem_parte_valida(payload: &Value) -> Value {
    let mut p = payload.clone();
    if let Some(obj) = p.as_object_mut() {
        obj.remove("allow_partial");
    }
    p
}

/// O mesmo pedido, marcado para executar só a parte válida.
fn com_parte_valida(payload: &Value) -> Value {
    let mut p = payload.clone();
    if let Some(obj) = p.as_object_mut() {
        obj.insert("allow_partial".into(), Value::Bool(true));
    }
    p
}

async fn cmd_task_status(client: &reqwest::Client, base: &str, id: &str) -> Result<()> {
    let resp = client
        .get(format!("{base}/api/v1/tasks/{id}"))
        .send()
        .await
        .context("Failed to query task")?;

    if resp.status().is_success() {
        let body: Value = resp.json().await?;
        print_json_or_table(&body, "task_status");
    } else {
        println!("Task not found: HTTP {}", resp.status());
    }
    Ok(())
}

async fn cmd_task_cancel(client: &reqwest::Client, base: &str, id: &str) -> Result<()> {
    let resp = client
        .post(format!("{base}/api/v1/tasks/{id}/cancel"))
        .send()
        .await
        .context("Failed to cancel task")?;

    handle_response(resp, "task cancelled").await
}

async fn cmd_config_show(client: &reqwest::Client, base: &str) -> Result<()> {
    let resp = client
        .get(format!("{base}/gateway/health"))
        .send()
        .await
        .context("Cannot reach gateway for config inspection")?;

    if resp.status().is_success() {
        let body: Value = resp.json().await?;
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        println!("Gateway returned HTTP {}", resp.status());
    }
    Ok(())
}

fn cmd_config_validate(path: &str) -> Result<()> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("Cannot read config: {path}"))?;

    if let Ok(val) = toml::from_str::<Value>(&content) {
        if val.is_object() || val.is_array() {
            println!("Valid TOML configuration");
            return Ok(());
        }
    }
    if let Ok(val) = serde_json::from_str::<Value>(&content) {
        if val.is_object() || val.is_array() {
            println!("Valid JSON configuration");
            return Ok(());
        }
    }
    if let Ok(val) = serde_yaml::from_str::<Value>(&content) {
        if val.is_object() || val.is_array() {
            println!("Valid YAML configuration");
            return Ok(());
        }
    }
    anyhow::bail!("File is not valid TOML, JSON, or YAML configuration");
}

async fn cmd_soar_list_playbooks(client: &reqwest::Client, base: &str) -> Result<()> {
    let resp = client
        .get(format!("{base}/api/v1/soar/playbooks"))
        .send()
        .await
        .context("Failed to fetch SOAR playbooks")?;

    if resp.status().is_success() {
        let body: Value = resp.json().await?;
        print_json_or_table(&body, "soar_playbooks");
    } else {
        println!("SOAR playbooks unavailable: HTTP {}", resp.status());
    }
    Ok(())
}

async fn cmd_soar_submit_event(
    client: &reqwest::Client,
    base: &str,
    event_path: &str,
) -> Result<()> {
    let content = std::fs::read_to_string(event_path)
        .with_context(|| format!("Cannot read event file: {event_path}"))?;

    let payload: Value = serde_json::from_str(&content)
        .or_else(|_| serde_yaml::from_str(&content))
        .context("Event must be valid JSON or YAML")?;

    let resp = client
        .post(format!("{base}/api/v1/soar/events"))
        .json(&payload)
        .send()
        .await
        .context("Failed to submit SOAR event")?;

    handle_response(resp, "soar event submitted").await
}

async fn cmd_logs(client: &reqwest::Client, base: &str, tail: usize) -> Result<()> {
    let resp = client
        .get(format!("{base}/api/v1/logs?tail={tail}"))
        .send()
        .await
        .context("Failed to fetch logs")?;

    if resp.status().is_success() {
        let text = resp.text().await?;
        print!("{text}");
    } else {
        println!("Log endpoint returned HTTP {}", resp.status());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn handle_response(resp: reqwest::Response, success_msg: &str) -> Result<()> {
    if resp.status().is_success() {
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        if body.is_null() {
            println!("{success_msg}");
        } else {
            print_json_or_table(&body, success_msg);
        }
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        eprintln!("Error ({status}): {text}");
        std::process::exit(1);
    }
    Ok(())
}

fn print_json_or_table(value: &Value, label: &str) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{json}"),
        Err(_) => println!("{label}: {value:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resumo_lista_o_que_fica_fora_e_o_que_poderia_rodar() {
        let corpo = serde_json::json!({
            "executaveis": ["a", "b"],
            "fora": [{"node_id": "x", "motivo": "depende de etapa inexistente: nao-existe"}]
        });
        let (texto, ha_parte) = resumo_dag_invalido(&corpo).unwrap();
        assert!(ha_parte);
        assert!(
            texto.contains("x: depende de etapa inexistente: nao-existe"),
            "{texto}"
        );
        assert!(texto.contains("nesta ordem: a, b"), "{texto}");
    }

    #[test]
    fn resumo_sem_nada_executavel_nao_oferece_parte_valida() {
        let corpo =
            serde_json::json!({"executaveis": [], "fora": [{"node_id": "x", "motivo": "ciclo"}]});
        let (texto, ha_parte) = resumo_dag_invalido(&corpo).unwrap();
        assert!(!ha_parte);
        assert!(texto.contains("Nenhuma etapa poderia rodar"));
    }

    #[test]
    fn resposta_que_nao_e_o_diagnostico_nao_vira_resumo() {
        assert!(resumo_dag_invalido(&serde_json::json!({"error": "outro"})).is_none());
        assert!(resumo_dag_invalido(&Value::Null).is_none());
    }

    #[test]
    fn so_sim_explicito_confirma() {
        for s in ["s", "S", "sim", " Sim \n", "y", "yes"] {
            assert!(resposta_e_sim(s), "devia confirmar: {s:?}");
        }
        for n in ["", "\n", "n", "nao", "não", "talvez", "ss"] {
            assert!(!resposta_e_sim(n), "nao devia confirmar: {n:?}");
        }
    }

    #[test]
    fn primeiro_envio_nunca_leva_a_marcacao_da_spec() {
        let spec = serde_json::json!({"label": "x", "nodes": [1], "allow_partial": true});
        let p = sem_parte_valida(&spec);
        assert!(p.get("allow_partial").is_none());
        assert_eq!(p["label"], "x");
        assert_eq!(p["nodes"], spec["nodes"]);
    }

    #[test]
    fn erro_422_em_texto_puro_nao_vira_diagnostico() {
        // o axum responde texto puro para JSON malformado; não pode virar "null"
        let texto = "Failed to deserialize the JSON body: missing field `action`";
        let corpo: Value = serde_json::from_str(texto).unwrap_or(Value::Null);
        assert!(resumo_dag_invalido(&corpo).is_none());
    }

    #[test]
    fn parte_valida_marca_o_pedido_sem_mudar_o_resto() {
        let p = serde_json::json!({"label": "x", "nodes": [1]});
        let q = com_parte_valida(&p);
        assert_eq!(q["allow_partial"], true);
        assert_eq!(q["label"], "x");
        assert_eq!(q["nodes"], p["nodes"]);
    }

    #[test]
    fn cli_parses_status() {
        let cli = Cli::try_parse_from(["zylos", "status"]).unwrap();
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn cli_parses_bot_perguntar() {
        let cli = Cli::try_parse_from(["zylos", "bot", "perguntar", "vigia", "tudo bem?"]).unwrap();
        if let Commands::Bot {
            action: BotAction::Perguntar { nome, pergunta },
        } = cli.command
        {
            assert_eq!(nome, "vigia");
            assert_eq!(pergunta, "tudo bem?");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn cli_parses_provedores() {
        let cli = Cli::try_parse_from(["zylos", "provedores", "listar"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Provedores {
                action: ProvedoresAction::Listar
            }
        ));
        let cli = Cli::try_parse_from(["zylos", "provedores", "modelos", "nvidia"]).unwrap();
        assert!(
            matches!(cli.command, Commands::Provedores { action: ProvedoresAction::Modelos { nome } } if nome == "nvidia")
        );
    }

    #[test]
    fn texto_de_provedores_e_modelos() {
        let v = serde_json::json!({"provedores": [
            {"nome": "nvidia", "lingua": "openai_chat", "chave_pronta": true},
            {"nome": "openai-api", "lingua": "openai_chat", "chave_pronta": false, "problema_da_chave": "OPENAI_API_KEY não está"}
        ]});
        let t = texto_dos_provedores(&v);
        assert!(t.lines().next().unwrap().contains("chave pronta"));
        assert!(t
            .lines()
            .nth(1)
            .unwrap()
            .contains("sem chave: OPENAI_API_KEY não está"));
        let m =
            texto_dos_modelos(&serde_json::json!({"provedor": "nvidia", "modelos": ["a", "b"]}));
        assert_eq!(m, "a\nb\n(2 modelos em nvidia)");
    }

    #[test]
    fn bot_espera_mais_que_o_gateway() {
        assert!(
            TEMPO_BOT.as_secs() > 120,
            "a CLI não pode cortar antes do 504 do gateway"
        );
    }

    #[test]
    fn texto_do_bot_sucesso_e_erros() {
        assert_eq!(
            texto_do_bot(true, r#"{"resposta":"olá","bot":"v"}"#),
            Ok("olá".to_string())
        );
        assert!(texto_do_bot(true, "{}").is_err());
        assert_eq!(
            texto_do_bot(
                false,
                r#"{"status":424,"detail":"NVIDIA_API_KEY não está no arquivo"}"#
            ),
            Err("NVIDIA_API_KEY não está no arquivo".to_string())
        );
        assert_eq!(
            texto_do_bot(false, "Failed to parse the request body\n"),
            Err("Failed to parse the request body".to_string())
        );
    }

    #[test]
    fn cli_parses_agent_list() {
        let cli = Cli::try_parse_from(["zylos", "agent", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Agent {
                action: AgentAction::List
            }
        ));
    }

    #[test]
    fn cli_parses_task_submit() {
        let cli = Cli::try_parse_from(["zylos", "task", "submit", "spec.yaml"]).unwrap();
        if let Commands::Task {
            action: TaskAction::Submit { spec_path },
        } = cli.command
        {
            assert_eq!(spec_path, "spec.yaml");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn cli_parses_logs_tail() {
        let cli = Cli::try_parse_from(["zylos", "logs", "--tail", "100"]).unwrap();
        if let Commands::Logs { tail } = cli.command {
            assert_eq!(tail, 100);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn cli_default_gateway() {
        let cli = Cli::try_parse_from(["zylos", "status"]).unwrap();
        assert_eq!(cli.gateway_url, "http://localhost:3000");
    }

    #[test]
    fn cli_custom_gateway() {
        let cli =
            Cli::try_parse_from(["zylos", "--gateway-url", "http://gw:8080", "status"]).unwrap();
        assert_eq!(cli.gateway_url, "http://gw:8080");
    }

    #[test]
    fn config_validate_valid_json() {
        let tmp = std::env::temp_dir().join("zylos_cli_test.json");
        std::fs::write(&tmp, r#"{"key": "value"}"#).unwrap();
        assert!(cmd_config_validate(tmp.to_str().unwrap()).is_ok());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn cli_parses_soar_list() {
        let cli = Cli::try_parse_from(["zylos", "soar", "list-playbooks"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Soar {
                action: SoarAction::ListPlaybooks
            }
        ));
    }

    #[test]
    fn cli_parses_soar_submit() {
        let cli = Cli::try_parse_from(["zylos", "soar", "submit-event", "event.json"]).unwrap();
        if let Commands::Soar {
            action: SoarAction::SubmitEvent { event_path },
        } = cli.command
        {
            assert_eq!(event_path, "event.json");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn config_validate_invalid() {
        let tmp = std::env::temp_dir().join("zylos_cli_bad.txt");
        std::fs::write(&tmp, "not valid anything {{{").unwrap();
        assert!(cmd_config_validate(tmp.to_str().unwrap()).is_err());
        let _ = std::fs::remove_file(&tmp);
    }
}
