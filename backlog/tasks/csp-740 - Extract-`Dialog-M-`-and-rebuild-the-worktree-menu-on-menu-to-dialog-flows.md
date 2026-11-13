---
id: CSP-740
title: Extract `Dialog<M>` and rebuild the worktree menu on menu-to-dialog flows
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 11:07'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-739
  - CSP-734
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
priority: medium
type: enhancement
ordinal: 826000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Confirmation has four idioms. The worktree menu is a mode machine in one 677-line widget: a list, a text prompt, three y/n confirm sub-screens (remove, merge, prune), and an `m`/`d` close-down choice, with about 215 lines that query the graph. Ctrl-C means close-everything in Controls but back-one-level in this menu. The Clean up modal has its own empty-state notice (inventory slice 04).

Scope: Create `Dialog<M>` in three forms: confirm (`y`/Enter, `n`/Esc), choice (named verbs), and notice. Rebuild the worktree menu as an `ActionMenu` opening dialogs and a `TextPrompt` on the overlay stack, with Esc stepping back one level. The graph-querying context capture, the `WorktreeAction` policy, and the `Msg::CommitWorktree*` mapping stay in Conspectus. The Clean up empty notice becomes a `Dialog` notice.

TUI changes:
- Consistency wins: one confirm keymap in every dialog; a stack replaces the mode machine
- Regressions or behavior changes: Ctrl-C in the worktree menu closes it instead of going back one level

Tests and checks: Tests for each worktree flow (create, remove, merge, prune, close-down) through the stack. The `worktree_menu_tests.rs` cases pass or are updated with a note. `--snapshot` of each dialog.

Blockers: `ActionMenu` and `TextPrompt`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `Dialog<M>` provides confirm, choice, and notice forms with one key table
- [x] #2 The worktree menu is built from `ActionMenu`, `Dialog`, and `TextPrompt` on the overlay stack, and its mode machine is removed
- [x] #3 The graph-querying context and the worktree policy stay in Conspectus
- [x] #4 The Clean up empty-state notice is a `Dialog` notice
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
1. tuikit overlay flows: `Outcome::Dismiss` (Ctrl-C/q), `ModalStack::push_step`, `Settled.closed` as a list; `Close` steps back one entry, `Commit` and `Dismiss` close the whole flow; `ActionMenu`, `TextPrompt`, and `Form` answer their abort keys with `Dismiss`; `Entry::opens_step` makes a pick report `Stay` so the menu stays under its next step. Adapt the host's route and Controls' sub-editors.
2. tuikit `dialog` module: `Dialog<M>` in confirm, choice, and notice forms over one standard table `DIALOG_KEYS` (`DialogKey`), compact frame, generated footer, body that scrolls under a fixed footer; tests, goldens, `docs/controls/dialog.md`, keys/overlay/architecture pages.
3. Host: the worktree menu becomes an `ActionMenu<Msg>` whose question entries open a `Dialog` or the branch `TextPrompt` as a step (`Msg::OpenWorktreeStep`, built from the captured context); `X` opens the close-down choice alone; the mode machine, `CLOSE_DOWN_KEYS`, `CONFIRM_KEYS`, and `ChoiceKey` go; `Modal::Dialog` and `Modal::Prompt` registered and checked; the Clean up empty notice becomes a `Dialog` notice.
4. ADR for the dialog and the flow rule (amending 0160, 0161, 0171), docs/worktrees.md, CHANGELOG, operator-review rows.
5. Before/after `--snapshot` frames of every flow at 100x30, 60x20, 40x10; fmt, clippy, full isolated suite, rustdoc, diff check; close the story.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slices: fced9cce (tuikit flows: `Outcome::Dismiss`, `ModalStack::push_step`, `Settled.closed` list, `Entry::opens_step`; menu/prompt/form answer Ctrl-C and q with Dismiss), c3f071c5 (`tuikit::dialog`: `Dialog<M>` confirm/choice/notice on `DIALOG_KEYS`, `Footer::only`, page `docs/controls/dialog.md`, goldens), 0b3ca549 (worktree menu = `ActionMenu<Msg>` + steps via `Msg::OpenWorktreeStep`; `Modal::Dialog`, `Modal::Prompt`; `X` opens the choice alone; empty Clean up notice; `CONFIRM_KEYS`, `ChoiceKey`, `CLOSE_DOWN_KEYS` removed), 71954d5b (ADR 0172, design, operator review).
Rule for Ctrl-C across two entries (the CSP-734 question): a step's Close steps back; Commit and Dismiss close the whole flow and nothing under it.
Dialogs are menu-sized: the compact size left the menu's border and cursor mark showing beside a dialog.
Conformance finds frames by differing corners, so a step drawn exactly over its menu is invisible to it; the host's step cases use three live sessions and the 40x10 body check closes the whole flow. Filed CSP-780.
Frames: /tmp/tuifw-baseline/frames-740-before, -after, and frames-740-diff.txt (categories).
Checks: fmt clean; clippy --workspace --all-targets --all-features -D warnings clean; full isolated suite 4175 passed, 0 failed, 1 ignored; RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps clean; git diff --check clean; tuikit independence guard 36/36.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::dialog` (`Dialog<M>` in confirm, choice, and notice forms on one standard table, `DIALOG_KEYS`: `Enter`/`y` confirm, `Esc`/`n` cancel, `Ctrl-C`/`q` dismiss, arrows and `k`/`j` scroll an overflowing body; generated footers; menu-sized frame; reference page `crates/tuikit/docs/controls/dialog.md`) and stacked steps in `tuikit::overlay` (`ModalStack::push_step`, `Outcome::Dismiss`; a step's `Close` steps back, its `Commit` or `Dismiss` closes the whole flow; `Entry::opens_step` keeps a menu open under its question). The worktree menu is an `ActionMenu<Msg>` whose entries open the branch `TextPrompt` and the merge, remove, prune, and close-down dialogs as steps (`Msg::OpenWorktreeStep`, `Modal::Prompt`, `Modal::Dialog`); its mode machine, `CONFIRM_KEYS`, `ChoiceKey`, and `CLOSE_DOWN_KEYS` are gone. `context_for_node`, the `WorktreeAction` policy, the words, and the `Msg::CommitWorktree*` mapping stay in Conspectus. The empty Clean up is a `Dialog` notice. ADR 0172 (amends 0160, 0161, 0171); design, operations, worktree guide, changelog, and operator review updated. Filed CSP-780.

Verified: worktree flow tests through the stack (create, refused branch, merge, remove forced and not, prune, close-down m/d from the menu and from `X`, Esc back with the cursor kept, Ctrl-C/q dismissing, n stepping back), dialog unit, golden, and conformance tests, the host registry, conformance, text-entry, and scrolling suites with the new modals; `--snapshot` frames of every flow at 100x30, 60x20, and 40x10 before and after (/tmp/tuifw-baseline/frames-740-*). fmt, clippy (`--workspace --all-targets --all-features -D warnings`), full isolated suite (4175 passed, 0 failed, 1 ignored), nextest, rustdoc with `-D warnings`, `git diff --check`, and the independence guard pass. Ctrl-C from any step closes the worktree flow, as since CSP-729, now through the stack.

**Regressions**
- `Esc` from a worktree step back to the menu clears the menu's opening hint (`worktree: ↑/↓ move · Enter pick · Esc close`) from the status line; the menu's footer shows the same keys.
- The close-down and prune questions are no longer bold (dialog bodies are plain text).
- At 40x10 the close-down choice's list of live sessions is below the fold with no scrollbar drawn (four body rows; the shared gutter draws none under five); the arrows scroll it.
- At 40 columns the menu's cursor mark can show beside the branch prompt, which is narrower than the menu.

**Consistency wins**
- One keymap in every dialog: the close-down choice also cancels on `n` (`m merge & close · d discard & close · Esc/n cancel`); the Clean up notice's keys are generated (`Enter/Esc close`, was hand-typed `Esc/Enter close`) and `y`/`n` close it too.
- A stack replaces the mode machine: the branch prompt and the dialogs open over the menu, which shows around the prompt (and above and below a dialog shorter than it); `Esc` returns to the menu with its cursor where it was.
- On short screens every confirmation, the close-down choice, and the Clean up notice keep their keys (at 40x10 they all lost them, the close-down choice showing no way to answer); blank rows give way first and the body scrolls under the footer.
- Dialogs gain a blank row above their text, like menus; the live-session heading takes the theme's `warning` with bold and wraps under its first word; prune's hand-broken explanation is one wrapped sentence.
- `X` opens the close-down choice on its own without a status-line hint, and `Esc` closes it (it showed a one-entry menu).
- On a worktree with no branch checked out, Remove worktree and Close down stream are shown disabled with the reason (the old confirmation closed silently on `y`).
<!-- SECTION:FINAL_SUMMARY:END -->
