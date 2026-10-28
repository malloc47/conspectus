# Inventory 05: Search, Help, Full-Screen Viewers And Preview Panes

Status: evidence notes for the TUI framework extraction plan (phase 2,
inventory). Slice: every reading surface (transcript viewer, pane scrollback
viewer, help pages, value modal, messages overlay, preview pane) and every
search/find surface (quick select, content search, in-viewer find, find
prompt, filter chips). Nothing here changes code. All claims come from
reading the source; no tests, cargo, or tmux were run, so behaviors marked
"by reading" are not machine-verified.

Paths are relative to the repo root. `file:line` citations are against
branch `worktree-tui-framework` at `ca668a3`.

## 0. How To Read This

- Section 1 maps every surface in one table.
- Section 2 gives the per-control rubric entries.
- Section 3 compares the surfaces on the dimensions that matter for
  deduplication (scroll, find, keys, chrome, footers).
- Section 4 assesses third-party crates. Sections 5 and 6 sketch the two big
  merge candidates (Pager, Search palette).
- Section 7 lists theme tokens by control. Section 8 lists tests.
- Section 9 is the numbered list of duplications and inconsistencies.
  Section 10 is open questions for the operator.

Vocabulary used below: **domain** = Conspectus-specific (sessions, muxes,
harnesses, graph nodes). **generic** = would make sense in any ratatui app.
Leak classes: (a) parameterizable by generic/closure/trait; (b) belongs in a
Conspectus-side adapter outside the framework; (c) inherently domain.

## 1. Surface Map

| # | Surface | Kind | Key files (LOC) | Generic / domain split |
|---|---|---|---|---|
| S1 | Transcript viewer | full-screen, `Modal::Viewer` | `src/viewer/widget.rs` (759), `state.rs` (246), `input.rs` (201), `find.rs` (412), `highlight.rs` (116), `render.rs` (702), `table.rs` (415), `search.rs` (609), `help.rs` (92), `model.rs` (223), `parser/*` (~2.1k), `theme.rs` (12); bridge `src/tui/viewer_bridge.rs` (100) | Pager chrome, scroll, find, help: generic. Gutter chips, tool/thinking/aborted/hidden toggles, model, parsers: domain. Markdown/table render: generic rich text. |
| S2 | Pane scrollback viewer | full-screen, `Modal::PaneViewer` | `src/tui/widgets/pane_viewer.rs` (616) | Almost all generic (a line pager with find). `a` attach, `PaneRef`, header label: domain. |
| S3 | Keymap help (`?`) | centered modal, `Modal::Help` | `src/tui/widgets/help.rs` (891) | Page renderer + binding rows: generic. `keymap_sections()` text and four glyph legends: domain. |
| S4 | Transcript viewer help | centered modal over S1 | `src/tui/widgets/viewer_help.rs` (128), `src/viewer/help.rs` (92) | Generic mechanism (key table + descriptions -> sections). |
| S5 | Pane viewer help | centered modal over S2 | inside `pane_viewer.rs:554-612` | Same mechanism as S4, copied. |
| S6 | Value modal (`o`) | centered modal, `Modal::ValueModal` | `src/tui/widgets/value_modal.rs` (239) | Generic text pager with copy. Rebuild-plan `$ ` highlighting is a small domain hook. |
| S7 | Messages overlay (`!`) | large centered modal | `src/tui/widgets/messages.rs` (251) + `src/tui/messages.rs` | Master list + detail pager. Log model: domain. |
| S8 | Preview pane | right-panel zone | `src/tui/ui/preview_pane.rs` (470), `src/tui/preview/*` (~2.0k non-test), `src/tui/preview_wrap.rs` (289), `src/tui/preview/search.rs` (137) | Zone chrome, scroll anchoring, ANSI/wrap: generic. Source kinds/loaders: domain. |
| S9 | Quick select (`/`), Current + All | centered modal, `Modal::Search` mode Quick | `src/tui/widgets/search.rs` (1523, shared with S10), `src/tui/search.rs` (162), `src/tui/search_entities.rs` (266) | Query line + ranked list + tab segments + footer: generic. Ranking, entities, outcome words: domain. |
| S10 | Content search (`Ctrl-/`, `s`) + options pane | same modal, mode Content | `search.rs` (above), `src/viewer/search.rs` (609) | Scan status + streaming results + debounce: generic-ish. Kinds, sources, history: domain. |
| S11 | Search view rows + header chips | left-pane view | `src/tui/ui/left_panel.rs:1357-1523`, `src/tui/ui/header.rs:336-354`, `src/tui/ui/right_panel.rs:870-918` | Uses the left-pane row grid (other slice). |
| S12 | Find prompt (in S1 and S2) | footer line editor | `src/viewer/find.rs:46-110`, `src/viewer/widget.rs:492-546` | Generic single-line editor (hand-rolled). |
| S13 | Snippet fitting + match painting | primitives | `src/tui/widgets/snippet.rs` (198), `src/viewer/search.rs:272-505`, `src/viewer/highlight.rs` | Generic. |
| S14 | Key tables and footer/help generation | layer-2 primitive | `src/tui/keybindings/overlay.rs` (834) | Generic by construction (`KeyRow<A>`). |
| S15 | Popup frame + modal sizing | primitive | `src/tui/widgets/popup_frame.rs` (127) + per-modal rect fns | Generic. |
| S16 | `src/tui/viewer.rs` (175) | not a UI control | external `claude-history` launch plan | Stays in Conspectus (process hand-off). Do not confuse with S1. |

Test volume for the slice: about 400 `#[test]` functions across 29 files
(section 8).

## 2. Per-Control Entries

### 2.1 Transcript viewer (S1)

**Purpose.** Full-screen scrollable view of a parsed session transcript with
find, toggles, and a generated help page.

**Layout** (`src/viewer/widget.rs:47-62`): five rows from `Layout::vertical`:
header (1), rule (1), body (`Min(1)`), rule (1), footer (1). The help page
(S4) is drawn on top as a centered popup by `runtime.rs:843-861`
(`draw_screen`), after `viewer::widget::draw`.

- Header (`widget.rs:64-117`): ` {harness}:{session_key} · {cwd or "(no cwd)"} ·
  {N} turn(s)`. Title bold in harness color; separators and counts
  `secondary_text` + literal `DIM`. If the assembled string is wider than the
  area it falls back to title only (no truncation of the title itself).
- Rule (`widget.rs:119-130`): `"─".repeat(width)`, `secondary_text` + `DIM`.
- Body (`widget.rs:132-183`): lines pre-composed and cached in
  `ViewerState.rendered` (`state.rs:81`, key = content width + 4 toggles).
  Visible window is a `Vec::to_vec()` slice (comment at `widget.rs:170-173`:
  slicing instead of `Paragraph::scroll` to avoid the u16 offset limit).
  Empty document: one dim line `transcript unavailable`.
- Footer (`widget.rs:254-458`): `[ top/total ]` position chip, optional find
  chip, `tools·sum (t)`, `think·off (T)`, `aborted·hide (I)`, optional
  `hidden·hide (H)` (only when any turn is `hidden`), then hand-written
  `(/) find`, `(?) help`, `j/k scroll · g/G top/end · PgUp/PgDn`, `(q) close`.
  On-state chips `success`+BOLD, off-state `secondary_text`+DIM, no-match find
  chip `warning`+BOLD. Four fit tiers (comment `widget.rs:356-364`; code
  `365-456`) drop the scroll hint and help chip, then one filter chip, then the
  other, keeping position/find/tools/think/close.
- Find prompt replaces the whole footer line (`widget.rs:255-258`, drawn by
  `draw_find_prompt` `widget.rs:492-546`): bold `/`, text with a REVERSED
  block cursor, left-scrolling by display cells (CJK aware), then
  `   Enter find · Esc cancel` from `state.find_hint` (generated from
  `VIEWER_FIND_KEYS` by the bridge, `viewer_bridge.rs:59`).

**Public surface.**

- State: `ViewerState` (`state.rs:107`, 14 pub fields: `document`,
  `scroll_offset`, `stick_to_end`, `tool_detail`, `show_thinking`,
  `show_aborted`, `show_hidden`, `show_help`, `help_scroll`,
  `viewport_height`, `total_lines`, `rendered`, `find`, `find_hint`).
  Constructor `ViewerState::new(TranscriptDocument)`. `open_at(&ViewerOpen)`
  for result landing (`find.rs:305`).
- Widget: free functions `draw(&mut ViewerState, &Theme, &mut Frame, Rect)`
  (`widget.rs:47`), `body_lines(&mut state, &Theme, width) -> &[Line]`
  (`widget.rs:192`, also used by the preview pane), `cached_body_lines`
  (`widget.rs:232`), `draw_find_prompt` (`pub(crate)`, `widget.rs:492`, also
  used by the pane viewer).
- Keys: pure reducer `reduce(ViewerState, ViewerMsg) -> (ViewerState,
  ViewerEffect)` (`input.rs:61`) and `reduce_find(ViewerState, ViewerFindMsg)
  -> (ViewerState, ViewerEffect)` (`find.rs:381`). `ViewerEffect` is
  `None | Close`. Key resolution is outside the module: `VIEWER_KEYS`,
  `VIEWER_FIND_KEYS` in `keybindings/overlay.rs:662,691`.
- Routing (the odd one out): `Action::ViewerOverlayKey` ->
  `handle_viewer_overlay_key` (`runtime/overlay_keys.rs:573`) -> `Msg::Viewer`
  / `Msg::ViewerFind` -> `App::update` pops the `Modal::Viewer`, runs the
  reducer, and pushes it back unless `Close` (`app.rs:1641-1668`). It does not
  implement `Overlay` (see section 9, item 2).

**Scroll model.** Line offset into pre-wrapped composed lines.
`scroll_offset` + `stick_to_end` (opens on the last turn; any movement key
clears it, `JumpToEnd` sets it, so it is sticky across resize/re-wrap).
`viewport_height` and `total_lines` are written back by `draw` (`widget.rs:180`)
and read by the reducer for paging (the "renderer mutates state" finding F5 in
`docs/tui-architecture-review.md`). Page = viewport; half page = viewport/2;
`g`/`G`, Home/End. `ScrollDown` clamps to `max_scroll()` (`state.rs:189`).

**Find model.** Case-insensitive ASCII substring (`find_ci`,
`viewer/search.rs:188`), per **turn** on the raw body, scoped to the kinds the
toggles currently show (`find.rs:165`). Prompt on `/` (prefilled with the last
query, `find.rs:177`); Enter confirms and jumps to the first match at or below
the viewport's top turn (`Prefer::Viewport`, `find.rs:195-240`). `n`/`N` step
with wrap-around and a one-key footer note (`wrapped to the first match`).
`pending_scroll` lands the turn's first line one line down from the top
(`widget.rs:153-159`: `start.line - 1`). `hidden_matches` counts matches in
turns the toggles hide and the chip says `no match · N in hidden turns`
(`widget.rs:479-481`). Toggling a filter re-runs the find and stays on the
current turn (`refresh_find`, `find.rs:250`). Opening at a search result
(`open_at`, `find.rs:305`) looks within 25 turns of the remembered turn, then
anywhere, reveals hidden content by switching toggles on, and says so in the
footer note.

**Highlighting** (`highlight.rs`): matches painted on the rendered lines after
the gutter (`leader` cells), per turn; the current turn REVERSED|BOLD (no
color), other matching turns `theme.match_style()`. A match split by wrapping
is found but not painted; markdown markers stripped on screen are not
matched.

**Esc stepping.** `Esc` = `Close`: help, then active find (`find.clear()`),
then leave. `q` and `Ctrl-C` = `Quit`: closes help only if open, else leaves
at once even with a find active (`input.rs:156-177`). Prompt: `Esc`/`Ctrl-C`
cancel the prompt only (keeping the earlier find).

**Help.** `?` toggles `show_help`; while open, movement msgs scroll the help
(`input.rs:73-81,184-197`); page size uses the transcript's `viewport_height`
(not the help modal's height; see section 9, item 7).

**Render pipeline.**

- Gutter-and-chip (`render.rs`): per-turn `compose_line` (`render.rs:226`):
  right-aligned role/kind chip (`you`, `ai`, `thinking`, `tool`, `↳ result`,
  `compact`, `system`) in a gutter sized to the widest visible label
  (`widget.rs:586`), then ` │ ` separator in the chip color on every line of
  the turn; blank spacer line between turns. Chip color map `render.rs:274`.
- Messages / compaction summaries: `table::segment_body` (`table.rs:80`)
  splits GFM pipe tables out of the body (own state machine that honors
  ``` and `~~~` fences), tables go through comfy-table
  (`table.rs:281`, `UTF8_NO_BORDERS`, `Dynamic`, `set_width`), prose goes
  through `tui_markdown::from_str` (`render.rs:310`, `highlight-code` on, so
  syntect), then `join_orphan_list_markers` (`render.rs:344`) and
  `wrap_styled_line` (`render.rs:649`) wrap to the content width.
- Thinking: plain word-wrap, italic + literal DIM (`render.rs:324`).
- Tools (`render.rs:382`): four levels (`ToolDetail` Hidden / Summary /
  Truncated(8 lines) / Full); Summary mode rolls a run of tool turns into one
  claude-history-style phrase (`aggregate_tool_phrase`, `render.rs:164`);
  line-number-prefix dimming (`detect_line_number_prefix`, `render.rs:497`).
  Tool output is **not** passed through ansi-to-tui, despite
  `docs/transcript-viewer-deps.md` listing it as the reason for the dep.
- Word wrap helper `viewer::render::word_wrap` (`render.rs:554`) is the
  project-wide string wrapper (used by help, popup_frame; see 2.11).

**Conspectus coupling (every `crate::` import in `src/viewer/`, outside tests
and docs).**

| Import | Where | Class |
|---|---|---|
| `crate::tui::theme::Theme` | `viewer/theme.rs:12` (the ADR 0052 carve-out), used everywhere | (a) replace with framework theme tokens |
| `theme.harness_color(harness)` | `widget.rs:724`, `render.rs:284` | (b)/(c): harness identity colors are domain; the user chip even reuses the codex color |
| `theme.pr_merged` | `render.rs:276` (compaction chip color) | (c)/(b): a PR-state token borrowed for a transcript chip; token abuse |
| `theme.cwd_mark` | `widget.rs:89` | (b) domain token |
| `theme.badge` | `render.rs:241` | (a) generic |
| `theme.match_style()` | `highlight.rs:46`, table via `secondary_text` | (a) generic |
| `theme.success/warning/secondary_text` | chips, footers | (a) generic |

No other `crate::` import exists in `src/viewer/` (non-test). Parsers
reference `HarnessAdapter` only in doc comments. `model.rs` has no `crate::`
import. The module enforces its boundary with `ALLOWED_EXTERNAL_DEPS`
(`viewer/viewer_tests.rs:9`) tested against
`docs/transcript-viewer-deps.md` (ADR 0052).

**Variants / callers.**

- Full-screen modal: `draw_screen` (`runtime.rs:848`), built by
  `build_viewer_state_at` (`viewer_bridge.rs:48`) from `execute_view_session`
  (`runtime/executor.rs:406`) and the snapshot driver (`snapshot.rs:246`).
- Inline preview: the same composer (`body_lines`, `cached_body_lines`,
  `highlight_matches`) feeds the right panel's Preview zone
  (`preview_pane.rs:311-373`, `app.rs:1220-1235` `prepare_preview`). So the
  transcript renderer already runs as a content source for two pagers.

**Tests.** `viewer/widget_tests.rs` (20, 8 insta snapshots), `find_tests.rs`
(28, 2 snapshots), `input_tests.rs` (17), `highlight_tests.rs` (7),
`render_tests.rs` (16), `table_tests.rs` (13, 2 table snapshots in
`widget_tests`), `search_tests.rs` (28), `help_tests.rs` (3),
`viewer_tests.rs` (dep manifest), plus `tui/widgets/viewer_help_tests.rs`
(13, 3 snapshots). Layout consistency test **skips** the viewers
(`consistency_tests/layout.rs:53-63`).

**Verdict: split into generic `Pager` core + Conspectus `TranscriptContent`
adapter.** Merge with S2 (section 5). The gutter-chip block
(`compose_line`) is a possible optional framework component ("role gutter")
but only one caller exists; keep adapter-side until a second user appears.

### 2.2 Pane scrollback viewer (S2)

**Purpose.** Full-screen pager over a snapshot of a tmux pane's scrollback,
opened from a pane search hit (`runtime/executor.rs:374` `open_pane_scrollback`).

Docs (`pane_viewer.rs:1-12`) say it "is the pane's counterpart of the
transcript viewer and keeps its shape".

**Layout** (`pane_viewer.rs:393-411`): the same five rows. Header
` {mux} {win}.{pane} scrollback · {command} · N lines` (accent + BOLD label,
dim rest; no overflow fallback). Body: `{:>5} ` line number gutter
(`secondary_text`; the current match's number is `panel_focus_accent` +
BOLD) then the raw line with every find occurrence in `match_style`. **No
wrapping, no horizontal scroll**: lines wider than the viewport are clipped by
`Paragraph` (matches beyond the edge are invisible). Empty: `the pane has no
scrollback`. Footer: `[ top/total ]`, find chip, note (warning), then
`j/k scroll · g/G top/end · / find · (a) attach · (?) help · (q) close`
falling back to the last three chips only (two tiers, vs four in S1;
`pane_viewer.rs:539-551`).

**Public surface.** `PaneViewerState` (13 pub fields: `pane`, `label`,
`lines`, `scroll`, `viewport`, `prompt`, `query`, `matches`, `current`,
`show_help`, `help_scroll`, `note`, `find_hint`), `PaneViewerState::open(pane,
lines, line, find)` (`:156`), `apply(PaneViewerMsg) -> PaneViewerOutcome`
(`:268`), `apply_find(ViewerFindMsg)` (`:339`), `handle_key(KeyEvent) ->
PaneViewerOutcome` (`:375`, resolves through the tables itself), free `draw`
(`:393`). `PaneViewerOutcome` = `Continue | Close | Attach`. `PaneViewerMsg`
has 15 variants (`:39`), a strict subset of S1's 18 plus `Attach`.

**Scroll/find model differences vs S1** (all from reading):

| Aspect | S1 transcript | S2 pane |
|---|---|---|
| Find unit | turn (many rows) | line (one row) |
| Initial position | end (sticky) | the requested line, or top |
| `G`/End | sets `stick_to_end` | sets `scroll = max` (not sticky) |
| Jump to match | always scrolls to turn start - 1 | scrolls only if match not visible, 2 lines of context (`CONTEXT_ABOVE`, `:35`) |
| Confirm with no match | chip says `no match` | chip says `no match` **and** a footer note `no match` (`:351`) |
| `n` with no find | note `nothing to find yet: press / to search` (`find.rs:268`) | note `no find: press / to search` (`:235`) |
| `n` when active but empty | silent (chip already says) | note `no match` (`:233`) |
| Current-match marker | whole turn REVERSED+BOLD | gutter number accent+BOLD |
| Wrap-around note | `wrapped to the first/last match` | same words |
| Match granularity in the footer | `find "q" 3/10` | `find "q" 3/10` |
| Reducer shape | pure `(state, msg) -> (state, effect)` | `&mut self` method returning outcome |

**Conspectus coupling.** `crate::content_search::PaneRef` (state field, header
label, `Attach` target), `crate::tui::Theme`, `crate::tui::keybindings::overlay::{PANE_VIEWER_KEYS,
VIEWER_FIND_KEYS, resolve, typed_char, KeyRow}`, `crate::viewer::find::{FindPrompt,
ViewerFindMsg}`, `crate::viewer::search::find_ci`, `crate::viewer::widget::draw_find_prompt`,
`super::help::{...}` and `super::viewer_help::key_notation`. Classification:
`PaneRef`/label/`Attach`/`command` are (b); everything else is (a) or already
generic code living in the wrong module (section 9, item 15).

**Tests.** `pane_viewer_tests.rs` (11 tests, plain string-grid rendering, no
insta). Excluded from the layout consistency test.

**Verdict: extract after refactor, by merging into the Pager.** After the
merge S2 is a content adapter of about 60 lines (lines + gutter + attach).

### 2.3 Help pages (S3, S4, S5)

**One renderer, three producers.** `render_help_page` (`help.rs:126`,
`pub(super)`) draws all three: `themed_popup` frame with title ` Help `,
`HelpBody` (`help.rs:226`) with body lines wrapped to the modal width, a
**pinned footer** (`page_layout`/`footer_rows`, `help.rs:166-221`), and a
`Scrollbar` when the page overflows (1 column, re-wrapping once with
`width - 1`). Callers pass a `&dyn Fn(usize) -> Vec<Line<'static>>`
(`lines_for(width)`). Modal rect `help_modal_rect` (`help.rs:873`):
`min(100, w-4).max(40)` x `(h-4).max(10)`.

Row helpers in `help.rs`: `section` (bold title, `:862`), `blank` (`:866`),
`binding_lines` (`:620`; key column 16 cells = 2 indent + 14, `panel_focus_accent`;
a key wider than 14 cells gets its own line; descriptions wrap under a hanging
indent via `viewer::render::word_wrap`), `legend_entry` (`:588`; glyph + 16-cell
name + wrapped blurb), `push_footer` (`:553`; footer text from
`footer(HELP_KEYS)` = `j/k/PgDn/PgUp to scroll · Esc/q/? to close`,
`theme.placeholder`).

| Page | Producer | Content source | State |
|---|---|---|---|
| S3 Keymap | `body_lines_for_width` (`help.rs:512`) | **hand-authored** `keymap_sections()` (`:307`): 9 sections, 48 `ratatui_cheese::help::Binding::new(key, text)` rows with prose (some 200+ chars), plus four legends: node-kind icons (10), mux state (5), PR state (4), pins (2), via `NodeKind::ALL`, `node_kind_style`, `icons::state::*` | `HelpOverlayState { scroll: u16 }` (`:26`), `Overlay` impl (`:86`) |
| S4 Viewer | `viewer_help::help_lines` (`viewer_help.rs:89`) | **generated**: `sections_for(VIEWER_KEYS)` pairs each `ViewerMsg::ALL` (18) with `ViewerMsg::help()` (`viewer/help.rs:44`, exhaustive match -> section + sentence) and the keys the table binds; message with no key is omitted; plain keys before `Ctrl-` chords; `key_notation` (`:72`) maps arrows to `↑↓←→`, Space to `Space` | `ViewerState.show_help`, `help_scroll: usize` (clamped and written back in `viewer_help::draw` `:122`) |
| S5 Pane | `pane_viewer::help_lines` (`pane_viewer.rs:591`) | **generated**, copy of S4's algorithm (`help_sections` `:563`), `PaneViewerMsg::ALL` (15) + `help()` tuples | `PaneViewerState.show_help/help_scroll` |

**Drift protection.**

- S3: `help_tests.rs:20` `every_keybindings_entry_appears_in_help_sections`
  checks one direction only (every `KEYBINDINGS` label appears in the sections;
  not that every section claim is bound). Hand prose can and does drift: the
  contents-modal blurb says "Ctrl-L switches between listing sessions and
  listing messages" (`help.rs:423`), but inside the modal that action is
  `S-Tab` (`overlay.rs:409-414`); `Ctrl-L` is the Search *view* key
  (`keybindings.rs:~400`).
- S4/S5: adding a message without a description fails to compile (S4's
  `help()` match is exhaustive); S5's `help()` too. A message without a key is
  silently omitted from the page.
- `consistency_tests/help.rs` (4 tests): S3 and S4 draw the same frame at four
  sizes, style headings/keys/footer alike, end with the same footer, scroll
  alike. S5 is not covered.

**Public surface** (S3): `HelpOverlayState::{new, handle_key -> HelpOutcome}`,
`HelpOverlayWidget::new(&state, &theme)` (`Widget`), `help_modal_rect`,
`centered_modal_rect` (shared with the value modal).

**Conspectus coupling.** `crate::tui::icons::{NodeKind, node_kind_style, state}`
(legends; (c)), `crate::tui::keybindings::{MESSAGES_HELP, HARD_REFRESH_HELP,
UNDO_KEY, overlay::HELP_KEYS}` ((b); strings reused from the global table),
`crate::viewer::render::word_wrap` ((a), wrong home), `ratatui_cheese::Binding`
(data container only), `tui_popup::KnownSize`.

**Scroll semantics.** S3: `u16` state, `PgDn`/`PgUp` = fixed 8 lines
(`help.rs:51-58`), `j`/`k` +-1, `g`/`G`, state is **not clamped back** after
render (render takes `&HelpOverlayState`), so extra `j` presses past the end
accumulate invisibly (by reading). S4/S5: `usize` state, viewport-sized pages,
clamped and written back at draw.

**Verdict.** Split: framework `HelpPage` (frame + pinned footer + scrollbar +
`section`/`binding`/`legend` row builders + `from_key_table(table, describe)`
generator) in layer 2; Conspectus supplies content (`keymap_sections` prose,
the four legends). Delete S5's copy and S4's `sections_for` in favor of the one
generator. Longer term, generate S3's binding rows from the global
`KEYBINDINGS` (it already has a `help_text` field) so the drift test becomes
two-way.

### 2.4 Value modal (S6)

**Purpose.** Show an untruncated value or a command plan in a scrollable popup;
`y` copies. Opened by `o` on an explorer row (`app.rs:748-780`, label = field
name), on a failed rebuild plan (`runtime/launch.rs:688`, label
`rebuild plan · {ref}`), and for a successful plan (`launch.rs:715`,
`ValueModalState::plan` with `highlight_commands` and a `ValueCopy`).

**Public surface.** `ValueModalState { label, value, scroll: u16, copy:
Option<ValueCopy>, highlight_commands }` (`value_modal.rs:32`); `new`,
`plan`, `copy_payload()`, `handle_key -> ValueModalOutcome` (`Continue |
Close`, identical to `HelpOutcome`), `Overlay` impl. `ValueModalWidget::new(&state,
&theme)` (`:158`).

**Look and feel.** Frame via `themed_popup`; rect `centered_modal_rect` (78 wide
max, 28 tall target, `help.rs:882`); title ` ` + accent-BOLD ` {label} ` + ` `;
body `Paragraph` with `Wrap { trim: false }` and `scroll((u16, 0))`; command
lines (`$ ` prefix) `panel_focus_accent`; one footer row
`footer(VALUE_MODAL_KEYS)` (`y to copy · Esc/q/o to close · j/k to scroll`,
as generated by verb merging; not wrapped) in
`secondary_text`.

**Keys.** `VALUE_MODAL_KEYS` (`overlay.rs:463`): `y`, Esc/q/`o` close, j/k/arrows
+-1, PgUp/PgDn/Space +-8, g/G/Home/End. `y` is resolved to `PagerKey::Copy`
but the widget returns `Continue`; the runtime copies before the key reaches it
(`value_modal.rs:97`).

**Gaps (by reading).** `G` sets `scroll = u16::MAX` and render passes it
straight to `Paragraph::scroll` with no clamp, unlike help (which has a test
that `G` shows the last lines, `help_tests.rs:310`); the value-modal test only
asserts the state (`value_modal_tests.rs:63-66`). Likely blank until `k`; needs a
real check. No scrollbar, no position hint.

**Conspectus coupling.** `crate::tui::Theme`, `crate::tui::widgets::help::centered_modal_rect`,
`crate::tui::Overlay`, `OverlayOutcome`. The `$ `-prefix rule is the only
domain behavior ((a): a line-style predicate). Copy payload/toast noun
`ValueCopy.what` is (b).

**Verdict: extract after refactor as the Pager's modal variant** with an
optional "copy action" hook; merge with help (S3) and messages detail (S7).

### 2.5 Messages overlay (S7)

Master list (newest first, up to 40% of height) + rule + detail pager
(`Paragraph` wrap + `scroll(u16)`) + generated footer. Rect inline in
`messages.rs:116-118` (`w-4` clamped 40..110, `h-2` min 10), title accent-BOLD
` Messages · N `. State `{ selected, detail_scroll: u16 }`. Keys `MESSAGES_KEYS`
(`overlay.rs:503`): j/k select, `J`/`K` detail +-1, PgUp/PgDn/Space detail
+-8, g/G first/last, `y` copy (runtime), Esc/q/`!` close. Domain: `LogEntry`,
`LogLevel` -> `level_style` (success/warning/error), `format_recency`. The list
half belongs to the list/menu inventory; the **detail half is a Pager**.
Verdict: stays in Conspectus as a composition (log model) built from the
framework's `List` + `Pager`.

### 2.6 Preview pane (S8)

**Purpose.** The right panel's Preview zone: whatever the selection (or an
explorer row) points at, shown read-only.

**Content-source shape (already pluggable).**

- `PreviewSource` (`preview/mod.rs:39`): `Pane(mux)`, `Transcript(session)`,
  `Path`, `Git(query)`, `Process(pid)`, `PullRequest(query)`.
- `PreviewStore` / `PreviewEntry` / `PreviewContent` (`:138,:146,:155`):
  in-memory cache keyed by source with per-kind caps (8 transcripts, 64 other)
  and `stale` marking. `PreviewContent` = `Text(PaneCapture) | Transcript(Box<ViewerState>) | Lines(Vec<Line>) | NoTarget | Unavailable | Failed | Unsupported`.
- `PreviewSubject` (`:289`): `Selection | Node | Field | Unresolved`, from the
  explorer cursor.
- Loaders (`git.rs`, `path.rs`, `process.rs`, `forge.rs`, `value.rs`): pure
  `-> Vec<Line<'static>>` composers. Domain, stay.
- `PreviewView { text: Text, window: Option<PreviewWindow> }`
  (`preview_pane.rs:8-41`): the contract between a source and the zone. `window
  = None` means "scroll my text from the top by `preview_scroll`"; `window =
  Some{total, top}` means "I already cut my window; draw it and a scrollbar at
  `top`". `below(header)` prepends a banner.

**Scroll anchoring (three models in one zone).**

1. Top-anchored (text, path, git, value): `Paragraph::scroll((app.preview_scroll(), 0))`
   with `wrapped_line_count` (`ui.rs:341`, ceil of width/pane width) driving the
   scrollbar (`preview_pane.rs:57-72`).
2. Bottom-anchored (transcript, pane capture): newest rows in view, scroll
   counts up from the bottom (`preview_pane.rs:347-357`); `ScrollPreviewBy`
   flips the sign (`app.rs:1526-1537`).
3. Match-anchored (a search is on display): `match_top` puts the current
   match one line below the top, `preview_match_offset` moves relative to it
   (`preview_pane.rs:350-355`, `app.rs:1523`).

Keys: only `J`/`K` +-1 line (`keymap.rs:255`); no paging, no `g`/`G`. State
`preview_scroll: u16` (`app.rs:159`).

**ANSI pipeline.** `render_captured_pane` (`preview_pane.rs:447`):
`ansi_to_tui::IntoText::into_text`, with a color-off path that flattens to
plain text and a parse-error fallback to raw text (ADR 0025). Then
`preview_wrap::layout_capture(lines, mode, pane_width, width)`
(`preview_wrap.rs:54`): per-cell model (`Cell { text, width, style }`,
tabs expanded to 8 columns), trailing blank rows dropped, three modes
(`PreviewWrap::{Plain, Smart (default), None}` defined in `tui/mod.rs:297`,
selectable in Controls): `Plain` = word wrap everything; `Smart` = drop
trailing padding, truncate when only decoration (box-drawing, rules)
overflows, squeeze interior padding runs, else word wrap with hanging indent;
`None` = hard-wrap at tmux's own width then clip. Only imports `super::PreviewWrap`
(+ ratatui, unicode-width); the "pane width" input is tmux-flavored.

**Search marking.** `preview/search.rs`: `preview_search(app)` derives the
active content search from app state (Search view's viewed search or the
applied filter); `apply(state, search)` points a Preview transcript's
`FindState` at it; `mark_rows` paints a pane capture's rows with
`viewer::highlight::paint` and picks the window so the newest match shows.

**Failure banner** (`failure_banner`, `preview_pane.rs:138`): level glyph +
summary (bold, `level_style`), `exit status N`, up to 6 indented output lines,
`! for the full output` (placeholder), then a 40-dash `theme.divider` rule.

**Chrome.** `preview_divider_line` -> `chip_divider_line("Preview", ...)` in
`detail_header.rs:281` (other slice). Scrollbar `scrollbar_layout` /
`render_vertical_scrollbar` (`left_panel.rs:152,191`, ADR 0076): 1-column
gutter, fade-on-fit, REVERSED removed, effective-content-length adjustment so
the thumb reaches the bottom.

**Verdict.** Split: generic `ScrollRegion` (Paragraph-or-windowed content +
anchor mode + scrollbar, fade-on-fit) and `AnsiText` (ansi shim +
`layout_capture` modes) in the framework; `PreviewSource`/store/loaders/
subject/banner/search-derivation stay in Conspectus. The `PreviewView`/
`PreviewWindow` pair is the right shape for the framework's content contract
(section 5).

### 2.7 Quick select (S9)

`SearchOverlayState` (`widgets/search.rs:165`, 16 private fields) in
`SearchMode::Quick`. `Reach::{Current, All}` toggled by Tab / Shift-Tab
(`SEARCH_KEYS`, `overlay.rs:387`). Opened by `/`.

**Layout.** Rect `modal_size` (`search.rs:78`): width `clamp(85%, 60, 200)`,
height `clamp(80%, 12, 50)`, always 1-cell margin; border via `themed_popup`.
Title: ` / quick select  [ Current ] All  ({n}) ` with the chosen segment
`panel_focus_accent` + BOLD + REVERSED (`choice_segments`, `:689`; both words
always present so the title width is constant). Body: row 0 query line (accent
`/` + text, **no cursor cell drawn**, `:821-831`; editing is `tui_input`),
list, then up to 4 wrapped footer rows from `footer(SEARCH_KEYS)` (`render_footer`,
`:1085`): `↑/↓ move · Enter pick · Esc close · Tab current/all`. Empty list:
`(start typing)` / `(no matches)` in `placeholder`.

**Current reach.** `items_from_rows(&app.tree().rows)` (`tui/search.rs:72`)
flattens each left-pane `Row` into `SearchItem { id: RowId, label, haystack }`
(haystack = joined fields), `rank` = `find_ci` position score. Row lines are
built by overlay-private `build_match_line` (`:1290`): `> ` mark, node-kind glyph
span (or two spaces), label, `  · ` and a `snippet_around(haystack, range,
budget)` with the match in `theme.match_style()`; cursor row REVERSED. **It
does not reuse the left-pane row renderer or column grid** (no badges, state
glyphs, ages); it shares only the node-kind glyph/color (`search_kind_glyph_span`,
`:1382`).

**All reach.** `EntityIndex::build(snapshot, live_preview)` (built lazily on
first widen; `search_entities.rs:75`) -> `rank(query)` keyed by (label vs field
tier, position, kind rank, recency). Row = cursor mark + kind glyph + label
(match range in `match_style`) + dim context + right-aligned outcome word,
columns from `entity_columns` (`:794`): outcome <= 16, label up to 45% (16..48),
context dropped under 10 cells. The outcome word comes from a host closure
`OutcomeWords = &dyn Fn(&NodeId) -> String` (`:628`).

**Outcomes.** `SearchOutcome` (8 variants, `:434`): `Continue | Cancel |
Confirm(Box<RowId>) | OpenEntity(Box<NodeId>) | OpenHit(usize) | ApplyFilter |
Keep | Regroup(Grouping)`. The `Overlay` impl (`:1447`) maps them to
`OverlayOutcome::{Consumed, Close, Commit(Msg::...), CommitAndStay}`, with 7
different `Msg` variants constructed inside the widget file.

**Coupling.** `crate::model::{NodeId, AgentSessionId}`, `RowId`, `EntityIndex`,
`NodeKind`, `Msg`, `PaneRef`, `TextInputState`, `cursor::wrap_step`,
`ContentKinds`, `Grouping` ((b) or (c)). Generic skeleton inside: query input,
cursor/scroll window (`compute_scroll` `:1423`), tab-segment title,
footer-from-table, empty-state line.

### 2.8 Content search (S10)

Same state/widget, `SearchMode::Content`; keys `SEARCH_CONTENT_KEYS`
(`overlay.rs:400`): everything in quick select plus S-Tab (sessions/messages
granularity), Ctrl-R/Ctrl-T (earlier/later remembered search), Ctrl-O
(options), Ctrl-F (apply as filter), Ctrl-S (keep in Search view).

- Title ` / search contents  [ Current ] All  [ Sessions ] Messages ` (two
  segment pairs).
- Status row under the query: `{n} result(s) · messages + tools + thinking ·
  {scanned}/{total} sessions and panes · {k} skipped`
  (`status_text`, `:1105`).
- Results: `ContentRow { target, label, chip, hits, turn, age, snippet,
  highlight }` (view-model built by the host, `app/content.rs:1601,1658`),
  `ContentView { rows, scanned, total, skipped, done, with_panes, empty_scope,
  unavailable }`. Row = `> `, source word (`session`/`pane`, 7), chip
  (`harness_badge` or pane command, 10), label (`clamp(n/5, 12, 32)`), tag
  (`N hits` or turn/`line N`, 14/20/22 by width), age (4), snippet via
  `snippet::fit`. `content_columns` (`:1181`) drops chip, then source, then
  age, then shrinks the label to keep a 20-cell snippet (documented in ADR
  0149 #25).
- Debounce protocol: `edited_at` + `take_due_search(now, force)` -> `DueSearch::
  {Preview{...}, Clear}` after `DEBOUNCE = 200ms` (`:38,350`): the host owns
  the scan thread.
- Options pane (`render_options`, `:1059`): 3 checkboxes `[x] Tool calls and
  output / Thinking / Pane scrollback` (`SEARCH_OPTIONS`, `:451`), own key
  table `SEARCH_OPTION_KEYS` (`overlay.rs:197`), Esc backs to the list.
- History: `PastSearch` + `page_past` (`:578`).

**Coupling.** Heavily domain: `ContentKinds`, `Grouping`, harness badge,
`ContentTarget` (`AgentSessionId`/`PaneRef`), status wording, the three
option names. Verdict for S9+S10: **split into generic `SearchPalette` +
Conspectus adapter** (section 6).

### 2.9 Search view rows and chips (S11)

The Search *view* is a left-pane view; its hit rows are `RowKind::Hit | Pane
| PaneHit | Mixed` rendered through the left-pane row grid
(`left_panel.rs:1045-1056` dispatch; `render_hit_spans` `:1385`,
`render_pane_spans` `:1431`, `render_pane_hit_spans` `:1464`,
`append_content_hit` `:1357`, `append_snippet` `:1499`). Fixed widths:
session label 12 (`FLAT_LABEL_WIDTH`), pane label 16, `turn {:<4}`, role
`{:<6}`, kind `{:<11}`, age `{:>4}`. A fourth rendering of the same hit lives in
the right panel's "match" header (`right_panel.rs:870-918`) and a fifth in the
header chip `find:"q" 1 session` (`header.rs:338`, 24-char query cut).
`docs/tui-columns.md` is the living spec for the grid; one consistency test
checks alignment (`search_flat.rs:388`).

So one search hit has **three column grids**: modal content row (2.8), Search
view row (this), and the preview/detail header. They share the data
(`Hit`, `highlight`) and the painter idiom, not code.

### 2.10 Find prompt and text entry (S12)

Hand-rolled `FindPrompt { text, cursor }` (`find.rs:46-110`: insert, backspace,
delete, left, right, home, end, clear-to-start) driven by `ViewerFindMsg`
(`find.rs:27`, 10 variants) and the table `VIEWER_FIND_KEYS` (`overlay.rs:691`:
Enter confirm, Esc cancel, Ctrl-C cancel, Backspace, Delete, arrows, Home/End,
Ctrl-U clear; any other `typed_char` inserts). Drawn by `draw_find_prompt`
(`widget.rs:492`), shared by S1 and S2. Compared with `TextInputState`
(`widgets/input.rs`, wraps `tui_input::Input`; the module doc credits it with
Unicode cursor math, word navigation, and paste handling, `input.rs:1-8`;
popup with REVERSED block cursor and visible-window slide, `input.rs:149-200`): two separate single-line editors,
two cursor-scrolling algorithms, and in S9/S10 a third rendering (no caret).
Verdict: **merge into one `LineEdit`** (state + key table + render with
caret) and have the find chip, quick select, rename, and forms use it.

### 2.11 Primitives

**Text wrapping family (6 implementations).**

| # | Where | Granularity | Used by |
|---|---|---|---|
| 1 | `viewer/render.rs:554,565,626` `word_wrap`, `word_wrap_with_budgets`, `hard_break` | `String` | help rows/footers/legends (`help.rs:183,559,603,634,647`), `popup_frame::wrapped_lines` (`:99`), render of thinking/tool bodies |
| 2 | `viewer/render.rs:649` `wrap_styled_line` | spans | markdown output in S1 |
| 3 | `tui/preview_wrap.rs:165,196,209` `word_wrap`, `hard_wrap`, `truncate` | per-cell with style | pane capture |
| 4 | ratatui `Paragraph::wrap(Wrap{trim:false})` | lines | value modal, messages detail, preview text, S1 empty banner |
| 5 | `popup_frame::wrapped_lines` | wrapper over #1 + indent | search footer |
| 6 | `ui.rs:341` `wrapped_line_count` | estimate (ceil of width/pane width) | preview scrollbar length |

#6 does not model word-boundary wrapping, so the scrollbar thumb can disagree
with what #4 draws (by reading).

**Ellipsis/truncation family (4+).** `popup_frame::ellipsize` (`:110`; width 0
returns `…`, i.e. 1 cell over budget), `ui/text.rs:6-103` (`truncate_to_width`
== `_strict`, `_middle`, `_no_marker`, `pad_to_width`), `snippet::head`
(`snippet.rs:72`), `output::render::truncate_to_width` (non-TUI), plus a private
width helper `cells()` (`search.rs:1419`) that re-implements `spans_width`
(`left_panel.rs:1525`) for strings.

**Snippet crop (2 algorithms).** `snippet_around` (`viewer/search.rs:272`:
char budget, centers the match, `…` markers, returns `MatchSnippet`) used by
`build_match_line`, the pane scanner (`content_search/panes.rs:420`) and
`snippet_for` (`viewer/search.rs:382`, which also normalizes whitespace/ANSI
while tracking the highlight range); versus `snippet::fit` (`snippet.rs:23`:
**cell** budget, favors 2/3 after-context, word-boundary cuts, strips backticks)
used by `left_panel::append_snippet` and `content_row_line`. Both are pure and
free of `crate::` imports besides the ranges; they should become one module.

**Match painters (7).** `highlight::paint` (`highlight.rs:54`, restyles an
existing styled line char-by-char after a leader, merges runs),
`pane_viewer::highlighted` (`:486`, string -> 3-part spans), `left_panel::append_snippet`
(`:1499`), `search::content_row_line` (`:1273`), `search::build_match_line`
(`:1339`), `search::entity_line` (`:950`), `right_panel.rs:895-918`;
`preview/search.rs::mark_rows` reuses `paint`. Style source: `Theme::match_style()`
(`theme.rs:271`, fg `match_highlight` + BOLD + UNDERLINED) in 8 call sites, except
S1's current turn (REVERSED+BOLD, `highlight.rs:44`) and cursor rows in search
(`add_modifier(REVERSED)` layered on top).

**Matcher.** `find_ci` (`viewer/search.rs:188`): ASCII-case-insensitive, byte
offsets, no allocation; re-exported by `tui/search.rs:16`; about a dozen call sites
(`grep find_ci`). ADR 0147: ASCII only because Unicode folding changes byte
lengths and would break highlight ranges.

**ANSI handling (3 paths).** `ansi-to-tui` for styled preview;
`discovery::tmux::strip_ansi` (`tmux/mod.rs:1080`) for scrollback lines;
`viewer::search::skip_escape` (`search.rs:476`) for snippet normalization.

**Scrollbar.** `scrollbar_layout` + `render_vertical_scrollbar` (left pane,
explorer, preview; `left_panel.rs:152,191`) vs the inline scrollbar in
`HelpBody::render` (`help.rs:266-286`, `ScrollbarState::new(max_scroll + 1)`).
Same look, two code paths. Viewers use a `[ n/m ]` text chip instead of a
scrollbar.

**Popup frame (S15).** `centered_rect` (`popup_frame.rs:43`), `themed_popup`
(`:58`: `Popup::new(body).borders(ALL).title(title).border_style(accent)`),
`LinesBody` (`:75`). Every modal defines a private `*Body` struct that implements
`KnownSize` only to *report the width/height it already computed* so
`tui_popup` re-derives the same centered rect. Callers of `themed_popup`: 11
(`help`, `search`, `value_modal`, `messages`, `input`, `picker`, `controls`,
`multi_select`, `pin_edit`, `new_mux`, `mux_launch`). Sizing rules in this slice
alone: help (`min(100, w-4)`, `h-4`), value (`min(78, w-4)`, 28 rows),
messages (`w-4` in 40..110, `h-2`), search (`85%`, `80%`, 1-cell margin),
text input (`min(60, w-4)`, 3 rows). Margins differ (1 cell vs 2 cells; 1 vs 2
rows).

**Key tables (S14).** `KeyRow<A>::{Bind{key,action,hint}, Note(&str)}`,
`Hint::{Shown{label,verb}, Alias}`, `resolve`, `footer` (verbs merge:
`↑/↓ move`, segments joined with ` · `), `mislabeled` (test helper checking
that a footer label names its key), `typed_char`. 20 `const` tables in
`overlay.rs`. `MODAL_CONVENTION` (`:782`) + `consistency_tests/keys.rs`
enforce "Esc leaves, Enter applies/activates, Space changes (or pages in a
pager)" for 17 modals. Tied to `KeyMatcher`/`key_label` in `keybindings.rs`.
Footer is generated for all modals except S1 and S2 (their rows are all
`Alias`; footers are hand-written strings, `widget.rs:293-295`,
`pane_viewer.rs:542-543`); `tests/overlay_keymap_hygiene.rs` scans only
`src/tui/widgets`, `runtime/overlay_keys.rs`, `modal.rs` for `KeyCode::` and
for literals containing ` · ` plus Esc/Enter/Tab (`:82-89`), so the viewers'
chips slip through.

**Chips.** `tool_chip_label`, `toggle_chip`, `abort_chip_label`,
`hidden_chip_label` (`widget.rs:551-581`): `name·state (key)` form with state
padding for stable width. `choice_segments` (`search.rs:689`): bracketed
selected tab. `[ n/m ]` position chip x3. Find chip x3 (section 3).

## 3. Cross-Surface Comparison

### 3.1 Chrome

| Surface | Frame | Rect | Title | Footer source | Footer style |
|---|---|---|---|---|---|
| S1 viewer | none (full screen) | whole area | header row | hand-built + `find_hint` | `secondary_text`+DIM, chips success/warning |
| S2 pane viewer | none | whole area | header row | hand-built + `find_hint` | same |
| S3/S4/S5 help | `themed_popup` ALL | `help_modal_rect` | ` Help ` plain | `footer(HELP_KEYS)` pinned | `theme.placeholder` modifier |
| S6 value | `themed_popup` | `centered_modal_rect` | accent BOLD ` {label} ` | `footer(VALUE_MODAL_KEYS)` | `secondary_text` color |
| S7 messages | `themed_popup` | inline | accent BOLD ` Messages · N ` | `footer(MESSAGES_KEYS)` | `placeholder` modifier |
| S9/S10 search | `themed_popup` | `modal_size` | tab strip, chosen REVERSED accent | `footer(SEARCH*_KEYS)` wrapped <= 4 rows | `placeholder` modifier |

Four "dim" idioms in one slice: `fg(secondary_text)+DIM` (S1, S2),
`Modifier` from `theme.placeholder` (S3, S7, S9), `theme.divider` modifier
(S7 rule, preview banner rule), `fg(secondary_text)` alone (S6). The viewers
hard-code literal `Modifier::DIM` 12 times (`widget.rs` 7, `render.rs` 2,
`table.rs` 1, `pane_viewer.rs` 2), so overriding `placeholder`/`divider` in
`[tui.theme]` does not reach them. `render.rs:454-459` itself warns that
color+DIM double-dims inconsistently, yet `widget.rs:79-82,124-126,275-277,541-543`
and `pane_viewer.rs:413-417` do exactly that.

### 3.2 Scroll

| Surface | State type | Page | Half page | End | Clamp | Indicator |
|---|---|---|---|---|---|---|
| S1 | `usize` offset + `stick_to_end` | viewport | viewport/2 | sticky | clamped and written back in `draw` | `[ n/m ]` |
| S2 | `usize` | viewport | viewport/2 | max | clamped in `draw` | `[ n/m ]` |
| S3 | `u16` | 8 lines | none | `u16::MAX`, render clamps (state not) | render-only | scrollbar |
| S4/S5 help | `usize` | **transcript** viewport | viewport/2 | `usize::MAX` | clamped and written back | scrollbar |
| S6 | `u16` | 8 | none | `u16::MAX` unclamped | none (by reading) | none |
| S7 detail | `u16` | 8 | none | n/a | none | none |
| S8 preview | `u16` + `i32` match offset | none (J/K = 1) | none | n/a | by anchor | scrollbar (fade-on-fit) |
| S9/S10 lists | cursor index + `compute_scroll` | none | none | wraps (Up on first -> last) | derived | none |

### 3.3 Find

| Aspect | S1 | S2 | S9 quick select | S10 content |
|---|---|---|---|---|
| Entry | `/` | `/` | `/` | `Ctrl-/` or `s` |
| Query editor | `FindPrompt` | `FindPrompt` | `tui_input` (no caret drawn) | `tui_input` |
| Matcher | `find_ci` per turn | `find_ci` per line | `find_ci` per haystack | `find_ci` per turn (scanner) |
| Result unit | turn | line | row/entity | session / message / pane line |
| Next/prev | `n`/`N` | `n`/`N` | arrows, Ctrl-N/P | arrows, Ctrl-N/P |
| Back out | Esc: help -> find -> viewer | same | Esc closes | Esc closes (cancels scan); options pane: Esc backs |
| Query chip | `find "q" 2/5` (16 cells + `…`) | same | n/a | n/a |
| Header chip (app-wide filter) | n/a | n/a | n/a | `find:"q" ...` (24 cells + `…`, `header.rs:338`) |
| Filter-edit gesture | n/a | n/a | n/a | Ctrl-F apply, Ctrl-S keep; `F` clears |

### 3.4 Pager key matrix (all from the `const` tables)

| Key | Help | Value | Messages | Viewer | Pane viewer |
|---|---|---|---|---|---|
| j/k, arrows | scroll 1 | scroll 1 | select entry | scroll 1 | scroll 1 |
| J/K | | | detail 1 | | |
| PgDn/PgUp | 8 | 8 | detail 8 | viewport | viewport |
| Space | PgDn alias | PgDn alias | detail PgDn | PgDn | PgDn |
| Ctrl-D/Ctrl-U | | | | half page | half page |
| g/Home, G/End | top/bottom | top/bottom | first/last | top/end | top/end |
| `G` matcher | `ch('G')` AnyMod | `ch('G')` | `ch('G')` | `UpperChar('G')` | `UpperChar('G')` |
| / n N | | | | find | find |
| ? | closes | | | toggles help | toggles help |
| Esc | close | close | close | step back | step back |
| q | close | close | close | leave at once | leave at once |
| Ctrl-C | Abort (close) | Abort (close) | Abort (close) | Quit | Quit |
| y | | copy | copy | | |
| opener key (`o`, `!`) | | closes | closes | | |
| a | | | | | attach |

About 55 movement rows are repeated across the five pager tables (11, 11, 8
selection rows, 13, 13) plus five `Ctrl-C` rows, all hand-typed.

## 4. Third-Party Crate Assessment

| Crate (pin) | Used where | How | Framework stance |
|---|---|---|---|
| `tui-popup` 0.7 | `popup_frame.rs:36,58` -> 11 widgets | `Popup::new(body).borders(ALL).title().border_style()`; each body implements `KnownSize` to echo precomputed dims | **Wrap privately** inside a framework `ModalFrame` (clear + border + title + rect policy), or drop: the used surface is clear+Block+center, and the `KnownSize` echo is a workaround that makes every modal compute its rect twice. Do not expose its types in the public API. |
| `ratatui-cheese` 0.7 | `help.rs:17` (`Binding`), `multi_select.rs` | `Binding::new(key, desc)` as inert data; the renderer is in-tree ("cheese's short/multi-column modes don't fit") | **Drop for help**; framework defines `HelpEntry`/`Binding` itself (3 fields). Multi-select is another slice. |
| `tui-markdown` 0.3 (+ `syntect` via `highlight-code`) | only `viewer/render.rs:310` | direct `from_str`, then in-tree post-processing (orphan list markers, styled wrap) | **Own behind a cargo feature** (`rich-text`): a `markdown::render(src, width, &theme)` that wraps the crate and owns the post-processing. The syntect weight (single-digit MB, deps doc) argues for the feature gate. |
| `comfy-table` 7 | only `viewer/table.rs:29` | own GFM segmenter (`segment_body`, about 250 LOC) + `render_table` + char-class border dimming | **Own, same feature**: the segmenter is the valuable part; comfy-table is replaceable. Keep the plain-cell v1 limits (ADR 0054). |
| `ansi-to-tui` 8 | `ui.rs:27` import, `preview_pane.rs:447-460` | `text.into_text()` with color-off and parse-error fallbacks | **Thin shim** `ansi::to_text(text, color)` in the framework; 15 lines. Merge the other two ANSI strippers behind it. |
| `tui-input` | `input.rs` | `TextInputState` wrapper | Keep as the one `LineEdit` backend; retire `FindPrompt`. |
| `unicode-width` | everywhere | direct | direct |
| `ratatui` 0.30 `Scrollbar`, `Paragraph`, `Layout` | everywhere | direct | the framework is built on it; expose helpers (`scrollbar_layout`) not wrappers |
| `insta` 1.42 | snapshot tests | direct | framework test harness dependency |

Existing precedent: `src/viewer/` already enforces an "extractable" boundary
with a dependency manifest test (`docs/transcript-viewer-deps.md`,
`viewer_tests.rs:36`). The framework's independence guard can reuse that
pattern (a manifest + test), and ADR 0052 rule 1 (the `Theme` carve-out)
should be superseded by the framework's own theme tokens.

## 5. A Single Pager Control (sketch)

The code already contains the seam: `viewer::widget::body_lines(state, theme,
width) -> &[Line]` is a headless content layout consumed by both the
full-screen viewer and the preview pane, and `PreviewView`/`PreviewWindow` is a
second, independent statement of the same contract.

```rust
// framework (layer 1+2)
pub trait PagerContent {
    type Action: Copy;                       // content-specific key actions
    /// Compose rows for `width`; content owns wrapping/styling/gutters.
    fn layout(&mut self, width: u16, theme: &Theme) -> usize;     // total rows
    fn rows(&self, range: Range<usize>) -> Vec<Line<'static>>;
    /// Find: ordered match ids (turn index, line index, ...).
    fn find(&self, query: &str) -> Vec<usize>;
    fn match_row(&self, id: usize) -> usize;                       // first row of a match
    fn paint(&self, rows: &mut [Line<'static>], first: usize, find: &FindView, theme: &Theme);
    fn header(&self, theme: &Theme) -> Line<'static>;
    fn chips(&self, theme: &Theme) -> Vec<Chip>;                   // toggles, state
    fn keys(&self) -> &'static [KeyRow<Self::Action>];             // t/T/I/H, a, y ...
    fn describe(&self, a: Self::Action) -> HelpEntry;              // generated help
    fn apply(&mut self, a: Self::Action) -> PagerEffect;           // may invalidate layout
}
pub struct Pager<C> { content: C, scroll: Scroll, find: Find, help: HelpState, note: Option<String> }
pub enum PagerEffect { None, Relayout, Close, Request(Box<dyn Any>) }
pub enum PagerMode { Screen /* header,rule,body,rule,footer */, Modal(ModalRect), Embedded /* no chrome, scrollbar */ }
pub enum Anchor { Top, Bottom /* follow newest */, Match }
```

What the Pager owns (all of it duplicated today between S1 and S2, or between
S1 and S3/S6/S7): the five-row chrome, rules, `[ n/m ]` chip, find prompt and
chip, `n`/`N` with wrap notes, Esc stepping (help -> find -> leave), the
sticky-end rule, page/half-page math from the viewport it actually drew,
clamp-on-draw write-back, the footer fit tiers, the generated help page from
`PagerContent::keys()` + `describe()` plus the shared Moving/Finding/Viewer
sections, and the copy-to-clipboard request hook (value modal's `y`).

What a content source provides: rows, match units, header, extra chips, extra
key rows and their descriptions. Concretely:

| Content | Rows | Match unit | Extra |
|---|---|---|---|
| Transcript (Conspectus adapter) | gutter + markdown/table/tools | turn | t/T/I/H, hidden-match count, reveal-on-open |
| Pane scrollback | numbered plain lines | line | `a` attach |
| Value text | wrapped `Text` | none (find optional) | `y` copy, command-line style |
| Help page | generated lines, pinned footer, scrollbar | none | opener-key closes |
| Messages detail | wrapped `Text` | none | selection owned by a List |
| Preview zone (Embedded) | any `Vec<Line>` or pre-windowed | optional | Anchor::Bottom/Match, no keys beyond J/K |

Design points that must be decided (they are visible deltas):

1. Esc = step back vs q = leave: single-level pagers coincide, so adopt S1/S2.
2. Page size: viewport-based everywhere (fixes S3/S6/S7's fixed 8 and S4's
   transcript-viewport paging).
3. Half-page keys everywhere or nowhere (today only S1, S2).
4. Wrapped vs clipped long lines (S2 clips today) and whether to add
   horizontal scroll.
5. Position indicator: `[ n/m ]`, scrollbar, or both (ADR 0076 chose scrollbars
   for panes; viewers never adopted them).
6. Find unit and current-match marker (REVERSED turn vs gutter accent).
7. Find confirm landing: always scroll (S1) vs only when off screen (S2); 1 vs 2
   lines of context.

## 6. A Search Palette Control (sketch)

Generic skeleton present in `widgets/search.rs`: a query line, a scope
strip in the title (`[ Current ] All`), a ranked list with a cursor and
bottom-pinned scroll window, a status row, a generated footer, a secondary
options pane, a debounce contract for slow providers, and an outcome enum.

```rust
pub struct Palette<R: PaletteRow> {
    query: LineEdit, cursor: usize, scopes: Vec<Segmented>, // Current/All, Sessions/Messages
    status: Option<String>, options: Option<OptionsPane>, debounce: Debounce,
}
pub trait PaletteRow { fn cells(&self, cols: &Columns, sel: bool, theme: &Theme) -> Line<'static>; }
pub struct ColumnPlan { /* ordered cells with min/max and drop priority */ }
```

- `ColumnPlan` generalizes `entity_columns`, `content_columns` (drop order
  chip -> source -> age -> shrink label; keep a minimum snippet) and the
  left-pane `render_*_spans` budget logic; today these are three hand-coded
  grids.
- Row painting helpers: cursor mark `> `/`  `, REVERSED cursor row, dim-off-cursor,
  match range in `match_style` via the one painter.
- Debounce/progress/streaming results are host-driven (`take_due_search`,
  `ContentView`); the framework provides the timer state machine
  (`edited_at`, `force`) and a `Progress { scanned, total, skipped, done }`
  status formatter.
- Adapters (Conspectus): `RowId`/`SearchItem` ranking, `EntityIndex`, outcome
  words, `ContentKinds` options, harness badge, node-kind glyph, `Msg`
  mapping, history paging.

Relationship to the left-pane grid: quick select **does not** reuse the
left-pane row renderer (section 2.7). Search results appear in three grids
(section 2.9). A shared `ColumnPlan` + `HitSnippet` would let the Search
view's rows and the modal's content rows come from one definition.

## 7. Theme Tokens By Control

| Control | Tokens used |
|---|---|
| S1 viewer | `secondary_text`, literal DIM, `cwd_mark`, `harness_color(...)`, `success`, `warning`, `pr_merged` (compact chip), `badge`, `match_style` |
| S2 pane viewer | `panel_focus_accent`, `secondary_text`, literal DIM, `success`, `warning`, `match_style` |
| Help (S3-S5) | `panel_focus_accent` (border, key column), `placeholder` (footer), legends: `warning`, `ghost`, `mux_attached`, `mux_unmuxed`, `pr_open/draft/merged/closed`, node-kind colors; literal BOLD |
| Value modal | `panel_focus_accent` (border, title, `$` lines), `secondary_text` |
| Messages | `panel_focus_accent`, `success/warning/error`, `secondary_text`, `selection_active`, `placeholder`, `divider` |
| Search | `panel_focus_accent` (border, `/`, chosen tab), `placeholder`, `match_style`, node-kind style, `pr_open`, harness badge; literal REVERSED/BOLD |
| Preview | `placeholder`, `divider`, `level_style`, `match_style` |

Generic tokens that emerge: accent, secondary text, dim/placeholder, divider,
success/warning/error, match, selection, badge. Domain tokens the viewers
borrow: `harness_color`, `cwd_mark`, `pr_merged`.

## 8. Tests Inventory

| Area | File (tests) | Kind |
|---|---|---|
| Viewer widget/state | `viewer/widget_tests.rs` (20), `state.rs` (4 inline), `input_tests.rs` (17) | buffer render + 8 insta snapshots, reducer |
| Viewer find/highlight/search | `find_tests.rs` (28, 2 snapshots), `highlight_tests.rs` (7), `search_tests.rs` (28) | reducer + snapshots + pure |
| Viewer render/table/model | `render_tests.rs` (16), `table_tests.rs` (13), `model_tests.rs` (11), parsers | pure |
| Viewer boundary | `viewer_tests.rs` | dependency manifest test vs docs |
| Pane viewer | `pane_viewer_tests.rs` (11) | string-grid render |
| Help | `help_tests.rs` (13), `viewer_help_tests.rs` (13, 3 snapshots), `consistency_tests/help.rs` (4) | page content, wrap properties, cross-help frame/style |
| Value modal | `value_modal_tests.rs` (10) | keys + render |
| Search overlay | `search_tests.rs` (12), `search_quick_tests.rs` (21, 6 snapshots at 80/160/220), `search_content_tests.rs` (30, 6 snapshots) | state machine + width-ladder snapshots |
| Search ranking | `tui/search_tests.rs` (11), `search_entities_tests.rs` (10) | pure |
| Snippet | `snippet_tests.rs` (10) | pure |
| Preview | `preview_wrap_tests.rs` (14), `preview/search_tests.rs` (9), `preview_tests.rs`, `git_tests.rs`, `tui_session_preview.rs` (integration) | pure + integration |
| Key tables | `keybindings/overlay_tests.rs`, `consistency_tests/keys.rs` (6: modal convention), `tests/overlay_keymap_hygiene.rs` (2) | table + source scan |
| App-level search | `consistency_tests/search_quick.rs` (18), `search_flat.rs` (15), `search_view.rs` (13), `content_filter.rs` (14), `content_interactions.rs` (10) | domain behavior against `App`; not framework material |

Reusable harness patterns for layer 2: (1) the "every `Modal` variant is
either built or listed with a reason" coverage test (`layout.rs:53-63`,
`keys.rs:72`), (2) `MODAL_CONVENTION` probing key tables by action name, (3)
two-help parity (`consistency_tests/help.rs`), (4) width-ladder snapshots
(80/160/220), (5) the source-scan hygiene guard. The five `search_*`/`content_*`
consistency files test `App` search semantics and stay in Conspectus.

## 9. Duplications And Inconsistencies

Numbered for backlog citation; counts are exact where stated.

1. **Two pagers, one shape.** `PaneViewerMsg` (15) is a subset of `ViewerMsg`
   (18) plus `Attach`: 14 variants identical. `PANE_VIEWER_KEYS` (21 rows)
   and `VIEWER_KEYS` (24 rows) share 20 identical rows. 12 of 15 help
   sentences are identical text (`viewer/help.rs:44-82` vs
   `pane_viewer.rs:84-111`). State fields duplicated: scroll, viewport,
   show_help, help_scroll, note, find_hint, find query/matches/current.
   Duplicated functions: `scroll_help` (`input.rs:184`, `pane_viewer.rs:322`),
   help-section generation (`viewer_help.rs:39` vs `pane_viewer.rs:563`),
   rule drawing (`widget.rs:119`, `pane_viewer.rs:441`), dim style
   (`widget.rs:79`, `pane_viewer.rs:413`), position chip (`widget.rs:259`,
   `widget.rs:373`, `pane_viewer.rs:512`), find chip (`widget.rs:466`,
   `pane_viewer.rs:521-533`), run/step find (`find.rs:195,266` vs
   `pane_viewer.rs:206,230`). Behavior deltas listed in section 2.2.
2. **Three dispatch patterns for modals.** `Overlay` trait impls: 14. The
   transcript viewer is routed through `Msg::Viewer`/`Msg::ViewerFind` with a
   pop/reduce/push in `App::update` (`app.rs:1641-1668`); the pane viewer
   through `Action::PaneViewerKey` -> `handle_pane_viewer_key` ->
   `PaneViewerState::handle_key` (`overlay_keys.rs:558`). The doc at
   `modal.rs:27` ("Every variant here now implements `Overlay`") is stale for
   these two.
3. **Six outcome/effect enums for one idea.** `HelpOutcome`,
   `ValueModalOutcome` (identical `Continue|Close`), `ViewerEffect`
   (`None|Close`), `PaneViewerOutcome` (+`Attach`), `InputOutcome`,
   `SearchOutcome` (8), all mapped by hand to `OverlayOutcome`.
4. **Three single-line editors** (`FindPrompt`, `tui_input` via
   `TextInputState`, `input.rs::visible_window`) and **three renderings**
   (S12 block caret + cell-aware slide; rename popup caret; search query line
   with no caret at all, `search.rs:821-831`). The find prompt's `/` is bold
   default color (`widget.rs:534`); the search `/` is accent colored
   (`search.rs:822`). ADR 0123 binds `Ctrl-C` first in every table to the
   overlay's `Abort`; the find prompt binds it to `Cancel` (leaves the prompt
   only) and both viewers bind it to `Quit`.
5. **Five pager key tables, about 55 repeated movement rows**, with two matcher styles
   for `G` (`ch('G')` vs `UpperChar('G')`), two meanings for `q`
   (close vs leave-at-once), and `Ctrl-D/U` in 2 of 5.
6. **Page size**: fixed 8 lines in S3/S6/S7 (`help.rs:51`, `value_modal.rs:106`,
   `messages.rs:73`) vs viewport in S1/S2. At the minimum help height the
   modal body is 7 rows, so a page of 8 skips one line.
7. **Viewer-help page size uses the wrong height** (by reading):
   `input.rs:185` pages the help by `state.viewport_height` (the transcript
   body, `area.height - 4`) while the help modal's body is `area.height - 4 - 2
   - footer rows`, so each page-down skips at least 3 lines. Same in
   `pane_viewer.rs:323`.
8. **Scroll clamp inconsistency**: S1/S2/S4/S5 clamp and write back during
   draw; S3 clamps in render only (state grows unbounded, so extra `j` presses
   at the end must be undone one by one); S6 does not clamp at all (`G` =
   `u16::MAX` into `Paragraph::scroll`). `u16` scroll state in four places
   (`HelpOverlayState`, `ValueModalState`, `MessagesOverlayState`,
   `App.preview_scroll`) vs `usize` in the viewers (which slice because of the
   65,535 limit, `widget.rs:170-173`).
9. **Hand-written footers in the two viewers** (`widget.rs:293-295`,
   `pane_viewer.rs:542-543`; all viewer table rows are `Alias`) contradict ADR
   0123's "footers cannot advertise a key the handler does not act on"; the
   hygiene guard's heuristic (` · ` + Esc/Enter/Tab) and file list miss them.
10. **Main help is prose; viewer help is generated.** `keymap_sections()`
    is 48 hand-written rows; drift test is one-directional; known prose
    drift: contents blurb cites `Ctrl-L` where the modal binds `S-Tab`
    (`help.rs:423` vs `overlay.rs:409`).
11. **Wrapping x6, ellipsis x4+, snippet crop x2, match painters x7, ANSI
    paths x3, scrollbar code x2, modal sizing rules x5** (section 2.11 for
    each list with locations).
12. **Dim idioms x4 and 12 literal `Modifier::DIM`** in the viewer subtree
    that bypass `placeholder`/`divider` tokens (section 3.1).
13. **Find chip wording and limits**: `find "q" n/m` (footers, 16-cell cut,
    hard-coded literal `16` in `pane_viewer.rs:522-525` and a const in
    `widget.rs:461`) vs `find:"q" ...` (header chip, 24-cell cut,
    `header.rs:340`). Same concept, two spellings and two limits.
14. **Footer fit strategies differ**: S1 four tiers, S2 two tiers, S6 none
    (truncated by the buffer), S9/S10 wrap up to 4 rows, S3 wrap to the modal.
15. **Generic code lives in `src/viewer/` and is imported by `src/tui/`**
    (inverted dependency relative to the ADR 0052 boundary): `word_wrap`
    (`render.rs:554`; 6 callers in `tui/widgets`), `find_ci` / `snippet_around` /
    `MatchSnippet` (`viewer/search.rs`, re-exported by `tui/search.rs:16`;
    callers in `pane_viewer`, `preview/search`, `content_search/panes.rs`,
    `search_entities.rs`), `FindPrompt` / `ViewerFindMsg` (`find.rs`),
    `draw_find_prompt` (`widget.rs:492`, `pub(crate)`), `highlight::paint`
    (used by `preview/search.rs:16`). Conversely `src/viewer/` imports the
    Conspectus `Theme`, which carries domain tokens it then uses
    (`harness_color`, `cwd_mark`, `pr_merged`).
16. **Layout coverage gap**: the two full-screen viewers are `SKIPPED` in the
    "nothing escapes the frame at 5 widths x 2 heights" test
    (`consistency_tests/layout.rs:53-63`); the help page over S4/S5 is covered
    for S3/S4 only (`consistency_tests/help.rs`).
17. **Search results in three column grids** (modal content row, Search view
    row, detail "match" header) and **quick select does not reuse the
    left-pane row renderer** (section 2.9).
18. **Mixed placement of ADR-driven facts in code**: `Grouping`, `ContentKinds`,
    `Hit` (domain search types) live in `viewer/search.rs` next to generic
    `find_ci`/`snippet_around`, so framework extraction cannot move the file
    whole; it must be split.
19. **Stale/unused dependency intent**: `docs/transcript-viewer-deps.md` says
    `ansi-to-tui` renders tool-output ANSI inside the viewer; no code does (tool
    bodies are plain-wrapped, `render.rs:382-438`).
20. **Non-UI file named like a control**: `src/tui/viewer.rs` is the external
    `claude-history` launch plan, not a viewer widget; worth a rename note
    for the extraction (do not move it).

## 10. Open Questions For The Operator

1. **Pager convention**: adopt the viewers' `Esc` = step back, `q` = leave
   at once, `Ctrl-C` = leave for every pager? Help, value modal, and messages
   bind both `Esc` and `q` to plain close, so for them the viewers' convention
   changes nothing visible. And should `Ctrl-C` in the find prompt keep
   cancelling only the prompt?
2. **Half-page and page keys**: Ctrl-D/Ctrl-U everywhere (and maybe Ctrl-F /
   Ctrl-B), with viewport-sized pages in help/value/messages (today 8)?
3. **Long lines in the pane viewer**: keep clipping, wrap like the transcript,
   or add horizontal scroll? This is a visible behavior change for S2.
4. **Scroll indicator**: standardize on `[ n/m ]`, a scrollbar, or both for all
   pagers? (Viewers and ADR 0076 disagree today.)
5. **Find semantics defaults**: unit (turn vs line is content-defined), current
   match marker (REVERSED vs gutter accent), scroll-on-confirm (always vs only
   if off screen), context lines (1 vs 2). Pick one default for the framework.
6. **ASCII-only matching** (`find_ci`, ADR 0147): keep as the framework default
   or support Unicode folding with range remapping?
7. **Rich text scope**: should markdown + syntect + comfy-table live in the
   framework behind a cargo feature, or stay Conspectus-side with the framework
   providing only a `RichText` trait? (Weight: syntect is the largest dep.)
8. **Boundary with the existing `src/viewer/` extraction plan (ADR 0052)**:
   supersede its "extractable transcript-viewer crate" rule with the framework
   split (generic Pager to the framework; model/parsers/transcript adapter stay
   or become their own crate)? Needs an ADR.
9. **Help source of truth**: generate the keymap help rows from `KEYBINDINGS`
   (it already has `help_text`) and drop the hand-authored prose, accepting
   shorter or more uniform descriptions? The four glyph legends would become
   app-provided `LegendSection`s.
10. **Query-line caret in quick select** (currently none drawn): adopt the find
    prompt's block caret as part of unifying the editors?
11. **Theme token cleanup**: should the viewers' literal DIM and the three
    borrowed domain tokens (`harness_color`, `cwd_mark`, `pr_merged`) be routed
    through new generic tokens (`dim`, `divider`, `accent2`, ...)? This changes
    what `[tui.theme]` overrides affect.
12. **Value modal `G`**: confirm whether it renders blank today (by reading it
    does not clamp); if so it is a bug to fix during extraction rather than a
    behavior to preserve.

## 11. Framework Verdict Summary

| Candidate base control | Source(s) | Verdict |
|---|---|---|
| `Pager` (+ `PagerContent`) | S1, S2, S3, S4, S5, S6, S7 detail, S8 | split into generic core + Conspectus adapters (merge 7 implementations) |
| `HelpPage` + `HelpGenerator` | S3, S4, S5, `render_help_page`, `binding_lines`, `legend_entry` | extract after refactor; content stays |
| `ScrollRegion` / `Scrollbar` helpers | S8, `left_panel.rs:152,191`, `help.rs:266` | extract after refactor (one code path) |
| `AnsiText` (+ wrap modes) | `render_captured_pane`, `preview_wrap.rs` | extract after decoupling `PreviewWrap` |
| `RichText` (markdown + tables) | `viewer/render.rs`, `viewer/table.rs` | extract behind a cargo feature; wrap tui-markdown/comfy-table |
| `SearchPalette` | S9, S10 | split into generic core + Conspectus adapter |
| `LineEdit` | `FindPrompt`, `TextInputState`, `draw_find_prompt`, `visible_window` | merge (extract after refactor) |
| `Snippet` + `MatchPaint` + `find_ci` | `snippet.rs`, `snippet_around`, 7 painters | merge into one module; extract as-is after merge |
| `text` (wrap, ellipsize, width) | six wrappers, four truncators | merge |
| `ModalFrame` + sizing policies | `popup_frame.rs`, 5 rect fns | extract after refactor; wrap or drop `tui-popup` |
| `KeyMap<A>` (rows, resolve, footer, help) | `overlay.rs` generic parts | extract as-is (rehome `KeyMatcher`/`key_label`) |
| `ModalConvention` harness | `MODAL_CONVENTION`, `keys.rs`, `help.rs`, `layout.rs` patterns | extract the pattern to layer 2 |
| `ToggleChip` / `SegmentedTitle` / `PositionChip` / `FindChip` | `widget.rs:551-581`, `search.rs:689`, three position chips | extract after refactor (shared with chips slice) |
| Transcript model, parsers, gutter chips, tools phrase | `viewer/*` domain half | stays in Conspectus |
| Preview sources/loaders/store/banner/search derivation | `tui/preview/*`, `preview_pane.rs` | stays in Conspectus |
| Messages log, entity index, outcome words, content search state | various | stays in Conspectus |
| `src/tui/viewer.rs` | external viewer launch | stays in Conspectus |
