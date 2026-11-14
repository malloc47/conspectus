---
id: CSP-860
title: See a turn that ended while you were attached to its mux
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 01:10'
updated_date: '2026-11-14 14:17'
labels:
  - live-state
  - ls-tui
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
- [x] #1 Inside tmux, a ready turn of an agent whose mux is the session the operator's client came back from, and that ended at or before the moment the client left it, reads seen as soon as the client is back on the TUI's session, without another attach or a timer refresh
- [x] #2 A turn that ended after the client left that session, or on a session the client was not last on (mux, other session, TUI), still reads unseen; a window change inside the TUI's own session credits nothing
- [x] #3 A turn seen this way stays seen across refreshes (it joins the seen-turns memory); outside tmux the behavior is unchanged
- [x] #4 Nothing is persisted, no effect is emitted, and no tmux subcommand is added: display-message is already sanctioned, so docs/capabilities.toml gains only the new ADR in the tmux adr list
- [x] #5 An ADR amending ADR 0214 decision 1 records the rule and its known limit (two clients on the TUI's own session); docs and CHANGELOG say what the operator sees
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. visibility.rs: the display-message format gains #{client_last_session}; a pure state machine turns observations into signals (Regained, as today, and Returned{socket, session, at_ms} when the TUI's session goes from no client to a client, read at that same poll so the value is fresh, with at_ms the last poll that saw the client away). A window change inside the TUI's own session fires nothing, since client_last_session is stale there.
2. rows/seen.rs: ClientReturn, kept in SeenTurns; unseen() reads a mux seen when a return from its session (matched by server and session name through on_server) is at or after the turn's end. Joins the seen-turns memory through the existing remember_seen_turns pass.
3. Msg::ClientReturned, applied in the reducer, refills the live rows in place; the runtime drains the watcher's signals.
4. Tests first: state machine, parse, judging (sparse/ambiguous cases), reducer, in-place refill.
5. New ADR amending ADR 0214 decision 1; manifest adr list; CHANGELOG, docs/operations.md.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Needs the operator's decision; stopped before implementing (2026-11-13). The fix needs a fact the TUI does not have: which tmux session the operator's client was on just before it came back to the TUI. The TUI knows each mux's `#{session_last_attached}` (moves on attach or switch, not while a client stays), whether a refresh listed a client attached, and, inside tmux, the moment its own pane turns visible again (the visibility watcher's `display-message -p -t <pane>` poll, which already triggers the regain refresh). At the regain the client is already back on the TUI's session, so no refresh can still list it on the mux it watched.

Options:
A. Inside tmux: add `#{client_last_session}` to the visibility watcher's existing `display-message` format and keep (that session, the regain time) in memory at each hidden-to-visible edge. A ready turn of an agent whose mux is that session and that ended at or before the regain counts as seen, joining the seen-turns memory like any other seen turn: nothing persisted, no effect, the anti-flicker property unchanged. `display-message ...` is already allowed for tmux in docs/capabilities.toml, so the manifest gains only the new ADR in the tmux `adr` list; the change needs an ADR amending ADR 0214 decision 1. Caveats: tmux expands a client format there against a client of the TUI's session it picks (the most recently active), so with two clients on that session it may read the other one; a client that went mux -> another session -> TUI reads the other session and the turn stays unseen (the safe side, criterion 2 holds).
B. Outside tmux (the hand-off runs `attach-session` in the foreground and returns on detach): the client is gone when the TUI resumes, so A cannot read it; covering it needs a `list-clients` poll during the attach (a new tmux subcommand in the manifest) or stays out of scope.
C. No new query, inferred from the regain refresh: treat the client as on the mux from its last attach until the earliest later `session_last_attached` of any other session. Unsound: a client that went mux -> X -> Z -> X -> TUI leaves X's time later than when it left, so a turn that ended on the mux after the client moved would read seen (breaks criterion 2).
D. Keep the gap as ADR 0214's consequences describe it (a timer refresh while attached sees the turn).

Recommendation if it should close: A inside tmux, with D's behavior outside tmux.

Operator decision 2026-11-13: option A (inside tmux, read at the client's return), D outside tmux. Refinement found while implementing: the 2026-11-13 probe on a private tmux server showed `#{client_last_session}` stays at the session the client came from after a window change inside the TUI's own session, so reading it at the hidden-to-visible edge would credit turns that ended after the client left. The watcher therefore reads it at the poll that finds a client on the TUI's session after one that found none (the same poll as the pane turning visible in the usual case) and dates the return by the last poll that found the client away. Recorded in ADR 0231. Verified: unit tests for the watcher state machine, the judging, the reducer; `just check` green (5730 passed).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Inside tmux, a ready turn that ended while the operator's client watched its mux now reads seen the moment the client is back on the TUI's session, with no attach and no timer refresh (ADR 0231, amending ADR 0214 decision 1). The visibility watcher's existing `display-message` format gains `#{client_last_session}`; a pure state machine (`Watch::step`) emits a `Returned` signal when the TUI's session goes from no client to a client, naming the session the client came from and dating it by the last poll that found the client away (so nothing ended after the client left is credited, and a window change inside the TUI's session credits nothing, since the value is stale there). `Msg::ClientReturned` stores the latest return per (server socket, session name) in `SeenTurns`; `unseen()` reads a mux's turn seen when a return from its session on its server (`on_server`) is at or after the turn's end, and the existing `remember_seen_turns` pass keeps it seen. Nothing persisted, no effect, no new tmux subcommand (docs/capabilities.toml gains only 0231 in tmux's adr list); outside tmux the behavior is unchanged. Known limit recorded in the ADR: two clients on the TUI's own session. Verified with unit tests (watcher, judging incl. server/name matching and the pane-agent and no-mux cases, reducer, in-place refill, memory across refreshes) and `just check` (5730 passed). No static frame changes: the only visible effect is a bold `›` turning plain on a return.
<!-- SECTION:FINAL_SUMMARY:END -->
