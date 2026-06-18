# ADR 0076: Scrollbar Widget Choice For Scrolled Panes

## Status

Accepted

## Context

Two of the three scrolled regions in the TUI already manage a
vertical scroll offset against a paragraph buffer:

- The left pane (row tree) — `App::left_scroll` +
  `App::adjust_left_scroll`, rendered via `Paragraph::scroll`.
- The right-pane explorer header (`H-RIGHT-SCROLL`) —
  `App::explorer_scroll` + `App::adjust_explorer_scroll`, same
  `Paragraph::scroll` pattern.
- The right-pane preview — `App::preview_scroll`, again driving
  `Paragraph::scroll`.

In each case, content can exceed the viewport. Today there is no
visible cue that more content exists beyond the rendered region —
the operator has to navigate to discover it. We want a vertical
scrollbar inside the pane border that:

- Indicates total content length and the visible window's position.
- Hides itself when content fits in the viewport (fade-on-fit), so
  short lists don't burn a column on a redundant signal.
- Composes with the existing `Paragraph::scroll(...)` machinery
  without inverting how scrolling is owned.

## Options Considered

### Option A — `ratatui::widgets::Scrollbar` (re-exported from
`ratatui-widgets 0.3`, already in our dependency tree via the
ratatui 0.30 dep)

`Scrollbar` is a `StatefulWidget`; `ScrollbarState` carries
`content_length`, `position`, and an optional
`viewport_content_length`. The widget renders a 1-column track on
the requested edge of a `Rect`. Position and length are caller-
owned values that the existing scroll-offset code already produces.

The canonical usage pattern (per the upstream example) is to
render the content (`Paragraph::scroll((offset, 0))`) and then
render the scrollbar over a `Rect::inner(Margin { vertical: 1,
horizontal: 0 })` of the same pane, with the appropriate
`ScrollbarOrientation`.

Trade-offs:
- + Already in our deps; no new crate adoption.
- + Zero refactor of how scrolling is owned. We already have the
  scroll offsets and the total-line counts; we just hand them to
  the widget.
- + Composes per-call: render the bar only when `content_length >
  viewport_height` to implement fade-on-fit.
- + Themable via the standard `Style` setters
  (`thumb_style`, `track_style`, `begin/end_style`).
- − Track / thumb characters are configurable but symbol-level
  (not pixel-level). Acceptable for a terminal UI.

### Option B — `tui-scrollview` (third-party crate)

`tui-scrollview` provides a `ScrollView` container that renders
arbitrary widgets into an offscreen buffer larger than the visible
viewport and clips a window of that buffer. State is owned by the
crate (`ScrollViewState`); the offset is driven by the operator's
key handling against that state.

Trade-offs:
- + Solves the "nested widgets that should scroll together" case
  cleanly (e.g. a form with multiple sub-widgets).
- − Requires inverting how scrolling is owned in all three places.
  Today the caller computes the offset, calls
  `Paragraph::scroll`, and lays out the body to the visible
  viewport. The `ScrollView` model wants the body laid out into
  an off-screen buffer of the full content height, with
  `ScrollView`'s own state machine driving the offset.
- − Adds a new dependency that we'd carry indefinitely. ADRs 0026
  / 0067 set a high bar for new tooling adoption (CLAUDE.md:
  "Do not introduce new project dependencies ... just because they
  are convenient.").
- − Our content is a single tall list per pane, not a heterogeneous
  widget grid. The crate's strength does not match the shape of
  our content.
- − The standalone scrollbar still lives inside the crate via
  `Scrollbar`; the broader `ScrollView` API is the part that
  costs us, and it does not replace the rendering we already
  have.

## Decision

Adopt `ratatui::widgets::Scrollbar` / `ScrollbarState` for the
left pane, the right-pane explorer header, and the right-pane
preview. No new crate dependency. Each call site:

1. Computes (or already has) the total `content_length` and the
   `viewport_content_length` (the visible body height).
2. Skips rendering when `content_length <= viewport_content_length`
   — the fade-on-fit rule keeps the bar from cluttering small
   lists.
3. Renders the paragraph as today (`Paragraph::scroll((offset,
   0))`).
4. Renders a `Scrollbar::new(ScrollbarOrientation::VerticalRight)`
   over the pane's inner area with a 1-row vertical margin (so the
   bar sits inside the border without overpainting it).
5. Drives `ScrollbarState::new(content_length).position(offset)`
   from the existing `*_scroll` state on `App`.

### Placement: inside the pane border

The scrollbar consumes the rightmost column of the inner area, with
a `Margin { vertical: 1, horizontal: 0 }` so the track does not
overpaint the existing border. This matches the upstream
documentation's example. Outside-the-border placement (rendering on
the border itself) was considered briefly and rejected: the border
is part of the focus-state visual language (highlighted on the
focused pane), and the scrollbar would fight that signal.

### Length semantics

`content_length` is the total wrapped (post-wrap) line count of the
buffered content. `viewport_content_length` is the visible body
height (the `Paragraph` area's height). For the explorer header
this matches the existing `wrapped_rows` sum we already compute
for the header-height budget; the new code reuses that value. For
the left pane it matches the visible row count; we already iterate
all visible rows to build `lines`. For the preview pane we sum
per-line wrap counts the same way the explorer header does, since
the preview renderer already produces a `Vec<Line<'static>>` per
draw.

### Fade-on-fit

The scrollbar render is gated on `content_length >
viewport_content_length` at each call site. When content fits, no
scrollbar is rendered and the rightmost column reverts to the
paragraph content. Operators on small lists pay nothing for the
indicator; once a list grows past the viewport the bar appears
without ceremony.

### Theme integration

In v1 the scrollbar uses ratatui's default `Style` — no new theme
keys. If operator feedback wants the thumb / track in the project
palette, a follow-up adds `theme.scrollbar_thumb` /
`theme.scrollbar_track` via the existing `Theme::known_keys` +
`set_color` machinery (ADR 0032). Deferring keeps the surface
small and avoids painting bikesheds into the initial story.

## Consequences

- No new dependency. The `ratatui-widgets` crate ships with our
  existing `ratatui 0.30` dep, so the change is render-only.
- Buffer-shaped tests (`render_to_buffer` + `buffer_to_string`)
  pick up the scrollbar glyphs without infrastructure changes.
  The two existing pixel-shape buffer tests (`right_pane_*`) keep
  working because the column scrubbed by the scrollbar is the
  rightmost; the assertions on the per-row content live to the
  left of it.
- The `MIN_PREVIEW_HEIGHT` floor from the right-pane scroll work
  remains the dominant constraint on the preview area; the
  scrollbar consumes only the rightmost column of whatever height
  the preview gets.
- The existing `Wrap { trim: false }` configuration on all three
  paragraphs is unchanged. Wrap counts feed `content_length` so
  the bar reflects the true rendered length.

## Alternatives Considered

- **Status-bar arrows / chevrons** (`↑12 ↓7` style hint): cheap to
  implement but only signals direction, not position; loses the
  proportional "how far through am I" cue that a scrollbar gives.
- **Always-visible scrollbar.** Rejected per the fade-on-fit
  discussion above.
- **Outside-the-border scrollbar.** Rejected per the placement
  discussion above.
- **Roll our own.** No reason to. The built-in widget already
  does what we need and tracks ratatui's idioms.

## Open Questions

- Whether the preview pane's scrollbar should reflect `chars(N)`
  vs `lines(N)` for variable-width content (PR diffs, transcripts
  with long single-line entries). v1 uses line-count; if the
  thumb feels off on diff-heavy previews, a follow-up can switch
  to wrap-aware row count or content-character count.
- Whether the explorer scrollbar should disappear when the
  cursor's row is on the `OtherHeader` (which is selectable but
  the toggle hasn't been activated). For v1 the bar reflects
  whatever the renderer drew; if operators read the bar's
  position as "cursor" instead of "viewport," a follow-up clarifies
  through a marker on the thumb itself.
