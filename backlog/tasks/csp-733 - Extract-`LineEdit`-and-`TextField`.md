---
id: CSP-733
title: Extract `LineEdit` and `TextField`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 04:13'
labels:
  - tui-framework
  - tuifw-text
milestone: m-25
dependencies:
  - CSP-723
  - CSP-721
  - CSP-728
  - CSP-715
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
  - docs/adr/0030-tui-text-input-primitive.md
priority: high
type: enhancement
ordinal: 819000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Eleven text-entry surfaces use two edit engines (`tui-input` through `TextInputState`, and the hand-rolled `FindPrompt` in the viewer) and four renderers of one editing state, with three cursor-windowing algorithms; only the viewer's is cell-width aware. The search and picker query lines draw no caret. Three code paths copy the value before a key and compare after to learn whether it changed. Seven match sites handle `InputOutcome::Confirm`, three of them unreachable (inventory slice 03).

Scope: Create `tuikit::text_edit` (or the module name the contribution guide settles on) with `LineEdit`, the only edit engine, wrapping `tui-input` and returning `changed`, `consumed`, and `visual_cursor`; `TextField`, one inline renderer with one cell-width-aware windowing algorithm, optional scroll marks and prefix glyph, a caret cue from the theme, and a placeholder. Handle `Event::Paste` for single-line fields, map ALT to word motion, and have the runtime enable bracketed paste. Replace `TextInputState`, `FindPrompt`, and the form, query-line, and find-prompt renderers.

TUI changes:
- Consistency wins: wide characters edit correctly; search and picker query lines show a caret; paste and word motion work everywhere; one text-editing behavior in all eleven surfaces
- Regressions or behavior changes: the viewer's find prompt gains `tui-input`'s editing keys and loses its own variants where they differ

Tests and checks: Tests for insert, delete, home and end, word motion, kill-line, paste, and wide characters through `LineEdit`. Snapshots of the query lines and the viewer find footer. The behavior-level tests from the text-entry defect task pass unchanged.

Blockers: The text and theme extractions, the key tables, and the text-entry defect task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `LineEdit` is the only text-edit engine; `TextInputState`, `FindPrompt`, and the three cursor-windowing algorithms are replaced by it and `TextField`
- [x] #2 `LineEdit` reports whether the value changed, and the three copy-and-compare probes are removed
- [x] #3 Search and picker query lines draw a caret
- [x] #4 Bracketed paste is handled for single-line fields, ALT word motion works, and editing is cell-width correct
- [x] #5 The three unreachable `InputOutcome::Confirm` arms in the forms are removed
- [x] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames (rename prompts, new mux / mux launch / pin edit forms, quick select, content search, picker, transcript and pane viewer find footers, worktree branch prompt; ASCII and CJK/emoji; 100x30 and 60x20) into /tmp/tuifw-baseline/frames-733-before.
2. tuikit::text_edit: `LineEdit` (wraps tui-input's Input through InputRequest; value, cursor, visual_cursor, handle -> Edit { changed, consumed }, apply(EditKey), paste), `single_line_paste` (the flatten rule), and `TextField` (one cell-width window that keeps the cursor in view, optional `<`/`>` marks, prefix, placeholder, field_cursor caret, optional fill). `EditKey` and `LINE_EDIT_KEYS` (readline keys, Meta read as Alt, Alt-arrow word motion) live with the standard tables in `keys` (plus an `alt_key` builder) and are re-exported from text_edit. Tests: insert, delete, home/end, word motion, kill-line, yank, paste, wide characters, windowing, styled golden.
3. Host: TextInputState becomes the host prompt state `TextPromptState` over LineEdit (kept until TextPrompt); the forms, path omnibox, launch spec, search, and picker hold LineEdit; the three copy-and-compare probes use `changed`; the three unreachable Confirm arms in the forms go; form.rs `window`, input.rs `visible_window`, and the find footer's window are replaced by TextField; search and picker query lines draw a caret; TEXT_INPUT_KEYS loses its word rows.
4. Viewer and pane viewer: FindPrompt deleted; the prompt is a LineEdit, `ViewerFindMsg::Edit(EditKey)` carries the shared editing keys, VIEWER_FIND_KEYS keeps Ctrl-C/Enter/Esc.
5. Docs: crates/tuikit/docs/controls/text_edit.md, architecture map, keys page, README, contributing; ADR 0165 (narrows ADR 0030) with the README index and design.md; operations.md and CHANGELOG; operator-review judgment calls.
6. After frames, compare by category; fmt, clippy, isolated suite, rustdoc -D warnings, git diff --check; final summary with Regressions and Consistency wins.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Framework: `tuikit::text_edit` (LineEdit, Edit, single_line_paste, TextField) with EditKey/LINE_EDIT_KEYS in `keys/standard.rs` (re-exported), so the key-table hygiene scan still finds key codes only in the key model; LINE_EDIT_KEYS is registered with the host's table checks as a fragment. Host: TextPromptState/TextPromptWidget keep the prompt contract; forms, omnibox, launch spec, search, picker, and both find prompts hold LineEdits; ViewerFindMsg::Edit(EditKey) replaces nine variants; the prompt's and find footer's reversed-cell exceptions in consistency_tests/selection.rs went stale and were removed. Unused title/label arguments went with TextInputState (PathOmniboxState::new, LaunchSpecInit, the spec's set_* and cycle_harness*); dead form::tagged_input_row deleted; the root package no longer lists tui-input.
Frames: 31 states x 2 sizes in /tmp/tuifw-baseline/frames-733-{before,after} (capture-733.sh; ansi733.py marks reversed and white-ground cells). Text changed only in CJK/emoji cases and the 60-column long quick-select query; every ANSI frame differs by the caret cue (query lines gain one, prompts and find footers move from reverse video to field_cursor) or by the unfocused form fields' cursor cell taking the field style.
Environment: the root filesystem filled up (100%) mid-run from this worktree's 101G target dir; removing target/debug/incremental freed 49G. A tar restore while splitting commits brought back old mtimes and a stale tuikit build; touching the files fixed it.
Checks on the final commit: cargo fmt --check, clippy --workspace --all-targets --all-features -D warnings, run-tests.sh (3981 passed, 0 failed, 1 ignored), RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps, git diff --check: all pass. The framework-only commit was checked on its own (3977 passed, clippy clean).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tuikit::text_edit`: `LineEdit`, the only edit engine (wraps `tui-input`; `handle` returns `Edit { changed, consumed }` with no Enter/Esc/Tab; `visual_cursor`; `paste`), `single_line_paste` (the paste flatten rule, moved from the host), and `TextField`, one inline renderer with one cell-width window that keeps the cursor in view, optional `<`/`>` marks, a prefix, a placeholder, a `field_cursor` caret, and a fill. The editing keys are `LINE_EDIT_KEYS` over `EditKey` (readline keys, Meta read as Alt, `Alt-←/→` word motion), kept with the standard tables in `keys`. Every host field moved onto it: `TextInputState` and `FindPrompt` are gone (the prompt contract stays as `TextPromptState` until `TextPrompt`), the three copy-and-compare probes use `changed`, the forms' three unreachable `Confirm` arms went, and the form rows, prompt row, query lines, and find footer draw through `TextField`. ADR 0165 narrows ADR 0030; the control page is `crates/tuikit/docs/controls/text_edit.md`.

Verified with new LineEdit and TextField tests (insert, delete, home/end, word motion, kill-line, yank, paste, wide characters, every window fits and holds the cursor, a styled golden), styled goldens of the search and picker query lines and the find prompt, the CSP-715 behavior tests unchanged, `--snapshot` frames of 31 states at 100x30 and 60x20 with ASCII and CJK/emoji input, and fmt, clippy, the isolated suite (3981 passed, 0 failed, 1 ignored), rustdoc with `-D warnings`, and `git diff --check`. Tests updated on purpose: the form window, prompt window, and FindPrompt unit tests now drive LineEdit and TextField; the find prompt key-table test checks the shared editing keys.

**Regressions**
- In the transcript and pane viewer find prompts, `Ctrl-U` clears the whole query (it deleted only what preceded the cursor), and word motion follows the shared Unicode word rule, which steps between ideographs where the old prompt took a run of them as one word.

**Consistency wins**
- Wide characters edit and window correctly in every field: the caret lands on the character it is on, a long CJK or emoji value scrolls to keep it in view, and a field no longer runs past the frame or pushes a path's validity mark or the pin edit tag column aside (rule 15).
- The quick select, content search, and picker query lines draw a caret and keep it in view; a long query used to be cut at the frame.
- One caret look: the prompts (rename, worktree branch, Controls editors) and the find footers take `field_cursor` (black on white) instead of reverse video, as the forms did (rule 13).
- The find prompts take every field's editing keys (`Ctrl-A`/`E`/`B`/`F`/`H`/`W`/`K`/`Y`, `Alt-d`, `Alt-Backspace`, `Ctrl-Delete`); `Alt-b`/`Alt-f` and `Ctrl-←`/`Ctrl-→` motion stays.
- Every field gains `Alt-←`/`Alt-→` word motion and `Alt-d` (delete the next word), and a Shift on `←`, `→`, `Home`, `End`, `Backspace`, or `Delete` no longer swallows the key.
- An unfocused form field draws the cell under its hidden cursor in its own style, so the `-` of an empty unfocused field is `field` gray rather than the default foreground; a value of spaces shows its spaces instead of `-`; a field under 5 cells shows the text around the cursor instead of only the cursor's character.
- ASCII frames are otherwise identical (same windowing rule; the find footer's layout and hint placement unchanged).
<!-- SECTION:FINAL_SUMMARY:END -->
