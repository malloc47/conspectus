# ADR 0094: Worktree-Backed Pins Realized at Launch

## Status

Accepted

## Context

The worktree interaction epic (`docs/backlog.md` §CSP-523, "new
stream") wants creating a pin — the natural "I'm starting a new stream
of work" gesture — to be able to spin up a **fresh worktree** for that
stream. The operator picks a branch, and the pin's session should run
in a worktree checking out that branch rather than in a shared
checkout.

A pin (ADR 0057) is a **declaration**, not a running thing. Writing a
pin never starts a mux or an agent; those come into being later, when
the pin is **launched** (`conspectus pin launch` / the TUI launch
path), which constructs the mux session and the agent process. So the
question CSP-523 raises is: **when a pin declares a worktree, when is
that worktree actually created?**

Creating it eagerly at pin-write time is tempting but wrong-shaped:

- It mutates git-worktree state as a side effect of saving a
  declaration, which no other pin write does. A cancelled or never-run
  pin would leave an orphaned worktree.
- The worktree's on-disk path is chosen by worktrunk, so it isn't even
  known at declare time — the pin can't record a meaningful `cwd` for
  a worktree that doesn't exist yet.

The realization model already has a natural moment where the worktree,
the mux, and the agent come into being **together**: launch. That is
where this ADR places worktree creation.

## Decision

A pin may be **worktree-backed**: it declares a branch, and the
worktree for that branch is created (if absent) and entered **at launch
time**, alongside the mux + agent it already constructs.

### Schema

`PinEntry` gains an optional block:

```toml
[[pins.entry]]
id = "feature-x"
cwd = "/home/me/src/app"        # the repo anchor (`-C` path)
harness = "codex"
[pins.entry.mux]
backend = "tmux"
name = "feature-x"
[pins.entry.worktree]
branch = "feature-x"
```

- `cwd` stays a valid absolute path — the **repo anchor** the worktree
  is created under (the `-C` path worktrunk operates from). It is *not*
  the worktree path (which isn't known until the worktree exists).
- `[worktree] branch` is the branch the stream runs on. Absent block =
  today's behavior (run directly in `cwd`).

### Realization at launch

When launching a worktree-backed pin, before constructing the mux:

1. **Resolve.** List the repo's worktrees (read-only git backend) and
   find the one checking out `branch`.
2. **Create if absent.** If none exists, create it via the configured
   mutation backend — `wt -C <cwd> switch --create <branch>`, a
   category-4 subprocess launch (ADR 0087). Base ref is the repo
   default branch (matching `worktree new` with no `--base`).
3. **Re-resolve** the freshly created worktree's path.
4. **Launch there.** The mux/agent is constructed with the worktree
   path as its working directory instead of `cwd`.

Realization is **idempotent**: an existing worktree for the branch is
reused, so re-launching a pin (or launching after a manual
`worktree new`) doesn't error or duplicate. If the worktree can't be
created (read-only host, backend failure), launch fails with a clear
message rather than silently running in the wrong directory.

### Confirmed defaults (CSP-523)

- **Base ref**: the repo's default branch.
- **Name derivation**: the create form's branch field defaults to the
  pin's derived id; branch / id / `mux.name` are aligned by default but
  independently editable. The existing id/mux derivation is untouched.
- **Toggle scope**: the "create a worktree" toggle is offered in all
  create-form modes. In modes that bind to an already-live mux (adopt),
  the declared branch is recorded but realization is a no-op because
  the session already exists — the toggle degrades to a plain
  declaration rather than being hidden.

### Envelope placement

This introduces **no new write path**. Writing a worktree-backed pin is
the same user-intent TOML store write as any other pin (ADR 0087
category 1). The worktree creation at launch is the **already-sanctioned
category-4 subprocess launch** the worktree backend performs (ADR 0092),
now reached from the launch path in addition to `worktree new`. Launch
is operator-initiated and foreground, so ADR 0087 prohibition 5
(background mutation) is untouched.

## Consequences

- "Create a pin for a new stream, in its own worktree" becomes a single
  declaration; the worktree materializes the first time the operator
  launches it, next to the mux and agent.
- The launch path (CLI + TUI) gains a pre-step: resolve-or-create the
  worktree and rewrite the working directory. Non-worktree pins are
  unaffected (the block is absent).
- The pin schema grows one optional block; older stores parse
  unchanged (`#[serde(default)]`).
- Re-launch and manual-worktree-first flows both work because
  realization is idempotent.
- A worktree-backed pin on a read-only host (no mutation backend) can be
  declared but not realized; launch reports the missing backend, the
  same message `worktree new` already gives.

## Alternatives Considered

**Create the worktree at pin-write time.** Rejected — it mutates git
state as a side effect of saving a declaration (unlike every other pin
write), can't know the worktree path at declare time, and orphans a
worktree if the pin is cancelled or never launched.

**Store the resolved worktree path directly in `cwd`.** Rejected — the
path doesn't exist at declare time and worktrunk owns worktree layout,
so there's nothing valid to store. Anchoring on the repo + branch and
resolving at launch keeps the declaration truthful.

**Hide the toggle outside fresh-create mode.** Considered; the operator
chose to surface it in all create modes for uniformity. Adopt-mode
realization is a no-op (the session already exists), so the branch is
recorded as intent without a contradictory second creation.

## Related ADRs

- ADR 0057 (session pins) — the declaration this extends; launch is the
  realization moment worktree creation joins.
- ADR 0087 (mutation envelope) — the pin write stays category 1; the
  launch-time worktree create is category 4, no new path.
- ADR 0092 (worktree backend seam) — the create primitive reused at
  launch.
- ADR 0093 (operator-initiated mux teardown) — close-down is the
  inverse gesture; together they bracket a stream's life.
