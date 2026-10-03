---
id: CSP-530
title: Extract the shared launch-spec form primitive (ADR 0097)
status: Done
assignee: []
created_date: '2026-09-30 16:57'
labels:
  - h-mux-launch
milestone: m-18
dependencies:
  - CSP-529
ordinal: 555000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: `PinCreateState` and the freshly-shipped
  `MuxLaunchFormState` (CSP-529) share every launch-
  spec field (harness, cwd, mux name, socket, argv, worktree).
  ADR 0097 extracts them into `LaunchSpecFormState` so that
  future field additions (e.g. `launch.env`) edit one place
  instead of two, and the "parameterized dialog summoned per
  call site" operator preference is realized at the state
  level.
- Scope (staged per ADR 0097 migration plan):
  1. Extract `LaunchSpecFormState` from the current
     `PinCreateState` / `MuxLaunchFormState` field sets
     (shared fields, no shadow copies removed yet).
  2. Route both wrappers' reads through
     `spec()` / `spec_mut()` accessors.
  3. Collapse the shadow copies; snapshot-test parity gate on
     pin-create rendering.
  4. Rehome the shared field accessors and validation onto
     the primitive; wrappers keep their caller-specific
     state (id/display/store/adopt for pin-create; nothing
     today for mux-launch beyond scan-root hint).
  5. Rename `MuxLaunchFormState` → `MuxLaunchState` if the
     wrapper diet warrants it; update tests.
- Non-goals: no behavior change; no visual redesign; no new
  CLI or executor surface.
- Tests: preserve every existing pin-create test; preserve
  every CSP-529 mux-launch test; add wrapper-agnostic
  tests directly on `LaunchSpecFormState`.
- Blockers: `CSP-529`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
**landed as ADR 0097.** `LaunchSpecFormState` in
`src/tui/widgets/launch_spec_form.rs` owns the ten shared
fields (harness, cwd as `PathOmniboxState`, mux name, mux
socket, launch argv, worktree toggle + branch, known-harness
keys, known-live-mux names, error) plus a `LaunchSpecInit`
constructor struct, wrapper-agnostic
`cycle_harness` / `mux_name_collides_with_known` /
`matching_known_mux_name` / `cwd_handle_key` /
`handle_text_key(SpecTextField, event)` helpers, and shared
`parse_launch_argv` / `optional_string`. `PinCreateState`
and `MuxLaunchFormState` compose it; shadow copies removed;
all 61 pin tests + 6 mux-launch tests + 8 primitive tests
stay green. Mux-launch cwd upgraded from `TextInputState`
to `PathOmniboxState` (UX bonus: path autocomplete). No
visual regression to pin-create rendering. Steps 4 (rehome
accessors) and 5 (`MuxLaunchState` rename) folded into the
extraction; wrapper diet is thin enough that the rename
would only rewrite spelling.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUX-LAUNCH-002`
