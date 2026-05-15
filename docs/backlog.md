# Conspectus Backlog

This file is the interim work tracker for Conspectus. Keep it concise,
reviewable, and aligned with `docs/design.md` and `docs/adr/`.

Backlog.md is the preferred next-phase tool once the project needs structured
CLI queries, dependency operations, or agent/MCP integration.

## Conventions

- Use short, stable IDs so tasks can be referenced from commits and PRs.
- Keep work grouped by phase, ordered by dependency where practical.
- Record blockers explicitly.
- Move completed work to the bottom of the relevant phase instead of deleting it
  when the history is useful.
- Promote significant design decisions into ADRs before implementation relies
  on them.

## Pre-Implementation Planning

- [x] `PLAN-001` Convert `docs/design.md` into implementation phases and
  milestone-level stories.
  - Blockers: none.
- [x] `PLAN-002` Identify the first vertical slice for the Rust crate and CLI.
  - Blockers: `PLAN-001`.
- [x] `PLAN-003` Define the fixture strategy for sparse graph and resolver
  tests.
  - Blockers: `PLAN-001`.
  - Outcome: see `docs/implementation/`; first vertical slice is JSON graph
    output with sparse graph and resolver fixtures.

## Phase 0: Project Foundation

Source plan: `docs/implementation/phase-00-project-foundation.md`.

- [x] `P0-001` Add the Rust package skeleton.
  - Scope: add `Cargo.toml`, `rust-toolchain.toml`, `src/lib.rs`, and
    `src/main.rs` for a Rust 2024 library-first CLI crate.
  - Tests: `cargo check` succeeds.
  - Manual checks: `cargo run -- --help`.
  - Blockers: `PLAN-001`, `PLAN-002`.
- [x] `P0-002` Add the thin CLI surface.
  - Scope: wire `clap` so `conspectus --help` and `conspectus --version`
    work without implementing graph discovery.
  - Tests: CLI smoke tests for `--help` and `--version`.
  - Manual checks: `cargo run -- --help`; `cargo run -- --version`.
  - Blockers: `P0-001`.
- [x] `P0-003` Add baseline module boundaries.
  - Scope: add minimal `model`, `resolve`, `discovery`, and `output` module
    boundaries aligned with ADR 0007, without committing a graph schema yet.
  - Tests: module-level compile coverage through `cargo check`.
  - Manual checks: inspect public module layout for ADR 0007 alignment.
  - Blockers: `P0-001`.
- [x] `P0-004` Add runtime and test dependencies.
  - Scope: add runtime dependencies `clap`, `serde`, `serde_json`, `toml`,
    `toml_edit`, `indexmap`, `anyhow`, and `thiserror`; add test
    dependencies `assert_cmd`, `predicates`, `tempfile`, `insta`, `rstest`,
    and `proptest`.
  - Tests: dependency graph resolves and `cargo test --all-targets
    --all-features` succeeds.
  - Manual checks: verify dependencies are grouped by runtime vs dev usage in
    `Cargo.toml`.
  - Blockers: `P0-001`.
- [x] `P0-005` Add local check automation.
  - Scope: add a `justfile` with targets for formatting, linting, tests,
    nextest, and whitespace diff checks.
  - Tests: `just check` runs the complete baseline check suite.
  - Manual checks: `just --list`.
  - Blockers: `P0-001`, `P0-004`.
- [x] `P0-006` Add project-foundation smoke tests.
  - Scope: add CLI integration tests covering `--help` and `--version`, and
    make them part of the baseline check flow.
  - Tests: `cargo test --all-targets --all-features`; `cargo nextest run
    --all-targets --all-features`.
  - Manual checks: run both CLI commands directly.
  - Blockers: `P0-002`, `P0-004`, `P0-005`.
- [x] `P0-007` Verify the foundation end state.
  - Scope: run the full Phase 00 manual and automated check set and record any
    follow-up tasks instead of expanding Phase 00 scope.
  - Tests: `just check`; `git diff --check`.
  - Manual checks: `nix develop`; `cargo run -- --help`.
  - Blockers: `P0-003`, `P0-005`, `P0-006`.
  - Outcome: `nix develop --command just check`, `cargo run -- --help`,
    `cargo run -- --version`, and `just --list` passed.

## Later

- [ ] Evaluate Backlog.md migration once task count, dependencies, or
  multi-agent coordination make manual tracking cumbersome.
