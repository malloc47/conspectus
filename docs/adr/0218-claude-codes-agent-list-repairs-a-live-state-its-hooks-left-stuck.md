# ADR 0218: Claude Code's Agent List Repairs A Live State Its Hooks Left Stuck

## Status

Accepted. The second live-state source beyond hooks under
[ADR 0207](0207-live-state-sources-beyond-hooks-need-their-own-decision.md)
(decision 4), approved by the operator as decision 11 in section 7.1 of
the live-state plan (`docs/plans/live-state/README.md`) after the check
recorded below. Uses the exception of
[ADR 0028](0028-hook-sidecar-mux-attribution.md) for a
harness-documented, non-mutating control channel audited as safe, for
live state only. Narrows what ADR 0207 decision 4 counts as a stale hook
record for this one source (decision 6). Adds a source to the vocabulary
of [ADR 0203](0203-live-state-is-a-neutral-observation-with-a-closed-state-set.md)
and a step to the binding of
[ADR 0205](0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md);
bumps the snapshot format to 14 (ADR 0083). Adds a subcommand of an
existing program and a signal to the capability manifest
([ADR 0117](0117-capability-manifest-and-traced-backstop.md)); writes
nothing (ADR 0143). Follows the template of
[ADR 0217](0217-a-codex-panes-title-fills-in-its-live-state-where-hooks-cannot.md).
Amended by
[ADR 0222](0222-claude-codes-agent-list-shows-a-known-agent-its-hooks-do-not-report.md)
(decisions 2, 7, and 10): the poll also runs while a running Claude Code
agent that discovery knows has no hook entry, and shows that agent's
state on its session from the poll alone, recorded with no pid so it
ages out when polls stop.

## Context

Claude Code's hooks (ADR 0206) leave two gaps that matter most
(`docs/plans/live-state/research/01-claude-code.md` §2.3, §6.4):

- **An Esc interrupt fires no `Stop`**, so the session reads `working`
  until the next prompt, and `quiet` after ten minutes.
- **An approval fires nothing**, so a session that asked for permission
  reads `waiting` through the whole run of the tool it was allowed to
  start, until `PostToolUse`.

Claude Code documents `claude agents --json` as the way to read session
state from outside: "Print active sessions (interactive and background)
as a JSON array and exit (for scripting; does not require a TTY)". Each
element carries, by the documentation (research §5.1): `pid` and
`status` (`busy`, `waiting`, `idle`) while the process is alive;
`waitingFor` while it waits (`permission prompt`, `input needed`,
`sandbox request`, `worker request`, `dialog open`); `sessionId` and
`name` when set (`name` can be a title generated from the prompt);
`cwd`, `kind`, and `startedAt`; and for background sessions `id` and
`state` (`working`, `blocked`, `done`, `failed`, `stopped`). None of it
has been seen in the output of a real session yet: the shapes come from
the documentation and are **unverified** until backlog `CSP-822`
records real output.

ADR 0207 put a check in front of it: does running it start Claude's
background supervisor, and what does a call cost?

### The check

Run with Claude Code 2.1.322, `env -i`, and `HOME`, every `XDG_*`
directory, and `CLAUDE_CONFIG_DIR` in a scratch directory, never against
the operator's configuration:

| Question | Finding |
|---|---|
| Does it start the supervisor? | No. It tries the supervisor's control socket (`/tmp/cc-daemon-<uid>/<hash>/control.sock`), finds none, and starts none. No new `claude` process, no socket or pid file is left behind. |
| Does it start any process? | No: one `execve` (itself) under `strace -f`. |
| Does it fire hooks? | No. A scratch `settings.json` with `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `Stop`, and `Notification` hooks that touch marker files: none fired. |
| Does it write? | The first run in an empty config directory created `.claude.json` and one timestamped backup under `backups/` (first-start initialization). Further runs created no file and left `.claude.json` unchanged. Not tested against a config that needs a version migration. |
| Does it reach the network? | Yes, by default: every run opened a TLS connection to an Anthropic address on port 443. With `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` it opened none; the only connection left was the attempt on the supervisor's local socket. `DISABLE_TELEMETRY=1` alone did not stop it. |
| What does it cost? | About 0.6 s wall time (the coordinator's run), 0.26 to 0.34 s in later runs (0.48 to 0.76 s under `strace`), exit 0, `[]` on stdout, nothing on stderr. |
| What does a refusal look like? | With `CLAUDE_CODE_DISABLE_AGENT_VIEW` set it exits 1 with one line on stderr. |

So it does not start a supervisor when none exists, and an initialized
config is not rewritten by repeated runs. At a third to two thirds of a
second per call it is a repair, not a per-refresh read.

## Decision

1. **What runs.** `claude agents --json`, the `claude` found on `PATH`,
   through `exec::command` (`Program::Harness`), with stdin from
   `/dev/null`, stderr discarded unread, and stdout read up to 1 MiB.
   The child gets Conspectus's environment plus
   `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, so a poll reaches no
   network. It is killed (`SIGKILL`) and reaped when it has not exited
   after 3 s. No other `claude` subcommand or option is run.

2. **When it runs: only as a repair.** The poll belongs to the
   processes that already discover in the background and keep a poller
   for their lifetime: the daemon's class cycles (`conspectus serve`)
   and the background refresh worker of a TUI with no daemon. A
   one-shot command (`table`, `graph`, `node show`, with or without
   `--refresh`), any discovery on the TUI's UI thread, and the TUI's
   one-second overlay never run it. On each discovery the live-state
   pass runs it when both hold:
   1. at least 30 s have passed since the last attempt, whatever its
      outcome;
   2. some Claude Code binding is **due**: its store entry recorded
      `working` or `waiting` from hook events, it bound to a session or
      a pane, its process was not found dead, it has had no event for
      the settle window of 10 s, and no earlier poll has already
      answered it. A poll answered it when the poll ran at least the
      settle window after the entry's latest event (so decision 6
      applied it) and either did not list the agent (or listed it with
      nothing this ADR maps) or read it `ready`, `error`, or `exited`,
      states that only a hook event (the next prompt) ends.
      A poll that read `working` or `waiting` leaves the entry due, so a
      wait answered later, or work interrupted later, is still found.

   A quiet entry (no event for ten minutes) and a wait no confirmation
   followed (research §6.4 item 4) are both entries with no event for
   the settle window, so both are due. While every Claude Code session
   is ready, or keeps sending events, nothing runs.

3. **When it fails, it says nothing.** No `claude` on `PATH`, a
   non-zero exit, a timeout, output over 1 MiB, or output that is not a
   JSON array of objects asserts nothing: no binding changes, nothing
   is printed or logged, and the attempt counts for the 30 s limit. The
   poller keeps the last poll that succeeded.

4. **What it asserts.** Per listed session, from `status`, or from
   `state` when `status` is absent or holds none of the values below:

   | Field | Value | Live state |
   |---|---|---|
   | `status` | `busy` | `working`, detail `generating` |
   | `status` | `waiting` | `waiting`, detail from `waitingFor` |
   | `status` | `idle` | `ready`, no detail |
   | `state` | `working` | `working`, detail `generating` |
   | `state` | `blocked` | `waiting`, detail from `waitingFor` |
   | `state` | `done` | `ready`, detail `finished` |
   | `state` | `failed` | `error`, detail `failed` |
   | `state` | `stopped` | `exited`, detail `ended` |
   | `waitingFor` | `permission prompt`, `sandbox request` | detail `permission` |
   | `waitingFor` | anything else, or absent | detail `input` |

   Values compare after trimming and lowercasing. Any other `status` or
   `state`, a field of the wrong type, or neither field asserts nothing
   for that session. `idle` reads `ready` with no detail: the list
   cannot tell a finished turn from an interrupted one, and the turn it
   repairs is usually an interrupt. `input needed` (a question or an MCP
   form) reads `input` because the list does not say which.

5. **Which agent is which.** A listed session is the same agent as a
   store entry when both pids are known and equal and, when both
   start times are known, so are those (the poller reads the listed
   pid's start time from `/proc` when it polls). When either pid is
   unknown, they match when both session ids are known and equal. A
   pid that differs never matches, whatever the session ids say. The
   hook writer records Claude Code's own pid (`CLAUDE_PID`, else the
   nearest `claude` ancestor); that it is the pid the list reports is
   **unverified**.

6. **How it combines with hook evidence.** The live-state binding (ADR
   0205 decision 3) folds the remembered poll in after the store's
   entries and the pane titles (ADR 0217), at the poll's time, so the
   discovery pass and the TUI's overlay apply the same rule to the same
   snapshot. A Claude Code binding that still holds its store entry's
   hook observation gives way to the poll's reading of the same agent
   when all of these hold:
   - the entry recorded `working` or `waiting`, has not exited, and its
     process was not found dead;
   - the poll ran at least the settle window (10 s) after the entry's
     latest event;
   - the poll asserts a state other than the entry's.

   The binding then shows the poll's state and detail with source
   `harness_poll`, observed at the poll, since the first poll of the
   agent's current run of that state that came after the entry's
   latest event (else this poll). It keeps the entry's process, the
   liveness found for it, and its subagent count. The store's entry is
   not rewritten; the next hook event is newer than the poll and takes
   over again.

   So a fresh hook event always wins: one newer than the poll, or less
   than the settle window older. A hook entry's `ready`, `error`,
   `compacting`, `starting`, `unknown`, and `exited` are never changed
   by a poll, nor a poll's reading equal to the entry's. ADR 0207
   decision 4 lets a poll override "a stale or absent hook record";
   for this source a hook record counts as stale once it has been
   silent for the settle window in `working` or `waiting`, the only two
   states whose end Claude Code's hooks can miss.

7. **Sessions with no hook entry get nothing.** A poll never creates a
   binding. The poll runs only while some hook entry is due, so a
   session without hooks would show a state only when another session
   happened to be stuck, and then keep it: `ready` and `waiting` do not
   age while the process lives (ADR 0203), so a session that started
   working afterwards would read `ready` indefinitely. A session with no
   events is already made visible by `hook status` (`silent`, ADR 0207
   decision 3). Polling for hook-less sessions needs a cadence of its
   own and is backlog `CSP-871`.

8. **What the poll keeps.** Its output is read in memory against a
   typed whitelist, a Tier 1 state reader whose result is Tier 3 (ADR
   0086): unknown keys are skipped by the parser without being kept,
   and every field read is reduced at once.
   - Kept, per listed session with a state and a pid or session id to
     match it by: the pid (above 1), the session id when it is a plain id
     token (ASCII letters, digits, `-`, `_`, `.`, `:`, at most 128
     characters), the mapped state and detail, when its run began, and
     the start time read for its pid; for the poll, when it ran.
   - Dropped at the parse: `cwd`, `name`, `kind`, `startedAt`, `id`,
     every other field, and the text of `status`, `state`, and
     `waitingFor` (only the mapped enum is kept). At most 256 sessions
     are kept.
   - Never: anything on stderr, any text in an error, a log line, or
     the store. The session ids never feed attribution or the
     resolver.

9. **Where it rides.** The poller holds the last successful poll in
   memory and puts it on every snapshot its process builds, as
   `GraphSnapshot::live_polls` (by harness key: one `LivePoll` with its
   time and its `PolledAgent`s), so a daemon snapshot carries it to the
   TUI and the overlay binds against it. It rides in `graph.bin` and
   stays out of JSON, as the mux probes do; its effect shows in
   `node_live` and `pane_live`. A one-shot command's own discovery has
   no poller and an empty table. `graph.bin` moves to format version 14
   (the table and the source name).

10. **Where it shows.** Bindings from this source say `source:
    harness_poll` and otherwise read like any other: the row cell, the
    header's and group headers' counts, `W`, the detail pane, `node
    show`, `table --columns +live`, and the graph JSON. No glyph, column,
    or key is added.

11. **The switch.** Setting `CONSPECTUS_DISABLE_LIVE_POLL` builds no
    poller, so `claude` is never run for live state.

12. **Capabilities.** `[programs.harness]` gains `read = ["agents
    --json"]` and cites this ADR; `[signals]` cites it for the `SIGKILL`
    of a child past its timeout. The traced capability test runs the
    daemon with a stand-in `claude` and a stuck entry, so the call is
    checked against the manifest.

## Consequences

- With `conspectus serve`, an Esc-interrupted Claude turn reads `ready`
  and an approved prompt reads `working` within about 10 to 45 s (the
  settle window, up to the 30 s limit, and a class cycle) instead of at
  the next prompt or when the approved tool ends. A daemonless TUI
  repairs at its next refresh past the same window. A one-shot command
  never repairs on its own; it shows a running daemon's repair.
- A dialog the hooks do not report (`sandbox request`, `dialog open`)
  while a hook says `working` reads `waiting` once the hook has been
  silent for the settle window.
- Cost: one call of a third to two thirds of a second, at most every
  30 s, while some Claude Code session works without events (a long
  tool) or waits on the operator. An interrupted session left idle
  costs one call. A host where every session is ready costs none.
- The poll reaches no network because Conspectus turns Claude Code's
  nonessential traffic off for the child. A future Claude Code that
  ignores that variable would open a connection per poll; the switch
  turns the poll off.
- A Claude Code update whose first start migrates its configuration
  could do that migration on the first poll after the update instead of
  on the operator's next session, as any start would.
- A poll's `ready` holds until the next hook event, like a hook's own
  `ready`; if a session started working with no hook event at all, it
  would read `ready` until one came.
- The output shape is from the documentation, not a recording, and so
  is the pid match; a recording (backlog `CSP-822`) changes the parser's
  table and nothing else.

## Alternatives Considered

- **Run it on every refresh.** A third to two thirds of a second per
  call and a network connection by default, for a state the hooks
  already report almost all of the time. Rejected; ADR 0207 alternative.
- **Read `~/.claude/sessions/` directly.** Cheaper, but the file format
  is undocumented and not stable (research §5.2); it is the operator's
  parked question 4.
- **Poll on demand** (a key or a flag). The gaps are the cases the
  operator does not know to ask about. Rejected.
- **Bind sessions with no hook entry.** See decision 7: their state
  would appear by accident and not age. Deferred to backlog `CSP-871`.
- **Let a poll override a fresh hook record.** A hook event is the
  harness speaking at the moment of change, the poll a sample up to
  30 s old. Rejected; only silence for the settle window lets the poll
  speak.
- **Stop polling once a poll agrees with the hook.** An agreeing
  `working` or `waiting` is exactly the state that can end without an
  event later. Rejected; only answers that only a hook event can end
  stop the polling.
- **Keep the poll's text** (`name`, `cwd`). Payload-adjacent and not
  needed to match an agent. Rejected.
- **A new `[programs]` entry for the poll.** The manifest finds a
  program's entry by its name, and `claude` already belongs to
  `harness`. Rejected for a `read` pattern on that entry.

## Open Questions Answered

- *Does it start Claude's background supervisor?* No (the check).
- *Does it change anything?* Not on an initialized configuration, and
  it fires no hook. Its first run ever initializes Claude Code's own
  config, as any start does; a hook entry, and so a poll, needs Claude
  Code to have run already.
- *Does it reach the network?* By default yes, so Conspectus sets
  `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` on the child.
