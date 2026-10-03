---
id: CSP-468
title: 'Declarative keybinding table for dispatch, overlays, and help'
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies:
  - CSP-497
ordinal: 99000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**Waves 1–5 landed 2026-07-04..05**: * Wave 1 (`83d6889`): new `src/tui/keybindings.rs` module. `KeyBinding { mode, key: KeyMatcher, action: fn() -> Action, help_text }` + `KeyMode::Global` + `KeyMatcher::{Exact, UpperChar}` + `pub const KEYBINDINGS: &[KeyBinding]`. Pilot seeds 6 view-switching bindings. 3 drift tests. * Wave 2 (`6826b20`): `keybindings::translate_via_table` entrypoint; `keymap::translate` short-circuits through it. 6 arms migrated. Behavior-preserving. * Wave 3 (`e069833`): 11 more bindings migrated (Quit / Resume / Rename / pin CRUD / ClearFilters / ToggleEdgeMeta). `keymap::translate` fallback arm count 52 → 32. * Wave 4 (`9ca5585`): expanded `KeyMatcher` with `AnyModExceptCtrl(char)` + `AnyMod(KeyCode)` variants. Migrated 22 remaining single-char / arrow / nav bindings. `keymap::translate` fallback arm count 32 → **2** (only `PageDown` / `PageUp` remain — they carry `viewport_height` in their payload). * Wave 5 (`74fed0d`): `key_label(&KeyMatcher) -> String` helper + `every_keybindings_entry_appears_in_help_sections` drift test in `widgets/help.rs`. Every KEYBINDINGS entry must appear in `keymap_sections`'s rendered output; handles multi-key help rows (`1 – 5`, `↓ / ↑`, etc.) via known-equivalent-form mapping. Enforces the dispatcher / help coherence the audit called out. **Scope closed to the migratable surface**: post-H-HYG-009-wave-2 (which extracted `widgets/pins.rs`'s test module), pins.rs has 40 KeyCode arms and runtime.rs has ~18 production arms; both are overlay-owned per-`handle_key` dispatchers (transcript viewer + PinCreate / PinBind / PinEdit / PinRemove / Pins overlays) that use their own message types (`ViewerMsg`, `PinCreateOutcome`, etc.), not `Action`. Migrating these needs overlay-scoped mode + per-overlay message dispatch in KEYBINDINGS — a bigger architectural change than what CSP-468's `Action`- centric table shape supports. That extension belongs alongside `CSP-497`'s modal stack contract (which supplies the mode column the table would need), so the remaining overlay migrations are deferred there. Global (non-overlay-owned) keymap coverage is complete.

- Scope: key handling is hand-matched (`KeyCode::` ×128 in `runtime.rs`,
  ×137 in `widgets/pins.rs`), `remap_for_focus` re-maps actions across
  ~200 lines, and `widgets/help.rs::keymap_sections` hand-maintains a
  parallel list of the same bindings — nothing forces the three to
  agree. Introduce one `(mode/focus, key, Action, help text)` table
  consumed by the dispatcher, the focus remap (becomes data), the help
  overlay, and the controls/pins hint footers. Serves the menu-first
  discoverability direction: overlays render from the table the
  dispatcher executes.
- Tests: existing key-handling unit tests; one drift test asserting
  every dispatched action appears in the table and vice versa.
- **Sizing note (2026-07-04)**: this is a substantial refactor —
  ~265 `KeyCode::` match arms plus the parallel `keymap_sections`
  plus `remap_for_focus`. Land as a dedicated multi-commit series:
  (a) `(mode/focus, key, Action, help text)` table shape + drift
  test infrastructure only; (b) migrate `runtime.rs` dispatch
  (76 arms); (c) migrate `widgets/pins.rs` dispatch (137 arms);
  (d) rewire `remap_for_focus` as a table-derived transform;
  (e) migrate `widgets/help.rs::keymap_sections` to consume the
  same table. Each wave is behavior-preserving; the drift test
  from wave (a) catches regressions across the intermediate
  landings.
- Blockers: none; works standalone, and the `CSP-497` modal stack
  later supplies the table's mode column.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-007`
