# ADR 0093: Operator-Initiated Mux Teardown (`kill-session`)

## Status

Accepted

## Context

The worktree interaction epic (CSP-520, `docs/backlog.md`) settles on
a **"close down a stream of work"** gesture: land or discard a
worktree's branch, remove the worktree, delete the branch, drop the
now-stale pin, and — critically — **end the running mux session and the
agent process inside it**. A stream of work is a worktree plus,
usually, a tmux session rooted there running an agent; closing it down
without ending that session leaves an orphaned agent whose working
directory just got removed.

Conspectus can create and rename mux sessions but cannot **end** one.
`MuxBackend` (ADR 0089) exposes `list_sessions`, `rename_session`,
`new_session`, `capture_pane`, `send_keys`, and `attach_session` — no
`kill_session`. The mutation envelope (ADR 0087) sanctions mux
lifecycle in category 3 (rename / new-session / launch-scoped
send-keys / attach) but **does not list session termination**. So
"close down" cannot end the mux under today's rules.

Two adjacent prohibitions bound the decision:

- **ADR 0028 (terminal injection, absolute).** Conspectus never
  `send-keys` arbitrary input into a live agent pane. A naive teardown
  — typing `C-c` or `exit` into the agent — is exactly this and is
  off-charter.
- **ADR 0087 prohibition 1 (harness-native state).** Conspectus never
  mutates a harness's own session records
  (`~/.claude/`, `~/.codex/…`, opencode's db, …).

The question this ADR answers: **may Conspectus terminate a tmux
session (ending the agent process inside it) as part of an
operator-initiated teardown, and if so under what constraints?**

## Decision

Sanction **`tmux kill-session`** (and its backend-neutral peer
`MuxBackend::kill_session`) as an **operator-initiated teardown
mutation** under ADR 0087 category 3. It is available only through an
explicit operator gesture on a mux the operator has selected, gated by
a configurable confirmation, runs graceful-first, and never runs in
the background.

### Two-phase teardown: graceful, then hard

Termination is **not** an immediate `kill-session`. Teardown gives the
running agent a chance to shut down cleanly first:

1. **Graceful.** Send `SIGTERM` to the session's foreground process
   (the pane PID Conspectus already observes as
   `MuxSessionNode.active_pane_pid`) so the agent can flush and exit on
   its own. This is a **signal**, not `send-keys` — no characters enter
   the agent's stdin, so ADR 0028 is untouched.
2. **Grace period.** Poll for the session to exit for a short,
   **configurable** window (`[worktree] teardown_grace`, default a few
   seconds).
3. **Hard.** If the session is still alive when the window elapses,
   escalate to `tmux kill-session -t <target>` (SIGHUP + reap).

The escalation is guaranteed: teardown never hangs waiting on a
wedged agent. The graceful phase is best-effort — if the pane PID is
unknown, the grace step is skipped and teardown goes straight to the
hard kill.

### Why this is not terminal injection (ADR 0028)

Terminating a session is **session lifecycle**, not input injection.
`kill-session` ends the session and its child processes; it does not
type characters into the agent's stdin, does not compose input on the
operator's behalf, and does not attempt to drive the agent's UI. The
ADR 0028 trust contract — "Conspectus never types *for* me into a live
session" — is untouched. This mirrors how `rename-session` (ADR 0029)
is a sanctioned mux mutation that likewise doesn't touch the pane's
stdin.

### Why harness state stays intact (ADR 0087 prohibition 1)

Teardown ends the **running process**. It does not delete the
harness's session record, transcript, or history — those remain on
disk exactly as the harness wrote them, and Conspectus continues to
read them read-only. "Closing down a stream" reclaims the working
tree and the terminal, not the agent's memory. Operator messaging says
so explicitly ("ends the running session; transcript is preserved").

### Constraints (all required)

1. **Operator-initiated, foreground only.** `kill_session` fires only
   from a direct CLI/TUI gesture (`worktree close`, the TUI close-down
   action, or an explicit `kill`), never from the daemon or any
   background pass (ADR 0087 prohibition 5 stays absolute).
2. **Targets a named, operator-selected mux.** Conspectus never
   enumerates every mux and terminates them; the target is the one the
   operator's gesture identifies. No wildcard / bulk kill of live
   sessions without per-session confirmation.
3. **Confirmation, per a configurable policy.** Because termination
   ends a running agent process, the gesture confirms — naming the mux
   and the agent(s) that will be ended — according to
   `[worktree] teardown_confirm` (see Configuration). The confirmation
   list is exactly what the live-session guard (H-WT-004a) already
   surfaces. Even under a `never` policy the operator still initiated
   the gesture; the policy governs the interstitial prompt, not
   whether the operation is operator-initiated.
4. **Blocking / foreground.** The whole two-phase teardown — graceful
   signal, grace wait, hard kill — completes before the dependent
   steps (worktree remove) run and before the graph is re-discovered,
   so the snapshot reflects reality and we never remove a worktree out
   from under a still-dying process.

### Configuration

Both the confirmation policy and the grace window live under
`[worktree]` (CSP-508's config block):

- `teardown_confirm = "always" | "live" | "never"` (default `"live"`).
  - `always` — confirm every worktree teardown / removal, even one
    with no live session.
  - `live` — confirm only when a live mux/agent would be terminated
    (the dangerous case); silent otherwise. This is the default and
    the natural extension of the H-WT-004a guard.
  - `never` — no interstitial prompt (trust / scripted use). The
    operator still triggered the gesture.
- `teardown_grace = "<duration>"` (default `"3s"`). The window between
  the graceful `SIGTERM` and the hard `kill-session`. `"0s"` skips the
  graceful phase entirely (immediate hard kill).

CLI flags override config per-invocation (`--yes` to skip the prompt,
`--grace <dur>`), matching how the rest of the CLI treats config vs
flags.
5. **Backend-neutral with a safe default.** `kill_session` joins
   `MuxBackend` with a default `Unsupported` outcome (like the other
   optional verbs), so backends that can't or shouldn't terminate
   sessions (or haven't implemented it — zellij, screen) simply report
   `Unsupported` and the teardown degrades to "remove the worktree only
   after you close the session yourself."

### Envelope placement

This extends ADR 0087 **category 3** (operator-initiated mux
lifecycle) with one verb: session termination. It introduces no new
category and removes no prohibition:

- Prohibition 1 (harness state) — untouched (process ends, records
  stay).
- Prohibition 2 (terminal injection) — untouched (kill ≠ send-keys).
- Prohibition 5 (background mutation) — untouched (operator-initiated,
  foreground only).

## Consequences

- "Close down a stream" becomes a single operator gesture instead of a
  chore that requires the operator to hand-terminate the agent first.
- The mutation envelope gains its first **destructive** mux verb.
  Termination ends a running agent process, so the graceful-first phase
  and the confirmation policy (constraints 2–3) are load-bearing, not
  decorative: the `SIGTERM`-then-grace window gives the agent a chance
  to exit cleanly, and the confirmation is where an operator catches a
  mistake before in-flight work is lost. The transcript survives;
  uncommitted *code* in the worktree is protected separately by the
  discard-vs-merge choice and worktrunk's untracked-file guard.
- `MuxBackend` grows `kill_session` (default `Unsupported`);
  `SystemTmux` implements the hard phase via
  `tmux kill-session -t <target>`. The teardown orchestrator owns the
  graceful `SIGTERM` + grace-poll (it has the pane PID), so the backend
  method stays a simple "end this session now" primitive. `FakeTmux`
  records the call for tests, matching the existing rename/new-session
  test seams.
- Two `[worktree]` config keys (`teardown_confirm`, `teardown_grace`)
  join `backend`, with CLI-flag overrides.
- CSP-522 (`close-down`) and a direct "kill session" affordance can
  now be built on a sanctioned primitive.

## Alternatives Considered

**Don't terminate; refuse close-down when a session is live.** The
operator stops the agent/mux themselves, then Conspectus removes the
worktree. Rejected per the settled scope (operator chose full
teardown); it makes the common "wind down this experiment" flow a
multi-tool chore.

**Send `C-c` / `exit` into the agent pane to stop it gracefully.**
Rejected — this is precisely the ADR 0028 terminal-injection
prohibition, which is absolute. Whatever the intent, Conspectus does
not type into a live agent pane.

**Delegate termination to worktrunk.** Rejected — worktrunk manages
git worktrees, not tmux sessions; `wt remove` does not (and should
not) kill an unrelated terminal multiplexer session.

**A separate "destructive mutation" category rather than extending
category 3.** Rejected — session termination is mux lifecycle, the
same class as rename/new-session, and belongs with them. The
destructiveness is handled by the confirmation constraint, not by a
new envelope category.

## Open Questions

- **Per-harness graceful quit.** The graceful phase is a generic
  `SIGTERM`. A harness that exposes a cleaner quit signal or command
  could get a per-harness graceful hook later; `SIGTERM` + grace is
  the backend-neutral v1. (A hook must still not be `send-keys` into
  the pane — ADR 0028.)
- **Attached sessions.** Terminating a session the operator is
  currently attached to detaches them. Acceptable (they invoked it),
  but the confirmation should note it when the target is the current
  session.
- **Multiple muxes in one worktree.** If more than one mux is rooted in
  the worktree, close-down confirms and terminates each; no implicit
  fan-out beyond the worktree's own sessions.

## Related ADRs

- ADR 0028 (hook sidecar / terminal-injection prohibition) — this ADR
  distinguishes session termination from input injection.
- ADR 0029 (lockstep session aliases) — the existing sanctioned
  `rename-session` mux mutation this sits beside.
- ADR 0087 (mutation envelope) — extends category 3 with session
  termination; cites this ADR from its Related ADRs.
- ADR 0089 (mux backend trait) — `kill_session` joins the trait with a
  default `Unsupported`.
- ADR 0092 (worktree backend seam) — the worktree teardown that drives
  the need.
