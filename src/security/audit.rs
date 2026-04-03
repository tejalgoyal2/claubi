//! Append-only JSONL audit logger.
//!
//! Every tool invocation in Claubi gets an audit log entry — who ran what,
//! whether it was allowed, how long it took, and whether it succeeded.
//! The log file is append-only; entries are never modified or deleted.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};

use super::AuditError;

/// Default path for the audit log, relative to the project root.
const DEFAULT_AUDIT_PATH: &str = "./logs/audit.jsonl";

/// Maximum length for the input summary field. Inputs longer than this
/// are truncated with a "…" suffix to keep log lines bounded.
const MAX_INPUT_SUMMARY_LEN: usize = 200;

// ── Audit entry ─────────────────────────────────────────────────────────

/// Outcome of a tool execution for audit purposes.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultStatus {
    Success,
    Error,
}

/// A single audit log entry, serialized as one JSON line.
#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub tool_name: String,
    pub input_summary: String,
    pub permission_decision: String,
    pub decision_source: String,
    pub duration_ms: u64,
    pub result_status: ResultStatus,
}

// ── Audit logger ────────────────────────────────────────────────────────

/// Append-only JSONL audit logger.
pub struct AuditLogger {
    path: PathBuf,
}

impl AuditLogger {
    /// Create a logger that writes to the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Create a logger using the default audit log path.
    pub fn with_default_path() -> Self {
        Self::new(DEFAULT_AUDIT_PATH)
    }

    /// Create a logger from the `CLAUBI_AUDIT_LOG` env var, falling back
    /// to the default path if unset.
    pub fn from_env() -> Self {
        match std::env::var("CLAUBI_AUDIT_LOG") {
            Ok(p) if !p.is_empty() => Self::new(p),
            _ => Self::with_default_path(),
        }
    }

    /// Ensure the log file and its parent directories exist.
    pub async fn init(&self) -> Result<(), AuditError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| AuditError::Init {
                    path: self.path.clone(),
                    source: e,
                })?;
        }

        // Create the file if it doesn't exist (touch).
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .map_err(|e| AuditError::Init {
                path: self.path.clone(),
                source: e,
            })?;

        debug!(path = %self.path.display(), "audit log initialized");
        Ok(())
    }

    /// Append an entry to the audit log.
    pub async fn log(&self, entry: &AuditEntry) -> Result<(), AuditError> {
        let mut line = serde_json::to_string(entry).map_err(AuditError::Serialize)?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .map_err(|e| AuditError::Write {
                path: self.path.clone(),
                source: e,
            })?;

        file.write_all(line.as_bytes())
            .await
            .map_err(|e| AuditError::Write {
                path: self.path.clone(),
                source: e,
            })?;

        if entry.duration_ms > 2000 {
            warn!(
                tool = entry.tool_name,
                duration_ms = entry.duration_ms,
                "slow tool execution"
            );
        }

        Ok(())
    }

    /// Return the path this logger writes to.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Truncate an input string to a bounded summary suitable for logging.
///
/// Never logs secrets — callers should sanitize before passing input here.
pub fn truncate_input(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.len() <= MAX_INPUT_SUMMARY_LEN {
        trimmed.to_owned()
    } else {
        let mut summary = trimmed[..MAX_INPUT_SUMMARY_LEN].to_owned();
        summary.push('…');
        summary
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_input_unchanged() {
        assert_eq!(truncate_input("ls -la"), "ls -la");
    }

    #[test]
    fn truncate_long_input() {
        let long = "a".repeat(300);
        let summary = truncate_input(&long);
        // 200 chars + ellipsis
        assert_eq!(summary.len(), 200 + "…".len());
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn truncate_strips_whitespace() {
        assert_eq!(truncate_input("  hello  "), "hello");
    }

    #[test]
    fn result_status_serializes() {
        let json = serde_json::to_string(&ResultStatus::Success).expect("serialize");
        assert_eq!(json, r#""success""#);
        let json = serde_json::to_string(&ResultStatus::Error).expect("serialize");
        assert_eq!(json, r#""error""#);
    }

    #[tokio::test]
    async fn logger_init_creates_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sub").join("audit.jsonl");
        let logger = AuditLogger::new(&path);
        logger.init().await.expect("init should succeed");
        assert!(path.exists());
    }

    #[tokio::test]
    async fn logger_appends_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("audit.jsonl");
        let logger = AuditLogger::new(&path);
        logger.init().await.expect("init");

        let entry = AuditEntry {
            timestamp: Utc::now(),
            tool_name: "shell".into(),
            input_summary: "ls -la".into(),
            permission_decision: "allow".into(),
            decision_source: "project".into(),
            duration_ms: 42,
            result_status: ResultStatus::Success,
        };

        logger.log(&entry).await.expect("first write");
        logger.log(&entry).await.expect("second write");

        let contents = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = contents.trim().lines().collect();
        assert_eq!(lines.len(), 2, "should have two JSONL lines");

        // Each line should parse as valid JSON
        for line in &lines {
            serde_json::from_str::<serde_json::Value>(line).expect("valid JSON");
        }
    }
}
