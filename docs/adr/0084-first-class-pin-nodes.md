# ADR 0084: First-Class Pin Nodes

## Status

Accepted

## Context

ADR 0057 introduced session pins as TOML-backed declarations and a
snapshot sidecar (`GraphSnapshot::pins`). That was enough for CLI
commands, launch/adopt flows, and synthetic TUI rows, but it left pins
outside the graph:

- pin rows could not always select a real detail target,
- graph exports could not show a pin's intended mux or realizing
  session,
- search and explorer surfaces had to special-case pins as non-nodes,
- store lineage lived only on the sidecar record.

CSP-460 requires pins to behave like graph entities while keeping
the TOML schema from ADR 0057 as the persistence source.

## Decision

Add `PinId`, `PinNode`, `NodeId::Pin`, and `GraphNode::Pin`.

Pin node identity is the effective pin id after normal local/global
shadowing, rendered as `pin:<id>`. This matches ADR 0057's operator
identifier and keeps identity stable across resolver binding changes.
The TOML pin entry remains authoritative; `PinNode` is a derived graph
projection rebuilt from `GraphSnapshot::pins`.

Pin nodes carry the declaration fields (`display_name`, `harness`,
`cwd`, `mux`, optional `launch_argv`, optional `reason`), store lineage
(`provenance`, `store_path`), and the resolver-populated `binding`.

Add pin-specific relation kinds:

- `pin_targets_mux`: from the pin to the intended mux. If the mux is not
  live, the edge targets an unresolved `mux_session` endpoint.
- `pin_realized_by_session`: from the pin to the live agent session when
  the resolver binds one.

Do not overload `linked_to_mux` for pin-to-mux identity. That relation
continues to mean agent-session-to-mux attribution and participates in
existing resolver ranking.

Because this adds archived enum variants, bump the rkyv snapshot cache
format version. Existing cache files are rebuildable and must be
discarded on version mismatch.

## Consequences

- TUI pin rows can select `NodeId::Pin` and render pin details directly.
- DOT/HTML/JSON graph exports expose pins as nodes plus candidate links.
- The resolver still computes binding from live mux/session evidence;
  binding remains non-authoritative cacheable state, not TOML state.
- `GraphSnapshot::pins` remains for CRUD and compatibility with pin
  command code, but it is no longer the only graph-visible projection.
- Declared/alias endpoint helpers can name pins through a lightweight
  `DeclaredEndpoint::Pin { id }` encoding.

## Alternatives Considered

- **Keep synthetic rows only.** This avoids a model change but preserves
  the detail/search/export gaps that CSP-460 exists to close.
- **Represent pins only as unresolved endpoints.** This makes unbound
  state visible but still gives bound pins no durable node identity.
- **Reuse `linked_to_mux` for pin-to-mux links.** Rejected because it
  would make pin declarations compete in the same resolver slot as
  agent-session mux attribution.

## Open Questions Answered

- **Does a pin id include store path?** No. Effective pins are already
  de-duplicated by local/global shadowing before graph projection, so
  the operator-facing id is the graph identity.
- **Is binding persisted in the pin node?** No. It is copied into the
  derived node from the resolver-mutated sidecar each snapshot cycle.
