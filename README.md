<p align="center">
  <img src="assets/claubi-logo.svg" alt="Claubi" width="120" />
</p>

<h1 align="center">Claubi</h1>

<p align="center">
  <strong>Your Personal AI Engineering Team — Local, Secure, Beautiful.</strong>
</p>

<p align="center">
  A security-hardened, local-first AI coding agent built in Rust.<br/>
  Inspired by Claude Code's architecture. Reimagined with security, aesthetics, and efficiency as first-class citizens.
</p>

<p align="center">
  <a href="#architecture">Architecture</a> ·
  <a href="#philosophy">Philosophy</a> ·
  <a href="#getting-started">Getting Started</a> ·
  <a href="#security-model">Security Model</a> ·
  <a href="#roadmap">Roadmap</a>
</p>

---

## Why Claubi Exists

AI coding agents are powerful. They're also reckless.

They generate code that works but isn't secure. They produce output that functions but looks like it was assembled by committee. They run commands on your machine with the confidence of someone who doesn't live with the consequences.

**Claubi takes a different position.** Every tool invocation passes through a permission system. Every piece of generated code runs through security analysis before it touches your project. Every output is held to an aesthetic standard — because "it works" was never the bar.

Built in Rust for performance and memory safety. Runs local models on Apple Silicon. Uses Claude API when the task demands it. Gives you a full engineering team that treats security and design as non-negotiable.

---

## Philosophy

### 🔒 Secure by Default

Security isn't a layer you bolt on after the fact. In Claubi, every generated artifact — code, config, infrastructure — passes through static analysis, dependency auditing, and secrets detection before it's written to disk. The agent doesn't just build things. It builds things that are safe to ship.

- OWASP Top 10 awareness baked into code generation prompts
- Automatic secrets scanning (API keys, tokens, credentials) in all output
- Dependency vulnerability checking before any package is added
- File operation sandboxing with explicit allowlists
- Full audit trail of every tool invocation and model decision

### 🎨 Beautiful by Design

AI-generated code has an aesthetic problem. Default templates, generic layouts, boilerplate everything. Claubi enforces design standards through opinionated output templates and review passes that catch the lazy patterns.

- Generated frontends follow real design systems, not Bootstrap defaults
- Documentation comes formatted and readable, not walls of text
- CLI output is clean, colored, and informative — not log spam
- Every project scaffold includes design tokens and style foundations

### ⚡ Efficient by Architecture

Rust isn't just fast — it's predictable. No garbage collection pauses. No runtime surprises. On an M4 Pro, Claubi coordinates multiple local models with minimal overhead, routing tasks to the right model for the job.

- Local model orchestration via Ollama (Qwen 2.5 Coder, Llama 3.1, Mistral)
- Claude API integration for complex architectural decisions
- Parallel task execution across model instances
- Streaming output — you see results as they're generated, not after

---

<h2 id="architecture">Architecture</h2>

Claubi's architecture draws from Claude Code's coordinator pattern but restructures it around three principles: every action is auditable, every output is reviewed, and every external call is sandboxed.

```
┌──────────────────────────────────────────────────────────┐
│                      CLAUBI CLI                          │
│                  (User Interface Layer)                   │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │  PLANNER    │  │  EXECUTOR    │  │   REVIEWER     │  │
│  │             │  │              │  │                │  │
│  │ Breaks down │  │ Routes tasks │  │ Security scan  │  │
│  │ ideas into  │──│ to the right │──│ Quality check  │  │
│  │ scoped tasks│  │ model/tool   │  │ Style review   │  │
│  └─────────────┘  └──────────────┘  └────────────────┘  │
│         │                │                   │           │
├─────────┼────────────────┼───────────────────┼───────────┤
│         ▼                ▼                   ▼           │
│  ┌────────────────────────────────────────────────────┐  │
│  │              SECURITY MIDDLEWARE                    │  │
│  │                                                    │  │
│  │  Permission Engine · Sandbox · Audit Log ·         │  │
│  │  Secrets Scanner · Dependency Checker              │  │
│  └────────────────────────────────────────────────────┘  │
│         │                │                   │           │
├─────────┼────────────────┼───────────────────┼───────────┤
│         ▼                ▼                   ▼           │
│  ┌──────────┐    ┌──────────────┐    ┌──────────────┐   │
│  │  LOCAL   │    │  CLAUDE API  │    │    TOOLS     │   │
│  │  MODELS  │    │  (Opus/      │    │              │   │
│  │          │    │   Sonnet)    │    │  File I/O    │   │
│  │  Ollama  │    │              │    │  Shell exec  │   │
│  │  Qwen    │    │  Architect   │    │  Git ops     │   │
│  │  Llama   │    │  decisions   │    │  Web fetch   │   │
│  │  Mistral │    │              │    │  DB queries  │   │
│  └──────────┘    └──────────────┘    └──────────────┘   │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

### Core Components

**Planner** — Takes a high-level idea or task description, breaks it into discrete, well-scoped work items. Uses Claude API for complex decomposition, local models for straightforward breakdowns. Every plan is presented for user approval before execution begins.

**Executor** — The orchestrator. Routes each task to the appropriate model based on complexity and type. Code generation goes to Qwen 2.5 Coder. Reasoning and analysis to Llama. Simple transformations to the fastest available local model. Claude API is reserved for architectural decisions and complex multi-file changes.

**Reviewer** — Nothing ships without review. The reviewer runs security analysis (SAST patterns, secrets detection, dependency checks), quality checks (linting, formatting, type safety), and aesthetic review (design system compliance, documentation standards). Failed reviews get sent back to the executor with specific feedback.

**Security Middleware** — Sits between every component and the outside world. File operations go through an allowlist. Shell commands require explicit approval for anything outside a safe set. Network calls are logged and auditable. Every model decision is written to an append-only audit log.

---

<h2 id="security-model">Security Model</h2>

### Threat Model

Claubi operates with the assumption that **AI-generated code is untrusted by default.** The security model is designed to catch:

| Threat | Mitigation |
|---|---|
| Hardcoded secrets in generated code | Pre-commit secrets scanner (regex + entropy analysis) |
| Vulnerable dependencies | Automated `cargo audit` / `npm audit` / `pip-audit` before any install |
| Command injection via shell tools | Allowlisted command set + argument sanitization |
| Path traversal in file operations | Sandboxed working directory with explicit allowlist |
| Prompt injection from untrusted input | Input sanitization layer between user content and model prompts |
| Insecure default configurations | Security-hardened templates for common frameworks |

### Permission Tiers

```
TIER 0 — ALWAYS ALLOWED (no prompt)
  Read files in project directory
  Run linters and formatters
  Generate code to memory (not yet written)

TIER 1 — AUTO-APPROVED WITH LOG
  Write files to project directory
  Run tests
  Git status / diff / log

TIER 2 — REQUIRES CONFIRMATION
  Install dependencies
  Run arbitrary shell commands
  Git commit / push
  Network requests

TIER 3 — ALWAYS BLOCKED
  Modify files outside project directory
  Access system credentials
  Execute as root
  Disable security checks
```

---

## Getting Started

### Prerequisites

- **macOS** on Apple Silicon (M1/M2/M3/M4) — optimized for Metal acceleration
- **Rust** toolchain (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Ollama** for local models (`brew install ollama`)
- **Anthropic API key** for Claude integration (optional but recommended)

### Install

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/claubi.git
cd claubi

# Build in release mode (optimized for M4 Pro)
cargo build --release

# Pull recommended local models
ollama pull qwen2.5-coder:7b
ollama pull llama3.1:8b
ollama pull mistral:7b

# Set up your environment
cp .env.example .env
# Edit .env and add your ANTHROPIC_API_KEY

# Run Claubi
./target/release/claubi
```

### First Run

```bash
# Start with something simple
claubi "Create a secure REST API in Rust with authentication and rate limiting"

# Claubi will:
# 1. Break this into tasks (plan phase)
# 2. Ask you to approve the plan
# 3. Generate code with security checks at every step
# 4. Run the reviewer before writing any files
# 5. Present the final output for your approval
```

---

<h2 id="roadmap">Roadmap</h2>

### Phase 1 — Foundation `[Current]`
- [ ] CLI scaffolding and argument parsing
- [ ] Ollama integration (model management, inference)
- [ ] Claude API client
- [ ] Basic tool system (file read/write, shell exec)
- [ ] Permission engine (tier-based)
- [ ] Audit logging

### Phase 2 — Intelligence
- [ ] Planner agent (task decomposition)
- [ ] Executor agent (model routing)
- [ ] Reviewer agent (security + quality)
- [ ] Multi-model orchestration
- [ ] Streaming output

### Phase 3 — Security Hardening
- [ ] Secrets scanner
- [ ] Dependency vulnerability checker
- [ ] SAST pattern matching for common vulnerabilities
- [ ] Sandboxed execution environment
- [ ] Prompt injection detection

### Phase 4 — Aesthetics Engine
- [ ] Design system templates (React, HTML, CLI)
- [ ] Output formatting standards
- [ ] Documentation generation with style
- [ ] Project scaffold templates (opinionated, beautiful defaults)

### Phase 5 — Memory & Learning
- [ ] Project context persistence between sessions
- [ ] Decision history and pattern learning
- [ ] User preference adaptation
- [ ] Cross-project knowledge transfer

---

## Project Structure

```
claubi/
├── src/
│   ├── main.rs              # Entry point
│   ├── cli/                  # CLI interface and argument parsing
│   ├── agents/
│   │   ├── planner.rs        # Task decomposition
│   │   ├── executor.rs       # Model routing and orchestration
│   │   └── reviewer.rs       # Security + quality + aesthetic review
│   ├── models/
│   │   ├── ollama.rs         # Local model integration
│   │   └── claude.rs         # Anthropic API client
│   ├── tools/
│   │   ├── filesystem.rs     # Sandboxed file operations
│   │   ├── shell.rs          # Command execution with permissions
│   │   ├── git.rs            # Git operations
│   │   └── web.rs            # Network requests
│   ├── security/
│   │   ├── permissions.rs    # Tier-based permission engine
│   │   ├── scanner.rs        # Secrets and vulnerability scanning
│   │   ├── sandbox.rs        # Execution sandboxing
│   │   └── audit.rs          # Append-only audit log
│   ├── design/
│   │   ├── templates.rs      # Opinionated project templates
│   │   └── review.rs         # Aesthetic quality checks
│   └── config/
│       └── mod.rs            # Configuration management
├── templates/                 # Design-system-aware scaffolds
├── security-rules/            # SAST patterns and scanning rules
├── tests/
├── CLAUDE.md                  # Instructions for Claude Code
├── Cargo.toml
├── .env.example
└── README.md
```

---

## The Name

**Claubi** — inspired by Claude, built to be your buddy. Not a fork. Not a clone. A reimagining of what an AI coding agent should be when security, design, and efficiency aren't afterthoughts.

---

## Legal

This project is an independent work. It does not contain, redistribute, or derive from any proprietary source code. Architectural patterns are studied from publicly available documentation, blog posts, and clean-room analysis. All code in this repository is original.

---

<p align="center">
  <sub>Built with Rust, paranoia, and good taste.</sub>
</p>
