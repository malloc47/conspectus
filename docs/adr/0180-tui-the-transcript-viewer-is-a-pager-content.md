# ADR 0180: The Transcript Viewer Is A Pager Content, And The Viewer Boundary Keeps Only The Transcript

## Status

Accepted. Carries out what
[ADR 0177](0177-tui-readers-are-one-pager-over-content-the-host-lays-out.md)
left for the transcript viewer, applying house rules 2, 4, 8, 12, 13, 14,
and 18 of [ADR 0151](0151-tui-framework-control-set-and-house-rules.md).
Narrows the extraction-ready boundary of
[ADR 0052](0052-native-session-viewer.md): its rule 1 carve-out for
`crate::tui::theme` is gone (the viewer draws from the framework's theme),
the framework crate is a permitted dependency of `src/viewer/`, and the
full-screen modal's chrome, scrolling, find prompt, key handling, and help
page leave the module for the framework's pager; the model, the parsers,
the rendering of turns, the find unit, the toggles, and the bridge stay.
Amends [ADR 0161](0161-tui-overlays-draw-themselves-on-one-host-owned-stack.md)
decision 7: the transcript viewer is no longer a nested reducer but the
framework's pager on the stack, and `Msg::Viewer` and `Msg::ViewerFind` are
gone. Amends [ADR 0162](0162-tui-help-pages-one-page-generated-from-key-tables.md)
decisions 6 and 7 and [ADR 0178](0178-tui-help-pages-and-long-values-are-pager-contents.md)
decision 6: the viewer's help page is the pager's `?` level, generated from
`READER_KEYS` and the transcript's table, and the interim `viewer_help`
adapter is removed. Amends [ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md):
`VIEWER_KEYS` and `VIEWER_FIND_KEYS` are gone; the viewer resolves
`READER_KEYS`, then its own `TRANSCRIPT_KEYS`, and its prompt `FIND_KEYS`.
Amends [ADR 0153](0153-tui-theme-roles-aliases-and-named-styles.md): the
host registers one more role, `transcript_compaction`, and the viewer no
longer reads `harness.codex`, `cwd_mark`, or `pr_merged`.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/05-search-help-viewers.md`, sections
2.1, 5, and 9) found the transcript viewer to be the last reader with its
own chrome, after the pane viewer, the value modal, the help page, and
Messages moved onto the pager (ADRs 0177, 0178, 0179). What it still kept:

- **Its own reader.** A header, two rules, and a footer with four
  hand-written fit tiers; a scroll offset with a sticky-end flag that any
  key but `G` cleared, and viewport and line counts the draw wrote back
  into the state; a find over its own `FindState`, prompt included; and
  `Esc`/`q` stepping coded in its reducer.
- **A nested reducer.** Keys reached it as `Msg::Viewer` and
  `Msg::ViewerFind`, whose reducer arms popped the entry, ran the viewer's
  reducer, and pushed it back.
- **Hand-written keys.** `VIEWER_KEYS` was all aliases, because the footer
  was written by hand, and the help page was a pager held in the viewer's
  state (the interim of ADR 0178 decision 6), routed by its reducer.
- **Borrowed theme roles.** `src/viewer/theme.rs` re-exported the host's
  theme (ADR 0052's carve-out), and the viewer drew user turns in the codex
  harness's color, the cwd in the launch-context mark's, and compaction
  summaries in the merged-PR color.
- **The Preview leaned on all of it.** The right pane's transcript preview
  held a whole `ViewerState` to borrow its composition and its find's
  matches.

The pager's content trait was sketched with this port in mind (its
`describe` and `find_detail` hooks), and the viewer's rows were already a
headless layout (`body_lines`) the Preview drew from.

## Decision

1. **The transcript is the content.** `crate::viewer::state::Transcript<M
   = ()>` holds the document, the four toggles, the layout cache (keyed by
   the width and the toggles), and the named style of the title, and
   implements `tuikit::pager::PagerContent` in `crate::viewer::widget`:
   - `layout` composes the rows and keeps them; `rows` slices them.
   - `header` is ` <harness>:<key> · <cwd> · N turns`, or the title alone
     when the rest does not fit.
   - `chips` are the toggles' chips, most important first: tool detail,
     thinking, aborted turns, and hidden records only for a transcript
     whose parser marked turns hidden.
   - `find` is the shown turns that hold the query, through the shared
     content matcher; `match_rows` is a turn's rows up to the next turn's
     (the turns of a tool run collapsed to one summary line share it);
     `paint` picks matches out after the gutter and never on a collapsed
     summary; `find_detail` counts the matches the toggles hide (`2 in
     hidden turns`).
   - `describe` keeps the viewer's words for `g`/`G` and `/`; `empty` says
     `transcript unavailable`, or `every turn here is hidden` for a
     transcript whose turns the toggles all hide.
   - `apply` flips a toggle and answers `Relayout`, so the find stays on
     its turn while it is still shown.

   `M` is the message of the pager the transcript sits in, as
   `HelpPage<M>`'s is; the transcript sends none.

2. **The transcript's keys are its own.** `TranscriptKey` (`t` tools, `T`
   thinking, `I` aborted, `H` hidden) and `TRANSCRIPT_KEYS` live in
   `crate::viewer::keys`, outside the TUI's overlay tables, since the
   viewer imports nothing from `crate::tui`. They are aliases (each chip
   names its key) resolved after `READER_KEYS`, and their help entries are
   a section of their own, `Showing more or less`, placed before the
   pager's last.

3. **Opening.** `crate::viewer::find::viewer(transcript, open)` is the pager
   at the end. With a search result, `land` finds the turn as before (the
   remembered turn, then within 25 turns of it, then anywhere, on the raw
   bodies), switches on the toggles that show it, lays the rows out again,
   calls `Pager::land`, and notes what it switched on (`showing full tool
   output`, with `the transcript changed; nearest match` when it moved) as
   info, or that the query is gone as a warning.

4. **The TUI holds the pager.** `Modal::Viewer(TranscriptViewer)`, where
   `TranscriptViewer = Pager<Transcript<Msg>>`, is an overlay for the App as
   every pager is; the bridge builds it with the title's role,
   `harness.<key>` (`tui::theme::harness_role`). The entry has no impl of
   its own and sends no message.

5. **The theme.** The viewer reads `tuikit::theme::Theme`. Roles any program
   has take tokens: rules, separators, and the turn count the inert tone; a
   user turn's chip `accent`; the cwd `accent` with `muted`; an assistant's
   `success`; a tool's `warning`; thinking and system turns `inert`; the
   current match `selection_active`. The transcript's own role is a named
   style the host registers: a compaction summary's chip reads
   `transcript_compaction` (magenta, the color it had), falling back to
   `link`; the title reads the role the bridge hands it, falling back to
   `accent`. Default frames are unchanged.

6. **One painter.** `tuikit::text::paint_matches` paints a query's
   occurrences after a gutter; the pager's `Find::paint`, the viewer's
   `highlight::paint_turns`, and the Preview's pane rows draw through it.
   `paint_turns` serves the pager's find and the Preview's `Marks`, and the
   Preview holds a `TranscriptPreview` (the content and the marks of the
   search on display) instead of a `ViewerState`. The Preview keeps its
   own window and anchoring until its scroll region moves to the framework.

7. **The boundary.** `src/viewer/` imports nothing from `crate::tui`, with
   no carve-out, and depends on `tuikit` (listed in
   `docs/transcript-viewer-deps.md`). What every reader shares is the
   framework's; what is the transcript's stays in the module: the model,
   the parsers, the rendering of turns and tables, the layout as rows, the
   find unit and the hidden count, the toggles and their keys, opening at
   a search result, and the content matcher. The extractable unit of ADR
   0052 is now that content over the framework: a standalone binary would
   run `Pager<Transcript>` itself rather than carry a reader of its own.

## Consequences

- **Visible changes in the transcript viewer** (the CSP-748 summary lists
  them): the footer is generated (`[ n/m ] · tools·off (t) · think·off (T)
  · aborted·hide (I) · j/k/PgDn/PgUp scroll · / find · ? help · Esc/q
  close`) and, when narrow, drops the scroll keys, then the find and help
  keys, then the chips from the last, keeping `Esc/q close` (a 40-column
  footer used to lose the close hint); a toggle at the end keeps the view
  at the end; hiding the current match's turn leaves the find on no match
  until `n` or `N`; the help page lists Finding before the toggles and
  takes the pager's words for `Esc`; a note is a chip of its own after the
  find chip, and a gone query's note is a warning; a transcript whose
  turns are all hidden says so. The header, the rows, their colors, and
  find, its landing, stepping, wrap notes, and hidden count are as before.
- The transcript viewer and the pane viewer page, find, step back, leave,
  explain their keys, and fit their footers by one implementation.
- A theme that set `harness.codex`, `cwd_mark`, or `pr_merged` no longer
  recolors the viewer; `accent` and `transcript_compaction` do.
- **CSP-342** (lift `src/viewer/` into a workspace member crate) changes
  shape: the crate would depend on `tuikit` and carry no reader, chrome,
  or key handling, and its binary would draw `Pager<Transcript>`. The
  closing assessment (CSP-766) re-scopes it.

## Alternatives Considered

- **An adapter in `src/tui`, as the pane viewer's.** The content trait
  could have been implemented beside the TUI's overlay tables, with `Msg`
  as its message. It keeps `tuikit` out of `src/viewer/`, but an extracted
  viewer would then have no viewer at all, only rows; the content belongs
  with the transcript.
- **A content with no message and a host wrapper.** `type Message = ()`
  with a TUI newtype mapping the pager's outcomes onto `Msg`. It costs an
  overlay impl and an accessor layer for a content that sends nothing;
  `HelpPage<M>` already set the type-parameter pattern.
- **Keep `ViewerState` around the pager.** A wrapper holding the pager
  and the toggles would keep call sites, but two states for one viewer is
  what the extraction removes.
- **The compaction chip on a generic token** (`link` or `accent`). No new
  key, but the chip changes color and collides with the user turn's
  accent; reading `pr_merged` by name keeps borrowing a PR state.
- **The Preview as an embedded pager now.** The Preview's anchoring (the
  newest turns, or the hit's turn) is its scroll region's, which moves to
  the framework in its own story; it takes the content and its marks now.

## Open Questions Answered

- *Where does the transcript's pager content live?* In `src/viewer/`, over
  the framework, generic over the pager's message.
- *Which keys are the viewer's own?* The four toggles; every other key is
  the pager's.
- *What happens to ADR 0052's boundary?* It is narrower: the reader is the
  framework's, the transcript is the viewer's, and the theme carve-out is
  gone.
- *Which colors are the viewer's?* None of its own but the compaction
  chip's named style; the rest are the framework's tokens and the title's
  role the host hands it.
