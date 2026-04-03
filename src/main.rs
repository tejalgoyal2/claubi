//! Claubi — Your Personal AI Engineering Team.
//!
//! Entry point: loads config, initializes subsystems, and starts the
//! interactive REPL.

mod agents;
mod cli;
mod models;
mod security;
mod tools;

use colored::Colorize;

use crate::models::ollama::OllamaClient;
use crate::security::audit::AuditLogger;
use crate::security::permissions::PermissionEngine;
use crate::tools::shell::ShellTool;

/// Default Ollama host if not set in .env.
const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";

/// Default model if not set in .env.
const DEFAULT_MODEL: &str = "qwen2.5-coder:7b";

#[tokio::main]
async fn main() {
    // Load .env (ignore if missing — env vars may be set externally).
    let _ = dotenvy::dotenv();

    // Init tracing.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("claubi=info")),
        )
        .init();

    // Read config from environment.
    let ollama_host =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.into());
    let model =
        std::env::var("CLAUBI_CODE_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into());

    // Initialize Ollama client.
    let ollama = OllamaClient::new(&ollama_host);

    // Initialize audit logger.
    let audit = AuditLogger::from_env();
    if let Err(e) = audit.init().await {
        eprintln!("{} {e}", "warning: audit log init failed:".yellow());
    }

    // Initialize permission engine and tool executor.
    let permissions = PermissionEngine::with_defaults();
    let tools: Vec<Box<dyn tools::Tool>> = vec![Box::new(ShellTool::new())];
    let executor = agents::ToolExecutor::new(tools, permissions, audit);

    // Start the REPL.
    cli::run(cli::ReplConfig {
        model,
        ollama,
        executor,
    })
    .await;
}
