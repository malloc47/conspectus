# Conspectus

Conspectus is a Rust CLI and library for surveying local AI-agent work across
sessions, muxes, repos, checkouts, workspaces, forks, branches, and forge PRs.
It reads local state, records relationship evidence in a provider-neutral graph,
and renders deterministic JSON or compact session tables.

Conspectus is read-only except for explicit `conspectus declared`,
`conspectus alias`, `conspectus rename`, and `conspectus pin`
commands, which store user-authored intent in `.conspectus.toml` or
user config.

## CLI

```sh
conspectus
conspectus tui [--view {sessions|mux|union|prs|forks}] [--scan-root PATH]...
conspectus graph --format {json|dot|html} [--scan-root PATH]...
                                          [--candidates {include|exclude}]
                                          [--diagnostic-nodes {include|exclude}]
conspectus session [--projection {agent|mux|union}] [--scan-root PATH]...

conspectus declared list [--store {all|project|user}] [--scan-root PATH]...
conspectus pin {create|list|show|launch|attach|bind|rebind|adopt|rename|rm} ...
```

Running `conspectus` without a subcommand opens the interactive TUI.
`graph` emits the full evidence-preserving graph document: `--format json`
is machine-readable, `--format dot` pipes through Graphviz for static
inspection, and `--format html` produces a self-contained interactive
explorer (see [graph visualization guide](docs/graph-visualization.md)).
`session` renders
agent-, mux-, or union-oriented table projections after resolution. The
`declared` subcommands can pin, ignore, remove, confirm, or override
relationships. `conspectus pin` (ADR 0057) declares a session pin — a
`(harness, cwd, display_name, mux)` tuple persisted in
`.conspectus.toml` — that renders as a first-class dashboard row
whether or not a live session realizes it, binds 1:1 on the mux
native name through the existing attribution pipeline, and can
launch the configured harness into a fresh tmux session on demand.
This replaces the agent-deck "new card" workflow without inheriting
the broader orchestrator scope; see
[`docs/operations.md`](docs/operations.md#session-pins) for the full
command surface and the agent-deck migration path via `pin adopt`.

## Session Pins

A **session pin** is a small TOML entry that declares "I want a logical
agent session here" — a `(harness, cwd, display_name, mux)` tuple
stored in `.conspectus.toml` (project) or the user-level config. It
behaves as a stable, one-keystroke dashboard row whether or not the
underlying tmux session is currently running. When a matching live mux
exists, the resolver binds the pin to its attributed agent session 1:1;
when nothing matches, the pin renders as `unbound` and `pin launch`
spawns the tmux session on demand.

Pins are a lightweight alternative to a full mux/agent orchestrator.
Conspectus does not own your tmux server, does not manage long-running
processes, and does not impose a window/pane layout. It just records
the *intent* of a session, watches what's actually running, and lets
you reach the running pieces with consistent keystrokes from the TUI
or scripts from the CLI.

### Lifecycle at a glance

```
pin create  ──►  pin launch  ──►  bound to live mux  ──►  pin attach
                                                              │
                  ▲                                            │
                  │                                            ▼
            pin adopt           ◄── operator already running tmux
            pin rebind          ◄── mux was renamed outside conspectus
            pin bind            ◄── multiple harness sessions claim the mux
```

The leftmost path is "I'm starting fresh"; the bottom-right transitions
are recovery paths — none of them touch tmux, they only update the
TOML so the resolver re-binds correctly.

### Command guide

| Command | What it does |
|---|---|
| `pin create <id> --harness <K> --cwd <PATH>` | Declare a new pin. Mux may not exist yet; the pin renders `unbound` until `pin launch`. |
| `pin list [--store ...] [--state ...]` | Print every pin, its binding state, store path, and bound session id. Read-only. |
| `pin show <id>` | Show the full entry plus any resolver diagnostic. Read-only. |
| `pin launch <id> [--no-attach]` | Resolve the pin's binding and act: bound → attach; stale → send-keys then attach; unbound → `tmux new-session` then attach. |
| `pin attach <id> [--no-attach]` | Same as `launch` semantically; intent label differs. Unbound falls through to launch with a note. |
| `pin rename <id> [<new-id>] [--display <name>]` | Rename the pin's id and/or display name. With `--display`, applies the ADR 0029 lockstep mux rename when bound. |
| `pin rm <id>` | Remove the pin from its owning store. Live tmux is left alone. |
| `pin bind <id> --to <SESSION_KEY>` | Resolve `PinAmbiguous` by writing a `LocalDeclared linked_to_mux` override (`label = "pin:<id>"`) the resolver treats as authoritative. |
| `pin rebind <id> --mux <NEW_NAME>` | Update the pin's `mux.name` (and optional `--mux-socket`) after an external tmux rename. Pure TOML write — never touches tmux. |
| `pin adopt <new-id> <existing-mux-name>` | Capture an already-running tmux session as a pin. Harness inferred from active attribution; cwd from the mux's observed `cwd`. |

The TUI exposes every action with a direct shortcut and a discoverable
`p` modal — see `docs/operations.md` for the keymap.

### Watch for these confusable pairs

**`create` vs `adopt`** — both write a new TOML entry, but they answer
different questions:
- `create` is a **forward declaration**. The mux may not exist; the
  pin sits `unbound` until you launch it. Use this for sessions you
  haven't started yet.
- `adopt` is **reverse capture**. The mux *must* already be running
  (CLI bails otherwise). Use this when you're migrating an existing
  agent-deck / hand-managed tmux session into a managed pin without
  restarting anything. Harness and cwd default from current
  attribution; `create` demands you type them.

**`bind` vs `rebind`** — both write to disk, but to different places:
- `bind` resolves **`PinAmbiguous`**: multiple harness sessions are
  attributed to the same mux and the resolver can't pick one. The
  override is a `LocalDeclared` link tagged `pin:<id>`, not an edit to
  the pin entry itself. The pin's `mux.name` stays the same.
- `rebind` recovers from an **external tmux rename**: the pin's
  configured `mux.name` no longer matches a running mux. The fix is to
  edit the pin's own TOML entry to point at the new mux name. No
  declared links involved.

**`launch` vs `attach`** — they share the same code path, only the
intent label differs. `pin attach` on an unbound pin falls through to
launch with a one-line note; `pin launch` on a bound pin just
attaches. Prefer `launch` in scripts that may run before the mux
exists; prefer `attach` in muscle-memory wrappers when you know the
mux is up.

**`rename` vs `rebind`** — `rename` changes how the *operator* refers
to the pin (`id`, `display_name`); when `--display` changes and the
pin is bound, it also lockstep-renames the mux per ADR 0029.
`rebind` changes which *mux* the pin points at and never touches
tmux. Use `rename` when you don't like the label; use `rebind` when
the mux moved.

### Diagnostics

The resolver emits four pin-specific diagnostics that surface in
`pin show`, the TUI status line, and the right detail pane:

- **`PinUnbound`** — `pin.mux.name` matches no live mux. Action:
  `pin launch <id>`. Carries an optional `last_session` field
  populated from the continuity sidecar (see below); when present,
  `pin show` and the TUI advertise `Enter resume <session-id>`
  instead of a generic `Enter launch`.
- **`PinStaleMux`** — mux is live but no `pin.harness` session is
  attributed. Action: `pin launch <id>` to inject the harness into
  the existing pane.
- **`PinAmbiguous`** — multiple harness sessions of the right kind
  are attributed to the bound mux. Action: `pin bind <id> --to
  <session-key>`.
- **`PinDrift`** — bound session's observed cwd diverges from the
  pin's declared cwd. Advisory; the binding still holds.

### Session continuity

Per ADR 0058, every fresh `Bound` resolution is recorded to a
per-pin JSON sidecar under
`$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`. When the
mux later dies and `pin launch <id>` flips to the unbound branch,
the launch path reads the sidecar, walks the ADR 0018
`parent_session` chain forward to the current head (stopping at
any fork), validates the session still exists on disk, and splices
the harness's `resume_argv(<head>, <cwd>)` into the tmux
`new-session` call. The result: closing tmux and relaunching the
pin resumes the same agent session you were last working in
(codex, claude-code, opencode) rather than starting fresh. Aider
tracks chat history per-cwd rather than per-session and falls back
to a fresh launch with a hint.

The sidecar is a rebuildable cache, not authoritative state — the
resolver never reads it, stale entries self-prune at launch time,
and clearing
`$XDG_CACHE_HOME/conspectus/pin-bindings/` only loses continuity
until the next `pin launch` from a bound state.

### Read-only invariant

Read commands (`graph`, `node show`, `table`, `query`, `pin list`,
`pin show`, `tui`) never create, mtime-touch, or content-modify any
`.conspectus.toml` / user config bearing a `[pins]` section. Mutation
is reserved to `pin create / rename / rm / bind / rebind / adopt` and
the TUI write paths they back. Enforced by
[`tests/cli_pin_invariants.rs`](tests/cli_pin_invariants.rs).

The same invariant extends to the continuity sidecar: read-only
commands run against configs with only unbound pins leave the
cache directory untouched, and existing sidecars survive
byte-for-byte across read-only commands. The positive case (a
`Bound` resolution producing a sidecar write) is by design.
Enforced by [`tests/cli_pin_resume_invariants.rs`](tests/cli_pin_resume_invariants.rs).

For the full reference — TOML schema, store-selection rules, launch
semantics, TUI keymap, scenario-mode behavior, continuity sidecar
internals — see
[`docs/operations.md`](docs/operations.md#session-pins) and ADRs
[0057](docs/adr/0057-session-pins.md) and
[0058](docs/adr/0058-pin-session-continuity.md).

## Docs

- [Feature summary](docs/feature-summary.md) describes the current CLI,
  discovery providers, declared-link behavior, and known limits.
- [Operations](docs/operations.md) documents runtime environment variables,
  provider toggles, state-root overrides, and config-file precedence.
- [Graph visualization](docs/graph-visualization.md) covers `--format dot`
  and `--format html`, the HTML explorer chrome, and common debugging
  recipes.
- [Library API](docs/library-api.md) describes stable entry points and
  pure/impure boundaries for consumers.
- [Atelier migration guide](docs/atelier-migration.md) maps overlapping Atelier
  observability commands to Conspectus replacements.
- [ADR index](docs/adr/) records architecture decisions, including the Phase 6
  library API and distribution policies.
- [Design notes](docs/design.md) remain as historical and forward-looking design
  context.

## Development

Use the Nix flake for a local development shell:

```sh
nix develop
```

The shell provides Rust tooling, `cargo-nextest`, `just`, `pre-commit`, tmux,
and GitHub CLI.

Common checks:

```sh
just check
cargo doc --no-deps
```

### TUI snapshot and fixture mode

A dev-only `snapshot` cargo feature exposes a one-shot render path and
fixture I/O on the `tui` subcommand. The flags do not appear in
production builds (no `--features snapshot`); the nix dev shell and
`just check` build with the feature on.

```sh
# Render one frame to stdout with ANSI styling preserved and exit
conspectus tui --snapshot

# Slice to a single pane (header | left | right | status)
conspectus tui --snapshot --snapshot-pane left --snapshot-width 160 --snapshot-height 40

# Drive the UI to a non-default state before snapshotting (vim-style:
# literals + `<Name>` for non-printables, `<C-x>` / `<A-x>` modifiers).
conspectus tui --snapshot --snapshot-keys '2'           # switch to mux view
conspectus tui --snapshot --snapshot-keys 'jjj<Enter>'  # navigate, expand

# Capture the live world to a fixture JSON for later iteration
conspectus tui --snapshot --snapshot-export-fixture world.json

# Re-render from a fixture (skips live discovery, deterministic)
conspectus tui --snapshot --snapshot-fixture world.json --snapshot-pane left

# Explore a fixture interactively in the full TUI. `r` re-reads the
# JSON from disk so you can edit the fixture in another buffer and
# cycle in the new state without leaving the session.
conspectus tui --fixture world.json
```

Snapshot output uses the same row builders and renderer as the
interactive TUI, so what you see is byte-for-byte what an operator
sees. Lift a dialed-in fixture into a regression test with
`serde_json::from_str(include_str!(...))` plus the existing
`render_to_buffer` / `buffer_to_string` helpers in `src/tui/ui.rs`.

Design recorded in ADRs
[0067](docs/adr/0067-tui-snapshot-mode-for-agent-iteration.md),
[0068](docs/adr/0068-snapshot-fixture-mode.md), and
[0069](docs/adr/0069-interactive-fixture-mode-for-tui.md).
