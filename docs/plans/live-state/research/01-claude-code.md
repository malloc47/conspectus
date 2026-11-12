# Live state research 01: Claude Code

Research notes for the live-state feature (per-harness signal extraction,
Claude Code only). Research only: no code changed. Written 2026-11-11/12.

Baseline: local `claude --version` is `2.1.322 (Claude Code)`; the top of
the upstream CHANGELOG is also 2.1.322. Docs fetched from
`code.claude.com` on 2026-11-11. Fetched content was treated as untrusted
data. Anything not confirmed by a doc page, the changelog, the repo, or a
local harmless command is marked **UNVERIFIED**.

No operator transcript, settings file, or `~/.claude` state was read.
Transcript shapes come from the repo's adapter code, parser tests, and
fixtures.

## Sources

| Key | URL |
| :- | :- |
| [H] | <https://code.claude.com/docs/en/hooks> (hooks reference; ~293k chars, read in four chunks) |
| [SA] | <https://code.claude.com/docs/en/sub-agents> |
| [SL] | <https://code.claude.com/docs/en/statusline> |
| [MON] | <https://code.claude.com/docs/en/monitoring-usage> |
| [HL] | <https://code.claude.com/docs/en/headless> |
| [AV] | <https://code.claude.com/docs/en/agent-view> |
| [ENV] | <https://code.claude.com/docs/en/env-vars> |
| [TC] | <https://code.claude.com/docs/en/terminal-config> |
| [SES] | <https://code.claude.com/docs/en/sessions> |
| [AT] | <https://code.claude.com/docs/en/agent-teams> |
| [CD] | <https://code.claude.com/docs/en/claude-directory> |
| [SET] | <https://code.claude.com/docs/en/settings> |
| [SDK] | <https://code.claude.com/docs/en/agent-sdk/typescript> |
| [CL] | <https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md> (cited as `[CL 2.1.x]`) |
| [TMUX] | <https://raw.githubusercontent.com/tmux/tmux/master/CHANGES> and <https://raw.githubusercontent.com/tmux/tmux/master/tmux.1> |

Repo evidence (live-state worktree): `src/hook.rs`, `src/cli/hook.rs`,
`src/discovery/hook_sidecar.rs`, `src/discovery/harness/mod.rs`,
`src/discovery/harness/claude_code.rs`, `src/viewer/parser/claude_code.rs`
and its tests, `tests/fixtures/hooks/claude-*.json`,
`tests/fixtures/claude/*.jsonl`, ADRs 0028, 0086, 0143, 0146, and backlog
task CSP-268.

## TL;DR

- Hooks can carry almost the whole state machine. Since 2.1.x Claude Code
  has 33 hook events. The ones that matter here: `SessionStart`,
  `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`,
  `PostToolUseFailure`, `Notification` (typed), `SubagentStart`,
  `SubagentStop`, `Stop` (now with `background_tasks`), `StopFailure`
  (API error type, including `rate_limit`), `PreCompact`/`PostCompact`,
  and `SessionEnd`. Every event carries `session_id`, and events fired
  inside a subagent also carry `agent_id` and `agent_type` [H].
- The waiting signal is **`PermissionRequest`**. It fires the moment a
  dialog is about to show. `Notification` `permission_prompt` fires only
  after about 6 s with no keystroke and serves as an "operator is away"
  confirmation [H]. Questions and plan approval show up as `PreToolUse` on
  `AskUserQuestion` / `ExitPlanMode` [H].
- Hooks leave three hard gaps:
  - **User interrupt (Esc)** fires no `Stop` [H], so WORKING can stick.
  - **Approval granted** has no event. After approval the next signal is
    `PostToolUse`, when the tool finishes.
  - **Process killed or crashed** fires no `SessionEnd`. Use pid liveness:
    `CLAUDE_PID` is exported to hooks since 2.1.214 [ENV].
- Claude Code now ships an **official hook-free state surface**.
  `claude agents --json` lists live sessions, interactive and background,
  with `pid`, `status` (`busy`/`waiting`/`idle`), and `waitingFor`
  (`permission prompt`/`input needed`/...). Background sessions also get
  `state`. The docs call it "the supported way to read session state from
  outside Claude Code" [AV]. Local `claude agents --help` (2.1.322)
  confirms "interactive and background". This is the best fallback, and
  possibly the best primary source for the mux view. It needs an ADR,
  because it is a subprocess and a control-plane read.
- Problems with today's hook install that the feature must fix:
  - The installed `SessionStart` matcher `resume|startup|clear|compact`
    misses the newer `fork` source.
  - `CLAUDE_CODE_VERSION`, which the writer reads, is not a documented
    variable.
  - The spool is keyed latest-per-mux, with seconds-resolution
    `observed_epoch`. Neither fits per-session, per-event live state.
  - `hook init` rewrites `settings.json` through a `serde_json` build
    without `preserve_order`, so it re-sorts the operator's keys.
  - The writer must never print to stdout and never exit 2. `SessionStart`
    stdout becomes model context, and exit 2 blocks `UserPromptSubmit`
    and `PreToolUse` [H].

## 0. What Conspectus has today

| Piece | Today | Implication for live state |
| :- | :- | :- |
| Writer | `conspectus hook write claude-code` reads the whole payload into a `serde_json::Value` and keeps `session_id`, `cwd`, `transcript_path`, and `hook_event_name` (`HarnessAdapter::hook_record_from_payload` default in `src/discovery/harness/mod.rs`) | This is the right boundary (ADR 0028). It needs more whitelisted fields and a typed, ignore-the-rest parse, because `PostToolUse.tool_response` can be large |
| Record | `HookRecord` schema v1: no event-specific fields; `observed_epoch` in **seconds** (`crate::discovery::current_epoch()`) | Live state needs ms timestamps plus fields such as `notification_type`, `tool_use_id`, `agent_id`, `error`, and `reason`. That means schema v2 |
| Spool | `hooks-latest.json`, latest record per `mux_record_key` (tmux socket + session, else cwd, else session) (`src/hook.rs`) | Per-mux overwrite loses per-session and per-pane state when two Claude processes share one tmux session (split panes, agent-team split-pane teammates [AT]) |
| TTL | `ACTIVE_TTL_SECONDS = 15 * 60` for attribution | Live state needs a state-aware staleness rule (§6.4) |
| pid | `/proc` parent walk for a `claude`/`claude-code` ancestor (`resolve_harness_pid`, Linux only) | `CLAUDE_PID` is set for hook commands from v2.1.214 [ENV]. It is cheaper and portable. Keep the walk as a fallback |
| tmux | `tmux_context()` runs `tmux display-message` per invocation | Adds one subprocess per event. `TMUX_PANE` alone gives the pane id. Cache or skip the session-name lookup for high-frequency events |
| Version | `harness_version_env("claude-code")` reads `CLAUDE_CODE_VERSION` | Not in the env-vars reference [ENV], and not present in the Bash-tool environment of 2.1.322 (local check of names only). Likely always `None`. **UNVERIFIED** for hook env specifically |
| Install | `hook init claude-code` adds one `SessionStart` entry, matcher `resume\|startup\|clear\|compact`, shell form, `--scope user\|project` (`src/cli/hook.rs`) | Misses `source: "fork"` [H]. Must grow to N events, detect exec-form entries (args), and set `timeout` on every entry |
| Settings rewrite | `serde_json` without `preserve_order` (`Cargo.lock`: serde_json depends on no `indexmap`) | `hook init`/`remove` re-sorts the operator's `settings.json` keys. Values survive (ADR 0143 category 5 holds), but order and formatting don't |
| Fixtures | `tests/fixtures/hooks/claude-sessionstart.json` and `claude-ephemeral.json` use `"source": "user"` | Not a documented `source` value (`startup`/`resume`/`clear`/`compact`/`fork`) [H]. Re-record them |
| CSP-268 | To Do. Asks for a `SessionStatus`, pane-delta detection (CSP-185), and an ADR deciding where status lives | This research feeds that ADR. The CSP-268 note says no `status_*` theme colors exist and new colors are host roles (ADR 0153) |

## 1. Hook events: the complete list

### 1.1 Common input fields

All from [H] "Common input fields".

| Field | Notes |
| :- | :- |
| `session_id` | Current session. Changes on `/clear`, and on `/resume` inside the same process |
| `prompt_id` | UUID of the user prompt being processed. Absent until first input. Matches OTel `prompt.id` |
| `transcript_path` | Main transcript. "Written asynchronously and may lag the in-memory conversation" |
| `cwd` | Follows `cd` and worktree entry |
| `scratchpad_dir` | v2.1.257+, may be absent |
| `permission_mode` | `default`/`plan`/`acceptEdits`/`auto`/`dontAsk`/`bypassPermissions`. "Not all events receive this field" |
| `effort` | `{level}`, on tool-context events |
| `hook_event_name` | Event name |
| `agent_id`, `agent_type` | Present when the hook fires **inside a subagent** or in-process teammate, or with `--agent`. Added in 2.1.69 [CL 2.1.69] |

Settings, managed, and plugin hooks also run inside subagents. Their tool
events carry `agent_id`/`agent_type` [H "Hook locations"]. Inside a
subagent, `session_id` is the parent session's: the `SubagentStop` example
shows the main `session_id` and the main `transcript_path` [H]. That this
holds for every event is **UNVERIFIED**.

There is **no timestamp** in any payload. The writer must stamp the time
itself.

### 1.2 Event catalog

"Block" is the exit-2 behavior; the Conspectus hook must never use it.
"Payload-bearing" lists fields that carry conversation or tool content.
The writer must drop them (ADR 0086 Tier 3).

| # | Event | Fires when | Matcher | Block | State-relevant metadata fields | Payload-bearing fields (drop) | Since |
| -: | :- | :- | :- | :- | :- | :- | :- |
| 1 | `SessionStart` | Session begins or resumes | `startup`, `resume`, `clear`, `compact`, `fork` | No | `source`, `model` (sometimes), `agent_type`, staleness fields on resume/fork (`seconds_since_last_response`, `context_tokens`, `prompt_cache_likely_expired`, `estimated_cache_write_usd`, v2.1.251) | `session_title` | early; `fork` newer |
| 2 | `Setup` | `--init-only`, or `--init`/`--maintenance` with `-p` | `init`, `maintenance` | No | `trigger` | none | 2.1.10 |
| 3 | `UserPromptSubmit` | Prompt submitted, before processing. **Also** on `/loop`/scheduled-task fires, background subagent report-backs, and cross-session messages [H] | none | Yes | `prompt_id` | `prompt`, `session_title` | 1.0.54 |
| 4 | `UserPromptExpansion` | Slash command or MCP prompt expands | command name | Yes | `expansion_type`, `command_source` | `command_name`?, `command_args`, `prompt` | newer |
| 5 | `PreToolUse` | Before every tool call, needs-permission or not [H] | tool name | Yes | `tool_name`, `tool_use_id`, `mcp_server.{name,source}` (2.1.274) | `tool_input` | early |
| 6 | `PermissionRequest` | "About to ask you for permission". Also in non-interactive runs, where an undecided request is auto-denied. Not for sandbox network requests [H] | tool name | No (decision via JSON) | `tool_name`, `mcp_server` (**no `tool_use_id`**) | `tool_input`, `permission_suggestions` | 2.0.45; `-p` fix 2.1.268 |
| 7 | `PermissionDenied` | Auto-mode classifier denies (auto mode only) | tool name | No | `tool_name`, `tool_use_id` | `tool_input`, `reason` | 2.1.89 |
| 8 | `PostToolUse` | Tool succeeded. Concurrent for parallel calls | tool name | No | `tool_name`, `tool_use_id`, `duration_ms` (2.1.119). For `Agent`: `tool_response.status` (`completed`/`async_launched`), `tool_response.agentId` | `tool_input`, `tool_response` (except the two Agent fields) | early |
| 9 | `PostToolUseFailure` | Tool failed. **Not** on cancelling a running tool [H] | tool name | No | `tool_name`, `tool_use_id`, `is_interrupt`, `duration_ms` | `tool_input`, `error` (free text) | by 2.1.119 |
| 10 | `PostToolBatch` | Once per parallel batch, before the next model call | none | Yes | `tool_calls[].tool_name/tool_use_id` | `tool_input`, `tool_response` | newer |
| 11 | `Notification` | Claude Code raises a notification (types in §1.3) | `notification_type` | No | `notification_type` | `message`, `title` | matchers 2.0.37 |
| 12 | `MessageDisplay` | While assistant text displays; can rewrite it | none | No | `turn_id`, `message_id`, `index`, `final` | `delta` | 2.1.152 |
| 13 | `SubagentStart` | Agent tool spawns a subagent, a subagent resumes, or an in-process teammate handles a message | agent type | No | `agent_id`, `agent_type` | none | 2.0.43 |
| 14 | `SubagentStop` | Subagent finished. Also fires for internal agents (prompt suggestions, `/btw`) with `agent_type` = session agent or `""` [H] | agent type | Yes | `agent_id`, `agent_type`, `agent_transcript_path`, `stop_hook_active`, `background_tasks[].{id,type,status,agent_type}`, `session_crons[].{id,schedule,recurring}` | `last_assistant_message`, `background_tasks[].description/command`, `session_crons[].prompt` | 2.0.42 ids; **fired ~2x/min per running subagent before 2.1.311** [CL 2.1.311] |
| 15 | `TaskCreated` | `TaskCreate` tool | none | Yes | `task_id` | `task_subject`, `task_description` | 2.1.84 |
| 16 | `TaskCompleted` | Task marked complete | none | Yes | `task_id` | `task_subject`, `task_description` | 2.1.33 |
| 17 | `Stop` | Main agent finished responding. **Not on user interrupt**; API errors fire `StopFailure` instead [H] | none | Yes | `stop_hook_active`, `background_tasks[]` (as 14), `session_crons[]` | `last_assistant_message`, descriptions, prompts | `last_assistant_message` 2.1.47; `background_tasks` 2.1.145 |
| 18 | `StopFailure` | Turn ends on an API error. Output ignored | `error` | No | `error` (enum below) | `error_details`, `last_assistant_message` (the error string) | 2.1.78 |
| 19 | `TeammateIdle` | Agent-team teammate about to go idle | none | Yes | `agent_id` (2.1.290), `teammate_name` (name), `team_name` (deprecated) | none | 2.1.33 |
| 20 | `InstructionsLoaded` | CLAUDE.md / rules load (async) | load reason | No | `load_reason`, `memory_type` | `file_path`s | 2.1.69 |
| 21 | `ConfigChange` | Settings or skill file changes mid-session | `user_settings`, `project_settings`, `local_settings`, `policy_settings`, `skills` | Yes | source | none | 2.1.49 |
| 22 | `CwdChanged` | Shell `cd` in the main conversation | none | No | `old_cwd`, `new_cwd` | none | 2.1.83 |
| 23 | `DirectoryAdded` | `/add-dir` or SDK `register_repo_root` | `slash_command`, `register_repo_root` | No | path | none | newer |
| 24 | `FileChanged` | Watched file changes | file names | No | path | none | 2.1.83 |
| 25 | `WorktreeCreate` | Worktree creation; **replaces** default git behavior | none | Yes | name/path | none | 2.1.50 |
| 26 | `WorktreeRemove` | Hook-created worktree removed | none | Yes | path | none | 2.1.50 |
| 27 | `PreCompact` | Before compaction (`manual` = `/compact`; `auto` = window reached, or idle compaction) | `manual`, `auto` | Yes | `trigger` | `custom_instructions` | 1.0.48 |
| 28 | `PostCompact` | After compaction | `manual`, `auto` | No | `trigger` | `compact_summary` | 2.1.76 |
| 29 | `PreModelSwitch` | Before a requested model switch | model name | Yes | `from_model`, `to_model` | none | 2.1.251 |
| 30 | `PostModelSwitch` | After any model change | model name | No | `from_model`, `to_model` | none | 2.1.251 |
| 31 | `Elicitation` | MCP server requests user input; a hook may answer it | MCP server | Yes | `mcp_server_name`, `mode`, `elicitation_id` | `message`, `url`, `requested_schema` | 2.1.76 |
| 32 | `ElicitationResult` | User responded, before the reply reaches the server | MCP server | Yes | `mcp_server_name`, action | content | 2.1.76 |
| 33 | `SessionEnd` | Session terminates | `clear`, `resume`, `logout`, `prompt_input_exit`, `other` | No | `reason` | none | 1.0.85 |

`StopFailure.error` values: `rate_limit`, `overloaded`,
`authentication_failed`, `oauth_org_not_allowed`, `account_on_hold`,
`billing_error`, `invalid_request`, `model_not_found`, `server_error`,
`max_output_tokens`, `cloud_credential_error` (v2.1.267+), `unknown` [H].

Version column: "since" values come from `Added ...` entries in [CL]
where found. "newer" or "early" means no explicit changelog entry was
found in this pass.

### 1.3 Notification types and timing

All from [H] "Notification".

| `notification_type` | When | Live-state meaning |
| :- | :- | :- |
| `permission_prompt` | Approval needed (tool or sandbox network request), **after about 6 s with no keystroke**; keystrokes defer it. Under SDK hosts (Desktop, VS Code): about 6 s, not deferred | WAITING(permission), operator likely away. The **only** signal for sandbox network prompts (no `PermissionRequest` there) |
| `idle_prompt` | About 60 s after Claude finished, with no typing since and **no background agent running**. Not sent during a usage-limit wait | READY, confirmed and away. Fixed in 2.1.288 to respect background agents [CL 2.1.288] |
| `elicitation_dialog` / `elicitation_url_dialog` | MCP form or URL request, after about 6 s with no keystroke | WAITING(elicitation) |
| `elicitation_complete` / `elicitation_response` | URL elicitation done / response sent | Back to WORKING |
| `agent_needs_input` | A background session starts waiting, **only while agent view is open**. Also a teammate setup question, or the auto-mode charge notice | WAITING (background session) |
| `agent_completed` | Background session finishes or fails, only while agent view is open | READY / ERROR (background) |
| `quota_auto_resume_fired` | Task continues after a claude.ai usage limit | RATE_LIMITED → WORKING |
| `quota_auto_resume_stale` | Limit reset during a sleep over ~30 min; waits for Enter | WAITING (press Enter) |
| `quota_auto_resume_disabled` | Wait ended without continuing | READY or ERROR |
| `auth_success` / `auth_storage_failure` | Login completes / credentials can't be saved | Clears an auth error / ERROR |
| `plugin_notification` | A mod raised a notification | ignore |

The `permission_prompt`, `idle_prompt`, and `elicitation_*` dialog types
share desktop-notification timing. In terminal sessions "you only see
them when you appear to be away from the terminal" [H]. They are
*confirmations*, not edges.

### 1.4 Delivery semantics that constrain the Conspectus hook

| Property | Behavior | Source |
| :- | :- | :- |
| Parallelism | "When an event fires, its matching hooks run in parallel." `PostToolUse` fires concurrently for parallel tool calls | [H] |
| Dedup | "If you define the same handler in more than one settings file, it runs once." A plugin's copy stays separate | [H] |
| Default timeouts | 600 s for command hooks; 30 s on `UserPromptSubmit`/`*ModelSwitch`; 10 s on `MessageDisplay`; `SessionEnd` has a **1.5 s** shared budget (raised by per-hook `timeout` up to 60 s, or `CLAUDE_CODE_SESSIONEND_HOOKS_TIMEOUT_MS`) | [H], [ENV] |
| Sync cost | "By default, hooks block Claude's execution until they complete." A slow `UserPromptSubmit` "stalls the session"; a stuck `PreToolUse` blocks the tool for up to its timeout | [H] |
| Async | `async: true` (command only): background, can't block, not timed out, "each execution creates a separate background process"; killed at teardown in `-p` | [H] |
| Exec form | `args: [...]` spawns `command` directly with no `sh -c` (v2.1.139+) | [H], [CL 2.1.139] |
| stdout | Exit 0 stdout is parsed as JSON. For `SessionStart` (and `PostModelSwitch`) "plain stdout is added as context". The same for `UserPromptSubmit` is **UNVERIFIED** in this pass. Conspectus must print nothing | [H] |
| Exit codes | 0 success; **2 blocks** on blocking events (a `UserPromptSubmit` exit 2 drops the prompt); other codes are non-blocking errors unless `onFailure: "block"` (v2.1.295) | [H] |
| Environment | "A hook process inherits the parent environment," minus `OTEL_*` exporter vars (and scrubbed credentials under `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`). So `TMUX`/`TMUX_PANE` come through when `claude` itself runs in tmux. Exported: `CLAUDE_PROJECT_DIR`, `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`, `CLAUDECODE=1`, `CLAUDE_CODE_CHILD_SESSION=1`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_PID` (2.1.214+), `CLAUDE_EFFORT`, `CLAUDE_ENV_FILE` (some events), `CLAUDE_JOB_DIR` (background sessions), `CLAUDE_CODE_MESSAGING_SOCKET/TOKEN` (when bound), `CLAUDE_CODE_BRIDGE_SESSION_ID` (Remote Control) | [H], [ENV] |
| cwd | Hooks run in the session's current directory, with fallbacks if it was deleted | [H] |
| Workspace trust | Interactive: hooks from **every** settings file, including `~/.claude/settings.json`, are held back until the folder's trust dialog is accepted. `-p`/SDK: treated as trusted | [H] |
| Disable | `disableAllHooks`; `--settings '{"disableAllHooks":true}'`; managed `allowManagedHooksOnly` blocks user/project/local/plugin hooks; `--bare` skips hook discovery | [H], [HL] |
| Live reload | "Direct edits to hooks in settings files are normally picked up automatically by the file watcher," so an install reaches running sessions | [H], [SET] |
| Unknown event names | A settings file with "an unknown hook event name" gives a **Settings Warning**: the entry is skipped and the rest stays in effect. This matters when installing newer events on an older `claude` | [SET] |
| Interrupt and PostToolUse | After an interrupting `Ctrl+Enter`, `PostToolUse` hooks get 10 s, then are killed (behavior since 2.1.319) | [H], [CL 2.1.319] |
| `-p` / headless | Hooks fire in `-p` unless `--bare`. `PermissionRequest` fires (fixed in 2.1.268). `AskUserQuestion`/`ExitPlanMode` are offered only with a permission host. SIGTERM: "Claude Code then runs SessionEnd hooks and exits" | [HL], [H], [CL 2.1.268] |

**Measured cost** (this machine, inside the nono sandbox, `$TMUX` unset,
`--state-root` temp spool, release build at
`/home/jwaggoner/src/oss/conspectus/target/release/conspectus`, 50 runs
each). These numbers are indicative only:

| Invocation | Mean wall time |
| :- | :- |
| `/bin/true` (spawn baseline) | ~2 ms |
| `conspectus hook write claude-code` exec form | ~5 ms |
| same via `sh -c` (today's shell form) | ~10 ms |

Inside tmux, add one `tmux display-message` spawn per event. The daemon
socket path was not measured. A sync hook on `PreToolUse`/`PostToolUse`
therefore adds about 10-20 ms per tool call, small next to model latency.
Parallel tool calls spawn concurrent writers.

## 2. Mapping events to canonical live states

### 2.1 State vocabulary

Proposed for Claude Code; the graph and TUI researchers own the final
names:

- `STARTING`: process seen, no `SessionStart` yet. Also covers dialogs
  that come before hooks.
- `READY`: turn finished; idle prompt.
- `WORKING`: generating or a tool running.
  - Sub-reason `tool:<tool_name>` when `PreToolUse` is subscribed.
- `WAITING`: needs the operator. Reasons: `permission`, `question`,
  `plan`, `elicitation`, `sandbox`, `resume_enter`.
- `SUBAGENTS`: an overlay count. The main agent can be `READY` while
  background subagents run.
- `COMPACTING`
- `ERROR`: reason is the `StopFailure.error`. `RATE_LIMITED` is a
  special case: `rate_limit` from `StopFailure`, or waiting on a usage
  reset.
- `ENDED`: `SessionEnd` seen, process maybe alive.
- `EXITED`: process gone.

### 2.2 Edge table

Certainty legend:

- C = certain; documented, and fires immediately.
- L = likely; documented, but timing-gated or inferred.
- A = ambiguous.
- U = undetectable via hooks.

| Signal | → State | Certainty | Notes |
| :- | :- | :- | :- |
| process in pane, no hook record | STARTING / unknown | A | Workspace-trust dialog, "Resume this conversation?" dialog, settings-error dialog, and login all precede hooks [H], [SES], [SET]. The operator is actually needed, but hooks can't see it (U) |
| `SessionStart` `startup`/`resume`/`fork` | READY | C | "First response waits for" SessionStart hooks [H]. A new session with no prompt is READY |
| `SessionStart` `clear` | READY (new `session_id`, same process) | C | Preceded by `SessionEnd` `clear` for the old id |
| `SessionStart` `compact` | resume pre-compaction state | L | Order relative to `PostCompact` is **UNVERIFIED** |
| `UserPromptSubmit` (no `agent_id`) | WORKING | C | Also fires for self-started turns (loop, background report-back, peer message) [H]. Any submit means a turn started |
| `PreToolUse` `AskUserQuestion` | WAITING(question) | C | Removed from subagents [SA], so always main-agent |
| `PreToolUse` `ExitPlanMode` | WAITING(plan) | C | Plan approval dialog [H] |
| `PreToolUse` other tool | WORKING(tool) | C | Fires before the permission check, so it is not proof the tool runs |
| `PermissionRequest` | WAITING(permission) | C (edge) | Immediate [H]. An operator's own `PermissionRequest` hook may auto-decide, so the dialog may never show (A). With `agent_id`: a subagent is asking; background-subagent prompts surface in the main session [SA] |
| `Notification` `permission_prompt` | WAITING(permission), away | L | about 6 s gate [H]. Also the only signal for sandbox network prompts |
| `Elicitation` / `Notification` `elicitation_*dialog` | WAITING(elicitation) | C / L | |
| approval granted in dialog | WORKING | **U** | No event. The next signal is `PostToolUse` when the tool **finishes**, so WAITING can outlive approval for the tool's duration. OTel `claude_code.tool_decision` (source `user_temporary`/`user_permanent`) is the only push signal [MON]; `claude agents --json` `status` flips too [AV] |
| permission denied in dialog | WORKING or READY | **A/U** | Whether `PostToolUse`/`PostToolUseFailure` fire on a user rejection is **UNVERIFIED**. "No, and tell Claude..." / Esc interrupts the turn, so no `Stop` (see interrupt) |
| `PostToolUse` / `PostToolUseFailure` | WORKING; clears WAITING for that agent | C | Pair by `tool_use_id` with `PreToolUse`. Pair `PermissionRequest` by (`agent_id`, `tool_name`), since it has no `tool_use_id` |
| `PermissionDenied` (auto mode) | WORKING | C | |
| `PreCompact` | COMPACTING | C | `auto` can fire mid-turn or idle ("idle compaction") [H] |
| `PostCompact` | WORKING (`auto`, mid-turn) / READY (`manual`, or idle compaction) | L | Previous state decides |
| `Stop`, `background_tasks` empty | READY | C | `stop_hook_active: true` means a Stop hook is forcing continuation, so stay WORKING |
| `Stop`, `background_tasks` has `type: "subagent"` | READY + SUBAGENTS(n) | C | `background_tasks` since 2.1.145. Shell, monitor, and workflow tasks also listed. `session_crons` means a scheduled wake is pending |
| `StopFailure` `rate_limit` | RATE_LIMITED | C | A usage-limit wait then ends with `quota_auto_resume_*` |
| `StopFailure` other | ERROR(error) | C | `authentication_failed` means the operator must act |
| API retry in progress (529/overloaded, transient) | still WORKING | **U** | No hook. stream-json `system/api_retry` [HL], OTel `claude_code.api_error` with `attempt` [MON] |
| Esc / Ctrl+C interrupt | READY | **U** | "Does not run if the stoppage occurred due to a user interrupt" [H]. Cancelling a tool fires no `PostToolUseFailure` [H]. The next `UserPromptSubmit` recovers. `idle_prompt` about 60 s later after an interrupt is **UNVERIFIED** |
| `Notification` `idle_prompt` | READY (confirmed, away) | L | Repairs a WORKING stuck after an interrupt, if it fires then (**UNVERIFIED**) |
| `SubagentStart` | SUBAGENTS +1 (`agent_id`) | C | Also on subagent resume and per in-process teammate message [H], so dedupe by `agent_id` |
| `SubagentStop` | SUBAGENTS -1 | C on ≥2.1.311; A before (fired ~2x/min) | Ignore unknown `agent_id`s: internal agents fire it with empty/session `agent_type` [H] |
| `TeammateIdle` | teammate idle | C | Experimental agent teams only [AT] |
| `SessionEnd` `prompt_input_exit`/`other`/`logout` | ENDED → EXITED when pid dies | C | 1.5 s budget. Fixed in 2.1.300 for exits during background commands [CL 2.1.300] |
| `SessionEnd` `clear`/`resume` | old session ENDED; process continues | C | A `SessionStart` with a new id follows. `/resume` fix 2.1.79 |
| SIGKILL, crash, OOM, pane killed | EXITED | **U via hooks** | pid liveness (`CLAUDE_PID` + process start time) is definitive on Linux. SessionEnd on SIGHUP (terminal or pane closed) is **UNVERIFIED**; SIGTERM is documented for `-p` [HL] |

### 2.3 Undetectable or ambiguous via hooks

These need a fallback or must be accepted:

1. Esc/Ctrl+C interrupt: stuck WORKING.
2. Permission approval: the WAITING → WORKING edge comes only at tool
   completion.
3. Permission rejection: may interrupt the turn.
4. Pre-hook dialogs: trust, resume-cost dialog, settings error, login.
5. API retry or backoff periods.
6. Kill or crash.
7. Whether a `PermissionRequest` was auto-decided by another hook.
8. The "worker request" and "dialog open" waits that `claude agents --json`
   reports [AV]. They have no hook equivalent; mod and other dialogs
   included.

## 3. Subagents

| Aspect | Finding | Source |
| :- | :- | :- |
| Start | `SubagentStart` with `agent_id` ("agent-abc123" in the example) and `agent_type` (built-in `general-purpose`/`Explore`/`Plan`, custom frontmatter `name`, or plugin-scoped `plugin:name`) | [H] |
| Stop | `SubagentStop` with `agent_id`, `agent_type`, `agent_transcript_path` (`~/.claude/projects/<p>/<session>/subagents/agent-<id>.jsonl`), and `stop_hook_active` | [H], [SA] |
| Activity | Subagent tool calls fire `PreToolUse`/`PostToolUse` with `agent_id`, so liveness and per-subagent "running Bash" are visible | [H], [SA] |
| Permission | A foreground subagent passes prompts through. A background subagent's prompt "surfaces in your main session and names the subagent". `PermissionRequest` carries the asker's `agent_id` (inferred from the common-field rule) | [SA], [H] |
| Questions | `AskUserQuestion` is removed from every subagent | [SA] |
| Default mode | "With nothing configured, a subagent that Claude spawns from your main conversation starts in the background." Ctrl+B backgrounds a foreground one; `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1` forces foreground | [SA], [ENV] |
| Background handshake | Agent `PostToolUse` returns `tool_response.status: "async_launched"` with `agentId`. Main `Stop` then lists it in `background_tasks` (`type: "subagent"`). Completion arrives as an automated turn, which fires `UserPromptSubmit` ("a background subagent reporting back") | [H], [SA] |
| Parallel | Several Agent calls in one message: N × (`PreToolUse` Agent, `SubagentStart`, ..., `SubagentStop`, `PostToolUse` Agent). Default concurrency cap 20 (`CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS`, 2.1.217+). Nesting depth default 3 (`CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH`) | [SA], [ENV] |
| Count | The `SubagentStart`/`SubagentStop` id set, reconciled at each `Stop`/`SubagentStop` against `background_tasks` (subagent entries, status). The UI also lists running and done subagents for 30 s in `/tasks` | [H], [SA] |
| Teammates (experimental) | `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`. In-process teammates fire `SubagentStart` per handled message and `TeammateIdle`. **Split-pane** teammates are separate Claude instances, each in its own tmux pane, so one mux holds several Claude processes. Team config at `~/.claude/teams/{team}/config.json` holds session ids and tmux pane ids | [AT], [H] |
| Transcripts | The current docs place subagent transcripts in separate files under `<session>/subagents/`, not in the main file. Whether the main JSONL still carries `isSidechain: true` records is **UNVERIFIED**; the repo fixtures have none | [SA], [CD] |
| stream-json | Subagent messages carry `parent_tool_use_id` = the Agent `tool_use` id; text needs `--forward-subagent-text` | [HL] |
| Internal agents | Prompt suggestions and `/btw` fire `SubagentStop` with `agent_type` empty or the session agent. A matching `SubagentStart` is **UNVERIFIED**, so ignore stops for unknown ids | [H] |

## 4. Delivery and install

### 4.1 Where hooks live

From [H] "Hook locations" and [SET]:

| Location | Scope | Notes |
| :- | :- | :- |
| `~/.claude/settings.json` | user, all projects | Claude Code itself writes this file (`/config`, `/model` default) [SET], so concurrent writes are possible |
| `.claude/settings.json` | project, committable | needs workspace trust |
| `.claude/settings.local.json` | project, personal | Claude Code writes approvals here and keeps it out of git [SET] |
| managed policy (`managed-settings.json`, `managed-settings.d/`, MDM, server-managed) | org | `disableAllHooks` outside managed can't disable managed hooks |
| plugin `hooks/hooks.json` | when plugin enabled | an alternative distribution (see Q12) |
| skill / subagent frontmatter | scoped | not relevant |
| `CLAUDE_CONFIG_DIR` | relocates `~/.claude` | `hook init` today hardcodes `$HOME/.claude/settings.json`, so it ignores `CLAUDE_CONFIG_DIR` (repo finding) |

Hook entries **merge** across levels ("user, project, and local settings
add their own hooks without removing managed ones") [H]. Identical handlers
in several files run once [H].

### 4.2 Proposed entry shape

Exec form needs ≥2.1.139. On an older version an `args`-bearing entry
would run `command` without its args. That version-floor behavior is
**UNVERIFIED**, so pick by detected version or keep shell form.

```json
{
  "hooks": {
    "SessionStart":       [{ "hooks": [{ "type": "command", "command": "/abs/conspectus", "args": ["hook", "write", "claude-code"], "timeout": 5 }] }],
    "UserPromptSubmit":   [{ "hooks": [ ...same handler... ] }],
    "PreToolUse":         [{ "matcher": "AskUserQuestion|ExitPlanMode", "hooks": [ ... ] }],
    "PermissionRequest":  [{ "hooks": [ ... ] }],
    "PostToolUse":        [{ "hooks": [ ... ] }],
    "PostToolUseFailure": [{ "hooks": [ ... ] }],
    "Notification":       [{ "matcher": "permission_prompt|idle_prompt|elicitation_dialog|elicitation_url_dialog|agent_needs_input|quota_auto_resume_fired|quota_auto_resume_stale|quota_auto_resume_disabled", "hooks": [ ... ] }],
    "SubagentStart":      [{ "hooks": [ ... ] }],
    "SubagentStop":       [{ "hooks": [ ... ] }],
    "Stop":               [{ "hooks": [ ... ] }],
    "StopFailure":        [{ "hooks": [ ... ] }],
    "PreCompact":         [{ "hooks": [ ... ] }],
    "PostCompact":        [{ "hooks": [ ... ] }],
    "SessionEnd":         [{ "hooks": [ ... ] }]
  }
}
```

Rules for safe coexistence with the operator's hooks:

- **Append our own matcher group** per event; never edit an
  operator-owned group. This matches today's `ensure_claude_hook`.
  - An omitted matcher matches all. A matcher of only
    letters/digits/`_`/`|` is an exact list. Anything else is an
    unanchored JS regex [H].
  - Don't use the `SessionStart` matcher `resume|startup|clear|compact`:
    it misses `fork`. Omit the matcher instead.
- **Identify our entries** by the handler, not the group. Today the check
  is a `command` substring `hook write claude-code`. Exec form puts that
  in `args`, so detection must check `command` + `args`.
- **Always set `timeout`** (proposal: 5 s; `SessionEnd` 2 s). This bounds
  a stuck daemon socket. The writer also needs its own internal deadline:
  check `server::call_command` for a read timeout (not verified here).
- **Sync, not async** for state events. Sync keeps per-session ordering,
  because Claude waits for the writer before the next step. Async spawns
  a detached process per event and can reorder `PostToolUse` against
  `Stop`.
- **Writer contract** (must, given ADR 0143 and model-context safety):
  - no stdout bytes, ever;
  - exit 0 always, including on parse/daemon failure; warn on stderr only
    if needed;
  - never return decision fields;
  - read stdin fully;
  - parse into a typed whitelist struct (serde ignores the rest). A
    whole-payload `Value` parse is wasteful on multi-MB `tool_response`.
- **Prefer a stable binary path.** `default_hook_command` embeds
  `current_exe()`. A Nix store path goes stale on upgrade, and with 14
  events every hook then errors on every event.
- **Status and remove** must cover all events. `hook status` should report
  per-event coverage: partial means an older install.
- **Key order:** enable `serde_json`'s `preserve_order` (adds `indexmap`;
  dependency rule says ADR first), or edit as text, so the operator's
  `settings.json` order survives.
- Payloads stay in the hook process memory only. Persist only the
  whitelist in §6.2 (ADR 0086 Tier 3).

### 4.3 Reliability

| Hazard | Effect | Mitigation |
| :- | :- | :- |
| Process killed (SIGKILL), crash, power loss | no `SessionEnd`, state frozen | Record `CLAUDE_PID` plus `/proc/<pid>/stat` start time (guards pid reuse). Treat dead pid as EXITED |
| Esc interrupt | no `Stop`, WORKING frozen | §6.4 staleness plus fallbacks (§5) |
| Untrusted folder | **no hooks at all** in interactive mode [H] | Process-tree discovery shows the session; state "unknown" |
| `--bare`, `disableAllHooks`, `allowManagedHooksOnly` | no hooks | same |
| Daemon or socket slow | delays every tool call up to `timeout` | Internal deadline; spool fallback |
| Two Claude processes in one tmux session | latest-per-mux spool overwrites | Key live state by (`harness`, `session_id`), with pane id as attribution |
| Background sessions (agent view) | Hooks run in a supervisor child whose env comes from the dispatching shell. `TMUX` may name someone else's pane | Existing `background_harness` detection (`daemon run`, `--bg-spare`, `--bg-pty-host` in `claude_code_is_background_process`). Attach the state to the session node, not a mux. `CLAUDE_JOB_DIR` is set in these sessions [ENV] |
| Nested sessions | A `claude` started from a Claude tool gets `CLAUDE_CODE_CHILD_SESSION=1` and is excluded from `claude agents` | Usually ignore |
| Version skew | `SubagentStop` spam before 2.1.311; `background_tasks` before 2.1.145; `StopFailure` before 2.1.78; `PermissionRequest` in `-p` before 2.1.268; `CLAUDE_PID` before 2.1.214; `idle_prompt` during background agents before 2.1.288 | Document a floor; degrade gracefully |
| Clock and order | No payload timestamp. Concurrent `PostToolUse` writers | Stamp ms at writer start plus a per-process monotonic counter; the reducer orders by (observed_ms, seq) and pairs by `tool_use_id` |

## 5. Hook-free fallbacks

### 5.1 `claude agents --json` (official, recommended fallback)

From [AV]; local `claude agents --help` on 2.1.322 says "Print active
sessions (interactive and background) as a JSON array and exit (for
scripting; does not require a TTY)".

| Field | Present | Meaning |
| :- | :- | :- |
| `cwd`, `kind` (`interactive`/`background`), `startedAt` (ms) | always | |
| `pid`, `status` (`busy`/`waiting`/`idle`) | while the process is alive | Covers interactive sessions without hooks |
| `waitingFor` | when `status` is `waiting` | `permission prompt`, `input needed` (question or MCP input), `sandbox request`, `worker request`, `dialog open` |
| `sessionId`, `name` | when set | `name` can be a generated title from prompt or plan: payload-adjacent, so drop it |
| `id`, `state` (`working`/`blocked`/`done`/`failed`/`stopped`) | background sessions | |

- History: added 2.1.145; `waitingFor` 2.1.162; `--all`, `id`, `state`
  2.1.169; sandbox/MCP-input/managed prompts reported as "Needs input"
  2.1.212; raw-mode terminal bug fixed 2.1.257 [CL].
- `--cwd <path>` filters.
- `CLAUDE_CODE_DISABLE_AGENT_VIEW` turns the feature off [ENV].
- Nested child sessions are excluded [ENV].

This closes the hook gaps: interrupt (status → idle), approval granted
(waiting → busy), pre-hook dialogs (`dialog open`), untrusted folders, and
`--bare`.

Unknowns:

- Cost per poll: it is a full CLI start. Must be measured.
- Whether it starts the supervisor when none runs. The docs are silent;
  that would be a side effect.
- Whether `status` exists for every interactive session version-wide.

It is a new subprocess, so it needs a `docs/capabilities.toml` entry and
an ADR. ADR 0028 already allows "a harness-documented non-mutating control
channel audited as safe" as an explicit control-plane adapter.

### 5.2 `~/.claude/sessions/` and `~/.claude/jobs/<id>/state.json`

- `sessions/` "holds one small file per running session, used to detect
  concurrent sessions and crashes", removed on exit [CD]. Format
  **UNVERIFIED**; it is probably what `claude agents --json` reads for
  interactive sessions. Not documented as stable.
- `jobs/<id>/state.json` is per background session. The docs say
  explicitly "not a stable interface"; read through `claude agents --json`
  instead [AV].

Reading either is cheap. It is Tier 1 if limited to pid/status fields,
but it is an operator decision (Q4).

### 5.3 Transcript JSONL tail

- Location: `~/.claude/projects/<project>/<session-id>.jsonl`. `<project>`
  is the cwd with non-alphanumerics → `-`; names over 200 chars are
  truncated plus a hash [SES].
- Subagents: `<session>/subagents/agent-<id>.jsonl` [CD].
- The docs warn: "The entry format is internal to Claude Code and changes
  between versions" [SES].

Shapes known from repo code and tests:

- Record keys `type`, `uuid`, `parentUuid`, `timestamp`, `cwd`,
  `sessionId`, `message{role,content}`, `isMeta`, `isCompactSummary`,
  `forkedFrom`.
- Record types `user`, `assistant`, `system` (`subtype: "compact_boundary"`
  in tests), `summary`, `permission-mode`, `file-history-snapshot`,
  `custom-title`, `agent-name`.
- Block types `text`, `thinking`, `tool_use`, `tool_result`.

| Tail pattern | Inference | Accuracy |
| :- | :- | :- |
| last record `user` with plain text, no child | turn submitted → WORKING (or aborted before reply) | A; the repo's viewer uses "orphan" as an abort heuristic, except for the last record |
| last `assistant` record has a `tool_use` block with no matching `tool_result` (by id) | tool running **or** awaiting permission (indistinguishable) | A |
| last `assistant` with `message.stop_reason: "end_turn"` | READY | **UNVERIFIED**: `stop_reason` on transcript records is not in repo fixtures |
| `system` `compact_boundary` / `isCompactSummary` | compaction just happened | L |
| interrupt marker user text (e.g. "[Request interrupted by user]") | READY after Esc | **UNVERIFIED**; also needs a content match, which conflicts with the Tier 1 rule (Q6) |
| mtime advancing | activity | L; the file "is written asynchronously and may lag" [H] |
| `isSidechain` | subagent record in main file | **UNVERIFIED** for current versions (separate files now) |

Accuracy limit: the tail can't tell "waiting for approval" from "tool
running". It lags the live UI, and its format is unstable. Use it as a
tiebreaker only. Example: a WORKING record older than N minutes whose
transcript ended with an interrupt marker or an `end_turn` should downgrade
to READY.

### 5.4 tmux pane metadata (no content scraping)

| Surface | Finding | Status |
| :- | :- | :- |
| `#{pane_title}` | Claude Code sets the terminal title, prefixed with a status glyph in terminals without OSC 7501 support [TC]. Busy-spinner glyphs: fixed-width braille, 2.1.7, updated 2.1.228. In tmux, written only when the text changes (2.1.236) [CL]. tmux takes OSC 0/2 into `pane_title` unless `allow-set-title off` [TMUX]. `CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1` disables it [ENV] | Glyph vocabulary per state **UNVERIFIED**; record it |
| OSC 7501 Program Status Protocol | 2.1.295+: "Inside tmux, Claude Code checks tmux for support instead of your terminal." If supported, the glyph prefix is omitted [TC] | Whether any tmux implements OSC 7501 is **UNVERIFIED**; not in the tmux CHANGES text fetched |
| `#{pane_pb_state}` / `#{pane_pb_progress}` | tmux 3.7+ stores OSC 9;4 progress as `hidden`/`normal`/`error`/`indeterminate`/`paused` [TMUX]. Claude Code emits OSC 9;4 (setting since 2.0.56; stays on across the full turn since 2.1.128; inside tmux forwarded via passthrough since 2.1.78) [CL] | Whether tmux's own parser sees it, rather than passthrough-wrapped, is **UNVERIFIED**. The local tmux is 3.7c |
| `#{pane_current_command}` | Already in `TMUX_LIST_FORMAT`. Shows `claude` vs a shell, which means alive or exited, not busy | C for EXITED-to-shell |
| `capture-pane` text (spinner line, "esc to interrupt", dialog text) | Prohibited as generic scraping by ADR 0028 unless a new ADR allows it for already-visible status surfaces. A custom statusLine hides "esc to interrupt" [SL] | not recommended |

`pane_title` and `pane_pb_state` are cheap: one more field in the
existing list-panes format. They are tmux metadata the agent publishes,
not pane content. ADR 0028 still asks for a separate ADR before using
"already-visible status surfaces".

### 5.5 Other official surfaces

| Surface | What it gives | Fit |
| :- | :- | :- |
| `statusLine` command [SL] | JSON per update (`session_id`, `transcript_path`, `version`, `model`, `cost`, `context_window`, `rate_limits.*.resets_at`, `prompt_id`, `worktree`, ...). Runs on new assistant message, `/compact` end, mode change, and so on; 300 ms debounce. **No busy/idle field**, and it is hidden during permission prompts | Poor. It is a single operator-owned slot, so Conspectus would have to own or chain the operator's status line. It could give `version` and rate-limit reset time |
| `subagentStatusLine` [SL] | `tasks[]` with `status` (`running`/`completed`/`failed`/`killed`), `agentType`, `startTime`, `tokenCount` per refresh tick | Good subagent data, but the same single-slot problem |
| OpenTelemetry [MON] | Events `claude_code.user_prompt`, `api_request`, `api_error` (`attempt`), `tool_result`, `tool_decision` (`decision`, `source`: `config`/`hook`/`user_permanent`/`user_temporary`/`user_abort`/`user_reject`); `agent_id`/`parent_agent_id` (2.1.310). Beta spans include `claude_code.tool.blocked_on_user`. Prompt and tool content redacted by default. Exporters `otlp`/`console`/`prometheus`; logs exported every 5 s | Only push source for approval granted and API retry. Needs `CLAUDE_CODE_ENABLE_TELEMETRY` plus env in the operator's config (outside ADR 0143 category 5) and a local OTLP receiver. A Phase-3 complement at best |
| `--output-format stream-json` [HL], [SDK] | `system/init`, `assistant`/`user` (`parent_tool_use_id`), `stream_event`, `system/api_retry` (`attempt`, `error`), `system/compact_boundary`, `system/permission_denied`, `result`, hook events | Only for headless runs Conspectus launches itself. It can't observe operator-launched sessions |
| `claude daemon status` [AV] | supervisor state, socket dir, worker count | Background sessions only |

## 6. Recommendation

### 6.1 Minimal subscription

**Phase 1 (MVP).** These are low-frequency except the two `PostToolUse`
events, which are needed to clear waits. 14 events:

| Event | Matcher | Why |
| :- | :- | :- |
| `SessionStart` | (none; all sources incl. `fork`) | identity, attribution (existing), READY |
| `UserPromptSubmit` | n/a | WORKING edge (incl. self-started turns) |
| `PermissionRequest` | (none) | immediate WAITING(permission), per `agent_id` |
| `PreToolUse` | `AskUserQuestion\|ExitPlanMode` | WAITING(question / plan). Low frequency thanks to the matcher |
| `PostToolUse` | (none) | clears WAITING; subagent liveness; Agent `async_launched` |
| `PostToolUseFailure` | (none) | same, for failures; `is_interrupt` |
| `Notification` | `permission_prompt\|idle_prompt\|elicitation_dialog\|elicitation_url_dialog\|agent_needs_input\|quota_auto_resume_fired\|quota_auto_resume_stale\|quota_auto_resume_disabled` | away-confirmations; sandbox-prompt WAITING; usage-limit transitions |
| `SubagentStart` / `SubagentStop` | (none) | subagent set (`agent_id`) |
| `Stop` | n/a | READY plus `background_tasks` snapshot |
| `StopFailure` | (none) | ERROR / RATE_LIMITED |
| `PreCompact` / `PostCompact` | (none) | COMPACTING |
| `SessionEnd` | (none) | ENDED |

- **Phase 2 (optional detail):** unrestricted `PreToolUse` for "running
  <tool>" and an in-flight tool set; `Elicitation`/`ElicitationResult`
  for immediate MCP-input waits; `TeammateIdle` if agent teams matter;
  `CwdChanged` for cwd attribution.
- **Never:** `MessageDisplay` (payload text, high frequency, can rewrite
  output), `PostToolBatch`, `UserPromptExpansion`, `WorktreeCreate`
  (replaces git behavior), `Setup`, `ConfigChange`, `TaskCreated`/`TaskCompleted`.

### 6.2 Field whitelist (persisted; everything else dropped in memory)

| Kept | Events |
| :- | :- |
| `session_id`, `hook_event_name`, `cwd`, `transcript_path`, `permission_mode`, `agent_id`, `agent_type`, `prompt_id` | all |
| `source`, `model` | SessionStart |
| `tool_name` (Q7 for MCP names), `tool_use_id`, `duration_ms`, `is_interrupt` | Pre/PostToolUse(Failure), PermissionRequest |
| `tool_response.status`, `tool_response.agentId` (Agent tool only) | PostToolUse |
| `notification_type` | Notification |
| `agent_transcript_path` (optional), `stop_hook_active` | SubagentStop, Stop |
| `background_tasks[].{id,type,status,agent_type}`, `session_crons` **count** | Stop, SubagentStop |
| `error` (enum) | StopFailure only. **Not** `PostToolUseFailure.error`, which is free text |
| `trigger` | Pre/PostCompact |
| `reason` | SessionEnd |
| writer-side: `observed_ms`, per-process `seq`, `pid` (`CLAUDE_PID` else walk), pid start time, tmux pane id/session/socket, `background_harness` | all |

Dropped: `prompt`, `tool_input`, `tool_response` (other than the two Agent
fields), `message`, `title`, `last_assistant_message`, `error_details`,
`compact_summary`, `custom_instructions`, `session_title`,
`permission_suggestions`, `background_tasks[].description/command`,
`session_crons[].prompt`, and Elicitation `message`/`url`/`requested_schema`.

### 6.3 Reducer

Per (`harness`, `session_id`). Agent-scoped sub-state is keyed by
`agent_id`; `main` = absent.

```
on SessionStart(src)            -> main=READY; clear in-flight, waits, subagents (unless src=compact: keep)
on UserPromptSubmit             -> main=WORKING; clear main waits
on PreToolUse(AskUserQuestion)  -> main=WAITING(question)
on PreToolUse(ExitPlanMode)     -> main=WAITING(plan)
on PermissionRequest(agent,t)   -> waits[agent]=permission(t, at)
on Notification(permission_prompt) -> waits[*] confirmed-away (or WAITING(sandbox) if no pending request)
on Notification(elicitation_*dialog) -> waits[main]=elicitation
on PostToolUse/Failure(agent,t,id) -> clear waits[agent] if tool matches or any; agent WORKING; if Agent+async_launched -> subagents += agentId
on PermissionDenied             -> clear waits[agent]
on PreCompact                   -> main=COMPACTING (remember prior)
on PostCompact(trigger)         -> main = prior==WORKING ? WORKING : READY
on SubagentStart(id)            -> subagents += id
on SubagentStop(id)             -> subagents -= id (ignore unknown)
on Stop(stop_hook_active=false) -> main=READY; subagents = reconcile(background_tasks type=subagent, status running)
on StopFailure(e)               -> main = e==rate_limit ? RATE_LIMITED : ERROR(e)
on Notification(quota_auto_resume_fired) -> main=WORKING
on Notification(quota_auto_resume_stale) -> main=WAITING(resume_enter)
on Notification(idle_prompt)    -> main=READY (repairs stuck WORKING)
on SessionEnd(reason)           -> session ENDED (process may continue for clear/resume)
on pid dead                     -> EXITED (definitive)

display = WAITING (any agent) > ERROR/RATE_LIMITED > COMPACTING > WORKING
          > READY+SUBAGENTS(n) > READY > ENDED > EXITED
```

### 6.4 Staleness rule

1. A recorded pid that is dead, or whose start time differs, means
   EXITED. This overrides everything.
2. WAITING and READY don't decay while the pid lives. An operator can
   leave a prompt for hours.
3. WORKING with no event for `T_work` (proposal: 10 min) and transcript
   mtime older than `T_work` becomes WORKING-stale ("unknown"). Consult
   the §5 fallbacks: `claude agents --json` status, then pane title or
   pb_state, then the transcript tail. An `idle_prompt` or the next
   `UserPromptSubmit` repairs it. Subagent tool events count as activity.
4. WAITING(permission) has no `permission_prompt` confirmation within
   about 10 s, then a `claude agents --json` `status != waiting`: the
   prompt was answered, so WORKING. This heuristic is gated on the
   fallback, because without it a fast approval still reads WAITING until
   the tool finishes.
5. A record with no pid uses today's 15 min TTL for every state.
6. `SessionEnd` `clear`/`resume` with no `SessionStart` from the same pid
   within 5 s: treat the process as ENDED.

### 6.5 Fixture plan (names only)

Record shapes with a redacting recorder in a throwaway project:

- `claude --settings '{"hooks":...}'`, so no operator settings file is
  touched.
- The recorder replaces every non-enum string with a placeholder.
- Then store the results under `tests/fixtures/hooks/claude-code/`.
- Re-record the two existing fixtures, which use the invalid
  `source: "user"`.

Single payloads:

- `sessionstart-startup.json`, `sessionstart-resume.json`,
  `sessionstart-clear.json`, `sessionstart-compact.json`,
  `sessionstart-fork.json`, `sessionstart-agent.json`
- `userpromptsubmit.json` (asserts `prompt` is dropped)
- `pretooluse-askuserquestion.json`, `pretooluse-exitplanmode.json`,
  `pretooluse-bash.json`, `pretooluse-subagent-bash.json`,
  `pretooluse-mcp.json`
- `permissionrequest-bash.json`, `permissionrequest-subagent.json`
- `posttooluse-bash.json`, `posttooluse-agent-completed.json`,
  `posttooluse-agent-async-launched.json`, `posttoolusefailure.json`,
  `posttoolusefailure-interrupt.json`
- `notification-permission-prompt.json`, `notification-idle-prompt.json`,
  `notification-elicitation-dialog.json`,
  `notification-agent-needs-input.json`,
  `notification-quota-auto-resume-fired.json`,
  `notification-quota-auto-resume-stale.json`
- `subagentstart.json`, `subagentstop.json`,
  `subagentstop-internal-empty-type.json`
- `stop.json`, `stop-background-subagent.json`, `stop-hook-active.json`,
  `stop-session-crons.json`
- `stopfailure-rate-limit.json`, `stopfailure-overloaded.json`,
  `stopfailure-authentication-failed.json`
- `precompact-auto.json`, `precompact-manual.json`,
  `postcompact-auto.json`
- `sessionend-prompt-input-exit.json`, `sessionend-clear.json`,
  `sessionend-resume.json`, `sessionend-other.json`
- `payload-large-tool-response.json` (parse-cost test)

Sequences (ordered JSONL of payloads plus writer ms stamps) for reducer
tests:

- `seq-simple-turn`, `seq-permission-approve`,
  `seq-permission-approve-long-tool`, `seq-permission-deny-esc`,
  `seq-esc-interrupt`
- `seq-askuserquestion`, `seq-plan-approve`, `seq-plan-reject`
- `seq-parallel-foreground-subagents`, `seq-background-subagent-outlives-stop`
- `seq-auto-compact-midturn`, `seq-manual-compact-idle`
- `seq-clear`, `seq-resume-in-process`, `seq-rate-limit-auto-resume`
- `seq-kill9-no-sessionend`, `seq-old-version-subagentstop-spam` (pre-2.1.311)

Fallback fixtures:

- `claude-agents-json-interactive-busy.json`,
  `claude-agents-json-interactive-waiting-permission.json`,
  `claude-agents-json-interactive-idle.json`,
  `claude-agents-json-background-blocked.json`,
  `claude-agents-json-background-done.json`
- Transcript tails: `tail-tool-use-pending.jsonl`, `tail-end-turn.jsonl`,
  `tail-interrupt.jsonl`, `tail-compact-boundary.jsonl`
- Pane titles: `pane-title-busy.txt`, `pane-title-idle.txt`,
  `pane-title-waiting.txt`
- `sessions-registry-entry.json` (if Q4 says yes)

## 7. Open questions for the operator

1. **Per-tool-call hooks.** Accept two writer spawns per tool call
   (`PostToolUse`/`PostToolUseFailure` for all tools, about 5-20 ms each,
   sync), or subscribe only to the low-frequency events and accept
   WAITING lingering until the next turn event?
2. **Sync vs async** writer entries. The recommendation is sync with
   `timeout: 5`.
3. **`claude agents --json` as a polled control-plane adapter.**
   - Needs a new ADR and a `docs/capabilities.toml` entry.
   - Run it each refresh, on demand, or only to repair stale WORKING and
     WAITING?
   - It must first be checked that it doesn't start the supervisor.
4. **Read `~/.claude/sessions/*.json`?** The format is undocumented and
   not stable. It is cheaper than a subprocess.
5. **tmux `#{pane_title}` / `#{pane_pb_state}`** as a status surface.
   ADR 0028 requires its own ADR. Record the glyph vocabulary first.
6. **Transcript-tail structure parsing** for the interrupt and `end_turn`
   repair. The interrupt marker needs a text match: does that stay
   Tier 1?
7. Is persisting **`tool_name`** acceptable for MCP tools
   (`mcp__server__tool` reveals server and tool names)? Is persisting
   `agent_type` for custom agents acceptable?
8. **Install UX.** Should `hook init claude-code` upgrade an existing
   SessionStart-only install to the full set automatically, or behind a
   flag?
   - Should a version check (`claude --version`) gate exec form and
     newer events?
   - Should `preserve_order` (a new dependency, so ADR) keep the
     operator's key order?
9. **Version floor** to support, e.g. ≥2.1.145, or ≥2.1.311 for clean
   `SubagentStop`.
10. Should **"away-confirmed"** waits (`permission_prompt`/`idle_prompt`
    after the 6 s / 60 s gates) render differently from fresh waits?
11. **One mux, many sessions** (split-pane teammates, background
    sessions attached in a pane): show per-pane state, or aggregate per
    mux with the highest-priority state?
12. **Distribution:** a Conspectus Claude Code plugin (`hooks/hooks.json`)
    instead of `settings.json` edits?
    - It avoids the settings rewrite and key reordering.
    - But plugin install writes under `~/.claude/plugins`, which falls
      outside ADR 0143 category 5 as written.
13. **Background `claude --bg` sessions:** hooks there carry the
    dispatching shell's environment. Rely on `claude agents --json` for
    them and ignore their tmux context, as `background_harness` does now?

## Appendix: UNVERIFIED items

- Transcript fields `message.stop_reason`, `isSidechain`, `toolUseResult`;
  interrupt-marker wording; `system` subtypes other than
  `compact_boundary`; whether sidechain records still appear in the main
  JSONL.
- The pane-title glyph per state; whether tmux implements OSC 7501;
  whether tmux's `pane_pb_state` sees Claude's OSC 9;4 inside tmux.
- The `~/.claude/sessions/` file format and whether it carries status.
- Whether `claude agents --json` starts the supervisor; its cost per run.
- Whether `PermissionRequest` fires for `AskUserQuestion`/`ExitPlanMode`.
- Whether `PostToolUse`/`PostToolUseFailure` fire on a user rejection in
  the dialog; whether `idle_prompt` fires after an Esc interrupt.
- `transcript_path` and `session_id` inside subagent tool events (implied
  to be the parent's).
- The order of `PreCompact` → `PostCompact` → `SessionStart(compact)`.
- `SessionEnd` on SIGHUP (closed terminal or killed pane) in interactive
  mode.
- Whether `CLAUDE_CODE_VERSION` exists in the hook environment.
- Whether split-pane teammates each have their own `session_id` and fire
  `SessionStart`.
- What an older `claude` (<2.1.139) does with an `args`-bearing (exec
  form) entry.
- `UserPromptSubmit` plain stdout becoming context (documented here only
  for `SessionStart`).
