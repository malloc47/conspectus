---
id: CSP-089
title: Factor harness adapter state-root scanning
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-ref
milestone: m-11
dependencies: []
ordinal: 89000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-05 (`ac7c1d3`). New
  `discovery::harness::discover_with_state_root(context,
  harness_key, inner)` shared envelope wraps the "look up
  state root; empty on absence; run inner; stamp fragment"
  pattern. Codex, claude-code, opencode adapters each
  collapse their `discover` fn from 10 lines to 4 (single
  delegating call). Aider stays put per its per-repo shape.
  2 new tests in `harness_tests.rs` (short-circuit +
  stamping).
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-REF-007`
