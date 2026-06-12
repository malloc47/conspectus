# ADR 0061: Drop Workspace Grouping From Non-Workspaces Views

## Status

Accepted

## Context

`H-WS-003` was filed to audit the four non-Sessions row builders
(Mux, Prs, Forks, Union) for the same (A)/(B) workspace-grouping
conflation that motivated `H-WS-001` in the Sessions view. The
backlog speculated that Mux (session→workspace→mux chain) and Prs
(branch→repo→workspace) likely produced false-positive workspace
grouping in daily-driver setups, with Forks (direct
workspace→fork) and Union (intentionally permissive) lower risk.

The audit found something different. None of the four views
actually implement workspace grouping today:

- `src/tui/rows/mux.rs:191` matches `MuxGrouping::Session |
  MuxGrouping::Workspace | MuxGrouping::Host` together and calls
  `emit_flat`. No workspace headers are emitted; the
  `MuxGrouping::Workspace` variant is a no-op label that selects
  the same flat layout as `Session`.
- `PrsBuildInputsFromConn`, `ForksBuildInputsFromConn`, and
  `UnionBuildInputsFromConn` carry no `grouping` field at all. The
  `PrsGrouping`, `ForksGrouping`, and `UnionGrouping` enums are
  referenced only from `src/tui/mod.rs` (for the cycler, the
  `as_str` label, and `values_for` menu), `src/config.rs`, and
  `src/cli.rs`. No row builder reads them.

The grouping cycler therefore advertises a "workspace" label in
four views that does nothing when selected. There is no (A)/(B)
conflation to fix because there is no workspace nesting to fix.

A secondary finding: for Prs, Forks, and Union the (A)/(B)
distinction does not translate cleanly. (A) means "session cwd is
inside the workspace's visible tree." PRs and forks have no cwd;
their only path to a workspace is via the branch→repo→workspace or
session→workspace chain — both structurally the (B) "weak
membership" case at the session level. There is no analog of "this
PR is workspace-rooted." Implementing workspace grouping in these
views would either rebrand the (B) relationship as primary
(reintroducing the conflation `H-WS-001` removed) or invent a new
(A)-like definition per node kind.

The Workspaces view introduced by `H-WS-002` is the canonical
place for workspace-first navigation. It already lists workspaces
as top-level rows with `in workspace` (A-class sessions) and
`related` (B-class sessions) subgroups, and `H-WS-002a` will extend
it with Provider / Activity / Repo groupings. Surfacing workspaces
as a peer grouping inside Mux/Prs/Forks/Union duplicates this view
without giving it a useful (A)/(B) shape.

## Decision

Drop the `Workspace` variant from `MuxGrouping`, `UnionGrouping`,
`PrsGrouping`, and `ForksGrouping` in `src/tui/mod.rs`. Update
`Grouping::as_str`, `Grouping::values_for`, and the dead match arm
at `src/tui/rows/mux.rs:191`. Existing comments in the four row
builders that deferred chip semantics to `H-WS-003` are updated to
record that the chip has no analog in views that do not implement
workspace grouping.

The Sessions view retains its strict-only nesting plus the
(B)-class chip (`H-WS-001`). The Workspaces view (`H-WS-002`,
`H-WS-002a`) is the only view that organizes around workspaces.

Configs that set `[tui.views.<mux|prs|forks|union>].grouping =
"workspace"` will now produce a `ConfigDiagnostic` listing the
valid values for that view. This is acceptable: the previous
acceptance was already silently no-op behavior, so anyone with
this setting was already seeing the default layout. The diagnostic
makes the breakage visible and points at the menu of valid values.

## Consequences

- The grouping cycler in Mux/Prs/Forks/Union loses one menu entry
  each; the views' menus now reflect what they actually implement.
- The cycler test `parse_and_as_str_round_trip_per_view` continues
  to pass because it iterates `values_for(view)`, which shrinks
  accordingly.
- `src/tui/rows/mux.rs:191` becomes a two-variant match instead of
  three; the test
  `mux_view_skips_pins_group_under_flat_groupings` and its
  failure message lose the "workspace" mention.
- Any user config containing `grouping = "workspace"` for the four
  affected views now surfaces a config diagnostic with the menu of
  valid options. The migration path is to choose a value that
  matches the view's actual layout (`session` / `host` for Mux,
  `repo` / `state` for Prs, `provider` / `parent` for Forks,
  `kind` / `repo` for Union) or to remove the key entirely and
  fall back to the view default.
- The Workspaces view becomes the single answer to "show me
  workspace-organized state," reducing the surface area where the
  (A)/(B) conflation could re-enter the codebase.

## Alternatives Considered

### A. Implement workspace grouping properly in each view

Build workspace headers and (A)/(B) semantics in each of the four
row builders, mirroring the `H-WS-001` strict-only + chip pattern.
Rejected because:

- The (A)/(B) distinction is not analogous outside the Sessions
  view. PRs and forks have no cwd to anchor (A) on; using the
  branch→repo→workspace chain as (A) re-introduces the exact
  conflation `H-WS-001` removed.
- Four parallel row-tree rewrites are a substantial scope to land
  a feature whose user-facing surface is largely covered by the
  Workspaces view. The duplication invites drift: each view's
  (A)/(B) policy can diverge silently.
- Cost is poorly matched to demonstrated need. The operator
  complaint that motivated `H-WS-*` was specifically about the
  Sessions/Graph layout; no equivalent complaint exists for the
  other four views.

### B. Per-view disposition

Keep and implement `Workspace` grouping only where it carries the
(A)/(B) shape cleanly — likely Mux only (since muxes host
sessions, which inherit cwd) — and drop the variant from Prs,
Forks, and Union. Rejected because:

- Even the Mux case adds a peer grouping that overlaps the
  Workspaces view's "in workspace" subgroup without giving Mux a
  surface the Workspaces view lacks.
- The asymmetry (one of four views implements workspace grouping)
  is harder to explain than the cleaner "Workspaces is the
  workspace-organized view" boundary.

### C. Leave the variants in place and document them as deferred

The status quo. Rejected because a menu entry that silently maps
to flat behavior is precisely the kind of UX confusion that
`H-WS-001` was filed to remove. Sustaining a dead menu entry
behind a `// TODO` comment makes the surface harder to read for
both operators and future contributors.

## Open Questions Answered

- *Q: Does the existing `MuxGrouping::Workspace` /
  `PrsGrouping::Workspace` need fixing before
  `H-WS-002` lands, or is it low-traffic enough to defer
  (`H-WS-003`)?* (from `docs/plans/workspace-view-redesign.md`)
  **A: Neither — the audit found those variants are unimplemented,
  not buggy. They are dropped here.**
