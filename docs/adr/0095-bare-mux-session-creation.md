# ADR 0095: Bare Mux Session Creation (No Pin, No Agent)

## Status

Accepted

## Context

Conspectus has three distinct paths that end in a new tmux session on
the operator's box:

- **Pin launch** (ADR 0057, ADR 0058, ADR 0094). A `pin launch` — or
  `Enter` on an unbound pin row — runs `tmux new-session -s <name> -c
  <cwd> <argv>` where `<argv>` is the harness's default launch argv
  (optionally spliced with `resume_argv`). The mux is owned by the
  pin; the operator's intent is "start the agent I declared."
- **Worktree stream** (ADR 0094, H-WT-007). A worktree-backed pin
  realizes its worktree at launch and then runs the same
  pin-launch path inside the freshly-materialized worktree cwd.
  The mux is still owned by a pin.
- (missing) A **bare tmux session** — one the operator wants to open
  from inside Conspectus purely as a shell console rooted in a
  discovered repo, checkout, or arbitrary cwd, without declaring a
  pin, running an agent, or tying the session to a worktree stream.

Today the operator has to leave Conspectus and type `tmux new-session
-s <name> -c <cwd>` at a shell to get that third shape. That is a
minor papercut on its own, but it is out of proportion with the rest
of the mux lifecycle surface: Conspectus can already rename, attach,
teardown (ADR 0093), and pin-launch mux sessions from the TUI and CLI.
"Create a bare session" is the one missing verb.

The mutation envelope (ADR 0087) sanctions **category 3 — operator-
initiated mux lifecycle: rename / new-session / attach**. Bare-session
creation is a straightforward instance of the `new-session` verb the
category already covers; the only prior caller happened to be pin
launch. The `MuxBackend::new_session` primitive (ADR 0089) is already
in place and used by pin launch.

The question this ADR answers: **may Conspectus expose bare tmux
session creation as a first-class TUI + CLI verb, and if so under what
shape?**

## Decision

Yes, under the shape below.

### Envelope

Bare-mux creation is an instance of ADR 0087 **category 3** (operator-
initiated mux lifecycle). No new mutation category is introduced. All
existing prohibitions stand — in particular:

- **No harness state is touched** (prohibition 1). The session runs a
  bare shell; no harness session record is created.
- **No terminal input is injected** (prohibition 2, ADR 0028). Bare
  creation uses `tmux new-session` with no argv; nothing is typed at
  the shell after the pane comes up.
- **No git state is mutated** (prohibition 6). The `--cwd` argument
  is a launch parameter, not a checkout operation.
- **No pin is written**. Bare mux creation is deliberately *not* a
  shortcut for `pin create` + `pin launch`. Operators who want a
  session persisted as a pin can `pin adopt` the resulting mux after
  the fact.

### Primitive

The existing `MuxBackend::new_session(socket, name, cwd, argv)`
primitive (`src/discovery/tmux/mod.rs:112`, ADR 0089) is the sole
mutation call. Bare creation invokes it with `argv = &[]`, which
tmux interprets as "run the operator's login shell in the new
window."

### CLI surface

```
conspectus mux new <NAME> [--cwd <PATH>] [--socket <NAME>] [--no-attach]
```

- `<NAME>` — the tmux session name. Required. Same validation and
  same `NameTaken` handling as pin launch (`report_new_session`).
- `--cwd <PATH>` — the working directory tmux passes as `-c`.
  Defaults to `$PWD` (the caller's cwd) when unset. Must exist on
  disk at invocation time.
- `--socket <NAME>` — non-default tmux socket (`tmux -L <name>`).
  Absent ⇒ default socket, matching pin's `mux.socket_name`.
- `--no-attach` — spawn detached and print the attach command,
  mirroring `pin launch --no-attach`. Useful for scripts and CI dry
  runs. Absent ⇒ attach via the existing exec-replace path
  (`attach_and_report` in `src/cli/pin.rs`).

The subcommand tree stays open for future mux verbs (e.g.
`conspectus mux attach`, `conspectus mux ls`) but v1 ships only
`new`. `conspectus rename mux …` remains the rename entrypoint;
this ADR does not reshuffle it.

### TUI surface

- **Hot key**: lowercase `n` opens a `NewMuxForm` overlay. Two
  fields: name (defaulting to a unique-per-live-mux slug derived
  from the selected row) and cwd (defaulting to the selected row's
  cwd if it resolves to a repo, checkout, mux, or agent-session
  row, else `$HOME`). Both are editable.
- **Discoverability**: the help overlay (`?`) surfaces the binding
  with an explanatory line. The form ships without a dedicated
  "mux action menu" shell even though attach (`a` / `Enter`) and
  rename (`R`) are also mux-relevant actions. The reasoning: those
  two are *polymorphic* on selection kind — `R` renames a session,
  mux, or pin per what's selected, and `a` / `Enter` is the
  default-action key that resolves per row kind. Neither is
  mux-specific in the way a proper "Mux ▸ …" menu would want.
  A menu becomes justified when a second *mux-specific* verb
  (candidates: bare-shell in an existing pane, mux duplicate,
  attach-with-socket-override) lands — see Follow-Ups. Until then,
  `n` + the help overlay are the discoverability path.
  Reference the `feedback_tui_discoverability` operator tenet.
- **Commit path**: submits a `Msg::CommitMuxNew { name, cwd,
  socket }` which the runtime routes to a `StoreOp::MuxNew`
  executor. On `Created`, the runtime hands the terminal off via
  the same exec-replace attach path pin launch uses; on
  `NameTaken`, the form stays open with a toast so the operator can
  pick a different name. On unavailable / failure the overlay
  closes and a toast reports the error.
- **`n` is available**: the current `UpperChar('N')` binding
  (OpenPinCreate) matches only `KeyCode::Char('N')`. Lowercase `n`
  arrives as `Char('n')` and is unbound today.

### Non-goals

- No pin is written. The next discovery cycle picks the bare mux
  up via the existing tmux discovery path; the operator can later
  `pin adopt` it if they want a durable declaration.
- No harness is launched. Bare mux is a shell.
- No worktree is created or realized. Worktree-backed streams
  remain the ADR 0094 pin flow.
- No `send-keys` is used to seed the pane with a command. That
  would violate ADR 0028's terminal-injection prohibition. The
  pane is exactly what tmux produces from `new-session` with no
  argv.

## Consequences

- Operators no longer need to drop out of Conspectus for the
  bare-shell case. The mux lifecycle surface is complete for
  create + rename + attach + teardown.
- `MuxBackend::new_session`'s existing empty-argv semantics are
  now load-bearing for two callers (pin launch spliced argv, bare
  mux empty argv). No new backend method is required.
- The absence of a dedicated mux-action-menu shell is a deliberate
  proportional-scope call. The moment a second mux-only action
  lands (candidates: bare-shell in an existing empty pane, mux
  duplicate, socket switch), a follow-up story should fold both
  actions into a menu keyed on `m`, matching the worktree menu on
  `w`. Until then, `n` + the help overlay are the discoverability
  path.
- CLI `conspectus mux new` becomes the natural home for future
  read verbs the operator may want (`mux ls`, `mux status`), but
  those are out of scope here.

## Alternatives Considered

- **Extend `conspectus pin create` with a `--bare` toggle that
  skips harness argv.** Rejected. Bare mux is not a pin. Making
  pins polymorphic on "has a harness or doesn't" invites resolver
  churn and confuses the ADR 0057 model, whose whole point is
  "pin is a declared next agent session."
- **Ship a dedicated `MuxActionMenuState` overlay now that would
  fold in attach, rename, and this new bare-create verb.**
  Rejected today, but not by much. Attach (`a` / `Enter`) and
  rename (`R`) already work off a mux row, but their bindings are
  polymorphic across node kinds — a "Mux ▸ Attach / Rename / New"
  menu would either duplicate those global keys or steal them,
  neither of which helps. The worktree menu (`w`) is a useful
  precedent because every action on it is worktree-specific and
  mutation-guarded; today the mux surface doesn't have enough
  mux-specific verbs to shape a comparable menu. Revisit when a
  second bare-mux-shape action is queued (see Follow-Ups).
- **Bind bare-mux creation to Shift-`M` instead of `n`.**
  Rejected. `n` is discoverable (mirrors uppercase `N` = pin
  create), collision-free, and matches the "single verb, single
  key" pattern the fallback keymap uses for `r`, `a`, `v`, `p`.
- **Use `send-keys` to launch a shell inside an existing pane
  instead of creating a new tmux session.** Rejected. That is
  the ADR 0028 injection prohibition head-on.
- **Skip the CLI and ship only the TUI form.** Rejected. Every
  mutation surface in Conspectus has CLI parity (pin, rename,
  worktree, hook) so scripts and non-TUI operators aren't cut
  off. `conspectus mux new` is trivial to add and preserves the
  contract.

## Follow-Ups

- Once a second bare-mux-shape action is queued, ship an
  `MuxActionMenuState` overlay keyed on `m` that fronts New +
  the future verb. Attach and rename would be listed there too
  so the menu doubles as the discoverability surface for
  mux-selection actions; the polymorphic global bindings (`a`,
  `Enter`, `R`) stay for muscle memory. `n` demotes to a shortcut
  accelerator that jumps straight into the create-form branch.
  Track under a new `H-MUX-MENU-*` story when the trigger action
  is defined.
- Consider `conspectus mux ls` and `conspectus mux status` if
  operators want a mux-scoped read surface distinct from
  `conspectus table mux` and `conspectus status`.
