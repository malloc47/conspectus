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

This means the (A)/(B) distinction is load-bearing at the
data-model layer: cross-link emits the `AssociatedWith` edge only
for (A); sessions/graph nests under workspace only when
`AssociatedWith` is present. Originally the chip surfaced (B) at
the row level too — that surface was removed by ADR 0063, so
(B) now has no UI representation; only the model-layer
distinction remains. The user's intuition that "workspaces
probably aren't a peer to cwd" is exactly right — the data model
needs both halves to agree before the UX reads correctly.

### Subsequent discovery: workspace-root launch shape + activeness gate

After landing the canonical-checkout fix, an operator smoke test
showed two more issues, both surfaced by inspection of a real
agent-deck setup with four multi-repo-worktrees:

1. **Workspace-root launches were silently (B)-classed.** Agent-deck
   launches the harness with `cwd = <multi-repo-worktree>/<id>`
   (the composite directory itself), not inside a specific member
   subdir. `workspace_member_roots` only indexed member
   `logical_path` values, so sessions at the workspace root sat
   above every member path and matched none. With zero (A)-class
   sessions, every (B)-class session got a chip and the cross-
   reference signal turned into a wall-of-chips on every shared
   repo. The fix: also index the workspace's own `root` in
   `workspace_member_roots`. The matching-workspaces logic still
   picks the deepest path per workspace, so a session in a
   specific member subdir still attributes via the member; a
   session at the workspace root attributes to the root. This
   fix is retained beyond ADR 0063: it is required for the
   strict-nesting outcome to identify workspace-rooted
   agent-deck launches correctly, independent of the chip.

2. **Dormant workspaces still produced chips.** [Resolved by
   ADR 0063 in a more aggressive way: chip removed entirely.]
   The historical fix here was to gate `weak_workspace_chip` on
   `active_workspaces()`. With the chip gone, the gate, the
   `weak_workspace_chip` helper, and `active_workspaces` itself
   are all removed. The wall-of-chips problem this finding
   diagnosed is now impossible by construction.


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

**Initial recommendation: 1b.** H-WS-001 shipped 1b — strict
nesting plus a (B)-class chip with an activeness gate and a
3-membership cardinality threshold.

**Resolved (ADR 0063): 1a.** After running 1b, the operator
reported the (B) cross-reference signal was not actionable — the
useful workspace relationship is strict nesting (A); knowing a
session's repo happens to be claimed by some workspace, without
the session being workspace-rooted, did not change any decision.
The chip and its supporting helpers are removed; the strict
nesting from 1a remains. The chip cardinality and on-(A)
sub-decisions below are mooted by the same ADR.

**Further reshape (ADR 0064 / `H-WS-004`): hybrid.** After 0063
landed, the operator surfaced two further problems with 1a's
shape: the repo level beneath a workspace duplicated the
project context, and agent-deck A-class sessions (cwd at
workspace composite root, no checkout) silently dropped into
"ungrouped" because `resolve_group_key`'s
`checkout_for_path(cwd)?` early return ran before the
workspace lookup. ADR 0064 reshapes Graph so workspaces and
repos are peer top-level parents, each with sessions directly
underneath at depth 1. The workspace level no longer requires
the session to be in a member checkout; the routing is
purely "does this session carry an `AssociatedWith Workspace`
edge." Workspace headers use the shared
`format_workspace_display` helper from `rows/mod.rs` so they
read identically to the Workspaces view.

~~Open knobs for `H-WS-001`:~~ resolved by ADR 0063 (chip
removed):
- ~~**Chip cardinality.** A repo in N workspaces: render `[ws-a]`,
  `[ws-a +N]`, `[N ws]`, or omit when N > some threshold.~~
- ~~**Chip on (A) too?** Strong workspace-rooted sessions already nest
  under the workspace header; double-marking with a chip would be
  noise. Default: chip is (B)-only.~~

### Axis 2: A dedicated Workspaces view

After the H-WS-002 MVP and the H-WS-002 polish (ADR 0062), the
shape is:

```
Workspaces  (group-by: provider | activity | repo | flat)
├── atelier-ws  conspectus+config+atelier-repo  (atelier)
│   ├── codex:plan-revamp    (cwd=~/atelier/conspectus/...)
│   └── claude:demo-fork
├── multi-task  conspectus+config  (agent-deck)
│   └── ...
```

Each workspace top-level row carries the member-repo list inline
as a `+`-joined span (matching the agent-table workspace column
convention from ADR 0060) and a parenthesized provider chip.
Below the workspace sit only its (A)-class sessions — direct
`AssociatedWith Workspace` edges — at depth 1, with no labeled
subgroup wrapper. (B)-class cross-references are not surfaced
anywhere in the UI after ADR 0063 — the Sessions / Graph chip
that originally carried them has been removed; only the
data-model edge remains for downstream tooling.

The original H-WS-002 strawman had three labeled subgroups per
workspace (`members` / `in workspace` / `related`). Operator
feedback flagged `members` as left-tree noise (already in the
detail pane) and the `in workspace` / `related` vocabulary as
unintuitive. ADR 0062 records the narrowing and the rationale.

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
  view. ~~Recommendation: yes, collapsed by default.~~ Resolved by
  the H-WS-002 polish: **only (A)** (ADR 0062). The downstream
  ADR 0063 then removed (B) from the Sessions view chip too, so
  (B) has no UI representation in any view today.
- **Workspaces with no activity.** Default to "show all"; offer an
  "active in last 7d" filter. A long quiet list of dormant
  workspaces could clutter the view but excluding them by default
  hides legitimate provider-emitted state.
- **Detail-pane integration.** Workspaces already expand to `root`,
  `provider`, `name`, `member: ...` rows (landed in
  `4bd3829`). No new detail work expected for this view.

### Axis 3: Other views (Mux/Prs/Forks/Union)

The original speculation was that each of these views had a
`Workspace` grouping that probably suffered from the same (A)/(B)
conflation. The `H-WS-003` audit found something different: the
variants are unimplemented, not buggy.

| View | Audit finding |
|---|---|
| Mux/Workspace | `src/tui/rows/mux.rs:191` lumps `Session \| Workspace \| Host` together and calls `emit_flat`. No headers emitted. |
| Prs/Workspace | `PrsBuildInputsFromConn` has no `grouping` field; `PrsGrouping` is unused outside `tui/mod.rs`. |
| Forks/Workspace | `ForksBuildInputsFromConn` has no `grouping` field; `ForksGrouping` is unused outside `tui/mod.rs`. |
| Union/Workspace | `UnionBuildInputsFromConn` has no `grouping` field; `UnionGrouping` is unused outside `tui/mod.rs`. |

A secondary finding: for Prs/Forks/Union the (A)/(B) distinction
does not translate cleanly. PRs and forks have no cwd; the only
edge from these node kinds to a workspace is the
branch→repo→workspace chain, which is structurally the (B)
"weak membership" case at the session level. There is no analog
of "this PR is workspace-rooted."

`H-WS-003` closed by dropping the `Workspace` variant from
`MuxGrouping`, `UnionGrouping`, `PrsGrouping`, and `ForksGrouping`
(ADR 0061). The Workspaces view from `H-WS-002` is the canonical
workspace-first surface.

## Recommended sequence

1. **`H-WS-001`** — strict-only + chip in Sessions/Graph. Shipped
   1b (strict-nesting + chip); ADR 0063 later reverted the chip
   to land at 1a. Strict nesting and the cross-link inference
   fix are retained.
2. **`H-WS-002`** — Workspaces view with provider/activity/repo/flat
   groupings, separate "in workspace" vs "related" subgroups.
   MVP shipped with three subgroups; the polish (ADR 0062)
   folded `members` inline on the workspace row and dropped the
   `related` (B)-class subgroup, leaving only (A)-class sessions
   under each workspace.
3. **`H-WS-003`** — audit Mux/Prs/Forks/Union workspace groupings
   for the same conflation; fix or defer per finding. Closed by
   dropping the unimplemented `Workspace` variants (ADR 0061).

## Open questions for design review

1. ~~Is `1b`'s chip the right surface, or does the user prefer the
   nuclear option (`1c`) — workspaces out of Sessions entirely,
   handled exclusively in the Workspaces view?~~ Answered by
   ADR 0063: **no** — the chip itself was reverted, landing
   between 1a and 1c. Sessions / Graph keeps strict nesting for
   (A) (the 1a outcome) but no chip. The Workspaces view is the
   only place workspaces appear as primary organization.
2. ~~Should (B)-class sessions appear in the Workspaces view at
   all?~~ Answered by the H-WS-002 polish: **no** (ADR 0062).
   ADR 0063 further removed (B) from the Sessions view chip, so
   (B) has no UI representation in any view.
3. Should the default Workspaces view filter to "has activity"?
4. Naming: is `Workspaces` the right view label, or something more
   evocative (`Composition`, `Bundles`, `Worktrees`)? The current
   nomenclature in code/ADRs uses `Workspace`; keep for now.
5. ~~Does the existing `MuxGrouping::Workspace` /
   `PrsGrouping::Workspace` need fixing before this lands, or are
   those low-traffic enough to defer (`H-WS-003`)?~~ Answered by
   the `H-WS-003` audit: neither needs fixing because both were
   unimplemented. Variants dropped per ADR 0061.

These questions become the dispositional checklist for the
follow-up ADR (`docs/adr/00NN-workspace-view-redesign.md`) at the
point `H-WS-002` enters scoping.
