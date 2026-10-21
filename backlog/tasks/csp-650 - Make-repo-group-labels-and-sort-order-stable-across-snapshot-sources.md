---
id: CSP-650
title: Make repo group labels and sort order stable across snapshot sources
status: Done
assignee:
  - '@claude'
created_date: '2026-10-21 18:00'
updated_date: '2026-10-21 20:43'
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
- [x] #1 The mux-view repo group label and its sort position are identical whether the snapshot comes from the daemon or from an in-process rebuild
- [x] #2 The sessions-view repo group label and ordering are likewise independent of which provider fragment merged first
- [x] #3 A repo with multiple checkouts probed by different providers (e.g. a primary checkout plus a workspace-member linked worktree) gets the same group label regardless of fragment merge order
- [x] #4 Tests cover the multi-checkout case with fragments merged in both orders
- [x] #5 If merge semantics for Repo nodes change, a new ADR records the decision and docs/design.md is updated
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Restore ADR 0034 as one helper, `tui::rows::repo_display_path(repo_id, repo_node)`: non-bare repos use `common_dir` minus `/.git`; bare repos use the first non-agent-deck `source_paths` entry, else the common dir. No merge-semantics change, so no new ADR (candidate fix 1); amend ADR 0034's scope to cover the mux view.
2. Sessions (`SessionsData::repo_display_path`) and mux (`PathIndex::repo_display_path`) call the helper; the mux repo bucket label and sort key follow.
3. `cwd_for_node` for a Repo node uses the same path, so "new mux" on a repo header opens where the header says.
4. Tests: helper unit cases (non-bare ignores probe order, bare, bare without paths); mux and sessions repo grouping built from the same two fragments merged in both orders produce identical rows, with the linked-worktree case that regressed.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Triage follow-up: ADR 0034 already decided the display-path rule for non-bare repos (derive from `common_dir` minus `/.git`; fall back to `source_paths.first()` only for bare repos). The sessions builder implemented it in 92b25f4, and the `SnapshotIndex` refactor in f8f7f14 dropped it, so `sessions.rs` `repo_display_path` regressed to `source_paths`-first. The mux builder's `PathIndex::repo_display_path` never followed ADR 0034. Candidate fix 1 is therefore "restore ADR 0034 in sessions and extend it to the mux view"; no new ADR is needed for that path.

Took candidate fix 1 (restore ADR 0034, extend to the mux view); merge semantics are unchanged, so AC #5 needed only the ADR 0034 amendment (c321ab3), no new ADR. Also routed `cwd_for_node` for Repo nodes through the helper so "new mux" on a repo header opens in the path the header shows, rather than `source_paths.first()`. Left as is: `declared/snapshot.rs` `repo_root` (outside the TUI; ADR 0034 scope). Regression tests fail on the pre-fix sessions.rs/mux.rs and pass after. Live check: a temporary daemon built from this branch (isolated runtime/data/state dirs) and an in-process `--refresh --no-cache` render gave identical mux `repo` groups, with developer at `~/src/drw/up/documentation/developer` above conspectus in both.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Repo group labels and their sort position no longer depend on which checkout discovery probed first. ADR 0034's rule (a non-bare repo is the directory holding its `.git`; bare repos fall back to a probed checkout) now lives in `tui::rows::repo_display_path`, used by the sessions view (where a refactor had dropped it), the mux view's repo buckets, and the new-mux directory for a repo header. Landed in c321ab3 and 29fbe3d with helper tests and mux/sessions tests that merge the same fragments in both orders. A temporary daemon from this branch and an in-process rebuild render identical mux groups on the operator machine. `just check` passes (2574 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
