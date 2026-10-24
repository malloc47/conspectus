# ADR 0132: Refuse Actions On Rows Awaiting A Hand-Off Refresh

## Status

Accepted. Amends ADR 0108 (item 2, which left the rest of the UI live,
including attaching again). Amended by ADR 0142: where this ADR says
"pin launch", read `rebuild` (and `mux launch`).

## Context

ADR 0108 marks the hand-off's mux row and its linked sessions as stale
(dimmed, with a spinner) until a background refresh lands. It left them
actionable: "the operator can move, filter, and attach again while the
refresh runs". That assumed the shown values were close enough. When the
operator detaches and the session was killed, or the mux changed, an
action on the stale row runs against something that is gone: `Enter`
attaches a mux that no longer exists and fails.

## Decision

1. **A row awaiting the refresh refuses actions on itself.** The marked
   rows are the hand-off's mux, the agent sessions linked to it, and its
   mux-candidate child rows. While the selected row is one of them:
   - the reducer refuses attach, resume, and pin launch with a toast;
   - the runtime refuses rename, the worktree menu and close-down, pin
     remove, rebind, and adopt with the same text in the status bar.
   The text reads `refreshing after tmux · wait for the row to update`.
2. **Read-only actions stay live.** Moving, folding (so `Enter` on an
   expandable row still folds it), filtering, search, the transcript
   viewer, copy, previews, and refreshing are untouched, as are all
   other rows.
3. **The block ends with the refresh.** It lifts when the refresh
   settles the mark, which a failed refresh also does (ADR 0108), so a
   failure can't leave a row locked.

## Consequences

- An attach right after a detach is refused for a fraction of a second
  (seconds without a daemon) instead of failing.
- An operator who wants to re-attach immediately must wait for the
  spinner to clear. The pre-hand-off values were not trustworthy for
  that anyway.

## Alternatives Considered

- **Let the action run and report its failure.** What happened before;
  the failure arrives after the screen has been torn down for the
  attach.
- **Queue the action and run it when the refresh lands.** Surprising if
  the row changed or vanished, and a keypress would act seconds later.
- **Block every action while any hand-off is pending.** Needlessly locks
  rows the hand-off did not touch.
