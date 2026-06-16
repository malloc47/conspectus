# ADR 0067: Dev-Only TUI Snapshot Mode For Agentic Iteration

## Status

Accepted

## Context

The TUI is interactive and lives behind an alternate-screen
crossterm session, so the operator (or an agent assisting them)
can only see what the renderer produces by running the app in a
real terminal and visually inspecting it. Iterations that depend
on rendering — column alignment, color/style normalization,
chip placement, header layout — currently require a screenshot
shared back to the agent, which adds latency to every cycle.

Ratatui ships a `TestBackend` that renders a `Frame` into a
`Buffer` (cells + styles) without touching the terminal. The
in-tree unit tests already use it: `render_to_buffer` and
`buffer_to_string` in `src/tui/ui.rs` build an `App`, draw one
frame, and flatten the buffer to a plain string for assertions.

What's missing is a one-shot CLI surface that does the same thing
against the *live* graph (the operator's actual sessions, mux
panes, and discovery state), emits the rendered buffer to stdout
with ANSI escapes preserved, and accepts a small scripted
keystroke prelude so the agent can place the UI into a non-default
state (different view, different grouping, cursor on a specific
row, an overlay open) before snapshotting.

## Decision

Add a dev-only **snapshot mode** to the `tui` subcommand. When
the operator runs `conspectus tui --snapshot`, the runtime
bypasses the interactive event loop and instead:

1. Builds an `App` exactly as `tui` does (same `RunConfig`, same
   scan roots, same color/grouping/filter flags).
2. Runs one synchronous discovery cycle so the row tree is
   populated.
3. Replays an optional scripted-key prelude
   (`--snapshot-keys "..."`) through the existing action
   dispatcher so the agent can navigate, switch views, or open
   overlays before the frame is captured.
4. Renders one frame into a ratatui `TestBackend` sized by
   `--snapshot-width` / `--snapshot-height` (defaults 160×40).
5. Optionally slices the rendered buffer to a single pane
   (`--snapshot-pane all|header|left|right|status`) — default
   `all`.
6. Walks the buffer cells, converts style transitions to ANSI
   escape sequences, and writes the result to stdout.

Implementation rules:

- **Cargo feature gate.** A new `snapshot` feature toggles every
  flag, module, and dispatch path so production builds compiled
  without `--features snapshot` carry zero snapshot code. Dev
  shells (nix + `just check`) build with the feature on so the
  CI surface exercises it on every change.
- **Key syntax.** Vim-style literals plus `<Name>` brackets for
  non-printables: `vjjj<Enter>`, `<Tab><Down><Down><Enter>`,
  `<C-r>`. Supported names: `Up Down Left Right Enter Esc Tab
  BackTab Space Backspace Delete Insert Home End PageUp PageDown
  F1..F12`. Control modifiers via `<C-c>`; shift is implicit in
  the uppercase letter; alt via `<A-x>`.
- **Pane slicing.** Re-uses the same `Layout::default()` split
  the renderer uses (vertical `[1, Min(3), 1]` for header /
  body / status; horizontal 50/50 inside the body when wide
  enough). `all` returns the full buffer; the other variants
  return a buffer sized to the matching `Rect` populated from
  the live frame's cells. Overlays draw on top of the body in
  the live frame, so they show up in any selection that overlaps
  their rect.
- **Action dispatcher subset.** The snapshot dispatcher handles
  the side-effect-free subset of `Action`: `Msg`, `SwitchView`,
  `CycleView`, `CycleGrouping`, `ClearFilters`, `OpenControls`,
  `OpenPins`, `OpenSearch`, `OpenHelp`, and their overlay-key
  follow-ups. Actions that would spawn a process or take over
  the terminal (`Attach`, `Resume`, `View`, `DefaultAction`,
  `LaunchPin`) are skipped with a one-line stderr warning so the
  agent knows the key was a no-op rather than silently dropped.
- **ANSI output.** Cells are flattened row-major; style
  transitions emit minimal escape sequences (reset, fg, bg,
  bold/dim/italic/underline/reverse). 24-bit color is used when
  ratatui's `Color` carries an RGB value; named/indexed colors
  emit their canonical sequence. Operators piping into `less -R`
  or `cat` see the rendered frame.
- **No write-side effects.** Snapshot mode never writes to the
  on-disk config, the agent-deck state, or anywhere else.
  Discovery still reads from disk because that's how the row
  tree gets built; the snapshot itself is render-only.

## Consequences

- **Tight agent iteration loop.** The agent can edit a renderer
  fix, run `conspectus tui --snapshot --snapshot-pane left`, and
  see the rendered left pane directly in its tool output —
  closing the loop without manual screenshots.
- **Live-state fidelity.** Because snapshot mode runs the same
  discovery pipeline as the production TUI, the agent sees the
  *operator's* world, including the bugs that only manifest with
  the operator's data shape (agent-deck workspaces, particular
  worktree layouts, etc.).
- **Build-time isolation.** The feature gate means downstream
  packagers and end-users never see the `--snapshot` flag in
  `conspectus tui --help`, and the binary they ship doesn't
  carry the TestBackend / ANSI-emit code. The CI matrix builds
  with the feature on; the release build will build without it.
- **Scripted-key fidelity is partial in V1.** Side-effect-full
  actions are explicitly out of scope. The agent can't replay an
  `Attach` to verify what tmux would see; that's the right
  tradeoff for a render-loop tool.
- **No fixture mode in V1.** Snapshot always uses live
  discovery. A future fixture mode (deterministic, machine-
  independent) is a follow-up if regression-snapshot needs
  emerge; in-tree unit tests already cover the deterministic
  case.

## Alternatives Considered

### A. Separate `conspectus snapshot` subcommand

Splits cleanly from `tui`'s argv. Rejected because the snapshot
needs every flag `tui` already supports (scan roots, view,
grouping, filters, color) — duplicating them costs more than
the flag-noise on `tui`. The `--snapshot` flag is mutually
exclusive with the interactive event loop, so there's no
real argv collision.

### B. Always-on (no cargo feature)

Cheaper to maintain; the snapshot code path is small and would
add only a few hundred lines to the production binary. Rejected
on the operator's preference: keeping the production binary
focused on the interactive UX avoids the "wait, what's this
`--snapshot` thing?" question from downstream users who never
need it.

### C. Fixture-driven snapshots only

Skip live discovery; snapshot only takes a fixture path. More
deterministic, easier to regression-test. Rejected because the
primary use case is debugging the operator's *current* world,
not a frozen one. Fixture-based snapshots already exist as
in-tree tests.

### D. Strict tokenized key syntax (`<v><j><j>`)

Trivial to parse, no ambiguity. Rejected for ergonomics: the
common case is literal letters, and `<j>` for every keypress
defeats the muscle-memory advantage of vim-style. The chosen
syntax falls back to bracketed form when needed, which captures
the same expressiveness with less typing.

## Open Questions

- Should snapshot mode eventually accept a `--snapshot-after
  <duration>` knob to let a background data refresh settle
  before capture? V1 always uses the first synchronous
  discovery's output; live refresh isn't relevant for a
  one-shot render.
- Should there be a `--snapshot-frames N` mode that captures a
  short series after each key, so agents can verify animations
  or transitions? Out of scope for V1.
- Are there scripted-key actions worth promoting from "skip
  with warning" to "handle"? `Attach` / `Resume` etc. are
  side-effect-full but could be stubbed in snapshot mode to
  record "would have attached to X" rather than skipping. Defer
  until a real use case asks for it.
