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

## Consequences

- Claude Code pays two small spawns per tool call (`PostToolUse` and its
  failure variant fire for every tool, to leave `waiting` promptly). The
  measured cost is about 5 to 10 ms against tool latency.
- The installers need work: Claude's matcher misses `fork`; Codex's
  installer replaces an operator's array-of-tables hooks and sets no
  timeout. These are fixed as part of the harness stories.
- The plugin reducers are TypeScript. The repository has no Node in its
  dev shell, so plugin logic is verified by fixtures replayed through a
  Rust mirror used only in tests, plus manual runs, until a plugin test
  runner is justified in its own ADR.
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
  report the other states.
