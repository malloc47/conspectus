# ADR 0179: Log Lists Read The Host's Log, And The Detail Is An Embedded Pager

## Status

Accepted. Carries out the `LogList` control of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), applying its
house rules 1, 2, 4, 5, 8, 11, 12, 13, 14, and 18, and extends
[ADR 0150](0150-tui-framework-workspace-crate.md) with the framework's
`log_list` module. Amends [ADR 0105](0105-tui-message-log.md) decision 2:
the Messages overlay is the framework's log list, `q` dismisses it (as
`Ctrl-C` does) and `Esc` and `!` close it, and the record pane scrolls as
every pager does. Amends
[ADR 0163](0163-tui-feedback-one-type-four-severities-in-tree-toasts.md)
decision 6: an entry's summary with its repeat count (`(×3)`) is the
framework's `Entry::summary_with_repeats`, not the host's. Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md):
`LOG_LIST_KEYS` (`LogListKey`) is a standard table, and the host's
`MessagesKey` is gone. Amends
[ADR 0154](0154-tui-glyph-marks-vetted-table-and-scan.md): the marks name
`TIMES`, `FAIL`'s character read as the multiplication sign, so the repeat
count is drawn through a mark. Carries out what
[ADR 0177](0177-tui-readers-are-one-pager-over-content-the-host-lays-out.md)
left for the Messages detail, and applies the footer-fit rule of
[ADR 0178](0178-tui-help-pages-and-long-values-are-pager-contents.md)
decision 3 to the log list's footer.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md`, section
3.9, and `05-search-help-viewers.md`, section 2.5) found the Messages
overlay to be a master list of operation outcomes over a detail pane, all
of it written by hand: a state struct with a `u16` detail offset, a key
handler, a widget, and an overlay impl. CSP-712 clamped its detail and
paged it by the rows shown, CSP-727 gave its list the shared selection
look, and CSP-731 moved its log into `tuikit::feedback::Log` over the
host's payload (`Report`: the target and the command record). What was
still its own:

- **The list kept no window.** It rebuilt one each frame from offset 0,
  so moving up from the bottom of a long log scrolled the list to keep
  the selection on its last line, and it had no scrollbar.
- **Rows were cut at the edge with no mark.** A long summary lost its end
  silently.
- **The detail scrolled its own way.** It wrapped its own rows and kept
  its own offset and page size beside the framework's pager, which every
  other reader now uses; it had no scrollbar.
- **The footer was one row,** cut at the frame's edge on a narrow screen
  (at 40 columns `y copy` and `Esc close` were lost), and in the empty log
  it sat right under the note instead of on the frame's last row.
- **The repeat count** (`(×3)`) was host code typed with the failure mark.

## Decision

1. **`tuikit::log_list` owns the reader.** `LogList<R, M, Cx>` is an
   overlay over the host's `Log<R>`: `R` is the entries' payload, `M` the
   host's message, `Cx` its context. It reads the log from the context
   through a closure (`Fn(&Cx) -> &Log<R>`) on every key and every draw,
   as the settings panel reads its rows, so an entry logged while the list
   is open shows at once; the time its ages are read against comes from
   the context through another (`Fn(&Cx) -> i64`), so the framework reads
   no clock. It keeps only its selection and its two windows.

2. **One row per entry,** newest first: the severity's mark in its theme
   color, the age in the age cell's width with its prose suffix, and
   `Entry::summary_with_repeats`, through `selection::row` and cut with
   `…`. The rows take up to two fifths of the frame (at least three while
   the log has three) and scroll in a window the list keeps, by the follow
   rule of ADR 0156, with the shared scrollbar. The selection stops at the
   newest and the oldest entry (house rule 2: a log is scrollable
   content).

3. **The detail is an embedded pager.** The host's `detail` closure gives
   the selected entry's text; the list lays it out a line at a time
   (`WrappedRows`) in a `Pager` in `Embedded` mode, routes `J`/`K` to its
   row scroll and `PgDn`/`Space`/`PgUp` to its pages, and sends it to the
   top when the selection moves. Its window is clamped on every draw and
   pages by the rows it shows, and it draws the shared scrollbar.

4. **Keys from a standard table.** `LOG_LIST_KEYS` binds `Ctrl-C` and `q`
   to `Abort`, `j`/`k` (with the arrows and `Ctrl-N`/`Ctrl-P`) to select,
   `g`/`Home` and `G`/`End` to the newest and the oldest, `J`/`K` to
   scroll the detail, `PgDn`/`Space`/`PgUp` to page it, and `Esc` to
   close. No standard row binds `Copy`: a host whose entries copy binds it
   (`y`), and asks its copy hook for the message, as the pager's copy hook
   does. Conspectus's `MESSAGES_KEYS` is that table, `y`, and `!` as
   another close.

5. **One footer rule.** The footer is generated from the table, the keys
   that leave last, and drawn on the frame's last rows, broken only
   between segments. When it is taller than the room under the list
   leaves beside a row of detail, its last rows stay and its first rows
   scroll after the detail's last row (ADR 0178 decision 3); the pager
   gains a crate-internal embedded draw with such a tail.

6. **The unseen count.** `mark_seen` marks the log seen as the list opens
   over it, so a count the host shows elsewhere (the status bar's `▲ N · !
   messages` chip) clears when the user looks.

7. **What stays in Conspectus.** `Report`, `LogTarget`, `CommandRecord`,
   the record each entry's detail reads as (`full_text`), the copy message
   (`Msg::Copy`, written by the executor), the title, the empty note, the
   preview banner, and `latest_failure_for`. `MessagesOverlayState`,
   `MessagesWidget`, `MessagesKey`, and the overlay's impl are removed; the
   Messages entry on the modal stack is `LogList<Report, Msg, App>`.

## Consequences

- **Visible changes in Messages** (the CSP-747 summary lists them): a long
  record shows the shared scrollbar and wraps a cell narrower beside it; a
  long log shows the scrollbar beside its list, and moving up inside the
  rows shown no longer scrolls the list; a summary too long for its row
  ends in `…`; on a narrow screen the footer wraps onto a second row
  instead of losing its last keys, so the record is a row shorter there;
  in an empty log the footer sits on the frame's last row and the note
  wraps instead of being cut. Rows, colors, the title, the default frames
  at 100 and 60 columns of a short log, and paging are otherwise
  unchanged.
- `q` and `Ctrl-C` answer `Dismiss` where they answered `Close`; Messages
  is no step, so they close it the same.
- A log reader a host adds is a `LogList` over its log: a title, two
  closures, and its detail and copy hooks.

## Alternatives Considered

- **A trait the context implements** (`fn log(&self) -> &Log<R>`). One
  type parameter fewer, but the settings panel already reads its context
  through a closure, and a trait would tie each context to one log.
- **Copy the log's entries into the list when it opens.** No closure, but
  an entry logged while the list is open would not show, as it does today.
- **A row trait for the host's entries** (severity, time, and summary from
  the host's type). ADR 0163 rejected it for the log itself: every entry
  has those, and the framework's `Entry<T>` carries them.
- **Keep the list's per-frame window** (follow from offset 0). Frames of
  the moved-down list stay the same, but moving back up scrolls when
  nothing needs to, and every other framework list keeps its window.
- **Bind `Ctrl-D`/`Ctrl-U` to the detail's half pages.** The pager has
  them, but the story keeps the keys the overlay had; it is a seam on the
  control's page.
- **Drop the footer's first rows on a short frame**, as the checklist
  does. Simpler, but it loses keys; the pager's rule keeps them a scroll
  away (backlog `CSP-784` aligns the other framed controls).

## Open Questions Answered

- *Where does the list read its log from?* The host's context, through a
  closure, on every key and every draw.
- *Who reads the clock?* The host, through a closure over its context.
- *Does the detail page by a fixed size?* No: by the rows it shows,
  clamped, as every pager.
- *Does `q` close or dismiss?* Dismiss, as in every standard table where
  nothing is typed.
