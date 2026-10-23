# Worktrees in Conspectus

Operators running several agents against one repo lean on **git
worktrees** — one linked working tree per branch / task / agent.
Conspectus discovers those worktrees as first-class rows and adds an
operator surface for the whole life of a "stream of work": create it,
work in it, land or discard it, and clean up.

This is the plain answer to *"what worktree operations does Conspectus
support?"* For the reasoning behind each, see the ADRs linked at the
bottom.

## The model in one paragraph

A worktree is **not a new kind of node** — it is a `Checkout` (the
concrete editable working tree of a repo, ADR 0026), distinguished from
a plain clone by source metadata: linked vs primary, the branch it
checks out, and its locked / prunable status. Discovery always surfaces
worktrees read-only. **Mutation is delegated**: Conspectus never runs
`git worktree add/remove` itself (ADR 0143 keeps "never mutate git
state" absolute). Instead it drives an external, mutation-capable
backend — [`worktrunk`](https://github.com/max-sixty/worktrunk) (`wt`)
— as a subprocess. No backend ⇒ listing still works, but the mutating
operations are hidden (TUI) or error clearly (CLI).

## Setup

- **Read-only listing** needs nothing but `git`.
- **Everything that mutates** (new / rm / merge / close / prune, and
  creating the worktree for a worktree-backed `mux launch`, and rebuilding a gone worktree) needs a mutation backend.
  Install `worktrunk` and it is autodetected on `PATH`, or pin it
  explicitly:

  ```toml
  [worktree]
  backend = "auto"   # "auto" (default) | "git" (read-only) | "worktrunk"
  ```

## Operations at a glance

| Operation | CLI | TUI | What it does | Needs backend |
|---|---|---|---|---|
| **List** | `worktree list` | rows in the tree | Enumerate a repo's worktrees (branch, linked/primary, locked/prunable) | no |
| **New** | `worktree new <branch>` | `w` menu → *New worktree* | Create a worktree + branch. Does **not** launch anything | yes |
| **Remove** | `worktree rm <branch>` | `w` menu → *Remove worktree* | Remove a worktree. Refuses if a live session is rooted in it (unless `--force`) | yes |
| **Merge back & close** | `worktree merge <branch>` | `w` menu → *Merge back & close* | Squash + rebase + fast-forward the branch into its target, then remove the worktree | yes |
| **Close down a stream** | `worktree close <branch> --merge\|--discard` | `X`, or `w` menu → *Close down stream* | Compound teardown: end the mux/agent sessions in the worktree, land or discard the branch, remove the worktree, forget its history records | yes |
| **New stream** (worktree-backed launch) | `mux launch <harness> --name N --worktree-branch <branch>` | `N` (mux launch form, worktree toggle pre-enabled) | Create the branch's worktree if absent and launch the mux + agent in it | yes |
| **Rebuild a worktree** | `rebuild <pin\|h_…>` | row action menu (`.`) → *Rebuild* | Bring a gone worktree back through `wt switch`, creating a deleted branch from its last head | yes |
| **Prune merged** | `worktree prune` | `w` menu (repo) → *Prune merged worktrees* | Remove worktrees already merged into the default branch (worktrunk `step prune`) | yes |
| **Reveal** | — | `w` menu → *Reveal checkout / sessions* | Jump the selection to the checkout containing a session, or a worktree's first live session | no |

TUI entry points: **`w`** opens the context-sensitive worktree action
menu for the selected repo / worktree / mux; **`X`** jumps straight to
close-down; **`N`** opens the mux launch form with the worktree toggle
already on (Space toggles it off for a plain launch).

## Two concepts worth understanding

### Close-down (winding a stream down)

"Close down" is the compound inverse of starting a stream. In order it:

1. **Terminates the mux/agent sessions** rooted in the worktree —
   graceful first (`SIGTERM` to the pane process, a short grace window),
   then a hard `kill-session` if it's still alive. Transcripts are
   preserved; this ends the *running process*, not the harness's
   records (ADR 0093).
2. **Lands or discards the branch** — `--merge` merges it back,
   `--discard` drops it. The choice is required, so uncommitted work is
   never silently lost.
3. **Removes the worktree** and **forgets the gone history records**
   rooted in it (ADR 0140), so its ghost muxes and checkout don't litter
   History. Records a pin targets are kept.

Confirmation follows `[worktree] teardown_confirm` (`always` | `live` |
`never`, default `live` — prompt only when a live session would be
ended). `teardown_grace` (default `3s`) is the graceful window; `0s`
skips straight to the hard kill. CLI flags `--yes` and `--grace <dur>`
override per-invocation.

### New-stream: worktree-backed launches and rebuilds

`mux launch --worktree-branch <branch>` (with `--worktree-repo <path>`
when the working directory isn't inside the repo) creates the branch's
worktree if it is absent and launches the mux and agent in it.
Realization is idempotent: an existing worktree for the branch is reused,
and nothing is created until the launch runs (ADR 0094).

A pinned worktree-backed mux remembers its branch as the
`worktree_branch` field of its recipe. If the worktree is later removed,
`conspectus rebuild <pin>` plans the worktree first (`wt switch`,
creating a deleted branch from its last head) and then the mux inside it
(ADR 0142). `rebuild --dry-run` shows both steps before anything runs.

## Config keys

All under `[worktree]`:

| Key | Values | Default | Meaning |
|---|---|---|---|
| `backend` | `auto` \| `git` \| `worktrunk` | `auto` | Which backend performs mutation (`git` = read-only) |
| `teardown_confirm` | `always` \| `live` \| `never` | `live` | When close-down prompts |
| `teardown_grace` | duration (`3s`, `500ms`, `0s`) | `3s` | Graceful window before the hard kill (`0s` skips it) |

## Guardrails and current limits

- **Read-only is the default.** Discovery never mutates git; every
  mutating operation is an explicit operator gesture through worktrunk.
- **Live-session guard.** `rm` / `merge` refuse a worktree that hosts a
  live agent/mux session unless forced; close-down is the sanctioned way
  to end those sessions first.
- **`prune` is a *merged-cleanup*, not git's stale-admin prune.** It
  removes worktrees whose branch is already merged into the default
  branch — distinct from the stale-admin cleanup the discovered
  `prunable` flag reflects. Use `--dry-run` to preview.
- **No lock/unlock.** worktrunk exposes neither, and mutation stays
  routed through worktrunk rather than raw git, so lock/unlock aren't
  offered.

## Where to go deeper

- **ADR 0092** — the worktree backend seam (read/mutate split, why
  mutation is delegated, `Checkout`-not-new-node modeling).
- **ADR 0093** — operator-initiated mux teardown (the graceful→hard
  `kill-session` that powers close-down).
- **ADR 0094** — worktree-backed launches realized at launch (new-stream).
- **ADR 0142** — rebuild plans, including worktree prerequisites.
- **`docs/design.md` §Worktree Management** — the north-star framing.
