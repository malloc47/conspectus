---
id: CSP-797
title: Decide whether the main screen's letters follow the one character rule
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 17:09'
updated_date: '2026-10-30 23:00'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-760
priority: low
ordinal: 883000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Overlays bind a character as typed (plain or with Shift, never a Control or Alt chord of it; ADR 0160 decision 3). The main screen's keymap, now one scoped `tuikit::keys::Keymap` (ADR 0192), kept its own matchers when it was extracted: most letters fire with any modifier but Control (`Alt-a` attaches, `Alt-p` pins), and the tree's `j`, `k`, and `G` fire with any modifier at all (`Ctrl-j` moves the tree). The extraction left this unchanged on purpose because changing it stops every `Alt` chord of a main-screen letter. Decide whether the program's keys adopt the overlay rule, and if so, which chords stop working and whether any should be bound on purpose.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The keymap's matchers for letters are one documented rule, the overlay rule or a stated exception
- [x] #2 Every key that stops or starts working is listed in the change's summary and in the CHANGELOG
- [x] #3 `keys::loose_characters`, or an equivalent keymap check, runs over the keymap's bindings in a test
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Decide by ADR 0151's consistency-first rule: adopt the overlay rule. No documented main-screen behavior uses a Control or Alt chord of a letter: the documented chords (`Ctrl-R`, `Ctrl-G`, `Ctrl-/`/`Ctrl-_`/`Ctrl-7`, `Ctrl-L`, `Ctrl-O`, `Ctrl-F`, `Shift-Enter`, `Alt-Enter`) are spelled-out bindings and keep working. The extra chords were matcher artifacts (and `Ctrl-J`, which many terminals send for `Ctrl-Enter`, moved the tree, where ADR 0192 made `Ctrl-Enter` do nothing).
2. `src/tui/keybindings.rs`: every character binding is `ch` (plain or with Shift); `unless_ctrl`, `any_mod`, and `plain` go. List every chord that stops working (Alt of f ! ? a i o w v r n m p u ] [ / s < > everywhere, Alt of h l g on the left pane, Ctrl and Alt of j k G on the left pane and of J K everywhere, and any other modifier a terminal reports such as Super) and what starts (q, ., 1-6 reported with Shift).
3. Framework: `Keymap::loose_characters` (the keymap twin of `keys::loose_characters`, by binding id and key), with a unit test and the keys page's seam updated; the host test runs it over `keymap()` and pins the rule (no binding ignores a chord of a character; spelled chords stay).
4. Update the tests that encode the old matchers (the Alt-g row in keybindings_tests, the left-tree chords in runtime_tests, the undo test's matcher).
5. New ADR 0197 (amends 0160 decision 3 and 0192 decision 6), README index, operator guide (the one character rule for the main screen), CHANGELOG (every key that stops or starts), keys.md, operator-review; snapshot that Alt-a and Ctrl-j do nothing and a and j work, at 100x30 and 60x20.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decision: adopt the overlay rule (ADR 0197). Clean fit by ADR 0151's consistency-first rule: no document, help row, or status hint named a chord of a main-screen character; the documented chords are bindings of their own. Implemented in 4d906d98 (`tuikit::keys::Keymap::loose_characters`, with a unit test and the keys page) and 52c9de42 (every character binding in `PROGRAM_KEYS` is `ch`; `unless_ctrl`, `any_mod`, `plain` removed). Tests: `every_character_binds_as_typed_and_every_chord_is_spelled_out` (keybindings_tests.rs) asserts `keymap().loose_characters()` is empty, presses Alt, Ctrl, Alt+Ctrl, Super, and Alt+Shift chords of every bound character on both panes (each reaches nothing or a binding that spells that chord), lists Alt-a, Alt-p, Alt-r, Alt-/, Ctrl-j, Ctrl-k, Ctrl-J as reaching nothing, and the spelled chords and shifted characters as reaching their bindings; `g_leaves_ctrl_g…` now expects Alt-g to do nothing; `chords_of_the_tree_keys_move_neither_pane` (renamed) expects the tree chords to do nothing on either pane; the undo test probes `ch(UNDO_KEY)`. Frames /tmp/tuifw-baseline/frames-797-after at 100x30 and 60x20: `<C-j>`, `<A-j>`, `<A-f>`, `<A-/>`, `<A-G>`, `<C-J>` leave the start frame byte-identical; `j`, `f`, `/`, `G`, `<C-g>` change it; `<Tab><A-j>` equals `<Tab>`. Help page and status bar unchanged (their goldens pass). Checks: fmt, clippy -D warnings, full isolated suite 4776 passed / 0 failed / 1 ignored, cargo doc -D warnings, examples, git diff --check, tuikit independence.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Decided: the main screen's keys adopt the overlays' one character rule (ADR 0197). It is a clean fit by ADR 0151's consistency-first rule: no documented behavior used a Control or Alt chord of a character, and the documented chords (`Ctrl-R`, `Ctrl-G`, `Ctrl-/`/`Ctrl-_`/`Ctrl-7`, `Ctrl-L`, `Ctrl-O`, `Ctrl-F`, `Ctrl-C`, `Shift-Enter`, `Alt-Enter`) are bindings of their own and keep working. Every character binding of the keymap is now `keys::ch` (typed alone or with Shift); the host's `unless_ctrl`, `any_mod`, and `plain` matchers are gone. No chord is bound on purpose in their place. The framework gains `Keymap::loose_characters`, the keymap twin of `keys::loose_characters`, which a host test runs over the keymap. ADR 0197 (amending 0160 decision 3 and 0192 decision 6), the ADR index, `docs/design.md`, the operator guide, the CHANGELOG, the keys page, and operator-review are updated.

Verification: `every_character_binds_as_typed_and_every_chord_is_spelled_out` checks `loose_characters` is empty and presses every chord of every bound character on both panes; the two tests that pinned the old chords now pin the rule. `conspectus tui --snapshot` at 100x30 and 60x20: `Ctrl-j`, `Alt-j`, `Alt-f`, `Alt-/`, `Alt-G`, `Ctrl-J` leave the screen byte-identical while `j`, `f`, `/`, `G`, and `Ctrl-G` act. fmt, clippy, full isolated suite (4776 passed, 0 failed, 1 ignored), cargo doc, examples, `git diff --check`, and the tuikit independence guard pass.

**Regressions**
- Keys that stop working (no modal open): on either pane, the Alt chord (and any Super or Meta chord) of `f`, `!`, `?`, `a`, `i`, `o`, `w`, `v`, `r`, `n`, `m`, `p`, `u`, `]`, `[`, `/`, `s`, `<`, `>` (so `Alt-a` no longer attaches and `Alt-p` no longer pins), and every chord but Shift of `J` and `K` (Preview scroll), Control included; on the left pane, the Alt chord of `h`, `l`, `g`, and every chord but Shift of `j`, `k`, `G`, Control included (`Ctrl-j`, which many terminals send for `Ctrl-Enter`, no longer moves the tree).

**Consistency wins**
- One rule for characters on every surface: a letter means the letter on the main screen, on both panes, and in every overlay (`Alt-j` used to move the tree and nothing on the right pane).
- `Ctrl-Enter` sent as `Ctrl-j` no longer moves the tree, matching `Enter`'s binding, which takes no modifier.
- Keys that start working: `q`, `.`, and `1` to `6` when a terminal reports them with Shift.
<!-- SECTION:FINAL_SUMMARY:END -->
