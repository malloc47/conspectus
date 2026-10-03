---
id: CSP-399
title: '`PinUnbound` diagnostic extension + UX surfaces'
status: Done
assignee: []
created_date: '2026-06-05 22:52'
labels:
  - h-pin-resume
milestone: m-11
dependencies:
  - CSP-397
  - CSP-398
ordinal: 340000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend the `PinUnbound` resolver diagnostic with an
  optional `last_session: Option<{session_id,
  observed_epoch}>` field per ADR 0058 Q5. Populate from the
  sidecar at resolve time when the pin is unbound. Wire the
  TUI status hint to read `Enter to resume <session_id>` when
  populated and fall through to `Enter to launch <name>`
  otherwise. Extend `pin show <id>` to surface a `last
  session   <session_id> (observed <iso8601>)` line. Right
  detail pane shows the same alongside the unbound state.
- Tests: resolver tests covering the unbound-with-sidecar
  and unbound-without-sidecar cases; reducer + snapshot tests
  for the TUI status hint and detail pane; CLI snapshot tests
  for `pin show` output.
- Blockers: `CSP-397`, `CSP-398`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`Diagnostic::PinUnbound` gains an optional
`last_session: Option<PinLastSession>` field. Resolver
leaves it `None` (evidence-only). New
`pin_bindings::decorate_unbound_diagnostics` post-resolve
pass reads the sidecar and patches in `last_session`; wired
into `cli::discover_and_resolve` as
`decorate_unbound_pins_best_effort`. UX surfaces branch on
the field: TUI status hint reads `Enter resume <id>`, right
detail pane appends `Last session: <id> (observed <epoch>)`
with an `Enter resume` annotation, and `pin show` adds a
`last_session <id> (observed <iso8601>)` line. ISO 8601
formatting uses an in-tree Hinnant date formatter (no
chrono/humantime dependency). 8 tests across pin_bindings
(4), cli (2), and tui::actions (2).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-RESUME-005`
