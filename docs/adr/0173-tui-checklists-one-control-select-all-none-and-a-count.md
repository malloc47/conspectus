# ADR 0173: Checklists Are One Control That Selects All Or None And Counts

## Status

Accepted. Carries out the `Checklist` control of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), applying its
house rules 1, 2, 4, 5, 8, 12, 13, and 14 to every checklist, and decision 4
of [ADR 0152](0152-tui-framework-dependency-ownership.md) for it: the
`ratatui-cheese` shim becomes in-tree code and the crate leaves the build.
Extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's `checklist` module. Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md):
`CHECKLIST_KEYS` (`ChecklistKey`) gains `a` select all and `n` select none
(shown), and `PgUp`/`PgDn`, `Home`/`g`, and `End`/`G` (aliases). Amends
[ADR 0031](0031-tui-filter-view-switching-and-per-view-state.md): the
Controls overlay's harness, mux-state, and PR-state editors are the
framework's checklist, not a multi-select primitive of its own.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md`, sections
3.5 and 3.6) found two checklists that are one control: the Controls
overlay's three filter editors and the History view's Clean up.

- **A shim with a copy of its state.** `MultiSelectState` wrapped
  `ratatui_cheese::multi_select`, whose state holds a validator closure,
  so the shim rebuilt it in a hand-written `Clone`, and drew each frame
  through a throw-away mirror of the cursor and the checked set. It used
  the crate only for a cursor and a set of indices.
- **Keys nobody saw.** `CHECKLIST_KEYS` had shown hints, but the list drew
  no footer, so its keys were in no frame.
- **What a list of choices usually has, missing.** No way to check or
  uncheck everything at once (the gap analysis's Tier 1 item), no count of
  what was checked, no way to show an item that cannot change, and no
  paging on a list taller than the screen (house rule 12).
- **A wrapper per caller.** Clean up was `CleanUpState` and
  `CleanUpWidget` around the shim, translating its outcome; each Controls
  editor repeated the same outcome match.

## Decision

1. **`tuikit::checklist` owns the checklist.** Its items: `Checklist<M>`,
   `Item`, and `CHECKLIST_SIZE` (`ModalSize::COMPACT`).

2. **Items are data, and the host decides what a confirm means.** An
   `Item` is a label, an optional detail, whether it is checked, and
   whether it can change. The host builds the items from its own state
   each time the checklist opens, with `Checklist::new(title, items,
   on_confirm)`. `Enter` hands the checked indices, in order, to
   `on_confirm`, an `Fn(Vec<usize>) -> Outcome<M>` shared behind an `Arc`,
   and the checklist reports its answer: a commit of the host's message,
   or a close when there is nothing to do. The checklist keeps no copy of
   its state and derives `Clone`.

3. **One contract and one table.** `Checklist<M>` implements `Overlay<M,
   Cx>` for every context and reads none. It resolves `CHECKLIST_KEYS`:
   the arrows, `k`/`j`, and `Ctrl-P`/`Ctrl-N` move, wrapping (house rule
   2); `PgUp`/`PgDn` move by the rows shown and `Home`/`g` and `End`/`G` go
   to the ends, stopping there (house rule 12); `Space` checks or unchecks
   the cursor's item; `a` checks and `n` unchecks every item that can
   change; `Enter` asks `on_confirm`; `Esc` closes (`Close`) and `q` and
   `Ctrl-C` dismiss (`Dismiss`), saying nothing (house rules 4 and 5).
   Letters bind as typed, so `Ctrl-A` is not `a`. The footer reads `↑/↓
   move · Space toggle · a select all · n select none · Enter confirm ·
   Esc cancel`.

4. **An item that cannot change is shown, keeps its check, and is
   stepped over.** It is drawn receding (`keys::unavailable_style`), its
   detail plain after the separator mark to say why; the cursor steps over
   it as it steps over a menu entry that cannot run (ADR 0171 decision 4),
   and neither `Space` nor select all or none changes it. When checked it
   counts and is in the indices `Enter` hands over.

5. **The count is on the bottom border.** The frame's footer slot carries
   `n of m selected`, in the border's color, so the count is in view at
   every size and whatever is scrolled, and costs no row.

6. **One look.** The compact frame with the title on its top border;
   inside, a blank row, one row per item (the cursor gutter through
   `selection::row`, the check mark, the label, and the detail, cut with
   `…` in cells, the detail first), a blank row, and the footer generated
   from `CHECKLIST_KEYS`. A checked item shows `marks::OK` (`√`) and an
   unchecked one `marks::BULLET` (`•`), as the shim drew them; a checked
   item's mark and label are in the theme's `accent`, bold. On a short
   screen the blank rows give way first, then the footer keeps the bottom
   rows (all but one, so the cursor's row shows) and the items scroll in a
   `ScrollWindow` the checklist keeps between draws, with the shared
   scrollbar.

7. **The host moves onto it.** The Controls editors are
   `Checklist<Vec<T>>`s that commit the checked option values, built by
   one helper over their options, and one helper settles every editor's
   outcome (the two text prompts' too). Clean up is
   `clean_up::checklist(candidates, now)`, a `Checklist<Msg>` held by
   `Modal::CleanUp`: its labels and the `Msg::ReapRecords` mapping stay in
   `clean_up.rs`, and with nothing checked it closes. `multi_select.rs`,
   `CleanUpState`, `CleanUpWidget`, and their overlay impl are deleted,
   and `ratatui-cheese` leaves `Cargo.toml` and `Cargo.lock`.

## Consequences

- **Visible changes** (the CSP-741 summary lists them): every checklist
  gains a blank row above its items and its footer, so it is four rows
  taller on a normal screen; the bottom border counts what is checked;
  `a`, `n`, the page keys, and the ends keys act; at 40x10 the footer
  keeps its three rows and the items scroll in the rest, where the list
  used every row for items and showed no keys. Long labels still end in
  `…`, and the rows look as they did.
- Clean up answers `q` and `Ctrl-C` with `Dismiss`, which closes it as
  `Close` did, since it is never a step.
- A checklist can no longer drift from the others' keys, footer, cursor,
  or scrolling, and the conformance suite checks Clean up's footer and
  selection like any list's.
- The framework has no dependency the host's checklists need beyond
  ratatui; one third-party crate fewer to keep current.

## Alternatives Considered

- **One key that toggles all or none** (`a`, as some prompt libraries
  have it). One key fewer, but what it does depends on the state, and the
  footer cannot say which; two keys each say what they do.
- **`Ctrl-A` for select all.** A chord where nothing is typed, beside
  plain `j`, `k`, and `q`; the plain letters are free in a checklist.
- **The count in the title.** The title is cut first on a narrow screen,
  and the count would mix the list's state into its name.
- **The count as a footer segment or a row of its own.** A segment is a
  key and a verb in the footer's grammar, not state; a row costs one of
  the six a 40x10 checklist has.
- **`[x]` and `[ ]` marks**, as the forms and the Controls panel draw a
  checkbox. One checkbox look across the program, but it changes a look
  the checklists have had, and the glyph table gives the bullet to an
  unchecked item in a list of choices and the boxes to options in a row.
  Left for review.
- **Items that cannot change where the cursor can rest.** `Space` there
  would be a silent refusal (house rule 7).
- **Preselection by indices** (`new(title, count, &[usize])`, the shim's
  shape). Items as data say each item's check where the item is built, as
  menu entries do.
- **Keeping the shim behind the framework's API.** Rejected by ADR 0152.

## Open Questions Answered

- *Is there a range selection?* No. Select all and none act on the whole
  list; checking a run of items is a seam on the control's page.
- *Does a checked item that cannot change count?* Yes, and `Enter` hands
  it over.
- *What does `Enter` with nothing checked do?* What the host's
  `on_confirm` answers: the Controls editors clear their filter, and
  Clean up closes without forgetting anything.
