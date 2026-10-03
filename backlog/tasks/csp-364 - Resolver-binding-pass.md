---
id: CSP-364
title: Resolver binding pass
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-363
ordinal: 297000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
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
- Blockers: `CSP-363`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`resolve::pins` binds pins through mux-anchored
attribution, synthesizes pin-derived links/aliases, and emits
`PinUnbound`, `PinStaleMux`, `PinAmbiguous`, and `PinDrift`
diagnostics covered by resolver and snapshot tests.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-004`
