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

## Phase 0: Planning

- [x] `P0-001` Convert `docs/design.md` into implementation phases and
  milestone-level stories.
  - Blockers: none.
- [x] `P0-002` Identify the first vertical slice for the Rust crate and CLI.
  - Blockers: `P0-001`.
- [x] `P0-003` Define the fixture strategy for sparse graph and resolver tests.
  - Blockers: `P0-001`.
  - Outcome: see `docs/implementation/`; first vertical slice is JSON graph
    output with sparse graph and resolver fixtures.

## Phase 1: Foundation

- [ ] `P1-001` Scaffold the Rust workspace with library-first CLI structure.
  - Blockers: `P0-002`.
- [ ] `P1-002` Implement core graph node, relation, evidence, and source
  metadata types.
  - Blockers: `P1-001`.
- [ ] `P1-003` Add serialization fixtures for machine-readable graph output.
  - Blockers: `P1-002`, `P0-003`.

## Later

- [ ] Evaluate Backlog.md migration once task count, dependencies, or
  multi-agent coordination make manual tracking cumbersome.
