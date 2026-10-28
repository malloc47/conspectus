---
id: CSP-729
title: Extract the `Overlay` contract and a host-owned `ModalStack`
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-conventions
milestone: m-25
dependencies:
  - CSP-728
  - CSP-726
documentation:
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
  - docs/adr/0085-tui-mvu-architecture.md
priority: high
type: enhancement
ordinal: 815000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The modal contract is half built. The `Overlay` trait has `handle` but no `render`, only the top modal is drawn (every accessor reads `modal_stack.last()`), and routing is a 16-branch ladder with 16 `Action::*Key` wrappers and 16 near-identical runtime handlers. Adding the newest modal (the Picker) touched about 9 production sites, and nothing tests that a new modal is key-routed. Controls and Search avoid a borrow conflict by cloning an 11-field context snapshot. `modal.rs` still claims that the whole stack draws and that every variant implements `Overlay` (inventory slice 06, section 2).

Scope: Create `tuikit::overlay` with `Overlay<M, Cx>` having `handle` and `render`, `Outcome<M>`, and a host-owned `ModalStack` that passes `&Cx` while holding `&mut` entries and draws every entry bottom to top, each clearing its own area. Migrate the 14 `Overlay` implementations and route key events to the stack top so the 16 wrappers and handlers disappear. Replace the cloned context snapshot in Controls and Search with `&Cx`. Add a registry test that every modal is registered, key-routed, and drawn. Fix the stale claims in `modal.rs` and ADR 0085.

TUI changes:
- Consistency wins: Help opened over Controls shows Controls underneath; adding a modal takes a struct, a trait impl, and one registration; cancel is silent in all overlays and Esc/Ctrl-C/q follow one rule
- Regressions or behavior changes: the worktree menu's Ctrl-C closes the menu instead of going back one level; six overlays stop saying `cancelled` in the status bar

Tests and checks: A registry test fails if a modal variant lacks a handler or a drawing. The `consistency_tests/layout.rs` checks pass with a stacked pair. The 16 handlers' tests are rewritten against the stack.

Blockers: The key-table extraction and the modal frame.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `Overlay<M, Cx>` has `handle` and `render`, and `ModalStack` is host-owned and draws every entry bottom to top
- [ ] #2 The 14 existing overlays implement it, and the 16 `Action::*Key` wrappers and per-overlay handlers are removed
- [ ] #3 Controls and Search read their context through `&Cx` and no longer clone an 11-field snapshot
- [ ] #4 A registry test fails when a modal is not routed, drawn, or registered
- [ ] #5 Esc steps back, Ctrl-C and q dismiss, and cancel is silent, in every overlay
- [ ] #6 The stale claims in `modal.rs` and ADR 0085 are corrected
- [ ] #7 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #8 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #9 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
