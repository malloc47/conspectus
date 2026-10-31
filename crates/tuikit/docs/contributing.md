# Contributing

How to change the framework without breaking what makes it reusable. Read
[architecture.md](architecture.md) first: it explains the layers, the one-way
dependency rule, and the host parameters `M` and `Cx` that the rules below
protect.

Contents:

- [Independence guards](#independence-guards)
- [The raw-style guard](#the-raw-style-guard)
- [The glyph guard](#the-glyph-guard)
- [Repository-wide hygiene scans](#repository-wide-hygiene-scans)
- [The documentation lint](#the-documentation-lint)
- [Adding a control](#adding-a-control)
- [The house rules](#the-house-rules)
- [Cataloging visible changes](#cataloging-visible-changes)
- [Testing](#testing)
- [Checks before you finish](#checks-before-you-finish)

## Independence guards

The crate must be liftable into its own repository, so it may know nothing
about the program that hosts it. Three guard tests in
`tests/independence.rs` enforce that. Each guard is a pure function over
text, and the file's self-tests feed every guard a deliberately leaking
snippet from memory, so no leaking fixture ever sits on disk.

| Guard | What it checks |
| --- | --- |
| Manifest | Every `Cargo.toml` under the crate is parsed. A dependency on the host (by key or by a `package =` rename) or on any crate outside the runtime and development allowlists fails. Build dependencies are never allowed: a build script is arbitrary code. The one dependency not on a list is the crate itself, by path `.`, as a development dependency: that is how its own tests turn on its `testkit` feature, and it adds no package. |
| Vocabulary | No file in the crate (source, documentation, tests, manifest, README) uses a word from the `DOMAIN_TERMS` list, except the reviewed `VOCABULARY_EXCEPTIONS`. The guard file itself is the one file skipped, because it has to contain the words it tests for. |
| Process and write | Non-test Rust source starts no process and writes, renames, or deletes no file. Read-only filesystem access (`File::open`, `fs::read`, `fs::read_dir`, `fs::metadata`) is allowed. Hosts supply sinks (a clipboard `Write`) and sources (path completion) instead. |

The policy lists are constants at the top of the guard file:
`RUNTIME_DEPENDENCIES`, `DEV_DEPENDENCIES`, `DOMAIN_TERMS`,
`VOCABULARY_EXCEPTIONS`, `CAPABILITY_NEEDLES`, `CAPABILITY_EXCEPTIONS`, and
the file-count minimums described below. Documentation cannot quote the word
list without tripping the guard, so it points at the constant instead.

### How matching works

- **Vocabulary matching is case-insensitive and whole-word.** Text is split on
  anything that is not a letter or digit, and identifiers are split into
  words at `snake_case`, `kebab-case`, and `camelCase` boundaries. For a
  listed word `foo`, the forms `Foo`, `FOO`, `foo_id`, `FooId`, and
  `foo-bar` all hit, while `foobar` and `unfoo` do not. A plural hits like its
  singular (`foos`, and `-es` forms after `s`, `x`, `z`, `ch`, and `sh`).
- **A multi-word term hits when its words are consecutive**, however the text
  is wrapped or joined: for a listed term `foo bar`, so do `foo_bar`,
  `FooBar`, and a line break between the two words.
- **The process and write scan removes comments, string and character
  literals, and `#[cfg(test)]` items first**, so prose and test fixtures do
  not trip it. It then looks for the needles in `CAPABILITY_NEEDLES` at a word
  start (`remove_dir` also covers `remove_dir_all`), plus names imported
  through `use std::fs::{...}`. Files named `tests.rs` or `*_tests.rs`, and
  anything under a `tests` directory, are test code and are skipped. It
  assumes `rustfmt`-formatted source. It is a tripwire for accidental
  additions, not a sandbox: the compiler and review remain the defense
  against deliberate evasion.
- **The manifest guard parses TOML** rather than scanning lines, because
  dependency tables come in many spellings (`[dependencies.foo]`, dotted
  keys, target-specific tables, renames).

### Tripwires

Each scan reports how many files it visited, and a tripwire fails the run
when that is below a stated minimum (`MIN_MANIFESTS`,
`MIN_VOCABULARY_FILES`, `MIN_CAPABILITY_FILES`). A scan pointed at the wrong
directory therefore fails instead of passing empty. Choose a minimum about
three quarters of the current count, not equal to it, so deleting a file or
two does not trip it while a wrong directory still does. Raise the minimums
as the crate grows.

### Writing around the word list

Read `DOMAIN_TERMS` before writing names or prose. The guard is whole-word, so
an innocent sentence can trip it. Say what the thing is in generic terms:

- The host's name and its short name: write "the host" or "the host
  application". A task-tracker ID also trips the guard, because the ID starts
  with the host's short name; describe the behavior instead of citing the
  task.
- A row or footer that stays in place at the edge of a list: "fixed" or
  "sticky".
- A test rig: "driver", "checker", or "suite". The list contains the word the
  host uses for its agent integrations, and the guard flags that word in its
  test-rig sense too.
- Long document content: "document" or "content".
- One execution of the program: "run" or "context".
- The host's version-control and review vocabulary, multiplexers, and agent
  products: describe the generic idea the control handles (a list row, a
  label, a path) rather than where the data came from.

### Requesting an allowlist entry

The guards exist so a convenient host type does not creep in the first time a
control is awkward to make generic. Fix the design first: pass the host
meaning in through a type parameter, a closure, a trait, the named-style
registry, or a host-supplied sink or source. If that truly cannot work:

1. Add one entry to the relevant constant in `tests/independence.rs`
   (`RUNTIME_DEPENDENCIES`, `DEV_DEPENDENCIES`, `VOCABULARY_EXCEPTIONS`, or
   `CAPABILITY_EXCEPTIONS`) with a reason that names the file and says why the
   design cannot change. Exceptions are per file and per term; they never
   cover a whole directory.
2. A new dependency needs its decision record first (see
   [decisions/README.md](decisions/README.md)); cite it in the reason.
3. Say in the commit message and the change summary that an allowlist
   changed, so the reviewer sees it. An exception that no longer matches
   anything fails the scan, so the lists stay short and current.

## The raw-style guard

House rule 13 says every control styles from the theme its host passes in.
`tests/raw_styles.rs` enforces the part a scan can see: non-test Rust source
names no `Color::` value, no `Modifier::DIM`, and does not import ratatui's
`Stylize` shorthands, and adds no `Modifier::REVERSED` of its own. Read a
typed token (`theme.warning`), use `muted` for text that should recede, or
look up a named style the host registered (`theme.style(name)`). A cursor
row is highlighted by `selection::highlight` from the theme's selection
pair and a chip takes the `badge` token, so neither reverses anything by
hand (house rule 1). Other modifiers (`BOLD`, `ITALIC`, `UNDERLINED`) are
typographic and stay allowed.

The guard shares its walk, its Rust sanitizer (comments, literals, and
`#[cfg(test)]` items are blanked first), and its report format with the
independence guards through `tests/support/source_scan.rs`, and like them it
feeds itself leaking snippets from memory in its self-tests. The theme module
is the reviewed exception: its token table holds the default palette (the
selection and chip looks are reversed by default) and its grammar maps color
and modifier names to values. Two other files name the reversed modifier
without drawing a cursor: the scrollbar removes it so a highlight does not
show through the bar, and the selection highlight looks for it to leave a
chip's fill alone. The test kit's ANSI writer (`src/testkit/ansi.rs`) names
every color to write its escape code; it reads the styles of drawn cells and
draws nothing. An exception is per file and per needle,
lists its reason in the guard's `EXCEPTIONS`, and fails the scan once it
matches nothing. A control that seems to need a raw color needs a token or a
named style instead; add the token to the table in `src/theme.rs` and to the
theme's page.

A host holds its own drawing code to the same rule with a scan of its own,
since a raw color there ignores the theme just the same. That scan lives in
the host's tests and keeps its own exception list: the file where the host
writes the defaults of the named styles it registers.

## The glyph guard

A glyph the terminal's font lacks is drawn from a fallback font and spills
over its neighbor, so every character the framework draws is measured once
and recorded in the vetted table (`src/glyphs/vetted.rs`), and shared glyphs
are defined once as constants in `glyphs::marks`. `tests/glyph_literals.rs`
scans non-test Rust source under `src/` with the scanner the module exports
(`glyphs::drawn_glyphs`: non-ASCII characters in code, outside comments and
`#[cfg(test)]` items) and fails on:

- a non-ASCII character in code anywhere but the `glyphs` module, vetted or
  not. Draw a mark (`marks::POINTER`, `marks::ELLIPSIS`, ...), or add the
  glyph to the module so it has one definition;
- a character inside the `glyphs` module that the framework's table does not
  hold.

To add a glyph, measure it with `scripts/glyph-widths.py GLYPH...` (run from
the crate directory), add a constant to `src/glyphs/marks.rs` if controls
share it and list it in `marks::ALL`, and add the table entry with a note on
how it was checked. A host registers the glyphs of its own vocabulary with
`VettedGlyphs::register` from an adapter and runs the same scanner over its
trees, so the table it checks against is the union. The guard has no exception
list: a control that wants a glyph needs a mark or a table entry. The
control's page, [controls/glyphs.md](controls/glyphs.md), is the reference.

## Repository-wide hygiene scans

The host repository runs source-scanning tests over every Rust tree it
contains, this crate's included. They are repository policy, so they live in
the host's tests and not in this crate, and each one decides for itself which
trees it walks. The kinds that reach this crate:

| Scan | What it enforces here |
| --- | --- |
| Test-module size | An inline `mod tests` block stays under 100 lines. A larger one moves to a sibling file named `<module>_tests.rs`. |
| Comment hygiene | Comments explain behavior and rationale. They do not cite task-tracker IDs or narrate history, in `src`, `tests`, and `examples`. |
| Key-table usage | Overlay key handling resolves through key tables, and footers and status-line hints are generated from them. Control code does not match raw key codes by hand; the key-table model itself is the one place that names them. |
| Working directory | Code does not read the process's working directory, which can vanish while the program runs. A control that needs a directory, such as path completion, takes it from its host as an input. Setting a child's directory is not a read. No file in this crate is exempt. |
| Terminal ownership | Code does not enter raw mode or the alternate screen, or leave them. The host takes the terminal over and hands it back, with bracketed paste on only while it has it, and gives controls the frames to draw and the key and paste events to handle. No file in this crate is exempt. |
| Process and write capability | The host keeps a manifest of every program, socket, signal, and write path it may use. The framework has no entries, and the scan asserts there is no such path in this crate's tree. |
| Glyph vetting | Every non-ASCII character that code in either crate draws (outside comments and test items) is in the vetted table the host builds: the framework's glyphs plus the host's registrations. A character the table does not hold fails the scan, with the file and line. |
| User-visible text | String literals that controls draw contain no decision-record numbers, task IDs, or design-document paths, because that text reaches users through the host. |

Rules that guard what the host does or says (its rendered screens, its
reference documents for them) stay host-only.

Every scan states the fewest source files it may visit, so a scan pointed at
the wrong place fails instead of passing empty. When a change moves code into
this crate, raise this crate's minimum in the host's test support in the same
change, and lower the host's if the move took it below its own; the two
should not shrink in sum. The mechanisms behind these scans (the file walk,
the Rust lexer that blanks comments, literals, and test items, and the string
literal reader) are the test kit's `testkit::source`, which this crate's own
guards use too; the rule sets stay with whichever repository hosts the
code.

## The documentation lint

`src/lib.rs` carries three crate-level attributes:

```text
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]
```

- Every public module has module docs (`//!`) saying what it is for and where
  it sits in the layers. Every public item (type, field, variant, function,
  method, constant, trait) has a doc comment. A missing one fails an ordinary
  build, `cargo check`, and `cargo clippy`, not only `cargo doc`.
- Intra-doc links must resolve and must not point at private items.
  `RUSTDOCFLAGS='-D warnings' cargo doc -p tuikit --no-deps` turns every other
  rustdoc warning into an error, and the host's CI runs it.
- The requirement lives in the crate and not in the workspace lint table for
  two reasons. Cargo does not let a package take the workspace table and add a
  lint to it, and the host package does not require a comment on every item.
  An attribute in the crate also survives the directory lift, when the
  workspace table is gone.
- The crate-level documentation is `README.md`, included with `include_str!`,
  so the two cannot drift. Keep its links relative, and tag every code fence
  with a language (`toml`, `text`): an untagged fence compiles as a doctest.
- `tests/documentation.rs` fails if the attributes or the include are removed,
  or if a page this guide promises goes missing.

A module's `//!` docs say what the module is for; the page under
`docs/controls/` is the full reference for a control. Neither replaces the
other.

### The pages and the code stay in step

`tests/doc_sync.rs` reads `src/lib.rs`, the pages, and `src/doc_pages.rs`,
and fails when:

- a public module has no page under `docs/controls/` (a page names its
  module in the `Module` row of its header table), or a page names a module
  that is not public;
- a page lacks a row of the header table or one of the template's sections,
  in the template's order, or its usage example has no `rust` block;
- a module behind a cargo feature has a page that does not name the
  feature;
- a page under `docs/` is not included by `src/doc_pages.rs`, whose
  includes make every `rust` block a doctest, or an include names a page
  that is gone;
- the module map in [architecture.md](architecture.md) lacks a public
  module or its page, or lists a module that is not public;
- a relative link in the documentation names a file or a heading that does
  not exist;
- the decision log's index and its records disagree (a record without a
  row, a row without a record, or a different title, status, or set of
  origins), or a record lacks a section or an `## Origin` that names its
  host records or says it has none
  ([decisions/README.md](decisions/README.md#reading-a-record)).

While the crate lives in its host's repository the same test also holds the
log to the host's decision records: every host record a record names
exists, the host's index marks exactly those records framework-owned, and
every record of the host index's interface section is either restated or
listed as left out. The host counts as present when the workspace manifest
two directories up names this crate as a member; when the crate stands
alone, those checks print a note and pass.

## Adding a control

Work through this list in order. Each step names the rule it applies.

1. **Decide that it belongs here.** A control is generic UI behavior. If it
   needs a concrete host type, a host word, or a side effect, the host part
   is an adapter that stays in the host; the control keeps only what survives
   being generic over `M`, `Cx`, `Id`, and `R`.
2. **Place it.** One file per module under `src/`, declared in `lib.rs` with
   module docs. Inline test modules stay under 100 lines.
3. **Take the theme.** The constructor or render function takes `&Theme`. No
   raw `Color::` values and no raw `Modifier::DIM`; host roles are looked up
   in the named-style registry (house rule 13, enforced by
   [the raw-style guard](#the-raw-style-guard)). Draw shared glyphs through
   `glyphs::marks` and never type the character
   ([the glyph guard](#the-glyph-guard)).
4. **Write its key table.** Define the control's keys as a `keys` table over
   its own action enum, implement `KeyAction` for the enum, and generate its
   footer and help from that table. Never hand-write hints (house rule 8).
   Use the standard tables for menus, dialogs, pagers, text input,
   checklists, and forms where the control matches one, building on them
   with `include` rather than copying rows; bind characters with `ch`
   (never a chord of them), move every list with `Ctrl-P`/`Ctrl-N`, start
   with `Ctrl-C`, and follow the close-key, confirmation, and paging rules
   (house rules 3, 4, 5, 12). Run `mislabeled`, `unresolved`,
   `loose_characters`, and `ctrl_c_first` over the table in its tests; see
   [controls/keys.md](controls/keys.md).
5. **Return outcomes.** Handling a key returns an outcome carrying the host's
   `M` where something is committed. The control performs no effect. Anything
   external arrives through a trait, a closure, or a sink supplied by the host.
   A control that has something to tell the user (a refusal, a confirmation)
   says it through the host as a `feedback::Feedback` of the severity that
   fits (house rule 11; see [controls/feedback.md](controls/feedback.md)).
6. **Measure in cells.** Truncation, padding, and wrapping go through the
   `text` module and measure terminal cells (house rule 14). A control
   with a text field edits it with `text_edit::LineEdit` and draws it with
   `text_edit::TextField`, never with an editor or a window of its own
   (house rule 15); see [controls/text_edit.md](controls/text_edit.md). A control
   that needs its row count (a scroll range, a scrollbar, a height) wraps
   with `scroll::WrappedRows` (built on `text::wrap_line`) and draws those
   rows, rather than letting the paragraph widget wrap and estimating; see
   [controls/text.md](controls/text.md) and
   [controls/scroll.md](controls/scroll.md).
7. **Scroll through one window.** A scrolled control steps its cursor
   by its list's rule (`selection::Ends`, step 8), keeps
   a `scroll::ScrollWindow` that clamps, follows the cursor by the one
   rule, draws its scrollbar through `scroll::ScrollGutter`, and returns
   the window it drew to its owner as a value. It never sends a message
   from inside a draw to report what it measured.
8. **Show the cursor one way.** A list draws each row through
   `selection::row` (the pointer in a 2-cell gutter and the theme's
   selection pair, active or inactive by focus) and steps its cursor with
   `selection::Ends`: `Wrap` for a short choice list, `Clamp` for
   scrollable content (house rules 1, 2, and 10). It never types a cursor
   glyph or reverses a row by hand; see
   [controls/selection.md](controls/selection.md).
9. **Frame a modal.** A control that floats over the screen draws inside
   `frame::ModalFrame` with one of the named sizes (or a `ModalSize` built
   from the same parameters) and is its `frame::ModalBody`: it is measured
   once at the width the size gives and never computes a rect, a border,
   or a title of its own; see [controls/frame.md](controls/frame.md).
10. **Leave seams open, do not build them.** If the control could one day
    carry availability on key rows, report its rendered area for mouse hit
    testing, or accept another content source, keep the types ready for it
    and record it on the page. Do not build the feature.
11. **Document it.** Module docs and doc comments on every public item (the
    lint requires them). Copy [controls/_template.md](controls/_template.md)
    to `docs/controls/<module>.md`, fill in every section, and add the page
    to `src/doc_pages.rs` so its examples compile as doctests. Add the
    module's row, linking the page, to the module map in
    [architecture.md](architecture.md) in the same change. The doc-sync
    test (`tests/doc_sync.rs`) fails until all three are done.
12. **Test it.** Unit tests for behavior, rendered-buffer tests for layout, and
    a case in the test kit's conformance suite for anything that opens as a
    modal (see [Testing](#testing)).
13. **Move the host onto it in the same change, when the control comes from
    the host.** Unify on the best variant instead of copying the host's
    disagreements, and catalog every visible difference
    ([Cataloging visible changes](#cataloging-visible-changes)).
14. **Run the checks** at the end of this guide.

A decision that changes the crate's public API shape, house rules,
dependencies, or independence rules gets a record in
[decisions/](decisions/README.md).

## The house rules

The design language every control follows is in
[house-rules.md](house-rules.md): the eighteen rules, why each exists, and
the pages of the controls that carry it out. Each change that adds or alters
a control applies them, and each control page lists which ones it follows
and how. The rules every modal shares (the cursor look, the close keys,
generated footers, the theme's title, nothing cut at the frame's edge) are
the test kit's conformance checks; the rest are each control's own tests and
review.

## Cataloging visible changes

The framework is consistency-first. When a control moves into it, the control
takes the best variant of each behavior the host had, in the same change,
instead of copying the host's disagreements into the library. That makes some
visible changes deliberate, and every change that alters what a user sees or
does lists each difference in its summary (the commit body or the review
description) under two headings:

- **Regressions:** something users had that is now gone or worse. A shortcut
  that no longer works, a label that carried more information, a message that
  disappeared and was useful.
- **Consistency wins:** a difference that exists because the control now
  follows a house rule. Menus that wrap like every other short list, one
  selection look, footers generated from key tables.

For example:

```text
Regressions
- A held Down key in the action menu now loops at the end instead of stopping.

Consistency wins
- Menus wrap at the ends like pickers and checklists (rule 2).
- Cancelling a prompt no longer posts a message (rule 5).
```

The rules for the catalog:

- Name every visible change, with its house rule number where one applies. If
  there are none, say so explicitly ("Regressions: none. Consistency wins:
  none.").
- Verify with rendered output, not by reading code: the test kit's frame
  driver (`testkit::render::FrameDriver`) and its style-aware goldens, or a
  host's own way of printing a frame. Check a normal width and a narrow
  width, and the focused and unfocused states.
- Update the tests and snapshots that encode the old behavior deliberately,
  and say so in the summary. A test that fails for any other reason is a bug
  in the change.
- A change users would be surprised by (a key, a label, a default) also goes
  in the host's user-facing documentation and changelog.

## Testing

A control's inputs are a key, a context, and a theme, and its output is an
outcome plus a rendered buffer. Test it through exactly those.

- **Behavior:** unit tests beside the code drive keys and assert outcomes. A
  test module of 100 lines or more lives in a sibling `<module>_tests.rs`
  file.
- **Layout and styling:** render into a buffer and compare it, at a normal and
  a narrow width, empty and overfull, with wide characters. A frame whose
  colors matter is a style-aware golden (`testkit::golden`); `insta` is on
  the development allowlist for snapshots of other values.
- **Documentation:** every `rust` block in the pages under `docs/` is a
  doctest, so an example that stops compiling or asserting fails
  `cargo test -p tuikit --doc`.
- **No host:** tests in this crate import nothing from the host and use no
  host fixtures. A control that cannot be tested without them is not generic
  yet.

### With the test kit

The test kit (module `testkit`, behind the `testkit` feature, which the
crate's own tests always have) packages the repeated parts. Its page,
[controls/testkit.md](controls/testkit.md), is the reference.

- `testkit::keys`: write input as a key script (`j<Tab><C-r>`) or build one
  press (`press`, `ctrl`, `chord`); never write a `KeyEvent` literal or a
  per-file key helper.
- `testkit::render` and `testkit::buffer`: draw a frame off screen and read
  it (the text of a row, token columns, the style under a piece of text, a
  modal's frame).
- `testkit::golden`: a golden that keeps every color and modifier, so a
  test fails when only a color changes. Check a frame with
  `assert_golden`, passing `std::fs::write` as the writer, and accept a
  deliberate change by running the test with `TUIKIT_ACCEPT_GOLDENS=1`.
- `testkit::conformance`: `assert_conventions` over a registry of modals
  holds each to the conventions (drawn, contained, one title form, a
  selection that moves no text, no word cut at the edge, keys by role,
  generated footers, the close keys). A control that has an overlay form
  registers it, in the change that adds it, with the suite in this crate's
  tests and in each host's.
- `testkit::source`: the lexer and walk the source guards use.

## Checks before you finish

While iterating, run the narrowest relevant test, for example
`cargo test -p tuikit --test independence` or `cargo test -p tuikit --test
doc_sync`. Before finishing, build the crate alone, with its features off and
then with `richtext`, so nothing outside `richtext` comes to need the feature
and no item only that module uses is left without its
`#[cfg(feature = "richtext")]` (it warns as unused when the feature is off).
`cargo test` without a target filter runs the doctests too, the pages'
examples among them:

```text
cargo fmt -p tuikit -- --check
cargo clippy -p tuikit --no-default-features -- -D warnings
cargo clippy -p tuikit --no-default-features --all-targets -- -D warnings
cargo clippy -p tuikit --no-default-features --features richtext -- -D warnings
cargo clippy -p tuikit --no-default-features --features richtext --all-targets -- -D warnings
cargo test -p tuikit --no-default-features
cargo test -p tuikit --no-default-features --features richtext
RUSTDOCFLAGS='-D warnings' cargo doc -p tuikit --no-deps --features richtext
git diff --check
```

`-p tuikit` keeps a host's features out of the build: a build of the whole
repository turns on whatever features the host's dependency on the crate
names, so it cannot see the feature-off build. The lib-only lints also build
without `testkit`, which this crate's own tests always turn on. A repository
that hosts the crate runs its own checks over every member as well.

If a guard fails, fix the design. Add an allowlist entry only through the
process above.
