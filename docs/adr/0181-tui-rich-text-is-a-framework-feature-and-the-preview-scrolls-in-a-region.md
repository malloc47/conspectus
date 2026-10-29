# ADR 0181: Rich Text Is A Framework Feature, And The Preview Scrolls In A Region Counted From An Anchor

## Status

Accepted. Carries out item 3 of
[ADR 0152](0152-tui-framework-dependency-ownership.md) (the rich-text
crates behind a `richtext` feature) and the `richtext` row of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md)'s control
set, applying its house rules 2, 12, 13, and 14, and extends
[ADR 0150](0150-tui-framework-workspace-crate.md) with the framework's
`richtext` module and `scroll::ScrollRegion`. Amends
[ADR 0025](0025-tui-preview-ansi-rendering-dependency.md),
[ADR 0051](0051-tui-transcript-preview-markdown-dependency.md), and
[ADR 0054](0054-viewer-table-rendering-comfy-table.md): `ansi-to-tui`,
`tui-markdown` (with its `syntect` highlighting), and `comfy-table` are
optional dependencies of the framework behind its feature, which
`conspectus` turns on, not direct dependencies of `conspectus`; the
shim, the Markdown entry, the table pre-pass, and the table layout they
describe are the framework's. Amends
[ADR 0106](0106-preview-pane-wrap-modes.md): the three wrap modes and
the captured-pane layout are the framework's `WrapMode` and
`layout_capture`; Conspectus keeps the setting (`[tui] preview_wrap`,
the state file's token, the Controls entry). Amends
[ADR 0154](0154-tui-glyph-marks-vetted-table-and-scan.md): a rendered
table's border glyphs are in the framework's vetted table, not the
host's. Carries out what
[ADR 0180](0180-tui-the-transcript-viewer-is-a-pager-content.md)
decision 6 left (the Preview's window and anchoring move to the
framework), with the Preview a scroll region rather than the embedded
pager [ADR 0177](0177-tui-readers-are-one-pager-over-content-the-host-lays-out.md)'s
control page sketched, and applies
[ADR 0156](0156-tui-scroll-window-follow-rule-and-viewports-by-value.md)'s
viewports-by-value rule to it.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/05-search-help-viewers.md`, section
2.6 and section 4, and `01-design-tokens.md`, section 8) found the
Preview zone and the transcript viewer holding generic rich-text code and
a generic scroll region beside their domain sources:

- **Rich text in three places.** The ANSI shim (`ansi-to-tui` with a
  color-off path and a malformed-input fallback) sat in the preview
  pane's renderer; the captured-pane layout and its three wrap modes in
  `src/tui/preview_wrap.rs`, typed by a config enum that also spelled the
  setting; and Markdown rendering (the `tui-markdown` call, a fix for a
  loose list's orphaned markers, the wrap) with the GFM table pre-pass
  and the `comfy-table` layout in the viewer. The three crates were
  direct dependencies of `conspectus`, though ADR 0152 put them behind a
  framework feature, and the host vetted the table border glyphs
  `comfy-table` draws.
- **Three anchoring models on two counters.** The Preview scrolled from
  the top (text, listings, a git status, a PR), from the bottom (a
  transcript, newest turns in view), or from a search match it opened
  on. The App held a `u16` offset that meant rows down from the top or
  rows up from the bottom, depending on the source, and an `i32` offset
  from the match; the reducer flipped the step's sign for the bottom;
  the frame clamped the first counter and not the second, so `K` after
  scrolling past the end of a transcript opened on a match did nothing
  for as many presses as had gone past. Each source cut its own window
  or let the zone scroll it, and the zone drew two scrollbar recipes.

## Decision

1. **`tuikit::richtext`, behind a feature.** The framework's `richtext`
   cargo feature (off by default) turns on the module and its three
   optional dependencies; `conspectus` names it in its dependency on the
   framework and drops its own. `syntect` stays a dependency of
   `tui-markdown`'s `highlight-code` feature, which the workspace entry
   already turns on, so the framework's dependency allowlist is
   unchanged and no package enters the lockfile.

2. **What it holds.** `ansi_text(raw, color)` (the shim, its fallbacks
   kept); `layout_capture(lines, mode, source_width, width)` and
   `WrapMode::{Plain, Smart, None}` (moved from `preview_wrap.rs`, the
   tmux pane width now the source's width); `markdown(body, width,
   theme)` (the GFM-aware rendering with tables and highlighted fenced
   code, the list-marker fix included); `segment_body`, `ParsedTable`,
   `TableAlign`, `BodySegment`, and `render_table` (moved from
   `src/viewer/table.rs`); and `code(source, language)`, a highlighting
   entry point for source outside Markdown. Their tests moved with them,
   expectations unchanged.

3. **What stays.** Conspectus keeps the setting: `crate::tui::WrapMode`
   re-exports the framework's type, and `crate::tui::preview_wrap` keeps
   its token (`smart`, `plain`, `none`) and its Controls label. The
   viewer keeps the rendering of turns (gutters, chips, tool bodies,
   thinking), calling `markdown` for a message body. The Preview keeps
   its sources, loaders, store, failure banner, Related-row header,
   search derivation, and the window a pane capture's marks pick.

4. **The table's glyphs.** The box-drawing joints, double rules, and
   dotted lines a rendered table's borders use join the framework's
   vetted table, and the set the renderer tells borders from cells by is
   a constant of the glyphs module; the host no longer registers them.

5. **`ScrollRegion`.** `tuikit::scroll::ScrollRegion` is a cursorless
   window whose offset counts from an `Anchor`: `Top`, `Bottom` (it
   keeps its distance from the end as rows arrive), or `Mark` (a row the
   content marks; the host gives a match's first row less its context).
   It is a `Copy` value the host keeps:
   - `scroll_by` steps it; a step toward the anchor's own end stops there
     at once, and a step past an end the anchor does not stop at is taken
     back by the next placement, for all three anchors;
   - `set_anchor` with another anchor, or a placement from another, starts
     it at that anchor;
   - `place` clamps it over the rows a frame laid out;
   - `draw_lines` wraps lines to the area and windows them, a leading
     banner scrolling with them; `draw_cut` draws rows the host cut at
     the placed window under a banner that stays put, the scrollbar
     tracking the window; both draw the shared scrollbar and return the
     region drawn; `rows_under` is the room a banner leaves.

6. **Not the pager's embedded mode.** The pager owns its content and
   draws through `&mut`; the Preview draws from a shared borrow of the
   App every frame, over a source the selection picks and the host
   rebuilds. The pager counts from the top with a sticky end; the
   Preview counts from the bottom or a match. The pager paints its own
   find; the Preview paints a search the host derives. Embedding the
   pager would have changed the frames and kept a second copy of every
   source's rows in it. The embedded mode stays the log list's detail.

7. **The Preview on the region.** The App holds one `ScrollRegion`. `J`
   and `K` set its anchor from what the Preview shows (a transcript's
   match when a search names a turn, a transcript's newest turns, or the
   top) and step it. A transcript places it from the bottom or at its
   match and cuts and paints only the rows of the placed window; every
   other body draws through `draw_lines` from the top; the frame returns
   the region drawn, and the App keeps it, except while the source is
   still loading, so keys pressed then are kept.

## Consequences

- **No frame changes.** Before and after frames of the Preview (a pane
  capture in each wrap mode and both focuses, a transcript scrolled
  either way and past both ends, opened on a match and moved from it, a
  hit in a hidden turn, PR, git, listing, and repository previews,
  Related rows over a transcript and over a git status, an unavailable
  transcript) at 100x30 and 60x20, and the transcript viewer's frames,
  are byte-identical, styles included.
- **Two behavior changes in live use**, both the framework's rule: an
  offset from a search match is taken back inside the transcript by
  each frame, as the other two already were, so the first `K` after
  scrolling past the end moves the Preview, and a match on the last
  page moves on the first `K`; and a change of anchor with no change of
  selection (a search shown or cleared on the same row) starts the
  Preview at the new anchor instead of restoring an offset kept from
  before. Neither shows in a one-frame snapshot, where the Preview loads
  after its keys.
- The framework builds and tests without the feature; a program that
  shows no rich content pays for none of the parsing crates.
- `src/viewer/` no longer imports the rich-text crates; its dependency
  manifest lists the framework's feature instead, so an extracted viewer
  (CSP-342) would depend on the framework with it.
- Another body that keeps its place from its end or a match (a log
  tail, a live capture) is a region and a source.

## Alternatives Considered

- **The pager's embedded mode for the Preview.** One reader for every
  scrolled text, but the reasons in decision 6: ownership and borrowing,
  the anchors it lacks (a seam on its page), its own find look, and
  frames that would change.
- **Keep the two counters inside the region.** It would restore an
  older offset after a match comes and goes on the same row, but it
  carries the model's accident (two meanings of one number) into the
  framework, and a region holds one place.
- **Leave the match offset unclamped.** Byte-identical in every key
  sequence, but it keeps dead keys after scrolling past an end, which
  house rule 12 and the other two anchors already rule out.
- **Keep `PreviewWrap` in the host beside `WrapMode`.** Two enums with
  the same three variants and a mapping between them; the setting needs
  only its spelling, so the host keeps the token and the label as
  functions over the framework's type.
- **Rich text always on.** Simpler for this host, which turns it on, but
  ADR 0152 kept a framework consumer that never renders rich content
  from building a highlighter.
- **Classify table borders by Unicode block.** Shorter than a set, but a
  box-drawing character inside a cell would take the border style; the
  renderer's own set keeps cells as they were.
- **No `code` entry point.** The callers today reach highlighting only
  through Markdown's fenced code; the entry point names the feature's
  syntax highlighting directly, for a content that shows a source file.

## Open Questions Answered

- *A `ScrollRegion` or the pager's embedded mode?* A region; the pager's
  page records why.
- *Where do the wrap modes live, and who spells them?* The modes and the
  layout in the framework; the token and the label in Conspectus.
- *Does the framework name `syntect`?* No: it comes with `tui-markdown`'s
  highlighting, as it did for the viewer.
