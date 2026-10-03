---
id: CSP-025
title: Add generic workspace inference
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-024
ordinal: 25000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: infer generic workspace roots from configured roots or layout
  evidence and link participating repos/checkouts without fabricating
  workspaces for standalone repo-only cases.
- Tests: fixture tests for multi-repo workspace roots, standalone repos, and
  checkouts outside any workspace.
- Manual checks: inspect JSON for generic workspace fixtures and confirm
  workspace nodes appear only when there is workspace evidence.
- Blockers: `CSP-024`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Inferred generic workspaces only for explicit scan roots with
multiple immediate git repo children, linked those repos with convention
evidence, and kept standalone or single-repo roots repo-only.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-005`
