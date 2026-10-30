# Inventory 07: App chrome and the right-pane explorer/detail

Slice: the frame around the screen (header line, pane frames, tab strip,
status/hint line, toast, overlay stacking) and the right pane (explorer,
detail model, Preview frame). Evidence-gathering only; nothing here changes
code. Paths are relative to the repo root; `file:line` citations are against
this worktree (`worktree-tui-framework`, HEAD `ca668a3`).

Method caveat: read-only code reading. I did not run cargo, tests, or the
snapshot renderer, so every claim about rendered output (including the
narrow-width arithmetic in section 6) is derived from the source, and is
labelled "by arithmetic" or "suspected" where it was not observed.

## 0. Reading guide

- Sections 1-2: the screen broken into parts, each mapped to a generic control.
- Section 3: per-control rubric entries (surface, variants, leaks, verdict).
- Section 4: the explorer state machine as a candidate generic control.
- Sections 5-7: theme tokens, glyph vocabulary, narrow-width and key behavior.
- Section 8: duplications and inconsistencies (counts + locations).
- Section 9: verdict table and API sketches. Section 10: open questions.

Size of the slice (LOC): `ui.rs` 395, `ui/header.rs` 681, `ui/right_panel.rs`
921, `ui/detail_header.rs` 376, `ui/preview_pane.rs` 470, `ui/text.rs` 210,
`explorer.rs` 2156 (+1690 tests), `detail.rs` 1409 (+1180 tests),
`app/explorer_nav.rs` 393, `widgets/toast.rs` 175, `widgets/value_modal.rs`
239, `cursor.rs` 52. Left panel (`ui/left_panel.rs`, 1710) is another slice;
only its frame, title, and scrollbar code is cited here.

## 1. Screen anatomy

```
row 0      header line            ui.rs:85 vertical![==1, >=3, ==1]; header.rs:5
rows 1..-1 body                   ui.rs:312 draw_body, 50/50 split
  left  pane = Block(ALL) + title "► sessions · mux · prs · pinned · history · search"
  right pane = Block(ALL) + title "► <kind-glyph> <kind> ◄ <breadcrumb> · depth N"
     explorer zone  (auto-height)  Node fields / "Related" rule / link rows / Other
     1-row divider  "Preview" chip rule
     preview zone   (>= 6 rows when it fits)
row -1     status bar             header.rs:188
overlay    top-of-stack modal     ui.rs:90-103 (14 draw_* calls, one live at most)
overlay    toast (always last)    ui.rs:104, widgets/toast.rs
```

`ui::draw` is not the only screen: `runtime.rs:843 draw_screen` is a three-way
switch (full-screen transcript viewer, full-screen pane viewer, else
`ui::draw`). So "the screen" is a mode switch above the chrome.

### 1.1 Structural part to generic control

| Part | Where | Generic control it is an instance of | Verdict (short) |
| --- | --- | --- | --- |
| Screen shell | `ui.rs:83-105`, `runtime.rs:843` | AppFrame: header row + body + footer row + overlay layer + toast layer | split generic core + adapter |
| Body split | `ui.rs:312-321`, dup `snapshot.rs:343` | SplitPane (side-by-side above a width threshold, stacked below) | extract after refactor |
| Pane frame + focus marker | `left_panel.rs:8`, `right_panel.rs:6`, `ui.rs:323-335` | PaneFrame (bordered block, title line, focus indicator in the title) | extract after refactor |
| Left title strip | `left_panel.rs:222-244` | TabBar / ViewSwitcher (active accented, rest secondary, ` · ` separated) | extract after refactor |
| Header line | `header.rs:5-48` | SummaryLine / ChipBar (bold prefix + opt-in chips) | prefix stays; chip composition generic |
| Status/hint line | `header.rs:188-227,482-582` | StatusLine + KeyHintBar with priority-drop fitting | extract after refactor |
| Provider/in-flight/unseen chips | `header.rs:232-312` | StatusChip (spinner chip, severity chip) | extract after refactor |
| Right title with breadcrumb | `right_panel.rs:214-275`, `explorer.rs:927-1063` | Breadcrumb in a pane title, with elision | split generic core + adapter |
| Chip divider rules | `detail_header.rs:281-320` | SectionRule / ZoneHeader (rule with filled chip label, optional summary) | extract as-is (rename anchors) |
| Node field rows | `right_panel.rs:440-476` | PropertyGrid row: label column, value, placeholder dim, annotation, kind glyph, "(truncated · o)" | extract after refactor |
| Related rows | `right_panel.rs:506-654` | LinkRow: `[prefix] verb-column glyph middle-truncated-label [meta]` | split generic core + adapter |
| Other header | `right_panel.rs:658-683` | Collapsible section header row that is itself selectable | extract after refactor |
| Explorer cursor/zone model | `app.rs:436-518`, `explorer.rs:543-738` | ZonedList with stable row keys + cursor reseating | split generic core + adapter |
| Drill / back / armed back | `explorer_nav.rs:225-392`, `app.rs:1409-1416` | NavStack (breadcrumb of frames) + ArmedConfirm | split generic core + adapter |
| Right-pane vertical layout | `right_panel.rs:5-172` | Adaptive stacked master-detail: content-sized list zone, divider, remainder detail | extract after refactor |
| Preview frame | `preview_pane.rs:8-87,138-159` | DetailPane body: text or windowed text + scrollbar + banner-above | split generic core + adapter |
| Scroll gutter + scrollbar | `left_panel.rs:152-216` | ScrollGutter (fade-on-fit) + follow-cursor offset | extract after refactor |
| Toast | `widgets/toast.rs`, `ui.rs:179-188` | Toast (already a shim over `ratatui-comfy-toaster`) | extract as shim, fix theming |
| Full-value inspect | `widgets/value_modal.rs`, `app.rs:748-783` | PropertyGrid "inspect full value" modal | (overlay slice) pair with PropertyRow |
| Cursor arithmetic | `cursor.rs` | `wrap_step` / `clamp_step` | extract as-is |
| Legacy sectioned detail | `detail.rs`, `ui/detail_header.rs` (minus rules) | none (superseded by the explorer) | delete, do not extract |

## 2. Cross-cutting facts about the chrome

1. **The chrome is a free-function renderer over `&App`.** Every function in
   `ui/` is `pub(super)`, takes `&App` or `&mut App`, and the files share
   state with `use super::*` (`ui.rs:60-65`), so each submodule sees all of
   `ui.rs`'s domain imports (`PrRow`, `RowKind`, `GroupRow`, `Msg`, ...). Crate
   references per file: `ui.rs` 29, `header.rs` 19, `right_panel.rs` 19,
   `preview_pane.rs` 25, `detail_header.rs` 2, `text.rs` 0. Because nothing is
   a standalone widget, none of the chrome can appear in the `tui-pantry`
   preview harness (`examples/pantry.rs` previews only overlays plus a theme
   sheet).
2. **Draw mutates app state.** `ui::draw` takes `&mut App` and dispatches
   `Msg::LeftViewportChanged` (`left_panel.rs:130`) and
   `Msg::ExplorerViewportChanged` (`right_panel.rs:138`) mid-frame, so the
   reducer can reconcile scroll offsets from post-layout measurements
   (`ui.rs:67-82` explains this as an accepted ADR 0085 deviation). This
   "measure in draw, feed back as a message" pattern is the framework's
   scroll contract; see 3.8.
3. **Only the top modal is drawn.** `modal.rs:1-9` says draw "renders the
   stack in order (bottom-to-top)", but every `app.<overlay>()` accessor in
   `app/overlays.rs` (`mux_menu` :398, `help_overlay` :727, `value_modal` :819,
   `controls_overlay` :580, ...) matches `modal_stack.last()` only. The 14
   overlay `draw_*` calls in `ui.rs:90-103` are therefore mutually exclusive and
   their order is irrelevant; only the toast (`ui.rs:104`) is genuinely layered on
   top. Stacked modals show only the top one over the base screen.
4. **Three feedback channels, no shared type.** `status_message`
   (62 `status_message = Some`/`SetStatus(Some` sites outside tests): a single
   line that replaces the hints, always drawn in `theme.warning`
   (`header.rs:194`), regardless of severity. Toast (`post_toast` +
   `Effect::Toast`, about 50 sites): one green Success toast, 1.5 s, bottom
   centre, newer replaces older (`overlays.rs:922`). Message log + unseen chip
   (`header.rs:232`). The explorer's "armed back" hint uses `status_message`.
5. **Focus is signalled by marker, not border.** Both pane `Block`s are
   `Block::default().borders(Borders::ALL)` with no `border_style`
   (`left_panel.rs:8`, `right_panel.rs:6`). Focus shows as: the `► ` title
   marker in `panel_focus_accent` (two spaces when unfocused, so titles do not
   shift; `ui.rs:331`), the dim `[left]`/`[right]` scope tag in the status bar
   (`header.rs:201-209`), and row highlight (left: `selection_active` when
   focused, `selection_inactive` otherwise; right: REVERSED only when
   focused). Modal frames, by contrast, always use an accent border
   (`widgets/popup_frame.rs:65`). ADR 0076 still says the border is "highlighted
   on the focused pane"; the code no longer does that.
6. **No living reference for the chrome.** `docs/tui-columns.md` is test-gated
   but covers only the left panel. `docs/tui-detail-mockup.md` is a review
   artifact that predates ADR 0074: it shows Upstream/Downstream sections, a
   title/breadcrumb block inside the pane, `★` markers, a pane-local hint
   footer, and a `left_pane_sync` setting (`mirror`/`follow`/`none`) that has no
   code (`grep left_pane_sync` finds nothing outside the doc). Mirror is
   hard-coded (`explorer_nav.rs:293`). `docs/plans/sessions-header-audit.md`
   proposes `⚠` and cross-view count words; code uses `▲` and only special-cases
   the Mux view. `docs/plans/workspace-view-redesign.md` is about left-pane
   grouping and has nothing for the chrome.
7. **`CSP-435` premise is out of date.** The task says `explorer.rs` carries "an
   in-tree tree state machine" and proposes `tui-tree-widget` for
   expand/collapse/selection. The current explorer is a flat list with one
   collapsible header (the `Other` zone, ADR 0074), not a tree. There is no
   per-node expansion state left (`explorer.rs:381` says the per-group
   expansion set was replaced by `other_expanded`). A tree widget would add
   nothing here.

## 3. Control-by-control inventory

### 3.1 Screen shell and overlay layering

- Files: `ui.rs:83-105,312-335,341-355`; `runtime.rs:843-861`; `modal.rs`.
- Purpose: split the frame into header/body/status, draw the active modal and
  toast over it.
- Surface: `pub fn draw(app: &mut App, frame: &mut Frame)`; layout is
  `vertical![==1, >=3, ==1]` (`ui.rs:85`), body split in `draw_body`.
  Each overlay has `draw_<name>(app, frame, area)` that early-returns unless
  its modal is on top and then `frame.render_widget(Widget::new(state, theme), area)`
  (`ui.rs:107-274`: 10 near-identical 7-line wrappers, plus 4 that add context
  or a builder: search, rename, controls, messages).
- Variants: three screen modes (viewer, pane viewer, normal). Overlay draw
  helpers differ only in widget type and extra context (`draw_search_overlay`
  builds items and outcome closures from `App`, `ui.rs:225-254`;
  `draw_messages_overlay` passes `current_epoch()`; `draw_controls_overlay`
  passes `controls_context()`).
- Leaks: every overlay widget is `crate::tui::widgets::*` (b). The glob
  imports make `Msg`, `View`, `SessionsGrouping`, `NodeDetail`, `SectionKind` visible to
  all chrome (b). Clock: `current_unix_epoch_for_render` is a cfg(test) switch
  (`ui.rs:276-306`), (a) injectable.
- Minimum-size handling: none. `grep "too small"` finds nothing; below about 5
  rows the body's `>=3` constraint wins and the footer may not render.
- Verdict: split generic core + adapter. Generic: a `Screen` with
  header/footer rows, body area, "top modal only" overlay layer, toast layer,
  and the measure-then-message viewport callback. Conspectus: the 14 wrappers
  collapse to one `modal.render(frame, area)` dispatch if `Modal` implements a
  render method alongside `Overlay::handle` (the layout test in
  `consistency_tests/layout.rs:34` already enumerates every variant).

### 3.2 Body split (SplitPane)

- `ui.rs:312-321`: `threshold = app.config().narrow_layout_threshold`
  (default 100, `config.rs:41`); `body.width < threshold` gives
  `vertical![==50%, ==50%]`, else `horizontal![==50%, ==50%]`.
- Variants: one caller. The same arithmetic is copied in
  `snapshot.rs:343-372 pane_rect` (Constraint::Length(1)/Min(3)/Length(1)
  + Percentage(50)) so the dev snapshot can slice panes; any layout change
  must be made twice.
- Also: `consistency_tests/fixtures.rs:233 left_pane_lines` slices the left
  half as `width / 2` characters, a third copy of the "left half" assumption.
- Fixed 50/50; no draggable or focus-weighted ratio (the mockup's idea of
  widening the focused pane, CSP-319/CSP-190, is unimplemented).
- Verdict: extract after refactor as `SplitPane::new(threshold, ratio)
  .areas(area) -> [Rect; 2]` with one implementation shared by `draw`, the
  snapshot tool, and the test helpers.

### 3.3 Pane frame and focus marker (PaneFrame)

- `left_panel.rs:8-12`, `right_panel.rs:6-10`, `ui.rs:323-335`.
- Surface: `Block::default().borders(ALL).title(Line)`; the title `Line` is
  built by `left_panel_title(app)` / `right_panel_title(app, width)`, each
  starting with `" "` + `focus_marker_span(app, Focus::X)`.
- `focus_marker_span` (`ui.rs:331`): `► ` (U+25BA) in `theme.panel_focus_accent`
  when focused, `  ` otherwise; 2 callers. `focus_marker_width`
  (`ui.rs:323`) counts `chars()`, not display width.
- Leaks: `Focus` enum (a); `► ` is hard-coded even though
  `icons::marks::POINTER` is that glyph (b, see 5.2).
- Verdict: extract after refactor as `PaneFrame { title: Line, focused: bool }`
  with the marker built in. Decide whether focus should also tint the border
  (open question 1); the modal frame already does it unconditionally.

### 3.4 Tab strip (TabBar / ViewSwitcher)

- `left_panel.rs:222-244`. Iterates `controls::VIEW_OPTIONS` (6 views today:
  sessions, mux, prs, pinned, history, search; `controls.rs:72`), label from
  `header.rs:429 view_label`. Active = `panel_focus_accent` + BOLD, others
  `secondary_text`, separator ` · ` in `secondary_text`.
- Variants: one caller. The Controls overlay has its own view list
  (`ControlsRow::View(usize)`, `controls.rs:119`) and `keymap.rs:140 cycle_view`
  and `keybindings.rs` digits 1-6 are a third and fourth encoding of the same
  list (another slice covers them).
- Narrow width: no elision. By arithmetic the strip is
  `1 + 2 + 33 + 5*3 + 1 = 52` cells; the block title area is `panel width - 2`.
  With the side-by-side split at 100-107 terminal columns the panel is 50-53
  wide, so the last tabs are clipped by `Block` (suspected; the only tab-strip
  test renders at 160 columns, `ui_tests.rs:456`, and its comment still says
  "only Sessions and Mux are surfaced"). Contrast the right title, which does
  elide (3.7).
- Verdict: extract after refactor: `TabBar::new(tabs, active).line(theme)`
  with a drop/elide rule shared with the breadcrumb.

### 3.5 Header line (SummaryLine / ChipBar)

- `header.rs:5-186`. Output: bold `"{freshness}{N/M} sessions · {M} mux[ · K stopped]"`,
  then optional per-harness badges (`show_harness_chips`, `config.rs:306`),
  then optional `▲ N` ambiguity chip. One `Paragraph`, no truncation.
- Separators: `SECTION_SEPARATOR = "  ·  "`, `CHIP_SEPARATOR = "  "`
  (`header.rs:87-89`). Those are the only shared constants; the status bar uses
  different separators (` · `, 3 cells) and the tab strip another ` · `.
- Surface for reuse: none; `HeaderCounts::from_app` walks `app.tree().rows`
  (`header.rs:64`), `snapshot_counts`/`gone_mux_count` read the graph snapshot
  (`header.rs:443-478`). Every number is domain.
- Quirk (code reading, unverified by render): `visible_agent_session_count`
  special-cases only `View::Mux` (`header.rs:154-171`); in PRs, Pinned, History
  and Search the visible count is `rows.filter(AgentSession).count()`, so the
  header likely reads `0/N sessions` there. The audit's cross-view
  `header_primary_kind_word(view)` was never built.
- Chips used: `harness_badge` (fixed-width REVERSED+BOLD pill,
  `widgets/badge.rs:30`) and a bold `▲` + count. Four other "chip" meanings
  exist elsewhere, see 8.6.
- Verdict: stays in Conspectus. The generic piece is "a line of segments
  with a separator policy", which is the same primitive as the status bar
  (3.6).

### 3.6 Status bar (StatusLine + KeyHintBar)

- `header.rs:188-227` (`draw_status_bar`), `482-582` (segment assembly and
  fitting), `232-312` (chips), `597-661` (`default_action_status_hint`).
- Two modes:
  - **Message mode** (`app.status_message()` is `Some`, `header.rs:190-197`):
    `[unseen chip][in-flight chips][message in theme.warning]`; hints, scope tag,
    stale mark, and provider chips are all suppressed.
  - **Hint mode**: `[unseen chip][in-flight chips][scope tag "[left]"/"[right]" in placeholder]
    ["stale  " warning bold][fitted segments][provider chips]`.
- Fitting (`fit_status_segments`, `header.rs:517-582`): each piece is
  `(priority, text, settings?)`; pieces are greedily kept in priority order
  while `sum(width) + 3*(n-1) <= room`; kept pieces are re-sorted into original
  order; if anything was dropped, re-fit into `room - 2` and append ` …`.
  Settings pieces (`group:`, `filter:`, `sort:`, `find:`) are drawn in
  `theme.cwd_mark`; key hints in `theme.placeholder`.
- Priorities (1 = kept longest): find 1, Backspace-back 1 (left focus with nav
  history), quit 2, filter 3, focus hint 3 (right focus) or 7 (left), default
  action 4, group 5, sort 6, Tab 8, refresh 9 (`header.rs:318-334,482-512`).
  Ties break by original order. The priorities are inline numbers with no named
  tiers.
- Granularity: a segment is all-or-nothing, but the hand-written hint strings
  bundle several keys (`"j/k cursor · Enter drill/expand · e group · Backspace back"`),
  so a narrow bar drops the whole right-focus key list, not one key.
- Provider chips (`push_provider_chips`, `header.rs:282-312`): `tmux:off`
  (placeholder), `tmux:<reason>` (warning), `forge:off`, `gh:<reason>`
  (warning, strips a leading `gh `). They are appended directly after the
  hints, not right-aligned (the sessions mockup drew them right-aligned).
  Domain: tmux, gh.
- In-flight chips (`header.rs:266-278`): Braille spinner frame
  (`marks::SPINNER`, 120 ms/frame from `Instant::elapsed`) + label, accent
  bold, one per op, leading the bar so long hints cannot push them off.
  Spinner state is wall-clock derived, not tick-driven (no animation state in
  the model).
- Unseen-messages chip (`header.rs:232-241`): `▲ N · ! messages` in
  `theme.error` bold, leading.
- Hint text is hand-written (`header.rs:482-512`) and is not generated from
  `KEYBINDINGS`, `remap_for_focus`, or the help overlay (see 7). The one
  contextual, domain-heavy piece is `default_action_status_hint`
  (`header.rs:597-661`): selected row kind, attach target, viewer target, pin
  status, resume target decide among about 10 strings.
- Tests: 8 `status_bar_*`/`contextual_status_*` tests in `ui_tests.rs`
  (e.g. :2384, :2467, :2656-2731) assert substrings at 120 columns; no test
  exercises the drop order at narrow widths except via those fixtures.
- Verdict: extract after refactor. Generic: `PriorityLine` (segments with
  priority + role, separator, ellipsis marker, width via `unicode-width`),
  `StatusChip` (spinner / severity), and a `KeyHint { keys, verb }` model that
  layer 2 can generate from key tables. Conspectus adapter: the segment
  producers (group/filter/sort/find, default-action hint, provider chips).
  Merge with the modal footer generator in `keybindings/overlay.rs`
  (`footer(rows)` already builds ` · `-joined verb segments from `KeyRow`s).

### 3.7 Right pane title with breadcrumb

- `right_panel.rs:214-275`; chain rendering `explorer.rs:927-1063`.
- Title = `" "` + marker + optional `<kind glyph> ` + bold kind label (from
  `right_panel_kind_label`, `right_panel.rs:277-293`, falls back to `detail`)
  + when the drill stack is non-empty `" ◄ " + chain + " · depth N"`.
- Chain rendering: each hop is `<glyph in kind colour> <tag>`; duplicate tags
  get a `·xxxx` suffix (last 4 of the id); separator ` › `; elision ladder:
  full chain, then `first › … › last`, then `… › last`, then `last` alone
  (`build_breadcrumb_line`, `explorer.rs:1000-1046`). Budget =
  `title width - fixed chrome`, floored at 8 (`right_panel.rs:258-263`).
- Quirk: in the two elided forms `styled_glyph(first, secondary)` ignores its
  `_secondary` argument (`explorer.rs:1061`), so glyphs keep kind colour while
  the full form behaves the same; harmless but the parameter is dead.
- Leaks: `BreadcrumbHop.focused: NodeId` is used only to derive the glyph/colour via
  `NodeKind::from(&hop.focused)` and `node_kind_style` and the last-4 suffix
  (`explorer.rs:951-993`); `ForgePr` colour is hard-coded to `theme.pr_open`
  because no state is available (`explorer.rs:967`, duplicated in
  `right_panel.rs:225-229` and `kind_chip_span` :490-501; so a closed or
  merged PR neighbor shows a green glyph in titles, chips, and breadcrumbs).
- Verdict: split generic core + adapter. Generic `Breadcrumb { hops:
  Vec<Hop { glyph: Span, tag: String, id_suffix: String }> }.fit(available)`;
  adapter converts `BreadcrumbHop` + theme to `Hop`.

### 3.8 Scroll plumbing (ScrollGutter, follow-cursor)

- `left_panel.rs:152-216`: `scrollbar_layout(area, content_len)` returns
  `(content, Option<gutter>)`: fade-on-fit (no gutter when content fits or
  `area.width < 2`), otherwise a 1-column gutter on the right.
  `render_vertical_scrollbar` normalises position (ADR 0076: pass
  `content - viewport + 1` so the thumb reaches the bottom), insets the
  track 1 row top and bottom (`Margin{vertical:1}`), and removes REVERSED so
  the selected-row highlight does not bleed into the bar.
- Callers: left panel (:136-140), explorer zone (`right_panel.rs:144-154`),
  Preview, two modes (`preview_pane.rs:59-84`), plus the help overlay
  (`widgets/help.rs:273-287`) which re-implements the same `ScrollbarState` /
  `Scrollbar::new(VerticalRight)` / inset-1 / remove-REVERSED recipe inline
  but with `max_scroll + 1` as content length: 2 implementations of the
  position normalisation.
- Follow-cursor: `App::adjust_left_scroll` (`app.rs:1280-1294`, line index)
  and `adjust_explorer_scroll` (`app.rs:1320-1345`, first/last row of a
  wrapped cursor line) are the same algorithm (scroll up if first < offset;
  scroll down if last >= offset+vh) in two methods on `App` writing to two
  `u16` fields (`left_scroll`, `explorer_scroll`). Preview scroll is a third
  scheme (`preview_scroll` counts up from the bottom for transcripts,
  `app.rs:1526-1538`).
- The wrap-aware row count needed to feed the explorer is computed in
  three places: `wrapped_line_count` (`ui.rs:341`), the inline `per_line_rows`
  closure (`right_panel.rs:86-97`, same maths, different width), and
  `header_field_line_count` (`detail_header.rs:96-121`, an "approximation" that
  models Paragraph word-wrap by a +1 case) plus the banner height estimate
  (`left_panel.rs:18-22`).
- Verdict: extract after refactor: `ScrollGutter::layout` + `ScrollGutter::render`,
  `follow_offset(offset, first, last, viewport) -> offset`, and one
  `wrapped_rows(lines, width)`. The help overlay should use the same gutter.

### 3.9 Chip divider (SectionRule / ZoneHeader)

- `detail_header.rs:281-320`. `chip_divider_line(label, suffix, width, theme,
  anchor)`: `leading ─ rule` (dim `theme.divider`) + chip `" label "` (accent fg
  + `theme.badge` modifier, i.e. REVERSED|BOLD) + optional suffix `" s "` in
  `secondary_text` + 2-cell trailing rule. In both anchors the chip sits at
  the right end; `ChipAnchor::Left` puts the suffix after the chip,
  `ChipAnchor::Right` puts it before. The doc comment (`detail_header.rs:269-274`)
  describes `Left` as "chip near the start of the line", which is not what the
  code does, and the first paragraph of `chip_divider_line`'s doc is stranded
  above the enum.
- Callers (3): `section_divider_line` (legacy; `Left`, no suffix),
  explorer "Related" (`right_panel.rs:353`; `Right`, summary suffix
  `"N validated · K other · M ▲ · L —"`), Preview divider
  (`preview_pane.rs:469`; `Left`, no suffix, unused `_app` parameter).
  So the `Left`+suffix combination is unused and the live variants are
  "label only" and "summary + label".
- Narrow: `saturating_sub`; if the width is below chip+suffix+2 the leading
  rule vanishes and the line overflows (Paragraph wrap pushes it to a second
  line; the code compensates by rendering at `inner.width - 1`,
  `right_panel.rs:40-48`).
- A fourth divider recipe exists in the failure banner:
  `"─".repeat(40)` with `add_modifier(theme.divider)` (`preview_pane.rs:150-157`),
  fixed 40 cells, no chip.
- Verdict: extract as-is after renaming anchors to `SuffixSide` (or
  `summary_before_label: bool`) and fixing the docs; the banner rule should use it.

### 3.10 Node-field row (PropertyGrid row)

- `right_panel.rs:440-476 render_node_field_line(field, highlight, theme)`;
  model `explorer.rs:137-157 CoreField`.
- Layout: `"  "` + label (`{:<14}` bold) + optional kind-glyph chip + `" "` +
  value (style: default, or `theme.placeholder` DIM if `placeholder`) +
  `"  (truncated · o)"` if `long_value` + `" "` + annotation in `theme.warning`.
- `CoreField` fields: `label: &'static str`, `value: String`, `placeholder`,
  `annotation: Option<&'static str>`, `long_value: Option<String>`,
  `kind_chip: Option<NodeKind>`, `preview: Option<FieldValue>`. `FieldValue`
  (10 variants: Path, Ref, Commit, Process, Text, List, Command, Timestamp,
  Glossary, Node) is the type that picks the Preview behaviour (ADR 0112); it
  is inherently domain.
- Truncation is baked into the data: builders call
  `explorer::truncate(value, 48)` (char-count, not display width,
  `explorer.rs:1856`) and attach `with_long(full)` at 7 sites
  (`explorer.rs:1408-1817`); the renderer never truncates, the Paragraph wraps.
- Highlight extent: only the **value span** is REVERSED
  (`right_panel.rs:445-452,463`); label, kind chip, hint, and annotation are
  not. Related rows highlight verb+chip+label (3.11); the left panel
  highlights the whole line via `Line::style` (`left_panel.rs:1085-1099`).
- Label width 14 here vs 10 in the legacy detail header (`detail_header.rs:327`,
  `LABEL_WIDTH` :101). The Preview re-implements the 14-cell label column four
  more times: two hand-padded literals (`"  neighbor    "` :771,
  `"  unresolved  "` :825) and two `{:<14}` formats (:792, :811, :835).
- Also reused unhighlighted inside the Preview for neighbor fields
  (`right_panel.rs:779`).
- Leaks: `NodeKind` chip (a: take a `Span`), `FieldValue` (c), the hint text
  `(truncated · o)` names the `o` key (b: key-hint should come from the key
  table). Colour policy: per-label colours (`link_id` for id/mux, `cwd_mark` for
  cwd, bold title) exist only in the legacy renderer (`field_value_style`,
  `detail_header.rs:344-358`); the live explorer value is uncoloured, so
  ADR 0033's colourisation is effectively dead in the shipped path.
- Verdict: extract after refactor: `PropertyRow { label, value, dim, hint,
  annotation, glyph: Option<Span> }` + `property_line(row, label_width,
  highlight)`; keep `CoreField` and `FieldValue` in Conspectus.

### 3.11 Related rows (LinkRow family)

- `right_panel.rs:506-702`; models `explorer.rs:408-541`.
- Shape: `[prefix][indent]verb(22 cols) <kind glyph> <label>[  · prov · conf · state]`.
  `VERB_COLUMN_WIDTH = 22` (`right_panel.rs:506`). The label is middle-truncated
  to the remaining width (`truncate_to_width_middle`, budget =
  `width - fixed - meta`, and edge meta is dropped when the label budget would
  fall under `MIN_RELATED_LABEL_WIDTH = 8`, `right_panel.rs:584-637`).
- Variants (4 renderers sharing `render_related_row`):

  | Variant | Fn | Indent | Prefix | Label style |
  | --- | --- | --- | --- | --- |
  | validated (resolver winner) | `render_validated_link_line` :512 | 2 | none | `link_id` + BOLD |
  | other: alt-of | `render_other_link_line` :550 | 6 | none | `edge_alt_of` |
  | other: conflict | same | 4 | `▲ ` in `edge_conflict` BOLD | `edge_conflict` BOLD |
  | other: unresolved | `render_other_unresolved_line` :688 | 6 | none (one plain `format!` string, placeholder DIM, `verb — evidence`) | placeholder |

  The unresolved row does not go through `render_related_row`, so it has no
  kind chip and no truncation (it relies on Paragraph wrap).
- Highlight: REVERSED on the verb, chip, prefix, label, meta (not the plain
  `" "` between chip and label).
- Leaks: `RelationshipGroup`, `RelationshipLink`, `directional_verb`
  (30+ `RelationKind` variants x 2 directions, exhaustive match,
  `explorer.rs:320-373`), `EdgeStateLabel`, `Provenance`/`Confidence`
  `snake_case()` (c).
- Verdict: split generic core + adapter. Generic `LinkRow { indent, prefix:
  Option<Span>, verb, glyph: Span, label, meta: Option<String>, style }` plus
  the truncation and meta-drop policy; the four variants become `RowTone`
  presets (primary, quiet, alert, inert).

### 3.12 Other header (collapsible section header row)

- `right_panel.rs:658-683`: `"  ▼|▶ Other  (N · K ▲ · L —)"` in
  `secondary_text` BOLD, REVERSED when highlighted. Toggled by Enter or `e`
  while selected (`explorer_nav.rs:138-163`).
- Hard-codes `▼`/`▶` although `icons::marks::EXPANDED/COLLAPSED` exist and are
  used by the left panel (`left_panel.rs:1701-1703`). Different indent and no
  accent on the glyph, whereas the left panel colours the disclosure glyph with
  `theme.disclosure`.
- Verdict: extract after refactor as `DisclosureRow` shared with the left
  panel's group rows.

### 3.13 Explorer zone composition and right-pane layout

- `right_panel.rs:5-172` (`draw_right_panel`) and `310-438`
  (`render_explorer_lines`).
- Composition order: optional hit block (`selected_hit_lines`, Search view hits,
  :879-921, spliced at index 0 and shifts `cursor_line`), Node fields, `Related`
  chip rule (only if any group exists), Related rows, Other header and children.
  Returns `ExplorerRender { lines, cursor_line }`, mapping the selectable
  cursor to a rendered line (decoration lines such as the rule are not
  selectable).
- Layout algorithm (all inline in `draw_right_panel`):
  1. content width = `inner.width - 1` (reserve the scrollbar gutter even when
     it will not appear, so a divider chip cannot wrap).
  2. `wrapped_rows` = sum of `ceil(width/pane_width)` per line.
  3. `preview_floor = min(6, inner.height - 4)`;
     `max_header = max(inner.height - (preview_floor + 1), 3)`;
     `header_height = clamp(wrapped_rows + 1, 3, max_header)` (the `+1` is a
     safety margin for word-wrap under-count).
  4. `vertical![==header_height, ==1, >=0]` = explorer zone, Preview rule,
     Preview zone.
  5. dispatch `ExplorerViewportChanged{cursor_first_row, cursor_last_row,
     viewport_height}`; read back `explorer_scroll`; render `Paragraph` with
     `Wrap{trim:false}` + `.scroll((scroll,0))`; scrollbar gutter if overflow.
  6. `app.prepare_preview(preview_width - 1)` then `draw_explorer_preview`.
- Preview composition for a Related row (`draw_explorer_preview`,
  `right_panel.rs:731-866`): neighbor + neighbor fields (or the neighbor's
  source view) + `edge` line (warning colour) + `evidence` lines; falls back to
  `draw_detail_preview` (the selected left row's preview) for Node-field and
  Other-header rows.
- The Preview's label widths are literal strings in `format!`
  (`"  neighbor    {neighbor_label}"`, `"  {label:<14}"`): a second, hand-aligned
  copy of the 14-column property layout.
- Verdict: split. Generic `StackedMasterDetail { list_lines, cursor_line,
  min_detail_rows }` that does steps 1-5 and returns the three areas +
  scrollbar; the line builders stay Conspectus. Step 1-3 are currently
  untestable apart from whole-frame tests
  (`right_pane_preview_keeps_minimum_height_when_related_full`,
  `ui_tests.rs:3225`).
- Dead branch: lines 174-208 are the legacy fallback when `explorer()` is
  `None` but `detail()` is `Some`. `recompute_detail` sets both from the same
  `find_node` lookup (`explorer_nav.rs:42-48`, `explorer.rs:64`,
  `detail.rs:49`), `explorer_drill_into`/`explorer_back` set both, so the
  fallback is unreachable in production (see 3.16).

### 3.14 Preview frame (DetailPane body)

- `preview_pane.rs`. (Preview content sourcing, transcripts, and pane captures
  belong to another slice; the frame mechanics are cited here.)
- `PreviewView { text: Text, window: Option<PreviewWindow{total, top}> }`
  (:8-21): `None` scrolls the text by `app.preview_scroll()`; `Some` means the
  body is already cut to the zone (transcript) and only the scrollbar uses the
  window. `PreviewView::below(header)` prepends a banner or Related-row
  evidence (:33).
- Failure banner (:113-159): glyph+summary (level style, BOLD), optional
  `exit status N`, up to 6 output lines, `! for the full output` hint, and a
  40-cell rule. Domain: `LogEntry` from the message log (b).
- Empty states are plain strings returned from the builders
  (`"loading…"`, `"preview disabled (--no-live-preview)"`, `"no preview for
  this node"`, ...), with no shared "empty state" component. The same applies
  to the left panel (`empty_left_panel_text`, `left_panel.rs:317-418`, about 12
  messages, many naming keys such as `` `F` ``/`` `s` ``) and the right pane
  (`empty_right_panel_text`, `right_panel.rs:868`: `Loading…` /
  `Select a row to view its detail.`). Capitalisation and ellipsis differ:
  `Loading…` vs `Loading discovery…` vs `loading…`.
- Unused parameters: `draw_detail_preview(app, _detail, ...)` and
  `preview_divider_line(_app, ...)`.
- Verdict: split generic core + adapter: generic `ScrollPane` (text or window,
  gutter, banner slot); Conspectus owns sourcing.

### 3.15 Toast

- `widgets/toast.rs`; drawn `ui.rs:179-188`; posted `overlays.rs:922`.
- Shim over `ratatui_comfy_toaster::ToastEngine<()>`: `engine()`,
  `builder_for(label)` (Success type, `ToastBorderMode::Full`, transparent bg,
  `Center` + `offset(0, i16::MAX)` to pin to the bottom edge, 1500 ms),
  `engine_dismiss_all` (replace semantics), `ToastEngineHolder` newtype for
  `Debug`. Runtime calls `prepare_toast_for_render(area)` (set_area + tick) before
  `terminal.draw` (`runtime.rs:833-838`).
- Only one `ToastType` is ever used (Success) for every toast, including
  notices and refusals; the border colour comes from the upstream's hard-coded
  `ToastType -> Color`, not the theme (documented caveat, `toast.rs:13-20`), so
  `theme.success` overrides do not apply.
- The toast overlays the bottom rows of the screen (status bar and bottom of
  the panels); there is no reserved toast area.
- Tests: 5 inline unit tests incl. replacement and expiry (`toast.rs:104-175`,
  one uses `sleep`).
- Verdict: extract as shim; add severity (success/info/warn/error) and theme
  colour pass-through; keep replace-on-post semantics as a documented option.

### 3.16 Legacy detail model and renderer (`detail.rs`, `detail_header.rs`)

- `detail.rs` (1409 LOC): `NodeDetail { kind, title_line, short_id, full_id,
  header_fields: Vec<HeaderField>, outgoing_links, incoming_links, resolved,
  diagnostics }`, `HeaderField { label, value, placeholder, annotation,
  target, expanded_kind, expanded_fields }`, `SectionKind` (Session, Process,
  Mux, Pr, Lineage, AmbiguousMux), `group_fields_into_sections`,
  per-kind builders. `detail_header.rs` renders it: sectioned fields with
  10-column labels, recursive inline expansion with indent, wrap-aware height.
- What is still live (checked with grep outside `*_tests.rs`):
  - `NodeDetail.kind` as the "is something selected" gate and the title
    glyph/label (`right_panel.rs:222,278`, `app.rs:135`).
  - `detail::ambiguous_muxes_for_group` (`actions.rs:271`).
  - `chip_divider_line` / `ChipAnchor` live in `detail_header.rs` but are used
    by the explorer and Preview (3.9).
- What is dead in the shipped path: `NodeDetail.header_fields/sections`,
  `outgoing_links`, `incoming_links`, `resolved`, `diagnostics` (no non-test
  reader), `detail_links_expanded` (read only by the unreachable legacy
  branch, `right_panel.rs:178-207`), `Msg::ToggleLinkedDetails` on the left
  pane (it flips `detail_links_expanded` and shows "detail: no linked entities to
  expand", with no visible effect; `tree.rs:253-266`; on right focus `e` is
  remapped to `ExplorerToggleGroup`, `keymap.rs:173`), `mux_runtime_rows`,
  `field_value_style`, `pr_value_style` (the only chrome user of
  `pr_merged/closed/draft`), `format_elapsed`.
- Duplicated names across the two models: `LinkStateLabel` (`detail.rs:279`
  and `explorer.rs:527`), `title_line` (`detail.rs:319` returns e.g.
  `forge_pr:o/r#n`; `explorer.rs:1211` returns `o/r#n`), `plain`/`placeholder`
  constructors (`detail.rs:919-943` vs `CoreField::plain/placeholder`,
  `explorer.rs:196-218`), `preferred_link` (`detail.rs:1099`, cited from
  `explorer.rs:430`).
- Tests tied to it: `detail_tests.rs` (25 tests), about 8 in `ui_tests.rs`
  (`header_field_line_count_*`, `expanded_session_under_mux_*`, ...).
- Verdict: delete (after retargeting `kind` and
  `ambiguous_muxes_for_group`), do not extract. It is 1409 + 1180 + 376 LOC of
  Conspectus-specific code with no generic remainder except `chip_divider_line`.

### 3.17 Copy-on-Enter, `o` full value, `i` copy id

- Enter on a Node field copies the value via OSC 52 and posts a toast
  `copied: <label>`: `runtime.rs:2278 explorer_enter` calls
  `app.explorer_copy_target()` (`app.rs:703-724`) then `Msg::ExplorerActivate`
  as fallback. `clipboard.rs` is a hand-rolled base64+OSC 52 writer (ADR 0056).
  `explorer_enter(app, write_clipboard: impl FnOnce(&str))` takes the clipboard
  as a closure so the snapshot driver reuses the branch (`snapshot.rs:315`).
- `o` opens `ValueModalState::new(label, value)` for the cursor row's
  `long_value` (`app.rs:748-783`); modal keys come from `VALUE_MODAL_KEYS`
  (esc/q/o close, j/k scroll, PgUp/PgDn 8 rows, g/G, y copy), footer is
  generated by `keybindings::overlay::footer`.
- `i` copies the full id of the selected agent/mux session (`runtime.rs:2289`,
  `app.rs:733-741`): a second copy gesture with its own toast label.
- Suspected latent bug: `explorer_copy_target` and `open_value_modal_for_cursor`
  index `state.view.core_fields.get(index)` (`app.rs:708,762`) but
  `ExplorerRow::NodeField.index` indexes `view.fields(full_detail_expanded)`
  (`explorer.rs:688`). With the `F` expanded view on, rows past the core
  count return `None`, so Enter does not copy and `o` reports "row has no
  truncated value" for exactly the extra fields (full id, long commands). The
  row already carries `long_value` and `label`; only `value` is missing.
  No test covers copy or `o` with `full_detail_expanded = true`
  (`app_tests.rs:1380-1420,1732-1760`). Not verified by running.
- Enter semantics differ per row kind: Node field = copy; validated/other link
  = drill; Other header = toggle; unresolved = status hint only
  (`explorer_nav.rs:198-223`).
- Verdict: `PropertyRow` + `copy`/`inspect` outcomes belong in the framework
  (`RowAction::{Copy(String), Inspect(label, text), Drill, Toggle, None}`);
  the OSC 52 writer is already dependency-free and can move as-is.

### 3.18 Text helpers (`ui/text.rs`, 210 LOC) and cursor arithmetic

- `truncate_to_width` (an alias of `truncate_to_width_strict`),
  `truncate_to_width_middle`, `truncate_to_width_no_marker`, `pad_to_width`,
  `fit_spans_to_width`, plus domain helpers (`compact_path_label`,
  `compact_path_secondary`, `compact_mux_label`, `compact_mux_native_id`).
  Zero `crate::` references in the file; the domain helpers are the exception to
  its "text" role.
- Other truncation implementations in the TUI (not in this slice, listed for
  the dedup phase): `explorer::truncate` (char count), `widgets/form.rs:573
  truncate_chars`, `widgets/popup_frame.rs:110 ellipsize` (same behaviour as
  `truncate_to_width_strict` except width 0/1), `preview_wrap.rs:209
  truncate` (cells), `header.rs:340-345` and `left_panel.rs:271-275` (inline
  `take(N) + '…'` for the find chip and the search header query). 8
  implementations of "cut with an ellipsis".
- `cursor.rs` (52 LOC): `wrap_step`, `clamp_step`, with unit tests; used by 9
  files (keymap, tree, explorer_nav, controls, picker, mux_launch,
  launch_spec_form, pin_edit, search). Clean, domain-free.
- Verdict: `text` module and `cursor` extract as-is after merging the 8
  truncators into one `ellipsize(text, width, Where::{End, Middle})`.

## 4. The explorer as a candidate "master-detail / property-grid explorer"

### 4.1 What the state machine is

State (`app.rs:436-455`):

| Field | Meaning | Generic? |
| --- | --- | --- |
| `view: NodeView` | the focused node's content (fields + relationship groups) | domain payload |
| `cursor: usize` | index into the flat selectable-row list | generic |
| `other_expanded: bool` | whether one collapsible zone is open | generic (one named flag) |
| `breadcrumb: Vec<BreadcrumbHop>` | drill history | generic stack, domain frame |
| `full_detail_expanded: bool` | swap the field zone between core and all fields | generic (view mode flag) |

App-level companions: `explorer_scroll`, `explorer_back_armed`, `focus`,
`edge_meta_visible`, `status_message`, `selection`, `detail` (7 fields on `App`
that the methods in `explorer_nav.rs` reach through `impl App`; the state
machine is not separable without moving them).

Zones (`NodeView::flat_rows`, `explorer.rs:686-738`): fields, then validated
link rows, then (only if any non-winner exists) a selectable `OtherHeader`
and, when expanded, its alt/conflict links and unresolved stubs. Selection
spans all zones with a single flat index (`ExplorerRow` enum, 5 variants,
`explorer.rs:587-616`). Decoration lines (the `Related` rule) are not rows.

Stable identity (`ExplorerRowKey`, :554): `NodeField{label}`,
`ValidatedLink(NeighborRowKey{direction, relation, neighbor})`, `OtherHeader`,
`OtherLink(..)`, `OtherUnresolved{link_id}`, plus a reserved `Title`. Used by
`reseat_cursor` (`app.rs:492-517`): after any rebuild (refresh, toggle,
expand/collapse, back) find the row with the same key; if missing, fall back
to the first "actionable" row (validated link, Other header, or other link),
never the field zone. Keys are by neighbor, not link id (ADR 0107), so a
representative-link change keeps the cursor.

Transitions (`explorer_nav.rs`):

| Msg | Effect | Generic skeleton | Domain/app dependency |
| --- | --- | --- | --- |
| `ExplorerNavDown/Up` | `clamp_step` (single step; PageUp/PageDown are remapped to one step too, `keymap.rs:169-170`) | yes | none |
| `ExplorerHome/End` | cursor 0 / last | yes | none |
| `ExplorerToggleGroup` | flip `other_expanded` when on the header, else status hint | yes | status text |
| `ExplorerToggleFullDetail` | flip, reseat cursor; status hint when no extras | yes | status text |
| `ExplorerActivate` | match row: toggle / drill / hint / none | yes (row "role") | `drill_target`, status text |
| drill | push `BreadcrumbHop`, build new `ExplorerState`, reset preview scroll, mirror left selection | stack push | `build_node_view`, `build_node_detail`, `mirror_left_pane_to` (touches `tree`, `expanded`, `selection`) |
| back | pop hop, rebuild view for `hop.focused`, restore flags + cursor key + left selection | stack pop | same + `RowId` |
| back on empty stack | right focus: arm, status hint; second press moves focus left; left focus: hint only | ArmedConfirm | `Focus`, status text |

Armed back: `explorer_back_armed` is set by the first empty-stack Backspace and
cleared by any reduce call except `ExplorerBack`, `LeftViewportChanged`, and
`ExplorerViewportChanged` (`app.rs:1409-1416`; a past regression where the
per-frame viewport msgs disarmed it is recorded there and has a test,
`app_tests.rs:1271`). "Back" is overloaded: the same Backspace key also pops
`nav_back` (view-level reveal history) when the left pane has focus and
`can_nav_back()` (`app.rs:1510-1515`), two stacks behind one gesture, and the
status hint adds `Backspace back` only in that left/nav-history case
(`header.rs:508`).

Left-pane sync (mirror): after a drill, if the neighbor has a row in the
current tree, expand its ancestors and move the left selection to it; on back,
restore the left selection saved in the hop (`explorer_nav.rs:279-322,383-391`).
The `left_pane_sync` modes in the mockup (`follow`, `none`) do not exist.

### 4.2 Entanglement with domain types (quantified)

- `explorer.rs` (2156 LOC): about 1040 lines (48%, 1113-2152) are domain
  builders (per-kind `*_core`/`*_extras`, `build_explorer`, `finalize_group`,
  `cwd_owner_kind`, `short_node_label`) importing 22 `crate::model` types
  (`explorer.rs:34-39`) + `output::table::node_short_id` + `icons::NodeKind`
  + `rows::shorten_home` + `preview::split_command_line`. The relationship
  catalog `directional_verb` (54 lines) is an exhaustive match over
  `RelationKind`. Roughly 280 lines (13%) are domain-free in shape: the
  breadcrumb renderer (`:914-1063`, 150 lines) and the row/zone/key types
  (`:543-672`, 130 lines).
- `explorer_nav.rs` (393 LOC): 60 of 326 code lines contain a domain token
  (`NodeId`, `RowId`, `snapshot`, `handle`, `build_node_*`, `home`, `self.tree`,
  `self.selection`, `self.config`). The domain touch points are 6 call sites
  (`build_node_view` x3, `build_node_detail` x3, `mirror_left_pane_to`,
  `home_for_config`), the remainder is control flow over `cursor`, flags, and a
  stack.
- `ExplorerState` (`app.rs:436-518`, 83 lines): generic except the `NodeView`
  type and the fallback predicate in `reseat_cursor`.
- Tests: about 24 state-machine tests in `app_tests.rs`
  (`explorer_*`, :904-1760) each build a real `GraphSnapshot` through
  `app_for_explorer()` (`app_tests.rs:880`); there is no standalone state test
  because the state is not constructible without a `NodeView`.

Assessment: the interaction model (flat cursor over zoned rows with stable
keys, section toggle, drill/back stack with saved context, armed confirmation
to leave the pane, post-layout viewport feedback, copy/inspect/drill/toggle
row actions) is a coherent, reusable "ZonedExplorer". About 20-25% of the code
by volume would move to the framework (cursor/zone/stack/armed logic and
breadcrumb/rows rendering); the 50%+ that is graph-model projection and
`directional_verb` stays. The seam is clean if the explorer is parameterised
by a trait:

```rust
trait ExplorerSource {
    type Id: Clone + Eq;            // NodeId
    type Key: Clone + Eq;           // ExplorerRowKey
    type Frame: Clone;              // extra context saved per hop (RowId)
    fn rows(&self, expanded: &Expanded) -> Vec<Row<Self::Key>>; // zones
    fn load(&self, id: &Self::Id) -> Option<Self::Content>;     // build_node_view
    fn drill_target(&self, row: &Row<Self::Key>) -> Option<Self::Id>;
}
```

The remaining coupling to design around: (1) the side effects on `App`
(mirror sync, status text, focus shift) must become returned effects or a
callback trait, (2) the fallback-cursor policy is a predicate on row role, (3)
`CoreField.preview: Option<FieldValue>` ties each row to the Preview contents
(ADR 0112), so the framework's detail pane should be a slot rendered by the
caller from the selected row, not a built-in.

Spike prior art that does not apply: `tui-tree-widget` (CSP-435, a tree) and
`rat-widget` (CSP-433, no-go for mandatory focus/event/scroll trifecta).

## 5. Theme tokens and glyph vocabulary used by the chrome

### 5.1 Theme fields

Counts of `theme.<field>` references in the chrome files (`ui.rs`, `header.rs`,
`right_panel.rs`, `detail_header.rs`, `preview_pane.rs`, `explorer.rs`), from
grep:

| Field | Refs | Used for | Generic token or domain? |
| --- | --- | --- | --- |
| `placeholder` (Modifier DIM) | 16 | dim hints, scope tag, empty text, unresolved rows, `?` chip | generic ("muted") |
| `panel_focus_accent` (Color) | 4 here (27 across the TUI) | focus marker, active tab, chip labels, in-flight chips, search header; modal borders elsewhere | generic "accent" with a misleading name |
| `secondary_text` (Color) | 6 | breadcrumb, tab separators, chip suffix, Other header, `edge_alt_of` default | generic ("subtle") |
| `warning` | 6 | status message, `stale`, annotation, `edge` value, provider warn chips, ambiguity chip | generic |
| `error` | 1 | unseen-messages chip | generic |
| `divider` (Modifier) | 3 | rules | generic |
| `badge` (Modifier REVERSED\|BOLD) | 1 | chip labels in rules | generic |
| `cwd_mark` | 2 | status-bar "settings" chips (view state) | domain name reused for a generic role |
| `link_id` | 5 | validated link label; legacy id/mux values | domain name reused |
| `edge_alt_of`, `edge_conflict` | 2 + 3 | Other-zone row tones | domain (graph edge semantics) |
| `pr_open` | 4 | ForgePr kind-glyph fallback (explorer, title) | domain |
| `pr_merged/closed/draft` | 1 each | legacy `pr_value_style` only | domain |
| `match_style()` / `match_highlight` | 2 | search hit text, preview search | generic ("match") |
| `selection_active/inactive` | 0 in chrome (left panel uses both) | left row highlight | generic; explorer ignores them |

`Theme` itself is not framework-clean: it carries harness colours, recency
buckets, mux state, per-node-kind colours, edge tones, PR states, and
`harness_color()` reaches into `crate::discovery::harness::registered_adapters()`
(`theme.rs:288`). The chrome uses a small subset; the framework `Theme` would
be the generic subset (accent, muted, subtle, warning, error, success, divider,
badge, selection active/inactive, match) with an extension slot for app tones.
Dimming appears in two mechanisms: `placeholder` (DIM modifier, documented as
unreliable on some terminals, `theme.rs:85-91`) for status hints and
`secondary_text` (Color) elsewhere: 16 vs 6 chrome uses.

### 5.2 Glyphs hard-coded in the chrome although `icons::marks` defines them

| Glyph | `marks::` constant | Hard-coded at | Via constant |
| --- | --- | --- | --- |
| `►` | `POINTER` | `ui.rs:333` | none |
| `◄` | `BACK` | `right_panel.rs:255,268` (twice) | none |
| `▼` `▶` | `EXPANDED`, `COLLAPSED` | `right_panel.rs:666` | `left_panel.rs:1701-1703` |
| `▲` | `WARNING` | `header.rs:128,239`; `right_panel.rs:347,564,669` (5) | `left_panel.rs:578` |
| `—` | `NO_VALUE` | `right_panel.rs:350,696`; `explorer.rs:1844,1851` | `left_panel.rs:767` etc. |
| spinner | `SPINNER` | none | `header.rs:246` |

`docs/tui-columns.md` and its test gate only the left panel's glyph list, so a
chrome glyph change is unchecked. Two look-alike codepoints appear in the
chrome: `►` U+25BA (focus marker) and `▶` U+25B6 (collapsed disclosure).
`icons.rs` carries a `VETTED` font-risk list and an override mechanism for node
kinds only (`[tui.theme.icons]`), not for marks.

## 6. Narrow-width degradation matrix

| Part | Behavior below its natural width | Test |
| --- | --- | --- |
| Body | stacks 50/50 vertically when body width < threshold (default 100) | `narrow_terminal_stacks_the_two_panels_vertically` (60 cols) |
| Header line | none; Paragraph clips | `header_fits_at_narrow_width_post_audit` (80 cols; asserts it is short) |
| Left tab strip | none; Block clips the title (by arithmetic the last tab(s) clip at 100-107 cols side by side) | none at narrow width |
| Right title | breadcrumb elides: full, `first › … › last`, `… › last`, `last`; floor 8 cells; kind label and depth fixed | `explorer_tests.rs` breadcrumb tests (width param) |
| Status bar | priority drop + ` …`; scope tag, stale mark, in-flight, provider chips never drop | indirect |
| Node fields | label 14 + value wraps (Paragraph); wrapped rows counted for height | `related_row_*` tests |
| Related rows | label middle-truncated; edge meta dropped under 8-cell label budget; verb column fixed at 22 | `related_row_truncates_long_labels_instead_of_wrapping_them_away` |
| Chip rule | leading rule shrinks to 0, then overflows/wraps | none |
| Explorer vs Preview | Preview keeps >= 6 rows if height allows (`min(6, h-4)`) | `right_pane_preview_keeps_minimum_height_when_related_full` |
| Scrollbar | gutter only when content > viewport and width >= 2 | 6 tests in `ui_tests.rs` (:3281-3576) |
| Whole screen | no minimum-size screen | none |

Because the verb column is a fixed 22 cells and the label gets the remainder,
a right pane under about 35 cells (stacked layout at about 60 columns gives a
58-cell pane, fine; side-by-side at 100 columns gives 48) leaves labels very
short. The mockup acknowledged this (CSP-319 responsive design) and it is still
open.

## 7. Keys and hints for the chrome and explorer

Three hand-maintained descriptions of the right-pane keys, plus the dispatch:

1. Dispatch: `keymap.rs:158-205 remap_for_focus` re-maps *left-pane* actions
   when `Focus::Right`: `NavDown/Up` and `PageDown/Up` (`keymap.rs:169-170`) to
   `ExplorerNavDown/Up` (so Page keys move one row), `ToggleLinkedDetails` (`e`) to
   `ExplorerToggleGroup`, `Home/End` to explorer Home/End, `ExpandRow`/
   `CollapseRow` (`h`/`l`/arrows) to nothing, `DefaultAction` (Enter) to
   `ExplorerEnter`, `ClearFilters` (`F`) to `ExplorerToggleFullDetail`.
   `KeyMode` has only `Global` (`keybindings.rs:51-54`); the table's `mode`
   field is not used for focus. `Backspace` is one global binding whose
   meaning is chosen in the reducer by focus (`app.rs:1510`).
2. Help overlay: a hand-authored section "Detail-pane graph explorer (right
   focus)" (`widgets/help.rs:465-494`) lists `j/k`, `Enter`, `e`, `Backspace`,
   `F`, `E`, `o`. Its `e` text ("multi-link group header") is stale (it
   toggles the Other zone), the table's own `e` help text says "Toggle explorer
   linked-details" (`keybindings.rs:440`), and the status bar says "e group":
   three wordings of one key.
3. Status bar: `"j/k cursor · Enter drill/expand · e group · Backspace back"`
   (`header.rs:489`); omits `o`, `F`, `E`, and the fact that Enter copies a field.
   Left: `"j/k move · h/l fold · Enter default"`.

The modal side is the generated model: `keybindings/overlay.rs` `KeyRow<A>
{Bind{key, action, hint}, Note}` + `resolve` + `footer` + `MODAL_CONVENTION`,
asserted by `consistency_tests/keys.rs`. Three modal-opening functions still
push a typed copy of the footer into the status bar (`runtime.rs:1514`
"worktree: ↑/↓ move · Enter pick · Esc close", `:1587`, `:1789`), so the hint
shows twice (status line and modal footer) with independent text.

Same key, different meaning by focus: `F` (clear filters / expanded node
detail), `Enter` (default action / copy-or-drill-or-toggle), `Backspace`
(nav history / drill history / arm-then-leave-pane), `e` (no-op flag toggle /
Other zone), `h`,`l` (fold / nothing). `Esc` is unbound in the base keymap, so
the only ways out of the right pane are `Tab` or two `Backspace`s.

## 8. Duplications and inconsistencies

Counts and locations; "x N" is the number of copies.

1. **Cut-with-ellipsis helpers x 8** (3.18): `ui/text.rs` x4 (one is an alias),
   `explorer::truncate`, `widgets/form.rs:573`, `popup_frame.rs:110`,
   `preview_wrap.rs:209`, plus inline `take(N)+'…'` at `header.rs:340`,
   `left_panel.rs:271`, `explorer.rs:1856`. Char-count vs display-width
   semantics differ (`explorer::truncate` counts chars).
2. **Wrapped-row counters x 4**: `ui.rs:341 wrapped_line_count`,
   `right_panel.rs:86-97` (inline), `detail_header.rs:96 header_field_line_count`,
   `left_panel.rs:18-22` (banner).
3. **Scrollbar recipes x 2** (`left_panel.rs:191`, `widgets/help.rs:273`) with
   different `content_length` conventions; **follow-cursor offset x 2**
   (`app.rs:1280`, `:1320`); **gutter-aware width** handled three ways
   (`scrollbar_layout` result, `inner.width - 1` pre-reserve in the explorer
   `right_panel.rs:48`, `saturating_sub(1)` for Preview `:164,186`).
4. **Layout split x 3**: `ui.rs:312`, `snapshot.rs:343`,
   `consistency_tests/fixtures.rs:233` ("left half" by `width/2`); plus
   `right_panel.rs:130` and `:185` both build the same
   `vertical![==h, ==1, >=0]` for explorer vs legacy paths.
5. **Highlight extent x 4 styles**: left row = whole line, `selection_active`
   (REVERSED|BOLD) focused, `selection_inactive` (BOLD) unfocused
   (`left_panel.rs:1085-1099`); node field = value span only, hard-coded
   REVERSED, nothing when unfocused (`right_panel.rs:331,445-452`); related
   row = verb+chip+label spans REVERSED (`:609-615`); Other header = whole
   text REVERSED (`:679`); unresolved = whole text REVERSED (`:698`). The
   explorer never reads `theme.selection_active`/`selection_inactive`, and its
   unfocused cursor is invisible (the left pane keeps a BOLD cue). The defect
   noted in `consistency_tests.rs` ("right pane keeps a highlighted row when
   unfocused") was fixed by making the right pane show nothing, rather than
   by giving it the left pane's inactive cue.
6. **Chip meaning x 5**: (a) `chip_divider_line` filled label in a rule,
   (b) `harness_badge`/`command_badge` fixed-width pills (`badge.rs`),
   (c) one-cell `kind_chip_span` glyphs, (d) plain-text segments in the status
   bar and header (`▲ N`, `tmux:off`, `group:...`), (e) in-flight spinner
   chips. They share no type; three different separators (`"  ·  "`, `"  "`,
   `" · "`, plus `" · "` in the tab strip).
7. **Focus indication x 3 channels** (marker, status scope tag, row cue) with
   no border cue; modal frames use accent border always; ADR 0076 text still
   claims a focused-border highlight.
8. **Hard-coded glyphs x 9 sites** that bypass `icons::marks` (5.2), versus 7
   uses of the constants in `left_panel.rs`.
9. **Label column widths**: 14 (explorer, `right_panel.rs:453`), 10 (legacy,
   `detail_header.rs:327`), and four more copies of the 14-cell column in the
   Preview (`right_panel.rs:771,792,811,825,835`: two hand-padded literals,
   three `{:<14}` formats): 3 implementations of "label: value" alignment.
10. **Per-kind colour for `ForgePr` hard-coded to `pr_open`** x 3 sites
    (`right_panel.rs:225`, `:495`, `explorer.rs:967`), which loses PR state
    in titles, chips, and breadcrumbs.
11. **Two detail models**: `NodeDetail`/`HeaderField` vs `NodeView`/`CoreField`
    with same-named `LinkStateLabel`, `title_line`, `plain`, `placeholder`
    (3.16). Only `kind` and `ambiguous_muxes_for_group` of the first are used
    outside tests.
12. **Hint text x 3 sources** for the right-pane keys (help section, status
    bar string, `remap_for_focus` behavior) plus 3 modal footers copied into
    the status bar (7). Stale wording for `e` in 2 of 3.
13. **"Back" x 2 stacks on one key** (`breadcrumb`, `nav_back`) and **copy x 2
    gestures** (Enter on a field, `i` for the id) with different toast labels.
14. **Back-arming state outside the explorer**: `explorer_back_armed` is an
    `App` field cleared by an allow-list of `Msg` variants in `reduce`
    (`app.rs:1409`), so adding a new per-frame message requires remembering to
    add it to that list.
15. **Dead/unused**: unreachable legacy right-pane branch
    (`right_panel.rs:174-208`), `_detail` and `_app` parameters
    (`preview_pane.rs:45,468`), `_secondary` (`explorer.rs:1061`),
    `ChipAnchor::Left`+suffix, `detail_links_expanded`, `Msg::ToggleLinkedDetails`
    on the left pane, `explorer::LinkRef` (`explorer.rs:841`),
    `RelationshipExplorer::{link_count, ambiguous_groups, unresolved_groups}`
    (`:386-406`) and `RelationshipGroup::is_single` (all referenced only by tests
    per grep), the doc comment naming nonexistent `upstream_groups`/
    `downstream_groups` (`:378`), and the `pin_placeholder` theme key (documented
    "no longer drawn"). `ExplorerRowKey::Title` is a live fallback for a row whose
    neighbor vanished (`:630,638`), not dead.
16. **Suspected defects found while reading** (verify before filing):
    copy and `o` on extra fields with `F` expanded (3.17); header count shows
    `0/N sessions` outside Sessions/Mux (3.5); tab strip clipped at 100-107
    columns (3.4); `ChipAnchor` docs contradict behavior (3.9).
17. **`panel_focus_accent` is the de-facto accent**: 27 uses across 16 files,
    most for non-focus roles (modal borders, tab highlight, chip labels,
    command lines).
18. **Docs/test drift**: `ui_tests.rs:459-463` comment says only Sessions and
    Mux are surfaced; `docs/tui-detail-mockup.md` (Upstream/Downstream, `★`,
    `left_pane_sync`), `sessions-header-audit.md` (`⚠`, `header_primary_kind_word`),
    ADR 0076 (focused border), `modal.rs` header ("renders the stack in order").

## 9. Framework verdicts

| Candidate base control | Source | Verdict | Merge with |
| --- | --- | --- | --- |
| `Screen` / `AppFrame` (header row, body, footer row, top-modal layer, toast layer, viewport-feedback hook) | `ui.rs:83-105`, `runtime.rs:843` | split generic core + adapter | modal dispatch (layer 2), snapshot `pane_rect` |
| `SplitPane` | `ui.rs:312` | extract after refactor | `snapshot.rs:343`, test helpers |
| `PaneFrame` + `FocusMarker` | `ui.rs:323-335`, `left_panel.rs:8`, `right_panel.rs:6` | extract after refactor | `popup_frame::themed_popup` (one frame vocabulary) |
| `TabBar` | `left_panel.rs:222` | extract after refactor (add elision) | `Breadcrumb` fit logic |
| `ChipLine` / `PriorityLine` + `StatusChip` | `header.rs:5-312,517-582` | extract after refactor | overlay `footer()`, `harness_badge` (as a `Pill`) |
| `KeyHintBar` (generated from key tables) | `header.rs:482-512` + `keybindings` | extract after refactor (layer 2) | `keybindings::overlay::footer`, help sections |
| `SectionRule` (`chip_divider_line`) | `detail_header.rs:281` | extract as-is (rename anchors) | failure-banner rule |
| `Breadcrumb` | `explorer.rs:914-1063` | split generic core + adapter | `TabBar` elision |
| `PropertyRow` / `PropertyGrid` (+ copy / inspect outcomes) | `right_panel.rs:440`, `explorer.rs:137` | extract after refactor | `value_modal`, legacy `render_header_field`, form field rows |
| `LinkRow` family (validated/quiet/alert/inert) | `right_panel.rs:506-702` | split generic core + adapter | PropertyRow (label column) |
| `DisclosureRow` | `right_panel.rs:658`, `left_panel.rs:1701` | extract after refactor | left group rows |
| `ZonedExplorer` (cursor over zones + stable keys + reseat + drill stack + armed back) | `app.rs:436-518`, `explorer_nav.rs`, `explorer.rs:543-738` | split generic core + adapter | left tree selection logic (`tree.rs`) for key-based reseating |
| `StackedMasterDetail` (content-sized list zone, divider, remainder, min detail rows, gutter) | `right_panel.rs:5-172` | extract after refactor | Preview frame |
| `ScrollPane` / `ScrollGutter` / `follow_offset` / `wrapped_rows` | `left_panel.rs:152-216`, `app.rs:1280-1345`, `ui.rs:341`, `help.rs:273` | extract after refactor | help overlay, left tree |
| `Toast` | `widgets/toast.rs` | extract as shim, add severity + theme colours | `status_message`, Messages log (feedback taxonomy) |
| `EmptyState` / message text | `left_panel.rs:317`, `right_panel.rs:868`, `preview_pane.rs` | extract after refactor (new, tiny) | n/a |
| `text` (`ellipsize`, `pad_to_width`, `fit_spans_to_width`) | `ui/text.rs`, 7 other truncators | extract as-is after merge | n/a |
| `cursor` (`wrap_step`, `clamp_step`) | `cursor.rs` | extract as-is | n/a |
| Header composition, status segment producers, `default_action_status_hint`, provider chips, `right_panel_kind_label`, hit block, builders, `directional_verb`, `FieldValue`, failure banner | various | stays in Conspectus | n/a |
| `detail.rs`, legacy `detail_header.rs` | | delete | n/a |

### API sketches (items supplied by data, not by `App`)

```rust
// Pre-rendered pieces, no domain types
PaneFrame::new(title: Line<'static>).focused(bool)            // marker + border policy
TabBar::new(&[Tab { label, id }], active_id).line(&Theme)      // with .fit(width)
PriorityLine::new(sep: " · ").piece(prio, spans, Role)         // .fit(room) -> Vec<Span>
SectionRule::new(label).summary(Option<&str>).summary_first(bool).line(width, &Theme)
PropertyRow { label, value, dim, hint: Option<&str>, annotation, glyph: Option<Span> }
LinkRow { indent, prefix, verb, glyph: Span, label, meta }     // + RowTone
Breadcrumb { hops: Vec<Hop { glyph: Span, tag, id_suffix }> }.fit(available)
ScrollGutter::layout(area, content_len) -> (Rect, Option<Rect>)
follow_offset(offset, first_row, last_row, viewport) -> offset
StackedMasterDetail::areas(inner, list_wrapped_rows, min_detail) -> (Rect, Rect, Rect)
ZonedExplorer<S: ExplorerSource>  // handle(Key) -> ExplorerOutcome { Copy, Inspect, Drill, Back, LeaveRight, Toggle, None }
```

Outcome-typed key handling matches the existing overlay contract
(`OverlayOutcome::{Consumed, Commit, CommitAndStay, Close}`), which the
explorer lacks: it works through `Msg` variants plus `Action::ExplorerEnter`
branching in the runtime.

## 10. Open questions for the operator

1. **Focus cue.** Keep the marker-only design (plain border, `► ` in the
   title, status-bar scope tag), or add a border/title tint on the focused
   pane so pane and modal frames share one vocabulary? This decides the
   `PaneFrame` API and is a visible change.
2. **Unfocused explorer cursor.** The left pane shows a BOLD cue when
   unfocused; the right pane shows nothing. Unify on the left pane's
   active/inactive pair (then the explorer reads `theme.selection_*`) or keep
   the invisible cursor? Related: whole-line vs value-only vs
   partial-span highlight extent.
3. **What to do with `detail.rs`/`detail_header.rs`.** Delete the legacy
   model (retargeting `kind` and `ambiguous_muxes_for_group`) before or as
   part of extraction? It removes 1409+1180+376 LOC and the dead `e`
   behavior on the left pane.
4. **`F`-expanded copy/`o` behavior.** Confirm the suspected defect
   (3.17), and decide whether it is fixed ahead of extraction (it changes
   what `PropertyRow` copy returns).
5. **Right-pane keys.** Should `h`/`l`/arrows, `Esc`, and PageUp/PageDown
   do something in the right pane (collapse/expand Other, leave pane,
   page), or stay inert / one-step? Same-key-two-meanings (`F`, Enter,
   Backspace) can be kept but should be declared in one table.
6. **Hint ownership.** Should status-bar hints become generated from key
   tables (like modal footers) at the cost of losing hand-tuned contextual
   strings such as `default_action_status_hint`, or stay hand-written with a
   consistency test that every advertised key resolves?
7. **Glyph policy.** Move the chrome's nine hard-coded glyph sites onto
   `icons::marks` and extend the glyph reference/test to the chrome, or keep
   chrome glyphs local? Also decide whether `►`/`▶` should stay two
   look-alike codepoints.
8. **Narrow-width contract.** Should the tab strip elide like the breadcrumb;
   should the header degrade by priority like the status bar; should there be a
   minimum-size screen? (CSP-319/CSP-190 are open.)
9. **Feedback taxonomy.** Keep three channels (status line, toast, message
   log) or give the framework one `Feedback { severity, text, ttl }` with
   renderers per channel? The status line currently paints every message
   in the warning colour, and every toast is Success-green.
10. **Header scope.** Is the header line a framework control (a `SummaryLine`)
    or purely app composition? I assumed app composition with only the
    segment/separator primitive shared.

## Verification (CSP-713)

The suspected defect in 3.17 (item 16 in section 8, question 4 in section 10)
was reproduced at runtime and fixed before the extraction. The reproduction
is `conspectus tui --snapshot` on the showcase fixture with
`jjjjjljjjj<Tab>Fjjjjjjjjjo`: the cursor sits on the `last_message_preview`
row of `showcase-claude`, which shows `(truncated · o)`, and the status line
read "explorer: row has no truncated value to open". The tests that pinned it
before the fix are in `src/tui/app_tests.rs` (every shown field, Expanded
Node Detail on and off) and `src/tui/snapshot_tests.rs` (the same keys).

- **Cause.** `ExplorerRow::NodeField { index, .. }` numbers the rows over
  `NodeView::fields(expanded_detail)`, which is `all_fields` with `F` on,
  but `explorer_copy_target` and `open_value_modal_for_cursor` looked the
  index up in `core_fields`. Past the five core rows the lookup missed
  (Enter fell through to the activate that does nothing on a field, and `o`
  reported no value).
- **Fix.** The row now carries its own `value` beside `label` and
  `long_value`, and both gestures read the row instead of looking the field
  up a second time. Copy and inspect therefore agree with what the pane
  draws by construction, which is the shape `PropertyRow` needs: its
  `Copy(String)` and `Inspect(label, text)` outcomes come from the row.
- **Left as is.** `preview_subject` still reads the field by `index`, but in
  `fields(expanded_detail)`, the list the rows are numbered over, so it was
  never wrong. Giving the row its typed `FieldValue` would remove that last
  lookup; the `PropertyGrid` extraction can decide it.

## Verification (CSP-716)

The two suspected chrome defects in 3.4 and 3.5 (D13 in the inventory's
suspected defects, and the tab-strip row of section 6) were run before the
extraction. Both reproduced and are fixed. The reproduction is
`conspectus tui --snapshot` on the showcase fixture (`--snapshot-now
1790769600`, `HOME=/fixture`), the strip read off the second row of the
frame in every view, and the header in every view at 160 columns. The
tests that pin the fixes are in `src/tui/consistency_tests/chrome.rs`; they
read the rendered frame, so they move to `TabBar` and the header's line of
segments unchanged.

- **Tab strip.** Confirmed, and wider than suspected. The strip is 52 cells
  (lead, marker, labels, separators, pad) and the pane's border clipped it
  mid-label whenever the title had less: `sea`, `sear`, `searc` side by side
  at 100 to 104 columns, the trailing pad only at 105 and 106, and stacked
  below the threshold at 52 columns and under (`his` at 40). With Search
  active its own name was the one cut. Now `left_panel_title` takes the
  title width and `tab_strip_fit` decides: every name when they fit (the
  pad is dropped only when that lets every name show); otherwise the
  inactive names give way from the end of the strip, the active one stays,
  and the strip ends with ` …` and a pad. Only a pane too narrow for the
  active name alone cuts it, with its own `…`. Checked at every width from
  40 to 120, both layouts, every view, both focus states; a snapshot keeps
  90, 100, 104, 107, and 120. Width is measured in cells, not `char`s. The
  breadcrumb in the right title still has its own rule (floor 8 cells);
  `TabBar` and the breadcrumb can share one drop-and-mark rule when they
  are extracted.
- **Header count.** Confirmed: Pinned, History, and Search before a search
  read `0/11 sessions`, and PRs read `4/11` with no filter (it counted the
  session rows under its PRs). `visible_agent_session_count` now applies
  the Mux view's rule everywhere: the plain total unless something narrows
  the view. Sessions counts its session rows; Search counts the sessions
  the search it shows found (session rows, or in the flat listing the
  sessions its hits name), and reads the plain total before any search;
  Mux and PRs read the plain total until a session predicate, the content
  search, pinned-only, or (PRs) a PR state or branch narrows them, then the
  sessions their visible rows hold; Pinned and History ignore the filter,
  so they always read the plain total. Mux used to look only at the saved
  filter's predicates, so the content search and pinned-only narrowed its
  rows without its count. The per-view count words ADR 0078 deferred
  (`N prs`, `N pins`) were not added: the header stays `sessions · mux`.

## Verification (CSP-758)

The tab strip, the header line, the status bar's segments, and the empty
states moved onto the framework (ADR 0190). Checked with `conspectus tui
--snapshot` on the showcase fixture and an empty one at 40, 60, 80, 100,
107, 120, and 160 columns: 329 frames before and after, 273
byte-identical.

- **Tab strip (3.4).** `TabBar` on `text::elide`, CSP-716's rule kept:
  byte-identical at every width; `tab_strip_fit` is gone.
- **Header line (3.5, section 6).** Confirmed by render: it had no fit and
  the edge cut it (`· 4` at 40 columns; a harness chip mid-name below 115
  with `show_harness_chips`). It is a `PriorityLine` in Conspectus now and
  drops whole parts by rank with ` …`; a consistency test checks every
  width from 40 to 120 in every view. The separators stay as they were
  (` · ` in the counts, `  ·  ` before a chip section, `  ` between chips),
  each a segment's own gap.
- **Status bar (3.6).** The hand-written packing is `text::elide_filled`
  under a `PriorityLine`, with named, distinct ranks in the old order:
  byte-identical at every width and in both focus states. The inline
  priority numbers the inventory noted are named tiers now.
- **Empty states (3.14).** The left pane has 15 messages today and the
  right pane 2; all are `EmptyState`s with their copy, drawn as before
  (the right pane's now wraps below 34 columns). The Preview's
  nothing-to-show placeholders take the same muted look; its failure
  reasons keep the body's.
- **Separators (8.6).** Three separators remain on purpose (` · `,
  `  ·  `, `  `), now declared as a line's separator and segments' gaps
  rather than spliced spans.
