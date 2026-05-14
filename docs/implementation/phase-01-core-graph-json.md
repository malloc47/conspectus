# Phase 01: Core Graph JSON

## Summary

Deliver the first real vertical slice: a CLI command that emits a deterministic
machine-readable graph. Discovery can be empty or fixture-backed at this stage,
but the core model and resolver contracts must be in place.

## End-State Behavior

- `conspectus graph --format json` emits a schema-stable graph document.
- Sparse graphs are valid: orphan sessions, mux-only rows, repo-only rows, and
  unresolved lineage evidence can all be represented.
- Candidate evidence and resolved relationships are separate in output.
- The resolver applies initial precedence rules without deleting lower-priority
  evidence.

## Implementation Changes

- Implement structured node IDs from ADR 0001 for `Repo`, `Worktree`,
  `Workspace`, `AgentSession`, `MuxSession`, `Branch`, `Fork`, and `ForgePr`.
- Implement typed nodes, `GraphLink`, relation kinds, provenance, confidence,
  freshness, source metadata, and ignored or overridden state.
- Implement a resolver skeleton that accepts GraphLink candidates and emits
  typed resolved relationships.
- Apply default precedence: local declared, global declared, strong discovered
  evidence, convention, then cached evidence.
- Implement deterministic JSON serialization with top-level `nodes`,
  `candidate_links`, `resolved_relationships`, and `diagnostics`.

## Tests

- Unit tests for ID construction and serialization stability.
- Unit tests for relation-kind serialization and round trips.
- Table-driven resolver tests for sparse links, conflicts,
  declared-over-discovered precedence, unresolved session endpoints, and mux
  candidate precedence.
- Snapshot tests for empty graph JSON and representative sparse graph fixtures.

## Manual Checks

```sh
cargo run -- graph --format json
cargo test --all-targets --all-features
cargo nextest run --all-targets --all-features
```

Inspect the JSON manually and confirm that key ordering is deterministic and
the output distinguishes raw candidate links from resolved relationships.

## Assumptions

- JSON is the primary public interface in this phase; human tables wait until
  later.
- Fixture builders can be internal test helpers rather than a public API.
