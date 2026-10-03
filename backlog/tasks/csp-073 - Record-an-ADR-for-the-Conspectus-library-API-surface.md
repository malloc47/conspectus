---
id: CSP-073
title: Record an ADR for the Conspectus library API surface
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies:
  - CSP-071
ordinal: 68000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: write an ADR that names the publicly stable modules
  (`model`, `output`, `resolve`, `config`, `declared`,
  `discovery::{git,tmux,forge,harness,atelier,workspace,declared,
  cross_link}`), declares the rest internal, and commits to a
  semver discipline. Decide whether to surface a curated
  `pub use` facade (e.g. `conspectus::api`) and how `#[doc(hidden)]`
  is applied to internals.
- Tests: docs-only; `git diff --check`.
- Manual checks: cross-check the proposed stable list against
  `src/lib.rs`, the existing `pub` items in each module, and
  the migration plan in `docs/design.md`.
- Blockers: `CSP-071`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Accepted ADR 0015, which names the stable public modules,
commits to a semver discipline, keeps existing module paths
supported, marks CLI internals outside the library contract, and
requires a curated `conspectus::api` facade for common consumers.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-001`
