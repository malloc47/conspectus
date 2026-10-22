# ADR 0142: Rebuild Plans And Executors

## Status

Accepted. Supersedes ADR 0094 (worktree-backed pins realized at
launch). Amends ADR 0093 (a torn-down mux leaves a ghost), ADR 0095
and ADR 0096 (adopting the result as a pin gives way to `--pin`), and
ADR 0098 (the splice applies to any effective argv). The worktree
location rule below is provisional until backlog `CSP-688` reports.
Part of the history, rebuild, and pin slots plan with ADRs 0140, 0141,
0143, and 0144.

## Context

Today the only way back from a dead mux is `pin launch` (ADR 0057),
which recreates a pinned mux, resumes its last session through the
binding sidecar (ADR 0058) with the resume tokens spliced into the
pin's argv (ADR 0098), falls back to a fresh launch when the resume is
rejected (ADR 0103), types the launch argv into a live mux whose agent
exited (ADR 0102), and, for a worktree-backed pin, creates the worktree
first (ADR 0094). The TUI runs it as `conspectus pin launch <id>
--no-attach` and then attaches.

History (ADR 0140) records every mux, checkout, and repo, so anything
recorded can be rebuilt, not only pinned muxes. The operator chose this
milestone's scope: a mux with its command, resuming its last session; a
session resumed into a mux; a linked worktree; a mux whose worktree is
gone; and a repo, by cloning it.

## Decision

### A rebuild is a plan

A **rebuild** turns a recipe back into a live entity. The recipe comes
from a pin slot's effective recipe (ADR 0141), from a history record,
or from a live node. A rebuild is first a **plan**, an ordered list of
steps, which the TUI shows before running and `--dry-run` prints:

```
rebuild mux "conspectus-feature" (gone, last seen 2d ago)
  1. rebuild worktree feature/ledger   wt -C ~/src/conspectus switch feature/ledger
  2. new tmux session                  tmux new-session -d -s conspectus-feature -c <worktree> …
  3. resume session e44d01cb           claude --resume e44d01cb --dangerously-skip-permissions
  4. attach                            tmux attach-session -t conspectus-feature
```

Each kind contributes an **executor** that plans its own steps and can
declare **prerequisites**, which are other rebuilds planned ahead of
it. Running a plan stops at the first failed step and reports which
step failed and which steps already ran. Nothing is rolled back: every
step leaves something the operator can see and remove.

```
conspectus rebuild <ref> [--dry-run] [--no-attach] [--session <id>]
conspectus rebuild --pinned [--dry-run]
```

`<ref>` is a slot id, a history record id (`h_…`), or a node reference
(a node id, or a mux's `backend:name`). `--session` picks which of a
mux's hosted sessions to resume. `--pinned` plans every pin slot whose
target is gone, continues past individual failures, and reports each
(backlog `CSP-686`).

### Executors

| Executor | Steps | Envelope (ADR 0143) |
|---|---|---|
| Mux, gone | `new-session` with the effective argv and the last session's resume spliced in, watched briefly; a rejected resume falls back to a fresh launch (ADR 0103); write the history record; attach | Category 3 |
| Mux, live, agent running | Attach | Category 3 |
| Mux, live, agent exited | `send-keys` the Conspectus-built argv into the pane, then attach. Only when pane-process evidence shows no harness running there (ADR 0102, ADR 0028); otherwise attach | Category 3 |
| Mux, live, pane dead | `kill-session`, then as for gone (ADR 0103) | Category 3 |
| Mux, zellij, exited | `zellij attach <name>`, which resurrects it; a session zellij no longer lists is created by `zellij attach --create <name>` | Category 3 |
| Session | Its last mux's executor with this session as the resume target. The last mux is the most recent one that hosted it in history. When that mux is live and running another session, a new mux named after it (`<name>-2`, …); when the session never had a mux, a new mux named after its harness and short id | Category 3 |
| Worktree | `wt -C <repo> switch <branch>`, or `wt -C <repo> switch --create --base <last head> <branch>` when the branch is gone. Prerequisite: the repo, when it's gone | Category 4 (ADR 0092) |
| Repo or primary checkout | `git clone <remote> <path>` into a path that doesn't exist, from the first recorded fetch remote unless the slot overrides it | Category 6 |
| Mux on a gone worktree | The worktree executor, then the mux executor in the rebuilt worktree | Composition |

A mux whose cwd is inside a gone checkout's root, or whose slot sets
`worktree_branch`, gets the checkout as a prerequisite. A planned mux
slot (ADR 0141) rebuilds the same way and is then retargeted at the
record the launch wrote, as part of the operator's gesture.

Orchestrator-owned muxes (agent-deck) are recorded and shown, but
their rebuild is refused with "owned by agent-deck", because the
orchestrator owns their lifecycle (backlog `CSP-451`).

### Argv

For a mux whose recipe names a harness, the effective argv is the
first of:

1. the slot's `argv` override,
2. the history record's recorded argv,
3. the project launch profile, `[launch.<harness>] argv` in the
   `.conspectus.toml` nearest the cwd,
4. the user launch profile, the same table in the user config,
5. a discovered `atelier exec <harness>` wrapper, when the cwd is
   inside an Atelier workspace and `atelier` is on `PATH`,
6. `HarnessAdapter::launch_argv`.

Resume tokens are spliced after the harness binary (ADR 0098), so
wrappers and options survive. A recorded argv that already resumes a
session (an observed `claude --resume X`) has those tokens removed
before the new ones go in. An argv whose harness binary can't be found
launches fresh with a note, as ADR 0098 says.

A mux with no harness (a shell, `emacsclient -c`, `conspectus serve`)
rebuilds with its recorded argv, or a login shell when none was
recorded.

### Worktree location (provisional)

Worktrunk's path template decides where a worktree lands, and it may
differ from the recorded root. For now the rebuild accepts worktrunk's
path. When the new root differs from the recorded one, the launch path
writes a new checkout record whose `rebuilt_from` names the old one,
retargets the slots that pointed at the old record (category 1, part
of the operator's gesture), and says so in the outcome. A slot's
`location` override that can't be honored is a warning in the plan,
not an error. Backlog `CSP-688` investigates where recorded roots come
from and whether exact locations should be honored.

### History writes

Every launch path, rebuilds included, writes the record of what it
created, with the argv it used (`argv_source = "conspectus"`,
`origin = "launched"`), under the history lock (ADR 0140), so a
rebuilt mux has its record before the next probe.

### In the TUI

`Enter` on a gone row, a planned slot, or a History row shows the plan
in a confirmation overlay. Confirming runs `conspectus rebuild <ref>
--no-attach` as a child process (category 4, as `pin launch` was run
before), then attaches and refreshes as launches do today (ADR 0127,
ADR 0133). A row awaiting its post-launch refresh refuses another
rebuild (ADR 0132).

### Retired

`pin launch` and `pin attach` give way to `rebuild <slot>`.
Worktree-backed pins (ADR 0094) become a mux slot with a
`worktree_branch` override, or a mux on a gone worktree, both of which
chain the worktree executor. `mux new` and `mux launch` stop
suggesting `pin adopt` and take `--pin` instead (ADR 0141).

Teardown (ADR 0093) and `worktree close` leave the mux's history
record behind, so a torn-down mux shows as a ghost and can be rebuilt.
`worktree close` forgets the gone records rooted in the worktree it
removed (ADR 0140), so closing a stream doesn't leave its ghosts.

## Consequences

- Any recorded mux, worktree, or repo can be rebuilt without a pin.
  After a reboot, every mux is a ghost and `rebuild --pinned` restores
  the pinned ones in one gesture.
- The operator always sees what a rebuild will run before it runs.
- Launch behavior carries over from `pin launch`: resume splicing,
  failed-resume fallback, the agent-exited path, and never typing into
  a running harness.
- Launch profiles let a plain `mux launch` start a harness the way the
  operator always starts it, without a pin.
- `git clone` enters the capability manifest, act context only.

## Alternatives Considered

- **Rebuild without showing a plan.** Faster for one mux, but a
  compound rebuild (worktree, then mux, then resume) does three things
  the operator should see first, and `--dry-run` falls out of having a
  plan anyway.
- **Roll back a failed plan.** Rolling back means removing a worktree
  or killing a session the plan just made, which are destructive steps
  of their own. Stopping and reporting leaves the operator in control.
- **Create worktrees with `git worktree add` at the recorded path.**
  Honors the location exactly, but it is a git mutation outside
  worktrunk, which the envelope keeps out (ADR 0092, ADR 0143). The
  location investigation may revisit this.
- **Keep `pin launch` as an alias.** The 0.2.0 version bump breaks the
  `pin` CLI on purpose (ADR 0141); an alias would keep a verb whose
  meaning changed.

## Open Questions Answered

- *What does rebuilding a live mux do?* Attach, or relaunch its exited
  agent; it never creates a second session with the same name.
- *Which session does a rebuilt mux resume?* The most recent session
  it hosted, or the one `--session` (the History view's "rebuild with
  this session") names.
- *Does a session rebuild ever type into a running agent?* No. A busy
  last mux gets a new mux instead.

## Related ADRs

- ADR 0018 — session lineage; a session rebuild resumes the head.
- ADR 0028 — never type into a live agent.
- ADR 0057, ADR 0058 — pin launch and resume; replaced.
- ADR 0092 — the worktree backend seam.
- ADR 0093 — teardown; amended.
- ADR 0094 — worktree-backed pins; superseded.
- ADR 0095, ADR 0096 — `mux new` and `mux launch`; amended.
- ADR 0097 — the launch-spec form.
- ADR 0098 — resume splicing; amended.
- ADR 0102 — the agent-exited path's pane-process check.
- ADR 0103 — failed-resume fallback and dead panes.
- ADR 0132, ADR 0133 — launches in the TUI.
- ADR 0140 — history.
- ADR 0141 — pin slots.
- ADR 0143 — the mutation envelope.
- ADR 0144 — where the TUI offers rebuilds.
