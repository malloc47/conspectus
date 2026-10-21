---
id: CSP-652
title: Discover Atelier workspaces from observed session and mux cwds
status: To Do
assignee:
  - '@claude'
created_date: '2026-10-21 19:39'
labels:
  - discovery
  - workspace
dependencies: []
references:
  - src/discovery/mod.rs
  - src/discovery/atelier.rs
  - src/discovery/providers.rs
  - docs/adr/0027-workspace-detection-precedence.md
priority: medium
type: bug
ordinal: 710000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`docs/design.md` requires discovery to probe observed agent-session and mux cwds "to backfill repo, checkout, branch, and workspace context even when those paths are outside the launch cwd or configured scan roots", and ADR 0111 relies on that ("workspaces ... still appear through observed session and mux paths"). Only the git half exists: `observed_cwd_git_fragment` in `src/discovery/mod.rs` runs git probes, while Atelier workspace detection (`AtelierWorkspaceDiscovery`, `src/discovery/atelier.rs`) only walks up from explicit scan roots.

Operator impact: Atelier workspaces such as `~/src/workspaces/sysadmin` (an `atelier.toml` with `config` and `work-config` members) and `~/src/workspaces/documentation` never appear in the daemon's graph, because the daemon runs from `~/src/oss/conspectus`. With `--scan-root ~/src/workspaces/sysadmin` the same workspace is discovered with its `workspace_contains_repo` links and session associations, so the model handles it; discovery just never looks. Without the workspace node, neither the sessions view's Workspace grouping nor the planned mux workspace grouping can group work under "sysadmin".

ADR 0027 keeps generic (manifest-less) workspace inference limited to explicit scan roots; that stays as is. Provider manifests carry explicit workspace intent, so finding them from observed cwds does not invent groupings.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A workspace whose `atelier.toml` sits at or above an observed agent-session or mux cwd appears as a Workspace node with its `workspace_contains_repo` links, even when no scan root is inside it
- [ ] #2 Agent sessions whose cwd is inside such a workspace get the same `associated_with` workspace links they get when the workspace is under a scan root
- [ ] #3 Generic workspace inference is still limited to explicit scan roots (ADR 0027)
- [ ] #4 A malformed `atelier.toml` reached through an observed cwd produces a diagnostic instead of failing discovery
- [ ] #5 The observed-cwd workspace slice refreshes with the git class in the daemon and each manifest is read at most once per discovery run
- [ ] #6 Tests cover discovery from an observed cwd, a workspace already found through a scan root, and the malformed-manifest case
- [ ] #7 An ADR records the decision and docs/design.md reflects it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
