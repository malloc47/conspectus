---
id: CSP-784
title: Fit a footer too tall for its frame one way in every framed control
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29 18:15'
updated_date: '2026-10-30 18:54'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 870000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When a modal's footer has more rows than its frame leaves for it, the framed controls disagree about which rows stay. The pager (and so the help page, the value modal, and every framed reader) keeps the footer's last rows, the keys that leave, and scrolls its first rows with the body (ADR 0178). Forms keep the footer's first rows and drop the rest (FormBody::bands), dialogs and checklists keep the bottom rows, and palettes cap the footer at four rows. On a short terminal the same footer therefore loses different keys in different modals, against house rule 8 of ADR 0151. CSP-746 settled the rule for pagers and found the others while deciding it; CSP-781 settles the footer's look, not which rows stay.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every framed control (form, dialog, checklist, settings panel, palette, action menu, pager) keeps the same footer rows when the footer does not fit, by one shared framework rule
- [x] #2 The rule is recorded in an ADR and in each affected control's reference page
- [x] #3 Tests cover a too-tall footer for each framed control at a short size
- [x] #4 Every visible change is listed in the final summary under Regressions and Consistency wins, checked with conspectus tui --snapshot at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
One overflow rule for every framed control, in the `ModalFooter` that CSP-781 adds (one function, landed together):
1. `ModalFooter::fit(width, room)`: wrap between segments, then keep the status line, then the keys keep their last rows, the keys that leave, and their first rows spill behind a blank row after the body's last row, where they scroll with it (the CSP-746 pager rule, generalised); `fit_beside` lays it out a cell narrower when the body overflows beside the scrollbar. The body keeps at least one row in every control (the dialog used to give the footer all rows).
2. Every framed control fits through it: form (was: first rows kept), dialog, checklist, settings panel, palette (was: first four rows kept), action menu, pager, log list; the prompt's border footer keeps its last row.
3. A footer lists the keys that leave last, so the last rows hold them: reorder SETTINGS_KEYS (the note first) and the host's SEARCH_KEYS and SEARCH_CONTENT_KEYS (`Esc close` last); the pager and log list already compose that way.
4. Conformance: the footer's last segment on its last row is a key that leaves; at 40x10 the frame's last row still names a key that leaves. Tests: a too-tall footer for each framed control at a short size.
5. ADR 0193 records the rule; each control page says it.
6. Verify with `conspectus tui --snapshot` before/after at 100x30, 60x20, 40x10, 40x8, 60x12 (frames-781-*).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed with CSP-781 in one function, `ModalFooter` (one implementation, separate closes). Verified the CSP-746 pager rule and generalised it: the keys keep their last rows and the first rows spill behind a blank row after the body. In a list that follows its cursor the spill never shows (the body is down to its one row when anything spills); a dialog's body, a pager, and Messages' detail scroll to it. Keeping the last rows keeps the keys that leave only where a footer lists them last: `Footer::leaving_last` makes that hold for any table, including a host table that includes a standard one and adds rows after it (the palette page's example), and the conformance suite checks the drawn order and the 40x10 fit. Moving Controls' note first kept its footer at three rows at 40 columns (before `Esc close` it wrapped to four); the status bar then gives notes a rank a step below the keys so it still names them.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
One overflow rule for every framed control, in the `ModalFooter` that CSP-781 added (commits 056e470f, 29dc6c0d, eceac299, bdd4dfd7): `ModalFooter::fit` wraps the keys between segments; on a frame too short for them the blank row goes first, the status line (a form's error line, a reader's find prompt) stays, the keys keep their last rows, and their first rows spill behind a blank row after the body's last row, where they scroll with it (the CSP-746 pager rule, generalised). The body keeps at least one row everywhere; `fit_beside` lays the footer out a cell narrower beside a body that overflows; a bottom-border footer keeps its last row. The form (was: first rows kept), dialog, checklist, settings panel, palette (was: its first four rows), action menu, framed pager, log list, and prompt all fit through it. A footer lists the keys that leave last, so the kept rows hold them: every control takes its table's footer from `Footer::leaving_last`, `SETTINGS_KEYS` puts its note first, and the host's `SEARCH_KEYS` and `SEARCH_CONTENT_KEYS` list `Esc close` last; a status bar gives a footer's note up before its keys.

The rule is ADR 0193 (amending 0160, 0166, 0168, 0171 to 0174, 0176, 0178, 0179) and each control page says it (`frame.md` "The footer", action_menu, dialog, checklist, settings_panel, form, palette, pager, log_list, text_prompt). Tests: `frame/footer_tests.rs` covers the fit; each framed control has a too-tall footer test at a short size (menu, checklist, settings panel, and dialog at 14 or 16 by 6, form at 40x8, palette at 40x10, 30x8, and 20x6, pager at 30x4, log list at 40x6, prompt at 22 columns); the conformance `Footer` check fails a modal whose footer's last segment is not a key that leaves at 200x50, or whose frame's last row names no key that leaves at 40x10 (a flawed modal that keeps its first rows fails it).

Verification: `conspectus tui --snapshot` of every framed modal before and after at 100x30, 60x20, 40x10, 40x8, 60x12, and 36x12 (/tmp/tuifw-baseline/frames-781-*); fmt, clippy `-D warnings`, the isolated suite (4742 passed, 0 failed, 1 ignored), rustdoc `-D warnings`, `cargo build --examples`, and `git diff --check` pass.

**Regressions**
- Content search at 40x10 shows one result instead of two and its footer's last five rows: `Esc close` stays, but `↑/↓ move · Enter open hit` and `Tab search current/all` are below the fold (they kept the first four rows, which lost `Esc close` once the key moved last; before the move they held it).
- The pin edit form at 40x8 shows `S-Tab back · ←/→/Space change`, `Ctrl-R follow again · Enter save`, `Esc cancel` and no longer `↑/↓ move · Tab complete/next`.
- With `Esc close` last, the status bar under quick select and content search at 60 columns gives it up before the filter chip (the footer above it still names it).
- A dialog on a frame with a single inner row (a 40x5 screen) shows its body's first row instead of its keys; a prompt on a screen narrower than 34 columns shows `Esc cancel` on its border instead of `Enter confirm`.

**Consistency wins**
- A footer too tall for its frame keeps the same rows, its last, in every framed control: forms and the palettes keep `Esc cancel` / `Esc close` on a short screen (the pin edit form at 40x8 used to lose `Esc cancel`), as the pagers, Messages, menus, dialogs, checklists, and Controls do.
- Every footer ends with the key that leaves: quick select reads `↑/↓ move · Enter pick · Tab search current/all · Esc close`, content search ends `· Ctrl-S open Search view · Esc close`, and Controls reads `changes apply as you make them · ↑/↓ move · Space select/toggle · Enter apply & close · Esc close` (its status bar keeps naming the four keys).
- A dialog's spilled keys scroll after its body as a pager's do (behind a blank row in both).
<!-- SECTION:FINAL_SUMMARY:END -->
