---
id: CSP-593
title: Report forge discovery failures instead of failing silently
status: Done
assignee: []
created_date: '2026-10-12 23:35'
updated_date: '2026-10-12 23:54'
labels:
  - p4-fu
milestone: m-9
dependencies: []
priority: high
ordinal: 642000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Why: a missing, logged-out, or offline `gh` quietly yields no PRs with no diagnostic, so an empty PRs view is indistinguishable from a repo without PRs. The status bar already has a forge-unavailable chip that nothing feeds.
- Scope: emit a diagnostic when `gh` is unavailable or a query fails, and surface it through the TUI's forge chip.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A missing, unauthenticated, or offline `gh` produces a diagnostic naming the reason
- [x] #2 The TUI status bar shows the forge-unavailable chip with that reason
- [x] #3 A failure for one repo does not hide PRs found for others
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Emit ProviderUnavailable diagnostics from forge failures; keep discovery diagnostics through resolution; feed the TUI forge chip after each load.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Found while wiring the chip: `resolve_snapshot` dropped every discovery diagnostic, config problems included. Fixed by keeping discovery-owned diagnostics and regenerating the resolver's own.
- Failures whose stderr looks like a dead network classify as offline (`GhUnavailableReason::Offline`).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A missing, unauthenticated, or offline `gh`, or a failed query, now produces a `ProviderUnavailable` diagnostic that survives resolution and shows in the TUI status bar's forge chip (`gh:can't reach the forge`), while other repos' PRs still load. Verified with `just check` (partial-failure, diagnostic, resolve idempotence, and chip tests) and live runs with an offline `gh` shim.
<!-- SECTION:FINAL_SUMMARY:END -->
