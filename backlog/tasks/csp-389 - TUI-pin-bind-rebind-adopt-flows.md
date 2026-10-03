---
id: CSP-389
title: TUI pin bind / rebind / adopt flows
status: Done
assignee: []
created_date: '2026-06-05 12:25'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-373
  - CSP-374
  - CSP-375
  - CSP-378
  - CSP-253
ordinal: 314000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: expose the CLI escape hatches from the Controls overlay
  and contextual accelerators so `PinAmbiguous`, external tmux
  renames, and agent-deck migration are solvable in the TUI.
  Bind presents competing agent-session ids from the selected
  `PinAmbiguous` diagnostic, with a manual id entry fallback, then
  calls the CSP-373 helper. Rebind edits the pin's mux target via
  CSP-374 and shows live mux-name candidates when available.
  Adopt starts from a selected live mux or an entered mux name,
  infers harness/cwd when the resolver can, and calls CSP-375.
  Each flow must surface the exact config store that will be
  mutated and leave read-only navigation paths untouched.
- Tests: reducer tests for bind-from-ambiguous, manual bind, rebind
  duplicate rejection, adopt with inferred fields, adopt refusal
  when cwd cannot be determined, and cancel-no-write. Snapshot tests
  for each picker / confirmation state.
- Blockers: `CSP-373`, `CSP-374`, `CSP-375`, `CSP-378`,
  `CSP-253`.
- Delivered: Controls overlay `Pins > bind` opens a picker from
  the selected row's `PinAmbiguous` diagnostic and writes the same
  `pin:<id>:bound` declared override as the CLI. `Pins > rebind`
  routes through the edit modal's mux-name/socket fields with the
  duplicate-mux preflight from `CSP-388`. The initial
  implementation exposed `Pins > adopt`; `CSP-453` later
  folded this into one create form with an adopt toggle while
  preserving the same selection-derived defaults and validated
  create/write path.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PIN-024`
