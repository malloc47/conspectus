# ADR 0007: Rust Development Approach

## Status

Accepted

## Context

Conspectus is a graph-first CLI whose core behavior is deterministic: discover
evidence, build `GraphLink` candidates, resolve typed relationships, and render
human-readable and machine-readable graph views. The implementation should
support a data-model-first workflow, strong test coverage around resolver
behavior, fast local iteration, and a complete SDLC from the start.

Atelier was built in Rust and has already shown that Rust works well for this
problem space: fast single-binary CLI distribution, filesystem and process
integration, strong typed models, and reasonable build performance.

The project also needs room to share pure discovery/parsing/model code with
Atelier without depending on Atelier command modules directly.

## Decision

Build Conspectus in Rust 2024 as a library-first CLI crate.

Recommended source layout:

- `src/lib.rs`: core model, discovery traits, resolver, and renderers
- `src/main.rs`: thin CLI entrypoint
- `src/model/`: typed nodes, IDs, `GraphLink`, and relation kinds
- `src/resolve/`: candidate-link resolution
- `src/discovery/`: adapters for git, harnesses, tmux, forge, and workspace
  providers
- `src/output/`: table and machine-readable output

Use a Conspectus-native graph model as the primary representation:

- stable typed IDs from ADR 0001
- typed nodes
- `GraphLink` candidates from ADR 0002
- resolver output as typed relationships and views

Do not make a generic graph crate the durable model. Use graph libraries only as
internal helpers when they materially simplify traversal, connected components,
lineage walks, or diagnostics.

Initial crate choices:

- CLI: `clap`
- serialization/config: `serde`, `serde_json`, `toml`, `toml_edit`
- deterministic maps/sets: `indexmap`
- graph algorithms: evaluate `petgraph` as an internal helper only when needed
- git discovery: start by shelling out to `git` for parity with Atelier and
  fewer library edge cases; evaluate `gix` later for deeper read-only git
  inspection
- errors/logging: `anyhow` for application errors, `thiserror` where typed
  library errors help, and `tracing` or `log` for diagnostics

Testing approach:

- Use TDD for model and resolver behavior.
- Write unit tests for IDs, relation kinds, precedence, and resolver rules.
- Write table-driven tests for graph resolution scenarios.
- Use snapshot tests for JSON and table output.
- Use CLI integration tests with temporary files, repos, and workspaces.
- Add property tests for resolver invariants where useful.

Recommended testing crates/tools:

- `assert_cmd`
- `predicates`
- `tempfile`
- `insta`
- `rstest`
- `proptest`
- `cargo-nextest`

SDLC baseline:

- Add `rust-toolchain.toml` to pin the stable Rust toolchain.
- Add a `justfile` for discoverable local and CI entrypoints.
- Add git-hook integration with `pre-commit`.
- Add CI entrypoints for formatting, linting, tests, and diff checks.

Baseline checks:

```sh
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo nextest run --all-targets --all-features
git diff --check
```

Optional later checks:

- `cargo deny` for licenses and advisories
- `cargo audit` if not covered by `cargo deny`
- `taplo` for TOML formatting
- `typos` for docs and code spelling

## Consequences

- Rust gives Conspectus a fast, strongly typed, single-binary implementation
  path that aligns with Atelier.
- A library-first crate keeps the CLI thin and makes core model/resolver code
  easier to test and potentially share.
- The Conspectus-native graph model keeps durable IDs, provider metadata,
  candidate links, and resolved relationships under project control.
- Starting with shell-based git discovery reduces early risk and keeps behavior
  close to Atelier; deeper library-backed git support can be adopted later.
- TDD and snapshot tests fit the resolver/output problem well because small
  fixtures can exercise precise graph behavior.
- `cargo-nextest`, `pre-commit`, and CI checks provide a complete feedback loop
  before the codebase grows.
- The toolchain introduces a moderate amount of setup, but it pays for itself
  by keeping graph semantics and CLI behavior stable during phased development.

## Alternatives Considered

- Build in Python or TypeScript. Rejected because they would reduce type-safety
  around the core graph/resolver and would not match Atelier's implementation
  path or single-binary distribution story.
- Use `petgraph` as the primary durable graph model. Rejected because
  Conspectus needs typed durable IDs, provider metadata, candidate evidence, and
  resolver semantics that should not be coupled to generic graph indexes.
- Use `git2` or `gix` immediately for all git discovery. Deferred because
  shelling out to `git` gives quicker parity with Atelier and fewer early
  semantic surprises. `gix` remains a strong later option for read-only git
  inspection.
- Rely only on `cargo test`. Rejected because `cargo-nextest` provides faster
  and clearer test execution, especially as integration tests grow.
- Skip hooks and rely only on CI. Rejected because formatting, linting, and
  basic checks should fail before bad changes are committed.

## Open Questions Answered

- Conspectus should be implemented in Rust 2024 unless a later constraint
  materially changes the trade-off.
- The codebase should be library-first with a thin CLI.
- The durable graph model should be Conspectus-native, not a direct `petgraph`
  model.
- TDD is appropriate for resolver and output behavior.
- CI and local hooks should enforce formatting, linting, tests, and diff checks
  from the start.
