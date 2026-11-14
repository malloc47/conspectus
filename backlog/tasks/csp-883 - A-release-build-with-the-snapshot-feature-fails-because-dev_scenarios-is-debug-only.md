---
id: CSP-883
title: >-
  A release build with the snapshot feature fails because dev_scenarios is
  debug-only
status: To Do
assignee: []
created_date: '2026-11-14 22:31'
labels:
  - tui
dependencies: []
priority: low
type: bug
ordinal: 960000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the scroll-stall work (CSP-874): `cargo build --release --features snapshot` fails on main because the `dev_scenarios` module is compiled only in debug builds, while the snapshot feature (the way CLAUDE.md says to render frames with `conspectus tui --snapshot`) refers to it. Timing measurements need a release build, so that work had to build the release binary without the feature. Reproduce with `scripts/dev-env.sh cargo build --release --features snapshot`. Decide whether the snapshot feature should work in release builds (compile `dev_scenarios` whenever the feature is on) or whether the docs should say it is debug-only. Not verified by the coordinator; reported by the implementing agent.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A release build with the snapshot feature either compiles and renders a frame, or the project instructions and README say the feature is debug-only and why
- [ ] #2 A test or CI step covers whichever behavior is chosen
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
