---
id: CSP-243
title: Live-session UX advisory
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-241
ordinal: 292000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: status-bar advisory when the operator renames a session whose
  mux indicator is `Attached` or `Ambiguous` (per `MuxIndicator` in
  `src/tui/rows/mod.rs:159-172`) and hook-sidecar evidence is fresh
  (per ADR 0028 `ACTIVE_TTL_SECONDS`, `src/discovery/hook_sidecar.rs:21-28`).
  Alias is safe; message is informational ("renamed live session:
  alias overlays harness title until session ends"). Establishes the
  live-detection plumbing the future write-back ADR will need.
- Tests: status-bar message tests across live / ambiguous / dormant /
  no-mux cases.
- Blockers: `CSP-241`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-013`
