# 0034: A follow-up question is a step, and a flow closes together

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/overlay.md](../controls/overlay.md) |

## Decision

- `ModalStack::push_step` opens an entry as the next step of the entry on
  top (on an empty stack it opens as `push` does). An entry and its steps
  are a flow.
- `Close` from a step closes the step only, so `Esc` gives the entry under
  it its keys back. `Dismiss` and `Commit` close the whole flow and nothing
  under it. `Back(M)` closes the step only and the host handles `M`: an
  editor applies its change and the entry under it has the keys again. For
  an entry opened with `push`, `Dismiss` is `Close` and `Back` is `Commit`.
- The framework's controls answer `Ctrl-C` and `q` with `Dismiss` and
  `Esc` with `Close` (house rule 4). `Settled.closed` lists every entry a
  key closed, top first.
- The entry under a step is drawn with `Overlay::render_under_step`, which
  is `render` unless the overlay overrides it; a list draws its cursor in the
  inactive look there (house rule 10). An overlay opened on its own over
  another is no step of it and leaves it drawn as it is.
- A menu entry whose action asks a further question (`Entry::opens_step`)
  reports its message with `Stay`, so the menu stays open, and the host
  opens the question over it as a step.

## Rationale

Menus that ask follow-up questions each ran a mode machine of their own,
with levels the stack could not see and a `Ctrl-C` rule of their own.
`Ctrl-C` must dismiss the whole flow it is pressed in, but not an unrelated
overlay under it, and an editor opened from a settings panel must apply and
return to the panel.

## Consequences

- A host builds multi-step flows from the framework's controls (a menu, a
  dialog, a prompt) on its stack, with no control of its own.

## Alternatives considered

- **A mode machine over the framework's controls.** Least churn, but levels
  the stack cannot see, and the next menu with a question grows its own.
- **The host checking `Ctrl-C` before a step sees it.** It works for one
  flow, but every flow repeats it.
- **`Ctrl-C` closing every entry on the stack.** Simpler than flows, but it
  closes an unrelated overlay under the flow.
- **A step's `Commit` returning to the entry under it.** A flow that is done
  would leave its menu open after acting.
- **`Stay`, then the host pops the step.** Every host would need a message
  meaning "and close the step"; `Back` names it once.
- **Telling every covered entry it lost focus.** One rule, but an unrelated
  overlay opened over a panel would restyle it; the step relation is what
  says a level of the same overlay has the keys.

## Origin

- host ADR 0172, "Dialogs are one control, and a menu's questions are steps
  on the stack": decisions 5 to 7.
- host ADR 0176, "Settings panels are one control that applies each change
  as it is made, and their editors are steps": decisions 4 and 5.
