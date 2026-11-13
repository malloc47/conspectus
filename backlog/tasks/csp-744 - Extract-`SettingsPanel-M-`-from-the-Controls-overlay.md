---
id: CSP-744
title: Extract `SettingsPanel<M>` from the Controls overlay
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 15:55'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-741
  - CSP-734
  - CSP-739
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/adr/0031-tui-filter-view-switching-and-per-view-state.md
priority: medium
type: enhancement
ordinal: 830000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Controls overlay is 1,344 lines: a sectioned panel with 24 rows, five row kinds (choice, checkbox, editor, cycle, button), and three sub-editor types, applying changes as you go. About 25 to 30% is generic (landable-row cursor, sections, row kinds, sub-editors, footer, scroll-to-cursor); the rest is Conspectus settings. Ctrl-C closes everything here but steps back in the worktree menu, it clones an 11-field context, and it has no paging keys despite about 30 rows (inventory slice 04).

Scope: Create `SettingsPanel<M>` with sections, the five row kinds, sub-editors that open a `Checklist` or a `TextPrompt`, scroll-to-cursor, and a generated footer; keep the "changes apply as you make them, Esc keeps them" behavior as first-class. The 24 rows, `ControlsContext`, and the filters stay in Conspectus. Apply the close-key and paging rules.

TUI changes:
- Consistency wins: paging keys on a 30-row panel; generated footer; Ctrl-C and Esc follow the shared rule
- Regressions or behavior changes: none expected

Tests and checks: The `controls_tests.rs` cases pass or are updated with a note. `--snapshot` of the panel and each sub-editor at 80x24 and 40x10.

Blockers: `Checklist`, `TextPrompt`, and `ActionMenu`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `SettingsPanel<M>` provides sections, five row kinds, sub-editors, and scroll-to-cursor
- [x] #2 The Controls overlay is a `SettingsPanel` instance and reads its context through `&Cx`
- [x] #3 `PgUp`, `PgDn`, `Home`, and `End` work on the panel
- [x] #4 The apply-as-you-go behavior is preserved and documented
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
1. Framework contract seams (tuikit::overlay, text_prompt): `Outcome::Back(M)` (close one level, then the host handles M: a step that applies and returns to the entry under it), `PromptResult::Back(M)`, and `Overlay::render_under_step` (the stack draws an entry with a step of its own over it this way, so a list shows its inactive cursor). Tests and the overlay/text_prompt pages.
2. `tuikit::keys::SETTINGS_KEYS` (`SettingsKey`): Ctrl-C/q dismiss, arrows/k/j/Ctrl-P/Ctrl-N move (wrap), PgUp/PgDn/Home/End/g/G page and ends (aliases, stop), Space change, Enter apply & close, Esc close, and the "changes apply as you make them" note.
3. `tuikit::settings_panel`: `SettingsPanel<Id, M, Cx>` over a host row model built from `&Cx` on every key and frame; titled sections; five row kinds (choice, checkbox, editor, cycle, button) as data with ids; a cursor over landable rows only (headers skipped), wrapping, paging by the rows shown; a kept `ScrollWindow` with the shared gutter; a fixed generated footer; editor rows that ask the host to open their editor as a step (`Stay`), whose Checklist/TextPrompt answers `Back`; `render_under_step` draws the cursor inactive. Apply-as-you-go: every change is a host message reported at once (`Stay`, or `Commit` for Enter's apply & close); staged apply recorded as a seam. Unit tests, goldens, conformance case, reference page `docs/controls/settings_panel.md`, architecture map, README.
4. Host: Controls becomes `SettingsPanel<ControlsCursor, Msg, App>`; `controls.rs` keeps ControlsContext, the 24 rows (as a row model), the filter mappings, and builds the editors (`Msg::OpenControlsEditor`, opened as `Modal::Prompt` / new generic `Modal::Checklist` steps with `Back` answers). Delete the hand-rolled cursor/body/footer/scroll and `CONTROLS_KEYS`. Forward `render_under_step` in `modals!`. Update controls_tests.rs, consistency tests (registry steps, conventions, layout builders, text_entry, content_filter, content_interactions), pantry.
5. Capture before/after frames (100x30, 60x20, 40x10) and classify differences; ADR 0176 (amends 0031, 0160, 0161, 0166, 0172), design.md, operations.md, CHANGELOG.md, operator-review.md.
6. fmt, clippy, full isolated suite, cargo doc -D warnings, build examples, git diff --check; finalize the story.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Commits: f567c969 (Outcome::Back, PromptResult::Back, Overlay::render_under_step), 014bd8ae (SettingsPanel, SETTINGS_KEYS, page), fd6bc1ec (conformance Case::step), 29246483 (Controls on SettingsPanel), e3e802c0 (ADR 0176 and docs, CSP-783 filed), c9b980c8 (checklist docs).
Judgment calls (operator-review rows): Stay rather than Commit for a change that keeps the panel open; editors as stack steps with a new Outcome::Back; render_under_step so the cursor still recedes behind an editor, plus Case::step in the conformance suite; row ids and a closure row model; plain generated footer; status line cleared on leaving an editor; name column fixed at 12.
Validation: cargo fmt --check; cargo nextest (isolated) 4306 passed, 1 skipped; cargo clippy --workspace --all-targets --all-features -D warnings; full isolated suite (run-tests.sh) 4306 passed, 0 failed, 1 ignored; RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps; cargo build --examples; git diff --check; independence guard; 177 snapshot frames per binary at 100x30, 60x20, 40x10 compared (rows-744.py: every panel row's words unchanged).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::settings_panel` and moved the Controls overlay onto it (ADR 0176).

Framework:
- `SettingsPanel<Id, M, Cx>` with `Section`, `Row`, `Kind`, and `SETTINGS_SIZE` (the menu size). The host's row model, a closure over `&Cx`, builds the sections on every key and every draw. The cursor is the host's row id: it skips headings, wraps (rule 2), and pages with `PgUp`/`PgDn` by the rows shown and `Home`/`End`/`g`/`G`, stopping at the ends (rule 12). There are five row kinds (choice, checkbox, editor, cycle, button). The panel keeps its `ScrollWindow` with the shared gutter, keeps the generated footer at the bottom, cuts rows with `…`, and brings a section's heading into view with its first row.
- Changes apply as they are made: acting on a row reports its message at once with `Stay`. `Enter` on a choice, and a row that `leaves`, report `Commit`. `Esc` closes and `q`/`Ctrl-C` dismiss, all keeping what was applied. The reference page records staged apply as a seam.
- Editors are steps. An editor row asks the host to open a `Checklist` or a `TextPrompt` with `push_step`, and the editor answers with the new `Outcome::Back` (or `PromptResult::Back`), which closes one level so the panel stays open. The stack draws an entry under a step of its own with the new `Overlay::render_under_step`, so the panel's cursor recedes. `testkit::conformance::Case::step` accepts that restyle.
- `keys::SETTINGS_KEYS`. The reference page is `docs/controls/settings_panel.md`. The overlay, prompt, keys, testkit, architecture, and README pages are updated.

Host: `Modal::Controls` holds `SettingsPanel<ControlsCursor, Msg, App>`, built by `controls::panel(App::controls_context)` and opened on the active view's row (`f`, as before). `ControlsContext`, which rows each view shows (`controls::sections`), the filter mappings, every `Msg`, and the editors stay in `controls.rs`. `Msg::OpenControlsEditor` opens a `Modal::Checklist` (new generic step) or a `Modal::Prompt` step. Removed: `ControlsOverlayState`, `ControlsOverlayWidget`, `ControlsOutcome`, `SubEditor`, `CONTROLS_KEYS`, and `ControlsKey`.

Tests: `controls_tests.rs` keeps every case, mapped to the new outcomes, plus paging and row-wording tests. The registry checks the editors as steps. The conformance registry builds them and checks Controls' selection. The selection check skips a scrollbar column. The tuikit tests include goldens and conformance cases. Frames: 177 per binary in `/tmp/tuifw-baseline/frames-744-{before,after}`. A row-by-row check finds every row's words unchanged.

**Consistency wins**
- The footer stays at the bottom at every size (rule 8). Before, it scrolled away and never showed at 100x30, 60x20, or 40x10. It is now plain, as in menus and checklists, where it was dimmed.
- A scrollbar shows beside sections that overflow.
- `PgUp`/`PgDn`, `Home`/`End`, and `g`/`G` move the cursor (rule 12).
- Scrolling moves only as far as the cursor needs, and wrapping to the top shows `View`.
- Long rows end in `…` instead of being clipped (rule 14).
- Close keys follow rule 4, unchanged since CSP-729 (verified): `Esc` closes the panel or steps back from an editor; `q` and `Ctrl-C` close everything, and `q` types in a prompt.

**Regressions**
- At 40x10 the footer takes three rows, so three panel lines show instead of six.
- In the live TUI, leaving an editor clears the `controls: …` status hint, as worktree steps do.
<!-- SECTION:FINAL_SUMMARY:END -->
