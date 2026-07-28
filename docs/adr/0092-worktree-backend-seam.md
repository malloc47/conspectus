# ADR 0092: Worktree Management Backend Seam

## Status

Accepted (design); implementation tracked as `H-WT-001` follow-ups in
`docs/backlog.md`.

## Context

Operators running several agents against one repo lean heavily on git
worktrees: one linked working tree per branch / task / agent, sharing a
single `.git`. Conspectus already *discovers* linked worktrees and
models them as `Checkout` nodes (ADR 0026, `docs/design.md` Core
Model), but it offers no way to *manage* them — create a worktree for a
new task, list the worktrees of a repo as a first-class surface, or
remove a finished one.

`H-WT-001` asks for first-class worktree management with a **pluggable
backend** seam, targeting [`worktrunk`](https://github.com/max-sixty/worktrunk)
as a rich backend and a thin built-in `git` wrapper as an
always-available fallback.

The tension is ADR 0087 prohibition 6: **Conspectus never mutates git
state** — no `git commit`, `git checkout`, `git push`, "or any command
that changes the repo state." `git worktree add` / `git worktree
remove` change repo state. ADR 0087 already anticipated this: "Fork /
workspace operations that need git mutation belong to Atelier or
another dedicated tool." Worktree creation is exactly such an
operation.

## Decision

Add a **worktree backend seam** that cleanly separates read-only
discovery (always available, Conspectus-owned) from mutation
(delegated to an external dedicated tool, never Conspectus running git
itself).

### Model

No new node type. A worktree is a `Checkout` (ADR 0026): a concrete
editable working tree of a repo, linked to its repo (shared common
`.git`) and its current `Branch`. Worktree-specific facts (linked vs
primary, locked/prunable status, the branch it checks out) live as
`Checkout` attributes and source metadata, not a new node. This keeps
the graph provider-neutral: a worktree and a plain clone are both
checkouts, distinguished by metadata.

### Backend trait

```text
trait WorktreeBackend {
    fn key(&self) -> &'static str;             // "git", "worktrunk"
    fn list(&self, repo) -> Result<Vec<WorktreeRecord>>;   // read-only, required
    fn capabilities(&self) -> WorktreeCaps;    // can_create / can_remove
    fn create(&self, req) -> Result<WorktreeOutcome>;      // optional (mutation)
    fn remove(&self, req) -> Result<WorktreeOutcome>;      // optional (mutation)
}
```

- **`list` is read-only and required.** Every backend can enumerate a
  repo's worktrees. The thin `git` backend implements it via
  `git worktree list --porcelain` (a read command — reading worktree
  metadata is already sanctioned; ADR 0087 permits reading git common
  dirs and worktree metadata).
- **`create` / `remove` are optional and mutation.** Only backends
  that are themselves the "dedicated tool" implement them.

### Two v1 backends

1. **`git` (thin, built-in): read/list only.** Wraps
   `git worktree list --porcelain`. It does **not** implement
   `create`/`remove`; its `capabilities()` reports both false. This
   keeps ADR 0087 prohibition 6 absolutely intact — the built-in
   backend never runs a git command that changes repo state.

2. **`worktrunk` (external): create/list/remove.** When the operator
   has `worktrunk` installed and selects it, Conspectus shells out to
   the `worktrunk` binary for create/remove. Conspectus constructs the
   argv from ADR-scoped inputs (repo path, branch/name, base ref) and
   launches the subprocess — an ADR 0087 **category 4** operation
   (subprocess launches Conspectus owns end-to-end, with
   Conspectus-constructed argv, never operator-typed argv verbatim).
   `worktrunk` is the "dedicated tool" that owns the git mutation;
   Conspectus never runs `git worktree add/remove` itself.

### Envelope placement

ADR 0087 prohibition 6 is **unchanged**. Conspectus does not gain a
git-mutation surface:

- Discovery / `list` is read-only (permitted today).
- `create` / `remove` are **delegated** to `worktrunk` as a category-4
  subprocess launch, exactly like `pin launch` re-execs a harness or
  the resume path spawns a resume command. The git mutation happens
  inside `worktrunk`, the dedicated tool ADR 0087 points to — not
  inside Conspectus.
- The built-in `git` backend deliberately stops at read/list, so
  "Conspectus itself runs `git worktree add`" never happens.

This is why the "delegate mutation" shape was chosen over amending
prohibition 6 to let a thin wrapper run `git worktree add` directly:
the delegation fits the existing envelope with no new mutation
category and no weakening of the git-state prohibition.

### Surfaces

- **Discovery** always lists worktrees per repo and folds them into
  the graph as `Checkout` nodes (enriching what ADR 0026 already
  models). This is backend-agnostic — the `git` backend's `list`
  suffices.
- **CLI**: `conspectus worktree list [<repo>]` (read). `conspectus
  worktree new`/`rm` delegate to the configured mutation-capable
  backend (worktrunk); they error with a clear message when no such
  backend is available rather than falling back to raw git.
- **TUI**: worktree create/remove as operator-initiated actions,
  available only when a mutation-capable backend is configured,
  surfaced menu-first (consistent with the pins/controls overlays).

### Backend selection

A `[worktree] backend = "git" | "worktrunk"` config key (default
`git`, i.e. read-only) plus autodetection: if `worktrunk` is on
`PATH`, offer it for mutation; otherwise mutation actions are hidden /
error. Selection mirrors the mux-backend and forge-adapter registries
(ADR 0089, H-EXT-012) so a third backend (e.g. a jj worktree tool)
slots in by registration.

## Consequences

- Worktree *visibility* improves immediately and everywhere (all
  backends list), because `list` is required and read-only.
- Worktree *mutation* is opt-in and delegated: operators who install
  `worktrunk` get create/remove; everyone else keeps a read-only,
  ADR-0087-clean tool. No operator is ever surprised by Conspectus
  mutating their git state through the built-in path.
- ADR 0087 stays intact — no new mutation category, prohibition 6
  unchanged. The one envelope note is that a worktree-mutation
  subprocess launch (worktrunk) is a category-4 launch and must cite
  ADR 0087, like every other subprocess Conspectus owns.
- The `Checkout` model absorbs worktrees without a new node type,
  keeping the graph provider-neutral (a jj or hg worktree-equivalent
  maps to the same `Checkout` with different source metadata).

## Alternatives Considered

**Amend ADR 0087 to add a git-mutation category so a thin built-in
backend can run `git worktree add/remove`.** Rejected (operator
direction). It reopens a git-mutation surface the project deliberately
closed, for marginal benefit over delegating to `worktrunk`, which
already does the job as a dedicated tool. Delegation keeps the trust
contract ("Conspectus never changes my git state itself") crisp.

**A dedicated `Worktree` node type.** Rejected. Worktrees are
checkouts; a parallel node type would fork the model and complicate
every consumer that already handles `Checkout`. Worktree-ness is
metadata on a checkout.

**Depend on Atelier for worktree operations.** Rejected for v1 per the
CLAUDE.md shape rule ("avoid making Conspectus depend on Atelier
command modules directly"). `worktrunk` is a standalone binary reached
by subprocess, not a code dependency, which respects that boundary.

## Open Questions

- **How `worktrunk`'s naming / base-ref conventions map to
  Conspectus's create request.** Needs a read of `worktrunk`'s CLI
  surface before the create/remove argv is fixed; deferred to the
  implementation story.
- **Whether worktree removal should refuse when the worktree hosts a
  live agent/mux session.** Likely yes (warn + require confirmation),
  reusing the session↔checkout links the graph already resolves.
  Deferred.

## Related ADRs

- ADR 0026 (`Checkout` as the provider-neutral working-tree term) —
  the node worktrees map onto.
- ADR 0087 (mutation envelope) — prohibition 6 (git state) unchanged;
  worktrunk create/remove is a category-4 delegated subprocess launch.
- ADR 0089 (mux backend trait) — the registry/seam pattern this
  backend seam mirrors.
