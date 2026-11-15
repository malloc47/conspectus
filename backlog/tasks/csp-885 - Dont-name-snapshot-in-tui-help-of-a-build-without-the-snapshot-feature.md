---
id: CSP-885
title: Don't name --snapshot in tui --help of a build without the snapshot feature
status: To Do
assignee: []
created_date: '2026-11-15 00:20'
labels:
  - tui
dependencies: []
priority: low
type: bug
ordinal: 962000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The `--no-resume-view` help (`src/cli/tui.rs`) ends "`--snapshot` implies this automatically", which `conspectus tui --help` prints in every build, including a release build without the `snapshot` feature, where no `--snapshot` flag exists. Found while verifying CSP-883 (the feature-off release binary's help still mentions the flag).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 tui --help in a build without the snapshot feature names no --snapshot flag, and a build with it still says --snapshot implies --no-resume-view
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
