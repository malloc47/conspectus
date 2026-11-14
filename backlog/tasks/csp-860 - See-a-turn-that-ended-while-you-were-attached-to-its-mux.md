---
id: CSP-860
title: See a turn that ended while you were attached to its mux
status: To Do
assignee: []
created_date: '2026-11-13 01:10'
updated_date: '2026-11-14 01:17'
labels:
  - live-state
  - ls-tui
  - needs-decision
milestone: m-26
dependencies: []
priority: low
ordinal: 945000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0214 (CSP-828) reads a `ready` turn of an agent in a mux as seen once the mux's last attach time is at or after the turn's end, or a snapshot loaded after the turn ended lists a client attached. tmux moves `#{session_last_attached}` only when a client attaches or switches to the session, so a turn that ends while the operator watches it in an attached client is seen only if a refresh happens to list that client while it is still there (a timer refresh, every 30 seconds by default). When the operator returns to the TUI's pane sooner, the turn they just watched reads unseen (bold) until their next attach. Closing the gap needs to know which session the client was on when it came back (for example tmux's `#{client_last_session}` at the regain the hand-off already detects), which is a new tmux query and so a capability-manifest entry and the operator's call.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A turn that ends while a tmux client stays attached to its mux reads seen once the operator returns to the TUI from that mux, without waiting for another attach
- [ ] #2 A turn that ends after the client moved to another session still reads unseen
- [ ] #3 Any new tmux query is listed in docs/capabilities.toml with its ADR, and nothing is persisted
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Needs the operator's decision; stopped before implementing (2026-11-13). The fix needs a fact the TUI does not have: which tmux session the operator's client was on just before it came back to the TUI. The TUI knows each mux's `#{session_last_attached}` (moves on attach or switch, not while a client stays), whether a refresh listed a client attached, and, inside tmux, the moment its own pane turns visible again (the visibility watcher's `display-message -p -t <pane>` poll, which already triggers the regain refresh). At the regain the client is already back on the TUI's session, so no refresh can still list it on the mux it watched.

Options:
A. Inside tmux: add `#{client_last_session}` to the visibility watcher's existing `display-message` format and keep (that session, the regain time) in memory at each hidden-to-visible edge. A ready turn of an agent whose mux is that session and that ended at or before the regain counts as seen, joining the seen-turns memory like any other seen turn: nothing persisted, no effect, the anti-flicker property unchanged. `display-message ...` is already allowed for tmux in docs/capabilities.toml, so the manifest gains only the new ADR in the tmux `adr` list; the change needs an ADR amending ADR 0214 decision 1. Caveats: tmux expands a client format there against a client of the TUI's session it picks (the most recently active), so with two clients on that session it may read the other one; a client that went mux -> another session -> TUI reads the other session and the turn stays unseen (the safe side, criterion 2 holds).
B. Outside tmux (the hand-off runs `attach-session` in the foreground and returns on detach): the client is gone when the TUI resumes, so A cannot read it; covering it needs a `list-clients` poll during the attach (a new tmux subcommand in the manifest) or stays out of scope.
C. No new query, inferred from the regain refresh: treat the client as on the mux from its last attach until the earliest later `session_last_attached` of any other session. Unsound: a client that went mux -> X -> Z -> X -> TUI leaves X's time later than when it left, so a turn that ended on the mux after the client moved would read seen (breaks criterion 2).
D. Keep the gap as ADR 0214's consequences describe it (a timer refresh while attached sees the turn).

Recommendation if it should close: A inside tmux, with D's behavior outside tmux.
<!-- SECTION:NOTES:END -->
