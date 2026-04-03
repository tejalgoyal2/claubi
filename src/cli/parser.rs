//! Extracts shell commands from model response text.
//!
//! Detects commands inside fenced code blocks with shell language tags
//! and single-line code blocks that start with known command prefixes.

/// Command prefixes that identify a code block as a shell command
/// even without a language tag.
const COMMAND_PREFIXES: &[&str] = &[
    "ls", "cd", "grep", "cat", "mkdir", "rm", "git", "cargo", "npm",
    "pip", "curl", "echo", "find", "sed", "awk", "chmod", "cp", "mv",
    "touch", "docker",
];

/// Shell language tags that mark a fenced code block as executable.
const SHELL_TAGS: &[&str] = &["bash", "sh", "shell", "zsh"];

/// Extract shell commands from a model response containing markdown.
///
/// Detection strategy:
/// 1. Fenced code blocks with a shell/bash language tag — extract all
///    non-empty lines as a single command (joined with ` && `).
/// 2. Single-line fenced code blocks (no tag or generic tag) where the
///    content starts with a known command prefix.
pub fn extract_commands(response: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut lines = response.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // Look for fenced code block opening: ```lang or ```
        if let Some(tag) = parse_fence_open(trimmed) {
            let block = collect_block(&mut lines);
            if block.is_empty() {
                continue;
            }

            if is_shell_tag(&tag) {
                // Tagged shell block — every non-empty line is a command.
                add_block_commands(&block, &mut commands);
            } else if tag.is_empty() {
                // Untagged block — check if lines look like shell commands.
                add_prefix_matched_commands(&block, &mut commands);
            }
        }
    }

    commands
}

/// Parse a fenced code block opening line. Returns `Some(tag)` if it's
/// a fence (tag may be empty), `None` if not a fence.
fn parse_fence_open(line: &str) -> Option<String> {
    if !line.starts_with("```") {
        return None;
    }
    let tag = line.trim_start_matches('`').trim().to_lowercase();
    Some(tag)
}

/// Collect lines until the closing fence. Consumes the closing ``` line.
fn collect_block<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut block = Vec::new();
    for line in lines.by_ref() {
        if line.trim().starts_with("```") {
            break;
        }
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            block.push(trimmed.to_owned());
        }
    }
    block
}

/// Check if a language tag indicates a shell block.
fn is_shell_tag(tag: &str) -> bool {
    SHELL_TAGS.iter().any(|&t| tag == t)
}

/// Add each line from a shell-tagged block as a separate command.
fn add_block_commands(block: &[String], commands: &mut Vec<String>) {
    for line in block {
        // Strip leading $ or # prompts that models sometimes include.
        let cleaned = line
            .strip_prefix("$ ")
            .or_else(|| line.strip_prefix("# "))
            .unwrap_or(line);
        let cleaned = cleaned.trim();
        if !cleaned.is_empty() && !commands.contains(&cleaned.to_owned()) {
            commands.push(cleaned.to_owned());
        }
    }
}

/// Add lines from an untagged block that start with a known command prefix.
fn add_prefix_matched_commands(block: &[String], commands: &mut Vec<String>) {
    for line in block {
        let cleaned = line
            .strip_prefix("$ ")
            .or_else(|| line.strip_prefix("# "))
            .unwrap_or(line)
            .trim();

        let first_word = cleaned.split_whitespace().next().unwrap_or("");
        if COMMAND_PREFIXES.iter().any(|&p| first_word == p) && !commands.contains(&cleaned.to_owned()) {
            commands.push(cleaned.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_from_bash_block() {
        let response = "Try this:\n```bash\ngrep -rn \"TODO\" src/\ncargo test\n```\n";
        let cmds = extract_commands(response);
        assert_eq!(cmds, vec!["grep -rn \"TODO\" src/", "cargo test"]);
    }

    #[test]
    fn extracts_from_sh_block() {
        let response = "Run:\n```sh\nls -la\n```\n";
        let cmds = extract_commands(response);
        assert_eq!(cmds, vec!["ls -la"]);
    }

    #[test]
    fn extracts_from_shell_block() {
        let response = "```shell\necho hello\n```";
        let cmds = extract_commands(response);
        assert_eq!(cmds, vec!["echo hello"]);
    }

    #[test]
    fn extracts_from_untagged_block_with_known_prefix() {
        let response = "Run this:\n```\ncargo build --release\n```\n";
        let cmds = extract_commands(response);
        assert_eq!(cmds, vec!["cargo build --release"]);
    }

    #[test]
    fn ignores_untagged_block_without_known_prefix() {
        let response = "Here's the config:\n```\nfoo = bar\nbaz = 42\n```\n";
        let cmds = extract_commands(response);
        assert!(cmds.is_empty());
    }

    #[test]
    fn ignores_non_shell_tagged_block() {
        let response = "```rust\nfn main() {}\n```\n";
        let cmds = extract_commands(response);
        assert!(cmds.is_empty());
    }

    #[test]
    fn strips_dollar_prompt() {
        let response = "```bash\n$ git status\n$ git diff\n```\n";
        let cmds = extract_commands(response);
        assert_eq!(cmds, vec!["git status", "git diff"]);
    }

    #[test]
    fn deduplicates_commands() {
        let response = "```bash\ncargo test\n```\n\n```bash\ncargo test\n```\n";
        let cmds = extract_commands(response);
        assert_eq!(cmds, vec!["cargo test"]);
    }

    #[test]
    fn handles_empty_blocks() {
        let response = "```bash\n```\n";
        let cmds = extract_commands(response);
        assert!(cmds.is_empty());
    }

    #[test]
    fn handles_multiple_blocks() {
        let response = "\
First:\n```bash\ngrep TODO src/\n```\n\
Then:\n```sh\ncargo test\n```\n\
And check:\n```\ngit status\n```\n";
        let cmds = extract_commands(response);
        assert_eq!(cmds, vec!["grep TODO src/", "cargo test", "git status"]);
    }
}
