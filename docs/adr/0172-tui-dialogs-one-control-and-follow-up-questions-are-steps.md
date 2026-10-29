# ADR 0172: Dialogs Are One Control, And A Menu's Questions Are Steps On The Stack

## Status

Accepted. Carries out the `Dialog<M>` control of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), applying its
house rules 3, 4, 5, 8, 13, and 14 to every confirmation, choice, and
notice, and extends [ADR 0150](0150-tui-framework-workspace-crate.md) with
the framework's `dialog` module. Amends
[ADR 0161](0161-tui-overlays-draw-themselves-on-one-host-owned-stack.md):
`Outcome` gains `Dismiss`, the stack opens an entry as a step of the one
under it, a `Commit` or `Dismiss` from a step closes its whole flow, and
`Settled.closed` lists every entry a key closed. Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md):
`DIALOG_KEYS` (`DialogKey`) is the one dialog table and replaces
`CONFIRM_KEYS` and `ChoiceKey<C>`, a choice's verbs being the dialog's own
keys, and a footer can keep some of a table's actions (`Footer::only`) or
take a control's own segments. Amends
[ADR 0171](0171-tui-menus-one-control-entries-as-data.md) decision 8: the
worktree menu is an `ActionMenu<Msg>` whose entries open steps
(`Entry::opens_step`), and its mode machine is gone. Amended by
[ADR 0176](0176-tui-settings-panels-apply-as-they-change-and-editors-are-steps.md):
the seam for a step that applies its answer and returns to the entry
under it is built as `Outcome::Back`.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md`, sections
3.3, 3.6, 3.12, 4, and 7) found four confirmation idioms, and the worktree
menu held three of them in one widget:

- **A mode machine.** `WorktreeMenuState` was a list, a branch prompt, three
  y/n screens (remove, merge, prune), and an `m`/`d` close-down choice,
  switched by a `Mode` enum. Its three confirm handlers differed only in the
  message they built, and its three renders each repeated the
  `▲ hosts N live session(s):` block. CSP-739 made the list an
  `ActionMenu` and CSP-734 the prompt a `TextPrompt`, both still held as
  levels of the mode machine.
- **A rule with no home.** `Esc` stepped back to the list, and `Ctrl-C`
  closed the whole menu (ADR 0161) only because the menu checked `Ctrl-C`
  before handing a key to its prompt. Opening the prompt on the stack
  instead needed a rule for one key closing two entries (the CSP-734
  operator-review row).
- **Two tables for one look.** The confirmations resolved `CONFIRM_KEYS`;
  the close-down choice resolved a host table, `CLOSE_DOWN_KEYS`, which
  bound `Esc` but not `n`.
- **Screens that lost their keys.** On a 40x10 terminal every confirmation
  and the close-down choice cut their footer off; the choice then showed
  no way to answer at all.
- **A notice drawn by hand.** Clean up with nothing to clean up drew its
  own frame with a hand-typed `Esc/Enter close` and answered `MENU_KEYS`.
- **`X` opened the menu's choice mode** over a one-entry list nobody had
  opened, so `Esc` there showed that list.

## Decision

1. **`tuikit::dialog` owns the dialog.** Its items: `Dialog<M>`, `Kind<M>`
   (`Confirm(M)`, `Choice(Vec<Verb<M>>)`, `Notice`), `Verb<M>` (a key, a
   verb, a message), `Warning` (a heading and items), and `DIALOG_SIZE`
   (`ModalSize::MENU`). A dialog is a title, body lines (each wrapped; an
   empty line is a blank row), an optional warning list, and its form. It
   implements `Overlay<M, Cx>` for every context and reads none; its
   answers are data, so it is `Clone` and `PartialEq` when `M` is.

2. **One table for every form.** `DIALOG_KEYS` binds `Ctrl-C` and `q` to
   `Abort` (`Dismiss`), the arrows and `k`/`j` to scrolling an overflowing
   body (aliases), `Enter` and `y` to `Confirm`, and `Esc` and `n` to
   `Cancel` (`Close`). A confirmation commits on `Confirm`; a notice closes
   on `Confirm` or `Cancel` and commits nothing; a choice commits a verb on
   the verb's key, resolved after the table, and has no default, so
   `Enter` and `y` do nothing in it. `Dialog::shadowed_verbs` finds a verb
   on a key the table takes. `CONFIRM_KEYS`, `ConfirmKey`, and
   `ChoiceKey<C>` are removed, and the host's `CLOSE_DOWN_KEYS` with them.

3. **One generated footer per form.** A confirmation's footer is
   `Footer::new(DIALOG_KEYS)` (`Enter/y confirm · Esc/n cancel`); a
   choice's is each verb's key and verb, then the table's cancel segment
   (`Footer::only`, which keeps the rows of some actions); a notice's is
   the first keys the table shows for confirming and cancelling under the
   verb `close` (`Enter/Esc close`), since leaving a notice loses nothing.
   A control composes such a footer by collecting `Segment`s into a
   `Footer`. `Dialog::footer` returns it, and a conformance case takes it.

4. **One look.** The menu-sized frame with the title on its top border;
   inside, a blank row, the body, a blank row and the warning list (the
   warning mark and heading in the theme's `warning` and bold, its
   continuation under its first word; each item after `  - `, wrapped
   under its first word), a blank row, and the footer. On a short screen
   the blank rows give way first (above the body, then above the keys),
   then the footer keeps the bottom rows (all of them if it must, since
   the title says what is asked) and the body scrolls in a window the
   dialog keeps, with the shared scrollbar.

5. **Steps.** `ModalStack::push_step` opens an entry as the next step of
   the entry on top; on an empty stack it opens as `push` does. The entry
   and its steps are a flow: `Outcome::Close` from a step closes the step
   only, so `Esc` gives the entry under it its keys back; the new
   `Outcome::Dismiss` and `Outcome::Commit` close the whole flow (the top
   and every entry it is a step of) and nothing under it. An entry opened
   with `push` is never part of the flow under it, and for it `Dismiss` is
   `Close`. `Settled.closed` is the list of closed entries, top first;
   `close_flow` and `top_is_step` complete the stack's API.

6. **`Ctrl-C` and `q` answer `Dismiss`.** The framework's menu, prompt,
   form, and dialog return `Dismiss` for their `Abort` keys and `Close`
   for `Esc` (house rule 4). A host overlay that is never a step may keep
   answering `Ctrl-C` with `Close`, which closes it the same way.

7. **A menu entry can open a step.** `Entry::opens_step` marks an entry
   whose action asks a further question: picking it reports its message
   with `Outcome::Stay`, so the menu stays open, and the host opens the
   question over it with `push_step`. Picking an entry disarms the menu.

8. **The worktree menu is a menu, dialogs, and a prompt.** `w` opens
   `worktree_menu(ctx)`, an `ActionMenu<Msg>` titled `Worktree` over the
   policy's wired verbs; a reveal commits `Msg::SelectRow`, and every
   other entry opens a step with `Msg::OpenWorktreeStep { action, context
   }`, whose reducer arm builds the step from the captured facts
   (`worktree_menu::step`): the branch `TextPrompt` (`Modal::Prompt`), the
   merge, remove, and prune confirmations, and the close-down choice
   (`Modal::Dialog`), each titled `Worktree` with the bodies the screens
   had. `X` opens the close-down choice alone. `context_for_node`, the
   `WorktreeAction` policy, every word, and the `Msg::CommitWorktree*`
   mapping stay in Conspectus; `WorktreeMenuState`, its widget, and its
   `Mode` are gone. Remove and Close down act on the branch, so on a
   worktree with none checked out they are shown, disabled, with the
   reason in their detail.

9. **The empty Clean up is a notice.** With no record past its reap age,
   `Msg::OpenCleanUp` opens `clean_up::empty_notice()`, a `Dialog` notice
   with the old words, instead of a checklist with an empty mode.

## Consequences

- **Visible changes** (the CSP-740 summary lists them): every dialog takes
  one keymap, so the close-down choice also cancels on `n` and its footer
  reads `Esc/n cancel`; the branch prompt and the dialogs open over the
  menu, which shows around the prompt and above and below a dialog
  shorter than it; `Esc` in a step returns to the menu with its cursor
  where it was, and the menu's opening hint leaves the status line; at
  40x10 every confirmation, the close-down choice, and the Clean up notice
  keep their keys, and a body that does not fit scrolls; dialogs gain a
  blank row above their body; the close-down and prune headings lose their
  bold, and the warning heading takes the theme's `warning`; prune's
  explanation is one wrapped sentence; the Clean up notice's keys read
  `Enter/Esc close` (it said `Esc/Enter close`) and `y` and `n` close it
  too; `X` no longer posts a status hint, and `Esc` closes its choice
  instead of showing a one-entry list; Remove and Close down are disabled
  on a worktree with no branch.
- `Ctrl-C` and `q` close the worktree flow from any step, as they did
  since ADR 0161, now through the stack instead of a check in the menu.
- A follow-up question is a modal of its own on the stack, checked by the
  registry and the conformance suite like every other: `Modal::Dialog` and
  `Modal::Prompt` are generic, so a later question needs no registration.
- The conformance suite finds a modal's frame by the corners that differ
  from the screen under it. A step drawn exactly over its menu's frame
  leaves no corner to find, so the host's step cases use worktree facts
  that size them apart, and the 40x10 body check compares against the
  screen under the whole flow.

## Alternatives Considered

- **Keep the mode machine over the framework's controls.** Least churn,
  but the worktree menu would stay the one overlay with levels the stack
  cannot see, and the next menu with a question would grow its own.
- **The host checks `Ctrl-C` before a step sees it** (what the menu and
  Controls did for their levels). It works for one flow, but every flow
  would repeat it, and the stack would still not know that two entries
  are one overlay.
- **`Ctrl-C` closes every entry on the stack.** Simpler than flows, but it
  would close an unrelated overlay under a flow, and a discard question
  over a form would take the form with it.
- **A step's `Commit` returns to the entry under it.** A flow that is done
  would leave its menu open after acting; a step that applies and goes
  back is a seam (`Stay`, then the host pops it).
- **Three standard tables, one per form.** Each would read right on its
  own, but a choice's keys are its verbs, so its table would still be the
  host's, and the three would drift; one table with footers composed from
  it keeps one keymap.
- **A default verb in a choice** (`Enter` merges). It would put a
  destructive close-down two `Enter`s from the list; kept as a seam.
- **A cursor over a choice's verbs**, drawn as rows. It would make a
  choice a menu inside a dialog; the verbs' own keys answer it.
- **The compact frame for dialogs** (the prompt's). A dialog over the
  menu-sized menu would leave the menu's border and gutter (its cursor
  mark) showing in the column beside it; one width covers the menu
  cleanly, and the old screens and the notice were menu-sized.
- **Titles that name the action** (`Remove worktree`). They repeat the
  first body line in every dialog; the flow's title, `Worktree`, is what
  the screens had.

## Open Questions Answered

- *What closes when `Ctrl-C` is pressed in a step?* The step and every
  entry it is a step of: the whole flow, never what is under it.
- *Does `Esc` in a step close it or the flow?* The step; the menu has the
  keys again.
- *Does a choice have a default?* No; its verbs' keys answer it.
- *Is the Clean up notice a mode of Clean up?* No, a dialog of its own.
