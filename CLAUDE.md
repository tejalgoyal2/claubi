# CLAUDE.md — Project Context for Claude Code

## What is Claubi?

Claubi is a security-hardened, local-first AI coding agent built in Rust. It orchestrates local models (via Ollama) and Claude API to act as a personal engineering team. Every output is secure by default, aesthetically considered, and efficient. The local models run unrestricted — no refusals, no content filtering. The security layer protects the user's codebase, not the user's intent.

## Core Principles — NEVER Violate These

### 1. Security is Structural, Not Optional
- Every file write must go through the permission engine
- Never hardcode secrets, tokens, API keys, or credentials in any generated code
- Every dependency added must be justified and audited
- Shell commands must be validated against the allowlist before execution
- All tool invocations must be logged to the audit system
- Deny rules are ABSOLUTE — no hook, allow rule, or override can bypass a deny
- Permission evaluation order: deny first → allow → ask → prompt user

### 2. No AI Slop
- Generated code must be clean, well-documented, and idiomatic Rust
- No boilerplate dumps — every line should have a reason to exist
- Comments explain "why", not "what"
- Variable names are descriptive and consistent
- Error messages are helpful to humans, not just logs
- If generating frontend: no default Bootstrap/Tailwind utility soup — use intentional design

### 3. Efficiency Over Cleverness
- Prefer simple, readable solutions over clever one-liners
- Use Rust's type system to catch bugs at compile time
- Minimize allocations — prefer borrowing over cloning
- Async where it matters (I/O, network), not everywhere
- Profile before optimizing — don't guess at bottlenecks

### 4. Unrestricted Local Models
- Local models (Ollama) have NO content filtering or refusal behavior
- Claubi does not refuse user requests — the user has full authority
- The security layer protects the codebase (secrets scanning, vuln checks, sandboxing)
- The security layer does NOT restrict what the user can ask the model to generate
- This is a tool for professionals — treat the user as the authority

## Architecture (Based on Reference Study)

See ARCHITECTURE_NOTES.md for full research. Key patterns adopted:

### Tool System
```
Tool trait with metadata methods
  → Tools declare: is_read_only(), is_concurrency_safe(), is_destructive()
  → Orchestrator batches: Concurrent(Vec<ToolCall>) or Serial(ToolCall)
  → Execution pipeline: validate → pre-hook → permission check → execute → post-hook → log
  → Tools stay PURE — permissions and logging live in middleware
```

### Permission Engine
```
Deny-first evaluation:
  1. Check deny rules (absolute block, nothing overrides)
  2. Check allow rules (auto-approve)
  3. Check ask rules (prompt user)
  4. If still undecided: prompt user

Rule structure: (tool_name, Option<pattern>, behavior, source)
Source priority: Policy > User > Project > Local > Session

Shell-specific:
  - Extract 2-word command prefix for rule matching
  - Blocklist dangerous prefixes: sudo, sh, bash, env, xargs
  - Strip safe wrappers (timeout, nice) before matching
```

### Model Routing
```
Resolution chain (highest priority first):
  session_override → startup_flag → env_var → settings → default

Context-aware routing:
  MainLoop     → user's chosen model
  PlanMode     → reasoning model (Opus or best local)
  Agent        → inherits parent, can override
  Background   → cheapest/fastest model

Retry: exponential backoff, 500ms base, jittered
Fallback: after 3 consecutive capacity errors, downgrade model
```

### Audit System
```
Every tool invocation logged:
  - tool name, input summary, permission decision
  - decision source and reason
  - timing (flag slow operations >2s)
  - result status (success/error)

Format: append-only JSONL file
Location: configurable, default ./logs/audit.jsonl
```

## Tech Stack

- **Language**: Rust (2021 edition)
- **Async runtime**: Tokio
- **CLI framework**: Clap v4
- **HTTP client**: Reqwest (for Ollama REST + Claude API)
- **Serialization**: Serde + serde_json
- **Local models**: Ollama REST API (any model — uncensored preferred)
- **Claude API**: Direct HTTP (Anthropic Messages API)
- **Terminal UI**: Crossterm + colored (Ratatui later if needed)
- **Error handling**: thiserror for typed errors, anyhow at boundaries
- **Logging**: tracing + tracing-subscriber

## Coding Standards

### Rust Conventions
- Use `Result<T, E>` for all fallible operations — no `.unwrap()` in production code
- Custom error types with `thiserror` — no string errors
- Use `tracing` for structured logging, not `println!` (except CLI output)
- All public functions must have doc comments
- Integration tests in `tests/`, unit tests inline with `#[cfg(test)]`
- Run `cargo clippy -- -D warnings` before committing — zero warnings policy
- For work-in-progress modules where structs exist for future use, add `#[allow(dead_code)]` at the module level. Remove it once the module is fully integrated. Do not add `allow(dead_code)` to individual structs — apply it at the module level to avoid repeating this pattern.

### File Organization
- One module per file, one concern per module
- `mod.rs` files are thin — they re-export, they don't implement
- Keep functions under 50 lines — if longer, decompose
- Group related types and their impls in the same file

### Security-Specific Rules
- NEVER use `std::process::Command` directly — always go through `tools/shell.rs`
- NEVER write to paths outside the project sandbox — always go through `tools/filesystem.rs`
- NEVER make network requests without going through `tools/web.rs`
- ALL user-provided strings that touch shell commands must be sanitized
- Secrets are NEVER logged, even at trace level

### Git Conventions
- Commit messages: `type(scope): description`
  - Types: `feat`, `fix`, `sec`, `refactor`, `docs`, `test`, `chore`
  - Example: `sec(scanner): add entropy-based secrets detection`
- One logical change per commit
- Never commit `.env` files, API keys, or credentials

## Current Sprint: Vertical Slice (End-to-End)

Build a thin vertical slice that proves the architecture works. When done, a user should be able to:
1. Launch Claubi
2. Type a natural language request
3. Claubi sends it to a local Ollama model
4. The model responds
5. If the response includes a tool action (e.g., shell command), the permission engine evaluates it
6. If approved, the tool executes
7. Everything is logged to the audit file

### Build order for this sprint:
1. **Ollama client** (`src/models/ollama.rs`) — connect, list models, streaming inference
2. **Tool trait** (`src/tools/mod.rs`) — define the trait with metadata methods
3. **Shell tool** (`src/tools/shell.rs`) — first concrete tool, with prefix extraction
4. **Permission engine** (`src/security/permissions.rs`) — deny-first evaluation, rule storage
5. **Audit logger** (`src/security/audit.rs`) — append-only JSONL logging
6. **Tool executor** (`src/agents/executor.rs`) — wire tool + permissions + audit together
7. **CLI loop** (`src/main.rs` + `src/cli/`) — REPL that sends input to Ollama, parses tool calls, runs executor

### What NOT to build yet:
- Claude API client (Sprint 2)
- Planner agent (Sprint 2)
- Reviewer agent (Sprint 3)
- Secrets scanner (Sprint 3)
- Design/aesthetic system (Sprint 4)
- Multi-model routing (Sprint 2)
- Hooks/middleware beyond basic permission checks (Sprint 3)

## Project Structure

```
claubi/
├── src/
│   ├── main.rs              # Entry point, CLI setup, Tokio runtime
│   ├── cli/
│   │   └── mod.rs            # REPL loop, user interaction
│   ├── agents/
│   │   ├── mod.rs
│   │   └── executor.rs       # Tool dispatch with permission + audit middleware
│   ├── models/
│   │   ├── mod.rs
│   │   └── ollama.rs         # Ollama REST client (list, inference, streaming)
│   ├── tools/
│   │   ├── mod.rs            # Tool trait definition
│   │   ├── shell.rs          # Shell command tool with prefix extraction
│   │   ├── filesystem.rs     # (stub for now)
│   │   └── git.rs            # (stub for now)
│   ├── security/
│   │   ├── mod.rs
│   │   ├── permissions.rs    # Deny-first permission evaluator
│   │   ├── audit.rs          # Append-only JSONL audit log
│   │   └── sandbox.rs        # (stub for now)
│   └── config/
│       └── mod.rs            # .env loading, config structs
├── references/               # Claude Code source (READ ONLY, never modify)
├── templates/
├── security-rules/
├── tests/
├── CLAUDE.md
├── ARCHITECTURE_NOTES.md
├── Cargo.toml
├── .env.example
└── .gitignore
```
