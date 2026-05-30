# ADR 0045: Sessions Graph Lineage And Repo Default

## Status

Accepted

## Context

The sessions TUI has multiple grouping modes. Before this decision,
`graph` was the default and primarily meant "include workspace
containment above repo and checkout grouping." At the same time,
session-lineage discovery had grown beyond one harness:

- opencode emits `parent_session` links and can classify subagent
  sessions.
- Codex emits `parent_session` links for `forked_from_id`.
- Claude Code emits `parent_session` links for transcript fork and
  successor evidence.

Only opencode subagent rows were nested in the sessions tree. Regular
Codex and Claude Code lineage was visible in table/detail surfaces, but
not in the left-panel graph view. That made `graph` less graph-like
than its name implied, while the default view could become visually
busy once full lineage nesting was applied.

## Decision

`repo` is the default sessions grouping. It remains location-first:
repo, optional checkout fan-out, then sessions.

`graph` becomes the topology-rich sessions grouping. It keeps the
existing workspace/repo/checkout hierarchy and also nests resolved
active `parent_session` edges under their parent session, regardless
of harness. This includes opencode subagents and regular Codex /
Claude Code fork or successor lineage when the parent session node is
known.

Unresolved parent endpoints remain visible in detail/table lineage
surfaces but cannot drive tree nesting until the parent node is
discovered. Non-graph groupings do not follow session lineage.

## Consequences

- The default TUI remains stable and scannable for session switching.
- Operators who want provenance can switch to `graph` and see session
  ancestry in the tree.
- `graph` now has a clear semantic distinction from `repo`: it follows
  graph topology, not only location containment.
- Filtering still applies to nested rows. A child is suppressed from
  the top-level list only when its resolved parent is visible in the
  same build.

## Alternatives Considered

- **Nest lineage in every grouping.** Rejected because it removes the
  location-first escape hatch and makes `repo` less predictable as the
  default switcher view.
- **Keep `graph` as default.** Rejected because full lineage nesting can
  be visually noisy, especially with subagents and fork-heavy workflows.
- **Add a separate `lineage` grouping.** Deferred. The existing `graph`
  name already fits the broader topology behavior, and adding another
  grouping before the current set is exercised would increase UI
  surface area.

## Open Questions Answered

- **Should unresolved lineage nest?** No. Tree nesting requires a
  concrete parent row.
- **Should provider-specific lineage semantics affect nesting?** No.
  The tree consumes the provider-neutral `parent_session` relation.
