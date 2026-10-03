---
id: CSP-197
title: Memorialize the checkout context model
status: Done
assignee: []
created_date: '2026-05-21 00:10'
labels:
  - h-checkout
milestone: m-11
dependencies: []
ordinal: 144000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: record the decision in ADR 0026, update `docs/design.md` to
  use checkout terminology for the north-star model, and create this
  backlog workstream.
- Tests: docs-only; `git diff --check`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0026 defines `Checkout`, the identity rule, cwd
probing, workspace overlay behavior, grouping precedence, and the
staged terminology migration from legacy `Worktree` names.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-CHECKOUT-001`
