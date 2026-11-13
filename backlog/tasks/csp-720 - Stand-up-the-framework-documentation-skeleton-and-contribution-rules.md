---
id: CSP-720
title: Stand up the framework documentation skeleton and contribution rules
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 04:03'
labels:
  - tui-framework
  - tuifw-workspace
milestone: m-25
dependencies:
  - CSP-717
documentation:
  - docs/adr/0150-tui-framework-workspace-crate.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: high
type: docs
ordinal: 806000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The framework is internal-reuse only, but it must stand alone: someone in a different repository has to be able to learn what it is, how it is layered, how to add a control, and which rules keep it independent without reading Conspectus. Writing the skeleton now means each extraction story fills in its own control's page as it lands, so the documentation is never a final-week scramble.

Scope: Create under `crates/tuikit/`: `README.md` (purpose, layers, status as internal-reuse, how to depend on it); `docs/architecture.md` (the layers and module map, the one-way dependency rule, the host message type `M` and context type `Cx`, intent outcomes instead of effects, what the framework never does); `docs/contributing.md` (independence rules and their guards, how to add a control, the house rules summarized, how to catalog TUI changes, how to test with the test kit); `docs/controls/_template.md` (purpose, usage example, key table, theme tokens, house rules followed, seams left open); and `docs/decisions/README.md` (an index of framework-scoped decisions, populated later). Decide and enforce a documentation lint for the public API: module docs required, and public types documented, so a missing page fails the build.

TUI changes: none.

Tests and checks: `RUSTDOCFLAGS='-D warnings' cargo doc -p tuikit --no-deps` passes. The doc lint fails on an undocumented public item in a scratch change.

Blockers: The workspace conversion.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `crates/tuikit` has a README, an architecture guide, a contribution guide, a per-control page template, and a decisions index
- [x] #2 The architecture guide names the layers, the dependency direction, and the `M` and `Cx` parameters
- [x] #3 A documentation lint is enforced for the public API and fails the build when a public module or type lacks docs
- [x] #4 The contribution guide describes the independence guards and how a story catalogs its TUI changes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Write the framework docs under crates/tuikit: README.md, docs/architecture.md, docs/contributing.md, docs/controls/_template.md, docs/decisions/README.md. Keep host vocabulary out of them (independence guard scans docs); describe the host's repo-wide hygiene scans generically, not by file name.
2. Doc lint: #![deny(missing_docs)] plus the two rustdoc link denies in crates/tuikit/src/lib.rs (the workspace lint table cannot carry missing_docs without hitting the host crate, and a lift to its own repository must keep the lint); make lib.rs crate docs the README via include_str so they cannot drift; add tests/documentation.rs that fails if the lint attributes, the pages, or the template sections go missing. Prove the lint fails on a scratch undocumented public item and a scratch undocumented module, then remove them.
3. Move the guard rules, matching behavior, tripwire notes, and the allowlist-request process out of the module docs of tests/independence.rs into docs/contributing.md; leave a short pointer; update the failure message and its self-test; raise the vocabulary tripwire minimum now that docs exist.
4. Host side: link the README from docs/index.md; add a pointer in tests/support/source_tree.rs to the generic description.
5. Run fmt, clippy (workspace), the isolated full suite, RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps, git diff --check; verify the ACs; close the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Handoff from CSP-719: the table of which repo hygiene rules apply to the framework, and why the rest do not, is the module doc of `tests/support/source_tree.rs` in the root workspace. Move it into `crates/tuikit/docs/contributing.md` (keeping the words the framework cannot use out of it; see the guard's `DOMAIN_TERMS`), and leave a pointer behind so the two do not drift. The same module doc explains how to raise a scan's minimum file count when code moves into the framework.

Moved the guard rules, matching notes, and allowlist-request process from the module docs of crates/tuikit/tests/independence.rs into crates/tuikit/docs/contributing.md and left a short pointer; the failure message and its self-test now name the guide. The host-specific rule-to-crate table from the CSP-719 handoff stays in tests/support/source_tree.rs because it names the host; the guide describes the same scans generically, and each side points at the other. Raised MIN_VOCABULARY_FILES from 1 to 7 (9 files visited). The doc lint is crate-level because Cargo rejects extending [workspace.lints] from a package. Validation: independence and documentation tests pass; a scratch undocumented pub struct and pub mod failed cargo check and cargo doc, then were removed; isolated suite 3481 passed, 0 failed, 1 ignored.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the framework documentation set under crates/tuikit: README.md, docs/architecture.md, docs/contributing.md, docs/controls/_template.md, and docs/decisions/README.md (empty index plus policy, including canonical-source rules before and after separation). All of it avoids the host vocabulary and passes the independence guard. The guard rules, matching notes, tripwire guidance, and allowlist-request process moved from the module docs of tests/independence.rs into contributing.md, with a pointer left behind; the host-specific hygiene table stays in tests/support/source_tree.rs (it names the host) and contributing.md describes those scans generically. Documentation lint decision: crate-level deny(missing_docs) plus both rustdoc link lints in src/lib.rs (Cargo rejects extending the workspace lint table, and an attribute survives a directory lift); the crate-level docs are the README via include_str; tests/documentation.rs fails if the attributes, include, pages, or template sections go missing. MIN_VOCABULARY_FILES rose from 1 to 7. docs/index.md links the README. Verified: cargo fmt --check, workspace clippy -D warnings, RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps, git diff --check, and the isolated full suite (3481 passed, 0 failed, 1 ignored; baseline 3478 plus 3 new tests) all pass; an undocumented public struct and an undocumented public module in a scratch change both failed the build, then were removed; a stray host word in contributing.md failed the vocabulary guard, then was removed. Regressions: none (no visible TUI changes). Consistency wins: none (no visible TUI changes).
<!-- SECTION:FINAL_SUMMARY:END -->
