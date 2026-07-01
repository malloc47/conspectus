# ADR 0062: Workspaces View Polish — Inline Members, Drop Related

## Status

Superseded by [ADR 0065](0065-sessions-workspace-grouping-replaces-workspaces-view.md).
The dedicated Workspaces view this ADR polished was removed;
`SessionsGrouping::Workspace` is the successor surface. The inline
member-span formatting decided here carried forward into the
workspace group headers.

## Context

`H-WS-002` shipped a Workspaces view MVP with each workspace
expanding to up to three labeled subgroups:

1. `members (N)` — every `WorkspaceContainsRepo` member, each
   emitted as a `RowKind::Repo` row with a Repo `NodeId` for
   left-tree navigation and detail-pane focus.
2. `in workspace (N)` — (A)-class agent sessions: cwd is inside
   the workspace tree.
3. `related (N)` — (B)-class agent sessions: cwd is inside a
   member repo's checkout but outside the workspace tree.

Operator feedback after running the MVP:

- The `members` subgroup is noise in the left tree. The repos are
  not interesting to navigate to from this view; the detail pane
  already surfaces them as `member` HeaderFields via
  `workspace_member_fields` (commit `4bd3829`). Having member
  repos as their own subgroup with their own rows takes vertical
  real estate without producing actionable structure.
- The `in workspace` / `related` labels read as jargon. The
  operator could not predict what would land in each bucket
  without consulting the design plan. The (A)/(B) distinction is
  load-bearing in the data model but does not translate into a
  useful UI vocabulary inside the Workspaces view itself.

The `related` subgroup also re-surfaces a signal the Sessions
view already carries: the `[ws-name]` chip from `H-WS-001`
(ADR-less; documented in `docs/plans/workspace-view-redesign.md`
§Axis 1). The operator already sees the cross-reference there
when looking at a session row; reproducing it under the workspace
inverts the same pair without adding information.

## Decision

Two changes to the Workspaces view, landed together as the
H-WS-002 polish pass.

### Move members from a subgroup to an inline span

The `members` subgroup and its `RowKind::Repo` children are
dropped. The workspace top-level row's `display_path` now carries
the member-repo list inline, `+`-joined, immediately after the
workspace name and before the parenthesized provider chip:

```text
atelier-ws  conspectus+config+atelier  (atelier)
```

The `+` separator matches the agent-table workspace column
convention from ADR 0060 / commit `dad589`, so the two surfaces
read the same. The detail pane continues to expose every member
as a navigable HeaderField, so removing the left-tree subgroup
loses no navigation affordance — the operator who wants to focus
a member repo now opens the workspace's detail pane.

Sections of the display string are omitted when their data is
missing: a workspace with no members drops the join segment, and
a workspace with no provider drops the parens. A workspace with
neither degrades cleanly to its bare name.

Truncation of the inline list is delegated to the renderer's
existing single-row clipping. If long member lists become
operator-visible noise, a future polish can introduce an
explicit "first N + count" rule; v1 keeps the rule simple.

### Drop the (B)-class `related` subgroup from this view

The Workspaces view now surfaces only (A)-class sessions:
sessions with a direct `AgentSession -- AssociatedWith →
Workspace` edge. Sessions whose cwd lives in a member repo's
checkout but which have no direct workspace edge are not shown
under the workspace at all. They continue to show up in the
Sessions / Graph view with the `[ws-name]` chip from `H-WS-001`.

With (B) gone, the remaining (A) sessions no longer need a
labeled wrapper. They sit at depth 1 directly under the
workspace row. The view simplifies to:

```text
atelier-ws  conspectus+config  (atelier)
├── codex:plan-revamp
└── claude:demo-fork
```

This narrows the (A)/(B) decision from `H-WS-002`. Previously
(A) and (B) were each load-bearing in two places — cross-link
emitted `AssociatedWith` for (A), Sessions/Graph nested only for
(A), and the Workspaces view distinguished both. After this
polish, the Workspaces view consumes only (A); (B) lives only as
the Sessions chip.

## Consequences

- `MemberSqlRow` slims down from four fields (repo node id,
  common dir, display name, canonical path) to a single
  `display_name`. The `canonical_path` derivation logic
  (`canonical_path_from_source_paths`) moves out with the
  members subgroup; the detail pane's existing member-rendering
  path is unaffected.
- `fetch_b_class_sessions` and its multi-hop
  `resolved_relationships`-join query are deleted. The
  (A)-class fetch is unchanged.
- The `RowId::Subgroup`, `RowKind::Repo`, and `RepoRow`
  scaffolding from `H-WS-002` remain defined — they are still
  wired into renderer, search, and selection paths and may be
  reused by `H-WS-002a`'s Repo grouping (which flips the tree
  to repos-as-top-level).
- The Workspaces view's row count per workspace drops from
  `1 + members + (A) + (B) + 2 or 3 subgroup wrappers` to
  `1 + (A)`. A dense daily-driver snapshot reads substantially
  shorter at the cost of losing the cross-reference inversion.
- The H-WS-002a scope shrinks: the originally planned
  "default-collapse the `related` subgroup at row-tree emit time"
  is no longer applicable — `related` is gone, not collapsed.
  The Provider / Activity / Repo grouping work continues as
  planned.

## Alternatives Considered

### A. Keep `related` but rename and default-collapse

The original H-WS-002a plan. Rejected because the operator could
not name what the bucket would contain without consulting the
plan, suggesting no rename gets all the way there. Surfacing the
same (A)/(B) pair on two surfaces (chip in Sessions, subgroup in
Workspaces) also creates two places where the policy can drift.

### B. Drop `related` only; keep `members` as a subgroup

Drops the unintuitive bucket but keeps the members navigation
affordance. Rejected because the navigation already exists via
the detail pane, and the per-member rows add vertical density
without expanding what the operator can do.

### C. Show only `members` in the workspace row; keep `in workspace` as a labeled subgroup

Less aggressive than the decision above. Rejected because the
`in workspace` label was the second unintuitive piece of
vocabulary; with (B) gone there is one bucket and labeling it
adds depth without adding distinction.

## Open Questions Answered

- *Q: Should (B)-class sessions appear in the Workspaces view at
  all?* (from `docs/plans/workspace-view-redesign.md` open
  question 2)
  **A: No. The Sessions/Graph chip covers the cross-reference;
  reproducing it as a subgroup re-introduces the (A)/(B)
  conflation H-WS-001 fixed in a different surface.**
