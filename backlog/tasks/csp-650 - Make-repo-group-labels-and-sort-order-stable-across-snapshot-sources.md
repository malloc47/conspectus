---
id: CSP-650
title: Make repo group labels and sort order stable across snapshot sources
status: To Do
assignee: []
created_date: '2026-10-21 18:00'
updated_date: '2026-10-21 19:34'
labels:
  - tui
  - discovery
  - model
milestone: m-11
dependencies: []
references:
  - src/tui/rows/mux.rs
  - src/tui/rows/sessions.rs
  - src/discovery/mod.rs
  - src/discovery/git.rs
priority: medium
type: bug
ordinal: 708000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Repo group headers in the mux view swap order depending on whether the `conspectus-serve` daemon is running. Observed: with the daemon up, the `~/src/oss/conspectus` group sorts above the developer docs group; after killing the `conspectus-serve` mux (and with it the daemon), it sorts below. The group that actually moves is the developer docs repo (common dir `~/src/drw/up/documentation/developer/.git`), not conspectus. Its header label and sort key flip between `~/src/workspaces/documentation/developer` (daemon snapshot) and `~/src/drw/up/documentation/developer` (in-process `conspectus graph --refresh --no-cache`).

Root cause:
- Each git probe emits a Repo node whose `source_paths` holds only the probed worktree root (`fragment_from_probe` / `repo_node` in `src/discovery/git.rs`).
- `merge_fragments` (`src/discovery/mod.rs`) is first-write-wins per node id, so the surviving Repo node carries whichever checkout's probe merged first.
- Several providers probe the same repo: the workspace provider probes workspace members (`~/src/workspaces/...`), while the observed-cwd git probe iterates a sorted root set (`~/src/drw/...` first). In-process discovery merges providers in registration order; the daemon refreshes per-class slices and merges fresh-over-prior (warm-start backstop merge), so a different fragment can win. The daemon's winner may also change between ticks depending on which slice refreshed last (not yet confirmed).
- The mux view uses the first non-agent-deck `source_paths` entry as both the group label and the `RepoBucketKey` sort key (`PathIndex::repo_display_path` in `src/tui/rows/mux.rs`). The sessions view's `repo_display_path` (`src/tui/rows/sessions.rs`) has the same exposure. Other `source_paths.first()` readers (`src/declared/snapshot.rs` `repo_root`, `src/tui/runtime.rs`, explorer `source_paths` field) inherit the same nondeterminism.

Candidate fixes identified during triage (worker should re-check and choose):
1. Deterministic display path (smallest, TUI-only): derive the label/sort key from the repo's primary checkout root, or the common dir with `/.git` stripped, instead of `source_paths[0]`. Both `repo_display_path` copies need it.
2. Fix at the source: have `merge_fragments` union Repo `source_paths` (sorted, deduped) instead of first-write-wins. Stabilizes every `source_paths` reader, but changes model merge semantics and needs a new ADR plus a `docs/design.md` check.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The mux-view repo group label and its sort position are identical whether the snapshot comes from the daemon or from an in-process rebuild
- [ ] #2 The sessions-view repo group label and ordering are likewise independent of which provider fragment merged first
- [ ] #3 A repo with multiple checkouts probed by different providers (e.g. a primary checkout plus a workspace-member linked worktree) gets the same group label regardless of fragment merge order
- [ ] #4 Tests cover the multi-checkout case with fragments merged in both orders
- [ ] #5 If merge semantics for Repo nodes change, a new ADR records the decision and docs/design.md is updated
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Triage follow-up: ADR 0034 already decided the display-path rule for non-bare repos (derive from `common_dir` minus `/.git`; fall back to `source_paths.first()` only for bare repos). The sessions builder implemented it in 92b25f4, and the `SnapshotIndex` refactor in f8f7f14 dropped it, so `sessions.rs` `repo_display_path` regressed to `source_paths`-first. The mux builder's `PathIndex::repo_display_path` never followed ADR 0034. Candidate fix 1 is therefore "restore ADR 0034 in sessions and extend it to the mux view"; no new ADR is needed for that path.
<!-- SECTION:NOTES:END -->
