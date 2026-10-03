---
id: CSP-203
title: Retire legacy user-facing worktree terminology
status: Done
assignee: []
created_date: '2026-05-21 00:10'
labels:
  - h-checkout
milestone: m-11
dependencies:
  - CSP-202
ordinal: 150000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: rename CLI columns, docs, help text, and TUI labels from
  worktree to checkout where the
  user-facing meaning is the broader ADR 0026 concept. Keep git-linked
  worktree wording only when specifically describing git's feature.
- Tests: CLI help snapshots/table snapshots once those exist; docs-only
  `git diff --check` for prose-only slices.
- Blockers: `CSP-202`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Sessions table output now exposes `checkout`/`CHECKOUT`
instead of `worktree`/`WORKTREE`, PR table help and TUI checkout detail
labels use checkout terminology, and `--sessions-grouping checkout` is
accepted. Legacy `worktree` table columns, TUI grouping values, and
checkout JSON deserialization aliases are intentionally not preserved.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-CHECKOUT-007`
