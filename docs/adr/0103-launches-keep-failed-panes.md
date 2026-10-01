# ADR 0103: Launches Keep Failed Panes And Confirm The Harness Started

## Status

Accepted. Amends ADR 0087 category 3 (mux lifecycle), ADR 0057
(pin launch), ADR 0058 (resume fallback) and ADR 0096 (mux launch).

## Context

`tmux new-session -d` succeeds as soon as the session exists. If the
command inside exits a moment later, tmux tears the session down and
its output goes with it. Conspectus saw only the next step fail.

Pin `conspectus-worker1` hit this on every launch. Its ADR 0058
sidecar recorded a Claude Code session whose transcript held only
metadata lines (title, permission mode) and no turns. Discovery still
emits a session node for that file, so the resume gate spliced
`--resume <id>`. Claude Code printed "No conversation found with
session ID …" and exited 1 after about 0.9 s. The TUI's next call,
`tmux has-session`, failed with "can't find session:
conspectus-worker1", which was the only message the operator saw,
in a toast. Every later launch retried the same resume.

ADR 0058 already says resume falls back to a fresh launch "on every
honest failure path", but it can only check what is visible before
launch: sidecar present, session on disk, no fork. A harness refusing
the session at startup is a failure only the harness can report.

## Decision

1. **Keep failed panes.** Every session Conspectus creates
   (`pin launch`, `mux launch`, `mux new`) is created as
   `tmux new-session -d … \; set-option -t <name> remain-on-exit
   failed`. The option is chained in the same tmux invocation so it is
   set before the server reaps a fast-exiting child. A process that
   exits non-zero leaves a dead pane with its output and tmux's
   "Pane is dead (status N)" banner. Clean exits still end the session.
   On a tmux too old for the `failed` value, the session is still
   created, without retention.
2. **Confirm the start.** After creating the session, `pin launch` and
   `mux launch` poll the pane (`MuxBackend::pane_status`, `tmux
   display-message '#{pane_dead} #{pane_dead_status}'`) for a short
   window: 1.5 s after a resume launch, 0.5 s after a fresh one. A
   dead pane, or a session that already vanished, counts as a failed
   start. A runner that can't report pane status is assumed started,
   which matches the old behavior.
3. **Resume falls back on a failed start.** If a resume launch dies,
   Conspectus captures the pane text, removes the dead session, clears
   the pin's sidecar, launches with the pin's fresh argv, and reports
   the harness's message as a warning. If a fresh launch dies,
   Conspectus removes the dead session and fails with the pane text,
   so the next launch starts clean and the error survives in the
   launch outcome.
4. **Dead panes are replaced, not typed into.** A `PinStaleMux` pin
   whose pane is dead (a failure after the watch window) is relaunched
   by removing the session and creating it again, not by `send-keys`.
   A dead pane takes no input.
5. **Captures include scrollback.** `capture-pane` adds `-S -100`. The
   dead-pane banner scrolls the harness's first line off screen, and
   previews crop to the lines they show anyway.

### Mutation envelope

ADR 0087 category 3 gains two operations. Both are triggered by the
operator's launch gesture and both target only the session that
launch is creating or replacing:

- setting `remain-on-exit failed` on a session Conspectus is creating;
- `kill-session` on a session whose pane is dead, as part of `pin
  launch` or `mux launch` (ADR 0093 sanctioned `kill-session` for
  teardown only).

Neither touches a live process or a pane's input (ADR 0028), and
neither touches sessions Conspectus did not create in that gesture,
apart from replacing a pin's own dead session.

## Consequences

- A harness that fails at startup now says why, in the CLI's stderr
  and in the TUI's message log (ADR 0105).
- Resuming a session the harness no longer accepts costs about one
  failed start, then launches fresh and forgets the session.
- Every pin and mux launch takes up to 0.5 s longer, or 1.5 s for a
  resume, before it attaches or returns.
- A harness that fails after the window stays visible as a dead pane:
  attach shows its output, the TUI preview captures it, and the next
  launch replaces it.
- A harness that exits non-zero *on purpose* (for example a wrapper
  that ends with a failing command) leaves a dead session the operator
  has to close or relaunch. That's the price of seeing failures.

## Alternatives Considered

**Check transcripts before resuming.** A harness-specific "has
conversation turns" check would have caught this case without a
failed start. It only covers failures Conspectus can predict, and it
belongs in each adapter. The failed-start check covers every harness
and every reason. A turn check can still be added as an optimisation
later.

**Poll for liveness without keeping the pane.** This detects the
failure, but the harness's error text is gone by the time Conspectus
looks, which is what made the original bug opaque.

**`remain-on-exit on`.** This keeps clean exits too, so quitting an
agent normally would leave a dead session behind.

**Wrap the launch argv in a shell that records output.** That would
change the argv the operator configured (ADR 0098), and process-tree
evidence (ADR 0046) would see a shell instead of the harness.

**A longer fixed window.** Any window misses a failure that comes
later. With retained panes a later failure still shows on attach, so
the window only needs to cover fast, predictable failures.
