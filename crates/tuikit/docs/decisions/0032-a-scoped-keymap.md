# 0032: A program's own keys are one scoped keymap with stable ids and no shadowing

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/keys.md](../controls/keys.md) |

## Decision

- **Scopes.** A binding lives in a `Scope`: `Global` (the program), a
  `Pane(name)` with focus, or an `Overlay(name)` mode. `ActiveScopes` looks a
  key up in the most particular scope first: the mode, the focused pane,
  then the program.
- **Bindings.** A binding is declared as a `const` and carries a stable id
  (lower-case letters, digits, `.`, and `-`), its keys (the first is the one
  a hint names, the rest aliases), the action, a short verb (none keeps it
  out of hints), the help section and text, a status rank, and an optional
  availability predicate. The keymap is generic over the action `A` and the
  context `Cx`.
- **No key shadows another.** `KeymapBuilder::build` returns every
  conflict: two bindings with one id, an id that is not well formed, a
  binding with no key or no help, and any two keys of one scope that one
  key event fires (`KeyMatcher::overlap`). Keys of different scopes may
  overlap by design; `Keymap::overrides` lists where a pane's or a mode's
  binding takes a global key. Because nothing shadows, a table's order
  decides nothing about resolution: it is the order the help page lists.
- **A control's table is a pane's scope.** `KeymapBuilder::table` takes a
  control's key table in, one binding per action, without copying its rows,
  so the control's footer and the program's help come from one source.
- **Remapping by id** is a seam: a user's remap would replace the keys of
  the bindings it names before `build`, which checks the result.

## Rationale

A flat program table resolved by order hid one binding behind another, and
pane keys were special cases in the event loop. Scopes make which keys act
where explicit, a build that refuses overlaps makes shadowing impossible,
and stable ids make a remap a configuration change rather than a code one.

## Consequences

- A program's help page and status hints are generated from its keymap
  (records [0035](0035-generated-help-pages.md) and
  [0066](0066-the-status-bar.md)).

## Alternatives considered

- **Keeping a flat table and giving its rows scopes.** Smaller, but its rows
  had no section or verb, so the status bar would still need a second
  table.
- **Rows over the host's own action type.** A host's action may hold boxed
  messages and not be `Copy`, so no `const` table; a `Copy` command enum the
  host maps to its action keeps the table `const`.
- **Order-dependent resolution with a "shadowed" check only.** The order
  would keep deciding meaning, and reordering for the help page could
  change what a key does. Refusing any overlap in a scope makes order free.
- **Copying a control's keys into the host's table.** One row type, but a
  copy of a framework table.

## Origin

- host ADR 0192, "The main screen's keys are one scoped keymap, and the help
  page is generated from it": decisions 1 to 4.
