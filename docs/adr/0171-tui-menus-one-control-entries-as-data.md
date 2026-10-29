# ADR 0171: Menus Are One Control Whose Entries Are Data

## Status

Accepted. Carries out the `ActionMenu<M>` control of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), applying its
house rules 1 to 5, 8, 13, and 14 to every action menu, and extends
[ADR 0150](0150-tui-framework-workspace-crate.md) with the framework's
`action_menu` module. Amends
[ADR 0096](0096-ephemeral-harness-mux-launch.md) (its **TUI surface**): the
Mux action menu is the framework's menu fed two entries, it has no heading
of its own inside its frame, and its entries show `n` and `N`, the keys
that open the same forms from the list, which answers ADR 0096's held
question about an accelerator for the harness-launch entry. Takes up the
seam the CSP-724 review left: a menu keeps its scroll window between
draws. Amended by
[ADR 0172](0172-tui-dialogs-one-control-and-follow-up-questions-are-steps.md):
an entry can open a step (`Entry::opens_step`, picked with `Stay`), the
menu answers `q` and `Ctrl-C` with `Dismiss`, and the worktree menu of
decision 8 is now an `ActionMenu<Msg>` whose entries open its prompt and
dialogs as steps.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md`, sections
3.1 to 3.3, 4, 6, and 7) found four menus that are one control: the row
menu (`.`), the mux menu (`m`), the worktree menu's list (`w`), and the
menu `p` opens when several pins point at a row. Earlier stories gave them
one frame size (ADR 0158), one cursor look and wrapping ends (ADR 0159),
one key table (ADR 0160), and one set of close keys (ADR 0161), but each
was still its own widget:

- **Three implementations of one list.** `RowMenuState`, `MuxMenuState`,
  and the worktree menu's list each kept a cursor, stepped it, resolved
  `MENU_KEYS`, and laid out rows, and their tests repeated each other.
- **A hard-coded menu.** The mux menu's two verbs were an enum with its own
  widget, an in-body `Mux actions` heading that repeated its title, and a
  footer that was not wrapped; at 40x10 the heading pushed the footer off
  the frame.
- **Footers and scrolling by accident.** The row menu scrolled (CSP-712)
  but from offset 0 on every draw, so the cursor rode the last line; the
  mux menu and the worktree list could not scroll, and the worktree list
  drew no footer at all.
- **Keys mentioned in prose.** The only way a menu showed a key was the
  Pin entry's explanation, `pin this (p)`, and `p` did nothing in the menu.
- **A second `Enter` that one entry asked for by hand.** Forget armed on
  the first `Enter` with a hand-written `— press Enter again to confirm`;
  no other entry could.
- **No way to show an action that cannot run now.** A host could only
  leave the entry out.

## Decision

1. **`tuikit::action_menu` owns the menu.** Its items: `ActionMenu<M>`,
   `Entry<M>`, `Confirm`, `MENU_SIZE` (`ModalSize::MENU`), and
   `confirm_note`.

2. **Entries are data.** An `Entry<M>` is a label, an optional detail, an
   optional shortcut (a `KeyMatcher`), whether it can run now (`enabled`),
   how it confirms (`Confirm::None` or `Confirm::Inline`), and the host's
   message `M`. The host builds the entries from its own state each time
   the menu opens; the menu reports the picked entry's message.

3. **One contract.** `ActionMenu<M>` implements `Overlay<M, Cx>` for every
   context and reads none. It resolves `MENU_KEYS` first: the arrows,
   `k`/`j`, and `Ctrl-P`/`Ctrl-N` move, wrapping (house rule 2); `Enter`
   picks; `Esc`, `q`, and `Ctrl-C` close with no message (house rules 4
   and 5). A key the table does not bind is an entry's shortcut or is
   swallowed. A host that holds a menu as a level of its own overlay makes
   `M` a value of its own and takes a commit as the pick.

4. **An entry that cannot run is shown and stepped over.** It stays in the
   list, drawn receding (`keys::unavailable_style`: `inert` with `muted`);
   the cursor steps over it in both directions and starts on the first
   entry that can run; neither `Enter` nor its shortcut commits it. Its
   detail stays plain, because that is where the host says why. A menu in
   which nothing can run marks no row and does nothing on `Enter`.

5. **An armed entry says what `Enter` will do.** `Confirm::Inline` arms on
   the first press (`Enter` or the shortcut); the row then reads the label,
   `·`, and `press Enter again to confirm`, the key written as `MENU_KEYS`
   shows it, in `error` and bold, and the second press commits. Moving
   disarms. `Esc`, `q`, and `Ctrl-C` close the menu, armed or not.

6. **Shortcuts sit in a key column.** When any entry has a shortcut, every
   label row has a column for it after the cursor gutter, as wide as the
   widest key label and two cells, so every label starts in one column.
   A shortcut moves the cursor to its entry and picks it. The table is
   resolved first, so a shortcut on a key the table binds, or on a key an
   earlier entry took, never fires; `ActionMenu::shadowed_shortcuts` finds
   one.

7. **One layout, a fixed footer, and a kept window.** Inside the menu
   frame: a blank row, each entry's label row and its detail wrapped three
   cells past the labels, a blank row, and the footer generated from
   `MENU_KEYS`. On a short screen the footer keeps the bottom rows (its
   blank row giving way first) and the entries scroll in a `ScrollWindow`
   the menu keeps between draws, moving no further than the cursor's entry
   needs and keeping its label in view. Overflowing entries give a column
   to the shared scrollbar and are laid out one cell narrower.

8. **The host moves onto it.** The row menu and the unpin menu are
   `ActionMenu<Msg>`s built by `row_menu_for`, `pin_management_entries`,
   `menu_title`, and `unpin_entry`; the mux menu is `actions::mux_menu`,
   two entries, and `mux_menu.rs` and `row_menu.rs` are deleted. The
   worktree menu's list is an `ActionMenu<WorktreeAction>` held as the
   menu's first level, until CSP-740 rebuilds the rest of that menu on
   dialogs. Pin's shortcut is `p` (its detail drops `(p)`), and the mux
   menu's are `n` and `N`: a host menu shows only a key that does the same
   thing on the list, which a consistency check holds it to.

## Consequences

- **Visible changes** (the CSP-739 summary and the changelog list them):
  the mux menu loses its `Mux actions` heading and keeps its footer at
  40x10; the worktree list gains the menu footer and scrolls on a short
  screen instead of cutting its last entries; menus whose entries
  overflow show the shared scrollbar beside them and lay their text out
  one cell narrower; a menu that has a shortcut shows a key column, which
  moves its labels and details three cells right (the row menu of an
  unpinned row: `p  Pin`; the mux menu: `n` and `N`); `p` in the row menu
  pins, and `n` and `N` in the mux menu open their forms; the armed
  Forget reads `Forget · press Enter again to confirm` (it had `—`); and
  a menu that scrolls keeps its window while the cursor moves inside it
  instead of riding the last line.
- A new menu is a list of entries; it cannot drift from the others'
  keys, footer, cursor, confirmation, or scrolling.
- The conformance suite holds the framework's menu and every host menu to
  the conventions, and the worktree menu's footer is now checked.
- On a screen whose entries get fewer than five rows the scrollbar's
  column is kept but no bar is drawn (the shared gutter's track keeps a
  row clear above and below); the control page records the seam.

## Alternatives Considered

- **Disabled entries the cursor can rest on**, as some desktop menus and
  the WAI-ARIA menu pattern have them. The row would highlight while
  `Enter` did nothing, a silent refusal (house rule 7), or the menu would
  need a refusal of its own. Stepping over them keeps `Enter` meaningful
  on every row the cursor reaches; the detail still says why.
- **Hiding entries that cannot run.** The host still omits what never
  applies; `enabled` is for an action that applies but not now, where
  seeing it teaches that it exists.
- **`Esc` disarms before it closes.** An armed entry is not a level the
  footer names, leaving loses nothing (`Esc close`), and it would take one
  more key to leave.
- **The footer says `Enter confirm` while an entry is armed.** A second
  place saying what the row already says, away from where the eye is.
- **The shortcut after the label, or at the row's right edge.** After the
  label it reads as part of the name, and colored it would be a second
  cue inside the highlight (ADR 0159); at the edge the highlight would
  have to run to the edge, the seam ADR 0159 left open. A key column is
  how help pages and other terminal menus show keys.
- **Shortcuts before the table.** `j` or `q` could then be accelerators,
  but the movement and close keys would stop being the same in every
  menu.
- **Keeping the menus as separate host widgets over shared helpers.**
  Less churn, but the three lists would keep drifting.
- **No scrollbar.** The menus would not say that entries are off screen;
  the shared gutter is how every other scrolled body says it.
- **The host's own words for an armed entry** (`press Enter again to
  forget`). Clearer for a destructive verb, but one generic sentence
  covers every menu today; the page keeps it as a seam.

## Open Questions Answered

- *Are disabled entries skipped by the cursor or just not committable?*
  Both: the cursor steps over them, and nothing commits them.
- *Does a shortcut confirm an inline entry?* It arms it; the next press
  of the shortcut or `Enter` commits it.
- *Which keys do the host's menus show?* Only a key that does the same
  thing on the list: `p` on Pin, `n` and `N` in the mux menu.
