//! Security subsystem — permissions, audit logging, and sandboxing.

// Temporary: suppressed until the executor wires up actual calls.
#![allow(dead_code)]

pub mod audit;
pub mod permissions;

#[allow(unused_imports)]
pub use audit::{AuditEntry, AuditLogger, ResultStatus};
#[allow(unused_imports)]
pub use permissions::{
    PermissionBehavior, PermissionDecision, PermissionEngine, PermissionRule, PermissionSource,
};

/// Errors from the audit subsystem.
#[derive(Debug, thiserror::Error)]
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
