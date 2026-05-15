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
- [x] `P2-005` Add generic workspace inference.
  - Scope: infer generic workspace roots from configured roots or layout
    evidence and link participating repos/worktrees without fabricating
    workspaces for standalone repo-only cases.
  - Tests: fixture tests for multi-repo workspace roots, standalone repos, and
    worktrees outside any workspace.
  - Manual checks: inspect JSON for generic workspace fixtures and confirm
    workspace nodes appear only when there is workspace evidence.
  - Blockers: `P2-004`.
  - Outcome: inferred generic workspaces only for explicit scan roots with
    multiple immediate git repo children, linked those repos with convention
    evidence, and kept standalone or single-repo roots repo-only.
- [x] `P2-006` Read Atelier workspace metadata.
  - Scope: parse `atelier.toml` enough to emit Atelier workspace context,
    workspace repo membership evidence, and related source metadata without
    depending on Atelier command modules.
  - Tests: fixture tests for minimal, multi-repo, and malformed Atelier
    workspace metadata.
  - Manual checks: run from an Atelier workspace with no forks and inspect
    workspace, repo, worktree, and branch nodes.
  - Blockers: `P2-003`, `P2-005`.
  - Outcome: added a read-only `atelier.toml` subset parser and parent-walk
    workspace discovery that emits Atelier workspace nodes, discovered repo
    graph fragments, and strong-discovered workspace membership links without
    depending on Atelier command modules.
- [x] `P2-007` Read Atelier fork index metadata.
  - Scope: parse `.atelier/forks/index.toml` into provider-neutral fork records
    with source metadata for worktree, selected, research, and standalone
    fork-like contexts.
  - Tests: fixture tests for empty indexes, worktree-mode forks,
    selected-mode forks, research forks, standalone repo forks, and malformed
    fork entries.
  - Manual checks: confirm parsing remains read-only and does not write
    `.conspectus.toml` or provider metadata.
  - Blockers: `P2-006`.
  - Outcome: added read-only `.atelier/forks/index.toml` parsing with
    provider-neutral fork records for worktree, selected, research, standalone,
    parent, repo membership, and harness lineage metadata; missing indexes load
    as empty.
- [x] `P2-008` Map Atelier forks into graph nodes and context-effect links.
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
  - Outcome: emitted one `Fork` node per Atelier fork plus candidate links for
    workspace scope, repo scope, created worktrees, referenced worktrees,
    created or associated branches, fork roots as unresolved path evidence, and
    parent forks, with snapshot coverage for worktree, selected, research, and
    standalone contexts.
- [x] `P2-009` Wire local discovery into `graph --format json`.
  - Scope: replace empty graph discovery with local discovery orchestration for
    cwd/configured roots while preserving deterministic output and existing
    Phase 1 JSON shape.
  - Tests: CLI integration tests for plain repo, non-repo cwd, invalid scan
    roots, and deterministic output across repeated runs.
  - Manual checks: run `cargo run -- graph --format json` from a plain repo, a
    linked worktree, an Atelier workspace with no forks, and an Atelier
    workspace with worktree, selected, and research forks.
  - Blockers: `P2-004`, `P2-008`.
  - Outcome: wired `graph --format json` to local discovery from the current
    directory or explicit `--scan-root` values, preserving deterministic JSON
    output and adding CLI coverage for non-repo, plain repo, missing-root, and
    invalid-format cases.
- [x] `P2-010` Add representative local-discovery snapshots.
  - Scope: snapshot graph JSON for plain repo, linked worktree, generic
    workspace, Atelier workspace without forks, and Atelier workspace with
    worktree, selected, research, and standalone fork contexts.
  - Tests: `cargo test --all-targets --all-features`; `cargo nextest run
    --all-targets --all-features`.
  - Manual checks: review snapshots for stable ordering, readable provenance,
    and separation of candidate links from resolved relationships.
  - Blockers: `P2-003`, `P2-005`, `P2-008`, `P2-009`.
  - Outcome: added normalized temp-fixture snapshots for plain repo, linked
    worktree, generic workspace, Atelier workspace without forks, and Atelier
    workspace with worktree, selected, and research fork metadata.
- [x] `P2-011` Verify the Phase 2 end state.
  - Scope: run the full Phase 2 automated and manual check set and record any
    follow-up tasks instead of expanding Phase 2 scope.
  - Tests: `just check`.
  - Manual checks: run `cargo run -- graph --format json` from the Phase 2
    manual-check contexts and confirm discovery remains read-only.
  - Blockers: `P2-009`, `P2-010`.
  - Outcome: `nix develop --command just check` passed with 56 tests, and
    `nix develop --command cargo run -- graph --format json` from the
    Conspectus repo emitted git repo, worktree, branch, candidate link, and
    resolved relationship JSON without modifying workspace files.

## Phase 3: Agent And Mux Discovery

Source plan: `docs/implementation/phase-03-agent-mux-discovery.md`.

- [x] `P3-001` Define agent harness discovery boundaries.
  - Scope: add read-only harness discovery traits, source-state inputs, and
    graph-fragment outputs for `AgentSession` nodes without binding the public
    graph model to provider-private schemas.
  - Tests: unit tests for empty harness discovery, missing state directories,
    and deterministic fragment merging.
  - Manual checks: inspect module boundaries for ADR 0007 alignment and confirm
    harness discovery does not perform output rendering.
  - Blockers: `P2-011`.
  - Outcome: added a `discovery::harness` module with a `HarnessAdapter` trait
    and `HarnessDiscovery` provider; extended `DiscoveryContext` with per-harness
    state-root overrides; covered empty adapters, missing state roots, state-root
    passthrough, and deterministic fragment merging.
- [x] `P3-002` Add synthetic harness fixture support.
  - Scope: add test helpers for creating provider state directories and session
    records for `claude-code`, `opencode`, `codex`, and `aider` without reading
    the user's real harness state.
  - Tests: fixture self-checks for generated paths, timestamps, cwd/root
    fields, and malformed records.
  - Manual checks: verify fixtures live under temporary directories and do not
    depend on local home-directory state.
  - Blockers: `P3-001`.
  - Outcome: added a `discovery::harness::fixtures` module with a
    `HarnessFixture` builder and standalone writers for Codex, Claude Code,
    opencode, and aider state layouts plus a malformed-record helper, all rooted
    at a caller-supplied temp directory; covered paths, optional fields, cwd
    encoding, opencode time fields, aider marker files, and malformed records.
- [x] `P3-003` Discover supported agent sessions.
  - Scope: implement read-only adapters that emit Conspectus-native
    `AgentSession` nodes and source metadata for supported local state from
    `claude-code`, `opencode`, `codex`, and `aider`.
  - Tests: fixture-based adapter tests for discovered sessions, orphaned
    sessions, malformed records, missing optional fields, and stable node IDs.
  - Manual checks: run against local synthetic state roots and inspect session
    nodes for readable provider metadata.
  - Blockers: `P3-001`, `P3-002`.
  - Outcome: added `CodexAdapter`, `ClaudeCodeAdapter`, `OpenCodeAdapter`, and
    `AiderAdapter`; widened the `HarnessAdapter` trait to receive the full
    `DiscoveryContext` so the per-repo aider adapter can walk scan roots while
    state-root harnesses pull their root via `harness_state_root`. Covered
    discovered sessions, missing state directories, malformed records, missing
    optional fields, and stable ID reproducibility for each adapter.
- [x] `P3-004` Preserve fork session lineage evidence.
  - Scope: map native, approximate, unsupported, fresh, and not-yet-discovered
    lineage evidence from provider metadata into candidate links or unresolved
    endpoints without fabricating placeholder session nodes.
  - Tests: unit tests for each lineage capability and unresolved parent/child
    session evidence per ADR 0005.
  - Manual checks: inspect JSON for unresolved lineage evidence and confirm the
    evidence is preserved without fake nodes.
  - Blockers: `P2-008`, `P3-003`.
  - Outcome: extended `fork_records_fragment` to emit `ParentSession` and
    `ChildSession` candidate links with unresolved-endpoint evidence carrying
    `harness_key`, `native_id`, fork root path, and a `lineage_kind` of
    `native`/`approximate`/`unsupported`/`fresh` plus any
    `degraded_warning`; capability maps to confidence (Native=High,
    Approximate=Medium, Unsupported/Fresh=Low); fresh sessions without a
    `source_session` omit the parent link rather than inventing one, and no
    placeholder `AgentSession` nodes are emitted.
- [x] `P3-005` Add injectable tmux command execution.
  - Scope: introduce a small command-runner seam for tmux discovery so tests can
    use fake output and production discovery can call `tmux` read-only.
  - Tests: unit tests for unavailable tmux, command failures, invalid UTF-8 or
    malformed rows, and deterministic error diagnostics.
  - Manual checks: verify no tests require a real tmux server.
  - Blockers: `P3-001`.
  - Outcome: added `discovery::tmux` with a `TmuxRunner` trait, a `SystemTmux`
    implementation that invokes `tmux list-sessions -F`, and a `FakeTmux`
    test runner; outcomes are classified as `Sessions`, `Unavailable`
    (binary missing or no server), or `Failed` with a stable diagnostic
    string, and stdout is decoded lossily so invalid UTF-8 surfaces to the
    parser rather than failing the runner.
- [x] `P3-006` Discover tmux sessions.
  - Scope: parse `tmux list-sessions` format output into `MuxSession` nodes,
    including session name, activity metadata when available, and root/cwd path
    evidence.
  - Tests: fake-command tests for zero sessions, one session, multiple
    sessions, paths with spaces, missing root/cwd fields, and unavailable tmux.
  - Manual checks: create a temporary tmux session and inspect mux-session JSON.
  - Blockers: `P3-005`.
  - Outcome: added a tab-separated `TMUX_LIST_FORMAT`
    (`#{session_name}\t#{session_path}\t#{session_activity}\t#{session_created}`),
    a `parse_list_sessions` parser that yields rich `TmuxSessionRow` values
    (preserving activity/creation epochs and paths with spaces), and a
    `TmuxDiscovery` provider that emits one `MuxSession` node per row while
    surfacing `Available`/`Unavailable`/`Failed` status to callers that need
    diagnostics. All tests use `FakeTmux` so no real tmux server is required.
- [x] `P3-007` Generate session, workspace, fork, and mux candidate links.
  - Scope: emit candidate links for session cwd/root matches, fork
    associations, mux candidates, parent session evidence, child session
    evidence, and unresolved lineage endpoints while preserving all plausible
    mux links.
  - Tests: graph-fragment tests for orphan sessions, mux-only sessions,
    one-to-many mux candidates, fork-linked sessions, and unresolved lineage.
  - Manual checks: inspect JSON to confirm ambiguous mux evidence remains in
    `candidate_links`.
  - Blockers: `P3-004`, `P3-006`.
  - Outcome: added a `discovery::cross_link::infer` post-merge pass that derives
    `AgentSession`→`MuxSession` `LinkedToMux` candidates (StrongDiscovered for
    exact cwd matches, Discovered for prefix matches) and
    `AgentSession`→`Fork` `AssociatedWith` candidates whenever a session cwd
    sits at or below an atelier `RootedAtPath` fork root; every plausible mux
    match is preserved and atelier-emitted `ParentSession`/`ChildSession`
    unresolved lineage links pass through untouched.
- [x] `P3-008` Implement session-to-mux resolver scoring.
  - Scope: apply ADR 0006 scoring for session-to-mux candidates: local
    declared, global declared, strong process or provider evidence, exact
    cwd/root match, naming convention, then recency or activity correlation.
  - Tests: table-driven resolver tests for each scoring tier, ties, ambiguity
    diagnostics, ignored candidates, and overridden candidates.
  - Manual checks: inspect resolved relationships for one-to-many mux scenarios
    and confirm lower-ranked candidates remain visible.
  - Blockers: `P3-007`.
  - Outcome: extended `MuxSessionNode` with optional `activity_epoch` and
    `created_epoch`, forwarded the activity through `cross_link::infer`
    onto `LinkedToMux` candidate metadata, and added a session-mux-specific
    resolver comparator that ranks declared > strong > exact-cwd >
    naming-convention > cached and breaks remaining ties by activity
    recency. Ignored and overridden candidates continue to be skipped and
    every losing candidate is recorded as a competing link plus a
    `Conflict` diagnostic.
- [x] `P3-009` Wire agent and tmux discovery into local graph discovery.
  - Scope: register the harness and tmux providers in local discovery so
    `conspectus graph --format json` emits repo, workspace, fork, session, and
    mux evidence from cwd/configured roots and supported local state.
  - Tests: CLI tests for deterministic graph output with fake harness and tmux
    discovery, unavailable tmux, and orphan sessions.
  - Manual checks: run `cargo run -- graph --format json` with a tmux smoke
    session and confirm useful output when sessions remain unlinked.
  - Blockers: `P3-003`, `P3-006`, `P3-008`.
  - Outcome: added a `LocalDiscoveryConfig` (harness state roots + optional
    tmux runner) and a `discover_local_with` entry point. The default
    `discover_local_at_roots` builds the config from the environment
    (`CONSPECTUS_CODEX_STATE` / `_CLAUDE_CODE_STATE` / `_OPENCODE_STATE`
    overrides, otherwise `$HOME`-relative paths, plus `CONSPECTUS_DISABLE_TMUX`
    to skip the tmux provider). Discovery now also calls
    `cross_link::infer` after merging fragments so session↔mux and
    session↔fork candidates appear automatically. CLI integration tests run
    with an isolated `$HOME` and `CONSPECTUS_DISABLE_TMUX=1`, and library
    tests exercise the full chain with `FakeTmux` plus fixture state.
- [x] `P3-010` Add representative agent and mux JSON snapshots.
  - Scope: snapshot graph JSON for orphan sessions, mux-only sessions,
    one-to-many mux candidates, fork-linked sessions, and unresolved session
    lineage evidence.
  - Tests: `cargo test --all-targets --all-features`; `cargo nextest run
    --all-targets --all-features`.
  - Manual checks: review snapshots for stable ordering, readable provenance,
    preserved ambiguity, and no placeholder session nodes.
  - Blockers: `P3-007`, `P3-008`, `P3-009`.
  - Outcome: added `tests/harness_mux_snapshots.rs` with six end-to-end
    snapshots driven by `discover_local_with` + `FakeTmux` + codex fixture
    state covering orphan harness sessions, mux-only output, unavailable
    tmux, exact-cwd session↔mux resolution, one-to-many mux candidates
    (resolver picks the most recently active session per ADR 0006), and a
    fork-associated session whose atelier harness entry stays as
    unresolved parent/child lineage. All temp paths are normalized to
    `/fixture` for stable ordering and no placeholder session nodes are
    emitted.
- [x] `P3-011` Verify the Phase 3 end state.
  - Scope: run the full Phase 3 automated and manual check set and record any
    follow-up tasks instead of expanding Phase 3 scope.
  - Tests: `just check`.
  - Manual checks: run the tmux smoke commands from the Phase 3 plan and run
    against real local harness state if available.
  - Blockers: `P3-009`, `P3-010`.
  - Outcome: `nix develop --command just check` passed with 124 tests. The
    tmux smoke test (`tmux new-session -d -s conspectus-smoke -c "$PWD"` +
    `cargo run -- graph --format json`) emitted one repo/worktree/branch,
    three mux sessions (including the smoke session at the conspectus repo
    cwd) and 16 agent sessions from the real `~/.codex`, `~/.claude`, and
    `~/.local/share/opencode` state. Discovery remained read-only.
  - Follow-up: real codex/claude/opencode state did not populate
    `agent_session.cwd`, so `cross_link::infer` never matched the smoke
    session against the live harness data. The Phase 3 adapters parse the
    synthetic fixture shapes; aligning them with the actual production
    JSONL/info.json layouts (and propagating cwd plus activity epochs)
    belongs in a Phase 4-or-later task rather than expanding Phase 3.

## Phase 3 Follow-Ups

- [ ] `P3-FU-001` Align harness adapter parsers with real provider state.
  - Scope: extend the Codex, Claude Code, and opencode adapters so the cwd
    and any activity/recency timestamps from real local state populate
    `AgentSessionNode.cwd` (and link metadata where applicable). The Phase 3
    `cross_link::infer` pass already correlates sessions and mux sessions on
    matching cwds, but real harness JSONL/info.json layouts left
    `agent_session.cwd` empty during the Phase 3 smoke test.
  - Blockers: none.

## Later

- [ ] Evaluate Backlog.md migration once task count, dependencies, or
  multi-agent coordination make manual tracking cumbersome.
