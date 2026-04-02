# Architecture Notes — Patterns from Claude Code Reference

> Study of `references/src/` to identify patterns adaptable to Claubi's Rust architecture.
> This is a research document, not a spec. Nothing here is committed to implementation yet.

---

## 1. Tool Orchestration

### How It Works in the Reference

Tools follow a **define → register → dispatch → execute → collect** lifecycle:

1. **Definition**: Each tool is a structured object with: `call()`, `inputSchema` (Zod), `description()`, `checkPermissions()`, `validateInput()`, plus metadata like `isConcurrencySafe()`, `isReadOnly()`, `isDestructive()`.

2. **Registry**: A central `getAllBaseTools()` function returns all tools, conditionally including some based on feature flags and environment. A `getTools(permissionContext)` function filters the registry by deny rules and mode (e.g., "simple mode" only exposes Bash/Read/Edit).

3. **Dispatch & Batching**: When the model returns tool_use blocks, they're partitioned into batches:
   - **Concurrent batch**: consecutive read-only, concurrency-safe tools run in parallel (up to ~10)
   - **Serial batch**: a single non-read-only tool runs alone
   - This is the key insight — *tools declare their own concurrency safety*, and the orchestrator respects it.

4. **Execution flow** (`checkPermissionsAndCallTool`):
   ```
   Validate input (schema) → Validate input (tool-specific)
     → Backfill observable input (clone for hooks/permissions)
     → Run pre-tool hooks (can allow/deny/modify input/halt)
     → Resolve permission decision (hooks + rules + interactive)
     → Execute tool.call()
     → Map result to API format
     → Run post-tool hooks (can modify output)
     → Process result (truncate if oversized, persist to disk if huge)
   ```

5. **Context modifiers**: Tools can return a `contextModifier` callback. For concurrent batches, modifiers are queued and applied in order after the batch completes. For serial tools, applied immediately.

6. **Streaming progress**: Tools emit progress via callback during execution, allowing real-time UI updates.

### Patterns to Adapt for Claubi

- **Tool trait with metadata methods**: In Rust, define a `Tool` trait with `fn is_concurrency_safe(&self, input: &Input) -> bool`, `fn is_read_only(&self) -> bool`, `fn is_destructive(&self) -> bool`. The orchestrator can batch based on these.
- **Input backfilling**: Clone input before passing to hooks/permissions so the actual tool execution sees the original. Prevents mutation bugs.
- **Batch partitioning**: Simple enum — `Batch::Concurrent(Vec<ToolCall>)` or `Batch::Serial(ToolCall)`. Iterate tool calls, group consecutive concurrency-safe ones.
- **Result size limits**: Each tool declares `max_result_size_chars`. If exceeded, persist to disk and give the model a file reference instead. Prevents context blowup.
- **Pre/post hooks as middleware**: Don't bake permission logic into tools. Keep it in a middleware layer so tools stay pure.

---

## 2. Model Selection & Routing

### How It Works in the Reference

Model selection follows a **priority hierarchy**:

```
Session override (/model command)
  → Startup override (--model flag)
  → ANTHROPIC_MODEL env var
  → User settings
  → Subscription-tier default
```

Key routing contexts:

| Context | Model | Notes |
|---------|-------|-------|
| Main conversation | User's chosen model | Resolved via hierarchy above |
| Plan mode | Special handling | Haiku upgrades to Sonnet; "opusplan" alias uses Opus only in plan mode |
| Agents/subagents | `inherit` by default | Can be overridden by agent frontmatter `model:` field |
| Background tasks | Haiku (small/fast) | Memory extraction, summaries, token estimation |
| Fallback on 529 | Downgrade model | After 3 consecutive overloaded errors, switch to fallback |

Retry logic:
- Max 10 retries with exponential backoff (500ms base, jittered)
- Foreground queries retry aggressively; background tasks bail immediately on 529
- "Fast mode" retries with fast flag on short retry-after; enters cooldown on long delays

API structuring:
- Messages normalized before sending (orphan tool_use/tool_result pairs repaired)
- Media items capped at 100 per request
- Tools filtered based on model capability
- System prompt is model-aware and supports caching

### Patterns to Adapt for Claubi

- **Model resolver as a chain**: Implement as a function that checks each source in priority order. In Rust, this is a simple cascade of `Option` checks: `session_override.or(startup_override).or(env_var).or(settings).unwrap_or(default)`.
- **Context-aware routing enum**:
  ```rust
  enum ModelContext {
      MainLoop,
      PlanMode,
      Agent { model_override: Option<ModelId> },
      BackgroundTask, // always cheap/fast model
  }
  ```
- **Separate local vs API routing**: Claubi uses both Ollama and Claude API. Route based on task complexity — local for simple completions, API for multi-file reasoning. This maps to the reference's "background task = haiku" pattern.
- **Retry with backoff**: Use `tokio::time::sleep` with exponential backoff. Key: differentiate foreground (retry aggressively) from background (fail fast).
- **Fallback on capacity errors**: Track consecutive failures per model. After threshold, downgrade. Reset counter on success.

---

## 3. Permission System

### How It Works in the Reference

The permission system is a multi-layered defense:

#### Permission Modes
Five user-facing modes: `default`, `acceptEdits`, `bypassPermissions`, `dontAsk`, `plan`
Plus internal modes: `auto` (ML classifier), `bubble` (escalate to parent)

#### Permission Rules
Rules are stored in settings JSON and organized by:
- **Behavior**: `allow`, `deny`, `ask`
- **Source** (priority order): `policySettings` > `userSettings` > `projectSettings` > `localSettings` > `flagSettings` > `cliArg` > `session`
- **Rule value**: `toolName` + optional `ruleContent` pattern (e.g., `Bash(npm test:*)` allows any npm test subcommand)

#### Decision Flow
```
1. Check forced decision (from hook or test)
2. Check rule-based permissions (settings deny/allow/ask rules)
   - Deny rules checked first (hard block)
   - Allow rules checked next (auto-approve)
   - Ask rules trigger prompt
3. If auto mode: run ML classifier
4. If still undecided: prompt user interactively
5. Log decision with reason, source, and timing
```

#### Hook Integration
Hooks can intercept tool execution at two points:
- **Pre-tool hooks**: Can `allow`, `deny`, modify input, halt execution, add context
- **Post-tool hooks**: Can modify output, halt, add context
- Critical invariant: **Hook `allow` does NOT bypass deny/ask rules from settings**. Hooks can only grant permission up to what rules allow — they can't override a deny.

#### Bash-Specific Security
- Command prefix extraction for rule suggestions (2-word prefix)
- Dangerous prefix blocklist (sh, bash, sudo, env, xargs — things that exec arbitrary code)
- Safe wrapper stripping (timeout, nice, stdbuf removed before matching)
- Environment variable whitelist (only known-safe vars stripped before permission matching)
- Compound command analysis capped at 50 subcommands to prevent exponential blowup
- AST-level parsing (tree-sitter) for semantic safety checks

#### Auto Mode Safeguards
- Certain tools are always allowed without classifier (Read, Grep, Glob — read-only ops)
- Dangerous rules detected and stripped: tool-level Bash allow, interpreter prefix rules (python:*, node:*)
- Agent allow rules blocked (would bypass classifier for subagents)
- Killswitch: can disable bypassPermissions remotely per org

#### Enterprise/Policy Controls
- `allowManagedPermissionRulesOnly`: only policy-level rules apply, user rules ignored
- "Always allow" UI hidden when managed-only is active
- Policy settings are highest priority source

### Patterns to Adapt for Claubi

- **Tiered permission enum**:
  ```rust
  enum PermissionBehavior { Allow, Deny, Ask }
  enum PermissionSource { Policy, User, Project, Local, Session }
  ```
  Sources have natural ordering — Policy > User > Project > Local > Session.

- **Rule matching with patterns**: Store rules as `(tool_name, Option<pattern>)`. Match tool name first, then pattern against input. For shell commands, extract a 2-word prefix for rule suggestions.

- **Read-only tools skip permission checks**: Tag tools with `is_read_only()`. If true, auto-allow in all modes except `plan`.

- **Hooks as middleware, not overrides**: Hooks can suggest `allow` but the rule engine has final say. This prevents hooks from being an escalation vector.

- **Command prefix analysis for shell tools**: Before running any shell command through `tools/shell.rs`, extract the command prefix. Match against allowlist. Never auto-allow interpreters (python, node, ruby) or privilege escalation (sudo).

- **Deny-first evaluation**: Always check deny rules before allow rules. A deny is absolute.

- **Sandbox enforcement is separate from permissions**: Permissions decide *if* a tool runs. Sandbox decides *where* it runs. Keep these orthogonal.

- **Consecutive denial tracking**: If a tool gets denied N times in a row, consider surfacing a "would you like to add a rule?" prompt rather than repeating the same dialog.

---

## 4. Cross-Cutting Patterns

### Observability / Audit
- Every permission decision logged with: tool name, decision, source, reason, timing
- Slow phases (>2s) and slow hooks (>500ms) trigger warnings
- OTel events for: tool errors, permission grants, permission denials, progress updates, cancellations
- **For Claubi**: Map this to the append-only audit log in `security/audit.rs`. Every tool invocation gets a log entry with the permission decision.

### Error Handling
- Tool errors classified by type (fs errors → errno code, custom errors → telemetry-safe message)
- Tool results marked `is_error: true` on failure — the model sees the error and can adapt
- Post-tool-failure hooks can request retry
- **For Claubi**: Use `thiserror` for tool errors. Return `Result<ToolOutput, ToolError>` from every tool. On error, format a human-readable message for the model.

### Context Window Management
- Tool results can be truncated or persisted to disk if they exceed size limits
- Messages normalized before API calls (orphaned tool pairs repaired)
- Media items capped per request
- **For Claubi**: Implement a `max_result_size` per tool. If exceeded, write to a temp file and give the model a summary + file path.

### Extensibility via Hooks
- Four hook types: shell command, prompt (AI-assisted), agent (nested AI), HTTP (remote endpoint)
- Hooks have timeouts (10 min for tool hooks, 1.5s for session-end hooks)
- Hook results validated with Zod schemas
- **For Claubi**: Start with shell command hooks only. Define a `HookResult` struct with optional fields for permission override, input modification, and halt signal.

---

## Summary: What to Build First

Based on this study, the foundation priorities from CLAUDE.md align well. The reference architecture suggests this build order for the permission/tool layer:

1. **Tool trait** with metadata methods (read-only, concurrency-safe, destructive)
2. **Permission rule storage** (allow/deny/ask by source, loaded from config)
3. **Permission evaluator** (deny-first, then allow, then ask — with mode awareness)
4. **Tool executor** with pre/post middleware slots (even if hooks are empty initially)
5. **Audit logging** of every permission decision and tool invocation
6. **Shell command prefix extraction** for the Bash/shell tool specifically
7. **Model resolver** with priority chain
8. **Retry/fallback** for API calls

This gives us the security-first foundation before we build the planner/executor/reviewer agents.
