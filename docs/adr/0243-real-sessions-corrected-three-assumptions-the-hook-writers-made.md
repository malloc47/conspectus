# ADR 0243: Real Sessions Corrected Three Assumptions The Hook Writers Made

## Status

Accepted. Amends [ADR 0206](0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md)
(decision 7, the handler timeout, and the claims of decision 19) and
[ADR 0028](0028-hook-sidecar-mux-attribution.md) (which hook process
stands for a session). Confirms [ADR 0217](0217-a-codex-panes-title-fills-in-its-live-state-where-hooks-cannot.md)
and [ADR 0218](0218-claude-codes-agent-list-repairs-a-live-state-its-hooks-left-stuck.md)
against real output. Follows [ADR 0242](0242-a-hook-trace-records-what-hook-write-receives-with-every-free-text-value-redacted.md).

## Context

The reducers, the fixtures, and the research notes were built from each
harness's documentation. On 2026-11-16 real sessions of Claude Code
2.1.322, Codex 0.167.0, and Pi 1.5.2 were driven in a private tmux
server, with a scratch configuration directory per harness and a hook
trace (ADR 0242) on, and the sequences compared with the reducers.
OpenCode was left out by the operator. The recordings are in
`tests/fixtures/hooks/recorded/`.

Most of what the documentation promised held. The reducers needed no
change for Claude Code, and the synthetic Claude Code sequences are the
real ones to the event. Three assumptions did not hold.

1. **Codex's hooks run in a shared process.** The terminal UI of Codex
   0.167 attaches to a managed `codex app-server --managed-daemon` that
   starts as a child of the first client, keeps that client's
   environment, and runs every session's hooks. A hook's `TMUX_PANE` is
   the first client's pane for every session after the first, and the
   process the writer finds by walking up from the hook is the app
   server, not the session's terminal UI. The research expected this
   only for an explicit `codex --remote`, and the one flag the writer
   has for it, `background_harness`, was never set for Codex.
2. **Codex clamps two hook timeouts.** `hook init codex` wrote `timeout =
   5` on every event. Codex keeps 3 s at most for `SessionEnd` and
   `Interrupt`, clamps a longer one, and warns on every start
   (`clamping SessionEnd hook timeout to 3s`, shown as warnings in the
   status line).
3. **Pi's last report never arrived.** The Pi extension started the
   writer and left it, relying on Node writing a child's stdin before the
   call returns. Pi is a Bun executable, which writes it later, so a
   quit or a closed pane, where Pi exits at once, reached the writer as
   an empty stdin: no `session_shutdown` landed, and the session read as
   whatever it last was until its pid was found dead.

## Decision

1. **A `codex app-server` process is a background process.** The Codex
   runtime signature's `is_background_process` is true when the
   command's subcommand (its first argument that is not a flag) is
   `app-server`, as opencode's is for `serve` and `acp`. Two things
   follow from the existing background rule. A hook the app server runs
   records no tmux context, so a later session is never shown in the
   first client's pane (the entry binds to its session by id, ADR 0205
   rule 1). And discovery no longer reads the app server as an agent's
   own process.
   - The recorded `pid` of such an entry is the app server's. A session
     whose terminal UI is gone ends when the app server unloads its
     thread, which the recording saw as `SessionEnd` about 60 s later,
     for a closed pane, a `kill -9`, and a cleared session alike. That
     is later than a pid check would be, and it is not a pid check's
     "never": it is accepted. A terminal UI started with `--no-daemon`
     runs its own hooks, so its pid and pane are its own and a crash is
     found by the pid.
2. **The Codex install writes 3 s for `SessionEnd` and `Interrupt`** and
   5 s for the other events. A handler is current for those two events
   when its timeout is positive and at most 3 s, and for the others when
   it is positive, so an install made before this change reads
   `outdated` for those two and `hook init codex` rewrites them. A
   timeout the operator chose within the limit is kept (ADR 0206
   decision 7 stands for every other event). Codex hashes the entry it
   has clamped, so the trust recorded for a 5 s entry holds for the 3 s
   one: with the installer's rewrite of a scratch config, Codex showed
   every hook active and asked for no review.
3. **The Pi extension's report that a session ended runs the writer to
   completion** (`spawnSync`, input on stdin, at most 2 s) and every
   other report is still started and left. The cost is the writer's few
   milliseconds, once, as Pi exits. It is checked by a test that the
   report is on disk when the handler returns, which fails against the
   old code, and by real Pi: a quit and a closed pane now each report
   `exited`.
4. **The rest of what the recordings showed is recorded and changes no
   code.**
   - Claude Code: a denied prompt, a rejected plan, and an Esc interrupt
     fire no hook (and no `idle_prompt` follows an interrupt), so the
     state stays `waiting` or `working` until the next prompt or the
     agent-list repair of ADR 0218, which the recording confirms lists
     `idle` for each. Compaction runs `PreCompact`, `SessionStart`
     (`compact`), `PostCompact`, in that order. A closed pane fires
     `SessionEnd` with reason `other`. The `StopFailure` `error` words
     seen are `authentication_failed`, `rate_limit`, `server_error` (for
     a 500 and a 529, with retries off), and `unknown`; `overloaded`,
     which the mapping keeps, did not occur.
   - Codex: a plan-mode `Stop` carries `permission_mode` `default`, so
     the plan-approval choice shows in neither a hook nor the pane title.
     A declined approval fires `Interrupt`. A question
     (`request_user_input`) fires no installed hook while it is open and
     `PostToolUse` when answered; the title's `Action Required` mark
     covers the wait (ADR 0217). `SessionStart` comes with the first
     prompt, not at launch.
   - Pi: a manual compaction of a session too small to compact reports
     `session_compact_failed` and reads `error` until the next event.
   - The recorded fixtures and the notes' appendices carry the full list.

## Consequences

- A Codex session's live state is no longer attributed to a pane by its
  hooks when an app server ran them. Binding by session id carries it,
  and a new session with no transcript yet shows no state until its
  first prompt creates one, which is when Codex sends `SessionStart`
  anyway.
- Operators with Codex installed run `conspectus hook init codex` again
  (no new review is needed); until then the two closing entries warn at
  every start, as they do today. The recording saw the warnings gone
  after the rewrite.
- The Pi extension blocks for one writer run as Pi exits, bounded by 2 s.
- The recordings become fixtures. Replaying them through the adapters as
  a regression test is its own task.

## Alternatives considered

- **Drop the pid of a background harness's entries.** A session whose
  terminal UI is gone would then wait for the time rules (ten quiet
  minutes) instead of the app server's own `SessionEnd` about a minute
  after. Rejected: the recording shows the app server reports the end.
- **Install `timeout = 3` for every Codex event.** Rejected: only two
  events are clamped, and the others have no reason to give up headroom.
- **Write every Pi report synchronously.** Rejected: a tool's start and
  end would each cost the writer's run on the agent's own thread. Only
  the last report needs it.
- **Detect Codex's app server by environment or by `TMUX_PANE` mismatch.**
  Rejected: neither is visible to the writer, and the command line is
  what the existing background rule already reads.

## Open questions answered

- *Does the Codex terminal UI run its hooks itself?* No, not by default;
  an app server does, shared across terminal UIs. `--no-daemon` makes the
  UI run them.
- *Is trust recorded at `hooks.state.<key>`?* Yes, under the key
  `<config path>:<event>:<group>:<hook>`. A copied configuration needs its
  keys rewritten, and removing a group renumbers the rest and asks for a
  new review.
