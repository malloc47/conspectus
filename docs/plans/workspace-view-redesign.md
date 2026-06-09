# Workspace UX Redesign (Tentative Plan)

**Status:** Tentative. Not yet promoted to phase work in
`docs/implementation/`. Captured to anchor `H-WS-*` backlog entries
and to make the design tradeoffs reviewable before scoping.

**Trigger:** Operator-noted confusion in the Sessions view's `graph`
grouping: sessions whose cwd is in a repo that *happens* to be a
member of some workspace get nested under that workspace, even when
the session itself has no workspace context. With heavily-used repos
that participate in multi-repo workspaces (`conspectus`, `config`,
`atelier`), the workspace level becomes a misleading parent.

## Diagnosis

`src/tui/rows/sessions.rs::resolve_group_key` (~line 650) selects
the workspace level under `SessionsGrouping::Graph` as:

```rust
data.workspace_for_session(&entry.id)               // strong: AssociatedWith
    .or_else(|| data.workspace_for_repo(&repo_node_id))   // weak: repo-is-member
```

The fallback conflates two semantically distinct relationships:

### Subsequent discovery: the canonical-checkout-root leak

After landing the initial grouping fix the operator reported that
nothing visibly changed. Investigation traced the cause to
`src/discovery/cross_link.rs::workspace_member_roots`, which indexed
both `logical_path` *and* `canonical_checkout_root` from every
`WorkspaceContainsRepo` link. For atelier/agent-deck workspaces
whose members are symlinks pointing outside the workspace tree, the
two paths diverge — `logical_path` is inside the workspace
composite directory, `canonical_checkout_root` is the symlink
target's canonical location. A session running at the canonical
checkout matched the second index entry and gained an
`AssociatedWith Workspace` candidate, so the strict grouping had
nothing to filter: every such session arrived already promoted to
(A)-class at inference time.

The fix is upstream of grouping: drop `canonical_checkout_root`
from the index. The session→workspace inference must require the
cwd to live inside the workspace's visible tree, not just inside
some repo the workspace happens to claim from elsewhere. The
field stays on the membership link's `source_metadata.fields` for
downstream lookups; it just doesn't drive AssociatedWith anymore.

This means the (A)/(B) distinction is now load-bearing in *two*
places: cross-link emits the AssociatedWith edge only for (A);
sessions/graph nests under workspace only when AssociatedWith is
present. The chip surfaces (B) at the row level. The user's
intuition that "workspaces probably aren't a peer to cwd" is
exactly right — the data model needs both halves to agree before
the UX reads correctly.


| Class | What it means | Edge present |
|---|---|---|
| **A. Workspace-rooted** | Session cwd is inside `<workspace_root>/<member>/...`; the session is doing workspace work. | `AgentSession -- AssociatedWith → Workspace` |
| **B. Repo-shared** | Session touches a repo that is *also* a workspace member, but lives outside the workspace tree. | None at the session level; only `Workspace -- WorkspaceContainsRepo → Repo` plus `Session -- (cwd) → Repo`. |

Today's grouping promotes (B) to look like (A), and (B) dominates
for daily-driver setups where workspace members are popular repos.

## Three axes worth deciding independently

### Axis 1: What should Sessions/Graph do with workspaces?

| Option | Behavior | Pros | Cons |
|---|---|---|---|
| **1a. Strict-only** | Nest under workspace iff (A). (B) falls through to repo-level. | Smallest change; fixes the lie immediately. | Loses the cross-reference signal for (B). |
| **1b. Strict + chip** | (1a) plus a `[ws-name]` chip on (B) rows so the cross-reference is still visible without false nesting. | Preserves the signal without the conflation. | Adds chip semantics needing a legend; rows with N>1 workspace memberships need a degradation rule. |
| **1c. Drop workspace level entirely** | Sessions/Graph nests repo → checkout → session only. Workspaces never appear here. | Cleanest separation. | Loses the affordance even for (A)-class sessions where workspace genuinely *is* the user-facing context. |

**Recommendation: 1b.** Tracked as `H-WS-001`.

Open knobs for `H-WS-001`:
- **Chip cardinality.** A repo in N workspaces: render `[ws-a]`,
  `[ws-a +N]`, `[N ws]`, or omit when N > some threshold.
- **Chip on (A) too?** Strong workspace-rooted sessions already nest
  under the workspace header; double-marking with a chip would be
  noise. Default: chip is (B)-only.

### Axis 2: A dedicated Workspaces view

Strawman tree:

```
Workspaces  (group-by: provider | activity | repo | flat)
├── atelier  (provider=atelier, 3 members)
│   ├── members
│   │   ├── atelier-repo
│   │   ├── conspectus
│   │   └── config
│   ├── in workspace  (A-class sessions)
│   │   ├── codex:plan-revamp    (cwd=~/atelier/conspectus/...)
│   │   └── claude:demo-fork
│   └── related            (B-class, collapsed by default)
│       ├── codex:bug-fix         (~/src/conspectus)
│       └── ...
├── multi-task  (provider=agent-deck, 2 members)
│   └── ...
```

Distinguishing the "in workspace" and "related" sections is the
key bit. (A)-class sessions are workspace-context work; (B)-class
sessions are cross-references. Defaulting "related" to collapsed
keeps the strong case clean while preserving the signal on demand.

Groupings:

- **provider** — separates atelier / agent-deck / generic
- **activity** — workspaces sorted by most-recent session touch
- **repo** — flipped: repo → workspaces it participates in → sessions
- **flat** — flat list of workspaces with summary stats

Filters (composable with the global filter set):

- harness, mux-state, provider, has-running-sessions, has-active-prs,
  activity-window.

Tracked as `H-WS-002`.

Open knobs for `H-WS-002`:
- **Whether to include (B)-class sessions at all** in the workspaces
  view. Recommendation: yes, collapsed by default. Counter: only A;
  treat workspaces as authored context and let cross-references stay
  in the Sessions view chip.
- **Workspaces with no activity.** Default to "show all"; offer an
  "active in last 7d" filter. A long quiet list of dormant
  workspaces could clutter the view but excluding them by default
  hides legitimate provider-emitted state.
- **Detail-pane integration.** Workspaces already expand to `root`,
  `provider`, `name`, `member: ...` rows (landed in
  `4bd3829`). No new detail work expected for this view.

### Axis 3: Other views (Mux/Prs/Forks/Union)

Each of these has a `Workspace` grouping option today that likely
suffers from the same (A)/(B) conflation:

| View | Risk |
|---|---|
| Mux/Workspace | groups muxes by session→workspace→mux chain. Probably has the weak-membership bug. |
| Prs/Workspace | PRs grouped via branch→repo→workspace. Strongly affected: every PR on a workspace-member repo gets pulled in. |
| Union/Workspace | union is intentionally permissive; lower priority. |
| Forks/Workspace | direct workspace→fork edge, low risk. |

Tracked as `H-WS-003` (audit, deferred until `H-WS-001` / `H-WS-002`
ship).

## Recommended sequence

1. **`H-WS-001`** — strict-only + chip in Sessions/Graph. Single-file
   change in `rows/sessions.rs`. Immediate UX win, no new view, no
   new ADR required.
2. **`H-WS-002`** — Workspaces view with provider/activity/repo/flat
   groupings, separate "in workspace" vs "related" subgroups.
   New `WorkspacesGrouping` enum, new row builder. ADR records the
   (A)/(B) distinction as a load-bearing model decision.
3. **`H-WS-003`** — audit Mux/Prs/Forks/Union workspace groupings
   for the same conflation; fix or defer per finding.

## Open questions for design review

1. Is `1b`'s chip the right surface, or does the user prefer the
   nuclear option (`1c`) — workspaces out of Sessions entirely,
   handled exclusively in the Workspaces view?
2. Should (B)-class sessions appear in the Workspaces view at all?
3. Should the default Workspaces view filter to "has activity"?
4. Naming: is `Workspaces` the right view label, or something more
   evocative (`Composition`, `Bundles`, `Worktrees`)? The current
   nomenclature in code/ADRs uses `Workspace`; keep for now.
5. Does the existing `MuxGrouping::Workspace` / `PrsGrouping::Workspace`
   need fixing before this lands, or are those low-traffic enough
   to defer (`H-WS-003`)?

These questions become the dispositional checklist for the
follow-up ADR (`docs/adr/00NN-workspace-view-redesign.md`) at the
point `H-WS-002` enters scoping.
