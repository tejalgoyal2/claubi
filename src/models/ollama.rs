//! Ollama REST API client for local model inference.
//!
//! Handles connection, model listing, and streaming chat completions
//! against a local Ollama instance. All network I/O goes through reqwest.

#![allow(dead_code)]

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, instrument, warn};

use crate::models::OllamaError;

/// Ollama REST client.
pub struct OllamaClient {
    client: Client,
    base_url: String,
}

// ── Request / Response types ────────────────────────────────────────────

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Request body for `/api/chat`.
#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
}

/// A single streamed chunk from `/api/chat`.
#[derive(Debug, Deserialize)]
pub struct ChatChunk {
    pub message: ChatMessage,
    pub done: bool,
    #[serde(default)]
    pub total_duration: Option<u64>,
    #[serde(default)]
    pub eval_count: Option<u64>,
}

/// Model metadata returned by `/api/tags`.
#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub digest: String,
}

/// Response from `/api/tags`.
#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

/// System prompt prepended to every Ollama chat call so the model
/// identifies as Claubi rather than its underlying weights and knows
/// it can execute shell commands.
const SYSTEM_PROMPT: &str = "\
You are Claubi, a local AI coding assistant running entirely on the user's machine. \
You are not a cloud service. You are not Qwen, not GPT, not Claude. You are Claubi. \
You help the user write secure, efficient, and well-designed code. \
Be direct, concise, and practical. \
IMPORTANT: You have the ability to execute shell commands on the user's machine. \
When the user asks you to do something that requires running a command \
(listing files, running tests, searching code, installing packages, etc.), \
put the command in a bash code block like:\n\
```bash\n\
ls -la\n\
```\n\
The user will be prompted to approve and execute it. \
Prefer shell commands over writing scripts when the task can be done with standard CLI tools. \
Always use the simplest command that gets the job done.";

// ── Implementation ──────────────────────────────────────────────────────

impl OllamaClient {
    /// Create a new client pointing at the given Ollama host.
    pub fn new(base_url: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_owned();
        Self {
            client: Client::new(),
            base_url,
        }
    }

    /// Check that Ollama is reachable by hitting the root endpoint.
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<(), OllamaError> {
        let url = format!("{}/", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(OllamaError::Connection)?;

        if resp.status().is_success() {
            debug!("ollama health check passed");
            Ok(())
        } else {
            Err(OllamaError::Unavailable {
                status: resp.status().as_u16(),
            })
        }
    }

    /// List all models available on the Ollama instance.
    #[instrument(skip(self))]
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, OllamaError> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(OllamaError::Connection)?;

        if !resp.status().is_success() {
            return Err(OllamaError::Api {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let tags: TagsResponse = resp.json().await.map_err(OllamaError::Deserialize)?;
        debug!(count = tags.models.len(), "listed ollama models");
        Ok(tags.models)
    }

    /// Stream a chat completion, yielding tokens through an mpsc channel.
    ///
    /// Returns a receiver that produces `ChatChunk`s as they arrive.
    /// The final chunk has `done: true` and includes timing metadata.
    #[instrument(skip(self, messages))]
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<ChatChunk, OllamaError>>, OllamaError> {
        let url = format!("{}/api/chat", self.base_url);
        let full_messages = with_system_prompt(messages);
        let body = ChatRequest {
            model,
            messages: &full_messages,
            stream: true,
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(OllamaError::Connection)?;

        if !resp.status().is_success() {
            return Err(OllamaError::Api {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let (tx, rx) = mpsc::channel(64);
        let mut byte_stream = resp.bytes_stream();

        // Ollama streams newline-delimited JSON — one JSON object per line.
        tokio::spawn(async move {
            let mut buffer = String::new();

            while let Some(chunk_result) = byte_stream.next().await {
                let bytes = match chunk_result {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx.send(Err(OllamaError::Stream(e.to_string()))).await;
                        return;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&bytes));

                // Process complete lines (NDJSON)
                while let Some(newline_pos) = buffer.find('\n') {
                    let line: String = buffer.drain(..=newline_pos).collect();
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<ChatChunk>(line) {
                        Ok(chunk) => {
                            let done = chunk.done;
                            if tx.send(Ok(chunk)).await.is_err() {
                                return; // receiver dropped
                            }
                            if done {
                                return;
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "failed to parse ollama chunk");
                            let _ = tx
                                .send(Err(OllamaError::Parse {
                                    message: e.to_string(),
                                    raw: line.to_owned(),
                                }))
                                .await;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Non-streaming chat completion. Convenience wrapper for simple calls.
    #[instrument(skip(self, messages))]
    pub async fn chat(
        &self,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<ChatChunk, OllamaError> {
        let url = format!("{}/api/chat", self.base_url);
        let full_messages = with_system_prompt(messages);
        let body = ChatRequest {
            model,
            messages: &full_messages,
            stream: false,
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(OllamaError::Connection)?;

        if !resp.status().is_success() {
            return Err(OllamaError::Api {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        resp.json().await.map_err(OllamaError::Deserialize)
    }
}

/// Prepend the Claubi system prompt if no system message is already present.
fn with_system_prompt(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    let has_system = messages.first().is_some_and(|m| m.role == "system");
    if has_system {
        return messages.to_vec();
    }

    let mut full = Vec::with_capacity(messages.len() + 1);
    full.push(ChatMessage {
        role: "system".into(),
        content: SYSTEM_PROMPT.into(),
    });
    full.extend_from_slice(messages);
    full
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_serializes_correctly() {
        let msgs = vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }];
        let req = ChatRequest {
            model: "llama3.1:8b",
            messages: &msgs,
            stream: true,
        };
        let json = serde_json::to_value(req).expect("serialization failed");
        assert_eq!(json["model"], "llama3.1:8b");
        assert!(json["stream"].as_bool().expect("stream should be bool"));
        assert_eq!(json["messages"][0]["role"], "user");
    }

    #[test]
    fn chat_chunk_deserializes_partial() {
        let raw = r#"{"message":{"role":"assistant","content":"hi"},"done":false}"#;
        let chunk: ChatChunk = serde_json::from_str(raw).expect("deserialization failed");
        assert!(!chunk.done);
        assert_eq!(chunk.message.content, "hi");
    }

    #[test]
    fn chat_chunk_deserializes_final() {
        let raw = r#"{"message":{"role":"assistant","content":""},"done":true,"total_duration":1234,"eval_count":10}"#;
        let chunk: ChatChunk = serde_json::from_str(raw).expect("deserialization failed");
        assert!(chunk.done);
        assert_eq!(chunk.total_duration, Some(1234));
    }
}
