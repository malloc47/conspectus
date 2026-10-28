---
id: CSP-728
title: Extract the key-table model to `keys`
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-conventions
milestone: m-25
dependencies:
  - CSP-718
  - CSP-720
documentation:
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
  - docs/adr/0123-overlay-keymap-tables.md
priority: high
type: enhancement
ordinal: 814000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Overlay keys resolve through declarative tables (`keybindings/overlay.rs`, about 210 rows in 20 tables) that also generate footers and help, which the ecosystem survey found to be on the leading edge. The generic half imports only `KeyMatcher` and `key_label`, but the file also holds viewer, search, and worktree tables that import three app action enums (`overlay.rs:634-636`), the modal convention matches action `Debug` names, and the same key means different things across overlays: `Ctrl-G` jumps to the top of every pager, `G` is matched two ways (`AnyMod` versus `UpperChar`), Alt-G jumps to the bottom of Help but not of the viewer, and footer wording varies between `close`, `to close`, and `= merge & close` (inventory slice 06).

Scope: Create `tuikit::keys` with `KeyMatcher`, `KeyRow<A>`, `Hint`, `resolve`, `footer`, `key_label`, `mislabeled`, and `typed_char`, generic over the action type. Add standard tables for the menu, dialog, pager, text input, checklist, and form, a `KeyRole` classifier in place of `Debug`-name matching in `MODAL_CONVENTION`, and an availability field on a row so a footer or help page can dim an action that is currently unavailable (the seam for the gap analysis's tri-state availability). The viewer, search, and worktree tables move to the app with their action enums. Settle one rule for characters in overlays and write down each key that stops or starts working. Adopt one footer grammar: a short imperative verb with no "to".

TUI changes:
- Consistency wins: the same key does the same thing in every overlay; one footer grammar; a footer cannot advertise a key its overlay ignores
- Regressions or behavior changes: Alt-G stops jumping to the bottom of Help and Ctrl-G stops jumping to the top of pagers if the one-rule decision is plain `g` and `G`; the story lists each such key; footer wording changes in the overlays that used a different grammar

Tests and checks: The existing `overlay_keymap_hygiene` and `consistency_tests/{keys,help}` checks pass against the moved tables. A test fails when an advertised key does not resolve. Footers are checked in `--snapshot` for every overlay.

Blockers: The independence guard and the documentation skeleton.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `tuikit::keys` provides the generic key-table model, the standard tables, and the `KeyRole` classifier, with no app action enum in the framework
- [ ] #2 A key row can carry availability, and a footer and a help page dim an unavailable action
- [ ] #3 One rule for characters in overlays is documented, with every key that changes listed in the final summary
- [ ] #4 One footer grammar is used by every generated footer
- [ ] #5 The viewer, search, and worktree tables live in the app and use the framework model
- [ ] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
