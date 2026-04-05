//! Interactive REPL for Claubi.
//!
//! Provides the main user-facing loop: reads input via rustyline (with
//! line editing and persistent history), routes to Ollama for inference,
//! renders markdown responses via termimad, and offers to execute any
//! detected shell commands through the tool executor.

pub mod parser;

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use tracing::error;

use crate::agents::ToolExecutor;
use crate::models::ollama::{ChatMessage, OllamaClient};

/// Configuration for the REPL session.
pub struct ReplConfig {
    pub model: String,
    pub ollama: OllamaClient,
    pub executor: ToolExecutor,
}

/// Path to the persistent command history file.
fn history_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claubi_history")
}

/// Run the interactive REPL loop.
///
/// Blocks until the user types "exit" or "quit", presses Ctrl+D, or
/// stdin closes.
pub async fn run(config: ReplConfig) {
    let mut active_model = config.model;
    print_banner(&active_model);

    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            print_error(&format!("failed to initialize line editor: {e}"));
            return;
        }
    };

    // Load history from disk (ignore if file doesn't exist yet).
    let hist_path = history_path();
    let _ = rl.load_history(&hist_path);

    let mut chat_history: Vec<ChatMessage> = Vec::new();

    loop {
        match rl.readline(&format!("{} ", "claubi>".green().bold())) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                rl.add_history_entry(trimmed).unwrap_or(false);

                match trimmed {
                    "exit" | "quit" => {
                        print_system("goodbye.");
                        break;
                    }
                    "/help" => print_help(),
                    "/models" => handle_models(&config.ollama).await,
                    "/model" => handle_models(&config.ollama).await,
                    "/clear" => {
                        chat_history.clear();
                        print_system("conversation cleared.");
                    }
                    cmd if cmd.starts_with("/model ") => {
                        handle_model_switch(&config.ollama, cmd, &mut active_model).await;
                    }
                    input => {
                        let response = handle_chat(
                            &config.ollama,
                            &active_model,
                            &mut chat_history,
                            input,
                        )
                        .await;

                        if let Some(text) = response {
                            let commands = parser::extract_commands(&text);
                            if !commands.is_empty() {
                                handle_detected_commands(&config.executor, &commands).await;
                            }
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C — cancel current input, continue loop.
                println!();
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D — exit gracefully.
                print_system("goodbye.");
                break;
            }
            Err(e) => {
                print_error(&format!("input error: {e}"));
                break;
            }
        }
    }

    // Persist history on exit.
    if let Err(e) = rl.save_history(&hist_path) {
        print_error(&format!("failed to save history: {e}"));
    }
}

// ── Command handlers ────────────────────────────────────────────────────

/// Send user input to the Ollama model and print the response.
/// Returns the full response text if successful (for command parsing).
async fn handle_chat(
    ollama: &OllamaClient,
    model: &str,
    history: &mut Vec<ChatMessage>,
    user_input: &str,
) -> Option<String> {
    history.push(ChatMessage {
        role: "user".into(),
        content: user_input.to_owned(),
    });

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
            println!();

            if full_response.is_empty() {
                None
            } else {
                // Re-render the complete response with markdown formatting.
                render_markdown(&full_response);

                history.push(ChatMessage {
                    role: "assistant".into(),
                    content: full_response.clone(),
                });
                Some(full_response)
            }
        }
        Err(e) => {
            history.pop();
            print_error(&format!("ollama error: {e}"));
            None
        }
    }
}

/// Render markdown text to the terminal using termimad.
fn render_markdown(text: &str) {
    // Only re-render if the response contains markdown syntax worth formatting.
    let has_markdown = text.contains("```")
        || text.contains("**")
        || text.contains("## ")
        || text.contains("- ")
        || text.contains("1. ")
        || text.contains('`');

    if !has_markdown {
        return;
    }

    // Print a separator then the formatted version.
    println!("{}", "─".repeat(40).dimmed());
    termimad::print_text(text);
}

/// Present detected commands to the user and offer execution.
async fn handle_detected_commands(executor: &ToolExecutor, commands: &[String]) {
    println!();
    println!(
        "{}",
        format!(
            "[Claubi detected {} command{}]",
            commands.len(),
            if commands.len() == 1 { "" } else { "s" }
        )
        .cyan()
        .bold()
    );

    for (i, cmd) in commands.iter().enumerate() {
        println!("  {}: {}", format!("{}", i + 1).cyan(), cmd.white());
    }

    println!();
    print!("{}", "Run these commands? [y]es all / [n]o / [s]elect individually: ".yellow());
    io::stdout().flush().unwrap_or(());

    let choice = read_line_lowercase();

    match choice.as_str() {
        "y" | "yes" => {
            run_commands(executor, commands).await;
        }
        "s" | "select" => {
            run_commands_selectively(executor, commands).await;
        }
        _ => {
            print_system("skipped.");
        }
    }
}

/// Execute all commands sequentially through the tool executor.
async fn run_commands(executor: &ToolExecutor, commands: &[String]) {
    for (i, cmd) in commands.iter().enumerate() {
        print_command_header(i + 1, cmd);
        execute_single_command(executor, cmd).await;
    }
}

/// Prompt for each command individually, then execute approved ones.
async fn run_commands_selectively(executor: &ToolExecutor, commands: &[String]) {
    for (i, cmd) in commands.iter().enumerate() {
        print!(
            "  {} {} {} ",
            format!("[{}/{}]", i + 1, commands.len()).dimmed(),
            cmd.white(),
            "[y/n]?".yellow()
        );
        io::stdout().flush().unwrap_or(());

        let choice = read_line_lowercase();
        if matches!(choice.as_str(), "y" | "yes") {
            execute_single_command(executor, cmd).await;
        } else {
            print_system("  skipped.");
        }
    }
}

/// Run one command through the executor and print the result.
async fn execute_single_command(executor: &ToolExecutor, cmd: &str) {
    let mut params = HashMap::new();
    params.insert(
        "command".into(),
        serde_json::Value::String(cmd.to_owned()),
    );

    match executor.execute("shell", params).await {
        Ok(output) => {
            if output.success {
                if !output.content.is_empty() {
                    println!("{}", output.content);
                }
            } else {
                print_error(&output.content);
            }
        }
        Err(e) => {
            print_error(&format!("{e}"));
        }
    }
}

/// Print a header before running a command.
fn print_command_header(index: usize, cmd: &str) {
    println!(
        "\n{} {}",
        format!("[{index}]").cyan().bold(),
        cmd.white().bold()
    );
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
    println!(
        "{}",
        "Commands in the model's response will be detected and offered for execution.".dimmed()
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

/// Read a line from stdin and return it lowercased and trimmed.
/// Used for y/n prompts where rustyline would be overkill.
fn read_line_lowercase() -> String {
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).unwrap_or(0);
    buf.trim().to_lowercase()
}
