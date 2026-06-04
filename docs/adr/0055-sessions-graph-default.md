# ADR 0055: Sessions Graph Default

## Status

Accepted

## Context

ADR 0045 made `repo` the default sessions grouping because full
lineage nesting could make the left pane visually busy. Since then,
the TUI detail pane and row rendering have been tightened so the
sessions view can carry richer topology without losing the ability to
inspect the selected node. In day-to-day use, the repo default hides
session lineage and workspace containment until the operator switches
grouping, which makes the first view less representative of
Conspectus's graph-first model.

## Decision

The sessions view defaults to `graph`.

`graph` remains the topology-rich grouping: workspace/repo/checkout
containment when known, plus resolved active `parent_session` nesting
across harnesses. `repo` remains available as the location-first
grouping for operators who want a simpler project-root switcher.

This supersedes only ADR 0045's default-grouping decision. ADR 0045's
semantics for `graph`, `repo`, and lineage nesting remain accepted.

## Consequences

- The default sessions list better matches Conspectus's graph-oriented
  product model.
- Resolved session lineage is visible without opening controls or
  cycling groupings.
- The left pane can be busier in fork-heavy workflows, but the
  location-first `repo` grouping remains one switch away.
- Configured groupings, CLI grouping flags, and the deprecated
  `[tui].sessions_grouping` alias keep their existing precedence.

## Alternatives Considered

- **Keep `repo` as default.** Rejected because it hides the graph
  topology that users now expect from the default sessions view.
- **Add a separate startup option for graph-first users.** Rejected
  because the existing grouping config already provides overrides, and
  the default should reflect the intended product stance.

## Open Questions Answered

- **Does this change `repo` grouping semantics?** No. `repo` remains
  location-first and does not follow session lineage.
- **Does this change persisted configuration precedence?** No.
  Explicit project/user config and CLI flags still override the
  default.
