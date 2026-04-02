//! Tool trait and shared types for all tool implementations.
//!
//! Every tool in Claubi implements the `Tool` trait. Tools stay pure —
//! permissions, audit logging, and hooks live in the executor middleware,
//! not in the tools themselves.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Tool input / output types ───────────────────────────────────────────

/// Structured input passed to a tool's `execute` method.
///
/// The `params` map holds tool-specific key-value arguments.
/// For a shell tool, this might be `{"command": "ls -la"}`.
/// For a filesystem tool, `{"path": "/src/main.rs", "content": "..."}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInput {
    pub tool_name: String,
    pub params: HashMap<String, serde_json::Value>,
}

/// Result returned by a tool after execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// Whether the tool completed successfully.
    pub success: bool,
    /// Human-readable output (shown to the model and/or user).
    pub content: String,
    /// Structured data for programmatic consumption (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl ToolOutput {
    /// Convenience constructor for a successful result.
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            data: None,
        }
    }

    /// Convenience constructor for a failed result.
    pub fn err(content: impl Into<String>) -> Self {
        Self {
            success: false,
            content: content.into(),
            data: None,
        }
    }
}

// ── Tool trait ───────────────────────────────────────────────────────────

/// Core trait that every tool must implement.
///
/// Tools declare metadata about themselves (name, safety properties)
/// and provide an async `execute` method. The executor middleware handles
/// permission checks and audit logging — tools should not do that themselves.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique identifier for this tool (e.g., "shell", "filesystem_read").
    fn name(&self) -> &str;

    /// One-line description shown to the model when listing available tools.
    fn description(&self) -> &str;

    /// True if this tool only reads state and never mutates anything.
    ///
    /// Read-only tools can be auto-approved in most permission modes
    /// and can run concurrently with other read-only tools.
    fn is_read_only(&self) -> bool;

    /// True if this tool can safely run in parallel with other
    /// concurrency-safe tools.
    ///
    /// The orchestrator groups consecutive concurrency-safe tool calls
    /// into a single concurrent batch. Conservative default: false.
    fn is_concurrency_safe(&self) -> bool {
        self.is_read_only()
    }

    /// True if this tool performs irreversible actions (delete, overwrite, send).
    ///
    /// Destructive tools get extra scrutiny in the permission engine —
    /// they are never auto-approved even if an allow rule matches,
    /// unless the rule explicitly covers the specific action.
    fn is_destructive(&self) -> bool {
        false
    }

    /// Validate the input before execution.
    ///
    /// Return `Ok(())` if valid, or an error message explaining what's wrong.
    /// The default implementation accepts all input.
    fn validate_input(&self, _input: &ToolInput) -> Result<(), String> {
        Ok(())
    }

    /// Execute the tool with the given input.
    async fn execute(&self, input: &ToolInput) -> ToolOutput;
}
