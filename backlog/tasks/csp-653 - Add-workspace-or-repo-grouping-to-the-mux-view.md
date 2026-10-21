---
id: CSP-653
title: Add workspace-or-repo grouping to the mux view
status: To Do
assignee:
  - '@claude'
created_date: '2026-10-21 19:39'
labels:
  - tui
  - workspace
dependencies:
  - CSP-652
references:
  - src/tui/rows/mux.rs
  - src/tui/mod.rs
  - src/discovery/cross_link.rs
  - src/resolve/mod.rs
  - docs/adr/0061-drop-workspace-grouping-from-non-workspaces-views.md
  - docs/adr/0064-hybrid-sessions-graph-grouping.md
priority: medium
type: enhancement
ordinal: 711000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator wants muxes grouped by the workspace they work in rather than by the checkout inside it: a "sysadmin" Atelier workspace holds worktrees for `config`, `work-config`, and private repos, and its muxes should sit under one "sysadmin" header instead of being split across those repos' groups. Mux groupings today are `session`, `host`, and `repo` (`MuxGrouping` in `src/tui/mod.rs`).

ADR 0061 dropped a no-op `MuxGrouping::Workspace` and rejected a mux-only workspace grouping because "the Workspaces view is the canonical place for workspace-first navigation". ADR 0065 then removed the Workspaces view in favour of the sessions view's Workspace grouping, so the mux view no longer has any workspace-first surface and 0061's rationale for it no longer holds.

The operator chose a workspace-or-repo shape, the mux analogue of the sessions view's hybrid Graph grouping (ADR 0064): a mux in a workspace groups under the workspace, and every other mux keeps its `repo` bucket. The model already names the relationship: `docs/design.md`'s ER diagram has `MUX_SESSION rooted_in WORKSPACE` and `RelationKind::RootedIn` exists (the explorer labels it "rooted in" / "hosts"), but nothing emits it yet. Repo-bucket label stability for muxes outside workspaces is tracked separately in CSP-650.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `workspace` is a mux grouping value accepted by `--grouping`, `[tui.views.mux] grouping`, the controls overlay, and the grouping cycle
- [ ] #2 Under it, every mux rooted in a workspace sits under one header for that workspace, whichever member checkout or subdirectory it is in, using the same header format as the sessions view
- [ ] #3 Muxes not rooted in any workspace fall back to the `repo` grouping buckets; muxes with no resolvable repo go to Ungrouped last, and the Pins group stays first
- [ ] #4 Workspace headers sort before repo headers, each by display label
- [ ] #5 Discovery emits `rooted_in` candidate links from a mux to the deepest workspace containing its session path, and the resolver keeps one resolved slot per target kind
- [ ] #6 Unbound pin placeholders inside a workspace group under that workspace
- [ ] #7 Tests cover no workspaces, nested workspaces, a mux at the workspace root and in a member subdirectory, and a pin placeholder
- [ ] #8 An ADR records the decision (refining ADR 0061 for the mux view), and docs/design.md and docs/operations.md list the new grouping
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
