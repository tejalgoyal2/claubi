//! Security subsystem — permissions, audit logging, and sandboxing.

pub mod audit;
pub mod permissions;

#[allow(unused_imports)] // Re-exports for external consumers.
pub use audit::{AuditEntry, AuditLogger, ResultStatus};
#[allow(unused_imports)] // Re-exports for external consumers.
pub use permissions::{
    PermissionBehavior, PermissionDecision, PermissionEngine, PermissionRule, PermissionSource,
};

/// Errors from the audit subsystem.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)] // Some variants only used by not-yet-called methods.
pub enum AuditError {
    #[error("failed to initialize audit log at {path}: {source}")]
    Init {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write audit entry to {path}: {source}")]
    Write {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("failed to serialize audit entry: {0}")]
    Serialize(serde_json::Error),
}
