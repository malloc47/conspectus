# 0053: Checklists select all or none and count on their border

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/checklist.md](../controls/checklist.md) |

## Decision

- **Items are data.** An `Item` is a label, an optional detail, whether it
  is checked, and whether it can change. The host builds them each time the
  checklist opens; `Enter` hands the checked indices, in order, to
  `on_confirm`, an `Fn(Vec<usize>) -> Outcome<M>` shared behind an `Arc`.
  The checklist keeps no copy of host state and is `Clone`.
- **One contract and one table.** `Checklist<M>` implements `Overlay<M, Cx>`
  for every context and reads none. `CHECKLIST_KEYS`: the arrows, `k`/`j`,
  and `Ctrl-P`/`Ctrl-N` move, wrapping; `PgUp`/`PgDn` move by the rows shown
  and `Home`/`g` and `End`/`G` go to the ends, stopping there; `Space`
  checks or unchecks the cursor's item; `a` checks and `n` unchecks every
  item that can change; `Enter` confirms; `Esc` closes and `q` and `Ctrl-C`
  dismiss, silently. Letters bind as typed, so `Ctrl-A` is not `a`.
- **An item that cannot change** is drawn receding with its reason after the
  separator mark, keeps its check, is stepped over by the cursor, and is
  untouched by `Space`, `a`, and `n`; when checked it counts and is handed
  over.
- **The count is on the bottom border**, `n of m selected`, in view at every
  size and costing no row.
- **One look.** The compact frame; a blank row, one row per item (the cursor
  gutter, the check mark `√` or `•`, the label, the detail, cut detail
  first), a blank row, and the footer. A checked item's mark and label are
  in `accent` and bold. On a short screen the items scroll in a window the
  checklist keeps, with the shared scrollbar.

## Rationale

The host's checklist was a third-party shim with a per-frame mirror of its
state, and selecting all or none is a basic other kits ship. Two keys that
each say what they do read plainly in a generated footer.

## Consequences

- The multi-select crate left the build (record
  [0008](0008-the-third-party-crates.md)).

## Alternatives considered

- **One key that toggles all or none.** What it does depends on the state,
  and the footer cannot say which.
- **`Ctrl-A` for select all.** A chord where plain letters are free.
- **The count in the title.** The title is cut first on a narrow screen.
- **The count as a footer segment or a row.** A segment is a key and a
  verb, not state; a row costs one of few on a small screen.
- **`[x]` and `[ ]` marks.** One checkbox look everywhere, but it changes a
  look the checklists had; left for review.
- **Items that cannot change where the cursor can rest.** `Space` there
  would be a silent refusal.
- **Preselection by indices.** Items as data say each check where the item
  is built.

## Origin

- host ADR 0173, "Checklists are one control that selects all or none and
  counts": decisions 1 to 6.
- host ADR 0152, "Who owns the framework's third-party crates": decision 4
  (the multi-select shim replaced with in-tree code).
