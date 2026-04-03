//! Tool executor — middleware connecting tools, permissions, and audit logging.
//!
//! The executor is the single entry point for running any tool. It enforces
//! the permission engine, times execution, and logs every invocation to the
//! audit trail. Tools themselves never check permissions or write audit entries.

// The executor is constructed in main.rs but not called until tool-call
// parsing is added to the REPL.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Instant;

use chrono::Utc;
use colored::Colorize;
use tracing::{debug, info, warn};

use crate::security::audit::{self, AuditEntry, AuditLogger, ResultStatus};
use crate::security::permissions::{PermissionBehavior, PermissionEngine};
use crate::tools::{Tool, ToolInput, ToolOutput};

/// Middleware that dispatches tool calls through the permission engine
/// and audit logger.
pub struct ToolExecutor {
    tools: Vec<Box<dyn Tool>>,
    permissions: PermissionEngine,
    audit: AuditLogger,
}

/// Errors that can occur during tool execution dispatch.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),

    #[error("input validation failed: {0}")]
    ValidationFailed(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("user rejected the tool call")]
    UserRejected,

    #[error("audit logging failed: {0}")]
    AuditFailed(#[from] crate::security::AuditError),
}

impl ToolExecutor {
    /// Create a new executor with the given tools, permission engine, and audit logger.
    pub fn new(
        tools: Vec<Box<dyn Tool>>,
        permissions: PermissionEngine,
        audit: AuditLogger,
    ) -> Self {
        Self {
            tools,
            permissions,
            audit,
        }
    }

    /// Execute a tool call by name with the given parameters.
    ///
    /// Convenience wrapper that builds the `ToolInput` internally.
    pub async fn execute(
        &self,
        tool_name: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<ToolOutput, ExecutorError> {
        let input = ToolInput {
            tool_name: tool_name.to_owned(),
            params,
        };
        self.execute_tool_call(&input).await
    }

    /// Execute a tool call through the full middleware pipeline:
    /// find → validate → permission check → execute → audit log.
    pub async fn execute_tool_call(
        &self,
        input: &ToolInput,
    ) -> Result<ToolOutput, ExecutorError> {
        // (a) Find the tool.
        let tool = self.find_tool(&input.tool_name)?;

        // (b) Validate input.
        tool.validate_input(input)
            .map_err(ExecutorError::ValidationFailed)?;

        // (c) Permission check.
        let input_summary = build_input_summary(input);
        let decision = self.permissions.evaluate(&input.tool_name, &input_summary);

        let permitted = match decision.behavior {
            PermissionBehavior::Deny => {
                let reason = decision
                    .matched_rule
                    .as_ref()
                    .and_then(|r| r.pattern.as_deref())
                    .unwrap_or("policy");

                warn!(tool = input.tool_name.as_str(), reason, "permission denied");
                self.log_audit(
                    &input.tool_name,
                    &input_summary,
                    &decision,
                    0,
                    ResultStatus::Error,
                )
                .await;

                return Err(ExecutorError::PermissionDenied(format!(
                    "denied by rule matching '{reason}'"
                )));
            }
            PermissionBehavior::Allow => {
                debug!(tool = input.tool_name.as_str(), "permission auto-allowed");
                true
            }
            PermissionBehavior::Ask => prompt_user_approval(tool.name(), &input_summary)?,
        };

        if !permitted {
            self.log_audit(
                &input.tool_name,
                &input_summary,
                &decision,
                0,
                ResultStatus::Error,
            )
            .await;
            return Err(ExecutorError::UserRejected);
        }

        // (d) Start timer.
        let start = Instant::now();

        // (e) Execute.
        info!(tool = input.tool_name.as_str(), "executing tool");
        let output = tool.execute(input).await;

        // (f) Stop timer.
        let duration_ms = start.elapsed().as_millis() as u64;

        // (g) Audit log.
        let status = if output.success {
            ResultStatus::Success
        } else {
            ResultStatus::Error
        };
        self.log_audit(&input.tool_name, &input_summary, &decision, duration_ms, status)
            .await;

        // (h) Return output.
        Ok(output)
    }

    /// Find a registered tool by name.
    fn find_tool(&self, name: &str) -> Result<&dyn Tool, ExecutorError> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(AsRef::as_ref)
            .ok_or_else(|| ExecutorError::UnknownTool(name.to_owned()))
    }

    /// Write an audit entry. Logs a warning on failure but does not
    /// propagate — a failed audit write should not block tool output
    /// from reaching the user.
    async fn log_audit(
        &self,
        tool_name: &str,
        input_summary: &str,
        decision: &crate::security::permissions::PermissionDecision,
        duration_ms: u64,
        result_status: ResultStatus,
    ) {
        let source = decision
            .matched_rule
            .as_ref()
            .map(|r| r.source.to_string())
            .unwrap_or_else(|| "default".into());

        let entry = AuditEntry {
            timestamp: Utc::now(),
            tool_name: tool_name.to_owned(),
            input_summary: audit::truncate_input(input_summary),
            permission_decision: decision.behavior.to_string(),
            decision_source: source,
            duration_ms,
            result_status,
        };

        if let Err(e) = self.audit.log(&entry).await {
            warn!(error = %e, "failed to write audit entry");
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Build a human-readable summary string from tool input params.
fn build_input_summary(input: &ToolInput) -> String {
    // For shell tools, the "command" param is the most meaningful.
    if let Some(cmd) = input.params.get("command").and_then(|v| v.as_str()) {
        return cmd.to_owned();
    }

    // Fallback: serialize params compactly.
    serde_json::to_string(&input.params).unwrap_or_else(|_| "<unserializable>".into())
}

/// Ask the user to approve a tool call via terminal y/n prompt.
fn prompt_user_approval(tool_name: &str, input_summary: &str) -> Result<bool, ExecutorError> {
    println!(
        "\n{}",
        "── Permission Required ──".yellow().bold()
    );
    println!(
        "  {} {}",
        "Tool:".dimmed(),
        tool_name.white().bold()
    );
    println!(
        "  {} {}",
        "Action:".dimmed(),
        input_summary
    );
    print!("{}", "  Allow? [y/N] ".yellow());
    io::stdout().flush().unwrap_or(());

    let mut response = String::new();
    io::stdin()
        .read_line(&mut response)
        .unwrap_or(0);

    let approved = matches!(response.trim().to_lowercase().as_str(), "y" | "yes");
    if approved {
        println!("  {}", "✓ Approved".green());
    } else {
        println!("  {}", "✗ Denied".red());
    }

    Ok(approved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_input_summary_extracts_command() {
        let mut params = HashMap::new();
        params.insert("command".into(), serde_json::json!("ls -la"));
        let input = ToolInput {
            tool_name: "shell".into(),
            params,
        };
        assert_eq!(build_input_summary(&input), "ls -la");
    }

    #[test]
    fn build_input_summary_fallback() {
        let mut params = HashMap::new();
        params.insert("path".into(), serde_json::json!("/tmp/foo"));
        let input = ToolInput {
            tool_name: "filesystem".into(),
            params,
        };
        let summary = build_input_summary(&input);
        assert!(summary.contains("/tmp/foo"));
    }
}
