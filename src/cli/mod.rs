//! Interactive REPL for Claubi.
//!
//! Provides the main user-facing loop: reads input, routes to Ollama
//! for inference or handles built-in commands, and prints responses.

use std::io::{self, BufRead, Write};

use colored::Colorize;
use tracing::error;

use crate::models::ollama::{ChatMessage, OllamaClient};

/// Configuration for the REPL session.
pub struct ReplConfig {
    pub model: String,
    pub ollama: OllamaClient,
}

/// Run the interactive REPL loop.
///
/// Blocks until the user types "exit" or "quit", or stdin closes.
pub async fn run(config: ReplConfig) {
    let mut active_model = config.model;
    print_banner(&active_model);

    let mut history: Vec<ChatMessage> = Vec::new();
    let stdin = io::stdin();

    loop {
        print_prompt();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                print_error(&format!("failed to read input: {e}"));
                continue;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match trimmed {
            "exit" | "quit" => {
                print_system("goodbye.");
                break;
            }
            "/help" => print_help(),
            "/models" => handle_models(&config.ollama).await,
            "/model" => handle_models(&config.ollama).await,
            "/clear" => {
                history.clear();
                print_system("conversation cleared.");
            }
            cmd if cmd.starts_with("/model ") => {
                handle_model_switch(&config.ollama, cmd, &mut active_model).await;
            }
            input => {
                handle_chat(&config.ollama, &active_model, &mut history, input).await;
            }
        }
    }
}

// ── Command handlers ────────────────────────────────────────────────────

/// Send user input to the Ollama model and print the response.
async fn handle_chat(
    ollama: &OllamaClient,
    model: &str,
    history: &mut Vec<ChatMessage>,
    user_input: &str,
) {
    history.push(ChatMessage {
        role: "user".into(),
        content: user_input.to_owned(),
    });

    // Use streaming for a responsive feel.
    match ollama.chat_stream(model, history).await {
        Ok(mut rx) => {
            let mut full_response = String::new();
            while let Some(chunk_result) = rx.recv().await {
                match chunk_result {
                    Ok(chunk) => {
                        let token = &chunk.message.content;
                        print!("{}", token.white());
                        io::stdout().flush().unwrap_or(());
                        full_response.push_str(token);
                    }
                    Err(e) => {
                        print_error(&format!("\nstream error: {e}"));
                        break;
                    }
                }
            }
            println!(); // newline after streamed response

            if !full_response.is_empty() {
                history.push(ChatMessage {
                    role: "assistant".into(),
                    content: full_response,
                });
            }
        }
        Err(e) => {
            // Pop the user message since we never got a response.
            history.pop();
            print_error(&format!("ollama error: {e}"));
        }
    }
}

/// Switch the active model for this session.
async fn handle_model_switch(
    ollama: &OllamaClient,
    cmd: &str,
    active_model: &mut String,
) {
    let requested = cmd.strip_prefix("/model ").unwrap_or("").trim();
    if requested.is_empty() {
        handle_models(ollama).await;
        return;
    }

    match ollama.list_models().await {
        Ok(models) => {
            if models.iter().any(|m| m.name == requested) {
                *active_model = requested.to_owned();
                println!("{}", format!("switched to {requested}").green());
            } else {
                print_error(&format!(
                    "model '{requested}' not found. Run /models to see available models."
                ));
            }
        }
        Err(e) => {
            print_error(&format!("failed to list models: {e}"));
        }
    }
}

/// List available Ollama models.
async fn handle_models(ollama: &OllamaClient) {
    match ollama.list_models().await {
        Ok(models) if models.is_empty() => {
            print_system("no models found. Pull one with: ollama pull <model>");
        }
        Ok(models) => {
            print_system(&format!("available models ({}):", models.len()));
            for m in &models {
                let size_mb = m.size / (1024 * 1024);
                println!(
                    "  {} {}",
                    m.name.white().bold(),
                    format!("({size_mb} MB)").dimmed()
                );
            }
        }
        Err(e) => {
            print_error(&format!("failed to list models: {e}"));
        }
    }
}

// ── Display helpers ─────────────────────────────────────────────────────

/// Print the welcome banner on REPL startup.
fn print_banner(model: &str) {
    let version = env!("CARGO_PKG_VERSION");
    println!();
    println!("{}", "┌─────────────────────────────────────┐".dimmed());
    println!(
        "{}  {}  {}",
        "│".dimmed(),
        "claubi".green().bold(),
        format!("v{version}").dimmed()
    );
    println!(
        "{}  {} {}",
        "│".dimmed(),
        "model:".dimmed(),
        model.white()
    );
    println!(
        "{}  {} {} {} {} {}",
        "│".dimmed(),
        "type".dimmed(),
        "/help".yellow(),
        "for commands,".dimmed(),
        "exit".yellow(),
        "to quit".dimmed()
    );
    println!("{}", "└─────────────────────────────────────┘".dimmed());
    println!();
}

/// Print the input prompt.
fn print_prompt() {
    print!("{} ", "claubi>".green().bold());
    io::stdout().flush().unwrap_or(());
}

/// Print the /help output.
fn print_help() {
    println!();
    println!("{}", "Commands:".white().bold());
    println!("  {}          — list available Ollama models", "/models".yellow());
    println!("  {} {} — switch to a different model", "/model".yellow(), "<name>".dimmed());
    println!("  {}           — clear conversation history", "/clear".yellow());
    println!("  {}            — show this help message", "/help".yellow());
    println!("  {}            — quit Claubi", "exit".yellow());
    println!();
    println!(
        "{}",
        "Anything else is sent to the model as a chat message.".dimmed()
    );
    println!();
}

/// Print a system message in dim gray.
fn print_system(msg: &str) {
    println!("{}", msg.dimmed());
}

/// Print an error message in red.
fn print_error(msg: &str) {
    error!("{}", msg);
    println!("{}", msg.red());
}
