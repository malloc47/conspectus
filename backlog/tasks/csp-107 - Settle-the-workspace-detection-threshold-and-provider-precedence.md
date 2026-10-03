---
id: CSP-107
title: Settle the workspace-detection threshold and provider precedence
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-design
milestone: m-11
dependencies: []
ordinal: 140000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `docs/design.md` "Remaining Design Questions" calls out (a)
  evidence threshold for inferring a generic `Workspace`, (b) handling
  of nested workspaces / nested repos / symlinked repos, (c)
  interaction with provider-specific metadata, and (d) precedence when
  multiple providers claim the same path. Today
  `src/discovery/workspace.rs` infers a workspace only when a scan root
  has 2+ immediate git repo children; document the rule in an ADR or
  refine it.
- Tests: fixture tests covering the chosen rule for nested, symlinked,
  and provider-claimed roots.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0027 settles the generic threshold as two or more
immediate git checkout children under an explicit scan root, with
provider-specific workspace metadata taking precedence over generic
inference at the same canonical root. Generic inference now stands
down when `atelier.toml` claims the scan root.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-DESIGN-001`
