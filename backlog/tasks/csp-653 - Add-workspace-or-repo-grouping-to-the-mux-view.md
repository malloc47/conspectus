---
id: CSP-653
title: Add workspace-or-repo grouping to the mux view
status: Done
assignee:
  - '@claude'
created_date: '2026-10-21 19:39'
updated_date: '2026-10-21 20:13'
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
- [x] #1 `workspace` is a mux grouping value accepted by `--grouping`, `[tui.views.mux] grouping`, the controls overlay, and the grouping cycle
- [x] #2 Under it, every mux rooted in a workspace sits under one header for that workspace, whichever member checkout or subdirectory it is in, using the same header format as the sessions view
- [x] #3 Muxes not rooted in any workspace fall back to the `repo` grouping buckets; muxes with no resolvable repo go to Ungrouped last, and the Pins group stays first
- [x] #4 Workspace headers sort before repo headers, each by display label
- [x] #5 Unbound pin placeholders inside a workspace group under that workspace
- [x] #6 Tests cover no workspaces, nested workspaces, a mux at the workspace root and in a member subdirectory, and a pin placeholder
- [x] #7 An ADR records the decision (refining ADR 0061 for the mux view), and docs/design.md and docs/operations.md list the new grouping
- [x] #8 Discovery emits a `rooted_in` candidate link from a mux to the innermost workspace containing its session path, and the resolver keeps one rooted-in workspace per mux, with declared links outranking inferred ones
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0139: mux `workspace` grouping (workspace-or-repo, refines ADR 0061 for the mux view), `MuxSession --rooted_in--> Workspace` from the mux session path, single resolved slot per mux. Update docs/design.md, docs/operations.md, ADR index.
2. Move cross_link's workspace path index (workspace root + member `logical_path`s, deepest match) and its path helpers into `src/model/workspace_paths.rs` so discovery and the TUI share one matcher; add an innermost-workspace query.
3. cross_link: emit `rooted_in` for each mux whose session path (`cwd`, else active pane path) sits in a workspace, targeting the innermost one.
4. TUI: `MuxGrouping::Workspace` ("workspace") in values_for/as_str/dispatch. Builder: Pins group first, then one header per workspace (resolved `rooted_in` for muxes, the shared matcher for unbound pin placeholders), sorted by label then root, then the remaining muxes through the existing repo bucketing + Ungrouped. Share the workspace header label with the sessions view via a helper in rows/mod.rs.
5. Tests: cross_link rooted_in (root, member subdir, nested workspaces, none); mux builder workspace grouping (no workspaces, mux at root and in member, nested, pin placeholder, ordering vs repo buckets); grouping round-trip. Snapshot check with `tui --snapshot`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in 61683e4 (ADR 0139, ADR 0061 forward link, design/operations docs), edf90bd (shared `model::workspace_paths` index, mux `rooted_in` from cross_link, resolver slot test, updated atelier delegation graph snapshot: one added candidate and resolved slot), and 9010cd7 (`MuxGrouping::Workspace`, builder, shared `workspace_header_label`). Kept `rooted_in` a single resolver slot rather than multi-target so declared links keep precedence; recorded in ADR 0139. Snapshot checks on a captured live fixture: `--grouping workspace` shows Pins, then `documentation  about+...  (atelier)` (doc-workspace-2, doc-ws-data-platform-server, doc-workspace-1 placeholder, doc-ws-developer-nextra) and `sysadmin  config+work-config  (atelier)` (sysadmin), then repo buckets dek, data-platform, conspectus; `repo` grouping on the same fixture shows six repo groups including `developer`. `[tui.views.mux] grouping = "workspace"` in a temp config selects it with no diagnostic; the controls overlay lists `workspace` under Grouping (Mux); Ctrl-G cycles repo -> workspace -> session; the explorer shows `rooted in ▦ sysadmin` on the sysadmin mux.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The mux view has a `workspace` grouping (ADR 0139): after the Pins group, one header per workspace a mux is rooted in, in the sessions view's header format and sorted by label, then the `repo` buckets for every other mux, then Ungrouped. Membership comes from new `MuxSession --rooted_in--> Workspace` links that cross-link inference emits for the innermost workspace containing the mux's session path; unbound pin placeholders are placed by cwd through the same shared path index. Covered by matcher, cross-link, resolver, and builder tests, and checked with `tui --snapshot` on live data, where the operator's `sysadmin` and `documentation` muxes each sit under one workspace header. `just check` passes.
<!-- SECTION:FINAL_SUMMARY:END -->
