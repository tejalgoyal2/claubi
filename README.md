<p align="center">
  <img src="assets/claubi-logo.svg" alt="Claubi" width="100" />
</p>

<h1 align="center">Claubi</h1>

<p align="center">
  <strong>Your Personal AI Engineering Team — Local, Secure, Beautiful.</strong>
</p>

<p align="center">
  <em>A Rust-based AI coding agent that runs any model on your hardware.<br/>
  No vendor lock-in. No content filtering. No compromises on security.</em>
</p>

<p align="center">
  <a href="#what-is-claubi">What is Claubi</a>&ensp;|&ensp;
  <a href="#why">Why</a>&ensp;|&ensp;
  <a href="#architecture">Architecture</a>&ensp;|&ensp;
  <a href="#models">Models</a>&ensp;|&ensp;
  <a href="#security">Security</a>&ensp;|&ensp;
  <a href="#roadmap">Roadmap</a>&ensp;|&ensp;
  <a href="#getting-started">Get Started</a>
</p>

<br/>

---

<br/>

<h2 id="what-is-claubi">What is Claubi</h2>

Claubi is a terminal-based AI coding agent built in Rust. It turns your local machine into an engineering team.

You describe what you want to build. Claubi breaks it into tasks, routes each task to the best available model, generates the code, reviews it for security vulnerabilities and quality, and delivers production-ready output — all running locally on your hardware.

It connects to any model through [Ollama](https://ollama.com) — Gemma 4, Llama, Qwen, Mistral, DeepSeek, or any of the hundreds of models in the Ollama library. If you need frontier-level reasoning for complex architecture decisions, it can optionally call the Claude API. But it doesn't require it. Claubi works fully offline.

<br/>

<h2 id="why">Why</h2>

AI coding tools today have three problems.

**They're not secure.** They generate code that works but ships with hardcoded secrets, vulnerable dependencies, and insecure defaults. The security review happens after the code is written — if it happens at all.

**They're not yours.** They run on someone else's servers, under someone else's content policies. Ask a cloud AI to write a penetration testing script and it lectures you about responsible use. Ask it to generate code that interacts with security tools and it hedges. These are tools that don't trust the people using them.

**They're not beautiful.** The output works but looks like it was assembled by committee. Default templates, boilerplate scaffolds, no design intention.

Claubi takes a different position:

- **Security is structural.** Every generated artifact is scanned for secrets, vulnerabilities, and insecure defaults *before* it touches your project. Permissions are deny-first. Every tool invocation is audit-logged.
- **You are the authority.** Local models have no content filtering. No refusals. The security layer protects your codebase — it doesn't gatekeep what you're allowed to ask.
- **Aesthetics matter.** Generated code follows real design systems. Documentation is formatted and readable. The terminal interface itself is clean, informative, and intentional.

<br/>

<h2 id="architecture">Architecture</h2>

```
YOU
 |
 v
+-----------------------------------------------------+
|                    CLAUBI TUI                        |
|            Beautiful terminal interface               |
+-----------------------------------------------------+
|                                                     |
|  PLANNER --> EXECUTOR --> REVIEWER                  |
|  Breaks      Routes       Security scan             |
|  ideas       tasks to     Quality check             |
|  into work   best model   Design review             |
|                                                     |
+-----------------------------------------------------+
|              SECURITY MIDDLEWARE                      |
|  Deny-first permissions | Sandbox | Audit log        |
|  Secrets scanner | Dependency checker | SAST          |
+-----------------------------------------------------+
|                                                     |
|  LOCAL MODELS        CLAUDE API        TOOLS        |
|  (Ollama)            (optional)                     |
|                                        Files        |
|  Gemma 4             Opus 4.6          Shell        |
|  Llama 3.1           Sonnet 4.6        Git          |
|  Qwen 2.5                              Web          |
|  Mistral                                            |
|  DeepSeek                                           |
|  Dolphin                                            |
|  ...anything                                        |
|                                                     |
+-----------------------------------------------------+
|              LOCAL MCP SERVER                        |
|  Long-term memory | Project history | Preferences    |
+-----------------------------------------------------+
```

### How it works

1. **You give an instruction** — "Build a secure REST API with JWT auth and rate limiting"
2. **Planner** breaks it into discrete tasks, presents the plan for your approval
3. **Executor** routes each task to the best model — code generation to Qwen, reasoning to Gemma 4, simple tasks to the fastest available model
4. **Security middleware** checks every tool invocation against the permission engine. Shell commands get prefix analysis. File writes go through the sandbox. Everything is audit-logged.
5. **Reviewer** scans all generated code — secrets detection, dependency auditing, SAST patterns, quality checks, design system compliance
6. **You approve** the final output. Nothing ships without your sign-off.

<br/>

<h2 id="models">Models</h2>

Claubi runs any model available through Ollama. No lock-in. Pull what you need:

```bash
# Reasoning and agentic tasks
ollama pull gemma4:26b

# Fast code generation
ollama pull qwen2.5-coder:7b

# Unrestricted generation (no refusals)
ollama pull dolphin-mistral

# General reasoning
ollama pull llama3.1:8b

# Lightweight background tasks
ollama pull gemma4:e4b
```

The model router picks the right model for each task automatically. You can override per-session or per-task.

Claude API integration is optional — useful for complex multi-file reasoning and architectural decisions, but not required for any core functionality.

<br/>

<h2 id="security">Security Model</h2>

### Philosophy

AI-generated code is **untrusted by default**. The security layer exists to protect *your codebase*, not to restrict *your intent*.

### Permission Tiers

| Tier | Behavior | Examples |
|------|----------|---------|
| **Tier 0** | Always allowed, no prompt | Read files, run linters, generate code to memory |
| **Tier 1** | Auto-approved, logged | Write files to project dir, run tests, git status |
| **Tier 2** | Requires your confirmation | Install dependencies, run shell commands, git push |
| **Tier 3** | Always blocked | Modify files outside sandbox, access system creds, run as root |

### Deny-First Evaluation

```
Incoming tool request
  --> Check DENY rules (absolute block, nothing overrides)
  --> Check ALLOW rules (auto-approve)
  --> Check ASK rules (prompt user)
  --> If undecided, prompt user
```

This is the same pattern used by AWS IAM policies, Okta authorization rules, and CrowdStrike prevention policies. A deny is absolute.

### Automated Scanning

Every piece of generated code passes through:
- **Secrets scanner** — regex + entropy analysis for API keys, tokens, credentials
- **Dependency auditor** — checks for known vulnerabilities before any package install
- **SAST patterns** — detects common vulnerability classes (injection, XSS, path traversal, insecure deserialization)
- **Audit log** — append-only JSONL log of every tool invocation, permission decision, and outcome

<br/>

<h2 id="roadmap">Roadmap</h2>

| Phase | Focus | Status |
|-------|-------|--------|
| **Phase 1** | CLI + Ollama + Shell tool + Permissions + Audit | **In progress** |
| **Phase 2** | Multi-model routing + Full tool suite + Claude API | Planned |
| **Phase 3** | Planner and Reviewer agents + Security scanning | Planned |
| **Phase 4** | Beautiful TUI with Ratatui | Planned |
| **Phase 5** | Local MCP server (long-term memory) | Planned |
| **Phase 6** | Multi-agent engineering team | Planned |

See [PLAN.md](PLAN.md) for the detailed breakdown of every phase.

<br/>

<h2 id="getting-started">Getting Started</h2>

### Prerequisites

- macOS on Apple Silicon (M1-M4) or Linux
- [Rust](https://rustup.rs) toolchain
- [Ollama](https://ollama.com) for local models
- Anthropic API key (optional, for Claude integration)

### Build

```bash
git clone https://github.com/tejalgoyal2/claubi.git
cd claubi

# Build (release mode, optimized)
cargo build --release

# Pull at least one model
ollama pull gemma4:26b

# Configure (optional, for Claude API)
cp .env.example .env
# Edit .env with your ANTHROPIC_API_KEY

# Run
./target/release/claubi
```

<br/>

## Project Structure

```
claubi/
├── src/
│   ├── main.rs                # Entry point
│   ├── cli/                   # Terminal interface
│   ├── agents/                # Planner, Executor, Reviewer
│   ├── models/                # Ollama client, Claude API client
│   ├── tools/                 # File, Shell, Git, Web tools
│   ├── security/              # Permissions, Audit, Scanner, Sandbox
│   ├── design/                # Output templates, aesthetic review
│   └── config/                # Environment and settings
├── templates/                  # Project scaffolding templates
├── security-rules/             # SAST patterns and scanning rules
├── CLAUDE.md                   # Context file for Claude Code
├── PLAN.md                     # Detailed project roadmap
├── ARCHITECTURE_NOTES.md       # Patterns studied from reference architecture
└── Cargo.toml
```

<br/>

## Legal

This project is an independent, clean-room implementation. It does not contain, redistribute, or derive from any proprietary source code. Architectural patterns are studied from publicly available documentation, blog posts, and clean-room analysis. All code in this repository is original work, licensed under MIT.

<br/>

---

<p align="center">
  <sub>Built with Rust, paranoia, and good taste.</sub>
</p>
