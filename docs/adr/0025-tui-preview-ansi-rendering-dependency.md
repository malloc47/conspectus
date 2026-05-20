# ADR 0025: TUI Preview ANSI Rendering Dependency

## Status

Accepted

## Context

The Phase 8 TUI's right-panel preview renders the output of
`tmux capture-pane` for the selected mux session
(`P8-009`). With `capture-pane -p -J` (the v1 invocation), tmux
emits plain text without color, so agent activity that uses
terminal colors (claude-code, codex, opencode, etc.) shows up
in the preview as a monochrome wall of text. Operators
validating the v1 binary called this out as a regression
relative to looking at the pane directly: the visual cues that
help them recognize what an agent is doing — diff coloring,
error highlights, status spinners — are gone.

Two design options for getting color into the preview:

1. **Hand-roll an SGR parser inside `src/tui/preview.rs`.** The
   subset needed is small (CSI `m` sequences, 8/16/256 color
   tables, bold/italic/underline). Maybe 100–150 lines of code
   plus tests. No new dependency, but ongoing maintenance burden
   inside the TUI module and another place to keep the ratatui
   colour map in sync with ADR 0022's palette.
2. **Pull in `ansi-to-tui`.** A small crate that parses ANSI
   escapes into a `ratatui::text::Text<'static>` of styled
   `Span`s. Active maintenance, ratatui-aware, used widely. Adds
   one direct dependency and one transitive (`nom 8`). ADR 0024
   reserved the right to pull in additional TUI crates after a
   follow-on ADR rather than freely; this ADR is that follow-on.

The dependency policy in ADR 0024 is intentionally narrow because
TUI crates can have outsized impact on binary size and build
time, and Conspectus distributes as a single binary (ADR 0016).
For each candidate dep the question is whether the surface area
it removes from our own code justifies the additional dep weight.

For `ansi-to-tui` specifically:

- The crate's purpose is exactly what we need (ANSI bytes →
  ratatui Text), with no overlap into territory we want to own.
- Compiled size adds roughly tens of KB after dead-code
  elimination; build time impact is negligible.
- The alternative (in-tree parser) means we'd write, test, and
  maintain a CSI/SGR parser ourselves — a known footgun with
  long-tail edge cases (256-colour, truecolour, hyperlink
  escape, OSC sequences) that the operator-facing preview would
  surface immediately.
- The operator-facing benefit is clearly visible: muxed agent
  output regains the colour it had in the source pane.

## Decision

Adopt **`ansi-to-tui = "8.0.1"`** with `default-features = false`
as a direct dependency of the `conspectus` crate. The crate
re-exports `nom 8` as a transitive build dependency; no other
transitives.

Use it from a new tiny helper in `src/tui/preview.rs` that
converts a captured pane string into a `ratatui::text::Text<'static>`
ready for `Paragraph::new`. The renderer keeps owning layout,
wrapping, and cropping; this dep handles only the byte → styled-
spans conversion.

Capture path change in `src/discovery/tmux/mod.rs`:

- The v1 `SystemTmux::capture_pane` invokes
  `tmux capture-pane -p -J -t <target>`. Add the `-e` flag so
  tmux emits the pane's escape sequences alongside the visible
  text. Existing fake-runner tests are unaffected — they return
  pre-canned payloads either way.

Render path change in `src/tui/ui.rs`:

- `format_preview_for_mux` now returns `ratatui::text::Text<'static>`
  instead of a plain `String`. Captured panes pass through the
  ansi-to-tui parser; non-text variants (`NoTarget`, `Failed`,
  `Unavailable`, `Unsupported`, "loading") remain plain text and
  build a `Text` of a single unstyled span so the renderer's
  call site doesn't have to branch.
- The existing `crop_bottom_lines` helper continues to apply to
  the raw captured string before parsing, so cropping happens
  on logical lines (matching what the operator would see in the
  source pane).

ADR 0022 `--color` resolution carries through: when colour is
disabled, the parser is bypassed and the captured text is
rendered with style stripped to a single plain span. The TUI's
`RunConfig.color` flag already encodes this.

Cargo features adopted: `default-features = false`. The crate's
default features enable `simd` (faster parsing via SIMD intrinsics)
and `zero-copy` (`Text<'a>` instead of `Text<'static>` when the
input outlives the output). Neither is needed for our workload —
preview captures are short and live longer than the render — and
keeping the feature surface tiny minimises future surprise.

## Consequences

- Muxed-agent preview regains the colours the operator sees in
  the source pane, which is the visual signal P8-009 set out to
  surface in the first place.
- Two new crates land in the dependency graph (`ansi-to-tui` +
  `nom`). The single-binary distribution story stays intact.
- The TUI module avoids owning a CSI/SGR parser, with its
  known edge cases (truecolour, OSC, hyperlinks).
- `--color=never` still strips styling — the ansi-to-tui pass
  is gated on the resolved colour flag, so the v1 colour
  contract (ADR 0022) stays consistent.
- Snapshot tests over `format_preview_for_mux` now assert on
  styled `Text` content rather than raw strings; the existing
  text-asserting tests continue to work because `Text`'s
  flattened string view matches the previous output minus
  escape bytes.

## Alternatives Considered

- **Hand-roll an SGR parser.** Rejected because the long-tail
  edge cases (256-colour, truecolour, OSC sequences, hyperlink
  escapes) would surface as operator-visible glitches and we'd
  spend the same time maintaining the parser that we'd save by
  not taking the dep. The 100–150 LOC estimate ignores test
  surface and ongoing fixups.
- **`ansi-to-tui-forked`.** Older fork pinned to a ratatui
  pre-rename; not maintained against ratatui 0.30. Rejected.
- **`vte` crate.** Lower-level terminal-emulator parser. Rejected
  because we'd still have to write the SGR → ratatui style
  mapping on top, defeating the "reduce surface area" goal.
- **Skip colour for v1; surface as a known limitation.**
  Rejected because operator feedback during binary validation
  flagged this as a regression worth fixing before v1 ships.
- **Pull `ansi-to-tui` with `default-features = true`.** Rejected
  because the `simd` feature requires an additional transitive
  (`bytecount`) and the `zero-copy` feature changes the public
  `Text<'a>` lifetime in ways we don't need.

## Open Questions Answered

- The dependency policy in ADR 0024 permits new TUI crates only
  after a follow-on ADR. This is that ADR for `ansi-to-tui`.
- `--color=never` continues to strip styling in the preview;
  the conversion to styled `Text` is gated on the colour flag.
- `tmux capture-pane` invocation adds `-e`. The pre-existing
  `-p -J` flags remain, so cropping/wrapping continue to work
  against logical lines.
- The crate's `simd` and `zero-copy` features stay off for v1
  to minimise the dependency surface; revisit if profiling
  shows the parser is a bottleneck.
