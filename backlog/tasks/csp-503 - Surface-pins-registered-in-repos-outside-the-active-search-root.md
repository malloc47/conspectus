---
id: CSP-503
title: Surface pins registered in repos outside the active search root
status: Done
assignee: []
created_date: '2026-07-28 03:30'
labels:
  - h-pin-root
milestone: m-18
dependencies: []
ordinal: 528000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `local_pin_store_paths` (`src/discovery/pins.rs:80`) only
  locates `.conspectus.toml` pin stores by walking up from
  `context.roots()` and from cwds of already-discovered nodes. A pin
  created in a repo that is neither a scan root nor referenced by any
  discovered session/mux node is never re-read, so it disappears on
  the next discovery cycle. Decide and implement how such pins stay
  visible — candidate approaches: (a) remember pin store paths seen in
  a prior snapshot / persisted sidecar, (b) treat a pin's own `cwd` as
  an additional project-config search root once known, (c) an explicit
  user-config registry of pin store locations. Preserve ADR 0087
  read-only / write-envelope guarantees.
- Tests: discovery test where a pin store lives outside every scan
  root and outside all node cwds and still loads; regression that
  in-root pins are unaffected.
- Blockers: needs a direction decision among (a)/(b)/(c); likely a
  short ADR.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Chose approach (a), the state-sidecar registry. Recorded
as ADR 0090. `src/pin_store_registry.rs` maintains
`$XDG_STATE_HOME/conspectus/pin-stores.json`; CLI `pin create` /
`pin adopt` and the TUI pin-create action record the project store
on write, and each discovery cycle folds the recorded (still-extant)
stores into the pin loader's search set. Best-effort, self-pruning,
read-only for `graph`/`table`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-ROOT-001`
