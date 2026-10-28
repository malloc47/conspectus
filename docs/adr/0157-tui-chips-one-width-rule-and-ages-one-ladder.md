# ADR 0157: Chips Share One Width Rule, And Ages One Ladder

## Status

Accepted. Applies house rules 13, 14, and 18 of ADR 0151 and extends
ADR 0150 with the framework's `chip` and `age` modules. Amends ADR 0149
item 25 (the content search's chip column is a chip's footprint from the
theme, not a fixed 10 cells). Keeps ADR 0032's chip contract (every chip
is `badge_width + 2` cells) and makes it hold for every chip.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/`) found the TUI drawing chips and
ages in many ways that shared no code:

- **Six chip forms.** The harness and program pills (`badge.rs`), the
  plain ` no agent ` span that stood in for a pill, the dim `N hits` and
  `was in <mux>` notes, the viewers' `name·state (key)` switches with
  hand-padded states and three copies of the `[ n/m ]` position text, the
  search title's chosen word with a hard-coded `BOLD | REVERSED` instead
  of the theme's chip look, and the status bar's spinner, warning, and
  provider chips, each styled inline. "Chip" meant five different things
  in the chrome alone.
- **One width rule ignored.** The content search sized its chip column
  with a constant 10, so a theme with another `badge_width` ran the
  session's label into its chip or left a gap, and pushed the snippet
  past the frame. The ` no agent ` span was 10 cells only by the length
  of its words.
- **Four TUI age formatters and the CLI's.** `format_recency` (s/m/h/d,
  clock skew as `0s`) behind about 25 callers, its thresholds and the
  recency buckets (5 minutes, an hour, a day) as literals in the row
  module; the explorer's `relative_epoch` adding ` ago`; the search
  status's `age_label` (`just now`, and hours with no days, so a day-old
  scan read `25h ago`); and the Clean up list borrowing the CLI's
  `format_relative_age`, which goes on to weeks, so a record the History
  row called `21d` read `3w` one keypress later. (The legacy detail
  header's `format_elapsed` had already gone with that header.) ` ago`
  was appended by hand at seven sites.
- **One age cell, not shared.** Session, mux, PR, and mixed rows drew the
  tinted, four-cell age cell; the Search view's match, pane, and
  pane-line rows printed the same text right-aligned in gray, untinted
  and uncut.

## Decision

1. **`tuikit::chip` owns five chip forms.** `Chip`, a filled pill of a
   fixed width; `ToggleChip`, `name·state (key)` with the state padded to
   the widest word it can show; `SegmentedTitle`, `[ One ] Two`;
   `PositionChip`, `[ n/m ]`; and `StatusChip`, a short status at its
   natural width in a `Tone` (muted, inert, accent, success, warning,
   error), optionally led by a mark or a busy spinner frame
   (`spinner_frame`, a frame every 120 ms of elapsed time). The middle dot
   between a switch's name and state becomes the mark `marks::SEPARATOR`.

2. **One width rule.** A filled chip is the theme's `badge_width` (at
   least 2) in terminal cells, cut with the ellipsis mark or padded, inside
   one cell of padding on each side, in one span so the padding carries
   the fill: `chip::width(theme)` cells, always. A surface that reserves a
   column for chips sizes it with `chip::width`; the content search does.

3. **Looks come from the theme.** A filled chip is the theme's `badge`
   modifier over a color the caller picks, or a style the caller supplies.
   The segmented title's chosen word is `accent` under `badge` (the old
   hard-coded `BOLD | REVERSED` is the default `badge`). An on switch is
   bold `success`, an off one `inert` under `muted`. The position chip is
   bold.

4. **The host keeps what chips mean.** `harness_badge` (a `Chip` in the
   harness color), `command_badge` (the program chip, a `Chip` in the
   host's `command_badge` styles), `no_agent_chip` (a muted `Chip`, so it
   keeps the column at any width), `hits_chip` and `was_in_chip` (inert
   `StatusChip`s two cells after the label) stay in
   `src/tui/widgets/badge.rs` as thin adapters. The viewers' switches, the
   search title's choices, the find chips, and the status bar's busy,
   unseen-messages, stale, ambiguity, and provider chips are built on the
   forms at their sites.

5. **`tuikit::age` owns ages.** `Age` is whole seconds, made from two
   Unix times (`Age::between`, a time in the future is 0) or from two that
   may be unknown (`Age::of`). A `Ladder` of `Unit`s writes an age in the
   largest unit it has reached, rounded down; `Ladder::DAYS` (s, m, h, d)
   is the default and `Ladder::WEEKS` goes on to weeks; a host passes its
   own units to `Ladder::new`. `ago` adds ` ago`, the one prose suffix.
   `Recency` (fresh, active, recent, cold) sorts an age by a
   `RecencyScale` whose bounds a host may replace (default 5 minutes, an
   hour, a day) and styles each step from `recency_*`. `age::cell` draws
   an age right-aligned in a fixed width (`CELL_WIDTH`, 4), cut with the
   ellipsis mark, the no-value mark when unknown, tinted by its recency.

6. **One ladder in the TUI.** `format_recency`, `recency_bucket` and
   `RecencyBucket`, `relative_epoch`, and `age_label` are removed; every
   age the TUI shows uses the default ladder, as a cell or as prose. The
   Search view's match, pane, and pane-line rows draw the shared age
   cell. There is no `just now`.

7. **The CLI shares the formatter and the thresholds, not the top unit.**
   `output::render::format_relative_age` writes `Ladder::WEEKS` and keeps
   `now` for a time in the future, so its output does not change. The two
   ladders are the same below a week, so a session reads `3h` in a table
   and in the tree. The TUI keeps days as its largest unit: its age column
   is four cells and fits `999d`, `docs/tui-columns.md` documents days,
   and the tree's ages are read against each other, where `9d` and `12d`
   say more than `1w` and `1w`. The CLI keeps weeks: its tables have no
   fixed age column, old history records read shorter, and changing the
   unit would change every table and the `history` and `rebuild` lists for
   no gain inside the CLI.

8. **Left as they are**, each for a reason: the find chips' text and query
   budget (the `Pager` extraction unifies the two viewers' find); the
   status bar's settings segments (`group:`, `filter:`, `sort:`, `find:`),
   which the `PriorityLine` story takes; the explorer's one-cell kind
   chips and the section rule's chip label (the frame and property grid
   stories); the transcript gutter's role chips (domain); and
   `format_duration_for_input`, which writes a duration for editing, not
   an age.

## Consequences

- Default frames are unchanged except the ages listed here: the
  before-and-after snapshot frames of every view, the Search rows, the
  search overlay, the viewers, the header, and the status bar at 60, 100,
  130, and 160 columns differ only in the Search rows' age tint and the
  search status's `scanned 0s ago` (it said `just now`). With another
  `badge_width` the content search's chip column and the ` no agent `
  chip follow it, so labels after them line up.
- Visible: the Search view's match and pane ages are tinted like every
  other age (a fresh pane reads bright green); the search status reads
  `0s ago`, and a scan a day old `1d ago`, instead of `just now` and
  `25h ago`; Clean up counts days (`21d ago`) like the History rows, not
  weeks (`3w ago`).
- The CLI's output is byte-identical; its table tests keep the week
  boundary and `now`.
- A new chip or a new age on any surface uses the forms and the ladder;
  the thresholds are values, not literals in row code.

## Alternatives Considered

- **One ladder with weeks everywhere.** The CLI unchanged, but every TUI
  age over a week changes (`9d` becomes `1w`, `12d` becomes `1w`), and the
  tree loses the day precision its age column has room for.
- **One ladder without weeks everywhere.** The TUI unchanged, but every
  CLI table and list that shows an old record changes, and the CLI's
  fixtures with it, for no gain inside the CLI.
- **Keep `just now`.** It reads warmly, but it is a fourth wording for a
  fresh age; every other prose age says `0s ago`.
- **Centered chip labels.** The option exists (`Chip::align`), but every
  chip today is left-aligned, and a centered column of names reads
  ragged; left stays the default.
- **One chip type with variants.** A single enum for filled, switch,
  title, position, and status chips would share a builder but force every
  form to carry fields only one needs; five small types each have one
  rule.
- **A host-wide spinner clock.** A frame counter in the App would animate
  the spinner deterministically, but it is state the draw must advance;
  deriving the frame from elapsed time keeps the chip pure.

## Open Questions Answered

- *Do the TUI and the CLI share one unit ladder?* They share the
  formatter and every threshold below a week; the CLI goes on to weeks and
  the TUI stops at days (item 7).
- *Is `ago` a caller concern?* It is the ladder's `ago`, so every surface
  writes it the same way.
- *Keep `just now`?* No (item 6).
