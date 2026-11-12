# ADR 0206: Live-State Emitters Subscribe To A Minimal Event Set Per Harness

## Status

Accepted. Part of the live agent state plan with ADRs 0203, 0204, 0205,
and 0207. Extends ADR 0028 (hook sidecars) with state events. Hook
installation remains operator-initiated, ADR 0143 category 5. Refines
ADR 0086 for hook payloads.

## Context

Conspectus installs one hook entry per harness today, for session start
only. State needs more events. Each costs a process spawn (Claude Code
and Codex run `conspectus hook write` per event) or a plugin handler in
the harness's own loop (OpenCode, Pi), so the set should be the smallest
that answers the three questions: is it waiting for me, is it ready, is
it working.

The per-harness research
(`docs/plans/live-state/research/01` to `04`) gives the events, their
payload field names, and the transitions. This ADR fixes the subscriptions,
the installation rules, and what a writer may keep. The reducer tables
are in the research notes and become tests.

## Decision

### Subscriptions

1. **Claude Code**: `SessionStart` (every source, including `fork`),
   `UserPromptSubmit`, `PermissionRequest`, `PreToolUse` limited by
   matcher to `AskUserQuestion|ExitPlanMode`, `PostToolUse`,
   `PostToolUseFailure`, `Notification` limited to `permission_prompt|
   idle_prompt|elicitation_dialog|elicitation_url_dialog|agent_needs_input|
   quota_auto_resume_fired|quota_auto_resume_stale|
   quota_auto_resume_disabled`, `SubagentStart`, `SubagentStop`, `Stop`,
   `StopFailure`, `PreCompact`, `PostCompact`, `SessionEnd`. Never:
   `MessageDisplay` and the events that can alter harness behavior
   (`WorktreeCreate`, `Setup`, `ConfigChange`, tool-batch events).
2. **Codex**: `SessionStart`, `UserPromptSubmit`, `PermissionRequest`,
   `PostToolUse`, `Stop`, `Interrupt`, `SessionEnd`, `SubagentStart`,
   `SubagentStop`, `PreCompact`, `PostCompact`. Each is its own matcher
   group beside the operator's, never merged into theirs. `notify` is a
   single program the operator may already use; it is never touched.
3. **OpenCode**: the plugin keeps a reducer per session and sends a
   complete snapshot on change. 1.x events: `session.status`,
   `session.idle`, `session.error`, `permission.asked`/`replied`,
   `question.asked`/`replied`/`rejected`, `session.created`/`deleted`,
   `session.compacted`, the compaction hook, and `dispose`. 2.x adds a
   server entrypoint so one writer sees every session, listening to
   `session.execution.*`, `session.retry.scheduled`,
   `session.compaction.*`, `permission.*`, `form.*`, `session.created`/
   `deleted`/`archived`. A child session's wait is raised to its root.
4. **Pi**: the extension keeps a small state machine and sends a snapshot
   on change. Events: `session_start`, `session_shutdown`, `agent_start`,
   `agent_end`, `agent_settled`, `message_end` (assistant only),
   `tool_execution_start`/`end` (top level), `ui_prompt_start`/`end`,
   `session_before_compact`, `session_compact`, `session_compact_failed`.
   No handler spawns a process on a streaming event, and sub-state changes
   are debounced.
5. **Heartbeat.** Plugin sources resend an unchanged snapshot every 60 s
   while the state is not `ready`, so a hung plugin shows as quiet (ADR
   0203). Event sources send none.

### Installation

6. **`hook init <harness>` installs the full set for that harness and is
   idempotent.** Re-running it upgrades an older install (a
   session-start-only entry gains the rest). Nothing changes unless the
   operator runs it. `hook status` reports coverage per event and a health
   line (the harness is active but no event has arrived: hooks are
   untrusted, disabled, or not installed). `hook remove` removes every
   entry Conspectus added and nothing else.
7. **Every entry has a timeout** of 5 s. Entries are recognized by their
   command, as today, and the matcher for session start includes every
   documented source.
8. **Codex trust is the operator's.** Current Codex runs unmanaged hooks
   only after the operator reviews them in `/hooks`. `hook init codex` says
   so. Conspectus never writes a trust value.
9. **The operator's own hooks are preserved**: values, and where the
   format allows, order. A change that would rewrite the operator's keys
   in a way a tool could not undo is not made (see Open Questions).

### What a writer may keep

10. **Enums, counts, ids, paths, and times only** (ADR 0086 Tier 3).
    Per event:
    - ids: `session_id`, `agent_id`, `tool_use_id` (not stored past the
      reducer), `prompt_id` (not stored);
    - paths: `cwd`, `transcript_path`;
    - enums: `hook_event_name`, `source`, `trigger`, `reason`,
      `notification_type`, `permission_mode`, the `StopFailure` error
      type, a `Stop`'s background-task statuses;
    - counts: background tasks and crons;
    - writer-side: `observed_ms`, pid and its start time, tmux socket,
      session, and pane id.
11. **Never kept or logged**: `prompt`, `tool_input`, `tool_response`
    (except the status of an `Agent` tool result), `message`, `title`,
    `last_assistant_message`, error text, `compact_summary`,
    `custom_instructions`, `permission_suggestions`, background-task
    descriptions and commands, elicitation text.
12. **`tool_name` is not stored.** The reducer reads it to classify
    (`AskUserQuestion` is a question, `ExitPlanMode` a plan, the
    subagent tool a subagent) and discards it. MCP tool names reveal
    server names, and no state needs them.
13. **A writer never disturbs the harness**: nothing on stdout or stderr,
    exit 0, bounded time (ADR 0204). Plugin handlers catch and swallow
    their own errors.

### Claude Code, as built

14. **One group of Conspectus's own per event.** A group is Conspectus's
    when every handler in it runs `hook write claude-code`, in shell form
    (`command`) or exec form (`command` and `args`). `hook init` appends
    one to each event that lacks one and brings an older one up to date
    in place (its matcher, a timeout), so a session-start-only install
    gains the rest and nothing is duplicated. It never edits a group of
    the operator's, even one that holds a Conspectus handler (Claude Code
    runs identical commands once), and refuses a `hooks` value or event
    list of the wrong type rather than replace it. It writes the shell
    form, which every Claude Code version runs. New handlers copy the
    command the existing ones run unless `--command` names another, which
    then moves them all; a timeout the operator set is kept.
15. **Remove and status.** `hook remove` takes every Conspectus handler
    out of every event, then only the groups and event lists that left
    empty, so an install followed by a remove gives back the operator's
    values. `hook status` says `installed`, `partial`, or `not-installed`
    for the file and `installed`, `outdated`, or `missing` per event.
    A matcher covers an event when it is absent, empty, `*`, or an exact
    list holding every name the event needs; anything else is outdated.
16. **The mapping where the research left a choice.**
    - A notification of an open dialog (`permission_prompt`, the two
      elicitation dialogs, `agent_needs_input`) names no agent and comes
      seconds late, only while the dialog is open. It confirms the wait
      already pending, whoever's it is, and makes one for the main agent
      only when none is: the neutral "wait confirmed" event (ADR 0204).
      `quota_auto_resume_stale` is a main-agent wait for input,
      `quota_auto_resume_fired` starts a turn, and `idle_prompt` and
      `quota_auto_resume_disabled` confirm an idle prompt, which leaves a
      finished or failed turn as it was.
    - `stop_hook_active` changes nothing. A `Stop` fires whenever a
      response finishes; the flag says only that this response was a
      continuation a Stop hook forced, so it ends that continuation too.
      A Stop another hook blocks reads as finished while the forced
      continuation runs, which no flag can tell.
    - An event fired inside a child agent carries `agent_id` and is the
      child's: its waits and tools count for the child, its paths are not
      recorded, and its turn events (prompt, stop, failure, compaction)
      are dropped, since the vocabulary has no state for a child's turn.
    - `PostToolUseFailure` with `is_interrupt` ends the main agent's turn
      as interrupted. A `PermissionRequest` for `AskUserQuestion` or
      `ExitPlanMode` keeps the question or plan wait.
    - The subagent tool's result (`Agent`, formerly `Task`) starts the
      child its `agentId` names when `async_launched` and stops it when
      `completed`; the id is taken to be the hooks' `agent_id`.
    - A `Stop`'s `background_tasks` replaces the child set with its
      `subagent` entries whose status is not a finished one; a Stop
      without the list leaves the set alone. `SubagentStop`'s list is not
      used, since other foreground children may still run.
    - `StopFailure`: `rate_limit` is rate limited;
      `authentication_failed`, `oauth_org_not_allowed`, and
      `cloud_credential_error` are auth; `overloaded` is overloaded;
      every other type, billing and unknown ones included, is failed.

    The fixtures that encode these are synthetic until a recording
    confirms them.

### Codex, as built

17. **One group of Conspectus's own per event, in the file's own
    form.** A group is Conspectus's when every handler in it is a
    command hook running `hook write codex`. `hook init codex` edits
    `config.toml` in place through `toml_edit` (already a dependency),
    so comments, layout, and every other key survive. It appends its
    group after the operator's for each event, in the form of the table
    it goes into: an array of tables (`[[hooks.<Event>]]`, the form Codex
    documents), or inline lists in an inline or dotted hooks table or in
    one that already holds inline lists. The group has no matcher and one
    handler with a 5 s timeout. The handler runs synchronously, Codex's
    default, because an asynchronous hook can finish after the next
    event's and the store relies on order. An older group of
    Conspectus's is upgraded in place, a group of the operator's is
    never edited, and a hooks value or event list of the wrong type is
    refused. The edited text is parsed back before it replaces the file,
    and the write follows a symlink and keeps the file's mode. An install
    never moves an operator's entry, so trust Codex recorded by place
    stays valid.
18. **Trust, `hooks.state`, and `hooks.json`.**
    - Keys of the hooks table that are not event lists, `hooks.state`
      among them, are never read or written, and no trust value is ever
      written. `hook init codex` prints the `/hooks` review step on every
      run.
    - A `hooks.json` beside the file is read only to say that Codex runs
      it too. The install stays in `config.toml`, the file ADR 0143
      category 5 names, and accepts the startup warning Codex may give
      for a layer with both. It neither edits `hooks.json` nor ships a
      Codex plugin.
    - `hook remove codex` takes out every Conspectus handler and the
      groups and lists that leaves empty. It says when that moved an
      operator's entry up a place, which may cost a review in `/hooks`.
    - Status reports per event as for Claude Code. A matcher covers its
      event only when it is absent or empty, since Codex reads a matcher
      as a regular expression.
19. **The mapping where the research left a choice.**
    - A `Stop` in plan mode is a finished turn (`ready/finished`). `ready`
      has no detail for a proposed plan, and only
      `last_assistant_message` could tell whether there was one, which is
      never read. A detail waits for a recording that shows how often it
      matters.
    - `PermissionRequest` is always a permission wait. The question tool
      (`request_user_input`) is not installed, since `PreToolUse`
      coverage for it is unverified.
    - An event that carries `agent_id` is the child's, as for Claude
      Code: its waits and tools count for the child, and its turn events
      and paths are dropped. `agent_type`, `tool_name`,
      `permission_mode`, and `turn_id` are never read.
    - Tier B (`PreModelRequest`, `PostModelRequest`) is not installed,
      and every event outside the subscribed set maps to nothing. The
      shared reducer needed no Codex-specific case.

    These fixtures are synthetic too, until a recording confirms them.

### OpenCode, as built

20. **The plugin keeps each session's state and sends snapshots.** The
    reducer (`plugins/opencode-hook/src/live.ts`) keeps per session its
    own state with its start, the pending permission, question, and
    form ids, how a 1.x turn will end, the state compaction interrupted,
    and the parent. A session shows, first match wins: `exited/ended`
    when it ended; its own wait (`permission`, then `question`, then
    `input`: a form other than the question tool's is input, since the
    vocabulary has no form detail); a descendant's wait, raised with
    `via_subagent`; `compacting`; `working` (`generating`, `retrying`,
    `rate_limited`); `error` with its class; `ready` (`finished`,
    `interrupted`, or none for a session without a turn yet).
    `subagents` counts the descendants working, waiting, or compacting.
    A wait shows its own start, and the session's own state its start
    once the wait ends. Child sessions get entries of their own too. A
    session nothing has reported sends nothing; one whose only evidence
    was a raised wait sends `unknown` when that wait ends.
21. **The payload.** `{session_id, cwd, hook_event_name, attribution?,
    live: {instance, seq, observed_ms, heartbeat_s, pid, pid_start,
    state, detail, subagents, via_subagent, since_ms}}`, sent on every
    change and every 60 s for a session that is working, waiting,
    compacting, or failed. The writer reads `live` against a whitelist
    (`src/discovery/harness/opencode_live.rs`): a name outside the closed
    sets is not copied, counts and the heartbeat are clamped, and no time
    runs ahead of its clock. The plugin's own pid, with its start time
    from `/proc/self/stat`, replaces the one the writer's parent walk
    finds, since the plugin knows which process it runs in.
22. **Attribution.** A payload takes the attribution path (ADR 0146) when
    it carries `"attribution": true`, or carries no `live`. The 1.x
    plugin marks a root session's lifecycle events (created, updated,
    status, idle, compacted), the events it forwarded before, so
    attribution still follows an in-app session switch; a child
    session's are never marked, so a pane no longer names a running
    subagent. The 2.x server entrypoint marks none, and the TUI
    entrypoint sends what it sent before, without live state. Snapshots
    and heartbeats reach only the live-state store. A new plugin with an
    older binary would attribute every snapshot, so the two are updated
    together.
23. **opencode 2.x gets a server entrypoint for state**, amending ADR 0126
    decision 3. `dist/v2/server.js` runs in the shared service, where
    every session is visible, including background children and turns
    that outlive their client. It subscribes through
    `ctx.event.subscribe` to the 2.x events of decision 3, plus
    `session.step.started` (to leave retrying) and `session.status` (in
    case 2.x sends it), keeps only its own location's events, and marks
    nothing for attribution. Listed once in `opencode.jsonc`, the
    package also loads the TUI entrypoint; listed only in `cli.json`, it
    gives attribution and no live state. The research's fallback of
    snapshots from a TUI's own stores is not built.
24. **The mapping where the research left a choice.**
    - Compaction ends in the state it interrupted (a turn with its start,
      or a ready prompt with its start), as the shared reducer does,
      rather than always in `working`. A 1.x status event during
      compaction ends it, since a failed 1.x compaction says nothing.
    - A 1.x `session.error` during a turn is held until the idle that
      ends it, and shows at once while idle; one with no session id is
      dropped. `MessageAbortedError` reads `ready/interrupted`,
      `ProviderAuthError` auth, and an API error by status: 429 rate
      limited, 401 and 403 auth, 503 and 529 overloaded. Anything else is
      failed.
    - A 2.x failed execution is classed by its error's status as above, a
      `retryAfterMs` as rate limited, then the words of its type (auth,
      overload, rate), else failed. A retry is rate limited for a 1.x
      action reason or a 2.x status 429 or `retryAfterMs`. Neither the
      reason nor the type is copied.
    - An execution interrupted by `user`, `superseded`, or `inactivity`
      reads `ready/interrupted`. One interrupted by `shutdown` ends
      (`exited/ended`): the vocabulary has no suspended state and the
      process is going away; the service's next start reports it afresh.
    - The instance going away (1.x `dispose`, the 2.x plugin unloading)
      ends every session still working, compacting, or waiting. A ready
      or failed one stays, and its process's death tells the rest.
    - A permission `reject` clears every pending permission of its
      session, as opencode rejects them all. A form whose session is
      `global` is no session's wait. A wait marks a turn running when
      none was seen, as when the plugin loaded mid-turn.

    The event scripts that encode these
    (`tests/fixtures/hooks/opencode-live/`) are synthetic until a
    recording confirms them. The plugin's tests replay them through the
    reducer and the Rust tests replay the emitted payloads through the
    parser and the store.

### Pi, as built

25. **The extension keeps the session's state and sends snapshots.** The
    state machine in `plugins/pi-hook/conspectus-hook.ts` keeps whether a
    run is active and when it began, a compaction and its start, the open
    dialog and its start, the top-level tool calls in flight (and which of
    them call a subagent tool), how the last assistant message ended, the
    error class, a retry in progress, and a 429 answer. A session shows,
    first match wins, as Pi's own status reporter orders it: `waiting`
    while a dialog is open; `compacting`; while a run is active `working`
    (`rate_limited` for a retry a rate limit holds back or after a 429,
    else `retrying`, `tool`, `generating`); otherwise how the last run
    ended: `error` with its class, `ready/interrupted`, `ready/finished`,
    or `ready` with no detail before any run. `subagents` counts the calls
    of the subagent tools in flight (`subagent`, the reference extension's
    name); a call another tool makes (`parentToolCallId`) is that tool's
    work. A wait shows its own start, a run its start, and a resting state
    its own, so a compaction while idle does not make a finished turn look
    new. Pi loads one file, so the machine shares the module with the
    handlers: the module imports only Node built-ins and Pi's types
    (`import type`), does nothing on import, and exposes the machine to its
    tests through one export documented as test-only, since no source
    confirmed what Pi does with exports besides the default.
26. **The payload and when it is sent.** `{session_id, cwd,
    transcript_path, hook_event_name, live: {instance, seq, observed_ms,
    heartbeat_s, pid, pid_start, state, detail, subagents, since_ms}}`,
    read against the whitelist the opencode payload uses
    (`src/discovery/harness/plugin_live.rs`). A change to or from
    `waiting`, `ready`, `error`, `exited`, or `compacting` is sent at once
    and drops a held one; a change between two working sub-states is held
    for 500 ms from the first such change, not restarted by later ones,
    and the latest is sent. The heartbeat resends the snapshot 60 s after
    the last one while the session is not ready. Both timers are unref'd,
    cleared at shutdown, and never touch a captured context. Each writer
    is spawned detached and unref'd with its payload already written to
    its stdin, never awaited, and every error is swallowed. Beside
    decision 4's events the extension reads `after_provider_response`,
    for its status alone; a Pi that never fires it loses only the 429
    detail. Nothing is subscribed that streams or that can change what Pi
    does.
27. **Attribution and the process.** A payload named
    `session_start:<reason>` (a reason outside Pi's five reads `other`)
    takes the attribution path, as before, and so does one without
    `live`, as the first extension version sent; a `live:<event>` payload
    never does. The extension's `process.pid` and its start time replace
    the process the writer's parent walk finds, for the snapshot and for
    the attribution record (amending ADR 0121). `instance` is fresh per
    extension load and `seq` counts within it.
28. **The mapping where the research left a choice.**
    - `ctx.ui.custom()` dialogs are no wait. Pi fires `ui_prompt_start`
      for every one, loaders and widgets included, and does not say which
      block, so marking them would raise false alarms; a dialog kind Pi
      adds later is no wait either. A `confirm` is a permission; a
      `select`, `input`, or `editor` a question, which is how a plan
      approval and the permission-gate example's select read.
    - An error is classified inside the extension from the status of the
      request's latest error answer (429 rate limited, 401 and 403 auth,
      503 and 529 overloaded, another 5xx a provider error) and else from
      the message's words. Rate limits, auth, and overload keep their
      detail; a context overflow, a network or provider error, and an
      unknown one read `failed`. Neither the words nor the class leave the
      extension.
    - Quitting, `/new`, `/resume`, and `/fork` end the session left,
      `exited/ended`: the vocabulary has no detached state, and a snapshot
      source ends its own sessions (ADR 0204 decision 5). A session start
      that no shutdown preceded ends the session it replaces. `/reload`
      sends nothing from the old load, and the new load reports the
      session afresh: `ready` with no detail, or `working` when Pi is not
      idle. An `exited` from the old load could, with an equal clock
      reading, replace the new load's report.
    - A compaction ends in the state under it. One neither cancelled nor
      retried is `error/failed`, at once when idle and when its run settles
      otherwise; a compaction with `willRetry` keeps or starts the run.
    - A settle without a run is ignored. A run is interrupted by the
      settle's `aborted` flag or, before Pi 1.1.0, by the last stop
      reason; the stop reasons `length` and `deferred` read finished, as
      Pi's reporter reads them.
    - Pi's version is not reported, since no exported version was
      confirmed; `hook status pi` states the floor instead.

    The event scripts that encode these
    (`tests/fixtures/hooks/pi-live/`) are synthetic until a recording
    confirms them. The extension's tests replay them through the state
    machine and the Rust tests replay the emitted payloads through the
    parser and the store.

## Consequences

- Claude Code pays two small spawns per tool call (`PostToolUse` and its
  failure variant fire for every tool, to leave `waiting` promptly). The
  measured cost is about 5 to 10 ms against tool latency.
- The installers need work: Claude's matcher misses `fork`; Codex's
  installer replaces an operator's array-of-tables hooks and sets no
  timeout. These are fixed as part of the harness stories.
- The plugin reducers are TypeScript. The repository has no Node in its
  dev shell, and a Rust mirror of a TypeScript reducer would test the
  mirror, not the plugin. So each reducer is a pure module (no imports
  from the harness package, no side effects on import, only syntax Node
  can strip) with its own `node --test` tests, run by hand on a machine
  with Node 22.6 or later (a `just` recipe wraps it and says so when Node
  is missing). The scripted event fixtures are shared data: the TypeScript
  tests replay each script through the reducer and compare the snapshots
  it emits, and the Rust tests feed the same expected snapshots through
  the payload parser and the store. Neither CI nor the dev shell gains
  Node here; that, and a CI run of the plugin tests, is a separate
  decision (a plugin test runner in its own ADR).
- Gaps remain by nature: an Esc interrupt in Claude fires no `Stop`; an
  approval granted fires no event until the tool ends (so `waiting` can
  linger through a long approved tool); a killed process says nothing.
  The first is repaired by the next turn event or the quiet window, the
  second by the next tool event, the third by pid liveness. ADR 0207
  lists the sources that could close them.

## Alternatives Considered

- **Subscribe to everything.** Gives a richer "what is it doing" detail at
  a cost of a spawn per tool event and a larger privacy surface. The
  minimal set answers the three questions.
- **Tool-level events only when asked.** Unrestricted `PreToolUse` (for a
  "running Bash" detail) is a candidate for a later opt-in; the state
  model does not need it.
- **A Claude Code plugin instead of settings edits.** Avoids rewriting the
  operator's settings, but plugin installation writes under
  `~/.claude/plugins`, outside ADR 0143 category 5 as written. Parked.
- **Detached writer processes.** Would hide spawn latency, but a detached
  event can finish after a later one. The lock-ordered, synchronous,
  bounded writer is simpler to reason about.

## Open Questions Answered

- *Should `hook init` upgrade an existing install silently?* No. It
  upgrades when the operator re-runs it, and `hook status` says when an
  install is partial.
- *Preserving key order in `settings.json`.* Enabling `serde_json`'s
  `preserve_order` adds a transitive dependency, which needs its own ADR.
  Until then `hook init` keeps values and re-sorts keys, as today, and
  the plan files a story for it.
- *Codex: Tier A only, synchronous, and where when a `hooks.json`
  exists?* Tier A, synchronous, 5 s timeouts, and always `config.toml`
  (decisions 17 and 18). `notify` is never touched, and trust is only
  ever explained, never written or read.
- *Which OpenCode and Pi versions?* OpenCode 1.x and 2.x both get a
  reducer. Pi's waiting state needs 0.84.4 or later; older versions
  report the other states, and `hook status pi` says so. 1.1.0 or later
  is recommended.
- *Pi's custom dialogs: waiting, a state of their own, or nothing?*
  Nothing (decision 28): the event cannot tell a dialog that waits from a
  loader.
