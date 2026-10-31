---
id: CSP-763
title: Rehearse separating the framework into its own repository
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:28'
updated_date: '2026-10-31 06:11'
labels:
  - tui-framework
  - tuifw-separation
milestone: m-25
dependencies:
  - CSP-761
  - CSP-718
documentation:
  - docs/adr/0150-tui-framework-workspace-crate.md
priority: high
type: task
ordinal: 849000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0150's goal is that the framework can move to its own repository at any time. That claim is only true if someone has done it: an unproven separation tends to fail on a workspace-inherited field, a relative path, or a test fixture that quietly reads a Conspectus file. A rehearsal that runs in CI keeps the property true afterward.

Scope: Write a script (for example `scripts/export-tuikit.sh`) that produces a standalone repository from `crates/tuikit`, keeping its history (`git subtree split`), and adds the root files a repository needs: a toolchain file, license, `.gitignore`, a CI workflow, a `justfile`, and a minimal dev-shell flake. It rewrites workspace-inherited manifest fields so the crate resolves alone. In a scratch directory, formatting, clippy with warnings denied, all tests with all features, and docs with warnings denied pass with no Conspectus checkout present. Run the rehearsal in CI (or as a `just` recipe that CI calls) on changes under `crates/tuikit`, and document the real move step by step in the framework docs.

TUI changes: none.

Tests and checks: Run the script on a clean clone and confirm the four checks pass from the scratch export. Introduce a deliberate leak (a relative path to `../../src`) and confirm the rehearsal fails.

Blockers: Complete documentation and the independence guard.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A script exports `crates/tuikit` with its history and the root files into a scratch directory
- [x] #2 In the export, formatting, clippy (warnings denied), all-feature tests, and docs (warnings denied) pass with no Conspectus files present
- [x] #3 CI runs the rehearsal on changes under `crates/tuikit`
- [x] #4 A deliberate leak makes the rehearsal fail
- [x] #5 The framework docs describe the real separation procedure
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Guard fix the export exposes: the vocabulary guard reads every file at the crate root, and the export's Cargo.lock names a third-party package whose name contains a listed word; skip machine-written lock files (Cargo.lock, flake.lock) in the vocabulary scan, with a self-test and a contributing.md note.
2. scripts/export-tuikit.sh: refuse a shallow or dirty source; git subtree split --prefix=crates/tuikit into a scratch repository (fetch the split by commit, no host ref created); rewrite the manifest (workspace-inherited package fields, dependencies from [workspace.dependencies], [workspace.lints.*] into [lints.*], an empty [workspace] table so cargo stops at the export); copy and prune Cargo.lock; add rust-toolchain.toml, LICENSE, .gitignore, CI workflow, justfile, flake.nix from one checks list; commit them in the export.
3. Isolation checks in the script: cargo locate-project resolves the export's own manifest, the export's grandparent holds none of the host's top-level entries, no rustfmt/clippy/cargo config in an ancestor, the export's dependency graph equals the host's for the crate.
4. Rehearsal in the export: fmt, clippy (all features; features off lib and all targets; richtext lib), tests (all features; features off), docs (features off; all features) with -D warnings, with per-step timings. --self-test injects a run-time host-relative read and a compile-time include, asserts each fails at the expected step, restores. Scratch dir defaults to target/separation-rehearsal and is removed afterwards unless --keep.
5. justfile separation-rehearsal recipe; CI job in ci.yml (run always, full history, script in $RUNNER_TEMP, with --self-test); flake banner line.
6. ADR 0199 plus framework record 0071, index rows and marks; crates/tuikit/docs/separation.md (the real move step by step, framework terms) linked from README and contributing; documentation.rs PAGES entry; design.md sentence.
7. Run the rehearsal and the self-test, record timings and output; full checks; operator-review rows; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
First rehearsal (before any fix) failed in the export at `cargo test --all-features`: `testkit::source::tests::the_walk_lists_files_sorted_and_skips_build_and_hidden_directories` asserted that no component of an absolute path is `target` (the export sat under `target/separation-rehearsal`). After fixing that, the vocabulary guard failed on the export's `Cargo.lock` (lines naming `pin-project-lite`, from `tui-markdown` through `futures-util` and `tracing`). Fixed by skipping lock files by exact name (`LOCK_FILES`), with a self-test; the guard report's host-relative path to the contribution guide became crate-relative. fmt, the four clippy runs, features-off tests, and both doc builds passed on the first try; the doc-sync host checks skipped as designed.
Script: `scripts/export-tuikit.sh` (shellcheck 0.11 clean via a one-off `nix run nixpkgs#shellcheck`; not added to the dev shell). Refusals exercised by hand: scratch inside the crate, a scratch holding the repository, a non-ignored scratch inside it, a non-empty foreign directory (left untouched), an unknown option, two scratch arguments, and a dirty crate (no export made).
Recorded run from a fresh `git clone` of HEAD 21c103ce, `--self-test`, default scratch under the clone's `target/`: split 128 commits 38s, prune lock 1s (245 of 320 packages), isolation 0s (143 packages and features identical), fmt 1s, clippy every target every feature 36s, lib features off 13s, every target features off 21s, lib richtext 5s, tests every feature 1m 14s, tests features off 1m 03s, docs features off 2s, docs every feature 2s; leak 1 (`include_str!("../../../src/lib.rs")`) failed the every-feature lint ("couldn't read `tests/../../../src/lib.rs`"), leak 2 (`CARGO_MANIFEST_DIR/../../src/lib.rs`) failed the every-feature tests (`reads_a_file_from_above_the_crate`), each restored; total 4m 40s (289 s wall with the dev shell). Build output 2.5 GB; scratch and clone deleted afterwards.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed the separation rehearsal (ADR 0199, framework record 0071).

- `scripts/export-tuikit.sh [--self-test] [--keep] [--allow-dirty] [SCRATCH]`: `git subtree split --prefix=crates/tuikit HEAD` fetched by commit id into SCRATCH/tuikit (no ref created); manifest made standalone (inherited package fields, `[workspace.dependencies]` entries plus `optional`, `[workspace.lints.*]` as `[lints.*]`, an empty `[workspace]` so cargo stops at the export; other spellings fail closed); Cargo.lock copied and pruned (245 of 320 packages, no version the host lacks); root files written and committed: rust-toolchain.toml, LICENSE, .gitignore, CI workflow, justfile, minimal flake.nix, the last three from one `CHECKS` list. Isolation checks: cargo's workspace root is the export, the export's grandparent holds no host top-level entry, no rustfmt/clippy/cargo config above it, `cargo tree` packages and features identical, no whitespace errors. Checks: fmt; clippy -D warnings (every target every feature, lib and every target features off, lib richtext); tests every feature and features off (doctests included); docs -D warnings features off and every feature. Scratch defaults to `target/separation-rehearsal`, removed unless `--keep`.
- `--self-test`: an `include_str!("../../../src/lib.rs")` test fails the every-feature lint, a `CARGO_MANIFEST_DIR/../../src/lib.rs` read fails the every-feature tests; both resolve in the host layout; restored after each.
- CI: `separation-rehearsal` job in ci.yml on every push and PR (not path-filtered: the export reads root inputs too), full history, `--self-test` into `$RUNNER_TEMP`. `just separation-rehearsal` recipe; banner line.
- Exposed and fixed in the crate: the vocabulary guard read the export's Cargo.lock (`pin-project-lite`); lock files are now skipped by name (`LOCK_FILES`, with a self-test). A source-walk test checked absolute paths for `target`; it checks below the root now. The guard report named the host path of the contribution guide.
- Docs: `crates/tuikit/docs/separation.md` (what stands alone, the rehearsal, failures, the real move in 7 steps), linked from README and contributing.md ("Keeping the crate liftable"), held by documentation.rs; design.md; ADR 0150 and index marks.

Verification: from a fresh clone of 21c103ce, `--self-test` passed in 4m 40s (split 38s; clippy 36s+13s+21s+5s; tests 1m 14s and 1m 03s; docs 2s+2s; both leaks caught; 2.5 GB, deleted). Host: fmt, workspace clippy all features, `clippy -p tuikit --all-targets` features off, `just tuikit-features`, isolated suite 4849 passed 0 failed 1 ignored, nextest isolated, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `git diff --check`, shellcheck clean.

Regressions: none. Consistency wins: none. No visible TUI changes.
Operator review: CI job runs always with the self-test; lock-file skip; stale manifest comments kept; no root README or flake.lock written; proposed Pre-Main Invariant for CSP-764.
<!-- SECTION:FINAL_SUMMARY:END -->
