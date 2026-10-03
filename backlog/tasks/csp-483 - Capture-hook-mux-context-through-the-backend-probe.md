---
id: CSP-483
title: Capture hook mux context through the backend probe
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-480
ordinal: 165000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03 (partial). New
  `discovery::tmux::MuxSessionContext { backend, session_name,
  pane_id, namespace }` struct + trait method
  `MuxBackend::current_session_context() ->
  Option<MuxSessionContext>` with `None` default. `SystemTmux`
  overrides: the pre-H-EXT-011 `$TMUX` + `tmux
  display-message` probe (previously inlined in
  `cli::tmux_context`) moves onto the impl. `SystemZellij`
  inherits the None default for now — a `$ZELLIJ` env-var
  contract is a follow-up (zellij's env probe is
  less-well-documented than tmux's).
  `cli::tmux_context` and `cli::current_tmux_session_name`
  both refactored to consult the trait method. The former
  iterates a compile-time list of `Box<dyn MuxBackend>`
  (tmux + zellij) and takes the first `Some`; the latter
  delegates to `SystemTmux::current_session_context` because
  the TUI runtime consumes an `Option<String>` shape
  directly for the self-attach guard.
  Deferred: (a) `HookTmuxRecord` rename to `HookMuxRecord`
  with a `backend` field on the record, (b) sidecar schema
  version bump, (c) zellij's own `current_session_context`
  impl once its env-var contract is settled. All are
  ADR 0028 schema follow-ups that don't need to land
  atomically with the trait-shape change.
  All 25 suites (1521 lib tests) pass byte-identically;
  fmt / clippy clean.
- Blockers: `CSP-480` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-011`
