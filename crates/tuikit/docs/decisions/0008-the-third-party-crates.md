# 0008: The framework owns a small set of third-party crates

| | |
| --- | --- |
| Status | Accepted |

## Decision

1. **Kept and owned:** `ratatui`, the rendering library every control draws
   with, whose crossterm backend's event types (reached through
   `ratatui::crossterm`, so the two cannot disagree on a version) are the
   keys every control handles; `unicode-width` for cell widths; and
   `tui-input`, the one single-line edit engine, behind `LineEdit`.
   `crossterm` stays on the allowlist although the crate names no direct
   dependency on it.
2. **Optional:** the rich-content crates sit behind a feature (record
   [0009](0009-rich-content-behind-a-feature.md)).
3. **Replaced with in-tree code:** a multi-select crate by `Checklist`, a
   toast crate by `Toasts`, and a popup-sizing crate by `ModalFrame`.
4. **Development only:** `toml` (the manifest guard's parser, already in the
   lockfile), `insta` (snapshots of values other than frames), and
   `tui-pantry` (a widget preview example).
5. **No new dependency without a record.** A change that needs a crate
   writes a record here first (the need, the alternatives compared, the
   conclusion), and the allowlist in the independence guard changes in the
   same change and cites it.

## Rationale

The three replaced crates cost more than the code they saved. The toast
crate drew a hard-coded green border that ignored the theme and had no
severity; the multi-select kept a per-frame mirror of its state and needed a
hand-written `Clone`; the popup crate made every modal compute its rect twice
and needed eleven identical trait impls. With a framework boundary, that
friction would land on every future host. `tui-input` had no such cost and
gets graphemes, word boundaries, and the kill buffer right, which is not
worth owning. Each dependency weighs on build time and binary size, so the
set stays narrow.

## Consequences

- The build lost three crates and gained about the code they wrapped (a
  checklist state machine, a toast queue, a sizing helper); the workarounds
  went with them.
- `tui-input` binds its word keys to Meta where crossterm reports Alt; the
  framework wraps that (`LineEdit` maps the modifier) rather than forking.
- Multi-line input, when wanted, would use `ratatui-textarea`: the older
  `tui-textarea` does not resolve against the current ratatui. It needs a
  record first.

## Alternatives considered

- **Keep wrapping the three crates behind the framework's API.** Least code,
  but the framework's behavior would follow three young crates' choices.
- **Adopt a cohesive kit** (`rat-widget`, `ratatui-interact`, `tuika`).
  All-or-nothing adoption, or high churn.
- **Replace `tui-input` with in-tree editing.** Not worth owning grapheme and
  word logic that an established crate gets right.
- **A retained-mode or component framework over ratatui.** Rejected in
  record [0010](0010-immediate-mode-and-no-component-framework.md).

## Origin

- host ADR 0152, "Who owns the framework's third-party crates": decisions 2
  to 6.
- host ADR 0024, "The interface's runtime, architecture, and dependency
  policy": ratatui with the crossterm backend, and a dependency policy
  narrower than the project's because interface crates weigh on size and
  build time.
- host ADR 0030, "A text-input primitive": adopting `tui-input` for
  single-line editing rather than editing by hand.
