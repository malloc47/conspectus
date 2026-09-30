# Pinning in Conspectus — A Walkthrough

## Why pins exist

Conspectus's dashboard is *reactive*: an `AgentSession` node only appears
when a harness has written evidence to disk. That's fine when you want a
snapshot of what's running right now, but it misaligns with how you
actually think about your work. You don't think of "Codex session
`01J9X…`"; you think of "the Codex session for the ingest refactor in
`~/work/ingest`." Those identities are stable across `/compact`,
`/resume`, machine reboots, and tmux session renames. Harness session
ids are not.

A **pin** is the missing layer: a TOML-persisted declaration of a
logical agent session — a `(harness, cwd, display_name, mux)` tuple.
It lives next to the declared links (ADR 0014) and aliases (ADR 0029)
you already use, but unlike those it renders as a first-class
dashboard row whether or not a live session realizes it right now.
When the resolver does see a matching live session, it binds the pin
to that session for the current cycle; when the session dies, the row
stays on the dashboard, dim and unbound, ready to be relaunched.

In short: pins replace agent-deck's "new card" surface for the core
dashboard workflow. They give you a durable handle on work that
outlives any individual harness process.

## The mental model in four bullets

- **A pin is just intent stored in TOML.** Project-local
  `.conspectus.toml` by default, user-level config when you pass
  `--store user`.
- **Binding is recomputed every discovery cycle, never persisted.**
  The resolver matches the pin's `mux.name` against live tmux
  sessions, then checks that a session of `pin.harness` is attributed
  to that mux. The result is one of: `Bound`, `PinUnbound` (mux
  missing), `PinStaleMux` (mux alive, harness absent), `PinAmbiguous`
  (multiple harness sessions attributed to one mux), `PinDrift`
  (advisory — bound but cwd diverged).
- **The pin's `display_name` doubles as the alias overlay** (so the
  bound session inherits its name) **and as the initial tmux session
  name at launch.** One name, three roles.
- **Lineage-aware continuity.** The resolver follows ADR 0018
  `parent_session` chains, so `/compact` and `/resume` don't break
  the binding. And a per-pin sidecar cache
  (`$XDG_CACHE_HOME/conspectus/pin-bindings/<id>.json`) remembers the
  last bound session so that the next `pin launch` after a mux death
  can splice in `--resume <id>` instead of starting cold.

## Lifecycle at a glance

```
pin create  ──►  pin launch  ──►  bound to live mux  ──►  pin attach
                                                              │
                  ▲                                            │
                  │                                            ▼
            pin adopt           ◄── operator already running tmux
            pin rebind          ◄── mux was renamed outside conspectus
            pin bind            ◄── multiple harness sessions claim the mux
```

The top row is the "starting fresh" path. The entries on the bottom are
recovery paths: none of them touch tmux, they only update TOML so the
resolver binds correctly on the next cycle.

## Commonly confused commands

**`create` vs `adopt`**: both write a new TOML entry, but they answer
different questions.

- `create` is a **forward declaration**. The mux may not exist yet; the
  pin sits `unbound` until you launch it. Use it for sessions you haven't
  started.
- `adopt` is a **reverse capture**. The mux *must* already be running (the
  CLI refuses otherwise). Use it to bring an existing agent-deck or
  hand-managed tmux session under a pin without restarting anything.
  Harness and cwd default from current attribution; `create` requires you
  to type them.

**`bind` vs `rebind`**: both write to disk, but to different places.

- `bind` resolves **`PinAmbiguous`**: several harness sessions are
  attributed to the same mux and the resolver can't pick one. It writes a
  `LocalDeclared` link tagged `pin:<id>`, not an edit to the pin entry;
  the pin's `mux.name` stays the same.
- `rebind` recovers from an **external tmux rename**: the pin's configured
  `mux.name` no longer matches a running mux. It edits the pin's own TOML
  entry to point at the new name. No declared links are involved.

**`launch` vs `attach`**: they share one code path and differ only in the
intent label. `pin attach` on an unbound pin falls through to launch with
a one-line note; `pin launch` on a bound pin just attaches. Prefer
`launch` in scripts that may run before the mux exists, and `attach` in
muscle-memory wrappers when you know the mux is up.

**`rename` vs `rebind`**: `rename` changes how *you* refer to the pin
(`id`, `display_name`); when `--display` changes on a bound pin it also
renames the tmux session in lockstep (ADR 0029). `rebind` changes which
*mux* the pin points at and never touches tmux. Use `rename` when you
don't like the label, and `rebind` when the mux moved.

## TUI walkthrough

All pin operations live behind a single discoverable modal (`p`) and
a set of direct row-action shortcuts that the modal mirrors 1:1. Use
the modal when you're learning; reach for the shortcuts once they're
in your fingers.

### Discover the menu

Press `p` from any view. A modal lists
`create / launch / rename / remove / bind / rebind / adopt`. `↑/↓`
navigate, `Enter` opens the form (or executes for `launch`/`bind`),
`Esc` closes. If an entry needs a row selection you don't have,
you'll get a status hint instead of a form.

### Create your first pin

1. Highlight something contextual: a session row whose harness/cwd/
   display you want to reuse, or a group row whose cwd is the project
   root, or a mux row whose name and observed cwd you want as seeds.
2. Press `N` (or `p` → `create`). The form opens pre-filled from the
   selection.
3. Adjust `id`, `harness`, `cwd`, `display`, `mux.name`, optional
   `mux.socket`, optional `launch.argv` overrides, and `--store`
   (project vs user).
4. Submit. A new pin row appears in the sessions tree, initially
   `unbound` if no live mux matches.

### Launch or attach

`Enter` on any pin row exec-spawns `conspectus pin launch <id>`. So
does `L` — same code path, different muscle memory. The decision
tree is:

- **Bound** → `tmux attach-session` (or `switch-client` if you're
  already inside tmux).
- **PinStaleMux** → `tmux send-keys` re-injects the configured argv
  into the existing pane, then attaches. Your window/pane layout
  survives.
- **PinUnbound** → `tmux new-session -d -s <name> -c <cwd> <argv>`
  then attaches. If the sidecar has a recorded prior session, the
  status bar will say `Enter resume <session-id>` and the launch
  will splice the harness's resume CLI (`codex exec --resume`,
  `claude --resume`, `opencode --session`) into the argv
  automatically.

### Rename a pin's display name

`R` on a pin row (same key as session rename) opens an inline
text-input. Submitting writes through `conspectus pin rename
--display`. The alias overlay updates lockstep, so the bound
session's row label updates too.

### Remove a pin

`Delete` on a pin row prompts a two-press confirmation, then calls
`conspectus pin rm`. The pin's TOML entry is removed from its owning
store; any live session it was bound to keeps running — pins only
declare intent, they don't own processes.

### Resolve `PinAmbiguous` with bind

Sometimes two harness sessions get attributed to the same mux
(resume mid-compaction, parallel hooks, etc.) and the resolver can't
pick. The pin's status hint says `PinAmbiguous`. Press `b`
(lowercase) to open a picker listing the candidate session keys.
Select one. The TUI writes a `LocalDeclared linked_to_mux` link
tagged `label = "pin:<id>"`, and the resolver's precedence pipeline
picks that override on the next cycle.

### Fix a `PinStaleMux` after an external tmux rename

You renamed a tmux session from outside Conspectus and now the pin
can't find it. Press `B` (uppercase) on the pin row. The form lets
you edit `mux.name` and optional `mux.socket_name`. This is a pure
TOML mutation — Conspectus does not touch tmux itself. Submit, and
the binding restores on the next discovery cycle.

### Adopt a live mux as a pin

Coming from agent-deck or just want to capture a long-running
session you started ad-hoc? Highlight the mux row in the dashboard,
press `A`, and the adopt form opens pre-filled with the mux's name,
observed cwd, and the harness currently attributed to it. Override
anything you want, give it a stable pin id, and submit. From that
point on the mux is a first-class pin row, and if it ever dies you
can relaunch it from the same row.

## When the mux dies

If you've ever lost a `/compact`-deep Codex conversation to a stray
`kill`, this is the part that matters. The next time you launch the
pin:

1. Conspectus reads the sidecar.
2. Looks up the recorded session in the current snapshot. If the
   session file is gone, the sidecar is deleted and you launch fresh.
3. Walks forward along `parent_session` lineage to the current head.
   Forks (multiple successors) abort the walk and you launch fresh
   with a hint.
4. Asks the harness adapter for a `resume_argv(head, cwd)`.
   Codex / Claude Code / opencode return one; aider returns `None`
   (it tracks chat history per-cwd, not per-session) and you launch
   fresh with a `no resume capability` hint.
5. Splices the resume argv into `tmux new-session` and attaches.

The status bar telegraphs this: `Enter resume <session-id>` means
continuity is available; `Enter launch` means fresh start.

## What pins do *not* do

- They don't track windows, panes, or layouts (that's tmuxinator /
  tmuxp territory; see `H-PIN-F-003` for an importer follow-up).
- They don't run lifecycle hooks beyond `launch.argv` (use
  `nix develop --command`, `direnv exec`, `op run` as prefix
  tooling; a `launch.before/after` surface is parked as
  `H-PIN-F-002`).
- They don't synthesize ephemeral entries from path globs
  (`H-PIN-F-004`).

## A two-minute dry run

To internalize the loop end-to-end, try this in a scratch project:

1. `p` → `create`, pin id `walkthrough`, harness `codex`, cwd
   `~/scratch`, mux name `walkthrough`, store `project`. Pin
   appears unbound.
2. `Enter`. Tmux launches Codex in a session named `walkthrough`
   and your terminal hands off.
3. Detach (`prefix d`). Refresh the dashboard — pin is now bound,
   the session row labeled with your display name.
4. From tmux outside Conspectus,
   `tmux rename-session walkthrough walkthrough-2`. Refresh: pin
   is `PinStaleMux`. Select the row, press `B`, change `mux.name`
   to `walkthrough-2`, submit. Refresh: bound again.
5. `tmux kill-session -t walkthrough-2`. Refresh: pin is
   `PinUnbound` with a sidecar-driven `Enter resume <id>` hint.
   `Enter` to resume the same Codex conversation in a fresh tmux.
6. `Delete`, confirm. Pin row gone; the underlying TOML entry is
   removed from `.conspectus.toml`.

That's the whole surface. The CLI commands
(`conspectus pin create / list / show / launch / attach / bind /
rebind / adopt / rename / rm`) are the same shape — useful for
scripts and for understanding what the TUI shortcuts shell out to.

## Further reading

- ADR 0057 — Session pin schema, binding, and launch contract.
- ADR 0058 — Pin session continuity (the sidecar cache + lineage
  walk).
- ADR 0014 — Declared-link TOML storage (the store-selection rules
  pins reuse).
- ADR 0029 — Alias overlay and lockstep mux rename.
- ADR 0018 — Intra-harness `parent_session` lineage.
- `docs/operations.md` §"Session pins" — the reference-style
  operator guide; this walkthrough is the teaching counterpart.
