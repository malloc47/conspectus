# Conspectus Implementation Plan

This directory catalogs the implementation phases for Conspectus. Each phase is
intended to be refinable into smaller units of work while preserving the
end-state behavior, test strategy, and manual checks for that milestone.

## Phase Order

1. [Phase 00: Project Foundation](phase-00-project-foundation.md)
2. [Phase 01: Core Graph JSON](phase-01-core-graph-json.md)
3. [Phase 02: Local Discovery](phase-02-local-discovery.md)
4. [Phase 03: Agent And Mux Discovery](phase-03-agent-mux-discovery.md)
5. [Phase 04: Forge And Table Views](phase-04-forge-and-table-views.md)
6. [Phase 05: Declared Links](phase-05-declared-links.md)
7. [Phase 06: Atelier Delegation](phase-06-atelier-delegation.md)

## Defaults Chosen

- First vertical slice: JSON graph output.
- Early sharing strategy: copy or port small pure Atelier pieces into
  Conspectus-native interfaces, then extract shared crates only after the seams
  stabilize.
- Work tracking: keep `docs/backlog.md` as the interim source of truth until
  the work volume justifies a structured tracker.
- Implementation shape: Rust 2024, library-first CLI, following ADR 0007.
