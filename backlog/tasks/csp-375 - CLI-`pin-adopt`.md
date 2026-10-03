---
id: CSP-375
title: CLI `pin adopt`
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-364
  - CSP-369
ordinal: 308000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: convert an existing live tmux session into a pin without
  creating a new mux. Required positional `<pin-id>` and
  `<mux-name>`; optional `--harness` (default: infer from the
  resolver's current attribution for that mux); `--display`
  (default: pin-id); `--mux-socket` (default: absent). `cwd`
  defaults to the mux's observed `cwd` if known, otherwise refused
  with a hint. The v1 migration path for replacing agent-deck.
- Tests: integration tests for adopt with explicit harness, adopt
  with inferred harness, adopt with unresolvable mux (rejected),
  adopt of an already-adopted mux (rejected).
- Blockers: `CSP-364`, `CSP-369`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`pin adopt` converts a live mux into a pin using
inferred or explicit harness/cwd fields, and rejects missing or
already-pinned mux targets.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-015`
