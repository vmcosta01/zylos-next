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
    #[arg(long, global = true, env = "ZYLOS_GATEWAY_URL", default_value = "http://localhost:3000")]
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
enum ConfigAction {
    /// Display current effective configuration
    Show,
    /// Validate a config file without applying it
    Validate { path: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
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
            AgentAction::Inspect { id } => cmd_agent_inspect(&client, &cli.gateway_url, &id).await?,
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
            SoarAction::SubmitEvent { event_path } => cmd_soar_submit_event(&client, &cli.gateway_url, &event_path).await?,
        },
        Commands::Logs { tail } => cmd_logs(&client, &cli.gateway_url, tail).await?,
    }

    Ok(())
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

    let body: Value = resp.json().await.context("Invalid JSON from /gateway/health")?;
    print_json_or_table(&body, "gateway_health");

    // Phase 2 subsystems
    if let Ok(resp) = client.get(format!("{base}/api/v1/phase2/status")).send().await {
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

    let resp = client
        .post(format!("{base}/api/v1/orchestrate"))
        .json(&payload)
        .send()
        .await
        .context("Failed to submit task")?;

    handle_response(resp, "task submitted").await
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
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read config: {path}"))?;

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

async fn cmd_soar_submit_event(client: &reqwest::Client, base: &str, event_path: &str) -> Result<()> {
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
    fn cli_parses_status() {
        let cli = Cli::try_parse_from(["zylos", "status"]).unwrap();
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn cli_parses_agent_list() {
        let cli = Cli::try_parse_from(["zylos", "agent", "list"]).unwrap();
        assert!(matches!(cli.command, Commands::Agent { action: AgentAction::List }));
    }

    #[test]
    fn cli_parses_task_submit() {
        let cli = Cli::try_parse_from(["zylos", "task", "submit", "spec.yaml"]).unwrap();
        if let Commands::Task { action: TaskAction::Submit { spec_path } } = cli.command {
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
        let cli = Cli::try_parse_from(["zylos", "--gateway-url", "http://gw:8080", "status"]).unwrap();
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
        assert!(matches!(cli.command, Commands::Soar { action: SoarAction::ListPlaybooks }));
    }

    #[test]
    fn cli_parses_soar_submit() {
        let cli = Cli::try_parse_from(["zylos", "soar", "submit-event", "event.json"]).unwrap();
        if let Commands::Soar { action: SoarAction::SubmitEvent { event_path } } = cli.command {
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
