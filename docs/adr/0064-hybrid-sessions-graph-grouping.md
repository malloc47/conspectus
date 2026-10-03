# ADR 0064: Hybrid Workspace+Repo Grouping In Sessions/Graph

## Status

Accepted

## Context

After ADRs 0061 (drop unimplemented `Workspace` variants from
Mux/Prs/Forks/Union), 0062 (Workspaces view polish), and 0063
(remove the cross-reference chip), the Sessions / Graph view's
strict workspace nesting reads:

- (A)-class session (cwd inside a workspace's visible tree):
  workspace → repo → checkout → session.
- (B)-class session (cwd in a workspace-member repo but outside
  the workspace tree): repo → checkout → session.
- Unaffiliated session: repo → checkout → session.

Two operator-noted problems with the current shape:

1. **The repo level beneath a workspace is noise.** For
   workspace-rooted work, the workspace already conveys the
   project context; nesting the member repo as an extra header
   below the workspace duplicates that signal. In atelier the
   member repo names and the workspace name often overlap
   visually; in agent-deck the workspace composite directory
   isn't a repo at all.
2. **Agent-deck A-class sessions drop into the ungrouped
   bucket.** Agent-deck launches the harness with `cwd =
   <workspace-root>` (the composite directory) rather than
   inside a specific member subdir. The composite directory
   isn't a checkout, so `resolve_group_key`'s
   `checkout_for_path(cwd)?` returns `None` and the function
   bails before the workspace lookup runs. On the operator's
   daily-driver machine this drops all 22 A-class sessions
   across four active workspaces into "ungrouped" — the
   workspace level never renders, because no live A-class
   session is in a member subdir.

A separate observation that motivated revisiting the shape: the
dedicated Workspaces view (`View::Workspaces` / keybinding `6`)
introduced by `CSP-406` is essentially `workspace → session`
with no repo level. If the Sessions / Graph view adopts the same
shape for its workspace buckets, the dedicated view becomes
structurally redundant — the same workspace-first slice is now a
top-level shape inside Sessions, and a future
`SessionsGrouping::Workspace` (still TBD) could replace the
dedicated view entirely. That convergence isn't in scope for
this ADR, but it's worth recording as the forward direction.

## Decision

Reshape Sessions / Graph into a **hybrid** where workspaces and
repos are peer top-level parents, each containing sessions
directly:

```text
atelier-ws  conspectus+config  (atelier)
├── codex:019e1d32
├── codex:019e243d
└── …
5fe41ada  config+personal+work-config  (agent-deck)
├── codex:019e0f98
└── codex:019e1777
~/src/conspectus
├── claude-code:01b8dd7b
├── claude-code:c3d97895
└── …
~/src/atelier
├── codex:6b4fdee8
└── …
```

Rules:

- **A-class session** (carries `AgentSession --
  AssociatedWith → Workspace`): groups under the workspace
  directly at depth 1. No intermediate repo level, no requirement
  that the session live in a member subdir or even a checkout.
- **B-class session** and **unaffiliated session**: falls through
  to the existing repo grouping (depth 0 = repo, depth 1 =
  session; depth 2 = session if the repo has ≥ 2 worktrees and
  the checkout level renders).
- **Workspace headers** use the canonical format from ADR 0062:
  `<name>  <repo-a+repo-b+...>  (<provider>)`. The formatter
  moves to `src/tui/rows/mod.rs` as `pub fn
  format_workspace_display` so the Workspaces view and Sessions
  / Graph share one implementation.
- **Bucket ordering.** Workspace buckets sort before repo
  buckets at top level — a custom `Ord` on `GroupKey` flips
  Rust's default `Option` ordering for the `workspace` field.

The (A)/(B) distinction stops driving any UI grouping or marker
beyond this branching at `resolve_group_key`. The data-model
edge (`AssociatedWith Workspace`) still exists for downstream
tooling, but the row builder consumes it only as a top-level
routing signal.

`GroupKey` carries a workspace-only shape (`workspace = Some,
repo = None, worktree = None`) or a repo shape (`workspace =
None, repo = Some(RepoBucket), worktree = Some`). The
`RepoBucket` newtype packages the three repo fields that used
to live as siblings on `GroupKey` (`common_dir`,
`repo_display_path`, `repo_id`) so the workspace branch can
drop them cleanly.

### Forward direction (out of scope, recorded as direction-only)

With this hybrid in place, a `SessionsGrouping::Workspace` that
filters Graph's two-bucket output down to only the workspace
buckets becomes a natural follow-up — and if it lands, the
dedicated `View::Workspaces` (`6` keybinding) becomes redundant
since both surfaces would render the same row shape. Whether to
deprecate the dedicated view, the keybinding, and
`WorkspacesGrouping` after such a grouping ships is a separate
decision that this ADR explicitly does not make.

## Consequences

- **Agent-deck workspaces actually populate the Graph view.**
  The 22 A-class sessions previously hidden in "ungrouped" on
  the operator's machine now sit under their workspace headers.
- **Workspace headers in Sessions / Graph match the Workspaces
  view.** Same name + members + provider format, one
  formatter, identical reading.
- **Repo-shared (B-class) sessions stay at top level under their
  repo.** The CSP-405 bug-fix property holds — sessions in
  `~/src/conspectus` whose cwd is outside any workspace tree do
  not nest under a workspace, even when their repo is a workspace
  member. The Workspaces view's polish (ADR 0062) already
  applies the same rule there.
- **`workspace_member_names` lookup** uses the resolver-selected
  `WorkspaceContainsRepo` candidate links' `logical_path` source
  field — same source the Workspaces view's `fetch_members` SQL
  uses, so the two view headers are guaranteed to read
  identically.
- **`GroupKey` Ord is custom.** Putting workspace buckets ahead
  of repo buckets requires inverting Rust's default `Option`
  ordering on the `workspace` field. The repo + worktree
  comparison stays lexicographic.
- **No new node-level or link-level model change.** The data
  layer is unchanged; the entire reshape lives in the row
  builder.
- **Tests updated.** `graph_grouping_uses_session_workspace_context`
  asserts depth 1 instead of depth 2 and a 2-row tree (no repo
  intermediate). `workspace_rooted_session_nests_under_workspace_with_no_chip`
  renamed to `workspace_rooted_session_nests_directly_under_workspace`.
  Two new tests cover the agent-deck workspace-root cwd case and
  the hybrid peer-parents shape.

## Alternatives Considered

### A. Workspace → repo → session (3 levels)

The intermediate interpretation I proposed before the operator
corrected me. Repos that are workspace members render as
children of the workspace; sessions nest under the repo as
usual. Cleaner data-model story (workspaces own repos; repos
own sessions), but the operator wanted sessions *directly*
under workspaces — the repo level under the workspace was the
specific noise they were trying to remove. Rejected on user
direction.

### B. Keep today's strict shape, just fix the agent-deck
ungrouped-bucket bug

A surgical fix would route A-class sessions whose cwd isn't a
checkout into the workspace bucket without a checkout, while
keeping the repo intermediate for sessions that DO have a
member-subdir checkout. The shape would be heterogeneous —
some workspace buckets render `workspace → session` and others
render `workspace → repo → session` depending on cwd. Rejected
because the asymmetry would be confusing to read, and the
homogeneous hybrid is simpler.

### C. Make `SessionsGrouping::Workspace` a new mode instead of
reshaping `Graph`

Add a fifth grouping value that produces the workspace-first
view, and leave Graph alone. The operator gets the new shape
only when they explicitly switch. Rejected because Graph is
the daily-driver default; if the new shape is the right one,
making it the default avoids a confusing two-of-everything
situation where Graph still shows the broken ungrouped bucket
and Workspace shows the correct one.

## Open Questions

- Should a `SessionsGrouping::Workspace` (Graph minus the repo
  buckets) ship next, and does it deprecate `View::Workspaces`
  and the `6` keybinding? Recorded as a forward direction
  above; not decided here.
- The `RowId::Group(Repo)` and `RowId::Group(Workspace)`
  variants both flow through the same `Row.id` shape. Today the
  workspace bucket can't collide with a repo bucket because
  their `NodeId` variants differ. If a future grouping mode
  wants to scope by workspace + repo (e.g., the CSP-406.01 Repo
  grouping when it lands), the scoped `RowId::Repo { workspace,
  repo }` / `RowId::WorkspaceAgentSession` patterns from the
  Workspaces view are already in `rows/mod.rs` and can be
  reused.
