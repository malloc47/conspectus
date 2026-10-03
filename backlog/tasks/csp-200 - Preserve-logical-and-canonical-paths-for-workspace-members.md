---
id: CSP-200
title: Preserve logical and canonical paths for workspace members
status: Done
assignee: []
created_date: '2026-05-21 00:10'
labels:
  - h-checkout
milestone: m-11
dependencies: []
ordinal: 147000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: when a workspace member is reached through a symlink or
  provider-local member path, store both the workspace-visible logical
  path and the canonical checkout root. Use canonical checkout root for
  identity and logical path/source metadata for display and evidence.
- Slice landed: generic and Atelier `workspace_contains_repo`
  candidates now preserve `logical_path`, `canonical_checkout_root`
  when discovered, and `member_path_kind` source metadata. Atelier
  links also preserve `provider_source_path` and `repo_name`.
- Tests: fixtures covering symlinked plain clones, provider member
  paths, broken symlinks, and duplicate logical paths resolving to the
  same checkout.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Generic discovery skips broken symlink members instead of
aborting the scan, and duplicate workspace-visible paths resolving
to the same repo get distinct candidate IDs keyed by logical path.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-CHECKOUT-004`
