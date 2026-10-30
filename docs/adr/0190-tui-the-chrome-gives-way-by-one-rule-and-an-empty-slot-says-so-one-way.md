# ADR 0190: The Tab Strip, The Header, And The Status Bar Give Way By One Rule, And An Empty Slot Says So One Way

## Status

Accepted. Carries out the `TabBar` and `PriorityLine` part of
[ADR 0188](0188-tui-the-explorer-is-a-framework-control-over-a-host-source.md)
decision 6 (one fit rule for pieces of a line) and extends
[ADR 0150](0150-tui-framework-workspace-crate.md) with the framework's
`tab_bar`, `priority_line`, and `empty_state` modules, `text::elide_filled`,
a gap per `text::Piece`, and `marks::MORE`. Takes the status bar's
settings segments that
[ADR 0157](0157-tui-chips-one-width-rule-and-ages-one-ladder.md) decision 8
left for this story. Applies house rule 14 of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md) to the header,
which had no fit. Keeps the header's content and order of
[ADR 0078](0078-tui-surface-division-of-labor.md) and gives it a narrow-width
order. Decision 6's hints, still hand-written here, are generated from the
key tables by [ADR 0191](0191-tui-the-status-bar-names-the-keys-of-the-scope-that-has-them.md).

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/07-chrome-and-explorer.md`, sections
3.4 to 3.6, 3.14, 6, and 9) found three lines of pieces that give way, each
with its own fit, and a dozen empty-state messages with no shared code:

- **The tab strip.** CSP-716 gave it a fit of its own (`tab_strip_fit`):
  every name when they fit, else the inactive names give way from the end of
  the strip, the active one stays, and the strip ends with ` …` and a free
  cell. ADR 0188 decided it would move onto `text::elide` with the
  breadcrumb.
- **The header.** One bold paragraph of counts, then optional harness chips
  and a `▲ N` triage chip, each set apart by `  ·  `. It had no fit: at 40
  columns the edge cut `4 stopped` to `4`, and with `show_harness_chips` on
  a chip was cut mid-name below 115 columns (the header's width on the
  showcase fixture).
- **The status bar.** Its settings and hints were packed by priority by
  hand: in order of priority, keep each piece that fits beside the kept
  ones; when one was left out, pack again into two cells less and add ` …`.
  That fills gaps: a narrow hint of low priority shows where a wide one of
  higher priority did not fit. `text::elide`, which CSP-790 extracted for the
  breadcrumb, gives way strictly by rank and does not, so at 107 columns
  the bar would lose `Tab focus`. The CSP-790 review asked this story to
  accept the strict rule or add a fill pass.
- **Empty states.** The left pane's placeholder copy (15 messages today,
  about 12 when the inventory counted), the right pane's two, and the
  Preview's stand-ins for content that is not there were plain strings at
  their sites. The left pane's went through the tree's placeholder (wrapped,
  muted); the right pane's was a muted paragraph that did not wrap; the
  Preview's were plain text, indistinguishable from content.

## Decision

1. **A piece may have its own gap.** `text::Piece` gains `gap`: the cells
   before the piece when an item precedes it, in place of the line's
   separator; a mark in a gap takes the separator, and the first item has
   nothing before it. A line whose pieces are not all joined alike (the
   header's `  ·  ` before a chip section, `  ` between chips, ` · ` between
   counts) is then measured exactly.

2. **The fill pass is added, not refused.** `text::elide_filled` applies
   `elide` and then brings back each piece that gave way, highest rank
   first, while the line still fits with it. Pieces of one rank come back in
   order, the earlier first, and once one does not fit the later ones of
   that rank stay out, so the kept pieces of each rank are always its first
   ones, as `elide` leaves them. With distinct ranks the result is the
   status bar's packing by priority exactly. A line whose pieces read in
   sequence (a breadcrumb, a tab strip) keeps the strict `elide`: filling
   would show a later piece past a gap the mark does not place. The visible
   effect of the choice is none: the status bar draws as before at every
   width. The strict rule would have shown one hint fewer at some widths
   (`Tab focus` at 107 columns, left pane focused).

3. **`tuikit::tab_bar::TabBar`.** Named tabs, one active (`accent`, bold),
   the rest `inert`, joined by `marks::SPACED_SEPARATOR`, on `text::elide`
   with the mark at the end: every tab while they fit; otherwise the
   inactive tabs give way from the end of the bar (one rank, the later
   first) and the active one never; a shortened bar ends in `marks::MORE`
   (` …`) with one blank cell kept after it, so the mark stays off the
   pane's corner; and when the active tab and the mark do not fit, the
   active tab shows alone, cut with `…` only when it alone is too wide. This
   is CSP-716's rule, so the strip draws as before. `marks::MORE` is the
   one line-end mark.

4. **`tuikit::priority_line::{PriorityLine, Segment}`.** Segments of
   styled spans (a `StatusChip` among them), each with a rank or none and
   an optional gap, joined by a separator (` · ` in `muted` by default), on
   `text::elide_filled` with ` …` (in `muted`) at the end. A segment with no
   rank never gives way and is cut last. The parts of a line that must
   never move (the status bar's unseen, busy, scope, stale, and provider
   chips) stay spans the host draws around the line, giving it the cells
   they leave. `StatusChip` stays in `tuikit::chip`, where CSP-725 put it.

5. **The header is a `PriorityLine` in Conspectus.** Its composition stays
   in the host. The session count has no rank; then, from the last to go,
   freshness (5), the mux count (4), the `▲ N` triage chip (3), the stopped
   count (2), and the opt-in harness chips (1, one segment each, from the
   end), following what ADR 0078 says the header carries first. The counts
   and their separators stay bold, the chip sections keep their `  ·  ` and
   `  ` gaps, and the mark is the muted ` …`. A header that fits draws as
   before.

6. **The status bar's segments are a `PriorityLine`.** The settings
   (`group:`, `filter:`, `sort:`, `find:`, in `cwd_mark`) and the hints (in
   `muted`) take named ranks with no ties that keep the old priorities'
   order: find, back, quit, filter, the right pane's keys, the default
   action, group, sort, the left pane's keys, `Tab focus`, refresh. With
   decision 2 the bar packs them as before. The hints themselves are still
   hand-written; CSP-759 generates them from the key tables on this line.

7. **`tuikit::empty_state::EmptyState`.** A message and an optional hint
   on rows of their own, wrapped by `text::wrap`, in the `muted` look,
   left-aligned from the slot's corner (the default) or centered across and
   down it; drawn in an area (which takes the look) or returned as lines.
   `TreeView`'s placeholder is one. In Conspectus every empty state uses it,
   with its copy unchanged: the left pane's 15 messages (each message and
   its second line become the message and the hint), the right pane's two,
   and the Preview's stand-ins for content that is not there (loading, no
   preview, no mux target, no turns, an empty pane, live preview off). A
   Preview that failed (`unavailable`, `unreadable`, `not found`, a loader's
   error) reports a reason; that is content and keeps the body's look.

8. **No minimum-size screen.** CSP-319 and CSP-190 stay open. The seam is
   named on the app frame's and the empty state's pages: one centered
   `EmptyState` over the whole screen below a size the host names.

## Consequences

- **Visible changes.** Of 329 before-and-after snapshot frames (the
  header, the status bar, and the whole screen in every view, both focus
  states, a search with no hits, a content filter with no hits, a filter
  that hides every row, the empty Sessions, PRs, Pinned, and History views,
  a group with no node, the Mux view's preview placeholders, a status
  message, and the header with harness chips, each at 40, 60, 80, 100,
  107, 120, and 160 columns), 273 are byte-identical. 38 differ in the
  header only: at 40 columns it ends `4 mux …` where the edge cut
  `4 stopped` to `4`, and with harness chips on it drops whole chips from
  the end and keeps `▲ N` (`claude 5  ·  ▲ 4 …` at 80 columns, where it
  cut `codex 4` and dropped the rest). 18 differ in style only: the Mux
  view's `loading mux preview…` and `no mux target for this row` are
  dimmed. The tab strip, the status bar, and the panes' empty states are
  byte-identical everywhere.
- The right pane's empty state wraps on a pane too narrow for it (below
  about 34 columns) instead of being cut by the border.
- One rule, ranked differently, now decides what gives way on every line
  of pieces: the breadcrumb and the tab strip strictly, the header and the
  status bar filled back.
- A new empty state is `EmptyState::new(..).hint(..)` at its site; a new
  line of droppable parts is a `PriorityLine` with ranks.

## Alternatives Considered

- **Accept the strict rule for the status bar.** One behavior for every
  line, but a narrow bar would show fewer hints than fit (`Tab focus` at
  107 columns) for no gain the operator can see. Rejected; the fill pass is
  one function over the same rule.
- **Fill back by the greedy order alone (no order within a rank).** Simpler,
  but a group given one rank (the header's harness chips) could show a
  later chip after an earlier one went, behind the narrow gap that belongs
  inside the group. Pieces of one rank keep their order instead.
- **One separator for the header.** No gap per piece; the header would read
  `… · 4 stopped · ▲ 4` and the chips `claude 5 · codex 4`. It changes a
  header that fits for no reason the operator asked for. Rejected; the
  sections keep their `  ·  `.
- **The harness chips as one segment.** All or none: simpler, but at 80
  columns the header would show no chip where one or two fit.
- **Freshness first to go.** It is the widest part; but ADR 0078 calls it
  the primary content signal, so it outlasts every count but the sessions'.
- **The status bar's fixed chips as unranked segments.** Their spacing
  belongs to the chips (`[left] `, `stale  `, `  tmux:off`), and which
  segment comes first would change the gap before it; they stay spans
  around the line.
- **Centered empty states.** The look many programs use for an empty
  screen, but every pane's empty state would move. Left-aligned stays the
  default; centered is there for a slot with nothing around it.
- **Muting the Preview's failures too.** One look for everything that is not
  content, but a reason a preview failed is content the operator needs to
  read; the failure banner and the Messages log already treat it so.
- **A minimum-size screen now.** Left to CSP-319 and CSP-190, as the story
  says; the seam is named.

## Open Questions Answered

- *Strict rule or a fill pass for the status bar?* A fill pass,
  `text::elide_filled`, with ties kept in order; the bar draws as before.
- *Does the header degrade by priority like the status bar?* Yes, on the
  same line control, in ADR 0078's order of importance.
- *Which messages are the empty states?* Every message that stands in for
  content a slot does not have: 15 in the left pane, 2 in the right pane,
  and the Preview's nothing-to-show placeholders; not the reasons a preview
  failed.
- *Where does `StatusChip` live?* In `tuikit::chip` (CSP-725); a priority
  line takes its span as a segment.
