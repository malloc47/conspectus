# ADR 0065: Workspace Grouping In Sessions Replaces The Workspaces View

## Status

Accepted

## Context

ADR 0064 ("Hybrid Workspace+Repo Grouping In Sessions/Graph") left
an explicit open question:

> Should a `SessionsGrouping::Workspace` (Graph minus the repo
> buckets) ship next, and does it deprecate `View::Workspaces` and
> the `6` keybinding? Recorded as a forward direction above; not
> decided here.

The hybrid Sessions/Graph view already renders workspace buckets at
the same depth, with the same header format, as the dedicated
Workspaces view. The two surfaces differ only in the slice of work
they show:

- Sessions/Graph: every workspace bucket *and* every repo bucket.
- Workspaces view: every workspace, including workspaces with zero
  active sessions; no repo buckets.

The dedicated view is therefore the workspace-only slice with one
extra property the Graph builder doesn't have today: it iterates
the workspace node list, so idle workspaces still render. Two
surfaces that differ by a filter is exactly the "two-of-everything"
situation ADR 0064's Alternative C warned against.

## Decision

1. **Ship `SessionsGrouping::Workspace`.** Add a new variant to
   `SessionsGrouping` between `Graph` and `Repo` in cycle order.
   In this mode the Sessions builder:

   - Emits one header for **every** discovered workspace node,
     even when no session is associated. (Differs from Graph,
     which only emits a workspace bucket when at least one
     (A)-class session lands in it.)
   - Groups (A)-class sessions — those with an active
     `AgentSession --AssociatedWith--> Workspace` edge — under
     their workspace header at depth 1.
   - Sends (B)-class and unaffiliated sessions to the existing
     synthetic **Ungrouped** bucket at the bottom of the tree.
     No repo or worktree headers render in this mode.
   - Reuses `format_workspace_display` so the header format is
     identical to Sessions/Graph and the (former) Workspaces
     view.

2. **Drop `View::Workspaces`.** With the workspace-only slice
   available as a Sessions grouping, the dedicated view is
   structurally redundant. Remove:

   - `View::Workspaces` from the `View` enum and its cycle.
   - The `6` keybinding (freed; not rebound).
   - `WorkspacesGrouping` and the `[tui.views.workspaces]` config
     block.
   - `src/tui/rows/workspaces.rs` and its `ViewLabel::Workspaces`
     entry.
   - The controls overlay row and CLI `--view workspaces` flag.

3. **Default view stays `Sessions`** with grouping `Graph`.
   Operators who lived in the Workspaces view can switch grouping
   via the controls overlay or the grouping cycle key — same
   shape, same headers, fewer top-level views to learn.

## Consequences

- **Two surfaces converge into one.** The workspace-first read is
  reachable through the Sessions view's grouping menu — the
  primary discoverability surface per the project's TUI menu-first
  preference — instead of through a separate view.
- **Idle workspaces remain visible.** Because Workspace grouping
  enumerates workspace nodes (not just session-bearing buckets),
  the operator still sees workspaces that exist but have no live
  sessions. This was a property of the dedicated view and is
  preserved.
- **(B)-class sessions are not hidden, they're parked.** In
  Workspace mode they fall into the existing `Ungrouped` synthetic
  group at the bottom of the tree. Operators who need to see repo
  context flip back to Graph or Repo grouping.
- **Persisted config migration.** Any saved
  `default_view = "workspaces"` is migrated at load time to
  `default_view = "sessions"` with `sessions_grouping = "workspace"`.
  Any `[tui.views.workspaces]` table is ignored with a warning.
- **No node-level or link-level model change.** The reshape lives
  in the Sessions row builder and the View/Grouping dispatch.
- **`RowId::WorkspaceAgentSession` is gone.** Workspace-grouped
  sessions use the regular `RowId::AgentSession` shape, since a
  given session has at most one `AssociatedWith Workspace` edge
  active per build and can't collide across two workspace
  buckets.

## Alternatives Considered

### A. Keep `View::Workspaces` and ship `SessionsGrouping::Workspace`

Both surfaces coexist; operators choose. Rejected as the explicit
"two-of-everything" outcome ADR 0064 already warned against.
Maintenance cost of two builders, two RowId schemes, two configs,
and divergent filter behavior outweighs the marginal flexibility.

### B. Workspace grouping shows only workspaces that have (A)-class sessions

Match Sessions/Graph's existing behavior — no synthetic empty
headers. Sparser tree, but loses the daily-driver property of
"see what workspaces exist on this machine." Rejected because the
dedicated view's idle-workspace visibility is the reason the
operator uses it.

### C. Workspace grouping keeps repo buckets like Graph

Then it would be Graph with the words rearranged. Rejected on the
same grounds as ADR 0064's Alternative C — a grouping mode that
doesn't change the bucket shape isn't a separate grouping.

### D. Rebind `6` to `SwitchGrouping(Sessions, Workspace)`

Preserve muscle memory by mapping the freed key to the new
grouping inside Sessions. Rejected because no other key crosses
the view/grouping boundary; the controls overlay already exposes
grouping switches uniformly. Adding a one-off key would muddy the
input model.

## Open Questions

None. ADR 0064's "ship the new grouping, drop the dedicated view"
question is resolved here in the affirmative.
