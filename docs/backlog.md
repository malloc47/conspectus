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

- [ ] `H-TRANSCRIPT-001` ADR: terminal markdown rendering for the
  inline transcript preview.
  - Scope: record a follow-on ADR (per ADR 0024) selecting the
    markdown rendering path for the TUI. Compare `tui-markdown`
    (Ratatui-native, returns `ratatui::text::Text`), `termimad`
    (crossterm-targeted, requires bridging), and a roll-your-own
    minimal styler over `pulldown-cmark`. Decide on adoption
    criteria including: crate maintenance posture, syntect/
    `highlight-code` feature use, license, supply-chain cost,
    and integration shape inside the existing `src/tui/` module.
  - Tests: ADR text only; no code in this story.
  - Blockers: none. Should land before `H-TRANSCRIPT-008` adds
    the dep.

- [ ] `H-TRANSCRIPT-002` Resolve ADR 0019 with the May 2026 survey.
  - Scope: move ADR 0019 from Proposed to Accepted (or amend it
    in place) with the updated candidate status: `ccview`
    disappeared; `claude-history`, `recall`, `ai-dash`,
    `lazyagent`, and `ccboard` are all active; `lazyagent`
    gained an HTTP API; `ccboard-core` is a candidate Rust
    library; `coding_agent_session_search` is new and covers
    20+ providers via `--json`. Narrow the `SessionViewer`
    trait sketch to the integration shapes that current
    candidates actually expose, and call out that the inline
    preview is a separate concern handled by the rest of this
    workstream rather than by `SessionViewer`.
  - Tests: ADR text only.
  - Blockers: none.

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

- [ ] `H-TRANSCRIPT-008` Add `tui-markdown` dependency.
  - Scope: add the crate selected in `H-TRANSCRIPT-001` to
    `Cargo.toml`. Decide on the `highlight-code` feature (and
    whether the syntect cost is worth it for the preview
    pane). Add the crate to the workspace lints/audit
    allow-list if applicable. No usage yet — this story
    isolates the dep change for review.
  - Tests: `cargo build` and `cargo clippy --all-targets
    --all-features -- -D warnings`.
  - Blockers: `H-TRANSCRIPT-001`.

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

- [ ] `H-TRANSCRIPT-012` External full-transcript viewer launch
  (optional follow-on).
  - Scope: per the refreshed ADR 0019, add a keybind that
    launches an external viewer (`claude-history` for Claude
    Code, `recall` for multi-harness) as a child process and
    hands control to it, the same way `P8-010` hands control
    to `tmux attach-session`. Discover the binary on `PATH`;
    surface a disabled-action reason when no viewer is
    available for the selected harness. This is a separate
    action from the inline preview, not a replacement.
  - Tests: action-resolver tests covering supported /
    unsupported harnesses and the no-binary-on-PATH path.
  - Blockers: `H-TRANSCRIPT-002`, `H-TRANSCRIPT-010`.

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

- [ ] `H-AGENTMUX-001` Audit each candidate orchestrator's evidence
  against MUXPROC and decide which adapters to build.
  - Scope: with `H-MUXPROC-002` landed, enumerate for each candidate
    tool (agent-deck, dmux, workmux, agent-of-empires) exactly what
    evidence it produces beyond what MUXPROC already covers. For each
    tool, classify findings into: (a) workspace composition the
    process tree cannot see (e.g. multi-repo combined directories),
    (b) container-isolated agents whose host process tree shows only
    the runtime, (c) exited / paused / pre-spawn sessions, (d)
    orchestrator-specific labels / lineage / profiles. Tools whose
    evidence is fully a MUXPROC subset should be closed as won't-do.
    For the survivors, design a single `AgentMuxAdapter` trait that
    carries the surviving evidence types as provider-neutral
    candidate links + `SourceMetadata.fields`. Record findings and
    the trait shape as an ADR per CLAUDE.md.
  - Tests: none directly; ADR + go/no-go decisions per tool are the
    deliverable. A scaffold trait may land alongside as a compile
    check.
  - Blockers: `H-MUXPROC-002` (cannot audit "non-overlapping" until
    MUXPROC exists). `H-REF-004` is friendlier to settle first if
    both are in flight, since the runner seam may inform the adapter
    surface.

- [ ] `H-AGENTMUX-002` Detect agent-deck multi-repo checkouts as a
  workspace provider.
  - Scope: implement the first concrete `AgentMuxAdapter` for
    agent-deck. **Justification vs MUXPROC:** the unique evidence is
    workspace composition — the process tree shows
    `cwd=~/.agent-deck/multi-repo-worktrees/<id>/` but cannot reveal
    that the directory is composed of N repo symlinks. The
    pane ↔ harness link itself is redundant with MUXPROC. Recognize
    `~/.agent-deck/multi-repo-worktrees/<id>/` (path location +
    immediate-child symlinks resolving to git common dirs) and emit a
    `Workspace` node (with an `agent-deck` provider identifier and a
    short label derived from `<id>`) plus `Workspace`→`Repo`
    membership candidate links for each resolved symlink. Sessions
    and mux sessions rooted at the checkout path should associate
    with the workspace via the existing cross-link inference. Treat
    the symlink target's canonical git common dir as the `Repo`
    identity so existing repo nodes from other scan roots merge
    cleanly. Do not emit pane ↔ harness evidence from this adapter —
    leave that to MUXPROC.
  - Tests: fixture tests for a multi-repo checkout with two symlinks,
    one symlink, broken symlinks, non-symlink children (skip), and a
    nested directory layout. Snapshot test for the session table
    confirming the workspace shows up and the participating repos
    are listed somewhere reachable from the agent row.
  - Manual checks: `cargo run -- graph --format json` from inside a
    real agent-deck checkout; `cargo run -- session` and confirm the
    new workspace/repo links appear.
  - Blockers: `H-AGENTMUX-001` (must survive the audit), `H-DESIGN-001`
    (workspace-provider precedence — agent-deck workspaces should not
    conflict with generic-workspace inference over the same path).

- [ ] `H-AGENTMUX-003` Surface multi-repo participants in the session
  table.
  - Scope: extend the agent projection so the `CWD` column (or a new
    "REPOS" column) shows the participating repo set when the session
    is rooted in an agent-deck workspace (or any future adapter that
    emits a multi-repo `Workspace`). Decide whether to replace the
    cwd with a short repo list (`atelier+conspectus`) or add a
    separate column; either way preserve byte-stable ordering.
  - Tests: snapshot tests for one-repo, two-repo, and many-repo
    workspaces.
  - Blockers: `H-AGENTMUX-002`, `H-OBS-006` (recency column work will
    touch the same renderer).

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
7. Do `H-MUXPROC-004` only after a schema audit proves stable
   read-only state fields. It is useful for Codex/opencode and future
   Claude state, but it has more locking/schema risk than hooks or
   file-activity correlation.
8. Land `H-MUXPROC-008` as soon as the ADR path is open, or fold it
   into `H-MUXPROC-001` if that ADR is still being written. This keeps
   terminal injection and slash-command probing out of the attribution
   design while the tempting `/usage` workaround is fresh.
9. Leave `H-MUXPROC-006`, `H-MUXPROC-007`, `H-MUXPROC-013`, and
   `H-MUXPROC-014` behind their audits and schema decisions. They
   improve cross-harness correctness, but they are not the shortest
   path to fixing the Claude Code mapping drift seen in
   `H-MUXPROC-015`.

- [ ] `H-MUXPROC-001` ADR: process-tree linker design and dependency
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

- [ ] `H-MUXPROC-002` Implement the process-tree linker as a
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

- [ ] `H-MUXPROC-003` Add read-only session-file activity correlation.
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

- [ ] `H-MUXPROC-004` Read harness state databases for live-session
  hints without mutating logs.
  - Scope: for harnesses that maintain sqlite or other indexed state,
    add read-only queries that can strengthen session ↔ mux
    attribution. Codex currently opens `state_*.sqlite` and
    `logs_*.sqlite`; opencode has a session database; future Claude
    Code state may expose a durable index. The provider should open
    databases in read-only mode, avoid long-lived locks, and extract
    only stable fields such as session id, cwd, last activity, parent
    session, and any pid / terminal / server binding if present.
    The output should refine existing candidates; it must not become
    the only source of session discovery.
  - Tests: temp sqlite fixtures for each supported schema, missing
    database degradation, unknown schema degradation, read-only lock
    behavior, and resolver tests proving database evidence ranks
    above cwd-only but below direct fd evidence.
  - Manual checks: run against live Codex and opencode state stores
    while sessions are active; confirm no write-ahead-log churn is
    introduced by Conspectus.
  - Blockers: schema audit per harness; ADR required before any new
    persistent schema dependency or long-running read strategy is
    introduced.

- [ ] `H-MUXPROC-005` Audit harness control planes for non-mutating
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

- [ ] `H-MUXPROC-006` Add Codex app-server attribution adapter if
  the audit proves a stable non-mutating query.
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

- [ ] `H-MUXPROC-007` Add opencode server/ACP attribution adapter if
  the audit proves a stable non-mutating query.
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

- [ ] `H-MUXPROC-008` Document terminal-injection attribution as a
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

- [ ] `H-MUXPROC-009` Audit harness hooks/plugins as definitive
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

- [ ] `H-MUXPROC-010` Define Conspectus hook sidecar schema and
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

- [ ] `H-MUXPROC-011` Implement hook-sidecar discovery provider.
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

- [ ] `H-MUXPROC-012` Add Claude Code hook sidecar emitter if audit
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

- [ ] `H-MUXPROC-015` Fix Claude Code mux attribution after
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

- [ ] `H-MUXPROC-013` Add Codex hook sidecar emitter if audit proves
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

- [ ] `H-MUXPROC-014` Add opencode plugin/server sidecar emitter if
  audit proves non-mutating session identity.
  - Scope: if opencode's plugin, server, ACP, or attach surfaces can
    expose the active session id without mutating session logs, build
    an opt-in sidecar emitter. Prefer a documented plugin/server API
    over shell wrappers. The emitter should record active session id,
    project directory, pid/server id, and any attach URL or socket
    metadata needed to correlate back to a mux pane.
  - Tests: fake plugin/server payload tests, sidecar generation
    tests, multiple-project/session ambiguity tests, and degradation
    when the plugin is absent.
  - Manual checks: run opencode with the plugin/server extension and
    confirm no transcript records are added by the Conspectus
    attribution path.
  - Blockers: `H-MUXPROC-009`, `H-MUXPROC-010`.

## Phase 7: Continuous Operation And Snapshot Persistence

Source plan: pending; this section is the workstream skeleton. See
`docs/design.md` sections "Continuous Operation Mode" and "Graph
Snapshot Persistence" for the high-level model. Phase goal: take
Conspectus from a pure one-shot CLI to a tool that can persist its
graph between invocations and optionally maintain it live in a
long-running server.

Dependency shape inside the phase:

```
P7-001 (snapshot ADR) ────┐
                          ├──→ P7-003 (warm-start save/load) ─┐
P7-002 (provenance/       │                                   │
        freshness model)──┤──→ P7-005 (partial eviction) ─────┤
                          │                                   │
P7-004 (server ADR) ──────┴───────────────────────────────────┴──→ P7-006 (serve) ──→ P7-007 (CLI ↔ server)
                                                                                 ├──→ P7-008 (status/inspection)
                                                                                 └──→ P7-009 (event-driven, stretch)
```

P7-001 and P7-004 can land in parallel. P7-002 is foundational and
should land before any persistence or eviction code.

- [ ] `P7-001` ADR: graph snapshot persistence format and lifecycle.
  - Scope: settle the on-disk snapshot format and lifecycle. Concrete
    decisions: (a) the versioned JSON shape (resolved `GraphSnapshot`
    plus schema version plus per-provider freshness map), (b) the
    location under `$XDG_DATA_HOME/conspectus/snapshots/` and the
    naming / rotation policy, (c) atomic-write semantics
    (temp-file + rename within the same directory), (d) the schema-
    version migration policy (drop and rebuild vs in-place upgrade vs
    field-by-field), (e) the `--no-cache` / `--refresh` CLI flag
    surface and their interaction with the warm-start path. Record
    as a new ADR under `docs/adr/`.
  - Tests: none directly; ADR is the deliverable. A scaffold
    serializer / deserializer pair may land alongside as a compile
    check that the chosen shape round-trips.
  - Blockers: none.

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
  - Scope: settle the architecture of `conspectus serve`. Concrete
    decisions: (a) CLI ↔ server transport — Unix domain socket at
    `$XDG_RUNTIME_DIR/conspectus/server.sock`, file-based snapshot
    polling, or both — and the protocol (line-delimited JSON,
    length-prefixed, or a small framing layer), (b) whether the
    server reuses the one-shot discovery code path 1:1 or forks
    into a coordinator with its own concurrency primitives, (c) the
    `[server]` and `[server.intervals]` TOML config shape and
    per-provider interval defaults, (d) provider failure isolation
    (per-provider back-off, surfaced via diagnostics), (e) server
    lifecycle expectations (user-managed; no auto-spawn from CLI;
    documented systemd / launchd integrations later). Record as a
    new ADR under `docs/adr/`.
  - Tests: none directly; ADR is the deliverable.
  - Blockers: none (parallel to `P7-001`).

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

## Later

- [ ] Evaluate Backlog.md migration once task count, dependencies, or
  multi-agent coordination make manual tracking cumbersome.
