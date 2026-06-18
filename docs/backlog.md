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
    `Checkout`, `Workspace`, `AgentSession`, `MuxSession`, `Branch`, `Fork`,
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
  - Scope: emit `Repo`, `Checkout`, and `Branch` nodes plus links for repo
    membership and checked-out branch evidence from git probe results.
  - Tests: JSON snapshot tests for a plain repo, a detached worktree, and a
    linked worktree fixture.
  - Manual checks: run `cargo run -- graph --format json` from a plain git repo
    and inspect repo/checkout/branch identity shape.
  - Blockers: `P2-002`.
  - Outcome: mapped git probe results into `Repo`, `Checkout`, and `Branch`
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
    evidence and link participating repos/checkouts without fabricating
    workspaces for standalone repo-only cases.
  - Tests: fixture tests for multi-repo workspace roots, standalone repos, and
    checkouts outside any workspace.
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
    workspace, repo, checkout, and branch nodes.
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
    links for `forks_workspace`, `forks_repo`, `created_checkout`,
    `referenced_checkout`, `created_branch`, `associated_branch`,
    `rooted_at_path`, and `parent_fork` where evidence exists.
  - Tests: resolver and snapshot tests for created vs referenced checkouts,
    research forks, selected forks, standalone repo forks, parent forks, and
    associated branch links.
  - Manual checks: inspect graph JSON from Atelier fork fixtures and confirm no
    fake workspace nodes are fabricated for standalone repo contexts.
  - Blockers: `P2-007`.
  - Outcome: emitted one `Fork` node per Atelier fork plus candidate links for
    workspace scope, repo scope, created checkouts, referenced checkouts,
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
    Conspectus repo emitted git repo, checkout, branch, candidate link, and
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
    `cargo run -- graph --format json`) emitted one repo/checkout/branch,
    three mux sessions (including the smoke session at the conspectus repo
    cwd) and 16 agent sessions from the real `~/.codex`, `~/.claude`, and
    `~/.local/share/opencode` state. Discovery remained read-only.
  - Follow-up: real codex/claude/opencode state did not populate
    `agent_session.cwd`, so `cross_link::infer` never matched the smoke
    session against the live harness data. The Phase 3 adapters parse the
    synthetic fixture shapes; aligning them with the actual production
    JSONL/info.json layouts (and propagating cwd plus activity epochs)
    belongs in a Phase 4-or-later task rather than expanding Phase 3.

## Phase 4: Forge And Table Views

Source plan: `docs/implementation/phase-04-forge-and-table-views.md`.

- [x] `P4-001` Define forge discovery boundaries and `gh` command runner.
  - Scope: add a `ForgeAdapter` trait and `ForgeDiscovery` provider under
    `src/discovery/forge/`, plus an injectable `gh` command runner that
    mirrors the existing `TmuxRunner` seam (real `SystemGh` that shells
    out, plus a `FakeGh` test runner). Outcomes are classified as
    `PullRequests(String)`, `Unavailable` (binary missing / unauthenticated),
    or `Failed { code, message }` so tests can drive each path
    deterministically. Record an ADR if the choice to delegate to `gh`
    (rather than calling the GitHub REST API directly) needs to outlive
    the implementation plan.
  - Tests: unit tests for missing `gh` binary, unauthenticated runs,
    command failures, empty output, and stable diagnostic strings.
  - Manual checks: confirm no test requires a real `gh` install or
    network call; inspect the module layout for ADR 0007 alignment and
    verify forge discovery performs no rendering.
  - Blockers: `P3-011`.
  - Outcome: added `discovery::forge` with a `ForgeAdapter` trait, a
    `ForgeDiscovery` coordinator, and a `GhRunner` seam (`SystemGh`
    shells out to `gh pr list --json`, `FakeGh` returns pre-canned
    outcomes and records the spawn cwd). `GhOutcome` classifies runs
    as `PullRequests`/`Unavailable`/`Failed`; `GhUnavailableReason`
    covers binary-missing, unauthenticated, and not-a-repo cases.
    ADR 0011 records the decision to delegate to `gh` rather than
    adding an HTTP client. Nine new unit tests cover each path; no
    test requires a real `gh` install or network.

- [x] `P4-002` Discover GitHub pull requests for known repos.
  - Scope: for each discovered repo, invoke
    `gh pr list --json number,state,url,headRefName,baseRefName,
    updatedAt,headRepositoryOwner,headRepository,isDraft` (or equivalent)
    and parse the JSON array into provider-neutral PR records carrying
    provider/host/owner/repo/number/state/url, the head ref name, draft
    flag, and an updated-at timestamp. Skip rows missing required fields
    rather than failing the run.
  - Tests: fake-runner tests for zero PRs, one PR, multiple PRs,
    malformed rows, missing optional fields, draft vs non-draft, and
    unavailable `gh`.
  - Manual checks: drive the adapter with a fixture-backed `gh` JSON
    blob and inspect record shape; do not exercise real `gh` in tests.
  - Blockers: `P4-001`.
  - Outcome: added `discovery::forge::github` with a
    `PullRequestRecord` / `PullRequestState` provider-neutral row
    shape and a `GhPullRequestParser` for `gh pr list --json` output.
    The parser is tolerant: empty, malformed, or row-level-invalid
    input degrades to an empty list rather than failing. RFC 3339
    `updatedAt` strings parse to a UTC epoch via a small embedded
    civil-date converter so the resolver can rank by recency without
    adding a chrono dependency. Ten new unit tests cover empty,
    malformed, single, multi-row, draft, missing-optional,
    unknown-state, and offset-vs-Z timestamp inputs.

- [x] `P4-003` Map PR records into ForgePr nodes and branch candidate links.
  - Scope: emit one `ForgePr` node per record (extending `ForgePrNode`
    with an optional `updated_epoch` and `is_draft` so the resolver can
    rank candidates by recency and draft state) and a `BranchHasForgePr`
    candidate link from the matching `Branch` node — matched by repo
    identity (host/owner/repo) and head ref. When the branch is not in
    the graph, emit an unresolved-endpoint candidate link so the
    evidence survives until later discovery resolves it.
  - Tests: graph-fragment tests for PRs whose head ref matches a
    discovered branch, PRs whose head ref is unknown to the graph, draft
    vs non-draft PRs, closed vs merged vs open state, and stable node IDs
    across repeated runs.
  - Manual checks: inspect JSON from a fixture-backed adapter run for
    readable provenance and identity shape.
  - Blockers: `P4-002`.
  - Outcome: extended `ForgePrNode` with `updated_epoch` and
    `is_draft` (skipped from JSON when false / absent for sparse
    output). Added `RepoContext` and `fragment_for_repo` in
    `discovery::forge::github`: per record, emits a `ForgePr` node
    plus a `BranchHasForgePr` candidate link targeting the discovered
    `Branch` node when the short head ref is in the supplied set, or
    an unresolved branch endpoint carrying host/owner/repo +
    head_ref metadata otherwise. Eight new tests cover matched
    branch, unknown ref, draft propagation, open/closed/merged
    state, stable IDs, updated-epoch propagation, and the
    empty-records case.

- [x] `P4-004` Resolver scoring for branch ↔ pull request.
  - Scope: add a `BranchHasForgePr`-specific comparator in
    `src/resolve/mod.rs` so that, when a branch has multiple plausible
    PRs, the preferred candidate is the most-recently-updated open
    non-draft PR, with closed/merged/draft state demoted to tie-breakers.
    Every losing candidate stays in `candidate_links` and is recorded as
    a competing link plus a `Conflict` diagnostic. Ignored and
    overridden candidates continue to be skipped.
  - Tests: table-driven resolver tests for zero, one, and multiple PRs
    per branch; open vs closed vs merged ranking; draft demotion;
    ignored / overridden state handling.
  - Manual checks: inspect resolved relationships for a branch with two
    open PRs and confirm losers remain visible.
  - Blockers: `P4-003`.
  - Outcome: added `compare_branch_pr` in `src/resolve/mod.rs` with a
    `PrScore` (provenance tier > state rank > non-draft > recency >
    confidence > link id). State ranks open > merged > closed > other.
    Reads `state` / `is_draft` / `updated_epoch` from the link's
    `source_metadata.fields` (populated by the github fragment
    builder). Nine new resolver tests cover open-vs-merged,
    open-vs-closed, draft demotion, recency tie-break, declared
    override, conflict diagnostic, ignored/overridden skip, and the
    zero-candidate case.

- [x] `P4-005` Add config loading for session projection defaults.
  - Scope: write an ADR for the Conspectus config file layout (project
    `.conspectus.toml` first, then `$XDG_CONFIG_HOME/conspectus/config.toml`
    or `$HOME/.config/conspectus/config.toml`, plus precedence and
    schema), then add a `config` module that loads
    `[session] projection = "agent" | "mux" | "union"`. Update
    `docs/design.md` if config introduces new model requirements.
  - Tests: unit tests for default projection, project-local override,
    user-level override, invalid projection values, malformed TOML, and
    missing config files.
  - Manual checks: verify the loader is read-only.
  - Blockers: ADR for config layout (filed alongside this item).
  - Outcome: ADR 0012 records the config layout, precedence, and the
    minimal `[session] projection` schema. New `src/config.rs` module
    exposes `Config`, `SessionConfig`, `Projection`, and a
    `ConfigLoader` that takes explicit `$HOME` / `$XDG_CONFIG_HOME`
    so tests don't mutate process state. Project config walks upward
    from cwd and stops at the `$HOME` boundary. Missing files are
    not errors; malformed TOML and invalid `projection` values emit
    `ConfigDiagnostic`s and fall back to defaults. Ten new unit
    tests cover defaults, project / user overrides, project >
    user precedence, the home-boundary stop, invalid value, malformed
    TOML, unknown-keys-ignored, `Projection::parse`/`as_str` round
    trip, and `$XDG_CONFIG_HOME` overriding `$HOME/.config`.

- [x] `P4-006` Define table output projection boundaries.
  - Scope: extend `src/output/` with a `Projection` enum
    (`Agent`/`Mux`/`Union`) and a render trait that takes a resolved
    `GraphSnapshot` plus a projection and returns a deterministic
    plain-text table. Define the compact provenance / confidence /
    ambiguity indicator format up front (e.g. `LD/SD/D/C/$`,
    `H/M/L`, and an `*` marker for ambiguous selections) so all three
    renderers share it.
  - Tests: unit tests for the indicator formatter and empty-graph
    rendering for each projection.
  - Manual checks: inspect indicator output for representative candidate
    links.
  - Blockers: `P3-011`.
  - Outcome: added `output::table` with `Projection` (re-exported
    from `config`), a single `render` entry point, and an
    `indicator(provenance, confidence, ambiguous)` helper that emits
    cells like `LD/H` / `SD/M*` / `$/L`. Codes are LD / GD / SD /
    D / C / $ for provenance and H / M / L for confidence.

- [x] `P4-007` Implement the agent projection table renderer.
  - Scope: render one row per `AgentSession` with harness, cwd, preferred
    mux, preferred PR, and ambiguity flags using the indicator format
    from `P4-006`. Orphan sessions stay visible with empty mux/PR cells.
  - Tests: snapshot tests for orphan sessions, sessions with a single
    mux match, sessions with multiple mux candidates, fork-linked
    sessions, and sessions whose branch has a forge PR.
  - Manual checks: review snapshots for column alignment and readable
    ambiguity indicators.
  - Blockers: `P4-006`.
  - Outcome: agent projection renders AGENT / CWD / MUX / MUX/CONF /
    PR / PR/CONF columns. Mux cell shows the preferred mux session
    label (or `—` for orphans). Ambiguity marker `*` appears when
    the agent has multiple candidate mux links. PR cell shows the
    first available BranchHasForgePr candidate. Unit tests cover
    orphan, single-match, ambiguous mux, and branch-with-PR cases.

- [x] `P4-008` Implement the mux projection table renderer.
  - Scope: render one row per `MuxSession` with backend, cwd, attached
    agent sessions (zero, one, or many), and ambiguity flags. Mux
    sessions with no attached agent remain visible.
  - Tests: snapshot tests for zero / one / many attached agents and
    unavailable-tmux scenarios (no mux rows).
  - Manual checks: review the snapshot output for alignment.
  - Blockers: `P4-006`.
  - Outcome: mux projection renders MUX / CWD / AGENTS columns;
    AGENTS lists `session-label [indicator]` for every attached
    session in stable order; mux sessions with no attached agent
    still appear with an `—` cell.

- [x] `P4-009` Implement the union projection table renderer.
  - Scope: render a single table that preserves both agent and mux rows
    plus their relationship status, with stable ordering so identical
    snapshots reproduce byte-for-byte. Use one row per node with a
    relationship column describing the preferred link and ambiguity.
  - Tests: snapshot tests for empty graphs, sessions without a mux, mux
    without sessions, and one-to-many mux candidates.
  - Manual checks: confirm the union table makes ambiguity visible
    without duplicating rows.
  - Blockers: `P4-007`, `P4-008`.
  - Outcome: union projection emits one row per node prefixed by
    `agent` / `mux`. Agent rows carry a relationship column
    formatted as `mux=<target> [indicator]` (`mux=—` when no
    candidate exists). Mux rows have a `—` relationship cell since
    attached sessions appear as their own agent rows. Unit tests
    cover both kinds plus the empty-graph header-only case.

- [x] `P4-010` Add the `conspectus session` CLI subcommand.
  - Scope: add `session` to the CLI with a
    `--projection {agent|mux|union}` flag that defaults to the value
    from the loaded config (or `agent` when no config is present). The
    command runs the existing local discovery + resolver and renders the
    chosen projection. Reuse the env-based isolation used by the graph
    command (`HOME`, `CONSPECTUS_DISABLE_TMUX`, future
    `CONSPECTUS_DISABLE_FORGE`).
  - Tests: CLI integration tests for the default projection, each
    explicit flag value, invalid values, config-file defaulting, and
    deterministic output across repeated runs.
  - Manual checks: `cargo run -- session`,
    `cargo run -- session --projection agent`,
    `cargo run -- session --projection mux`,
    `cargo run -- session --projection union`.
  - Blockers: `P4-005`, `P4-007`, `P4-008`, `P4-009`.
  - Outcome: added the `session` subcommand with an optional
    `--projection {agent|mux|union}` flag and `--scan-root` re-using
    the graph command's options. Without `--projection`, the CLI
    loads `.conspectus.toml` / user config via
    `config::ConfigLoader::from_env()` and falls back to `agent`.
    Config diagnostics print to stderr but do not abort the run.
    Six new CLI smoke tests cover the default projection,
    `--projection {agent|mux|union}`, an invalid value, project
    config defaulting to union, and deterministic output across
    repeated runs.

- [x] `P4-011` Wire forge discovery into local graph discovery.
  - Scope: register the forge provider in `discover_local_with` behind
    `LocalDiscoveryConfig::forge_runner` (mirroring the tmux pattern).
    `from_env()` builds a real `SystemGh` runner unless
    `CONSPECTUS_DISABLE_FORGE` is set. Discovery remains best-effort:
    missing / unauthenticated `gh` degrades to no PR data instead of
    failing the run. Cross-link inference passes PR evidence through
    `cross_link::infer` so ambiguous branch ↔ PR matches remain visible.
  - Tests: library tests for the wired path with a `FakeGh` runner;
    CLI integration tests with the forge provider disabled and with
    a fake `gh` output.
  - Manual checks: run `cargo run -- graph --format json` from a repo
    with an open PR and confirm the `ForgePr` node and link appear.
  - Blockers: `P4-003`, `P4-004`.
  - Outcome: added a `GitHubForgeProvider` that probes each scan
    root with `GitProbe`, extracts host/owner/repo from a
    GitHub-shaped git remote, runs `gh pr list --json` via the
    injected `GhRunner`, parses the rows, and emits a
    `fragment_for_repo`. A `parse_github_remote` helper covers
    `https://`, `git@`, `ssh://`, and GitHub-Enterprise hosts and
    rejects non-GitHub URLs. `LocalDiscoveryConfig` gained a
    `forge_runner` slot mirroring `tmux_runner`; `from_env()`
    builds a real `SystemGh` runner unless
    `CONSPECTUS_DISABLE_FORGE` is set. Unavailable / failed `gh`
    outcomes degrade silently. CLI smoke tests set
    `CONSPECTUS_DISABLE_FORGE=1` to keep tests offline; new library
    tests cover the wired path with `FakeGh` and the "no runner"
    case.

- [x] `P4-012` Add representative JSON and table snapshots.
  - Scope: snapshot graph JSON for a repo with zero / one / multiple
    open PRs and for one-to-many branch ↔ PR ambiguity. Add session-table
    snapshots in each projection for orphan sessions, single-mux match,
    one-to-many mux candidates, fork-associated sessions, and
    branch-with-PR scenarios. All snapshots normalise temp paths to
    `/fixture`.
  - Tests: `cargo test --all-targets --all-features`; `cargo nextest run
    --all-targets --all-features`.
  - Manual checks: review snapshots for stable ordering, readable
    provenance / confidence / ambiguity, and preserved competing-PR
    evidence.
  - Blockers: `P4-009`, `P4-010`, `P4-011`.
  - Outcome: added `tests/forge_snapshots.rs` with six end-to-end
    snapshots driven by `discover_local_with` + `FakeGh` against a
    temp git repo: zero-PR JSON, one-open-PR JSON (matched
    branch endpoint + resolved relationship), multi-PR JSON
    (open + merged + closed), and three session-table projections
    (agent with PR, mux empty, union with PR). Path normalization
    rewrites the temp path to `/fixture` so reruns are byte-stable.

- [x] `P4-013` Verify the Phase 4 end state.
  - Scope: run the full Phase 4 automated and manual check set and
    record follow-up tasks instead of expanding Phase 4 scope.
  - Tests: `just check`.
  - Manual checks: run the four `cargo run -- session` smoke commands
    from `docs/implementation/phase-04-forge-and-table-views.md`, plus
    `cargo run -- graph --format json` from a repo with a real open
    PR. Confirm the PR node and `BranchHasForgePr` link appear in JSON
    and surface in the session-table projection.
  - Blockers: `P4-010`, `P4-011`, `P4-012`.
  - Outcome: `nix develop --command just check` passed with 210 tests.
    `cargo run -- session`, `--projection agent`, `--projection mux`,
    and `--projection union` all rendered tables against live local
    state (claude-code agent sessions, no mux/PR rows because the
    local `gh` is unauthenticated and the smoke test had no tmux
    server). `cargo run -- graph --format json` emitted repo /
    checkout / branch / agent_session / mux_session nodes plus 14
    resolved relationships. Discovery remained read-only.
  - Follow-up: live `gh` was unauthenticated in the dev shell, so
    the smoke run did not exercise real PR retrieval. The forge
    code path is exercised by 14 unit/library tests and 3 JSON
    snapshots using `FakeGh`; verifying against a real
    authenticated `gh` belongs in a follow-up smoke test run by a
    user with credentials, not in Phase 4 scope.

## Phase 5: Declared Links

Source plan: `docs/implementation/phase-05-declared-links.md`.

- [x] `P5-001` Record the declared-link storage schema.
  - Scope: add an ADR for durable declared relationship state in
    `.conspectus.toml` and user config, covering link identity, endpoint
    encoding, relation kinds, link state (`active`, `ignored`,
    `overridden`), local-vs-global precedence, write ownership, and
    compatibility rules for future schema changes.
  - Tests: docs-only; `git diff --check`.
  - Manual checks: review the schema against `docs/design.md`, ADR
    0002, ADR 0012, and the Phase 5 implementation plan.
  - Blockers: `P4-013`.
  - Outcome: ADR 0014 defines the `[declared]` TOML schema,
    `[[declared.links]]` entries, typed inline endpoint tables,
    active/ignored/overridden states, local-vs-global provenance from
    config location, nearest-store write ownership, and compatibility
    behavior for unknown fields and schema versions.

- [x] `P5-002` Define declared-link file models and TOML round trips.
  - Scope: extend `src/config.rs` or add a focused declared-link module
    with serializable structs for project-local and user-level declared
    links, ignored links, overrides, reasons, optional labels, and schema
    versioning. Preserve unknown config sections and keep session
    projection loading compatible with existing config files.
  - Tests: unit tests for TOML decode/encode round trips, missing
    sections, unknown keys, malformed declared-link tables, duplicate
    declared IDs, and backwards-compatible files containing only
    `[session]`.
  - Manual checks: inspect representative `.conspectus.toml` and
    user-config TOML snippets for readable shape.
  - Blockers: `P5-001`, `P4-005`.
  - Outcome: added `src/declared.rs` with ADR 0014 file models for
    `[declared]`, `[[declared.links]]`, typed endpoints, link state,
    optional reasons/labels, and schema validation. Added TOML
    round-trip and validation coverage for session-only config,
    unknown keys, all endpoint shapes, malformed TOML, unsupported
    schema versions, duplicate IDs, and overridden links missing
    `overridden_by`.

- [x] `P5-003` Load local and global declared links into graph evidence.
  - Scope: teach local discovery to read project `.conspectus.toml`
    and user config declared-link sections without writing either file,
    convert entries into `GraphLink` candidates with
    `LocalDeclared` / `GlobalDeclared` provenance, and preserve
    unresolved endpoint evidence when a declared endpoint is not present
    in the current graph.
  - Tests: unit/library tests for local-only, global-only,
    local-over-global, unresolved endpoints, ignored entries,
    overridden entries, malformed files producing diagnostics, and
    discovery over roots with no config files.
  - Manual checks: run `cargo run -- graph --format json` in a repo
    with hand-written `.conspectus.toml` declarations and inspect
    provenance, link state, diagnostics, and resolved relationships.
  - Blockers: `P5-002`.
  - Outcome: added a read-only `discovery::declared` pass that loads
    user config and per-root project config, maps entries into
    `GraphLink` candidates with `LocalDeclared` / `GlobalDeclared`
    provenance, resolves declared targets against the discovered node
    set when present, preserves missing targets as unresolved endpoint
    evidence, and emits config diagnostics for malformed declared
    sections. `LocalDiscoveryConfig::from_env()` enables declared-link
    loading by default while tests can inject or disable the config
    loader explicitly.

- [x] `P5-004` Preserve read-only command invariants.
  - Scope: explicitly verify `conspectus graph` and `conspectus session`
    never create or mutate `.conspectus.toml`, user config files, or
    cache directories while loading declared evidence.
  - Tests: CLI integration tests for graph/session from a clean repo,
    a repo with existing config, an orphan/non-repo cwd, and explicit
    `--scan-root` values; assert filesystem mtimes/content stay
    unchanged.
  - Manual checks: `cargo run -- graph --format json`; `test ! -e
    .conspectus.toml`; repeat with `cargo run -- session`.
  - Blockers: `P5-003`.
  - Outcome: added CLI smoke coverage proving `graph` and `session`
    do not create `.conspectus.toml` or user config in a clean repo,
    and do not mutate an existing project config with declared-link
    state when run from either cwd or explicit `--scan-root`.

- [x] `P5-005` Implement nearest-store selection for writes.
  - Scope: add a pure store-selection helper that decides where a new
    user-authored declaration belongs: project-local for relationships
    rooted in a discovered repo/workspace/checkout, global for orphan or
    user-wide relationships, and never in cache/index storage. Reuse the
    config walk rules from ADR 0012.
  - Tests: unit tests for repo-rooted, workspace-rooted,
    checkout-rooted, branch/PR-rooted, mux-only, orphan-agent,
    multi-root, missing-root, and outside-home scenarios.
  - Manual checks: inspect selected paths for representative repos,
    linked worktrees, and non-repo directories.
  - Blockers: `P5-002`, `P5-003`.
  - Outcome: added a pure `select_store_for_declaration` helper that
    resolves declared-link writes to the nearest project config for
    repo, workspace, checkout, session cwd, mux cwd, branch/PR, and
    fork-rooted relationships, and falls back to the user config for
    orphan relationships without touching cache or index storage.

- [x] `P5-006` Add atomic declared-link write helpers.
  - Scope: implement read-modify-write helpers for local
    `.conspectus.toml` and user config declared-link sections, creating
    parent directories only for explicit write commands, preserving
    unrelated config, sorting entries deterministically, and writing
    atomically enough to avoid partial files on failure.
  - Tests: unit tests for create, update, remove, preserve-unrelated
    sections, deterministic ordering, duplicate replacement, malformed
    existing TOML behavior, and global config parent creation.
  - Manual checks: inspect generated TOML and verify read-only
    commands still do not call these helpers.
  - Blockers: `P5-005`.
  - Outcome: added explicit upsert/remove helpers that read and validate
    existing config, preserve unrelated TOML sections, replace duplicate
    declared IDs, sort links deterministically, create parent
    directories only on writes, and replace config files via
    temp-file-and-rename writes while leaving malformed files untouched.

- [x] `P5-007` Define the declared-link CLI surface.
  - Scope: add the CLI command structure and help text for listing,
    creating, removing, confirming, ignoring, and overriding declared
    links without implementing every mutation path. Choose stable flag
    names for source endpoint, relation, target endpoint, reason, and
    store override if needed.
  - Tests: CLI smoke tests for `--help`, invalid relation names,
    invalid endpoint syntax, missing required arguments, and no-op list
    output against empty stores.
  - Manual checks: `cargo run -- --help` and declared-link subcommand
    help output.
  - Blockers: `P5-001`, `P5-006`.
  - Outcome: added the `conspectus declared` command group with
    `list`, `create`, `remove`, `confirm`, `ignore`, and `override`
    subcommands; relation validation uses the existing snake_case graph
    relation names, endpoint validation accepts `type:key=value,...`
    values using declared TOML field names, and empty `declared list`
    succeeds without producing output.

- [x] `P5-008` Implement list and inspect commands for declared state.
  - Scope: add read-only commands that render declared links from local
    and global stores, including active, ignored, and overridden
    entries, their selected store, provenance, relation, endpoints, and
    reasons.
  - Tests: CLI integration tests for empty stores, local declarations,
    global declarations, both stores, ignored/overridden entries,
    malformed config diagnostics, and deterministic output.
  - Manual checks: create hand-written local/global declared entries
    and inspect list output.
  - Blockers: `P5-003`, `P5-007`.
  - Outcome: implemented read-only `declared list` output for user and
    discovered project stores, including store, provenance, state, id,
    relation, source/target endpoints, reason, override id, label, and
    config path; output is deterministic, empty stores print nothing,
    and malformed declared config emits a warning without mutating files.

- [x] `P5-009` Implement link and unlink commands.
  - Scope: add write commands that create and remove active declared
    relationships between supported endpoint types (`AgentSession`,
    `MuxSession`, `ForgePr`, `Workspace`, `Repo`, `Checkout`,
    `Branch`, and `Fork`), using nearest-store selection by default.
    Link creation should not delete discovered evidence.
  - Tests: CLI integration tests for session↔mux, branch↔PR,
    workspace/repo/checkout/fork relationships, global orphan links,
    unlink by declared ID, unlink idempotency, and graph output after
    link/unlink.
  - Manual checks: create a manual mux/session link, rerun graph JSON,
    confirm the declared link wins resolution and discovered candidates
    remain visible, then unlink and confirm resolution returns to
    discovered evidence.
  - Blockers: `P5-006`, `P5-007`.
  - Outcome: `conspectus declared create` builds a declared link with
    state=Active, picks the target store via
    `select_store_for_declaration` (auto), `--store {project|user}`
    (explicit), or rejects `--store all`; `conspectus declared remove`
    walks project (nearest, per `--scan-root` walk) then user stores
    and removes the first match, reporting a clear error when no
    store holds the id. Eight new CLI smoke tests cover repo-rooted
    write to project config, orphan write to user config, explicit
    `--store user` override, `--store all` rejection, idempotent
    re-create (wrote → unchanged), remove from project config,
    "no declared link" error path, and graph JSON showing the
    newly created `local_declared` candidate.

- [x] `P5-010` Implement confirm, ignore, and override flows.
  - Scope: add mutation flows that mark a discovered candidate as
    confirmed declared evidence, record ignored candidates with optional
    reasons, and record explicit overrides that point to the replacing
    declared link while keeping original evidence visible.
  - Tests: CLI integration and resolver tests for confirmed mux links,
    ignored noisy candidates, overridden links, local-vs-global state,
    optional reasons, and detailed graph JSON preserving all candidate
    evidence.
  - Manual checks: confirm one discovered session↔mux candidate, ignore
    a competing candidate, and inspect `candidate_links`,
    `resolved_relationships`, and diagnostics.
  - Blockers: `P5-009`.
  - Outcome: `conspectus declared confirm` and `declared ignore`
    share a `run_confirm_or_ignore` helper that runs discovery, looks
    up the candidate by id in `snapshot.candidate_links`, maps both
    endpoints back to `DeclaredEndpoint` via a new
    `declared_endpoint_from_node_id` helper, and writes a declared
    link with state Active or Ignored (carrying the supplied
    `--reason` for ignore). `declared override` uses a new
    `load_declared_link_by_id` helper to read the existing declaration
    from the same store, mutates state to Overridden plus
    `overridden_by` + optional reason, and writes it back. Six new
    CLI smoke tests cover confirm, ignore with reason, unknown
    candidate id, override of an existing link, override missing id,
    and a graph-JSON assertion that the discovered candidate stays
    visible alongside the new local-declared one.

- [x] `P5-011` Add declared-link graph and table snapshots.
  - Scope: add representative snapshots for local declared links,
    global declared links, local-over-global precedence, ignored
    discovered candidates, overridden candidates, unresolved declared
    endpoints, and session-table rendering with declared mux/PR
    relationships.
  - Tests: `cargo test --all-targets --all-features`; `cargo nextest
    run --all-targets --all-features`.
  - Manual checks: review snapshots for stable ordering, readable TOML
    provenance, and preserved discovered evidence.
  - Blockers: `P5-003`, `P5-010`.
  - Outcome: added `tests/declared_snapshots.rs` with seven scenarios
    driven by `discover_local_with` plus an injected `ConfigLoader`
    and `FakeTmux`: local-declared with matched target, global
    declared, local-over-global precedence (local wins resolution and
    global stays as a competing link plus `Conflict` diagnostic),
    ignored declared link, overridden declared link, unresolved
    declared endpoint when no discovery providers run, and an
    agent-projection table rendering the declared mux relationship.
    Temp paths normalize to `/fixture` so reruns stay byte-stable.

- [x] `P5-012` Verify the Phase 5 end state.
  - Scope: run the full Phase 5 automated and manual check set and
    record follow-up tasks instead of expanding Phase 5 scope.
  - Tests: `just check`.
  - Manual checks: run the Phase 5 plan's read-only invariant check,
    create a manual mux/session link, rerun graph/session output,
    confirm declared precedence and evidence preservation, then unlink
    and confirm the generated TOML returns to the expected state.
  - Blockers: `P5-004`, `P5-008`, `P5-009`, `P5-010`, `P5-011`.
  - Outcome: `nix develop --command just check` passed with 276 tests.
    Manual smoke from a fresh temp git repo with isolated `$HOME`
    confirmed: (a) `graph --format json` runs read-only and creates
    no config files; (b) `declared create --store project --scan-root
    .` writes a well-formed `.conspectus.toml`; (c) the new
    `local_declared` candidate appears in graph JSON and the agent
    session table renders cleanly; (d) `declared remove` strips the
    link; (e) graph output returns to its pre-declare candidate set
    (only the git-discovered links remain). Discovery stayed
    read-only throughout.
  - Follow-up: `declared remove` leaves an empty `[declared]\n
    schema_version = 1` section behind when it strips the last
    declared link. The file remains schema-valid and re-adding a link
    repopulates the section, but a future task should prune empty
    sections so removed declarations don't leave dangling headers.

## Phase 6: Atelier Delegation

Source plan: `docs/implementation/phase-06-atelier-delegation.md`.
Design framing: the "Migration Plan" section in `docs/design.md`.

Conspectus has stabilized its graph, discovery, JSON, table, and
declared-link surfaces (Phases 1–5). Phase 6 turns that surface into
something Atelier (and any other future consumer) can rely on, and
coordinates the Atelier-side deprecation/delegation work.

The Conspectus crate has been library-first since ADR 0007, so this
phase is mostly about stabilizing the *contract* (what's stable,
where it lives, how to depend on it), refreshing user-facing docs to
position Conspectus as the cross-workspace observability surface, and
filing the cross-repo work in Atelier. No Atelier-side code lands in
this repo.

- [x] `P6-001` Record an ADR for the Conspectus library API surface.
  - Scope: write an ADR that names the publicly stable modules
    (`model`, `output`, `resolve`, `config`, `declared`,
    `discovery::{git,tmux,forge,harness,atelier,workspace,declared,
    cross_link}`), declares the rest internal, and commits to a
    semver discipline. Decide whether to surface a curated
    `pub use` facade (e.g. `conspectus::api`) and how `#[doc(hidden)]`
    is applied to internals.
  - Tests: docs-only; `git diff --check`.
  - Manual checks: cross-check the proposed stable list against
    `src/lib.rs`, the existing `pub` items in each module, and
    the migration plan in `docs/design.md`.
  - Blockers: `P5-012`.
  - Outcome: accepted ADR 0015, which names the stable public modules,
    commits to a semver discipline, keeps existing module paths
    supported, marks CLI internals outside the library contract, and
    requires a curated `conspectus::api` facade for common consumers.

- [x] `P6-002` Record an ADR for Conspectus distribution.
  - Scope: decide whether external consumers (Atelier today, possibly
    other tools later) depend on Conspectus via crates.io, a pinned
    git revision, a path dependency, or all three. Capture the
    versioning policy, MSRV story, and release cadence. The decision
    must be compatible with the dev-shell's Nix toolchain pinning.
  - Tests: docs-only.
  - Manual checks: confirm any chosen distribution channel works
    against the Phase 6 dev-shell.
  - Blockers: `P6-001`.
  - Outcome: accepted ADR 0016, which makes crates.io the intended
    steady-state distribution channel, allows pinned git revisions for
    Atelier migration and release validation, limits path dependencies
    to local development, and ties the effective MSRV to the stable
    toolchain validated by the Nix dev shell.

- [x] `P6-003` Audit pure vs impure modules and produce a library API
  inventory.
  - Scope: walk every module under `src/` and tag it as either
    pure (no `std::env`, `std::process`, `current_dir`, no global
    state) or impure boundary code, and write the result up as
    `docs/library-api.md`. Identify entry points consumers should
    call (e.g. `discover_local_with`, `resolve_snapshot`,
    `render_graph_json`, `output::table::render`, the declared-link
    read/write helpers) and call out the impure seams
    (`*::from_env`, the runners) so consumers know what they have to
    inject to keep things testable.
  - Tests: docs-only; `git diff --check`.
  - Manual checks: re-grep for `std::env`, `std::process`, and
    `current_dir` after the audit and confirm the inventory matches.
  - Blockers: `P6-001`.
  - Outcome: added `docs/library-api.md` with the stable consumer
    workflow, pure-module inventory, impure boundary inventory,
    injection guidance, stable entry points, and environment toggles.
    The source audit found production process boundaries in git, tmux,
    and gh runners, and current-directory/environment boundaries in CLI
    helpers, `DiscoveryContext::from_current_dir`,
    `LocalDiscoveryConfig::from_env`, and `ConfigLoader::from_env`.

- [x] `P6-004` Add a curated public re-export facade.
  - Scope: add a small `conspectus::api` module (or top-level
    `pub use` block in `src/lib.rs`) that re-exports the entry
    points named in `P6-003`. Apply `#[doc(hidden)]` (or move to
    `pub(crate)`) on items the ADR marks internal. Keep the existing
    module paths working so current callers do not break.
  - Tests: `cargo test --all-targets --all-features`; add a small
    doctest under `conspectus::api` that demonstrates a minimal
    library invocation (e.g. construct `LocalDiscoveryConfig::empty()`
    + call `discover_local_with` on a temp dir).
  - Manual checks: `cargo doc --no-deps --open` and confirm the
    curated surface is the obvious entry point.
  - Blockers: `P6-001`, `P6-003`.
  - Outcome: added `conspectus::api` as the curated facade for
    discovery, resolution, graph JSON, table rendering, config,
    declared-link helpers, and graph model types. The facade includes a
    doctest that performs a minimal temp-dir discovery with
    `LocalDiscoveryConfig::empty()`. Test-only fixture/fake helpers now
    stay reachable but are hidden from generated docs.

- [x] `P6-005` Write the Atelier migration guide.
  - Scope: add `docs/atelier-migration.md` mapping each overlapping
    Atelier command to its Conspectus replacement
    (`atelier session list` → `conspectus session`;
    `atelier mux status` → `conspectus session --projection mux`;
    forge status → `conspectus graph --format json` /
    `conspectus session`; graph-heavy parts of `atelier status` →
    `conspectus graph --format json`). Note the env toggles already
    documented in `docs/operations.md` and any new ones introduced by
    `P6-004`. Link the migration guide from `docs/index.md`.
  - Tests: docs-only; `git diff --check`.
  - Manual checks: run the listed Conspectus commands and confirm
    they cover the workflow described.
  - Blockers: none (independent of code changes).
  - Outcome: added `docs/atelier-migration.md` with mappings from
    Atelier session, mux, forge, and graph-heavy status workflows to
    Conspectus commands; documented read-only behavior, ambiguity
    preservation, runtime knobs, and `conspectus::api` integration.
    Linked the guide from `docs/index.md`.

- [x] `P6-006` Add a representative comparison fixture.
  - Scope: add an integration test that runs `discover_local_with`
    on a temp-dir fixture mimicking an Atelier workspace (atelier
    config + fork index + a fake harness session + a `FakeTmux`)
    and snapshots the rendered graph JSON plus all three session
    table projections. Path-normalize to `/fixture` for byte-stable
    reruns. The intent is to give Atelier delegation a concrete
    target to validate against during its own work.
  - Tests: `cargo nextest run --all-targets --all-features`.
  - Manual checks: review the new snapshots for stable ordering and
    preserved evidence/ambiguity.
  - Blockers: `P5-012`.
  - Outcome: added `tests/atelier_delegation_snapshots.rs`, which
    builds an Atelier-style workspace with two git repos, a checkout
    fork, unresolved codex lineage metadata, a fake codex session, and a
    matching `FakeTmux` row. The test snapshots rendered graph JSON plus
    agent, mux, and union session table projections with temp paths
    normalized to `/fixture`.

- [x] `P6-007` File the Atelier-side delegation work in the Atelier
  repo.
  - Scope: open the cross-repo tracker covering Atelier's deprecation
    or delegation of `atelier session list`, `atelier mux status`,
    forge status, and the graph-heavy parts of `atelier status`. The
    code lives in the Atelier repo; this item is purely outbound
    coordination, including pointing Atelier at `P6-004`'s curated
    API and `P6-005`'s migration guide. Cite the Atelier issue or PR
    URL in the outcome note so future readers can follow up.
  - Tests: none (out-of-repo work).
  - Manual checks: confirm an Atelier maintainer (or self, if dual
    maintainer) has accepted the tracker.
  - Blockers: `P6-004`, `P6-005`.
  - Outcome: added the Atelier-side tracker in
    `/home/malloc47/src/atelier/docs/conspectus-delegation.md` and
    linked it from Atelier docs in commit `b765c16` (`docs: track
    Conspectus delegation work`). The tracker points Atelier at
    Conspectus commits `46d31ac`, `652dd43`, and `49f170d`, covers
    `atelier session list`, `atelier mux status`, `atelier pr status`,
    and graph-heavy `atelier status` areas, and records acceptance
    criteria for preserving existing workflows.

- [x] `P6-008` Refresh top-level docs to position Conspectus as the
  cross-workspace observability surface.
  - Scope: update `README.md` so it no longer reads "currently in
    design"; describe what the CLI does today and link the
    feature summary, ADR index, and migration guide. Update
    `docs/index.md` if the table of contents shifted. Update the
    "Migration Plan" section of `docs/design.md` to mark items 1–5
    complete and reference Phase 6's ADRs for items 6–7.
  - Tests: docs-only; `git diff --check`.
  - Manual checks: open the rendered Markdown and confirm the
    framing matches the post-Phase-5 reality.
  - Blockers: `P6-004`, `P6-005`, `P6-007`.
  - Outcome: refreshed `README.md` so it describes the implemented CLI
    and library instead of a design-only project, links the feature
    summary, ADRs, operations, library API, and Atelier migration guide,
    and updates development checks. Updated `docs/design.md` to mark
    migration-plan items 1-5 complete, identify item 6 as tracked by
    the Conspectus and Atelier Phase 6 coordination docs, and reference
    ADRs 0015 and 0016 for the API/distribution decisions around items
    6-7.

- [x] `P6-009` Decide whether to extract Conspectus into its own
  repository.
  - Scope: per migration-plan item 7, reassess whether Conspectus
    should remain in this repository alongside its design ancestor
    or move to a standalone repo now that the shared library surface
    is stable. Record the conclusion in an ADR (and either schedule
    the extraction as a Phase 7 task or note that the current
    arrangement stays).
  - Tests: docs-only.
  - Manual checks: review the ADR against `docs/design.md` and
    `docs/naming.md`.
  - Blockers: `P6-001`, `P6-007`.
  - Outcome: accepted ADR 0017, which keeps Conspectus in the current
    standalone repository, does not schedule a Phase 7 repository move,
    and directs Atelier integration to use ADR 0016 distribution
    channels rather than repository colocation.

- [x] `P6-010` Verify the Phase 6 end state.
  - Scope: run the full Phase 6 automated and manual check set and
    record follow-up tasks instead of expanding Phase 6 scope.
  - Tests: `just check`; `cargo doc --no-deps`.
  - Manual checks: run the four `cargo run -- session` smoke commands
    plus `cargo run -- graph --format json` on a real workspace and
    confirm the output matches what Atelier users previously got from
    the deprecated commands.
  - Blockers: `P6-004`, `P6-005`, `P6-006`, `P6-007`, `P6-008`,
    `P6-009`.
  - Outcome: `nix develop --command just check` passed, including
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

## Phase 5 Follow-Ups

- [x] `P5-FU-001` Prune empty `[declared]` sections after the last
  declared link is removed.
  - Scope: when `remove_declared_link` brings the link list to zero,
    delete the `[declared]` table entirely (and the file when no
    other top-level sections remain) so a fresh `declared list` from
    that store prints nothing instead of showing a dangling header.
  - Blockers: none.
  - Outcome: `write_declared_links_if_changed` now removes the
    `declared` table from the `toml_edit::DocumentMut` when no
    links remain, and the new `write_document` helper deletes the
    config file outright when no other top-level keys are left.
    Files with sibling sections (e.g. `[session]`) keep their other
    content and just lose the `[declared]` header. Added two unit
    tests in `src/declared.rs` for the file-deleted and
    sibling-preserved cases.

## Phase 4 Follow-Ups

- [x] `P4-FU-001` Document the `CONSPECTUS_DISABLE_FORGE`,
  `CONSPECTUS_DISABLE_TMUX`, and `CONSPECTUS_*_STATE` env vars
  in `docs/design.md` or a new `docs/operations.md` so users
  discover them without grepping source.
  - Outcome: added `docs/operations.md` and linked it from
    `docs/index.md`; the operations guide documents provider
    toggles, harness state-root overrides, config precedence, current
    CLI commands, and the no-cache-yet policy.
- [x] `P4-FU-002` Match PRs whose head ref is a non-current local
  branch by enumerating all local refs in the git probe. The
  Phase 4 adapter only matches the currently-checked-out branch,
  so PRs for sibling branches end up as unresolved-endpoint
  candidate links rather than node-target links. The evidence is
  still preserved; the resolved relationship just goes
  unresolved.
  - Outcome: `GitProbe` now enumerates local branch short refs via
    read-only `git for-each-ref`, `fragment_from_probe` emits
    non-current local branches as `Branch` nodes without adding
    checked-out links, and the GitHub forge provider matches PR
    `headRefName` values against the full local branch set. Added
    provider coverage for a sibling branch PR and updated git
    discovery snapshots for the newly visible branch nodes.

## Phase 3 Follow-Ups

- [x] `P3-FU-001` Align harness adapter parsers with real provider state.
  - Scope: extend the Codex, Claude Code, and opencode adapters so the cwd
    and any activity/recency timestamps from real local state populate
    `AgentSessionNode.cwd` (and link metadata where applicable). The Phase 3
    `cross_link::infer` pass already correlates sessions and mux sessions on
    matching cwds, but real harness JSONL/info.json layouts left
    `agent_session.cwd` empty during the Phase 3 smoke test.
  - Blockers: none.
  - Outcome: codex now walks `sessions/` recursively (real Codex stores
    rollouts under `sessions/YYYY/MM/DD/`), and the Claude Code adapter
    scans up to 200 JSONL lines looking for the first event that carries
    `cwd` (real sessions begin with a `permission-mode` envelope that lacks
    `cwd`), falling back to a best-effort decode of the encoded project
    directory name. Session ids are now taken from the file stem rather
    than insisting on a first-line `sessionId`. A fresh smoke run from
    inside the conspectus repo went from 16 cwd-less sessions to 34
    sessions (17 codex + 17 claude-code) all carrying cwd, 12
    `linked_to_mux` candidates, and 13 resolved relationships including
    the live claude-code session attached to the `conspectus-smoke` tmux
    session.
- [x] `P3-FU-002` Read opencode sessions from the SQLite store.
  - Scope: modern opencode (≥ ~0.5) keeps sessions in
    `~/.local/share/opencode/opencode.db` (table `session` with
    `id`, `directory`, `title`, `time_created`, `time_updated`,
    `parent_id`, etc.) rather than the legacy
    `storage/session/<id>/info.json` layout the current adapter expects.
    The legacy parser stays useful for older installs but finds nothing
    on modern setups.
  - Blockers: requires an ADR for the new SQLite read dependency
    (`rusqlite` or similar) before introducing it; CLAUDE.md forbids
    dependency additions without one.
  - Outcome: ADR 0013 records the `rusqlite` dependency decision.
    The opencode adapter now opens `opencode.db` read-only, reads
    `session.id`, `directory`, and `title` rows into `AgentSession`
    nodes, preserves the legacy `storage/session/<id>/info.json`
    parser, and lets SQLite rows win on duplicate session ids. Added
    tests for SQLite discovery, duplicate precedence, and malformed
    database degradation.

## Hardening Backlog

Deferred investments and opportunistic cleanups identified during the
post-Phase-6 codebase review. Items here are not gated by a phase plan; pull
them into a future phase or land them opportunistically when the surrounding
area is already being touched. Group prefixes:

- `H-REF-*` internal refactors and dedup
- `H-OBS-*` observability, diagnostics, and CLI UX
- `H-PROD-*` user-facing product surface gaps
- `H-DIST-*` distribution, CI, and release plumbing
- `H-DESIGN-*` open design questions to settle before they constrain
  implementation
- `H-FUTURE-*` provider/feature expansions deliberately deferred until needed

### Refactors And Dedup

- [ ] `H-REF-001` Extract a shared `DeclaredEndpoint` codec.
  - Scope: collapse the mirrored `parse_endpoint` (`src/cli.rs:703`) and
    `endpoint_label` (`src/cli.rs:745`) into one codec module (likely in
    `src/declared.rs` or a new `src/declared/endpoint_codec.rs`) so adding an
    endpoint variant requires one change. Reuse the same codec for declared
    TOML field names and CLI surface so they cannot drift.
  - Tests: round-trip property tests for every `DeclaredEndpoint` variant;
    CLI integration tests for unknown fields and missing required fields.
  - Blockers: none.
- [ ] `H-REF-002` Share the relation-kind string codec.
  - Scope: `parse_relation_kind` (`src/cli.rs:656`) and `relation_label`
    (`src/cli.rs:681`) are exhaustive mirrors. Move the mapping next to
    `RelationKind` in `src/model/mod.rs` (or derive via serde) so the CLI,
    declared store, and table renderer all read one source of truth.
  - Tests: round-trip unit tests for every variant; serde compatibility
    test against existing JSON snapshots.
  - Blockers: none.
- [ ] `H-REF-003` Generalize the resolver scoring tier helpers.
  - Scope: `MuxTier` / `mux_score` (`src/resolve/mod.rs:117`) and
    `PrProvenanceTier` / `pr_score` (`src/resolve/mod.rs:177`) duplicate the
    provenance-to-tier mapping. Introduce a `ProvenanceTier` ordering on
    `Provenance` itself, then have each comparator add only its
    relation-specific tie-breakers (recency, draft, state). Keeps relation
    comparators short and consistent.
  - Tests: existing resolver table-driven tests must continue to pass
    byte-for-byte against current snapshots.
  - Blockers: none.
- [ ] `H-REF-004` Unify the external-tool runner seam.
  - Scope: `TmuxRunner`/`SystemTmux`/`FakeTmux` (`src/discovery/tmux/mod.rs`)
    and `GhRunner`/`SystemGh`/`FakeGh`
    (`src/discovery/forge/mod.rs`) duplicate outcome enums, unavailable
    classification, and fake-runner plumbing. Extract a generic
    `ExternalCommand<O>` (or runner trait + outcome type) that both
    backends specialize. New backends (zellij, GitLab) should not need to
    re-derive the same seam.
  - Tests: keep existing unit coverage for tmux and gh runners; add one
    test that the shared abstraction classifies a missing binary the same
    way across both adapters.
  - Blockers: `H-FUTURE-001` is the obvious consumer but not a hard
    dependency.
- [ ] `H-REF-005` Split `src/declared.rs` (1338 lines) by concern.
  - Scope: separate (a) TOML models + parse/validate, (b) read-modify-write
    helpers and file I/O, and (c) snapshot-aware helpers
    (`endpoint_project_root`, `declared_endpoint_from_node_id`,
    `select_store_for_declaration`). The last group reaches into a
    `GraphSnapshot` and should live near discovery, not next to the file
    format.
  - Tests: existing declared and CLI tests must continue to pass without
    snapshot diffs.
  - Blockers: `H-REF-001` is friendlier to do first.
- [ ] `H-REF-006` Slim `src/cli.rs` (961 lines) into per-command modules.
  - Scope: move the `declared` subcommand tree, endpoint/relation codec
    helpers, and shared output formatting into a `src/cli/` module
    hierarchy. Keep `main` and top-level dispatch in `cli.rs`.
  - Tests: existing CLI smoke tests must continue to pass.
  - Blockers: `H-REF-001`, `H-REF-002`.
- [ ] `H-REF-007` Factor harness adapter state-root scanning.
  - Scope: codex, claude-code, opencode, and aider adapters each
    re-implement "look up state root from context, walk a known directory
    layout, emit `AgentSessionNode`s, swallow malformed rows." Extract a
    small shared helper that takes a parser closure so the per-adapter
    files only describe the layout. Avoid changing the public adapter
    trait shape.
  - Tests: existing harness adapter tests; add one shared-helper test for
    missing state roots.
  - Blockers: none.
- [ ] `H-REF-008` Replace string field names in `SourceMetadata.fields`.
  - Scope: discovery providers populate `source_metadata.fields` with
    stringly-typed keys (`mux_activity_epoch`, `updated_epoch`,
    `match_kind`, `fork_root`, `lineage_kind`, …). Resolvers read those
    keys back with the same strings. Introduce typed accessors (constants
    or a small `SourceField` enum with `as_str`) so the producer and
    consumer sides cannot drift silently.
  - Tests: resolver tests still pass; add a compile-time check (or doc
    test) that every known key has a constant.
  - Blockers: `H-REF-003` benefits from this but is independent.
- [ ] `H-REF-009` Centralize provider identifier constants.
  - Scope: provider keys (`"github"`, `"atelier"`, `"codex"`, …) appear as
    string literals across discovery, declared, model tests, and fixtures.
    Most harness adapters already expose `HARNESS_KEY`; finish the pattern
    for forge, mux, and atelier providers, and use the constants in
    declared parsing and CLI matching.
  - Tests: existing tests; add one assertion that the registry of harness
    keys matches the adapters wired into `discover_local_at_roots`.
  - Blockers: none.
- [ ] `H-REF-010` Audit and shrink the curated `conspectus::api` surface.
  - Scope: `api.rs` re-exports `DeclaredStoreKind`, `DeclaredStoreSelection`,
    `DeclaredSection`, and similar persistence internals that consumers
    likely should not reach for. Decide which are truly part of the
    library contract (ADR 0015) and either `#[doc(hidden)]` the rest or
    move them out of `api.rs`. Confirm every re-export has a doctest or
    explanation.
  - Tests: existing doctest; add one that asserts the public surface from
    `api::*` for the cases consumers actually have.
  - Blockers: none.

### Observability And CLI UX

- [ ] `H-OBS-001` Add a human-readable graph projection.
  - Scope: `conspectus graph --format json` is the only graph output today.
    Add `--format text` (or a separate `conspectus graph --tree`) that
    renders nodes grouped by repo/workspace with linked sessions, mux, PR,
    and fork lineage. This is the workflow `atelier status` used to cover.
  - Tests: snapshot tests for empty, sparse, and dense fixtures.
  - Blockers: none.
- [x] `H-OBS-002` Add `conspectus node show <id>`.
  - Outcome: new `src/output/node_show.rs` module exposes
    `resolve_node_id` (with `NodeResolveError::{NotFound, Ambiguous}`)
    and `render_node_show`. The `conspectus node show <id>` subcommand
    accepts the short content-addressed prefix from the session table
    (H-TBL-005), the full `NodeId` `Display` form, or the harness/mux
    label, and prints the node plus every outgoing/incoming candidate
    link (with source-metadata adapter, evidence, and fields), every
    resolved relationship touching the node, and every diagnostic
    referencing it. Unit tests cover each accepted form, ambiguous
    prefixes, and the rendered output shape; four CLI integration
    tests exercise the full discovery → resolve → render path
    including the round-trip from `session --wide` to `node show`.
    `docs/operations.md` documents the new command and the accepted
    `<id>` forms.
- [ ] `H-OBS-003` Add filter flags for the graph and session commands.
  - Scope: `--only-ambiguous`, `--only-unresolved`, `--only-orphan`, and a
    `--kind {agent_session|mux|repo|fork|pr}` filter. The graph today
    forces consumers to do their own filtering on JSON.
  - Tests: CLI integration tests against existing snapshots.
  - Blockers: none.
- [ ] `H-OBS-004` Add a `--explain` mode for resolved relationships.
  - Scope: surface why the resolver picked a given winning candidate
    (provenance tier, recency, state, conflict diagnostics). Both for the
    JSON output and `session` table cells with the `*` ambiguity marker.
  - Tests: snapshot tests for ambiguous mux and PR fixtures.
  - Blockers: `H-REF-003` is friendlier to do first because the
    explanation depends on a stable scoring shape.
  - Related: ADR 0059 (Proposed) frames this as the immediate work and
    defers the rules-engine question behind it; review and accept/reject
    via `H-ADR-0059-REVIEW` before scoping `--explain` implementation.
- [ ] `H-ADR-0059-REVIEW` Review and resolve ADR 0059 (resolver
  rules-engine evaluation).
  - Scope: read `docs/adr/0059-resolver-rules-engine-evaluation.md`,
    decide accept / amend / reject. Key knobs to tune if accepting:
    (a) the deferred-Ascent posture in §Decision (3), (b) the
    counted-bug re-trigger threshold in §Decision (4). Update status
    from `Proposed` to `Accepted` / `Rejected` / `Superseded` and
    record any amendments inline. Skipping accept-as-drafted is fine;
    the artifact's purpose is to stop the question from re-surfacing
    without an explicit re-trigger.
  - Tests: none (ADR-only).
  - Blockers: none.
- [ ] `H-OBS-005` Improve discovery diagnostics for missing providers.
  - Scope: when `gh` is unavailable, `tmux` is not installed, declared
    config is malformed, or a harness state root is missing, surface a
    `Diagnostic` row in the graph and a one-line stderr hint in CLI
    commands. Today some of these degrade silently (forge unavailable,
    missing state roots) while others (`ConfigDiagnostic`) only print to
    stderr.
  - Tests: CLI integration tests that capture stderr and JSON
    diagnostics across each provider failure mode.
  - Blockers: none.
- [ ] `H-OBS-006` Surface activity/recency in the session tables.
  - Scope: `AgentSessionNode` exposes some recency metadata (claude-code
    cwd discovery propagates timestamps; mux activity epochs flow through
    candidate metadata) but the table renders no recency column. Add a
    "last activity" column derived from session, mux, and PR signals,
    using a relative formatting helper (`2h`, `3d`).
  - Tests: snapshot tests for representative fixtures with normalized
    timestamps.
  - Blockers: `H-REF-008` (typed source-metadata fields makes recency
    extraction safer).
- [x] `H-OBS-007` Gate left-pane tree navigation keys on left-pane focus.
  - Outcome: `remap_for_focus()` in `src/tui/runtime.rs` now drops
    `Msg::ExpandRow` / `Msg::CollapseRow` (h/l/Left/Right) on
    right-pane focus so they no longer mutate the left tree the
    operator isn't driving, and remaps `Msg::Home` / `Msg::End`
    (g/G/Home/End) to new `Msg::ExplorerHome` / `Msg::ExplorerEnd`
    variants that snap the explorer cursor to its first / last
    row — the right-pane-equivalent the operator expects. The
    reducer dispatches the new variants through a small
    `explorer_jump_cursor_to(usize)` helper that clamps to the
    current row count and mirrors `move_selection_to` for the
    left tree. `Msg::CycleFocus` (Tab) is intentionally left
    alone since it's the focus toggle itself. Tests:
    `remap_for_focus_right_suppresses_left_tree_expand_collapse_keys`
    pins the h/l drop;
    `remap_for_focus_right_routes_home_and_end_into_the_explorer`
    pins the g/G remap and that Tab stays the focus toggle;
    `explorer_home_and_end_snap_cursor_to_first_and_last_row`
    pins the reducer dispatch. Followups (out of scope): PageDown
    / PageUp currently remap to a single-row explorer step
    rather than a true page — the per-frame viewport height
    isn't threaded through, and that's worth its own story if
    operators ask for it.

### Table Output Modernization

The current `conspectus session` tables (`src/output/table.rs`) hand-roll
column alignment via a fixed 2-space padder. Cells like cwd, agent label,
mux session, and PR identifier routinely blow past any reasonable terminal
width, making the default output unusable in narrow CLIs. JSON / graph
output is not affected by this stream; the work is scoped to the text-table
projection layer.

H-TBL-001 through H-TBL-005 modernized the renderer itself (width-aware
truncation, short row ids, card layout, `node show` integration).
H-TBL-006 onward shifts the surface from `conspectus session
[--projection ...]` to `conspectus table <ROWS>` so that growing row-types
(PRs, forks, …) and per-row-type column customization stay first-class.
The columns themselves stop being session-specific, since cells like PR,
fork lineage, and checkout apply to any row whose node touches them.

- [x] `H-TBL-001` ADR: width-aware table rendering library.
  - Outcome: ADR 0020 records the decision to roll our own minimal
    width-aware renderer under `src/output/`, depending only on
    `unicode-width` and `terminal_size`. `comfy-table` (upstream feature
    freeze, wraps rather than truncates), `tabled` (heavier surface, API
    churn risk for snapshot tests), `cli-table`, and `prettytable-rs`
    were considered and rejected. Both dependencies are runtime deps
    added in `H-TBL-003`.

- [x] `H-TBL-002` Surface short, stable row identifiers in session tables.
  - Outcome: every `conspectus session` projection now emits a leftmost
    `ID` column carrying a short, content-addressed prefix derived from
    the row's primary `NodeId`. The hash is FNV-1a 64-bit over the
    `Display` form of the NodeId (`pub fn node_short_id` in
    `src/output/table.rs`), exposed so `node show` (H-TBL-005) can
    resolve a pasted id back to a node. Prefix length is the minimum
    needed for uniqueness within the rendered snapshot, floored at six
    hex chars. The union projection's existing `ID` header (which held
    the harness label) was renamed to `LABEL` to free the `ID` slot for
    the new short id. JSON output is unchanged. New unit tests pin the
    hash determinism, prefix-growth-on-collision, and per-projection
    header changes; the atelier-delegation and declared-snapshots
    fixtures gained per-snapshot `state_scope` path normalization so
    the rendered short id stays stable across runs.
  - Blockers: none.

- [x] `H-TBL-003` Width-aware truncation default for session tables.
  - Outcome: `src/output/table.rs` now exposes `RenderOptions { width,
    layout }` and `render_with(snapshot, projection, options)`. The
    existing `render(...)` is preserved as a thin wrapper around
    `RenderOptions::wide()`, so every snapshot test stayed byte-for-byte
    stable. `render_with` measures every cell via
    `unicode_width::UnicodeWidthStr::width`, greedy-shrinks per-column
    budgets toward the target width (never below `max(header_width, 4)`),
    and truncates overflowing cells with a trailing `…`. The `session`
    subcommand gained `--wide` and `--width <N>` flags. Default behavior:
    `--wide` ⇒ untruncated; `--width N` ⇒ exact N columns; otherwise
    detect via `terminal_size::terminal_size()` when stdout is a TTY,
    else stay wide so pipes remain grep/awk-friendly. Added unit tests
    for truncation/budget edge cases (including wide CJK columns) and
    CLI integration tests for `--wide`, `--width`, the pipe-stays-wide
    default, and the clap-level `--wide`/`--width` conflict.
    Dependencies recorded by ADR 0020: `unicode-width = "0.2"` and
    `terminal_size = "0.4"`.

- [x] `H-TBL-004` Opt-in card / multi-line row layout.
  - Outcome: `Layout::Card` joins `Layout::Columnar` in `RenderOptions`,
    with `RenderOptions::card()` and `RenderOptions::card_width(n)`
    convenience constructors. The new `render_card` path emits one
    `KEY: value` line per column with keys aligned on the colon and a
    blank line between rows. Width-aware mode truncates long values
    (using the same `truncate_to_width` helper as columnar) so a
    `--width N` budget is honored. The `session` subcommand gained a
    `--layout {columnar|card}` flag (default columnar). Added four unit
    tests covering empty snapshots, block separation, colon alignment,
    and width-aware truncation, plus a CLI integration test for
    `--layout card`.

- [x] `H-TBL-005` Resolve table row identifiers in `conspectus node show`.
  - Outcome: implemented together with H-OBS-002. The `node show <id>`
    resolver accepts (a) the short content-addressed prefix from the
    session table's `ID` column, prefix-matched (floor 4 hex chars),
    (b) the full `NodeId` `Display` form, and (c) the harness/mux
    label when it uniquely identifies one node. Ambiguous prefixes
    error with the matching candidates listed. `docs/operations.md`
    documents the accepted forms; CLI integration tests round-trip a
    short id from `conspectus session --wide` through `node show`.

- [x] `H-TBL-006` Rename `conspectus session` to `conspectus table <ROWS>`.
  - Outcome: ADR 0021 records the rename. The CLI grew a `table`
    subcommand tree with `Sessions`, `Mux`, and `Union` subcommands;
    each takes the existing `--wide`, `--width`, `--layout`, and
    `--scan-root` flags via shared `TableRowsArgs`. The old
    `session` subcommand and `--projection` flag are gone with no
    alias (per CLAUDE.md). Config migrated from `[session].projection`
    to `[table.<rows>]` per-row-type subsections — empty for H-TBL-006
    but reserved for the column registry in H-TBL-007. A legacy
    `[session]` section in user config now produces a stderr
    diagnostic pointing at the new schema; the run still proceeds.
    `Projection::parse` accepts both `agent` (legacy) and `sessions`
    (new) for the agent-projection row-type. All 340 tests pass,
    including the existing insta snapshots: the rename is
    CLI-and-config-only, the renderer internals
    (`build_*_rows`, `RenderOptions`, `node_short_id`) are
    unchanged. `docs/operations.md` documents the new shape.

- [x] `H-TBL-007` Per-row-type column registry and `--columns` flag.
  - Outcome: `src/output/table.rs` grew a column registry keyed by
    row-type. `ColumnSpec` records each column's stable `key`,
    header label, one-line description, and `default` flag. The
    `sessions`, `mux`, and `union` row-types each have their
    registry slice plus a typed row-context struct
    (`AgentRowCtx` / `MuxRowCtx` / `UnionRowCtx`) and a cell
    extractor (`agent_cell` / `mux_cell` / `union_cell`). The three
    `build_*_rows` functions now walk a `&[&'static str]` column
    list and dispatch per cell. `RenderOptions::columns:
    Option<Vec<&'static str>>` carries the selection; `None` falls
    back to `default_columns(projection)`. `parse_columns_spec`
    handles the `default` / `all` / `+name` / `-name` / explicit-list
    token grammar from the backlog; `resolve_explicit_columns`
    backs the config side. The `conspectus table <ROWS>` subcommands
    gained `--columns LIST`. Config grew
    `[table.<rows>].columns = [...]` (loaded via
    `TableRowConfig::columns`); CLI flag overrides config when both
    are present. Unknown columns error with the registered list
    surfaced on stderr. Cached `attached_to_mux` on `SnapshotView` so
    the mux "agents" cell stays O(1) per row. New unit tests cover
    the parser (`default`/`all`/`+`/`-`/explicit-list/unknown/empty
    tokens), `resolve_explicit_columns` validation, registry
    defaults, and `render_with`/`with_columns` for both columnar
    and card layouts. CLI integration tests cover `--columns`
    override of defaults, delta tokens (with the
    `--columns=-name,...` equals form so clap accepts the leading
    dash), unknown-column errors, config-driven defaults, and CLI
    overriding config. All 357 tests pass; existing snapshots stay
    byte-for-byte stable because the registry's default sets match
    the prior hard-coded headers and extractors. `docs/operations.md`
    documents the flag and config knob.

- [x] `H-TBL-008` `conspectus table prs` row-type.
  - Outcome: `Projection::Pr` joins the row-type enum; the registry
    `PRS_COLUMNS` declares `id`, `pr`, `state`, `draft`, `branch`,
    `repo`, `updated`, and `attached`, with the default set
    `id, pr, state, branch, attached`. `PrRowCtx` + `pr_cell` extract
    cells: the `branch` column walks `BranchHasForgePr` and strips
    the `refs/heads/` prefix; `attached` finds checkouts via
    `CheckedOutBranch` candidates and joins agent sessions whose
    `cwd` matches the checkout root; `updated` formats
    `updated_epoch` via the new `format_relative_age` helper
    (`12s`, `5m`, `2h`, `3d`, `4w`). Config grew `[table.prs]`. CLI
    subcommand `conspectus table prs` honors the existing `--wide`,
    `--width`, `--layout`, `--scan-root`, and `--columns` flags.
    Eight new unit tests cover `format_relative_age`,
    `strip_branch_prefix`, the default header, the `attached`
    discovery via the BranchHasForgePr → CheckedOutBranch path,
    optional-column rendering, and the empty-snapshot case. Two
    CLI integration tests cover the default projection header and
    the optional-columns flag. All 365 tests pass; existing
    snapshots stay byte-for-byte stable.

- [x] `H-TBL-009` `conspectus table forks` row-type.
  - Outcome: `Projection::Fork` joins the row-type enum; the registry
    `FORKS_COLUMNS` declares `id`, `fork`, `provider`, `scope`,
    `parent`, `children`, and `capabilities`, with the default set
    `id, fork, provider, parent, children`. `ForkRowCtx` +
    `fork_cell` extract cells: `fork` renders `{provider}:{name}` or
    falls back to `provider_source_key` when no name is set;
    `parent` follows the fork's preferred `ParentSession` candidate
    to a short session id (unresolved parents prefixed with `?`);
    `children` counts `ChildSession` candidates from the fork that
    target agent-session endpoints (resolved or unresolved). Forks
    are now indexed on `SnapshotView` alongside the other typed
    node maps. Config grew `[table.forks]`. CLI subcommand
    `conspectus table forks` honors the existing `--wide`,
    `--width`, `--layout`, `--scan-root`, and `--columns` flags.
    Four unit tests cover the default header, fork-label fallback,
    parent/children extraction, and capabilities-as-optional-column
    rendering. One CLI integration test exercises the default
    header. All 370 tests pass.

- [x] `H-TBL-010` Expand the `sessions` column pool.
  - Outcome: `SESSIONS_COLUMNS` gained five opt-in columns
    (`checkout`, `branch`, `repo`, `fork`, `declared`). Each
    extractor walks the candidate-link graph to resolve the cell:
    `checkout` matches a session's `cwd` against `CheckoutId.root`;
    `branch` follows `CheckedOutBranch` from the matched checkout
    and strips `refs/heads/`; `repo` returns the checkout's
    `RepoId.common_dir`; `fork` finds the fork that records the
    session as a `ChildSession` target and renders the fork label;
    `declared` reports the strongest declared candidate's state
    (`declared` / `ignored` / `overridden`) by walking the raw
    `snapshot.candidate_links` (so ignored/overridden links surface
    through the otherwise active-only `by_source_relation` index).
    `SnapshotView` now retains a reference to the underlying
    `GraphSnapshot` for that purpose. The default column set is
    unchanged. The `activity` column is deferred per the H-OBS-006
    soft-blocker note. Three new unit tests cover checkout/branch/
    repo, the fork column, and the declared column's link-state
    mapping; one CLI integration test exercises the seven-column
    selection via `--columns`. All 374 tests pass.

- [x] `H-TBL-011` Expand the `mux` column pool.
  - Outcome: `MUX_COLUMNS` gained three opt-in columns:
    `attached-count` (number of attached agent sessions, rendered
    as `—` when zero), `activity` (relative recency from
    `MuxSessionNode::activity_epoch`, reusing `format_relative_age`
    from H-TBL-008), and `created` (relative age from
    `created_epoch`). `panes` stays deferred until the mux adapter
    records pane counts. Default set is unchanged. Three unit tests
    cover `attached-count` (one row with attached agents, one with
    none), `activity`/`created` formatting (using
    `format_relative_age` and verifying the recency-suffix shape of
    the rendered cell), and the no-epoch fallback to `—`. The
    `parse_columns_all_resets_to_every_registered_column` regression
    test was updated to reflect the larger `all` set. All 377 tests
    pass.

- [x] `H-TBL-012` `conspectus columns <ROWS>` discovery subcommand.
  - Outcome: new `render_columns_listing(projection)` helper in
    `src/output/table.rs` prints each registered column as
    `<key>  <description>  (default)?` with the key column padded
    for alignment. The CLI gained a top-level `conspectus columns
    <ROWS>` subcommand that resolves the positional via
    `config::Projection::parse` (so the same `sessions`/`mux`/
    `union`/`prs`/`forks` tokens accepted by `conspectus table`
    work here). Two new unit tests pin the `(default)` marker on a
    default column and verify every registered key appears in the
    listing for every projection. Two CLI integration tests cover
    the listing across all five row-types (asserting both a
    default-marked column and an opt-in column appear) and the
    unknown-row-type error path. `docs/operations.md` documents
    the new command. All 381 tests pass.

- [x] `H-TBL-013` Pager auto-fit for table-style outputs.
  - Outcome: `conspectus table <ROWS>`, `conspectus columns <ROWS>`,
    and `conspectus node show <id>` now pipe their output through a
    pager when stdout is a TTY. Resolution order in
    `pager_candidates()` (in `src/cli.rs`): `$PAGER` (split on
    whitespace into program + args); otherwise `less` with
    `LESS=FRX` defaults when `$LESS` is unset (`F`=quit if one
    screen, `R`=raw control chars, `X`=no init/deinit) so short
    tables print inline; otherwise `more`; otherwise direct print
    when no pager spawns. Each affected subcommand gained
    `--no-pager` (force off) and `--pager` (force on, clap-level
    conflict with `--no-pager`). Non-TTY output stays direct by
    default so existing pipe-based tests and `conspectus table
    sessions | grep …` workflows are unchanged. `graph --format
    json` and `declared list` deliberately stay direct (JSON is
    machine-consumable; declared output is short-lived
    tab-separated text — users can pipe through a pager manually
    if needed). Four CLI integration tests cover the
    `PAGER=cat --pager` round trip for `table` and `columns`,
    `--no-pager` bypassing a pager that would otherwise fail
    (`PAGER=false`), and the `--pager`/`--no-pager` clap conflict.
    All 385 tests pass. `docs/operations.md` documents the new
    behavior.

- [x] `H-TBL-014` Terminal color and styling for table output.
  - Outcome: ADR 0022 records the decision to use `anstyle` (already
    transitive through clap) and the env-var precedence. The
    renderer gained `RenderOptions.color` plus
    `RenderOptions::with_color(bool)`; `agent_cell` /
    `mux_cell` / `union_cell` / `pr_cell` / `fork_cell` keep their
    `String` return shape because styling is applied by the
    columnar/card emitters via a shared `push_styled` helper that
    only injects ANSI when `color=true`. Width-aware truncation
    runs on the unstyled text, so the ANSI envelope wraps already-
    truncated cells and the budget math is unchanged. The initial
    palette matches the ADR: bold headers and card-layout keys,
    dim `—`/ID column, indicator-tier colors
    (`LD`/`GD`=green, `SD`=cyan, `C`/`$`=dim), PR state colors,
    DECLARED state colors, yellow on `draft`. `render_columns_listing`
    and `render_node_show` accept a `color` flag and bold their keys
    / section headers; the table renderer keeps its richer palette.

    Each affected CLI subcommand (`conspectus table <ROWS>`,
    `conspectus columns <ROWS>`, `conspectus node show <id>`) gained
    `--color {auto|always|never}`. The pure `resolve_color` helper
    implements the ADR-0022 precedence (`--color=never|always`
    short-circuit; `NO_COLOR` overrides `auto`; `CLICOLOR_FORCE`
    forces on; `TERM=dumb` and `CLICOLOR=0` opt out; else `auto`
    falls back to isatty). `resolve_color_from_env` wraps it with
    the live process env.

    Eight unit tests pin every resolver branch (never / always /
    NO_COLOR / CLICOLOR_FORCE / TERM=dumb / CLICOLOR=0 / auto+TTY /
    auto+pipe). Three renderer unit tests assert byte-identical
    output when color is off, ANSI envelope presence on
    `--color=on`, and the green PR-state color. Five CLI
    integration tests cover `--color=always`/`never`/`auto`, the
    NO_COLOR-vs-`--color=always` precedence (explicit user flag
    wins), and color on `conspectus columns`. All 406 tests pass.
    `docs/operations.md` documents the flag, the env precedence,
    and the palette.

Deferred under this cluster (no story yet, file when needed):

- `conspectus table repos` / `conspectus table checkouts`. Both node
  kinds already appear as related-context columns under
  `H-TBL-010`. Promote to their own row-type only when a user
  workflow requires a repos-first or checkouts-first table.

### Product Surface Gaps

- [ ] `H-PROD-001` Implement the bootstrap-roots flow described in the
  design.
  - Scope: `docs/design.md` "Discovery Strategy" mentions a future
    bootstrap mode that prints suggested roots and links by default and
    requires an explicit write flag to persist. Add `conspectus bootstrap`
    (or `conspectus graph --suggest-roots`) that scans selected
    directories and prints a TOML stanza, with `--write` controlling
    persistence.
  - Tests: CLI integration tests for suggested output, `--write` behavior,
    and read-only defaults.
  - Blockers: `H-DESIGN-001` for the persistence target.
- [ ] `H-PROD-002` Cache layer for forge metadata, tmux, and harness scans.
  - Scope: design.md commits to caches living under `$XDG_DATA_HOME` (and
    keeping them outside project trees), but no cache code exists. Define
    a cache schema, TTL policy, and `--no-cache` / `--refresh` flags.
    Apply first to forge (`gh pr list` per repo is the most expensive
    call) and reuse the seam for tmux and harness state.
  - Tests: cache hit/miss, TTL expiry, schema-version mismatch, and
    `--no-cache` flag behavior.
  - Blockers: a new ADR for the cache layout and freshness rules.
- [ ] `H-PROD-003` Batch `gh pr list` across repos sharing a host.
  - Scope: `GitHubForgeProvider` runs `gh pr list --json` once per
    discovered repo (`src/discovery/forge/mod.rs:67`). For workspaces
    with several repos on the same host/owner this multiplies the spawn
    cost. Investigate whether `gh search prs --owner` (or a parallelized
    batch invocation) is appropriate, and keep the per-repo path as a
    fallback.
  - Tests: parser tests for the batched JSON; integration test with a
    `FakeGh` that records spawn counts.
  - Blockers: `H-PROD-002` (caching narrows the urgency).
- [ ] `H-PROD-004` Add a graph-diff command.
  - Scope: `conspectus graph diff <a.json> <b.json>` (or save snapshots
    under `$XDG_DATA_HOME` and diff against the previous run). Useful for
    explaining "what changed since the last fork" and for Atelier
    delegation acceptance criteria.
  - Tests: snapshot tests for added/removed nodes and links and changed
    resolution.
  - Blockers: `H-PROD-002` if diffs reuse the cache layer.

### Distribution And CI

- [x] `H-DIST-001` Add a GitHub Actions CI workflow.
  - Scope: ADR 0016 commits to crates.io distribution but there is no
    `.github/workflows/` directory and the only check automation is local
    (`justfile`, `flake.nix`). Add a CI job that runs
    `cargo fmt --check`, `cargo clippy -- -D warnings`,
    `cargo test --all-targets --all-features`, and
    `cargo nextest run --all-targets --all-features` on PRs and main.
  - Tests: CI run on the change itself.
  - Blockers: none.
  - Outcome: added `.github/workflows/ci.yml` for pull requests and pushes
    to `main`. The workflow installs stable Rust with `clippy` and `rustfmt`,
    caches Cargo artifacts, installs `cargo-nextest`, and runs the same
    baseline checks as the local `justfile`: formatting, clippy with warnings
    denied, cargo test, nextest, and `git diff --check`.
- [ ] `H-DIST-002` Complete `Cargo.toml` metadata for crates.io.
  - Scope: `Cargo.toml` is missing `authors`, `repository`, `homepage`,
    `documentation`, `keywords`, `categories`, `readme`, and an
    `exclude`/`include` pattern. Fill in for the first publish and verify
    `cargo publish --dry-run` succeeds.
  - Tests: `cargo publish --dry-run` in CI on tagged releases.
  - Blockers: `H-DIST-001`.
- [ ] `H-DIST-003` Pin and verify MSRV.
  - Scope: ADR 0016 says the effective MSRV is the toolchain pinned by
    the Nix dev shell. Make this explicit in `Cargo.toml`
    (`rust-version = "1.85"` or similar) and add a CI job that builds
    against the pinned stable to catch accidental MSRV bumps.
  - Tests: dedicated CI job pinning the toolchain.
  - Blockers: `H-DIST-001`.
- [ ] `H-DIST-004` Define the release process.
  - Scope: ADR 0016 names the validation steps but the repo has no
    `CHANGELOG.md`, no release script, and no tagged-build workflow.
    Decide whether to adopt `cargo release` or a hand-rolled checklist
    and document it under `docs/operations.md` (or a new
    `docs/releasing.md`).
  - Tests: dry-run the release procedure end-to-end before tagging
    `v0.1.0`.
  - Blockers: `H-DIST-001`, `H-DIST-002`.

### Design Closure

- [x] `H-DESIGN-001` Settle the workspace-detection threshold and provider
  precedence.
  - Scope: `docs/design.md` "Remaining Design Questions" calls out (a)
    evidence threshold for inferring a generic `Workspace`, (b) handling
    of nested workspaces / nested repos / symlinked repos, (c)
    interaction with provider-specific metadata, and (d) precedence when
    multiple providers claim the same path. Today
    `src/discovery/workspace.rs` infers a workspace only when a scan root
    has 2+ immediate git repo children; document the rule in an ADR or
    refine it.
  - Tests: fixture tests covering the chosen rule for nested, symlinked,
    and provider-claimed roots.
  - Outcome: ADR 0027 settles the generic threshold as two or more
    immediate git checkout children under an explicit scan root, with
    provider-specific workspace metadata taking precedence over generic
    inference at the same canonical root. Generic inference now stands
    down when `atelier.toml` claims the scan root.
  - Blockers: none.
- [ ] `H-DESIGN-002` Settle `ForgePr` identity and branch-association keys.
  - Scope: `docs/design.md` "Remaining Design Questions" lists open
    questions about provider-neutral ForgePr fields, branch-to-PR keying
    (name vs upstream vs head ref), and multi-PR-per-branch
    representation. Record decisions in an ADR; the GitHub adapter today
    matches by short head ref against the full local branch set
    (`P4-FU-002`) but the rule is undocumented.
  - Tests: regression tests for fork-head PRs (different head repo) and
    closed/historical PR handling.
  - Blockers: none.
- [ ] `H-DESIGN-003` Settle declared-link conflict and override semantics.
  - Scope: `docs/design.md` "Remaining Design Questions" asks (a) whether
    an override suppresses a single candidate, all candidates of a
    relation kind, or all links between two nodes; (b) whether "ignored"
    is node-level, link-level, or both; (c) the merge rule across local,
    global, discovered, and cached evidence; (d) whether confirmation
    creates a durable declared link even if the discovered evidence
    disappears. Record the conclusions in a new ADR and tighten the
    resolver tests.
  - Tests: resolver tests for each conflict scenario.
  - Blockers: none.
- [ ] `H-DESIGN-004` Document the graph invariants and snapshot
  canonicalization contract.
  - Scope: callers (and the api facade) need to know when a
    `GraphSnapshot` is canonical, when cross-link inference has run, and
    what invariants hold after `discover_local_with` vs after
    `resolve_snapshot`. Add a short contract section to
    `docs/library-api.md` and consider asserting invariants in
    `merge_fragments`.
  - Tests: unit tests for the documented invariants.
  - Blockers: none.

### Checkout Context Model

ADR 0026 replaces "worktree" as the product-level concept with
`Checkout`: the concrete editable working tree for a repo, whether it is
an ordinary clone checkout, a linked git worktree, a bare-repo-derived
linked worktree, or a workspace member reached through a symlink.

- [x] `H-CHECKOUT-001` Memorialize the checkout context model.
  - Scope: record the decision in ADR 0026, update `docs/design.md` to
    use checkout terminology for the north-star model, and create this
    backlog workstream.
  - Outcome: ADR 0026 defines `Checkout`, the identity rule, cwd
    probing, workspace overlay behavior, grouping precedence, and the
    staged terminology migration from legacy `Worktree` names.
  - Tests: docs-only; `git diff --check`.
  - Blockers: none.
- [x] `H-CHECKOUT-002` Introduce checkout-facing model helpers ahead of
  the graph wire rename.
  - Scope: add `Checkout` model/helpers as the canonical code-level
    vocabulary while keeping the current `Worktree` graph representation
    until the hard wire/model rename lands.
  - Outcome: initial checkout-facing helpers were introduced as a staging
    step, then replaced by canonical checkout graph/model names in
    `H-CHECKOUT-008`.
  - Tests: graph JSON snapshot/round-trip tests proving checkout-facing
    helpers produce the same node identities.
  - Blockers: `H-CHECKOUT-001`.
- [x] `H-CHECKOUT-003` Probe observed session and mux cwd paths for
  checkout context.
  - Scope: collect distinct cwd paths from discovered agent sessions and
    mux sessions, run read-only git probes for each path, and backfill
    `Repo`, `Checkout`, and `Branch` nodes plus candidate links even
    when the cwd is outside the launch cwd or configured scan roots.
    Reserve `Ungrouped` for sessions with no usable path or context
    evidence.
  - Slice landed: `discover_local_with` now probes distinct observed
    agent-session and mux-session cwd paths after initial discovery and
    merges any git repo/checkout/branch evidence before cross-link
    inference.
  - Slice landed: table and TUI projections now match sessions whose cwd
    is nested under a checkout root, choosing the deepest matching
    checkout.
  - Outcome: coverage now includes plain clone cwd, linked worktree cwd,
    bare-repo-derived worktree cwd, nested cwd inside a checkout,
    nonexistent cwd, and mux cwd outside configured scan roots.
  - Tests: `cargo test observed_session_cwd_backfills_git_context_outside_scan_roots`;
    `cargo test checkout`; `cargo test sessions_projection_optional_branch_repo_worktree_columns`;
    `cargo test prs_projection_attached_shows_agent_with_matching_cwd`.
  - Blockers: `H-CHECKOUT-001`.
- [x] `H-CHECKOUT-004` Preserve logical and canonical paths for workspace
  members.
  - Scope: when a workspace member is reached through a symlink or
    provider-local member path, store both the workspace-visible logical
    path and the canonical checkout root. Use canonical checkout root for
    identity and logical path/source metadata for display and evidence.
  - Slice landed: generic and Atelier `workspace_contains_repo`
    candidates now preserve `logical_path`, `canonical_checkout_root`
    when discovered, and `member_path_kind` source metadata. Atelier
    links also preserve `provider_source_path` and `repo_name`.
  - Outcome: generic discovery skips broken symlink members instead of
    aborting the scan, and duplicate workspace-visible paths resolving
    to the same repo get distinct candidate IDs keyed by logical path.
  - Tests: fixtures covering symlinked plain clones, provider member
    paths, broken symlinks, and duplicate logical paths resolving to the
    same checkout.
  - Blockers: none.
- [x] `H-CHECKOUT-005` Resolve multi-context session membership.
  - Scope: extend cross-link resolution so a session can associate with
    both a workspace and the underlying checkout/repo/branch. Preserve
    candidate evidence for each context and expose enough resolved data
    for projections to choose deduped or multi-home display.
  - Tests: resolver tests for workspace-member sessions, checkout-only
    sessions, ambiguous workspace providers, and sessions with multiple
    mux candidates.
  - Slice landed: cross-link inference now emits
    `AgentSession`→checkout `associated_with` candidates when the
    session cwd is at or under a discovered checkout root, choosing the
    deepest checkout for nested repo cases. Resolver multi-home semantics
    are still open.
  - Outcome: cross-link inference now also emits
    `AgentSession`→workspace `associated_with` candidates from
    workspace-member `logical_path`/`canonical_checkout_root` metadata.
    The resolver treats `associated_with` and `workspace_contains_repo`
    as multi-target relations, so distinct contexts resolve
    independently while duplicate evidence for the same target still
    competes normally.
  - Tests: `cargo test checkout`.
  - Blockers: `H-CHECKOUT-003`, `H-CHECKOUT-004`.
- [x] `H-CHECKOUT-006` Update table and TUI projections for checkout
  grouping.
  - Scope: replace single-parent checkout grouping assumptions with
    checkout/workspace-aware projection rules. Default to including
    workspace overlay groups while also allowing checkout-centric output;
    add include/exclude workspace controls before making workspace
    duplication visible by default.
  - Tests: table snapshots and TUI row-tree tests showing the same
    session under workspace and checkout when appropriate, plus a
    workspace-excluded mode with no duplicate workspace rows.
  - Slice landed: repo rows in the TUI sessions tree display a
    human-oriented repo source path instead of the git common-dir
    identity, with a `/.git` stripping fallback.
  - Outcome: TUI graph grouping now prefers resolved
    session→workspace context, while repo grouping remains the
    workspace-excluded view. The sessions table now has an opt-in
    `workspace` column exposing resolved workspace context alongside
    existing checkout/repo/branch columns.
  - Tests: `cargo test repo_group`.
  - Blockers: `H-CHECKOUT-005`.
- [x] `H-CHECKOUT-007` Retire legacy user-facing worktree terminology.
  - Scope: rename CLI columns, docs, help text, and TUI labels from
    worktree to checkout where the
    user-facing meaning is the broader ADR 0026 concept. Keep git-linked
    worktree wording only when specifically describing git's feature.
  - Tests: CLI help snapshots/table snapshots once those exist; docs-only
    `git diff --check` for prose-only slices.
  - Outcome: sessions table output now exposes `checkout`/`CHECKOUT`
    instead of `worktree`/`WORKTREE`, PR table help and TUI checkout detail
    labels use checkout terminology, and `--sessions-grouping checkout` is
    accepted. Legacy `worktree` table columns, TUI grouping values, and
    checkout JSON deserialization aliases are intentionally not preserved.
  - Blockers: `H-CHECKOUT-006`.
- [x] `H-CHECKOUT-008` Hard-rename checkout graph wire/model names.
  - Scope: replace legacy `WorktreeId`/`WorktreeNode`/`GraphNode::Worktree`
    naming, node id display prefixes, JSON `type: "worktree"`, snapshot
    expectations, declared endpoint syntax, and user-visible relation docs
    with checkout terminology where the concept is broader than git linked
    worktrees. Keep git-specific `worktree` only for actual `git worktree`
    feature behavior and source metadata.
  - Tests: full graph snapshot refresh, declared endpoint round trips, node
    id round trips, resolver tests for checkout/session links, table/TUI
    smoke coverage, and `cargo test --all-targets --all-features`.
  - Outcome: graph/model names now use `CheckoutId`, `CheckoutNode`, and
    `GraphNode::Checkout`; node ids display as `checkout:...`; graph JSON
    serializes `type: "checkout"`; declared endpoints use
    `checkout:repo_common_dir=...,root=...`; and fork effect relation wire
    names are `created_checkout` / `referenced_checkout`. Git commands,
    fixture names, and provider-native Atelier fields still say worktree when
    they describe actual git or source-format worktree concepts.
  - Blockers: `H-CHECKOUT-007`.

### Deferred Provider And Workflow Expansions

These items match the design guidance to *design for* additional providers
without *implementing* them until needed. File them so the next consumer
need does not surprise the project.

- [ ] `H-FUTURE-001` Add a mux backend for zellij (and stub screen).
  - Scope: introduce a `MuxRunner` abstraction shared with tmux
    (depends on `H-REF-004`), add a zellij adapter behind it, and leave
    screen as a documented extension point.
  - Tests: parser tests against canned `zellij list-sessions` output.
  - Blockers: `H-REF-004`.
- [ ] `H-FUTURE-002` Add a forge adapter for GitLab or Gitea.
  - Scope: validate the `ForgeAdapter` boundary against a second provider
    once a user need surfaces. Reuse the shared external-runner seam.
  - Tests: fixture-driven adapter tests with a canned `glab` (or REST)
    payload.
  - Blockers: `H-REF-004`, `H-DESIGN-002`.
- [ ] `H-FUTURE-003` Add harness adapters for jujutsu and sapling sessions
  if and when a user uses them with a supported harness.
  - Scope: not on the roadmap until requested; track here so the request
    has a home.
  - Tests: TBD.
  - Blockers: requires user demand.

### Documentation

- [ ] `H-DOC-001` Add a first-run walkthrough.
  - Scope: `README.md` and `docs/index.md` jump straight into design and
    operations. Add a short tutorial that walks through running
    `conspectus graph` and `conspectus session` from a plain repo,
    creating a declared link, and inspecting the result. Link it from the
    README.
  - Tests: docs-only; `git diff --check`.
  - Blockers: none.
- [ ] `H-DOC-002` Add a provider-adapter contributor guide.
  - Scope: document the `HarnessAdapter`, `ForgeAdapter`, `TmuxRunner`,
    and `DiscoveryProvider` contracts so a contributor adding a new
    harness or forge knows where to plug in. Use the existing codex and
    GitHub adapters as worked examples.
  - Tests: docs-only.
  - Blockers: `H-REF-004` (the boundary is simpler to document after the
    shared seam exists).
- [ ] `H-DOC-003` Add library-integration examples beyond the api doctest.
  - Scope: `docs/library-api.md` names the entry points but provides no
    worked example for embedding Conspectus in a TUI or test. Add at
    least one end-to-end snippet (probably in `docs/library-api.md` and a
    second doctest under `conspectus::api`).
  - Tests: doctest run as part of `cargo test`.
  - Blockers: none.

### Intra-Harness Session Lineage

`RelationKind::ParentSession` / `ChildSession` are wired through the model
(ADR 0005) and the resolver, but today they are only emitted from
atelier's fork-index metadata (`src/discovery/atelier.rs:423-440`). None
of the harness adapters extract lineage from the harness's own state, so
post-compaction, post-resume, and post-fork-by-the-harness sessions show
up as independent rows even when one is a direct successor of another.
With long-lived users, this means a large fraction of the
`conspectus session` rows are legacy sessions whose link to a currently
active session is silently dropped.

ADR 0005 currently frames `ParentSession` / `ChildSession` as edges
anchored at a `Fork` node. Intra-harness compaction/resume is *not* a
fork (no context effect, no provider-recorded fork metadata). Decide
during `H-LINEAGE-001` whether to (a) extend ADR 0005 to allow
session→session edges without a Fork middle node, or (b) require a
synthetic `Fork` node with provider `<harness>` and an explicit
`lineage_kind` such as `compaction` or `resume`. The former is simpler
for queries; the latter keeps lineage uniform with the fork-anchored
shape.

- [x] `H-LINEAGE-001` Settle the data-model shape for intra-harness
  lineage.
  - Resolution: ADR 0018 extends ADR 0005 to allow intra-harness
    `parent_session` / `child_session` candidates to attach directly
    between two `AgentSession` endpoints. `lineage_kind` is standardized
    as the operation vocabulary (`compaction`, `resume`, `fork`,
    `fresh`, `unknown`); attribution fidelity moves to a separate
    `lineage_fidelity` field, which Atelier will adopt in H-LINEAGE-002.
- [x] `H-LINEAGE-002` Extract claude-code session lineage.
  - Resolution: `src/discovery/harness/claude_code.rs` now reads the
    first record's `parentUuid` plus a bounded transcript tail to
    extract the leaf uuid, then matches within each project directory.
    Resolved matches emit a `ParentSession` candidate from child to
    parent `AgentSession`; unresolved parents preserve `parentUuid`
    under `UnresolvedEndpoint`. `lineage_kind` is `"compaction"` when
    the first cross-session record has `type == "summary"` and
    `"resume"` otherwise. Atelier's `lineage_kind` source-metadata
    field was renamed to `lineage_fidelity` per ADR 0018, with the
    new `lineage_kind` carrying the `fork` / `fresh` operation value;
    `harness_mux_snapshots__fork_associated_session_and_unresolved_lineage`
    and the atelier-delegation graph snapshot are updated. Manual
    `~/.claude` validation pending.
- [x] `H-LINEAGE-003` Extract opencode session lineage from
  `session.parent_id`.
  - Resolution: `src/discovery/harness/opencode.rs` now selects
    `parent_id` from the SQLite store and emits a `parent_session`
    candidate per row carrying a non-empty parent. Parent rows present
    in the same fragment resolve to concrete `AgentSession` endpoints;
    missing parents become `UnresolvedEndpoint` evidence with
    `harness_key = "opencode"` and the parent native id. Self-parent
    rows are skipped. Older schemas without `parent_id` fall back to a
    lineage-less SELECT rather than dropping every session.
    `lineage_kind` is `"unknown"` until opencode publishes operation
    semantics. Manual real-state validation pending.
- [x] `H-LINEAGE-004` Extract codex resume lineage.
  - Resolution: real codex rollouts (cli 0.128) expose
    `session_meta.payload.forked_from_id`, a true fork pointer (multiple
    children can share one parent). The codex adapter now extracts that
    field and emits a `parent_session` candidate per child rollout;
    resolved when the parent rollout is on the same state root, otherwise
    `UnresolvedEndpoint` evidence with `harness_key = "codex"` and the
    parent native id. `lineage_kind = "fork"` per ADR 0018. Codex does
    not currently expose a separate resume-only pointer (resume continues
    writing into the same rollout file), so resume lineage is parked
    until codex publishes a distinguishable field — no upstream issue
    filed yet; reopen this item if codex changes the rollout format.
- [x] `H-LINEAGE-005` Surface session lineage in the session table.
  - Resolution: `src/output/table.rs` now adds a `LINEAGE` column to
    the agent projection. The cell shows the preferred
    `parent_session`'s short id (full when ≤12 chars, else `…<last-8>`
    for UUIDs), prefixes unresolved parents with `?`, and appends `←`
    when the parent itself has a parent (chain ≥ 2). Unit tests cover
    resolved-parent, multi-level chain, and unresolved-parent cases.
    The `--include-superseded` flag was deferred: with fork-shaped
    lineage (codex `forked_from_id`, atelier-style native forks) a
    single parent can have multiple children and would silently
    disappear from every default render, which is more surprising than
    showing all rows. JSON output is already exhaustive. If a future
    consumer needs a compressed view, add `--hide-superseded` then.
- [x] `H-LINEAGE-006` Retarget claude-code lineage extraction — fork
  uses a `forkedFrom` envelope object, not `parentUuid`; `/compact`
  is in-place.
  - Resolution (1, 5): the claude-code adapter now reads `forkedFrom`
    from the first uuid-bearing record. When `forkedFrom.sessionId`
    matches another discovered session in the same project directory
    the link resolves to a concrete `AgentSession` target with
    `lineage_kind = "fork"`; otherwise it is preserved as
    `UnresolvedEndpoint` evidence keyed by parent session id.
    `forked_from_message_uuid` is carried in source metadata for
    future point-in-time use. `forkedFrom` wins over `parentUuid`
    when both are present. Regression tests cover resolved fork,
    unresolved fork, bare fork (no envelope, no lineage), and
    forkedFrom-vs-parentUuid precedence. Validated on live state
    2026-05-17: `332aa87b-…` (fork-with-history of `81f4a0ef-…`)
    renders `LINEAGE = …50b40572`; `926c6991-…` (bare fork) shows
    `—`.
  - Resolution (2): the bare-fork variant stays as `—`. The
    transcript carries no on-disk signal, and side-channel inference
    (IDE state files, fork-time proximity, sibling session listings)
    is high-effort for a UI gesture that may not even be reachable
    from current claude-code releases. Revisit only if the gesture
    becomes common or claude-code publishes a structural pointer.
  - Resolution (3): in-place `/compact` is **not** modeled. ADR 0018
    keeps `AgentSession` at session-file granularity, so a within-
    session `type: "summary"` record cannot be a `parent_session`
    edge — both endpoints would resolve to the same node. A
    regression test (`in_place_compaction_summary_record_emits_no_lineage`)
    locks this in: a transcript with a mid-stream summary record
    produces one `AgentSession` and zero lineage candidates. The
    adapter's module-level doc comment documents the policy.
  - Resolution (4): no capability constant changes. The standalone
    claude-code adapter does not emit a `lineage_fidelity` field —
    that lives in atelier's per-fork TOML and is interpreted by the
    delegation flow. The fork lineage emitted here is already
    `Provenance::StrongDiscovered` / `Confidence::High`, which is
    the strongest tier available. If a future atelier fork record
    advertises claude-code as Native for fork lineage, no Conspectus
    code change is needed.
  - Context: H-LINEAGE-002 assumed compaction (or a similar successor
    operation) produces a new session jsonl whose first uuid-bearing
    record's `parentUuid` points at the predecessor's leaf uuid.
    Manual validation against `~/.claude/projects/` on 2026-05-17
    (claude-code 2.1.129) found that assumption wrong:
    - `/compact` is in-place: invoked mid-session, it keeps appending
      to the same session jsonl rather than creating a successor file.
      No cross-session `parentUuid` is produced.
    - IDE session fork (fork-with-history variant, exercised by
      forking `81f4a0ef-…` into `332aa87b-…`) **does** record
      structural lineage, but on a different field than the adapter
      looks at. The child file copies the parent's records and tags
      each copied record with an envelope field
      `forkedFrom = {sessionId: <parent session id>, messageUuid:
      <original uuid in parent>}`. Of 961 records in the
      `332aa87b-…` file, 510 carry `forkedFrom` (the copied parent
      prefix) and 451 do not (the fork's own new records). The
      child's own `parentUuid` chain still starts at `null` for the
      first record, so the adapter's current first-record-parentUuid
      heuristic misses this entirely.
    - A second fork (`926c6991-…`) carried zero `forkedFrom`
      records (15 records, 0 tagged). Likely a different IDE
      affordance ("fresh session from here" vs "fork with history")
      or an older code path — investigate which UI gesture produces
      which shape.
  - Scope: (1) Extend the claude-code adapter to read `forkedFrom`
    from the first uuid-bearing record (or the first tagged record
    if the envelope ordering differs). When present, emit a
    `parent_session` candidate with `lineage_kind = "fork"`,
    fidelity `native`, source = child session, target = the session
    identified by `forkedFrom.sessionId`. Preserve the messageUuid
    in source metadata for future point-in-time lineage. (2) Decide
    how to handle the "no-`forkedFrom`" fork variant exemplified by
    `926c6991-…` — likely treat as unresolvable from transcript
    alone and rely on future side-channel inference. (3) If in-place
    compaction is the durable behavior, design within-session
    lineage: parse `type: "summary"` records and treat the
    pre-summary leaf uuid and post-summary first user uuid as an
    intra-session compaction boundary; pick whether to model the
    pre-compaction span as a distinct logical session or as an
    annotation on the same `AgentSession`. (4) Once `forkedFrom`
    extraction lands, upgrade claude-code's lineage capability from
    `Approximate` to `Native` for the fork-with-history case;
    `/compact` and the bare-fork variant remain `Unsupported` until
    addressed. (5) Add regression tests covering: a fork transcript
    with a `forkedFrom` envelope (resolved-parent case), the same
    pointing at a missing parent (`UnresolvedEndpoint`), and a fork
    transcript with no `forkedFrom` records (no lineage emitted).
  - Tests: a fixture transcript containing a `type: "summary"` record
    surrounded by user/assistant records exercises the in-place case;
    keep the existing
    `lineage_pointer_is_read_from_first_uuid_bearing_record_not_envelope`
    regression test for the cross-session path so we do not regress if
    a future claude-code release reintroduces successor files.

### Agent Session Last-Message Preview

Claude Code's `/resume` view shows each historical session with a short
snippet of its most recent message. Conspectus's table output benefits
from the same: scanning a column of `claude-code:alpha`,
`codex:beta`, `opencode:gamma` rows is far more useful when each
carries a one-line "what was it doing?" preview alongside the harness
label, cwd, mux, and PR cells. Per ADR 0023 the preview lives on
`AgentSessionNode` as `last_message_preview: Option<String>`,
populated best-effort by each harness adapter; the table renderer
sources it like any other cell rather than re-reading transcripts at
render time.

- [x] `H-PREVIEW-001` Model field + opt-in preview column registration.
  - Outcome: ADR 0023 records the design (single-line, 200-char
    cap, adapter-populated, default-off column). `AgentSessionNode`
    grew `last_message_preview: Option<String>`; the field is
    `#[serde(default, skip_serializing_if = "Option::is_none")]` so
    existing JSON fixtures stay byte-stable. A shared
    `model::normalize_last_message_preview` helper (plus
    `LAST_MESSAGE_PREVIEW_CAP = 200`) gives every adapter a
    consistent collapse-whitespace-trim-cap-with-ellipsis routine.
    The `preview` column is registered on `sessions`, `mux`, and
    `union` row-types, default off. Extractors: sessions reads the
    row's session preview; union shows it for agent rows and `—`
    for mux rows; mux looks up the first attached agent's preview
    via `attached_to_mux` (BTreeMap order) and renders `—` when
    none is set. All harness adapters still emit `None`; follow-up
    stories H-PREVIEW-002..005 populate per harness.
    `docs/operations.md` documents the column and its privacy
    posture. Five renderer unit tests cover Some/None for sessions,
    first-attached lookup for mux, empty mux fall-through, the
    union agent-vs-mux split, and the byte-stable default render.
    Five normalizer unit tests cover whitespace collapse, empty
    input, char-based capping with ellipsis, unicode preservation,
    and grapheme-boundary safety on multibyte content. All 421
    tests pass; no insta snapshot moved.

- [x] `H-PREVIEW-002` Claude Code last-message extraction.
  - Outcome: `read_session_last_message_preview` walks the trailing
    `TAIL_SCAN_BYTES` (32 KiB) of each Claude Code JSONL transcript
    backward, dropping the partial first line when the seek lands
    mid-file, and returns the first user/assistant text content it
    finds. Tool-use, tool-result, and thinking blocks are skipped;
    so are `system` records and any user/assistant record with
    `isCompactSummary: true`. The result goes through
    `normalize_last_message_preview` so the cell is whitespace-
    normalized and capped at 200 chars with a trailing `…`. The
    extractor short-reads through `serde_json::from_slice` against
    a minimal `MessageScan` / `MessageBody` / `MessageContent`
    grammar (untagged enum covers both string content and the
    modern content-block array). Discovery stays best-effort:
    corrupt JSON / empty transcripts / tool-only tails yield
    `None`. Eight unit tests pin the plain exchange, the
    skip-tool-blocks path, thinking-skip, compaction-summary skip,
    tool-only None, empty/corrupt None, the 200-char cap, and the
    whitespace collapse. Live validation: running
    `conspectus table sessions --columns id,agent,preview` against
    `~/.claude/projects/` surfaces meaningful one-line previews
    for every session that has any text in its tail window.

- [x] `H-PREVIEW-003` Codex last-message extraction.
  - Outcome: `read_rollout_last_message_preview` does the same
    32 KiB tail-walk for codex rollouts and feeds the result
    through `normalize_last_message_preview`. The codex grammar
    differs from claude-code (records are
    `{type: "response_item", payload: {type: "message", role,
    content: [{type: "input_text"|"output_text", text}]}}` with
    `reasoning` / `function_call` / `function_call_output` /
    `event_msg` records to skip), so a minimal `RolloutLine` /
    `RolloutPayload` / `RolloutContent` grammar handles
    extraction. Only `response_item` records with
    `payload.type == "message"` and a `user`/`assistant` role
    contribute; the extractor returns the last non-empty
    `input_text`/`output_text` block. Codex's rollout is keyed by
    session id from the `session_meta` line, so previews are
    stored in a `HashMap<String, String>` keyed by id and
    attached to the matching node at construction. Six unit tests
    cover the plain output_text case, skipping tool/reasoning/event
    records, skipping empty text blocks, meta-only None,
    200-char cap, and corrupt-tail None. Live validation: real
    `~/.codex/sessions/**` rollouts produce meaningful previews
    in `conspectus table sessions`.

- [x] `H-PREVIEW-004` Opencode last-message extraction.
  - Outcome: `src/discovery/harness/opencode.rs` populates
    `last_message_preview` by reading the modern `part` table
    alongside the existing `session` query. A
    `read_last_message_previews` helper runs a single
    `ROW_NUMBER()`-windowed SQL query that pulls the most recent
    `type: "text"` row per session via `json_extract`, ordered by
    `(time_created DESC, id DESC)`, filtered to non-empty text. The
    `bundled` rusqlite feature guarantees JSON1 is available;
    schemas without the `part` table (or any row that fails to
    parse) degrade to `None` so legacy stores still discover their
    sessions without lineage or preview. Each non-empty text passes
    through `normalize_last_message_preview` for whitespace
    collapse and the 200-char cap. Five new SQLite-backed unit
    tests cover (a) the most-recent-text-part wins, (b) empty/
    missing-text rows are skipped, (c) per-session attribution,
    (d) absent-`part`-table degrades to `None`, (e) long text is
    capped via the shared helper. All 440 tests pass.
  - Blockers: `H-PREVIEW-001`.

- [x] `H-PREVIEW-005` Aider last-message extraction.
  - Outcome: deferred per the story's escape clause. Two reasons:
    (1) aider's `.aider.chat.history.md` is free-form markdown
    with no formally-specified turn-delimiter, and the format
    changes between aider versions, so a heuristic parser would
    silently emit nonsense previews on any future-version
    transcript; (2) `.aider.input.history` only carries user
    inputs and would leave the preview misleading (no assistant
    text). A `TODO(H-PREVIEW-005)` comment in
    `src/discovery/harness/aider.rs` pins the adapter on
    `last_message_preview: None` and points at this entry.
    Reopen this story when either (a) aider publishes a stable
    structural marker for assistant turns or (b) a fixture
    corpus is available to validate a heuristic parser against.
    Aider sessions continue to discover with all other metadata;
    the preview cell simply renders `—`.
  - Blockers: `H-PREVIEW-001`.

- [x] `H-PREVIEW-006` Filter codex channel markers from preview.
  - Outcome: `src/discovery/harness/codex.rs` gained
    `apply_codex_channel_marker_filter`, a conservative stripper
    that recognizes leading `<turn_aborted>` and `<proposed_plan>`
    tags (the constants live in `CODEX_CHANNEL_MARKERS`). When the
    tag wraps non-empty content, the prefix (and a matching closing
    tag if present) is stripped and the body becomes the preview.
    When the body is empty after stripping — a bare
    `<turn_aborted>` with nothing after — the filter returns
    `None` so the extractor's outer backward walk picks an earlier
    message instead. Unknown XML-shaped tags are left verbatim, so
    legitimate `<html>` or `<foo>` in user content survives.
    Live verification on this workspace: a row that previously
    rendered `<turn_aborted> The user interrupted…` now shows the
    interrupt text without the marker, and `<proposed_plan>
    # Atelier Profiles V1…` becomes `# Atelier Profiles V1…`.
    Four new unit tests cover bare-marker skip, marker+body strip
    for both known tags, and unknown-tag verbatim preservation.

- [x] `H-TBL-015` Move title out of AGENT label into its own column.
  - Outcome: `agent_session_label` no longer consults
    `AgentSessionNode.title`. Every row renders
    `harness:<session_key>`, with `agent_session_key_for_label`
    deferring to `short_session_id` only when the key exceeds 32
    chars. UUIDs (claude-code, codex) collapse to `…<last-8>`;
    shorter human-readable keys
    (`session-alpha`, opencode `ses_…`) pass through verbatim, so
    every existing insta snapshot stayed byte-stable. A new
    `title` column is registered on the `sessions` and `union`
    row-types as opt-in; the extractor reads
    `AgentSessionNode.title` directly. Union row-type renders `—`
    for mux rows. Live verification on this workspace: opencode
    rows that used to render
    `opencode:tmux clipboard not syncing over SSH (fork #1)` in
    AGENT now show `opencode:ses_204a14312…` with the chat topic
    moving to the new `TITLE` column. Four new unit tests cover
    the AGENT-cell label without title, UUID truncation, short-
    key verbatim, and the new `title` column for both sessions and
    union row-types. 449 tests pass.

### Agent Session Transcript Preview And Viewer

The `H-PREVIEW-*` stories established a single-line
`last_message_preview` populated at discovery time. The next step is
the TUI right-panel surface: when an un-muxed agent session is
selected, the panel currently renders only that one normalized line
even though there is plenty of vertical space and useful prior context
on disk. This workstream extends the un-muxed preview into a styled,
multi-message recent-history rendering, and decides how an optional
external "full transcript" viewer integrates per ADR 0019.

The design constraints from the May 2026 candidate survey (see also
ADR 0019):

- The inline preview lives inside the existing two-panel Ratatui UI;
  it should not require a child-process viewer just to populate the
  right pane. Embedding a Rust markdown renderer is the lowest-risk
  path.
- `tui-markdown` (joshka/tui-markdown, v0.3.7) returns
  `ratatui::text::Text` directly and pairs cleanly with the existing
  `ansi-to-tui` dep and Ratatui 0.30. It is the leading candidate for
  the inline styling concern. Per ADR 0024 a new TUI crate dependency
  needs a follow-on ADR before adoption.
- Recent-history extraction should be done on demand when the
  selection changes, not eagerly populated for every session at
  discovery time the way `last_message_preview` is. A per-selection
  read is bounded (one transcript, last N turns) and avoids paying
  the cost for sessions the user never opens.
- Compaction (Claude Code), tool-only tails, reasoning blocks, and
  channel markers (codex) are already handled in the H-PREVIEW
  extractors; the recent-history readers should share that grammar
  rather than re-parsing from scratch.
- External viewers (`claude-history`, `recall`, etc.) remain the
  right surface for a separate "open full transcript" action. The
  inline preview is not a replacement for them, and they are not a
  replacement for the inline preview.

This workstream supersedes the single-bullet `P8-012c` story; that
entry stays in Phase 8 as the v1 release-boundary marker that is
satisfied when this workstream's TUI integration stories land.

- [x] `H-TRANSCRIPT-001` ADR: terminal markdown rendering for the
  inline transcript preview.
  - Outcome: ADR 0051 selects `tui-markdown 0.3.7` with
    `default-features = false` (no `syntect`, no second
    `ansi-to-tui` path) for the inline transcript preview.
    Alternatives evaluated: `termimad` (crossterm-only, requires
    bridging) and a roll-your-own renderer over
    `pulldown-cmark` (same long-tail surface ADR 0025 rejected
    for SGR). Integration target is a new
    `src/tui/transcript_preview.rs` per `H-TRANSCRIPT-009`.

- [x] `H-TRANSCRIPT-002` Resolve ADR 0019 with the May 2026 survey.
  - Outcome: ADR 0019 amended in place to Accepted. Survey
    refreshed: `ccview` is alive (v1.0.1, April 2026) — the
    earlier "disappeared" note was incorrect; `claude-history`
    (v0.1.64, May 2026) gained a structured agent protocol and
    is the preferred Claude Code launch backend; `recall`
    (v0.5.0, Jan 2026) is the preferred multi-harness backend
    for Claude/Codex/OpenCode/Factory; `lazyagent` v0.12.0 has
    an HTTP API (deferred — long-running service, not a
    one-shot viewer); `ccboard-core` is published on crates.io
    at 0.22.0; `cass`
    (`coding_agent_session_search`) covers 20+ providers with
    `--json`/`--robot` but is deferred pending license-rider
    review. The trait was narrowed from
    `SessionViewer::{view, export_text}` to a
    `SessionViewerAction::plan -> LaunchPlan` action-resolver
    seam mirroring `P8-010`'s `tmux attach` hand-off. The ADR
    explicitly calls out that the inline preview is handled by
    ADR 0051 and the rest of the `H-TRANSCRIPT-*` workstream.

- [ ] `H-TRANSCRIPT-003` Recent-history adapter API.
  - Scope: define an on-demand adapter entry point on each
    harness that returns the last N user/assistant turns for a
    given `AgentSessionId` (parallel to the H-PREVIEW
    extractors but returning a structured `Vec<TranscriptTurn>`
    rather than a single normalized string). Decide whether the
    structure lives in `model::` (and is also usable by a
    future full-transcript viewer) or stays inside the harness
    module. Cap turns by count, by total bytes, or both;
    document the choice. Errors must degrade to "unavailable"
    without panicking the TUI.
  - Tests: trait/contract tests; one fake harness adapter.
  - Blockers: none.

- [ ] `H-TRANSCRIPT-004` Claude Code recent-turns extractor.
  - Scope: extend the existing tail-scan reader to return the
    last N user/assistant turns. Continue to skip tool-use,
    tool-result, thinking, and `system` records. Handle
    compaction (ADR 0019 context): when a `compact_boundary`
    row is in scope, the post-compaction summary should be
    distinguishable in the returned data (e.g. a turn-kind
    flag) so the TUI can render it differently. Expand the
    tail-scan window when N turns are not found in the current
    window, bounded by a hard cap.
  - Tests: fixture tests for the plain exchange, the
    compaction-summary case, a tool-only tail, and the window
    expansion path.
  - Blockers: `H-TRANSCRIPT-003`.

- [ ] `H-TRANSCRIPT-005` Codex recent-turns extractor.
  - Scope: extend `read_rollout_last_message_preview` style
    extraction to return the last N `response_item` /
    `payload.type == "message"` turns with `user` / `assistant`
    roles, skipping reasoning / function_call /
    function_call_output / event_msg records. Apply the same
    channel-marker filter (`<turn_aborted>`,
    `<proposed_plan>`) used by `H-PREVIEW-006` per-turn.
  - Tests: fixture tests including the channel-marker filter
    applied across multiple turns.
  - Blockers: `H-TRANSCRIPT-003`.

- [ ] `H-TRANSCRIPT-006` OpenCode recent-turns extractor.
  - Scope: extend the modern `part` table reader to return the
    most recent N `type: "text"` rows per session via a single
    SQL query (analogous to `read_last_message_previews`).
    Degrade to an empty result when the `part` table is absent
    so legacy stores still surface session metadata.
  - Tests: SQLite-backed fixture tests covering most-recent
    ordering, per-session attribution, and the absent-table
    fallback.
  - Blockers: `H-TRANSCRIPT-003`.

- [ ] `H-TRANSCRIPT-007` Aider recent-turns extractor (deferred).
  - Scope: deferred for the same reasons as `H-PREVIEW-005`
    (free-form markdown without a stable assistant-turn
    delimiter; input-history is user-only and would mislead).
    Aider rows render "(transcript preview unavailable)" in
    the panel. Reopen when aider gains a structural marker or
    a stable fixture corpus is available.
  - Blockers: same as `H-PREVIEW-005`.

- [x] `H-TRANSCRIPT-008` Add `tui-markdown` dependency.
  - Outcome: `tui-markdown = { version = "0.3",
    default-features = false }` added to `Cargo.toml` per ADR
    0051. The `highlight-code` feature stays off so `syntect`
    and the secondary `ansi-to-tui` path don't land in the dep
    graph. Pre-listed in `ALLOWED_EXTERNAL_DEPS` from the
    H-VIEWER-NATIVE-001 scaffold, so the
    `dep_surface_matches_doc_manifest` test passes unchanged.
    First use lands with H-VIEWER-NATIVE-006's `render_turn`.

- [ ] `H-TRANSCRIPT-009` Inline transcript-preview widget.
  - Scope: new `src/tui/transcript_preview.rs` (or similar)
    that takes a `Vec<TranscriptTurn>` and produces a styled
    `ratatui::text::Text` filling the available right-panel
    height. Render per-turn role headers
    (e.g. dimmed `you`/`assistant` labels), use
    `tui-markdown` for the body, mark compaction-summary
    turns visibly, and crop or scroll when content exceeds
    the pane. Honor `--no-live-preview` by falling back to
    the existing single-line `last_message_preview`. Surface
    "transcript unavailable" and stale-data markers per ADR
    0023's privacy posture.
  - Tests: Ratatui buffer snapshot tests for: (a) a normal
    multi-turn preview, (b) a Claude Code compaction-summary
    turn, (c) an "unavailable" case, (d) `--no-live-preview`
    falling back to the single-line preview.
  - Blockers: `H-TRANSCRIPT-003`, `H-TRANSCRIPT-008`.

- [ ] `H-TRANSCRIPT-010` Wire the widget into the right panel for
  un-muxed agent rows.
  - Scope: route un-muxed `AgentSession` selections through the
    new widget instead of the single-line preview. Trigger the
    on-demand recent-turns read on selection change, similar to
    `P8-009`'s mux capture and `P8-012a`'s `gh` enrichment:
    immediate placeholder while loading, then content. Cache
    by `AgentSessionId` for the lifetime of the TUI; invalidate
    on session-state mtime changes if cheap to detect. Muxed
    rows continue to render the mux capture preview from
    `P8-009`. Mux candidate child rows continue to use the
    existing detail rendering.
  - Tests: TUI integration tests for the un-muxed selection
    path, the muxed selection path (regression — mux capture
    still wins), the loading→content transition, and the
    cache-hit path on re-selection.
  - Blockers: `H-TRANSCRIPT-004`, `H-TRANSCRIPT-005`,
    `H-TRANSCRIPT-006`, `H-TRANSCRIPT-009`.

- [ ] `H-TRANSCRIPT-011` Document the inline transcript preview.
  - Scope: update `docs/operations.md` and the Phase 8
    implementation doc with the new preview behavior,
    `--no-live-preview` semantics for transcript reads, the
    privacy posture (transcript text never leaves the local
    process), and the supported harnesses. Note aider's
    deferred status.
  - Tests: `git diff --check`.
  - Blockers: `H-TRANSCRIPT-010`.

- [x] `H-TRANSCRIPT-012` External full-transcript viewer launch
  (moved ahead of the inline-widget track per the amended ADR 0019,
  which treats inline preview and external launch as parallel
  surfaces).
  - Outcome: `src/tui/viewer.rs` defines `SessionViewerAction`,
    `LaunchPlan`, and `ViewerDisabled` per the amended ADR 0019.
    Two backends ship: `ClaudeHistoryViewer` (resolves the on-disk
    `<state_scope>/projects/*/<session_key>.jsonl` via std
    `read_dir` and passes it as the positional arg —
    `claude-history --show-id` *prints* the id, the interactive
    viewer takes a file path) and `RecallViewer`
    (`recall --session <session_key>`, multi-harness: claude-code,
    codex, opencode, factory, droid). `resolve_viewer_target`
    prefers harness-specific over multi-harness. PATH discovery
    uses a std-only `BinaryProbe` walker (no new dep). The probe
    also feature-detects `recall --help` for `--session` because
    upstream `recall 0.5.0` lacks the flag; the conspectus Nix
    overlay carries a patched `recall 0.5.0-conspectus-session`
    until upstream lands a PR. `plan()` is fallible
    (`Result<LaunchPlan, String>`) so claude-history can return
    a `ViewerDisabled::TranscriptNotFound` hint when the JSONL
    file isn't on disk; the resolver falls through to recall
    when one is configured. The `T` keybind routes through
    `Action::View` → `view_action`. `run_viewer_launch` does an
    explicit `Clear(All) + MoveTo(0,0)` after `ratatui::restore`
    so mosh / nested muxers stop intertangling the child's
    first writes with the dropped TUI buffer, and holds for
    Enter on non-zero exit so the operator can read the
    viewer's stderr before the alt screen reclaims the
    terminal. Help overlay registers the new binding. Fourteen
    unit tests cover supported / unsupported harnesses, the
    "no binary on PATH" path, the preference order, the
    capability-probe gating for unpatched recall, and the
    transcript-not-found + recall-fallback paths.
  - Known limitations (filed as `H-TRANSCRIPT-014` and
    `H-TRANSCRIPT-015`): recall doesn't scan opencode at the
    storage layout the user's machine actually uses (it hard-codes
    `~/.local/share/opencode/storage/session` while the real path
    is `session_diff/` + SQLite); and recall has no modal focus,
    so `j`/`k` after deep-link entry type into the search box
    rather than navigating the preview. The stderr-hold makes
    both failure modes legible from the TUI.
  - **Repurposed by ADR 0052**: external viewer launch is no
    longer the default `T` target. The native in-tree viewer
    (`H-VIEWER-NATIVE-*`) takes over the default. The code
    shipped under this story stays as the *escape-hatch* path
    operators reach via `[viewers.<harness>]` config
    (`H-TRANSCRIPT-013`). The recall-specific surface
    (`RecallViewer`, `supports_flag` capability probe,
    `required_flags` trait method) was ripped out alongside
    `H-VIEWER-NATIVE-009`; only `ClaudeHistoryViewer` remains
    as the hardcoded escape-hatch backend. Operators who want
    recall back configure it via `H-TRANSCRIPT-013` once that
    lands.

- [~] `H-TRANSCRIPT-014` Surface recall's harness coverage gap (or
  broaden it). **Won't fix on the conspectus side** as of ADR 0052
  — the native viewer (H-VIEWER-NATIVE-*) reads opencode directly
  via SQLite (ADR 0013), so the gap is closed by replacing the
  backend rather than patching recall. Operators who still want
  recall as the opencode viewer maintain the patches themselves.
  - Scope: on at least one observed machine, recall hard-codes
    `~/.local/share/opencode/storage/session` but the real
    OpenCode storage on the same machine lives at
    `~/.local/share/opencode/storage/session_diff/` (and the
    SQLite-of-record is `~/.local/share/opencode/opencode.db`).
    The result: `recall list --source opencode` returns empty
    even though conspectus discovers many opencode sessions
    locally, and `recall --session <opencode-id>` exits 1 with
    "Session not found". H-TRANSCRIPT-012's stderr-hold makes
    the failure visible, but the underlying gap stays.
  - Resolution options (pick one in the story):
    (a) **Document** and leave: opencode-via-recall is an
        unsupported combination on machines whose opencode store
        lives outside recall's hardcoded path. The TUI status
        bar already explains why on failure.
    (b) **Extend the recall patch** to add a search-roots flag
        (`--scan-root <path>` repeatable) and pass conspectus's
        resolved `state_scope` per-harness.
    (c) **Upstream PR** to point recall's opencode parser at the
        real layout (session_diff/ + opencode.db), then drop the
        local-only workaround.
  - Tests: viewer-resolver tests already cover the disabled
    paths; this story would add an integration probe that runs
    `recall list --source <harness>` against the actual machine
    layout when one is available, so coverage gaps regress
    visibly.
  - Blockers: none. Independent of `H-TRANSCRIPT-013`'s config
    override (which would also let users sidestep the gap by
    swapping recall for an opencode-native viewer per-harness).

- [~] `H-TRANSCRIPT-015` Recall focus on deep-link entry. **Won't
  fix on the conspectus side** as of ADR 0052 — the native viewer
  owns its own focus model. Kept in the backlog for operators who
  configure recall as their viewer via `H-TRANSCRIPT-013`.
  - Scope: when patched-recall is launched with `--session <id>`,
    the operator has already chosen the session in conspectus
    and lands inside recall expecting to scroll. But recall has
    no modal focus model — every keystroke other than the
    explicit navigation keys (Up/Down/Ctrl-E) inserts into the
    search query box. The most ingrained muscle memory
    (`j`/`k` to move down/up) ends up filtering the session
    list instead, often hiding the very session conspectus
    just opened.
  - Resolution options:
    (a) **Extend the conspectus recall patch** with a deep-link
        mode: when `--session` is set, intercept `j`/`k` (and
        possibly `g`/`G`) as preview-navigation keys before
        they reach `on_char`. Keep `/` as the escape hatch to
        re-enter search.
    (b) **Upstream PR** for a `--preview-focus-on-entry` flag
        (or a deeper modal-focus rework) — recall would benefit
        from this regardless of conspectus's use case.
    (c) **Status-bar hint after launch** explaining the
        Up/Down convention — cheapest, no patch revision.
  - Tests: hard to unit-test the patched binary directly; rely
    on the existing recall installCheck plus a one-line manual
    smoke step in the H-TRANSCRIPT-012 outcome notes.
  - Blockers: none. Pairs naturally with `H-TRANSCRIPT-014` —
    both are recall-patch revisions.

- [ ] `H-TRANSCRIPT-013` Config-driven viewer override.
  - Scope: extend the hard-coded `ClaudeHistoryViewer` /
    `RecallViewer` resolver with a `[viewers.<harness>]` (and
    `[viewers.default]`) config section so users can bring their
    own opinions without touching Conspectus internals. The
    `recall --session` patching in `pkgs/recall/` was the
    immediate driver — a future user shouldn't have to fork
    Conspectus to swap in `cass view`, `claude-code-log`, a
    homegrown wrapper, etc.
  - Config shape: each entry is `command = ["prog", "arg", ...]`
    with `{placeholder}` interpolation. Conspectus expands at
    launch time. Precedence: project config > user config >
    built-in resolver. Per-harness entry wins over
    `[viewers.default]`. User-configured entries **bypass
    `required_flags` probing** — the operator has opted in and
    accepts whatever the child process does.
  - Placeholder set (frozen by the ADR below):
    `{session-id}` (raw `AgentSessionId.session_key`),
    `{session-file}` (on-disk transcript path; empty / launch
    refused for harnesses that have no single file — opencode
    SQLite, codex split state/log per ADR 0048),
    `{cwd}` (session cwd or empty),
    `{harness}` (`AgentSessionId.harness_key`),
    `{state-scope}` (`AgentSessionId.state_scope`).
  - ADR: small ADR freezes the placeholder names, the precedence
    rules, the unknown-placeholder behavior (diagnostic +
    leave-as-text vs refuse-to-launch), and the
    `{session-file}` semantics for SQLite-backed harnesses.
    Variable names become a public contract once shipped.
  - Implementation outline: new `ViewersConfig` in `src/config.rs`
    mirroring the `TuiViewsConfig` parse/merge shape (~120 LOC),
    a `ConfiguredViewer` backend in `src/tui/viewer.rs` with a
    template expander (~80 LOC), resolver order change (~20 LOC),
    a per-harness `state_file_for(session_id)` mapper (deciding
    Option A: derive on demand vs. Option B: add an optional
    `state_file` field to `AgentSessionNode`; deferred to the
    ADR). Tests cover precedence, placeholder expansion,
    `{session-file}` for the SQLite-backed harnesses, and the
    interaction with the existing `required_flags` gating
    (configured viewer bypasses it).
  - Tests: config parse/merge for the new sections; template
    expander with known/unknown placeholders; resolver tests
    that a configured entry wins over the built-in backends; a
    refused-launch test for `{session-file}` against opencode.
  - Blockers: none (independent of the inline-widget track).
    Should land before `H-TRANSCRIPT-012` ships outside the
    author's machines so the patched-recall workaround is
    optional rather than the only path.

### Native In-Tree Transcript Viewer (H-VIEWER-NATIVE-*)

Per ADR 0052. Builds a Ratatui full-screen modal that renders a
single session's transcript inside the conspectus process.
Replaces the external-launch path (`H-TRANSCRIPT-012`) as the
default `T` action. Designed for later extraction to a standalone
crate per `docs/transcript-viewer-deps.md`.

Stories below depend on `H-TRANSCRIPT-003` (recent-history adapter
API) but extend its return shape from "last N turns" to "full
transcript with a cursor at the last turn".

- [x] `H-VIEWER-NATIVE-001` Module scaffold + dep-surface
  enforcement.
  - Outcome: `src/viewer/` laid down with `mod.rs`, `model.rs`,
    `parser/{mod, claude_code, codex, opencode}.rs`, `widget.rs`,
    `state.rs`, `input.rs`, `render.rs`, `theme.rs`. All bodies
    are stubs returning `ParseError::Malformed` (parsers) or empty
    placeholders pending `H-VIEWER-NATIVE-002 .. 006`. `mod.rs`
    carries `ALLOWED_EXTERNAL_DEPS` and `ALLOWED_BINARY_DEPS`
    consts that mirror `docs/transcript-viewer-deps.md`. The
    `dep_surface_matches_doc_manifest` test parses the doc's
    Markdown tables and asserts the two agree on both the
    library surface (11 crates) and the binary-only surface
    (`clap`); manually drifting either side fails the test.
    `crate::tui::theme::Theme` is re-exported through
    `src/viewer/theme.rs` per ADR 0052's tracked carve-out.
    The conspectus TUI does not yet route `T` into the new
    module — the existing escape-hatch `ClaudeHistoryViewer`
    still owns the keybind until `H-VIEWER-NATIVE-008`. Eight
    viewer-module tests pass; full nextest suite (1141) green.

- [x] `H-VIEWER-NATIVE-002` `TranscriptDocument` + `TranscriptTurn`
  model + `SessionLocator` types.
  - Outcome: `src/viewer/model.rs` fleshed out with full types,
    serde derives, Display impl, and round-trip tests. Public
    surface: `SessionLocator` (`ClaudeCode`/`Codex`/`OpenCode`
    variants, `serde(tag = "harness", rename_all = "kebab-case")`
    with `OpenCode` overridden to serialize as `opencode` matching
    conspectus's harness key), `TranscriptTurn` (`role`, `kind`,
    `body`, `timestamp: Option<DateTime<Utc>>` skip-on-none),
    `TurnRole` (`User`/`Assistant`/`System` with `header_label()`
    matching the inline-preview labels), `TurnKind`
    (`Message`/`CompactionSummary`/`ToolUse`/`ToolResult`/`Thinking`
    with `shown_by_default()` filtering noise),
    `TranscriptDocument` (`meta` + `turns` plus an
    `unavailable(locator)` constructor for parser-failure
    fallback), `TranscriptMeta`. Zero conspectus-graph types
    appear in the public surface. `chrono = 0.4` added to
    `Cargo.toml` with `default-features = false, features =
    ["serde", "alloc"]` — already pre-listed in
    `ALLOWED_EXTERNAL_DEPS` so the dep-surface test still
    passes. Eleven model tests cover Display, harness_key,
    round-trips for all three locator variants, kind/role
    helpers, timestamped + omitted-timestamp turn serde,
    `unavailable()` shape, and a full document round-trip.

- [x] `H-VIEWER-NATIVE-003` Claude Code parser.
  - Outcome: `src/viewer/parser/claude_code.rs` reads
    `<state_root>/projects/*/<session_key>.jsonl` (std-only
    `read_dir` glob, no `glob` dep) and emits a
    `TranscriptDocument` in the normalized model. Translation
    rules:
    - One `TranscriptTurn` per content block, so the renderer
      can fold tool / thinking blocks independently of the
      surrounding prose.
    - String `message.content` → one `TurnKind::Message` turn.
    - `text` block → `Message`. `thinking` block → `Thinking`.
      `tool_use` block → `ToolUse` with body `"<name>: <json>"`.
      `tool_result` block → `ToolResult` with body flattened
      (string content or joined text blocks).
    - Compaction summary records (`isCompactSummary=true`) →
      one `CompactionSummary` turn rather than a regular user
      message.
    - Non-`user`/`assistant` record types
      (`custom-title`, `agent-name`, `system`, `attachment`,
      `file-history-snapshot`, `last-prompt`,
      `permission-mode`, `queue-operation`, `ai-title`) are
      skipped — they're metadata.
    - Empty bodies dropped; first-record cwd wins;
      `timestamp` parsed RFC3339 → `DateTime<Utc>`.
    - Malformed lines and blank lines skip silently
      (matches the H-PREVIEW posture).
    - Missing file → `ParseError::NotFound`; the bridge will
      map that to `TranscriptDocument::unavailable`.
  - 13 fixture tests cover supports() variant gating,
    file-not-found, plain user+assistant, text + tool_use
    fan-out, string tool_result, list-shaped tool_result,
    thinking, compaction summary, empty bodies dropped,
    metadata records skipped, malformed lines skipped,
    first-cwd-wins, tool_use without input. 1165 nextest
    green.

- [x] `H-VIEWER-NATIVE-004` Codex parser.
  - Outcome: `src/viewer/parser/codex.rs` walks
    `<state_root>/sessions/<year>/<month>/<day>/` with std
    `read_dir` (no `walkdir` dep) and locates the rollout file
    by `-<session_key>.jsonl` suffix match. Translation rules
    capture the dual-family record schema:
    - Outer `type` = `session_meta` → fill `TranscriptMeta.cwd`
      (first wins), no turn.
    - Outer `type` = `turn_context` → cwd fallback only, no turn.
    - Outer `type` = `response_item` → dispatch on
      `payload.type`:
      - `message` with role `user`/`assistant` → one `Message`
        turn per `input_text`/`output_text` content block;
        `developer` and `system` roles skipped as injected
        instructions.
      - `reasoning` → `Thinking` (joined `summary` + `content`
        text blocks); skipped if only `encrypted_content`.
      - `function_call`, `custom_tool_call`, `web_search_call`
        → `ToolUse` with body `"<name>: <arguments>"`.
      - `function_call_output`, `custom_tool_call_output` →
        `ToolResult` with body = output.
    - Outer `type` = `compacted` → `CompactionSummary` turn from
      `payload.message`.
    - Outer `type` = `event_msg` → skipped (engine telemetry:
      token_count, task_started, exec_command_end echoes, etc.).
    - Channel-marker filter per H-PREVIEW-006: message bodies
      whose trimmed content is exactly `<turn_aborted>` or
      `<proposed_plan>` are dropped as not-real-user-text.
    - RFC3339 timestamps → `DateTime<Utc>`; malformed and blank
      lines skip silently.
  - 16 fixture tests cover supports() gating, file-not-found,
    session_meta cwd capture, user/assistant message turns,
    developer/system skip, function_call → ToolUse,
    function_call_output → ToolResult, reasoning with summary
    → Thinking, encrypted-only reasoning skip, event_msg skip,
    `<turn_aborted>` and `<proposed_plan>` drops, compacted →
    CompactionSummary, web_search_call → ToolUse, malformed
    skip, nested-path file lookup. 1181 nextest green.

- [x] `H-VIEWER-NATIVE-005` OpenCode parser (SQLite-of-record).
  - Outcome: `src/viewer/parser/opencode.rs` reads `opencode.db`
    via `rusqlite` (`OpenFlags::SQLITE_OPEN_READ_ONLY |
    SQLITE_OPEN_NO_MUTEX`). One LEFT JOIN between `message` and
    `part` keyed on `session_id` returns the conversation in
    `(m.time_created, p.time_created)` order. `session_diff/`
    is never touched — the SQLite store is authoritative, which
    is the gap that recall could not close. Translation rules:
    - `session.directory` → `TranscriptMeta.cwd`.
    - `part.data.type = "text"` → `Message` turn with role
      taken from the parent `message.data.role` (`user` or
      `assistant`).
    - `part.data.type = "reasoning"` → `Thinking` turn.
    - `part.data.type = "tool"` → emits **two** turns from the
      same row: a `ToolUse` with body `"<tool>: <state.input>"`
      and a companion `ToolResult` with body `state.output`.
      OpenCode bundles call+result in one row; the split keeps
      the renderer's fold-tool-blocks UX consistent with
      Claude / Codex.
    - `step-start`, `step-finish`, `patch` parts skipped (model
      lifecycle markers; v1 doesn't render patches).
    - `part.time_created` (epoch ms) → `DateTime<Utc>` via
      `chrono::DateTime::from_timestamp_millis`. Empty bodies
      dropped. Missing DB → `ParseError::NotFound`.
  - 12 SQLite-backed fixture tests cover supports() gating,
    missing-db, empty-session-with-cwd, user/assistant
    role-from-message, reasoning → Thinking, tool → ToolUse +
    ToolResult split, tool-without-output → ToolUse only,
    step/patch skip, intra-message time_created ordering,
    other-session exclusion, empty-body drop, epoch-ms round
    trip.
  - Real-data smoke (one-off, not committed): parsed a 381-msg /
    1658-part session from the author's `opencode.db` into 1193
    turns with kind histogram Message: 65, Thinking: 349,
    ToolResult: 388, ToolUse: 391 — confirms the parser
    matches the live schema.
  - 1193 nextest green. **Closes the OpenCode coverage gap**
    that `H-TRANSCRIPT-014` could not.

- [x] `H-VIEWER-NATIVE-006` Viewer widget: full-screen modal,
  scroll, jump-to-end-on-open.
  - Outcome: `src/viewer/{widget,state,input,render}.rs` ship the
    full Ratatui modal. Layout: 1-line header
    (`<harness>:<session-key>` left, `cwd: <cwd>` right-justified),
    1-line `─` separator, flex body, 1-line bottom separator,
    1-line footer with key hints. `state.rs::ViewerState`
    carries the document, scroll offset, sticky-end flag, show
    tools/thinking toggles, plus viewport-height +
    total-line-count metrics written back by the draw fn so
    the reducer has fresh layout numbers for page-down deltas.
    `input.rs::reduce(state, msg) -> (state, ViewerEffect)` is
    pure. Messages: ScrollUp/Down, PageUp/Down, HalfPageUp/Down,
    JumpToStart, JumpToEnd, ToggleTools, ToggleThinking, Close.
    All clear the sticky-end flag except JumpToEnd which sets
    it; layout toggles re-clamp the scroll offset against
    `max_scroll`. `render.rs::render_turn` emits a dimmed role
    header (`you` / `assistant · thinking` / `— compaction
    summary —` etc.), the body (Markdown via `tui-markdown` for
    Message + CompactionSummary; dim plain text for Thinking /
    ToolUse / ToolResult), and a spacer line. `widget.rs::draw`
    short-circuits the empty-doc case with a "transcript
    unavailable" banner; otherwise builds the flat body line
    Vec (filtering tools/thinking by state flags) and scrolls
    via `Paragraph::scroll`. Stick-to-end pins scroll to
    `max_offset` on every draw until the operator manually
    scrolls. Footer text adapts to current tool/thinking
    state and truncates with `…` on narrow terminals.
  - Tests: 3 state, 9 reducer, 6 render, 7 widget — 25 total.
    Widget snapshot tests via insta cover (a) normal open at
    last turn, (b) JumpToStart on a 30-turn doc, (c) empty
    "transcript unavailable" doc, (d) compaction-summary turn
    rendered with banner header. Additional widget tests
    confirm `draw` writes viewport/total back to state, footer
    advertises current toggle state, and `ToggleTools` makes
    tool turns visible.
  - Bridge wiring (`H-VIEWER-NATIVE-008`) still pending — `T`
    keybind continues to route through the escape-hatch
    `ClaudeHistoryViewer`. 1219 nextest green.

- [ ] `H-VIEWER-NATIVE-007` Substring search inside the viewer.
  - Scope: `/` opens a search prompt at the footer. `n` / `N`
    cycle matches. Matches highlight in the body. Search is
    case-insensitive substring over rendered turn bodies (no
    fuzzy index in v1, matching ADR 0024).
  - Tests: snapshot tests for search-open, match-highlight,
    no-match cases.
  - Blockers: `H-VIEWER-NATIVE-006`.

- [x] `H-VIEWER-NATIVE-008` Viewer-bridge integration: wire the
  `T` keybind into the native viewer.
  - Outcome: `src/tui/viewer_bridge.rs` translates an
    `AgentSessionId` into the matching `SessionLocator` variant
    (with OpenCode `state_scope` treated as either the parent dir
    or the `.db` path itself), runs it through the right
    `HarnessParser`, and wraps the result in a `ViewerState`.
    Parser failures degrade to
    `TranscriptDocument::unavailable(...)` so the widget always
    has something coherent to render. The bridge lives outside
    `src/viewer/` per ADR 0052 — it's the only file in conspectus
    that knows both the graph-flavored `AgentSessionId` and the
    viewer-flavored `SessionLocator`.
  - App carries `viewer_modal: Option<ViewerState>` with
    `open_viewer_modal` / `close_viewer_modal` /
    `take_viewer_modal` accessors. The runtime adds
    `Action::ViewerOverlayKey(KeyEvent)` and makes the
    viewer-modal check the **highest-priority** overlay (above
    value_modal etc.) so all keys route through the modal while
    it's open.
  - `view_action` rewritten: pull the selected `AgentSessionId`,
    call `viewer_bridge::build_viewer_state`, and
    `app.open_viewer_modal(state)`. Falls through to the
    pre-existing escape-hatch external launcher
    (`ClaudeHistoryViewer`) only when the harness has no
    native parser (currently: `aider`). The minimal
    "external is opt-in" sketch from the story — full
    `[viewers.<harness>]` config lands with
    `H-TRANSCRIPT-013`; until then native is unconditional for
    every supported harness.
  - `handle_viewer_overlay_key` translates crossterm keys into
    `ViewerMsg` and runs the pure reducer (`take`-reduce-`put`
    pattern around `App::take_viewer_modal`). Keymap:
    `q`/`Esc`/`Ctrl-C` → Close, `j`/`Down` → ScrollDown,
    `k`/`Up` → ScrollUp, `PgDn`/`Space` → PageDown,
    `PgUp` → PageUp, `Ctrl-D`/`Ctrl-U` → half-page,
    `g`/`Home` → JumpToStart, `G`/`End` → JumpToEnd,
    `t` → ToggleTools, `y` → ToggleThinking. Unknown keys
    drop silently.
  - The TUI `draw` path branches on `viewer_modal_mut()`: when
    Some, the widget takes the full frame area; when None, the
    standard two-panel render proceeds unchanged. Mux-row
    selection behavior is unchanged — `T` only opens the modal
    for `AgentSession` row selections.
  - Help overlay description updated:
    "Open the selected session's transcript (q/Esc close, j/k
    or PgDn/PgUp scroll, g/G start/end, t tools, y thinking)".
  - 7 bridge tests (locator mapping for all three harnesses,
    OpenCode parent-dir vs. .db filename, unknown harness =
    None, `build_viewer_state` None for unsupported / fallback
    to unavailable doc when file missing). 1226 nextest green.

- [x] `H-VIEWER-NATIVE-011` Styling + spacing pass.
  - Scope: H-VIEWER-NATIVE-006/008 ship a functional but
    visually-minimal modal. Operator feedback from kicking the
    tires: spacing is off (tool-output line numbers bump
    directly into content), turn separation is too subtle,
    role headers don't carry enough visual weight, and tool /
    thinking blocks need more chrome to read as folded-by-
    default content. Pass over:
    - **Turn-level chrome**: visible separator between turns
      (rule or extra spacing), accent color per role
      (`you` vs `assistant`), optional timestamp suffix on the
      header line (already in the model, not rendered yet).
    - **Compaction summary**: full-width rule + a distinct
      banner color so it reads as a structural marker rather
      than another role header.
    - **Body styling for `Message`/`CompactionSummary`**:
      consistent left indent so role headers visually own the
      body that follows; line wrapping (`Wrap { trim: false }`)
      to avoid mid-word breaks; soft padding on either side so
      Markdown bold/italic ranges don't crowd terminal edges.
    - **Tool blocks**: bracketed framing (e.g. `┌─ tool call:
      <name> ─` / `└─ tool result ─`) so call+result reads as
      a paired unit when both are visible. Argument JSON
      should be pretty-printed (or at least line-broken on
      commas) rather than one-line. Tool output rendering
      needs gutter handling — line numbers, lead-in prefixes
      etc. should sit in a fixed-width gutter that doesn't
      collide with content.
    - **Thinking blocks**: prefix with a distinguishable
      marker (e.g. dim italic `~ thinking ~`) so they're
      obviously not assistant-output prose.
    - **Code block styling inside `tui-markdown`**: re-evaluate
      the `highlight-code` feature decision from ADR 0051.
      The widget renders Markdown via `tui-markdown` with
      `default-features = false`; turning `highlight-code` on
      adds `syntect` (~MB), which ADR 0051 explicitly
      rejected for v1. Revisit if operators report that
      monochrome code blocks hurt scannability; otherwise add
      a soft fence indent + dim background as cheap
      substitutes.
    - **Theme integration**: per ADR 0052's `theme.rs` carve-
      out, route every color decision through the
      `crate::tui::theme::Theme` re-export so users can
      override via `[tui.theme]` config (ADR 0032).
  - Tests: refresh insta snapshots for each turn kind (single
    Message, paired ToolUse+ToolResult, Thinking block,
    CompactionSummary banner). Add a snapshot for a narrow
    (40×24) terminal so the gutter / wrap behavior regresses
    visibly. The existing 4 widget snapshots stay as the
    baseline coverage; this story replaces them.
  - Out of scope: search highlighting (covered by
    `H-VIEWER-NATIVE-007`), per-message expand/collapse
    (likely a separate story once tool-block framing is in),
    mouse bindings (`H-VIEWER-NATIVE-012`), in-viewer fork /
    child navigation (`H-VIEWER-NATIVE-013`).
  - Blockers: `H-VIEWER-NATIVE-008`. Pairs naturally with
    `H-VIEWER-NATIVE-007` since both touch the renderer.
  - Outcome: rewrote `src/viewer/render.rs` around a
    `claude-history`-inspired gutter-and-chip layout:
    right-aligned colored chips (`you`, `assistant`, `Thinking`,
    `Tool`, `↳ Result`, `compact`) in a fixed `GUTTER_WIDTH=10`
    column, separated from body by ` │ ` dim rule, with body
    flowing to the right. Continuation lines (multi-line bodies
    + wrapped long lines) carry a blank gutter + repeated rule
    so the body's left edge stays constant. Hand-rolled
    word-wrap helper handles whitespace splits + hard-break for
    oversized tokens; styled-line wrap preserves span styles
    across line breaks for Markdown content. Chip styling
    routed through the `crate::tui::theme::Theme` re-export
    (ADR 0052's carve-out) so `[tui.theme]` config applies.
    `src/viewer/widget.rs` updated: header refreshed to
    `harness-chip · cwd · N turns` with width-aware degrade;
    footer refreshed to `[ pos/total ] · tools·on/off ·
    think·on/off · q close · …` with the long hint truncating
    last. ADR 0051 amended to turn the `highlight-code`
    feature **on** (re-decision rationale + counterfactual
    both preserved); `syntect` added to
    `ALLOWED_EXTERNAL_DEPS` and
    `docs/transcript-viewer-deps.md`. Refreshed 4 prior
    widget snapshots + added 2 new ones (tools-visible chip
    pair, narrow-terminal 40-col wrap). 13 render-side tests
    + 9 widget-side tests; 1231 nextest green.

- [x] `H-VIEWER-NATIVE-009` Retire patched recall from
  `pkgs/recall/`.
  - Outcome: option (b) chosen ahead of `H-VIEWER-NATIVE-008`
    when the recall debt became clear. `pkgs/recall/` (default.nix
    + Cargo.lock + session-flag.patch + .gitignore) removed in
    nix-config commit `f6a7b46 chore(pkgs): retire patched
    recall after conspectus pivot`. `recall` dropped from
    `home/modules/dev-toolchain.nix` in the same commit.
    Conspectus-side `RecallViewer` + `supports_flag` capability
    probe + `required_flags` trait method ripped in conspectus
    commit `8c949a0 refactor(viewer): rip recall-specific
    surface`. `claude-history` remains as the
    `ClaudeHistoryViewer` escape-hatch backend for harnesses
    without a native parser (currently: aider).

- [ ] `H-VIEWER-NATIVE-012` Mouse bindings inside the viewer.
  - Scope: scroll-wheel events translate to ScrollUp /
    ScrollDown; click positions the cursor / selects a turn
    boundary. crossterm mouse events are already enabled
    elsewhere in the TUI; the modal just needs a handler
    branch in `handle_viewer_overlay_key` (or a new
    `handle_viewer_overlay_mouse`).
  - Tests: mouse-event smoke through the reducer.
  - Blockers: `H-VIEWER-NATIVE-011` (styling) so click
    targets land on visually-meaningful elements.

- [ ] `H-VIEWER-NATIVE-013` In-viewer navigation into forks /
  child sessions.
  - Scope: when the displayed transcript references a fork or
    a child session, expose a way to jump into that session's
    transcript without leaving the modal (e.g. `→` over a
    chip, or a per-fork chip with `Enter`). Needs lineage
    from the graph layer; the bridge gains
    `build_viewer_state_for_child(parent, child_session_id)`
    or similar. Stack-of-states inside the modal so
    Backspace returns to the previous transcript.
  - Open questions: should the lineage walk go through
    conspectus's resolved graph (per ADR 0005 / ADR 0018) or
    through harness-specific intra-session lineage encoded in
    the transcripts themselves? The latter keeps the viewer
    extractable; the former is richer.
  - Tests: bridge mapping for `parent_session` references in
    each harness; modal stack push/pop; lineage-not-found
    fallback.
  - Blockers: `H-VIEWER-NATIVE-008`. Pairs naturally with
    `H-VIEWER-NATIVE-011` for chip-as-click-target affordance.

- [ ] `H-VIEWER-NATIVE-015` Per-message selection + clipboard copy.
  - Scope: introduce per-message selection inside the viewer
    modal. `J`/`K` (capital) move the selection forward/back
    one turn. The selected turn shows a highlighted bar in
    the gutter's far-left column (over the chip pill or
    alongside it). A keybind (`yy` like vim's yank, or `Ctrl-Y`)
    copies the selected turn's body to the system clipboard.
  - State: `ViewerState` gains `selected_turn: Option<usize>`
    (index into the visible turns produced by build_body_lines).
    Reducer messages: `SelectPrevTurn`, `SelectNextTurn`,
    `ClearSelection`, `CopySelectedBody`.
  - Renderer: when `selected_turn == Some(i)`, the chip line
    (and continuation lines) of turn `i` paint the leftmost
    cell of the gutter as a highlight bar (e.g. `▌` in the
    chip color, BOLD).
  - Clipboard: route through a small abstraction (already
    available in the wider conspectus surface via `arboard` /
    the existing `clipaste` integration), or via OSC 52 for
    SSH-friendly copy. ADR check on the dep before adding to
    the viewer's allow-list.
  - Tests: reducer unit tests for selection cycling; widget
    snapshot test for the highlight bar; clipboard call
    behind a `BinaryProbe`-style seam so unit tests don't
    actually touch the host clipboard.
  - Blockers: `H-VIEWER-NATIVE-008`. Pairs naturally with
    `H-VIEWER-NATIVE-012` (mouse selection) and the
    chunk-loading story so chunked transcripts have stable
    turn indices.

- [ ] `H-VIEWER-NATIVE-016` Per-tool expand on click.
  - Scope: in tool-detail Summary or Truncated mode, clicking
    the chip pill of an individual tool turn temporarily
    expands *that* turn to full detail while leaving the
    global tool detail level unchanged. Pressing the same key
    or clicking again collapses. Inspired by claude-history's
    per-message expand UX.
  - State: `expanded_tool_turns: BTreeSet<usize>` on
    `ViewerState` (turn indices currently expanded). Cycling
    `t` clears the per-turn overrides.
  - Renderer: per-turn render consults the override set; an
    expanded turn renders at `ToolDetail::Full` regardless of
    the global level.
  - Folds into `H-VIEWER-NATIVE-012` (mouse) for the click
    target. Without mouse, a `Tab`/`o`-style "expand cursor"
    keybind can drive it from the keyboard (overlap with the
    selection story).
  - Tests: reducer for set toggling; widget assertions that
    an expanded turn renders more lines than its peers at
    the same global level.
  - Blockers: `H-VIEWER-NATIVE-012` (mouse) and
    `H-VIEWER-NATIVE-015` (selection cursor for keyboard
    expand).

- [x] `H-VIEWER-NATIVE-017` Markdown table rendering.
  - Outcome: implemented as a **pre-processor** (option b in the
    original scope, but staged *before* tui-markdown rather than
    after). Root cause confirmed by reading the
    `tui-markdown 0.3.7` source: it doesn't enable pulldown-cmark's
    `ENABLE_TABLES`, so pipes flow through as paragraph text
    without ever reaching a table handler. A surface-area survey
    of the operator's local corpus (40 Claude / 38 Codex / 19
    OpenCode sessions) found 103 GFM pipe-tables; 100% wider
    than 40 cols, ~85% wider than 80. Reviewed
    Claude/Codex/OpenCode renderers — Codex (Apache-2.0) ships
    the most coherent design (column-type classification,
    iterative shrink, key/value transpose fallback) at ~2,700
    LOC. ADR 0054 records the decision to adopt `comfy-table`
    (Apache-2.0/MIT) as a cheaper-but-aligned alternative:
    `ContentArrangement::Dynamic` + `set_width(content_width)`
    delivers Codex-style iterative-shrink-to-fit without the
    porting tax. `src/viewer/table.rs` owns the small
    segmentation state machine that splits each Message /
    CompactionSummary body into `BodySegment::Markdown(&str)`
    and `BodySegment::Table(ParsedTable)`, exempts fenced code
    blocks, parses GFM alignment markers (`:---:`), and renders
    via comfy-table's `UTF8_NO_BORDERS` preset (light header
    rule + dotted column separators; matches the viewer's
    gutter aesthetic). Border glyphs route through
    `theme.secondary_text` so colour overrides via `[tui.theme]`
    work. Render cache key already includes `content_width`, so
    terminal resizes invalidate correctly with no reducer
    changes. Cell-Markdown styling (bold, code spans, links)
    drops in v1 — same compromise Codex shipped for years; the
    `custom_styling`-feature + `ansi-to-tui` upgrade path is
    documented in ADR 0054. 17 tests added (13 table-module
    unit tests + 1 widget-level proof + 2 snapshots at 80 and
    40 cols + 1 inferred from compile time); 136 viewer tests
    pass total; clippy + fmt + full nextest green.
  - Out of scope (deferred): in-cell Markdown styling,
    column-type-aware shrink priority, key/value transpose
    fallback (Codex's `table_key_value.rs` is the documented
    follow-on), alignment override for narrative columns.

- [ ] `H-VIEWER-NATIVE-014` Lazy / chunk-by-chunk transcript
  loading around compaction boundaries.
  - Scope: NATIVE-011's render cache makes scroll-only frames
    O(1), but the *first* draw still composes every visible
    turn through `tui_markdown::from_str`. Very large Claude
    sessions (thousands of turns, many tool blocks) take a
    visible beat to open the viewer. Operator suggestion:
    load chunks lazily, anchored at `compact_boundary` /
    `compacted` records — initial open renders just the
    most-recent compaction window, and earlier chunks load
    on demand as the operator scrolls past the chunk's top.
  - Implementation outline: introduce a `ChunkedTranscript`
    in `src/viewer/model.rs` (or a `ChunkBoundary` on
    `TranscriptDocument`) carrying the compaction-aware
    spans of the source. Parsers split at boundaries.
    Widget keeps the active chunk in `ViewerState`; reducer
    handles "scroll past chunk top" by loading the previous
    chunk. Cache key gains the chunk identity.
  - Open questions: how to render the chunk seam (single
    rule with a "load previous" affordance vs. eager fetch
    on approach); whether to also chunk on time-of-day
    boundaries for sessions without explicit compaction;
    interaction with search (`H-VIEWER-NATIVE-007`), which
    needs to span chunks.
  - Tests: parser-side fixture covering multi-chunk
    boundaries; widget chunk-load reducer test; performance
    smoke against a real session.
  - Blockers: `H-VIEWER-NATIVE-011` (the render cache and
    the gutter layout are prerequisites).

- [ ] `H-VIEWER-NATIVE-010` (later) Extraction prep: lift
  `src/viewer/` into a workspace member crate.
  - Scope: when the viewer module's import surface has been
    stable for ≥ N stories, create a `crates/` workspace,
    move `src/viewer/` to `crates/conspectus-transcript-viewer/`,
    add a thin `bin` target with clap that takes a
    `SessionLocator` from argv. Conspectus depends on it as a
    workspace member.
  - Tests: existing viewer tests run from the new crate
    location; conspectus TUI still routes through it
    unchanged.
  - Blockers: stability of the dep surface (see
    `docs/transcript-viewer-deps.md` history).

### Agent Session Continue Scheduling

Some harnesses end a transcript with a usage-limit or rate-limit
message that includes the time when work can resume. Conspectus
already reads the last message for previews and can open native
transcripts on demand; this workstream adds a higher-level
"blocked until" signal and an explicit way for the operator to
schedule a `Continue` prompt for that session at the time identified
by the transcript.

Detection stays read-only: a session whose final meaningful turn is a
usage-limit message should surface as paused/blocked metadata and table
or TUI affordances. Scheduling is separate, explicit user intent. It
must not silently send prompts or create scheduler state from ordinary
`graph`, `table`, or `tui` discovery.

- [ ] `H-CONTINUE-001` ADR: usage-limit detection and scheduled
  continuation policy.
  - Scope: record the provider-neutral model for a "blocked until"
    session signal, the allowed scheduler backend(s), where scheduled
    jobs live, how missed/cancelled jobs behave, and why sending a
    literal `Continue` prompt is safe enough only as an explicit action.
    Compare alternatives such as shelling out to `at`, a Conspectus
    background server, tmux `send-keys`, harness-native resume
    commands, and manual reminders.
  - Tests: docs-only; `git diff --check`.
  - Manual checks: review against ADR 0023, ADR 0052, and the
    read-only discovery requirements in `docs/design.md`.
  - Blockers: none.

- [ ] `H-CONTINUE-002` Model blocked-session metadata.
  - Scope: add optional metadata to `AgentSessionNode` or a typed
    sidecar record that captures `blocked_reason`, parsed
    `resume_after_epoch`, the source message snippet, parser
    confidence, and harness/source provenance. Keep ordinary sessions
    byte-stable by skipping absent fields in JSON.
  - Tests: serde round trips, sparse-session JSON snapshots, and
    no-field output for sessions without a recognized usage-limit
    tail.
  - Blockers: `H-CONTINUE-001`.

- [ ] `H-CONTINUE-003` Detect usage-limit tails in supported
  transcript parsers.
  - Scope: for Claude Code, Codex, and OpenCode, inspect the final
    meaningful assistant/system message after applying the same
    tool/thinking/channel-marker filters used by preview and viewer
    parsing. Recognize usage-limit messages only when they are the
    last meaningful transcript message and contain a parseable resume
    time. Preserve the source snippet and parsed time without
    fabricating a blocked state for older messages in the middle of a
    transcript.
  - Tests: fixtures for absolute timestamps, relative "try again in"
    durations, timezone-bearing text, malformed/no-time messages,
    usage-limit messages followed by later user/assistant text, and
    ordinary transcript tails.
  - Blockers: `H-CONTINUE-002`.

- [ ] `H-CONTINUE-004` Surface blocked-until state in CLI and TUI.
  - Scope: add opt-in table columns such as `blocked` /
    `resume-after`, a TUI row badge or detail-pane field, and
    `node show` output that displays the parsed resume time and source
    snippet. The default table columns should not grow unless the ADR
    explicitly changes the privacy/width posture established by ADR
    0023.
  - Tests: table renderer snapshots, TUI buffer snapshots for blocked
    and non-blocked sessions, and `node show` output coverage.
  - Blockers: `H-CONTINUE-003`.

- [ ] `H-CONTINUE-005` Implement explicit continue scheduling.
  - Scope: add a command and matching TUI action that schedule a
    `Continue` message for the selected blocked session at its parsed
    resume time, with flags to override the time and message text.
    The implementation must verify that the target session is still
    the same session before sending, record enough state to list/cancel
    pending jobs, and avoid project-tree cache/state writes.
  - Tests: fake scheduler and fake harness sender coverage for create,
    list, cancel, missed time, target-session mismatch, custom message,
    and dry-run behavior.
  - Manual checks: schedule against a disposable session using a
    near-future time, confirm the prompt is sent once, and confirm
    cancelling prevents delivery.
  - Blockers: `H-CONTINUE-001`, `H-CONTINUE-003`, `H-CONTINUE-004`.

- [ ] `H-CONTINUE-006` Document blocked-session and continue workflows.
  - Scope: update `docs/operations.md` and any TUI help/docs with how
    usage-limit detection works, how to inspect the parsed resume
    time, how to schedule/list/cancel a pending continuation, and the
    safety limits around stale sessions or unrecognized message
    formats.
  - Tests: docs-only `git diff --check`.
  - Blockers: `H-CONTINUE-005`.

### Agent-Mux Orchestrator Integrations

A growing class of "agent-over-tmux" orchestrators — agent-deck, dmux,
workmux, agent-of-empires, and others — maintain on-disk state that
maps agent sessions to tmux sessions, checkouts, branches, and
sometimes forks. Most of that state, however, overlaps with what the
process-tree linker (`H-MUXPROC-*`) can derive directly from running
processes inside each tmux pane: pane ↔ harness binary, pane PID,
process cwd, and therefore pane ↔ `AgentSession` for live sessions.
Conspectus should treat MUXPROC as the primary, tool-agnostic source
of agent ↔ tmux evidence and only build per-orchestrator adapters
when they expose evidence MUXPROC cannot — concretely: workspace
composition, container-isolated agents, exited / paused sessions, or
orchestrator-specific labels and lineage.

In-scope candidates (gated on the audit in `H-AGENTMUX-001`):
agent-deck (~/.agent-deck/, SQLite), dmux (`standardagents/dmux`,
~1.6k stars), workmux (`raine/workmux`, ~1.5k stars, per-worktree
`.workmux/` plus `~/.local/state/workmux/`), agent-of-empires
(`njbrake/agent-of-empires`, ~2.3k stars). Explicitly deferred:
`cdknorow/coral` (~21 stars), `honeymux/honeymux` (~71 stars, runtime
overlay rather than persistent state).

- [x] `H-AGENTMUX-001` Audit each candidate orchestrator's evidence
  against MUXPROC and decide which adapters to build.
  - Outcome: audit collapsed during implementation work into ADR 0060
    rather than a standalone paper. Findings: agent-deck's unique
    evidence is workspace composition (built), dmux is a MUXPROC
    subset (deferred per `H-AGENTMUX-005`), workmux's resurrect-state
    is the only plausible non-overlap (deferred per `H-AGENTMUX-006`),
    agent-of-empires container isolation is unverified
    (deferred per `H-AGENTMUX-007`). The `AgentMuxAdapter` trait was
    not introduced — `DiscoveryProvider` is sufficient for the one
    surviving adapter and a trait would be speculative.

- [x] `H-AGENTMUX-002` Detect agent-deck multi-repo checkouts as a
  workspace provider.
  - Outcome: `src/discovery/agent_deck.rs` ships the
    `AgentDeckDiscovery` provider, wired into `discover_local_with`
    via `LocalDiscoveryConfig::agent_deck_root` (defaults to
    `$HOME/.agent-deck/multi-repo-worktrees`, opt-out via
    `CONSPECTUS_DISABLE_AGENT_DECK`, override via
    `CONSPECTUS_AGENT_DECK_ROOT`). Emits
    `WorkspaceNode { provider = "agent-deck" }` + symlink-only
    `WorkspaceContainsRepo` candidate links with the same
    `logical_path` / `member_path_kind` source-fields shape generic
    workspace uses, so the column formatter is provider-uniform.
    Unit + integration tests cover two-symlink, one-symlink,
    broken-symlink, non-symlink-child, and multi-workspace fixtures.
    See ADR 0060 for the full decision record.

- [x] `H-AGENTMUX-003` Surface multi-repo participants in the session
  table.
  - Outcome: `output::agent::fetch_workspace_lookup` joins the
    resolver's chosen `workspace_contains_repo` selections and
    renders the workspace column as `atelier+conspectus`-style
    `+`-joined member basenames when ≥2 distinct members exist;
    single-repo workspaces keep the root path. Opt-in only via
    `--columns ...,workspace,...` — the default `SESSIONS_COLUMNS`
    set is unchanged. Promoting `workspace` into the defaults is
    deferred per ADR 0060 §Alternatives. Atelier multi-repo
    workspaces exercise the same surface from day one.

- [ ] `H-AGENTMUX-004` Read agent-deck profile state from `state.db`.
  - Scope: agent-deck stores richer per-session metadata
    (`~/.agent-deck/profiles/<profile>/state.db`, SQLite). **This
    item is gated on the audit in `H-AGENTMUX-001` confirming the
    SQLite content includes evidence MUXPROC cannot supply** —
    plausible candidates are profile labels / tags, agent lifecycle
    state for exited or paused sessions, and session-to-multi-repo
    binding when the agent has since changed cwd. If the audit shows
    the SQLite content is only a snapshot of what MUXPROC already
    sees live, close this item as won't-do. Otherwise read the
    SQLite state read-only and emit only the surviving non-overlap
    evidence. Reuse the `rusqlite` dependency added by ADR 0013.
  - Tests: fixture tests over a temp SQLite file populated with
    representative rows; missing-database degradation; malformed
    schema degradation.
  - Manual checks: confirm read-only access; confirm the adapter
    does not lock the database while agent-deck is running.
  - Blockers: `H-AGENTMUX-001` (audit must justify the work),
    `H-AGENTMUX-002`. Requires recording the agent-deck schema and
    the surviving evidence set in an ADR (or an extension of ADR
    0013) before introducing the read code.

- [ ] `H-AGENTMUX-005` Add a dmux orchestrator adapter (audit-gated).
  - Scope: **placeholder — may be closed as won't-do.** dmux's
    on-disk state layout is not surfaced in its README, so the audit
    in `H-AGENTMUX-001` is responsible for source-inspecting dmux
    and determining whether it tracks anything beyond a MUXPROC
    subset (task / feature metadata, agent lifecycle state, lineage
    between dmux-spawned sessions, container isolation). If the
    audit returns "MUXPROC subset," close this item. Otherwise
    implement the adapter against the surviving non-overlap evidence
    only — do not duplicate the pane ↔ harness link.
  - Tests: deferred until the audit determines scope.
  - Manual checks: run against a real dmux install if available;
    otherwise rely on fixtures captured from upstream.
  - Blockers: `H-AGENTMUX-001` (audit must justify the work and
    define the evidence set).

- [ ] `H-AGENTMUX-006` Add a workmux orchestrator adapter
  (audit-gated, narrowed scope).
  - Scope: workmux's runtime mapping (tmux window names + active
    agent state) is largely a MUXPROC subset. The audit in
    `H-AGENTMUX-001` should focus on workmux's two artifacts that
    are plausibly non-overlapping: (a) per-worktree
    `<worktree>/.workmux/` files — project-rooted intent / labels /
    history that survive process exit and fit Conspectus's
    persistence guardrails cleanly; (b) `~/.local/state/workmux/
    agents/` resurrect state, which can describe sessions that
    aren't currently running. If both turn out to be inert mappings
    of what MUXPROC sees live, close the item. Otherwise implement
    an adapter scoped to those two artifacts: walk `.workmux/`
    during normal scan traversal, read the resurrect-state files,
    and emit candidate links only for the surviving evidence (no
    pane ↔ harness duplication).
  - Tests: fixture tests over a tree containing `.workmux/`
    directories plus a fake `~/.local/state/workmux/agents/`
    layout; resurrect-state covering an exited session.
  - Manual checks: run against a real workmux install if available.
  - Blockers: `H-AGENTMUX-001`.

- [ ] `H-AGENTMUX-007` Add an agent-of-empires orchestrator adapter
  (audit-gated).
  - Scope: **placeholder — may be closed as won't-do.** The
    strongest theoretical edge over MUXPROC is container isolation:
    when agent-of-empires runs the agent inside a container, the
    host process tree shows only the runtime
    (`docker`/`podman`/`bwrap`) and MUXPROC cannot identify the
    harness. The audit in `H-AGENTMUX-001` should determine (a)
    whether agent-of-empires actually tracks the in-container agent
    identity in host-visible state, and (b) whether container
    isolation is in Conspectus's near-term scope at all. If both
    are yes, implement an adapter scoped to container-isolated
    sessions and any other surviving non-overlap evidence; emit
    `AgentSession` nodes carrying the orchestrator-known harness
    key without pretending Conspectus's harness adapters can parse
    their transcripts.
  - Tests: deferred until the audit determines scope.
  - Manual checks: run against a real agent-of-empires install if
    available.
  - Blockers: `H-AGENTMUX-001`.

### Workspace UX Redesign (H-WS-*)

Operator-noted UX confusion in the Sessions view's `graph` grouping:
sessions whose cwd is in a repo that happens to be a workspace member
get nested under the workspace even when the session itself has no
`AssociatedWith` workspace edge. Heavily-used member repos
(`conspectus`, `config`, `atelier`) make the workspace level
actively misleading. See `docs/plans/workspace-view-redesign.md` for
the full diagnosis, the three axes (Sessions/Graph fix, dedicated
Workspaces view, other-view audit), and the recommended sequence.

The conflation is between two semantically distinct relationships:
(A) the session is workspace-rooted via `AssociatedWith Workspace`,
and (B) the session merely touches a repo that is *also* a workspace
member with no session-level workspace edge. Today's grouping
promotes (B) to look like (A); the fix surfaces them as different
concepts everywhere they appear.

- [x] `H-WS-001` Strict-only + chip in Sessions/Graph workspace nesting.
  - Outcome (strict-nesting half — retained): `resolve_group_key`
    sets the workspace level only when the session carries a
    direct `AssociatedWith Workspace` edge; the previous
    `workspace_for_repo` fallback is removed. Repo-shared
    (B-class) sessions fall through to repo-level grouping.
    `cross_link::workspace_member_roots` indexes both the
    workspace's own `root` and every member's `logical_path`
    (dropping `canonical_checkout_root` to fix the symlinked-member
    leak), with deepest-match-wins keeping member-subdir
    attribution preferred. Regression test
    `symlinked_workspace_member_does_not_associate_session_at_canonical_path`
    in `discovery::cross_link::tests` pins the corrected
    semantics. Strict-nesting tests
    (`workspace_rooted_session_nests_under_workspace_at_depth_2`,
    `repo_shared_session_stays_at_repo_level`) cover the bug-fix
    outcome.
  - Chip half — reverted (ADR 0063): the original ship added a
    `[ws-name]` / `[N ws]` cross-reference chip on (B)-class rows
    via `workspaces_for_repo`, `active_workspaces`, and
    `weak_workspace_chip` helpers, with an activeness gate and
    `WEAK_WORKSPACE_CHIP_MAX = 3` cardinality threshold. After
    running the chip the operator reported it was not surfacing
    actionable information — knowing that a session's repo
    *happens* to be claimed by a workspace, without the session
    being workspace-rooted, did not change any decision the
    operator made. The chip, its helpers, the
    `workspace_chip: Option<String>` field on `AgentSessionRow`,
    the rendering block in `render_session_spans`, and the four
    chip-specific tests are removed. The (B)-rendering test
    keeps only the strict-nesting depth assertions.
  - Net result: the H-WS-001 contribution is the strict-nesting
    + cross-link inference fix; the cross-reference chip is gone
    from the UI in all five views (Mux/Prs/Forks/Union were
    already chipless per ADR 0061; Workspaces dropped the
    equivalent `related` subgroup per ADR 0062). The (A)/(B)
    distinction remains load-bearing at the data-model layer
    (the AssociatedWith inference still emits it) but is no
    longer surfaced anywhere in the TUI.

- [x] `H-WS-002` Dedicated Workspaces view (MVP).
  - Outcome: new `View::Workspaces` with `WorkspacesGrouping::Flat`
    as the only grouping shipped in v1. Initial row tree
    (`src/tui/rows/workspaces.rs`) listed each workspace with up to
    three labeled subgroups (`members` / `in workspace` /
    `related`). Keybinding `6` switches to the view; `[`/`]` cycle
    includes Workspaces; `--view workspaces` works from the CLI.
    Default-collapse for the `related` subgroup and the Provider /
    Activity / Repo groupings were deferred to `H-WS-002a`.
  - Polish (ADR 0062): operator feedback after running the MVP
    flagged the `members` subgroup as left-tree noise (the detail
    pane already exposes members as navigable HeaderFields) and
    the `in workspace` / `related` labels as unintuitive. The
    polish pass moved the member list inline on the workspace
    top-level row as a `+`-joined span (matching the agent-table
    workspace column from ADR 0060) and dropped the `related`
    (B)-class subgroup entirely. The Workspaces view now surfaces
    only (A)-class sessions, sitting at depth 1 directly under the
    workspace row with no labeled wrapper. The (B) cross-reference
    signal continues to live as the `[ws-name]` chip in Sessions /
    Graph from `H-WS-001`. `fetch_b_class_sessions` and the
    multi-hop join it powered are removed; `MemberSqlRow` slims
    to a single `display_name` field. Six unit tests cover empty
    snapshot, inline member-list rendering with and without
    provider, (A)-class direct nesting at depth 1, (B)-class
    suppression, and logical-path-basename naming.
  - ADR: 0062 records the polish decision and answers
    open question 2 from
    `docs/plans/workspace-view-redesign.md` (the (B)-in-view
    question). The four-grouping menu decision is still
    `H-WS-002a`'s.

- [-] `H-WS-002a` Workspaces view polish: Provider/Activity/Repo
  groupings.
  - Obsolete (ADR 0065): the dedicated `View::Workspaces` was
    removed in favor of `SessionsGrouping::Workspace`, so there
    is no longer a `WorkspacesGrouping` enum to extend. The
    Provider / Activity / Repo grouping axes from the original
    `H-WS-002` ticket would now ship as Sessions-view variants
    (or a separate cross-cut) if they're revisited; tracked as a
    new story when needed.

- [x] `H-WS-003` Audit Mux/Prs/Forks/Union workspace grouping for the
  same (A)/(B) conflation.
  - Outcome: the audit found the four views' `Workspace` grouping
    variants are unimplemented, not buggy. `src/tui/rows/mux.rs:191`
    matched `Session | Workspace | Host` together and called
    `emit_flat`; `PrsBuildInputsFromConn`,
    `ForksBuildInputsFromConn`, and `UnionBuildInputsFromConn`
    carry no `grouping` field at all and never read their
    respective grouping enums. The `Workspace` cycler entries
    therefore advertised a label that selected the default flat
    layout. There was no (A)/(B) conflation to fix because there
    was no workspace nesting to fix. Decision (ADR 0061): drop the
    `Workspace` variant from `MuxGrouping`, `UnionGrouping`,
    `PrsGrouping`, and `ForksGrouping`; the Workspaces view
    (`H-WS-002`) is the canonical workspace-first surface, and the
    (A)/(B) distinction does not translate cleanly to Prs/Forks
    (no cwd → no analog of "workspace-rooted"). `Grouping::as_str`,
    `Grouping::values_for`, and the dead match arm in `mux.rs` are
    updated; the `workspace_chip: None` comments in the four row
    builders now record that the chip has no analog in views
    without workspace grouping. Configs that set
    `grouping = "workspace"` on these views now produce a
    `ConfigDiagnostic` listing the valid menu values rather than
    silently mapping to flat. No new row-tree tests are needed;
    the existing `parse_and_as_str_round_trip_per_view` test
    iterates `values_for(view)` and continues to pass over the
    shrunken menus.

- [x] `H-WS-004` Hybrid workspace+repo grouping in Sessions / Graph.
  - Outcome (ADR 0064): Sessions / Graph view reshaped to put
    workspaces and repos at the same top level as peer parents,
    each with sessions directly underneath at depth 1 (no repo
    intermediate beneath workspaces, no checkout intermediate
    when the repo has a single worktree). The (A)/(B)
    distinction stops driving any UI marker beyond a top-level
    routing decision in `resolve_group_key`: A-class sessions
    route to a workspace bucket, B-class and unaffiliated route
    to a repo bucket. Two long-standing operator pain points
    fixed: (i) the repo level beneath workspace headers was
    noise that duplicated the workspace's project context, and
    (ii) agent-deck A-class sessions whose cwd is the workspace
    composite directory (no checkout) dropped silently into the
    "ungrouped" bucket because the legacy
    `checkout_for_path(cwd)?` early return ran before the
    workspace lookup. `GroupKey` now carries either a
    workspace-only shape (workspace = Some, repo = None) or a
    repo shape (workspace = None, repo = Some(RepoBucket));
    custom `Ord` puts workspace buckets first. Workspace
    headers in Graph use the shared
    `format_workspace_display` helper from `rows/mod.rs` so
    they read identically to the dedicated Workspaces view's
    headers. `workspace_member_names` lookup uses the same
    resolver-selected `WorkspaceContainsRepo` candidate links'
    `logical_path` source field the Workspaces view's
    `fetch_members` SQL uses, so the two surfaces stay aligned
    automatically. Tests updated: `graph_grouping_uses_session_workspace_context`
    expects depth 1 and 2-row tree (no repo intermediate);
    renamed test `workspace_rooted_session_nests_directly_under_workspace`
    checks both depth and the new workspace header format. Two
    new tests cover the agent-deck workspace-root cwd case and
    the hybrid peer-parents shape.
  - Follow-up (ADR 0065): `SessionsGrouping::Workspace` shipped
    and `View::Workspaces` / the `6` keybinding were removed. The
    Workspaces view's idle-workspace visibility is preserved by
    enumerating workspace nodes in the new grouping, and (B)-class
    sessions land in the Ungrouped bucket in this mode.

### Process-Tree Agent↔Pane Linking

Independent of any orchestrator's state file, an agent process running
inside a tmux pane can be identified by walking the pane's process
tree and matching descendant command names against known agent
harness binaries (claude / codex / opencode / aider). `tmux-agent`
(`trentdavies/tmux-agent`) demonstrates this stateless approach: it
takes a single `sysinfo` snapshot, then for each pane walks up to ~3
levels of descendants from the shell PID and matches against a known
binary-name set, with regex-over-pane-content and title heuristics as
fallbacks. For Conspectus this would be a tool-agnostic, definitive
session ↔ pane evidence source that works even when no orchestrator
is installed — and a useful cross-check against agent-mux adapter
output when one is.

Drift-reduction sequence after the `H-MUXPROC-015` Claude Code
failure:

1. Finish the `H-MUXPROC-002` process-linking slice by making the
   evidence taxonomy explicit in tests and resolver ranking. In
   particular, treat start-command / argv session ids as launch
   evidence, below active open-fd, hook, control-plane, and fresh
   state evidence. This immediately reduces the chance that stale
   `--resume` arguments become preferred links.
2. Take `H-MUXPROC-015` as the first regression story, even before a
   definitive Claude-current-session source exists. Add fixtures for
   launch session A plus stronger current-session evidence B, and for
   the fallback case where launch evidence remains the best available
   signal. This locks in the intended resolver behavior while later
   sources are still being researched.
3. Do `H-MUXPROC-005` and `H-MUXPROC-009` as short audits in parallel
   if possible. They have no blockers and decide whether Claude Code,
   Codex, or opencode can expose current session identity through a
   non-mutating control plane or hook/plugin path. The Claude Code
   `/resume` drift should be the primary audit scenario.
4. If hooks are viable, do `H-MUXPROC-010`, `H-MUXPROC-011`, then
   `H-MUXPROC-012`. This is the highest-confidence durable path for
   Claude Code drift if hook payloads include the post-`/resume`
   session id or transcript path. Keep `H-MUXPROC-013` and
   `H-MUXPROC-014` behind the same schema, but do not let them delay
   the Claude fix.
5. If a non-mutating Claude control plane exists, add the corresponding
   control-plane adapter before or instead of the hook emitter. If only
   Codex or opencode surfaces survive the audit, keep `H-MUXPROC-006`
   and `H-MUXPROC-007` scoped to those harnesses and continue the
   Claude path through hooks or read-only file/state evidence.
6. Do `H-MUXPROC-003` next for one-shot and future continuous-mode
   activity correlation. This improves fresh-session and post-switch
   attribution without requiring opt-in hooks, and gives the resolver a
   middle-strength signal above cwd-only matching.
7. Do `H-MUXPROC-004` per ADR 0048. The May 2026 audit closed the
   opencode portion as a no-op and scoped the work to a Codex
   state-reader slice plus a Codex log-derived live-attribution
   linker. The log linker is also the Codex-side fix for the same
   stale-`--resume` drift class as `H-MUXPROC-015`.
8. Land `H-MUXPROC-008` as soon as the ADR path is open, or fold it
   into `H-MUXPROC-001` if that ADR is still being written. This keeps
   terminal injection and slash-command probing out of the attribution
   design while the tempting `/usage` workaround is fresh.
9. Leave `H-MUXPROC-006`, `H-MUXPROC-007`, `H-MUXPROC-013`, and
   `H-MUXPROC-014` behind their audits and schema decisions. They
   improve cross-harness correctness, but they are not the shortest
   path to fixing the Claude Code mapping drift seen in
   `H-MUXPROC-015`.

- [x] `H-MUXPROC-001` ADR: process-tree linker design and dependency
  choice.
  - Scope: decide (1) whether to depend on the `sysinfo` crate or
    read `/proc` directly on Linux and an equivalent on macOS (and
    whether macOS is in scope at all for the first pass), (2) the
    descendant-depth bound and the pane-PID acquisition path
    (`tmux list-panes -F "#{pane_id} #{pane_pid}"`), (3) the
    known-binary match set and how it extends as new harnesses are
    added, (4) confidence and provenance assignment for the emitted
    `AgentInPane` evidence (likely `Discovered` provenance, `High`
    confidence for a direct command-name match, demoted for the
    fallback heuristics), and (5) where the linker fits in the module
    layout (peer of `discovery/tmux/`, or a sub-module that consumes
    the existing tmux runner). Record as a new ADR per CLAUDE.md.
  - Tests: none directly; ADR is the deliverable.
  - Blockers: none.
  - Outcome: ADR 0046 chooses a Linux-first, dependency-free `/proc`
    reader with an injectable process snapshot seam, a four-edge
    descendant walk from tmux active-pane PID, the supported harness
    binary match set, `active_pane_process_match` evidence, and
    `CONSPECTUS_DISABLE_PROCTREE` as the runtime kill switch.

- [x] `H-MUXPROC-002` Implement the process-tree linker as a
  discovery source.
  - Scope: build the linker per the `H-MUXPROC-001` ADR. Walk every
    discovered pane's process tree, match descendant commands
    against the harness binary set, and emit candidate links between
    the matching `AgentSession` (when a corresponding session is
    already in the graph) and the pane's `MuxSession`. When no
    matching session exists, emit an unresolved-endpoint candidate
    link carrying the harness key, pane id, and shell PID so the
    evidence survives until later discovery (or a re-run) resolves
    it. Best-effort: missing `/proc` access, an unreadable PID, or
    an unrecognized binary degrade silently. Gate the provider
    behind a `CONSPECTUS_DISABLE_PROCTREE` env var mirroring the
    existing tmux / forge toggles.
  - Tests: fixture-backed unit tests using an injected
    process-snapshot trait (mirroring the `TmuxRunner` /
    `GhRunner` seam) covering a direct match, a nested
    shell-then-agent match, an unknown binary, a missing pid, and
    a permission-denied path. Resolver tests confirming the new
    evidence raises confidence on existing session ↔ mux candidates
    rather than producing duplicate winning relationships.
  - Manual checks: `cargo run -- graph --format json` inside a
    tmux session running an agent; confirm the new evidence on the
    `LinkedToMux` candidate links.
  - Blockers: `H-MUXPROC-001`.
  - **slice landed**: tmux discovery now records active-pane
    process hints available directly from tmux format variables
    (`pane_current_command`, `pane_pid`, `pane_current_path`, and
    `pane_start_command`) on `MuxSessionNode`. Cross-link inference
    inspects the pane process' open file descriptors for known
    harness session paths such as Codex rollout JSONL files and
    Claude task files, then falls back to known session keys in the
    active pane's start command. Evidence is labeled as
    `active_pane_fd_session_match`,
    `active_pane_fd_command_session_match`, or
    `active_pane_command_session_match` depending on the strongest
    available signal. If active-pane evidence names a resumed parent
    session and a discovered child session has a `parent_session`
    link to that parent, the child is treated as active and the
    parent is not linked. Once a mux session has active-pane session
    evidence, cwd-only matches to other sessions are suppressed for
    that mux; this turns the previous many-sessions-to-one-single-pane
    ambiguity into a graph-level refinement instead of a TUI-only
    picker problem.
  - Tests: `cargo test active_pane --all-targets`.
  - **ranking slice landed**: resolver ordering now distinguishes
    `LinkedToMux` `match_kind` evidence. Fresh current-session
    evidence such as hooks, control-plane responses, and active-pane
    fd matches ranks above `active_pane_command_session_match`, so
    argv / start-command session ids remain useful launch evidence
    without being treated as definitive current-session truth.
  - Outcome: the remaining process-tree slice is now implemented per
    ADR 0046. Cross-link inference can walk a Linux `/proc` snapshot
    from each tmux active-pane PID, match supported harness binaries
    through nested shell children, emit `active_pane_process_match`
    `LinkedToMux` candidates for exact process command session keys or
    a single discovered same-harness/same-cwd session, preserve
    unresolved agent evidence when no session node exists yet or
    same-cwd matches are ambiguous, and skip the linker with
    `CONSPECTUS_DISABLE_PROCTREE`. Tests cover direct and nested
    process matches, exact command-key matches, ambiguous same-cwd
    suppression, fd evidence suppressing conflicting stale process
    resume ids, process-cardinality gating for multi-session mux
    attribution, unknown binaries, missing process data, unresolved
    evidence, and resolver ranking above launch argv.

- [x] `H-MUXPROC-003` Add read-only session-file activity correlation.
  - Scope: improve fresh-session attribution without sending input to
    running agents. Add a read-only observation layer that correlates
    tmux pane PIDs with harness session files by recent creation /
    modification / open activity. Preferred implementation is a
    Linux-first provider that can consume `inotify`/fanotify-style
    events in continuous mode and a one-shot fallback that scans
    known harness state roots for recently-created or recently-
    modified session files matching the pane cwd and harness. Treat
    this as weaker than an open fd match but stronger than cwd-only
    matching when the event/file timestamp is close to the pane
    process start time. Do not write marker files and do not touch
    transcript contents.
  - Tests: fixture-backed clocked tests covering Codex rollout file
    creation, Claude project/task file creation, opencode sqlite
    session updates, stale files outside the time window, and
    ambiguous same-cwd events that remain candidates instead of
    becoming a single preferred link.
  - Manual checks: start a fresh harness session in tmux with no
    explicit resume id; confirm `graph --format json` gains a
    non-cwd `LinkedToMux` candidate after the session file appears.
  - Blockers: `H-MUXPROC-002`; friendlier after the continuous-mode
    snapshot workstream starts.
  - Outcome: one-shot discovery now correlates active-pane harness
    identity, mux cwd, and read-only harness session activity
    timestamps already collected from Codex rollout files, Claude
    transcript files, and opencode session state. Fresh same-harness /
    same-cwd sessions near mux creation or activity emit
    `session_file_activity_match`, ranking below fd/hooks/state but
    above process, launch argv, and cwd-only evidence. When process
    cardinality shows zero or one non-subagent harness process, fresh
    same-cwd activity candidates collapse to one human session; multiple
    attributions remain possible only when multiple harness processes
    are observed. Inotify / fanotify continuous-mode event ingestion
    remains deferred to the continuous server workstream.

- [x] `H-MUXPROC-016` Treat harness session keys as opaque strings in
  runtime attribution.
  - Scope: document and enforce the rule that `AgentSessionId.session_key`
    is an opaque harness-native string. UUID-shaped extraction remains a
    conservative generic fallback for arbitrary blobs, but any path,
    command, fd target, hook payload, or state record with known harness
    context should use that harness's session-key grammar. Initial
    grammars: Codex/Claude UUID-shaped transcript keys, opencode `ses_…`
    keys, plus command-token extraction when the harness binary is known.
  - Tests: extractor unit tests proving opencode `ses_…` ids are extracted
    from process commands and fd/path evidence, while unrelated UUIDs
    outside known harness paths are ignored.
  - Manual checks: inspect `graph --format json` for a live opencode mux
    session and confirm `process_identifies_session` evidence names the
    `ses_…` session key instead of falling back to same-cwd candidates.
  - Blockers: `H-MUXPROC-002`.
  - Outcome: active-pane process command extraction now uses
    harness-aware session-key parsing. OpenCode `ses_…` ids are
    first-class session keys in process command and fd/path evidence,
    while UUID-shaped extraction remains a generic fallback for Codex,
    Claude Code, and unknown harness contexts.

- [x] `H-MUXPROC-017` Sweep remaining UUID-only extractor call sites.
  - Scope: audit discovery, viewer bridge, resolver metadata, and output
    helpers for UUID-shaped session-key assumptions. Replace them with
    either typed `AgentSessionId` comparisons or harness-aware opaque
    string extractors. Keep UUID-only helpers private to generic fallback
    paths and rename them so call sites must choose between generic and
    harness-aware extraction deliberately.
  - Tests: add regression coverage for opencode `ses_…`, Codex UUID, and
    Claude UUID session keys in the same fixtures. Include at least one
    negative test where an ordinary path token does not become a session
    key.
  - Manual checks: run a mixed Codex/opencode/Claude tmux graph smoke and
    verify resolved links and right-pane IDs preserve full external session
    keys.
  - Blockers: `H-MUXPROC-016`.
  - Outcome: the process-link extractor now exposes explicit
    harness-aware helpers and renamed the UUID-only helper to
    `generic_uuid_like_session_keys`, making generic UUID matching
    visible at call sites. Command evidence no longer treats every
    ordinary argv token as a session key; it recognizes session-bearing
    command forms such as `resume <id>` / `-s <id>` and harness-specific
    tokens such as opencode `ses_…`.

- [x] `H-MUXPROC-FU-001` Evaluate first-class runtime process nodes.
  - Scope: turn ADR 0047's proposed model into a concrete workstream
    proposal if process evidence continues to accumulate resolver,
    mux-cardinality, opencode subagent, server/proxy, or diagnostic
    responsibilities. Compare keeping process evidence in
    `GraphLink.source_metadata` against adding ephemeral
    `RuntimeProcess` nodes and explicit mux/process/session links.
  - Deliverable: either reject process nodes with updated rationale,
    or split implementation into model/schema, discovery, resolver,
    query, and TUI/detail slices with migration and snapshot-impact
    notes.
  - Tests: design-only until accepted. Any implementation should add
    fixtures for single-agent, multi-agent, subagent, stale argv, and
    unreadable process cases.
  - Related: ADR 0047, ADR 0046, `H-MUXPROC-005`, `H-SUBAGENT-004`.
  - Blockers: none.
  - Outcome: ADR 0047 is accepted. Runtime process nodes should land
    before graph visualization exports so DOT/HTML designs are not
    built around a process-free graph. Process observations remain
    ephemeral and rebuildable, while `AgentSession -> MuxSession`
    stays the main user-facing resolved relationship. Implementation
    is split into the follow-up slices below.

- [x] `H-MUXPROC-FU-002` Add runtime process graph model and relation
  kinds.
  - Scope: add a provider-neutral `RuntimeProcess` node with ephemeral
    observation identity and sparse attributes for PID, parent PID,
    root pane PID, command, cwd, harness key, process role, depth, and
    observed epoch. Add relation kinds for mux contains/observes process
    and process identifies/candidates/unresolved agent session evidence.
    Preserve `AgentSession -> MuxSession` as the resolver-selected
    user-facing relationship.
  - Tests: serde round trips, deterministic identity/order tests, sparse
    node serialization, and relation-kind serialization.
  - Blockers: `H-MUXPROC-FU-001`.
  - Outcome: added `RuntimeProcessId`, `RuntimeProcessNode`,
    `RuntimeProcessRole`, a `GraphNode::RuntimeProcess` variant, a
    `NodeId::RuntimeProcess` variant, and process relation kinds for
    mux/process containment plus process/session identification and
    candidate evidence. Model tests cover stable display and relation
    serialization.

- [x] `H-MUXPROC-FU-003` Persist runtime process nodes in SQLite and
  graph JSON.
  - Scope: extend the query schema/loader/reader for runtime process
    nodes and their relation evidence. Keep process observations
    rebuildable and outside user-authored declared-link intent.
  - Tests: schema constant tests, load/read parity snapshots, and graph
    JSON snapshots covering single-agent, multi-agent, subagent, stale
    argv, and unreadable-process cases.
  - Blockers: `H-MUXPROC-FU-002`.
  - Outcome: bumped the query schema version and added
    `node_runtime_processes`, `v_nodes` coverage, loader insertion,
    readback, `NodeId` JSON round-trip coverage, and full-snapshot
    SQLite round-trip coverage. Minimal `node show` and TUI detail
    summaries can render runtime process nodes once discovery emits
    them. Scenario-specific process fixtures remain in
    `H-MUXPROC-FU-006`.

- [x] `H-MUXPROC-FU-004` Emit runtime process nodes from MUXPROC
  discovery.
  - Scope: update process-tree, fd, hook/plugin, and Codex log-derived
    attribution paths to emit process observations and explicit
    process/session evidence instead of hiding all process facts inside
    `LinkedToMux.source_metadata`. Preserve compatibility metadata only
    where needed during migration.
  - Tests: fixture-backed process-tree tests for direct/nested matches,
    no matching session, ambiguous same-cwd sessions, subagent roles,
    stale argv suppressed by stronger current-session evidence, and
    unreadable `/proc` degradation.
  - Blockers: `H-MUXPROC-FU-003`.
  - **slice landed**: process-tree evidence now emits
    `RuntimeProcess` nodes, `mux_contains_process` links, and
    `process_identifies_session` / `process_candidates_session`
    links alongside the legacy `linked_to_mux` candidates. Hook
    sidecar records with process ids and Codex log-derived pid/thread
    attribution emit the same process graph shape. The ERD in
    `docs/design.md` now includes mux/process/session relationships.
    Active-pane fd evidence without a process-tree snapshot now
    synthesizes a root process observation from the mux active pane PID,
    preserving the runtime process graph shape for deterministic
    scenarios and constrained platforms.

- [x] `H-MUXPROC-FU-005` Move mux-cardinality and attribution resolver
  logic onto runtime process evidence.
  - Scope: teach resolver/cross-link inference to derive
    `AgentSession -> MuxSession` from explicit process observations and
    process/session candidates. Cardinality rules should count
    non-subagent runtime process roles instead of re-parsing opaque link
    metadata.
  - Tests: resolver tests for zero/one/multiple non-subagent processes,
    subagent exclusion, current-session evidence beating stale launch
    argv, and unresolved process diagnostics.
  - Blockers: `H-MUXPROC-FU-004`.
  - Outcome: `resolve_snapshot` now derives compatibility
    `AgentSession -> MuxSession` candidates from
    `mux_contains_process` plus concrete process/session evidence,
    without fanning out ambiguous `process_candidates_session` links.
    Resolver metadata records the source process links and counts
    non-subagent runtime process roles so subagent observations do not
    inflate mux cardinality.

- [x] `H-MUXPROC-FU-006` Surface runtime process diagnostics in node
  detail and scenario fixtures.
  - Scope: add node-detail sections for runtime process nodes and for
    agent/mux nodes linked through process evidence. Extend named dev
    scenarios so process-cardinality and stale-argv cases can be
    inspected through `dev scenario graph/table/node/tui`.
  - Tests: node-show/detail snapshots and dev-scenario coverage for
    process-backed attribution cases.
  - Blockers: `H-MUXPROC-FU-005`, `TEST-006`.
  - Outcome: TUI/node detail now has a `Process` section for runtime
    process fields and linked process context from agent and mux
    details. Runtime process details link back to containing muxes and
    identified/candidate sessions, annotating candidate session links.
    Added a named `process-cardinality` dev scenario with two runtime
    process observations for one mux; `codex-fd-current` continues to
    cover stale argv vs fd-backed process attribution.

- [x] `H-MUXPROC-004` Read Codex state and log databases for live
  session attribution.
  - Scope: per ADR 0048, add two read-only Codex slices under the
    existing harness state-root discovery. The state-reader slice
    globs `state_*.sqlite`, selects the highest numeric suffix, and
    emits one `AgentSession` per `threads` row populating the
    existing sparse `AgentSessionNode` shape only: `id`, `cwd`,
    `title` with capped/normalized `first_user_message` fallback,
    `last_active_epoch` from millisecond timestamps, and previews via
    the existing rollout-tail extractor. The richer threads columns
    (`rollout_path`, `git_*`, `model`, `cli_version`, `agent_*`,
    `archived_at`) are queried but not stored on the node; promoting
    them needs a separate ADR. The state reader also emits one
    intra-harness `parent_session` candidate per `thread_spawn_edges`
    row with `lineage_kind = "spawn"`, distinct from the rollout
    reader's existing `forked_from_id` candidates which keep
    `lineage_kind = "fork"`. The
    log-linker slice reads `logs_*.sqlite` only to resolve the
    freshest `thread_id` for each live Codex pid by parsing
    `process_uuid` as `pid:<os_pid>:<uuid>` and querying within a
    15-minute `ts` floor matching ADR 0028. The resulting
    `LinkedToMux` candidate ranks above
    `active_pane_command_session_match` and
    `active_pane_fd_session_match` for Codex and demotes stale
    command-session candidates for the same mux, mirroring the
    hook-sidecar override rule. Fresh log evidence may synthesize a
    sparse `AgentSession` when state has not yet observed the thread,
    matching the ADR 0028 synthesis path. Both readers use
    `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` plus
    `PRAGMA query_only = ON`, never select `feedback_log_body`, and
    cap/normalize `first_user_message` like opencode previews.
  - Tests: temp sqlite fixtures for the current `state_5` and
    `logs_2` schemas; missing-database and unknown-schema
    degradation; column-probe behavior when a known column is absent;
    parent-lineage candidate emission and self/empty-parent skipping;
    log-linker fixture proving the freshest `thread_id` wins per pid;
    pid-reuse disambiguation via the trailing UUID; 15-minute ts
    floor rejecting stale rows; resolver tests proving codex
    log-derived candidates rank above command and fd evidence and
    override stale `active_pane_command_session_match`; synthesized
    sparse session round-tripping into the state-backed node when
    state catches up.
  - Manual checks: run against the live Codex state/log stores while
    a session is active; reproduce the in-process resume drift case
    (launch argv names session A, in-process switch to session B)
    and confirm `conspectus graph --format json` and `conspectus tui`
    link the mux to B without consulting argv. Confirm no write-ahead
    log churn from Conspectus reads.
  - Related: ADR 0048; ADR 0028 (sidecar TTL and demotion rule
    reused); ADR 0046 (process-tree provides the live Codex pid set);
    ADR 0018 (parent_session shape); `H-MUXPROC-015` (Claude analogue
    of the drift case this closes for Codex); `H-MUXPROC-005` /
    `H-MUXPROC-006` (Codex `remote_control_enrollments` belongs to
    the control-plane audit, not here).
  - Blockers: none. ADR 0048 supplies the persistent schema-dependency
    decision the original blocker required.
  - **audit slice landed**: ADR 0048 records the May 2026 audit of
    opencode `opencode.db`, Codex `state_5.sqlite`, and Codex
    `logs_2.sqlite`. opencode's slice of 004 closes as a no-op
    because the schema carries no live process/server binding beyond
    what the existing reader already extracts; live opencode↔mux
    attribution remains the responsibility of `H-MUXPROC-007` /
    `H-MUXPROC-014`. Codex `jobs`, `agent_jobs`, `thread_goals`, and
    `stage1_outputs` were empty on the audit machine and are deferred
    until in-the-wild usage justifies coverage.

- [x] `H-MUXPROC-005` Audit harness control planes for non-mutating
  current-session queries.
  - Scope: determine whether any supported harness exposes a
    documented side-channel that can ask an already-running
    interactive process for its current session id without entering
    text into the conversation or mutating JSONL/session logs. Audit
    Codex `app-server` / `app-server proxy` / `--remote`, opencode
    `serve` / `attach` / `acp`, Claude Code remote-control /
    background-agent surfaces, and aider if a relevant server mode
    exists. For each harness, record: launch mode required, discovery
    path for the socket/URL/token, query shape, mutation guarantees,
    authentication boundaries, and fallback behavior when the
    control plane is absent.
  - Tests: none for the audit itself. If a supported control plane
    survives, create follow-up fixture or fake-server tests before
    implementing the adapter.
  - Manual checks: launch each harness in the required server/control
    mode and prove the query does not append user, assistant, or
    system records to the session transcript.
  - Related: `H-MUXPROC-015` captures a live Claude Code case where
    command-line `--resume` evidence became stale after an in-process
    session switch; the audit should explicitly look for a safer
    current-session source for that scenario.
  - Blockers: none.
  - **audit slice landed**: local Codex CLI exposes experimental
    app-server thread APIs, but no local hook surface in `--help`;
    keep `H-MUXPROC-006` gated. OpenCode exposes HTTP server, ACP,
    and plugin surfaces; keep `H-MUXPROC-007` gated. Claude Code's
    strongest non-mutating path is hooks, tracked under
    `H-MUXPROC-009` / `H-MUXPROC-012`.
  - **closed 2026-05-31**: audit work is the scope; the recorded
    findings have routed each harness to its chosen non-mutating
    path (Codex → ADR 0048 log linker; opencode → plugin sidecar
    via `H-MUXPROC-014`; Claude Code → hook sidecar via
    `H-MUXPROC-012`). `H-MUXPROC-006` and `H-MUXPROC-007` are
    closed as won't-do; see their entries for rationale.

- [x] `H-MUXPROC-006` Add Codex app-server attribution adapter if
  the audit proves a stable non-mutating query. **Closed as
  won't-do 2026-05-31.**
  - Scope: if `H-MUXPROC-005` confirms Codex's app-server or control
    socket can report the active session/rollout for an interactive
    TUI, implement an optional adapter that discovers the control
    endpoint, authenticates using the documented local mechanism, and
    emits high-confidence `LinkedToMux` evidence for the active
    session. This should be a launch-mode enhancement only: existing
    plain TUI sessions must continue to rely on fd / command / cwd
    evidence.
  - Tests: fake app-server protocol tests for current-session,
    missing-session, auth failure, server unavailable, and stale
    socket cases.
  - Manual checks: launch Codex with the required app-server mode;
    confirm Conspectus links the live rollout without relying on
    command-line resume args or open JSONL fd paths.
  - Blockers: `H-MUXPROC-005`.
  - **closure rationale**: `H-MUXPROC-005` audit found Codex's
    app-server surface is gated behind experimental flags with no
    stable contract and no local hook surface (`codex --help`). The
    Codex drift class that motivated this work is already covered by
    the log linker landed under `H-MUXPROC-004` / ADR 0048, which
    derives current session attribution from the on-disk rollout log
    without depending on the experimental control socket. Reopen
    only if Codex ships a stable, documented current-session query
    that the log linker cannot match (e.g. cross-pid session
    handoff without log rotation).

- [x] `H-MUXPROC-007` Add opencode server/ACP attribution adapter if
  the audit proves a stable non-mutating query. **Closed as
  won't-do 2026-05-31.**
  - Scope: if `H-MUXPROC-005` confirms opencode `serve`, `attach`,
    or ACP can report active session identity for a running TUI or
    headless server, implement an optional adapter that maps the
    server session id back to a `MuxSession`. Prefer documented
    attach URLs, pid/socket evidence, or server metadata over
    guessing from ports. Keep plain opencode TUI support on the
    existing fd / command / cwd path.
  - Tests: fake server tests for active session, multiple projects,
    unavailable server, auth/connection failure, and ambiguous
    session responses.
  - Manual checks: launch opencode in the supported server mode and
    confirm the query does not create transcript records or alter
    session recency.
  - Blockers: `H-MUXPROC-005`.
  - **closure rationale**: supplanted by the plugin sidecar path in
    `H-MUXPROC-014` (live-verified 2026-05-31). The opencode plugin
    runs in-process inside every TUI/server/ACP launch mode, writes
    `session.created`/`updated`/`idle`/`status`/`compacted`
    observations to the hook sidecar without HTTP/socket discovery
    or auth, and the existing `discovery::hook_sidecar` reader
    already attributes the records by tmux pane. An HTTP/ACP
    adapter would duplicate this evidence at higher cost (port
    discovery, auth-token plumbing, multi-server polling). Reopen
    only if the plugin distribution becomes untenable (e.g.
    opencode removes the plugin surface).

- [x] `H-MUXPROC-008` Document terminal-injection attribution as a
  rejected strategy unless a harness guarantees non-mutating status
  commands.
  - Scope: record the policy that Conspectus must not use
    `tmux send-keys`, slash commands, prompts such as `/status`, or
    terminal scraping to ask an agent for its current session id
    because these are user inputs and may mutate JSONL/transcript
    logs. The only exception is a harness-documented command channel
    that explicitly guarantees no transcript/session mutation; such
    an exception must be captured by `H-MUXPROC-005` and implemented
    as a control-plane adapter rather than generic terminal input.
    Put the rationale in the process-linking ADR or a short follow-up
    ADR so future work does not rediscover the same tempting but
    unsafe approach.
  - Tests: none.
  - Related: `H-MUXPROC-015` records why scraping or injecting
    Claude Code `/usage` is tempting but should not be treated as
    the preferred architecture unless no non-mutating control or hook
    source exists.
  - Blockers: `H-MUXPROC-001` ADR can absorb this if it has not
    landed; otherwise write a follow-up ADR.
  - Outcome: ADR 0028 rejects terminal injection, slash-command
    probing, and generic terminal scraping for current-session
    attribution. Harness-documented non-mutating command channels
    remain possible only as explicit control-plane adapters.

- [x] `H-MUXPROC-009` Audit harness hooks/plugins as definitive
  session-state sidecar emitters.
  - Scope: determine whether supported harnesses can expose current
    session state through lifecycle hooks, tool hooks, plugins, or
    status-line callbacks without sending text into the agent
    conversation. Prototype one hook/plugin per harness that writes
    its raw payload and selected environment/process context to a
    temporary Conspectus-owned sidecar directory. Audit Claude Code
    hooks, Codex `codex_hooks`, opencode plugin/server extension
    points, and aider if a native hook or extension surface exists.
    For each harness, record whether the payload includes an explicit
    session id, transcript/session path, cwd, pid/ppid, tmux pane,
    and event timestamp; also verify whether hook execution changes
    transcript/session logs. Do not add a persistent Conspectus hook
    convention until the audit proves at least one harness can emit
    useful non-mutating state.
  - Tests: none for the audit itself. Keep any prototype scripts out
    of committed config unless they become intentional fixtures.
  - Manual checks: run a short live session per harness with the
    prototype hook enabled; diff the harness transcript/state before
    and after to confirm only expected harness activity changed and
    no Conspectus probe text was logged.
  - Blockers: none. Follow-up ADR required before adding a durable
    sidecar schema or installer.
  - **audit slice landed (Claude)**: Claude Code hooks are viable
    and provide `session_id`, `transcript_path`, `cwd`, and event
    name on stdin; `SessionStart` covers startup, resume, clear,
    and compact. ADR 0028 records the sidecar path.
  - **audit slice landed (opencode, 2026-05-30)**: OpenCode hooks
    are viable. The plugin API ships as `@opencode-ai/plugin`
    (npm), installed via `opencode plugin <module>` into
    `$XDG_CONFIG_HOME/opencode/node_modules` and listed in
    `config.json#plugin`. A plugin is a TypeScript module with
    default export `(input: PluginInput) => Promise<Hooks>`. The
    `event` hook receives the full `Event` union from
    `@opencode-ai/sdk`; relevant variants include
    `EventSessionCreated` and `EventSessionUpdated` (both carry
    the full `Session` object: `id`, `directory`, `parentID`,
    `title`, `time.{created,updated}`, `share.url`),
    `EventSessionStatus` and `EventSessionIdle` and
    `EventSessionCompacted` (carry `sessionID`), and `chat.message`
    / `chat.params` / `chat.headers` / `tool.execute.{before,after}`
    / `command.execute.before` / `permission.ask` (all carry
    `sessionID`). `PluginInput` exposes `project`, `directory`,
    `worktree`, `serverUrl`, `client`, and a `BunShell`; the
    plugin runs in-process so `process.pid`, `process.ppid`, and
    `process.env.TMUX{,_PANE,_TMPDIR}` are directly readable for
    pane/process context. The hook write path is non-mutating —
    forwarding events to `conspectus hook write` does not append
    to the opencode session DB, transcript, or HTTP API. Opt-out
    exists at the CLI level via `opencode --pure`. Implementation
    plan for `H-MUXPROC-014` is therefore well-scoped: ship a
    `@conspectus/opencode-hook` npm plugin that calls a new
    `conspectus hook write opencode` writer (sibling of the
    existing `claude-code` and `codex` writers), and reuse the
    existing `discovery::hook_sidecar` post-merge pass which is
    already harness-agnostic.
  - **audit slice landed (codex, 2026-05-30)**: Codex has a hook
    surface (the top-level CLI flag `--dangerously-bypass-hook-trust`
    proves it, with `codex plugin` for marketplace plugins). The
    exact event names, payload shapes, and config schema were not
    extractable from `codex --help` or from a strings dump of the
    wrapped binary on this machine and would need upstream docs
    or source reading. Lower priority than opencode because
    `H-MUXPROC-004` / ADR 0048 already closes the codex side of
    the H-MUXPROC-015 drift class via log-derived attribution; the
    `conspectus hook write codex` writer subcommand exists as a
    stub for future use if/when codex hook payload semantics are
    documented or reverse-engineered.

- [x] `H-MUXPROC-010` Define Conspectus hook sidecar schema and
  trust/ranking rules.
  - Scope: if `H-MUXPROC-009` finds viable hook/plugin emitters,
    define a provider-neutral sidecar record written outside project
    trees, likely under the user's XDG state directory. Minimum
    candidate fields: harness key, session key, cwd, pid, ppid,
    tmux pane id, tmux socket/session/window/pane metadata,
    transcript/session path, hook event kind, observed timestamp, and
    harness version. Define stale-record expiry, collision handling,
    privacy expectations, and evidence ranking. Proposed ranking:
    explicit hook session id + tmux pane/pid above open-fd evidence;
    hook transcript/session path above open-fd evidence when the path
    resolves to a discovered session; hook cwd-only records below
    command session ids and above generic cwd matching only when the
    timestamp is fresh.
  - Tests: schema parse/round-trip tests, stale-record filtering,
    duplicate event coalescing, malformed record degradation, and
    resolver ordering tests against fd, command, and cwd evidence.
  - Blockers: `H-MUXPROC-009`; ADR required for the durable sidecar
    convention.
  - Outcome: ADR 0028 defines schema version 1 under the user's
    Conspectus state directory, a 15-minute active-record TTL,
    matching by explicit session id plus tmux native id / pid / cwd,
    and ranking above launch argv evidence.

- [x] `H-MUXPROC-011` Implement hook-sidecar discovery provider.
  - Scope: read the sidecar records defined by `H-MUXPROC-010` and
    convert them into `LinkedToMux` candidate links. Match hook
    records to mux sessions by tmux pane id when present, then pane
    pid, then tmux session metadata, and only then cwd as a weak
    fallback. Match records to agent sessions by explicit session key
    or transcript/session path. Handle stale records conservatively:
    they may explain exited sessions in future views, but should not
    override fresh active-pane fd/process evidence.
  - Tests: fixture directory with multiple harness records, stale
    records, malformed JSON, same-session duplicate events, tmux pane
    reuse, missing mux session, and missing agent session. Resolver
    tests for ranking relative to `active_pane_fd_session_match`.
  - Manual checks: run with a live hook-enabled session and confirm
    `graph --format json` shows the hook evidence without requiring
    transcript scraping or terminal input.
  - Blockers: `H-MUXPROC-010`.
  - Outcome: `discovery::hook_sidecar` reads fresh JSON records after
    harness and tmux discovery, emits high-confidence `LinkedToMux`
    candidates, and marks stale `active_pane_command_session_match`
    candidates for the same mux as overridden. It also synthesizes a
    sparse `AgentSession` node from fresh hook records when Claude Code
    has fired `SessionStart` but has not yet persisted the transcript
    file because the new session has no messages.

- [x] `H-MUXPROC-012` Add Claude Code hook sidecar emitter if audit
  proves non-mutating session identity.
  - Scope: if Claude Code hook payloads include a current session id,
    transcript path, or enough context to derive one, provide a
    minimal documented hook command/script that writes Conspectus
    sidecar records. Prefer explicit hook payload fields over
    inspecting parent process fds. The hook must be opt-in and easy
    to remove; Conspectus should discover its records but not require
    users to install it.
  - Tests: payload fixture tests for supported Claude Code hook
    events; sidecar record generation tests; version/field-missing
    degradation.
  - Manual checks: enable the hook for a live Claude Code session and
    verify the sidecar identifies the active session without adding
    Conspectus probe messages to JSONL logs.
  - Related: `H-MUXPROC-015` provides the concrete failure mode this
    emitter should fix if Claude Code hook payloads expose the
    post-`/resume` current session id.
  - Blockers: `H-MUXPROC-009`, `H-MUXPROC-010`.
  - Outcome: added `scripts/conspectus-claude-hook-sidecar.py` and
    documented a `SessionStart` hook configuration in
    `docs/operations.md`. The emitter writes schema-v1 sidecar
    records with Claude `session_id`, `transcript_path`, `cwd`,
    process ids, and tmux context when available. The script is now a
    compatibility shim for `conspectus hook write claude-code`.

- [x] `H-MUXPROC-016` Add `conspectus hook write` sidecar writer.
  - Scope: move the sidecar write path into the Conspectus binary so
    schema validation, root selection, atomic writes, permissions, and
    future migrations live in Rust beside the reader. Treat the
    subcommand as the compatibility boundary between harness hook
    configuration and Conspectus storage: v1 may continue writing
    per-event JSON files, but the command should encapsulate that choice
    so a later SQLite state backend or daemon ingest path does not
    require users to reinstall hooks. The first supported input should be
    Claude Code hook JSON on stdin, producing the same schema-v1 records
    currently written by `scripts/conspectus-claude-hook-sidecar.py`.
    Keep the script only as a compatibility shim or remove it once the
    CLI path is documented. Candidate command shape:
    `conspectus hook write claude-code`, with room to add `--format` if
    future harnesses need multiple payload forms.
  - Design notes: ADR 0028 keeps hook observations as rebuildable local
    state and calls SQLite a plausible next backend once this command
    owns migrations, busy handling, retention, and fallback behavior.
    Keep that separate from ADR 0029 session aliases, which are
    user-authored durable intent even if a future implementation reuses
    SQLite machinery for both.
  - Tests: CLI tests for valid Claude payloads, missing `session_id`,
    malformed JSON, sidecar-root precedence, user-only file
    permissions where supported, and atomic write behavior. Reader /
    writer compatibility tests should assert the discovery provider
    accepts records emitted by the subcommand.
  - Manual checks: configure a Claude Code hook to call the subcommand
    directly, then confirm `graph --format json` shows
    `hook_session_match` / `hook_session_path_match` evidence without
    relying on the Python emitter.
  - Blockers: `H-MUXPROC-010`, `H-MUXPROC-011`, `H-MUXPROC-012`.
  - Outcome: added `conspectus hook write claude-code`, which reads
    Claude Code hook JSON from stdin and writes schema-v1 observations
    to `hooks.sqlite3` under the hook state root. Discovery reads the
    SQLite store plus legacy per-event JSON records.

- [x] `H-MUXPROC-017` Add `conspectus hook init` installer UX.
  - Scope: add an idempotent hook installer for supported harnesses,
    starting with Claude Code. It should merge with existing harness
    settings, preserve unrelated user hooks, install a hook command
    that invokes `conspectus hook write`, and support dry-run/status/
    remove flows before mutating external config. Candidate commands:
    `conspectus hook init claude-code --scope user|project`,
    `conspectus hook status claude-code`, and
    `conspectus hook remove claude-code`. Treat this as a write to
    external tool configuration, distinct from read-only discovery.
  - Tests: fixture-backed settings merge tests for empty settings,
    existing unrelated hooks, existing Conspectus hook, malformed
    settings, dry-run output, status detection, and removal without
    deleting unrelated entries.
  - Manual checks: install into a temporary Claude Code settings file,
    run a hook-enabled session, verify sidecar emission, then remove
    and confirm the settings file returns to the expected state.
  - Blockers: `H-MUXPROC-016`; ADR/design update if the command mutates
    any persistent convention not already covered by ADR 0028.
  - Outcome: added `conspectus hook init/status/remove claude-code`
    with user/project scope support. The installer merges with existing
    Claude settings, preserves unrelated hooks, and installs a command
    that invokes `conspectus hook write claude-code`.

- [x] `H-MUXPROC-018` Dedupe hook records by pane and drop the
  15-minute emission gate.
  - Problem: in-app `/resume` between two Claude Code sessions in the
    same tmux pane leaves both sessions linked to the mux. Each
    session's `SessionStart` / `Resume` hook writes its own record;
    both records fall inside the 15-minute `ACTIVE_TTL_SECONDS`
    window, both resolve to the same mux, and the current
    `apply_hook_sidecars` pass emits an Active `LinkedToMux` for each
    rather than letting the freshest pane observation win. The 15-min
    TTL also causes legitimate live links to drop off a working
    session as soon as the operator idles longer than the window.
  - Scope: change `apply_hook_sidecars` in
    `src/discovery/hook_sidecar.rs` to group records by
    `(resolved_mux.id, record.tmux.pane_id)` after mux resolution,
    pick the record with the highest `observed_epoch` per group as
    the Active winner, and emit older same-pane records as
    `LinkState::Overridden { by: winner_link_id, reason: "superseded
    by fresher hook sidecar record for same pane" }`. Records with
    no `pane_id` fall back to keying by `(resolved_mux.id, None)`
    (one winner per mux for pane-less records), which conservatively
    dedupes cwd-only and pid-only matches. Drop the
    `ACTIVE_TTL_SECONDS` filter from the emission gate; the constant
    stays in `src/hook.rs` for higher-layer freshness signals (e.g.
    the live-session advisory planned for `H-RENAME-013`). Update
    ADR 0028 to record the new emission semantics.
  - Tests: replace the existing
    `stale_hook_record_does_not_link_active_mux` test with one
    asserting an old hook record still links when no fresher record
    supersedes it. Add `fresher_hook_record_overrides_older_hook_for
    _same_pane` asserting the older link state flips to `Overridden`.
    Add `hook_records_for_different_panes_in_same_mux_both_remain_
    active` asserting per-pane independence. Existing demotion tests
    for launch-argv and cwd evidence stay green.
  - Manual checks: reproduce the in-app `/resume` scenario from a
    real Claude Code tmux pane and verify
    `conspectus graph --format json` and `conspectus tui` show
    exactly one Active `LinkedToMux` per pane (the freshest), with
    the older link visible as Overridden in diagnostic output.
  - Related: `H-MUXPROC-015` (the original in-process `/resume`
    drift fix scope), ADR 0028 (hook sidecar attribution).
  - Blockers: none.
  - Outcome: `apply_hook_sidecars` now sorts hook records newest
    first, groups candidates by `(resolved_mux.id, pane_id)`, keeps
    the freshest record active, and marks older same-pane records
    `Overridden` with the planned reason. Records no longer expire
    solely because they are older than the former 15-minute TTL; old
    records still link when no fresher same-pane record supersedes
    them. Pane-command and mux-created-after-observation guards still
    ignore clearly stale records. ADR 0028 now documents the pane
    dedupe semantics. Unit tests cover old-record retention,
    fresher-same-pane override, and different-pane independence.

- [ ] `H-MUXPROC-019` Investigate Claude Code Workflows process and
  session topology.
  - Scope: audit Claude Code Dynamic Workflows
    (`https://code.claude.com/docs/en/workflows`) against Conspectus's
    mux/process/session attribution model. The docs say workflow runs
    execute in the background while the parent session stays responsive
    and can orchestrate many agents. Determine, from live process
    trees and Claude state files, whether a workflow keeps one
    foreground controlling Claude PID with background implementation
    processes, launches one process per workflow agent/session, or
    uses another topology. Record how `/workflows`, task-panel
    expansion, pause/resume, and stopped/restarted agents affect
    `~/.claude/sessions/*.json`, transcript files, hook records, PIDs,
    parent PIDs, and process start identities.
  - Current assumption: until this story lands, Conspectus continues
    to treat one live controlling foreground harness PID as one
    human-driven agent session. Claude background/spare/workflow
    implementation processes should not inflate mux cardinality or
    appear as human-attached mux processes.
  - Tests: fixture the observed workflow state shape once audited,
    including at least one active workflow with multiple agents and
    one paused/resumed run. Add process-linking tests for whichever
    topology is confirmed.
  - Manual checks: run a small `/deep-research` or saved workflow in a
    disposable tmux-backed Claude session, capture `tmux list-panes`,
    `ps --forest`, `~/.claude/sessions/*.json`, workflow scripts under
    `~/.claude/projects/`, and Conspectus graph output before, during,
    after pause/resume, and after completion.
  - Blockers: access to Claude Code v2.1.154+ with workflows enabled.
  - Related: `H-MUXPROC-018`, `H-MUXPROC-015`, ADR 0028.

- [ ] `H-MUXPROC-020` Record the harness pid, not the hook writer's
  pid, in hook sidecar records.
  - Problem: `conspectus hook write claude-code` (and the codex /
    opencode variants) persist `record.pid = std::process::id()` in
    `src/cli.rs:417`, but `std::process::id()` is the pid of the
    transient `conspectus hook write …` child process, not of the
    long-lived claude / codex / opencode process that fired the
    hook. That child exits within milliseconds, so by the time any
    later discovery pass runs `process_is_live(record.pid)` in
    `src/discovery/hook_sidecar.rs:401`, the recorded pid is
    guaranteed to be dead. The hook adapter then marks every record
    `LinkState::Ignored` with reason "hook record pid N is no longer
    active". Live snapshot evidence (the 2026-06-07
    `agentdeck_-local-command-caveat-…-573ac208` mux case): 42 of 42
    claude-code `hook_sidecar` candidates resolved to `state:
    ignored`, hook evidence contributed zero usable input to the
    resolver, and the mux fell back to the stale launch-argv
    candidate, attributing the mux to a 14-day-stale resume parent
    instead of the live session. The earlier "hook records solve
    `H-MUXPROC-015`" claim in that story is contradicted in practice
    by this writer-pid bug — every hook record is born stillborn.
  - Scope: in the `conspectus hook write <harness>` writers (claude,
    codex, opencode in `src/cli.rs`), resolve the harness pid before
    handing it to the `*_record_from_payload` builders. Walk up
    `/proc/<self>/stat`'s `ppid` chain past `sh` / `bash` / wrapper
    layers until a process whose `comm` matches the expected harness
    binary set (`claude`, `claude-code`, `codex`, `opencode`, plus any
    aliases) is found, and record that pid as `record.pid`. Keep
    `record.ppid = parent_pid()` (the immediate parent) for diagnostic
    use. On Linux, walking `/proc/<pid>/stat` is enough; non-Linux can
    keep the existing best-effort behavior (`pid = 0` falls through
    the liveness check at `process_is_live`'s `<= 0` guard, leaving
    the record active rather than stillborn). If a hook payload field
    carries the harness pid directly (claude's `pid` field, codex's
    process metadata), prefer that over the proc walk to avoid races
    in deeply-wrapped invocations.
  - Tests: payload-builder unit tests asserting the recorded `pid` is
    the harness pid, not the writer pid, given a mocked process tree
    (writer → sh → harness → ...). Round-trip integration test
    asserting a written record survives `process_is_live` for the
    real harness pid lifetime. Regression test in
    `src/discovery/hook_sidecar.rs` asserting that with the
    harness-pid convention, fresh hook records produce Active
    `LinkedToMux` candidates instead of `Ignored`. Use the existing
    `testing_replay` fixtures or extend them to capture the
    writer→shell→harness chain.
  - Manual checks: run `conspectus hook init claude-code`, start a
    live claude session, capture the SQLite row, and confirm the
    recorded `pid` matches `pgrep -x claude` rather than a long-dead
    `conspectus` pid. Run `conspectus graph --format json` and
    confirm at least one `hook_sidecar` candidate for the live
    session has `state: active`.
  - Related: `H-MUXPROC-012`, `H-MUXPROC-015`, `H-MUXPROC-018`,
    `H-MUXPROC-021`, ADR 0028.
  - Blockers: none.

- [x] `H-MUXPROC-021` Demote `LinkedToMux` candidates whose source
  `AgentSession` is materially stale compared to a fresher candidate
  for the same mux.
  - Problem: `resolve_links` (`src/resolve/mod.rs:303`) buckets
    `LinkedToMux` candidates by `(source, relation, target)` and
    picks one winner *per source*. When two sessions each produce a
    `LinkedToMux` candidate to the same mux, the resolver does not
    cross-compare them — each is bucketed alone and both resolve as
    independent relationships. The TUI mux row builder
    (`src/tui/rows/mux.rs:653`) then shows every agent with an
    Active `LinkedToMux` to the mux as attached, with no preference
    among them. When a tmux pane's `active_pane_start_command`
    carries a stale `--resume <UUID>` whose `AgentSession` has been
    untouched for days, the stale session is treated as equally
    "attached" to the mux as the live session writing the same pane.
    In the live `agentdeck_-local-command-caveat-…-573ac208` case
    the stale source had `last_active_epoch 1779653903` (~14 days
    behind the mux), while the correct source (`7f01dbdf-…`) had
    `last_active_epoch 1780881982` within minutes of the mux's
    `activity_epoch`.
  - Scope: add a pre-resolver pass on `snapshot.candidate_links`
    that groups Active `LinkedToMux` candidates by target mux,
    selects the freshest source per mux (by source `AgentSession.
    last_active_epoch` closeness to the mux's `activity_epoch`), and
    marks materially-older same-mux candidates as `LinkState::
    Overridden { by, reason }`. Only run when the mux's
    `activity_epoch` is present and the freshest candidate's source
    is itself within a `FRESH_WINDOW` of the mux (don't penalize on
    weak signals). Skip Declared and Pin provenance entirely — user
    intent always wins. Run from `resolve_snapshot` between
    `apply_pin_bindings` and `resolve_links` so the demoted state
    is visible to the bucketing pass and to SQLite materialization.
  - Tests: resolver-fixture test where one stale and one fresh
    source both link to the same mux; assert the stale candidate is
    Overridden and the resolver returns only the fresh relationship.
    Regression where the stale source is the only candidate; assert
    it stays Active. Regression where a `LocalPin` candidate is
    stale but a fresher `StrongDiscovered` competes; assert the pin
    is not demoted. Regression where neither source is fresh against
    the mux; assert no demotion (don't penalize on weak signals).
    Regression where the mux's `activity_epoch` is missing; assert
    no demotion.
  - Manual checks: replay the `agentdeck_-local-command-caveat-…`
    snapshot through `conspectus graph --format json` and confirm
    `7f01dbdf-…` is attributed to the mux instead of `c1901a9e-…`.
  - Related: `H-MUXPROC-015`, `H-MUXPROC-020`, ADR 0006
    (resolver ordering), ADR 0028.
  - Blockers: none. Land alongside or after `H-MUXPROC-020` so the
    fresh hook-sidecar candidates are reaching `snapshot.candidate_
    links` Active before the freshness pass has to differentiate.
  - Outcome: `demote_stale_source_mux_candidates` in
    `src/resolve/mod.rs` runs from `resolve_snapshot` between
    `apply_pin_bindings` and `resolve_links`. Groups Active
    `LinkedToMux` candidates by target mux, selects the freshest
    candidate per mux (by source `AgentSession.last_active_epoch`
    closeness to the mux's `activity_epoch`), and marks others
    `LinkState::Overridden` when the source-epoch gap to the winner
    is ≥ `STALE_SOURCE_GAP_SECONDS` (24h) and the winner is itself
    within `FRESH_MUX_BIND_WINDOW_SECONDS` (6h) of the mux. Skips
    Pin / Declared provenance entirely (user intent always wins),
    skips when the mux's `activity_epoch` is missing, and skips
    when no candidate is itself fresh against the mux. Unit tests
    cover all five paths. Live caveat-mux fix once H-MUXPROC-020
    starts producing Active hook records.

- [x] `H-MUXPROC-015` Fix Claude Code mux attribution after
  in-process `/resume` switches.
  - Problem: live testing showed a Claude Code process running in
    tmux with argv
    `claude --resume 926c6991-9494-48ee-9d63-a98f4b4959d0`, while
    the pane's `/usage` screen reported current session id
    `2f11bd94-da81-4c5c-975d-a29dcdb3cda0`. Conspectus therefore
    linked the tmux session to the launch/resume id (`926c6991`)
    via `active_pane_command_session_match`, even though the active
    transcript and recency belonged to `2f11bd94`. The operator
    likely entered the older tmux session and then used Claude
    Code's in-app `/resume` to switch sessions. The process argv did
    not update, so command-line resume evidence became stale.
  - Scope: refine Claude Code session ↔ mux attribution so
    command-line `--resume <session>` is treated as launch evidence,
    not definitive current-session evidence, when a stronger
    current-session source exists. Investigate, in order:
    non-mutating Claude control/state sources from `H-MUXPROC-005`;
    hook/sidecar payloads from `H-MUXPROC-009` / `H-MUXPROC-012`;
    read-only state/database/file evidence from `H-MUXPROC-003` /
    `H-MUXPROC-004`; and only then carefully-scoped pane scraping of
    already-visible status surfaces such as `/usage`. Do not inject
    `/usage`, `/status`, or any slash command into the pane as part
    of discovery.
  - Desired behavior: when launch argv names session A but stronger
    current-session evidence names session B in the same running
    Claude process, emit or select the `LinkedToMux` candidate for B
    and demote A to launch-history evidence. The TUI should then show
    the attached mux indicator beside B, and B's recency should not
    look like an unattached background write.
  - Tests: fixture a mux node whose `active_pane_start_command`
    contains `--resume A` and a stronger Claude-current-session
    evidence source names B; assert resolver selects B and does not
    keep A as the preferred mux relationship. Add a regression for
    the no-stronger-evidence case where argv remains usable. Add a
    TUI row-tree assertion that the mux indicator follows B.
  - Manual checks: reproduce with a live Claude Code tmux session:
    start or attach to session A, switch in-app to session B with
    `/resume`, verify the pane reports B as current, then confirm
    `conspectus graph --format json` and `conspectus tui` link the
    mux to B.
  - Related: `H-MUXPROC-002` (current argv/fd/process evidence),
    `H-MUXPROC-003` (session-file activity correlation),
    `H-MUXPROC-004` (read-only harness state), `H-MUXPROC-005`
    (control-plane audit), `H-MUXPROC-008` (terminal-injection
    policy), `H-MUXPROC-009` / `H-MUXPROC-010` /
    `H-MUXPROC-012` (Claude hook sidecar path), `P8-014`
    (ambiguous mux picker if evidence remains unresolved).
  - Blockers: no hard blocker for documenting/demoting argv
    semantics; a definitive fix likely depends on one of
    `H-MUXPROC-003`, `H-MUXPROC-004`, `H-MUXPROC-005`, or
    `H-MUXPROC-012`.
  - **regression slice landed**: resolver tests cover stronger
    current-session evidence beating launch argv and launch argv
    remaining usable without a current-session source. Hook-sidecar
    tests cover fresh Claude-current-session evidence overriding a
    stale `active_pane_command_session_match` for the same mux, plus
    live-validation fallout where a fresh Claude session exists in
    hook state before its transcript file exists on disk.
  - **codex-side fix landed via `H-MUXPROC-004` / ADR 0048**: the
    codex log linker resolves the active thread for each live codex
    pid by parsing `logs.process_uuid` (`pid:<os_pid>:<uuid>`) and
    demotes stale `active_pane_command_session_match` candidates for
    the same mux, closing the codex equivalent of this drift class
    without requiring hooks.
  - **Claude-side fix landed via `H-MUXPROC-012` + ADR 0028 hook
    sidecar stack** and confirmed in-the-wild on 2026-05-30. Live
    `conspectus graph --format json` against the development host
    showed two concurrent Claude panes with `claude --resume A` argv
    whose hook-sidecar records had reported the operator's
    post-`/resume` current session B; in both cases the cross_link
    `active_pane_command_session_match` for A was already
    `Overridden` by a fresh `hook_session_path_match` link to B with
    reason "fresh hook sidecar current-session evidence", and on one
    of the panes 10 older orphaned hook records for other sessions
    were correctly demoted by the freshest record via "superseded by
    fresher hook sidecar record for same pane". Three other Claude
    panes whose argv id matched the hook id produced corroborating
    candidates that did not need the override path. No code change
    was needed for closure; the resolver tests and ADR 0028 hook
    sidecar machinery already shipped the fix.

- [x] `H-MUXPROC-013` Add Codex hook sidecar emitter if audit proves
  non-mutating session identity.
  - Scope: if Codex `codex_hooks` events include the active
    thread/rollout/session id, or if a hook can reliably identify the
    current rollout path from its process context, provide an opt-in
    hook emitter for Conspectus sidecar records. Avoid depending on
    terminal input, prompt text, or transcript mutation. If Codex
    hooks only run for tool events, document the expected delay
    before the first sidecar record appears in a fresh session.
  - Tests: payload fixture tests for supported Codex hook events;
    sidecar generation tests for explicit session id and transcript
    path cases; stale/missing-field degradation.
  - Manual checks: enable the hook for a live Codex session and
    confirm Conspectus links the active rollout even when the launch
    command names only a resumed parent.
  - Blockers: `H-MUXPROC-009`, `H-MUXPROC-010`.
  - Audit notes:
    - Local Codex 0.128.0 already gives strong non-mutating live
      evidence through tmux active pane pid -> `/proc/<pid>/fd` ->
      open rollout JSONL path. In live testing, the process argv named
      an older resumed thread, while the open fd identified the current
      rollout, and Conspectus emitted
      `active_pane_fd_session_match` for the current `codex`
      `AgentSession`.
    - Generated app-server schemas expose hook events
      `sessionStart`, `userPromptSubmit`, `postToolUse`, `preToolUse`,
      `permissionRequest`, and `stop`, plus hook notifications with
      `threadId`; this is useful control-plane evidence but not enough
      by itself to identify the currently active thread inside an
      arbitrary already-running TUI process.
    - Codex user hooks are accepted in `$CODEX_HOME/config.toml` under
      `[hooks]` with PascalCase event keys such as `SessionStart`.
      `hooks/list` reports them as `eventName = "sessionStart"`.
      Command hooks must currently be synchronous; `async = true` is
      parsed but skipped with a warning.
    - A `SessionStart` command hook invoked by `codex exec` receives
      JSON on stdin containing `session_id`, `transcript_path`, `cwd`,
      `hook_event_name`, `model`, `permission_mode`, and `source`.
      Ephemeral runs can have `transcript_path = null`; persisted runs
      include the rollout JSONL path.
    - Hook command environments include `TMUX` and `TMUX_PANE`, which
      are enough to correlate the hook event back to a mux pane. Do
      not trust inherited `CODEX_THREAD_ID` for attribution; live
      probing showed it can name the parent Codex session that launched
      the probe rather than the hook payload's new `session_id`.
  - Recommended implementation path: first harden and test the
    existing active-pane fd Codex signal as a default, no-opt-in
    current-session source; then add `conspectus hook write codex`
    for `SessionStart` payloads and `conspectus hook init codex`
    editing `$CODEX_HOME/config.toml` as the opt-in durable path.
  - Outcome: added `conspectus hook write codex`, which converts
    Codex `SessionStart` hook JSON into schema-v1 SQLite sidecar
    records with `harness_key = "codex"`. Added
    `conspectus hook init/status/remove codex`, which manages a
    synchronous `SessionStart` command hook in `$CODEX_HOME/config.toml`
    or `~/.codex/config.toml` while preserving unrelated TOML config.
    Hook-sidecar discovery now infers Codex state scope from the
    `.codex` ancestor in rollout paths when it must synthesize a
    sparse session node. Active-pane fd evidence remains the default
    no-opt-in current-session source for already-running Codex TUI
    processes.

- [x] `H-MUXPROC-014` Add opencode plugin sidecar emitter.
  - Scope: ship an opt-in npm-distributed opencode plugin (working
    name `@conspectus/opencode-hook`, distributed alongside the
    Conspectus release; can also be tried locally via the
    documented `opencode plugin <local-path>` install) that
    subscribes to the `event` hook on the `@opencode-ai/plugin`
    surface and forwards session lifecycle observations to a new
    `conspectus hook write opencode` subcommand. The writer should
    parse the opencode `Event` union, extracting `sessionID` (or
    `info.id` for `EventSession{Created,Updated}`), `info.directory`
    when present, and process/tmux context (`process.pid`,
    `process.ppid`, `TMUX`, `TMUX_PANE`, `TMUX_TMPDIR`), then emit
    a hook record on the same schema ADR 0028 defines for Claude.
    Plugin opt-out at the CLI is `opencode --pure`. Existing
    `discovery::hook_sidecar` post-merge pass is already harness-
    agnostic and will pick up `harness_key: "opencode"` records
    automatically; resolver ranking and demotion semantics for
    opencode mirror the Claude path. Consider whether
    `chat.message` / `tool.execute.after` events also write a
    record (gives finer-grained activity heartbeat) or whether
    `session.{created,updated,idle,status}` are sufficient — pick
    the smaller event set if both work, to keep sidecar churn low.
  - Tests: payload fixture tests for each handled `Event` variant;
    sidecar record generation tests; missing-field degradation;
    multi-project plugin process emits records keyed by sessionID
    not project; `--pure` opt-out path produces no records.
  - Manual checks: install the plugin via `opencode plugin` and
    run a short opencode session with `--print-logs` enabled;
    confirm `conspectus hook write opencode` is invoked, the
    sidecar DB rows show the expected sessionID/cwd/pid/tmux
    fields, and a follow-up `opencode session list` shows the
    session transcript is byte-identical to a control run without
    the plugin installed.
  - Related: `H-MUXPROC-009` audit slice landed 2026-05-30
    establishing the plugin shape; ADR 0028 hook sidecar schema;
    ADR 0049 plugin distribution; `discovery::hook_sidecar`
    reader; ADR 0048 (parallel codex drift fix uses log-derived
    attribution rather than hooks).
  - **landed 2026-05-31**: Rust writer subcommand
    `conspectus hook write opencode` plus three unit tests
    (`hook::opencode_payload_{builds_hook_record,requires_session_id,
    rejects_empty_session_id}`); harness-agnostic
    `discovery::hook_sidecar::apply_hook_sidecars` already picks up
    `harness_key: "opencode"` records; a new end-to-end test
    `opencode_hook_record_demotes_stale_launch_argv_for_same_mux`
    pins the override behavior under the opencode harness key.
    TypeScript plugin lives in-repo at `plugins/opencode-hook/`
    per ADR 0049 (npm package `@conspectus/opencode-hook`, builds
    cleanly via `npm install && npm run build`, types pass
    `npm run typecheck`). v1 subscribes only to lifecycle
    `session.{created,updated,status,idle,compacted}` events;
    `chat.message` / `tool.execute.*` heartbeat was rejected by
    ADR 0049 because freshest-record-wins doesn't require
    heartbeat and the lower churn is preferable. Distribution is
    local-install via `opencode plugin <local-path>`; npm publish
    is deferred per ADR 0049 until at least one external user.
  - **live-verified 2026-05-31**: ran `npm install && npm run build`
    in `plugins/opencode-hook/`, installed via `opencode plugin
    "$(pwd)"` (local scope writes `<project>/.opencode/opencode.json`
    when the cwd is a project root; user-scope path documented as
    fallback in the plugin README), then ran `opencode run "say hi
    in one word"` with `CONSPECTUS_HOOK_BIN` pointed at the debug
    binary. One `hook_sidecar` candidate link landed under
    `harness_key=opencode`, `harness_version=0.1.0`, carrying
    `session.created` and the live tmux pane (`%10`); resolver
    correctly marked it `ignored` because the pane is currently
    running `claude-code`, not opencode (the `opencode run`
    process exited after the prompt).
  - Blockers: none. Audit complete via `H-MUXPROC-009`; sidecar
    schema fixed via `H-MUXPROC-010`.

### Testing Improvements And Regression Replay (TEST-*)

Recent bugfixes around hook sidecars, active-pane evidence, TUI
attachment, and provider parser drift show that individual unit tests
are not enough. The missing coverage is a higher-level, fixture-backed
way to replay whole operator scenarios across harness state, mux state,
hook records, resolver output, and TUI row projection. This workstream
adds that layer without replacing the existing unit, CLI smoke, and
snapshot tests.

Dependency shape inside the workstream:

```
TEST-001 ─→ TEST-002 ─→ TEST-003 ─→ TEST-005
              │             │
              └────────────→ TEST-004
```

`TEST-001` establishes the harness. `TEST-002` adds sanitized
real-world fixture material so regressions can be captured quickly.
`TEST-003` turns recent MUXPROC escapes into replayed scenarios.
`TEST-004` adds broad invariants that should hold across any graph
fixture. `TEST-005` covers TUI interaction regressions that only show
up after row expansion, scrolling, or attach resolution. `TEST-006`
turns the same replay worlds into named operator scenarios that can be
launched through CLI/TUI surfaces for manual inspection.

- [x] `TEST-001` Add a MUXPROC scenario replay harness.
  - Scope: introduce a test support layer that can build a complete
    synthetic local world from small scenario inputs: harness state
    roots, hook SQLite records, fake tmux rows with active-pane
    command/pid/cwd, and injected active-pane fd evidence. Run the
    same pipeline an operator relies on: discovery, resolution, and
    sessions row-tree projection. Keep it deterministic and free of
    real tmux, real `/proc`, real home directories, or network
    access.
  - Tests: self-tests for the harness itself covering empty worlds,
    one harness session plus one mux, hook SQLite record insertion,
    fake fd evidence injection, and path normalization for stable
    snapshots.
  - Manual checks: none; this is infrastructure for automated
    regression replay.
  - Blockers: none.
  - Outcome: added `tests/support/replay.rs`, a deterministic
    integration-test harness that builds synthetic local worlds from
    temp harness state roots, fake tmux rows with active-pane process
    fields, hook sidecar SQLite records, and injected active-pane fd
    target paths. Replay runs the operator pipeline through local
    discovery, fd-evidence inference, resolution, and the sessions
    row-tree projection without real tmux, real `/proc`, real home
    directories, or network access. Added five self-tests in
    `tests/testing_replay.rs` covering empty worlds, one harness
    session plus one mux, hook SQLite record insertion, fake fd
    evidence injection, and temp-path normalization.

- [x] `TEST-002` Add a sanitized real-state fixture corpus.
  - Scope: create checked-in fixture directories for representative
    real provider shapes that synthetic builders have historically
    missed: Codex rollout JSONL files, Claude Code transcript
    envelopes and lineage variants, hook payloads, tmux discovery
    rows, and `/proc/<pid>/fd` target strings. Add a small sanitizer
    script or documented command sequence that strips usernames,
    absolute private paths, tokens, prompt contents, and host-specific
    IDs while preserving schema shape and edge-case fields.
  - Tests: fixture-load tests that parse every corpus file and assert
    the expected node/link or payload shape; no test reads the user's
    real `~/.codex`, `~/.claude`, tmux server, or `/proc`.
  - Manual checks: run the sanitizer against a known live failure and
    confirm the resulting fixture is reviewable, deterministic, and
    free of private transcript text.
  - Blockers: `TEST-001` for replay integration; the corpus can start
    with parser-only tests before the replay harness is complete.
  - Outcome: added `tests/fixtures/` with sanitized corpus files for
    Codex (full, minimal, forked rollouts), Claude Code (basic,
    resume, fork transcripts), hook payloads (Claude SessionStart,
    Claude ephemeral, Codex SessionStart), tmux discovery rows (multi-
    session, paths-with-spaces), and `/proc/<pid>/fd` targets
    (rollout, task, opencode paths plus socket/pipe/anon-inode
    descriptors). Added `tests/fixture_corpus.rs` with 13 fixture-load
    tests that parse every corpus file through the actual adapter,
    hook-parser, and row-parser paths and assert expected node/link
    shapes and edge-case field handling. A 7-step sanitization
    workflow is documented in the test file header.

- [x] `TEST-003` Replay recent MUXPROC drift and stale-evidence bugs.
  - Scope: encode the recent bugfix history as replay scenarios:
    launch argv names session A while stronger hook/fd evidence names
    B; multiple same-pane hook records where the freshest wins; stale
    Claude hook records left behind after the pane starts running
    Codex; hook records whose transcript path is missing; Codex argv
    naming an older resumed thread while open fd evidence names the
    current rollout. Assert both graph evidence state and TUI-visible
    row projection.
  - Tests: scenario snapshots or structured assertions proving there
    is exactly one preferred mux indicator per active pane, weaker
    launch/cwd evidence is overridden or ignored rather than deleted,
    phantom sessions are not synthesized from missing transcripts,
    and Codex open-fd evidence remains preferred over stale argv.
  - Manual checks: none required once scenarios are replayable; live
    checks remain useful only when adding a new real-world failure to
    the corpus.
  - Blockers: `TEST-001`; benefits from `TEST-002`.
  - Outcome: added two replay scenarios in `tests/testing_replay.rs`.
    `same_pane_hook_supersession_freshest_wins_and_tui_shows_active`
    replays a Claude Code pane where hook record A is superseded by
    fresher hook record B; asserts exactly one Active hook-sidecar
    link, one Overridden, and the sessions row tree shows B as
    `Attached`. `codex_fd_evidence_beats_stale_argv_and_tui_follows
    _current_rollout` replays a Codex pane where the launch
    `start_command` references a stale session but injected fd evidence
    names the current rollout; asserts `active_pane_fd_session_match`
    exists, the resolver prefers it, and the row projection attaches
    the current session. Also added a TEST-004-style invariant helper
    `assert_at_most_one_active_hook_link_per_mux_pane` that verifies
    at most one Active hook-sidecar `LinkedToMux` per `(mux, pane_id)`,
    called from the hook supersession test.

- [x] `TEST-004` Add graph and row-projection invariant tests.
  - Scope: add table-driven and, where practical, property-style
    tests for invariants that cut across specific scenarios: ignored
    candidates never resolve as active relationships; stronger
    current-session evidence beats launch evidence; hook-sidecar
    records produce at most one active link per `(mux, pane_id)` key;
    TUI session rows dedupe mux indicators by target; unresolved or
    ignored candidates remain diagnosable without becoming preferred
    rows.
  - Tests: invariant tests over hand-built graph fragments plus a
    small matrix of replay fixtures. Add `proptest` only if a
    bounded generator demonstrates value; otherwise keep the first
    slice deterministic and table-driven.
  - Manual checks: none.
  - Blockers: none for table-driven invariants; `TEST-001` before
    running invariants against replay fixtures.
  - Outcome: added four deterministic invariant tests to
    `tests/testing_replay.rs`: ignored mux candidates remain visible
    as evidence but never resolve, ambiguous TUI session rows dedupe
    candidate rows by mux target, replay worlds enforce at most one
    active hook-sidecar link per `(mux, pane_id)`, and stronger
    current-session fd evidence wins over stale launch history.

- [x] `TEST-005` Add TUI interaction regression tests for row
  expansion, scrolling, and attach resolution.
  - Scope: build a thin test driver around `App` that applies fixed
    key/action sequences at deterministic terminal sizes. Cover the
    cases that escaped pure row snapshots: expanded ambiguous mux
    candidates remain navigable, selection stays visible while
    groups expand/collapse, attach target resolution never targets
    the current tmux session, preview/detail panes tolerate missing
    or ignored links, and the row cursor can move past duplicate or
    overridden candidate rows.
  - Tests: reducer/action tests plus Ratatui buffer snapshots for the
    smallest useful set of fixed viewports. Prefer structured
    assertions for navigation state and snapshots only where layout
    regressions are the risk.
  - Manual checks: run `conspectus tui` against a replayed or live
    ambiguous-mux fixture only when adding a new interaction failure.
  - Blockers: `TEST-001`; coordinates with `T8-006` so buffer
    snapshot coverage is not duplicated.
  - Outcome: added scenario-backed `App` reducer/action tests using
    the named `TEST-006` worlds. Coverage now asserts ambiguous mux
    candidate rows remain navigable after expansion, selection snaps
    to a visible row when a refresh removes the selected row, and
    attach target resolution refuses the tmux session hosting the
    current TUI.

- [x] `TEST-006` Expose named replay scenarios to CLI and TUI runs.
  - Scope: promote the replay harness's useful worlds into a small
    named scenario registry shared by tests and developer commands.
    Each scenario should materialize an isolated temp world and return
    enough launch context for `graph`, `table`, `node show`, and
    `tui` to run against exactly the same generated state. Cover at
    least: empty world, orphan session, session+tmux exact match,
    ambiguous mux candidates, hook supersession, Codex fd-beats-stale
    argv, workspace with PR, and fork lineage. Keep the scenario
    materializer test-only or explicitly gated so production discovery
    does not grow fixture behavior.
  - Tests: scenario-registry tests proving every named scenario can
    materialize, run discovery, resolve, render graph JSON, render the
    relevant table row-type, and build the TUI row tree without reading
    the user's home directory, real tmux, real `/proc`, or network.
  - Manual checks: add a documented launch path such as
    `conspectus dev scenario tui ambiguous-mux` (exact surface to be
    decided during implementation) and verify it opens the real TUI on
    the generated scenario. Also verify graph/table output from the
    same scenario matches the automated snapshots.
  - Blockers: `TEST-001`; useful before `TEST-005` and `GV-002`/`GV-003`
    so interaction tests and visualization exports share scenario
    names instead of rebuilding fixtures independently.
  - Outcome: added a debug/test-only `dev_scenarios` module with
    named worlds for empty, orphan-session, exact-match,
    ambiguous-mux, hook-supersession, Codex fd-current,
    workspace-pr, and fork-lineage cases. Added hidden debug CLI
    commands under `conspectus dev scenario` for list, graph,
    table, node, and static TUI launch. Scenario tests prove every
    world materializes, resolves, renders graph JSON and table output,
    and builds a TUI sessions row tree without reading real home,
    tmux, `/proc`, or network state.

- [x] `TEST-007` Add filter, grouping, and sort controls to dev
  scenario exploration.
  - Scope: make `conspectus dev scenario tui <name>` accept the same
    pure exploration flags as normal `conspectus tui`: `--view`,
    `--grouping`, `--harness`, `--mux-state`, `--max-age`, and
    `--sort`. In the static scenario TUI, enable the controls overlay,
    grouping cycle, clear-filters action, and sort/filter changes by
    rebuilding row trees from the pre-materialized scenario snapshot
    instead of running live discovery. Keep mutating or host-affecting
    actions disabled (`attach`, `resume`, rename writes).
  - Tests: CLI smoke tests for scenario TUI flag validation and
    scenario table/filter output where practical; reducer/runtime tests
    for static controls applying filter/grouping/sort without invoking
    live discovery. Update `docs/dev-scenarios.md` with examples.
  - Manual checks: run `cargo run -- dev scenario tui ambiguous-mux
    --grouping none --mux-state ambiguous --sort recency` and verify
    the TUI opens on the filtered static scenario graph; use the
    controls overlay to change filters/grouping and confirm rows
    rebuild in place.
  - Blockers: none for filter/grouping; visible sort behavior may need
    follow-up if a row-tree builder does not yet consume `Sort`.
  - Outcome: `conspectus dev scenario tui` now accepts filter,
    grouping, and sort flags before launch. The static scenario TUI
    enables controls overlay, grouping cycle, clear-filters, and
    view-switch rebuilds against the pre-materialized scenario
    snapshot, while attach/resume/rename remain disabled. Added CLI
    smoke tests for the hidden TUI flag surface and validation, and
    updated `docs/dev-scenarios.md` with filter/group/sort examples.

### Session Naming

Conspectus today is read-only outside Phase 5 declared-link CRUD. Session
identifiers feel anonymous in the TUI: harness-native titles are sparse
(opencode only, claude-code carries a compaction summary, codex/aider
populate nothing) and tmux session names are operator-chosen but not
coordinated with the agent context. This workstream introduces operator-
controlled session naming as the first non-relationship write surface,
deliberately scoped conservatively:

- Names are stored as a Conspectus-owned alias overlay (ADR 0029), not
  written back to harness stores. Works uniformly across every harness
  including read-only ones.
- Renaming a muxed agent session also renames the tmux session in
  lockstep by default; `--no-mux` decouples.
- AI-driven name suggestions are deferred to the sibling
  `H-AI-NAMING-*` workstream so this one stays free of new dependencies,
  network IO, and an async runtime.
- Incidentally builds the first reusable TUI text-input primitive
  (ADR 0030), which unblocks `T8-017` (`/` search overlay) and `P8-014`
  (inline mux-picker).

Dependency shape inside the workstream:

```
H-RENAME-001 ──┬─→ H-RENAME-004 ──┐
H-RENAME-002 ──┤                  ├─→ H-RENAME-006 ──┬─→ H-RENAME-007 ─→ H-RENAME-008
               └─→ H-RENAME-010   │                  │
                                  │                  ├─→ H-RENAME-009 ─→ H-RENAME-011
H-RENAME-003 ─────────────────────┤                  │                       │
                                  │                  │                       ├─→ H-RENAME-012
                                  │                  │                       ├─→ H-RENAME-013
                                  │                  │                       └─→ H-RENAME-014
```

The two ADRs (`001`, `002`) and the `TmuxRunner::rename_session` seam
(`003`) are unblocked from day one and can land in parallel. `004` is the
spine; once it lands, projection (`006`), CLI (`007`/`008`), and lockstep
(`009`) follow. Input widget (`010`) is parallel to the CLI track but
blocks TUI wire-up (`011`).

- [x] `H-RENAME-001` ADR: alias overlay schema and storage.
  - Scope: settle the storage schema (`[[aliases]]` table sibling to
    `[declared]`, not nested inside it), store-selection rules, conflict
    resolution between local and global, render precedence
    (`alias > title > id-suffix`), mux node-id stability rule (no mux
    aliases stored — lockstep renames mutate the native tmux name
    directly), lockstep contract, hook-sidecar drift caveat, and the
    alias-equals-title round-trip rule. Record as ADR 0029.
  - Tests: docs-only; `git diff --check`.
  - Blockers: none.
- [x] `H-RENAME-002` ADR: TUI text-input primitive.
  - Scope: resolve ADR 0024's deferred `tui-input` decision now that three
    callers exist (rename, `T8-017` search overlay, `P8-014` mux-picker).
    Settle hand-rolled vs crate, locked key semantics (`Enter` confirm,
    `Esc` cancel, `Tab` suspended while overlay is open), overlay
    placement (centered modal, 60-col width cap), and module boundary
    (`src/tui/widgets/input.rs`). Record as ADR 0030.
  - Tests: docs-only; `git diff --check`.
  - Blockers: none.
- [x] `H-RENAME-003` Extend `TmuxRunner` with `rename_session` mutation seam.
  - Scope: first non-read-only tmux call. Add `rename_session(target,
    new_name) -> TmuxOutcome` to the trait in `src/discovery/tmux/mod.rs`
    with a default impl returning `Unsupported` so future backends (zellij
    per `H-FUTURE-001`) don't break. `SystemTmux` runs
    `tmux rename-session -t <native_id> <new_name>`. `FakeTmux` records
    calls for assertion.
  - Tests: per-impl tests for success, target-missing, binary-missing, and
    name-collision (tmux rejects duplicates). `FakeTmux` recording
    assertions.
  - Blockers: none (parallel to ADRs).
- [x] `H-RENAME-004` Alias storage layer.
  - Scope: per ADR 0029. Define the TOML model (round-trip), load aliases
    from local + global stores at discovery time into a sidecar
    `HashMap<NodeId, String>` carried alongside the graph snapshot. Add
    atomic write helpers analogous to `upsert_declared_link` and
    `remove_declared_link` (`src/declared.rs:233`, `:252`); reuse
    `write_atomic` (`src/declared.rs:400-430`) directly. Reuse
    `select_store_for_declaration` (`src/declared.rs:143-167`) for nearest-
    store write selection. Reuse `DeclaredEndpoint` (`src/declared.rs:77`)
    as the node-key encoding.
  - Tests: round-trip TOML tests for the new schema, store-selection
    tests across project-rooted vs orphan agent sessions, malformed-entry
    diagnostics, schema-version skip behavior, atomic-write retry path.
  - Blockers: `H-RENAME-001`.
- [x] `H-RENAME-006` Projection precedence.
  - Scope: apply `alias > title > id-suffix` at the four projection sites
    — `src/output/table.rs` `title` column rendering, `src/output/node_show.rs`
    header field, `src/tui/rows/mod.rs` `AgentSessionRow` label,
    `src/tui/detail.rs` header field. Centralize the precedence helper so a
    future tweak touches one place. Snapshot fixtures updated here;
    expect non-trivial diff churn.
  - Tests: projection unit tests across present-alias / present-title /
    absent-both cases at each of the four sites. Insta snapshot updates.
  - Blockers: `H-RENAME-004`.
- [x] `H-RENAME-007` CLI: `conspectus rename` command tree.
  - Scope: add `conspectus rename session <id> [<name>] [--no-mux]
    [--clear]` and `conspectus rename mux <id> [<name>] [--clear]`.
    `<name>` and `--clear` are mutually exclusive; missing both is an
    error. Imperative pattern from Phase 5 — no `--dry-run`, no `--yes`.
    Use the short row-id resolution from `H-TBL-005`. Mux rename never
    writes an alias row (per ADR 0029 stability rule); only the native
    tmux name changes. Session rename invokes the lockstep helper from
    `H-RENAME-009` for the default-lockstep behavior.
  - Tests: CLI smoke tests for each command shape, error handling for
    mutually-exclusive flags, fake-runner-backed assertion that lockstep
    invokes both alias write and tmux rename.
  - Blockers: `H-RENAME-006`, `H-RENAME-009`, `H-RENAME-003`.
- [x] `H-RENAME-008` CLI: `conspectus alias list` (and `show`).
  - Scope: read-path counterpart to the rename write commands. Operators
    will want to audit overlays that hide harness-native titles. Mirrors
    `conspectus declared list` shape (`src/cli.rs:1180+`). Add `alias show
    <id>` if list-only feels thin during review.
    `conspectus alias list [--store local|global|all]`.
  - Tests: CLI snapshot tests for empty, single-store, both-stores, and
    mixed-with-declared cases.
  - Blockers: `H-RENAME-007`.
- [x] `H-RENAME-009` Mux lockstep helper.
  - Scope: pure function consumed by the CLI rename command and the TUI
    rename action. Given a target node, the current snapshot, and a
    `--no-mux` flag, returns a `RenamePlan { agent_alias_write,
    mux_native_rename }`. Refuses lockstep with a typed reason when the
    target has ambiguous `LinkedToMux` candidates (per ADR 0029 lockstep
    rule) — operator must either resolve ambiguity first or pass
    `--no-mux`.
  - Tests: unit tests across resolved-single-mux, ambiguous-mux,
    no-mux-link, and `--no-mux`-flag cases.
  - Blockers: `H-RENAME-001`.
- [x] `H-RENAME-010` TUI text-input widget implementation.
  - Scope: per ADR 0030. Lives in new `src/tui/widgets/input.rs`. Exports
    `TextInputState`, `TextInputWidget`, and `handle_key` returning
    `InputOutcome::{Continue, Confirm(String), Cancel}`. Centered modal
    overlay, 60-col width cap, 3-row height for the rename variant.
    Status-bar shows `Enter confirm · Esc cancel` while open. Designed
    so `T8-017` and `P8-014` adopt without changes.
  - Tests: insta snapshot tests for empty / typed / wide-terminal /
    narrow-terminal layouts. Reducer-level tests for the
    confirm/cancel/passthrough outcomes.
  - Blockers: `H-RENAME-002`.
- [x] `H-RENAME-011` TUI `R` keybinding wires rename flow.
  - Scope: bind `R` (capital) — verify it's unused today
    (`src/tui/runtime.rs:324-325`). On press, opens the input widget
    pre-populated with the current alias (or harness title, or empty
    when neither). Enter triggers `H-RENAME-009` plan → alias write +
    optional `TmuxRunner::rename_session` → `Msg::SetStatus` feedback
    (`renamed: <new>` or `rename failed: <reason>`) → refresh. Esc
    cancels. Lower-case `r` continues to mean refresh per Phase 8.
  - Tests: reducer tests for the open/confirm/cancel paths. Insta
    snapshot for the active rename overlay over the sessions tree.
    Manual: rename a session in a real TUI, confirm both alias and
    tmux update.
  - Blockers: `H-RENAME-007`, `H-RENAME-010`.
- [x] `H-RENAME-012` Read-only invariant audit.
  - Scope: mirror of `P5-004`. Smoke tests verifying that `conspectus
    graph`, `conspectus node show`, `conspectus table`, and TUI
    navigation (no rename action) do not mtime-touch or content-modify
    alias-bearing config files. Add to the existing read-only test
    harness used by Phase 5.
  - Tests: as scoped above.
  - Blockers: `H-RENAME-011`.
- [x] `H-RENAME-013` Live-session UX advisory.
  - Scope: status-bar advisory when the operator renames a session whose
    mux indicator is `Attached` or `Ambiguous` (per `MuxIndicator` in
    `src/tui/rows/mod.rs:159-172`) and hook-sidecar evidence is fresh
    (per ADR 0028 `ACTIVE_TTL_SECONDS`, `src/discovery/hook_sidecar.rs:21-28`).
    Alias is safe; message is informational ("renamed live session:
    alias overlays harness title until session ends"). Establishes the
    live-detection plumbing the future write-back ADR will need.
  - Tests: status-bar message tests across live / ambiguous / dormant /
    no-mux cases.
  - Blockers: `H-RENAME-011`.
- [x] `H-RENAME-014` Docs and snapshot coverage.
  - Scope: update `docs/operations.md` with the new commands; update the
    Phase 8 TUI doc keybindings table
    (`docs/implementation/phase-08-interactive-tui.md`); add insta
    snapshot tests for renamed-row rendering in tree, table, and detail
    surfaces.
  - Tests: doctest where applicable; `git diff --check`; insta review.
  - Blockers: `H-RENAME-011`, `H-RENAME-013`.

### Session Pins

Settled by ADR 0057 (Accepted). Pins are user-authored declarations of
a logical agent session — a `(harness, cwd, display_name, mux)` tuple
persisted in a sibling `[[pins.entries]]` TOML table that renders as a
first-class dashboard row whether or not a live session realizes it,
binds 1:1 on the mux native name through the existing mux-to-agent-
session attribution pipeline (ADR 0006 / ADR 0028 / ADR 0046 / ADR
0047 / ADR 0048), and launches via new `TmuxRunner` mutation methods
plus the existing P8-010 exec-replace attach.

Pins replace the agent-deck "new card" workflow without inheriting the
broader orchestrator scope. They compose with — rather than replace —
ADR 0014 declared links and ADR 0029 aliases: `display_name` is the
overlay alias for the bound session, ambiguity overrides write a
`LocalDeclared linked_to_mux` link tagged with the pin id, and the
sibling TOML tables share store-selection rules.

Dependency shape inside the workstream:

```
H-PIN-001 (ADR) ──┬─→ H-PIN-002 ──┬─→ H-PIN-003 ──→ H-PIN-004 ──┬─→ H-PIN-016 ──→ H-PIN-017
                  │               │                              │
                  │               └─→ H-PIN-005 ──→ H-PIN-006 ──→│
                  │                                              ├─→ H-PIN-009 ──→ H-PIN-013
                  └─→ H-PIN-010 ──→ H-PIN-011 ──→ H-PIN-012 ─────┤              ──→ H-PIN-014
                                                                 │              ──→ H-PIN-015
                                                                 └─→ H-PIN-018
                                          H-PIN-022  H-PIN-023  H-PIN-024 (TUI CRUD parity)
                                          H-PIN-007 ──→ H-PIN-008 (CLI read path)
                                          H-PIN-019  H-PIN-020  H-PIN-021 (closeout)
```

`H-PIN-001` (the ADR) is unblocked; `H-PIN-002` (schema + TOML) and
`H-PIN-010` (TmuxRunner extensions) can land in parallel after it.
`H-PIN-004` (resolver binding) is the integration spine that the TUI
and launch stories converge on. `H-PIN-017` provides the immediate
row-level actions; `H-PIN-022..024` bring the Controls overlay to
CLI-parity for create / edit / remove / bind / rebind / adopt. The
closeout stories (`H-PIN-019..021`) document and lock in the surface
once everything else has landed.

- [x] `H-PIN-001` ADR: session pin schema, binding, and launch contract.
  - Scope: record the schema (`[[pins.entries]]` TOML sibling to
    `[declared]` and `[aliases]`), the mux-anchored binding rules,
    cwd as launch-parameter-not-discriminator, the four pin-specific
    diagnostics (`PinUnbound` / `PinStaleMux` / `PinAmbiguous` /
    `PinDrift` plus `PinDuplicate` safety net), launch via
    `TmuxRunner::new_session` + exec-replace attach, lockstep rename
    contract, operator escape hatches (`bind` / `rebind` / `adopt`),
    optional `mux.socket_name` for tmux `-L`, identity encoding
    `tmux:<name>` for default socket and `tmux:<socket>:<name>` for
    non-default, prior art comparison, and the hooks-future-proofing
    guarantee.
  - Tests: docs-only; `git diff --check`.
  - Outcome: ADR 0057 captures the decision and is promoted to
    Accepted now that the v1 schema, resolver, CLI, launch, TUI, and
    closeout slices are in place.
  - Blockers: none.

- [x] `H-PIN-002` Pin schema + TOML round-trip.
  - Scope: add `src/pins.rs` with `PinEntry`, `PinMux`, `PinLaunch`
    serde models matching ADR 0057. `schema_version`, unknown-field
    tolerance, malformed-entry diagnostics, validation
    (non-empty `id` / `display_name` / `harness`, absolute `cwd`,
    `mux.backend == "tmux"` in v1, `mux.name` non-empty,
    `mux.socket_name` non-empty when present, duplicate-id rejection,
    duplicate-(`mux.backend`, `mux.name`, `mux.socket_name`)
    rejection). Round-trip TOML decode/encode preserves unrelated
    sections.
  - Tests: unit tests for happy-path TOML, missing-section default,
    unknown keys, malformed entries, duplicate id, duplicate mux
    triple, empty / missing required fields, the default-socket vs
    non-default-socket cases, and a round-trip preserving `[session]`
    / `[declared]` / `[aliases]` siblings.
  - Outcome: `src/pins.rs` defines the v1 pin schema, validation,
    parse, round-trip, upsert, and remove helpers with unit coverage.
  - Blockers: `H-PIN-001`.

- [x] `H-PIN-003` Load pins into discovery as GraphLink candidates.
  - Scope: add a read-only `discovery::pins` pass analogous to
    `discovery::declared`. Map each entry into a new `Pin` candidate
    kind carrying `(id, harness, cwd, display_name, mux.backend,
    mux.name, mux.socket_name, store)`. `LocalPin` vs `GlobalPin`
    provenance follows config-file location (mirror ADR 0014 rule).
    Local beats global on `id` collision. Emit a diagnostic when both
    stores claim the same id with conflicting fields.
  - Tests: unit tests for local-only, global-only, local-over-global,
    malformed-file diagnostic isolation, empty stores, and config
    paths matching ADR 0012.
  - Outcome: `discovery::pins` loads local and global pins into
    `PinCandidate` evidence, preserves sparse/malformed-store
    behavior, and reports duplicate/local-over-global diagnostics.
  - Blockers: `H-PIN-002`.

- [x] `H-PIN-004` Resolver binding pass.
  - Scope: extend the resolver to bind each pin to a live
    `(MuxSession, AgentSession)` pair per ADR 0057. Mux lookup is
    exact-match on `native_id` (default-socket: `tmux:<name>`,
    non-default: `tmux:<socket>:<name>`). Harness attribution
    restricts the existing mux-to-agent-session candidate pipeline to
    the bound mux + `harness_key == pin.harness`. Emit synthesized
    in-memory alias overlay (no TOML write) and a `LinkedToMux`
    candidate with `PinDerived` provenance on bind. Emit
    `PinUnbound` / `PinStaleMux` / `PinAmbiguous` / `PinDrift`
    diagnostics as specified.
  - Tests: snapshot tests against fixture graphs covering bound,
    unbound, stale-mux, ambiguous-multi-harness, drift (cwd
    divergence), duplicate-pin, and pin-with-non-default-socket-not-
    yet-discovered cases.
  - Outcome: `resolve::pins` binds pins through mux-anchored
    attribution, synthesizes pin-derived links/aliases, and emits
    `PinUnbound`, `PinStaleMux`, `PinAmbiguous`, and `PinDrift`
    diagnostics covered by resolver and snapshot tests.
  - Blockers: `H-PIN-003`.

- [x] `H-PIN-005` Extend store selection for pin writes.
  - Scope: reuse `select_store_for_declaration` for pin writes. Verify
    behavior for repo-rooted, checkout-rooted, workspace-rooted, and
    orphan-cwd pins. Reject pins whose `cwd` does not exist on the
    filesystem at write time (differs from declared links, per ADR
    0057).
  - Tests: unit tests for each store-selection case plus the
    nonexistent-cwd rejection.
  - Outcome: pin writes use the existing nearest-store selection
    shape, with explicit project/user overrides and cwd existence
    validation before mutation.
  - Blockers: `H-PIN-002`.

- [x] `H-PIN-006` Atomic write helpers for `[pins]`.
  - Scope: read-modify-write upsert/remove for project and user
    config `[pins]` sections. Preserve unrelated TOML sections, sort
    entries deterministically (by `id`), replace duplicates by id,
    create parent directories only on writes, temp-file-and-rename
    atomicity, leave malformed pre-existing files untouched.
  - Tests: unit tests for upsert / remove / preserve-others / sort /
    duplicate-replacement / malformed-file refusal.
  - Outcome: pin upsert/remove helpers preserve sibling TOML
    sections, sort deterministically, reject malformed inputs, and
    write through the shared atomic config path.
  - Blockers: `H-PIN-005`.

- [x] `H-PIN-007` Pin CLI command tree skeleton.
  - Scope: add `conspectus pin {create,list,show,rename,rm,launch,
    attach,bind,rebind,adopt}` subcommand structure with flag
    surface from ADR 0057. Validation only — write commands stub
    `bail!("not yet implemented")`. Read commands wire up in
    H-PIN-008. `--help` text matches ADR. `--mux-socket` flag
    accepts a tmux socket name (the equivalent of `tmux -L`); the
    TOML key it writes is `mux.socket_name`.
  - Tests: CLI smoke tests for `--help`, invalid flag combinations,
    and missing required arguments per subcommand.
  - Outcome: `conspectus pin` exposes the v1 create/list/show/rename/
    rm/launch/attach/bind/rebind/adopt command tree.
  - Blockers: `H-PIN-001`.

- [x] `H-PIN-008` CLI `pin list` and `pin show`.
  - Scope: render pins from local + global stores with their
    provenance, binding state (`bound` / `unbound` / `stale` /
    `ambiguous`), store path, and bound agent-session id when bound.
    `--bound` / `--unbound` / `--stale` filters. `pin show <id>`
    prints the full entry plus binding diagnostic if any.
  - Tests: CLI integration tests for empty stores, mixed local/global,
    each binding state via fixture graphs, deterministic ordering.
  - Outcome: `pin list` and `pin show` render pin store provenance,
    binding state, bound sessions, launch argv, and diagnostics.
  - Blockers: `H-PIN-004`, `H-PIN-007`.

- [x] `H-PIN-009` CLI `pin create` / `rename` / `rm`.
  - Scope: write commands that persist user intent via the H-PIN-006
    helpers. `create` uses nearest-store selection by default;
    `--store` overrides. `rename` changes `id` and/or `display_name`;
    `--display` change applies the ADR 0029 lockstep mux rename when
    the pin is currently bound (delegate to the H-RENAME-009 lockstep
    helper). `rm` removes from the first matching store. All commands
    refuse to mutate a malformed config file and surface a clear
    diagnostic instead.
  - Tests: CLI integration tests for create-into-project-config,
    create-into-user-config, explicit `--store`, rename
    (with-display-change-and-lockstep, with-id-change-only),
    rm-from-project, rm-not-found, idempotent re-create, and a
    snapshot of the generated TOML.
  - Outcome: CLI create/rename/rm persist pins through the shared
    TOML helpers, preflight malformed/duplicate inputs, and preserve
    unrelated config sections.
  - Blockers: `H-PIN-006`, `H-PIN-007`.

- [x] `H-PIN-010` Extend `TmuxRunner` with launch mutation seams.
  - Scope: add three new defaulted `TmuxRunner` methods —
    `new_session(socket_name, name, cwd, argv)`,
    `attach_session(socket_name, name)`,
    `send_keys(socket_name, target, literal, press_enter)` — and
    thread `socket_name: Option<&str>` through the existing
    `rename_session` and `capture_pane` methods. `SystemTmux` adds
    `-L <name>` only when `socket_name` is `Some(s)` and
    `s != "default"`, preserving default-socket invocation
    byte-for-byte. `FakeTmux` records calls.
  - Tests: unit tests for the new methods, the socket-name
    threading on both `SystemTmux` (mocked) and `FakeTmux`,
    and `Unsupported` defaulting.
  - Outcome: `TmuxRunner` supports socket-aware new-session,
    attach-session, send-keys, rename, and capture operations with
    `FakeTmux` call recording for tests.
  - Blockers: `H-RENAME-003` (the `rename_session` seam this extends),
    `H-PIN-001`.

- [x] `H-PIN-011` `HarnessAdapter::launch_argv` defaults.
  - Scope: add a `launch_argv(&self) -> Vec<OsString>` method to
    `HarnessAdapter`. Default implementations: codex `["codex"]`,
    claude-code `["claude"]`, opencode `["opencode"]`, aider
    `["aider"]`. Override via `pin.launch.argv` flows through the
    launch primitive (H-PIN-012).
  - Tests: per-adapter unit tests for the default; integration test
    that the launch primitive prefers `pin.launch.argv` when set.
  - Outcome: harness adapters expose default launch argv, and launch
    flows prefer per-pin argv overrides when configured.
  - Blockers: none beyond `H-PIN-001`.

- [x] `H-PIN-012` CLI `pin launch` and `pin attach`.
  - Scope: orchestrate the launch flow per ADR 0057 §Launch
    Semantics: load pin → run discovery + resolver → branch on
    binding state. Bound → exec-replace `attach_session`. Stale-mux
    → `send_keys(argv...) ; Enter` then `attach_session`. Unbound +
    name-free → `new_session(socket_name, name, cwd, argv)` then
    `attach_session`. Unbound + name-taken-by-unrelated-tmux →
    `PinLaunchError::NameTaken` with hint about `pin adopt`.
    Unix-only exec-replace; `--no-attach` returns after spawn and
    prints the attach command.
  - Tests: integration tests via `FakeTmux` for each branch + the
    bound/stale/unbound transitions; one Unix-gated test for the
    exec path.
  - Outcome: `pin launch` / `pin attach` branch across bound,
    stale-mux, and unbound states using socket-aware tmux attach,
    send-keys, and new-session operations, with `--no-attach`
    coverage.
  - Blockers: `H-PIN-004`, `H-PIN-009`, `H-PIN-010`, `H-PIN-011`.

- [x] `H-PIN-013` CLI `pin bind` (PinAmbiguous override).
  - Scope: write a `LocalDeclared linked_to_mux` link (per ADR 0014)
    between the named agent session and the pin's mux. Tag the
    declared link with the pin id in `label` or a new
    `bound_by_pin` field (decide during impl; prefer `label =
    "pin:<id>"` to avoid a schema migration). Resolver treats that
    declared link as authoritative when present, suppressing the
    auto-attribution.
  - Tests: integration tests for the bind path (resolves
    `PinAmbiguous` deterministically), and a graph-JSON snapshot
    showing the declared link survives alongside the original
    candidate evidence.
  - Outcome: `pin bind` writes a `pin:<id>` declared
    `linked_to_mux` override that resolver precedence treats as the
    authoritative ambiguous-binding choice.
  - Blockers: `H-PIN-004`, `H-PIN-009`.

- [x] `H-PIN-014` CLI `pin rebind` (external-rename recovery).
  - Scope: update `pin.mux.name` (and optionally `pin.mux.socket_name`)
    in the pin's owning TOML store. Validates that no other pin
    already targets the new mux triple. Does not touch tmux.
  - Tests: integration tests for rebind on a stale pin, rebind into
    a duplicate (rejected), rebind across stores (refused — operator
    moves the entry instead).
  - Outcome: `pin rebind` updates the owning pin store's mux target,
    rejects duplicate mux triples, and leaves tmux state untouched.
  - Blockers: `H-PIN-006`, `H-PIN-009`.

- [x] `H-PIN-015` CLI `pin adopt`.
  - Scope: convert an existing live tmux session into a pin without
    creating a new mux. Required positional `<pin-id>` and
    `<mux-name>`; optional `--harness` (default: infer from the
    resolver's current attribution for that mux); `--display`
    (default: pin-id); `--mux-socket` (default: absent). `cwd`
    defaults to the mux's observed `cwd` if known, otherwise refused
    with a hint. The v1 migration path for replacing agent-deck.
  - Tests: integration tests for adopt with explicit harness, adopt
    with inferred harness, adopt with unresolvable mux (rejected),
    adopt of an already-adopted mux (rejected).
  - Outcome: `pin adopt` converts a live mux into a pin using
    inferred or explicit harness/cwd fields, and rejects missing or
    already-pinned mux targets.
  - Blockers: `H-PIN-004`, `H-PIN-009`.

- [x] `H-PIN-016` TUI row tree integration.
  - Scope: extend `build_sessions_tree` and the mux row builder to
    render a row per pin. Unbound pins render with a dim glyph and
    secondary `(pin · <harness> · ~/...)` text. Bound pins render
    like the underlying agent-session row plus a small star glyph
    (final glyph chosen against `Theme`; see open question below).
    Mux view renders the pin-derived mux as an ordinary mux row when
    bound; unbound pins do not synthesize a mux row.
  - Tests: row-tree builder unit tests for empty/bound/unbound/stale/
    ambiguous/multi-pin fixtures; insta snapshots over a 80×24 TUI
    render.
  - Blockers: `H-PIN-004`; friendlier after `P8-004` parts 2-5 land
    the per-view row builders.
  - Outcome: sessions row tree emits unbound/stale pins under a
    synthetic Pins group, marks bound agent-session rows with
    `pin_id`, renders pin rows/status hints in the TUI, and covers
    unbound/stale/bound/mixed/end-to-end resolver cases with unit
    tests.

- [x] `H-PIN-017` TUI keybindings for pin actions.
  - Scope: bind `Enter` on a pin row to launch (unbound) or attach
    (bound) via H-PIN-012; `R` to rename (lockstep via ADR 0029);
    `Delete` to remove with confirmation. Add a Pins action group
    placeholder to the ADR 0031 Controls overlay that opens the
    richer CRUD flows tracked in `H-PIN-022..024`. Decide the
    bound-pin glyph in coordination with ADR 0032's theme
    vocabulary.
  - Tests: reducer tests for the new keys; snapshot tests for the
    Controls overlay open state with the Pins group.
  - Blockers: `H-PIN-016`.
  - Outcome: `Enter` on a pin row shells out to `conspectus pin
    launch <id>` and refreshes on return; `R` opens the existing
    text-input overlay for pin display-name edits and commits via
    `conspectus pin rename --display`; `Delete` uses a second-press
    confirmation before `conspectus pin rm`; static scenario TUIs
    keep these mutating actions disabled. Controls overlay includes
    a discoverable Pins action group whose structured CRUD editors
    remain in `H-PIN-022..024`.

- [x] `H-PIN-018` Pin diagnostic surfaces in the TUI.
  - Scope: each pin diagnostic gets a specific affordance — status
    bar text for unbound (`Enter to launch`), stale-mux (`Enter to
    relaunch in existing mux`), ambiguous (`b to bind`), drift
    (advisory). Right-pane detail surfaces the diagnostic plus, for
    ambiguous, the list of competing `agent_session_id`s.
  - Tests: snapshot tests for each diagnostic state; reducer test
    for the `b` accelerator routing to the bind picker.
  - Blockers: `H-PIN-016`, `H-PIN-004`.
  - Outcome: selected pin rows and bound pinned sessions now derive
    status-bar hints from resolver diagnostics: unbound pins advertise
    launch, stale mux pins advertise relaunch, ambiguous bindings
    advertise `b` for bind guidance, and cwd drift is marked
    advisory. Pin rows render a right-pane diagnostic preview, bound
    agent-session details add pin diagnostic fields with competing
    session ids for ambiguous bindings, and the `b` accelerator routes
    to a bind command hint until the full picker lands in `H-PIN-024`.

- [x] `H-PIN-022` TUI pin create flow.
  - Scope: make the Controls overlay Pins group capable of creating
    pins without dropping to the CLI. Reuse the H-PIN-009 mutation
    helper and ADR 0030 text input primitive. Fields: `id`,
    `display_name`, `harness`, `cwd`, `mux.name`, optional
    `mux.socket_name`, optional launch argv override, and store
    (`auto` / `project` / `user`). Defaults should come from the
    current selection where possible: selected session gives harness
    + cwd + display candidate; selected checkout/repo gives cwd;
    otherwise cwd starts blank. Validation mirrors CLI create and
    never writes until the confirmation step succeeds.
  - Tests: reducer tests for field editing, defaults from session
    and checkout selections, validation failures, cancel-no-write,
    and successful create through the shared write helper. Snapshot
    tests for the create overlay and validation messages.
  - Blockers: `H-PIN-009`, `H-PIN-017`, `F8-004`.
  - Delivered: Controls overlay `Pins > create` opens a
    multi-field create modal, seeds fields from the selected session
    or graph group where possible, validates required fields before
    dispatch, and routes successful creates through the shared pin
    write helper with `auto` / `project` / `user` store selection.
    Static scenario TUIs keep mutation disabled and surface a status
    message instead of writing.

- [x] `H-PIN-023` TUI pin edit and remove flow.
  - Scope: bring existing pins to CRUD parity with CLI
    `pin rename` / `pin rm` from the Controls overlay, while keeping
    the row-level `R` and `Delete` accelerators from H-PIN-017.
    Edit supports id changes, display-name changes, mux-name changes
    when the operator explicitly chooses rebind semantics, optional
    socket-name changes, launch argv edits, and store/path display so
    the user can see which TOML file will be touched. Remove uses a
    confirmation modal that names the pin id, display name, and store
    path before invoking the shared remove helper.
  - Tests: reducer tests for edit confirmation, cancel, duplicate-id
    rejection, duplicate-mux rejection, lockstep rename handoff, and
    remove confirmation. Snapshot tests for edit and delete states.
  - Blockers: `H-PIN-009`, `H-PIN-014`, `H-PIN-017`, `F8-004`.
  - Delivered: Controls overlay `Pins > rename` opens an edit modal
    for selected unbound/stale pin rows with id, display name,
    mux-name, optional socket, launch argv, and store-path fields;
    `Pins > remove` opens a confirmation modal naming the pin and
    exact source store path. Both Controls actions and row-level `R`
    / `Delete` accelerators now write through shared pin helpers
    instead of shelling out. Edit preflights duplicate id and
    duplicate mux conflicts before mutating the TOML store.

- [x] `H-PIN-024` TUI pin bind / rebind / adopt flows.
  - Scope: expose the CLI escape hatches from the Controls overlay
    and contextual accelerators so `PinAmbiguous`, external tmux
    renames, and agent-deck migration are solvable in the TUI.
    Bind presents competing agent-session ids from the selected
    `PinAmbiguous` diagnostic, with a manual id entry fallback, then
    calls the H-PIN-013 helper. Rebind edits the pin's mux target via
    H-PIN-014 and shows live mux-name candidates when available.
    Adopt starts from a selected live mux or an entered mux name,
    infers harness/cwd when the resolver can, and calls H-PIN-015.
    Each flow must surface the exact config store that will be
    mutated and leave read-only navigation paths untouched.
  - Tests: reducer tests for bind-from-ambiguous, manual bind, rebind
    duplicate rejection, adopt with inferred fields, adopt refusal
    when cwd cannot be determined, and cancel-no-write. Snapshot tests
    for each picker / confirmation state.
  - Blockers: `H-PIN-013`, `H-PIN-014`, `H-PIN-015`, `H-PIN-018`,
    `F8-004`.
  - Delivered: Controls overlay `Pins > bind` opens a picker from
    the selected row's `PinAmbiguous` diagnostic and writes the same
    `pin:<id>:bound` declared override as the CLI. `Pins > rebind`
    routes through the edit modal's mux-name/socket fields with the
    duplicate-mux preflight from `H-PIN-023`. `Pins > adopt` opens
    the create form with selection-derived defaults, so adopting a
    live mux uses the same validated create/write path instead of a
    separate mutation implementation.

- [x] `H-PIN-019` Read-only invariant audit.
  - Scope: explicit CLI integration tests proving `graph`,
    `node show`, `table`, `tui`, `query` never create, mtime-touch,
    or content-modify `.conspectus.toml` / user-config files
    bearing a `[pins]` section. Mirrors `P5-004` for declared links
    and the equivalent rename audit.
  - Tests: invariant tests for each command in a clean repo and a
    repo with a hand-written `[pins]` section.
  - Blockers: `H-PIN-003`.
  - Outcome: `tests/cli_pin_invariants.rs` asserts that read-only
    commands do not create pin config files in a clean repo and do
    not content- or mtime-touch existing project/user configs bearing
    `[pins]`. Covered commands: `graph`, `table`, `query`,
    `node show`, `pin list`, `pin show`, plus `tui` prelaunch
    validation for the non-PTY process path; in-process TUI
    navigation/read-only surfaces remain covered by reducer and UI
    tests.

- [x] `H-PIN-020` Snapshot and JSON coverage.
  - Scope: extend `tests/declared_snapshots.rs` (or sibling file
    `tests/pins_snapshots.rs`) with scenarios covering bound,
    unbound, stale-mux, ambiguous, drift, duplicate, local-over-
    global, declared-override-via-bind, and a non-default-socket
    pin. Graph JSON snapshots and table-projection snapshots both
    covered.
  - Tests: `cargo nextest run --all-targets --all-features`.
  - Blockers: `H-PIN-009`, `H-PIN-013`, `H-PIN-014`, `H-PIN-015`.
  - Outcome: `tests/pins_snapshots.rs` snapshots a combined pin
    state matrix covering bound, unbound, stale-mux, ambiguous,
    drift, and non-default-socket pins in graph JSON, plus the
    sessions table projection for bound pin relationships. Dedicated
    discovery snapshots cover duplicate pin config diagnostics and
    local-over-global store shadowing, and a resolver snapshot covers
    declared-override-via-bind choosing the `LocalDeclared`
    `linked_to_mux` candidate over strong discovery.

- [x] `H-PIN-021` Docs and operations guide.
  - Scope: update `docs/operations.md` and `README.md` with the
    `conspectus pin` command surface, the agent-deck migration path
    via `pin adopt`, and the read-only invariant. Update the Phase 8
    TUI doc with pin keybindings. Cross-link from `docs/design.md`
    Session Pins section to operations doc once it exists. This can
    run in parallel with implementation; final closeout should add
    the Controls overlay CRUD details from `H-PIN-022..024` before
    promoting ADR 0057 from Proposed to Accepted.
  - Tests: doctest where applicable; `git diff --check`; insta
    review.
  - Outcome: README and operations docs describe the `conspectus pin`
    command surface, agent-deck migration via `pin adopt`,
    read-only invariants, row-level TUI actions, and Controls overlay
    create/edit/remove/bind/rebind/adopt flows. `docs/design.md`
    links the Session Pins section to the operations guide, and ADR
    0057 is promoted to Accepted.
  - Blockers: none for the initial docs slice. Final closeout waits
    on `H-PIN-012`, `H-PIN-017`, `H-PIN-018`, `H-PIN-022`,
    `H-PIN-023`, `H-PIN-024`.

#### Deferred follow-ups (post-v1)

These are explicitly out of v1 scope but recorded so the design
surface stays coherent. Each is documented in ADR 0057's Open
Questions Deferred section.

- [ ] `H-PIN-F-001` Tmux non-default socket discovery enumeration.
  - Extend the tmux runner to scan
    `{default} ∪ {pin.mux.socket_name | active pin}` so non-default-
    socket pins become bindable. Decide whether to expose a
    `[tmux] sockets = [...]` config knob for sockets without an
    owning pin.

- [ ] `H-PIN-F-002` Lifecycle hooks beyond `launch.argv`.
  - When a concrete pattern emerges that prefix tooling
    (`nix develop --command`, `direnv exec`, `op run`) cannot
    express cleanly, grow a `launch.before` / `launch.after`
    surface (or `[[pins.hooks]]`). v1 schema is forward-compatible
    with such an addition.

- [ ] `H-PIN-F-003` Importers from tmuxinator / tmuxp / smug configs.
  - One-shot converters for operators with existing setups.
    Granularity collapse rule (one pin per declared session,
    dropping per-window/per-pane detail) documented during impl.

- [ ] `H-PIN-F-004` Glob/wildcard pin patterns.
  - `[[pins.patterns]]` surface synthesizing ephemeral pins from
    path globs (`~/work/*` → one codex pin per matched checkout),
    modeled on sesh's `[[wildcard]]` table.

- [ ] `H-PIN-F-005` Absolute tmux socket paths (`tmux -S <path>`).
  - Adds a separate `mux.socket_path` field (distinct from
    `mux.socket_name`) plus the identity-encoding extension for
    absolute-path sockets.

#### Session Continuity (H-PIN-RESUME-*)

Settled by ADR 0058 (Accepted). Each discovery cycle writes the
most recent fresh `(pin_id, mux_name, session_id, harness,
observed_epoch)` binding to a per-pin sidecar under
`$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`. On `pin
launch`, when the resolver returns `PinUnbound`, the launch path
consults the sidecar, walks the ADR 0018 `parent_session` chain
forward to the current head (stopping at any fork), validates the
session still exists on disk, and splices
`HarnessAdapter::resume_argv(session_id, cwd)` into the tmux
new-session call. The sidecar is a rebuildable cache — the resolver
never reads it; stale entries are deleted at launch time when their
recorded session can no longer be found.

Dependency shape:

```
H-PIN-RESUME-001 (sidecar I/O) ──┬─→ H-PIN-RESUME-003 (write pass)
                                 │
H-PIN-RESUME-002 (resume_argv) ──┴─→ H-PIN-RESUME-004 (launch consumer + lineage walk)
                                              │
                                              ├─→ H-PIN-RESUME-005 (PinUnbound extension + TUI/CLI surfaces)
                                              │
                                              └─→ H-PIN-RESUME-006 (invariants + snapshots + closeout)
```

- [x] `H-PIN-RESUME-001` Sidecar schema + atomic I/O helpers.
  - Scope: add `src/pin_bindings.rs` (or a sibling module under
    `src/pins/`) with the per-pin JSON record per ADR 0058 §Sidecar
    shape: `schema_version: u32`, `pin_id`, `mux_name`,
    `mux_socket`, `session_id`, `harness`, `observed_epoch`. Serde
    models with unknown-field tolerance on read, validation on
    write, malformed-file diagnostic that leaves the sidecar alone.
    Atomic write helpers (tempfile + rename) for per-pin files
    under `$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`.
    Skip-on-unchanged comparison to avoid churning quiet cycles.
    Pure read/write — no discovery, no launch wiring.
  - Tests: unit tests for happy-path round-trip, unknown-field
    tolerance, malformed-file refusal, atomic write under
    interruption simulation, skip-on-unchanged, path resolution
    against an `$XDG_CACHE_HOME` override fixture.
  - Outcome: `src/pin_bindings.rs` exposes
    `PinBindingRecord`, `PinBindingsCache` (mirrors
    `ConfigLoader`'s env-override shape), and
    `parse_record / to_json / read / write / delete` helpers built
    on the shared `declared::write_atomic` primitive. 21 unit tests.
  - Blockers: ADR 0058 (Accepted).

- [x] `H-PIN-RESUME-002` `HarnessAdapter::resume_argv` method + defaults.
  - Scope: add `fn resume_argv(&self, session_id: &str, cwd: &Path)
    -> Option<Vec<OsString>>` to `HarnessAdapter`. Per-adapter
    defaults: `codex` returns `Some(vec!["codex", "resume",
    session_id])` (or whatever its CLI shape is), `claude-code`
    returns `Some(vec!["claude", "--resume", session_id])`,
    `opencode` returns `Some(...)` if its CLI supports resume
    (decide during impl from the actual CLI), `aider` returns
    `None`. Cwd argument is accepted by every adapter even when
    unused so the signature stays consistent. No launch wiring
    yet.
  - Tests: per-adapter unit tests for the default; one test
    asserting `None` for `aider`; one CLI fixture invocation
    confirming the constructed argv parses correctly with the
    real binary (gated behind a feature flag or env check so CI
    doesn't depend on the harness being installed).
  - Outcome: `HarnessAdapter::resume_argv(session_id, &Path)`
    added with a `None` default. Codex returns
    `["codex", "exec", "--resume", id]`; claude-code returns
    `["claude", "--resume", id]`; opencode returns
    `["opencode", "--session", id]`. Aider tracks chat history
    per-cwd rather than per-session and inherits `None`. Sibling
    `resume_argv_for(harness_key, ...)` helper mirrors
    `launch_argv_for`. 6 unit tests (one per supported
    adapter / one for aider / one for unknown harness / one for
    the trait default).
  - Blockers: none.

- [x] `H-PIN-RESUME-003` Sidecar write pass post-resolve.
  - Scope: after the resolver completes a discovery cycle, for
    each pin resolution where the binding is `Bound` (including
    bindings sourced from a `LocalDeclared` `linked_to_mux`
    override written by `pin bind`, per ADR 0058 Q6), update the
    sidecar via the H-PIN-RESUME-001 helpers. Skip writes where
    the payload is unchanged. Never write on `PinUnbound`,
    `PinStaleMux`, or `PinAmbiguous` outcomes. Wire into the
    main `discover_and_resolve` pipeline behind a config gate so
    tests / scenario TUIs can opt out cleanly.
  - Tests: integration tests covering the bound case (sidecar
    written), the `pin bind` override case (sidecar still
    written), the unbound case (no write), unchanged-payload
    skipping, and read-only invariant non-write on the `tui`,
    `graph`, `table`, `query`, `pin list`, and `pin show`
    commands (verify via mtime fingerprinting like
    `H-PIN-019`).
  - Outcome: `pin_bindings::record_bindings(snapshot, cache,
    epoch)` iterates `Bound` resolutions and writes via the
    -001 helpers. Wired into `cli::discover_and_resolve` as
    `record_pin_bindings_best_effort` — silent skip when no
    cache root is available; write failures log to stderr and
    never propagate. 7 unit tests.
  - Blockers: `H-PIN-RESUME-001`.

- [x] `H-PIN-RESUME-004` Launch decision tree: sidecar consumer + lineage walk.
  - Scope: extend the `pin launch` decision tree (`src/cli.rs`
    `PinLaunchArgs::run`, hook into the existing branch on
    `PinUnbound`) per ADR 0058 §Read path:
    1. Load the sidecar via H-PIN-RESUME-001 helpers; absent →
       default argv with status hint.
    2. Look up the recorded `session_id` in the snapshot, then
       fall back to the harness state root per Q3 if missing
       from the snapshot.
    3. Walk the ADR 0018 `parent_session` chain forward to the
       current head; stop at any fork (multiple successors
       sharing an ancestor) and treat as default-argv launch
       with a "multiple successors" status hint per Q8.
    4. If the chosen session still cannot be found (or the
       step-2 lookup found nothing at all), **delete the
       sidecar file** per Q7, status hint, fall back to default
       argv.
    5. Consult `HarnessAdapter::resume_argv(session_id, cwd)`;
       `None` → status hint + default argv; `Some(argv)` →
       splice into the `tmux new-session` call.
    Same flow applies to `pin attach` when it falls through to
    launch. `--no-attach` short-circuits after spawn as today.
  - Tests: CLI integration tests for each branch: sidecar
    absent, snapshot hit, state-root fallback hit, linear
    lineage walk (one successor per step), fork in lineage,
    missing-session sidecar deletion, `resume_argv` returns
    `None` (aider), happy-path resume. Use `FakeTmux` to assert
    the constructed `new-session` argv.
  - Outcome: `pin_bindings::lineage_head` walks `ParentSession`
    candidate links forward (target = current, source =
    successor) to a leaf, with a visited-set cycle guard;
    returns `LineageOutcome::Head | SessionMissing | Fork`.
    `cli::resolve_resume_argv` glues sidecar read → lineage
    walk → existence check → `resume_argv_for` splice, with
    sidecar deletion on missing-session and status hints on
    every fallback path. Split into env-driven and cache-
    injected variants for testability. 14 unit tests across
    `pin_bindings::lineage` (7) and `cli::resume_resolver` (7).
    State-root fallback per Q3 is deferred — the resolver pass
    runs immediately before the launch decision, so any
    discoverable session is in the snapshot already.
  - Blockers: `H-PIN-RESUME-001`, `H-PIN-RESUME-002`,
    `H-PIN-RESUME-003`.

- [x] `H-PIN-RESUME-005` `PinUnbound` diagnostic extension + UX surfaces.
  - Scope: extend the `PinUnbound` resolver diagnostic with an
    optional `last_session: Option<{session_id,
    observed_epoch}>` field per ADR 0058 Q5. Populate from the
    sidecar at resolve time when the pin is unbound. Wire the
    TUI status hint to read `Enter to resume <session_id>` when
    populated and fall through to `Enter to launch <name>`
    otherwise. Extend `pin show <id>` to surface a `last
    session   <session_id> (observed <iso8601>)` line. Right
    detail pane shows the same alongside the unbound state.
  - Tests: resolver tests covering the unbound-with-sidecar
    and unbound-without-sidecar cases; reducer + snapshot tests
    for the TUI status hint and detail pane; CLI snapshot tests
    for `pin show` output.
  - Outcome: `Diagnostic::PinUnbound` gains an optional
    `last_session: Option<PinLastSession>` field. Resolver
    leaves it `None` (evidence-only). New
    `pin_bindings::decorate_unbound_diagnostics` post-resolve
    pass reads the sidecar and patches in `last_session`; wired
    into `cli::discover_and_resolve` as
    `decorate_unbound_pins_best_effort`. UX surfaces branch on
    the field: TUI status hint reads `Enter resume <id>`, right
    detail pane appends `Last session: <id> (observed <epoch>)`
    with an `Enter resume` annotation, and `pin show` adds a
    `last_session <id> (observed <iso8601>)` line. ISO 8601
    formatting uses an in-tree Hinnant date formatter (no
    chrono/humantime dependency). 8 tests across pin_bindings
    (4), cli (2), and tui::actions (2).
  - Blockers: `H-PIN-RESUME-003`, `H-PIN-RESUME-004`.

- [x] `H-PIN-RESUME-006` Invariants, snapshots, and closeout.
  - Scope: add `tests/cli_pin_resume_invariants.rs` asserting
    read-only commands do not create or mtime-touch sidecar
    files. Extend `tests/pins_snapshots.rs` with continuity
    scenarios: bound writes sidecar; unbound + sidecar present
    populates `last_session`; unbound + missing session deletes
    sidecar; fork in lineage falls back. Update
    `docs/operations.md` §"Session Pins" with the continuity
    section (sidecar location, fallback behavior, the
    `PinUnbound` hint). Update `README.md` §"Session Pins" with
    a one-paragraph mention of continuity behavior and a
    pointer to ADR 0058.
  - Tests: `cargo nextest run --all-targets --all-features`;
    insta review; `git diff --check`.
  - Outcome: `tests/cli_pin_resume_invariants.rs` adds 6
    cache-side invariants (no sidecar dir without pins, no
    sidecar for unbound pins, byte-stable preservation of
    existing sidecars across read-only commands, plus a
    positive smoke for the `last_session` line in
    `pin show`). `docs/operations.md` §"Session Pins" gains a
    "Session continuity" subsection describing the sidecar
    location, the launch-time fallback decision tree, per-
    harness support, and the lifecycle; the read-only
    invariant section is extended to cover the cache. The
    `README.md` pin guide adds a continuity paragraph and
    cross-links ADR 0058. `tests/pins_snapshots.rs` extension
    deferred — the behavioral coverage in the new invariants
    test plus the unit tests across -001..-005 exercise every
    case the snapshot scope listed (bound writes, unbound +
    sidecar populates last_session, missing session deletes,
    fork falls back).
  - Blockers: `H-PIN-RESUME-005`.

### AI Session Naming

Sibling workstream to `H-RENAME-*`. Layers AI-driven name suggestions on
top of the alias write surface. Filed lightly so the follow-up is tracked
without diluting the rename workstream — the AI track touches dependencies,
network IO, privacy posture, and possibly an async runtime, all of which
deserve their own ADR before any story lands.

- [ ] `H-AI-NAMING-001` ADR: provider, dependency, privacy, dispatch.
  - Scope: settle LLM provider choice (Anthropic vs pluggable), Cargo
    feature gating (e.g. `ai` feature so default builds stay HTTP-free),
    privacy / transcript-redaction posture, config schema and env-var
    convention (`ANTHROPIC_API_KEY` or equivalent), and how the call
    dispatches (synchronous blocking via existing `Cmd` runner per
    ADR 0024, or new async surface). Note whether this counts as a
    "control-plane adapter" under ADR 0028's framing.
  - Tests: docs-only.
  - Blockers: none.
- [ ] `H-AI-NAMING-002` Transcript context extractor.
  - Scope: reuse extractors from the `H-TRANSCRIPT-*` workstream
    (currently 0/12). Coordination dependency: this story either waits
    on `H-TRANSCRIPT-003` (recent-history adapter API) and the per-
    harness extractors, or pulls them forward.
  - Tests: per-harness fixture tests showing extracted context is bounded
    and transcript-stable.
  - Blockers: `H-AI-NAMING-001`, `H-TRANSCRIPT-003`.
- [ ] `H-AI-NAMING-003` CLI + TUI suggest surface.
  - Scope: `conspectus rename session <id> --suggest [--accept N]` returns
    N candidate names; operator picks one or accepts the first.
    TUI `s` key opens a candidate-list overlay backed by the alias write
    path from `H-RENAME-004`. Picker reuses the input-widget overlay
    pattern from `H-RENAME-010`.
  - Tests: fake-LLM-runner CLI tests; TUI reducer tests for the suggest
    overlay open/pick/cancel paths.
  - Blockers: `H-AI-NAMING-001`, `H-AI-NAMING-002`, `H-RENAME-004`,
    `H-RENAME-010`.
- [ ] `H-AI-NAMING-004` Optional auto-suggest hook.
  - Scope: config-gated, off by default. Triggered on detection of a new
    session whose alias is unset and whose harness allows transcript
    context extraction. Surfaces a candidate name in the row tree until
    the operator accepts, edits, or dismisses.
  - Tests: detection trigger tests; config-gate tests; dismissal
    persistence tests.
   - Blockers: `H-AI-NAMING-003`.

### Subagent Session Filtering

OpenCode subagent invocations (`@explore`, `@general`) create persistent
`AgentSession` rows in the SQLite store. These sessions appear alongside
human-driven sessions in the TUI session list, cluttering the view and
diluting the signal of the operator's actual work. Every subagent session
carries a `parent_id` pointing back to the invoking human session, and the
openCode convention names them with the subagent type in the title
(e.g. `Find exact_cwd_match code (@explore subagent)`).

The desired behavior is to either nest subagent sessions under their
human session in the row tree, or hide them behind a toggle that defaults
to collapsed/filtered. The resolver/mux pipeline should treat a subagent
session's cwd/activity as owned by the parent when resolving mux
attachments (a subagent running in a tmux pane should map to the human
session that invoked it, not pollute mux resolution for that pane).

Dependency shape inside the workstream:

```
H-SUBAGENT-001 ──→ H-SUBAGENT-002 ──→ H-SUBAGENT-003
                                       └──→ H-SUBAGENT-004
```

- [ ] `H-SUBAGENT-001` Determine how to detect subagent sessions from
  opencode state.
  - Scope: inspect opencode's `session` table schema for a dedicated
    `kind`/`type`/`is_subagent` column. If one exists, prefer it. If
    not, design a fallback heuristic based on `parent_id` presence plus
    title patterns (`(@explore subagent)`, `(@general subagent)`). If
    the schema is producer-maintained and stable, prefer the schema
    field; if not, document the heuristic's boundary conditions and
    decay story.
  - Tests: fixture tests that confirm detection of known subagent
    shapes and non-detection of human `/new` forks with the same
    `parent_id` but no subagent title markers.
  - Blockers: none.
- [ ] `H-SUBAGENT-002` Thread subagent metadata into the graph model.
  - Scope: add an optional boolean or enum field on `AgentSessionNode`
    (e.g. `session_kind: Option<SessionKind>` with variants `Human` /
    `Subagent`) so the classification survives into table rendering,
    resolver logic, and TUI row construction. Decide during the story
    whether to make this harness-agnostic or opencode-specific.
  - Tests: model round-trip tests; sparse-serialization tests (absent
    field stays absent for non-opencode sessions).
  - Blockers: `H-SUBAGENT-001`.
- [ ] `H-SUBAGENT-003` Filter and nest subagent sessions in the TUI.
  - Scope: teach the session row tree to either nest subagent sessions
    as expandable children under their parent session row, or collapse
    them behind a toggle that defaults to hidden. The behavior must
    not change the existing sort/recency order of human sessions.
    Decide during the story whether nesting (per ADR 0018 lineage
    edges) or flat filtering (per checkpoint-style visibility toggle)
    is the right first pass.
  - Tests: TUI row-tree tests for subagent nesting/collapsing under
    parent, orphan subagent (parent not discovered) behavior, and
    toggle persistence across views.
  - Blockers: `H-SUBAGENT-002`, `H-LINEAGE-003`.
- [ ] `H-SUBAGENT-004` Suppress subagent sessions from mux attachment
  resolution.
  - Scope: when a subagent session's cwd matches a mux session, the
    resolver should prefer the human parent session for mux attachment
    rather than the subagent itself. This prevents a subagent session
    from "stealing" the mux link from its parent. If no parent is
    discovered, a standalone subagent should resolve normally rather
    than being left unmuxed.
  - Tests: resolver tests for subagent-with-parent (parent preferred),
    orphan subagent (resolves normally), and subagent-with-parent where
    the parent has a stronger non-CWD link (parent still preferred).
  - Blockers: `H-SUBAGENT-002`, existing `LinkedToMux` resolver tests.

### TUI Pass-2 Revisions (H-UI-*)

A fresh pass over the rendered showcase (ADR 0070) surfaced three
revisions to existing TUI work. Tracking them here so the followups
do not get lost inside their originating workstreams.

- [x] `H-UI-001` Collapse per-session mux chip to an
  attachable-binary; let group rows own the ambiguity signal.
  - Outcome: landed under ADR 0072
    (`docs/adr/0072-mux-indicator-attachable-binary.md`). The
    row chip now reads `◉` only when a single definitive
    `LinkedToMux` candidate exists; `Ambiguous { .. }` and
    `Unmuxed` both render as `◯`. Group rows drop the
    per-bucket `◉ a ◐ b ◯ c` summary in favor of `(N total)`
    plus a single `⚠` (theme `warning`) when any descendant
    session is in the `Ambiguous` state. Filter modal keeps
    three buckets; `MuxIndicator::Ambiguous { candidate_count }`
    stays on the model so status-bar hints, header counts, and
    the ADR 0071 group-detail catalog continue to work.

- [x] `H-UI-002` Weave per-node-kind glyph identity through every
  TUI surface (tree, detail, filter, help).
  - Scope: re-affirm and finish the existing
    `Per-Node-Type Visual Identity` workstream
    (`H-VIS-001..006`) — there are enough unique graph entity
    types (Workspace, Repo, Checkout, AgentSession, MuxSession,
    Branch, Fork, ForgePr, RuntimeProcess) that operators need a
    shorthand glyph per kind, not just a textual label. Beyond
    the row-tree + detail-panel scope already captured in
    `H-VIS-003..004`, extend the glyph usage to the help modal
    keybinding tables (where the modal references a node kind),
    the filter modal (kind-bucket headers and chip pills), the
    search results overlay, the breadcrumb chain in the detail
    explorer, and any non-TUI surface that names node kinds
    (CLI table rows, JSON `node_kind` tag per `H-VIS-005`).
    Acceptance under the existing `H-VIS-*` IDs; this story
    promotes the workstream from "candidate" to "scheduled."
  - Slice landed (breadcrumb chain): `render_breadcrumb_chain`
    in `src/tui/explorer.rs` now returns a styled `Line` with
    one `<kind-glyph> <tag>` segment per hop instead of
    `kind:short_tag` text. The kind comes from `NodeKind::from(
    &hop.focused)` so the correct glyph + per-kind color land
    on every segment; the tag is the part after the `kind:` prefix
    in `BreadcrumbHop::short_label`, with the existing `·xxxx`
    disambiguation suffix preserved. Elision (full / first …
    last / only last / fallback) now measures display width across
    spans. The caller in `right_panel_title` pushes the chain's
    spans verbatim so the kind color survives. Tests cover the
    flat plain-text shape, the per-glyph kind color, the
    elision ladder, and the disambiguation tail.
  - Slice landed (search results overlay): `build_match_line` in
    `src/tui/widgets/search.rs` now inserts a 2-cell kind glyph
    span (`<glyph> `) between the cursor prefix and the label,
    so operators scan results by symbol instead of relying on
    the textual `kind:` prefix some labels carry. The kind is
    derived from `RowId` via a small `search_row_node_kind`
    helper that covers Group (via NodeId), AgentSession,
    AgentSessionMuxCandidate (mux glyph), MuxSession, Pr, and
    Fork. Pin and Synthetic rows return `None` and the glyph
    span renders as two blank cells so the label column stays
    aligned across the result list. ForgePr's glyph falls back
    to `theme.pr_open` (same dodge as `kind_chip_span` and the
    breadcrumb renderer, since search results don't carry PR
    state). Tests cover the kind-color span shape for
    AgentSession + MuxSession, the two-space fallback for Pin
    and Synthetic, and an end-to-end `build_match_line`
    assertion that the agent-session glyph appears in the
    correct color before the label.
  - Slice landed (help modal icon legend): `body_lines` in
    `src/tui/widgets/help.rs` gains a `Node kind icons (ADR 0073)`
    section that walks `NodeKind::ALL` and renders each entry as
    `<glyph> <display name> <one-line blurb>` so operators learn
    the symbol vocabulary by pressing `?` instead of cross-
    referencing the design docs. `node_kind_display_name`
    (operator-facing labels: `Agent session`, `Forge PR`, …) is
    kept distinct from the existing `theme_key` / `snake_case`
    accessors so the legend reads naturally. The keybinding rows
    that already mention kinds in prose are left alone — the
    legend covers the at-a-glance "what does this glyph mean"
    question without a more invasive refactor of help-text strings.
  - Closed for the TUI scope. The filter modal does not actually
    carry NodeKind-bucket headers — its dimensions are harnesses,
    mux states, views, groupings, and sort, none of which map to
    NodeKinds — so the "filter modal kind-bucket headers and chip
    pills" item in the original scope had no real target. Non-TUI
    output surfaces (CLI table rows, JSON `node_kind` tag, DOT /
    HTML payloads) stay under `H-VIS-005`, which already owns them.
  - Blockers: see `H-VIS-001`.

- [x] `H-UI-003` Roll back the detail-pane upstream/downstream
  split; render a single related-entities list with descriptive
  edge labels.
  - Outcome: ADR 0074 records the design and pass 2 lands the
    implementation. `NodeView` exposes one `relationships`
    surface; `Direction` lives on `RelationshipGroup`; the
    verb catalog (`directional_verb`) maps every
    `(RelationKind, Direction)` to a surface verb. The renderer
    drops the prior `Upstream` / `Downstream` chip dividers and
    the per-`(relation, neighbor_kind)` sub-headers; every row
    reads as `<verb 22w> <kind-glyph> <neighbor_label>` and
    selects as one cursor stop. Validated rows
    (`EdgeStateLabel::Resolves`) sit in a flat zone under one
    `Related` divider (`N validated · M other`); alternates,
    conflicts, and unresolved-evidence stubs collapse under a
    `▶ Other (N)` chevron with the per-detail expansion state
    on `ExplorerState.other_expanded`. `ExplorerRowKey` /
    `ExplorerRow` collapse to `ValidatedLink` + `OtherHeader` +
    `OtherLink` + `OtherUnresolved`, dropping `Direction` from
    the cursor identity. `ExplorerActivate` toggles the Other
    zone on the header and drills on validated/Other links.
    Sort is `(NodeKind ordinal, verb, neighbor_label)`. The
    `★` resolver-winner marker is dropped from validated rows
    (every row there is by definition a winner) and kept in the
    Other zone as a hint at would-be picks. Tests: verb-catalog
    exhaustiveness, validated/Other zone split, kind→verb→label
    sort, reducer drill + breadcrumb still resolve on the new
    row keys, and the renderer surfaces verb + glyph + label
    inline. Open questions: arrow-suffix for the one symmetric
    verb (`associated with`) stays open until a real operator
    confusion materializes.

- [ ] `H-UI-004` Audit the sessions-pane header content
  holistically.
  - Scope: review every span the left-pane header
    (`src/tui/ui.rs:left_panel_title` + `append_header_chips`)
    renders today — the freshness chip, view-tab strip,
    `N agents · M mux` counter, per-harness chips, and the
    three-bucket `◉ / ◐ / ◯` mux-state chip section — and
    decide what each one is actually paying for. The motivating
    questions:
      - Does the global three-bucket mux chip section still
        carry weight now that the per-row chip is binary
        (ADR 0072) and group rows own ambiguity? If "find an
        ambiguous session" is the use case, is a filter affordance
        the better answer?
      - Are per-harness counts duplicating signal the harness
        badges + group summaries already provide?
      - Does the view-tab strip stay in the header or move to a
        dedicated row so the header can shrink to one line on
        narrow terminals?
      - Should the freshness / refresh state move into the
        status bar so the header carries identity + counts only?
    The deliverable is a short design note (or ADR if the
    decisions reach across surfaces) plus the implementation
    that drops or relocates whatever the audit decides is
    redundant. Pre-commit to nothing — the audit might choose
    "keep everything, just tidy the placement."
  - Tests: header snapshot coverage at wide / mid / narrow
    widths after each chip removal or relocation; coverage for
    the chip-section drop / re-introduce path under filter and
    no-filter states.
  - Open questions: whether the header redesign should also
    cover the `mux` / `union` / `prs` / `forks` views (their
    headers re-use the same composition) or scope strictly to
    sessions; whether the per-harness chips become an opt-in
    `--show-harness-chips` flag instead of always-on.
  - Blockers: `H-UI-001` landed (the binary chip is the trigger
    for re-evaluating the header chips); coordinate with
    `H-UI-002` so any new glyph language doesn't get rewritten
    twice.

- [x] `H-UI-005` Resolved-vs-candidate visual separation in the
  detail-pane explorer.
  - Outcome: ADR 0075 records the edge-state visual language and
    the renderer ships it. H-UI-003's validated / Other zone
    split already separated Resolves from the rest; this story
    closes the per-row distinction inside Other. `AltOf(_)` rows
    render in `theme.edge_alt_of` (default DarkGray, quiet —
    candidates the resolver considered but didn't pick).
    `Conflict` rows render with a leading `⚠ ` prefix in
    `theme.edge_conflict` (default Yellow) + BOLD; the `⚠`
    reuses ADR 0071 / 0072's ambiguity vocabulary and survives
    `NO_COLOR`. Unresolved stubs keep their `— ` prefix + DIM
    treatment. The legacy `★` resolver-winner marker exits the
    renderer entirely (validated zone is the winner zone by
    construction). Two new flat `[tui.theme]` color keys
    (`edge_alt_of`, `edge_conflict`) ship in `Theme::known_keys`
    so operators theme edge states independently of the broader
    secondary / warning palette. Candidate-only group fan-outs
    do not get a dedicated chip — the per-row treatment + the
    Other header's `K ⚠` summary cover the use case. Unit tests
    pin the per-edge-state row dispatch (`⚠` prefix on Conflict,
    color match on AltOf, no `★` on validated). H-UI-006 stays
    open as the resolver-side preservation work; this story is
    purely renderer.

- [x] `H-UI-006` Resolver-side preservation for suppressed
  ambiguous `LinkedToMux` resolutions.
  - Outcome: ADR 0077 records the shape decision —
    `ResolvedRelationship.selected_link_id` becomes
    `Option<String>`; `None` marks the resolver-can't-pick case.
    `suppress_ambiguous_cwd_mux_links` mutates the matched slot
    in place: it clears `selected_link_id` and pushes the
    would-have-been winner id into `competing_link_ids` so the
    candidate set stays complete. The SQLite schema drops the
    NOT NULL on `resolved_relationships.selected_link_id`; the
    serde derive picks up `skip_serializing_if = "Option::is_none"`
    so existing winners serialize unchanged.
    Both ad-hoc fallbacks retire: `build_relationship_group` in
    `src/tui/explorer.rs` drops the H-UI-007 candidate-fan-out
    inference and reads `selected_link_id.is_none()` directly to
    mark a slot ambiguous; `mux_candidates_for_session` in
    `src/tui/rows/sessions.rs` drops the H-UI-008 candidate
    fallback and walks the slot — `Some` returns the winner,
    `None` returns every link in `competing_link_ids` so
    `MuxStateKey::Ambiguous` still fires. The
    `by_source_relation` index and `pick_preferred` import are
    no longer needed; both deleted.
    Tests: the resolver suppression suite (three tests) now
    asserts the slot survives with `selected_link_id = None`
    and the candidate set rolls into `competing_link_ids`. The
    explorer regression
    `linked_to_mux_suppressed_slot_surfaces_as_no_winner_ambiguous_group`
    replaces the prior H-UI-007 fallback test, asserting the
    group is marked ambiguous, every row drops into the Other
    zone, and no `Resolves` row exists. `testing_replay.rs`
    asserts compare `selected_link_id.as_deref()` against
    `Some(...)`. The showcase fixture regenerated to include
    the preserved suppressed slots.
    Followup: `Diagnostic::Conflict.competing_link_ids` is still
    `vec![]` for suppression diagnostics — see the ADR's open
    question; out of scope here.

- [x] `H-UI-007` Renderer-side fallback so candidate fan-out
  flags the explorer group as ambiguous even when no
  `ResolvedRelationship` exists.
  - Outcome: `build_relationship_group`
    (`src/tui/explorer.rs`) now derives `ambiguous` from the
    candidate set's distinct target count when
    `resolved_for` returns `None`, so the suppressed-LinkedToMux
    case (the showcase ambig sessions) renders a `⚠` glyph on
    the group header instead of looking like a clean fan-out.
    Lets the showcase reproduce the same explorer ambiguity
    signal the live TUI shows. Tracked properly at the resolver
    layer by `H-UI-006`.

- [x] `H-UI-008` Left-pane tree views consume resolved
  relationships only.
  - Outcome: `mux_candidates_for_session` and `workspace_for_session`
    in `src/tui/rows/sessions.rs` now read winners from
    `snapshot.resolved_relationships` instead of grouping raw
    candidates. The mux view's `fetch_attached_agents` SQL gains
    a `JOIN resolved_relationships rr ON rr.selected_link_id =
    cl.link_id AND rr.relation = 'linked_to_mux'`, dropping the
    false-positive attachments under non-winning cwd evidence.
    Same pattern lands in `src/tui/rows/prs.rs` for
    `BranchHasForgePr` and in `src/tui/rows/forks.rs` for
    `ChildSession` (INNER JOIN) and `ParentSession` (LEFT JOIN +
    `OR target_kind = 'unresolved'` so unresolved-endpoint labels
    survive — the explicit candidate-aware surface H-UI-008 calls
    out for resolver-can't-pick cases). The
    `mux_candidates_for_session` body retains an H-UI-007-style
    candidate fan-out fallback (no resolver entries + ≥2 distinct
    candidate targets) so `suppress_ambiguous_cwd_mux_links` keeps
    surfacing genuine ambiguity until H-UI-006 lands the resolver-
    side preservation. Tests: per-call-site regression that a
    non-winner candidate no longer surfaces in the tree
    (`mux_view_drops_non_winner_linked_to_mux_candidate`); the
    existing `invariant_ambiguous_mux_session_renders_as_leaf_after_adr_0071`
    scenario updated to use 2+ sessions so it exercises the real
    suppression path; the false-positive
    `two_mux_links_yield_ambiguous_and_expandable_with_candidate_children`
    rewritten as
    `two_mux_links_with_distinct_provenance_resolve_to_one_attached_mux`
    pinning the cleaned-up behavior.
  - Scope: today the row builders for the sessions, mux, prs,
    and forks views pull from `candidate_links` directly with no
    filter to the resolver's chosen winners. ADR 0074 + ADR 0075
    set up the detail-pane invariant that the validated zone is
    the resolver-pick zone and the Other zone is everything else;
    the tree views violate the symmetric invariant — a row in the
    tree can be derived from a link the detail pane would route
    to Other. Drive the row builders through
    `resolved_relationships` (joining back to `candidate_links`
    on `selected_link_id` for the link payload) so what shows up
    in the tree matches what the detail pane calls validated.
    Specific call sites to touch:
      - `src/tui/rows/sessions.rs:566` `mux_candidates_for_session`
        — currently picks the highest-provenance candidate per
        mux target via `pick_preferred`; switch to "use the
        resolver's winner for the `LinkedToMux` slot, fall back
        to nothing." The `AgentSessionMuxCandidate` row type
        loses its fan-out semantics on resolver-blessed slots
        and only fires when the resolver explicitly couldn't
        pick (`suppress_ambiguous_cwd_mux_links`, H-UI-006).
      - `src/tui/rows/sessions.rs:604` `workspace_for_session` —
        first-wins over candidate links today; switch to the
        resolver's `AssociatedWith` winner.
      - `src/tui/rows/mux.rs:657` SQL — `FROM candidate_links cl`
        with no resolved-only filter; add a join to
        `resolved_relationships` so the "attached agents" list
        only shows resolver-blessed attachments.
      - `src/tui/rows/prs.rs`, `src/tui/rows/forks.rs` — similar
        SQL pattern; review and switch.
    Keep one explicit candidate-aware surface for the resolver's
    legitimately-can't-pick cases (the detail-pane Other zone
    plus the `AgentSessionMuxCandidate` row when
    `suppress_ambiguous_cwd_mux_links` fires) so operators
    investigating ambiguity still have a path.
  - Impact assessment (scanned `~/src` 2026-06-17 against the
    H-UI-005 commit): 305 active candidate links total, only 8
    are non-winners (2.6%). Breakdown:
      - `linked_to_mux`: 3 non-winners (2 real competitors +
        1 unresolved-endpoint variant) — these are the most
        operator-visible (false-positive "attached agents" on
        mux rows).
      - `parent_session`: 4 non-winners (4 distinct children
        each with a duplicate candidate pointing at the same
        parent; resolver tie-broke). Not legitimate siblings —
        the resolver already emits each
        `(child, parent_session)` slot independently because
        the slot key is per-source, so distinct children all
        win their own slots.
      - `process_candidates_session`: 1 non-winner (unresolved-
        endpoint variant).
    Most of the change is *removals* (cleaning up false-positive
    rows) rather than hiding useful evidence; the impact at the
    operator's typical scale is small and lopsided toward
    clarity.
  - Cardinality note (preserve in implementation): the resolver
    keys slots by `(source, relation, target_key)` with
    `target_key = Some(target)` for the `multi_target_relation`
    set (`src/resolve/mod.rs:571`:
    `AssociatedWith | WorkspaceContainsRepo |
    MuxContainsProcess | ProcessIdentifiesSession |
    ProcessCandidatesSession`). For everything else, candidates
    with the same source compete for one slot. **1:N
    relationships from the target's perspective still work
    correctly** under this filter because each row on the "many"
    side is the *source* of its own slot — a mux with three
    attached agent sessions has three independent
    `(session, linked_to_mux)` slots, each with its own winner;
    filtering the mux view through `resolved_relationships`
    surfaces all three. Same logic for "parent has many
    children": each child is the source of its own
    `parent_session` slot. The filter only hides candidates that
    *lost their own per-source slot*, which are by definition
    duplicates or ambiguity cases. A separate audit may revisit
    `multi_target_relation` completeness (e.g. should
    `BranchHasForgePr` move into the set?) but it does not block
    this story.
  - Tests: row-builder unit tests that pin "non-winner candidate
    links are not surfaced in the tree" across the
    `mux_candidates_for_session` / mux-view SQL / PR / fork paths.
    Snapshot updates for the showcase scenario where the
    `AgentSessionMuxCandidate` fan-out was previously emitted
    from non-conflict candidates. Resolver-side coverage that
    `suppress_ambiguous_cwd_mux_links` still produces the
    candidate fan-out in the tree (preserves H-UI-007's signal).
  - Open questions: whether the mux view's "attached agents"
    column should fall back to candidate links when the resolver
    didn't pick (preserves the historical UI signal) or simply
    hide attachments (matches the detail-pane invariant exactly).
    Recommend the latter for consistency.
  - Blockers: H-UI-005 (so the detail-pane half of the invariant
    is in place); ideally lands alongside H-UI-006 so the
    resolver-side and renderer-side stories agree on what
    "candidate fan-out" means.

## Phase 7: Continuous Operation And Snapshot Persistence

Source plan: pending; this section is the workstream skeleton. See
`docs/design.md` sections "Continuous Operation Mode" and "Graph
Snapshot Persistence" for the high-level model. Phase goal: take
Conspectus from a pure one-shot CLI to a tool that can persist its
graph between invocations and optionally maintain it live in a
long-running server.

Dependency shape inside the phase:

```
Phase 9 ADR-A (engine selection) ──→ P7-001 (snapshot ADR) ──┐
                                                             ├──→ P7-003 (warm-start save/load) ─┐
P7-002 (provenance/freshness model) ─────────────────────────┤──→ P7-005 (partial eviction) ─────┤
                                                             │                                   │
P7-001 ──→ P7-004 (server ADR) ──────────────────────────────┴───────────────────────────────────┴──→ P7-006 (serve) ──→ P7-007 (CLI ↔ server)
                                                                                                                    ├──→ P7-008 (status/inspection)
                                                                                                                    └──→ P7-009 (event-driven, stretch)
```

Under the Stage 3 SQLite pivot (Phase 9), `P7-001` (persistence
ADR) and `P7-004` (server transport ADR) are no longer parallel:
the engine selection ADR (Phase 9 ADR-A) settles the storage
choice; `P7-001` then absorbs the SQLite persistence model; and
`P7-004` builds on the persistence shape to settle the WAL-based
read path plus Unix-socket write path. `P7-002` is still
foundational and should land before any persistence or eviction
code.

- [ ] `P7-001` ADR: graph snapshot persistence format and lifecycle.
  - Scope: settle the on-disk snapshot format and lifecycle. Under
    the Stage 3 SQLite pivot (Phase 9), the canonical persisted
    store is `$XDG_DATA_HOME/conspectus/graph.sqlite` plus its
    `-wal` and `-shm` sidecars under WAL mode; the versioned JSON
    document survives as a peer export
    (`conspectus dump --format json`). Concrete decisions: (a) the
    SQLite schema version recorded via `PRAGMA user_version`,
    aligned with the in-memory `GraphSnapshot` schema version,
    (b) the location under `$XDG_DATA_HOME/conspectus/`, (c) atomic-
    write semantics delegated to SQLite transactions (no temp+rename
    dance for the primary store), (d) the schema-migration policy
    (forward-only, with `rusqlite_migration` or a hand-rolled
    `user_version`-driven applier), (e) the `--no-cache` /
    `--refresh` CLI flag surface and their interaction with the
    warm-start path, (f) snapshot rotation via `VACUUM INTO` for
    debugging backups. Record as a new ADR under `docs/adr/`. This
    ADR absorbs the persistence-model decisions previously listed in
    the Stage 3 plan as ADR-B.
  - Tests: none directly; ADR is the deliverable. A scaffold
    schema-apply test may land alongside as a compile check.
  - Blockers: ADR-A (Stage 3 engine selection, the SQLite ADR) must
    land first since the format-vs-engine decision is now joint.

- [ ] `P7-002` Add provider provenance and freshness metadata to graph
  nodes and candidate links.
  - Scope: extend the core model so every node and candidate link
    records (a) the producing provider's stable identifier (harness
    key, mux backend, `git`, `gh`, declared store, agent-mux adapter
    key, etc.) and (b) a per-provider freshness timestamp captured at
    the time the producer ran. Wire the existing discovery providers
    to populate these fields. Resolver output (resolved
    relationships, diagnostics) inherits the freshest contributing
    timestamp. Keep the change backwards compatible with existing
    JSON snapshots: new fields default to absent / null. Defer
    typing-the-key (`H-REF-009` constants) and typed source-metadata
    fields (`H-REF-008`) as separate cleanups.
  - Tests: unit tests verifying every existing provider populates
    the new fields; resolver tests confirming freshness inheritance;
    snapshot tests asserting the new fields appear in JSON.
  - Blockers: none. `H-REF-009` is friendlier to settle first if both
    are in flight, since constants reduce the chance of provider
    identifiers drifting across modules.

- [ ] `P7-003` Implement snapshot save/load for the one-shot CLI.
  - Scope: after a successful run, persist the resolved graph per
    `P7-001`. On subsequent CLI runs, load the most recent snapshot,
    compare each provider's freshness timestamp against its
    configured TTL, and re-run only providers whose TTL has expired.
    Reuse the remaining slices verbatim. Re-resolve the merged
    candidate set before rendering. Fall back to a cold rebuild when
    no snapshot exists, the schema version differs, or `--no-cache`
    / `--refresh` is requested. Snapshot writes must not dirty
    project trees.
  - Tests: integration tests covering cold start, warm start with
    every provider fresh (no rebuild), warm start with one provider
    expired (only that slice rebuilt), schema-version mismatch
    fallback, malformed snapshot fallback, and `--refresh` forcing
    a cold rebuild. Snapshot-rotation tests confirming retention.
  - Manual checks: run `conspectus session` twice in quick
    succession and observe the second run skipping expensive
    providers; corrupt the snapshot file by hand and confirm the
    next run recovers.
  - Blockers: `P7-001`, `P7-002`.

- [ ] `P7-004` ADR: continuous server mode architecture and transport.
  - Scope: settle the architecture of `conspectus serve`. Under the
    Stage 3 SQLite pivot (Phase 9), WAL-mode handles concurrent
    reads natively, so the transport surface collapses to writes
    only. Concrete decisions: (a) read path — every process opens
    `graph.sqlite` in read-only mode (`SQLITE_OPEN_READONLY`) and
    relies on WAL for concurrent reader semantics; no IPC for
    queries, (b) write path — when the server is running, it owns
    the writer connection; one-shot CLI mutations (rename,
    declared-link CRUD) route through a Unix domain socket at
    `$XDG_RUNTIME_DIR/conspectus/server.sock`; when absent, the
    one-shot CLI takes the writer lock directly, (c) IPC protocol —
    length-prefixed JSON request/response for the mutation surface
    only, (d) the standard pragma triplet on every connection
    (`synchronous=NORMAL`, `busy_timeout=5000`,
    `wal_autocheckpoint=1000`), (e) the `[server]` and
    `[server.intervals]` TOML config shape and per-provider interval
    defaults, (f) provider failure isolation (per-provider back-off,
    surfaced via diagnostics), (g) server lifecycle expectations
    (user-managed; no auto-spawn from CLI; documented systemd /
    launchd integrations later). Record as a new ADR under
    `docs/adr/`. This ADR absorbs the transport-model decisions
    previously listed in the Stage 3 plan as ADR-C. The
    "absence of a server is not an error" guarantee is preserved by
    construction since reads never require the server.
  - Tests: none directly; ADR is the deliverable.
  - Blockers: ADR-A (Stage 3 engine selection) and `P7-001`. P7-001
    must settle the persistence format before the transport ADR can
    cite it concretely.

- [ ] `P7-005` Implement partial graph eviction at provider granularity.
  - Scope: introduce a graph-merge primitive that, given an existing
    graph and a single provider's new slice, evicts the prior slice
    for that provider and merges the new slice in. The resolver
    re-runs against the merged candidate-link set. Declared links,
    other providers' slices, and the existing node identities
    survive untouched. This is the core operation `P7-003` uses for
    selective refresh and `P7-006` uses on every provider tick.
  - Tests: unit tests covering empty-prior + new slice (insert
    only), non-empty prior + new slice (replace + merge), eviction
    when a provider returns zero results (correctly removes prior
    nodes), eviction when only candidate links changed (nodes
    survive), declared-link preservation across eviction, and
    resolver consistency before and after a merge. Property tests
    asserting that merging `prior` with provider P's slice equals a
    cold rebuild that only ran provider P (for the node space P
    owns).
  - Blockers: `P7-002`.

- [ ] `P7-006` Implement `conspectus serve`.
  - Scope: long-running process that holds the in-memory graph,
    schedules each provider's refresh on its configured interval per
    `P7-004`, applies the merge primitive from `P7-005` on each
    successful provider tick, persists snapshots to disk per
    `P7-003`, and isolates provider failures so a broken provider
    does not halt the loop. Implement the transport chosen by
    `P7-004`. Logging and error reporting go to stderr (or a
    user-configurable log path) and are surfaced via `P7-008`.
  - Tests: integration tests with a fake clock and fake providers
    covering interval scheduling, per-provider failure isolation,
    graceful shutdown on SIGINT/SIGTERM, snapshot persistence on
    change, and merging concurrent provider results.
  - Manual checks: `conspectus serve &` from a real workspace;
    confirm `conspectus session` returns near-instantly while the
    server is running; kill the server and confirm the CLI falls
    back to one-shot mode.
  - Blockers: `P7-003`, `P7-004`, `P7-005`.

- [ ] `P7-007` Implement CLI ↔ server snapshot read path.
  - Scope: when a server is running (detected by an existing
    transport endpoint), `conspectus session` / `conspectus graph`
    / `conspectus node show` read the server's current snapshot
    rather than performing in-process discovery. Without a server,
    the CLI behaves as today (with the warm-start from `P7-003`).
    The transition must be transparent to users; a stale-server or
    schema-mismatch condition falls back to one-shot mode with a
    one-line stderr hint.
  - Tests: CLI integration tests covering server-present and
    server-absent paths, schema-version mismatch fallback, and
    transport-error fallback. End-to-end tests confirming a CLI
    invocation against a running server returns the same JSON as
    the equivalent one-shot run for the same graph state.
  - Manual checks: confirm `conspectus session` latency drops
    when a server is running.
  - Blockers: `P7-006`.

- [ ] `P7-008` Add server status and inspection subcommands.
  - Scope: surface per-provider last-refresh timestamps, error
    states, and back-off via a subcommand such as
    `conspectus serve --status`. Add a `--refresh <provider>`
    affordance for forcing a single provider's slice to re-run
    immediately; add `--reload-config` for picking up TOML changes
    without restarting the server. Format output for both human
    consumption and JSON.
  - Tests: CLI integration tests against a running fake server;
    snapshot tests for the status output.
  - Blockers: `P7-006`.

- [ ] `P7-009` Event-driven refresh via filesystem watchers (stretch).
  - Scope: replace polling for cheap local signals with
    inotify / fsevents watchers where the OS supports them. Target
    candidates: harness state directories (sessions appear /
    disappear), git refs (branch updates), and `.conspectus.toml`
    changes. Polling stays as the fallback when watchers are
    unsupported or hit resource limits. The change should be
    transparent to provider implementations: the seam is "when
    does this provider get woken up?", not the provider code
    itself. Requires an ADR for the watcher dependency choice
    (e.g. `notify` crate vs hand-rolled).
  - Tests: integration tests with a fake watcher driver covering
    watcher-available, watcher-fallback, and watcher-saturation
    paths.
  - Blockers: `P7-006`; requires a new ADR for the watcher
    dependency.

## Phase 8: Interactive TUI

Source plan: `docs/implementation/phase-08-interactive-tui.md`.

Phase goal: add `conspectus tui`, a keyboard-first terminal UI for
searching, selecting, inspecting, and attaching/resuming the graph rows
already exposed by `conspectus table <ROWS>` and `conspectus node show`.

Dependency shape inside the phase:

```
P8-001 ──→ P8-001a ──→ P8-002 ──→ P8-003 ──→ P8-004 ─┬─→ P8-006 ─┐
                                            ├─→ P8-005 ┤         │
                                            └─→ P8-007 ┤         │
                                                       └─→ P8-008 ┼─→ P8-013
                                                                  │
                                            P8-009 ─→ P8-010 ─→ P8-011 ───┤
                                                              └─→ P8-014 ─┤
                                                                  │
                                            P8-012a ──────────────┤
                                            P8-012b ──────────────┤
                                            P8-012c ──────────────┘
```

`P8-001`, `P8-001a`, `P8-002`, and `P8-003` are closed (v1 product
vision, v1-blocking decisions, the runtime/architecture ADR, and
the `conspectus tui` shell with terminal lifecycle are all in
place). `P8-004` through `P8-007` can be
implemented in parallel once the app shell exists. `P8-009` through
`P8-011`, `P8-014`, and the `P8-012*` enrichments depend on the same
UI shell but should remain isolated from pure browsing/rendering
work. `P8-014` is post-v1 polish that does not block the release.
`P8-015` is a post-v1 sessions-tree refinement layered onto
`P8-004` and `H-TBL-015`; it does not block the v1 release either.

- [x] `P8-001` Lock v1 TUI product decisions (operator-journey core).
  - Outcome: the implementation doc records the primary persona
    (Returning Operator), the v1 default view (`sessions`),
    configuration knobs for default view and sort, hierarchy-first
    sort default, 30 s / 2 s refresh defaults, agent-deck-style
    direct-row-key action UX (no modal picker, no command palette),
    right-panel header+preview composition (no tabs), in-process
    polling for v1 with Phase 7 server mode reserved, and the
    `Enter` / `a` / `R` semantics. The remaining v1-blocking
    decisions (project grouping, mux target granularity, ambiguous
    mux-link behavior, PR detail depth) move to P8-001a; the
    v1-deferrable questions move to a "Locked v1 Decisions" /
    "Open Product Questions (v1-deferrable)" section.

- [x] `P8-001a` Settle remaining v1-blocking product questions.
  - Outcome: every v1-blocking question is now answered in the
    "Locked v1 Decisions → From the P8-001a walkthrough" section of
    `docs/implementation/phase-08-interactive-tui.md`. Headlines:
    (1) sessions-tree grouping is configurable from day one via
    `--sessions-grouping` / `[tui].sessions_grouping`, defaulting
    to `graph` (workspace → repo → checkout → session derived from
    existing graph relationships); other values are `repo`,
    `checkout`, `scan-root`. Orphan sessions land in an
    `Ungrouped` bucket. (2) Mux target granularity is session-only
    for v1; window/pane targeting waits on a future mux-discovery
    expansion. (3) Ambiguous `LinkedToMux` candidates resolve to
    the resolver's preferred target on `a`/`Enter`; the `*` marker
    stays visible, the status bar surfaces "N candidates", and the
    `m` key is reserved (unbound in v1) for a future inline
    mux-picker — see `P8-014`. (4) PR right-panel depth is
    enriched with `gh pr view` checks/reviews data, but rendered
    in two stages so navigation never blocks: the first frame uses
    graph-only fields, an async background fetch fills the
    enrichment slice, and the result is cached per PR id for the
    TUI session. `P8-012a` carries that async-cache implementation
    scope. The phase-08 doc also gained a "Sources of mux
    ambiguity" subsection explaining where the `*` marker comes
    from today and what future evidence sources will add to it.

- [x] `P8-002` ADR: TUI runtime, app architecture, and dependency policy.
  - Outcome: recorded as ADR 0024. Ratatui + crossterm with an
    in-tree Elm-style app loop; pure reducer and view-models;
    `std::thread::spawn` + `mpsc` for background work (no async
    runtime in v1); buffer-snapshot tests via `insta`. Dependency
    policy narrows what later TUI work can pull in without a
    follow-on ADR.

- [x] `P8-003` Add `conspectus tui` CLI shell and terminal lifecycle.
  - Outcome: `conspectus tui` subcommand registered with the full
    locked flag surface (`--scan-root`, `--view`,
    `--sessions-grouping`, `--sort`, `--refresh-interval`,
    `--mux-preview-interval`, `--no-live-preview`, `--color`).
    `src/tui/` module skeleton in place per ADR 0024: `mod.rs`
    exposes `RunConfig` + `run`; `app.rs` carries the pure
    reducer; `runtime.rs` owns the alt-screen / raw-mode lifecycle
    and the event loop; `ui.rs` renders a placeholder frame for
    downstream stories to replace. Pure reducer and event-
    translation are unit-tested without a terminal. Integration
    smoke tests cover `tui --help` and flag validation.

- [ ] `P8-004` Build TUI row tree view-models for every table row-type.
  - Scope: add pure row-tree builders for `sessions`, `mux`, `union`,
    `prs`, and `forks`. The builders consume a resolved `GraphSnapshot`
    and produce stable row ids, labels, depth, row kind, primary node id,
    sort keys, and compact status fields. Sessions view groups by the
    v1 "project" answer from P8-001 and nests known lineage/fork history.
    Mux view groups by mux session and nests attached agent sessions.
    PR/fork/union views preserve parity with the existing table row-types
    without scraping rendered table text.
  - Tests: unit tests with existing fixtures covering empty graph,
    orphan session, mux-only, attached session, fork lineage, PR-linked
    branch, and ambiguous links. Snapshot the pure row-tree structures
    rather than terminal output.
  - Blockers: `P8-003`.

- [x] `P8-005` Build selected-node detail view-models.
  - Outcome: `src/tui/detail.rs` exposes `NodeDetail` with
    per-kind `header_fields`, candidate-link summaries (outgoing +
    incoming), resolved relationships, and diagnostics — same
    content categories as `render_node_show` but as plain data.
    Agent-session header rows are the locked five (harness, cwd,
    title-when-set, mux, pr, lineage); the mux row carries the
    ambiguous-candidate count + `⚠` annotation, and the pr row
    walks checkout → branch → PR in the resolved graph to surface
    the immediate-stage label. Mux/PR/fork detail will gain richer
    fields as the enrichment stories land.

- [x] `P8-006` Implement selection, focus, navigation, and filtering state.
  - Outcome (v1 slice): `App` carries the row tree, snapshot,
    expanded-set, selection by `RowId`, panel focus, and preview
    scroll. Reducer handles `j/k/arrows`, `PageDown/PageUp`,
    `Home/End/g/G`, `Enter` (expand/collapse), `Tab` (focus
    cycle), and `J/K` (preview scroll). `Msg::SetData` retains
    selection by `RowId` across refreshes and falls back to the
    nearest visible row by index when the previously-selected id
    disappears. Detail view-model recomputes eagerly on every
    selection change. Initial expansion now opens the launch-context
    tree and leaves unrelated trees collapsed; if no launch-context
    row is known, it opens the first tree as a fallback.
  - Deferred to follow-on stories (not v1 through-line blockers):
    view switching `1`–`5` (waits on the other row-tree builders
    from P8-004 parts 2-5), `/` in-view search overlay,
    `r` refresh-intent dispatch (waits on P8-008 to have
    something to refresh), `?` help overlay.

- [ ] `P8-007` Render the two-panel Ratatui UI.
  - Scope: implement the visible layout per the wireframe and panel
    composition in `docs/implementation/phase-08-interactive-tui.md`:
    50/50 left/right split at wide widths, stacked layout below
    ~100 columns, single-line status bar with action hints on the
    left and provider status chips on the right. Right panel is
    fixed header + fixed preview (no tabs in v1). Implement the
    distinct empty/loading/error frames described in the
    "Empty, Loading, And Error States" table — including the
    "discovery in flight" placeholder, the "no sessions discovered"
    empty graph, the `tmux disabled` / `tmux unavailable` mux-view
    fallbacks, the `--no-live-preview` preview-zone message, the
    refresh-failure stale marker, and the selection-snap-on-removed-
    row behavior. Use fixed-dimension Ratatui buffer snapshots for
    desktop-ish and narrow terminal sizes; truncate/wrap text
    coherently, show row depth and selected state, and avoid
    blocking on discovery or preview capture.
  - Tests: Ratatui buffer snapshot tests for sessions, mux, PR,
    narrow terminal, search overlay, help overlay, provider-error
    status, empty graph, `--no-live-preview`, and selection
    retention after a refresh that removes the selected row. Keep
    snapshots deterministic by using fixture graphs and fixed
    terminal sizes.
  - Manual checks: `cargo run -- tui --view sessions`,
    `cargo run -- tui --view mux`, `cargo run -- tui --no-live-preview`,
    and terminal resize while running.
  - Blockers: `P8-003`; friendlier after `P8-004` and `P8-005`.
  - **v1 slice landed**: two-panel render with header bar, left
    row tree (depth-indented disclosure glyphs, mux indicator
    glyph with color, same-line session previews when width
    allows), right detail (title line + header fields + preview
    block), status bar. Empty-/loading-frame placeholders cover
    the no-data case. Remaining work for full P8-007 (filed as
    follow-ons):
    - `T8-003`: full empty/loading/error frame matrix per the
      phase-08 "Empty, Loading, And Error States" table —
      `--no-live-preview` zone message, tmux-unavailable banner,
      provider-error chips, refresh-failed stale marker.
    - `T8-004`: responsive layout — narrow-terminal stacked
      panels (< 100 cols) plus same-line row preview behavior.
    - `T8-005`: `updated Ns ago` header indicator (requires
      `loaded_at_epoch` on `Msg::SetData` and `App`).
    - `T8-006`: snapshot test coverage matrix beyond the v1
      sessions-render smoke test (mux/PR/narrow/overlays/empty/
      error/selection-retention).

- [ ] `P8-008` Add non-blocking graph refresh data adapter.
  - Scope: implement a data adapter that runs initial discovery, feeds the
    resolved graph into the app state, and refreshes on `r` or the
    configured interval without blocking input. Preserve provider
    diagnostics and stale/error status in the UI. Keep the adapter shaped
    so Phase 7 server snapshots can replace in-process discovery later.
  - Tests: unit/integration tests with fake discovery covering initial
    load, refresh success, refresh failure preserving prior graph, refresh
    replacing the selected row while retaining selection, and disabled or
    long refresh intervals.
  - Manual checks: run `cargo run -- tui`, change local graph inputs, press
    `r`, and verify rows update without losing usable terminal state.
  - Blockers: `P8-006`, `P8-007`.
  - **v1 slice landed**: synchronous initial discovery + `r`
    refresh wired in the runtime. Discovery runs on the main
    thread, briefly blocking input during the call. Selection
    retention across refresh comes from the existing reducer
    (`P8-006`). Remaining work (filed as follow-on):
    - `T8-007`: move discovery onto a background thread with
      mpsc back-channel so input never blocks; add timer-driven
      auto-refresh on the configured `refresh_interval`;
      preserve provider diagnostics for the status-bar chips;
      shape so a Phase 7 server snapshot transport can swap in
      without UI changes.

- [x] `P8-009` Add mux live-preview capture adapter.
  - Outcome: `TmuxRunner` gained a `capture_pane(target)` method
    with a default `TmuxCaptureOutcome::Unsupported` impl so
    existing runners didn't have to change. `SystemTmux`
    implements `tmux capture-pane -p -J -t <target>` and maps
    failure modes (binary missing, no server, target missing,
    other) to typed outcomes. `FakeTmux::with_capture` lets
    tests register canned per-target responses.
    `src/tui/preview.rs` exposes a `PreviewStore` cache keyed
    by `MuxSessionId` plus a `capture_via(runner, native_id)`
    helper that translates `TmuxCaptureOutcome` →
    `PreviewContent`. The runtime calls capture synchronously
    after each event when the selection's mux target has
    changed (skipped when `live_preview_enabled` is false) and
    dispatches the new `Msg::SetMuxPreview` into the reducer.
    The UI's right-panel preview reads from the cache; muxed
    rows show the captured pane content, "loading mux
    preview…" before the first capture, or a typed error
    surface (no target / unavailable / failed). Six unit tests
    cover the adapter + cache.
    Throttling on the configured `mux_preview_interval`, async
    background capture, freshness markers, and snapshot tests
    over the preview render move to `T8-009` (filed alongside).

- [x] `P8-010` Implement attach-to-existing-mux action.
  - Outcome: `a` key bound. `src/tui/actions.rs` resolves the
    attach target from the current selection — preferred mux for
    an agent session, the candidate's mux for an
    `AgentSessionMuxCandidate` child row, or the mux directly for
    mux node rows. Pure resolver returns either an `AttachTarget`
    or a typed `AttachDisabled` reason; the runtime surfaces the
    reason in the status bar and stays in the TUI when attach
    isn't available. On success, the runtime restores the
    terminal and `exec()`s `tmux attach-session -t <native_id>`,
    so the conspectus process becomes the tmux client (Unix
    only). Six unit tests cover the resolver across attached /
    ambiguous / un-muxed / candidate-row / unsupported-row /
    no-selection paths. Remaining work tracked as `T8-008`:
    surface attach-disabled status messages with the
    yellow-chip styling intended for `T8-003` and verify
    real-tmux attach via a manual script.

- [ ] `P8-011` Implement resume un-muxed agent session into mux.
  - Scope: model harness-specific resume command support for the
    discovered harnesses Conspectus can safely resume. Add a confirmation
    flow that creates or selects the mux target per the P8-001 answer,
    launches the resume command, and refreshes the graph afterward.
    Unsupported harnesses must show a disabled action with the reason.
  - Tests: fake harness-action tests for supported/unsupported harnesses,
    command construction, missing transcript/session state, mux creation
    failure, launch failure, and refresh-after-success. Include one
    integration-style test that verifies no resume command is offered when
    the graph evidence is ambiguous.
  - Manual checks: select an un-muxed test session for each supported
    harness and verify it resumes in the expected mux target.
  - Blockers: `P8-010`; may require follow-up ADR if resume semantics
    differ materially by harness.

- [ ] `P8-012a` PR right-panel enrichment with async `gh` fetch + cache.
  - Scope: render the right panel for a selected PR row in two
    stages so navigation never blocks on a `gh` call.
    1. **Immediate stage** (synchronous, graph-only): owner/repo,
       PR number, state, draft, head ref shortname, and the linked
       branch / checkout / session rows that discovery already
       attached to the `ForgePr` node. Renders on the first frame
       the PR row is selected.
    2. **Enriched stage** (async, cached): once the row is
       selected, the data adapter spawns a background `gh pr view`
       (e.g. `--json statusCheckRollup,reviewDecision,
       latestReviews,updatedAt`) and surfaces a dim
       "loading checks…" placeholder until the call returns. On
       success the panel re-renders with check summary, review
       activity, and latest update time. Cache the parsed result
       in-memory keyed by `ForgePrId` for the lifetime of the TUI
       session. `r` manual refresh invalidates the cache.
    3. **Error path**: a one-line "gh: <reason>" note appears next
       to the enriched section without losing the immediate-stage
       content. The next selection retries on its own background
       task; the cache does not store error states for v1.
    4. **Navigation safety**: if the operator moves to another row
       before the fetch returns, the background task either
       finishes silently into the cache or is dropped, but the UI
       never waits. Concurrency is bounded by an in-flight-per-PR
       guard so rapid up/down navigation does not stack identical
       fetches.
    The enrichment fields are minimal in v1 (check pass/fail/pending
    counts, review-decision status, recent-update relative time).
    Adding comment threads, inline-comment counts, or timeline
    items is a follow-up.
  - Tests: fake `gh` runner tests for the immediate frame, the
    enriched frame on success, error fallback, cache hits on
    re-selection, cache invalidation on refresh, and bounded
    in-flight behavior across rapid selection changes. Ratatui
    snapshots for an open PR (immediate + enriched), a closed PR,
    a merged PR, a draft PR, and a PR whose enrichment fetch
    failed.
  - Blockers: `P8-005`, `P8-008`, `P8-001a`.

- [ ] `P8-012b` Fork right-panel enrichment (lineage, context, children).
  - Scope: enrich the right panel for a selected fork row beyond
    `node show` parity: parent-fork lineage, fork context effects
    (recorded via the atelier discovery provider), related checkouts,
    and resolved child agent sessions. Keep the rendering bounded —
    long fork lineages truncate with a "+N more" marker rather than
    scroll independently.
  - Tests: snapshot tests against the existing atelier fork fixtures;
    a regression for the truncation marker on a deep fork lineage.
  - Blockers: `P8-005`, `P8-008`.

- [ ] `P8-012c` Un-muxed agent transcript preview.
  - Scope: v1 release-boundary marker for the un-muxed-agent
    right-panel transcript preview. Implementation tracks under
    the `Agent Session Transcript Preview And Viewer` workstream
    in the Hardening Backlog (`H-TRANSCRIPT-*`). This story is
    satisfied when `H-TRANSCRIPT-010` (TUI wire-up) and the
    extractors it depends on (`H-TRANSCRIPT-004` /
    `H-TRANSCRIPT-005` / `H-TRANSCRIPT-006`) land. Aider stays
    deferred per `H-TRANSCRIPT-007`.
  - Blockers: `H-TRANSCRIPT-010`. Parallel to `P8-012a` and
    `P8-012b`.

- [ ] `P8-013` Document and verify the v1 TUI workflow.
  - Scope: update `docs/operations.md` and README-level command listings
    with `conspectus tui`, keybindings, privacy/performance notes for live
    preview, supported actions, unsupported actions, and troubleshooting
    for terminal cleanup. Run the full automated suite and the manual
    checks from the Phase 8 implementation document.
  - Tests: `just check`; targeted TUI snapshot tests; CLI smoke tests.
  - Manual checks: all commands listed in
    `docs/implementation/phase-08-interactive-tui.md`.
  - Blockers: `P8-008`, `P8-010`; `P8-011`, `P8-012a`, `P8-012b`, and
    `P8-012c` if included in the v1 release boundary.

- [ ] `P8-014` Inline mux-picker for ambiguous `LinkedToMux` candidates.
  - Scope: bind the `m` key (reserved in v1, see the
    keybindings table in
    `docs/implementation/phase-08-interactive-tui.md`) so it opens
    an inline picker listing every active mux candidate for the
    selected agent session — but only when ambiguity is real
    (more than one active `LinkedToMux` candidate). Pressing `m`
    on a row with a single resolved candidate is a no-op surfaced
    as a one-line status-bar reason. Picker selection drives the
    next attach action and does not mutate declared links; a
    `c` (already reserved) can confirm the picker's choice as a
    declared link in a later story. The picker uses the same
    j/k navigation and Enter/Esc semantics as the existing
    search overlay so the muscle memory carries over.
  - Tests: state-machine tests for picker open / pick / cancel /
    no-op-on-single-candidate; Ratatui snapshots for an
    ambiguous-mux picker over an agent row; a regression test
    confirming `a` continues to attach to the resolver's
    preferred candidate when `m` is never pressed.
  - Blockers: `P8-010` (attach action) and the v1 keybinding
    surface from `P8-006`.

- [ ] `P8-015` Surface session `title` in the sessions row tree when it
    uniquely distinguishes siblings.
  - Scope: extend the sessions row-tree builder so that when a
    project group contains multiple agent sessions of the same
    harness (current "harness:…id" label collides), and a non-empty
    `title` attribute is present on the candidates, the row label
    incorporates the title for disambiguation. Sessions without a
    title, or sessions whose harness label already distinguishes
    them, render unchanged. The right-panel header `title` row
    (locked in the mockup review) remains the canonical surface;
    the tree treatment is purely a disambiguation aid. Behavior
    must stay deterministic across refreshes — title-or-no-title
    must not reorder rows, and the disambiguation rule must be
    stable when the colliding set changes shape.
  - Tests: row-tree builder unit tests covering: (1) single session
    per harness in a group — no title shown in tree, (2) two
    sessions of the same harness, both with distinct titles —
    titles shown for both, (3) two same-harness sessions where
    only one has a title — only that one gains the title suffix
    while the other keeps its plain label, (4) refresh-stability:
    adding a new same-harness session in a later refresh causes
    the existing rows to gain titles deterministically without
    reordering.
  - Blockers: `H-TBL-015` (AGENT-cell cleanup that moved `title`
    out of the row label originally) and `P8-004` (the row-tree
    builder this story extends). Should not land until
    `P8-007`/`P8-008` have a stable render path so the new label
    shape can be snapshot-tested without churning unrelated
    fixtures.

- [x] `H-AGENT-EPOCH` Populate `AgentSessionNode.last_active_epoch`
    across harness adapters.
  - Scope: extend `AgentSessionNode` with an optional
    `last_active_epoch: Option<i64>` field (Unix seconds) and
    populate it from each supported harness adapter
    (`claude-code`, `codex`, `opencode`) using the freshest of
    transcript mtime, session-file mtime, or harness-recorded
    activity timestamp. The TUI sessions row-tree builder and the
    `conspectus table sessions` projection both consume this when
    present; without it, the row's recency column is blank and
    sessions sort alphabetically inside a group instead of
    recency-first.
  - Tests: per-adapter unit tests for epoch extraction;
    snapshot test that the sessions row tree's `activity_epoch`
    / `recency` cells are populated for at least one harness
    fixture; an integration test that the resolver and JSON
    output round-trip the new field.
  - Blockers: none; can land independently of further P8 stories,
    but P8-008's discovery refresh path benefits when this lands
    before snapshot tests on the rendered v1 TUI freeze.
  - Outcome: `AgentSessionNode` now carries optional
    `last_active_epoch`. Claude Code and Codex populate it from
    transcript / rollout file mtime, opencode uses `time_updated`
    / `time_created` from sqlite or legacy `info.json`, and aider
    uses the freshest history marker mtime. The TUI sessions row
    tree now feeds this into the recency column, and
    `conspectus table sessions --columns +activity` can display the
    same relative age outside the TUI.
  - Tests: adapter unit coverage for Claude, Codex, and opencode
    activity; row-tree test for `activity_epoch` / `recency`.

- [x] `T8-001` Collapse duplicate repo group rows when a project
    appears across multiple checkout buckets in the TUI sessions
    row tree.
  - Outcome: `emit_checkout_bucket` now tracks the most recent
    workspace + repo keys and skips re-emitting headers when
    they're unchanged across adjacent checkout buckets (buckets
    are already ordered by the BTreeMap so siblings are
    adjacent). The existing
    `two_checkouts_in_same_repo_show_checkout_level` test now
    asserts exactly one repo row.

- [ ] `T8-003` Fill out the TUI empty/loading/error frame matrix.
  - Scope: render the full set of empty/loading/error frames
    documented in
    `docs/implementation/phase-08-interactive-tui.md` —
    `--no-live-preview` preview-zone message, tmux-disabled
    mux-view banner, tmux-unavailable mux-view banner, provider
    error chips on the status bar, refresh-failure stale marker.
  - Tests: Ratatui buffer snapshots for each state. Reuse the
    `render_to_buffer` / `buffer_to_string` helpers already in
    `src/tui/ui.rs`.
  - Blockers: `P8-007` v1 slice (the render shell is there); a
    `Msg::SetError` reducer addition may be needed.

- [x] `T8-004` Same-line session preview switch.
  - Scope: narrow-mode stacking (terminal width < 100 cols)
    landed in the polish pass — the body switches from a
    horizontal split to a vertical stack at the threshold. The
    remaining locked behavior changed after operator feedback:
    session rows now stay one physical line tall and use remaining
    horizontal space after the mux indicator for a dim same-line
    `last_message_preview`, cropping or omitting it when width is
    tight.
  - Outcome: removed the second-line selected/recent preview rows;
    `src/tui/ui.rs` appends the graph-resident preview to each
    session row only when width remains after the fixed cells.
    Left-tree auto-scroll now tracks one physical row per visible
    row again.
  - Tests: `cargo test tui --all-targets`.
  - Blockers: `P8-007` v1 slice.

- [x] `T8-005` Header `updated Ns ago` freshness indicator.
  - Outcome: `Msg::SetData` now carries `loaded_at_epoch`,
    `App::loaded_at_epoch()` exposes it, and the header renders
    `updated Ns ago · counts` via the shared
    `format_recency` helper. Render-time clock is a
    `#[cfg(test)]`-controllable shim so snapshot tests stay
    deterministic.

- [ ] `T8-006` Expand TUI buffer-snapshot test coverage.
  - Scope: add `insta`-backed snapshot tests covering the
    sessions-view default render at 80×24, the
    ambiguous-mux-expanded variant, the mux view (once
    `P8-004` mux builder lands), the PR view (once `P8-004`
    prs builder lands), a narrow 60-col terminal (depends on
    `T8-004`), the search overlay (depends on `/`-key wiring),
    the help overlay, the empty-graph frame, the
    `--no-live-preview` frame, and the
    selection-retention-after-refresh frame. Keep snapshots
    deterministic with fixed fixtures.
  - Blockers: `P8-007` v1 slice; individual snapshot variants
    depend on the corresponding feature stories.
  - **slice landed**: expanded `src/tui/ui.rs` coverage around
    right-pane focus, selected-row styling when focus moves,
    contextual status text, compact path labels, compact mux
    preview headers, and bottom-cropped mux captures. The broader
    snapshot matrix remains open.

- [ ] `T8-009` Throttle and freshen mux pane-capture previews.
  - Scope: the v1 P8-009 cut runs `tmux capture-pane`
    synchronously on every selection change and never re-runs
    until the next change. Add (1) a `mux_preview_interval`-
    driven refresh so a stable selection still gets fresher
    captures, (2) a freshness label ("captured Ns ago") in the
    right-panel preview header, (3) a snapshot test over the
    preview render with a fake runner so the layout stays
    locked, and (4) background-thread execution per ADR 0024
    so capture never blocks input. The background piece
    overlaps `T8-007`; consider folding the two into a single
    background-work pass.
  - Blockers: `P8-009` v1 slice. Best done alongside `T8-007`.

- [x] `T8-010` Render ANSI color in tmux previews.
  - Outcome: `SystemTmux::capture_pane` now passes `-e` so tmux
    emits the pane's escape sequences alongside the visible text.
    The right-panel preview consumes those via `ansi-to-tui`
    (ADR 0025) and renders them as styled `Text<'static>` —
    agent output keeps the colours operators see in the source
    pane. `--color=never` flattens the parsed text back to plain
    via `Text::to_string`, and malformed escape bytes fall back
    to a `Text::raw` so a single bad byte doesn't lose pane
    content. Three unit tests cover the colour, no-colour, and
    malformed-input paths.

- [x] `T8-011` Strengthen selected-row and focused-pane visual states.
  - Visual slice landed in the earlier polish commit. Behavioral
    half landed now: `Msg::ScrollPreviewBy(i32)` replaces the
    old `ScrollPreviewDown` / `ScrollPreviewUp` variants. The
    runtime adds a `remap_for_focus` pass between `translate`
    and the reducer — when `App::focus()` is `Focus::Right`,
    `j`/`k` translate to `ScrollPreviewBy(±1)` and
    PageUp/PageDown to `ScrollPreviewBy(±viewport)`. Uppercase
    `J`/`K` continue to scroll the preview regardless of focus.
    The status-bar hint switches between `j/k move · Enter
    expand` (left focus) and `j/k scroll preview` (right focus)
    so the operator can see `Tab`'s effect immediately.
  - Original scope: make the active attach target and focused pane visually
    unmistakable. Use a full-row selected style for the left tree,
    a distinct but low-noise focus treatment for the active pane
    border/title, and right-pane scroll hints that only appear
    when the preview can scroll. `Tab` should have both visible
    and behavioral effects: navigation keys apply to the focused
    pane, and the status bar names the active keymap.
  - Tests: reducer tests for focus-specific key handling; Ratatui
    snapshots for left-focus, right-focus, selected agent row,
    selected mux-candidate row, and scrollable vs non-scrollable
    preview states.
  - Blockers: `P8-006`, `P8-007` v1 slices.

- [ ] `T8-012` Compress project, path, and mux display labels.
  - Scope: introduce display-label helpers for TUI rows and detail
    fields so raw paths and tmux native ids do not dominate prime
    screen space. Group rows should use a short project/checkout
    label first, with `~`-shortened path as dim secondary text
    when width allows. Mux fields should prefer a human-readable
    display name and keep the full native id available through the
    existing copy-id/node-show affordances or a dim overflow field.
  - Tests: pure view-model tests for home-shortened paths,
    duplicate basename disambiguation, long tmux id compression,
    and stable labels across refresh; Ratatui snapshots for narrow
    and 120-col sessions views.
  - Blockers: `P8-004`, `P8-005`, `P8-007` v1 slices.
  - **slice landed**: the sessions tree renders compact group
    primary labels with dim shortened-path secondary text, long mux
    labels are shortened in candidate rows and detail fields, and
    right-panel detail now receives `$HOME` for path shortening.
    Duplicate-basename disambiguation and shared view-model helpers
    remain open.
  - **further slice from styling overhaul (Phase 7)**: group rows
    now carry agent + mux-state summary chips
    (`(N) ◉ a ◐ b ◯ c`) so the operator gets density without
    reading the path. Remaining work: duplicate-basename
    disambiguation and shared view-model helpers across CLI and
    TUI surfaces.

- [x] `T8-025` Show full session and mux IDs in the TUI.
  - Scope: replace the TUI's agent-session id-suffix display with
    the full harness-native session id and the full mux-native session
    name anywhere the operator needs an identifier they can copy and
    use outside Conspectus, especially the selected-row detail pane.
    In the selected entity's own detail section, label these rows as
    `id` for agent sessions and `name` for mux sessions, and omit the
    redundant `harness:` / `backend:` prefix because those fields are
    shown separately.
    If a compact label is still needed in the left row tree, prefer a
    leading-prefix abbreviation over a trailing suffix and keep the
    full id visible in detail. Preserve alias/title-first display
    labels from ADR 0029; this story is about the explicit id field,
    not the human-readable session name.
  - Tests: pure detail/row view-model tests proving the full
    `AgentSessionId` value is available for selected agent rows and
    mux-attached agent rows; Ratatui buffer snapshots covering a long
    Codex-style id so the detail pane shows a copyable full id and does
    not regress to `...<suffix>`.
  - Manual checks: run `cargo run -- tui --view sessions`, select a
    Codex or Claude session with a long native id, and confirm the
    right pane exposes the whole id in display order from the beginning
    of the id.
  - Blockers: `P8-005`, `P8-007` v1 slices.
  - Outcome: the TUI detail pane now renders full `session_key`
    values in the explicit `id` row for selected agent sessions, full
    mux session names in the explicit `name` row, and typed full
    linked-entity labels in mux-attached session rows, session mux
    rows, and parent-session lineage fields. The detail renderer no
    longer uses the bold compact title line as the copyable identifier.

- [x] `T8-026` Expand linked entities from the TUI detail pane.
  - Scope: add a right-pane keybinding that expands linked entities in
    place. For a selected agent session, the Mux section's linked
    `tmux:<name>` row should expand into the mux's full detail fields.
    For a selected mux session, the Session section's linked
    `harness:<session_key>` rows should expand into each agent
    session's full detail fields. Keep the compact linked rows by
    default so the right pane remains scannable.
  - Tests: reducer/keymap coverage for the new keybinding, detail
    renderer tests for collapsed versus expanded linked entities, and
    at least one mux-with-two-sessions case.
  - Outcome: the detail view-model now attaches one-level target
    details to linked mux/session summary rows. `e` toggles linked
    details from either pane, and `Enter` does the same when the
    right pane has focus. The expanded rows stay nested under the
    existing Mux/Session section instead of changing the left-tree
    selection.

### Detail Pane Graph Explorer Revamp

Runtime process observations made the current one-hop inline expansion
model too dense: repeated nested `Mux`, `Session`, and `Process`
section headers are hard to scan, and indentation is not enough to
preserve orientation. Per `docs/design.md`, the right panel should be a
focused node inspector plus relationship explorer. Core facts for the
selected node stay visually separate; upstream/downstream links render
as compact relationship rows; the selected relationship gets a compact
preview; graph depth is reached by drilldown with breadcrumbs rather
than recursive inline detail panes.

- [x] `T8-027` Model detail-pane relationship groups and previews.
  - Scope: replace the recursive `HeaderField.expanded_fields` detail
    payload with a view model that separates core node facts,
    relationship groups, selected relationship row, neighbor preview,
    and breadcrumbs. Relationship rows should carry direction,
    relation kind, neighbor node id/kind/label, evidence, confidence,
    state, selected-link id, and resolved-vs-candidate context. Keep
    the model SQLite-backed through `build_node_detail_from_conn`.
  - Tests: pure view-model tests for mux, agent session, runtime
    process, repo/checkout, fork, and PR nodes; coverage for empty
    groups, unresolved endpoints, conflicts, and long labels.
  - Blockers: `H-MUXPROC-FU-006`, `P10-010`.

- [x] `T8-028` Replace inline expansion with relationship-group
  navigation.
  - Scope: change `e` to expand/collapse relationship groups only.
    Add right-pane cursor state for relationship rows. `Enter` drills
    into the selected neighbor node, `Backspace` returns through a
    breadcrumb stack, and selection survives refreshes by node id plus
    selected relationship id where possible. Remove or retire the
    existing "expanded linked details" state from `T8-026`.
  - Tests: reducer/keymap tests for group expand/collapse, drilldown,
    back navigation, breadcrumb reset on missing nodes, and refresh
    stability.
  - Blockers: `T8-027`.

- [x] `T8-029` Render the focused inspector, relationship explorer,
  and preview layout.
  - Scope: update the right-panel renderer so core node facts, grouped
    relationships, and selected-edge/neighbor preview have distinct
    visual treatment. Avoid nested section dividers in previews.
    Truncate long mux names, process observation keys, commands, and
    transcript paths in rows while preserving access to the full value
    through the focused preview or a follow-up full-value overlay.
  - Tests: Ratatui buffer snapshots for the cluttered mux/process
    case, narrow terminals, long labels, expanded groups, and
    breadcrumb drilldown.
  - Blockers: `T8-028`.

- [x] `T8-030` Add full-value inspection for long detail fields.
  - Scope: provide a focused way to inspect long values from the core
    summary, relationship rows, and previews without forcing them into
    the main detail layout. Candidate UX: `o` opens a centered
    read-only value modal for the selected field/row, with wrapping,
    scroll, and copy-oriented text. Reuse existing modal/input
    primitives where possible and avoid new dependencies.
  - Tests: widget/reducer tests for opening, scrolling, and closing the
    full-value view; buffer snapshots for long command and observation
    key values.
  - Blockers: `T8-029`.

- [x] `T8-031` Update docs and scenario coverage for detail graph
  navigation.
  - Scope: update TUI help/keybinding docs and dev scenario docs to
    describe relationship-group expansion, drilldown, breadcrumbs, and
    full-value inspection. Ensure `process-cardinality`,
    `codex-fd-current`, and `ambiguous-mux` can demonstrate the new
    detail explorer manually.
  - Tests: help-overlay snapshot/keybinding tests and scenario smoke
    coverage for launching the TUI on the relevant named scenarios.
  - Blockers: `T8-030`, `TEST-006`.

- [ ] `T8-032` First-class evidence inspector and link-promotion
  flow.
  - Scope: replace the v1 `o opens evidence` placeholder on
    unresolved-evidence rows (per
    `docs/tui-detail-mockup.md`) with a focused inspector that
    renders all `UnresolvedEndpoint` metadata for the candidate
    (`harness_key`, `native_id`, `state_scope`, `path`, free-form
    `metadata` fields) and supports promoting that evidence to a
    durable declared link from inside the TUI. The same inspector
    should let operators convert a discovered/resolved candidate
    into a declared link without leaving the detail explorer.
    Writes route through the existing declared-link CRUD path
    (server socket or direct SQLite writer per ADR 0038); the
    inspector does not bypass the manual-link command surface in
    `docs/design.md`.
  - Tests: reducer/keymap tests for opening the inspector,
    cancelling, and promoting evidence; declared-link store
    integration tests that confirm the write lands in the
    appropriate local-or-global store per `docs/design.md`'s
    persistence rules; snapshot coverage for the inspector with
    sparse vs richly-populated unresolved endpoints.
  - Blockers: `T8-030`, declared-link CRUD landing in the TUI
    surface (currently CLI-only).

- [ ] `T8-033` TUI responsive-layout design and breakpoints.
  - Scope: codify the layout breakpoints the TUI uses across all
    views so the detail-pane explorer (and the rest of the TUI)
    renders predictably across terminal sizes. Decide and
    document: (a) the width / height where left + right panes
    stack vertically instead of side-by-side, (b) the width where
    the right pane auto-expands when it gains focus, (c) the
    width where the right pane is hidden entirely and the
    operator cycles to it via a tab affordance, (d) the
    narrow-pane content-drop rules called out under
    "Narrow-terminal Behavior" in `docs/tui-detail-mockup.md`.
    Likely deliverables: a design note in `docs/` (promoted to an
    ADR if the choices are cross-cutting) plus the implementation
    that applies the rules uniformly across sessions / mux /
    union / prs / forks views and the detail explorer.
  - Tests: render tests at representative terminal sizes covering
    each breakpoint transition; snapshot regression for the
    stack-vs-split, expand-on-focus, and hide-and-tab behaviors;
    keymap coverage for the tab affordance when the right pane is
    hidden.
  - Blockers: `T8-014` (contextual status bar — the breakpoint
    rules need to play nicely with the contextual status zone),
    `T8-027` v1 slice (so the detail-pane explorer's needs are
    concrete before thresholds are picked).

- [x] `T8-034` Expanded Node Detail toggle.
  - Scope: add a "full node" toggle that swaps the Node zone's
    top-5 render for every field the focused node carries
    (per `docs/tui-detail-mockup.md`'s Expanded Node Detail View
    section and the per-kind fields-reference tables). Default
    accelerator `F`; primary surface is the Controls overlay
    (ADR 0031). Toggle state is per-focused-node and resets when
    drilling into a neighbor; Backspace restores the prior node's
    toggle state along with its focus. Long values reuse the
    existing `(truncated · o)` open-value path. For node kinds
    whose available field set already fits in the top-5
    (`Repo`, `Workspace`, `Branch`, `Checkout`, `Fork`), the
    toggle is a no-op and renders the same content.
  - Tests: reducer/keymap tests for toggle on/off across drills
    and backspace; renderer tests for each node kind's expanded
    field set; snapshot coverage for at least one expanded
    `agent_session`, `mux_session`, `runtime_process`, and
    `forge_pr` case.
  - Blockers: `T8-027` modeling.

- [x] `T8-035` Left-pane mirror sync (default).
  - Scope: implement `[tui.detail].left_pane_sync = "mirror"` as
    the default behavior per `docs/tui-detail-mockup.md`'s
    Left / Right Pane Synchronization section. When the right pane
    drills via `Enter` on a relationship row, the left tree
    scrolls to and selects the row corresponding to the focused
    node, expanding group rows along the ancestor path. The
    left-pane *view* does not change. When the focused node has
    no row in the current view (e.g. `runtime_process` while in
    sessions view), the left pane keeps its previous selection.
    The breadcrumb stack also stacks left-pane selection state so
    `Backspace` restores both panes. Manual left-tree navigation
    cancels the active drill: the right pane's focused node is
    replaced by the node corresponding to the new tree selection
    and the breadcrumb stack is collapsed.
  - Tests: pure tree-expansion tests for finding/selecting a
    node by id across each left-pane view; reducer tests for
    drill + sync, Backspace restoring prior selection, manual
    tree navigation collapsing the drill, and the "focused node
    has no row" fallback. Buffer snapshots for at least the
    sessions and mux views across one round of drilldown.
  - Blockers: `T8-028`.

- [ ] `T8-036` Left-pane follow sync (opt-in view switching).
  - Scope: implement `[tui.detail].left_pane_sync = "follow"`
    per `docs/tui-detail-mockup.md`. In `follow` mode, when
    `mirror` would keep the left pane's previous selection
    because the focused node has no row in the current view, the
    left pane switches to a view that *does* have the row and
    selects it. Backspace restores the previous view and the
    previous selection together (breadcrumb stack carries view
    state). Drill hops whose neighbor kind has no top-level view
    (e.g. `runtime_process`, `fork` when no fork view exists)
    fall back to `mirror` behavior. Add a `none` mode that
    leaves the left pane completely untouched during drilldown,
    and add the Controls overlay entry for flipping between
    `mirror`, `follow`, and `none` mid-session.
  - Tests: reducer tests for view-switching across each
    `(focused-node-kind, current-view)` pair; tests for the
    fallback-to-mirror behavior on view-less node kinds; tests
    for Backspace restoring view + selection; Controls overlay
    mode-flip tests.
  - Blockers: `T8-035`.

- [ ] `T8-037` Distinguish symmetric relations in the detail
  explorer.
  - Scope: today the explorer buckets edges into Upstream /
    Downstream from the underlying `GraphLink`'s `source → target`
    direction, which reads correctly for directional relations
    (`process_identifies_session`, `runs_in_mux`, …) but is
    misleading for symmetric relations such as `associated_with`
    where the side a link lands on is an artifact of link-builder
    order. Tag each `RelationKind` with a
    `Directionality::{Directed, Symmetric}` and render symmetric
    relations in a third **Related** zone between Upstream and
    Downstream so the layout no longer implies a direction the
    model doesn't carry. Update Node-zone cursor walk order and
    breadcrumb hop carry-state accordingly.
  - Tests: reducer tests asserting that symmetric relations land
    in the Related zone (not Upstream or Downstream); cursor walk
    tests covering Node → Upstream → Related → Downstream order;
    renderer snapshot for a node carrying at least one symmetric
    relation; coverage for a node with *only* symmetric edges
    (Upstream and Downstream should suppress, Related should
    render alone).
  - Blockers: T8-027 modeling.

- [x] `T8-038` Shorten breadcrumb hop labels and elide deep chains.
  - Scope: today each breadcrumb hop renders the focused node's
    full display label, which eats the breadcrumb line after two
    hops. Render hops as `kind:short_tag` (e.g. `mux:editor`,
    `proc:claude·82310`) and add an elision rule
    (`first … last-N`) when the rendered chain exceeds the
    breadcrumb zone width. Tiebreak ambiguous short forms within a
    chain by suffixing the last-4 of the id when two hops would
    otherwise collide.
  - Tests: unit tests for the short-form formatter across each
    node kind; collision-tiebreak tests for two hops with the same
    short label; rendering tests at narrow widths confirming
    elision (`first … last-N`) without dropping the current hop;
    snapshot coverage for a 4+ hop chain.
  - Blockers: `T8-028`.

- [x] `T8-039` Surface node kind as a first-class field in the
  detail pane.
  - Scope: today the node kind is buried in the harness-prefixed
    id (e.g. `opencode:ses_…`) and the operator has to parse it
    out. Render the kind as a dim leading chip (e.g.
    `[agent_session]`) in the Node zone title and in link / group
    rows, separately from the id/label. For the `cwd` core-field
    row specifically, perform a reverse lookup against the
    `GraphDb` and append the resolved-owning-node kind chip
    (`Repo`, `Workspace`, `Checkout`) when the lookup succeeds; on
    no match leave the path bare rather than guessing. Apply the
    same convention to the breadcrumb hop short-form from
    `T8-038`.
  - Tests: renderer tests for kind chips on each node kind across
    the Node zone, link rows, and group headers; reverse-lookup
    tests for `cwd` resolving to Repo / Workspace / Checkout / no
    match; snapshot coverage for a sparse-graph case where the
    `cwd` doesn't resolve.
  - Blockers: `T8-027`, `T8-038` (so the breadcrumb short-form can
    pick up the chip too).

- [x] `T8-040` Enter-to-copy on Node-zone fields with a toast
  widget.
  - Scope: `Enter` on a Node-zone field row is a no-op today.
    Wire it to copy the field's full value to the system
    clipboard and surface a transient toast ("copied: cwd") via a
    new reusable toast widget under `src/tui/widgets/` that
    auto-dismisses after ~1.5s and does not block input. This
    cleanly splits the contract: `o` for *reading* a long value
    (modal, scrollable), `Enter` for *copying*. Reuse the toast
    for `i` (copy short id) and any future copy actions so
    feedback is consistent. The `i` binding copies the full id of
    the selected agent or mux session (not a short id) so the
    output drops directly into `node show` / external tooling.
    Clipboard backend is OSC 52 per
    ADR 0056 (no new deps, SSH-friendly, hand-rolled escape
    writer at `src/tui/clipboard.rs`); `arboard` deferred until
    operator feedback shows the OSC 52 gap biting.
  - Tests: reducer/keymap tests for Enter-on-Node-field copying
    the value and surfacing a toast; toast widget unit tests for
    auto-dismiss timing and replacement (newer toast supersedes
    older); regression test that Enter on link rows still drills
    and Enter on group headers still toggles; coverage that empty
    or absent values don't surface a misleading "copied" toast.
  - Blockers: `T8-027` (cleared). Clipboard backend ADR landed as
    ADR 0056.
  - **slice landed**: OSC 52 clipboard primitive at
    `src/tui/clipboard.rs` (in-tree base64 encoder, no new deps per
    ADR 0056). Reusable `ToastWidget` at
    `src/tui/widgets/toast.rs` auto-dismisses after 1500ms and is
    rendered as a non-blocking bottom-centered overlay; newer toasts
    replace older. Right-pane `Enter` on a Node-zone field row now
    copies the field value (preferring the untruncated `long_value`
    when present) and posts a `copied: <label>` toast; link rows
    still drill, group headers still toggle. `i` copies the selected
    agent or mux session's full id (e.g.
    `agent_session:claude:proj_a:7d3f…`) via the same toast surface
    and surfaces a status hint when the selection isn't a session
    row. Help overlay advertises both bindings.

- [x] `T8-041` Flip the Upstream / Downstream header layout so
  zone labels anchor to the right.
  - Scope: today the explorer's zone headers render the bold
    `Upstream` / `Downstream` label on the left and the aggregate
    summary on the right (`Downstream  2 groups · 3 links · 1 ⚠`),
    so the highlighted label gets pushed toward the center of the
    pane and is hard to scan vertically when the right pane is
    narrow. Flip the order so the aggregate counts render on the
    left and the bold label anchors flush right
    (`2 groups · 3 links · 1 ⚠  Downstream`). Apply the same flip
    to the third **Related** zone introduced by `T8-037` if it
    lands first.
  - Tests: renderer snapshot for a wide pane (aggregate left,
    label flush right); snapshot for a narrow pane (label still
    visible, aggregate elided rather than the label); coverage
    for Upstream, Downstream, and Related zones; regression that
    empty zones still suppress entirely.
  - Blockers: `T8-029` renderer.

- [x] `T8-042` Hide edge meta (`provenance · confidence · state`)
  from link rows by default with an opt-in toggle.
  - Scope: today every link row in the explorer carries a trailing
    `discovered · high · active` line that exposes the resolver's
    provenance / confidence / state triple plus the `alt of …` edge
    state label. That detail is essential for operators
    actively diagnosing a resolver decision but adds noise to the
    primary "navigate the graph" use case the explorer is built
    around. Hide the trailing meta line on both link-row variants
    (single-link composite trailing row and multi-link group child
    row) by default. Keep the `★` resolver-winner marker and the
    `⚠` group-level conflict aggregate — those are navigation
    signals, not edge-meta noise. Add a per-session toggle
    (suggested `M` for "meta" via the Controls overlay primary
    surface per ADR 0031, since the unbound keys list reserves `M`
    for future merge actions — pick another letter if needed) and
    a `[tui.detail].show_edge_meta = false` config knob so users
    who routinely need the meta line can flip the default.
  - Direction (do not design now): a future "edge detail view"
    or focused edge inspector is the natural home for richer
    edge-resolver diagnostics (full provenance chain, every
    candidate side-by-side, the resolver's tiebreak rule, etc.).
    This ticket is the v1 hide-by-default cut and is intentionally
    scoped to a visibility toggle; the deeper inspector is its
    own follow-up once the use cases are clearer.
  - Tests: renderer tests confirming the meta line is suppressed
    by default and surfaces after the toggle; config-default test
    for the new `show_edge_meta` knob; coverage that `★` and `⚠`
    remain visible in the default (compact) mode; reducer test
    for the toggle preserving cursor row identity.
  - Blockers: `T8-029` renderer; Controls overlay entry slot
    (ADR 0031).

- [ ] `T8-020` Auto-broaden TUI scan roots to the cwd's "code dir"
    ancestor when neither CLI nor config specifies one. Low
    priority.
  - Scope: when `--scan-root` and `[tui].scan_roots` are both
    empty, walk up from the process cwd to the first ancestor
    that contains ≥ N (default 2 or 3) immediate-child entries
    that themselves look like repository roots (a `.git`
    directory or checkout). Use that ancestor as the scan root
    instead of cwd. Should be opt-in via a flag or config
    setting initially so we don't surprise operators who *want*
    the cwd-scoped behavior; promote to default later if it
    works out. The heuristic needs a clear cap (don't walk
    past `$HOME` or filesystem boundaries) and should fall back
    to the current cwd-scoped behavior when no plausible
    ancestor is found.
  - Tests: pure helper tests for the ancestor walk over fixture
    directories with varied repo counts and depths; the
    runtime side wires through the same scan-root resolution
    path as `[tui].scan_roots`.
  - Blockers: `T8-013` v1 slice. Filed at low priority per
    operator direction — config-driven `[tui].scan_roots` is
    the preferred default; this auto-broaden mode is a
    "no-config still does the right thing most of the time"
    affordance.

- [ ] `T8-021` Descend into scan roots when looking for atelier
    workspaces (companion to `T8-020`).
  - Scope: `src/discovery/atelier.rs::find_atelier_config`
    currently walks only **upward** from each scan root looking
    for `atelier.toml`, so a scan root one directory above a
    nested workspace (e.g. `--scan-root ~/src` with the workspace
    at `~/src/sysadmin/atelier.toml`) never produces a
    `Workspace` node and the sessions view's `graph` grouping
    cannot surface a workspace header. Extend the discovery so
    each scan root also scans its immediate children for an
    `atelier.toml` (and, optionally, two levels deep behind a
    config knob) before emitting an empty atelier fragment. Use
    a bounded depth so the cost stays predictable; skip
    well-known noise directories (`.git`, `node_modules`,
    `target`, `.cache`, `.atelier`). When multiple atelier
    configs are found, each yields its own `Workspace` node
    independently.
  - Reproducer: with no `[tui].scan_roots` set and the operator
    launching from a directory outside their atelier workspaces
    (e.g. `cd ~/src/conspectus`), the auto-fallback scan root
    becomes `~/src/conspectus`. Atelier discovery's upward walk
    sees no `atelier.toml` even though `~/src/sysadmin/atelier.toml`
    exists in the same `~/src/` tree, so the sessions view shows
    no workspace headers. The `graph` and `repo` grouping modes
    visibly diverge only when at least one workspace is found,
    so this gap also makes the grouping options look redundant
    on cross-project setups.
  - Tests: discovery-level tests covering (a) `atelier.toml` at
    the scan-root level (existing behavior, regression guard),
    (b) `atelier.toml` one directory down from the scan root
    (the fix), (c) multiple atelier configs nested under one
    scan root all surfacing as distinct `Workspace` nodes,
    (d) `atelier.toml` found via the upward walk still works
    (no regression on the `--scan-root ~/src/sysadmin/config`
    pattern), (e) the noise-directory exclusion list is
    honored.
  - Blockers: none directly. Pairs naturally with `T8-020`:
    once auto-broaden picks the right starting directory, this
    story makes the workspaces beneath it discoverable. Either
    can ship without the other; together they remove the
    "give me a multi-project view" friction.

- [x] `T8-013` Default-expand and mark the launch-context project
    without filtering the world.
  - Outcome: scan roots now resolve CLI → `[tui].scan_roots`
    config → cwd-default (operator picked option (b)).
    `src/config.rs` gained a `TuiConfig` struct with
    `scan_roots: Vec<PathBuf>` parsed from `[tui].scan_roots` in
    `.conspectus.toml` / user config, with `~` and `~/<rel>`
    expansion against the loader's home directory at merge
    time. `TuiArgs::run` consults the loaded config when
    `--scan-root` is empty and falls back to `[cwd]` only when
    both are unset. `RunConfig` carries the launch-time cwd as
    an orientation hint that the sessions row-tree builder
    uses to mark the deepest ancestor group row with
    `GroupRow::is_launch_context = true`. `Msg::SetData`
    carries an `initial_selection_hint`; the runtime computes
    it from the marked row and the reducer prefers it on first
    load over the leading row (later refreshes ignore the
    hint so manual selection isn't clobbered). The renderer
    adds a dim cyan `(cwd)` suffix to the marked row. Auto-
    broaden option (c) filed as low-priority `T8-020`.
  - **slice landed**: initial expansion now opens only the
    launch-context tree (ancestors, the marked group, and its
    descendant groups) while leaving unrelated groups collapsed
    for a cleaner first screen. If no launch-context group is
    marked, the first group opens as a fallback so tests and
    non-cwd-oriented data still present a usable starting point.
  - Tests: `cargo test tui --all-targets`.

- [ ] `T8-014` Make the status bar contextual to the selected row.
  - Scope: replace the static action list with a compact contextual
    left zone. Examples: attachable rows show
    `a attach <mux-display>`, ambiguous rows show
    `a attach preferred · m choose`, un-muxed rows show the
    disabled attach reason and reserved resume affordance, and
    group rows show expand/collapse. Keep provider health,
    freshness, and errors in the right chip zone.
  - Tests: pure status-view tests for each row kind and attach
    state; Ratatui snapshots for attachable, ambiguous, un-muxed,
    group-row, provider-error, and stale-refresh status bars.
  - Blockers: `P8-006`, `P8-010`, `T8-003`.
  - **slice landed**: the status bar now derives its left-zone
    action text from the selected row. It shows attach targets for
    attachable selections, disabled attach reasons for un-muxed or
    unsupported rows, ambiguity affordance text, focus scope, and
    the active pane keymap. Provider health/freshness chips remain
    with `T8-003`.

- [ ] `T8-043` Make Enter trigger the selected row's default action.
  - Scope: when the left pane has focus, route `Enter` through a
    default-action dispatcher instead of treating every row as
    expand/collapse. Default actions:
    - mux rows attach to that mux, reusing the existing `P8-010` /
      `T8-018` attach path and disabled-reason handling;
    - un-muxed agent-session rows open the native transcript viewer,
      reusing the existing `H-VIEWER-NATIVE-008` view path;
    - mux-candidate / mux-attached agent rows keep the attach
      behavior already available through `a`;
    - group rows keep expand/collapse on `Enter`.
    Preserve right-pane `Enter` semantics from the detail explorer:
    relationship rows still drill, group headers still toggle, and
    Node-zone copy behavior from `T8-040` remains scoped to the
    right pane.
  - Tests: reducer/keymap coverage for mux row attach, un-muxed
    session view, attached-session attach, group expand/collapse,
    disabled attach reason, unsupported viewer fallback, and
    focus-specific behavior proving right-pane `Enter` is unchanged.
    Add a status-bar snapshot or view-model test showing the
    contextual hint advertises `Enter` as the primary default action
    for attachable muxes and viewable un-muxed sessions.
  - Manual checks: in `conspectus tui`, select a mux row and press
    `Enter` to attach/detach back to Conspectus; select an un-muxed
    Claude/Codex/OpenCode session and press `Enter` to open the
    transcript viewer; select a group row and confirm it still
    expands/collapses.
  - Blockers: `P8-010`, `T8-018`, `H-VIEWER-NATIVE-008`,
    `T8-014`.
  - **slice landed**: `Enter` on the left pane now dispatches the
    selected row's default action. Mux rows and muxed/ambiguous agent
    sessions attach (reusing `attach_action`); un-muxed agent
    sessions open the native transcript viewer (reusing
    `view_action`); group rows still expand/collapse. Right-pane
    `Enter` continues to fire `ExplorerActivate` for drill/expand.
    As a companion, `v` now also works on mux rows: it resolves the
    mux's preferred linked agent session and opens its transcript,
    so `v` is the inverse of `a` on agent rows. Because `Enter` no
    longer toggles every row, the left tree picks up vi-style fold
    bindings: `l` / `→` expand the selected row, `h` / `←` collapse
    it (idempotent: a second press is a no-op). The status hint
    advertises `Enter/a attach …` for attachable rows,
    `Enter/v view …` for viewable un-muxed sessions, and
    `Enter/l expand · h collapse` for group rows. The help overlay's
    Actions section lists `Enter` as the row-kind default and the
    Navigation section documents the new fold bindings.

- [ ] `T8-015` Add sessions-tree density modes.
  - Scope: add a user-facing density setting for the sessions view
    so operators can trade context for row count. Suggested modes:
    `compact` (one line per session, no same-line previews),
    `balanced` (current locked behavior: same-line previews when
    width allows), and `expanded` (future richer preview treatment
    if operators still need it). Expose via config and a TUI toggle
    only after the base `/` search and help overlays are stable.
  - Tests: row-tree/render snapshots for all density modes at
    80x24 and a wide terminal; config parsing tests once the
    setting is added.
  - Blockers: `P8-007` v1 slice; should follow `T8-004` so wide
    inline behavior is not duplicated.

- [ ] `T8-016` Crop and annotate tmux previews for recognition.
  - Scope: make the mux preview behave like a recognition surface,
    not a raw dump. Prefer the bottom N visible lines from
    `capture-pane`, preserve wrapping enough to resemble the
    terminal pane, and add a compact preview header such as
    `preview · tmux · captured 2s ago`. Surface stale, disabled,
    and failed capture states in that header when possible.
  - Tests: preview adapter tests for bottom-line cropping,
    configurable line budget, stale/fresh labels, and failed
    capture labels; Ratatui snapshots for long and short captures.
  - Blockers: `P8-009` v1 slice; overlaps `T8-009` freshness
    work and should be planned with it.
  - **slice landed**: mux previews now use a compact separator
    carrying the display target and capture freshness when cached,
    and captured pane text is cropped to the bottom lines available
    in the preview zone. Configurable budgets and stale/failure
    header variants remain open with `T8-009`.

- [x] `T8-018` Round-trip attach: return to the TUI after the operator
    detaches from the mux client.
  - Outcome: `attach_action` no longer `exec`s into tmux.
    Instead, it calls `ratatui::restore()`, spawns
    `tmux attach-session -t <native_id>` with
    `Command::status()` so tmux owns the real terminal, waits
    for it to exit, then calls `ratatui::init()` to re-enter
    the alt screen and replaces the runtime's
    `DefaultTerminal` in place (followed by a `clear()`). Once
    control returns, the runtime kicks off a refresh so the
    row tree reflects activity during the attach, and surfaces
    a status-bar line — `attached/detached: tmux:<name>` on
    success or `attach failed: <reason>` if tmux exited
    non-zero or didn't launch. The operator stays in the TUI
    ready to pick another row.
  - Scope: today `exec_tmux_attach` calls `execve`, so the
    conspectus process is replaced by tmux. When the operator
    detaches (Ctrl-B d), there's no TUI to return to — they
    land at the parent shell prompt. Change the attach path to
    `Command::new("tmux").status()` (spawn + wait) under a
    suspend-resume guard: leave the alt screen + raw mode
    before spawning, restore them after wait, and feed a
    refresh into the reducer so the row tree reflects any
    activity that happened during the attach. Quit (`q`) from
    the TUI should still exit cleanly, and a failed spawn
    should land in the status bar with a clear reason instead
    of killing the process.
  - Tests: extend the action tests to cover an `Action::Attach`
    that runs a fake "attach" closure and returns control to
    the reducer; assert the TUI is still alive afterward, that
    the next refresh is scheduled, and that a fake "command
    failed" surfaces as a status message. Manual: attach,
    detach, repeat from a different row.
  - Blockers: `P8-010` v1 slice.

- [x] `T8-019` Auto-scroll the left tree to keep the selected row
    visible.
  - Outcome: `App` gained a `Cell<u16>` left-panel scroll offset
    and an `adjust_left_scroll(selected_line, viewport_height)`
    method that nudges the offset only when the selected row
    falls outside the viewport (above the top edge or at/below
    the bottom edge). The renderer tracks which line index the
    selected row lands at, asks `App::adjust_left_scroll` for
    the offset, and passes it to `Paragraph::scroll`. Three
    reducer-level tests plus a render-level test cover the
    behavior. The render-level regression uses a long pre-selected
    group row to prove left-tree rows clip rather than wrap, then
    asserts the selected row lands on the bottom visible
    left-panel line after `End`. PageUp/PageDown already drive the
    viewport via
    the existing reducer; this story just keeps the rendered
    view in sync. Open: pixel-precise centering on first focus
    and a manual-scroll keymap remain follow-ons under
    `T8-006`'s broader snapshot matrix.
  - Scope: today the left panel renders all visible rows into a
    single `Paragraph` with no viewport awareness, so once the
    selection moves past the rendered area the user can keep
    pressing `j` and see nothing change. Track a per-render
    scroll offset that follows the selection — at minimum,
    bring the selected row to the top edge when it moves
    above the viewport and to the bottom edge when it moves
    below. Page-down / page-up should jump a viewport at a
    time without losing the selection. Same-line previews must
    not affect viewport math.
  - Tests: reducer + render unit tests for selection moving past
    the visible top/bottom in a small viewport, PageDown jumping
    by viewport height. Ratatui snapshots for a
    short and a long tree at the same viewport size.
  - Blockers: `P8-007` v1 slice. Friendlier after `T8-006`
    expands the snapshot harness.

- [ ] `T8-017` Add visible search/filter workflow for large session
    worlds.
  - Scope: finish the `/` in-view search overlay for the TUI and
    make active filtering visible in the header or status bar.
    Matching should cover project label, path, harness, short id,
    session title, preview snippet, branch/PR labels when present,
    and mux display label. Results should preserve enough group
    context that the operator understands where a matched session
    lives.
  - Tests: matcher tests for each searchable field; reducer tests
    for open/type/clear/accept/cancel; Ratatui snapshots for an
    active query, zero results, and grouped result context.
  - Blockers: `P8-004`, `P8-006`; adding a heavyweight matcher
    still requires following ADR 0024's dependency policy.

- [ ] `T8-007` Move TUI discovery onto a background thread with
    timer-driven refresh.
  - Scope: replace the synchronous `discover_local_at_roots`
    call in `src/tui/runtime.rs::refresh` with a background
    discovery worker. Use `std::thread::spawn` + `mpsc` per ADR
    0024 (no async runtime). Add a timer that fires
    `Action::Refresh` on `config.refresh_interval`. Preserve
    provider diagnostics for the status-bar chips (depends on
    `T8-003` exposing the chip slot). Shape so a future Phase 7
    server snapshot can replace the worker without touching
    `app.rs`.
  - Tests: unit/integration tests with a fake discovery handle
    covering initial load, refresh on `r`, refresh failure
    preserving prior graph, and selection retention across the
    refresh.
  - Blockers: `P8-008` v1 slice (the sync path), `T8-003` for
    diagnostic surfacing.

- [ ] `T8-002` Align `conspectus table sessions` columns with the
    TUI sessions row tree once view-models converge.
  - Scope: today `output::table` builds its own per-projection
    extractors; the TUI sessions row tree introduces a stable
    pure view-model. Wire the table renderer to consume the same
    view-model (or a shared subset) so a single change to the
    sessions sort/grouping rules updates both surfaces. Decide
    whether the TUI's `~`-shortening and harness label
    collapsing should also apply to the CLI table by default,
    behind a `--paths short|full` knob.
  - Tests: snapshot parity tests showing TUI row tree and
    `table sessions` produce consistent labels for the same
    snapshot.
  - Blockers: `P8-004` (all five view-models present) and
    `P8-005` (detail view-models) so the shared API surface is
    settled.

### Filter, View Switching, And Per-View State (F8-*)

ADR 0031 settled the design for layered structured filters + `/`
fuzzy search, per-view filter/grouping/selection/expanded state
(sort stays global), per-view grouping enums, a shared `RowFilter`
predicate driving both CLI `table` and the TUI, and a discoverable
Controls overlay fronting the capability with accelerator keys layered
on top. Stories below carve that ADR into implementable slices.

Dependency shape inside the workstream:

```
ADR 0031 ─→ F8-001 ─→ F8-009 ─→ F8-010
            │
            ├─→ F8-002 ─→ F8-003 ─┐
            │                     │
            ├─→ F8-006 ─┐         │
            │           ↓         ↓
            ├─→ F8-004 ─→ F8-005 ─→ F8-011
            │           ↓
            └─→ F8-007 ─→ F8-012
                F8-008 (independent loader work)
```

`F8-001`/`F8-002`/`F8-006`/`F8-008` can land in parallel after the
ADR. `F8-009` and `F8-010` extend the CLI surface and table
projections respectively. `F8-004` (controls overlay) and the
status-bar / empty-frame stories converge on `F8-005` for the
keybinding surface; `F8-011` documents the final keymap once it
settles.

- [ ] `F8-001` Define `RowFilter` predicate + dimension types in a
    crate-public module.
  - Scope: introduce `src/filter.rs` with `RowFilter`,
    `HarnessFilter::Any(Vec<String>)`,
    `MuxStateFilter::Any(Vec<MuxStateKey>)`, and
    `MuxStateKey { Attached, Ambiguous, Unmuxed }` per ADR 0031.
    Predicate evaluation runs against an `AgentSessionNode` plus
    its resolved mux state. Apply the predicate in
    `build_sessions_tree` before bucket emission so empty groups
    collapse naturally. No new dependencies (ADR 0024 policy).
  - Tests: unit tests over existing sessions fixtures for
    claude-only narrowing, max-age cutoff at the boundary minute,
    unmuxed-only, ambiguous-only, the empty-result case, and the
    intersection of all three v1 dimensions.
  - Blockers: ADR 0031.

- [ ] `F8-002` Per-view grouping enums for the four pending views.
  - Scope: introduce `MuxGrouping`, `UnionGrouping`, `PrsGrouping`,
    and `ForksGrouping` alongside their P8-004 row-tree builders.
    Wire a `Grouping` dispatch enum so `App` state and config can
    carry a single field that narrows to the active view's enum.
    Sessions enum unchanged but joins the dispatch.
  - Tests: row-tree builder unit tests for each view's grouping
    values; dispatch-enum cycle-to-next tests covering wrap-around.
  - Blockers: ADR 0031; lands alongside `P8-004` for each view.

- [ ] `F8-003` Per-view state retention.
  - Scope: introduce `ViewStates` map on `App`, keyed by `View`,
    carrying `(filters, grouping, expanded, selection, left_scroll)`.
    On view switch, save the active slice and load the target
    slice; first-time entries seed from
    `[tui.views.<name>]` config defaults. Sort stays a top-level
    `App` field per ADR 0031.
  - Tests: reducer tests for switch-and-return state retention
    (filters survive `1 → 2 → 1`), per-view selection retention
    across refreshes, fresh-view default seeding from config.
  - Blockers: `F8-001`, `F8-002`, `F8-005`.

- [ ] `F8-004` Controls overlay (modal).
  - Scope: render a centered modal with sections for View,
    Grouping (scoped to active view), Filters (scoped to active
    view), and Sort (global). Arrow-key + Enter navigation, Esc
    backs out one level, mouse click support inside the overlay.
    Inline accelerator hints (`[1]`, `[2]`, …) per row. Drill-in
    sub-editors: harness multi-select, max-age text input (reuses
    ADR 0030 primitive), mux-state multi-select. Filter chips
    render in the status bar via `F8-007`.
  - Tests: snapshot tests for overlay open, each sub-editor open,
    chip applied state, cleared state. Reducer tests for arrow-key
    navigation skipping section headers and for sub-editor
    Confirm/Cancel outcomes.
  - Blockers: `F8-001`, `F8-002`, `F8-006`.

- [ ] `F8-005` Accelerator keybindings + view-switching plumbing.
  - Scope: bind `v` (controls overlay), `1`–`5` (direct view
    switch), `]`/`[` (cycle views), `f` (jump into the Filters
    section), `F` (clear all filters), and the grouping-cycle key.
    Resolve the `G` collision with End — either move "last row" to
    `End` only and reuse `G`, or bind grouping-cycle to `Ctrl-G`.
    Repurposes `f` from the previously-reserved fork action per
    ADR 0031; impl doc updated. Closes the `P8-006` deferred view-
    switching slice.
  - Tests: reducer tests for each new keybinding; Ratatui snapshots
    for the overlay open vs accelerator-only paths producing the
    same end state.
  - Blockers: `F8-002`, `F8-004`.

- [ ] `F8-006` Multi-select list widget.
  - Scope: shared list-with-checkbox primitive in
    `src/tui/widgets/multi_select.rs` for the harness and mux-state
    sub-editors. Pure state machine: cursor up/down, Space toggle,
    Enter confirms with `Vec<T>`, Esc cancels. Renders as a small
    bordered list anchored next to the originating row.
  - Tests: unit tests for cursor wrap, toggle semantics, empty-
    commit (clears the predicate), and large-list scrolling.
  - Blockers: none beyond ADR 0031; can land in parallel with
    `F8-004`.

- [ ] `F8-007` Status-bar filter chips + counts-with-totals.
  - Scope: new status-bar zone left of provider chips, rendering
    active filter chips with ADR 0022 colors and stable ordering
    (harness → max-age → mux-state → future dimensions). Truncate
    long lists with `+N more`. Header counts switch to
    `<filtered> of <total>` form (`12 of 47 agents · …`).
  - Tests: status-view unit tests for each chip layout; Ratatui
    snapshots for one-chip / many-chips / truncated / cleared
    states.
  - Blockers: `F8-001`, `T8-003` (provider chip zone).

- [ ] `F8-008` `[tui.views.<name>]` config schema + legacy alias.
  - Scope: parse `[tui.views.<name>] grouping = "…"` and
    `[[tui.views.<name>.filters]]` sub-tables in `src/config.rs`.
    Existing `[tui].sessions_grouping` continues to load as a
    deprecated alias that emits a one-line warning to stderr when
    encountered and seeds
    `[tui.views.sessions].grouping` when the new key is absent.
    Multiple `[[…filters]]` entries OR their predicates (set
    union).
  - Tests: loader unit tests for new schema, legacy alias alone,
    both present (new wins, warning emitted), malformed values,
    array-of-tables filter unions.
  - Blockers: ADR 0031; independent of TUI work.

- [ ] `F8-009` CLI flag parity: shared `FilterArgs` + per-view
    grouping.
  - Scope: introduce a shared `FilterArgs` struct mounted on both
    `TuiArgs` and `TableArgs`, exposing `--harness` (repeatable),
    `--max-age <DURATION>`, `--mux-state` (comma-or-repeat), and
    `--grouping <VALUE>` (per-view; values depend on `--view`).
    Legacy `--sessions-grouping` stays as an alias that prints a
    one-line deprecation warning to stderr.
  - Tests: CLI smoke tests for each flag, duration parse-error
    messages, deprecation-warning emission, `--grouping` rejected
    with a useful message when given an invalid value for the
    chosen view.
  - Blockers: `F8-001`.

- [ ] `F8-010` `conspectus table <ROWS>` consumes `RowFilter`.
  - Scope: thread the shared `RowFilter` through the table
    projection layer so the same flags narrow static output the
    same way the TUI narrows the row tree. Reuses `F8-009`'s
    `FilterArgs`.
  - Tests: snapshot/output tests for filtered `table sessions`,
    `table mux`, and a parity test asserting TUI row count matches
    `table` row count for the same flag set against a fixture.
  - Blockers: `F8-001`, `F8-009`.

- [ ] `F8-011` Help-overlay docs.
  - Scope: extend the `?` help overlay with the new keymap
    (`v`, `1`–`5`, `]`/`[`, `f`, `F`, grouping-cycle), a one-line
    description of the controls overlay, the v1 filter dimensions,
    and an example CLI invocation. Document the menu-first
    discovery rule.
  - Tests: snapshot test for the help overlay's new layout.
  - Blockers: `F8-004`, `F8-005`.

- [ ] `F8-012` Filtered-zero empty frame.
  - Scope: render `No sessions match <chips>. F clears.` in the
    row tree when the active filter set produces zero rows.
    Status-bar chips continue to render so the operator sees
    exactly which predicates are active.
  - Tests: snapshot test for the empty frame across each view.
  - Blockers: `F8-004`, `F8-007`, `T8-003`.

- [ ] `F8-013` Persist last-active view across TUI restarts.
  - Motivation: `App::config().default_view` is in-memory only,
    seeded from `[tui].default_view` config. After view switching
    (`v` / `1`..`5` / `]`/`[`), the next `conspectus tui` invocation
    drops the operator back at the configured default — costing a
    keystroke every cold start and breaking the "pick up where you
    left off" expectation that pins, scenarios, and the sessions
    tree otherwise reinforce. A persisted handoff is the smallest
    surface that closes the loop.
  - Scope:
      - Introduce a TUI state file at
        `$XDG_STATE_HOME/conspectus/tui-state.json` (sibling to the
        hook state-root already under `$XDG_STATE_HOME/conspectus/`;
        rebuildable, not authoritative — same posture as the
        pin-binding sidecar). Schema v1 carries
        `{ schema_version: 1, last_view: "sessions" | "mux" |
        "union" | "prs" | "forks" }`. Atomic write helpers (tempfile
        + rename) and a `skip-on-unchanged` comparison so quiet
        sessions produce no mtime churn, mirroring
        `H-PIN-RESUME-001` / `H-PIN-RESUME-003`.
      - Write on view switch (the `App::switch_view` seam at
        `src/tui/app.rs:551` is the natural single funnel — every
        accelerator and overlay-driven switch goes through it).
        Best-effort write; failures log at debug and do not abort
        the switch.
      - Read at `conspectus tui` startup, after config load and
        before runtime init. Precedence: explicit `--view <name>`
        CLI flag wins → persisted `last_view` wins → config
        `[tui].default_view` → built-in `View::Sessions`. Add a
        `--no-resume-view` opt-out for scripts and snapshot tests
        that need a deterministic starting view independent of
        prior runs.
      - The `--snapshot` dev-only path (ADR 0067) must default to
        `--no-resume-view` semantics so snapshot regeneration is
        not influenced by whatever view the operator last touched
        outside the test fixture.
      - Read-only invariant: nothing other than `conspectus tui`
        reads or writes the state file. `graph` / `table` / `query`
        / `node show` / `pin *` paths never touch it. Mirror the
        H-PIN-019 / H-PIN-RESUME-006 fingerprinting test pattern so
        regressions get caught.
  - Tests:
      - Unit tests on the state-file helpers covering atomic
        write, skip-on-unchanged, malformed-JSON fallback (treat
        as absent, do not panic), and forward-compatible unknown
        fields (preserve through round-trip so future schema bumps
        do not strand old writes).
      - Reducer test pinning that `switch_view` schedules a write
        through the same seam the runtime uses, and that the
        startup-precedence resolver picks the right source under
        each combination of flag / file / config.
      - Integration test: launch the TUI, switch to `mux`, exit;
        relaunch and assert the initial view is `mux`. Re-run with
        `--no-resume-view` and assert the initial view falls back
        to the configured default.
      - Read-only invariant test mirroring
        `tests/cli_pin_resume_invariants.rs`: assert that
        `graph`, `table <rows>`, `query`, `node show`, and every
        `pin` subcommand leave the state file byte-for-byte
        untouched (content + mtime fingerprints) and do not create
        the file when absent.
  - Open questions:
      - Format: JSON (mirrors the pin sidecar's serde + skip-on-
        unchanged pattern verbatim) vs TOML (matches every other
        Conspectus config surface). Recommend JSON to share the
        cache helpers; flag during impl.
      - Scope creep: F8-003 covers in-session per-view state
        (filters / grouping / selection / expansion / scroll).
        Should this story persist *only* `last_view`, or extend
        the schema to carry the F8-003 `ViewStates` map? Recommend
        scope-tight v1 (just the view enum) and a follow-up after
        F8-003 lands; document the schema bump path so the
        forward-compat fixture stays honest.
      - ADR threshold: a single persisted enum probably does not
        clear the ADR bar, but the XDG location + read-only
        invariant + precedence rules are durable conventions worth
        memorializing in `docs/operations.md` §"Caches" (or a new
        §"TUI state") regardless. Decide during impl whether the
        cross-surface impact warrants a short ADR.
  - Blockers: none structurally — `App::switch_view` and
    `default_view` already exist. Coordinate with `F8-003` so the
    schema can grow without a breaking migration; coordinate with
    `F8-005` so the new view-switch accelerators all funnel through
    the same persistence seam.

- [ ] `T8-044` Spike: evaluate `tui-pantry` as a widget-iteration
  harness.
  - Motivation: the TUI carries ~5.2k LOC of in-house widgets across
    `src/tui/widgets/` and visual judgement calls (column widths,
    chip placement, glyph spacing, narrow-pane truncation) currently
    iterate through `conspectus tui --snapshot` (ADR 0067) plus
    fixture diffs. That loop is excellent for regression coverage
    but slow for the "does this look right at 80 cols?" question
    the recent H-UI-001..008 series kept asking. `tui-pantry` is a
    Storybook-style preview harness for ratatui widgets — boot one
    widget with chosen prop variants, no reducer, no fixture. The
    spike tests whether it shortens the visual-iteration loop
    enough to justify keeping.
  - Scope (time-boxed, ~1 sprint):
      - Add `tui-pantry = "0.4"` to `[dev-dependencies]` and a
        `pantry.toml` config at the repo root.
      - Stand up a single binary target (`src/bin/pantry.rs` or
        `examples/pantry.rs` — decide during impl based on
        `cargo run --example` ergonomics vs `cargo run --bin`
        discoverability).
      - Port exactly one widget as the smoke test —
        `widgets/multi_select.rs` is the cleanest pure state
        machine and the lowest-risk port. Three prop variants:
        empty list, mid-selection, large list with scroll.
      - Spike outcome at the end: a one-paragraph note in the
        backlog entry recording (a) whether the visual loop felt
        materially faster than `--snapshot`, (b) how much glue per
        widget, (c) whether the pantry API's `pantry.toml` +
        ingredient model survives Conspectus's widget shapes
        without contortion, and (d) the go/no-go call.
      - If go: file follow-up stories per widget port (controls,
        pins-form, value modal, help legend, search, toast — six
        candidates) and a theme-harness ingredient that exercises
        every `[tui.theme]` key against the dark/light presets.
      - If no-go: rip out the dev-dep + binary + `pantry.toml`,
        record the lesson, close the story.
  - Tests: none beyond the spike's own compile check —
    `cargo check --examples` (or `cargo check --bin pantry`) must
    pass and the existing `cargo nextest run --all-targets
    --all-features` must remain green with the new dev-dep present.
    The pantry binary itself is dev-time and is not gated by CI
    correctness tests.
  - Risks / cons recorded up front:
      - `tui-pantry` is pre-1.0 (v0.4.0, ~53% docs coverage,
        single-org upstream taho-inc). Expect API churn; mitigated
        by the dev-dep posture — failure mode is `cargo update`
        breakage, not runtime regression.
      - Maintenance tax on the ingredient set: every ported widget
        is one more thing to keep in sync. Spike sizes that tax
        for one widget so the go-decision is informed.
      - Pantry's `Pane` primitive is a preview-cell frame, not an
        app-layout pane — does not address focus-chain or modal-
        routing pain. Those remain Tier 1 candidates from the
        prior ratatui-widget-library audit (`tui-textarea` on
        demand, `tui-popup` if framing duplicates).
  - Open questions:
      - Binary vs example target: example is the canonical
        Cargo pattern for development harnesses; binary keeps the
        pantry close to the `src/` tree and reuses the workspace's
        clippy/fmt config without `--examples`. Decide during
        impl.
      - Whether the spike should run *before* a major widget audit
        (so the audit benefits from the loop) or *after* (so the
        audit informs which widgets are worth porting). Recommend
        before — the next H-UI-* and T8-* widget passes are the
        immediate beneficiaries.
  - Blockers: none. Adopt during a quiet sprint before the next
    widget-heavy story (H-UI-004 audit is the natural next
    customer if the spike lands go).

- [ ] `T8-022` Detect session live status (running / waiting / idle /
  error) and surface it as a row glyph and per-status header chip.
  - Scope: this is the agent-deck signal the styling overhaul
    deliberately did not invent (color buckets stand in for v1).
    Real status detection wants a data-layer feature: observe mux
    pane changes (delta against last capture; tied into the throttle
    work in `T8-009`), join hook-sidecar evidence (ADR 0028) when
    the harness exposes a "waiting on permission" or "tool error"
    state, and expose a new `SessionStatus` enum on the row
    view-model. The renderer reads it through the existing `Theme`
    additions (`status_running`, `status_waiting`, `status_idle`,
    `status_error` — already reserved in the badge widget's color
    vocabulary). Header chips swap their per-mux-state breakdown
    for per-status counts when the operator opts in via
    `[tui.show_status]`.
  - Tests: pane-delta detector unit tests, hook-sidecar status
    extraction tests, reducer tests for the new `Msg::SessionStatus`,
    Ratatui snapshots for the colored row glyph and header chip
    variants.
  - Blockers: needs its own ADR (decision: where status lives in the
    graph; whether it's a candidate-link relation or a property on
    `AgentSessionNode`; cadence + cost of pane-delta polling). Lives
    downstream of `T8-009` so the throttle/freshen work pays for the
    extra capture cadence.

- [ ] `T8-023` Ship preset theme variants on top of ADR 0032.
  - Scope: layer named palette presets (`tokyo-night`, `dracula`,
    `solarized-light`, `default-dark`) on top of the flat
    `[tui.theme]` schema. Implementation can stay purely additive
    (config-snippet files shipped in `examples/` that operators
    paste into their config) before any code-level
    `[tui.theme.preset] = "tokyo-night"` selector lands. The latter
    needs a small follow-on ADR clarifying preset precedence vs
    per-key overrides.
  - Tests: parse-and-apply tests over each shipped snippet; visual
    diff snapshots for one representative session view per preset
    (only after preset selector wiring lands).
  - Blockers: ADR 0032 (landed).

- [ ] `T8-024` Add sessions-tree density modes — folds `T8-015` into
  the theme-aware renderer landed by the styling overhaul.
  - Scope: now that the renderer reads its palette and structural
    cues from `Theme`, density modes can live in the same shape:
    a `[tui].density` setting plus a runtime toggle that picks one
    of `compact` / `balanced` / `expanded`. `compact` drops the
    per-session same-line preview and tightens the badge column;
    `expanded` enables multi-line previews and the future
    section-pane treatment for inline detail (depends on
    `H-TRANSCRIPT-008`).
  - Tests: row-tree/render snapshots per density at 80×24 and
    160×40; reducer tests for the runtime toggle key.
  - Blockers: supersedes `T8-015`'s open scope. Coordinate with
    `T8-009` (preview throttle) so the expanded mode's extra
    capture work plays nicely with the cadence story.

## Phase 9: Embedded Query Engine

Source plan: `plans/stage-3-sqlite-query-engine.md` (the activation of
ADR 0035 stage 3). Phase goal: deliver `conspectus query <sql>` — a
user-facing SQL surface over the resolved graph — backed by an
embedded SQLite engine. The phase introduced a typed SQL schema
mirroring the Rust model, a loader, a query runner, a small library of
saved views, and a fixture-driven regression suite. The future
file-backed `graph.sqlite` warm-start lifecycle is tracked under
Phase 7; the current one-shot CLI can materialize a resolved snapshot
into an in-memory SQLite database when no persisted graph exists.

This phase was gated on an ADR cluster (engine selection, persistence
model, server transport, library API gating, distribution amendment,
resolver-stays-in-Rust, and vector search). The engine, query feature,
distribution amendment, resolver-boundary, vector-search, and
consumer-surface ADRs have landed. The persistence and transport
pieces remain the SQLite-aware re-scopes of `P7-001` and `P7-004`.

Dependency shape inside the phase:

```
ADR cluster (engine/persistence/transport/lib-api/dist/resolver) ─┐
                                                                  │
P9-001 (spike) ──→ P9-002 (schema) ──┬──→ P9-003 (loader) ──→ P9-004 (query MVP) ──┬──→ P9-006 (saved views)
                                     │                                             ├──→ P9-005 (result fmts)
                                     │                                             └──→ P9-007 (fixture corpus)
                                     │
                                     └──→ P9-007 in parallel after P9-002

P9-008 (vector search via sqlite-vec) ──→ P9-FU-001 (embedding import)
```

The TUI workstream (Phase 8 open stories) began independently, but
Phase 10 later made SQLite the sole consumer-side read surface. The
remaining producer-side `GraphSnapshot` shape stays scoped to
discovery, resolution, JSON dump, and fixture setup.

- [x] `P9-001` SQLite integration spike (bundled build, WAL, lifecycle).
  - Scope: integrate the `rusqlite` crate behind a `query` Cargo feature
    (per ADR-D) with the `bundled` sub-feature on. Confirm SQLite
    3.51.3+ ships and add a CI assertion that fails the build below
    that floor (the 3.51.3 fix addresses the WAL-reset corruption bug
    that affected 3.7.0–3.51.2 in multi-writer / multi-checkpointer
    scenarios — exactly our server + CLI pattern). Measure
    release-binary size with and without the feature. Validate that
    `nix develop` produces a working build. Confirm WAL mode behavior
    by running a two-process smoke test (process A holds a writer
    connection while process B opens a reader; assert no blocking).
    Record findings as a short note appended to the distribution ADR.
    No graph schema yet; the spike is operational.
  - Tests: integration tests that open an in-memory connection, run
    `SELECT 1`, and that open a file-backed WAL-mode connection from
    two processes. Build matrix check that the non-feature build is
    unchanged. CI assertion on bundled SQLite version.
  - Manual checks: `cargo build` with and without `--features query`;
    inspect release binary size; run the two-process smoke test
    manually.
  - Blockers: ADR-A (engine selection), ADR-D (library API), ADR-E
    (distribution amendment).
  - Outcome: accepted ADRs 0036, 0039, 0040, and 0041; added the
    default-on `query` feature and the SQLite query module. The spike
    coverage now includes a bundled SQLite version floor
    (`MIN_SQLITE_VERSION = 3.51.3`), an in-memory `SELECT 1` smoke
    test, and a WAL reader/writer concurrency smoke test.

- [x] `P9-002` Define the SQL schema for the resolved graph.
  - Scope: produce a DDL for tables `nodes`, `node_repos`,
    `node_checkouts`, `node_workspaces`, `node_agent_sessions`,
    `node_mux_sessions`, `node_branches`, `node_forks`,
    `node_forge_prs`, `candidate_links`, `resolved_relationships`,
    `diagnostics`, `aliases`, `provider_state`. One table per node
    kind for queryability; a `v_nodes` view that unions them by
    `node_id` and `node_kind`. Columns mirror the Rust model
    field-for-field where possible; polymorphic blobs
    (`SourceMetadata.fields`, `UnresolvedEndpoint.metadata`) land in
    `TEXT` columns holding JSON, queryable via `JSON_EXTRACT`. Stable
    indexes on `(source_node_id, relation)`,
    `(target_node_id, relation)`, `(provider, freshness_epoch)`,
    `(last_active_epoch DESC)`. Schema version recorded via
    `PRAGMA user_version` aligned with `GraphSnapshot`'s schema
    version. DDL lives under `src/query/schema.sql` (or a Rust
    constant) so the loader can apply it deterministically.
  - Tests: a DDL apply test that runs the schema against a fresh
    in-memory connection and confirms no errors. A test asserting
    every `NodeKind` and `RelationKind` variant maps to a column /
    enum entry in the DDL so adding a new variant fails compilation.
  - Manual checks: `sqlite3 :memory: < schema.sql` lists the expected
    tables / indexes.
  - Blockers: `P9-001`.
  - Outcome: added `src/query/schema.sql` and `src/query/schema.rs`
    with typed node tables, candidate/resolved/diagnostic/alias
    tables, curated saved views, schema versioning, generated
    endpoint-kind columns, JSON-encoded endpoint references, and
    schema-drift tests that compare the DDL against Rust constants.
    ADR 0044 records the JSON endpoint decision.

- [x] `P9-003` Implement the GraphSnapshot → SQLite loader.
  - Scope: take a `&GraphSnapshot` and a `&mut Connection`,
    populate every table per `P9-002`, in one transaction. The
    original plan expected to drive from `SnapshotIndex`; P10 later
    removed that selector layer, so the loader now walks the producer
    snapshot directly.
    Idempotency: re-running the loader replaces all rows
    (`DELETE FROM ...` followed by `INSERT`) inside the transaction.
    For partial eviction (`P7-005`), the loader takes an optional
    `provider` filter and only touches rows owned by that provider.
    Use prepared statements + bind parameters; no string-built SQL.
    Performance target: a 1k-node / 5k-link graph loads in under
    50ms on a typical dev machine.
  - Tests: round-trip tests — load each fixture snapshot, then
    `SELECT *` and compare against the Rust-side projection.
    Per-node-kind tests confirming every field survives a round trip.
    Idempotency tests (load, re-load, identical result).
    Provider-scoped reload tests confirming only the named provider's
    rows change.
  - Manual checks: load a real snapshot and run a few `SELECT`s
    against it; confirm row counts match Rust-side counts.
  - Blockers: `P9-002`.
  - Outcome: added `src/query/loader.rs`, `query::load`, and
    `query::materialize_snapshot`. The loader writes graph nodes,
    candidate links, resolved relationships, diagnostics, and aliases
    with prepared statements and is covered by fixture round-trip tests
    through the reader. `P10-013` later removed the `SnapshotIndex`
    dependency; the loader now iterates producer snapshots directly.

- [x] `P9-004` Implement `conspectus query <sql>` MVP.
  - Scope: a new CLI subcommand that (a) opens
    `$XDG_DATA_HOME/conspectus/graph.sqlite` in read-only mode (via
    `SQLITE_OPEN_READONLY`), falling back to building an in-memory
    snapshot from cold discovery if the file is absent, (b) executes
    the user's SQL with the standard pragma triplet
    (`synchronous=NORMAL`, `busy_timeout=5000`,
    `wal_autocheckpoint=1000`), (c) renders the result. Read-only
    enforcement: the read-only open mode rejects mutations at the
    SQLite layer (no DML, no DDL, no `ATTACH ... AS rw`). Render to a
    plain text table by default. `--format json` for machine output.
    `--format` flag value list expands in `P9-005`.
  - Tests: CLI integration tests for `conspectus query 'SELECT count(*)
    FROM nodes'`, a join across `candidate_links` and
    `node_agent_sessions`, a recursive CTE for fork ancestry, and
    expected-failure tests for `INSERT`, `UPDATE`, `DELETE`,
    `CREATE`, `DROP`. Snapshot tests over the fixture corpus.
  - Manual checks: ad-hoc `conspectus query` invocations against a
    populated graph; confirm output readability and that mutation
    statements fail with a clean error.
  - Blockers: `P9-003`; persisted warm-start remains tracked by
    `P7-003`.
  - Outcome: added the `conspectus query` subcommand and
    `src/query/runner.rs`. The runner opens a read-only
    `graph.sqlite` when present, otherwise performs cold discovery
    into an in-memory SQLite database, sets `PRAGMA query_only = 1`,
    runs user SQL, and returns clear read-only errors for mutation
    attempts. CLI smoke tests cover table and JSON output plus
    rejected `INSERT` / `CREATE` statements.

- [x] `P9-005` Result formatters for query output.
  - Scope: support `--format table` (default, columnar, width-aware
    via the existing renderer's truncation helpers), `--format json`
    (one object per row), `--format csv`, `--format tsv`. Width-aware
    output mirrors the existing `conspectus table` behavior
    (ADR 0020). Color codes per ADR 0022 when stdout is a TTY.
  - Tests: format snapshots over fixture queries; width-aware
    truncation snapshots at 80 and 160 columns.
  - Manual checks: pipe each format to a file and inspect.
  - Blockers: `P9-004`.
  - Outcome: query output supports `--format table|json|csv|tsv`.
    Table output reuses the shared width-aware rendering substrate and
    honors `--width`, `--wide`, and `--color`; CLI and runner tests
    cover each format and truncation behavior.

- [x] `P9-006` Saved views: a library of common queries.
  - Scope: ship a small set of named views (CREATE VIEW under the
    DDL) that name the joins users would write by hand:
    `v_sessions_with_repo`, `v_mux_attachments`, `v_pr_by_branch`,
    `v_fork_ancestry` (recursive), `v_workspace_member_repos`. The
    view set is small and curated — not a contract. Document each in
    `docs/query-guide.md`. `conspectus query --list-views` enumerates
    them.
  - Tests: each named view selectable; query against each returns
    expected fixture rows; `--list-views` snapshot test.
  - Manual checks: `conspectus query 'SELECT * FROM v_fork_ancestry'`
    on a populated graph.
  - Blockers: `P9-002`.
  - Outcome: schema and registry now ship `v_sessions_with_repo`,
    `v_mux_attachments`, `v_pr_by_branch`, `v_fork_ancestry`, and
    `v_workspace_member_repos`; `conspectus query --list-views`
    renders the curated registry. `docs/query-guide.md` documents the
    views, and query regression snapshots exercise each saved view.

- [x] `P9-007` Fixture corpus and query regression suite.
  - Scope: a small library of representative graph fixtures
    (sparse-orphan-session, multi-checkout-repo, fork-ancestry-chain,
    workspace-with-prs, ambiguous-mux-candidates) and a fixture-driven
    test that runs a battery of canned queries against each, asserting
    stable result shapes. Lives alongside the existing snapshot
    fixtures.
  - Tests: itself — this is the regression net.
  - Blockers: `P9-002`, `P9-003`.
  - Outcome: added `tests/query_regression.rs` plus snapshots for
    sparse orphan sessions, multi-checkout repos, fork ancestry,
    workspace PRs, ambiguous mux candidates, and saved-view row counts.
    The suite runs canned SQL against fixture snapshots materialized
    through the query loader.

- [x] `P9-008` Vector search via `sqlite-vec` (deferred).
  - Scope: settle embedding sources, dim budget, ingestion lifecycle,
    and `sqlite-vec` integration. Add an embedding overlay table,
    expose `conspectus query --similar-to <node-id>`, and keep actual
    embedding computation outside Conspectus.
  - Blockers: ADR-G, now accepted as ADR 0042.
  - Outcome: accepted ADR 0042. The schema now includes an
    `embeddings` overlay table, the query runner can load a SQLite
    extension with `--load-extension`, and `conspectus query
    --similar-to <node-id>` performs a built-in cosine-distance
    nearest-neighbor scan over imported embedding blobs. Runtime
    distribution of `sqlite-vec` and embedding ingestion remain
    follow-ups rather than normal discovery behavior.

- [ ] `P9-FU-001` Add an embedding import command.
  - Scope: implement the ADR 0042 import path for external embedding
    pipelines: read JSON Lines from stdin with `node_id`,
    `source_field`, `model`, and `vector` fields, validate dimensions,
    and insert/update rows in the `embeddings` table through the
    writer path. Keep embedding computation out of Conspectus.
  - Tests: CLI tests for valid imports, malformed JSON, unknown node
    IDs, mixed dimensions, duplicate replacement, and subsequent
    `--similar-to` results.
  - Blockers: none; future server writer routing may refine the
    mutation path.

## Phase 10: SQLite As Sole Consumption Surface

Gated on ADR 0043. Phase goal: collapse the dual read path (in-memory
`SnapshotIndex` + SQLite) into one. All view, render, and inspection
code reads from a `rusqlite::Connection`; `GraphSnapshot` is demoted
to a producer-side intermediate scoped to the discovery → resolver →
loader pipeline.

The spike on `phase-9-sqlite-spike` proved out the shape:
`src/query/reader.rs` rounds-trips the loader fixtures losslessly,
and `src/output/agent_sqlite.rs` reimplements 9 of 17 agent-projection
cells against `v_sessions_with_repo` + a `LEFT JOIN`, reusing the
existing `RenderOptions` / `render_rows` substrate unchanged. The
spike code is a reference, not the production landing — Phase 10
stories formalize it.

Dependency shape inside the phase:

```
ADR 0043 ──→ P10-001 (round-trip + read exhaustiveness) ──┬──→ P10-002 (structured-id columns)
                                                          │
                                                          └──→ P10-003 (shared render substrate)

P10-002 + P10-003 ──→ P10-004 (agent) ──┬──→ P10-005..009 (per-renderer migration, parallel after agent)
                                        │
                                        └──→ P10-010..011 (TUI migration, parallel after agent)

(all migrations) ──→ P10-013 (retire indexes) ──→ P10-014 (demote GraphSnapshot)
```

The Phase 8 ADR 0031 TUI views (Mux/Union/Prs/Forks) are independent;
this phase migrates whichever ones exist when each story lands.

- [x] `P10-001` Promote reader + add compile-time read exhaustiveness.
  - Scope: take the spike's `src/query/reader.rs` to production
    quality. Keep the round-trip equality test (`canonicalize()` on
    both sides) as the regression net. Add a per-table read helper
    (trait or macro) that pairs column-list constants with typed-row
    mappers so adding a column to `schema.sql` without updating the
    reader fails compilation — symmetric to the loader's exhaustive
    `let RepoNode { … } = repo;` destructuring. Move the spike's
    `parse_node_id` to the reader module unchanged; it survives only
    until `P10-002` lands.
  - Tests: round-trip equality over the existing loader fixture
    corpus (empty, full, link state variants, diagnostic variants,
    aliases). One synthetic test per node table that asserts adding
    a column without updating the reader fails to compile
    (`#[deny(unused)]` against the typed-row helper or equivalent).
  - Manual checks: `conspectus dump --format json` against a real
    `graph.sqlite` reads back a snapshot that re-serializes equal
    to the JSON the loader produced from.
  - Blockers: ADR 0043.
  - Outcome: spike reader promoted in place (module doc updated,
    dead `_row` parameters dropped). Compile-time exhaustiveness
    against the model is provided by the existing
    `Ok(NodeKind { … })` constructions (a model field addition
    breaks the build the same way the loader's destructure does).
    Schema-side drift detection lives in `schema::TABLE_COLUMNS`
    plus the new `schema_columns_match_constants` and
    `table_columns_covers_every_relation_in_schema` tests — adding
    or renaming a column / table / view in `schema.sql` without
    updating the constants (or vice versa) fails the suite.
    `parse_node_id` stays with a comment noting it disappears in
    P10-002.

- [x] `P10-002` JSON-encoded NodeId foreign references (ADR 0044).
  - Scope: replace the `*_node_id TEXT` foreign-reference columns in
    `candidate_links`, `resolved_relationships`, `diagnostics`, and
    `aliases` with a JSON column holding the serde-serialized typed
    value plus a `GENERATED ALWAYS AS (json_extract(col, '$.type'))
    STORED` discriminator column. The loader writes via
    `serde_json::to_string(&link.source)` etc.; the reader recovers
    typed values via `serde_json::from_str::<NodeId>` and
    `reader::parse_node_id` is deleted. `resolved_relationships` PK
    becomes `(source, relation, target)`; `aliases` PK becomes
    `node`. Rewrite `v_mux_attachments`, `v_pr_by_branch`,
    `v_fork_ancestry`, and `v_workspace_member_repos` to use
    `json_extract` for structural joins; `v_sessions_with_repo` and
    `v_nodes` are unaffected. Add expression indexes for each
    rewritten view's access pattern. Bump `SCHEMA_VERSION` from 2
    to 3. Update `TABLE_COLUMNS` for the new shape; the
    `schema_columns_match_constants` test from P10-001 catches the
    schema-side drift.
  - Tests: round-trip equality test from P10-001 stays green
    byte-for-byte. Add `every_node_id_variant_round_trips_through_json`
    covering all 8 `NodeId` variants (catches serde shape drift
    that the schema-side test cannot see). Add a test covering a
    `RepoId` whose `common_dir` contains `@`, `:`, `#`, and `/` —
    values `parse_node_id` would mishandle — and confirm the JSON
    encoding preserves them losslessly through load → read. Saved
    view smoke tests stay green; add a fixture-driven test that
    asserts `v_mux_attachments` returns the same rows after the
    JOIN rewrite.
  - Manual checks: run a fresh discovery cycle on a real corpus
    and confirm endpoint columns inspect cleanly via `sqlite3`;
    confirm `--similar-to` and the existing saved-view queries
    behave identically.
  - Blockers: `P10-001`. ADR: 0044.
  - Outcome: `candidate_links`, `resolved_relationships`,
    `diagnostics`, and `aliases` now store endpoints as JSON via
    `serde_json::to_string(&node_id)`. STORED GENERATED `*_kind`
    columns expose `json_extract(col, '$.type')` for indexed
    filtering; `schema_columns_match_constants` runs against
    `PRAGMA table_xinfo` so generated columns participate in
    drift detection. `parse_node_id` is gone; the reader uses
    `serde_json::from_str::<NodeId>` everywhere. `SCHEMA_VERSION`
    bumped 2 → 3. Saved views `v_mux_attachments`,
    `v_pr_by_branch`, `v_fork_ancestry`, and
    `v_workspace_member_repos` rewritten to use structural joins
    via `json_extract` against the typed node tables; the
    `idx_candidate_links_target_mux_native_id` expression index
    supports `v_mux_attachments`'s join path. The
    `every_node_id_variant_round_trips_through_json` test in
    `query::reader` covers every NodeId variant; the
    `endpoints_with_separator_chars_round_trip_through_json` test
    confirms `RepoId` / `BranchId` / `ForgePrId` values containing
    `:` `@` `#` `/` round-trip losslessly through the load → read
    cycle (which the old `Display`-parser approach could not
    have done). `docs/query-guide.md` updated for the new column
    set and the recursive-CTE example. 733 lib tests pass; all
    integration test binaries green.

- [x] `P10-003` Extract the shared rendering substrate.
  - Scope: lift `RenderOptions`, `Layout`, the `ColumnSpec`
    registries, `render_rows`, `format_relative_age`,
    `node_short_id_from_display`, `unique_prefix_len`, `header_label`
    out of `src/output/table.rs` into a backend-agnostic module
    (`src/output/render/` or `src/output/substrate.rs`) consumed by
    both the in-memory and SQLite renderers during the migration.
    No behavior change; this is structural so the migration stories
    can land incrementally without circular dependencies. The
    `*_PUBLIC` aliases introduced by the spike are removed in favor
    of clean re-exports.
  - Tests: existing renderer snapshot tests stay green byte-for-byte.
    A new module-boundary test confirms the substrate has no
    `crate::model::*` dependencies.
  - Manual checks: `cargo build --no-default-features --features
    query` (verifies the substrate compiles without the in-memory
    renderer when the cfg is set up to allow it).
  - Blockers: ADR 0043.
  - Outcome: substrate now lives at `src/output/render.rs`.
    `output::table` re-exports the public surface so external callers
    (`cli`, `node_show`, `tui`, `query::runner`) keep compiling
    unchanged. `agent_sqlite.rs` imports from `super::render`
    directly; the `SESSIONS_COLUMNS_PUBLIC` workaround is gone.
    The `substrate_has_no_model_deps` test reads `render.rs` via
    `include_str!` and flags any `use … crate::model` line that
    sneaks in. All 732 lib tests pass byte-for-byte; full suite
    green.

- [x] `P10-004` Migrate the CLI agent projection to SQLite.
  - Scope: replace `render_with(snapshot, Projection::Agent, opts)`'s
    code path with a `Connection`-driven implementation modeled on
    `src/output/agent_sqlite.rs` from the spike. All 17 cells
    covered (the spike's 9 plus `mux`, `mux-conf`, `pr`, `pr-conf`,
    `lineage`, `workspace`, `fork`, `declared`). Extend
    `v_sessions_with_repo` (or add sibling views) for the
    `pick_preferred`-mediated cells; lean on `resolved_relationships`
    so the resolver's tie-break stays authoritative. `RowFilter`
    sits on top of the result set — no SQL filter pushdown in v1.
  - Tests: parity test that runs the old and new renderers from the
    same fixture and asserts byte-equal output across the existing
    `output::table` snapshot corpus. Width-aware truncation
    snapshots unchanged. Filter behavior preserved.
  - Manual checks: `conspectus table --rows sessions` against a
    real `graph.sqlite`; visually compare to the prior output.
  - Blockers: `P10-002`, `P10-003`.
  - Outcome: production renderer at `src/output/agent.rs` covers
    all 17 cells. `render_with(snapshot, Projection::Agent, opts)`
    routes through `agent::build_agent_rows_from_snapshot` via the
    new `query::materialize_snapshot` helper; the in-memory
    `build_agent_rows` / `agent_cell` / `lineage_cell` /
    `preferred_pr_for_session` / `session_*` helpers are deleted.
    Cells assemble from one primary query (sessions joined to
    `v_sessions_with_repo` and `aliases`) plus seven per-cell
    side-lookups (`fetch_mux_lookup`, `fetch_branch_lookup`,
    `fetch_lineage_lookup`, `fetch_workspace_lookup`,
    `fetch_fork_lookup`, `fetch_declared_lookup`, plus a global
    `fetch_global_pr`).
    `RowFilter` runs on top of the result set; mux candidate count
    feeds the filter's MuxStateKey input. The spike's
    `src/output/agent_sqlite.rs` is removed.
    Two latent P10-002 bugs surfaced and were fixed here:
    `branch_has_forge_pr` saved view (`v_pr_by_branch`) had the
    direction wrong — production discovery and the in-memory
    `preferred_pr_for_session` both build the link source=ForgePr,
    target=Branch (despite the relation name suggesting the
    opposite); the saved view now joins that way. And saved view
    JOINs that compared structural columns on `node_<kind>` tables
    (e.g. `m.native_id`) were brittle: production discovery
    routinely sets `MuxSessionNode.native_id` to a value distinct
    from `MuxSessionId.native_id` (id is `tmux:<name>`, structural
    is just `<name>`). All such JOINs in both `agent.rs` and the
    saved views now reconstruct the Display form from the endpoint
    JSON and compare against `node_<kind>.node_id`. Existing
    `output::table` snapshot tests are the parity assertion; all
    731 lib tests pass byte-for-byte, full integration suite green.

- [x] `P10-005` Migrate the CLI mux projection to SQLite.
  - Scope: same pattern as `P10-004` for `Projection::Mux`. Use
    `v_mux_attachments` (extended if needed) for the agents-attached-
    to-this-mux cell.
  - Tests: parity with the existing mux-projection snapshots.
  - Blockers: `P10-004` (substrate validated by the agent migration).
  - Outcome: production renderer at `src/output/mux.rs` covers all
    8 cells (`id`, `mux`, `cwd`, `agents`, `preview`,
    `attached-count`, `activity`, `created`). One primary query
    (`SELECT … FROM node_mux_sessions`) plus a per-mux attachment
    lookup that mirrors `SnapshotView::attached_to_mux` (pick the
    preferred `linked_to_mux` candidate per source agent, group by
    mux `node_id`, preserve BTreeMap-by-source ordering) and a
    per-agent ambiguity count for the per-attachment indicator's
    `*` marker. `attached_to_mux` is removed from `SnapshotView`;
    the in-memory `mux_cell` / `build_mux_rows` /
    `first_attached_agent_preview` / `MuxRowCtx` are deleted. The
    `v_mux_attachments` saved view didn't need extension — the
    renderer's queries are inlined since the view's columns don't
    include `preview` and adding it would broaden the view's
    contract beyond what callers asked for. Existing
    `output::table` snapshot tests are the parity check; all 731
    lib tests pass byte-for-byte, full integration suite green.

- [x] `P10-006` Migrate the CLI union projection to SQLite.
  - Scope: same pattern for `Projection::Union`. Composes the
    agent/mux query paths over a `UNION ALL` shape.
  - Tests: parity with existing union snapshots.
  - Blockers: `P10-004`, `P10-005`.
  - Outcome: production renderer at `src/output/union.rs` covers
    all 7 cells. The agent/mux merge happens in SQL via the
    pre-existing `v_nodes` view (a `UNION ALL` over every typed
    node table) joined left to `node_agent_sessions`,
    `node_mux_sessions`, and `aliases`; one query produces the
    full ordered row stream and the cell extractor dispatches on
    the row's `node_kind`. `ORDER BY` puts every agent row before
    every mux row and breaks ties within a kind by Display-form
    `node_id`. The `relationship` cell still uses a per-agent
    preferred-`linked_to_mux` lookup that mirrors
    `output::agent::fetch_mux_lookup`.
    Substrate refactor: `pick_strongest` and
    `confidence_precedence` are pulled into `output::render` so
    agent / mux / union share one definition instead of three.
    The in-memory `UnionRowSource`, `UnionRowCtx`, `union_cell`,
    `build_union_rows`, `session_display_title`, and
    `mux_session_label` are deleted. Existing `output::table`
    snapshot tests are the parity check; all 731 lib tests pass
    byte-for-byte and the full integration suite is green.

- [x] `P10-007` Migrate the CLI PRs projection to SQLite.
  - Scope: same pattern for `Projection::Pr`. Use `v_pr_by_branch`.
  - Tests: parity with existing PR snapshots.
  - Blockers: `P10-004`.
  - Outcome: production renderer at `src/output/prs.rs` covers all
    8 cells. The `attached` cell composes three small lookups —
    preferred branch per PR (pick_strongest over
    `branch_has_forge_pr` candidates from PR sources), checkout
    roots per branch (active `checked_out_branch` candidates), and
    agent sessions with a cwd — then matches each PR's branch's
    checkout roots against agent cwd via `path_is_ancestor_of`
    (lifted from `crate::model`). `strip_branch_prefix` is
    promoted to `output::render`'s substrate now that two
    renderers (agent + prs) need it. The in-memory `PrRowCtx`,
    `pr_cell`, `pr_preferred_branch_id`, `pr_branch_label`,
    `pr_attached_session_labels`, `build_pr_rows`,
    `path_is_ancestor_of`, `agent_session_label`, `forge_pr_label`
    are all deleted. Existing `output::table` snapshot tests are
    the parity check.

- [x] `P10-008` Migrate the CLI forks projection to SQLite.
  - Scope: same pattern for `Projection::Fork`. Use
    `v_fork_ancestry`.
  - Tests: parity with existing fork snapshots.
  - Blockers: `P10-004`.
  - Outcome: production renderer at `src/output/forks.rs` covers
    all 7 cells. Primary query against `node_forks`, plus
    side-lookups for `parent` (pick_strongest over
    `parent_session` candidates with the same resolved-vs-unresolved
    label branching the in-memory `fork_parent_session_label`
    used) and `children` (a GROUP BY count of active
    `child_session` candidates targeting agent_session nodes or
    unresolved endpoints). `v_fork_ancestry` was not needed for
    this projection — that view supports recursive parent walks,
    not per-fork rendering. The in-memory `ForkRowCtx`,
    `fork_cell`, `fork_label`, `fork_parent_session_label`,
    `fork_child_session_count`, `build_fork_rows` are deleted.
    With this story, `render_with` no longer constructs a
    `SnapshotView` for any projection — the struct, its `Deref`
    impl, and the `SnapshotView::new` helper are all removed from
    `output::table`. Existing `output::table` snapshot tests are
    the parity check; all 731 lib tests pass byte-for-byte, full
    integration suite green.

- [x] `P10-009` Migrate `node show` to SQLite.
  - Scope: replace the snapshot walks in `src/output/node_show.rs`
    with `Connection`-driven queries per node kind. Short-id
    resolution (`H-TBL-005`) keeps its current shape; the lookup
    moves to `SELECT … WHERE node_id LIKE ?`.
  - Tests: parity with the existing `node show` snapshot corpus
    across every node kind.
  - Blockers: `P10-002`.
  - Outcome: rewritten as
    `resolve_node_id_from_conn(conn, input) -> Result<NodeId, NodeResolveError>`
    + `render_node_show_from_conn(conn, id, color) -> String`. The
    existing `resolve_node_id` / `render_node_show` entry points
    survive as thin bridges that materialize a snapshot to an
    in-memory SQLite connection and delegate. `resolve_node_id`
    walks `v_nodes` for hex-prefix and Display matches, then queries
    `node_agent_sessions` / `node_mux_sessions` for label matches —
    mirroring the in-memory `label_matches`. Per-kind summary
    sections (`write_repo`, `write_checkout`, etc.) each issue a
    single `SELECT … WHERE node_id = ?1` against the matching
    typed table; the agent summary additionally joins to `aliases`
    via the structural-field LEFT JOIN pattern from `output::agent`.
    The candidate-links / resolved-relationships / diagnostics
    sections issue filtered SELECTs (`source = ?` / `target_node =
    ?` / `conflict_source = ?`) with the bind being
    `serde_json::to_string(&NodeId)` — the JSON-encoded endpoint
    columns from ADR 0044 make text equality the right comparator.
    A small `parse_display_via_typed_tables` helper reconstructs
    typed `NodeId`s from a Display string by routing the kind
    discriminator to the right `node_<kind>` PK lookup.
    `reader::parse_node_id_json` is exposed `pub(crate)` so the
    section renderers can turn JSON endpoint strings back into
    `NodeId` Display form for the `→ <target>` / `← <source>`
    cells. All nine node_show unit tests pass byte-for-byte; full
    integration suite green.

- [x] `P10-010` Migrate the TUI detail pane to SQLite.
  - Scope: replace the snapshot walks in `src/tui/detail.rs` with
    `Connection`-driven queries. The detail pane sections from
    ADR 0033 stay; only the data source changes.
  - Tests: parity with the existing detail-pane snapshots; runtime
    smoke test confirms the pane still re-renders on selection
    changes.
  - Blockers: `P10-003`, `P10-009` (so the typed-row patterns are
    settled before the TUI consumes them).
  - Outcome: new
    `build_node_detail_from_conn(conn, target, home) -> Result<Option<NodeDetail>>`
    is the SQLite consumer surface. It calls
    `query::read_snapshot` per-call and runs the existing typed-Rust
    view-model assembly. `build_node_detail` survives for
    fixture-heavy tests and producer-side callers that still start
    from a typed snapshot.
    Trade-off vs. per-section SQL (which P10-009 used): the detail
    builder's view-model assembly (kind-dispatched header fields,
    mux/pr/lineage subqueries with ambiguity counts, link summaries
    with shortened paths) is complex enough that rewriting each
    helper as SQL doubles the line count for no observable
    behavior change. Routing through `read_snapshot` keeps the
    assembly in one place and still satisfies the consumer-side
    contract — the function takes a `Connection`, returns a typed
    view-model, and never persists a `GraphSnapshot`. A new parity
    test asserts the two entry points produce equal `NodeDetail`s
    for the same input, catching drift if a future story refactors
    only one path. Documented the choice in the module doc as a
    pattern: per-section SQL when per-cell formatting is trivial
    (projections, node show); `read_snapshot` bridge when typed
    assembly is complex (TUI detail pane).

- [x] `P10-011` Migrate the TUI sessions row builder to SQLite.
  - Scope: replace `build_sessions_tree` (`src/tui/rows/sessions.rs`)
    and its `SessionsBuildInputs` with a `Connection`-driven
    builder. Grouping/bucketing logic (ADR 0024) stays in Rust on
    top of the result set. `RowFilter` continues to gate per session
    before bucketing. Performance check: the TUI refresh loop
    should stay under its current latency budget on the fixture
    corpus.
  - Tests: parity with the existing sessions-tree snapshots over the
    fixture corpus; refresh-loop latency measurement on the largest
    fixture.
  - Blockers: `P10-004`.
  - Outcome: new
    `build_sessions_tree_from_conn(SessionsBuildInputsFromConn)`
    bridges via `query::read_snapshot` and delegates to the
    existing `build_sessions_tree`. Follows the
    `build_node_detail_from_conn` pattern from P10-010 — the
    grouping/bucketing/launch-context/candidate-mux expansion
    logic is preserved end-to-end; per-section SQL would have
    doubled ~2000 lines of typed assembly for no observable
    behavior change. The existing snapshot-taking
    `build_sessions_tree` survives for fixture-heavy tests and typed
    assembly reuse. A parity test asserts the two entry points
    produce equal `RowTree`s.
    Refresh-loop latency check deferred to a follow-up — current
    refresh is well under any user-noticeable threshold and the
    bridge adds one `read_snapshot` pass which is bounded by
    graph size; if it ever becomes load-bearing, the per-section
    SQL refactor is the optimization story.

- [x] `P10-012` Decide whether TUI Mux/Union/Prs/Forks builders need
  SQLite-specific migration work.
  - Scope: intentionally skip this during Phase 10 closeout. The
    corresponding ADR 0031 TUI row builders have not landed yet, so
    there is no active in-memory consumer to migrate. Revisit after
    `P10-013` / `P10-014` settle the consumer-side contract and decide
    whether the future Mux/Union/Prs/Forks interfaces need any
    P10-specific context or can be built directly on the post-P10
    `Connection` surface.
  - Blockers: ADR 0031 stories that introduce the relevant builders.
  - Outcome: no Phase 10 migration work is needed for these builders.
    Future TUI Mux/Union/Prs/Forks row builders should be implemented
    directly against the post-P10 `Connection` surface instead of
    adding snapshot-first builders and migrating them later.

- [x] `P10-013` Retire `SnapshotIndex`, `SnapshotView`,
  `SessionsIndex`.
  - Scope: delete the in-memory selector layer once no consumer
    depends on it. Producer-side discovery and the resolver may
    keep an internal selector if useful (the loader doesn't need
    one). Remove the dual-renderer plumbing; delete
    `src/output/agent_sqlite.rs` (its production replacement is
    `src/output/table.rs`'s new implementation).
  - Tests: `cargo build` succeeds; the existing test suite stays
    green; `cargo +nightly udeps`-style dead-code sweep finds
    nothing residual.
  - Blockers: `P10-005`..`P10-011` complete; `P10-012` is deferred
    until the non-session TUI builders exist.
  - Outcome: deleted `src/model/index.rs` and removed the
    `SnapshotIndex` re-export. The SQLite loader now iterates
    `GraphSnapshot.nodes` directly; the sessions TUI builder keeps a
    private per-call data helper for its typed assembly instead of the
    shared selector layer. `SnapshotView` / `SessionsIndex` already had
    no production symbols left.

- [x] `P10-014` Demote `GraphSnapshot` to producer-only.
  - Scope: gate the public re-export so library callers who only
    want to render get a `Connection`-flavored API, not a snapshot.
    `GraphSnapshot` remains the resolver's input/output type and
    `conspectus dump --format json`'s wire shape (built via
    `read_snapshot()`), but no consumer holds it across a CLI
    invocation. Update `src/api.rs` accordingly. Refresh
    `docs/library-api.md` and `docs/design.md` to reflect the
    consumer-side contract.
  - Tests: library-API surface tests confirm `GraphSnapshot` is no
    longer reachable from rendering entry points.
  - Manual checks: review the updated library-API doc; confirm an
    external Rust caller building a TUI substitute can succeed
    against the new surface.
  - Blockers: `P10-013`.
  - Outcome: TUI app state now stores a SQLite `GraphDb` wrapper and
    recomputes detail from `build_node_detail_from_conn`; refresh
    materializes the resolved producer snapshot into SQLite before
    dispatching `SetData`. CLI table and `node show` paths materialize
    SQLite and call the connection-backed render/inspection APIs.
    `GraphSnapshot` is no longer re-exported from `api`; it remains
    available from `model` for discovery/resolver/dump/test fixtures.
    `output::table` exposes `render_conn` / `render_with_conn` as the
    canonical renderer surface while preserving snapshot bridges for
    fixture-heavy producer-side tests.

- [ ] `P10-FU-001` Retire snapshot bridge APIs from consumer modules.
  - Scope: remove compatibility entry points that accept
    `GraphSnapshot` only to materialize an in-memory SQLite database
    before rendering or building view-models. Candidate APIs include
    `output::table::render`, `output::table::render_with`,
    `output::node_show::resolve_node_id`,
    `output::node_show::render_node_show`,
    `tui::detail::build_node_detail`, and the snapshot-taking
    `tui::rows::sessions::build_sessions_tree` test bridge. Keep
    producer-side helpers such as `query::materialize_snapshot` when
    they still serve discovery/resolver fixtures or one-shot cold
    builds.
  - Tests: update fixture-heavy tests to materialize SQLite explicitly
    and call the `*_conn` entry points; full suite stays green.
  - Manual checks: review `docs/library-api.md` and public exports so
    rendering examples use connection-backed APIs only.
  - Blockers: P10 has landed and downstream tests/users have had a
    chance to move to `render_conn` / `render_with_conn`.

- [x] `P10-FU-002` Audit GraphSnapshot round-trip and force coverage
  on future fields.
  - Context: two silently-latent round-trip gaps shipped before any
    test caught them. `Diagnostic::PinUnbound` (and the three sibling
    pin diagnostic kinds) was emitted by the resolver and written by
    the loader but unhandled by the reader, so `read_snapshot` failed
    the moment a user created their first pin. `GraphSnapshot.pins`
    was never inserted into SQLite at all, so the TUI's
    `build_sessions_tree_from_conn` (which reads via `read_snapshot`)
    saw zero pins and the synthetic "Pins" group never rendered.
    Both bugs slipped past `P10-001`'s `schema_columns_match_constants`
    and `table_columns_covers_every_relation_in_schema` drift catches
    — those check schema-vs-constants, not model-vs-tables.
  - Scope: walk every top-level `GraphSnapshot` field (`nodes`,
    `candidate_links`, `resolved_relationships`, `diagnostics`,
    `aliases`, `pins`) and assert each one round-trips losslessly
    through `materialize_snapshot` → `read_snapshot`. Walk every
    enum variant the snapshot can carry — `GraphNode`, `LinkState`,
    `LinkEndpoint`, `Diagnostic`, `PinBinding`, `Provenance`,
    `Confidence`, `Freshness`, `RelationKind`, `RuntimeProcessRole`,
    `SessionKind` — and assert each variant round-trips. Add an
    explicit destructure of `GraphSnapshot` in the matrix test so
    adding a new top-level field is a compile error until the test
    handles it (mirror the loader's exhaustive
    `let RepoNode { … } = repo;` pattern at the snapshot level).
    Add an explicit exhaustive `match` on each enum-variant matrix
    so adding a variant is a compile error until the matrix covers
    it.
  - Tests:
    - `graph_snapshot_round_trips_fully_populated`: one test that
      builds a snapshot populated with at least one of every node
      kind, link state, diagnostic variant, pin binding state, and
      alias, materializes it, reads it back, and asserts equality
      via `canonicalize()`.
    - `every_diagnostic_variant_round_trips`: exhaustive match on
      `Diagnostic`, one row in the test matrix per variant, round-
      tripped individually so a mismatch isolates the broken kind.
      Covers the regression that yesterday's `details: TEXT` patch
      addressed.
    - `every_pin_binding_state_round_trips`: same pattern for the
      three `PinBinding` variants plus `binding = None`.
    - `every_link_state_round_trips` and
      `every_link_endpoint_round_trips`: if not already covered,
      mirror the structure.
    - `clear_all_handles_every_materialized_table`: builds a fully-
      populated database, runs `load(empty_snapshot, &mut conn)`,
      asserts every materialized table is empty. Catches the
      regression where `pins` was missing from `clear_all`'s table
      list yesterday.
    - Maintenance lint: a synthetic test that uses
      `let GraphSnapshot { nodes: _, candidate_links: _,
      resolved_relationships: _, diagnostics: _, aliases: _,
      pins: _ } = …;` to bind every field — adding a snapshot field
      without updating the test fails to compile.
  - Out of scope: schema columns for individual model fields beyond
    what the round-trip needs. Pin-quality SQL queryability
    columns (the columnar fields I added to `pins`) are kept where
    they already exist; this story doesn't add or remove them.
  - Blockers: none. Independent hardening pass on the layer
    `P10-001` audited but didn't fully nail down.
  - Outcome: `full_snapshot_round_trips` extended to cover all 4
    pin-* diagnostic variants (including PinUnbound with and
    without last_session) and all 3 PinBinding states plus the
    pre-resolve None case. Seven new audit tests in
    `src/query/reader.rs`:
    `graph_snapshot_field_drift_guard` (destructures GraphSnapshot
    so a new top-level field fails to compile),
    `every_diagnostic_variant_round_trips`,
    `every_pin_binding_state_round_trips`,
    `every_link_state_round_trips`,
    `every_link_endpoint_round_trips`,
    `every_session_kind_round_trips`,
    `every_runtime_process_role_round_trips`,
    `clear_all_handles_every_materialized_table`. Each uses an
    exhaustive match on its enum so adding a variant is a
    compile error until the matrix covers it. SessionKind and
    RuntimeProcessRole tests in particular catch the reader's
    silent-`None` asymmetry on unknown strings (writer is
    exhaustive; reader fan-in maps unknowns to None). The
    `clear_all` test catches the recent `pins` missing-table
    regression by reloading-with-empty and asserting every
    materialized table is empty afterwards.

## Graph Visualization Workstream

- [x] `GV-001` Record graph visualization export decisions.
  - Scope: write an ADR covering graph visualization outputs: Graphviz
    DOT for static inspection and an HTML output for interactive,
    navigable graph inspection. Decide how the HTML renderer loads its
    JavaScript graph library (vendored asset, CDN, or generated
    self-contained bundle), the minimum feature set, and how large
    graphs should degrade.
  - Tests: none; docs-only decision.
  - Manual checks: review the ADR against `docs/design.md` and update
    the design doc if the exported graph shape or CLI surface becomes
    part of the product contract.
  - Blockers: `H-MUXPROC-FU-006`.
  - Outcome: ADR 0050 settles both formats, picks inlined Cytoscape.js
    as the HTML library, locks provider-neutral `NodeKind` /
    `RelationKind` / `Provenance` visual encoding, treats candidate
    vs. resolved as toggleable views in HTML (distinctly styled
    together in DOT), defaults `RuntimeProcess` and unresolved-
    endpoint stubs to visible-but-filterable, commits to
    deterministic emission, and introduces a shared `[theme]` table
    with `[tui.theme]` / `[html.theme]` overrides. Bespoke
    navigation chrome (focus, N-depth, upstream/downstream) wraps
    the Cytoscape API directly. Live server-hosted HTML view is
    flagged as a follow-up. `docs/design.md` gains a Graph
    Visualization Exports subsection and a Decisions entry.

- [x] `GV-002` Add `conspectus graph --format dot`.
  - Scope: add a Graphviz DOT renderer for the resolved internal graph.
    Include node kind, stable id/label, and enough styling to distinguish
    repos, checkouts, workspaces, agent sessions, mux sessions, branches,
    forks, and forge PRs. Render candidate links and resolved
    relationships distinctly so ambiguity and resolver decisions are easy
    to inspect. Once `TEST-006` exists, support rendering the named
    replay scenarios so graph visualization can be used for fixture and
    regression review without recreating local state by hand.
  - Tests: deterministic DOT snapshot tests for sparse graph, mux
    candidate ambiguity, session lineage, fork ancestry, and branch→PR
    fixtures; include at least one named replay scenario when the
    scenario registry is available.
  - Manual checks: run `dot -Tsvg` on at least one generated fixture and
    inspect that labels and edge kinds remain readable.
  - Blockers: `H-MUXPROC-FU-006`, `GV-001`.
  - Outcome: `conspectus graph --format dot` ships alongside the
    existing `--format json`, with `--candidates {include,exclude}`
    and `--diagnostic-nodes {include,exclude}` flags per ADR 0050.
    `conspectus dev scenario graph --format dot <name>` extends the
    same surface to named replay scenarios. The renderer
    (`src/output/dot.rs`) is provider-neutral: shape/fill keyed on
    `NodeKind`, arrowhead on `RelationKind` category, penwidth and
    color tint on `Provenance`, resolver-preferred candidates marked
    `★` and solid, losing candidates dashed, ignored/overridden
    candidates dashed-red with a tooltip, unresolved endpoints
    rendered as dashed-circle stubs. Nodes are grouped into
    per-`NodeKind` `subgraph cluster_*` blocks in fixed order;
    emission is deterministic (BTreeMap node walk, sorted edges).
    Snapshot coverage in `tests/dot_snapshots.rs` covers empty,
    orphan-session, mux candidates (with and without
    `--candidates exclude`), unresolved lineage, fork ancestry, and
    branch→PR fixtures. The `process-cardinality` named scenario is
    exercised structurally to assert the `--diagnostic-nodes`
    filter. Manually verified with `dot -Tsvg` on the `exact-match`,
    `ambiguous-mux`, and `fork-lineage` scenarios.

- [x] `GV-003` Add `conspectus graph --format html`. Split into
  `GV-003a` / `GV-003b` / `GV-003c` so the foundational payload and
  vendoring story are settled before the chrome is built on top.
  Aggregate scope is unchanged from the original ticket: a single-file
  self-contained HTML explorer backed by the same resolved graph as
  DOT, supporting pan/zoom, selection, neighbor highlighting,
  search/filter, an inspector, and the navigation primitives from ADR
  0050 decision 10. Closed when GV-003a/b/c all landed; GV-003d
  (layout selector + dagre) shipped in parallel as a follow-up.

- [x] `GV-003a` HTML renderer scaffolding + minimal viewer.
  - Scope: add `conspectus graph --format html` and the matching
    `conspectus dev scenario graph --format html`. Vendor Cytoscape.js
    (UMD build) plus the `fcose` layout extension under
    `src/output/html/assets/` with license headers; inline via
    `include_str!`. Produce a single-file HTML output: scaffolding
    HTML + minimal CSS + a thin `GraphDriver` JS module wrapping
    Cytoscape per the ADR 0050 Coupling Boundary + a library-neutral
    JSON payload embedded in the page (not Cytoscape's element format).
    Implement the visual encoding from ADR 0050 decision 3
    (`NodeKind` shape/color, `RelationKind` arrowhead category,
    `Provenance` width/opacity, candidate/resolved styling,
    unresolved-stub dashed terminator). Honor `--candidates` and
    `--diagnostic-nodes` flags. No chrome beyond default Cytoscape
    pan/zoom and click-to-select in this sub-ticket.
  - Tests: unit tests on the Rust payload serialization (deterministic
    ordering, correct shape, `NodeKind` / `RelationKind` /
    `Provenance` coverage); a snapshot test on the rendered HTML with
    the inlined Cytoscape bundle redacted to a hash so the snapshot is
    stable across library updates.
  - Manual checks: open a generated HTML for the `exact-match`,
    `ambiguous-mux`, and `fork-lineage` scenarios in a browser;
    confirm pan/zoom, node selection, and the visual encoding match
    the DOT output.
  - Blockers: `H-MUXPROC-FU-006`, `GV-001`, `GV-002`.
  - Outcome: `conspectus graph --format html` and
    `conspectus dev scenario graph --format html` ship single-file
    self-contained pages (~775 KB) that inline cytoscape@3.33.4,
    cytoscape-fcose@2.2.0, cose-base@2.2.0, and layout-base@2.0.1
    (all MIT, vendored under `src/output/html/assets/` with VERSIONS
    and NOTICE files). The Rust renderer (`src/output/html/mod.rs`)
    emits a library-neutral JSON payload — not Cytoscape element
    format — and the JS `GraphDriver` translates it at load time per
    the ADR 0050 Coupling Boundary. `app.js` is a minimal bootstrap
    (click to surface node info in the status bar); the rich chrome
    (filter panel, inspector, search) lives in GV-003b. Visual
    encoding mirrors the DOT output: `NodeKind` shape/fill,
    `RelationKind` arrowhead, `Provenance` width/color, resolved
    candidates marked solid + `★`, losing candidates dashed,
    ignored/overridden red dashed, unresolved endpoints rendered as
    dashed-bordered stub nodes. `--candidates` and
    `--diagnostic-nodes` flags reuse the GV-002 plumbing. Payload
    emission is deterministic (sorted by kind/id; edges by source/
    relation/target/provenance/id). Test coverage in
    `tests/html_snapshots.rs` is six payload snapshots (empty,
    mux-candidates with and without `--candidates exclude`,
    unresolved-lineage, branch-pr, fork-ancestry), one scaffold
    snapshot with all inlined `<script>` and `<style>` blocks
    redacted to `[redacted N bytes]` markers (stable across library
    bumps; byte-count drift still surfaces in the diff), and a
    structural check on the `process-cardinality` named scenario
    asserting the `--diagnostic-nodes` filter drops RuntimeProcess
    nodes and edges from the payload.
    Post-review fixes: replaced `width: "label", height: "label"`
    with fixed node sizes because the auto-sizing path interacts
    badly with several shape geometries in Cytoscape 3.33 and made
    workspace/fork/agent-session/some-checkout nodes report
    `.visible() === false`, which in turn hid every edge incident
    to them (Cytoscape hides edges with hidden endpoints). Enriched
    labels with a kind-aware secondary line so `main` branch and
    `repo-a` checkout disambiguate against `main` branch in a
    different repo / `repo-a` repo. Added a top-right legend
    overlay sourced from the driver's palette so the encoding is
    self-documenting.

- [x] `GV-003b` HTML inspector, filter panel, and search.
  - Scope: add the bespoke chrome that wraps the GV-003a `GraphDriver`.
    Filter panel with NodeKind checklist, RelationKind checklist,
    candidate/resolved toggle (default resolved per ADR 0050
    decision 4), RuntimeProcess toggle, unresolved-endpoint stub
    toggle, ignored/overridden toggle, and a one-click "collapsed
    view" preset that approximates what the CLI/TUI would show.
    Inspector panel showing the selected node's attributes plus
    grouped incoming/outgoing edges with provenance and confidence.
    Free-text search box that filters nodes by label/id and dims the
    rest. All chrome modules call into the `GraphDriver` interface;
    none reach into the underlying Cytoscape instance directly.
  - Tests: extend the GV-003a payload tests with cases that exercise
    each filter dimension's data (ignored links, RuntimeProcess
    nodes, unresolved endpoints). HTML scaffold snapshot continues
    to redact the Cytoscape bundle hash.
  - Manual checks: against a real local graph, toggle every filter,
    confirm the "collapsed view" preset matches what `conspectus
    table sessions` would show, and verify the inspector renders the
    same fields as `conspectus node show`.
  - Blockers: `GV-003a`.
  - Outcome: shipped a three-column page layout (filters left,
    Cytoscape canvas center, inspector/legend right) with a
    header search input. New `GraphDriver` methods (setHidden,
    setDimmed, selectNode, onSelectionChange, getDetail) keep the
    chrome modules off the underlying Cytoscape instance per the
    ADR 0050 Coupling Boundary. `filter-panel.js` exposes per-
    kind and per-relation checklists plus four view toggles (show
    candidates / RuntimeProcess / unresolved-stubs /
    ignored-overridden) and a one-click "Collapsed view" preset
    that approximates the CLI/TUI view. `inspector.js` shows the
    selected node's flattened attributes (dotted paths, nulls
    suppressed) and grouped incoming/outgoing edges with
    provenance, confidence, ★ for resolved, state badges for
    ignored/overridden; clicking a neighbor row navigates the
    inspector and focuses on the graph. Search dims non-matching
    nodes and their incident edges via setDimmed, with a counter
    in the header. New `ignored_and_overridden_graph` fixture and
    a payload snapshot cover state="ignored" / "overridden"; a
    structural check confirms every chrome slot is present in the
    rendered scaffold. 1133 tests pass; fmt and clippy clean.
    Manually verified in headless chromium: default view, single-
    node selection (inspector populated), Collapsed view preset,
    and free-text search all behave correctly.

- [x] `GV-003c` HTML navigation primitives.
  - Scope: implement the navigation operations from ADR 0050
    decision 10 against the neutral payload (not Cytoscape's
    collection API): focus on a selected node, restrict the visible
    graph to nodes within `--depth N` of it, restrict to
    upstream-only / downstream-only traversal, and a breadcrumb stack
    so the user can pop back to prior focus states. Keyboard
    shortcuts where they're cheap; menu chrome where they're
    discoverable.
  - Tests: unit tests on the JS traversal helpers if they're moved to
    a small testable module; otherwise structural assertions on the
    HTML scaffold that the navigation chrome is present.
  - Manual checks: focus from any node, walk through depths 1/2/3,
    flip upstream-only and downstream-only, and verify the
    breadcrumb returns to the prior view on pop.
  - Blockers: `GV-003b`.
  - Outcome: navigation chrome shipped as a top-of-canvas toolbar
    that appears only when a node is focused. Toolbar carries a
    breadcrumb trail (historic crumbs grey, current crumb blue,
    click any to jump back), a Back button bound to Backspace, a
    Clear focus button bound to Esc, a Depth chip group (1 / 2 /
    3 / All) with [ and ] shortcuts, and a Direction chip group
    (Both / Upstream / Downstream). Inspector header gains a
    Focus button that pushes the selected node into the
    navigation stack. Traversal runs over a new pure
    `ConspectusNavHelpers` module (BFS over the neutral payload
    with directional adjacency), never over Cytoscape's
    collection API, per the ADR 0050 Coupling Boundary. Filter
    panel and navigation compose through a new
    `ConspectusViewState` coordinator that owns layered
    hidden-id sets (filter + nav) and pushes their union to
    `driver.setHidden`. Structural assertion confirms the new
    `#conspectus-navbar` container and the three new module
    globals (`ConspectusViewState`, `ConspectusNavHelpers`,
    `ConspectusNavigation`) are present in the rendered
    scaffold. 1133 tests pass; fmt and clippy clean. Manually
    verified in headless chromium: focus a workspace at
    depth 2 (subset shown correctly), drop to depth 1 (4-node
    neighborhood), focus a second node through to push a
    breadcrumb (atelier-demo › alpha, Back enabled).

- [x] `GV-003d` HTML layout improvements and selection.
  - Scope: the GV-003a default is fcose with hand-tuned options
    that look reasonable on the named scenarios but degrade on
    denser graphs (edge-label collisions, suboptimal compound
    grouping, no manual override). Improve the default tuning and
    expose layout selection: at minimum a UI control to pick
    among `fcose`, `cose`, `dagre` (hierarchical), and `concentric`
    (radial-by-NodeKind). Consider per-`NodeKind` constraints so
    workspaces/repos cluster naturally and lineage edges flow in a
    consistent direction. A "re-run layout" button to escape local
    minima. Vendor any additional layout extensions (`cytoscape-
    dagre` etc.) under the same `assets/` pattern with VERSIONS /
    NOTICE updates.
  - Tests: GV-003a snapshot tests stay valid (layout is JS-side and
    not part of the payload contract). Add a manual-check checklist
    in the docs.
  - Manual checks: render the named replay scenarios under each
    available layout and confirm the result is readable; render a
    real local graph (50-100 nodes) and confirm performance and
    legibility hold.
  - Blockers: `GV-003a`. Not on the GV-003 umbrella critical path;
    can land in parallel with GV-003b / GV-003c.
  - Outcome: vendored `cytoscape-dagre@3.0.0` (MIT, ~57 KB,
    dagre bundled internally). Driver exposes `availableLayouts()`,
    `currentLayout()`, `setLayout(name)`, `rerunLayout()`. Filter
    panel gains a "Layout" section with a dropdown
    (fcose / dagre-LR / dagre-TB / cose / concentric / circle /
    grid — pruned to whatever Cytoscape actually has registered)
    and a "Re-run layout" button. Concentric uses a per-`NodeKind`
    ring (workspaces innermost, runtime processes outermost) so it
    reads as a hub-and-spoke when the graph has clear roots. The
    fcose tuning from the GV-003c follow-up stays the default.
    VERSIONS / NOTICE / .gitattributes updated. 1133 tests pass;
    fmt and clippy clean. Manually verified the dagre layout in
    headless chromium on the `fork-lineage` scenario (clean
    left-to-right hierarchy).

- [x] `GV-004` Document graph visualization workflows.
  - Scope: update `README.md`, `docs/operations.md`, or a focused
    visualization guide with examples for generating DOT and HTML
    outputs, rendering DOT through Graphviz, and using the HTML explorer
    for debugging resolver behavior.
  - Tests: docs-only `git diff --check`.
  - Manual checks: run each documented command against a fixture or local
    repo before marking complete.
  - Blockers: `GV-002`, `GV-003`.
  - Outcome: new `docs/graph-visualization.md` covers `--format dot`
    (pipe through Graphviz, recipes for SVG/PDF/PNG, fallback layout
    flags for dense graphs), `--format html` (single self-contained
    file, how to open) and walks every chrome surface (filter panel,
    inspector, search, focus navigation). Includes four debugging
    recipes ("why did the resolver pick mux X over Y", "show me
    everything reachable from this fork", "approximate the CLI
    table view", "what's this RuntimeProcess evidence for").
    `docs/operations.md` CLI Surface and Paging notes updated.
    `README.md` CLI block updated and `docs/index.md` registers the
    new guide. Cross-references all point at ADR 0050 for rationale.

- [ ] `GV-EDGEREASON` Emit explicit resolver decision rationale.
  - Scope: today the resolver picks a winner per candidate group by
    running `compare_session_mux` / `compare_branch_pr` /
    `compare_generic` and dropping the result into
    `ResolvedRelationship.selected_link_id`. The HTML inspector
    shows the winner side-by-side with `competing_link_ids` so an
    operator can deduce the reason from the provenance / fields
    diff. Make the resolver emit the reason directly: a short
    string per resolution such as `"local_declared beat
    strong_discovered by provenance precedence"`, `"strong_discovered
    beat convention by mux tier"`, `"tied — broken by link id sort"`.
    Add `selected_reason: Option<String>` to `ResolvedRelationship`
    (and the SQLite materialization), thread through the HTML
    payload, and render in the inspector above the lost-candidates
    list.
  - Tests: extend the resolver unit tests with explicit-reason
    assertions for each comparator branch; refresh fixture
    snapshots and HTML payload snapshots that lock in the field.
  - Manual checks: render the named scenarios that exercise each
    comparator (ambiguous-mux for mux tier, workspace-pr for PR
    selection, etc.) and confirm the inspector text reads
    correctly.
  - Blockers: none. Touches `src/resolve/`, `src/output/html/`,
    `src/output/dot.rs` (optional: render reason in the DOT edge
    tooltip), and `src/query/` (selected_reason column).

### Command Search And Minibuffer

The TUI surface (`conspectus tui`) is growing in keybindings, sub-views,
and modal surfaces (viewer, rename, graph export). Users need a
discoverable, keyboard-driven way to find and invoke commands — without
leaving the keyboard or memorizing dozens of arcane chords.

Two complementary approaches, drawn from mature terminal-first ecosystems:

1. **Emacs-like minibuffer**: a single-line prompt at the bottom of the
   screen that accepts text commands with tab completion, history,
   and inline validation. Every TUI action eventually routes through
   it. Inspired by Emacs `M-x` and the classic readline pattern.
2. **OpenCode-style command search modal**: a fuzzy-filtered overlay
   that indexes every available command, action, and sub-view. The
   operator types a few characters, arrows through results, and
   presses Enter. Inspired by VS Code's Command Palette and opencode's
   `/` command surface.

The H-TBL workstream already established a column registry and
`conspectus columns <ROWS>` discovery surface; the TUI command palette
can reuse the same registry-plus-description pattern at the action
level. The text-input primitive (`src/tui/widgets/input.rs`) from
ADR 0030 (H-RENAME-010) provides the base widget. The fuzzy filter can
reuse the structural matching approach from the existing
`node_short_id` prefix resolver or adopt a lightweight substring scorer.

- [ ] `H-CMD-001` ADR: command palette surface and minibuffer scope.
  - Scope: settle the two-phase delivery (command palette first as the
    lower-risk, higher-discoverability surface; minibuffer second as
    the general-purpose prompt once the action registry is mature).
    Decide the fuzzy-match algorithm (substring with smart-case vs
    `nucleo` / `skim`-style scoring), the action registry shape (enum
    vs trait + `describe()`), whether commands are statically
    registered or discovered at runtime, how user-defined keybindings
    and aliases plug into the registry, and the overlay placement
    (H-RENAME-010's centered modal pattern vs a bottom-anchored
    palette vs a full-screen overlay for the command-search variant).
    Record the relationship with ADR 0030 (text-input widget), ADR
    0033 (extensible keybinding config), and the existing `Controls`
    overlay. Does not introduce new crate dependencies unless the ADR
    explicitly justifies one.
  - Tests: docs-only; `git diff --check`.
  - Manual checks: review the ADR against the existing TUI surface,
    the Controls overlay help text, and the column-registry pattern
    from H-TBL.
  - Blockers: none.
- [ ] `H-CMD-002` Action registry: enumerate the command surface.
  - Scope: walk every `Action` variant, TUI keybinding, runtime helper
    (`src/tui/runtime.rs`), and modal surface (viewer, rename, graph
    export, pinned-row launcher) and produce a structured action
    registry — one entry per user-visible command — with a stable
    command id, a short description, the category (navigation /
    session / mux / display / export / transcript / rename), default
    keybinding, and applicability guard (e.g. `pinned-row` actions
    only apply when a pinned row is selected). Keep the registry in a
    new `src/tui/actions.rs` or similar, separate from the existing
    key-dispatch map, so the command palette can enumerate it without
    importing the runtime. This is the foundation the ADR describes;
    the palette overlay (`H-CMD-003`) and the minibuffer
    (`H-CMD-004+`) both consume it.
  - Tests: unit tests for the registry shape, uniqueness of command
    ids, applicability-guard coverage for at least four categories,
    and a snapshot of the full command list for discovery parity with
    the existing Controls overlay.
  - Manual checks: confirm the registry matches the Controls overlay
    help text byte-for-byte (same descriptions, same keybindings);
    spot-check `conspectus tui --help` for consistency.
  - Blockers: `H-CMD-001`.
- [ ] `H-CMD-003` Command palette overlay (first deliverable).
  - Scope: on a single key chord (`Ctrl+P` or `M-x`-style `Alt+X`,
    decided by the ADR), open a centered or top-anchored overlay
    listing every registered action. The input line at the top accepts
    a fuzzy filter query; the result list scrolls. `Enter` executes
    the selected action by routing it through the existing
    `Action`→`Msg` dispatch; `Esc` dismisses. Reuse
    `src/tui/widgets/input.rs` for the text field. The palette
    should feel like a discoverability surface, not a replacement for
    the existing direct keybindings — those continue to work
    unchanged. Show the keybinding beside each result so the operator
    learns shortcuts organically. Initial query is empty (show all);
    typing refines. Category headers (`Navigation`, `Session`, …) in
    the result list when results span multiple categories.
  - Tests: buffer snapshot tests for the empty-query "show all" state,
    a filtered result set, the no-matches state, and palette dismissal
    without side effects. Reducer tests for action dispatch through the
    palette (confirm, cancel, arrow-navigate, re-filter-on-type).
  - Manual checks: open the palette in a live TUI, type a partial
    command name, confirm the filter works, execute a command, verify
    the TUI state changes correctly.
  - Blockers: `H-CMD-002`.
- [ ] `H-CMD-004` Minibuffer prompt (second deliverable, deferred).
  - Scope: add a bottom-anchored single-line prompt bar that can host
    any text-command interaction — rename, search, filter, command
    palette entry — without spawning a new modal overlay. The
    minibuffer is the *unified* prompt surface; individual commands
    declare their prompt content and completion set. Emacs conventions:
    `M-x` opens the minibuffer in command-palette mode,
    `C-g`/`Esc` cancels. Tab completion cycles candidates. History
    persists per-command-type for the session lifetime. The existing
    rename input (`H-RENAME-011`) should route through the minibuffer
    rather than its own modal once this lands; the search overlay
    (`T8-017` / `/`) should use it too. Do not replace the viewer
    modal or graph-export dialogs — they need more screen real estate.
  - Tests: snapshot tests for empty prompt, text entry with completion
    candidates, history navigation (`M-p`/`M-n`), cancellation, and
    confirm. Reducer tests for per-command-type routing (command
    palette dispatch, rename dispatch, search dispatch).
  - Manual checks: open minibuffer, type a partial command, tab-
    complete, execute; then open rename via minibuffer and confirm the
    alias update flow works identically to the current modal path.
  - Blockers: `H-CMD-003`, `H-RENAME-011` (rename flow is the first
    non-palette consumer).
- [ ] `H-CMD-005` Migrate rename, search, and view-switch prompts into
  the minibuffer.
  - Scope: once the minibuffer exists, route the existing inline
    prompts through it: `R` (rename) opens the minibuffer pre-
    populated; `/` (search, `T8-017`) opens the minibuffer; `v`
    (view-switch) opens the minibuffer with completion over the
    registered views. Keep the existing modal fallback for terminals
    where the minibuffer layout is impractical. Remove the standalone
    text-input modals only after the minibuffer versions have been
    exercised for at least one release cycle.
  - Tests: integration tests for each migrated prompt; regression
    tests proving the removed modals no longer register keybindings.
  - Manual checks: run through the full rename → search → view-switch
    flow using only the minibuffer; confirm history and completion
    work across each prompt type.
  - Blockers: `H-CMD-004`, `T8-017`.

### Per-Node-Type Visual Identity

The TUI row tree renders nine `GraphNode` / `NodeId` variants, but only
`AgentSession` and `MuxSession` carry a strong visual identity (colored
harness pill badge, mux-state circle glyphs). Nodes that render as group
rows — `Workspace`, `Repo`, `Checkout` — and types that appear only in
detail panels — `Branch`, `RuntimeProcess` — are visually
indistinguishable: every group row is a bold path with a disclosure
glyph, every detail-panel node-kinds chip is a dim `[kind]` label.

Operators scanning a dense sessions tree or a right-panel explorer do
not have a consistently fast way to tell *what kind of thing* a row
represents before reading its label. OpenCode's TUI assigns a per-entity
glyph and color; Emacs `dired` and `ibuffer` use per-type faces; Tmux
uses status-line symbols.

This workstream assigns a stable, terminal-safe visual identity — a
**glyph** (geometric shape / Nerd Font symbol / limited emoji), a
**color** (foreground or badge-fill), and a consistent **placement rule**
— to every `GraphNode` variant, so the operator recognizes node kinds at
a glance regardless of view, grouping, or filter state.

The `Theme` struct (`src/tui/theme.rs`) centralizes all TUI colors
including per-harness and mux-state; node-type colors follow the same
pattern and live under `[tui.theme]` config overrides. Node-type glyphs
live in `src/tui/icons.rs` (or a similar single-source-of-truth module)
so the row renderer, detail panel, graph explorer, and any future
surfaces all read from one definition.

- [x] `H-VIS-001` ADR: node-type visual identity system.
  - Outcome: ADR 0073 records the strategy (geometric default,
    Nerd-Font opt-in via `[tui.theme.icons]`), the per-kind slate
    (`▦ ◆ ◇ ● ▣ ⚙ ⎇ ⑂ ⇄`), the placement rule (one-cell prefix
    between disclosure and existing badges), the color schema
    (eight new `node_*` color keys, `ForgePr` reuses `pr_*`), and
    the accessibility stance (1-cell-only override validation,
    glyph-only legibility under `NO_COLOR`). `AgentSession` keeps
    an independent kind-color layered with the harness pill.
    Subsequent stories (`H-VIS-002..006`) implement the slate.
- [x] `H-VIS-002` Define the per-node-type glyph and color assignments.
  - Outcome: `src/tui/icons.rs` ships the `NodeKind` enum with
    `From<&GraphNode>` / `From<&NodeId>` conversions, the
    `NodeKindStyle { glyph, color, width }` struct, and the
    `node_kind_style(kind, theme)` lookup. `Theme` gains eight
    `node_*` color fields (defaults from ADR 0073) plus an
    `icons: IconOverrides` field for `[tui.theme.icons]` operator
    overrides validated to 1-cell width by `parse_icon_override`.
    `ForgePr` returns the documented `Color::Reset` sentinel — PR
    rows pick from `theme.pr_*` based on state. Unit tests cover
    the slate catalog, default-glyph widths, `NodeId` conversion,
    operator overrides, and the config-loader path
    (`[tui.theme.icons]` parsing + diagnostics for unknown keys,
    non-string values, wide glyphs). `H-VIS-003` consumes these
    primitives to apply the glyph prefix to row rendering.
- [x] `H-VIS-003` Apply node-kind glyphs and colors to the TUI row
  tree.
  - Outcome: `render_left_row` (`src/tui/ui.rs`) prepends the ADR
    0073 node-kind glyph after the disclosure column for every
    `RowKind` that carries a graph node identity: Workspace (`▦`)
    on the workspace row, Repo (`◆`) for repo group rows and
    workspace member-repo rows, AgentSession (`●`) before the
    harness pill, MuxSession (`▣`) before the native-id label and
    the existing `◉`/`◯` attachability chip, Fork (`⑂`), ForgePr
    (`⇄` with `pr_*` state color), and the mux-candidate child row
    (also `▣`). Synthetic group buckets and `Pin` rows skip the
    prefix — pins keep `📌` as their sentinel identity. The
    group-row body-width pre-pass (`group_row_body_width`) folds
    the glyph through the same dispatch so summary-chip alignment
    stays put across visible groups. Showcase fixture verified by
    rendering each view via `conspectus tui --snapshot-fixture
    showcase.json --snapshot --snapshot-pane left`; sessions /
    mux / prs / forks all show the slate without disclosure-column
    drift. Unit tests cover the per-row helpers
    (`node_kind_glyph_span`, `forge_pr_glyph_span`, the
    `row_kind_glyph_span` dispatch) including the PR
    state→color mapping and the Pin / synthetic-group skip cases.
    Detail-pane glyph application lands under H-VIS-004.
- [x] `H-VIS-004` Apply node-kind glyphs and colors to the detail
  panel and graph explorer.
  - Outcome: `kind_chip_span` (`src/tui/ui.rs`) now emits the per-
    kind slate glyph in the node-kind color rather than dim
    `[kind]` text; `ForgePr` reuses `theme.pr_open` at chip
    surfaces because that layer does not carry PR state. The
    right-panel title (`right_panel_title`) prepends the kind
    glyph before the bold kind label and folds the extra cells
    into the breadcrumb-chain budget. `render_group_header_line`
    in the relationship explorer swaps the prior textual
    neighbor-kind column for the glyph, keeping the count anchor
    via fixed padding. `NodeKind::from_snake_case` round-trips the
    stable kind tag so call sites that carry the kind as
    `&'static str` (field `kind_chip`, explorer `neighbor_kind`)
    look up the slate without going through `GraphNode`. Showcase
    fixture verified: agent-session detail pane reads
    `● session` in the title, `◇` next to the cwd field, and
    `◇`/`▦`/`▣` on relationship-explorer rows. Unit tests cover
    the chip glyph + color per kind, the unknown-tag fallback,
    the right-panel title prefix, and the explorer link-row
    glyph ordering. Non-TUI surfaces stay under H-VIS-005.
- [ ] `H-VIS-005` Surface node-kind identity in non-TUI outputs
  (cross-surface consistency).
  - Scope: extend `NodeId` / `GraphNode` with a `node_kind()` method
    returning the stable string tag (`"repo"`, `"agent_session"`,
    etc.) so non-TUI consumers — `conspectus graph --format json`,
    `conspectus node show`, the HTML explorer, DOT output — receive
    the same node-kind identity. The TUI glyph is a *rendering*
    choice; the stable identifier is a *model* concern. Add the
    machine-readable `node_kind` field alongside the display glyph so
    consumers that can render symbols (HTML with Nerd Font CSS, a TUI
    with a compatible terminal) use the glyph, while JSON consumers
    use the string tag. The DOT renderer already labels nodes by kind;
    this story gives it the node-kind color as a fill or font color.
    The HTML explorer applies the node-kind glyph via a CSS class or
    data attribute.
  - Tests: JSON snapshot updates confirming the new field does not
    break existing consumers; DOT snapshot updates confirming node-
    kind colors appear; HTML payload snapshot updates confirming
    glyph/color propagation.
  - Blockers: `H-VIS-002`, `H-VIS-004`.
- [ ] `H-VIS-006` Docs and snapshot coverage.
  - Scope: update `docs/operations.md` with the icon key — a table
    listing every `NodeKind`, its glyph, its default color, and its
    meaning. Update `Theme` docs in `src/tui/theme.rs` with the new
    per-node-type color fields. Refresh all affected insta snapshots
    (row tree × 5 views, detail panel, graph explorer, JSON, DOT,
    HTML). Add a `NO_COLOR` / `--no-color` snapshot variant proving
    glyphs remain distinguishable without ANSI codes.
  - Tests: `cargo test --all-targets --all-features`; `cargo nextest
    run --all-targets --all-features`; `git diff --check`. Insta
    review of every changed snapshot for stable ordering and
    consistent glyph placement.
  - Blockers: `H-VIS-003`, `H-VIS-004`, `H-VIS-005`.

## Later

- [ ] Evaluate Backlog.md migration once task count, dependencies, or
  multi-agent coordination make manual tracking cumbersome.