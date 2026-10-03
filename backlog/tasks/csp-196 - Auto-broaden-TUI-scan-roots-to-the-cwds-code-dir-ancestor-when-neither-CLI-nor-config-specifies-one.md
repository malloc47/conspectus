---
id: CSP-196
title: >-
  Auto-broaden TUI scan roots to the cwd's "code dir" ancestor when neither CLI
  nor config specifies one
status: To Do
assignee: []
created_date: '2026-05-20 18:50'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-189
ordinal: 434000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Low priority.

- Scope: when `--scan-root` and `[tui].scan_roots` are both
  empty, walk up from the process cwd to the first ancestor
  that contains ≥ N (default 2 or 3) immediate-child entries
  that themselves look like repository roots (a `.git`
  directory or checkout). Use that ancestor as the scan root
  instead of cwd. Should be opt-in via a flag or config
  setting initially so we don't surprise operators who *want*
  the cwd-scoped behavior; promote to default later if it
  works out. The heuristic needs a clear cap (don't walk
  past `$HOME` or filesystem boundaries) and should fall back
  to the current cwd-scoped behavior when no plausible
  ancestor is found.
- Tests: pure helper tests for the ancestor walk over fixture
  directories with varied repo counts and depths; the
  runtime side wires through the same scan-root resolution
  path as `[tui].scan_roots`.
- Blockers: `CSP-189` v1 slice. Filed at low priority per
  operator direction — config-driven `[tui].scan_roots` is
  the preferred default; this auto-broaden mode is a
  "no-config still does the right thing most of the time"
  affordance.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-020`
