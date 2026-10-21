# ADR 0139: Workspace-Or-Repo Grouping In The Mux View

## Status

Accepted. Refines ADR 0061 for the mux view. Builds on ADR 0064 (the
hybrid workspace-and-repo shape), ADR 0065 (sessions-view workspace
grouping), and ADR 0138 (workspaces found from observed cwds).

## Context

The operator keeps workspaces that span several repos: an Atelier
`sysadmin` workspace holds worktrees of `config`, `work-config`, and a
private repo. The mux view's groupings are `session`, `host`, and
`repo`, so the workspace's muxes scatter across its members' repo
groups, and nothing gathers them under "sysadmin".

ADR 0061 dropped a `MuxGrouping::Workspace` that did nothing, and it
rejected a real mux workspace grouping because "the Workspaces view is
the canonical place for workspace-first navigation". ADR 0065 then
folded that view into the sessions view's `workspace` grouping, so the
mux view lost any workspace-first route and ADR 0061's reason no longer
holds.

Muxes also have no workspace link to group by. Cross-link inference
associates agent sessions with workspaces by cwd, but not muxes.
`docs/design.md` already models the relationship as
`MUX_SESSION rooted_in WORKSPACE`, and `RelationKind::RootedIn` exists
with explorer labels, but nothing emitted it.

## Decision

1. **A `workspace` mux grouping.** `--grouping workspace`,
   `[tui.views.mux] grouping = "workspace"`, the controls overlay, and
   the grouping cycle all offer it, after `repo`.
2. **Workspace first, then repo.** The Pins group comes first, as under
   `repo`. Then comes one header per workspace that has a mux or pin
   placeholder in it, sorted by label then root. Every other mux keeps
   the bucket `repo` grouping gives it, sorted by repo display path,
   and muxes with no repo go in Ungrouped last. Muxes inside a bucket
   sort as they do under `repo`.
3. **Membership is a graph link.** Cross-link inference emits a
   `MuxSession --rooted_in--> Workspace` candidate when the mux's
   session path (tmux `session_path`, else the active pane's path) is
   inside a workspace root or a member's logical path. It uses the same
   path index that associates agent sessions, so a symlinked member's
   canonical checkout elsewhere doesn't count. With nested workspaces
   the innermost one wins.
4. **The session path, not the pane's cwd.** A mux stays in its
   workspace while its panes `cd` around. The repo fallback still
   buckets by the active pane's path, as `repo` grouping does.
5. **One rooted-in workspace per mux.** `rooted_in` stays a single
   resolver slot, so a declared link outranks the inferred one by
   provenance. A producer for another `rooted_in` target (repo,
   checkout, fork) will need a slot per target kind.
6. **Pin placeholders match by path.** An unbound pin has no mux in the
   graph, so its cwd goes through the same path index.
7. **Headers match the sessions view.** A workspace header reads
   `<name>  <members>  (<provider>)`, built by one helper both views
   share.

## Consequences

- Muxes rooted anywhere in `sysadmin` sit under one `sysadmin` header,
  and their labels no longer depend on which checkout path a repo node
  carries (backlog `CSP-650`).
- The graph gains `rooted_in` links from muxes. The explorer already
  shows them ("rooted in" / "hosts"), and the JSON, DOT, and HTML
  exports carry them.
- Workspace grouping in the mux view depends on workspace discovery;
  ADR 0138 makes Atelier workspaces with live work show up without
  scan roots.
- Only workspaces with muxes or placeholders get headers. For idle
  workspaces, use the sessions view's `workspace` grouping.

## Alternatives Considered

- **Workspace only, as in the sessions view (ADR 0065).** Every mux
  outside a workspace would land in Ungrouped. The operator chose
  workspace-or-repo so those muxes keep their repo groups.
- **Work membership out in the TUI by path, with no graph link.**
  Rejected: the design is model-first, and a link appears in the graph
  and explorer and can be overridden by a declared link.
- **Inherit the workspace from attached agent sessions.** Rejected as
  the source of truth because a mux without an agent has no answer, and
  a mux has its own path. A mux whose session path is outside every
  workspace stays in its repo group even if an agent inside it works
  in one.
- **Use the active pane's path.** Rejected: the group would change
  whenever a pane changes directory.
- **Make `rooted_in` multi-target.** Rejected for now: with only
  workspace targets it would keep both a declared and an inferred link
  and lose the override's precedence.
- **Show every workspace, idle or not.** Rejected: the mux view lists
  muxes, and empty headers would crowd it.

## Open Questions Answered

- *Does this reopen ADR 0061's concern about conflating workspace-rooted
  and repo-shared work?* No. Membership comes from the mux's own path
  inside the workspace's visible tree, the (A)-class rule. A mux in a
  member repo's canonical checkout elsewhere on disk is not rooted in
  the workspace.
