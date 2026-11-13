---
id: CSP-781
title: Give every modal footer the same look
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29 13:01'
updated_date: '2026-10-30 18:52'
labels:
  - tui-framework
  - tuifw-conventions
milestone: m-25
dependencies:
  - CSP-742
priority: medium
type: enhancement
ordinal: 867000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Modal footers come in two looks after the control extractions. Menus, dialogs, and checklists draw the footer in the plain style under a blank row; forms and palettes draw it dimmed. The footer is generated from the same key tables everywhere, so the look should be one thing too (house rule 8 of ADR 0151). The CSP-742 implementer noticed this while moving the picker onto Palette. Decide which look wins, make it a single framework function every footer goes through, and keep the blank-row spacing consistent.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 One framework function draws every modal footer, and menus, dialogs, checklists, forms, palettes, prompts, and pagers all use it
- [x] #2 The footer's style and the blank row above it are the same in every modal, checked with conspectus tui --snapshot at a normal and a narrow width
- [x] #3 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
One framework footer for every modal, shared with CSP-784 (same function, two rules):
1. Add `tuikit::frame::ModalFooter`: lays a `Footer` out in one look (the theme's `muted`, no color of its own, as the status bar draws hints) under a blank row, with an optional status line (a form's error line, a pager's find prompt or chip) above the keys; `border(width)` for a footer on the bottom border (the prompt).
2. Route every framed control's footer through it: ActionMenu, Dialog, Checklist, SettingsPanel, Form (FormBody takes a ModalFooter; `form::footer_lines` removed), Palette, Pager (framed), LogList, TextPrompt.
3. One blank-row rule (the menus' rule): the blank row is the footer's first row and gives way first, only when the footer does not fit the rows the body leaves; so every modal shows it at a normal size (Controls, help, palettes, Messages, the value modal gain it; dialogs, checklists, and Controls keep it while their body scrolls).
4. Conformance `Footer` check: in a frame, every segment in the footer look (muted, regular weight, the color of its place), on the frame's last rows under a blank row; a control that deviates fails. Flawed test modals for each rule.
5. Update goldens/snapshots deliberately; docs: frame.md and each control page; ADR 0193 (with CSP-784); operator-review rows.
6. Verify with `conspectus tui --snapshot` before/after at 100x30, 60x20, 40x10 (frames-781-*).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Solved together with CSP-784 in one framework function (`ModalFooter`), as requested: this task's part is the look (muted, no color of its own) and the blank row (gives way first, only when the footer does not fit); CSP-784's is which rows stay and the keys-that-leave-last order. Chose the receding look over the menus' plain one (it matches the status bar's hints and the forms, and separates keys from entries) and the menus' blank-row rule over the dialogs' (which hid the row whenever a body scrolled, so Controls at 100x30 had none). The full-screen pager's one-row footer is screen chrome and keeps its look. Snapshots of every framed modal before and after at 100x30, 60x20, 40x10, 40x8, 60x12, 36x12 in /tmp/tuifw-baseline/frames-781-*; full isolated suite 4742 passed, 0 failed, 1 ignored (one `discovery::forge` timing flake on an intermediate run, passing on rerun).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::frame::ModalFooter`, the one function every modal footer goes through (commits 056e470f, 29dc6c0d, eceac299, bdd4dfd7, shared with CSP-784, which settles which rows stay): the action menu, dialog, checklist, settings panel, form (`FormBody` takes a `ModalFooter`; `form::footer_lines` is removed), palette, framed pager (the help pages, the viewers' help, the value modal), log list (Messages), and the prompt's bottom border (`ModalFooter::border`).

The one look: the theme's `muted` with no color of its own (the status bar's hint look), on the frame's last rows under a blank row; the blank row gives way first, and only when the footer does not fit the rows the body leaves (the menus' rule). The conformance `Footer` check now fails a framed modal whose keys are out of that look, off the frame's last rows, or have no blank row above them (`footer_look` in `testkit/conformance/checks.rs`, with a flawed test modal per rule). ADR 0193 records it; `frame.md` gains "The footer", and every control page points at it.

Verification: `conspectus tui --snapshot` of every framed modal reachable from the showcase fixture (row, mux, and worktree menus; the worktree dialogs and branch prompt; Clean up; Controls and its checklist and prompt editors; quick select, content search, the picker; the three forms; the rename prompts; help, the viewer's help, the value modal, Messages) before and after at 100x30, 60x20, and 40x10 (and 40x8, 60x12, 36x12), frames in /tmp/tuifw-baseline/frames-781-{before,after}; the ANSI frames show the dim modifier on every footer. `cargo fmt --check`, clippy `-D warnings`, the isolated suite (4742 passed, 0 failed, 1 ignored), `cargo doc` with `-D warnings`, `cargo build --examples`, and `git diff --check` pass.

**Regressions**
- Controls, the help pages, the viewer's help, quick select, content search, the picker, the value modal, and Messages gain a blank row above their keys, so their body is a row shorter (in the picker, the value modal, and Messages that row is usually empty already).
- On short screens dialogs, checklists, and Controls keep that blank row while their body scrolls, so they show one body row fewer: Clean up and the harness and mux-state checklists at 40x10 list two items, not three; the worktree remove dialog at 40x10 shows one of its two sessions, and at 40x8 its warning heading is below the fold.
- The prompts' keys on the bottom border recede (dim over the accent) instead of the plain accent.

**Consistency wins**
- One footer look in every modal: the row, mux, and worktree menus, the worktree dialogs and the Clean up notice, the four checklists, and Controls recede as the forms, the palettes, the readers, and the status bar's hints do.
- The blank row above the keys shows in every modal at every size its footer fits; Controls at 100x30, whose sections scroll, had none.
- The worktree merge dialog at 40x10 no longer draws a scrollbar beside a body that fits.
<!-- SECTION:FINAL_SUMMARY:END -->
