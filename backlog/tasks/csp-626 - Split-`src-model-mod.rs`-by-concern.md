---
id: CSP-626
title: Split `src/model/mod.rs` by concern
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:09'
updated_date: '2026-10-18 01:22'
labels:
  - h-hyg
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: low
type: chore
ordinal: 684000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-470 split the TUI and CLI monoliths, but `src/model/mod.rs` was not on the audit's list and has since become the largest production file in the crate, about 2,470 lines. It holds the id types and the `simple_id!` macro, every node struct, `RelationKind`, `Provenance`, `Confidence`, and `Freshness`, `GraphLink` and `SourceMetadata`, `NodeKind` and `MatchKind`, the resolver explanation types, `Diagnostic`, `GraphSnapshot`, and `SnapshotIndex`. Features start from the model (the design guardrails are data-model-first), so reviewers read this file more than most. Extends audit section D.

- Scope: move the groups into submodules under `src/model/` and re-export them from `mod.rs`, so every `crate::model::X` path and every `conspectus::api` export (ADR 0015) stays unchanged. Serde field names and the rkyv archive layout must not change.
- Tests: all suites pass; JSON graph fixtures and `graph.bin` round-trip tests are byte-identical, with no `snapshot::FORMAT_VERSION` bump.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `src/model/mod.rs` holds only module declarations, re-exports, and small cross-cutting helpers
- [x] #2 Every existing `crate::model::` path and `conspectus::api` export still resolves
- [x] #3 Serialized JSON and rkyv snapshot output are byte-identical, with no `FORMAT_VERSION` bump
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Group the top-level items of `src/model/mod.rs` by concern: `ids` (id types, `NodeId`), `node` (`GraphNode`, node structs, `NodeKind`, preview helpers), `link` (relation, provenance, confidence, freshness, `GraphLink`, `SourceMetadata`, `MatchKind`, `pick_preferred`), `resolution` (resolved relationships, scores, `NodeProvenance`, `Diagnostic`), `pin` (pin candidate, binding, mux ref), `graph` (`GraphSnapshot` with its serde helper, `SnapshotIndex`). Name it `graph`, not `snapshot`, to avoid confusion with `crate::snapshot`.
2. Move each item with its docs and attributes, in file order, via a script that asserts every original non-blank line lands in exactly one output file. Submodules `use super::*;`; `mod.rs` glob re-exports them so every `crate::model::X` path is unchanged.
3. Verify serialization is byte-identical: rkyv bytes and JSON of the showcase fixture from both versions; `FORMAT_VERSION` untouched.
4. `just check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `src/model/mod.rs` is now 46 lines: imports, `rkyv_adapters`, `Metadata`, `path_is_ancestor_of`, module declarations, and `pub use` re-exports. New files: `ids.rs` (306), `node.rs` (833), `link.rs` (569), `resolution.rs` (245), `pin.rs` (170), `graph.rs` (355).
- No privacy or import changes were needed: submodules see the parent's imports through `use super::*;`, and `GraphSnapshot` stays next to the `node_provenance_serde` module its `#[serde(with = ...)]` string names.
- Validation: a throwaway example serialized `tests/fixtures/showcase.json` to `graph.bin` bytes and back to JSON on the old and new model code; both sha256 hashes match (57,632-byte rkyv archive). `snapshot::FORMAT_VERSION` unchanged at 6. `just check` green (2413 tests), including rustdoc with broken intra-doc links denied and every `conspectus::api` re-export.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/model/mod.rs` is split by concern into `ids`, `node`, `link`, `resolution`, `pin`, and `graph` submodules, re-exported from a 46-line `mod.rs`, so every `crate::model::` path and `conspectus::api` export still resolves. Serialization is unchanged: the showcase fixture's rkyv `graph.bin` bytes and JSON are byte-identical before and after, and `FORMAT_VERSION` stays at 6. Verified with that comparison and `just check` (2413 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
