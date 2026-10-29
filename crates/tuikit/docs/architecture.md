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
| `glyphs` | The vetted glyph table with host registration; shared marks (pointer, back, disclosure, ok, fail, warning, bullet, ellipsis, no-value, rule, derived corner, separator, spinner) and the option boxes built from them (checkbox `[x]`/`[ ]`, one-of-N `(•)`/`( )`); cell-width and emoji-risk checks; the source scanner that finds unvetted glyphs. Page: [controls/glyphs.md](controls/glyphs.md) | landed |
| `text` | Cell-width measuring; end and middle truncation; padding; fitting spans; word, character, and separator wrapping; snippet fitting; ASCII case-folding find; the match painter. Page: [controls/text.md](controls/text.md) | landed |
| `scroll` | Wrap-or-clamp cursor stepping; the minimal-motion follow rule; the clamping scroll window a renderer returns to its owner; the fade-on-fit scrollbar gutter and the one scrollbar look; wrapped rows counted from the rows drawn. Page: [controls/scroll.md](controls/scroll.md) | landed |
| `chip` | The fixed-width filled chip and its one width rule (`badge_width` in cells, cut with the ellipsis mark, padding that carries the fill); toggle chips that keep their width; the segmented title; the `[ n/m ]` position chip; status chips in a tone, with a mark or a busy spinner frame. Page: [controls/chip.md](controls/chip.md) | landed |
| `age` | Ages from two Unix times; the unit ladder (days by default, weeks on request, or a host's units) and the one prose suffix; recency steps with injectable bounds and their theme tint; the fixed-width age cell. Page: [controls/age.md](controls/age.md) | landed |
| `frame` | The modal frame: named sizing policies from a share, a cap, a minimum, and a margin per direction; one cleared, accent-bordered frame with one title form and optional padding; bodies measured once at the width the policy gives. Planned in the same module: the pane frame with its focus cue, the split pane, rules. Page: [controls/frame.md](controls/frame.md) | landed (modal frame) |
| `selection` | The list cursor: the pointer marker in a 2-cell gutter; the full-row highlight from the theme's selection pair, active while the list has focus and inactive while it does not, with filled spans kept; the ghost and pending row looks; the wrap-or-clamp rule for stepping a cursor by the kind of list. Page: [controls/selection.md](controls/selection.md) | landed |

### Controls (layer 1)

| Family | Controls | Status |
| --- | --- | --- |
| Text entry | `LineEdit` and `TextField` (module `text_edit`): the one line editor, which wraps `tui-input` and binds the shared editing keys, and the one inline renderer, which windows a line in cells around its caret. Page: [controls/text_edit.md](controls/text_edit.md) | landed |
| Text entry | `TextPrompt<M>` and `PromptResult<M>` (module `text_prompt`): the one prompt for one line of text, an overlay for any context whose `Enter` asks the host's `on_confirm` to commit, refuse with a reason, or stay; a refusal shows one error line until the next edit, `Esc` and `Ctrl-C` close it silently, and its footer is generated on the compact frame's bottom border. Page: [controls/text_prompt.md](controls/text_prompt.md) | landed |
| Text entry | `PathField`, `PathSource`, `PathKind`, and `PathFieldView` (module `path_field`): the one path field, a line editor that asks for a directory or a file, completes with the host's known paths ahead of what a read-only source the host supplies lists, adds the best remainder on `Tab`, marks a path advisory when it is missing or the other kind, expands `~` on commit, and looks at its source only when its text changes, so drawing does no I/O; one hint rule. Page: [controls/path_field.md](controls/path_field.md) | landed |
| Text entry | `Form<M>`, `Field`, `FieldKind`, `Submit<M>`, and `FormBody` (module `form`): the one multi-field form, an overlay for any context over an ordered list of fields (text with a check, a path, a choice of chips with an optional custom value, a toggle, a check group, a one-of-N row drawn `(•)`, read-only notes, derived lines, and warnings) shown by conditions over its values; override rows (`OverrideRow`) that follow an inherited value until edited, go back on the reset key the form's own table binds, and end in a source column saying where their value stands; an intro paragraph and a band of detail lines that explain the focused field; one ring of stops, `Enter` submits from any field through the host's `on_submit`, one error line in the footer slot until the next edit, a footer generated from its key table that stays in view while the rows scroll, and one label grid; plus the row kit a host form draws its own rows with. Page: [controls/form.md](controls/form.md) | landed |
| Choice and command | `ActionMenu<M>`, `Entry<M>`, and `Confirm` (module `action_menu`): the one action menu, an overlay for any context over entries that are data (a label, a detail, a shortcut shown in a key column, whether it can run, an inline second-`Enter` confirmation that says what `Enter` will do, and the host's message); the shared cursor look, wrapping and stepping over entries that cannot run; keys and footer from the standard menu table; a fixed footer under entries that scroll in a window the menu keeps, with the shared scrollbar, in the menu-sized frame. Page: [controls/action_menu.md](controls/action_menu.md) | landed |
| Choice and command | `Dialog<M>`, `Kind<M>`, `Verb<M>`, and `Warning` (module `dialog`): the one dialog, an overlay for any context in three forms (a confirmation that commits the host's message, a choice whose named verbs on their own keys each commit one, and a notice that commits nothing), with a body, an optional warning list under a heading in the theme's `warning`, keys from the one standard dialog table, a footer generated from it (and the verbs), and a body that scrolls under that fixed footer on a short screen, in the menu-sized frame; a menu's entry opens one as a step of the menu. Page: [controls/dialog.md](controls/dialog.md) | landed |
| Choice and command | `Checklist<M>` and `Item` (module `checklist`): the one checklist, an overlay for any context over items that are data (a label, a detail, whether it is checked, and whether it can change), whose `Enter` hands the checked indices to the host's `on_confirm`; `Space` checks the cursor's item and `a` and `n` every item that can change; the shared cursor look, wrapping and stepping over items that cannot change, paging and ends keys that stop; the checked count on the bottom border; a fixed footer from the standard checklist table under items that scroll in a window the checklist keeps, with the shared scrollbar, in the compact frame. Page: [controls/checklist.md](controls/checklist.md) | landed |
| Choice and command | `Palette<R>` (filter picker, quick select, content search, and a command palette are instances); `SettingsPanel<M>` | planned |
| Reading | `Pager` with `Find`; `LogList`; `richtext` (a cargo feature) | planned |
| Reading | `HelpPage` (in the `help` convention module, below) | landed |
| Data display | `TreeView<R>` with `TreeState<Id>` and `ColumnSchema`; `PropertyGrid`; `Explorer` | planned |
| Feedback | `Toasts` (in the `feedback` convention module, below) | landed |
| Chrome | `AppFrame`; `TabBar`; `StatusBar`; `EmptyState`; the pane frame with its focus cue | planned |

### Conventions (layer 2)

| Module | Provides | Status |
| --- | --- | --- |
| `keys` | The key-table model: matchers with one rule for characters, rows that can include another table and carry availability, hints, resolving a key against a table, footers in one grammar (dimming what cannot act now), one key-label notation, the `KeyRole` classifier and the modal key convention checked by it, and checks for mislabeled, shadowed, and loosely bound rows; standard tables and action enums for menus, dialogs (one table for every form), pagers, text input, checklists, and forms; footers that show only some of a table's actions or take a control's own segments. Later: a scoped program-wide keymap. Page: [controls/keys.md](controls/keys.md) | landed |
| `overlay` | The `Overlay<M, Cx>` contract (`handle` and `render`, and whether an overlay fills the screen), `Outcome<M>` (consumed, close, dismiss, commit, stay), and `ModalStack<E>` over the host's entry type, which the host owns and passes its state to as `&Cx`, which routes a key to the top entry, opens an entry as a step of the one under it, closes the top for a close and its whole flow of steps for a dismissal or a commit, and draws every entry from bottom to top. Page: [controls/overlay.md](controls/overlay.md) | landed |
| `help` | `HelpPage`, the one help page: bold headings, keys in an accent column with descriptions wrapped beside them, legend rows for marks, a scrollbar, and a footer generated from the key table that scrolls and closes it, in the reader-sized modal frame; `sections_for`, which generates a page from a key table and a `HelpAction` description per action, and `undescribed`, which finds a key the page would leave out; `HelpScroll`, which pages by the rows shown and takes an offset past the end back at the next draw. Page: [controls/help.md](controls/help.md) | landed |
| `focus` | Declared focus rules: the pane focus cue, the modal focus lock, the Tab cycle | planned |
| `feedback` | `Severity` (info, success, warning, error: each with its theme color, chip tone, mark, and toast lifetime) and one `Feedback { severity, text, ttl }` type feeding three channels: the status line (`Feedback::span`), `Toasts` (bottom-centered boxes bordered in the severity's color, replaced or stacked, each gone after its lifetime, errors last longest), and `Log<T>` (bounded, newest-last, repeat-collapsing, counting unseen warnings and errors, over the host's payload `T`); `clipboard::copy`, which writes OSC 52 to a sink the host supplies. Page: [controls/feedback.md](controls/feedback.md) | landed |
| `testkit` | Behind the `testkit` feature. The key-script notation (`j<Tab><C-r>`) and press constructors; `FrameDriver`, the one-frame driver over the host's draw callback; buffer probes (text, token columns, the style under a piece of text, a modal's frame rectangle and the text inside it); the ANSI writer and `strip_ansi`; style-aware goldens that keep every color, checked against files and accepted through an environment variable; the conformance suite, `assert_conventions` over a host's `Registry` of modals and its `Rig`; the source-scan helpers (the crate's lexer, word-start matching, a read-only walk); and `MemoryPaths`, an in-memory file system for path fields that counts the questions asked of it. Page: [controls/testkit.md](controls/testkit.md) | landed |

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
// The overlay contract's outcome (`tuikit::overlay::Outcome`); a control
// that is not an overlay reports its own result type the same way.
match prompt.handle(key, &cx) {
    Outcome::Commit(message) => {
        host.close_top_overlay();
        host.dispatch(message);
    }
    Outcome::Stay(message) => host.dispatch(message),
    Outcome::Close => host.close_top_overlay(),
    Outcome::Consumed => {}
}
```

- A commit carries the host's `M`. What a commit does (rename something, start
  a task, save a setting) is the host's effect system, not the control's.
- Dismissal is an outcome too, so the host's stack can decide what to do
  next. Going back a level inside an overlay is the overlay's own business
  (it stays open), and an open overlay owns every key, so no outcome says
  "this key is not mine".
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

The `testkit` feature adds no dependency: it turns on the `testkit` module,
which a host's tests and dev tools use and its production build leaves out.
The crate depends on itself by path in its dev-dependencies so its own
integration tests and doctests have the module.

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
