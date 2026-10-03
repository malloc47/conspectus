---
id: CSP-007
title: Add runtime and test dependencies
status: Done
assignee: []
created_date: '2026-05-15 02:31'
labels:
  - p0
milestone: m-1
dependencies:
  - CSP-004
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add runtime dependencies `clap`, `serde`, `serde_json`, `toml`,
  `toml_edit`, `indexmap`, `anyhow`, and `thiserror`; add test
  dependencies `assert_cmd`, `predicates`, `tempfile`, `insta`, `rstest`,
  and `proptest`.
- Tests: dependency graph resolves and `cargo test --all-targets
  --all-features` succeeds.
- Manual checks: verify dependencies are grouped by runtime vs dev usage in
  `Cargo.toml`.
- Blockers: `CSP-004`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P0-004`
