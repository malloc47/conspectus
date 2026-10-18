---
id: CSP-620
title: Drive overlay keys and footer hints from the keybinding table
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:09'
updated_date: '2026-10-18 01:02'
labels:
  - h-hyg
milestone: m-11
dependencies:
  - CSP-619
references:
  - docs/code-hygiene-audit.md
priority: low
type: chore
ordinal: 678000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-468 put global keys in one table (`src/tui/keybindings.rs`, 47 entries) that both the dispatcher and the help overlay read, with a drift test between them. It deferred keys inside overlays to CSP-497's modal stack, which shipped without the per-overlay mode the table needed, so that work was dropped. Overlay keys are still matched by hand in each widget (production `KeyCode::` arms on 2026-10-16: `widgets/value_modal.rs` 21, `widgets/pins/create.rs` 15, `widgets/pins/bind.rs` 15, `runtime/overlay_keys.rs` 15, `widgets/mux_launch.rs` 14, `widgets/worktree_menu.rs` 13, `widgets/pins/edit.rs` 11, `widgets/messages.rs` 10). Their footers are hand-written strings, such as "Enter/y to confirm · Esc/n to cancel" in the worktree menu, that nothing ties to the handler, so a footer can advertise a key its overlay ignores. The TUI relies on these menus and footers for discoverability rather than memorized keys. Audit finding C3.

- Scope: add overlay-scoped modes to the keybinding table, resolve each overlay's keys through it, and render each overlay's footer hint from its rows. Overlay-specific message types (`ViewerMsg`, form outcomes) stay; the table maps keys to them. Free-text fields keep receiving raw characters. CSP-391's action registry covers global commands for the command palette; this task covers keys inside overlays, in rows the registry can read later.
- Tests: extend the drift test so every overlay footer lists exactly the keys its handler accepts; existing overlay key tests pass.
- Manual checks: `conspectus tui --snapshot` for each overlay shows its footer unchanged, or corrected where it had drifted.
- Blockers: CSP-619.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every overlay resolves its non-text keys through the keybinding table
- [x] #2 Every overlay footer hint is generated from the same rows its handler uses
- [x] #3 A test fails when an overlay footer and its handler disagree
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Design (recorded as an ADR in this task):
- New `tui::keybindings::overlay` module: `KeyRow<A>` = `Bind { key: KeyMatcher, action: A, hint: Hint }` or `Note(&str)`; `Hint` = `Shown { label, verb }` or `Alias` (bound on purpose but left out of the footer, e.g. j/k, Ctrl-C). `resolve(rows, event) -> Option<A>` (first match) and `footer(rows) -> String` (labels sharing a verb join with `/`, segments join with ` · `).
- Each overlay gets an action enum and a `const` table there. Handlers match exhaustively on their enum, so a row whose action a handler ignores fails to compile. Overlays with identical menus share one table. Unbound keys fall through to the text field where the overlay has one; context-dependent keys (Space types in a text field, toggles on an option field) resolve to an action and the handler decides.
- Matchers keep today's semantics: code-only matches use `AnyMod`, modifier checks use `Exact`, Ctrl-C rows come first.
Steps:
1. Infra + tests: `resolve`, `footer`, and a label check that every `Shown` label names its bound key.
2. Wave A: controls list and sub-editors, mux menu, pins menu, multi-select, text input/rename, new-mux form, search, value modal, messages, help.
3. Wave B: worktree menu, mux-launch form, path omnibox.
4. Wave C: pin create, edit, bind, remove forms.
5. Viewer: key translation moves into a table; its footer stays viewer-owned (stateful toggle chips behind the ADR 0052 extraction boundary).
6. Guard test under `tests/` failing on `KeyCode::` matches in overlay widget code; per-overlay test that the rendered footer equals `footer(table)`.
7. Footers stay byte-identical except where the generator normalizes wording (noted per overlay); verify with `tui --snapshot` against the showcase fixture.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Wave A landed: overlay keymap infra (`tui::keybindings::overlay`) plus tables for menus (Controls, pins menu, mux menu), multi-select, text input, new-mux form, search, help, value modal, and messages. Footers, the rename/controls/pins status hints, and the messages copy path are generated from or resolved through those tables. Renders of the snapshot-mode overlays are byte-identical to the baseline; just check green (2408 tests).

Wave B landed: worktree menu (list via the shared menu table, one confirm table for remove/merge/prune, a close-down table), mux-launch form, and path omnibox. Ctrl-C now backs out of the worktree list and its confirms like Esc does; before, those modes ignored it. Renders byte-identical to baseline; just check green (2408 tests).

- Wave C landed: pin create, edit, bind, rebind, and remove forms resolve through `PIN_CREATE_KEYS`, `PIN_EDIT_KEYS`, `PIN_BIND_KEYS`, `PIN_REBIND_KEYS` (with a `Note("type to edit")` row), and `PIN_REMOVE_KEYS`; their footers are generated.
- Viewer: key translation now resolves through `VIEWER_KEYS` (all aliases); `viewer_keys_resolve_as_the_old_match_did` pins every old mapping, modifier cases included. Its footer stays viewer-owned behind the ADR 0052 boundary; the ADR records that exception.
- Guards: `tests/overlay_keymap_hygiene.rs` fails on `KeyCode::` matching or a key-hint-shaped literal in overlay production code (widgets, `runtime/overlay_keys.rs`, `modal.rs`). Probed by injecting both into `mux_menu.rs`: both tests failed naming the line and literal; restored.
- ADR 0123 records the convention (typed per-overlay tables, `Hint::Alias`, Ctrl-C as `Abort`, text fall-through, viewer exception) and is indexed in `docs/adr/README.md`.
- Validation: overlay renders against the showcase fixture match the pre-change baseline for every state snapshot mode can reach (Controls, Help, Pins, pin create, Messages), apart from the wall-clock "updated Ns ago" header; footer wording tests pin every table's footer; `just check` green (2411 tests).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Overlay keys and footer hints now come from one place. Every overlay resolves its keys through a typed table in `tui::keybindings::overlay` and matches exhaustively on its own action enum, so a bound key the handler ignores fails to compile. Every footer, plus the matching status-bar hints, is generated from the same rows. Footers read as before except the help overlay's (normalized), and Ctrl-C now backs out of the worktree menu like everywhere else. The viewer's keys use a table too, but its stateful footer stays viewer-owned per ADR 0052. ADR 0123 records the convention. A guard test fails on hand-matched `KeyCode` or hand-written key hints in overlay code (verified by injecting both), a label test checks every footer label names its bound key, and `just check` passes (2411 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
