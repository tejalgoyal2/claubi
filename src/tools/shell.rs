//! Shell command tool — executes validated shell commands in a child process.
//!
//! All shell execution in Claubi goes through this module. Direct use of
//! `std::process::Command` or `tokio::process::Command` elsewhere is forbidden.

use async_trait::async_trait;
use tokio::process::Command;
use tracing::{debug, warn};

use super::{Tool, ToolInput, ToolOutput};

/// Prefixes that are never allowed because they can execute arbitrary code
/// or escalate privileges.
const DANGEROUS_PREFIXES: &[&str] = &[
    "sudo", "su", "sh", "bash", "zsh", "env", "xargs", "eval", "exec",
];

/// Maximum time (seconds) a shell command is allowed to run.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Shell command tool.
pub struct ShellTool {
    timeout_secs: u64,
}

impl ShellTool {
    /// Create a new shell tool with the default timeout.
    pub fn new() -> Self {
        Self {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Create a shell tool with a custom timeout.
    #[allow(dead_code)] // Used when config specifies CLAUBI_MAX_SHELL_TIMEOUT_SECS.
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }
}

// ── Prefix extraction ───────────────────────────────────────────────────

/// Extract a 2-word command prefix from a shell command string.
///
/// Skips leading whitespace and environment variable assignments
/// (e.g., `FOO=bar`). Returns the first two "words" joined by a space,
/// or fewer if the command has fewer words.
///
/// # Examples
/// ```
/// # use claubi::tools::shell::extract_command_prefix;
/// assert_eq!(extract_command_prefix("npm test -- --coverage"), "npm test");
/// assert_eq!(extract_command_prefix("FOO=1 cargo build"), "cargo build");
/// assert_eq!(extract_command_prefix("ls"), "ls");
/// assert_eq!(extract_command_prefix(""), "");
/// ```
pub fn extract_command_prefix(command: &str) -> String {
    let words: Vec<&str> = command
        .split_whitespace()
        // Skip env var assignments like KEY=value
        .skip_while(|w| w.contains('=') && !w.starts_with('='))
        .take(2)
        .collect();

    words.join(" ")
}

/// Check whether a command starts with a dangerous prefix.
fn is_dangerous_command(command: &str) -> Option<&'static str> {
    let first_word = command
        .split_whitespace()
        // Skip env var assignments
        .find(|w| !w.contains('=') || w.starts_with('='))?;

    DANGEROUS_PREFIXES
        .iter()
        .find(|&&prefix| first_word == prefix)
        .copied()
}

// ── Tool implementation ─────────────────────────────────────────────────

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return stdout/stderr"
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_destructive(&self) -> bool {
        // Shell commands can do anything — treat as potentially destructive.
        true
    }

    fn validate_input(&self, input: &ToolInput) -> Result<(), String> {
        match input.params.get("command") {
            Some(serde_json::Value::String(cmd)) if !cmd.trim().is_empty() => Ok(()),
            Some(serde_json::Value::String(_)) => {
                Err("command cannot be empty".into())
            }
            _ => Err("missing required parameter: command (string)".into()),
        }
    }

    async fn execute(&self, input: &ToolInput) -> ToolOutput {
        let command = match input.params.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => return ToolOutput::err("missing required parameter: command"),
        };

        // Block dangerous prefixes before anything touches a process.
        if let Some(prefix) = is_dangerous_command(command) {
            warn!(prefix, command, "blocked dangerous shell command");
            return ToolOutput::err(format!(
                "command blocked: '{prefix}' is a dangerous prefix and cannot be executed"
            ));
        }

        debug!(command, "executing shell command");

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_secs),
            run_command(command),
        )
        .await;

        match result {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => ToolOutput::err(format!("failed to execute command: {e}")),
            Err(_) => ToolOutput::err(format!(
                "command timed out after {}s",
                self.timeout_secs
            )),
        }
    }
}

/// Spawn the command and capture its output.
async fn run_command(command: &str) -> Result<ToolOutput, std::io::Error> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let content = if stderr.is_empty() {
        stdout.into_owned()
    } else if stdout.is_empty() {
        format!("[stderr]\n{stderr}")
    } else {
        format!("{stdout}\n[stderr]\n{stderr}")
    };

    if output.status.success() {
        Ok(ToolOutput::ok(content))
    } else {
        let code = output.status.code().unwrap_or(-1);
        Ok(ToolOutput::err(format!(
            "exit code {code}\n{content}"
        )))
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_extraction_basic() {
        assert_eq!(extract_command_prefix("npm test -- --coverage"), "npm test");
        assert_eq!(extract_command_prefix("cargo build"), "cargo build");
        assert_eq!(extract_command_prefix("ls"), "ls");
        assert_eq!(extract_command_prefix(""), "");
    }

    #[test]
    fn prefix_extraction_skips_env_vars() {
        assert_eq!(extract_command_prefix("FOO=1 cargo build"), "cargo build");
        assert_eq!(
            extract_command_prefix("A=1 B=2 npm install"),
            "npm install"
        );
    }

    #[test]
    fn prefix_extraction_handles_whitespace() {
        assert_eq!(extract_command_prefix("  git   status  "), "git status");
    }

    #[test]
    fn dangerous_prefix_detected() {
        assert_eq!(is_dangerous_command("sudo rm -rf /"), Some("sudo"));
        assert_eq!(is_dangerous_command("bash -c 'echo hi'"), Some("bash"));
        assert_eq!(is_dangerous_command("env FOO=1 cmd"), Some("env"));
        assert_eq!(is_dangerous_command("xargs rm"), Some("xargs"));
    }

    #[test]
    fn safe_commands_pass() {
        assert_eq!(is_dangerous_command("npm test"), None);
        assert_eq!(is_dangerous_command("cargo build"), None);
        assert_eq!(is_dangerous_command("git status"), None);
        assert_eq!(is_dangerous_command("ls -la"), None);
    }

    #[test]
    fn dangerous_check_skips_env_assignments() {
        // "FOO=bar sudo" — the env var is skipped, "sudo" is the real first word
        assert_eq!(is_dangerous_command("FOO=bar sudo rm -rf /"), Some("sudo"));
    }

    #[test]
    fn validate_input_requires_command() {
        let tool = ShellTool::new();
        let input = ToolInput {
            tool_name: "shell".into(),
            params: std::collections::HashMap::new(),
        };
        assert!(tool.validate_input(&input).is_err());
    }

    #[test]
    fn validate_input_rejects_empty_command() {
        let tool = ShellTool::new();
        let mut params = std::collections::HashMap::new();
        params.insert(
            "command".into(),
            serde_json::Value::String("  ".into()),
        );
        let input = ToolInput {
            tool_name: "shell".into(),
            params,
        };
        assert!(tool.validate_input(&input).is_err());
    }

    #[test]
    fn validate_input_accepts_valid_command() {
        let tool = ShellTool::new();
        let mut params = std::collections::HashMap::new();
        params.insert(
            "command".into(),
            serde_json::Value::String("ls -la".into()),
        );
        let input = ToolInput {
            tool_name: "shell".into(),
            params,
        };
        assert!(tool.validate_input(&input).is_ok());
    }
}
