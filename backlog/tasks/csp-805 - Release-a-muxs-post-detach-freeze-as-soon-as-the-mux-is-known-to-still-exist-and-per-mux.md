---
id: CSP-805
title: >-
  Release a mux's post-detach freeze as soon as the mux is known to still exist,
  and per mux
status: Done
assignee:
  - '@claude'
created_date: '2026-11-11 23:28'
updated_date: '2026-11-12 00:36'
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
- [x] #1 A mux that exited on detach stays frozen until a refresh shows it gone, including when the server still lists it for a moment after the exit
- [x] #2 Detaching from mux A and attaching mux B never leaves A frozen while it waits for B: with a blocking attach, A is released on the first drain after the return from B; inside tmux, A is released when its own refresh lands
- [x] #3 An ADR amends ADR 0108 with the per-mux settle and the existence check; a new tmux call has a `docs/capabilities.toml` entry; `docs/operations.md` describes when the spinner shows and clears
- [x] #4 After a detach from a mux that still exists, the mux's row and the rows of its sessions stop spinning and accept actions without waiting for a full discovery; a test with a fake existence probe shows the release before any refresh result
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reducer: replace the single PendingHandoff set with per-mux frozen entries (each mux with its linked sessions); on HandoffReturned keep the mux's incarnation (backend, socket, backend_session_id, created_epoch) from the pre-attach snapshot and classify it (check the incarnation / settle by its listing when there is none or a launch ran in it / nothing to wait out for a mux the screen did not show). New Msg::HandoffReleased(mux); HandoffSettled still ends the hand-off (follow waits for it).
2. Runtime: WorkerLedger records per frozen mux the generation of the refresh started after its latest return; finish() releases every mux at or below the result's generation; confirm(mux, generation) releases a mux the existence check confirmed unless a later return re-froze it. A later hand-off or a pane return never moves an earlier mux's point.
3. Existence check (src/tui/runtime/handoff.rs): IncarnationProbe trait; TmuxIncarnation runs `tmux [-L socket] display-message -p -t <session id> '#{session_id} #{session_created}'` through exec::command and compares both fields; one re-check after 150 ms when the first answer does not confirm. It runs on a thread beside the refresh's discovery and reports confirmations through a channel the drain turns into HandoffReleased.
4. Re-probe only what the check could not confirm: probe again while the listing still shows an unconfirmed (or uncheckable) returned mux; keep the old unchanged-set rule for the pane coming back into view.
5. Make the loop's discovery injectable (Discover) so LiveMode is tested end to end with a fake probe and gated discovery; tests: ledger, reducer, check/parse, settle, a private-tmux-server probe test, and live-loop tests (still there, gone while listed, confirmed by the re-check, A then B blocking, A then B inside tmux).
6. ADR 0201 amending 0108 and 0132 (PID alternative weighed), capabilities.toml citation, design.md, operations.md (when the spinner shows and clears), CHANGELOG Fixed entry, operator-review entries.
7. fmt, clippy (workspace all-features and tuikit features off), isolated full suite, rustdoc -D warnings, cargo build --examples, just tuikit-features, git diff --check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Measured tmux 3.6a against a private server with a pty-attached client (scripts under /tmp, TMUX unset, private TMUX_TMPDIR): right after the client returns, `display-message -p -t <id> '#{session_id} #{session_created}'` answers the same id and creation time after a detach, empty fields with exit 0 after an exit or kill-session, and `no server running` after kill-server; an empty target resolves to another session. Hence the field-by-field comparison and no reliance on the exit status (ADR 0201 Context).

Landed in two code-bearing commits: the ADR (63201b68) and the fix with its tests (ddcd4f0c). Tests written against the new API failed against the old single-settle behavior (the ledger made to release only at the newest hand-off and the confirmation ignored: 4 live-loop tests failed), then passed with the fix.

Checks: cargo fmt --check, clippy workspace all-features -D warnings, clippy -p tuikit features off -D warnings, rustdoc -D warnings, cargo build --examples, just tuikit-features, git diff --check, the isolated full suite (4889 passed, 0 failed, 1 ignored; baseline 4864), doc/hygiene tests rerun after the doc edits.

Not possible unattended: the live inside/outside tmux manual check, and `conspectus tui --snapshot`, which renders one frame and has no hand-off. The live-loop tests with a scripted probe and a gated discovery, the private-server probe test, and the existing tests/tui_tmux_e2e.rs flows (outside/inside exit, detach leaves the mux running, launch and detach) stand in.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Each mux a tmux hand-off froze is now released on its own, and at once when tmux still answers for the same session (ADR 0201, amending ADRs 0108 and 0132).

What landed:
- `WorkerLedger` records, per frozen mux, the generation of the refresh started after that mux's latest return; any applied result at or after it releases the mux and its linked sessions' rows (`Msg::HandoffReleased`). The newest hand-off refresh still ends the hand-off (`Msg::HandoffSettled`), which an entity to follow waits for.
- On `HandoffReturned` the reducer keeps the mux's incarnation (backend, socket, `backend_session_id`, `created_epoch`) from the pre-attach snapshot. The refresh's worker runs `tmux [-L socket] display-message -p -t <session id> '#{session_id} #{session_created}'` through `exec::command` on a thread beside its discovery and compares both fields; a match releases at once, otherwise one re-check after 150 ms. A launch return or a mux without a recorded incarnation waits for the refresh as before.
- The re-probe (twice, 150 ms apart) now runs only while the listing still shows a returned mux the check didn't confirm; the refresh for the pane coming back into view keeps the unchanged-set rule.
- `docs/capabilities.toml` (tmux cites 0201; `display-message` was already a read subcommand), `docs/design.md`, `docs/operations.md` ("Rows after a detach"), `docs/tui-columns.md`, CHANGELOG Fixed entry, six operator-review rows (the PID alternative flagged for review).

Verification: reducer tests (`src/tui/app/handoff_tests.rs`), ledger tests, check/parse/settle tests and a private-tmux-server probe test (`src/tui/runtime/handoff_tests.rs`), and live-loop tests with a scripted probe and gated discovery: release before any refresh result (AC 4), a mux gone while still listed stays frozen until a refresh without it (AC 1), confirmed only by the re-check, A then B with a blocking attach released on the first drain (AC 2), A then B inside tmux each released by its own refresh (AC 2). Four of them failed against the old single-settle behavior. fmt, clippy (workspace all-features; tuikit features off), rustdoc -D warnings, `cargo build --examples`, `just tuikit-features`, `git diff --check`, the isolated suite (4889 passed, 0 failed, 1 ignored) and nextest (4889 passed, 1 skipped). The live inside/outside tmux manual check could not be run unattended, and `tui --snapshot` cannot show a hand-off; the tests above and `tests/tui_tmux_e2e.rs` stand in.

**Regressions**
- A mux whose session survives with a dead pane (`remain-on-exit`), or in which the operator ended one agent session and started another, is released before the refresh shows its new state: its rows are undimmed and accept actions while still showing pre-hand-off values.
- Each return costs one more `tmux display-message` (two when the mux is gone).

**Consistency wins**
- A detach that leaves the mux running releases its row and its sessions' rows almost at once instead of after three discoveries and 300 ms.
- Attaching another mux right after one never keeps the first spinning: outside tmux it is released on the first drain after the return; inside tmux by its own refresh, and returning to the TUI's pane no longer extends any freeze.
- A mux that exited stays frozen while the listing still shows it, even when another mux changed meanwhile.
<!-- SECTION:FINAL_SUMMARY:END -->
