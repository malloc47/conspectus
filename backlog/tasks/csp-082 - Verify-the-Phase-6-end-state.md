---
id: CSP-082
title: Verify the Phase 6 end state
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies:
  - CSP-076
  - CSP-077
  - CSP-078
  - CSP-079
  - CSP-080
  - CSP-081
ordinal: 77000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: run the full Phase 6 automated and manual check set and
  record follow-up tasks instead of expanding Phase 6 scope.
- Tests: `just check`; `cargo doc --no-deps`.
- Manual checks: run the four `cargo run -- session` smoke commands
  plus `cargo run -- graph --format json` on a real workspace and
  confirm the output matches what Atelier users previously got from
  the deprecated commands.
- Blockers: `CSP-076`, `CSP-077`, `CSP-078`, `CSP-079`, `CSP-080`,
  `CSP-081`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`nix develop --command just check` passed, including
formatting, clippy, `cargo test --all-targets --all-features`,
`cargo nextest run --all-targets --all-features` with 279 tests,
and `git diff --check`. `nix develop --command cargo doc --no-deps`
passed and generated docs for the curated API facade. Manual smoke
checks against the Conspectus workspace with tmux/forge disabled
passed for `cargo run -- session --scan-root .`,
`cargo run -- session --projection mux --scan-root .`,
`cargo run -- session --projection union --scan-root .`, and
`cargo run -- graph --format json --scan-root .`; outputs contained
45, 2, 45, and 588 lines respectively.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-010`
