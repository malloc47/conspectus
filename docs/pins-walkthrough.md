# Pins, History, and Rebuild — A Walkthrough

## Why this exists

Your work doesn't think in tmux sessions or harness session ids. You think
"the Codex session for the ingest refactor in `~/work/ingest`." The tmux
session dies when the machine reboots, the worktree gets pruned, the
harness mints a new session id on every `/compact`. Conspectus keeps the
thing you care about on the dashboard anyway, and can bring it back.

That takes three cooperating layers (ADRs 0140–0142):

- the **graph** is what exists right now, rebuilt every discovery cycle;
- **history** is everything Conspectus has seen, including what is gone,
  with what it takes to rebuild it;
- **pins** are the entities you chose to keep at hand.

Pinning is deliberately light. It doesn't create anything, configure
anything, or bind anything: it adds a named pin to
`$XDG_DATA_HOME/conspectus/pins.toml` that points at an entity, live or
gone. `docs/operations.md` ("Pins, history, and rebuild") is the
reference; this walkthrough is the teaching counterpart.

## The mental model

- **History is automatic.** You never record anything. Every mux,
  worktree, repo, and PR Conspectus observes gets a record with a stable
  id (`h_…`), pinned or not.
- **Gone is a checked fact.** A mux is gone only when tmux answered and
  didn't list it. If the check couldn't run, the row says `unknown`.
  Absence from a scan never means gone.
- **A pin is a pointer plus overrides.** Each field of the recipe
  resolves `overrides > live graph > history > snapshot > defaults`, so
  until you override something a pinned mux behaves exactly like an
  unpinned one.
- **Rebuild is explicit.** Nothing is recreated in the background. You
  run `conspectus rebuild`, and you can read the plan first.

## A two-minute dry run

Try it in a scratch project. Everything here uses a throwaway tmux
session.

1. Start something to keep: `conspectus mux launch codex --name
   walkthrough --cwd ~/scratch`. Detach (`prefix d`).
2. Pin it: `conspectus pin add walkthrough` (or launch and pin in one
   step with `mux launch ... --pin`; `--pin --no-launch` writes a planned
   pin and starts nothing until its first rebuild). The argument is a mux name,
   a node id, or a history id. A pin named `walkthrough` appears in
   `conspectus pin list` with status `live`. `conspectus pin show
   walkthrough` lists each recipe field and which layer it came from.
3. Let it die: `tmux kill-session -t walkthrough`. On the next refresh the
   mux is **gone**, and the row is still there, dimmed, with its
   last-seen age. `conspectus history list --gone` shows the record, and
   `pin list` shows the pin as `gone`. In the TUI the Mux view dims the
   ghost, and the Pinned view (`4`) and History view (`5`) list it too.
4. Read the plan: `conspectus rebuild walkthrough --dry-run`. It prints
   each step: a `tmux new-session` with the recorded argv, with the last
   session's resume spliced in after the harness binary.
5. Run it: `conspectus rebuild walkthrough`. Conspectus recreates the
   session, resumes the same conversation, and hands the terminal to
   tmux. Pass `--no-attach` to get the attach command printed instead.
   If the harness rejects the resume, Conspectus falls back to a fresh
   launch and says so.
6. Unpin when you're done: `conspectus pin rm walkthrough`. The mux and
   its history record are untouched; only the pin goes.

## Changing what a rebuild does

Overrides win over everything else:

```sh
conspectus pin edit walkthrough --set argv="codex --model o3"
conspectus pin edit walkthrough --set cwd=~/scratch/sub
conspectus pin edit walkthrough --unset argv
```

Overridable fields depend on the kind (a mux takes `name`, `socket`,
`cwd`, `harness`, `argv`, and `worktree_branch`); `label` works
everywhere. `pin refresh walkthrough` copies the current recipe into the
pin's snapshot so it still renders and rebuilds if history is ever
lost. `pin retarget walkthrough <other mux>` points the pin at another
entity of the same kind, and `pin move walkthrough --top` reorders the
Pinned view.

## Planned pins

A pin doesn't need a target. A **planned** pin holds a recipe for
something that doesn't exist yet:

```sh
conspectus pin add --planned --kind mux --name ingest \
    --set cwd=~/work/ingest --set harness=codex
conspectus rebuild ingest
```

The first rebuild builds the entity, records it in history, and points the
pin at the record.

## When more is missing

Rebuild plans chain prerequisites. A mux pinned inside a worktree whose
directory has since been pruned rebuilds the worktree first (through
worktrunk's `wt switch`, so `wt` must be on your `PATH`), and a missing
clone is cloned from its recorded remote. `--dry-run` shows the whole
chain before anything runs. Orchestrator-owned muxes (agent-deck) are
shown but never rebuilt, and Conspectus never types into a pane that has
a live agent in it.

## Housekeeping

Gone records pile up slowly. `conspectus history reap --dry-run` lists
what would be removed under `[history.reap_after]` (14 days for muxes, 30
for checkouts and PRs by default), and dropping `--dry-run` removes it.
Pinned and unknown records are never reaped. `conspectus history forget
<h_…>` removes one record. `conspectus worktree close` forgets the gone
records rooted in the worktree it removes.

## Coming from the earlier pin model

If you have `[[pins.entries]]` in `.conspectus.toml` or your user config,
they still load as read-only pins after the real ones, with a
`LegacyPins` diagnostic. Convert them:

```sh
conspectus pin migrate --dry-run     # see each conversion
conspectus pin migrate
```

Each entry becomes a mux pin with the same name, pointing at a history
record seeded from its launch settings and from the session it last
bound. The old `pin create`, `launch`, `attach`, `bind`, `rebind`, and
`adopt` verbs are retired; they print what to use instead (`pin add`,
`rebuild`, `pin retarget`).

## What pins do *not* do

- They don't launch anything on their own; `rebuild` does.
- They don't bind sessions to muxes. Session-to-mux attribution stays with
  the general resolver, and a pin no longer renames a session (a session's
  name falls back from alias to title to mux name).
- They don't write to your project. Pins are personal and live in one
  user-scope file.
- They don't take over your tmux server or impose a window layout.

## Further reading

- ADR 0140 — History and presence.
- ADR 0141 — Pins.
- ADR 0142 — Rebuild plans and executors.
- ADR 0143 — The mutation envelope (what each command may write).
- ADR 0144 — The Pinned and History views.
- ADR 0018 — Intra-harness `parent_session` lineage, which rebuild uses to
  resume the latest session.
- `docs/operations.md` §"Pins, history, and rebuild" — the reference.
