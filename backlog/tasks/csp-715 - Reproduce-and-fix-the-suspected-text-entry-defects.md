---
id: CSP-715
title: Reproduce and fix the suspected text-entry defects
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 08:25'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
priority: medium
type: bug
ordinal: 801000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The inventory found four text-entry problems by reading (slice 03). `~/x` passes path validation and is committed unexpanded to a CLI that does not expand it. A multi-line paste submits a form, because bracketed paste is not enabled. Alt-B and Alt-F word motion is probably inert, because `tui-input` binds META while crossterm reports ALT. Invalid input is rejected silently in the Controls max-age editor, the empty worktree branch prompt, the new mux form, and the empty pin rename. These are fixed first, with behavior-level tests, so `LineEdit`, `TextPrompt`, and `PathField` start with a regression suite that already passes.

Scope: Reproduce each with a test. Apply the smallest fix in the current code: expand `~` when the path value is committed, enable bracketed paste in the runtime and ignore or flatten embedded newlines in single-line fields, map ALT to word motion, and show a status message for each silent rejection. Record any defect that does not reproduce.

TUI changes:
- Consistency wins: `~` paths work; multi-line paste cannot submit a form; word motion works; rejected input explains itself
- Regressions or behavior changes: a pasted multi-line value is flattened or rejected instead of acting on the first line

Tests and checks: Tests exercise each field at the behavior level and are written so they move to the new controls unchanged. Manual check by pasting a multi-line value into the rename prompt in a live terminal.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Each suspected defect is reproduced and fixed, or recorded as not reproducible
- [x] #2 A committed path value beginning with `~` is expanded before it reaches the command
- [x] #3 Bracketed paste is enabled and a multi-line paste into any single-line field does not submit it
- [x] #4 Alt-B and Alt-F move by word in every text field
- [x] #5 Each silent rejection (max-age, empty branch, new mux, empty pin rename) shows a message
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce each suspected defect with behavior-level tests in a new consistency module (src/tui/consistency_tests/text_entry.rs) that drives the real surfaces through action_for_event and their key handlers and asserts on committed values, status text, and rendered frames.
2. `~` on commit: the new mux, mux launch (cwd and worktree repo), and pin edit path rows commit PathOmniboxState::expanded_value(); the mux launch default-argv preview reads the expanded cwd too. Record why the three `~` expansions stay separate (the omnibox moves to the framework crate).
3. Bracketed paste: one terminal enter/leave helper in src/tui/runtime/terminal.rs enables it after ratatui::init and disables it before ratatui::restore (startup, exit, attach, viewer hand-off, panic); a source scan keeps raw init/restore calls in that helper. Event::Paste types the flattened text into the overlay that owns the keys, one key at a time, and stops if the overlay closes; outside overlays a paste is ignored. Flattening: drop line breaks at the ends, one space per run inside, tab to space, other controls dropped.
4. Word motion: TEXT_INPUT_KEYS gains Alt-B / Alt-F rows that TextInputState maps to tui-input word motion; the viewer and pane-viewer find prompt gains word motion on Alt-B / Alt-F and Ctrl-Left / Ctrl-Right.
5. Rejections: the Controls max-age editor, the empty worktree branch prompt, and the new mux form say why in the status bar while staying open; verify the empty pin rename toast already shows and record it as not reproducing.
6. Docs: operations.md (editing text in fields), CHANGELOG.md, a Verification (CSP-715) section in inventory slice 03, operator-review rows for judgment calls.
7. Run fmt, clippy, the isolated full suite, rustdoc with -D warnings, git diff --check; manual check of a multi-line paste into the rename prompt in a live terminal is not possible unattended, so record the snapshot/test evidence instead.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reproduced first with src/tui/consistency_tests/text_entry.rs against the unchanged code: `~/projects` was committed raw by the new mux form, the launch form (cwd and worktree repo), and pin edit; Alt-B/Alt-F failed in all 15 text fields and Ctrl-Left/Right in both find prompts; the max-age editor, empty worktree branch, and new mux form showed nothing. The empty pin rename already toasted `pin: nothing entered` (not reproduced). Paste: the runtime never enabled bracketed paste and dropped `Event::Paste`.
Judgment calls (also in docs/plans/tui-framework/operator-review.md): refusals toast instead of using the status line, because the Controls overlay covers the status line; a paste is flattened, and ignored outside text fields (App::text_field_has_keys) so pasted letters never act as menu keys; the three `~` expansions stay separate; a new hygiene scan tests/terminal_hygiene.rs keeps ratatui init/restore in src/tui/runtime/term.rs.
Live check: a pseudo-terminal run of `conspectus tui --fixture tests/fixtures/showcase.json` emitted ESC[?2004h at start and ESC[?2004l at exit, and a pasted `first\r\nsecond\n` was drawn as `first second` in quick select with the overlay still open (fixture mode keeps rename disabled, so quick select stood in for the rename prompt). Snapshots of the max-age toast at 120x30 and 60x16 show it drawn over the Controls overlay.
Checks: cargo fmt --check, clippy -D warnings, run-tests.sh (3514 passed, 0 failed, 1 ignored), nextest (3514 passed, 1 skipped), cargo doc with RUSTDOCFLAGS=-D warnings, git diff --check: all pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Reproduced the suspected text-entry defects with behavior tests (`src/tui/consistency_tests/text_entry.rs`) and fixed them in the current code, so `LineEdit`, `TextPrompt`, `PathField`, and `Form` start with a passing regression suite.

- `~` committed unexpanded: reproduced in the new mux form, the launch form (cwd and worktree repo), and pin edit. They now commit `PathOmniboxState::expanded_value()`; the launch form's default-argv preview reads the same expanded cwd. The three `~` expansions stay separate (the omnibox's moves to the framework).
- Multi-line paste submits: reproduced (bracketed paste never enabled, `Event::Paste` dropped). `src/tui/runtime/term.rs` enables it on every take-over and disables it on every hand-back and on panic; `tests/terminal_hygiene.rs` keeps ratatui init/restore there. A paste is typed into the focused text field as one line (`single_line_paste`); outside a text field it is ignored.
- Alt-B/Alt-F inert: reproduced in all 15 fields. `TEXT_INPUT_KEYS` maps them to word motion; the find prompts gain Alt-B/Alt-F and Ctrl-Left/Right.
- Silent rejections: reproduced for max age, empty worktree branch, and the new mux form, which now toast why and stay open (`Msg::Toast`). The empty pin rename already toasted: not reproduced.

Verified with the new tests, a pseudo-terminal run of the fixture TUI (paste markers on and off; a pasted two-line value drawn as one line in quick select, which stayed open), `--snapshot` frames of the max-age toast at 120x30 and 60x16, and fmt, clippy, the isolated suite (3514 passed, 0 failed, 1 ignored), nextest, rustdoc with `-D warnings`, and `git diff --check`. Updated tests that encoded the silent rejections: `max_age_invalid_value_keeps_editor_open`, `enter_on_empty_name_snaps_focus_back_to_name`, `enter_on_empty_cwd_stays_on_cwd`.

**Regressions**
- A pasted multi-line value is flattened to one line (line breaks become a space) instead of acting on its first line, and a paste outside a text field (tree, menu, list, option row) no longer acts as keystrokes.
- In the find prompts, Ctrl-Left/Ctrl-Right now move by word instead of by one character.

**Consistency wins**
- `~` paths work in every path field.
- A multi-line paste cannot submit a prompt or form, and pasted letters cannot trigger a menu choice.
- Alt-B/Alt-F (and Ctrl-Left/Right) move by word in every text field.
- Refused input explains itself in a toast, like the existing empty-rename toasts.
<!-- SECTION:FINAL_SUMMARY:END -->
