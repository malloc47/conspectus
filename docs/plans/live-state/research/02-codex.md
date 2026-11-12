# Live State Research 02: Codex

Research notes for the live-state feature. Scope: one harness, OpenAI's
`codex` CLI. These notes cover per-harness signal extraction only; the graph
representation and TUI surfacing come later.

Method: the repo's own adapter code, fixtures, ADRs, and backlog tasks for
on-disk shapes (no operator data under `~/.codex` was read), plus Codex docs
and `openai/codex` source on `main`, fetched 2026-11-11/12. Every external
claim cites a URL in the Sources section by tag (`[S#]`). Anything I could not
confirm is marked **UNVERIFIED**. Fetched content was treated as untrusted
data.

Two caveats on the sources:

- `developers.openai.com/codex/*` now answers with `308` redirects to
  `learn.chatgpt.com/docs/*`. The redirect target was server-supplied and I
  could not verify it independently, but its content is consistent with the
  Codex source.
- The `main`-branch source and schemas can run ahead of the latest release.
  The docs say so for the hook schemas [S1]. The latest release on 2026-11-11
  is `rust-v0.167.2` [S30].

## 0. Key findings

1. **Codex now has a full hook system, and it composes.** It has 14 events,
   including `PermissionRequest`, `Stop`, `Interrupt`, `SessionEnd`,
   `SubagentStart`/`SubagentStop`, `PreCompact`/`PostCompact`, and
   `Pre`/`PostModelRequest`. Hooks from every config layer and every matcher
   group run, so Conspectus's entry sits beside the operator's own hooks
   [S1][S3]. This is the primary live-state channel. `notify` is not: it is
   one argv array, it fires only on `agent-turn-complete`, and Conspectus
   would have to displace the operator's program to use it [S2][S9].
2. **Approval requests are never written to the rollout.** The rollout
   policy drops `exec_approval_request`, `apply_patch_approval_request`,
   `request_user_input`, `elicitation_request`, `request_permissions`,
   `error`, and `stream_error` [S6]. Without hooks, WAITING cannot be read
   from the transcript directly. On disk, a pending approval looks the same
   as a long-running tool call.
3. **Codex writes its own run state into the terminal title.** The default
   title items include `activity`. A running task shows a braille spinner
   frame. When the bottom pane needs the operator (an approval, question,
   elicitation, or plan prompt), the title shows `[ ! ] Action Required`
   [S20][S21]. tmux exposes this read-only as `#{pane_title}` [S29]. It is
   the best hook-free WAITING signal, and it is the only signal that clears
   promptly when an approval is answered. ADR 0028 allows already-visible
   status surfaces only through a separate ADR.
4. **Codex already models the canonical states internally.**
   - The TUI's `ProgramStatus` is `Idle | Working | Blocked(Permission |
     Question | Auth) | Done | Error`, emitted as OSC 7501 [S22]. tmux likely
     drops OSC 7501 (UNVERIFIED).
   - The app-server's `ThreadStatus` is `notLoaded | idle | systemError |
     active{activeFlags: waitingOnApproval | waitingOnUserInput}` [S15][S16].
     It is reachable only for operators who run a shared
     `codex app-server --listen` and attach their TUIs with `--remote` [S24].
5. **The existing `conspectus hook init codex` probably no longer fires on
   current Codex, and it can destroy operator hooks.**
   - Unmanaged hooks are skipped until the operator trusts them in `/hooks`.
     Trust is keyed by hash and position [S1][S12].
   - The installer replaces an operator's `[[hooks.SessionStart]]` array of
     tables (the form Codex's docs show) with its own inline array. This is
     from reading the code (§4.6).
6. **Subagents are visible.**
   - Hooks: `SubagentStart`/`SubagentStop` carry `agent_id`/`agent_type`.
     Subagent hooks reuse the parent's `session_id` [S1][S3].
   - State DB: `thread_spawn_edges.status` is `open | closed` [S13][S14].
   - Rollout: child threads have their own rollouts, with `session_meta.source
     = subagent/thread_spawn{parent_thread_id,...}` [S5].

## 1. Baseline: what Conspectus already has for Codex

| Piece | Where | What it does |
|---|---|---|
| Codex adapter | `src/discovery/harness/codex.rs` | Reads `state_<N>.sqlite#threads` (`id, cwd, title, first_user_message, updated_at_ms, created_at_ms`) and `thread_spawn_edges(parent, child)` (the status column is **not** read). Walks `sessions/**/rollout-*.jsonl`: first `session_meta` line plus a 32 KB tail for the preview (Tier 2). Runtime signature: process basenames `codex`, `codex-rs`; fd patterns `/.codex/sessions/`, `/.codex/tmp/`. |
| Log linker | `src/discovery/codex_log.rs`, ADR 0048 | `logs_<N>.sqlite`: `SELECT thread_id, process_uuid, ts` where `process_uuid LIKE 'pid:<P>:%'`. Maps a live pane pid to its current thread and ranks 55, above the fd and hook kinds. Never selects `feedback_log_body`. |
| Transcript viewer | `src/viewer/parser/codex.rs` | Knows the outer record types `session_meta`, `turn_context`, `response_item`, `event_msg`, `compacted`. Uses `event_msg` `turn_aborted` and `task_complete` for abort marking. |
| Hook writer | `src/cli/hook.rs` `hook write codex` | Reads stdin JSON. The default `hook_record_from_payload` (`src/discovery/harness/mod.rs:336`) keeps `session_id`, `cwd`, `transcript_path`, `hook_event_name`, plus pid/ppid (from a parent walk to `codex`) and tmux context. Sends to the daemon (`hook_ingest`) or falls back to the spool `hooks-latest.json` (ADR 0146). |
| Hook installer | `src/cli/hook.rs` `hook init/status/remove codex` | Edits `$CODEX_HOME/config.toml` (default `~/.codex/config.toml`), or `.codex/config.toml` under `--scope project`. Appends `hooks.SessionStart = [{ hooks = [{ type = "command", command = "<exe> hook write codex", async = false }] }]`. No matcher, no timeout. ADR 0143 category 5. |
| Fixtures | `tests/fixtures/codex/rollout-{minimal,full,forked}.jsonl`, `tests/fixtures/hooks/codex-sessionstart.json` | `rollout-full` has `task_started`, `exec_command_end` (an older-version shape; see §2.3), and `task_complete`. The hook fixture has `session_id, transcript_path, cwd, hook_event_name, model, permission_mode, source`. |
| CSP-228 audit (Done) | backlog | On Codex 0.128.0: hooks under `[hooks]` with PascalCase keys. Events available then: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Stop`. Command hooks had to be synchronous (`async = true` was parsed and skipped with a warning). The hook env included `TMUX` and `TMUX_PANE`. Do **not** trust an inherited `CODEX_THREAD_ID`. |
| CSP-268 (To Do) | backlog | The umbrella "detect session live status" story. It needs an ADR on where status lives and on polling cost. The 2026-10-31 note says new colors must be framework theme roles, and so on. |
| Spool semantics | `src/hook.rs` | Latest record per mux key `tmux:<socket>:<session_name>` (no pane), else `cwd:`, else `session:`. `ACTIVE_TTL_SECONDS = 15*60`. `observed_epoch` is in seconds. |

What follows from the baseline:

- The write path (`hook write codex`) already accepts any Codex hook event,
  because `session_id` is common to all of them. Live state needs three
  things the writer lacks today:
  - a per-event derived state with extra metadata fields (`turn_id`,
    `tool_name`, `agent_id`, `agent_type`, `source`, `trigger`, `status`,
    `permission_mode`);
  - millisecond timestamps;
  - a per-pane key.
- The installer must learn the other events.

## 2. Signal inventory

### 2.1 Lifecycle hooks (primary)

Config lives in `~/.codex/hooks.json`, `~/.codex/config.toml` `[hooks]`,
`<repo>/.codex/hooks.json`, `<repo>/.codex/config.toml`, plugin hook files,
and managed `requirements.toml`. Hooks from all layers run and none replaces
another. Project hooks run only if the project `.codex/` layer is trusted
[S1][S2]. The feature flag `[features] hooks` is on by default;
`codex_hooks` is a deprecated alias [S1][S3].

The structure is event, then matcher group (`matcher` regex), then handlers
(`type = "command" | "mcp_tool"`, `command`, `timeout` in seconds with a
default of 600, `statusMessage`, `async`, `additionalContextLimit`) [S1][S3].
Handlers run as `$SHELL -lc <command>` (from the session env snapshot, else
`/bin/sh`), in their own process session, with stdin JSON. stdout and stderr
are captured. On timeout the hook's process group is killed [S11].

Common stdin fields: `session_id` (subagent hooks use the **parent's** id),
`transcript_path` (string or null; "not a stable interface"), `cwd`,
`hook_event_name`, `model`, and `permission_mode` on most events (`default |
acceptEdits | plan | dontAsk | bypassPermissions`) [S1][S8].

| Event | When | Live-state fields (metadata) | Payload fields (must not persist) | Matcher on |
|---|---|---|---|---|
| `SessionStart` | session begins; sources `startup, resume, clear, compact, fork` [S8a]. The exact firing time relative to the first prompt is **UNVERIFIED**; it may run lazily at the first turn. | `source`, `session_id`, `transcript_path` | (none) | `source` |
| `UserPromptSubmit` | the operator submits a prompt | `turn_id` | `prompt` | ignored |
| `PreModelRequest` | each model request attempt, retries included | `turn_id`, `model`, `agent_id`/`agent_type` (optional) | (none) | ignored |
| `PostModelRequest` | after each attempt | `status = completed \| failed \| cancelled`, `token_usage` (ints) | `error` (string, may carry server text) | ignored |
| `PreToolUse` | before a tool runs. Shell and `exec_command` count as `Bash`; `apply_patch` also matches `Edit` and `Write`; MCP tools as `mcp__srv__tool`; other local tools by name; `spawn_agent` also as `Agent`. Hosted tools such as WebSearch are not covered [S1]. | `turn_id`, `tool_name`, `tool_use_id` | `tool_input` | tool name |
| `PermissionRequest` | only when Codex is about to prompt for approval (shell escalation, managed network) [S1]. It runs **before** the approval UI shows [S10]. | `turn_id`, `tool_name`, `agent_id`/`agent_type` (optional; set for subagent approvals) [S8b] | `tool_input` (incl. `description`) | tool name |
| `PostToolUse` | after the tool, including Bash that exits non-zero | `turn_id`, `tool_name`, `tool_use_id` | `tool_input`, `tool_response` | tool name |
| `PreCompact` / `PostCompact` | around compaction | `turn_id`, `trigger = manual \| auto` | (none) | `trigger` |
| `SubagentStart` | a subagent begins | `turn_id`, `agent_id`, `agent_type` [S8c] | (none) | `agent_type` |
| `SubagentStop` | a subagent's turn or run ends; per-turn or per-lifetime is **UNVERIFIED** | `turn_id`, `agent_id`, `agent_type`, `stop_hook_active` | `last_assistant_message`; `agent_transcript_path` is a path, so it is metadata | `agent_type` |
| `Stop` | main-thread turn ends | `turn_id`, `stop_hook_active`, `permission_mode` | `last_assistant_message` [S8d] | ignored |
| `Interrupt` | the operator interrupts an active turn (main thread only) | `turn_id`, `permission_mode` | (none) | ignored |
| `SessionEnd` | main thread ends: normal close, archive/delete, or 30 min idle and open in no client. Not for subagents. Timeout defaults to 1 s, at most 3 s. Always synchronous. | `reason` (currently `other`) | (none) | `reason` |

Decision control: this is the safety-critical part, because Conspectus must
only observe.

- **Exit 0 with no stdout is success with no decision.**
- `PermissionRequest`:
  - exit 2 with stderr text means **deny**;
  - JSON `hookSpecificOutput.decision.behavior` can allow or deny;
  - any other exit code is `Failed` with no decision, and the normal prompt
    appears [S10].
- `PreToolUse`: exit 2 blocks the tool.
- `Stop`/`SubagentStop`: exit 2 forces the turn to **continue**, and plain
  stdout is invalid.
- `SessionStart`/`UserPromptSubmit`: plain stdout becomes **developer
  context** for the model [S1].

So the writer must never print to stdout and never exit 2. `anyhow` errors
exit 1, which is safe. One residual risk: hooks run under a login shell
(`-lc`), so a noisy login profile that prints to stdout would inject text
into the model's context on `SessionStart`/`UserPromptSubmit`. Conspectus
cannot control that.

`async = true` (command hooks only):

- runs the hook in the background;
- "can't block, approve, rewrite";
- at most 8 run concurrently per session;
- completion order is not guaranteed;
- unfinished hooks are cancelled when the session ends [S1][S11].

`SessionEnd` ignores `async`. On 0.128.0, `async = true` was skipped with a
warning (CSP-228). The version where async started working is
**UNVERIFIED**.

Trust:

- "Non-managed hooks are skipped until reviewed and trusted." Trust is
  recorded against the hook's hash and reviewed in `/hooks`.
- `--dangerously-bypass-hook-trust` applies per invocation [S1][S24].
- In source:
  - Trust state is `HookStateToml { enabled, trusted_hash,
    trusted_content_hash }`. A test fixture nests it under `hooks.state`, so
    the full key path is probably `hooks.state.<key>` (**UNVERIFIED**).
  - The key is `"{source_path}:{event_label}:{group_index}:{handler_index}"`.
  - The hash covers event, effective matcher, command, timeout, `async`,
    `statusMessage`, and `additionalContextLimit`.
  - Untrusted hooks are dropped **silently** at dispatch. The docs say a
    startup warning points at `/hooks` [S1][S12][S12a].

Timeline (partial):

| Version | Change |
|---|---|
| 0.128.0 | 6 events, sync only (CSP-228) |
| 0.133.0 (21 May 2026) | adds `SubagentStart`/`SubagentStop` and "compact SessionStart" [S31] |
| 0.155.0 (17 Sep 2026) | "Detach Unix hook commands from the controlling terminal" [S32] |
| 0.158.0 | "native POSIX spawning for command hooks" [S32] |

The release that introduced `Pre`/`PostModelRequest`, `Pre`/`PostCompact`,
`Interrupt`, `SessionEnd`, hook trust, and `hooks.json` is **UNVERIFIED**:
the release search found no notes for `PreModelRequest` [S33].

### 2.2 `notify` (legacy external program)

- Config is `notify = ["prog", "arg"]` in the **user** `config.toml` only.
  Project config ignores it [S2][S4].
- The only event is `agent-turn-complete` [S2][S9].
- Payload: one JSON argument appended as the **last argv**. Keys:
  - `type = "agent-turn-complete"`
  - `thread-id`
  - `turn-id`
  - `cwd`
  - `client` (optional)
  - `input-messages` (payload)
  - `last-assistant-message` (payload) [S9]
- Delivery is fire-and-forget: `spawn()` with stdin, stdout, and stderr set
  to null, and the result is never awaited. The env comes from the "session
  snapshot" [S9].
- It is a single command, so it does not compose. Using it means replacing
  or wrapping the operator's program, which would mean rewriting a setting
  the operator owns. It adds nothing that the `Stop` hook does not already
  provide. **Do not use.**
- The TUI's built-in `tui.notifications` (types `agent-turn-complete`,
  `approval-requested`, `plan-mode-prompt`, `async-question`) go to the
  terminal as OSC 9 or BEL. The default condition is `unfocused` [S2][S4][S23].
  With `notification_method = "bel"`, tmux would set `window_bell_flag`
  (`monitor-bell` [S29]). That is weak and depends on operator config. Not
  recommended.

### 2.3 Rollout JSONL (`$CODEX_HOME/sessions/YYYY/MM/DD/rollout-<ts>-<id>.jsonl`)

Every line is `{"timestamp", "type", "payload"}` with outer `type` one of:

- `session_meta`
- `turn_context`
- `response_item`
- `event_msg`
- `compacted`

`world_state` may also appear (**UNVERIFIED**) (repo fixtures and parser;
[S5]). `session_meta` has `id`, `forked_from_id`, `parent_thread_id`, `cwd`,
`originator`, `cli_version`, `source` (`cli | vscode | exec | mcp |
subagent{thread_spawn{parent_thread_id, depth, agent_path, agent_nickname,
agent_role}} | ...`), `history_mode`, and more [S5].

What the policy persists [S6] (`codex-rs/rollout/src/policy.rs`; the history
modes are `Legacy` and `Paginated`):

| `event_msg` type | Persisted | Live-state use |
|---|---|---|
| `task_started` (`TurnStarted`: `turn_id`, `started_at`, `collaboration_mode_kind`, `model_context_window`) | both modes | turn opened, so WORKING or WAITING |
| `task_complete` (`TurnComplete`: `turn_id`, `completed_at`, `duration_ms`, `error: Option<ErrorEvent{message, codex_error_info}>`, `last_agent_message`) | both | turn closed, so READY; `error.codex_error_info` gives ERROR/RATE_LIMIT |
| `turn_aborted` (`turn_id`, `reason = interrupted \| replaced \| review_ended \| budget_limited`, `error`) | both | READY (interrupted) or ERROR |
| `token_count` (`info`, `rate_limits{primary/secondary{used_percent, resets_at}, rate_limit_reached_type, credits...}`) | both | rate-limit or usage-limit flag, with no payload |
| `thread_goal_updated`, `thread_settings_applied`, `thread_rolled_back` | both | none |
| `item_completed` | Legacy: only `FunctionCallOutput`, `Plan`, `Sleep`, and SubAgentActivity `Completed`. Paginated: all | `Plan` item means a plan was proposed (plan prompt likely) |
| `user_message`, `agent_message`, `agent_reasoning*`, `patch_apply_end`, `context_compacted`, `mcp_tool_call_end`, `web_search_end`, `entered/exited_review_mode` | Legacy only | coarse activity, mostly payload |
| `exec_approval_request`, `apply_patch_approval_request`, `request_permissions`, `request_user_input`, `elicitation_request`, `guardian_assessment`, `error`, `warning`, `stream_error`, `exec_command_begin/end`, `patch_apply_begin`, `mcp_tool_call_begin`, `collab_*`, `session_configured`, `shutdown_complete`, `hook_started/completed`, all deltas | **never** | n/a: WAITING, mid-turn errors, and exit are invisible here |

Notes:

- `response_item` records give tool-call structure: `function_call`,
  `custom_tool_call` (e.g. `apply_patch`), and `web_search_call`, each with
  `name` and `call_id`; plus `function_call_output` and
  `custom_tool_call_output`. These come from the parser fixtures in
  `src/viewer/parser/codex_tests.rs`. The tool **name** is metadata; the
  arguments and outputs are payload.
- The repo fixture `rollout-full.jsonl` contains `event_msg`
  `exec_command_end`. The current policy never persists that event, so the
  fixture reflects an older Codex.
- A test treats a missing `history_mode` as `Legacy` [S7]. Whether new
  threads default to `Paginated` is **UNVERIFIED**. The `threads` table has
  had a `history_mode` column since migration 0040 [S13].

### 2.4 `codex exec --json`

`--json` prints JSONL on the exec process's own stdout [S24]. The events
are:

- `thread.started{thread_id}`
- `turn.started`
- `turn.completed{usage}`
- `turn.failed{error}`
- `item.started` / `item.updated` / `item.completed{item}`, where item types
  include `command_execution{status: in_progress|completed|failed|declined}`,
  `file_change`, `mcp_tool_call`, `collab_tool_call{agents_states{status:
  pending_init|running|interrupted|completed|errored|shutdown|not_found}}`,
  `todo_list`, and `error`
- `error` [S25]

Only the process that launched `exec` can read this stream. It helps only
for runs Conspectus itself launches (ADR 0143 category 4). Hooks fire in
exec mode too (CSP-228).

### 2.5 State SQLite (`state_<N>.sqlite`)

- `threads` (62 migrations, through 0062 [S13]) has **no status or running
  column** [S14a]. It has `updated_at(_ms)`, `recency_at(_ms)` (0039), and
  `tokens_used`, and the struct has no live field. When `updated_at` is
  bumped is **UNVERIFIED**.
- `thread_spawn_edges(parent_thread_id, child_thread_id PK, status)` is
  indexed on `(parent_thread_id, status)`. Status is
  `DirectionalThreadSpawnEdgeStatus` with values `open | closed`
  [S13a][S14]. The exact moment an edge flips to `closed` is
  **UNVERIFIED**.
- Use: `SELECT count(*) FROM thread_spawn_edges WHERE parent_thread_id = ?
  AND status = 'open'` gives open subagents. It is cheap, read-only, and
  carries no payload. Conspectus reads this table today but ignores
  `status`.

### 2.6 Logs SQLite (`logs_<N>.sqlite`)

The writer records:

- `ts`, `ts_nanos`, `level`, `target`, `module_path`, `file`, `line`
- `thread_id`, `process_uuid` (`pid:<pid>:<uuid>`)
- `message` and `feedback_log_body`, which are payload [S27]

Writer behavior:

- The filter defaults to TRACE, with some targets capped.
- Batches are written at 512 entries or every **10 s**.
- When the queue (2048) is full, entries are dropped [S27].

Use: the newest `ts` per `thread_id` and pid is a coarse "recently busy"
heartbeat, with up to 10 s of latency. Whether an idle session keeps logging
is **UNVERIFIED**. Inferring state from `target` module names is
undocumented and brittle. Keep this to a heartbeat at most; never select
`message` or `feedback_log_body`.

### 2.7 App-server protocol (control plane)

- `codex app-server --listen stdio:// | ws://IP:PORT | unix:// |
  unix://PATH` (Experimental). TUIs can attach to it with `codex --remote
  ws://… | unix://…` [S24]. By default the TUI embeds an **in-process**
  app-server, so there is no external socket [S17].
- `thread/status/changed{threadId, status}` [S15][S18]. `ThreadStatus` is
  `notLoaded | idle | systemError | active{activeFlags: [waitingOnApproval,
  waitingOnUserInput]}`.
- `thread/list` returns `status`, `updatedAt`, `recencyAt`, `source`, and
  `ephemeral` [S16]. `thread/loaded/list` returns in-memory threads [S18].
- `TurnStatus` is `completed | interrupted | failed | inProgress` [S19b].
- `ErrorNotification{threadId, turnId, willRetry, error{codexErrorInfo}}`.
  `codexErrorInfo` values include `usageLimitExceeded`, `rateLimitExceeded`,
  `contextWindowExceeded`, `serverOverloaded`, `unauthorized`,
  `responseStreamDisconnected{httpStatusCode}`, `tooManyDenials`, and
  `other` [S19].
- Risks:
  - Server-initiated approval requests (`item/commandExecution/requestApproval`,
    `mcpServer/elicitation/request`) go to clients [S17][S18]. How they are
    routed across several clients is **UNVERIFIED**. A Conspectus client must
    not look like an approval-capable client (`requiredHumanApproval`).
  - Subscribing (`thread/start`/`resume`) loads threads. `thread/read` does
    not subscribe [S18].
  - ADR 0028 makes this an audit-gated control-plane adapter.
  - It is useful only for operators who opt into a shared app-server.
- The default socket path for `unix://` is **UNVERIFIED** ("Codex's default
  app-server control socket" [S18]).

### 2.8 Terminal surfaces the TUI writes

| Surface | Content | Visible to Conspectus via | Source |
|---|---|---|---|
| Window title (OSC 0, BEL-terminated) | Defaults: `main` uses `["activity", "thread-name", "project-name"]`; the docs say `["spinner", "project"]` (version drift). **Spinner** frames `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` at 100 ms while a task runs or MCP starts up. The prefix **`[ ! ] Action Required`** (alternating with `[ . ] Action Required` when animations are on) shows while the bottom pane needs input; the spinner is suppressed then. Idle shows no activity segment. An optional `status` item gives `Starting / Ready / Working / Waiting / Thinking`. `tui.terminal_title = null` disables the title. Under GNU screen the glyphs fall back to ASCII. | `tmux list-panes -F '#{pane_title}'` (`allow-set-title` pane option; programs set the title with `\e]0;…` or `\e]2;…` [S29]) | [S4][S20][S21] |
| OSC 7501 | `state=idle\|working\|blocked\|done\|error:app=codex[:kind=permission\|question\|auth]` | **not visible.** Likely dropped by tmux, which does not handle unknown OSCs; **UNVERIFIED** | [S22] |
| OSC 21337 (iTerm2) | Idle / Working / Waiting | suppressed under a multiplexer | [S22] |
| OSC 9 / BEL notifications | approval-requested and similar | BEL becomes `window_bell_flag` only if the operator chooses `bel` | [S2][S23] |

The title is the strongest hook-free signal, with these limits:

- `tui.animations = false` or the progress effect off means no spinner,
  though Action Required still shows.
- A custom `tui.terminal_title` without the spinner or activity item, or
  `null`, means no signal.
- After a crash the title is stale. Pair it with `pane_current_command` and
  pid liveness.
- Thread and project names in the title are operator-visible text. Classify
  them and drop them; never persist.

### 2.9 OpenTelemetry

Codex can export `codex.tool_decision`, `codex.sse_event`,
`approval.requested`, `turn.e2e_duration_ms`, and more via `[otel]`
[S2][S4b]. Using them would require Conspectus to edit harness config beyond
its hook entry and to run an OTLP receiver (a new listener). That breaks the
ADR 0143 envelope. Rejected.

### 2.10 Process and tmux metadata (already collected)

- `pane_current_command`, `pane_pid`, and the process tree (ADR 0046) give
  liveness: Codex present means STARTING, READY, or busy; Codex gone means
  EXITED.
- `/proc/<pid>/fd` points at the open rollout (CSP-228).
- `session_activity` is already in `TMUX_LIST_FORMAT`. Adding
  `#{pane_title}` there costs no new subprocess.

## 3. Signal-to-state mapping

Legend: **C** means certain (documented, direct), **A** means ambiguous
(needs a second signal), and **U** means undetectable by that channel.

| Canonical state | Hooks | Rollout tail | pane_title | Other |
|---|---|---|---|---|
| STARTING | **A**: no record yet. `SessionStart` may be lazy (**UNVERIFIED**). | **U**: rollout may not exist yet (ephemeral, or before the first message) | **A**: spinner with no turn means MCP startup; the `Starting` text appears only with the `status` item | pid alive and no session yet |
| READY (turn done, idle) | **C**: `Stop` (main thread), `Interrupt`, `SessionStart(startup\|resume\|clear\|fork)` before any prompt | **C**: last boundary is `task_complete` or `turn_aborted` | **C** (with defaults): no spinner, no Action Required | `notify` `agent-turn-complete` (not used) |
| WORKING (model / tools) | **C**: `UserPromptSubmit`, `PreModelRequest`, `PreToolUse`, `PostToolUse`, `PostCompact` mid-turn | **A**: `task_started` with no close. It may be WAITING. | **C**: spinner frame present | logs heartbeat (10 s lag) |
| WAITING: command or patch approval | **C** on entry: `PermissionRequest` (`tool_name` `Bash` means command; `apply_patch`/`Edit`/`Write` means patch; `mcp__…` means MCP). **A** on exit: no hook fires on the decision. The next `PostToolUse`, `PreModelRequest`, `Stop`, or `Interrupt` clears it, so a long approved command still reads WAITING until it finishes. | **U**: never persisted. A pending `function_call` looks the same as a running tool. | **C** on entry and exit: Action Required, but no kind | OSC 7501 `blocked:permission` (not visible) |
| WAITING: network approval | **C**: `PermissionRequest` for managed network [S1] | **U** | **C** | |
| WAITING: question (`request_user_input`) | **A**: `PreToolUse` with `tool_name = request_user_input` (tool-hook coverage for it is **UNVERIFIED**). Not a `PermissionRequest`. | **A**: pending `function_call` named `request_user_input` (name **UNVERIFIED**) | **C** | `async-question` TUI notification |
| WAITING: MCP elicitation | **U** (no hook) | **U** | **C** | |
| WAITING: plan approval | **A**: `Stop` with `permission_mode = plan` (a plan *may* have been proposed; reading `last_assistant_message` for `<proposed_plan>` would be a payload read) | **A**: `item_completed` with item type `Plan` before `task_complete`; `task_started.collaboration_mode_kind` = plan (wire value **UNVERIFIED**) | **C**: Action Required when the plan popup is up (**UNVERIFIED** that it counts as bottom-pane input) | `plan-mode-prompt` notification type |
| WAITING: auth | **U** | **U** | **A** | OSC 7501 `blocked:auth` (not visible) |
| SUBAGENTS ACTIVE | **C**: set of `agent_id` from `SubagentStart` minus `SubagentStop`. Subagent `PermissionRequest` carries `agent_id`, so the WAITING belongs to a child. | **A**: parent has a pending `wait_agent`/`spawn_agent` call (tool names from [S4]); child rollouts with `source.subagent` and open turns | **U** | **C**: `thread_spawn_edges` `status = 'open'` count (DB reads have their own latency) |
| COMPACTING | **C**: `PreCompact` to `PostCompact` (`trigger` manual or auto) | **A**: only the end shows (`compacted` record, plus `context_compacted` in Legacy). The start is invisible. | **U**: no compaction text in the title | |
| ERROR / rate limit | **A**: `PostModelRequest status = failed` with retries possible. The class is in the `error` string (payload-ish), and `Stop` may follow. | **C** at turn end: `task_complete.error.codex_error_info` or `turn_aborted.error`; `token_count.rate_limits.rate_limit_reached_type != null` | **U** | `ErrorNotification.willRetry` (app-server only) |
| EXITED | **C** on clean exit: `SessionEnd` (main thread). **U** on crash or kill. | **A**: open turn with no close means it crashed mid-turn, if the pid is gone | **A**: title stale | **C**: pid or `pane_current_command` gone (ADR 0046) |

The hard limits:

1. With hooks, the exit from an approval wait is late by the length of the
   approved tool's run. Only the title is prompt.
2. MCP elicitation and auth waits have no hook. Only the title shows them,
   and without a kind.
3. Nothing distinguishes "plan proposed" from "turn ended in plan mode"
   without a payload peek or the `Plan` item.
4. Crashes leave no event. Liveness must come from the process.

## 4. Delivery and install

### 4.1 Where the config lives

- User scope: `$CODEX_HOME/config.toml` (default `~/.codex/config.toml`) or
  `$CODEX_HOME/hooks.json`.
- Project scope: `<repo>/.codex/config.toml` or `hooks.json`. These load
  only for trusted projects.
- Plugins: `hooks/hooks.json`.
- Managed: `requirements.toml`, where `allow_managed_hooks_only = true`
  disables everything else [S1][S3][S26].
- Conspectus's `codex_config_path` already honors `CODEX_HOME`.

### 4.2 Composition with the operator's own hooks

- Hooks compose. All matcher groups and all layers run; multiple matching
  handlers for one event start concurrently [S1]. Conspectus appends its own
  matcher group per event and never edits the operator's groups. That fits
  ADR 0143 category 5 as is.
- `notify` does not compose (§2.2). Leave it alone.
- If a layer has both `hooks.json` and inline `[hooks]`, Codex loads both and
  **warns at startup** [S1][S2]. If the operator uses `~/.codex/hooks.json`,
  Conspectus writing inline `[hooks]` in `config.toml` triggers that warning
  on every launch. Writing a separate Conspectus-owned file instead is not
  possible: the hook file name is fixed (`hooks.json`) per layer. Options:
  - edit `hooks.json` when it exists;
  - accept the warning;
  - ship as a Codex plugin (plugins may bundle `hooks/hooks.json` [S1]). This
    resembles the Pi and opencode extension approach (ADR 0121, ADR 0049) and
    would need its own ADR.

### 4.3 Trust

- New or changed unmanaged hooks run only after the operator approves them
  in `/hooks`.
- The trust key includes the source path, event, **group index**, and
  handler index. The hash covers command, matcher, timeout, `async`, and
  `statusMessage` [S12][S12a].

Consequences:

- `hook init codex` must say, in operator-facing text with no ADR or doc
  citations, that Codex will ask the operator to review the new hooks with
  `/hooks`.
- Conspectus must **not** write `trusted_hash` itself. That would bypass a
  harness security control and mutate harness state beyond the hook entry
  (ADR 0143 prohibitions 1 and 7).
- The entry's shape (command, timeout, async, events) should be fixed once.
  Every change forces a re-review.
- `hook remove` deletes a group, which **renumbers later groups**. The
  operator's hooks added after Conspectus's then have new trust keys and are
  likely to need review again. This is inferred from `hook_key`
  (**UNVERIFIED** in practice).
- `hook status codex` could check, read-only, whether a hook-state entry
  exists for its key. A better health check: "Codex panes are active, but no
  Codex hook record arrived in the last N minutes" means the hooks are
  untrusted or disabled.

### 4.4 Environment and mux attribution

- The hook env is the session's captured environment snapshot replayed into
  an empty env, minus "non-inheritable" names [S11]. Which names count as
  non-inheritable is **UNVERIFIED**.
- CSP-228 observed `TMUX` and `TMUX_PANE` in the hook env on 0.128.0.
- Since 0.155.0, hooks are detached from the controlling terminal [S32].
  `SystemTmux::current_session_context` runs `tmux display-message -p
  '#{pane_id}'` **without `-t "$TMUX_PANE"`**. It therefore relies on tmux
  falling back to `$TMUX_PANE` when the client has no tty (**UNVERIFIED**).
  In a multi-pane session it could record the session's active pane instead.
  Recommendation: read `TMUX_PANE` directly and pass `-t`.
- `codex --remote` against a shared app-server (UNVERIFIED): hooks probably
  run in the app-server's environment. That is the opencode 2.x
  shared-service case, so the record should be `background_harness = true`.
  Detect it from the harness command line (`codex app-server`).
- Do not use `CODEX_THREAD_ID` from the env (CSP-228).
- The pid walk to the `codex` process still works. `NewSession` changes the
  session id, not the parent.

### 4.5 Latency and reliability

- **Latency**: the hook spawn costs `$SHELL -lc` startup plus a Conspectus
  start, about tens of ms (UNVERIFIED; depends on the login profile). Then
  the daemon `hook_ingest` updates the live graph immediately, or the writer
  writes the spool and the next refresh picks it up.
  - Synchronous `PermissionRequest` hooks delay the approval prompt by that
    amount, because they run before the UI [S10].
  - Synchronous `PreToolUse`/`PostToolUse` delay every tool call.
  - Use `async = true` where the Codex version honors it, and a short
    `timeout` (for example 5 s) everywhere. **The current install has no
    timeout, so the default is 600 s.**
  - `SessionEnd` is capped at 1 to 3 s.
- **Ordering**: async hooks finish in no guaranteed order [S1], and the
  payloads carry no timestamp. Order events by `turn_id` (a UUIDv7, so
  time-ordered [S19b]), then by writer receipt time in **ms**. Within a turn,
  let closing events win: `Stop` or `Interrupt` beats a late `PostToolUse`
  from the same `turn_id`.
- **Loss**:
  - A crash or kill fires no `SessionEnd`.
  - Unfinished async hooks are cancelled at session end.
  - Without a daemon, two hooks race on the spool's read-modify-write
    (ADR 0146). High-frequency events make that likelier.
  - Hooks that are untrusted, disabled (`features.hooks = false`), or
    managed-only fail silently.
- **Spool keying**: `mux_record_key` is per tmux *session*. Two Codex panes
  in one tmux session overwrite each other. Live state must key per pane
  (`socket:pane_id`), or per `(harness, session_id, agent scope)`.

### 4.6 Existing install path: defects found by reading the code

These come from reading `src/cli/hook.rs`. Confirm each with a test before
fixing.

1. **Clobbers `[[hooks.SessionStart]]`.** `ensure_codex_hook` takes
   `hooks_table.entry("SessionStart")`. An operator's array of tables (the
   TOML form Codex documents [S2]) is `Item::ArrayOfTables`, so
   `is_array()` is false. The installer replaces it with an empty inline
   array and pushes its own entry, deleting the operator's hooks. Per the
   `toml_edit` docs, `is_array_of_tables` is separate [S34].
2. **Clobbers an inline `hooks = {…}`.** `is_table()` is false for an
   `InlineTable` [S34], so the whole `hooks` value is replaced.
3. `has_codex_hook` and `remove_codex_hook` see only the inline-array form.
   They miss array-of-tables entries and `hooks.json`.
4. No `timeout` (default 600 s) and `async = false`.
5. Trust is not mentioned in `hook init` output or `docs/operations.md`.
6. `harness_version_env` has no `codex` mapping. `session_meta.cli_version`
   and `threads.cli_version` exist.
7. Fixture nit: `tests/fixtures/hooks/codex-sessionstart.json` has
   `"model": "claude-sonnet-4-20250514"`.

## 5. Hook-free fallbacks

### 5.1 Rollout tail (live sessions only: a pid holds the rollout open, or a log or hook match)

Algorithm:

1. Read the last 32 to 64 KB.
2. Deserialize **metadata-only** structs. Allowed fields:
   - `type`, `timestamp`
   - `payload.type`, `payload.name`, `payload.call_id`, `payload.turn_id`,
     `payload.reason`
   - `payload.error.codex_error_info`
   - `payload.rate_limits.rate_limit_reached_type`
   - `payload.collaboration_mode_kind`
   - `payload.item.type`

   Do not bind text fields (`content`, `arguments`, `output`, `message`,
   `last_agent_message`).
3. Find the last turn boundary, and the open `call_id`s after the last
   `task_started`.

| Tail shape | Inference | Accuracy |
|---|---|---|
| last boundary `task_complete` with no `error` | READY | high |
| `task_complete` with `error.codex_error_info` | READY with last error (usage or rate limit, etc.) | high at turn end; mid-turn retries invisible |
| `turn_aborted` with `reason = interrupted` | READY (interrupted) | high |
| `task_started`, open `function_call`/`custom_tool_call`, mtime still | tool running **or** approval pending | **ambiguous**: `exec_command_output_delta` is never persisted, so mtime stands still during long commands too |
| `task_started` with open call `request_user_input` | WAITING (question) | medium (tool name UNVERIFIED) |
| `task_started` with open `wait_agent` / `spawn_agent` | SUBAGENTS ACTIVE | medium |
| `item_completed{Plan}` then `task_complete` | READY, plan proposed (likely plan prompt) | medium (shape UNVERIFIED) |
| `token_count.rate_limits.rate_limit_reached_type` set | RATE LIMITED flag | high |
| `task_started` with no close, codex pid gone | EXITED (crashed mid-turn) | high |
| `task_started`, mtime advancing | WORKING | high |

`turn_context` records carry `approval_policy` [S7]. When it is `never`,
WAITING(approval) is impossible, which resolves the ambiguous row toward
WORKING. In `Paginated` history mode many `event_msg`s are dropped, but
boundaries and `token_count` remain [S6].

### 5.2 tmux `#{pane_title}` (recommended companion)

Classify a Codex pane's title with these rules (glyphs from [S20][S21]):

- starts with `[ ! ] Action Required` or `[ . ] Action Required`, or the
  same after a leading `● `: WAITING (kind unknown)
- starts with a braille spinner frame (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`), or the ASCII
  screen fallback `| / - \`: WORKING, or STARTING if no turn has been seen
- anything else, with Codex alive in the pane: READY (if the title was
  previously seen to carry Codex patterns), else UNKNOWN

Requires a separate ADR (ADR 0028's carve-out for already-visible status
surfaces). This is not scrollback scraping: it reads one tmux format field.
The polling cost rides on the existing `list-panes` call.

### 5.3 State DB and logs

- `thread_spawn_edges` gives the count of open children (§2.5).
- The logs heartbeat gives "busy within the last 10 to 20 s" (§2.6).

Both are read-only, use `query_only`, and need no new process.

## 6. Recommendation

### 6.1 Minimal signal set

- **Tier A, default when the operator opts into hooks.** One Conspectus
  matcher group per event for `SessionStart`, `UserPromptSubmit`,
  `PermissionRequest`, `Stop`, `Interrupt`, `SessionEnd`, `SubagentStart`,
  `SubagentStop`, `PreCompact`, `PostCompact`, and `PostToolUse` (needed to
  leave WAITING after an approved tool). Each handler is `timeout = 5`. Use
  `async = true` on everything except `SessionEnd` where the installed Codex
  supports it; otherwise use sync, and `PermissionRequest` must stay fast.
- **Tier B, optional (high frequency).** `PreModelRequest`, which leaves
  WAITING promptly after a decline, and `PostModelRequest` (failed status,
  so error hints). Skip `PreToolUse`, except possibly with matcher
  `^request_user_input$` once tool coverage is verified.
- **Hook-free, always on.** Process liveness (existing), `#{pane_title}`
  classification (after its ADR), the rollout tail for live sessions only,
  and `thread_spawn_edges.status`.
- **Not used.** `notify`, OTel, `exec --json` (except for
  Conspectus-launched runs), and the app-server (later audit-gated adapter).

### 6.2 Event-to-state transitions (per `session_id`; subagent events update the child set)

| From | Input | To |
|---|---|---|
| (none) | codex pid seen in pane, no record | STARTING (title spinner) / READY (no spinner) / UNKNOWN |
| any | `SessionStart` source `startup`, `resume`, `clear`, or `fork` | READY (turn closed) |
| any | `SessionStart` source `compact` | keep: WORKING if a turn is open, else READY |
| READY, STARTING | `UserPromptSubmit` | WORKING (open turn = `turn_id`) |
| any open-turn state | `PreModelRequest` / `PreToolUse` / `PostToolUse` / `PostModelRequest(completed)` | WORKING |
| WORKING | `PermissionRequest` (no `agent_id`) | WAITING(approval: command / patch / mcp / other by `tool_name`) |
| WORKING | `PermissionRequest` (with `agent_id`) | WORKING plus child WAITING; display as "subagent waiting" |
| WORKING | `PreToolUse(request_user_input)` (if verified) | WAITING(question) |
| WAITING | `PostToolUse` / `PreModelRequest` / title loses Action Required for 2 polls | WORKING |
| WORKING | `PreCompact` | COMPACTING |
| COMPACTING | `PostCompact` | WORKING (open turn) / READY |
| any | `SubagentStart(agent_id)` / `SubagentStop(agent_id)` | add to / remove from the active-children set; SUBAGENTS ACTIVE when non-empty (overlay on WORKING or READY) |
| any open turn | `PostModelRequest(failed)` | WORKING with an error hint (retry likely) |
| any open turn | `Stop` | READY (with `permission_mode = plan`: READY, plan mode) |
| any open turn | `Interrupt` | READY (interrupted) |
| any | rollout `task_complete.error` or `rate_limit_reached_type` | ERROR / RATE LIMITED annotation on READY |
| any | `SessionEnd` | EXITED |
| any | codex pid dead or pane command changed | EXITED (crashed if a turn was open) |

### 6.3 Staleness rule

- Hook live state is valid **while the recorded codex pid is alive**.
  Guard against pid reuse by comparing the pid's start time with the
  record's time; ADR 0048 already defers `starttime`.
- With no pid, fall back to the existing 15-minute TTL: past it, any state
  becomes UNKNOWN.
- Never age out WAITING or READY by time while the pid lives. The operator
  may be away for hours. Clear them only by a later event, by the title, or
  by exit.
- Show WORKING with no event for more than 10 minutes as "working (quiet)".
  If the title shows Action Required, prefer WAITING.
- Hook health: Codex pane active (title spinner or rollout mtime moving)
  with no hook record for more than 2 minutes means "hooks inactive"
  (untrusted or disabled). Degrade to the hook-free path and surface a
  diagnostic.

### 6.4 Privacy handling

- `hook write codex` parses stdin in memory and extracts only the metadata
  fields in §2.1's live-state column.
- It never persists, forwards to the daemon, or logs `prompt`,
  `last_assistant_message`, `tool_input`, `tool_response`, or the
  `PostModelRequest` `error` string. Reduce `error` to the `status` enum
  only.
- Tests must assert, with sentinel strings in the fixtures, that no payload
  reaches the spool or the ingest message.

### 6.5 Fixture plan (shapes by name; all synthetic)

Hook stdin, under `tests/fixtures/hooks/`, each with sentinel payload text:

- `codex-sessionstart-{startup,resume,compact,fork}.json`, plus
  `codex-sessionstart-ephemeral.json` (`transcript_path: null`)
- `codex-userpromptsubmit.json` (`prompt` sentinel)
- `codex-permissionrequest-bash.json`,
  `codex-permissionrequest-apply-patch.json`,
  `codex-permissionrequest-mcp.json`, and
  `codex-permissionrequest-subagent.json` (`agent_id`, `agent_type`)
- `codex-posttooluse-bash.json` (`tool_response` sentinel)
- `codex-pretooluse-request-user-input.json` (pending verification)
- `codex-stop.json` and `codex-stop-plan-mode.json`
  (`last_assistant_message` sentinel)
- `codex-interrupt.json`
- `codex-sessionend.json` (`reason: other`)
- `codex-subagentstart.json` and `codex-subagentstop.json`
- `codex-precompact-auto.json` and `codex-postcompact-manual.json`
- `codex-premodelrequest.json` and `codex-postmodelrequest-failed.json`
  (`status: failed`, `error` sentinel, `token_usage: null`)
- `codex-unknown-event.json` (a future `hook_event_name`, ignored
  gracefully)

Rollout tails, under `tests/fixtures/codex/`:

- `rollout-live-working.jsonl`: `task_started`, user `message`, open
  `function_call`
- `rollout-live-ready.jsonl` and `rollout-live-aborted.jsonl`
  (`turn_aborted{reason: interrupted}`)
- `rollout-live-usage-limit.jsonl` (`task_complete.error.codex_error_info`;
  the snake_case value is UNVERIFIED) and `rollout-live-rate-limited.jsonl`
  (`token_count.rate_limits.rate_limit_reached_type`)
- `rollout-live-request-user-input.jsonl`, `rollout-live-wait-agent.jsonl`,
  and a child `rollout-subagent-child.jsonl` (`session_meta.source.subagent`)
- `rollout-live-plan-proposed.jsonl` (`item_completed{item.type: Plan}`)
- `rollout-live-compacted.jsonl`
- `rollout-paginated.jsonl` (`history_mode: paginated`) and
  `rollout-legacy-exec-end.jsonl` (the old shape)
- `rollout-live-approval-never.jsonl` (`turn_context.approval_policy:
  never`)

Other:

- tmux titles (string table in a unit test):
  - `"⠋ fix-auth | conspectus"`
  - `"[ ! ] Action Required fix-auth | conspectus"`
  - `"[ . ] Action Required …"`
  - `"● ⠙ …"`
  - `"fix-auth | conspectus"`
  - `""`
  - `"| fix-auth"` (screen)
  - a custom operator title
- State DB: `thread_spawn_edges` rows with `open` and `closed`.
- TOML install cases:
  - operator `[[hooks.SessionStart]]` array of tables
  - inline `hooks = {…}`
  - an existing `hooks.json` beside `config.toml`
  - a `hooks.state` table present
  - remove when Conspectus's group sits between operator groups

### 6.6 Manual checks (one live Codex session, private tmux per memory)

1. Install. Confirm the startup `/hooks` review warning and that the hooks
   are silent until trusted.
2. Confirm whether `SessionStart` fires at launch or at the first prompt.
3. Run an approval: see the `PermissionRequest` timing, the title Action
   Required appearing and clearing, and what fires after accept and after
   decline.
4. Spawn subagents: see the `SubagentStart`/`SubagentStop` cardinality and
   when `thread_spawn_edges.status` flips.
5. Trigger `request_user_input` and plan mode: confirm the title and
   `PreToolUse` coverage.
6. Confirm whether quiet hooks show as rows in the Codex TUI (**UNVERIFIED**
   [S28]).
7. Confirm that `tmux display-message` without `-t` picks the hook's pane
   when Codex has several.

## 7. Open questions for the operator

1. **Title signal.** Do you accept reading `#{pane_title}` as an
   already-visible status surface (a new ADR under ADR 0028's carve-out)?
   It is the only prompt exit from WAITING and the only hook-free WAITING.
2. **Hook tier.** Should the install default to Tier A only, or Tier A plus
   B? Should `async = true` be used when the detected Codex version supports
   it, and what `timeout` should apply?
3. **Trust workflow.** Should `hook init codex` only print the `/hooks`
   review instruction (recommended), or also report trust state read-only in
   `hook status`? Never write `trusted_hash`. Confirm.
4. **Representation.** Should the hook live in `config.toml` `[hooks]` even
   when the operator uses `hooks.json` (that triggers a startup warning), or
   should Conspectus edit `hooks.json` when present, or ship a Codex plugin
   bundle (new ADR)?
5. **Payload peeks.** May a state classifier deserialize metadata fields out
   of payload-bearing rollout lines and hook stdin (tool names, call ids)
   without persisting them? Should the "plan proposed" check ever look at
   `last_assistant_message`? I recommend no: use the `Plan` item and
   `permission_mode`. This refines ADR 0086 Tier 1.
6. **Fix the existing installer defects in §4.6 first?** The
   array-of-tables clobber destroys operator hooks.
7. **Spool keying.** Move live state to a per-pane key (and to ms
   timestamps), keeping `hooks-latest.json` latest-per-mux for attribution?
   Or bump the schema to v2?
8. **App-server adapter.** Is a read-only `thread/list` and `status` adapter
   worth an audit for operators who run `codex app-server --listen
   unix://` with `--remote` TUIs?
9. **`notify`.** Confirm Conspectus never touches it.

## Sources

- [S1] Codex Hooks guide: <https://developers.openai.com/codex/hooks> (308 to <https://learn.chatgpt.com/docs/hooks>)
- [S2] Advanced config (notify, tui.notifications, hooks, OTel): <https://developers.openai.com/codex/config-advanced> (to <https://learn.chatgpt.com/docs/config-file/config-advanced>)
- [S3] Config reference (`hooks.<Event>`, `features.hooks`, agents, approval_policy): <https://learn.chatgpt.com/docs/config-file/config-reference>
- [S4] Config reference, second half (`tui.terminal_title`, `tui.status_line`, otel): same URL, offset 100000
- [S4b] Same as [S2] (OTel section)
- [S5] `EventMsg` and protocol types: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/protocol/src/protocol.rs>
- [S6] Rollout persistence policy: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/rollout/src/policy.rs>
- [S7] protocol.rs test tail (history mode default, TurnContextItem): same as [S5], offset 200000
- [S8] PermissionRequest input schema: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/schema/generated/permission-request.command.input.schema.json>
- [S8a] SessionStart input schema: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/schema/generated/session-start.command.input.schema.json>
- [S8b] Same as [S8]; [S8c] SubagentStart input schema: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/schema/generated/subagent-start.command.input.schema.json>
- [S8d] Stop input schema: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/schema/generated/stop.command.input.schema.json>; model-request schema: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/schema/generated/model-request.command.input.schema.json>; schema dir: <https://github.com/openai/codex/tree/main/codex-rs/hooks/schema/generated>
- [S9] Legacy notify: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/src/legacy_notify.rs>
- [S10] PermissionRequest event: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/src/events/permission_request.rs>
- [S11] Command runner (shell, env snapshot, timeout, async cap): <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/src/engine/command_runner.rs>
- [S12] Hook discovery and trust: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/src/engine/discovery.rs>
- [S12a] `hook_key`, `HOOK_EVENT_NAMES`: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/hooks/src/lib.rs>
- [S13] State migrations list: <https://github.com/openai/codex/tree/main/codex-rs/state/migrations>; [S13a] <https://raw.githubusercontent.com/openai/codex/main/codex-rs/state/migrations/0021_thread_spawn_edges.sql>; 0001: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/state/migrations/0001_threads.sql>; 0039: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/state/migrations/0039_threads_recency_at.sql>
- [S14] Spawn edge status enum: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/state/src/model/graph.rs>; [S14a] ThreadMetadata: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/state/src/model/thread_metadata.rs>
- [S15] ThreadStatusChangedNotification schema: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server-protocol/schema/json/v2/ThreadStatusChangedNotification.json>
- [S16] ThreadListResponse schema: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server-protocol/schema/json/v2/ThreadListResponse.json>
- [S17] App-server README: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server/README.md>
- [S18] App-server docs: <https://learn.chatgpt.com/docs/app-server>
- [S19] ErrorNotification schema: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server-protocol/schema/json/v2/ErrorNotification.json>; [S19b] TurnCompletedNotification: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server-protocol/schema/json/v2/TurnCompletedNotification.json>
- [S20] Title composition: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/tui/src/chatwidget/status_surfaces.rs>
- [S21] Title writer (OSC 0, glyph fallbacks): <https://raw.githubusercontent.com/openai/codex/main/codex-rs/tui/src/terminal_title.rs>
- [S22] Program status OSC 7501/21337: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/tui/src/terminal_program_status.rs>
- [S23] TUI notification types: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/tui/src/chatwidget/notifications.rs>
- [S24] CLI reference (`app-server --listen`, `--remote`, `exec --json`, `--dangerously-bypass-hook-trust`): <https://learn.chatgpt.com/docs/cli/reference>
- [S25] exec JSON events: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/exec/src/exec_events.rs>
- [S26] docs/config.md (managed-hooks-only): <https://raw.githubusercontent.com/openai/codex/main/docs/config.md>
- [S27] Log DB writer: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/state/src/log_db.rs>
- [S28] Hook lifecycle in TUI: <https://raw.githubusercontent.com/openai/codex/main/codex-rs/tui/src/chatwidget/hook_lifecycle.rs>
- [S29] tmux(1) (`allow-set-title`, `allow-passthrough`, `monitor-bell`, `alert-bell`): <https://man7.org/linux/man-pages/man1/tmux.1.html>
- [S30] Latest release metadata (`rust-v0.167.2`, 2026-11-11): <https://api.github.com/repos/openai/codex/releases?per_page=15>
- [S31] 0.133.0 notes (SubagentStart/Stop): <https://github.com/openai/codex/releases?q=SubagentStart&expanded=true>
- [S32] 0.155.0 / 0.158.0 hook notes: <https://github.com/openai/codex/releases?q=%22hooks%22+trust&expanded=true>
- [S33] No release mentions PreModelRequest: <https://github.com/openai/codex/releases?q=PreModelRequest&expanded=true>
- [S34] toml_edit `Item`: <https://docs.rs/toml_edit/latest/toml_edit/enum.Item.html>
- Internal: ADR 0028, 0046, 0048, 0086, 0143, 0146; `src/hook.rs`, `src/cli/hook.rs`, `src/discovery/harness/codex.rs`, `src/discovery/codex_log.rs`, `src/discovery/hook_sidecar.rs`, `src/discovery/tmux/mod.rs`, `src/viewer/parser/codex.rs`; backlog CSP-228, CSP-268.
