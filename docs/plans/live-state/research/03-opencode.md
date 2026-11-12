# Live State Research 03: OpenCode

Evidence base for the per-harness signal extraction step of the live
state feature. The question: which OpenCode signals tell an outside
observer that a session is waiting on the operator, ready for a new
prompt, being worked on (by the agent or by subagents), compacting,
erroring or rate limited, starting, or gone.

Researched 2026-11-11. Nothing was cloned, installed, or executed, and
the operator's `~/.local/share/opencode` was not read. Shapes come from
the repo (adapter, plugin, fixtures, ADRs 0013, 0049, 0125, 0126), the
opencode docs, and opencode source files read at release tags on
GitHub.

## Method and source tags

- The upstream repository is now `github.com/anomalyco/opencode`
  (`sst/opencode` resolves there), default branch `dev` [S-repo].
  The newest release is `v2.2.7` (10 Nov) [S-rel]. Source was read at
  three points: `v1.18.32` (the operator's 1.x build, per ADR 0125),
  `v2.0.32` (the version ADR 0125 tested), and `v2.2.7` or `dev`.
  The tag is named next to every claim, because these differ.
- Source and docs were fetched through a summarizing fetch tool, so a
  quote is the tool's reading of the page, not a byte-exact copy.
  Where the tool reported that something wasn't on the page, the claim
  is marked **UNVERIFIED**.
- Confidence tags: **[src]** read in opencode source; **[doc]** read
  in opencode docs; **[repo]** in this repo (code, fixtures, or the ADR
  0125 sandbox run); **[inf]** inferred from the above; **UNVERIFIED**
  not confirmed anywhere.
- Every external claim cites a source key from the Sources list at the
  end.

## 0. Key findings

1. **What we have can't produce live state yet.** The plugin already
   forwards `session.status` and `session.idle`, but the payload is
   `{session_id, cwd?, hook_event_name?}` with no status type
   [repo: `plugins/opencode-hook/src/index.ts`, `src/write.ts`]. A
   `session.status` record can't tell busy from idle. Only "the last
   record was `session.idle`" carries state.
2. **OpenCode publishes every state we need as plugin events, and the
   payloads are metadata-shaped.**
   - Busy, idle, and retry come from `session.status`, whose
     `status.type` is `idle`, `busy`, or `retry` [S-v1status, S-sdk1v2].
   - Permission asks come from `permission.asked` and
     `permission.replied`.
   - Questions are `question.asked`, `question.replied`, and
     `question.rejected` in 1.x, and forms with `metadata.kind:
     "question"` in 2.x.
   - 1.x adds `session.error`. 2.x adds `session.execution.started`,
     `succeeded`, `failed`, and `interrupted`, plus
     `session.retry.scheduled` and `session.compaction.*`.
   - Children carry `parentID`.
   - Sources: S-sdk1v2, S-v1perm, S-v1q, S-v2events, S-v2form,
     S-v2qtool.
3. **WAITING is visible only through events or a live process.** In
   1.x, pending permissions and questions sit in in-memory maps
   [S-v1perm-src, S-v1q-src]. In 2.x they are ephemeral events
   [S-v2perm, S-v2form], and nothing found persists them. Storage
   polling can't see WAITING in either major.
4. **No TCP server by default in 1.x.** The `v1.18.32` TUI runs its
   server in an in-process worker over RPC. It listens on TCP only with
   `--port`, `--hostname`, or mDNS [S-v1tui]. The V1 server docs still
   say the TUI "randomly assigns a port" [S-doc-server], which is stale.
   2.x has one shared service (default port 49374, password in
   `service.json`) [repo: ADR 0125]. Using it would need a TCP socket and
   an HTTP client, but `docs/capabilities.toml` allows only `AF_UNIX`
   sockets and no networking crate [repo]. Plugin push is the practical
   path.
5. **Subagent behavior depends on the version.**
   - 1.x `task` blocks by default. Background subagents sit behind
     `OPENCODE_EXPERIMENTAL_BACKGROUND_SUBAGENTS` [S-v1task].
   - 2.0.32 and 2.2.7 `subagent` default to foreground and accept
     `background: true` [S-v2sub-0032, S-v2sub-227].
   - `dev` (unreleased) is background-only [S-v2sub-dev].
   - So "parent READY while children work" must be modeled. Each child
     is a session with `parentID` and its own status events.
6. **2.x storage gained useful state columns.**
   - 2.2.7 `session_v2` has `time_busy`, `time_idle`, `idle_outcome`,
     `time_viewed`, and `time_suspended` (the execution claim)
     [S-v2sql-227, S-v2proj].
   - The 2.0.32 fixture has `time_idle`, `idle_outcome`, and
     `time_suspended` but no `time_busy` [repo: `v2-fresh.sql`].
   - This allows a coarse hook-free WORKING, READY, ERROR, and
     unread-READY in 2.x.
7. **The current store can't hold live state as-is.**
   - The spool keeps one record per mux. `mux_record_key` uses the tmux
     session, else the cwd, else the session [repo: `src/hook.rs`].
   - Its `observed_epoch` has one-second resolution [repo:
     `src/discovery/mod.rs`].
   - So a subagent's record replaces its parent's attribution record.
     Pane-less records from the same cwd collapse into one. Busy and idle
     in the same second can land in either order.
   - Live state needs its own per-session store, ordered by a
     plugin-supplied sequence number. This is input for the graph
     representation step.
8. **Two defects in the existing plugin, found in passing.** Details in
   section 8.
   - The 1.x entrypoint forwards events for subagent sessions, so a
     pane's freshest record briefly names the child session.
   - The 2.x entrypoint listens for `session.compacted`, which the 2.x
     public event set doesn't contain [S-v2events, S-v2manifest].

## 1. Baseline: what Conspectus already has

| Piece | Today | Source |
|---|---|---|
| Plugin, 1.x | Root entrypoint `dist/index.js`. The `event` hook forwards `session.created`, `session.updated`, `session.status`, `session.idle`, and `session.compacted` for any session it sees | `plugins/opencode-hook/src/index.ts`, ADR 0049 |
| Plugin, 2.x | TUI entrypoint `dist/v2/tui.js`. Forwards `tui.route` (route polled each second) and `session.created`, `session.renamed`, `session.status`, `session.idle`, and `session.compacted`, only for the routed session | `src/v2/tui.ts`, ADR 0126 |
| Payload | Flat `{session_id, cwd?, hook_event_name?}`. Version in env `CONSPECTUS_OPENCODE_HOOK_VERSION` (`0.2.0`) | `src/write.ts` |
| Writer | `conspectus hook write opencode` adds pid and ppid (first `opencode` ancestor), tmux context, and `background_harness` (ancestor argv `serve` or `acp`) | `src/cli/hook.rs` |
| Store | Daemon `hook_ingest` writes the spool `hooks-latest.json` first. Without a daemon, the writer writes it. One record per mux key | `src/hook.rs`, ADR 0146 |
| Attribution | Freshest record per pane wins. Dead pid gives `Ignored`. Background records are skipped. No TTL is applied (`ACTIVE_TTL_SECONDS` is defined but unused) | `src/discovery/hook_sidecar.rs`, `src/hook.rs` |
| Install | Manual. `hook init` supports only `claude-code`, `codex`, and `pi` (`HookHarnessFlag`) | `src/cli/hook.rs` |
| Storage reader | Read-only `opencode.db` with an mtime and WAL-stamped cache. 1.x `session`, `message`, and `part`. 2.x `session_v2` and `session_message`. Lineage from `parent_id` and `fork_session_id` | `src/discovery/harness/opencode.rs` |
| Fixtures | `tests/fixtures/opencode/state.sql` (1.14), `v2-fresh.sql`, and `v2-migrated.sql` (2.0.32). No opencode hook payload fixture (`tests/fixtures/hooks/` holds Claude and Codex only) | repo |
| Sockets | `[sockets] families = ["AF_UNIX"]` only. No networking crate | `docs/capabilities.toml` |

## 2. Versions and process model

| | 1.x (`v1.18.32`) | 2.0.32 | 2.2.7 / `dev` |
|---|---|---|---|
| Process | Each TUI process has its own server in a worker (RPC). No TCP unless `--port`, `--hostname`, or mDNS [S-v1tui] | One shared `opencode serve --service`, started by the first client and reparented. The endpoint, pid, and password are in `$XDG_STATE_HOME/opencode/service.json`; default port 49374 [repo: ADR 0125] | Same as 2.0.32 [inf] |
| Plugins | `plugin` key in `opencode.json`, plus auto-loaded `~/.config/opencode/plugins/` and `.opencode/plugins/` [S-doc-v1plugins] | Server plugins: `plugins` in `opencode.jsonc`, plus `~/.config/opencode/plugins/` and `.opencode/plugins/` [S-doc-plugins]. TUI-only plugins go in `cli.json` [repo: ADR 0125, S-doc-cliconfig] | Same [doc] |
| Plugin scope | One instance per directory. Events are filtered to `event.location?.directory === ctx.directory` [S-v1plugin] | Server plugin: one instance per location in the service, with the service's env. TUI plugin: in the client process, with the pane's env [repo: ADR 0125] | Same [doc: S-doc-srvplugin, S-doc-tuiplugin] |
| Session status | In memory. Idle means no entry [S-v1status] | `session.status` is declared; execution events are durable [S-v2events] | `session.status` is in the public set; no emitter found (section 3.2) |
| Questions | `question.*` events [S-sdk1v2, S-v1q] | Form (`form.created`, `metadata.kind:"question"`) [S-v2qtool-0032] | Same [S-v2qtool] |
| Subagents | `task` tool. Blocks by default; background via experimental flag [S-v1task] | `subagent` tool. Foreground by default, `background?` [S-v2sub-0032] | 2.2.7 as 2.0.32 [S-v2sub-227]. `dev` background-only [S-v2sub-dev] |
| Storage | `session` (has `time_compacting`), `message`, `part` [repo: `v2-migrated.sql`] | `session_v2` (`time_idle`, `idle_outcome`, `time_compacting`, `time_suspended`, `resume_attempts`, `time_viewed`) and `session_message` (types include `idle`) [repo: `v2-fresh.sql`] | Adds `time_busy` and a `session_part` table [S-v2sql-227] |

Treat any opencode below 1.18 as UNVERIFIED. The legacy 1.x SDK types
still describe `permission.updated` with a different shape
[S-sdk1v1]. The `v1.18.32` runtime defines only `permission.asked` and
`permission.replied` [S-v1perm]. When the switch happened was not
established.

## 3. Signal inventory

### 3.1 1.x plugin surface (`@opencode-ai/plugin` 1.18.32)

Plugin input (`PluginInput`) has these fields [S-v1plugin-types]:

- `client`, `project`, `directory`, `worktree`
- `experimental_workspace`
- `serverUrl`. Without a listener it falls back to
  `http://localhost:4096` [S-v1plugin], so it does not reveal a
  reachable port.
- `$`

Hooks relevant to live state [S-v1plugin-types]:

| Hook | Input fields | Live-state use |
|---|---|---|
| `event` | `{event}`; delivered as `{id, type, properties}`, fire-and-forget [S-v1plugin] | Main channel (table below) |
| `dispose` | none | Called when the instance's scope closes [S-v1plugin]. Graceful-exit signal (ENDED). Whether a TUI quit or SIGHUP runs it: UNVERIFIED |
| `experimental.session.compacting` | `{sessionID}` | Fires before compaction starts [S-v1compact]. The only 1.x compaction-start signal |
| `tool.execute.before` / `.after` | `{tool, sessionID, callID}` (+`args`) | WORKING(tool) sub-state. High volume |
| `permission.ask` | `Permission` (legacy shape), `output.status` | Runs before the decision, so the result may be `allow`. Not a reliable WAITING signal. Use the event |
| `chat.message` | `{sessionID, agent?, model?, messageID?}` | A user turn was admitted. Redundant with `busy` |

1.x `event` types that matter. Names come from the legacy SDK union
[S-sdk1v1], the 1.x "v2" SDK union [S-sdk1v2], and the V1 plugin docs
[S-doc-v1plugins]. Payloads are `event.properties`:

| Type | Fields (names only) | Notes |
|---|---|---|
| `session.status` | `sessionID`, `status{type: idle\|busy\|retry, attempt, message, next, action?{reason, provider, title, message, label, link?}}` | Published on every status set. Idle also publishes `session.idle` and drops the map entry [S-v1status]. `busy` is set at the top of each prompt-loop iteration [S-v1prompt] and by the runner's `onBusy`. `idle` is set by `onIdle` and by cancel with no runner [S-v1runstate]. `next` is epoch ms of the next attempt. `action.reason` is `free_tier_limit` or `account_rate_limit` [S-v1retry] |
| `session.idle` | `sessionID` | Redundant with status idle |
| `session.error` | `sessionID?`, `error?` (union of `ProviderAuthError`, `UnknownError`, `MessageOutputLengthError`, `MessageAbortedError`, `StructuredOutputError`, `ContextOverflowError`, `ContentFilterError`, `ApiError`) [S-sdk1v2] | Published for unknown agent, model, or command, read failures, and content filter. An interrupt writes an abort error onto the message and publishes no event [S-v1prompt]. The error discriminant field is probably `name` (NamedError): UNVERIFIED |
| `session.compacted` | `sessionID` | Success path only. A failed or overflowed compaction publishes nothing [S-v1compact] |
| `session.created` / `.updated` / `.deleted` | `info: Session{id, projectID, directory, parentID?, title, version, time{created, updated, compacting?}}` [S-sdk1v1] | `parentID` marks a subagent child |
| `permission.asked` | `id`, `sessionID`, `permission`, `patterns[]`, `metadata`, `always[]`, `tool?{messageID, callID}` [S-v1perm, S-sdk1v2] | WAITING(permission). The ask blocks on a deferred until a reply comes [S-v1perm-src] |
| `permission.replied` | `sessionID`, `requestID`, `reply: once\|always\|reject` | `reject` also rejects every other pending request in the session and publishes a `replied` event for each [S-v1perm-src] |
| `question.asked` | `id`, `sessionID`, `questions[]`, `tool?` | WAITING(question). Blocks on a deferred [S-v1q-src] |
| `question.replied` / `question.rejected` | `sessionID`, `requestID` (+`answers`) | Clears the question |
| `message.updated` | `info: Message` (assistant: `time{created, completed?}`, `error?`, `finish?`, `summary?`, `mode`) [S-sdk1v1] | Generating versus done. Volume is high |
| `message.part.updated` | `part`, `delta?` | Carries text deltas. Too much churn, and the delta is payload. Avoid |
| `todo.updated` | `sessionID`, `todos[]` | Not a state signal |
| `server.instance.disposed` | `directory` | Instance teardown [S-sdk1v1] |

**Do 1.x events reach plugins?** Events published through the
`EventV2Bridge` (`permission.asked`, `question.*`, `session.status`)
reach the plugin `event` hook. The hook listens on that bridge
(`events.listen`) and filters only by directory [S-v1plugin].
Confirming it end to end is a sandbox check (section 7.4).

### 3.2 2.x event surface

The public stream's types come from the V2 API docs [S-v2events].
Payload fields come from source at `dev`: S-v2status, S-v2perm,
S-v2form, and S-v2sessev. Each event has the envelope `{id, created,
type, data, location?, metadata?, durable?}` [S-v2events]. In the TUI
plugin, `ctx.data.listen(({details}) => ...)` delivers `details.type`
and `details.data`. The existing plugin reads `details.data.sessionID`,
and that works in the sandbox [repo: CSP-607].

| Type | Durable | `data` fields | Live-state use |
|---|---|---|---|
| `session.execution.started` | yes | `sessionID` | WORKING. Commits the execution claim [S-v2exec] |
| `session.execution.succeeded` | yes | `sessionID` | READY, outcome `succeeded` |
| `session.execution.failed` | yes | `sessionID`, `error{type, message, status?, response?, retryAfterMs?}` [S-v2err] | ERROR (sticky until the next start) |
| `session.execution.interrupted` | yes | `sessionID`, `reason: user\|shutdown\|superseded\|inactivity` | `user`: READY (interrupted). `shutdown`: the service is stopping and the claim is kept for resume [S-v2exec]. `inactivity` and `superseded`: meaning UNVERIFIED |
| `session.status` | no | `sessionID`, `status{idle\|busy\|retry(attempt, message, next, action?)}` | Same as 1.x. It is in the public set [S-v2manifest, S-v2events], but no 2.x emitter was found: `execution.ts` and `run-coordinator.ts` publish no status [S-v2exec, S-v2runcoord]. Whether 2.x emits it: UNVERIFIED. Prefer the execution events |
| `session.idle` | no | `sessionID` | Marked `// deprecated` at 2.0.32 [S-v2status-0032] |
| `session.retry.scheduled` | yes | `sessionID`, `assistantMessageID`, `attempt`, `at`, `error{type, status?, retryAfterMs?, ...}` | RETRYING. RATE_LIMITED when `error.status == 429` or `retryAfterMs` is set [inf] |
| `session.compaction.started` / `.ended` / `.failed` | yes | `reason: auto\|manual` (+`error`, cost, tokens) | COMPACTING and back |
| `permission.asked` | no | `id`, `sessionID`, `action`, `resources[]`, `save?`, `metadata?`, `source?{type:"tool", messageID, id}`, `message?` [S-v2perm] | WAITING(permission) |
| `permission.replied` | no | `sessionID`, `requestID`, `reply: once\|always\|reject` | Clears it. `reject` clears all pending in the session [S-doc-perms] |
| `form.created` | no | `form{id (frm_...), sessionID, title, metadata?, fields}` [S-v2form] | WAITING(question) when `metadata.kind == "question"` (the question tool [S-v2qtool]). Otherwise WAITING(form), for example MCP elicitation, whose `sessionID` may be the literal `"global"` [S-v2form] |
| `form.replied` / `form.cancelled` | no | `id`, `sessionID` (+`answer`) | Clears it |
| `session.created` | yes | `sessionID`, `parentID?`, `projectID`, `location`, `agent?`, ... [S-v2sessev] | Registers a child when `parentID` is set |
| `session.synthetic` | yes | `text`, `description?`, `metadata?` | A finished subagent reports to its parent with `metadata{source:"subagent", childID, agent, state}` [S-v2subdone] |
| `session.step.started` / `.first-token` / `.ended` / `.failed` | yes | `assistantMessageID`, `agent`, `model`, ... | WORKING sub-states (requesting, generating) |
| `session.tool.input.started` / `.called` / `.success` / `.failed` | yes | `assistantMessageID`, `id` (+`name` on `input.started` only) | WORKING(tool) sub-state. High volume |
| `session.shell.started` / `.ended` | yes | `shell` | An operator `!` shell command |
| `session.viewed` | yes | `idle` (epoch-ms watermark) | Unread READY becomes seen |
| `session.deleted` / `.archived` | yes | `sessionID` | Drop the session |
| `session.inbox.enqueued` / `.delivered` | yes | `inboxID`, `item` | A prompt was queued while busy [S-v2session-spec] |
| `location.shutdown` | | | Instance going away |

Not in the 2.x public set [S-v2events, S-v2manifest]:

- `session.compacted`
- `session.error`
- `question.*`
- `message.updated`
- `todo.updated`

Use the replacements above.

**Delivery guarantees (2.x HTTP stream).** The `/api/event` stream is
"volatile", with no replay [S-v2stream]:

- Each connection has a dropping queue of 4,096 frames.
- An overflow fails that subscriber.
- An encoding failure fails every subscriber [S-v2feed].
- A heartbeat is sent every 15 s [S-v2stream].
- Durable events can be replayed per session through
  `sessions.log({sessionID, after?, follow?})` [S-v2session-spec].

Whether plugin `ctx.event.subscribe` shares the stream's lossiness:
UNVERIFIED.

### 3.3 2.x plugin context: stores and actions

- **Server plugin** [S-doc-srvplugin]:
  - `ctx.event.subscribe({types?, signal?})` is an async iterable that
    filters by type.
  - Permissions: `ctx.permission.list({sessionID})` and
    `get({sessionID, requestID})`.
  - `ctx.permission.hook("evaluate", ...)` runs for allow and ask
    outcomes.
  - Tool hooks: `ctx.tool.hook("execute.before" | "execute.after")`.
  - Retry hook: `ctx.session.hook("retry", ...)` with `{sessionID,
    agent, model, error{type, message, status?}, attempt, decision}`.
  - `ctx.location.directory`.
  - No session-status accessor is documented. Server plugins run inside
    the service, so the writer marks their records
    `background_harness` and they produce no mux links (ADR 0126).
- **TUI plugin** [S-doc-tuiplugin]:
  - `ctx.data.listen(handler)` covers all server events.
    `ctx.data.on(type, handler)` takes one type.
  - Session stores: `ctx.data.session.status(sessionID)` (return shape
    undocumented: UNVERIFIED), `session.permission.list(sessionID)`,
    `session.form.list(sessionID, location)`, and
    `session.pending.list(sessionID)` (inbox items).
  - `session.root(sessionID)` and `session.family(sessionID)`.
  - `ctx.ui.router.current()`.
  - `ctx.attention.notify`.
  - All these stores are in-process reads. They need no auth and no
    socket.

### 3.4 HTTP server, SSE, and status endpoints

| | 1.x | 2.x |
|---|---|---|
| Listener | None by default. In-process worker RPC (`http://opencode.internal`). TCP only with `--port`, `--hostname`, or mDNS [S-v1tui]. `opencode serve` defaults to port 4096 [S-doc-server] | Shared service on 49374. Endpoint, pid, and password in `$XDG_STATE_HOME/opencode/service.json`; basic auth `opencode:<password>` [repo: ADR 0125] |
| Port discovery | None for a TUI. Nothing writes the URL to a file [S-v1tui]. mDNS only if enabled | `service.json`. `GET /api/info` returns `{version, pid, urls, paths, ...}` [S-v2api] |
| Status | `GET /session/status` returns `{[sessionID]: SessionStatus}` [S-doc-server] | `GET /api/session/active` lists sessions with a running turn [S-v2api] |
| Pending asks | `GET /permission` and `GET /question` cover all sessions [S-v1routes] | `GET /api/permission/request` and `GET /api/form` per location; `GET /api/session/{id}/permission` and `/form` per session [S-v2api] |
| Events | `GET /event` (SSE). The first event is `server.connected` [S-doc-server] | `GET /api/event` (SSE), filterable by `types` [S-v2api] |
| Writes to avoid | `POST .../permissions/...`, `/tui/*` | `POST /api/session/{id}/view` (sets `time_viewed`) and every reply route [repo: ADR 0125] |

So 1.x has no external API unless the operator adds `--port`. 2.x has a
complete pull API: running sessions, pending permissions, and pending
forms. Using it would need:

- A TCP socket and an HTTP client. `capabilities.toml` allows only
  `AF_UNIX` and bans networking crates.
- A credential read from `service.json`.
- An ADR. ADR 0126 already rejected this API for attribution.

### 3.5 Storage records (polling)

- **1.x:** `session` has a `time_compacting` column [repo:
  `v2-migrated.sql`]. Nothing found sets it: `compaction.ts` doesn't
  [S-v1compact], so its use is UNVERIFIED.
  - `message.data` (assistant) has `time.created`, `time.completed?`,
    `error?`, `finish` (`stop`, `tool-calls`), `mode`, and `summary?`
    [repo fixture, S-sdk1v1].
  - `part.data` tool parts have `state.status: pending|running|completed|error`
    [S-sdk1v1].
  - Compaction writes a user message with a `compaction` part and an
    assistant message with `mode:"compaction"` and `summary:true`
    [S-v1compact].
- **2.0.32:** `session_v2.time_idle` and `idle_outcome` (`succeeded`
  in the fixture) [repo].
  - `session_message.type` takes `user`, `assistant`, `idle`
    (`{time, outcome}`), and `system` [repo, ADR 0125].
  - Assistant `content[].state.status` takes `completed` and `error`
    (seen), and `streaming` and `running` (schema only) [repo: ADR
    0125].
  - The `compaction` message has `status: running|completed|failed`
    [S-v2api, ADR 0125, unverified there].
- **2.2.7 and `dev`:** the projector maintains these columns
  [S-v2proj]:
  - `time_busy` is set by `execution.started`.
  - `time_idle` and `idle_outcome` (`succeeded|failed|interrupted`)
    are set by the terminal events. A shutdown interruption skips the
    write, so busy can stay set across a service stop.
  - `time_viewed` is set by `session.viewed`.
  - The projector deliberately leaves `time_updated` alone on
    execution events.
  - `time_suspended` is the execution claim [S-v2sql-227]. It is held
    from the start of a busy period until success, failure, or a user
    interrupt. It is kept on shutdown or crash, and the service resumes
    such sessions at its next start [S-v2session-spec, S-v2exec].
  - A `session_inbox` table holds queued prompts [S-v2sql-dev], and
    there is a new `session_part` table [S-v2sql-227].
  - A "running Job row" is kept for subagent jobs [S-v2exec]. Its table
    name: UNVERIFIED.

### 3.6 tmux and process cues (no content)

| Cue | 1.x | 2.x | Value |
|---|---|---|---|
| Pane process tree | Shell and tool subprocesses are children of the TUI [inf: tools run in the TUI's in-process server] | Tools run under the service, not the pane [repo: ADR 0125] | 1.x only: "a tool is running" |
| Process liveness | The TUI pid is the session host | The client pid is only the viewer; the service pid hosts the sessions | EXITED / service down |
| `#{pane_title}` | UNVERIFIED | `terminal.title` setting, "Updates the terminal window title" [S-doc-cliconfig]. Content UNVERIFIED | Possibly a session title. A state is unlikely |
| Bell or notification | UNVERIFIED | `attention.notifications` and `attention.sound`, with sounds for `question`, `permission`, `error`, `done`, and `subagent_done` [S-doc-cliconfig]. Whether this reaches tmux (BEL, OSC): UNVERIFIED | Would give WAITING and READY through `window_bell_flag` if it emits BEL |
| Pane activity | Redraws while streaming (spinner) [inf] | Same [inf] | Weak WORKING hint. Animation makes it ambiguous |
| Pane capture | The permission dialog, spinner | Same | Would need its own ADR (ADR 0028 forbids generic scraping). CSP-185 and CSP-268 track it. Not recommended |

## 4. Mapping signals to canonical live states

The vocabulary matches research 01:

- STARTING
- READY, with an `unread` flag in 2.x
- WORKING, with sub-reasons `generating` and `tool`
- WAITING, with reasons `permission`, `question`, and `form`
- SUBAGENTS, an overlay count
- COMPACTING
- ERROR, with RATE_LIMITED and RETRYING as special cases
- ENDED
- EXITED

Certainty: **C** certain (event fires on the transition), **L** likely,
**A** ambiguous, **U** undetectable through this channel.

### 4.1 1.x (plugin `event` hook in the TUI process)

| Signal | State | Cert. | Notes |
|---|---|---|---|
| `session.status{type:"busy"}` | WORKING | C | Set at each loop iteration [S-v1prompt] |
| `session.status{type:"retry"}` | RETRYING. RATE_LIMITED if `action.reason` is set | C | `next` gives the resume time. Do not store `message` (free text) |
| `session.status{type:"idle"}` / `session.idle` | READY | C | After an error, show ERROR until the next busy (sticky) |
| `session.error` (error ≠ abort) | ERROR | L | Not every failure publishes one. A provider failure inside the processor may only write `message.error` [S-v1prompt]: UNVERIFIED either way. `sessionID` is optional, and errors without it can't be attributed |
| Interrupt (Esc) | READY | L | No `session.error`; status goes idle through the runner [S-v1prompt, S-v1runstate] |
| `permission.asked` / `.replied` | WAITING(permission) and back | C | Status stays `busy` while waiting [inf: no status write near the ask, S-v1prompt]. So WAITING must override busy |
| `question.asked` / `.replied` / `.rejected` | WAITING(question) and back | C | Same overlay rule |
| `experimental.session.compacting` hook, then `session.compacted` | COMPACTING, then the next state | C / A | Start is certain. A failed compaction publishes nothing, so leave COMPACTING on the next status event |
| `session.created{info.parentID}` + child `session.status` | SUBAGENTS overlay on the parent | C | The child's events come through the same instance |
| `dispose` hook | ENDED | L | Not called on SIGKILL or crash |
| pid dead (writer records the TUI pid) | EXITED | C | `process_is_live` already exists |
| Process up, no session yet (home screen) | STARTING | U (events) | The pane process tree shows `opencode`, but no session exists to key the state on. This is a mux-level fact |
| Waiting on the provider's first token | WORKING | U | No 1.x event without `message.part.updated` churn |

### 4.2 2.x (server or TUI plugin)

| Signal | State | Cert. | Notes |
|---|---|---|---|
| `session.execution.started` | WORKING | C | Durable. Observed in the sandbox [repo: ADR 0125] |
| `session.execution.succeeded` | READY | C | Observed in the sandbox [repo: ADR 0125] |
| `session.execution.failed` | ERROR | C | `error.type` and `error.status` are safe metadata. `message` is free text |
| `session.execution.interrupted{user}` | READY (interrupted) | C | |
| `session.execution.interrupted{shutdown}` | WORKING → SUSPENDED (resumes at next service start) | L | The claim is kept [S-v2exec]. Display it as "stopped, will resume" |
| `session.retry.scheduled` | RETRYING / RATE_LIMITED | C | Leaves it on the next step or terminal event [S-v2session-spec] |
| `session.status` | as 1.x | A | Emitter not found in 2.x (section 3.2) |
| `permission.asked` / `.replied` | WAITING(permission) | C | Ephemeral. Lost if the subscriber misses it (see staleness) |
| `form.created{metadata.kind:"question"}` / `.replied` / `.cancelled` | WAITING(question) | C | The question tool [S-v2qtool] |
| `form.created` (other kinds) | WAITING(form) | L | MCP elicitation may have `sessionID:"global"`, which has no session to key on |
| `session.compaction.started` → `.ended` / `.failed` | COMPACTING → previous | C | 2.0.32 couldn't drive `/compact` in the sandbox [repo: ADR 0125] |
| `session.step.started` → `.first-token` | WORKING(requesting) → WORKING(generating) | C | Optional sub-state |
| `session.tool.input.started` → `.success` / `.failed` | WORKING(tool) | C | Optional. Volume |
| `session.created{parentID}` + child execution events | SUBAGENTS overlay | C | |
| `session.synthetic{metadata.source:"subagent"}` | A child finished | C | Redundant with the child's terminal event |
| `session.viewed` vs last idle | READY(unread) → READY | C | Mirrors opencode's own unread check [S-v2proj] |
| `session.inbox.enqueued` while busy | WORKING + queued | C | Optional |
| TUI plugin cleanup, or client pid dead | Client EXITED; the session may still be WORKING in the service | L | In 2.x, closing a TUI doesn't stop execution [inf: the service owns `SessionExecution`, S-v2exec]. UNVERIFIED end to end |
| Service pid dead (`service.json`) | Every session EXITED; the claimed ones are SUSPENDED | C | |
| Service resumes claimed sessions at startup | STARTING (resuming) | L | `resume_attempts` counts attempts [S-v2sql-227] |

### 4.3 Subagents

- **Identity.** A child is a session whose `parentID` is the caller's
  session.
  - 1.x `task`: `parentID: ctx.sessionID`. Result metadata has
    `sessionId` and `parentSessionId` [S-v1task].
  - 2.x `subagent`: `parentID: context.sessionID` [S-v2sub-227]. The
    tool result's `state.metadata.sessionID` is the child id [repo:
    ADR 0125].
  - Storage: the `parent_id` column (1.x and 2.x), already read as
    `spawn` lineage [repo].
- **Active.** Any child in WORKING, WAITING, RETRYING, or COMPACTING.
  The child's own events say so. The parent overlay is a count.
- **Parent while children work.**
  - Foreground, the default before `dev`: the parent stays WORKING; it
    blocks on the child [S-v1task, S-v2sub-0032].
  - Background, opt-in, or always in `dev`: the parent can be READY
    while the children run. Completion arrives as a synthetic message
    to the parent [S-v2subdone]. In 1.x a synthetic prompt is injected
    [S-v1task], which wakes the parent.
- **A child asking for permission is WAITING on the operator.** It
  bubbles up to the root session's row, because that is what the
  operator looks at. Whether the 1.x and 2.x TUIs surface child prompts
  in the parent's view: UNVERIFIED. Either way, the operator must act.
- **Depth.** 2.x limits nesting with `experimental.subagent_depth`,
  default 1 [S-v2sub-227]. The built-in `general` agent cannot launch
  subagents [S-doc-agents].

### 4.4 Summary: certain, ambiguous, undetectable

- **Certain with plugin events:**
  - WORKING, READY, and RETRYING in 1.x; WORKING and READY through
    execution events in 2.x
  - WAITING(permission) and WAITING(question) in both
  - Subagent membership and activity
  - Compaction start; in 2.x also the end
  - ERROR in 2.x (`execution.failed`)
  - EXITED through pid liveness
- **Ambiguous:**
  - 1.x ERROR, since `session.error` isn't published for every failure
  - 1.x failed compaction
  - 2.x `session.status` emission
  - 2.x client exit versus session exit
  - Forms with no real session
  - `interrupted{inactivity|superseded}`
- **Undetectable without events:**
  - WAITING in both majors (in memory or ephemeral)
  - RETRYING in 1.x
  - STARTING as a session state
- **Undetectable even with events:** time-to-first-token in 1.x
  without message-part churn.

## 5. Delivery and install

### 5.1 Where plugins are configured

| Version | Server / in-process plugin | TUI-only plugin |
|---|---|---|
| 1.x | `plugin` key in `opencode.json` (user: `~/.config/opencode/`; project: `.opencode/`). Auto-loaded files and packages in `~/.config/opencode/plugins/` and `.opencode/plugins/` [S-doc-v1plugins]. `opencode plugin <path>` writes the config entry [repo: plugin README]. `--pure` disables external plugins [repo: ADR 0049, S-v1plugin] | none |
| 2.x | `plugins` in `opencode.jsonc` at `~/.config/opencode/`, `./`, and `./.opencode/`, merged lowest to highest. Global `~/.config/opencode/plugins/`, project `.opencode/plugins/`. Configured local paths must be directories [S-doc-plugins]. A package listed there also has its `tui` entrypoint loaded by the TUI [repo: ADR 0125] | `cli.json` `plugins` (`$XDG_CONFIG_HOME/opencode/cli.json`, user scope only). Edits reload live [S-doc-cliconfig] |

Two cautions:

- 1.x and 2.x share `~/.config/opencode/plugins/`. A 1.x-shaped file
  there would also be offered to 2.x as a server plugin. 2.x ignores a
  V1 `server()` export [S-doc-srvplugin]. How 1.x treats a 2.x
  `{id, setup}` default export: UNVERIFIED.
- `hook init opencode` therefore needs care before it writes an owned
  file there. The Pi pattern is an owned file with a marker line
  (`src/cli/hook.rs`). Editing `opencode.jsonc` or `cli.json` instead
  means JSONC editing that preserves comments. The repo has
  `toml_edit` but no JSONC editor, so that would be a new dependency
  and needs an ADR.

### 5.2 What the existing plugin sends, and how to extend it

Today's records carry identity and an event name only (section 1).
Proposed extension: keep the same writer command and add an optional
`live` object to the flat payload. The writer (adapter override of
`hook_record_from_payload`) parses it.

```json
{
  "session_id": "ses_...",
  "cwd": "/home/user/proj",
  "hook_event_name": "permission.asked",
  "parent_session_id": "ses_...",
  "event_ms": 1792300904859,
  "seq": 42,
  "live": {
    "phase": "busy",
    "pending_permissions": 1,
    "pending_questions": 0,
    "pending_forms": 0,
    "outcome": "failed",
    "error_type": "api",
    "http_status": 429,
    "retry_attempt": 2,
    "retry_next_ms": 1792300910000,
    "retry_reason": "account_rate_limit",
    "subphase": "tool"
  }
}
```

- **Snapshot, not delta.** The plugin keeps a small per-session state
  machine and sends the whole derived snapshot on each change. Records
  then don't depend on each other. A lost or reordered spawn is healed
  by the next one, and the Rust side needs no read-modify-write
  reducer. Within one plugin instance, `seq` is monotonic; with
  `event_ms` (2.x `created` or `Date.now()`) it orders records
  correctly within the same second. That fixes the one-second
  `observed_epoch` race.
- **Privacy (ADR 0086 Tier 3).**
  - Counts and enumerated words only.
  - Never send `resources`, `patterns`, `always`, the permission or
    question `message` or `questions` text, `status.message` (retry
    text), `error.message`, tool `input` or output, or form `fields`.
  - `error_type`, `http_status`, and `retry_reason` are enumerations
    or numbers.
  - The permission `action` / `permission` word (`shell`, `edit`) is
    borderline: open question 3.
- **Which records drive attribution.** State records for child
  sessions, and pane-less ones, must not compete for the pane's
  attribution slot. Either keep them in a separate per-session store
  keyed `(harness_key, session_key)`, or mark them `attribution:false`.
  This is the graph step's decision; today's per-mux spool can't hold
  them (finding 7).
- **1.x entrypoint.** Extend the `event` switch with:
  - `session.status`, `session.idle`, and `session.error`
  - `permission.asked` and `permission.replied` (plus legacy
    `permission.updated`, read defensively)
  - `question.asked`, `question.replied`, and `question.rejected`
  - `session.compacted`, `session.created`, and `session.deleted`
  - the `experimental.session.compacting` hook, and `dispose`

  Report every session the instance sees, children included. Send
  attribution records only for sessions with no `parentID`.
- **2.x: two options.**
  - **A. TUI entrypoint only, scoped to the routed family.** On any
    event for a session in `ctx.data.session.family(routed)`, send a
    snapshot from the TUI's own stores: `session.status`,
    `permission.list`, `form.list`, and `pending.list`. The TUI's store
    becomes the reducer.
    - Pros: no new install surface; pane-accurate; no new trust in the
      service.
    - Cons: misses sessions no client is routed to, such as background
      children of an unrouted session, and work that continues after
      the client closes.
    - Two clients showing the same family write duplicate snapshots,
      which is harmless because snapshots are idempotent.
  - **B. Add a server entrypoint for state.** One instance per
    location subscribes with `ctx.event.subscribe({types: [...]})`,
    filtered to its own `location`, and writes pane-less
    `background_harness` snapshots for every session, children
    included.
    - The TUI entrypoint keeps attribution.
    - A package directory with `server.js` and `tui.js` listed once in
      `opencode.jsonc` loads both [repo: ADR 0125].
    - This amends ADR 0126 decision 3 ("no server entrypoint"), whose
      rejection rested on attribution, not on state.
    - Whether each location instance receives only its own location's
      events: UNVERIFIED. Filter on `event.location` anyway.
  - **Recommendation: B** for completeness. It's the only way to see
    service-side work with no client, and `dev` makes every subagent
    background. Keep A's store snapshot as the fallback when only
    `cli.json` is installed.
- **Event volume.** The minimal set produces about 2 spawns per turn,
  plus 2 per permission or question, 1 per retry, 2 per compaction, and
  2 per child [inf]. Leave out `step.*` and `tool.*` sub-states by
  default, or coalesce them in the plugin with a 250 ms debounce that
  sends only on a `subphase` change.

### 5.3 Latency

- **Push.** The path is plugin `spawn` (Node), Rust start-up, daemon
  `hook_ingest` over the Unix socket, the live graph, and the TUI.
  Expected well under a second. Not measured: UNVERIFIED; measure in
  the sandbox.
  - 1.x delivers to the plugin fire-and-forget and in-process
    [S-v1plugin].
  - 2.x delivers events from service to client over SSE before the TUI
    plugin sees them.
- **Poll fallback.** The latency is the harness class interval (`5s`
  default, ADR 0079) plus the WAL-stamped cache.
  - A busy session keeps changing the WAL, so the cache misses every
    cycle while anything is working.
  - The live-state query should be narrow (section 6), not a re-run of
    the full session scan.

### 5.4 Reliability and staleness

- **Lost on process death.** 1.x pending maps die with the TUI.
  - A finalizer rejects deferreds on instance shutdown [S-v1perm-src,
    S-v1q-src]. Those rejections may publish `replied`/`rejected` events
    that never reach a dying plugin.
  - In 2.x, permission and form events are ephemeral. A server plugin
    that restarts (service restart) loses pending asks it didn't
    re-read. On start, it should re-list them with
    `ctx.permission.list`; whether a forms equivalent exists:
    UNVERIFIED.
- **Writer failures.** The plugin ignores them (ADR 0049). A missing
  binary means no records, which means an unknown state, never READY.
- **Heartbeat.** While a session is in a non-READY state, re-send its
  snapshot every 60 s with the same `seq` lineage. Cost: one spawn per
  minute per working session.
- **Version.** Records from plugin `0.2.0` (no `live`) carry no live
  state. Show unknown.

## 6. Hook-free fallbacks (storage polling)

Every query below reads only metadata columns or enumerated JSON keys.
No `text`, `input`, `output`, or `content` text is read. Reading
`json_extract(data, '$.status')` touches a payload column, though. ADR
0086 Tier 1 says attribution readers "never read payload columns", so
this needs an explicit ADR note (open question 4).

| Version | Query sketch | Reveals | Limits |
|---|---|---|---|
| 2.2.7+ | `SELECT id, parent_id, time_busy, time_idle, idle_outcome, time_viewed, time_suspended, resume_attempts FROM session_v2 WHERE time_archived IS NULL` | `time_busy > coalesce(time_idle,0)` gives WORKING. Otherwise READY, or ERROR when `idle_outcome='failed'`. `time_idle > coalesce(time_viewed,0)` gives unread. `time_suspended IS NOT NULL` gives a claim held (busy, or orphaned until the next service start). Children through `parent_id` | No WAITING, RETRYING, or COMPACTING. A crash or shutdown leaves busy markers set, so check the service pid from `service.json` [S-v2proj, S-v2exec] |
| 2.0.32 | `session_v2.time_idle`, `idle_outcome`, `time_suspended`, plus the newest `session_message.type` per session (`idle` vs `user`/`assistant`) and `json_extract(data,'$.status')` where `type='compaction'` | Last row `idle` means READY with outcome. A `user` or `assistant` row after the last `idle` means WORKING or crashed. Compaction running | `time_suspended` semantics at 2.0.32: UNVERIFIED. No `time_busy` |
| 1.x | Newest `message` per session: `json_extract(data,'$.role')`, `'$.time.completed'`, `'$.error.name'`, `'$.finish'`, `'$.summary'`. Tool parts with `json_extract(data,'$.state.status') IN ('pending','running')` | An assistant with no `completed` means WORKING. An `error` means ERROR, or READY if aborted. `summary:true` in flight means COMPACTING. A running tool part means WORKING(tool) | No WAITING (a running tool part can't separate "awaiting permission" from "long command"), no RETRYING. A crash leaves the message without `completed` forever, so tie it to a live `opencode` process. That is cwd- or fd-level evidence, not session-level. `'$.error.name'`: UNVERIFIED |
| All | Process tree and pid liveness (already collected) | EXITED. In 1.x, a tool subprocess under the TUI | |

Accuracy, in short:

- Polling gives READY, WORKING, and ERROR (2.x well, 1.x fair).
- It gives subagent activity in both, and COMPACTING (1.x, 2.0.32).
- It never gives WAITING, which is the state the operator most wants.
- The 2.x HTTP API (section 3.4) would give WAITING without a plugin,
  but crosses the socket and credential boundary.

## 7. Recommendation

### 7.1 Minimal event set

- **1.x plugin:**
  - Events: `session.status`, `session.idle`, `session.error`,
    `permission.asked`, `permission.replied`, `question.asked`,
    `question.replied`, `question.rejected`, `session.created`,
    `session.deleted`, and `session.compacted`.
  - Hooks: `experimental.session.compacting` and `dispose`.
  - Optional: `permission.updated` (legacy) and `tool.execute.*`
    (sub-state).
- **2.x plugin (server entrypoint):**
  - `session.execution.started`, `.succeeded`, `.failed`, and
    `.interrupted`
  - `session.retry.scheduled`
  - `session.compaction.started`, `.ended`, and `.failed`
  - `permission.asked` and `permission.replied`
  - `form.created`, `form.replied`, and `form.cancelled`
  - `session.created`, `session.deleted`, `session.archived`, and
    `session.viewed`
  - `session.status` if it ever arrives
  - Optional: `session.step.started`, `.first-token`, and
    `session.tool.*` (sub-state)
- **2.x TUI entrypoint:** unchanged for attribution. Fix the
  `session.compacted` entry (section 8).

### 7.2 Event-to-state transitions (per session; plugin-side reducer)

State variables:

- `phase` ∈ {idle, busy, retry, compacting}
- `perms` (set of request ids)
- `questions` (set of ids)
- `forms` (set of ids)
- `outcome` ∈ {none, succeeded, failed, interrupted}
- `retry{attempt, next_ms, reason}`
- `parent`

| Event (1.x / 2.x) | Effect |
|---|---|
| `session.status busy` / `execution.started` | `phase=busy`, `outcome=none`, clear `retry` |
| `session.status retry` / `retry.scheduled` | `phase=retry`, set `retry` |
| Next `busy` / `step.started` after a retry | `phase=busy` |
| `session.status idle` / `execution.succeeded` | `phase=idle`, `outcome=succeeded` (1.x: keep `failed` if `session.error` came in this turn). Clear `perms`, `questions`, `forms` (safety) |
| `session.error` / `execution.failed` | `outcome=failed`, `error_type`, `http_status`. 2.x: `phase=idle` |
| `execution.interrupted{user}` | `phase=idle`, `outcome=interrupted`, clear sets |
| `execution.interrupted{shutdown}` | `phase=suspended` (claim kept) |
| compacting hook / `compaction.started` | `phase=compacting` |
| `session.compacted` / `compaction.ended` / `.failed` | `phase=busy` (it continues inside the turn) |
| `permission.asked` | `perms += id` |
| `permission.replied` | `perms -= requestID`. On `reject`, clear `perms` for the session |
| `question.asked` / `form.created{kind:question}` | `questions += id` |
| `question.replied` / `.rejected` / `form.replied` / `.cancelled` | remove the id |
| `session.created{parentID}` | `parent=parentID` |
| `session.deleted` / `.archived` | Send a final snapshot with `phase=gone` |
| `dispose` (1.x) | Send `phase=ended` for every session of the instance that is not idle |

Derived state, highest precedence first. The consumer computes it, so
the rule is shared across harnesses:

1. The record's pid is dead: EXITED. For 2.x service records, every
   session goes to EXITED, with `suspended` kept when the session was
   busy.
2. `perms` not empty: WAITING(permission).
3. `questions` not empty: WAITING(question).
4. `forms` not empty: WAITING(form).
5. `phase=compacting`: COMPACTING.
6. `phase=retry`: RATE_LIMITED when `retry.reason` is set or
   `http_status=429`, else RETRYING.
7. `phase=busy`: WORKING.
8. `phase=idle` and `outcome=failed`: ERROR.
9. Otherwise READY. Add `unread` in 2.x when the last idle is newer
   than the last view.

The overlay `SUBAGENTS=n` counts children whose derived state is in
2–7. A child in 2–4 also raises WAITING(subagent) on the root session.

### 7.3 Staleness rule

- **Liveness first.** 1.x records carry the TUI pid. 2.x server
  records carry the service pid. 2.x TUI records carry the client pid.
  - A dead pid means EXITED.
  - The existing check is pid-only, so add a process start time
    (`/proc/<pid>/stat` field 22) to guard against pid reuse.
- **Heartbeat TTL.** A non-READY snapshot older than 3 × the heartbeat
  (180 s) with a live pid shows "WORKING?" or unknown, not WORKING.
  - WAITING doesn't expire this way. A waiting session with a live
    process really is waiting.
  - Re-sent snapshots keep WAITING fresh, so this only matters if
    heartbeats stop while the process lives (a hung plugin).
- **READY doesn't expire** while the pid lives.
- **Fallback TTL.** A plugin without heartbeat support gets 15 minutes
  for non-READY states (`ACTIVE_TTL_SECONDS`).
- **Disagreement.** When a poll disagrees with a fresh hook snapshot,
  the hook wins. A poll overrides only when the hook record is stale
  or absent.

### 7.4 Fixture and test plan (all synthetic or sandbox-captured)

1. **Payload fixtures**:
   - Files: `tests/fixtures/hooks/opencode-live-busy.json`, `-retry.json`,
     `-permission.json`, `-question.json`, `-error.json`, `-child.json`,
     `-v1-legacy.json` (no `live`).
   - Rust tests for the opencode `hook_record_from_payload` override.
     They parse the payload and reject or strip non-whitelisted keys
     (privacy test: assert no `resources`, `patterns`, or `message`
     survives).
2. **Reducer and precedence tests** in Rust on sequences of snapshot
   records:
   - `seq` ordering within the same second
   - pid dead → EXITED, and pid reuse via start time
   - heartbeat expiry
   - child rollup using `v2-fresh.sql`'s subagent parent and child
   - WAITING bubbling from a child to the root
3. **Poll fixtures**:
   - `tests/fixtures/opencode/v2-live.sql`: hand-written `session_v2`
     rows for busy (`time_busy > time_idle`), claimed (`time_suspended`),
     failed, unread, and a busy child.
   - `state-live.sql` for 1.x: an in-flight assistant message, a running
     tool part, an aborted message, and a compaction summary message.
   - Assert the queries select no payload keys.
4. **Plugin event captures.** Use the ADR 0125 sandbox method:
   isolated XDG dirs, a mock model, a private tmux server (see the tmux
   isolation memory). Capture JSON lines of the event envelopes for:
   - a permission ask (`.env` read asks by default [S-doc-perms])
   - the question tool
   - a 429 from the mock provider, for retry
   - `/compact`, which hung at 2.0.32
   - a foreground and a background subagent
   - an Esc interrupt
   - a TUI close while busy (2.x)

   Sanitize them like the SQL dumps. They drive a TS reducer test if
   plugin CI is added (open question 7) and confirm the UNVERIFIED
   rows above.
5. **Manual checks** on 1.18.32 and 2.2.7:
   - Is `session.status` emitted in 2.x?
   - Do `permission.asked` and `question.asked` reach the 1.x plugin
     `event` hook?
   - Does `dispose` run on a TUI quit?
   - Does a 2.x server-plugin location instance get other locations'
     events?
   - Plugin-to-TUI latency.

## 8. Existing issues found in passing

1. **1.x entrypoint, child attribution.** `index.ts` forwards
   `session.created` and `session.status` for every session in the
   instance, so subagent children are included. 1.x runs children in
   the same process [S-v1task], and the spool keeps one record per mux.
   - During a subagent run, the pane's freshest record names the
     child. Attribution flips to the subagent session until the
     parent's next event.
   - `hook_sidecar` doesn't special-case subagents (no `session_kind`
     check).
   - Severity: transient misattribution. Not reproduced: UNVERIFIED.
2. **2.x entrypoint, dead filter entry.** `LIFECYCLE_EVENTS` includes
   `session.compacted`, which the 2.x public set doesn't contain
   [S-v2events, S-v2manifest]. Harmless, but it is a dead filter
   entry. The compaction events are `session.compaction.*`.
3. **README install text.** The README says `opencode plugin <path>`
   writes `~/.config/opencode/config.json`. The V1 docs name
   `opencode.json` [S-doc-v1plugins]. Not checked against 1.18.32:
   UNVERIFIED.

## 9. Open questions for the operator

1. **Which majors?** The operator runs 1.18.32 (ADR 0125), and the
   newest release is 2.2.7 [S-rel]. Live state for both, or 2.x only?
2. **A 2.x server entrypoint?** This amends ADR 0126 decision 3: one
   `opencode.jsonc` entry instead of `cli.json`. Or stay TUI-only with
   family-scoped snapshots, accepting blind spots for unrouted and
   client-less sessions?
3. **Privacy line.** May records keep the permission action word
   (`shell`, `edit`, `subagent`) and the retry reason
   (`account_rate_limit`)? Or counts only?
4. **Polling.** Is a narrow `json_extract` of enumerated status keys
   (`$.status`, `$.outcome`, `$.time.completed`, `$.error.name`) from
   opencode `data` columns acceptable under ADR 0086 with an ADR note?
   Or column-only polling (2.2.7 `session_v2` times) and nothing for
   1.x and 2.0.32?
5. **HTTP API.** Confirm Conspectus never talks to the 2.x service API.
   It would mean TCP, an HTTP client, and reading the `service.json`
   password, even though it is the only hook-free WAITING source.
6. **Heartbeat cost.** Is one writer spawn per minute per non-READY
   session acceptable, with 180 s staleness? Or a longer cadence?
7. **Plugin CI.** ADR 0049 deferred a plugin build in CI. A stateful
   plugin reducer is the point where `npm run build` and a small test
   are worth it. Add them, or keep the reducer in Rust and send raw
   normalized transitions instead of snapshots?
8. **`hook init opencode`.** Support it, as an owned file under
   `~/.config/opencode/plugins/` for 1.x and a JSONC edit for 2.x (new
   dependency, ADR)? Or keep the manual install?
9. **Unread READY.** Surface it in 2.x (finished, not yet viewed in
   opencode)? It mirrors opencode's own marker.
10. **Client-less sessions.** Should a 2.x session the service runs
    with no TUI attached (closed client, background child) appear as a
    live row with no mux?
11. **Section 8.** Fix the two plugin defects now, as their own task,
    or with the live-state work?

## Sources

Repository (read in this worktree):

- `plugins/opencode-hook/` (README, `src/index.ts`, `src/write.ts`,
  `src/v2/tui.ts`, `package.json`)
- `src/hook.rs`, `src/cli/hook.rs`, `src/discovery/hook_sidecar.rs`,
  `src/discovery/harness/opencode.rs`, `src/discovery/harness/mod.rs`,
  `src/discovery/mod.rs`
- `docs/capabilities.toml`
- `tests/fixtures/opencode/{state,v2-fresh,v2-migrated}.sql` (read
  for column names and `type` and key names only)
- ADRs 0013, 0028, 0049, 0079, 0086, 0125, 0126, 0143, 0146
- CSP-268, CSP-607

External (fetched 2026-11-11; untrusted, summarized by the fetch tool):

- [S-repo] https://github.com/sst/opencode (now anomalyco/opencode)
- [S-rel] https://github.com/anomalyco/opencode/releases
- [S-doc-plugins] https://opencode.ai/docs/plugins/
- [S-doc-v1plugins] https://opencode.ai/v1/docs/plugins/
- [S-doc-server] https://opencode.ai/docs/server/ (V1 docs)
- [S-doc-srvplugin] https://opencode.ai/docs/build/plugins/server/
- [S-doc-tuiplugin] https://opencode.ai/docs/build/plugins/cli/
- [S-doc-migrate] https://opencode.ai/docs/build/plugins/migrate-v1/
- [S-v2api] [S-v2events] https://opencode.ai/docs/api/
- [S-doc-agents] https://opencode.ai/docs/agents/
- [S-doc-perms] https://opencode.ai/docs/permissions/
- [S-doc-cliconfig] https://opencode.ai/docs/cli/config/
- [S-v2stream] https://raw.githubusercontent.com/anomalyco/opencode/dev/specs/v2/event-stream-architecture.md
- [S-v2session-spec] https://raw.githubusercontent.com/anomalyco/opencode/dev/specs/v2/session.md
- [S-v2status] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/schema/src/session-status-event.ts
- [S-v2status-0032] https://raw.githubusercontent.com/anomalyco/opencode/v2.0.32/packages/schema/src/session-status-event.ts
- [S-v2perm] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/schema/src/permission.ts
- [S-v2form] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/schema/src/form.ts
- [S-v2sessev] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/schema/src/session-event.ts
- [S-v2err] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/schema/src/session-error.ts
- [S-v2manifest] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/schema/src/event-manifest.ts
- [S-v2qtool] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/core/src/tool/plugin/question.ts
- [S-v2qtool-0032] https://raw.githubusercontent.com/anomalyco/opencode/v2.0.32/packages/core/src/tool/plugin/question.ts
- [S-v2sub-dev] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/core/src/tool/plugin/subagent.ts
- [S-v2sub-227] https://raw.githubusercontent.com/anomalyco/opencode/v2.2.7/packages/core/src/tool/plugin/subagent.ts
- [S-v2sub-0032] https://raw.githubusercontent.com/anomalyco/opencode/v2.0.32/packages/core/src/tool/plugin/subagent.ts
- [S-v2subdone] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/core/src/session/subagent-completion.ts
- [S-v2job] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/schema/src/job.ts
- [S-v2sql-dev] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/core/src/session/sql.ts
- [S-v2sql-227] https://raw.githubusercontent.com/anomalyco/opencode/v2.2.7/packages/core/src/session/sql.ts
- [S-v2proj] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/core/src/session/projector.ts
- [S-v2exec] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/core/src/session/execution.ts
- [S-v2runcoord] https://raw.githubusercontent.com/anomalyco/opencode/v2.2.7/packages/core/src/session/run-coordinator.ts
- [S-v2feed] https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/server/src/event-feed.ts
- [S-sdk1v1] https://unpkg.com/@opencode-ai/sdk@1.18.32/dist/gen/types.gen.d.ts
- [S-sdk1v2] https://unpkg.com/@opencode-ai/sdk@1.18.32/dist/v2/gen/types.gen.d.ts
- [S-v1plugin-types] https://unpkg.com/@opencode-ai/plugin@1.18.32/dist/index.d.ts
- [S-v1plugin] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/plugin/index.ts
- [S-v1status] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/session/status.ts
- [S-v1prompt] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/session/prompt.ts
- [S-v1runstate] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/session/run-state.ts
- [S-v1retry] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/session/retry.ts
- [S-v1compact] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/session/compaction.ts
- [S-v1perm] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/schema/src/v1/permission.ts
- [S-v1perm-src] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/permission/index.ts
- [S-v1q] [S-v1q-src] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/question/index.ts
- [S-v1routes] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/server/routes/instance/httpapi/groups/permission.ts
  and `.../groups/question.ts`, `.../groups/event.ts`
- [S-v1tui] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/cli/cmd/tui.ts
- [S-v1task] https://raw.githubusercontent.com/anomalyco/opencode/v1.18.32/packages/opencode/src/tool/task.ts
