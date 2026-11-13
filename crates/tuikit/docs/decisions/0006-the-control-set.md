# 0006: The control set

| | |
| --- | --- |
| Status | Accepted |

## Decision

The framework ships one control for each shape of interaction:

- **Foundations:** `theme`, `glyphs`, `text`, `scroll`, `chip`, `age`,
  `frame` (the modal frame, the pane frame, the split pane, the section
  rule), and `selection`.
- **Text entry:** `LineEdit` and `TextField`, `TextPrompt`, `PathField`,
  `Form`.
- **Choice and command:** `ActionMenu`, `Dialog`, `Checklist`, `Palette`
  (filter pickers, quick select, content search, and a command palette are
  instances), `SettingsPanel`.
- **Reading:** `Pager` with `Find`, `HelpPage`, `LogList`, and `richtext`
  behind a feature.
- **Data display:** `TreeView` with `TreeState` and `ColumnSchema`,
  `PropertyGrid`, `Explorer`.
- **Chrome:** `AppFrame`, `TabBar`, `PriorityLine`, `StatusBar`,
  `EmptyState`, `Breadcrumb`, `StackedMasterDetail`, and the pane frame with
  its focus cue.
- **Conventions:** `keys`, `overlay`, `help`, `feedback` (with the clipboard
  sink and the toasts), and `testkit`.

The tree landed first with lines its host lays out and then moved to column
schemas one kind of row at a time; the explorer came last, after a design of
its own. What stays with a host is an adapter: its row builders, its domain
colors and glyphs, its document models and preview sources, menus that query
its data, its message, action, and application types, its effects, and undo.

## Rationale

A survey of the host found about sixty surfaces and shared primitives, many
of them one control in different clothes: eighteen menu, dialog, prompt,
list, and toast surfaces, eleven text-entry surfaces, seven scrolling
readers, and eight tree views. One control per shape removes the copies and
the disagreements between them.

## Consequences

- Each control has a module, a reference page, and a row in the module map
  ([architecture.md](../architecture.md#module-map)).
- Focus has no module of its own: its rules live with the pane frame, the
  overlay contract, the keymap's pane scopes, and the selection look
  ([architecture.md](../architecture.md#focus)).
- Controls other frameworks ship that this set lacks (multi-line input,
  mouse hit testing, a declared focus cycle) are on the
  [roadmap](../roadmap.md); each affected control keeps the seam open.

## Alternatives considered

- **A smaller set that stops at a tree with lines the host lays out.**
  Cheaper, but leaves the columnar look in host code, with no consistency
  across row layouts.
- **The explorer left in the host.** Lower risk, but its drill stack and
  zoned cursor are reusable, and the property grid wants them (record
  [0062](0062-the-explorer.md)).
- **Multi-line input in the first cut.** Not needed yet; see record
  [0008](0008-the-third-party-crates.md) for the crate it would use.

## Origin

- host ADR 0151, "The framework control set and its house rules": the
  control set, its staging, and the open question on multi-line input.

Dropping the planned `focus` module in favor of the rules where they act
was settled while the documentation was completed and is first recorded
here.
