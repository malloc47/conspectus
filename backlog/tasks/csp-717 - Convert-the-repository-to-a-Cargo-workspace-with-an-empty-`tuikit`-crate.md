---
id: CSP-717
title: Convert the repository to a Cargo workspace with an empty `tuikit` crate
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-workspace
milestone: m-25
dependencies: []
documentation:
  - docs/adr/0150-tui-framework-workspace-crate.md
  - docs/plans/tui-framework/workspace-impact.md
priority: high
type: chore
ordinal: 803000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0150 puts the TUI framework in a workspace member crate so the compiler, not review, keeps Conspectus types out of it. The repo is a single package today, with a root `[lints.clippy]` table and package-scoped commands in CI, the `justfile`, the dev-shell banner, and the Pre-Main Invariants. With the root still a package, `cargo test` and `cargo clippy` cover only that package, so a framework crate's tests could stop running in CI without any failure. The workspace-impact notes list what changes.

Scope: Add a `[workspace]` to the root `Cargo.toml` with `crates/tuikit` as a member and the root package kept. Move shared dependency versions to `[workspace.dependencies]` and the curated clippy levels to `[workspace.lints]`, with `lints.workspace = true` in both crates. Create `crates/tuikit/` with `publish = false`, an edition and license inherited from the workspace, and a `lib.rs` with a module-level doc. Add one smoke test in `tuikit` so a workspace run provably executes it. Add `--workspace` to `fmt`, `clippy`, `test`, `nextest`, and `doc` in the `justfile`, `.github/workflows/ci.yml`, the `flake.nix` banner, `CLAUDE.md`, and `AGENTS.md`. Keep `Cargo.lock` and `rust-toolchain.toml` shared at the root. Confirm `cargo install --path .` still installs `conspectus` and `csp`.

TUI changes: none expected. `conspectus tui --snapshot` output and every insta snapshot are unchanged.

Tests and checks: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features`, `cargo nextest run --workspace --all-targets --all-features`, and `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` all pass. CI shows the `tuikit` smoke test running.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The root `Cargo.toml` declares a workspace with the `conspectus` package and a `crates/tuikit` member; `tuikit` has `publish = false` and a module-level doc
- [ ] #2 Shared dependency versions are in `[workspace.dependencies]` and the clippy levels in `[workspace.lints]`, and both crates inherit them
- [ ] #3 `justfile`, CI, the dev-shell banner, `CLAUDE.md`, and `AGENTS.md` run formatting, clippy, tests, nextest, and docs with `--workspace`
- [ ] #4 A smoke test in `tuikit` runs under `cargo test --workspace` and under CI
- [ ] #5 `cargo install --path .` still installs `conspectus` and `csp`
- [ ] #6 Every existing test and snapshot passes unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
