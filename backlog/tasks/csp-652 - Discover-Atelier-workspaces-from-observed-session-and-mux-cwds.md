---
id: CSP-652
title: Discover Atelier workspaces from observed session and mux cwds
status: Done
assignee:
  - '@claude'
created_date: '2026-10-21 19:39'
updated_date: '2026-10-21 20:13'
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
- [x] #1 A workspace whose `atelier.toml` sits at or above an observed agent-session or mux cwd appears as a Workspace node with its `workspace_contains_repo` links, even when no scan root is inside it
- [x] #2 Agent sessions whose cwd is inside such a workspace get the same `associated_with` workspace links they get when the workspace is under a scan root
- [x] #3 Generic workspace inference is still limited to explicit scan roots (ADR 0027)
- [x] #4 A malformed `atelier.toml` reached through an observed cwd produces a diagnostic instead of failing discovery
- [x] #5 The observed-cwd workspace slice refreshes with the git class in the daemon and each manifest is read at most once per discovery run
- [x] #6 Tests cover discovery from an observed cwd, a workspace already found through a scan root, and the malformed-manifest case
- [x] #7 An ADR records the decision and docs/design.md reflects it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0138: provider workspace manifests (Atelier) are discovered from observed agent-session and mux cwds; generic inference stays scan-root-only (ADR 0027). Update docs/design.md and the ADR index.
2. Add provider key `atelier::cwd` (git class) to the registry and `ProviderClass::Git.providers()` so the slice is gated and evicted with the git class.
3. Add `atelier::observed_cwd_fragment(snapshot, context)`: collect session cwd, mux cwd and active pane path; walk up for `atelier.toml`; dedupe manifests; skip workspaces already in the snapshot; run the existing `discover_config`; turn errors into `Diagnostic::Config`; stamp `atelier::cwd`.
4. Call it in `discover_local_warm_with` on the merged (fresh + prior) snapshot, before forge discovery and mutators, so cross_link sees the workspace and its membership links.
5. Tests: workspace discovered via an observed Codex session cwd outside the scan root, with an `associated_with` workspace link; a workspace already found through a scan root is not re-read or duplicated; a malformed manifest yields a diagnostic; registry tests updated.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in 82cac5a (ADR 0138) and 3d12164. New tests in atelier_tests.rs: observed Codex session cwd outside the scan root discovers the workspace with `atelier::cwd` provenance, membership links and a session `associated_with` link; a mux session path finds the manifest; a workspace already in the snapshot is skipped (manifest broken afterwards produced no diagnostic, so it was not re-read); a malformed manifest via an active-pane path yields one `Config` diagnostic. Full `cargo nextest run --all-targets --all-features`: 2558 passed. Real machine, in-process `graph --refresh --no-cache` from ~/src/oss/conspectus: `~/src/workspaces/sysadmin` and `~/src/workspaces/documentation` now appear (provider atelier) with 14 resolved session `associated_with` workspace relationships; previously absent.

Generic inference: `GenericWorkspaceDiscovery` is untouched and the new pass calls only Atelier's `discover_config`; existing workspace_tests pass. Read-once: manifests are collected into a set before any is loaded, and `observed_cwd_skips_workspaces_already_in_the_snapshot` shows a known workspace's manifest is not reopened. Class membership: `atelier::cwd` is a git-class heavy key in the registry and `ProviderClass::Git.providers()`, covered by the registry round-trip tests.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Atelier workspaces are now found from observed agent-session cwds and mux paths, not only from scan roots (ADR 0138). A post-pass on the merged snapshot walks up to the nearest `atelier.toml`, runs the existing Atelier discovery for workspaces the snapshot lacks, stamps the slice `atelier::cwd` (git class), and turns a malformed manifest into a diagnostic. Generic inference stays scan-root-only (ADR 0027). Landed in 82cac5a and 3d12164 with four new tests. On the operator machine an in-process rebuild now shows `~/src/workspaces/sysadmin` and `~/src/workspaces/documentation` with 14 resolved session associations; neither appeared before. `just check` passes.
<!-- SECTION:FINAL_SUMMARY:END -->
