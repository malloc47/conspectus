---
id: CSP-543
title: Settle platform posture and CI shape
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 614000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: CI is ubuntu-only and runs the suite twice (`cargo test`, then
  `cargo nextest`). Process-tree attribution reads `/proc`; signal
  handling and watchers are Unix-only. Either add a macOS build/test job
  or state Linux-only in the README; drop the duplicate test run.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-012`
