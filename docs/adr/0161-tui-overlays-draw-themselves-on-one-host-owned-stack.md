# ADR 0161: Overlays Draw Themselves On One Host-Owned Modal Stack

## Status

Accepted. Amends ADR 0085 contract 3 (the modal stack and the one `Overlay`
contract): the contract is the framework's `tuikit::overlay`, its `handle`
and `render` both take the host's context, the stack draws every entry, and
the outcome has four variants. Amends [ADR 0123](0123-overlay-keymap-tables.md):
`Ctrl-C`, the `Abort` action every table binds first, dismisses the whole
overlay from any level (ADR 0123 left each handler to decide, and the
worktree menu's screens stepped back on it), and `q` joins it wherever
nothing is typed. Amends [ADR 0156](0156-tui-scroll-window-follow-rule-and-viewports-by-value.md):
a scrolled modal keeps the window it drew as it draws, so the viewport
report carries only the panes. Applies house rules 4 and 5 of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md) and extends
[ADR 0150](0150-tui-framework-workspace-crate.md) with the framework's
`overlay` module.

## Context

ADR 0085 contract 3 promised that the App holds a stack of modals, that
input routes to the top of the stack, that `draw` renders the stack in
order, that every overlay implements one trait with `handle` and `render`,
and that adding a modal is a state struct, a trait impl, and one variant.
The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/06-interaction-conventions.md`,
section 2) found it half built:

- **Only the top modal drew.** The trait had `handle` and no `render`.
  `ui::draw` called fifteen `draw_*` helpers in a fixed order, each reading
  an accessor that peeked only the top of the stack, so a modal under
  another was never drawn. The stacking tests asserted accessors, never
  frames.
- **Routing was a ladder.** A sixteen-branch `if app.x().is_some()` ladder
  produced one of sixteen `Action::*Key` forwarders, each answered by its
  own handler in the live loop, the fixture TUI, and the snapshot driver;
  fourteen handlers shared one skeleton. The newest modal touched about nine
  production sites, and nothing checked that a new modal was routed: a
  missing branch would have sent its keys to the main keymap.
- **The context was copied.** Each overlay declared its own context type.
  Because the stack is a field of the App, an overlay borrowed mutably could
  not read the App, so Controls cloned an eleven-field snapshot before every
  key and the search overlay built owned vectors of its rows and results.
- **The viewers stood apart.** The transcript and pane viewers implemented
  no trait and drew through their own path, which also kept toasts from
  showing over them.
- **Closing disagreed.** Six overlays said `<x>: cancelled` on Esc, the
  transcript viewer said `viewer closed`, and the rest said nothing.
  `Ctrl-C` closed Controls from any level but stepped back in the worktree
  menu; `q` closed the pagers and Messages, did nothing in the menus, the
  checklists, the confirmations, and Controls, and backed out of the
  close-down choice; in the viewers `q` closed only the help page.
- **The documents said otherwise.** `modal.rs` and ADR 0085 described the
  whole contract as done.

## Decision

1. **`tuikit::overlay` owns the contract.** `Overlay<M, Cx>` has
   `handle(&mut self, key, &Cx) -> Outcome<M>` and
   `render(&mut self, area, buf, &Cx, &Theme)`, plus `fills_screen` (an
   overlay that covers the whole screen) and `takes_text` (a text field has
   the keys, so a paste goes there). `M` is the host's message and `Cx` its
   context. `Outcome<M>` is `Consumed`, `Close`, `Commit(M)` (close, then
   the host handles `M`), or `Stay(M)` (the overlay stays open and the host
   handles `M`). `Stay` is kept because Controls applies settings as they
   change, the search overlay regroups its results, and an overlay that
   refuses input posts a notice while it stays open. There is no "this key
   is not mine": an open overlay owns every key.

2. **The stack is the host's, over the host's entry type.**
   `ModalStack<E>` routes a key to its top entry (`route`), does the stack's
   part of the outcome (`settle`: the top entry closes for `Close` and
   `Commit`, and comes back so the host can do its bookkeeping), draws every
   entry from bottom to top (`draw`), starting at the topmost entry that
   fills the screen, and opens and closes entries (`push`, `pop`, `pop_if`,
   `close_all`). `E` is the host's choice: its own enum of overlays, or a
   boxed trait object.

3. **One registration lists the modals.** A `modals!` invocation in
   `src/tui/modal.rs` declares the `Modal` enum, the list of its names, and
   the `Overlay<Msg, App>` impl that hands each call to the entry. Each
   entry implements the trait in `src/tui/modal/entries.rs`, delegating to
   the widget's own key handling and widget. Adding a modal is a state
   struct, an impl, and one line of the registration; the keymap, the
   loops, and the draw take no new branch.

4. **The App is the context, and it lends its stack out.** Routing and
   drawing take the stack out of the App for the call
   (`App::with_modals`), pass the App as `&Cx`, and put the stack back.
   Controls and the search overlay read the live App; nothing is copied. An
   entry reads its own state from itself, since the App's stack is empty
   while it is lent out: the one App method that read the top modal during
   a draw (the content modal's query) now takes the query from the entry
   (`App::content_view_for`).

5. **One route.** Any key press an open modal owns becomes one
   `Action::ModalKey`. `runtime::modal_key` routes it, settles it, does the
   host's bookkeeping for the entry that closed (the status line is
   cleared; a closing search remembers its options and drops its preview
   unless it was kept), and hands what the overlay committed or sent to the
   caller's executor: the live executor in the live loop, the pure one in
   the fixture and scenario TUI, and the snapshot driver's own policy keyed
   by the entry's name. The value modal and Messages copy through
   `Msg::Copy` and `Effect::Copy`, since the clipboard write is the
   executor's.

6. **One draw.** `ui::draw` draws the header, panes, and status bar unless
   an open entry fills the screen, then every entry bottom to top, then a
   toast. Each entry clears its own frame (`frame::ModalFrame` does) or
   fills the screen. `render` takes `&mut self`, so the help page, the value
   modal, and Messages keep the window they drew, and the viewport report
   carries only the panes.

7. **The viewers are entries.** Both fill the screen. The transcript
   viewer stays a nested reducer, the reference example of ADR 0085
   contract 1: its `handle` sends `Msg::Viewer` or `Msg::ViewerFind`, whose
   reducer arms apply the message to the entry on top and close it when
   the viewer's own reducer says to leave. The pane viewer handles its keys
   itself and asks to attach with `Stay(Msg::AttachPane)`. Their draws take
   a buffer, not a terminal frame.

8. **One rule for leaving (house rule 4).** `Esc` steps back one level
   (`Consumed` while there is a level to return to: an editor in Controls,
   a worktree screen, the search options, a viewer's help page or find) and
   closes the overlay at its first level. `Ctrl-C` dismisses the whole
   overlay from any level. `q` dismisses it too wherever nothing is typed:
   the standard menu, confirmation, and checklist tables bind it to their
   `Abort`, the pager table to `Close`, and a choice dialog to its `Abort`;
   the text-input and form tables leave it to type. Whether something is
   typed is a property of the level: a form, a prompt, a query, and a find
   prompt type `q` (a form on one of its option rows included); Controls
   dismisses on `q` except in its max-age and contents editors.

9. **Cancelling is silent (house rule 5).** Closing posts no status message
   and no toast. A key that closes an overlay also clears the status line,
   so a hint the overlay showed while it was open does not outlive it.

10. **Checked by a registry.** `consistency_tests/registry.rs` builds every
    registered modal and fails when one is not built there. For each it
    checks that every key press is the modal's while it is on top, that
    `Ctrl-C` reaches it and closes it, that it draws and stays drawn under a
    prompt, and the close keys of decisions 8 and 9, at its first level and
    inside it. The layout suite checks a stacked pair: over any modal a
    prompt draws as it does alone, and outside the prompt the modal draws
    as it does alone.

## Consequences

- **Visible changes** (the CSP-729 summary and the changelog list them):
  every open modal is drawn, so one opened over another shows the lower
  one around its frame (no operator key opens a modal over another today:
  each opener closes the one it came from); a toast shows over the
  full-screen viewers; `q` closes the row, mux, and worktree menus, Clean
  up, Controls and its checklists, the worktree confirmations, the
  close-down choice (which it used to back out of), and the content search
  options; `Ctrl-C` closes the worktree menu from its branch prompt,
  confirmations, and close-down choice instead of returning to its list;
  in the viewers `q` and `Ctrl-C` leave from the help page and `Ctrl-C`
  from the find prompt; the six `cancelled` messages and `viewer closed`
  are gone, and a closed overlay's hint no longer stays on the status
  line. In the live TUI, Controls, the help page, the value modal, and
  Messages commit through the live executor like every other overlay.
- **Footers do not change.** `q` is an alias in every table that gained
  it. The viewers' help page still reads `Esc/q/? close` from the main
  help's table while `q` now leaves the viewer; the generated help page of
  the `Pager` extraction replaces that footer.
- **Adding a modal** is a struct, an impl, and one registration line, and
  the registry test fails until it is built there. The per-overlay
  accessors (`help_overlay()`, `close_row_menu()`, and the rest) remain as
  conveniences; a new modal needs none (`App::open_modal`,
  `App::top_modal_name`).
- **Lending the stack has a rule.** While the stack is lent out, an entry
  that asked the App for the modal on top would see none. The registry and
  layout checks draw and route every modal, so such a read shows up as a
  wrong frame or key there.
- **The snapshot driver** no longer prints the clipboard escape of a
  Messages copy into its frame; it skips the copy with a note, as it did
  for the value modal.
- ADR 0085 contract 3's sketch now shows the shape that exists, and
  `src/tui/modal.rs` describes it.

## Alternatives Considered

- **The stack beside the App** in a host struct, so borrowing both needs no
  lending. The reducer opens modals from about thirty `update` arms, so the
  stack would have to travel with every `update`, or opening a modal would
  have to become an effect; either is a larger change than this decision
  needs.
- **A context narrower than the App** (a struct of borrowed fields). The
  stack could then stay a field, but Controls and the search overlay read a
  dozen values the App computes in its own methods, which a field-borrowing
  view cannot call; it would copy them back in.
- **Boxed trait objects as the host's entries.** The framework supports
  them, but the host's per-overlay accessors and the bookkeeping when a
  search closes need the concrete variants, and an enum keeps `Clone` and
  `Debug` for the App.
- **Draw only the top entry**, as before. Simpler and invisible in today's
  flows, but it keeps a modal under another undrawn, which ADR 0085
  promised against and any future stacking flow would trip over.
- **The stack owns `Ctrl-C` and `q`**, dismissing before the entry sees
  the key. Every overlay would follow decision 8 with no table change, but
  it would be a second source of keys that footers and the key-table
  checks of ADR 0160 cannot see; the keys stay in the tables.
- **A `Back` outcome** for stepping back a level. Stepping back is the
  overlay's own business and changes nothing on the stack, so it is
  `Consumed`.
- **Keep the clipboard write in the runtime** before the key reaches the
  overlay. It was the only per-overlay branch left in the route; a message
  and an effect put it where every side effect goes.

## Open Questions Answered

- *Does the stack draw every entry?* Yes, bottom to top, each entry
  clearing its own frame; nothing dims what is underneath (a seam the
  framework page records).
- *Does the stack live outside the model, or is it lent out?* Lent out,
  for the call, by the App that owns it.
- *Is cancelling reported?* No. It is silent, and the status line is
  cleared.
- *Is `q` typed or a way out in a form's option row?* Typed: a form is a
  typing surface throughout.
