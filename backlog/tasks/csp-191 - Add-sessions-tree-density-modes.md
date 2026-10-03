---
id: CSP-191
title: Add sessions-tree density modes
status: To Do
assignee: []
created_date: '2026-05-20 03:28'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-166
  - CSP-181
ordinal: 439000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a user-facing density setting for the sessions view
  so operators can trade context for row count. Suggested modes:
  `compact` (one line per session, no same-line previews),
  `balanced` (current locked behavior: same-line previews when
  width allows), and `expanded` (future richer preview treatment
  if operators still need it). Expose via config and a TUI toggle
  only after the base `/` search and help overlays are stable.
- Tests: row-tree/render snapshots for all density modes at
  80x24 and a wide terminal; config parsing tests once the
  setting is added.
- Blockers: `CSP-166` v1 slice; should follow `CSP-181` so wide
  inline behavior is not duplicated.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-015`
