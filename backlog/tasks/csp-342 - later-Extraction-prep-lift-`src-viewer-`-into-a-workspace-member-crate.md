---
id: CSP-342
title: '(later) Extraction prep: lift `src/viewer/` into a workspace member crate'
status: To Do
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 224000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: when the viewer module's import surface has been
  stable for ≥ N stories, create a `crates/` workspace,
  move `src/viewer/` to `crates/conspectus-transcript-viewer/`,
  add a thin `bin` target with clap that takes a
  `SessionLocator` from argv. Conspectus depends on it as a
  workspace member.
- Tests: existing viewer tests run from the new crate
  location; conspectus TUI still routes through it
  unchanged.
- Blockers: stability of the dep surface (see
  `docs/transcript-viewer-deps.md` history).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIEWER-NATIVE-010`
