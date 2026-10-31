# 0055: A settings panel applies each change as it is made

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/settings_panel.md](../controls/settings_panel.md) |

## Decision

- **Rows from the host's context.** A `SettingsPanel<Id, M, Cx>` is a title
  and a row model: a closure that builds titled sections of rows from the
  host's `&Cx`, called on every key and every draw, so a row always shows
  the value in force. The panel keeps only its cursor (the host's row id, so
  it stays on its row when rows around it come and go) and its scroll
  window.
- **Five kinds of row, as data.** A choice, a checkbox, an editor (a value
  and an `[ Edit ]` button that opens its editor), a cycle, and a button;
  each carries the host's message, or none when acting on it changes
  nothing, and whether acting on it leaves the panel.
- **Changes apply as they are made.** Acting on a row reports its message at
  once with `Stay`; the host applies it and the next draw shows it. `Enter`
  on a choice applies it and closes. `Esc` closes and `q` and `Ctrl-C`
  dismiss, with nothing to report and nothing to undo, silently.
- **Editors are steps.** An editor row asks the host to open its editor as a
  step over the panel; the editor answers `Back(M)`, so the change applies
  and the panel has the keys again (record
  [0034](0034-steps-and-flows.md)). Under a step the panel draws its cursor
  row in the inactive look.
- **One table and one look.** `SETTINGS_KEYS` (the moves wrap; paging and
  the ends stop; `Space` changes; `Enter` applies and closes; a note says
  changes apply as they are made). The menu-sized frame; bold section
  headings with a blank line between sections; rows through the cursor
  look; editor and cycle names in a column the host can fix so values line
  up; a fixed footer; sections that scroll in a kept window with the shared
  scrollbar, a section's heading brought into view with its first row.

## Rationale

Settings that apply as they change let the user see each effect at once,
and an `Esc` that keeps what was done needs no undo. Editors as steps put
them on the stack every other flow uses, instead of a second stack hidden
inside one overlay.

## Consequences

- A staged panel (pending changes with *Apply* and *Cancel*) is a seam on the
  page, not built.

## Alternatives considered

- **Editors as levels inside the panel.** Self-contained, but a second,
  invisible stack with its own `Ctrl-C` rule.
- **`Stay`, then the host pops the step.** Every host would need a message
  meaning "and close the step".
- **Staged apply.** It would change what `Esc` means for every setting.
- **A trait for the row model.** A closure lets one context feed several
  panels and a test feed a fixture.
- **A positional cursor.** A row appearing above it would move it to another
  setting.

## Origin

- host ADR 0176, "Settings panels are one control that applies each change
  as it is made, and their editors are steps": decisions 1 to 3, 6, and 7.
