---
id: CSP-626
title: Split `src/model/mod.rs` by concern
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
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
- [ ] #1 `src/model/mod.rs` holds only module declarations, re-exports, and small cross-cutting helpers
- [ ] #2 Every existing `crate::model::` path and `conspectus::api` export still resolves
- [ ] #3 Serialized JSON and rkyv snapshot output are byte-identical, with no `FORMAT_VERSION` bump
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
