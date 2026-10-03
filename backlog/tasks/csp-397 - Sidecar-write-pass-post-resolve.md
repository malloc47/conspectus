---
id: CSP-397
title: Sidecar write pass post-resolve
status: Done
assignee: []
created_date: '2026-06-05 22:52'
labels:
  - h-pin-resume
milestone: m-11
dependencies:
  - CSP-395
ordinal: 338000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: after the resolver completes a discovery cycle, for
  each pin resolution where the binding is `Bound` (including
  bindings sourced from a `LocalDeclared` `linked_to_mux`
  override written by `pin bind`, per ADR 0058 Q6), update the
  sidecar via the CSP-395 helpers. Skip writes where
  the payload is unchanged. Never write on `PinUnbound`,
  `PinStaleMux`, or `PinAmbiguous` outcomes. Wire into the
  main `discover_and_resolve` pipeline behind a config gate so
  tests / scenario TUIs can opt out cleanly.
- Tests: integration tests covering the bound case (sidecar
  written), the `pin bind` override case (sidecar still
  written), the unbound case (no write), unchanged-payload
  skipping, and read-only invariant non-write on the `tui`,
  `graph`, `table`, `query`, `pin list`, and `pin show`
  commands (verify via mtime fingerprinting like
  `CSP-379`).
- Blockers: `CSP-395`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`pin_bindings::record_bindings(snapshot, cache,
epoch)` iterates `Bound` resolutions and writes via the
-001 helpers. Wired into `cli::discover_and_resolve` as
`record_pin_bindings_best_effort` — silent skip when no
cache root is available; write failures log to stderr and
never propagate. 7 unit tests.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-RESUME-003`
