# ADR 0096: Mux Launch — Harness in a New Mux Without a Pin

## Status

Accepted

## Context

Conspectus today has three launch shapes that end in a live tmux
session on the operator's box:

- **Pin launch** (ADR 0057, ADR 0058, ADR 0094). Spawns the pin's
  harness into a fresh tmux session and attaches. Persists a
  `[[pins.entries]]` TOML block first; the pin is the durable owner
  of the (harness, cwd, mux) tuple and survives across restarts,
  `/compact`, and `/resume`.
- **Bare mux new** (ADR 0095). `tmux new-session -c <cwd>` with
  `argv=&[]` — the operator's login shell in a fresh mux, no
  harness, no pin.
- **Worktree stream** (ADR 0094). A worktree-backed pin realizes
  its worktree at launch and runs the pin-launch path inside it.

There is a fourth shape the surface does not cover: **spawn a
harness in a fresh mux without declaring a pin.** The operator's
intent is "I want a Codex session for this repo right now, but I do
not intend to keep this as a first-class dashboard row." Today they
have three unsatisfactory options:

1. Create a pin, launch it, delete it afterwards — three writes to
   TOML for a session they will never resume by id.
2. Run `conspectus mux new` for a bare shell, then type the harness
   command at the shell — dropping out of the "Conspectus knows how
   to launch this harness" pipeline and losing the `HarnessAdapter`
   launch argv (ADR 0057).
3. Leave Conspectus, run `tmux new-session -c <cwd> codex` at a
   shell — the same papercut ADR 0095 closed for the bare-shell
   case, still open for the harness case.

Pin launch requires persistence; bare mux new refuses argv. The
harness-in-a-fresh-mux-without-a-pin shape falls between them and
is genuinely missing.

The mutation envelope (ADR 0087) already sanctions the underlying
mechanics:

- Category 3 — operator-initiated mux lifecycle — covers
  `tmux new-session` (used today by pin launch and bare mux new).
- Category 4 — subprocess launches Conspectus owns end-to-end —
  covers spawning the harness argv via `MuxBackend::new_session`,
  because the argv is constructed from `HarnessAdapter::launch_argv`
  the same way pin launch constructs it.

Nothing in the envelope demands a pin persist for the launch to be
sanctioned. What the envelope demands is that (a) the operator
initiates each mutation, (b) argv is Conspectus-constructed, (c) no
harness-native state is touched, (d) no terminal input is injected
into a live agent pane. All four hold for this shape.

The question this ADR answers: **may Conspectus expose ephemeral
harness launch — spawn a harness inside a fresh mux without writing
a pin — as a first-class TUI + CLI verb, and under what shape?**

## Decision

Yes, under the shape below.

### Envelope

Ephemeral harness launch is an instance of ADR 0087 **category 3**
(operator-initiated mux lifecycle: `new-session`) plus **category 4**
(subprocess launch — Conspectus-constructed harness argv). No new
mutation category is introduced. All existing prohibitions stand:

- **No harness-native state is touched.** The harness starts fresh in
  a new pane; no session file, no SQLite mutation.
- **No terminal input is injected** (prohibition 2, ADR 0028). The
  harness argv is passed to `tmux new-session` as the pane's
  initial command, not typed via `send-keys` after the fact.
- **No pin is written** (prohibition-adjacent — this ADR's whole
  point). The mux-launch flow deliberately does not touch
  `[[pins.entries]]`. Operators who want persistence run
  `conspectus pin create` or `conspectus pin adopt` after the fact.
- **No git state is mutated** except through the ADR 0094 /
  ADR 0092 worktree backend when the worktree toggle is enabled;
  that path is already sanctioned by the worktree backend seam.
- **No background mutation.** The flow fires only on direct
  operator gesture (CLI invocation or TUI form commit).

### Primitive

The existing `MuxBackend::new_session(socket, name, cwd, argv)`
primitive (`src/discovery/tmux/mod.rs`, ADR 0089) is the sole
mutation call for the mux side. Where pin launch passes the
resolved `pin.launch_argv` (or `HarnessAdapter::launch_argv` when
absent) and bare mux new passes `&[]`, mux-launch passes the
harness's `launch_argv` directly.

When the worktree toggle is enabled, `WorktreeBackend::add` (or the
equivalent per ADR 0092's selection) materializes the worktree
first, and the mux is spawned with `cwd = <worktree path>`. This
reuses ADR 0094's realize-at-launch pattern minus the pin-store
read; the branch and repo anchor come directly from the form or
CLI args.

### CLI surface

```
conspectus mux launch <HARNESS>
  --name <MUX_NAME>
  [--cwd <PATH>]
  [--socket <NAME>]
  [--argv <ARG> [<ARG>...]]        # overrides HarnessAdapter default
  [--worktree-branch <BRANCH>]     # realize worktree at launch
  [--worktree-repo <PATH>]         # repo anchor when --worktree-branch is set
  [--no-attach]
  [--scan-root <PATH>]
```

- `<HARNESS>` — required. One of the known `HarnessAdapter`
  keys (`codex`, `claude-code`, `opencode`, `aider`). Same
  validation the pin CLI uses.
- `--name <MUX_NAME>` — required. The tmux session name for the
  new mux. Same validation and same `NameTaken` handling as pin
  launch and `mux new` (`report_new_session`).
- `--cwd <PATH>` — the working directory tmux passes as `-c`.
  Defaults to `$PWD` (the caller's cwd) when unset. Must exist on
  disk at invocation time. Ignored when `--worktree-branch` is set
  and the realized worktree path is used instead.
- `--socket <NAME>` — non-default tmux socket (`tmux -L <name>`),
  identical to `mux new` and pin `mux.socket_name`.
- `--argv <ARG>...` — overrides the harness adapter's default
  launch argv. When absent, `HarnessAdapter::launch_argv(&pin.harness)`
  is used (the same fallback pin launch takes when
  `pin.launch.argv` is empty).
- `--worktree-branch <BRANCH>` — realize the worktree for
  `<BRANCH>` under `--worktree-repo` at launch, and use the
  worktree path as the mux cwd. Idempotent per ADR 0094. When
  set, `--worktree-repo` is required.
- `--worktree-repo <PATH>` — repo anchor for the worktree
  realization. Required with `--worktree-branch`, ignored
  otherwise.
- `--no-attach` — spawn detached and print the attach command,
  identical to `pin launch --no-attach` and `mux new --no-attach`.
- `--scan-root <PATH>` — passed through for consistency with
  other CLI verbs that consult discovery; harmless when
  ephemeral-launch has no discovery lookup to perform.

The verb intentionally lives under `mux`, not `pin` or `session`.
Every existing outcome that ends in a fresh tmux session already
lives here (`mux new` today, `mux attach` / `mux ls` in the
ADR 0095 follow-up backlog). "New tmux + harness" is the same
grain of operation as "new tmux + shell." Choosing `mux` over
`session` also avoids the terminology overload — `AgentSession`
is a discovered graph node, and `session launch` would blur that
with the "start a subprocess" verb.

### TUI surface

- **Menu shell**: this ADR triggers the ADR 0095 follow-up. A new
  `m`-keyed **Mux action menu** overlay fronts the mux-relevant
  verbs. First landing populates it with:
  - `New tmux session` — `mux new`, the ADR 0095 bare-shell verb.
  - `Launch harness in new mux…` — this ADR's ephemeral launch.
  - `Attach` — surfaces the existing polymorphic `a` / `Enter`
    binding as a menu entry when the selection is a mux row, for
    discoverability. Rename (`R`) may follow when the modal grows
    to accept a mux selection from the picker; v1 keeps rename on
    the polymorphic global key.

  The hot key layout stays:
  - `n` (lowercase) — bare mux new (ADR 0095 accelerator, unchanged).
  - `m` — open the Mux action menu (new).
  - `N` (uppercase) — pin create with worktree toggle (ADR 0094,
    unchanged).

  No single-key accelerator is added for the harness-launch entry
  in v1; the menu is the discoverability path and the accelerator
  can be added once operator muscle memory demands it.

- **Form**: the menu's "Launch harness in new mux…" entry opens a
  parameterized launch-spec form. The form is a shared primitive
  (see ADR 0097) that both pin create and mux-launch instantiate.
  In mux-launch mode:
  - Fields: harness, cwd, mux name, mux socket, launch argv,
    optional worktree toggle + branch.
  - Pin-specific fields (id, display name, store, adopt) are
    hidden.
  - Title reads "Launch harness in new mux — no pin".
  - Submit key label reads "Launch".
  - Commit emits `Msg::CommitMuxLaunch(MuxLaunchRequest)`; the
    runtime routes it to `ExecSpec::MuxLaunch`.

  The mode is fixed at open time — the operator cannot toggle
  "actually persist as a pin" from inside the form. To persist,
  they cancel and open pin create instead. This matches the
  operator preference recorded during ADR intake: "the dialog
  should make it clear what will happen, but the behavior should
  be defined by how it is launched, not by something toggle-able
  to change behavior midway through."

- **Discoverability**: the help overlay (`?`) surfaces the `m`
  binding and its menu entries. The Mux action menu title makes
  the difference between "new shell" and "launch harness" visible
  before the operator picks either.

### Executor and commit path

Ephemeral launch reuses the existing pin-launch executor plumbing:

1. Reducer emits `Effect::Exec(ExecSpec::MuxLaunch { harness, name,
   cwd, socket, argv, worktree })` for TUI commits, or the CLI
   dispatches directly to the executor helper.
2. Executor suspends the alt screen (`ratatui::restore`), re-execs
   into `conspectus mux launch …` with `--no-attach`, refreshes
   discovery on return, then attaches via the existing
   `run_tmux_attach_with_socket` path (mirror of
   `execute_launch_pin` in `src/tui/runtime.rs`).
3. On `NameTaken`, the launch fails with the same error surface as
   pin launch and bare mux new; the TUI keeps the form open with
   a toast (parallel to pin-create's duplicate handling).
4. Discovery picks up the fresh mux + its attributed harness
   session on the next refresh via the ADR 0006 / ADR 0028 /
   ADR 0046 / ADR 0047 / ADR 0048 attribution pipeline — no
   special-case row.

### Non-goals

- **No pin is written.** The mux and its harness session render as
  ordinary discovered rows next refresh. Operators who change
  their mind can `conspectus pin adopt <mux-name>` afterwards.
- **No binding is persisted.** There is no
  `(ephemeral-launch-id → agent-session-id)` record. The launch
  is an operator gesture, not a durable entity.
- **No worktree declaration is persisted.** When
  `--worktree-branch` is used, the worktree is materialized (per
  ADR 0092 backend), the mux uses it as cwd, and nothing else is
  written. The worktree becomes ordinary discovery, prunable via
  the worktree menu (`w`) later.
- **No resume splicing** (ADR 0058). Resume is a pin-scoped
  feature — its sidecar is keyed on pin id. Ephemeral launches
  have no pin id and so cannot participate. Operators who want
  `/compact` / `/resume` continuity across relaunches must
  persist a pin.
- **No `send-keys` seeding** (ADR 0028). argv is passed to
  `new_session` directly, as with bare mux new.

## Consequences

- The launch surface is now four-shape-complete: bare shell (mux
  new), harness ephemeral (mux launch), harness persistent (pin
  launch), harness persistent + worktree (pin launch with
  worktree). The operator's choice between them is the persistence
  and worktree axes, both fixed at gesture time.
- ADR 0095's "when a second bare-mux-shape verb lands, ship the
  `m`-keyed Mux action menu" follow-up is discharged. This is
  that second verb; the menu ships with this ADR.
- `MuxBackend::new_session` picks up a third caller (pin launch
  spliced argv, bare mux empty argv, ephemeral harness argv). No
  new backend method is required.
- `HarnessAdapter::launch_argv` becomes the shared launch-argv
  source for both pin launch (as a fallback) and ephemeral launch
  (as the default). Any harness adapter that returns an empty
  `launch_argv` blocks both flows equally; the CLI's
  `--argv` override is the escape hatch for either.
- The `Msg::CommitMuxNew` / `ExecSpec::MuxNew` path from ADR 0095
  is joined by `Msg::CommitMuxLaunch` / `ExecSpec::MuxLaunch`. The
  two executor branches share the alt-screen suspend/restore, the
  subprocess re-exec pattern, and the post-refresh attach path;
  the difference is which subprocess argv they build and which
  harness-attribution outcome they expect.
- The TUI's Mux action menu opens the door to a fourth verb later
  (`mux ls`, `mux status`, socket switch). The menu is now the
  agreed shell for mux-specific verbs; new mux-only actions land
  as menu entries rather than new global hot keys.
- The launch-spec form primitive (ADR 0097) is a soft prerequisite.
  This ADR can land without the extraction — a standalone mux-launch
  form would work — but the form-primitive ADR is drafted alongside
  because operator feedback locked in the "parameterizable dialog,
  not tied to either concept" shape.

## Alternatives Considered

- **Extend `conspectus mux new` with `--harness <key>`.** Rejected.
  The shell-versus-harness distinction is exactly what the operator
  is choosing at gesture time; overloading `mux new` on a flag
  requires the reader to know that `mux new codex-mux` and
  `mux new codex-mux --harness codex` produce different pane
  contents. A distinct verb (`launch`) makes the outcome obvious in
  scripts, in `--help`, and in TUI menu labels. It also keeps
  `mux new`'s ADR 0095 contract intact (`argv=&[]`, no harness).

- **Ship as `conspectus session launch`.** Rejected. `session` in
  the Conspectus data model refers to `AgentSession`, a discovered
  graph node. A `session launch` verb would blur "start a
  subprocess" with the read-side session vocabulary and would
  imply a symmetric `session ls` / `session show` that already
  belongs under `pin` (for declared) or `table sessions` (for
  discovered). `mux` is the correct home because the gesture's
  primary artifact is a new mux; the harness is a parameter.

- **Add "persist as pin" toggle to the launch form.** Rejected on
  operator preference recorded during intake ("behavior should be
  defined by how the dialog is launched, not by something
  toggle-able midway through"). A mid-flow toggle also blurs the
  ADR 0057 framing that a pin is a *declared* logical session.
  The dialog knows at open time whether it is servicing pin create
  or mux launch, and its title/submit label show that to the
  operator.

- **Extend `pin create` with `--ephemeral` that skips the TOML
  write.** Rejected for the same reason: it makes persistence a
  hidden mode on a verb whose name is "create." The mux-launch
  verb keeps ephemeral its default and only shape; persistence
  requires a different verb (`pin create`).

- **Add a single hot key (e.g. `Shift-M`) instead of a menu.**
  Rejected as inconsistent with the discoverability tenet and with
  the ADR 0095 follow-up. The Mux menu is now warranted (this ADR
  is the second bare-mux-shape verb ADR 0095 named as the
  trigger); shipping a hot key without the menu would leave the
  ADR 0095 follow-up open and add memorized bindings the operator
  has to hunt for.

- **Reach the verb only from the pins overlay.** Rejected on
  operator intake: the launch dialog should not be conceptually
  tied to pins. Reaching it from the pins overlay would embed
  exactly that coupling.

- **Skip worktree support in v1.** Considered. The worktree
  toggle adds form complexity, and worktree lifecycle without a
  durable owner is the mux-launch worktree's shape. But the
  operator explicitly asked for symmetry with pin create on the
  worktree axis (the alternative "no worktree in ephemeral
  launch" was rejected at intake), and ADR 0094's realize-at-
  launch primitive is already idempotent, so ephemeral realization
  is a cheap reuse. The trade-off — no pin means nothing durable
  points at the worktree — is deliberate: discovery picks the
  worktree up on the next refresh, and the worktree menu (`w`) is
  the pruning surface if the operator wants it gone.

- **Persist a lightweight "ephemeral launch history" sidecar** so
  the operator can rerun a recent gesture. Rejected as scope creep.
  If the pattern proves useful, the follow-up is `pin adopt` after
  the fact or a dedicated "recent launches" surface — neither of
  which needs to land in this ADR.

## Open Questions Answered

- **Where does the verb live?** Under `conspectus mux` — the
  gesture's primary artifact is a new mux, and the sibling verbs
  (`mux new`, future `mux attach`, `mux ls`) already collect
  there.
- **What is the TUI entrypoint?** The `m`-keyed Mux action menu,
  which this ADR ships. `n` remains the bare-mux accelerator; `N`
  remains the pin-create-with-worktree accelerator.
- **Does the launch form know it is doing pin-vs-mux-launch?**
  Yes, at open time. Mode is fixed by the entrypoint and cannot
  be toggled inside the form. Title and submit label reflect the
  mode.
- **Does ephemeral launch write anything?** Only what the mux
  backend writes (`tmux new-session`) and, when the worktree
  toggle is on, what the worktree backend writes (`git worktree
  add`). No Conspectus-owned TOML or sidecar.
- **Does discovery see the mux and harness?** Yes, next refresh,
  through the existing attribution pipeline. No special-case row.
- **Can the operator persist afterwards?** Yes, via
  `conspectus pin adopt <mux-name>` (or the TUI's adopt action).
  Ephemeral launch and pin adoption compose cleanly.

## Open Questions Deferred

- **`mux launch` presets** (e.g. save a mux-launch invocation as a
  named preset). Deferred — the escape hatch is `pin create`,
  which is exactly the persistent form of "I keep launching this
  same shape." Reconsider if operators start hand-editing
  `--argv` invocations frequently.
- **Batch launch** (spawn N harnesses across N repos with one
  gesture). Deferred; workspace-level launch belongs to the
  H-WS-* redesign.
- **Ephemeral launch history / recent list.** Not needed for v1;
  operators can shell-history the CLI, and the discovered mux row
  is a persistent handle in the TUI until the mux is torn down.
- **Mux action menu accelerator for the harness-launch entry.**
  Held until operator muscle memory demands one; the menu is the
  discoverability path today.

## Related ADRs

- ADR 0028 (hook sidecar records) — terminal-injection prohibition
  this ADR upholds by passing argv to `new-session` rather than
  `send-keys`.
- ADR 0057 (session pins) — pin launch, the persistent sibling of
  this shape.
- ADR 0087 (mutation envelope) — categories 3 + 4 cover the
  mutations this ADR performs.
- ADR 0089 (mux backend trait) — the `new_session` primitive.
- ADR 0092 (worktree backend seam) — realization backend when
  `--worktree-branch` is set.
- ADR 0094 (worktree-backed pins realized at launch) — idempotent
  realize-at-launch pattern reused here without the pin-store
  read.
- ADR 0095 (bare mux session creation) — the sibling verb; this
  ADR discharges its `m`-keyed Mux action menu follow-up.
- ADR 0097 (shared launch-spec form primitive) — the TUI form
  primitive that both pin create and mux launch instantiate.
