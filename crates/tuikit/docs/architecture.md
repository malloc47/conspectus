# Architecture

This guide explains how the framework is layered, where each module sits, and
the rules that keep it independent from the application that hosts it. Read it
before adding a control; [contributing.md](contributing.md) turns these rules
into a checklist.

## Layers

```text
host application
└─ adapters       row builders · theme registrations · message mapping · domain copy
        │  depends on (one way)
        ▼
tuikit
├─ foundations    theme · glyphs · text · scroll · chip · age · frame · selection
├─ controls       text entry · choice and command · reading · data display · chrome · feedback
├─ conventions    keys · overlay · help · focus · feedback · testkit
└─ (feature) richtext    markdown · tables · syntax
        │  depends on
        ▼
ratatui · crossterm · unicode-width · tui-input   (+ optional rich-text crates)
```

The framework covers two of three layers.

- **Layer 1, components and design tokens.** Foundations (the theme, glyphs,
  text and scroll helpers, chips, ages, frames, selection looks) and the
  controls built on them.
- **Layer 2, interaction conventions.** Key tables that generate footers,
  hints, and help; the overlay contract and a host-owned modal stack; focus
  rules; a feedback model; and a test kit that checks controls against the
  house rules.
- **Layer 3, the application runtime**, is not in this crate: the event loop,
  the effect lifecycle, undo, and suspending the terminal to hand it to a
  child process belong to the host. A host runs its own loop and calls into
  the controls; the framework never calls out to run anything.

## Module map

The map lists the target set. A row's status is **planned** until the module
lands and **landed** after; the change that lands a module flips its row and
adds its page under [controls/](controls/_template.md) in the same change.
Foundations and conventions are modules; controls are types, and the page for
each names its module path.

### Foundations (layer 1)

| Module | Provides | Status |
| --- | --- | --- |
| `theme` | Generic role tokens (accent and focus cue, muted, inert, selection, success, warning, error, info, link, match highlight, badge, recency scale, surface, input fields); a registry of named styles for roles only the host knows; alias keys; the style-spec grammar; applying overrides from a config table the host supplies. Page: [controls/theme.md](controls/theme.md) | landed |
| `glyphs` | The vetted glyph table with host registration; shared marks (pointer, back, disclosure, ok, fail, warning, bullet, ellipsis, no-value, rule, separator, spinner); cell-width and emoji-risk checks; the source scanner that finds unvetted glyphs. Page: [controls/glyphs.md](controls/glyphs.md) | landed |
| `text` | Cell-width measuring; end and middle truncation; padding; fitting spans; word, character, and separator wrapping; snippet fitting; ASCII case-folding find; the match painter. Page: [controls/text.md](controls/text.md) | landed |
| `scroll` | Wrap-or-clamp cursor stepping; the minimal-motion follow rule; the clamping scroll window a renderer returns to its owner; the fade-on-fit scrollbar gutter and the one scrollbar look; wrapped rows counted from the rows drawn. Page: [controls/scroll.md](controls/scroll.md) | landed |
| `chip` | The fixed-width filled chip and its one width rule (`badge_width` in cells, cut with the ellipsis mark, padding that carries the fill); toggle chips that keep their width; the segmented title; the `[ n/m ]` position chip; status chips in a tone, with a mark or a busy spinner frame. Page: [controls/chip.md](controls/chip.md) | landed |
| `age` | Ages from two Unix times; the unit ladder (days by default, weeks on request, or a host's units) and the one prose suffix; recency steps with injectable bounds and their theme tint; the fixed-width age cell. Page: [controls/age.md](controls/age.md) | landed |
| `frame` | The modal frame: named sizing policies from a share, a cap, a minimum, and a margin per direction; one cleared, accent-bordered frame with one title form and optional padding; bodies measured once at the width the policy gives. Planned in the same module: the pane frame with its focus cue, the split pane, rules. Page: [controls/frame.md](controls/frame.md) | landed (modal frame) |
| `selection` | The list cursor marker and the selected, active, inactive, ghost, and pending row styles | planned |

### Controls (layer 1)

| Family | Controls | Status |
| --- | --- | --- |
| Text entry | `LineEdit` and `TextField`; `TextPrompt<M>`; `PathField`; `Form<M>` | planned |
| Choice and command | `ActionMenu<M>`; `Dialog<M>`; `Checklist`; `Palette<R>` (filter picker, quick select, content search, and a command palette are instances); `SettingsPanel<M>` | planned |
| Reading | `Pager` with `Find`; `HelpPage`; `LogList`; `richtext` (a cargo feature) | planned |
| Data display | `TreeView<R>` with `TreeState<Id>` and `ColumnSchema`; `PropertyGrid`; `Explorer` | planned |
| Feedback | `Toasts` | planned |
| Chrome | `AppFrame`; `TabBar`; `StatusBar`; `EmptyState`; the pane frame with its focus cue | planned |

### Conventions (layer 2)

| Module | Provides | Status |
| --- | --- | --- |
| `keys` | The key-table model: matchers, rows, hints, resolving a key against a table, footer generation, key labels, and a check for mislabeled rows; standard tables for menus, dialogs, pagers, text input, checklists, and forms; later, a scoped global keymap | planned |
| `overlay` | The `Overlay<M, Cx>` contract (`handle` and `render`), `Outcome<M>`, and `ModalStack`, which the host owns and which draws every entry from bottom to top | planned |
| `help` | The generator that turns key tables into help pages, and the page renderer | planned |
| `focus` | Declared focus rules: the pane focus cue, the modal focus lock, the Tab cycle | planned |
| `feedback` | One `Feedback { severity, text, ttl }` type feeding three channels (status line, toast, message log); a bounded, repeat-collapsing log; the clipboard sink | planned |
| `testkit` | Buffer inspection; a key DSL; an ANSI-preserving one-frame driver; a convention conformance suite (`assert_conventions` over a registry of overlays); source-scan helpers; documentation-table sync | planned |

## The one-way dependency rule

Dependencies run in one direction only: the host depends on `tuikit`, and
`tuikit` depends on third-party crates and on nothing from the host.

- The manifest names no host crate, and its dependency set is an allowlist.
  A new third-party crate needs a decision record first (see
  [decisions/README.md](decisions/README.md)).
- Source, documentation, tests, and manifest use none of the host's domain
  vocabulary. The compiler enforces the type side (a control that wants the
  host's message type must be generic over it), and a guard test enforces the
  vocabulary side.
- Host meaning enters the framework in exactly five ways: a type parameter
  (`M`, `Cx`, `Id`, `R`), a closure, a trait the host implements, an entry in
  the named-style registry, or a sink or source the host supplies.

The guards, and what to do when one fails, are in
[contributing.md](contributing.md#independence-guards).

## The host message type `M` and the context type `Cx`

A control is generic over what the host needs to say and know, never over a
concrete host type.

- **`M` is the host's message type.** A control that commits something (a
  confirmed prompt, a submitted form, a chosen menu entry) builds an `M` with
  a closure or a mapping the host supplies, and reports it in its outcome. The
  control never names, matches, or inspects a concrete message type.
- **`Cx` is the host's context type.** When a control needs host data to
  handle a key or to render (the names a path field may complete, the labels a
  list shows), the host passes a borrowed `&Cx` into `handle` and `render`.
  The control holds no copy of host state and reads no global state.
- **`Id` and `R` follow the same idea.** `Id` is the host's stable identity for
  a row, so a cursor can survive a rebuild of the rows. `R` is the host's row
  or result type, which the control stores and hands back to host closures
  without looking inside.
- **`&Theme` is the third input.** Every control takes the theme explicitly.
  Styles for roles only the host knows are registered by name, and a control
  looks them up by name.

A control takes only the parameters it needs. Its page lists them, and
anything it cannot be made generic over is a sign the control is still
carrying host meaning that belongs in an adapter.

## Intent outcomes instead of effects

A control never acts on the world. Handling a key returns an **outcome**: a
value that says what the user meant, which the host interprets in its own
runtime.

```rust
// Illustrative shape only; each control's page gives the real outcomes.
match prompt.handle(key, &cx) {
    Outcome::Commit(message) => host.dispatch(message),
    Outcome::Close => host.close_top_overlay(),
    Outcome::Consumed | Outcome::Ignored => {}
}
```

- A commit carries the host's `M`. What a commit does (rename something, start
  a task, save a setting) is the host's effect system, not the control's.
- Dismissal, going back a level, and "this key is not mine" are outcomes too,
  so the host's stack can decide what to do next.
- Where a control needs an external capability, the host supplies it: the
  clipboard takes a `Write` sink, and path completion takes a source trait.
  The framework starts no process and writes no file itself.

This keeps controls testable with nothing but a key and a buffer, and it
keeps the effect lifecycle (queuing, undo, retries, hand-off to child
processes) in one place, the host runtime.

## What the framework never does

- Import anything from its host, or name the host's domain words.
- Start a process, open a socket, install a signal handler, or write, rename,
  or delete a file. Read-only filesystem access is allowed where a control
  needs it (completion, for example), behind a source the host can replace.
- Read global state. A control gets what it needs through its parameters, so
  two instances never share hidden state.
- Own the event loop, terminal setup and teardown, the effect lifecycle, or
  undo.
- Use raw colors. Every control takes `&Theme`.
- Silently reject input. A rejected confirm sets an error line (house rule 7).
- Promise API stability to outside users: the crate is for internal reuse.

## Features and dependencies

| Dependency | Why | Reach |
| --- | --- | --- |
| `ratatui` | Rendering library every control draws with | always |
| `crossterm` | Key and mouse event types the interaction layer maps | always |
| `unicode-width` | Display-width arithmetic for truncation, padding, wrapping | always |
| `tui-input` | The single-line edit engine behind the text-entry controls | always |
| `ansi-to-tui`, `tui-markdown`, `comfy-table` | ANSI text, Markdown with syntax highlighting, and tables as styled lines | `richtext` feature only |

The complete allowlist, with the reason for each entry, is in the independence
guard (`tests/independence.rs`); a dependency that is not listed there fails
the guard.

## Layout of the crate

```text
crates/tuikit/
├─ Cargo.toml
├─ README.md             crate summary; also the crate-level API documentation
├─ src/                  the library (documentation is mandatory)
├─ scripts/              glyph-widths.py, which measures glyphs in common fonts
├─ tests/                independence guards, documentation checks, integration tests
├─ examples/             previews of controls, once the first one lands
└─ docs/
   ├─ architecture.md    this guide
   ├─ contributing.md    independence rules, adding a control, house rules
   ├─ controls/          one page per control, from _template.md
   └─ decisions/         framework-scoped decision records
```
