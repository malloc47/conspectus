# Phase 04: Forge And Table Views

## Summary

Add GitHub PR association and the first human-readable session views. JSON
remains the graph API; tables are projections over the resolved graph.

## End-State Behavior

- Conspectus associates GitHub PRs with discovered repo and branch pairs.
- `conspectus session` renders the default agent-oriented table.
- `conspectus session --projection agent|mux|union` selects the table
  projection.
- Ambiguous PR and mux relationships are visible instead of silently collapsed.

## Implementation Changes

- Add a GitHub forge adapter, initially using `gh` for auth and network
  behavior.
- Add `ForgePr` nodes and GraphLinks from branches or worktrees to PRs.
- Add config loading for `[session] projection = "agent" | "mux" | "union"`.
- Add table renderers for agent, mux, and union projections.
- Include compact provenance, confidence, and ambiguity indicators in table
  output.

## Tests

- Forge adapter tests with mocked `gh` output.
- Resolver tests for zero, one, and multiple PRs per branch.
- Snapshot tests for table output in agent, mux, and union projections.
- Snapshot tests for graph JSON containing ForgePr nodes and PR links.
- CLI tests for projection flags and config defaulting.

## Manual Checks

```sh
cargo run -- graph --format json
cargo run -- session
cargo run -- session --projection agent
cargo run -- session --projection mux
cargo run -- session --projection union
```

Run from a repo branch with an open GitHub PR and confirm the PR node appears
in JSON and the table projection surfaces the PR relationship.

## Assumptions

- `gh` is the first implementation path because the development shell already
  provides it and it delegates auth to existing user setup.
- Non-GitHub forge providers remain out of scope for this phase.
