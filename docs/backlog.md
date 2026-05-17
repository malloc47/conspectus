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
    worktree / branch / agent_session / mux_session nodes plus 14
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
    rooted in a discovered repo/workspace/worktree, global for orphan or
    user-wide relationships, and never in cache/index storage. Reuse the
    config walk rules from ADR 0012.
  - Tests: unit tests for repo-rooted, workspace-rooted,
    worktree-rooted, branch/PR-rooted, mux-only, orphan-agent,
    multi-root, missing-root, and outside-home scenarios.
  - Manual checks: inspect selected paths for representative repos,
    linked worktrees, and non-repo directories.
  - Blockers: `P5-002`, `P5-003`.
  - Outcome: added a pure `select_store_for_declaration` helper that
    resolves declared-link writes to the nearest project config for
    repo, workspace, worktree, session cwd, mux cwd, branch/PR, and
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
    `MuxSession`, `ForgePr`, `Workspace`, `Repo`, `Worktree`,
    `Branch`, and `Fork`), using nearest-store selection by default.
    Link creation should not delete discovered evidence.
  - Tests: CLI integration tests for session↔mux, branch↔PR,
    workspace/repo/worktree/fork relationships, global orphan links,
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
    builds an Atelier-style workspace with two git repos, a worktree
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
- [ ] `H-OBS-002` Add `conspectus node show <id>`.
  - Scope: a read-only command that prints every candidate link, resolved
    relationship, source metadata, and diagnostic touching a given node
    id. Useful for users debugging "why is this session orphaned?".
  - Tests: CLI integration tests against the existing fixtures.
  - Blockers: none.
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

- [ ] `H-DIST-001` Add a GitHub Actions CI workflow.
  - Scope: ADR 0016 commits to crates.io distribution but there is no
    `.github/workflows/` directory and the only check automation is local
    (`justfile`, `flake.nix`). Add a CI job that runs
    `cargo fmt --check`, `cargo clippy -- -D warnings`,
    `cargo test --all-targets --all-features`, and
    `cargo nextest run --all-targets --all-features` on PRs and main.
  - Tests: CI run on the change itself.
  - Blockers: none.
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

- [ ] `H-DESIGN-001` Settle the workspace-detection threshold and provider
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
- [ ] `H-LINEAGE-005` Surface session lineage in the session table.
  - Scope: add a compact `LINEAGE` (or `PARENT`) column to the agent
    projection that shows the immediate parent session's short id when
    one exists, and a chain marker (`←`) for sessions with a longer
    ancestry. Decide whether `conspectus session` should default to
    hiding "leaf-only" rows (sessions superseded by a known
    successor) behind a `--include-superseded` flag. Keep the JSON
    output exhaustive.
  - Tests: snapshot tests for one-level and multi-level chains, plus
    a `--include-superseded` flag toggle.
  - Blockers: `H-LINEAGE-002` (so there's lineage to show);
    `H-OBS-003` should land first if it's already in flight, since
    these are the same renderer.

### Agent-Deck Multi-Repo Workspace Support

agent-deck creates per-session directories under
`~/.agent-deck/multi-repo-worktrees/<id>/` whose immediate children are
symlinks into one or more real git repos (e.g. `cb355de8/atelier ->
/home/malloc47/src/atelier` and `cb355de8/conspectus ->
/home/malloc47/src/conspectus`). The agent runs from inside the combined
directory and operates across every linked repo. Today Conspectus shows
the combined directory as the session `CWD` and produces no link to the
participating repos, so a row like
`codex:… /home/malloc47/.agent-deck/multi-repo-worktrees/cb355de8` hides
the fact that the session is working on both atelier and conspectus.

- [ ] `H-AGENTDECK-001` Detect agent-deck multi-repo worktrees as a
  workspace provider.
  - Scope: add a workspace discovery adapter that recognizes
    `~/.agent-deck/multi-repo-worktrees/<id>/` (path location +
    immediate-child symlinks resolving to git common dirs) and emits a
    `Workspace` node (with an `agent-deck` provider identifier and a
    short label derived from `<id>`) plus `Workspace`→`Repo` membership
    candidate links for each resolved symlink. Sessions and mux sessions
    rooted at the worktree path should associate with the workspace via
    the existing cross-link inference. Treat the symlink target's
    canonical git common dir as the `Repo` identity so existing repo
    nodes from other scan roots merge cleanly.
  - Tests: fixture tests for a multi-repo worktree with two symlinks,
    one symlink, broken symlinks, non-symlink children (skip), and a
    nested directory layout. Snapshot test for the session table
    confirming the workspace shows up and the participating repos are
    listed somewhere reachable from the agent row.
  - Manual checks: `cargo run -- graph --format json` from inside a
    real agent-deck worktree; `cargo run -- session` and confirm the
    new workspace/repo links appear.
  - Blockers: `H-DESIGN-001` (workspace-provider precedence — agent-deck
    workspaces should not conflict with generic-workspace inference
    over the same path).
- [ ] `H-AGENTDECK-002` Surface multi-repo participants in the session
  table.
  - Scope: extend the agent projection so the `CWD` column (or a new
    "REPOS" column) shows the participating repo set when the session
    is rooted in an agent-deck workspace. Decide whether to replace the
    cwd with a short repo list (`atelier+conspectus`) or add a separate
    column; either way preserve byte-stable ordering.
  - Tests: snapshot tests for one-repo, two-repo, and many-repo
    workspaces.
  - Blockers: `H-AGENTDECK-001`, `H-OBS-006` (recency column work will
    touch the same renderer).
- [ ] `H-AGENTDECK-003` Read agent-deck profile state from `state.db`.
  - Scope: agent-deck stores richer per-session metadata
    (`~/.agent-deck/profiles/<profile>/state.db`, SQLite) including
    active sessions, profile, and likely the
    session-to-multi-repo-worktree mapping. Once the on-disk
    convention adapter from `H-AGENTDECK-001` covers the basic graph
    shape, read the SQLite state for any links that can't be inferred
    from filesystem layout alone (e.g. session-to-worktree binding
    when the agent has moved cwd, or labels/tags). Reuse the
    `rusqlite` dependency added by ADR 0013.
  - Tests: fixture tests over a temp SQLite file populated with
    representative rows; missing-database degradation; malformed
    schema degradation.
  - Manual checks: confirm read-only access; confirm the adapter does
    not lock the database while agent-deck is running.
  - Blockers: `H-AGENTDECK-001`; requires inspecting agent-deck's
    schema and recording the relevant tables/columns in an ADR (or an
    extension of ADR 0013) before introducing the read code.

## Later

- [ ] Evaluate Backlog.md migration once task count, dependencies, or
  multi-agent coordination make manual tracking cumbersome.
