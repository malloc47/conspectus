---
id: CSP-620
title: Drive overlay keys and footer hints from the keybinding table
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
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
- [ ] #1 Every overlay resolves its non-text keys through the keybinding table
- [ ] #2 Every overlay footer hint is generated from the same rows its handler uses
- [ ] #3 A test fails when an overlay footer and its handler disagree
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
