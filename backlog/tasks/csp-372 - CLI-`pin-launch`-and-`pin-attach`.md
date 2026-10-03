---
id: CSP-372
title: CLI `pin launch` and `pin attach`
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-364
  - CSP-369
  - CSP-370
  - CSP-371
ordinal: 305000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: orchestrate the launch flow per ADR 0057 §Launch
  Semantics: load pin → run discovery + resolver → branch on
  binding state. Bound → exec-replace `attach_session`. Stale-mux
  → `send_keys(argv...) ; Enter` then `attach_session`. Unbound +
  name-free → `new_session(socket_name, name, cwd, argv)` then
  `attach_session`. Unbound + name-taken-by-unrelated-tmux →
  `PinLaunchError::NameTaken` with hint about `pin adopt`.
  Unix-only exec-replace; `--no-attach` returns after spawn and
  prints the attach command.
- Tests: integration tests via `FakeTmux` for each branch + the
  bound/stale/unbound transitions; one Unix-gated test for the
  exec path.
- Blockers: `CSP-364`, `CSP-369`, `CSP-370`, `CSP-371`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`pin launch` / `pin attach` branch across bound,
stale-mux, and unbound states using socket-aware tmux attach,
send-keys, and new-session operations, with `--no-attach`
coverage.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-012`
