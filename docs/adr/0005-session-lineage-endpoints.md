# ADR 0005: Session Lineage Endpoints

## Status

Accepted

## Context

ADR 0003 replaces separate `SessionFork` nodes with a polymorphic `Fork` node.
Session lineage is therefore represented by links between a `Fork` and agent
sessions rather than by a dedicated session-fork entity.

Provider evidence for session lineage is often partial. Atelier can record a
source session without a discovered child session, a fresh session with no
parent, unsupported native forking, approximate history-copy behavior, or a
child session that may only become discoverable after a later harness write.
Other providers may have similar partial or degraded evidence.

Conspectus must preserve this provenance without inventing concrete sessions
that have not been discovered or declared.

## Decision

Represent session lineage as `Fork` relation candidates with structured
unresolved endpoint evidence. Resolve to concrete `AgentSession` relationships
only when concrete endpoint nodes exist.

Relation semantics:

- A `Fork` may have `parent_session` and `child_session` relation candidates.
- If the endpoint session is discovered or declared, the relation targets an
  `AgentSession` node.
- If the endpoint session is not discovered but a provider reports a native ID,
  store that ID in `GraphLink` evidence metadata as an unresolved endpoint
  reference.
- If the provider records an intentional fresh session, store that as lineage
  evidence, such as `lineage_kind = "fresh"` or `fresh_session = true`. Do not
  invent a parent session.
- If a child session is expected but not discovered yet, store structured
  evidence such as harness key, state scope, fork root, time window, provider
  source, and native ID when available.
- If behavior is approximate or unsupported, preserve that in source metadata.
  A later data-model decision may promote common values such as `native`,
  `approximate`, `unsupported`, and `fresh` into a provider-neutral enum.

Resolver behavior:

- Emit concrete typed `parent_session` and `child_session` relationships only
  when endpoint `AgentSession` nodes exist.
- Keep unresolved endpoint evidence available for diagnostics and
  machine-readable graph output.
- Allow later discovery to reconcile unresolved endpoint metadata into concrete
  `AgentSession` links.

## Consequences

- Conspectus preserves session-lineage intent and evidence even when one or both
  endpoint sessions are missing.
- The graph avoids fake or placeholder `AgentSession` nodes that could be
  mistaken for discovered sessions.
- Fresh-session, unsupported, approximate, and not-yet-discovered cases remain
  visible to table views and downstream tools.
- `GraphLink` evidence metadata must support unresolved endpoint references,
  not only concrete node IDs.
- Resolver and projection code must distinguish concrete relationships from
  unresolved lineage diagnostics.

## Alternatives Considered

- Create session lineage only when both parent and child `AgentSession` nodes
  exist. Rejected because it loses provider intent and provenance for fresh,
  unsupported, approximate, and not-yet-discovered cases.
- Create placeholder `AgentSession` nodes for unresolved endpoints. Rejected
  because it pollutes the graph with sessions that were not discovered or
  declared and blurs the difference between missing evidence and real nodes.
- Store unresolved session information only as provider metadata on `Fork`.
  Rejected because the resolver needs relation-like evidence to reconcile later
  discovered sessions and to expose candidate links consistently.

## Open Questions Answered

- Session lineage can exist even when parent or child session nodes are missing.
- Missing endpoints should not automatically become placeholder
  `AgentSession` nodes.
- Fresh, unsupported, approximate, and not-yet-discovered session lineage cases
  should be preserved as relation/source metadata.
- Concrete typed session relationships should resolve only when concrete
  endpoint `AgentSession` nodes exist.
