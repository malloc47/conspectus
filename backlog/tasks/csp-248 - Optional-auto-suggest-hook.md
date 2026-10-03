---
id: CSP-248
title: Optional auto-suggest hook
status: To Do
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-ai-naming
milestone: m-11
dependencies: []
ordinal: 350000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: config-gated, off by default. Triggered on detection of a new
  session whose alias is unset and whose harness allows transcript
  context extraction. Surfaces a candidate name in the row tree until
  the operator accepts, edits, or dismisses.
- Tests: detection trigger tests; config-gate tests; dismissal
  persistence tests.
 - Blockers: `CSP-247`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-AI-NAMING-004`
