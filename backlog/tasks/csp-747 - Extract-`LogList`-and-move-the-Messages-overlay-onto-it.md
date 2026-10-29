---
id: CSP-747
title: Extract `LogList` and move the Messages overlay onto it
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 19:34'
labels:
  - tui-framework
  - tuifw-reading
milestone: m-25
dependencies:
  - CSP-745
  - CSP-731
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/adr/0105-tui-message-log.md
priority: medium
type: enhancement
ordinal: 833000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Messages overlay (`!`) is a scrolling master list of operation outcomes with a detail pane. It sets its own selection highlight (it is one of the three places that read `theme.selection_active`), pages the detail by a fixed 8 lines, and sets an unclamped offset on `G` (inventory slices 04 and 05).

Scope: Create `LogList<R>` with rows `{severity, time, summary}`, a selection, and an embedded `Pager` for the detail. The message payload (`CommandRecord`, `LogTarget`, argv, exit status, stderr, stdout) and the preview banner stay in Conspectus.

TUI changes:
- Consistency wins: the detail pane behaves like every pager; severity colours from the theme
- Regressions or behavior changes: none expected

Tests and checks: Messages tests ported. `--snapshot` at 80x24 and 40x10.

Blockers: `Pager` and the feedback extraction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `LogList<R>` provides the master list and an embedded pager for the detail
- [x] #2 The Messages overlay is a `LogList` instance whose payload stays in Conspectus
- [x] #3 The detail pane clamps on `G` and pages by the viewport
- [x] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames of the Messages overlay (empty; mixed severities; a long entry paged, past its end, G/g; many entries) at 100x30, 60x20, 40x10 into /tmp/tuifw-baseline/frames-747-before.
2. tuikit: `log_list` module with `LogList<R, M, Cx>`: reads the host's `feedback::Log<R>` through a closure over the context (and the time through another), rows `{severity mark, age, summary (xN)}` newest first through `selection::row`, cut with the ellipsis, in a kept `ScrollWindow` with the shared scrollbar; clamped selection (`g`/`G` newest/oldest); the selected entry's detail (a host closure) in an embedded `Pager` scrolled by `J`/`K`/`PgDn`/`PgUp`/`Space` (clamped, paged by the rows shown, reset to the top when the selection moves); a generated footer whose last rows stay and whose first rows scroll with the detail when it does not fit; `Esc` closes, `q`/`Ctrl-C` dismiss; a copy hook; the empty note; `mark_seen` as the unseen-count hook. `LogListKey` and `LOG_LIST_KEYS` join the standard tables; `Entry::summary_with_repeats` and `marks::TIMES` move the repeat count into the framework.
3. tuikit tests (keys, drawing, scrolling, footer fit, conformance registration) and docs: `docs/controls/log_list.md`, module map, README, keys and feedback pages.
4. Host: the Messages overlay becomes `LogList<Report, Msg, App>` (payload, detail text, copy message, empty prose, and the `!`/`y` keys stay in Conspectus); drop `MessagesOverlayState`, `MessagesWidget`, `MessagesKey`, and its overlay impl; update tests mechanically.
5. ADR (next free number) for the log list, ADR index, design summary, operations guide, changelog, operator review.
6. After frames, compare by category; run fmt, clippy, the isolated suite, rustdoc, examples, diff check; close the story.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented `tuikit::log_list::LogList<R, M, Cx>` (commit 971f2dfe), its reference page and docs (1128be07), the Messages overlay on it (23b70e89), and ADR 0179 with the host docs (b500d768).
Judgment calls (recorded in docs/plans/tui-framework/operator-review.md): the log and the clock are read from the context through closures (the settings panel's pattern); the list keeps its window and draws the shared scrollbar, and long rows end in an ellipsis (the framework's list rules); the footer is generated and fitted by the framed pager's rule, so it wraps at 40 columns and sits at the bottom of the empty list; `q` dismisses (standard table); no Ctrl-D/Ctrl-U in the detail (seam).
Validation: tuikit unit, doc, independence, documentation, glyph and raw-style guards; host Messages, key table, consistency (conventions, selection, scrolling, layout, registry, marks, text) and UI tests; full isolated suite 4379 passed, 0 failed, 1 ignored; nextest 4379 passed, 1 skipped after one rerun (first run hit a tmux "server exited unexpectedly" flake in cli_history); fmt, clippy -D warnings, rustdoc -D warnings, cargo build --examples, git diff --check.
Frames: /tmp/tuifw-baseline/frames-747-{before,after} at 100x30, 60x20, 40x10 (diff in frames-747-diff.txt), after-only extras (far past the end, 40x8/60x7/40x6) and 80x24.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The Messages overlay (`!`) is now the framework's log list, so its list selects, scrolls, and fits its footer like every framework list, and its record pane pages like every pager.

Framework: `tuikit::log_list::LogList<R, M, Cx>` implements `Overlay<M, Cx>` over the host's `feedback::Log<R>`, read from the context through a closure on every key and draw, with ages against a host clock closure. Rows are `{severity mark, age, summary (×N)}`, newest first, through `selection::row`, cut with `…`, in a kept window with the shared scrollbar; the selection clamps; `g`/`G` newest/oldest. The selected entry's detail (a host closure) is an embedded `Pager` scrolled by `J`/`K`/`PgDn`/`Space`/`PgUp`, clamped, paged by the rows shown, back to the top when the selection moves. The footer is generated from `LOG_LIST_KEYS` (new standard table, `LogListKey`) with the keys that leave last; on a short frame its last rows stay and its first rows scroll after the detail (ADR 0178's rule, via a crate-internal embedded draw with a tail). Also a copy hook, the empty note, and `mark_seen` (the unseen-count hook). `Entry::summary_with_repeats` and `marks::TIMES` move the repeat count in. Page: `crates/tuikit/docs/controls/log_list.md`; ADR 0179.

Host: `MessagesList = LogList<Report, Msg, App>` built by `widgets::messages::open`; the payload (`Report`, `LogTarget`, `CommandRecord`), `full_text`, the `Msg::Copy` message, the empty prose, `!`/`y` (`MESSAGES_KEYS`), and the preview banner stay in Conspectus. `MessagesOverlayState`, `MessagesWidget`, `MessagesKey`, the overlay impl, and the host repeat rendering are removed. Tests changed on purpose: `q`/`Ctrl-C` expect `Dismiss`; the detail offset is read after a draw; the table-order footer of `MESSAGES_KEYS` lists `y copy` after `Esc close`.

Verified: full isolated suite 4379 passed, 0 failed, 1 ignored; nextest 4379 passed (one rerun after a tmux flake); fmt, clippy, rustdoc, examples, diff check. Frames before/after at 100x30, 60x20, 40x10 (`/tmp/tuifw-baseline/frames-747-*`): short logs at 100 and 60 columns are byte-identical; rows' styles are unchanged everywhere.

**Regressions**
- At 40 columns the record shows one row less (the footer takes two rows).
- A record or log that overflows wraps or cuts a column narrower beside its scrollbar.

**Consistency wins**
- The record pane shows the shared scrollbar, as every pager (it already clamped and paged by the rows shown).
- A long log shows the scrollbar beside its list, and moving back up inside the rows shown no longer scrolls it.
- Summaries too long for their row end in `…` instead of being cut silently.
- The footer wraps between segments instead of losing `y copy · Esc close` at narrow widths, and keeps its last row on very short screens.
- With no messages the footer sits on the frame's last row and the note wraps instead of being cut.
- Severity marks keep their theme colors (unchanged since CSP-731).
<!-- SECTION:FINAL_SUMMARY:END -->
