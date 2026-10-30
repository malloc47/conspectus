# tuikit

A terminal UI framework built on [ratatui](https://ratatui.rs). It supplies
the pieces that make an application's screens look and behave alike: design
tokens, ready-made controls, and interaction conventions, together with the
tests that hold controls to those conventions.

The framework serves a host application. The host owns what the program means
and does; the framework owns how a screen is drawn and how a key press is
read. The two meet through type parameters, closures, traits, and a registry
of named styles, never through shared types.

## Layers

| Layer | Name | In this crate | What it holds |
| --- | --- | --- | --- |
| 1 | Components and design tokens | yes | Theme and palette, glyphs, text measuring and wrapping, scroll helpers, chips, relative ages, frames and selection looks, and the controls built from them: text entry, menus, dialogs, checklists, palettes, settings panels, pagers, help pages, logs, trees, property grids, explorers, toasts, and chrome. |
| 2 | Interaction conventions | yes | Key tables that generate footers, hints, and help; the overlay and modal-stack contract; focus rules; a feedback model that drives the status line, toasts, and a message log; a clipboard sink; and a test kit. |
| 3 | Application runtime | no | The event loop, the effect lifecycle, undo, and terminal hand-off to child processes stay with the host. |

[`docs/architecture.md`](docs/architecture.md) gives the module map, the
one-way dependency rule, the host message type `M` and context type `Cx`, and
the list of things the framework never does.

## Status

Internal reuse only. The crate is not published (`publish = false`), carries
no semver promise, and has no release pipeline. `tuikit` is a working name
chosen to carry none of the host's vocabulary, so renaming it is mechanical.

The crate is built to leave its current repository by a directory lift, so its
documentation has to stand alone: someone who has never seen the host should
be able to learn what the framework is, add a control, and find the rules that
keep it independent from these pages.

The crate is populated one module at a time as controls move in from the
host. Today it holds the `theme` module (design tokens, named styles, and
the style-spec grammar), the `glyphs` module (shared marks, the vetted-glyph
table, and the source scanner), the `text` module (measuring, cutting,
wrapping, snippets, match painting, and the one rule for pieces of a line
that give way, in terminal cells), the `breadcrumb` module (the chain of
steps that led to what a pane shows, which gives way hop by hop), the
`master_detail` module (a list zone sized to its rows over a divider over
a detail area, the list scrolled in a window its host keeps), the
`app_frame` module (the layers a screen is drawn in: a header, the body,
and a footer, then the overlays, then the toasts), the `tab_bar` module
(the names of a pane's views, the active one kept, which give way tab by
tab), the `priority_line` module (a line of ranked segments that drop
lowest first to fit and fill the room back), the `status_bar` module (the
bottom row of a screen: a message in its severity's style, or the keys
that act now, generated from key tables and fitted by priority beside the
host's segments), the `empty_state` module
(what a slot says when it has nothing to show, one look for every such
message), the
`text_edit` module (the line editor every text field edits with and the
text field that draws it), the `text_prompt` module (the one-line prompt
that commits, refuses with a reason, or stays), the `path_field` module
(the path field that completes through a source its host supplies), the
`form` module (the multi-field form that submits from any field and keeps
its keys in view), the `action_menu` module (the action menu whose entries
are data, with shortcuts, entries that cannot run, an inline confirmation,
entries that open a step, and a footer that stays in view while the
entries scroll), the `dialog` module (the dialog that confirms, offers a
choice of named verbs, or tells, on one key table), the `checklist`
module (the checklist that checks one item, all, or none, counts what is
checked, and hands the checked indices to its host), the `log_list`
module (the reader of a log: its entries newest first over the selected
one's detail in an embedded pager, read from the host's context), the
`settings_panel` module (the settings panel whose rows its host builds
from its context, which applies each change as it is made and opens its
editors as steps over itself), the `palette`
module (the palette: a query with its caret over results its host ranks
or sends, with scope strips, a status row, an options pane, a debounce
for a slow provider, and a footer that stays in view), the `scroll`
module (cursor stepping, the follow rule, the clamping scroll window, the
scrollbar gutter, wrapped-row counts, and the scroll region whose offset
counts from the top, the bottom, or a marked row), the `columns`
module (a row of cells declared once as keyed columns, measured over a
whole list, and given way by one narrow-width policy), the `tree` module
(the tree: a state kept apart from its host's rows, with the selection
kept by identity across rebuilds, folds, and paging by the rows shown, and
the pane that draws the rows from lines its host lays out or from column
schemas), the `property_grid` module (the rows a detail pane lists about
one thing: properties in a label column with values cut in cells, links
in a verb column in one of four tones, and section headers that fold,
each reporting what `Enter` and an inspect key ask its host to do), the
`explorer` module (a cursor over zones of those rows that its host
supplies for each call, kept by row key across rebuilds, with sections
that fold, a drill stack that follows a link and comes back, a two-press
`Back` that leaves, one key table, and an outcome for every key that
says what is left for its host), the
`richtext` module
behind its feature (ANSI text, captured terminal output fitted in a wrap
mode, and Markdown with its tables and highlighted code), the `chip` module (filled,
toggle, segmented, position, and status chips), the `age` module (age
text, recency tint, and the age cell), the `frame` module (the modal
frame, the one modal footer in its one look and fit, its named sizes, the pane frame
with the focus cue on its border and title, the split pane, and the
section rule that heads a section of a pane), the `selection` module (the list cursor's
marker and highlight, the ghost and pending looks, and the wrap-or-clamp
rule), the `keys` module (key tables, footers, key labels, roles, the
standard tables for menus, dialogs, pagers, prompts, checklists, log lists,
palettes, settings panels, and forms, and the scoped keymap a program's own
keys live in), the `overlay` module (the overlay contract, its outcomes, and the
host-owned modal stack that routes keys to its top entry, opens a
follow-up question as a step of the overlay that asks it, and draws every
entry), the `help` and `feedback` modules, the `testkit` module behind its
feature (key scripts, the one-frame driver, buffer probes, ANSI text,
style-aware goldens, the conformance suite, and source-scan helpers), the
source guards, and this documentation set; the module map in the
architecture guide says which modules have landed.

## Depending on it by path

A host depends on the framework by path:

```toml
[dependencies]
tuikit = { path = "crates/tuikit" }
```

The path is relative to the host's manifest. If the framework moves to its own
repository, only this line changes (to a `git` or other `path` dependency).

The framework's public API exposes ratatui types, so the host must use the
same ratatui version the framework declares; a mismatch shows up as
confusing "two different types with the same name" errors.

Cargo features are off by default and add to the build only what a host asks
for. The `richtext` feature turns ANSI text, captured terminal output,
Markdown, tables, and syntax highlighting into styled lines for content
views; it is the only feature that pulls in the heavier parsing crates, so a
host that shows such content names it in its dependency:

```toml
[dependencies]
tuikit = { path = "crates/tuikit", features = ["richtext"] }
```

The `testkit` feature turns
on the test kit for a host's tests and dev tools; it adds no dependency, and a
host enables it from its dev-dependencies so production builds leave it out:

```toml
[dev-dependencies]
tuikit = { path = "crates/tuikit", features = ["testkit"] }
```

## Independence

The crate imports nothing from its host, names none of the host's domain
vocabulary in source, documentation, tests, or manifest, starts no process,
and writes no file. Guard tests enforce each rule, and a reviewed allowlist
process exists for the rare exception. The rules and the process are in
[`docs/contributing.md`](docs/contributing.md).

## Documentation

- [`docs/architecture.md`](docs/architecture.md): layers, module map, the
  dependency rule, `M` and `Cx`, intent outcomes, and what the framework never
  does.
- [`docs/contributing.md`](docs/contributing.md): independence rules and
  guards, the documentation lint, how to add a control, the house rules, how a
  change catalogs visible differences, and how to test.
- [`docs/controls/_template.md`](docs/controls/_template.md): the page every
  control fills in; finished pages sit beside it.
- [`docs/decisions/README.md`](docs/decisions/README.md): the index of
  framework-scoped decisions and the policy for adding to it.

## Working on the crate

```text
cargo test -p tuikit
cargo test -p tuikit --features richtext
cargo test -p tuikit --test independence
RUSTDOCFLAGS='-D warnings' cargo doc -p tuikit --no-deps
```

The library denies undocumented public modules and items, so a missing page of
API documentation fails the build.
