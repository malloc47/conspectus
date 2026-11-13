---
id: CSP-734
title: Extract `TextPrompt<M>`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 05:03'
labels:
  - tui-framework
  - tuifw-text
milestone: m-25
dependencies:
  - CSP-733
  - CSP-729
  - CSP-726
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
priority: medium
type: enhancement
ordinal: 820000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Five surfaces are the same dialog: rename (session, mux, pin), the worktree branch prompt, and the Controls max-age and contains editors. Each has its own empty-input behavior (a silent no-op in three, a toast in one), pin rename needs a `SlotPrompt` side flag to re-route `CommitRename`, and Esc in the branch prompt returns to the menu while rename closes (inventory slice 03, section 2).

Scope: Create `TextPrompt<M>` with a title, an initial value, and an `on_confirm: Fn(String) -> PromptResult<M>` returning `Commit(M)`, `Reject(String)`, or `Stay`. A rejection shows one error line in the footer slot until the next edit. Migrate the five surfaces; the two rename paths become two closures, removing `SlotPrompt`. Esc steps back one level and cancel is silent.

TUI changes:
- Consistency wins: every rejection explains itself; one prompt for five surfaces; the pin rename prompt no longer needs a routing flag
- Regressions or behavior changes: the prompt modal grows one row while an error shows; the empty-pin-rename toast becomes the prompt's error line

Tests and checks: Tests for each surface's validation and for confirm, reject, cancel, and back. `--snapshot` of each prompt with and without an error.

Blockers: `LineEdit`, the overlay contract, and the modal frame.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `TextPrompt<M>` replaces the rename overlay, the worktree branch prompt, and the Controls max-age and contains editors
- [x] #2 `PromptResult` is the only way a prompt accepts, rejects, or stays, and a rejection shows an error line until the next edit
- [x] #3 `SlotPrompt` and the `CommitRename` re-routing are removed
- [x] #4 Esc steps back one level and cancel produces no status message
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
1. Capture before frames of the five prompts (session rename, pin rename, worktree branch, Controls max age and contains) with empty, valid, invalid, Enter, Esc, and edit-after-refusal input at 100x30 and 60x20 (/tmp/tuifw-baseline/frames-734-before).
2. Framework: `ModalFrame::footer`, a line on the bottom border in the title's form, so a one-row prompt keeps its 3-row frame and still shows its generated keys; the conformance footer check reads the bottom border too.
3. Framework: `tuikit::text_prompt` with `TextPrompt<M>` (title, initial value, optional placeholder, `on_confirm: Fn(&str) -> PromptResult<M>` shared behind an `Arc`) and `PromptResult { Commit(M), Reject(String), Stay }`; implements `Overlay<M, Cx>` for any `Cx`: Enter asks `on_confirm`, Reject sets one error line (a body row in `error`, cut to the width) that the next edit clears, Esc and Ctrl-C close, Tab is swallowed, `q` types; footer from `TEXT_INPUT_KEYS` on the bottom border; 3 rows, 4 while an error shows. Unit tests and the reference page `docs/controls/text_prompt.md`.
4. Host: rename prompts become `Modal::Rename(TextPrompt<Msg>)` with two closures (session/mux rename commits `CommitRename`, refusing an empty mux name; pin rename commits a new `CommitSlotRename`, refusing empty input with `pin: nothing entered`); remove `SlotPrompt`, `slot_prompt`, `RenameOverlayState`, and the `CommitRename` re-routing; drop the hand-posted status hint now that the prompt draws its keys.
5. Host: the worktree branch prompt is a `TextPrompt<Msg>` level of the worktree menu (Esc back to the list, Ctrl-C closes the menu, empty branch rejected in the prompt); the Controls max-age and contains editors are `TextPrompt` levels of Controls validated in `on_confirm` (parse_max_age stays in Conspectus; the Msg mapping reads the live context); the CSP-715 interim toasts go.
6. Remove `TextPromptState`, `TextPromptWidget`, `InputOutcome`; update pantry, registry/conventions/text_entry/app/ui tests deliberately.
7. ADR 0166 and index, ADR 0165/0151 status notes, framework architecture/README/frame/text_edit/keys pages, operator-review rows, CHANGELOG and docs/operations.md where an operator would notice.
8. After frames, compare by category; fmt, clippy, full isolated suite, cargo doc, git diff --check; final summary with Regressions and Consistency wins.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in four commits: the frame footer (371f4c5e), `tuikit::text_prompt` (2481b3c3), the host migration (38a13804), and ADR 0166 with the docs (a59a3d25).
Before/after frames: /tmp/tuifw-baseline/frames-734-{before,after} (23 states x 100x30 and 60x20; capture-734.sh, diff-734.sh). Every open-prompt frame differs (footer on the bottom border; error line for refusals; session rename's status hint gone); the five Esc frames are byte-identical (silent cancel, Esc back to the worktree list and to Controls). A live mux rename cannot be captured (the fixture has no live mux with tmux disabled); unit tests cover its refusal.
Checks: cargo fmt --check, clippy -D warnings, full isolated cargo test (3997 passed, 0 failed, 1 ignored), cargo nextest (3997 passed, 1 skipped), RUSTDOCFLAGS=-D warnings cargo doc, git diff --check: all pass. The page example in docs/controls/text_prompt.md was compiled and run as a temporary integration test, then removed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`TextPrompt<M>` (module `tuikit::text_prompt`) is the one prompt for one line of text: an `Overlay<M, Cx>` for any context with a title, an initial value, an optional placeholder, and `on_confirm: Fn(&str) -> PromptResult<M>` (`Commit(M)`, `Reject(String)`, `Stay`), shared behind an `Arc`. `Enter` asks `on_confirm`; a rejection shows one error line (in `error`) under the field until the next edit changes the text; `Esc` and `Ctrl-C` close silently; `Tab` is swallowed; `q` types. The prompt is the compact frame, 3 rows (title, field, the `TEXT_INPUT_KEYS` footer on the bottom border through the new `ModalFrame::footer`), 4 while an error shows; on a screen of 5 rows or fewer the reason takes the footer's place. Reference page `crates/tuikit/docs/controls/text_prompt.md`; ADR 0166 (amends 0158, narrows 0030).

All five surfaces moved: the session/mux rename and the pin rename are separate closures (`SlotPrompt`, `App::slot_prompt`, `RenameOverlayState`, and the `CommitRename` re-route are gone; a pin commits the new `Msg::CommitSlotRename`); the worktree branch prompt and the Controls max-age and contents editors are prompts held as levels of their overlays (the parent checks `Ctrl-C` first; `Esc` steps back to the list or the panel). `parse_max_age` and the `Msg` mapping stay in Conspectus; message wording is unchanged.

Verified with before/after `conspectus tui --snapshot` frames of each prompt (empty, valid, invalid, Enter, Esc, edit after a refusal) at 100x30 and 60x20 (`/tmp/tuifw-baseline/frames-734-*`), framework unit tests and a style golden, host tests for each surface's validation and for confirm, reject, cancel, and back, the conformance registry (which now checks the prompt's footer), fmt, clippy, the full isolated suite (3997 passed, 0 failed, 1 ignored), nextest, cargo doc, and `git diff --check`.

Tests updated deliberately: `max_age_invalid_value_keeps_editor_open` reads the error line instead of a toast; the pin-rename tests in `ui_tests.rs` drive the prompt instead of sending `CommitRename`, and a blank name is refused in the prompt; the `text_entry.rs` refusal checks assert the reason is inside the prompt with no toast; `input_tests.rs` moved into the framework's prompt tests; stack tests build `TextPrompt`s.

**Regressions**
- A prompt grows one row while an error line shows, re-centered (its top moves up a row).
- An empty pin rename and an empty mux rename keep the prompt open with the reason (`pin: nothing entered`, `mux rename: name cannot be empty`) instead of closing it with a toast.
- An invalid max age and an empty branch name show the reason on the error line (red) instead of a yellow-bordered toast.
- The rename prompt no longer puts `rename: Enter confirm · Esc cancel` on the status line; the status line keeps what it showed.
- On a screen of 5 rows or fewer a refusal replaces the key hints on the prompt's bottom border.

**Consistency wins**
- Every rejection explains itself in the prompt, until the next edit.
- All five prompts show their keys, generated from `TEXT_INPUT_KEYS`, on the bottom border (the branch, max-age, and contents prompts showed none).
- One prompt for five surfaces; the pin rename needs no routing flag.
- Cancel stays silent and `Esc` still steps back to the worktree list and to Controls (those frames are byte-identical before and after).

For the operator's review (see operator-review.md): the footer on the bottom border rather than a body row; the error line in `error` rather than `warning`; the nested prompts kept as levels until CSP-740/CSP-744; the mux rename's blank refusal; `rename_node` kept; the worktree menu's own status hint still showing while its branch prompt is open (CSP-759).
<!-- SECTION:FINAL_SUMMARY:END -->
