//! Model clients for local (Ollama) and remote (Claude API) inference.

// Temporary: nothing calls into these modules until the REPL is wired up.
#![allow(dead_code)]

pub mod ollama;

#[allow(unused_imports)]
pub use ollama::{ChatChunk, ChatMessage, ModelInfo, OllamaClient};

/// Errors from the Ollama client.
#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    #[error("cannot connect to ollama: {0}")]
    Connection(reqwest::Error),

    #[error("ollama returned non-success status {status}")]
    Unavailable { status: u16 },

    #[error("ollama API error (HTTP {status}): {message}")]
    Api { status: u16, message: String },

    #[error("failed to deserialize ollama response: {0}")]
    Deserialize(reqwest::Error),

    #[error("stream error: {0}")]
    Stream(String),

    #[error("failed to parse ollama chunk: {message}\nraw: {raw}")]
    Parse { message: String, raw: String },
}
