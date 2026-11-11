---
id: CSP-805
title: >-
  Release a mux's post-detach freeze as soon as the mux is known to still exist,
  and per mux
status: To Do
assignee: []
created_date: '2026-11-11 23:28'
updated_date: '2026-11-11 23:29'
labels:
  - tui
dependencies: []
references:
  - docs/adr/0108-handoff-refreshes-run-in-the-background.md
  - docs/adr/0132-refuse-actions-on-rows-awaiting-a-handoff-refresh.md
priority: high
type: bug
ordinal: 891000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Why: after a detach, the mux's row and its sessions' rows are dimmed with a spinner and refuse actions until a refresh lands (ADRs 0108 and 0132), in case the mux exited. The operator hit two problems with it.

1. It is slow when the mux still exists, which is the usual case. The settling refresh (`spawn_discovery_worker_settling` in `src/tui/runtime.rs`) probes again while the live mux set equals the one from before the hand-off (`probe_until_changed`, `SETTLE_TRIES` = 2, `SETTLE_DELAY` = 150 ms). A detach that leaves the mux running is exactly that unchanged case. So the common path always pays three full discoveries, each nudging the daemon's `mux` and `harness` classes (or running a local discovery when there is no daemon), plus 300 ms, before the row is released. The retries exist for the opposite case: a mux that just exited and still answers a listing for a moment.

2. A freeze waits on later hand-offs. `PendingHandoff` is one set, and `WorkerLedger::settles_handoff_at` keeps only the newest hand-off's generation. So a frozen mux is released only by the refresh of the latest hand-off (ADR 0108 decisions 2 and 4). Outside tmux, `run_tmux_attach_with_socket` blocks the UI thread in `tmux attach-session` for as long as the operator stays in the second mux. The first mux's settling result waits undrained in the channel. On return, `HandoffReturned(second)` and `start_requested_refreshes` move the settle generation past that result before the drain reads it. The first mux then keeps spinning until the second mux's refresh, itself three discoveries, lands, however long the operator was away. Inside tmux, `switch-client` returns at once, and every `ViewRegained` and every new attach also moves the settle point forward, which extends earlier freezes.

Scope (proposed; settle it in an ADR that amends ADR 0108):
- Settle per mux: record the generation each frozen mux waits for, and let any result at or after it release that mux and its sessions. The first mux's waiting result then releases it on the first drain after the return.
- A fast existence check: keep the mux's identity from before the attach (the tmux `session_id` and created epoch, which `MuxSessionNode::backend_session_id` and `created_epoch` already hold, or the active pane's PID, as the operator suggested). Right after the return, run one cheap check, with a short re-check (about 150 ms) for the exit race. If the same incarnation is still there, release the freeze at once and leave the full refresh to update everything else in the background. If it is not there, keep the freeze until the refresh lands, as today. The ADR weighs a PID check (no subprocess, but a pane can die while its session lives, and PIDs are reused) against a tmux query (`display-message -p -t <session id>` through `exec::command`: exact, one subprocess, and it needs a `docs/capabilities.toml` entry under ADR 0117).
- Keep the re-probe only for a mux the fast check finds gone.

TUI changes:
- Consistency wins: a detach that leaves the mux running releases its rows almost at once. Toggling between muxes is never blocked by an earlier mux's freeze.
- Regressions or behavior changes: if the check only asks whether the session exists, a mux whose session survives with a dead pane (`remain-on-exit`) is released before the refresh shows its new state.

Tests and checks: reducer and `WorkerLedger` tests with a fake existence probe (mux still there, mux gone, gone only after the re-check, A then B with a blocking attach, A then B inside tmux); a manual check inside and outside tmux.

Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A mux that exited on detach stays frozen until a refresh shows it gone, including when the server still lists it for a moment after the exit
- [ ] #2 Detaching from mux A and attaching mux B never leaves A frozen while it waits for B: with a blocking attach, A is released on the first drain after the return from B; inside tmux, A is released when its own refresh lands
- [ ] #3 An ADR amends ADR 0108 with the per-mux settle and the existence check; a new tmux call has a `docs/capabilities.toml` entry; `docs/operations.md` describes when the spinner shows and clears
- [ ] #4 After a detach from a mux that still exists, the mux's row and the rows of its sessions stop spinning and accept actions without waiting for a full discovery; a test with a fake existence probe shows the release before any refresh result
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
