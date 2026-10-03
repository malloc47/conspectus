---
id: CSP-103
title: Add a GitHub Actions CI workflow
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-dist
milestone: m-11
dependencies: []
ordinal: 136000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: ADR 0016 commits to crates.io distribution but there is no
  `.github/workflows/` directory and the only check automation is local
  (`justfile`, `flake.nix`). Add a CI job that runs
  `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test --all-targets --all-features`, and
  `cargo nextest run --all-targets --all-features` on PRs and main.
- Tests: CI run on the change itself.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `.github/workflows/ci.yml` for pull requests and pushes
to `main`. The workflow installs stable Rust with `clippy` and `rustfmt`,
caches Cargo artifacts, installs `cargo-nextest`, and runs the same
baseline checks as the local `justfile`: formatting, clippy with warnings
denied, cargo test, nextest, and `git diff --check`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-DIST-001`
