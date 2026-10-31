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

Every module the control set names has landed: the foundations (theme,
glyphs, text, scroll, chips, ages, frames, selection), the controls (text
entry, menus, dialogs, checklists, palettes, settings panels, pagers, help
pages, logs, trees and column schemas, property grids, the explorer, toasts,
and the chrome: the screen frame, tab bar, breadcrumb, priority line, status
bar, empty state, and stacked master-detail layout), the conventions (keys
and the scoped keymap, the overlay contract and modal stack, help, feedback),
`richtext` behind its feature, and the test kit behind its feature. The
[module map](docs/architecture.md#module-map) lists each with its page.

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

- [`docs/guide.md`](docs/guide.md): building a program on the framework,
  from the dependency line to the test kit. Start here.
- [`docs/architecture.md`](docs/architecture.md): layers, module map, the
  dependency rule, `M` and `Cx`, intent outcomes, and what the framework never
  does.
- [`docs/house-rules.md`](docs/house-rules.md): the eighteen design rules
  every control follows, why, and which pages carry each out.
- [`docs/controls/`](docs/controls/_template.md): one reference page per
  public module, from the template beside them.
- [`docs/contributing.md`](docs/contributing.md): independence rules and
  guards, the documentation lint and doc-sync test, how to add a control, how
  a change catalogs visible differences, and how to test.
- [`docs/roadmap.md`](docs/roadmap.md): controls and capabilities other
  frameworks ship that may come later, and what is deliberately left out.
- [`docs/decisions/README.md`](docs/decisions/README.md): the index of
  framework-scoped decisions and the policy for adding to it.

## Working on the crate

```text
cargo test -p tuikit
cargo test -p tuikit --features richtext
cargo test -p tuikit --test independence
cargo test -p tuikit --test doc_sync
RUSTDOCFLAGS='-D warnings' cargo doc -p tuikit --no-deps
```

The library denies undocumented public modules and items, so a missing page of
API documentation fails the build. Every `rust` block in the pages under
`docs/` runs as a doctest, and the doc-sync test fails when a public module
has no reference page or a page names a module that is gone.
