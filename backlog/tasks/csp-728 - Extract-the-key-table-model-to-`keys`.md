---
id: CSP-728
title: Extract the key-table model to `keys`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 20:37'
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
- [x] #1 `tuikit::keys` provides the generic key-table model, the standard tables, and the `KeyRole` classifier, with no app action enum in the framework
- [x] #2 A key row can carry availability, and a footer and a help page dim an unavailable action
- [x] #3 One rule for characters in overlays is documented, with every key that changes listed in the final summary
- [x] #4 One footer grammar is used by every generated footer
- [x] #5 The viewer, search, and worktree tables live in the app and use the framework model
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
1. Add the arrow-key marks and the spaced separator to `tuikit::glyphs::marks` so key labels and the hint separator are drawn through the glyph module.
2. Create `tuikit::keys`: `KeyMatcher` (Char, Exact, AnyModExceptCtrl, AnyMod) with one label notation (`key_label`), const row builders, `KeyRow<A>` (Bind with `available`, Note, Include), `Hint`, `resolve`, `rows`, `with_availability`, `Footer`/`footer`/`HINT_SEPARATOR`/`unavailable_style`, `mislabeled`, `unresolved`, `typed_char`, the `KeyRole` classifier (`KeyAction` trait) and the generic `Convention` check; standard enums and tables for the menu, confirm dialog, choice dialog (`ChoiceKey<C>`), pager, text input, checklist, and form. Unit tests; register the files in `KEY_MODEL_FILES`; reference page `docs/controls/keys.md`; flip the module map.
3. Host: `keybindings.rs` and `keybindings/overlay.rs` keep only app tables (search, picker, Controls, help, value modal, Messages, close-down, the three forms on the standard form rows, omnibox, viewers, find) and `MODAL_CONVENTION`, with `KeyAction` impls for app enums; widgets import framework items from `tuikit::keys`.
4. One rule for characters (`ch` is plain or Shift only; chords are spelled), Ctrl-N/Ctrl-P in every list, Esc/Enter wording, and one footer grammar (imperative verb, no "to", no "="); update the tests that encode the old semantics deliberately.
5. Tests: every registered table's advertised keys resolve (plus a scan that every `*_KEYS` table is registered), Ctrl-C first, the convention by role; help pages and footers dim an unavailable action.
6. ADR 0160 (+ index, ADR 0123/0151 notes), operations.md and CHANGELOG key notes, operator-review entries.
7. Before/after `--snapshot` frames at 100x30 and 60x20; fmt, clippy, full isolated suite, cargo doc, git diff --check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in four slices: the framework module (6e703a8), the host on it with the one character rule, Ctrl-N/Ctrl-P in every list, and one footer grammar (26f2357), help-page availability dimming (c027460), and ADR 0160 with the operator docs (f38c70f).

Decisions: a letter binds plain or with Shift only (applied to every character row, not only g/G); Ctrl-N/Ctrl-P in every list with a cursor, never in a pager; Esc close/cancel/back by what leaving costs; tables compose with `KeyRow::Include` so the help page, value modal, and forms build on the standard rows instead of copying them; availability is a bool per bound row set through `with_availability`, and `resolve` ignores it so an overlay can explain itself.

Verification: full isolated suite 3809 passed, 0 failed, 1 ignored (22 new tests); fmt, clippy -D warnings, cargo doc -D warnings, git diff --check, and the independence guard pass. Before/after `--snapshot` frames of 37 overlay states at 100x30 and 60x20 are in /tmp/tuifw-baseline/frames-728-{before,after}; 30 frames changed, all in footer text only (ANSI identical elsewhere). Probes with the after binary: Ctrl-N/Ctrl-P move the row menu and Controls; Ctrl-G no longer jumps to the help page's top while g does.

Follow-up filed: CSP-776 (use availability in Messages and pin edit; draw every overlay footer through Footer).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tuikit::keys` now owns the key-table model: `KeyMatcher` (one rule for characters), `key_label` (one notation), const builders, `KeyRow<A>` with `Include` and an `available` flag, `Hint`, `resolve`, `rows`, `with_availability`, `Footer`/`footer`/`HINT_SEPARATOR`/`unavailable_style`, `typed_char`, `KeyRole` + `KeyAction`, the modal key `Convention` checked by role, and the checks `mislabeled`, `unresolved`, `loose_characters`, `ctrl_c_first`. Standard tables and framework enums: `MENU_KEYS`, `CONFIRM_KEYS`, `ChoiceKey<C>`, `PAGER_KEYS`, `TEXT_INPUT_KEYS`, `CHECKLIST_KEYS`, `FORM_KEYS` + `FORM_OPTION_KEYS`. The host keeps its own tables (search, picker, Controls, help, value modal, Messages, close-down, the three forms, omnibox, viewers, find) in `tui::keybindings::overlay`, built on the standard rows, with `KeyAction` impls and `MODAL_CONVENTION` as data. ADR 0160; reference page `crates/tuikit/docs/controls/keys.md`.

**The rule for characters**: in an overlay a letter fires plain or with Shift, never with Ctrl or Alt; chords are spelled. `Ctrl-N`/`Ctrl-P` move every list with a cursor; pagers bind neither.

Keys that stop working:
- Help page and value modal: `Ctrl-G`/`Alt-G` (top), `Alt-G`/`Ctrl-Shift-G` (bottom), Ctrl/Alt with `j`, `k`, `Space`, `q`, `?` (help), `o` and `y` (value: `Ctrl-Y` copied).
- Messages: Ctrl/Alt with `j`, `k`, `g`, `G` (`Alt-G`), `J`, `K`, `Space`, `q`, `!`; `Alt-Y` copy.
- Row, mux, worktree menus and empty Clean up: Ctrl/Alt-`j`/`k`. Controls and the checklists: Ctrl/Alt-`j`/`k`/`Space`. Search options: Ctrl/Alt-`j`/`k`.
- Worktree confirmations: Ctrl/Alt-`y` (confirm), Ctrl/Alt-`n` (cancel). Close-down: Ctrl/Alt-`m`, Ctrl/Alt-`d` (discard), Ctrl/Alt-`q`.
- Mux launch and pin edit forms: Ctrl/Alt-`Space`.
- Transcript viewer: Ctrl/Alt with `q`, `j`, `k`, `Space`, `g` (`Ctrl-G` top), `t`, `/`, `n`, `?`. Pane viewer: the same plus `a` (attach).

Keys that start working: `Ctrl-N`/`Ctrl-P` in the row, mux, and worktree menus, empty Clean up, Controls, the checklists (Controls filters, Clean up), Messages, and the new mux, mux launch, and pin edit forms' fields.

**Regressions**
- The chords listed above no longer act in overlays.
- The content search footer takes three rows at 100x30 (was two) and four at 60x20 (was three); the spare row above it absorbs this at those sizes.

**Consistency wins**
- One footer grammar (house rule 8). "to" dropped: help and viewer help `j/k/PgDn/PgUp scroll · Esc/q/? close`; worktree remove/merge/prune `Enter/y confirm · Esc/n cancel`. "=" dropped: close-down `m merge & close · d discard & close · Esc cancel`. Verbs for noun segments: `Tab search current/all`, `S-Tab list sessions/messages`, `Ctrl-R recall search`, `Ctrl-O set options`, `Ctrl-S open Search view` (quick select, content search), `←/→/Space change` (mux launch, pin edit).
- The value modal builds on the pager rows: `j/k/PgDn/PgUp scroll · Esc/q/o close · y copy` (the help's order, PgDn/PgUp shown).
- The same key does the same thing in every overlay; `Ctrl-G` no longer means both "group" and "top"; a destructive `d` needs the plain letter.
- A footer cannot advertise a key that an earlier row takes (`unresolved`, registry test over every table plus a scan for unregistered tables).
- Help pages and footers dim an unavailable action (`inert` + `muted`); nothing sets it yet (CSP-776).
- Unchanged frames: menus, Controls and its editors, picker, search options, new mux form, rename prompts, Messages, transcript viewer and find prompt, Clean up.

Checked with `conspectus tui --snapshot` at 100x30 and 60x20 (37 states each; 30 frames differ, footer text only, styles identical); the pane viewer is not reachable in snapshot mode and is covered by its help snapshots and tests.

Tests changed deliberately: generic resolve/footer/label tests moved to `crates/tuikit/src/keys_tests.rs`; `overlay_tests.rs` checks every table through a registry and pins the new wording and the chords that stopped; `consistency_tests/keys.rs` classifies by `KeyRole` (three Esc/Enter/Space tests merged, a Ctrl-C test added); the help drift map lost dead entries; 14 insta snapshots and wording assertions in search, value modal, help, showcase, and the tmux e2e test follow the new footers.

Verification: isolated full suite 3809 passed, 0 failed, 1 ignored; fmt, clippy -D warnings, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `git diff --check`, independence guard: pass.
<!-- SECTION:FINAL_SUMMARY:END -->
