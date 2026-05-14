# Phase 06: Atelier Delegation

## Summary

Move overlapping observability behavior out of Atelier once Conspectus has
stable graph, discovery, JSON, table, and declared-link behavior.

## End-State Behavior

- Atelier can delegate or deprecate session, mux, forge, and graph-heavy
  status surfaces.
- Shared code extraction is based on proven Conspectus interfaces.
- Existing Atelier workflows remain compatible or fail with intentional
  migration guidance.

## Implementation Changes

- Identify stable pure modules for extraction after Conspectus behavior is
  tested: harness parsers, tmux listing, Atelier metadata readers, and git
  probes.
- Extract shared code into a small common crate only where it reduces
  duplication without coupling Conspectus to Atelier command modules.
- Add Atelier delegation or compatibility shims for `atelier session list`,
  `atelier mux status`, forge status, and graph-heavy parts of
  `atelier status`.
- Update documentation to name Conspectus as the cross-workspace
  observability surface.

## Tests

- Cross-crate regression tests for shared parsers and metadata readers.
- Atelier integration tests proving existing commands still work or produce
  intentional deprecation/delegation output.
- Conspectus regression tests proving extracted code preserves graph JSON and
  table behavior.
- Comparison tests on representative Atelier workspaces where practical.

## Manual Checks

```sh
atelier session list
atelier mux status
conspectus session
conspectus session --projection mux
```

Compare output on the same workspace and confirm users can still find the
session, mux, fork, and PR information they previously reached through
Atelier.

## Assumptions

- This phase does not begin until Conspectus has enough standalone behavior to
  replace the overlapping surfaces.
- Shared code extraction is a consequence of stable seams, not a prerequisite
  for the first Conspectus implementation.
