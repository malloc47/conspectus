---
id: CSP-759
title: Extract `StatusBar` and generate its hints from the key tables
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 16:00'
labels:
  - tui-framework
  - tuifw-chrome
milestone: m-25
dependencies:
  - CSP-728
  - CSP-730
  - CSP-723
  - CSP-731
documentation:
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 845000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The status bar's hint string, the help section, and `remap_for_focus` each describe the right-pane keys by hand, and the wording of `e` differs across them. Four status hints in `runtime.rs` duplicate or drift from the generated modal footers, and `tests/overlay_keymap_hygiene.rs` never scans `runtime.rs`. Three modal-opening functions copy footer text into the status bar. The status line paints every message in the warning colour (inventory slices 06 and 07).

Scope: Create `StatusBar` and `KeyHintBar` generated from the key tables with priority-drop fitting, using the feedback severities for message styling. Replace the hand-written hints in `runtime.rs` and the modal-open copies. Keep Conspectus's contextual segment producers and `default_action_status_hint` as annotations on table rows. Extend the hygiene guard to `runtime.rs`.

TUI changes:
- Consistency wins: a hint cannot advertise a key that does not resolve; status messages are styled by severity
- Regressions or behavior changes: hand-tuned contextual hint strings are regenerated, so their wording changes

Tests and checks: A test asserts that every advertised key resolves in the current mode. The extended `overlay_keymap_hygiene` passes. `--snapshot` of the status bar in each view and focus.

Blockers: The key tables, help generation, and text.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `StatusBar` and `KeyHintBar` are generated from key tables and fit by priority
- [x] #2 The hand-written hints in `runtime.rs` and the modal-open copies are removed
- [x] #3 `overlay_keymap_hygiene` scans `runtime.rs`
- [x] #4 A test fails when an advertised key does not resolve
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
1. Framework key model: a bound row carries an optional rank (const builder `.rank(n)`), and a footer segment records the keys it names and its rank, so a hint line can fit by priority and a check can press what it advertises.
2. `tuikit::status_bar`: `KeyHintBar` (hints generated from key tables in table order, labels that share a verb joined, per-row `Annotation`s from the host for a verb, a detail, a rank, or availability, plus whole segments of a control's footer) and `StatusBar` (lead chips, a hint-mode lead, the host's segments and the hints on one `PriorityLine`, trailing chips, or a message in its severity's style cut with `…`). `Overlay::hints()` names the keys an overlay takes now (every framework control and the host's three entries); the conformance suite skips the host's status rows when comparing with and without a modal and gains a check that every key the status line names resolves in the modal's tables. Reference page `docs/controls/status_bar.md`, module map, README.
3. Conspectus: `MAIN_VIEW_KEYS` (`MainKey`) with per-row ranks; the status bar names the scope that has the keys (the left pane and the program, the right pane's table and the program, or the top overlay's hints); `default_action_status_hint` and the contextual producers become annotations on table rows; the hand-written hints in `runtime.rs` and the modal-open copies (Controls, quick select, content search, worktree and mux menus) are gone. Tests: every advertised key routes through `action_for_event` to what its row says, in each scope; the overlay table registry takes the new table.
4. Extend `overlay_keymap_hygiene` (hint scan) to `runtime.rs`, `app/overlays.rs`, `runtime/executor.rs`, and `ui/header.rs`, with its plumbing docs.
5. ADR 0191, ADR index and amendments, design/operations/CHANGELOG, operator-review rows; before/after status-bar frames at 160/100/60/40 in every view, focus, row kind, and reachable overlay; full checks.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Framework slice f9ea6782: status_bar module, KeyRow::rank, footer segment keys and rank, Overlay::hints, conformance status rows and Hints check. Host slice 7fb5c771: StatusBar adoption, MAIN_VIEW_KEYS with annotations, footer copies removed, advertised-keys test. Hygiene slice 29b7c013; docs 83f9041b (ADR 0191). Ranked an overlay's keys above the settings after the first capture showed `filter:all …` alone at 40 columns with a menu open. Full isolated suite 4697 passed, 0 failed, 1 ignored; nextest 4697 passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::status_bar` (`StatusBar`, `KeyHintBar`, `KeyHint`, `Annotation`; page `crates/tuikit/docs/controls/status_bar.md`), a rank on bound key rows (`KeyRow::rank`), footer segments that list their keys and rank, `Overlay::hints` on every framework control and the host's entries, and two conformance additions (`Rig::status_rows`, the `Hints` check). Conspectus draws its status bar through `StatusBar` with the hints of the scope that has the keys: the top overlay's own footer; on the left pane the new `MAIN_VIEW_KEYS` (`MainKey`) table, each row worded for the selection by an annotation (`Enter`'s verb from `selected_default_action`); on the right pane `a attach …`, `RIGHT_PANE_KEYS`, and the program's keys. `default_action_status_hint` and the bundled strings are gone; the worktree and mux menus, Controls, quick select, and content search no longer post footer copies (they clear the status line). The `!` of the unseen chip and `· ! details` comes from its table row. ADR 0191; ADRs 0078, 0123, 0160, 0161, 0164, 0190 amended; design, operations, CHANGELOG, operator review (15 rows), inventory 06/07 updated. Follow-up filed: CSP-796 (key names in refusal messages, after CSP-760).

Verification: `every_key_the_status_bar_names_does_what_it_says` presses every advertised key through `action_for_event` in every view, row, and focus (and after a reveal); `a_hint_that_does_not_do_what_it_says_is_caught` shows it failing; conformance `Hints` over every registered modal; `overlay_keymap_hygiene` now scans `runtime.rs`, `runtime/executor.rs`, `app/overlays.rs`, `ui/header.rs` (planted-hint coverage test). Full isolated suite 4697 passed, 0 failed, 1 ignored (one `discovery::forge` timing flake in a subset run passed on rerun); nextest 4697 passed; fmt, clippy `-D warnings`, rustdoc `-D warnings`, `cargo build --examples`, `git diff --check`, independence guard clean. Frames: `/tmp/tuifw-baseline/frames-759-{before,after}` (248 status-bar frames at 160/100/60/40 in every view, focus, row kind, overlay, and message severity; 114 whole-screen frames): 163 bars differ, 85 identical, no whole-screen frame differs outside the bar; message severities draw green/green/yellow/red as before (CSP-731).

**Regressions** (wording changes, before → after)
- Left pane keys: `j/k move · h/l fold · Enter default` → `↑/↓ move · ←/→ fold` (letters still bound).
- Group rows: `Enter/l expand · h collapse` → `Enter/←/→ fold`.
- PRs view: `r refresh · ^R forges` → `r refresh · Ctrl-R hard refresh`.
- Pins: `pin `x` is stopped|removed · Enter runs `conspectus rebuild x` to bring it back`, `… isn't built yet · Enter runs …`, `…: target not found · …` → `Enter rebuild`; `pin `x` is a live PR · Enter shows its URL` → `Enter show URL`; unchecked target → `Enter rebuild: couldn't check whether its target exists` (dimmed); live pin with nothing to rebuild → `Enter rebuild` (dimmed). The Preview keeps the sentences.
- Cannot attach: `attach: <why>` → `a attach: <why>` (dimmed; `Enter/a attach: <why>` where Enter attaches).
- Right pane: `Enter/a attach X` → `a attach X`; the left row's view/resume/menu/fold/attach-reason hints dropped; `Tab inspect candidates` → `Tab focus`.
- Overlays: `mux: …`, `worktree: …`, `quick select: type to find · …`, `search contents: type a query · …`, `controls: …` → the overlay's generated keys without prefix or note, beside the settings and chips; no `[left]`/`[right]` tag under an overlay; unseen chip `▲ N` (no `· ! messages`) while an overlay is open.
- Fit: hints give way one by one (a narrow bar keeps more, e.g. `↑/↓ move` at 60 columns); an overlay's keys outrank the settings.

**Consistency wins**
- A hint can no longer name a key that does something else where it is shown (main-view hints under modals, `Enter`/`h`/`l`/`Tab` hints on the right pane, the duplicate `Tab`, `Enter default` beside `Enter/a`), checked by test.
- Every status hint is generated from a key table in the one notation; nothing writes footer text into the status line.
- A key that cannot act is dimmed with its reason; a too-long status message ends with `…`; severity styling verified unchanged.
<!-- SECTION:FINAL_SUMMARY:END -->
