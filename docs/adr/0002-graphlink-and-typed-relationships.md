# ADR 0002: GraphLink And Typed Relationships

## Status

Accepted

## Context

Conspectus models a sparse graph of sessions, mux sessions, repos, checkouts,
branches, workspaces, forks, and forge PRs. Links can come from direct on-disk
evidence, provider metadata, conventions, user declarations, or rebuildable
caches. The same source and target may have multiple candidate links with
different provenance and confidence, and declared links can override discovered
links without deleting the original evidence.

The resolution layer is expected to be nontrivial. It needs an explicit data
structure to operate on so it can apply precedence rules, preserve diagnostics,
and produce stable table and machine-readable graph views.

## Decision

Use both normalized `GraphLink` candidates and typed resolved relationships,
with a strict separation of responsibility.

`GraphLink` is the canonical candidate and evidence edge model. Every
discovered, convention-derived, declared, and cached relationship enters the
graph as a `GraphLink` candidate.

Each `GraphLink` should include:

- source node ID
- target node ID
- relation kind enum
- provenance
- confidence
- source adapter or evidence metadata
- freshness
- ignored or overridden state

Typed nodes and typed relationship structs are the resolved domain model used by
internal code, table projections, and higher-level graph views. A resolver layer
turns `GraphLink` candidates into typed relationships for the current graph
snapshot after applying precedence, freshness, ignore, and override rules.

Authority rules:

- `GraphLink` is authoritative for evidence, provenance, conflict reporting,
  overrides, and durable declared link state.
- Typed relationships are authoritative only for the resolved graph snapshot
  after candidate links have been processed.
- Declared links affect resolution priority but do not delete discovered or
  convention-derived candidate links.
- Machine-readable graph output should expose candidate links and enough
  resolved relationship data for consumers to distinguish raw evidence from the
  current projection.

## Consequences

- The resolver has a single uniform input structure, which keeps complex
  precedence and conflict handling out of provider adapters and table views.
- Provenance, confidence, freshness, ignore state, and override state are
  represented consistently for all relationship kinds.
- Sparse and conflicting relationships can be preserved for diagnostics while
  still producing opinionated table projections.
- Typed resolved relationships keep application code from becoming stringly
  typed around relation names and node IDs.
- The model has more moving pieces than a pure typed-edge design or a pure
  normalized-edge design, but that complexity is isolated at the resolver
  boundary.

## Alternatives Considered

- Store every relationship only as `GraphLink`. Rejected because common graph
  operations and projections would become too stringly typed, and important
  domain invariants would be harder to express.
- Store every relationship only as typed structs. Rejected because provenance,
  confidence, declared overrides, and conflict diagnostics would need to be
  duplicated across every relationship type.
- Store only declared links as `GraphLink`. Rejected because the resolver needs
  a uniform structure for discovered, convention-derived, cached, and declared
  candidates.

## Open Questions Answered

- Concrete ERD relationships should exist as typed resolved relationships, but
  they should be backed by normalized `GraphLink` candidates.
- `GraphLink` should represent every discovered, convention-derived, declared,
  and cached relationship candidate.
- `GraphLink` is authoritative for evidence, provenance, conflict reporting,
  overrides, and durable declared state.
- Typed relationships are authoritative only for resolved graph snapshots and
  table projections.
