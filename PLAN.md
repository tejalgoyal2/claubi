# Claubi — Project Plan

> Last updated: April 2, 2026
> Status: Phase 1 in progress

---

## Vision

Claubi is a local-first AI coding agent that gives you a personal engineering team in your terminal. You describe what you want. Claubi plans it, builds it, reviews it for security and quality, and delivers clean, beautiful, production-ready code.

It runs on your hardware. It uses any model you want — Gemma 4, Llama, Qwen, Mistral, Dolphin, or Claude API when you need the heavy reasoning. No vendor lock-in. No content filtering on local models. No "sorry, I can't help with that."

The security layer protects your codebase — not your intent. Every tool invocation is permission-checked and audit-logged. Every generated artifact is scanned for secrets, vulnerabilities, and insecure defaults before it touches your project.

---

## Architecture Overview

```
┌────────────────────────────────────────────────────────────────┐
│                         CLAUBI TUI                             │
│              Beautiful terminal interface (Ratatui)             │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐                │
│   │ PLANNER  │───▶│ EXECUTOR │───▶│ REVIEWER │                │
│   │          │    │          │    │          │                │
│   │ Breaks   │    │ Routes   │    │ Security │                │
│   │ ideas    │    │ tasks to │    │ Quality  │                │
│   │ into     │    │ best     │    │ Design   │                │
│   │ tasks    │    │ model    │    │ checks   │                │
│   └──────────┘    └──────────┘    └──────────┘                │
│        │               │               │                      │
│   ┌────▼───────────────▼───────────────▼─────┐                │
│   │         SECURITY MIDDLEWARE               │                │
│   │  Permissions · Sandbox · Audit · Scanner  │                │
│   └────┬───────────────┬───────────────┬─────┘                │
│        │               │               │                      │
│   ┌────▼────┐    ┌─────▼─────┐   ┌─────▼─────┐              │
│   │  LOCAL  │    │  CLAUDE   │   │   TOOLS   │              │
│   │ MODELS  │    │   API     │   │           │              │
│   │         │    │(optional) │   │ Files     │              │
│   │ Ollama  │    │           │   │ Shell     │              │
│   │ Any     │    │ Opus      │   │ Git       │              │
│   │ model   │    │ Sonnet    │   │ Web       │              │
│   └─────────┘    └───────────┘   └───────────┘              │
│                                                                │
├────────────────────────────────────────────────────────────────┤
│                     LOCAL MCP SERVER                           │
│         Long-term memory · Project history · Preferences       │
└────────────────────────────────────────────────────────────────┘
```

---

## Phase Plan

### Phase 1 — Foundation (Current)

**Goal:** Talk to a local model, execute one tool, with permission checks and audit logging. Prove the architecture works end-to-end.

| # | Component | File | Status |
|---|-----------|------|--------|
| 1 | Ollama client | `src/models/ollama.rs` | Done (uncompiled) |
| 2 | Tool trait | `src/tools/mod.rs` | Done (uncompiled) |
| 3 | Shell tool | `src/tools/shell.rs` | Not started |
| 4 | Permission engine | `src/security/permissions.rs` | Not started |
| 5 | Audit logger | `src/security/audit.rs` | Not started |
| 6 | Tool executor | `src/agents/executor.rs` | Not started |
| 7 | CLI REPL loop | `src/cli/mod.rs` + `src/main.rs` | Not started |

**Done when:** You can type a request in the terminal, Claubi sends it to Ollama, gets a response, the response includes a shell command, the permission engine evaluates it, you approve, it executes, and the audit log records everything.

---

### Phase 2 — Multi-Model & Full Tool Suite

**Goal:** Route tasks to the right model. Support all core tools (files, git, web). Optional Claude API for heavy reasoning.

| # | Component | Description |
|---|-----------|-------------|
| 1 | Model router | Priority chain: session → flag → env → config → default |
| 2 | Model registry | Add/remove models, define capabilities per model |
| 3 | Claude API client | Optional — for architectural decisions and complex reasoning |
| 4 | Filesystem tool | Read/write/create files with sandbox enforcement |
| 5 | Git tool | Status, diff, commit, branch, push |
| 6 | Web tool | Fetch URLs, search (with logging) |
| 7 | Context-aware routing | Code tasks → code model, reasoning → reasoning model, fast tasks → small model |

**Done when:** Claubi can pick the best available model for each task type, and you have a full set of tools for file manipulation, git operations, and web access — all through the permission engine.

---

### Phase 3 — Agents & Security Hardening

**Goal:** Planner and Reviewer agents that turn Claubi from a tool runner into an engineering team. Security scanning on all output.

| # | Component | Description |
|---|-----------|-------------|
| 1 | Planner agent | Takes high-level task, decomposes into scoped work items, presents plan for approval |
| 2 | Reviewer agent | Runs after every code generation: security scan, quality check, style review |
| 3 | Secrets scanner | Regex + entropy analysis on all generated code before writing to disk |
| 4 | Dependency checker | Audit dependencies before install (`cargo audit`, `npm audit`, `pip-audit`) |
| 5 | SAST patterns | Pattern matching for common vulnerability classes (injection, XSS, path traversal) |
| 6 | Hooks/middleware | Pre/post tool execution hooks (shell command hooks first) |
| 7 | Prompt injection detection | Sanitize untrusted input before it reaches model prompts |

**Done when:** You can say "build me a REST API with auth" and Claubi plans the work, generates code, reviews it for security issues, and flags problems before anything is written — all without you having to specify the security checks.

---

### Phase 4 — Beautiful TUI

**Goal:** A terminal interface that looks better than Claude Code. Clean, informative, memorable.

| # | Component | Description |
|---|-----------|-------------|
| 1 | Ratatui integration | Full terminal UI framework |
| 2 | Streaming output display | See model output as it generates, with syntax highlighting |
| 3 | Permission prompts | Clean, clear approve/deny UI for tool actions |
| 4 | Status bar | Current model, token usage, active tools, project info |
| 5 | Diff viewer | Side-by-side or inline diffs for file changes |
| 6 | Plan display | Visual task breakdown when planner proposes work |
| 7 | Theming | Color schemes, customizable look |

**Done when:** Using Claubi feels polished and intentional — you'd be proud to show it to someone.

---

### Phase 5 — Local MCP Server (Long-Term Memory)

**Goal:** Claubi remembers you, your projects, your preferences, and your patterns across sessions.

| # | Component | Description |
|---|-----------|-------------|
| 1 | MCP server scaffold | Local HTTP server implementing MCP protocol |
| 2 | User preferences store | Coding style, framework preferences, security rules |
| 3 | Project history | Past architecture decisions, patterns used, templates created |
| 4 | Session memory | What you worked on last, where you left off |
| 5 | Pattern learning | Recognize repeated decisions and offer them as defaults |
| 6 | Cross-project knowledge | "Build it like the API from project X" actually works |

**Done when:** Claubi knows your preferences without you restating them. It suggests approaches based on what worked before. Starting a new project feels like resuming a conversation, not starting from scratch.

---

### Phase 6 — Multi-Agent Engineering Team

**Goal:** Multiple specialized agents that discuss, debate, and iterate to produce better output than any single agent.

| # | Component | Description |
|---|-----------|-------------|
| 1 | Agent protocol | Standard message format for agent-to-agent communication |
| 2 | Architect agent | High-level design decisions, system structure |
| 3 | Coder agent | Implementation — writes the actual code |
| 4 | Reviewer agent (enhanced) | Security + quality + performance + style |
| 5 | Tester agent | Writes and runs tests, reports coverage |
| 6 | Orchestrator | Manages agent workflow, resolves disagreements, controls iteration count |
| 7 | Agent routing | Different models for different agents based on their role |

**Done when:** You say "build me a dashboard" and watch multiple agents plan the architecture, write the code, review it, write tests, find bugs, fix them, and deliver a finished product — with you approving key decisions along the way.

---

## Development Workflow

```
┌──────────────────┐     git push     ┌──────────────────┐
│   WORK LAPTOP    │ ───────────────▶ │     GITHUB       │
│   (Windows)      │                  │  tejalgoyal2/    │
│                  │                  │  claubi          │
│  Claude Code     │ ◀──── Issues ── │                  │
│  writes code     │                  │                  │
└──────────────────┘                  └────────┬─────────┘
                                               │ git pull
                                               ▼
                                     ┌──────────────────┐
                                     │   MACBOOK PRO    │
                                     │   M4 Pro         │
                                     │                  │
                                     │  cargo build     │
                                     │  cargo test      │
                                     │  Ollama models   │
                                     │  Runtime testing │
                                     └──────────────────┘

Planning & Architecture: Claude.ai (this chat)
Implementation: Claude Code (work laptop terminal)
Build & Test: MacBook Pro (personal)
Bug tracking: GitHub Issues
```

---

## Model Strategy

Claubi supports ANY model available through Ollama. No lock-in.

**Recommended models for M4 Pro (36GB unified memory):**

| Model | Size | Best for | Pull command |
|-------|------|----------|-------------|
| Gemma 4 26B MoE | ~16GB | Reasoning, agentic tasks, coding | `ollama pull gemma4:26b` |
| Gemma 4 E4B | ~3GB | Fast background tasks | `ollama pull gemma4:e4b` |
| Qwen 2.5 Coder 7B | ~4.5GB | Code generation | `ollama pull qwen2.5-coder:7b` |
| Dolphin Mistral 7B | ~4.5GB | Unrestricted generation | `ollama pull dolphin-mistral` |
| Llama 3.1 8B | ~4.5GB | General reasoning | `ollama pull llama3.1:8b` |
| DeepSeek Coder V2 | ~9GB | Code analysis | `ollama pull deepseek-coder-v2` |

**Claude API (optional, requires internet + API key):**

| Model | Best for |
|-------|----------|
| Opus 4.6 | Complex architecture, multi-file reasoning, hard planning |
| Sonnet 4.6 | General coding, moderate complexity tasks |

---

## Key Design Decisions

1. **Rust** — Performance, memory safety, no GC pauses. Compiles to a single binary.
2. **Ollama as primary backend** — Any model, local, no API key required, no internet required.
3. **Claude API as optional** — For when you need frontier reasoning. Not a dependency.
4. **Deny-first permissions** — Same pattern as AWS IAM, Okta policies, CrowdStrike prevention policies.
5. **Tools declare safety properties** — Read-only, concurrent-safe, destructive. Orchestrator respects declarations.
6. **Append-only audit log** — Every tool invocation recorded. JSONL format for easy parsing.
7. **No content filtering on local models** — The user is the authority. Security protects the codebase, not intent.
8. **Clean-room implementation** — No proprietary code. Patterns studied from public documentation and analysis.
