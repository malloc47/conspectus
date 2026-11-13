---
id: CSP-760
title: Design and extract the scoped global keymap and generate the main help from it
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 17:22'
labels:
  - tui-framework
  - tuifw-chrome
milestone: m-25
dependencies:
  - CSP-759
  - CSP-730
  - CSP-710
documentation:
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
priority: medium
type: enhancement
ordinal: 846000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The global keymap (`KEYBINDINGS`) has a single `KeyMode`, 58 `help_text` strings that are never rendered, seven keys outside the table, and a duplicate-key test that misses shadowed rows. The main `?` page is 48 hand-written rows with a one-directional drift test. Several keys change meaning by focus (`F`, Enter, Backspace). It is wired into `Action`, `Msg`, and `View`, so the framework designs a scoped keymap fresh and Conspectus adopts it (inventory slice 06, section 1).

Scope: Add a scoped keymap to `tuikit::keys` (modes: global, pane focus, overlay) whose rows carry help text that renders, with collision and shadow detection at construction and stable ids that leave room for user remapping later. Adopt it in Conspectus, declare the same-key-two-meanings in one table, and generate the main help page from the table with the hand-written prose rows kept as annotations.

TUI changes:
- Consistency wins: the main help page and the status bar always match the keys; shadowed and duplicate keys fail at construction
- Regressions or behavior changes: the wording of some main-help rows changes as prose rows become table descriptions

Tests and checks: The keymap drift tests are replaced by two-way tests. `--snapshot` of the main help page.

Blockers: The status bar and help extractions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The scoped keymap has modes, rendered help text, and collision and shadow detection, and the global table uses it
- [x] #2 The main help page is generated from the table, with prose rows kept as annotations
- [x] #3 Rows have stable ids, so a later user remap needs no table change
- [x] #4 The two-way drift test replaces the one-directional test
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
1. Framework (tuikit::keys): a scoped keymap. `Scope` (global, a named pane, a named overlay), `ActiveScopes` (overlay, then pane, then global), `Bind` const declarations (stable id, scope, keys, action, short verb, help section and text, status rank, optional availability predicate over the host's context), `Keymap::builder()` taking declarations and a control's own key table (`table`, one binding per listed action), and `build()`, which fails on duplicate or malformed ids, undescribed bindings, keyless bindings, and any two keys in one scope that can match one key event (collisions and shadowing). `KeyMatcher::overlaps`. `resolve` walks the active scopes.
2. Framework (tuikit::help, tuikit::status_bar): `help::keymap_sections` generates a page from a keymap with `HelpAnnotation`s (more prose for a row, a row of its own after a row, a section of its own) and `unanchored` finds annotations naming no binding; `KeyHintBar::keymap` names the active scopes' bindings with verbs, annotated by the host, leaving out what the predicate says cannot act now and what a higher scope takes. Tests, docs (keys.md scoped keymap section, help.md, status_bar.md, architecture.md).
3. Host: `Command` (Copy) replaces `MainKey`; one keymap (`keybindings::keymap()`) of global, left-pane, and right-pane scopes (the right pane is `RIGHT_PANE_KEYS` taken in), with the seven hand-matched keys brought in, `guard_focus`/`right_pane_action`/`translate_via_table`/`KEYBINDINGS`/`MAIN_VIEW_KEYS` removed, `action_for_event` resolving the keymap by focus. Fix the shadowing the check finds (`g` vs `Ctrl-G`, `Enter` vs `Shift-Enter`/`Alt-Enter`).
4. Host: the status bar reads the keymap; the main help page is generated from it, the hand-written prose kept only as annotations; replace the one-directional drift test with a two-way test; update the parity, lifecycle, undo, and runtime tests mechanically.
5. Before/after `--snapshot` frames of the help page (every page) and the status bar (every view and focus) at 100x30, 60x20, 40x10; ADR 0192, operator review, CHANGELOG/operations; full checks.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Framework slice 5b05df3a: keys::keymap (Scope, ActiveScopes, Bind/bind, Keymap, KeymapBuilder with rows/table/build, Binding, Conflict/Conflicts), KeyMatcher::overlap, help::keymap (HelpAnnotation, keymap_sections, keymap_sections_in, unanchored), KeyHintBar::keymap, reference pages. Host slice 92cb8b2f (+ doc-link fix 347adeda): keybindings::keymap() with Command, PROGRAM_KEYS, the right pane table taken in, AFTER_RIGHT_PANE_KEYS; translate_in; status_hints in four groups; generated help page with annotations; two-way test. Docs 98519a4c (ADR 0192). The build check found two real overlaps (g vs Ctrl-G, Enter vs Shift/Alt-Enter), fixed. Before/after frames: 132/132 status frames identical; help rows changed as listed. Full isolated suite 4721 passed, 0 failed, 1 ignored; nextest 4721 passed, 1 skipped. Filed CSP-797.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The main screen's keys are one scoped keymap, and the `?` page and the status bar are generated from it (ADR 0192).

Framework (`5b05df3a`): `tuikit::keys` gains `Scope` (global, a named pane, a named overlay mode), `ActiveScopes` (overlay, then pane, then global), `bind`/`Bind` const declarations (stable id, keys, action, verb, help section and text, status rank, optional availability predicate over the host's context), `Keymap`/`KeymapBuilder`/`Binding`, and `Conflict`/`Conflicts`. `build` refuses duplicate or malformed ids, keyless or undescribed bindings, and any two keys of one scope that one key event fires (`KeyMatcher::overlap`), in either order, so no key shadows another and the table's order is the page's. `KeymapBuilder::table` takes a control's table in as a pane's scope. `help::keymap_sections` generates a page from a keymap, with `HelpAnnotation`s (more prose, a row after a row, a section) anchored by id and `unanchored` to find orphans; `KeyHintBar::keymap` names the active scopes' bindings. Reference pages: keys.md (scoped keymap section), help.md, status_bar.md, explorer.md, architecture.md.

Host (`92cb8b2f`, `347adeda`): `keybindings::keymap()` holds the program's, the left pane's, and the right pane's (`RIGHT_PANE_KEYS` taken in) bindings; `Command` replaces `MainKey`; `KEYBINDINGS`, `KeyMode`, `translate_via_table`, the seven hand-matched keys, `right_pane_action`, `guard_focus`, `MAIN_VIEW_KEYS`, and its parity test are gone. The status bar reads the keymap in four groups; the help page is generated, its prose kept only as annotations (other overlays' keys in four rows, `/ then Tab`, the four legends). Docs (`98519a4c`): ADR 0192, amendments to 0160/0162/0188/0191, design, operations, CHANGELOG, inventory, operator review.

Verified with `conspectus tui --snapshot` at 100x30, 60x20, 40x10: every help page (each PgDn to the end, G, past the end, back) and the status bar in every view and focus, on each row kind, with each reachable overlay (frames in `/tmp/tuifw-baseline/frames-760-{before,after}`). Status bar: 132 of 132 frames byte-identical. Help: 15 identical, 70 differ and 4 pages added, all from the row changes below.

**Regressions**
- `Ctrl+Enter` on the left pane no longer runs the row's default action: `Enter` binds no modifier there, as on the right pane, which the overlap check required beside `Shift-Enter`/`Alt-Enter` (reveal).
- The help page is longer: 16 pages at 60x20 (15), 55 at 40x10 (52), 6 at 100x30 as before.
- Rows that described two keys in one sentence are two rows with half the sentence each (below).

**Consistency wins**
- The help page and the status bar are generated from the keys, so they always match them; the keys that change meaning by focus (`F`, `Enter`, `Backspace`, moves, folds) are two bindings of one table.
- Shadowed and duplicate keys fail at construction; the two it found are bound so they cannot overlap (`g` with any modifier but Control, so `Ctrl-G` stays grouping's; `Enter` above).
- Help rows, before then after:
  - `1 / 2 / 3 / 4` "Switch directly to the sessions / mux / PRs / pinned view" -> `1`..`4` "Switch directly to the sessions view" (mux, PRs, pinned).
  - `] / [` "Cycle to next / previous view" -> `]` "Cycle to the next view", `[` "Cycle to the previous view".
  - `Ctrl-/ (or Ctrl-_ / Ctrl-7) / s` -> `s / Ctrl-/ / Ctrl-_ / Ctrl-7` (text unchanged).
  - `< / >` "...show the previous (older) or next (newer) remembered search..." -> `<` "...show the previous (older) remembered search without searching again; Controls lists them", `>` "...show the next (newer) remembered search without searching again; Controls lists them".
  - `j / k / ↓ / ↑` "Move selection down / up" -> `↑ / k` "Move the selection up", `↓ / j` "Move the selection down".
  - `l / → / h / ←` "Expand / collapse the selected left-tree row (vi-style fold)" -> `← / h` "Collapse the selected left-tree row (vi-style fold)", `→ / l` "Expand ...".
  - `PgDn / PgUp` "Page through the row tree" -> `PgDn` "Page down through the row tree", `PgUp` "Page up through the row tree".
  - `g / G` "First / last row" -> `Home / g` "Jump to the first row", `End / G` "Jump to the last row".
  - `J / K` "Scroll the right-panel preview" -> `J` "... preview down", `K` "... preview up".
  - Navigation `Enter` "Left tree: row-kind default action (attach / view / rebuild / expand); on the right pane, see its keys below" -> removed; the Actions `Enter` row stays.
  - New rows for keys that were bound but unlisted: `o` (Actions) "Open the full untruncated value of the right pane's cursor row in a modal, from either pane"; `Backspace` (Navigation) "Return to where the last reveal started".
  - Every other row, the right pane's section, the legends, and the footer read as before.

Test expectation changes: the one-way drift test became `the_keymap_page_and_the_keymap_agree_both_ways`; the help needles follow the split rows; the `MAIN_VIEW_KEYS` parity test went and `every_key_the_status_bar_names_does_what_it_says` now presses each hint key through the loops and requires the binding the keymap resolves; the focus-guard assertions became left-pane-scope assertions; the undo, lifecycle, refresh, and Ctrl-/ tests read the keymap; the overlay registry drops `MAIN_VIEW_KEYS`.

Checks: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`, the isolated full suite (4721 passed, 0 failed, 1 ignored), `cargo nextest run` (4721 passed, 1 skipped), `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `cargo build --examples`, the independence guard, and `git diff --check` all pass. Follow-up: CSP-797 (whether the main screen's letters follow the one character rule). Operator review: ten CSP-760 rows.
<!-- SECTION:FINAL_SUMMARY:END -->
