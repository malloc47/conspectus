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

## Phase 1: Core Graph JSON

Source plan: `docs/implementation/phase-01-core-graph-json.md`.

- [x] `P1-001` Define graph node identity types.
  - Scope: implement structured node IDs from ADR 0001 for `Repo`,
    `Worktree`, `Workspace`, `AgentSession`, `MuxSession`, `Branch`, `Fork`,
    and `ForgePr`.
  - Tests: unit tests for ID construction, display/debug behavior, serde round
    trips, and deterministic ordering.
  - Manual checks: inspect JSON snippets from unit fixtures for stable ID
    shape.
  - Blockers: `P0-007`.
- [x] `P1-002` Define typed node models.
  - Scope: add typed node structs/enums for the Phase 1 graph without
    provider-specific discovery behavior.
  - Tests: unit tests for serde round trips and sparse node serialization.
  - Manual checks: inspect representative serialized orphan session,
    mux-only, and repo-only nodes.
  - Blockers: `P1-001`.
- [x] `P1-003` Define GraphLink evidence types.
  - Scope: implement `GraphLink`, relation kinds, provenance, confidence,
    freshness, source metadata, unresolved endpoint evidence, and ignored or
    overridden state.
  - Tests: unit tests for relation-kind serialization, round trips, unresolved
    endpoints, ignored links, and overridden links.
  - Manual checks: inspect serialized candidate links for readable relation and
    provenance names.
  - Blockers: `P1-001`.
- [x] `P1-004` Define graph snapshot JSON output.
  - Scope: add the deterministic top-level graph document with `nodes`,
    `candidate_links`, `resolved_relationships`, and `diagnostics`.
  - Tests: snapshot tests for empty graph JSON and sparse graph fixtures.
  - Manual checks: confirm key ordering and separation between candidate links
    and resolved relationships.
  - Blockers: `P1-002`, `P1-003`.
- [x] `P1-005` Add fixture builders for sparse graph scenarios.
  - Scope: add internal test helpers for orphan sessions, mux-only rows,
    repo-only rows, unresolved lineage evidence, conflicts, and mux
    candidates.
  - Tests: fixture self-checks through JSON snapshot coverage.
  - Manual checks: verify fixtures are internal test helpers, not public API.
  - Blockers: `P1-002`, `P1-003`.
- [x] `P1-006` Implement the resolver skeleton.
  - Scope: accept GraphLink candidates and emit typed resolved relationships
    without deleting or mutating lower-priority evidence.
  - Tests: table-driven resolver tests for sparse links, no-op empty graphs,
    unresolved lineage evidence, and conflict preservation.
  - Manual checks: inspect resolver output for a sparse fixture and confirm
    candidate evidence remains present.
  - Blockers: `P1-003`, `P1-005`.
- [x] `P1-007` Implement resolver precedence rules.
  - Scope: apply default precedence: local declared, global declared, strong
    discovered evidence, convention, then cached evidence.
  - Tests: table-driven tests for declared-over-discovered precedence,
    discovered-over-cached precedence, ignored candidates, overridden
    candidates, and mux candidate precedence.
  - Manual checks: inspect diagnostic output for conflicts and selected
    relationships.
  - Blockers: `P1-006`.
- [x] `P1-008` Add `conspectus graph --format json`.
  - Scope: add the CLI command that emits the Phase 1 graph document; the
    command may produce an empty graph or fixture-backed graph, but not local
    discovery.
  - Tests: CLI integration tests for `graph --format json`, invalid formats,
    and deterministic output.
  - Manual checks: `cargo run -- graph --format json`.
  - Blockers: `P1-004`, `P1-006`.
- [x] `P1-009` Add representative graph JSON snapshots.
  - Scope: snapshot empty graph JSON and sparse fixtures covering orphan
    session, mux-only, repo-only, unresolved lineage, conflicts, and mux
    candidates.
  - Tests: `cargo test --all-targets --all-features`; `cargo nextest run
    --all-targets --all-features`.
  - Manual checks: review snapshots for stable ordering and public shape.
  - Blockers: `P1-004`, `P1-005`, `P1-007`, `P1-008`.
- [x] `P1-010` Verify the Phase 1 end state.
  - Scope: run the full Phase 1 automated and manual check set and record any
    follow-up tasks instead of expanding Phase 1 scope.
  - Tests: `just check`.
  - Manual checks: `cargo run -- graph --format json` and inspect that output
    distinguishes candidate links from resolved relationships.
  - Blockers: `P1-007`, `P1-008`, `P1-009`.
  - Outcome: `nix develop --command cargo fmt --all -- --check`,
    `nix develop --command cargo clippy --all-targets --all-features -- -D warnings`,
    `nix develop --command cargo test --all-targets --all-features`,
    `nix develop --command cargo nextest run --all-targets --all-features`,
    `git diff --check`, and `nix develop --command cargo run -- graph --format json`
    passed.

## Phase 2: Local Discovery

Source plan: `docs/implementation/phase-02-local-discovery.md`.

- [x] `P2-001` Define local discovery orchestration boundaries.
  - Scope: add discovery traits and a local discovery coordinator that can
    collect provider graph fragments, merge them into a `GraphSnapshot`, and
    leave candidate-link resolution to the existing resolver.
  - Tests: unit tests for merging empty and single-provider graph fragments
    without dropping nodes or candidate links.
  - Manual checks: inspect module boundaries for ADR 0007 alignment and confirm
    discovery does not perform output rendering.
  - Blockers: `P1-010`.
  - Outcome: added provider, context, graph-fragment, and local coordinator
    boundaries; discovery merges fragments into an unresolved graph snapshot and
    leaves resolution/output to existing modules.
- [x] `P2-002` Add read-only git command probes.
  - Scope: shell out to `git` for repo common dir, worktree root, current
    branch/refname, remotes, upstream, and per-worktree metadata when available.
  - Tests: integration tests using temporary git repos, detached HEADs, branch
    upstreams, and linked worktrees.
  - Manual checks: run probes from a plain repo and linked worktree and verify
    no files are modified.
  - Blockers: `P2-001`.
  - Outcome: added read-only git probes for common dir, worktree root, git dir,
    branch ref, upstream, and remotes with temp-repo coverage for plain,
    detached, upstream, and linked-worktree cases.
- [x] `P2-003` Map git probes into graph nodes and candidate links.
  - Scope: emit `Repo`, `Worktree`, and `Branch` nodes plus links for repo
    membership and checked-out branch evidence from git probe results.
  - Tests: JSON snapshot tests for a plain repo, a detached worktree, and a
    linked worktree fixture.
  - Manual checks: run `cargo run -- graph --format json` from a plain git repo
    and inspect repo/worktree/branch identity shape.
  - Blockers: `P2-002`.
  - Outcome: mapped git probe results into `Repo`, `Worktree`, and `Branch`
    nodes with strong-discovered candidate links for repo membership and checked
    out branches, plus fixed-path JSON snapshots for plain, detached, and linked
    worktree cases.
- [x] `P2-004` Add cwd and configured scan-root discovery inputs.
  - Scope: discover from the current working directory and from explicitly
    configured scan roots without recursively walking `$HOME` by default.
  - Tests: unit tests for scan-root normalization, duplicate-root handling, and
    missing/non-git roots.
  - Manual checks: verify running outside a git repo still returns a valid
    sparse graph document.
  - Blockers: `P2-001`, `P2-003`.
  - Outcome: added current-directory and explicit scan-root context builders,
    canonicalization and deduplication for existing roots, missing-root errors,
    and local discovery over non-git roots without recursive scanning.
- [ ] `P2-005` Add generic workspace inference.
  - Scope: infer generic workspace roots from configured roots or layout
    evidence and link participating repos/worktrees without fabricating
    workspaces for standalone repo-only cases.
  - Tests: fixture tests for multi-repo workspace roots, standalone repos, and
    worktrees outside any workspace.
  - Manual checks: inspect JSON for generic workspace fixtures and confirm
    workspace nodes appear only when there is workspace evidence.
  - Blockers: `P2-004`.
- [ ] `P2-006` Read Atelier workspace metadata.
  - Scope: parse `atelier.toml` enough to emit Atelier workspace context,
    workspace repo membership evidence, and related source metadata without
    depending on Atelier command modules.
  - Tests: fixture tests for minimal, multi-repo, and malformed Atelier
    workspace metadata.
  - Manual checks: run from an Atelier workspace with no forks and inspect
    workspace, repo, worktree, and branch nodes.
  - Blockers: `P2-003`, `P2-005`.
- [ ] `P2-007` Read Atelier fork index metadata.
  - Scope: parse `.atelier/forks/index.toml` into provider-neutral fork records
    with source metadata for worktree, selected, research, and standalone
    fork-like contexts.
  - Tests: fixture tests for empty indexes, worktree-mode forks,
    selected-mode forks, research forks, standalone repo forks, and malformed
    fork entries.
  - Manual checks: confirm parsing remains read-only and does not write
    `.conspectus.toml` or provider metadata.
  - Blockers: `P2-006`.
- [ ] `P2-008` Map Atelier forks into graph nodes and context-effect links.
  - Scope: emit one polymorphic `Fork` node per provider fork and candidate
    links for `forks_workspace`, `forks_repo`, `created_worktree`,
    `referenced_worktree`, `created_branch`, `associated_branch`,
    `rooted_at_path`, and `parent_fork` where evidence exists.
  - Tests: resolver and snapshot tests for created vs referenced worktrees,
    research forks, selected forks, standalone repo forks, parent forks, and
    associated branch links.
  - Manual checks: inspect graph JSON from Atelier fork fixtures and confirm no
    fake workspace nodes are fabricated for standalone repo contexts.
  - Blockers: `P2-007`.
- [ ] `P2-009` Wire local discovery into `graph --format json`.
  - Scope: replace empty graph discovery with local discovery orchestration for
    cwd/configured roots while preserving deterministic output and existing
    Phase 1 JSON shape.
  - Tests: CLI integration tests for plain repo, non-repo cwd, invalid scan
    roots, and deterministic output across repeated runs.
  - Manual checks: run `cargo run -- graph --format json` from a plain repo, a
    linked worktree, an Atelier workspace with no forks, and an Atelier
    workspace with worktree, selected, and research forks.
  - Blockers: `P2-004`, `P2-008`.
- [ ] `P2-010` Add representative local-discovery snapshots.
  - Scope: snapshot graph JSON for plain repo, linked worktree, generic
    workspace, Atelier workspace without forks, and Atelier workspace with
    worktree, selected, research, and standalone fork contexts.
  - Tests: `cargo test --all-targets --all-features`; `cargo nextest run
    --all-targets --all-features`.
  - Manual checks: review snapshots for stable ordering, readable provenance,
    and separation of candidate links from resolved relationships.
  - Blockers: `P2-003`, `P2-005`, `P2-008`, `P2-009`.
- [ ] `P2-011` Verify the Phase 2 end state.
  - Scope: run the full Phase 2 automated and manual check set and record any
    follow-up tasks instead of expanding Phase 2 scope.
  - Tests: `just check`.
  - Manual checks: run `cargo run -- graph --format json` from the Phase 2
    manual-check contexts and confirm discovery remains read-only.
  - Blockers: `P2-009`, `P2-010`.

## Later

- [ ] Evaluate Backlog.md migration once task count, dependencies, or
  multi-agent coordination make manual tracking cumbersome.
