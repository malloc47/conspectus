---
id: CSP-056
title: Add representative JSON and table snapshots
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-053
  - CSP-054
  - CSP-055
ordinal: 54000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: snapshot graph JSON for a repo with zero / one / multiple
  open PRs and for one-to-many branch ↔ PR ambiguity. Add session-table
  snapshots in each projection for orphan sessions, single-mux match,
  one-to-many mux candidates, fork-associated sessions, and
  branch-with-PR scenarios. All snapshots normalise temp paths to
  `/fixture`.
- Tests: `cargo test --all-targets --all-features`; `cargo nextest run
  --all-targets --all-features`.
- Manual checks: review snapshots for stable ordering, readable
  provenance / confidence / ambiguity, and preserved competing-PR
  evidence.
- Blockers: `CSP-053`, `CSP-054`, `CSP-055`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tests/forge_snapshots.rs` with six end-to-end
snapshots driven by `discover_local_with` + `FakeGh` against a
temp git repo: zero-PR JSON, one-open-PR JSON (matched
branch endpoint + resolved relationship), multi-PR JSON
(open + merged + closed), and three session-table projections
(agent with PR, mux empty, union with PR). Path normalization
rewrites the temp path to `/fixture` so reruns are byte-stable.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-012`
