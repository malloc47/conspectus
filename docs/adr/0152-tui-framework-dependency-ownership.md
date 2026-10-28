# ADR 0152: Who Owns The Framework's Third-Party Crates

## Status

Accepted. Extends [ADR 0150](0150-tui-framework-workspace-crate.md). Narrows
ADR 0024's dependency policy for the TUI: the framework crate owns the TUI's
third-party widget dependencies, and three shims are replaced. Reverses the
adoption decisions behind CSP-426 (`ratatui-cheese` multi-select) and CSP-427
(`ratatui-comfy-toaster`), for the reasons below.

## Context

Moving widgets into a new crate moves ownership of the crates they wrap.
The project rule is that a dependency change needs documented need,
alternatives, and an ADR. The TUI depends on `ratatui` and `crossterm`, plus
`tui-input`, `ratatui-cheese`, `ratatui-comfy-toaster`, `tui-popup`,
`tui-markdown`, `ansi-to-tui`, `comfy-table`, and, as a dev dependency,
`tui-pantry`.

The inventory examined what each one costs today:

| Crate | Use | Cost found |
| --- | --- | --- |
| `ratatui-cheese` | `multi_select` shim; `help::Binding` | the checklist shim keeps a per-frame mirror state and a hand-written `Clone`; the help use is an inert struct |
| `ratatui-comfy-toaster` | toasts | the border is hard-coded green and ignores `theme.success`; there is no severity |
| `tui-popup` | popup framing | each modal computes its rect twice to satisfy `KnownSize`; 11 identical hand-written `KnownSize` impls |
| `tui-input` | the single-line edit engine | none found; keeps `visual_cursor` that the house rules need |
| `tui-markdown`, `syntect`, `comfy-table`, `ansi-to-tui` | transcript and preview rendering | heavy, and only the pager's rich-text content needs them |
| `tui-pantry` (dev) | the widget preview example | none; a dev-only harness |

The ecosystem survey (`docs/plans/tui-framework/gap-analysis.md`,
`research/`) found no adoptable cohesive replacement: `rat-widget` is
all-or-nothing (CSP-433), and the new design kits (`tuika`, `tui-lipan`,
`ratatui-themekit`) are 0.x with high churn.

## Decision

1. **Crate name.** The framework crate is `tuikit`, at `crates/tuikit/`, with
   `publish = false`. The name is a working name: it is neutral (it carries no
   Conspectus vocabulary) so extraction is a directory lift. A rename before
   extraction is mechanical.

2. **Keep and own.** The framework depends on `ratatui`, `crossterm`,
   `unicode-width`, and `tui-input` (behind `LineEdit`, the only edit engine).

3. **Behind a feature.** `ansi-to-tui`, `tui-markdown` (with `syntect`
   highlighting), and `comfy-table` sit behind a `richtext` cargo feature, off
   by default in the framework and enabled by `conspectus`.

4. **Replace with in-tree code.**
   - the `ratatui-cheese` checklist shim becomes the framework's own
     `Checklist`, and its `help::Binding` use disappears with `HelpPage`;
   - `ratatui-comfy-toaster` becomes the framework's own `Toasts`, with
     severity and a theme-honoring border;
   - `tui-popup` becomes the framework's `ModalFrame`, which sizes once.

   Three dependencies leave the build. Each replacement lands inside the story
   that extracts the control, and that story records the line-count and
   behavior comparison.

5. **Dev dependency.** `tui-pantry` stays a dev dependency, used by the
   existing preview example, which moves with the controls it previews. No new
   gallery or coverage rule is introduced.

6. **No new dependencies.** This ADR adds none. A framework story that needs
   one writes its own ADR first (the standing rule).

## Consequences

- The build loses three dependencies and gains about the code they wrapped,
  mostly small: a checklist state machine, a toast queue, and a popup sizing
  helper. The workaround code that exists only to fit the shims goes with them.
- The earlier swaps (CSP-426, CSP-427) traded maintenance for dependencies
  when the TUI had no framework boundary. With one, the shims' friction (theme
  bypass, mirror state, double sizing) is the cost, and it lands on every
  future consumer.
- `richtext` keeps a framework consumer that never renders a transcript from
  paying for `syntect`.
- `tui-input`'s Alt-b/Alt-f behavior (it binds META where crossterm reports
  ALT) is wrapped, not forked: `LineEdit` maps the modifier.
- `tui-textarea` stays out. Version 0.7.0 declares `ratatui ^0.29` and does not
  resolve against 0.30; CSP-430 should retarget `ratatui-textarea` when a
  multi-line field is wanted.

## Alternatives Considered

- **Keep wrapping all three shims behind the framework API.** Least code now;
  keeps the green-only toast border, the mirror-state workaround, and the
  double rect computation, and makes the framework's public behavior depend on
  three young crates' choices. Rejected by the operator.
- **Adopt a cohesive kit** (`rat-widget`, `ratatui-interact`, `tuika`). Rejected
  for the reasons in the context: all-or-nothing adoption or churn.
- **Replace `tui-input` with in-tree editing.** The Rust ecosystem's headless
  editor is established (0.15.x, hundreds of thousands of recent downloads), and
  the house rules only need `visual_cursor`. Not worth owning grapheme and word
  logic.
- **Drop `richtext` from the framework.** Leaves the pager with no rich
  content; the preview pane and the transcript viewer both want it.

## Open Questions Answered

- *Does the framework need a mouse or animation crate?* No. Mouse hit testing
  and animation are Tier 2 and skipped items in the gap analysis; each story
  leaves a seam open.
- *Where does `tui-pantry` go?* It stays a dev dependency; the preview example
  moves with the controls.
