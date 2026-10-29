---
id: CSP-745
title: Extract `Pager` with `Find` and port the pane scrollback viewer onto it
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 17:31'
labels:
  - tui-framework
  - tuifw-reading
milestone: m-25
dependencies:
  - CSP-733
  - CSP-730
  - CSP-724
  - CSP-723
  - CSP-729
  - CSP-725
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/adr/0052-native-session-viewer.md
priority: high
type: enhancement
ordinal: 831000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seven scrolling surfaces are one shape: the transcript viewer, the pane scrollback viewer, the keymap help, the viewer help, the pane help, the value modal, and the Messages detail pane. The transcript and pane viewers share 14 of their 15 to 18 messages, 20 key rows, and 12 help sentences, yet behave differently in find unit, scroll-on-match, end-sticky, and wording. The five pager key tables repeat about 55 movement rows. Page size is a fixed 8 lines in three surfaces and viewport-sized in the others (inventory slice 05).

Scope: Create `Pager<C: PagerContent>` with modes Screen (header, rule, body, rule, footer), Modal, and Embedded; the `PagerContent` trait (rows for a width, match units, header, extra chips, extra key rows with descriptions, apply); `Find` on `LineEdit` with `n` and `N`, wrap notes, and a current-match marker; Esc stepping (help, then find, then leave); the sticky-end rule; viewport paging and half-page keys; clamp-on-draw; a generated `?` page from `HelpPage`; and a copy hook. Port the pane scrollback viewer first, as a roughly 60-line content adapter. Apply house rule 18: Esc steps back, `q` leaves, a confirmed find scrolls to the match, a `[ n/m ]` chip in Screen mode.

TUI changes:
- Consistency wins: the pane viewer gets the transcript viewer's find, paging, and help behavior; one position indicator rule
- Regressions or behavior changes: the pane viewer's find unit and scroll-on-confirm change to the pager's; long lines in the pane viewer wrap or scroll sideways instead of being clipped (the story picks one and says so)

Tests and checks: Port the pane viewer's tests. Property tests for the scroll window and sticky-end. `--snapshot` of the pane viewer at 80x24 and 40x10, with and without find.

Blockers: `LineEdit`, the help extraction, scroll, text, the overlay contract, and chips.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `Pager<C>` provides Screen, Modal, and Embedded modes, `Find`, Esc stepping, sticky-end, viewport and half-page paging, clamp-on-draw, and a generated help page
- [x] #2 The pane scrollback viewer is a `PagerContent` adapter of about 60 lines and its duplicate help page and key table are removed
- [x] #3 The pane viewer's behavior for each documented difference from the transcript viewer is decided and listed in the final summary
- [x] #4 A `[ n/m ]` position chip appears in Screen mode
- [x] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Keys: grow `PagerKey` (half page, find, next/previous, help) with a `HelpAction` impl; split the scroll rows into the `PAGER_SCROLL_KEYS` fragment; `PAGER_KEYS` binds `q` to `Abort` and `Ctrl-D`/`Ctrl-U` (aliases); add `READER_KEYS` (find and help), `PAGER_HELP_KEYS` (the help level: `Esc`/`?` back, `q` close), and `FIND_KEYS`/`FindKey` (the find prompt). Host help page and value modal take the new arms; host table registry lists the tables.
2. Framework core: `tuikit::pager` with `Pager<C: PagerContent>`, `PagerContent` (layout, rows, header, chips, find, match rows, paint, find detail, empty note, keys, apply, copy hook), `PagerEffect`, `NoKeys`; a scroll state clamped on every draw and written back, viewport and half-page paging, `g`/`G`, sticky end (follow newest); Screen mode (header, rule, body, rule, footer with `[ n/m ]`, find chip, note, chips, footer fitted by dropping hint groups); `Overlay<C::Message, Cx>` impl; generated `?` help page (pager table plus content sections) with its own footer; Esc stepping (help, find, leave), `q` leaves.
3. Find: `Find` on `LineEdit` (prompt prefilled with the active query, Enter confirms and always scrolls to the match one row below the top, empty query clears, `n`/`N` wrap with notes, current-match look from `selection_active`, find chip with a content detail), landing near a match id for openers.
4. Modes: Modal (ModalFrame, scrollbar gutter, footer kept on the last rows) and Embedded (body and scrollbar only); `conformance::part` for a table consulted after another; register Screen and Modal pagers in the framework conformance suite; property tests for the scroll window and sticky end.
5. Port the pane scrollback viewer as a `PagerContent` adapter (header label, numbered lines that wrap with a `w` toggle, find over lines, `a` attach); delete `PaneViewerState`, `PaneViewerMsg`, `PANE_VIEWER_KEYS`, its help and scroll/find code; move its tests; update the host registry, conventions, snapshot, consistency, and e2e tests. Generate both viewers' help footers from `PAGER_HELP_KEYS`.
6. Docs: control page `crates/tuikit/docs/controls/pager.md` (with Ports sketches for the transcript, value modal, help body, Messages detail, preview), architecture map, keys/help/testkit pages, ADR 0177 and the index, operations.md and CHANGELOG, operator-review rows.
7. Before/after `--snapshot` frames at 100x30, 60x20, 40x10 in /tmp/tuifw-baseline/frames-745-*; fmt, clippy, full isolated suite, rustdoc, examples, diff check; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slices: (1) keys: `PagerKey` grows half pages, find, next/previous, help; `PAGER_SCROLL_KEYS` fragment; `PAGER_KEYS` binds `q` to `Abort` and `Ctrl-D`/`Ctrl-U`; `READER_KEYS`, `PAGER_HELP_KEYS`, `FIND_KEYS`. (2) `tuikit::pager`: `Pager<C>`, `PagerContent`, `PagerEffect`, `PagerMode` (Screen, Modal, Embedded), `Find`, `NoKeys`, `conformance::part`. (3) The viewers' help footers from `PAGER_HELP_KEYS`. (4) `WrappedRows::hanging`, `cut`, `line_of`. (5) `PagerContent::describe`, found missing by sketching the transcript viewer's port. (6) The pane viewer port, deleting `PaneViewerState`, `PaneViewerMsg`, `PaneViewerOutcome`, `PANE_VIEWER_KEYS`, and its help, scroll, find, footer, and overlay code. (7) Docs: `crates/tuikit/docs/controls/pager.md` with Ports, ADR 0177, operations, changelog, design, operator review.

Frames: /tmp/tuifw-baseline/frames-745-before and frames-745-after (81 frames: 27 states at 100x30, 60x20, 40x10, in a copy of the showcase world, world745, whose recorded pane is 65 lines with long lines), compared in frames-745-diff.txt. Twelve are identical with styles (the frames after `q`, `Esc Esc`, `?q`, and `a`, which leave the viewer); six are new (`w` cut views); the rest differ by wrapping, the generated footer, the landing row, and the help page's `w` row.

Checks: fmt clean; clippy --workspace --all-targets --all-features -D warnings clean; RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps clean; cargo build --examples clean; git diff --check clean; full isolated suite via run-tests.sh green.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::pager`: `Pager<C: PagerContent>` with Screen (header, rule, body, rule, footer led by the `[ n/m ]` chip), Modal (a named `ModalFrame`, the shared scrollbar, the footer on the last rows), and Embedded (body and scrollbar) modes; `PagerContent` (layout for a width, rows for a range, header, chips, find over units, match rows, paint, find detail, `describe` for the pager's keys, empty note, const `KEYS`, `apply` returning `PagerEffect::{None, Relayout, Close, Request}`, and the `copy` hook); one window clamped on every draw and kept, viewport paging, half pages, `g`/`G`, and a sticky end; `Find` on `LineEdit` (`/`, `Enter` always scrolls to the match a row below the top, `n`/`N` wrap with a note, the current match in `selection_active`); `Esc` stepping (help, prompt, find, leave) and `q`/`Ctrl-C` leaving; a generated `?` help page with the help level's own footer; a generated, fitted footer. Key tables: `PagerKey` grows half pages, find, next/previous, help; `PAGER_SCROLL_KEYS`, `READER_KEYS`, `PAGER_HELP_KEYS`, `FIND_KEYS`; `PAGER_KEYS` binds `q` to `Abort` and `Ctrl-D`/`Ctrl-U`. `WrappedRows` gains `hanging`, `cut`, `line_of`; the conformance suite gains `conformance::part` and checks a full-screen and a framed pager. Reference page `crates/tuikit/docs/controls/pager.md` (with Ports sketches for a rich document, a value, the help body, a log's detail, and a preview; the document sketch found the `describe` gap, now fixed); ADR 0177.

The pane scrollback viewer is `Pager<PaneScrollback>`: the `PagerContent` impl is about 100 lines (numbered lines, wrap toggle, find over lines, header, `a` attach); `PaneViewerState`, `PaneViewerMsg`, `PaneViewerOutcome`, `PANE_VIEWER_KEYS`, its help, scroll, find, footer, and overlay code are deleted (module 539 to 210 lines).

Decided differences from the transcript viewer: long lines wrap under their number, and `w` cuts them at the edge with `…` (no sideways scroll); the find unit is a line; every landing (confirm, `n`, `N`, open) puts the line one row below the top, always; `G` sticks to the end; the current match is the pager's `selection_active` look plus the pane's accent line number; the pager's notes and footer words.

Verified: tuikit unit, property (generated keys, sizes, growth), find, conformance, and doc tests; host pane viewer, viewer help, conventions registry, help parity, scrolling, text entry, snapshot, keybinding, and tmux e2e tests; the full isolated suite (4354 passed, 0 failed, 1 ignored) and nextest (4354 passed, 1 skipped); fmt, clippy -D warnings, rustdoc -D warnings, `cargo build --examples`, `git diff --check`. Snapshot frames before and after in /tmp/tuifw-baseline/frames-745-{before,after} at 100x30, 60x20, 40x10 (81 frames, diff in frames-745-diff.txt).

**Regressions**
- The `[ n/m ]` chip counts screen rows, so with wrapping it runs past the header's line count (`[ 14/74 ]` beside `65 lines`).
- A find, `n`/`N`, and the opening match always scroll, so stepping through matches on one screen moves the view; one row of context instead of two.
- `no find: press / to search` became `nothing to find yet: press / to search`, and a confirm or `n` with no match no longer adds a `no match` note (the chip still says it).
- At 60 and 40 columns the footer no longer names `? help` (it drops the scroll keys, then `/ find · ? help`, before the wrap chip and `a attach`, as the transcript viewer drops its help chip first); at 40 columns a long find chip still pushes the close keys into the `…` cut.

**Consistency wins**
- Long lines wrap like the transcript viewer's (rule 14); `w` (new) cuts them at the edge with `…` instead of a silent clip.
- A confirmed find always scrolls to its match (rule 18), as in the transcript viewer; the current match takes the transcript viewer's reversed look (now `selection_active`).
- `G` sticks to the end (rule 18, as the transcript viewer).
- The footer is generated from the tables in the one grammar (rule 8): `wrap·on (w) · j/k/PgDn/PgUp scroll · / find · ? help · a attach · Esc/q close`, dropping groups when narrow; the header and footer cut with `…` (rule 14).
- Notes take the feedback severities: a wrap note is info (green), a refusal a warning.
- Both viewers' help pages read `j/k/PgDn/PgUp scroll · Esc/? back · q close` instead of `Esc/q/? close` (rules 4, 8); the pane page lists `w`, and its `Esc` row reads `close this help`, the pager's one wording.
- `Ctrl-D`/`Ctrl-U` half pages in the keymap help and the value modal (rule 12).
- Every frame that leaves the viewer (`q`, `Esc Esc`, `?q`, `a`) is identical, styles included.
<!-- SECTION:FINAL_SUMMARY:END -->
