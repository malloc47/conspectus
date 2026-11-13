---
id: CSP-729
title: Extract the `Overlay` contract and a host-owned `ModalStack`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 22:38'
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
- [x] #1 `Overlay<M, Cx>` has `handle` and `render`, and `ModalStack` is host-owned and draws every entry bottom to top
- [x] #2 The 14 existing overlays implement it, and the 16 `Action::*Key` wrappers and per-overlay handlers are removed
- [x] #3 Controls and Search read their context through `&Cx` and no longer clone an 11-field snapshot
- [x] #4 A registry test fails when a modal is not routed, drawn, or registered
- [x] #5 Esc steps back, Ctrl-C and q dismiss, and cancel is silent, in every overlay
- [x] #6 The stale claims in `modal.rs` and ADR 0085 are corrected
- [x] #7 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #8 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #9 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. tuikit::overlay: Outcome<M> (Consumed, Close, Commit(M), Stay(M)), Overlay<M, Cx> with handle(key, &Cx) and render(area, buf, &Cx, &Theme) plus fills_screen, a Box<T> blanket impl, and ModalStack<E> (push, pop, top, close-all, route to the top, draw every entry bottom to top from the last full-screen entry). Unit tests, docs/controls/overlay.md, architecture and README rows.
2. Host onto the contract: Modal declared by one `modals!` registration that generates the enum, the delegating Overlay<Msg, App> impl, and the name list; every entry (14 overlays plus the two viewers) implements handle and render in src/tui/modal/entries.rs. App keeps a ModalStack<Modal> and lends it out with the rest of the App as the context (take and put back), so Controls and Search read &App instead of a cloned snapshot; the content modal's query comes from the entry, not the stack.
3. One route: Action::ModalKey replaces the 16 Action::*Key wrappers and runtime::modal_key(app, key, send) the 16 handle_*_key handlers (close on Close/Commit, send on Commit/Stay). The draw ladder becomes the stack's draw; pagers keep their scroll window in render, so ViewportReport keeps only the panes. Value modal and Messages copy through Msg::CopyText -> Effect::Copy. Snapshot driver keeps its per-modal send policy keyed by the entry's name.
4. House rules 4 and 5: cancel is silent (six `cancelled` statuses and `viewer closed` go); Ctrl-C dismisses from every level (worktree menu screens, viewers' help and find); q dismisses where nothing is typed (menus, checklists, confirmations, Controls, close-down choice, search options, viewers' help).
5. Registry test: every registered modal is built or has a reason, is key-routed, closes on Ctrl-C, follows the q and Esc rules, is drawn, and stays drawn under another; a stacked layout check.
6. ADR 0161 (amends 0085 contract 3 and 0123), ADR index, stale claims in modal.rs and ADR 0085, operations and changelog, operator-review rows.
7. Before/after --snapshot frames (100x30, 60x20) and stacked-state frames; full checks; final summary.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed the framework module (tuikit::overlay: Overlay, Outcome, ModalStack, plus takes_text) and the house rule 4 and 5 behavior on the old routing: q in the standard menu, confirm, and checklist tables, Controls and search options q, worktree Ctrl-C closes from every screen, viewers leave on q and Ctrl-C from help and on Ctrl-C from find, cancel messages and `viewer closed` gone. The tmux e2e suite caught that a silent close left the opening hint (`viewing …`) on the status line, so a key-close now clears the status line. Full suite green (3833 passed). Next: move the host onto the stack.

Host moved onto tuikit::overlay (one `modals!` registration; entries in src/tui/modal/entries.rs; App::with_modals lends the stack with the App as context; Action::ModalKey and runtime::modal_key replace the 16 forwarders and handlers; the stack draws every entry; viewers are full-screen entries drawing into a Buffer; copy via Msg::Copy/Effect::Copy; snapshot send policy by entry name). Registry test (consistency_tests/registry.rs) and a stacked-pair layout check added. ADR 0161 amends 0085 c3, 0123, 0156. Validation: fmt clean; clippy --workspace --all-targets --all-features -D warnings clean; run-tests.sh 3843 passed, 0 failed, 1 ignored; nextest 3843 passed, 1 skipped; RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps clean; git diff --check clean; tuikit independence guard 35 passed. Frames: /tmp/tuifw-baseline/frames-729-before and -after, 84 scripted states at 100x30 and 60x20 plus 6 stacked pairs.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::overlay`: `Overlay<M, Cx>` with `handle(key, &Cx) -> Outcome<M>` and `render(area, buf, &Cx, &Theme)` (plus `fills_screen` and `takes_text`), `Outcome<M>` (Consumed, Close, Commit, Stay), and a host-owned `ModalStack<E>` that routes a key to its top entry, settles the outcome (closing the top and handing it back), and draws every entry bottom to top from the topmost one that fills the screen. Reference page `crates/tuikit/docs/controls/overlay.md`; ADR 0161 amends ADR 0085 contract 3, ADR 0123, and ADR 0156.

The host is on it: one `modals!` registration declares `Modal`, its names, and the delegating impl; all 16 entries (the 14 overlays plus the transcript and pane viewers, the transcript viewer still a nested reducer) implement handle and render in `src/tui/modal/entries.rs`. The 16 `Action::*Key` forwarders became `Action::ModalKey` and the 16 handlers `runtime::modal_key`; the 15-call draw ladder became the stack's draw. The App lends its stack out with itself as the context, so Controls and Search read `&App` instead of a cloned snapshot. Adding a modal is a struct, an impl, and one line; `consistency_tests/registry.rs` fails until a new modal is built there and checks routing, drawing, and the close keys of every modal, and the layout suite checks a stacked pair.

Verified: fmt clean; clippy `--workspace --all-targets --all-features -D warnings` clean; isolated `cargo test` 3843 passed, 0 failed, 1 ignored; nextest 3843 passed, 1 skipped; `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` clean; `git diff --check` clean; independence guard passes. `conspectus tui --snapshot` before and after at 100x30 and 60x20 (84 states, `/tmp/tuifw-baseline/frames-729-*`): every frame with one modal open is byte-identical; the 28 close-path states differ only as listed below; of 6 stacked pairs (rendered by a throwaway test, since no key opens one), 3 now show the lower modal and 3 are covered by the top one.

**Regressions**
- `q` now closes the row, mux, and worktree menus, Clean up, Controls (outside its max-age and contents editors, where it types) and its checklists, the worktree confirmations, the content search options, and the viewers from their help page; it used to do nothing there or close only the help, and in the worktree close-down choice it used to step back to the list.
- `Ctrl-C` in the worktree menu's branch prompt, confirmations, and close-down choice closes the menu instead of returning to its list; in the viewers it leaves from the help page and from the find prompt.
- The six `<x>: cancelled` statuses (rename, worktree, new tmux session, mux menu, mux launch, pin edit) and `viewer closed` no longer appear. A key-close also clears the status line, so a message shown before the overlay opened is gone once it closes.
- A toast now draws over the full-screen viewers.

**Consistency wins**
- Every open modal is drawn bottom to top: a smaller modal over another shows the lower one around its frame (the rename prompt over the row menu, the new tmux session form over the mux menu, Controls over search). Help over Controls, the value modal over Messages, and the picker over pin edit look as before, because the top one covers the lower frame entirely; no operator key opens a stacked pair today.
- One close rule in every overlay (house rule 4): Esc steps back one level and closes at the first, Ctrl-C and `q` (where nothing is typed) dismiss; cancel is silent (house rule 5) and no opening hint outlives its overlay.
- Every overlay commits through the live executor in the live TUI; the snapshot driver no longer prints Messages' clipboard escape into its frame.
- Footers are unchanged: `q` is an alias. The viewers' help page still shows `Esc/q/? close` while `q` now leaves the viewer (the Pager story regenerates it).

Judgment calls are in `docs/plans/tui-framework/operator-review.md`; rule 4 was applied to every overlay, so CSP-739, CSP-740, CSP-744, and CSP-748 find their close-key items done.
<!-- SECTION:FINAL_SUMMARY:END -->
