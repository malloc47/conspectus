---
id: CSP-204
title: Hard-rename checkout graph wire/model names
status: Done
assignee: []
created_date: '2026-05-22 02:30'
labels:
  - h-checkout
milestone: m-11
dependencies:
  - CSP-203
ordinal: 151000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace legacy `WorktreeId`/`WorktreeNode`/`GraphNode::Worktree`
  naming, node id display prefixes, JSON `type: "worktree"`, snapshot
  expectations, declared endpoint syntax, and user-visible relation docs
  with checkout terminology where the concept is broader than git linked
  worktrees. Keep git-specific `worktree` only for actual `git worktree`
  feature behavior and source metadata.
- Tests: full graph snapshot refresh, declared endpoint round trips, node
  id round trips, resolver tests for checkout/session links, table/TUI
  smoke coverage, and `cargo test --all-targets --all-features`.
- Blockers: `CSP-203`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Graph/model names now use `CheckoutId`, `CheckoutNode`, and
`GraphNode::Checkout`; node ids display as `checkout:...`; graph JSON
serializes `type: "checkout"`; declared endpoints use
`checkout:repo_common_dir=...,root=...`; and fork effect relation wire
names are `created_checkout` / `referenced_checkout`. Git commands,
fixture names, and provider-native Atelier fields still say worktree when
they describe actual git or source-format worktree concepts.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-CHECKOUT-008`
