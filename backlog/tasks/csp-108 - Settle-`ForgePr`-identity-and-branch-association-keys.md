---
id: CSP-108
title: Settle `ForgePr` identity and branch-association keys
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-design
milestone: m-11
dependencies: []
ordinal: 141000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `docs/design.md` "Remaining Design Questions" lists open
  questions about provider-neutral ForgePr fields, branch-to-PR keying
  (name vs upstream vs head ref), and multi-PR-per-branch
  representation. Record decisions in an ADR; the GitHub adapter today
  matches by short head ref against the full local branch set
  (`CSP-059`) but the rule is undocumented.
- Tests: regression tests for fork-head PRs (different head repo) and
  closed/historical PR handling.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-DESIGN-002`
