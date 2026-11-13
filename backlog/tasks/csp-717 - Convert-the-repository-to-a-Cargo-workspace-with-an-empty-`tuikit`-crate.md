---
id: CSP-717
title: Convert the repository to a Cargo workspace with an empty `tuikit` crate
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 03:05'
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
- [x] #1 The root `Cargo.toml` declares a workspace with the `conspectus` package and a `crates/tuikit` member; `tuikit` has `publish = false` and a module-level doc
- [x] #2 Shared dependency versions are in `[workspace.dependencies]` and the clippy levels in `[workspace.lints]`, and both crates inherit them
- [x] #3 `justfile`, CI, the dev-shell banner, `CLAUDE.md`, and `AGENTS.md` run formatting, clippy, tests, nextest, and docs with `--workspace`
- [x] #4 A smoke test in `tuikit` runs under `cargo test --workspace` and under CI
- [x] #5 `cargo install --path .` still installs `conspectus` and `csp`
- [x] #6 Every existing test and snapshot passes unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Root `Cargo.toml`: add `[workspace]` (members `.` and `crates/tuikit`, `resolver = "3"`, `default-members` of both so a bare cargo command cannot skip the framework), `[workspace.package]` (edition, license), `[workspace.dependencies]` (every third-party dependency and dev-dependency, with its comments and feature sets, so later extraction stories write `dep = { workspace = true }`), and `[workspace.lints]` (the curated clippy and rustdoc tables moved verbatim). Root package inherits edition and license, uses `workspace = true` for its dependencies, and sets `[lints] workspace = true`. `default-run`, `[features]`, and binaries stay as they are.
2. Create `crates/tuikit/` (`publish = false`, edition and license inherited, `[lints] workspace = true`, no dependencies yet), a `lib.rs` with a module-level `//!` doc that names no Conspectus domain word, and one integration smoke test under `crates/tuikit/tests/`.
3. Add `--workspace` to clippy, test, nextest, and doc in `justfile`, `.github/workflows/ci.yml`, the `flake.nix` dev-shell banner, and the Pre-Main Invariants in `AGENTS.md` (`CLAUDE.md` is a symlink to it). Add one line noting the repository is a workspace.
4. Verify: `cargo metadata` shows both members and the shared lockfile; `cargo install --path . --locked --offline --debug` into a throwaway `--root` and `--target-dir` under `/tmp` installs `conspectus` and `csp`; fmt, clippy `--workspace -D warnings`, the isolated full test run with `--workspace` (baseline 3424 passed, 0 failed, 1 ignored plus the smoke test), nextest `--workspace`, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `git diff --check`, and `cargo run --features snapshot -- tui --snapshot` still works.
5. Update the execution brief only if its check commands turn out to be wrong, then close the task with the final summary, checked criteria, and Done.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Moved every third-party dependency and dev-dependency (with its comments and feature sets) into `[workspace.dependencies]`, so later extraction stories write `dep = { workspace = true }` instead of deciding per story which versions are shared. The root package inherits edition and license from `[workspace.package]`; `tuikit` has no dependencies yet.
Added `default-members = [".", "crates/tuikit"]` as a belt-and-suspenders guard: a bare `cargo clippy` or `cargo test` still reaches the framework. CI and `just` pass `--workspace` regardless. `cargo run --features snapshot`, `cargo build --features snapshot`, and `cargo nextest run --all-features --test tui_columns_doc` (the `just tui-columns` form) still work with it.
`RUSTDOCFLAGS='-D warnings' cargo doc --no-deps` already failed on `main` (eight broken intra-doc links in `content_search`, `tui/search.rs`, `tui/effect.rs`, `tui/widgets/viewer_help.rs`, confirmed on an export of the pristine commit). Fixed those comments in a separate docs commit so the workspace doc check can pass; no behavior changed.
Edited `/tmp/tuifw-baseline/run-tests.sh` to pass `--workspace` so the isolated runner also executes the `tuikit` suite (baseline 3424 passed plus the smoke test = 3425).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The repository is now a Cargo workspace: the root `conspectus` package plus `crates/tuikit` (`publish = false`, no dependencies, a crate-level `//!` doc, and `tests/smoke.rs`). Edition and license come from `[workspace.package]`, all third-party versions from `[workspace.dependencies]` (the whole set, so later stories write `{ workspace = true }`), and the clippy and rustdoc levels from `[workspace.lints]`; both crates set `[lints] workspace = true`. `default-run`, the `snapshot` feature, the binaries, and `rust-toolchain.toml` are unchanged, and `Cargo.lock` gained only the `tuikit` entry. `justfile`, CI, the dev-shell banner, and the Pre-Main Invariants in `AGENTS.md` (`CLAUDE.md` is a symlink to it) run clippy, test, nextest, and doc with `--workspace`.

Choices to review: `default-members` lists both crates (a bare cargo command also reaches `tuikit`), and every dependency moved into the workspace table rather than only the framework-bound ones. Also fixed eight broken intra-doc links that already failed `cargo doc -D warnings` on `main` (comments only, separate commit), and edited `/tmp/tuifw-baseline/run-tests.sh` to pass `--workspace`.

Verification: `cargo metadata` lists both members; a probe violation in `tuikit` was rejected by the inherited `uninlined_format_args` deny; `cargo install --path . --locked --offline --debug --root /tmp/...` installed `conspectus` and `csp` (both report 0.1.0; scratch dirs removed); `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, and `git diff --check` pass; the isolated `cargo test --workspace --all-targets --all-features` run gave 3425 passed, 0 failed, 1 ignored (baseline 3424 plus the smoke test); `cargo nextest run --workspace --all-targets --all-features` gave 3425 passed, 1 skipped and lists `tuikit::smoke workspace_test_run_executes_tuikit_tests`. No snapshot or test changed. `tui --snapshot` output is unchanged.

Regressions: none. Consistency wins: none. No visible TUI change.
<!-- SECTION:FINAL_SUMMARY:END -->
