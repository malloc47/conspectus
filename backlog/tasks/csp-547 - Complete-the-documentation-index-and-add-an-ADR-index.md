---
id: CSP-547
title: Complete the documentation index and add an ADR index
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 618000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Progress: the ADR index (`docs/adr/README.md`) landed with `CSP-536.02`;
  the `docs/index.md` gaps remain.
- Scope: `docs/index.md` omits `mux-link-resolution.md`,
  `provider-adapter-guide.md`, `comparison.md`, the three audits,
  `transcript-viewer-deps.md`, the `tui-*` reviews and mockups, and
  `plans/`. Add `docs/adr/README.md` grouping the 97 ADRs by theme with
  status (model and evidence, discovery and attribution, persistence and
  daemon, TUI, pins and mux lifecycle, worktrees, process and tooling,
  superseded); it doubles as a talk slide.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-016`
