---
id: CSP-370
title: Extend `TmuxRunner` with launch mutation seams
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-234
  - CSP-361
ordinal: 303000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add three new defaulted `TmuxRunner` methods —
  `new_session(socket_name, name, cwd, argv)`,
  `attach_session(socket_name, name)`,
  `send_keys(socket_name, target, literal, press_enter)` — and
  thread `socket_name: Option<&str>` through the existing
  `rename_session` and `capture_pane` methods. `SystemTmux` adds
  `-L <name>` only when `socket_name` is `Some(s)` and
  `s != "default"`, preserving default-socket invocation
  byte-for-byte. `FakeTmux` records calls.
- Tests: unit tests for the new methods, the socket-name
  threading on both `SystemTmux` (mocked) and `FakeTmux`,
  and `Unsupported` defaulting.
- Blockers: `CSP-234` (the `rename_session` seam this extends),
  `CSP-361`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`TmuxRunner` supports socket-aware new-session,
attach-session, send-keys, rename, and capture operations with
`FakeTmux` call recording for tests.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-010`
