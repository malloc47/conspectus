---
id: CSP-365
title: Extend store selection for pin writes
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-362
ordinal: 298000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: reuse `select_store_for_declaration` for pin writes. Verify
  behavior for repo-rooted, checkout-rooted, workspace-rooted, and
  orphan-cwd pins. Reject pins whose `cwd` does not exist on the
  filesystem at write time (differs from declared links, per ADR
  0057).
- Tests: unit tests for each store-selection case plus the
  nonexistent-cwd rejection.
- Blockers: `CSP-362`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Pin writes use the existing nearest-store selection
shape, with explicit project/user overrides and cwd existence
validation before mutation.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-005`
