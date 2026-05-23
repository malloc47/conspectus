# ADR 0030: TUI Text-Input Primitive

## Status

Accepted

## Context

ADR 0024 set the TUI runtime and dependency policy. The dependency-policy
section deferred a concrete decision on text input:

> `tui-input` if and only if the v1 `/` search overlay needs more than a
> manual `String` + cursor index. Prefer hand-rolled text input first; add
> only if hand-rolled becomes fragile.

At the time only one caller (`/` search overlay, `T8-017`) was on the
horizon, and that caller was not yet implemented. The deferral was correct
then.

Two further callers have since accumulated:

- **Rename** (`H-RENAME-011`) opens an input prompt pre-populated with the
  current alias (or harness title, or empty) so the operator can edit and
  submit a new display name.
- **Inline mux-picker** (`P8-014`) opens an overlay listing active mux
  candidates for ambiguous `LinkedToMux` rows. The picker is selection-only
  today, but the same overlay primitive that hosts a text input will also
  host the picker.

Three callers is enough evidence to commit to a shared primitive rather than
re-deriving line-edit semantics three times. The shape of the primitive —
hand-rolled vs `tui-input` crate, how it interacts with the existing focus
cycle, what key-handling semantics it locks in — is significant enough to
ADR rather than embedding the decision in any single feature story.

The primitive is general-purpose: it underwrites rename, search overlay,
and any future single-line input the TUI accumulates (declared-link
quick-create, filter expressions, etc.). It does **not** need to support
multi-line editing, undo/redo history, completion, or paste-large-payload
flows in v1.

## Decision

Adopt the `tui-input` crate as the v1 text-input primitive.

```toml
tui-input = { version = "0.x", default-features = false, features = ["crossterm"] }
```

(Pin to the latest stable minor at adoption time; bump policy follows the
crate's own SemVer.)

Justification:

- Three concurrent callers exhausts the "prefer hand-rolled first" stance
  in ADR 0024. The crate is small, well-scoped to single-line input, and
  uses `crossterm` (already in the dep tree).
- Hand-rolled line edit means re-implementing word-boundary navigation
  (Ctrl-W, Alt-B / Alt-F), kill-line (Ctrl-K), and Unicode cursor math
  three times. The crate gets that right and is maintained.
- Alternative: keep hand-rolling. Each caller would carry its own
  `String` + `usize` cursor, its own backspace/delete branches, and its
  own Unicode-width slicing. Bugs compound across callers.

### Key-Handling Semantics (Locked)

The primitive is hosted by a TUI overlay that owns focus while open. While
the overlay is open:

- `Enter` confirms; the host dispatches the typed-in value through the
  reducer (`Msg::ConfirmRename(String)`, `Msg::SetSearchQuery(String)`,
  etc.).
- `Esc` cancels; the host closes the overlay and discards the buffer. No
  state change.
- All other keys pass through to `tui-input` for line-edit handling.
- `Tab` does **not** cycle focus while an overlay is open; the existing
  Tab focus cycle from `P8-006` is suspended for the overlay's lifetime
  and resumes when the overlay closes.
- The status bar shows `Enter confirm · Esc cancel` while the overlay is
  open so the operator always sees the active keymap.

### Overlay Placement and Render

Overlays render as a centered modal frame over the existing two-panel
body. The frame width is `min(60, terminal_width - 4)`; height is fixed
at 3 rows (border + input row + border). The mux-picker variant grows
height to fit candidates. Background body content stays visible but
dimmed.

This shape is locked because three callers all want roughly the same
mid-screen prompt. A future heavyweight input case (multi-line, multi-
field) would need its own ADR addendum.

### Module Boundary

The primitive lives at `src/tui/widgets/input.rs` (new module). It exports:

- `TextInputState` — wrapper around `tui_input::Input` with the
  Conspectus-specific defaults (initial value, max length if any).
- `TextInputWidget` — Ratatui widget that renders the bordered modal.
- `handle_key` — pure key dispatcher that returns one of
  `InputOutcome::Continue`, `InputOutcome::Confirm(String)`,
  `InputOutcome::Cancel`.

Callers (`rename_action`, `/` search, `P8-014` picker) own their overlay
state in `App` and call into the primitive for key handling and render.
The primitive does not own any global state.

### Dependency Boundary

`tui-input` is the only addition. The crate is scoped to the TUI module
and must not leak into discovery, model, or output crates. This matches
the boundary stance in ADR 0024.

This ADR does **not** lift any of ADR 0024's other deferrals: heavyweight
matchers, async runtimes, and retained-mode frameworks still require
their own follow-on ADRs.

## Consequences

- Rename (`H-RENAME-011`), `/` search (`T8-017`), and inline mux-picker
  (`P8-014`) share a single line-edit implementation. Bug fixes in
  Unicode cursor math, word-jump behavior, or paste handling land once.
- Operators get consistent line-edit muscle memory across all TUI input
  surfaces.
- The TUI dependency surface grows by one small crate. Binary-size
  impact is expected to be negligible; verify on adoption.
- Future multi-line or multi-field input cases require a separate ADR
  addendum, not an organic extension of this primitive.
- `T8-017` may now land without re-litigating the input-widget question.
  That unblocks a story that has been waiting on the rename workstream
  to settle the dependency policy.

## Alternatives Considered

- **Hand-roll line edit per caller.** Rejected. Three concurrent callers
  is too many for "prefer hand-rolled first." Compounded Unicode and
  word-boundary bugs are not worth saving one small dependency.
- **Build a Conspectus-internal mini text-input.** Rejected. Equivalent
  to hand-rolling but with the same maintenance cost in one place. The
  external crate is already that.
- **Use `tui-textarea` (multi-line) instead.** Rejected. None of the
  three v1 callers need multi-line; the crate is heavier and changes the
  default key bindings in ways that would conflict with the locked
  semantics above.
- **Defer until a fourth caller appears.** Rejected. Three is already
  enough; a fourth would re-open the question only to land on the same
  answer with more accumulated technical debt.

## Open Questions Answered

- The line-edit primitive uses `tui-input`, not a hand-rolled string +
  cursor index.
- The overlay key semantics are locked at `Enter` confirm / `Esc`
  cancel; `Tab` does not cycle focus while an overlay is open.
- The primitive lives at `src/tui/widgets/input.rs` and is owned by
  caller-side overlay state, not by a global modal manager.
- This ADR does not lift any of ADR 0024's other dependency deferrals.
