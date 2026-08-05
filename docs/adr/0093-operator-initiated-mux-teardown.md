# ADR 0093: Operator-Initiated Mux Teardown (`kill-session`)

## Status

Proposed

## Context

The worktree interaction epic (H-WT-ENV, `docs/backlog.md`) settles on
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
a confirmation, and never runs in the background.

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
3. **Explicit confirmation.** Because termination ends a running agent
   process, the gesture requires a confirmation step that names the
   mux and the agent(s) that will be ended. The existing live-session
   guard (H-WT-004a) surfaces exactly this list.
4. **Blocking / foreground.** The kill completes before the dependent
   steps (worktree remove) run and before the graph is re-discovered,
   so the snapshot reflects reality and we never remove a worktree out
   from under a still-dying process.
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
  Termination ends a running agent process, so the confirmation +
  clear messaging (constraint 3) are load-bearing, not decorative:
  this is the point where an operator could lose in-flight agent work
  if the confirmation is sloppy. The transcript survives; uncommitted
  *code* in the worktree is protected separately by the discard-vs-merge
  choice and worktrunk's untracked-file guard.
- `MuxBackend` grows `kill_session` (default `Unsupported`);
  `SystemTmux` implements it via `tmux kill-session -t <target>`.
  `FakeTmux` records the call for tests, matching the existing
  rename/new-session test seams.
- H-WT-006 (`close-down`) and a direct "kill session" affordance can
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

- **Graceful vs. abrupt.** `kill-session` is abrupt (SIGHUP/SIGKILL to
  the pane processes). Should teardown first attempt a gentler path
  (e.g. the harness's own quit command if one exists)? For v1: no —
  kill is abrupt and the confirmation makes that explicit; a
  per-harness graceful-quit hook can be a follow-up.
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
