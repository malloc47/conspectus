---
id: CSP-796
title: Name the keys in status and refusal messages from the key tables
status: To Do
assignee: []
created_date: '2026-10-30 15:58'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-760
priority: low
ordinal: 882000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-759 generated the status bar's hints from the key tables and holds the status-line code (`runtime.rs`, `runtime/executor.rs`, `app/overlays.rs`, `ui/header.rs`) to a stricter hygiene rule, but messages elsewhere still name keys by hand, so they can drift from the keymap: `app/explorer_nav.rs` (`explorer: no drill history — press Backspace again to return to the left pane`, `Backspace: no reveal to return from · Tab to the right pane to back out of its drill`), `app/content.rs` (`Ctrl-F filters by the search the Search view shows (6)`), `runtime.rs` (`i: select an agent or mux session row to copy its id`), `actions.rs` (`pin_status_hint`'s `· Enter runs `conspectus rebuild x``, shown in the Preview; `attach_disabled_why`'s ``try `r` to refresh``).

Scope: name each key from the table that binds it (the scoped keymap of CSP-760, `RIGHT_PANE_KEYS`, `MAIN_VIEW_KEYS` until it merges), with `keys::key_label`, and extend the status scan of `tests/overlay_keymap_hygiene.rs` to `app.rs`, `app/`, and `actions.rs`.

TUI changes: none intended beyond key labels written in the one notation (`Ctrl-F` stays, `Backspace` stays); list any in the summary.

Tests: the extended scan passes; the messages' tests read the label from the table.

Blockers: CSP-760 (the scoped keymap gives every program-wide key a table row).
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
