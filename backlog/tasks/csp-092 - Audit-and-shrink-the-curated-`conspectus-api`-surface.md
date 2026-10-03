---
id: CSP-092
title: 'Audit and shrink the curated `conspectus::api` surface'
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-ref
milestone: m-11
dependencies: []
ordinal: 92000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-05 (`06c8423`). `api.rs` re-organized
  into 3 tiers with module docstring naming the tiering:
  (1) core API (graph model, discovery, resolution,
  render); (2) persistence + orchestration (declared /
  aliases / rename write paths); (3) `#[doc(hidden)]`
  internal support (`AliasWriteError`,
  `AliasWriteOutcome`, `AliasesDocument`, `AliasesSection`,
  `DeclaredSection`, `DeclaredStoreKind`,
  `DeclaredStoreSelection`, `select_store_for_declaration`,
  etc.). Every item still `pub use` so dev_scenarios +
  integration tests keep compiling; only the rustdoc
  surface is reduced.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-REF-010`
