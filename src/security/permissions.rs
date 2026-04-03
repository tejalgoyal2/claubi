//! Deny-first permission evaluator.
//!
//! Evaluates whether a tool invocation should be allowed, denied, or
//! require user confirmation. Deny rules are absolute — nothing overrides
//! them, not hooks, not allow rules, not session overrides.

use std::fmt;

use serde::{Deserialize, Serialize};
use tracing::debug;

// ── Core types ──────────────────────────────────────────────────────────

/// Outcome of a permission check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
}

impl fmt::Display for PermissionBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::Deny => write!(f, "deny"),
            Self::Ask => write!(f, "ask"),
        }
    }
}

/// Where a permission rule originates. Higher-priority sources are checked
/// first within each behavior tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionSource {
    /// Organization-wide policy — highest priority.
    Policy = 0,
    /// User-level settings (~/.claubi/settings.json).
    User = 1,
    /// Project-level settings (.claubi/settings.json).
    Project = 2,
    /// Machine-local settings (.claubi/settings.local.json).
    Local = 3,
    /// Temporary rules that last only for the current session.
    Session = 4,
}

impl fmt::Display for PermissionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy => write!(f, "policy"),
            Self::User => write!(f, "user"),
            Self::Project => write!(f, "project"),
            Self::Local => write!(f, "local"),
            Self::Session => write!(f, "session"),
        }
    }
}

/// A single permission rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Tool name this rule applies to (e.g., "shell").
    pub tool_name: String,
    /// Optional pattern to match against the tool's input.
    /// For shell tools, this is typically a command prefix like "npm test".
    /// `None` means the rule matches any input for that tool.
    pub pattern: Option<String>,
    /// What to do when the rule matches.
    pub behavior: PermissionBehavior,
    /// Where this rule came from.
    pub source: PermissionSource,
}

/// Result of a permission evaluation, including which rule triggered it.
#[derive(Debug, Clone)]
pub struct PermissionDecision {
    pub behavior: PermissionBehavior,
    /// The rule that produced this decision, if any.
    pub matched_rule: Option<PermissionRule>,
}

impl PermissionDecision {
    /// Shorthand for an Ask decision with no matched rule (the default).
    fn ask() -> Self {
        Self {
            behavior: PermissionBehavior::Ask,
            matched_rule: None,
        }
    }
}

// ── Rule store ──────────────────────────────────────────────────────────

/// Holds all active permission rules, organized by behavior for fast lookup.
pub struct PermissionEngine {
    rules: Vec<PermissionRule>,
}

impl PermissionEngine {
    /// Create an engine with the given rules.
    pub fn new(rules: Vec<PermissionRule>) -> Self {
        Self { rules }
    }

    /// Create an engine with sensible defaults for Sprint 1.
    ///
    /// Blocks dangerous shell prefixes and allows common read-only operations.
    pub fn with_defaults() -> Self {
        let mut rules = Vec::new();

        // Deny rules for dangerous shell prefixes — absolute blocks.
        for prefix in &[
            "sudo", "su", "sh", "bash", "zsh", "env", "xargs", "eval", "exec",
        ] {
            rules.push(PermissionRule {
                tool_name: "shell".into(),
                pattern: Some((*prefix).into()),
                behavior: PermissionBehavior::Deny,
                source: PermissionSource::Policy,
            });
        }

        // Allow rules for common safe operations.
        for prefix in &[
            "ls", "cat", "head", "tail", "wc", "grep", "find", "echo",
            "pwd", "which", "whoami", "date", "git status", "git log",
            "git diff", "cargo check", "cargo build", "cargo test",
            "cargo clippy", "npm test", "npm run",
        ] {
            rules.push(PermissionRule {
                tool_name: "shell".into(),
                pattern: Some((*prefix).into()),
                behavior: PermissionBehavior::Allow,
                source: PermissionSource::Project,
            });
        }

        Self { rules }
    }

    /// Add a rule at runtime (e.g., from user session input).
    pub fn add_rule(&mut self, rule: PermissionRule) {
        self.rules.push(rule);
    }

    /// Evaluate whether the given tool invocation is allowed.
    ///
    /// Evaluation order (deny-first):
    /// 1. Check deny rules — if any match, return Deny immediately.
    /// 2. Check allow rules — if any match, return Allow.
    /// 3. Check ask rules — if any match, return Ask.
    /// 4. If nothing matched, return Ask (safe default).
    pub fn evaluate(&self, tool_name: &str, input: &str) -> PermissionDecision {
        // Sort matching rules by source priority (lower ordinal = higher priority).
        let matching: Vec<&PermissionRule> = self
            .rules
            .iter()
            .filter(|r| rule_matches(r, tool_name, input))
            .collect();

        // Phase 1: deny (absolute, checked first)
        if let Some(rule) = find_by_behavior(&matching, PermissionBehavior::Deny) {
            debug!(tool = tool_name, rule = ?rule.pattern, "permission denied by rule");
            return PermissionDecision {
                behavior: PermissionBehavior::Deny,
                matched_rule: Some(rule.clone()),
            };
        }

        // Phase 2: allow
        if let Some(rule) = find_by_behavior(&matching, PermissionBehavior::Allow) {
            debug!(tool = tool_name, rule = ?rule.pattern, "permission allowed by rule");
            return PermissionDecision {
                behavior: PermissionBehavior::Allow,
                matched_rule: Some(rule.clone()),
            };
        }

        // Phase 3: ask
        if let Some(rule) = find_by_behavior(&matching, PermissionBehavior::Ask) {
            debug!(tool = tool_name, rule = ?rule.pattern, "permission requires user confirmation");
            return PermissionDecision {
                behavior: PermissionBehavior::Ask,
                matched_rule: Some(rule.clone()),
            };
        }

        // Phase 4: no rule matched — default to Ask.
        debug!(tool = tool_name, "no permission rule matched, defaulting to ask");
        PermissionDecision::ask()
    }
}

// ── Rule matching ───────────────────────────────────────────────────────

/// Check if a rule matches the given tool name and input.
fn rule_matches(rule: &PermissionRule, tool_name: &str, input: &str) -> bool {
    if rule.tool_name != tool_name {
        return false;
    }

    match &rule.pattern {
        None => true, // No pattern means "match everything for this tool"
        Some(pattern) => input_matches_pattern(input, pattern),
    }
}

/// Check if the input string matches a pattern.
///
/// Matching is prefix-based: the pattern "npm test" matches any input
/// that starts with "npm test" (e.g., "npm test -- --coverage").
fn input_matches_pattern(input: &str, pattern: &str) -> bool {
    let input_trimmed = input.trim();
    let pattern_trimmed = pattern.trim();

    // Exact match
    if input_trimmed == pattern_trimmed {
        return true;
    }

    // Prefix match: input starts with pattern followed by whitespace or end
    if let Some(rest) = input_trimmed.strip_prefix(pattern_trimmed) {
        return rest.is_empty() || rest.starts_with(' ');
    }

    false
}

/// Find the highest-priority rule with the given behavior.
fn find_by_behavior<'a>(
    rules: &[&'a PermissionRule],
    behavior: PermissionBehavior,
) -> Option<&'a PermissionRule> {
    rules
        .iter()
        .filter(|r| r.behavior == behavior)
        // Lower PermissionSource ordinal = higher priority
        .min_by_key(|r| r.source)
        .copied()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rule(
        tool: &str,
        pattern: Option<&str>,
        behavior: PermissionBehavior,
        source: PermissionSource,
    ) -> PermissionRule {
        PermissionRule {
            tool_name: tool.into(),
            pattern: pattern.map(Into::into),
            behavior,
            source,
        }
    }

    #[test]
    fn deny_overrides_allow() {
        let engine = PermissionEngine::new(vec![
            make_rule("shell", Some("sudo"), PermissionBehavior::Deny, PermissionSource::Policy),
            make_rule("shell", Some("sudo"), PermissionBehavior::Allow, PermissionSource::Session),
        ]);
        let decision = engine.evaluate("shell", "sudo rm -rf /");
        assert_eq!(decision.behavior, PermissionBehavior::Deny);
    }

    #[test]
    fn allow_rule_matches_prefix() {
        let engine = PermissionEngine::new(vec![
            make_rule("shell", Some("npm test"), PermissionBehavior::Allow, PermissionSource::Project),
        ]);
        let decision = engine.evaluate("shell", "npm test -- --coverage");
        assert_eq!(decision.behavior, PermissionBehavior::Allow);
    }

    #[test]
    fn no_match_defaults_to_ask() {
        let engine = PermissionEngine::new(vec![
            make_rule("shell", Some("npm test"), PermissionBehavior::Allow, PermissionSource::Project),
        ]);
        let decision = engine.evaluate("shell", "rm -rf /tmp");
        assert_eq!(decision.behavior, PermissionBehavior::Ask);
        assert!(decision.matched_rule.is_none());
    }

    #[test]
    fn wildcard_rule_matches_all_input() {
        let engine = PermissionEngine::new(vec![
            make_rule("filesystem", None, PermissionBehavior::Ask, PermissionSource::User),
        ]);
        let decision = engine.evaluate("filesystem", "/any/path/here");
        assert_eq!(decision.behavior, PermissionBehavior::Ask);
        assert!(decision.matched_rule.is_some());
    }

    #[test]
    fn wrong_tool_does_not_match() {
        let engine = PermissionEngine::new(vec![
            make_rule("shell", Some("ls"), PermissionBehavior::Allow, PermissionSource::User),
        ]);
        let decision = engine.evaluate("filesystem", "ls");
        assert_eq!(decision.behavior, PermissionBehavior::Ask);
    }

    #[test]
    fn higher_priority_source_wins_within_behavior() {
        let engine = PermissionEngine::new(vec![
            make_rule("shell", Some("git"), PermissionBehavior::Deny, PermissionSource::Session),
            make_rule("shell", Some("git"), PermissionBehavior::Deny, PermissionSource::Policy),
        ]);
        let decision = engine.evaluate("shell", "git push --force");
        assert_eq!(decision.behavior, PermissionBehavior::Deny);
        // Policy (ordinal 0) should win over Session (ordinal 4)
        assert_eq!(
            decision.matched_rule.as_ref().map(|r| r.source),
            Some(PermissionSource::Policy)
        );
    }

    #[test]
    fn prefix_match_respects_word_boundary() {
        // "ls" should not match "lsof"
        assert!(!input_matches_pattern("lsof", "ls"));
        // "ls" should match "ls -la"
        assert!(input_matches_pattern("ls -la", "ls"));
        // Exact match
        assert!(input_matches_pattern("ls", "ls"));
    }

    #[test]
    fn defaults_block_dangerous_prefixes() {
        let engine = PermissionEngine::with_defaults();
        for prefix in &["sudo", "su", "sh", "bash", "zsh", "env", "xargs", "eval", "exec"] {
            let decision = engine.evaluate("shell", &format!("{prefix} something"));
            assert_eq!(
                decision.behavior,
                PermissionBehavior::Deny,
                "expected deny for '{prefix}'"
            );
        }
    }

    #[test]
    fn defaults_allow_safe_commands() {
        let engine = PermissionEngine::with_defaults();
        for cmd in &["ls -la", "git status", "cargo build", "npm test"] {
            let decision = engine.evaluate("shell", cmd);
            assert_eq!(
                decision.behavior,
                PermissionBehavior::Allow,
                "expected allow for '{cmd}'"
            );
        }
    }
}
