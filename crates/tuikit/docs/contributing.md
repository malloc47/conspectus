# Contributing

How to change the framework without breaking what makes it reusable. Read
[architecture.md](architecture.md) first: it explains the layers, the one-way
dependency rule, and the host parameters `M` and `Cx` that the rules below
protect.

Contents:

- [Independence guards](#independence-guards)
- [The raw-style guard](#the-raw-style-guard)
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
| Manifest | Every `Cargo.toml` under the crate is parsed. A dependency on the host (by key or by a `package =` rename) or on any crate outside the runtime and development allowlists fails. Build dependencies are never allowed: a build script is arbitrary code. |
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
`Stylize` shorthands. Read a typed token (`theme.warning`), use `muted` for
text that should recede, or look up a named style the host registered
(`theme.style(name)`). Other modifiers (`BOLD`, `ITALIC`, `REVERSED`) are
typographic and stay allowed.

The guard shares its walk, its Rust sanitizer (comments, literals, and
`#[cfg(test)]` items are blanked first), and its report format with the
independence guards through `tests/support/source_scan.rs`, and like them it
feeds itself leaking snippets from memory in its self-tests. The theme module
is the reviewed exception: its token table holds the default palette and its
grammar maps color names to colors. An exception is per file and per needle,
lists its reason in the guard's `EXCEPTIONS`, and fails the scan once it
matches nothing. A control that seems to need a raw color needs a token or a
named style instead; add the token to the table in `src/theme.rs` and to the
theme's page.

## Repository-wide hygiene scans

The host repository runs source-scanning tests over every Rust tree it
contains, this crate's included. They are repository policy, so they live in
the host's tests and not in this crate, and each one decides for itself which
trees it walks. The kinds that reach this crate:

| Scan | What it enforces here |
| --- | --- |
| Test-module size | An inline `mod tests` block stays under 100 lines. A larger one moves to a sibling file named `<module>_tests.rs`. |
| Comment hygiene | Comments explain behavior and rationale. They do not cite task-tracker IDs or narrate history, in `src`, `tests`, and `examples`. |
| Key-table usage | Overlay key handling resolves through key tables, and footers are generated from them. Control code does not match raw key codes by hand; the key-table model itself is the one place that names them. |
| Working directory | Code does not read the process's working directory, which can vanish while the program runs. A control that needs a directory, such as path completion, takes it from its host as an input. Setting a child's directory is not a read. No file in this crate is exempt. |
| Terminal ownership | Code does not enter raw mode or the alternate screen, or leave them. The host takes the terminal over and hands it back, with bracketed paste on only while it has it, and gives controls the frames to draw and the key and paste events to handle. No file in this crate is exempt. |
| Process and write capability | The host keeps a manifest of every program, socket, signal, and write path it may use. The framework has no entries, and the scan asserts there is no such path in this crate's tree. |
| User-visible text | String literals that controls draw contain no decision-record numbers, task IDs, or design-document paths, because that text reaches users through the host. |

Rules that guard what the host does or says (its rendered screens, its
reference documents for them) stay host-only.

Every scan states the fewest source files it may visit, so a scan pointed at
the wrong place fails instead of passing empty. When a change moves code into
this crate, raise this crate's minimum in the host's test support in the same
change, and lower the host's if the move took it below its own; the two
should not shrink in sum. The mechanisms behind these scans are meant to
become helpers in the test kit; the rule sets stay with whichever repository
hosts the code.

## The documentation lint

`src/lib.rs` carries three crate-level attributes:

```rust
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
   [the raw-style guard](#the-raw-style-guard)).
4. **Write its key table.** Define the control's keys as a key table, and
   generate its footer and help from that table. Never hand-write hints
   (house rule 8). Use the standard tables for menus, dialogs, pagers, text
   input, checklists, and forms where the control matches one, and follow the
   close-key, confirmation, and paging rules (house rules 3, 4, 5, 12).
5. **Return outcomes.** Handling a key returns an outcome carrying the host's
   `M` where something is committed. The control performs no effect. Anything
   external arrives through a trait, a closure, or a sink supplied by the host.
6. **Measure in cells.** Truncation, padding, and wrapping go through the
   `text` module and measure terminal cells (house rule 14).
7. **Leave seams open, do not build them.** If the control could one day
   carry availability on key rows, report its rendered area for mouse hit
   testing, or accept another content source, keep the types ready for it and
   record it on the page. Do not build the feature.
8. **Document it.** Module docs and doc comments on every public item (the
   lint requires them). Copy [controls/_template.md](controls/_template.md)
   to `docs/controls/<control>.md` and fill in every section. Flip the
   control's row in the module map in [architecture.md](architecture.md) to
   landed in the same change.
9. **Test it.** Unit tests for behavior, rendered-buffer tests for layout, and
   the test kit's conformance suite once it exists (see [Testing](#testing)).
10. **Move the host onto it in the same change, when the control comes from
    the host.** Unify on the best variant instead of copying the host's
    disagreements, and catalog every visible difference
    ([Cataloging visible changes](#cataloging-visible-changes)).
11. **Run the checks** at the end of this guide.

A decision that changes the crate's public API shape, house rules,
dependencies, or independence rules gets a record in
[decisions/](decisions/README.md).

## The house rules

The design language every control follows. Each change that extracts or
alters a control applies these, and each control page lists which ones it
follows and how. The rules the test kit can check mechanically become its
conformance checks; until then review enforces them.

| # | Topic | Rule |
| --- | --- | --- |
| 1 | List cursor and highlight | Every list highlights the full selected row from the theme's `selection_active` (`selection_inactive` when the list is unfocused) and draws the `marks::POINTER` marker in a 2-cell gutter. The marker keeps the selection visible without color. |
| 2 | List ends | Short choice lists (menus, pickers, checklists, palettes) wrap at the ends. Scrollable content (trees, property lists, pagers, logs) clamps. |
| 3 | Confirmation | An inline arm (`Enter` again) on a menu entry when there is nothing more to say; a `Dialog` (`y`/`Enter` confirms, `n`/`Esc` cancels) when a body helps. A reversible action needs neither. |
| 4 | Close keys | `Esc` steps back one level. `Ctrl-C` and `q` dismiss the overlay (`q` only where nothing is typed). |
| 5 | Close feedback | Cancelling is silent: it is not an event worth a message. |
| 6 | Forms | `Enter` submits from any row. `Tab`/`Down` move to the next row and `Shift-Tab`/`Up` to the previous over one ring. |
| 7 | Validation errors | One error line in the modal's footer slot, set by a failed confirm and cleared by the next edit. No input is rejected silently. |
| 8 | Footers and hints | Every modal footer and the status-bar hints are generated from key tables, in one grammar: a short imperative verb with no "to". |
| 9 | Focus cue | The focused pane's border and title marker take `panel_focus_accent`. |
| 10 | Unfocused cursor | Every list shows the active or inactive selection pair, so an unfocused list still shows where its cursor is. |
| 11 | Feedback severity | `Feedback` has info, success, warning, and error. Each channel (status line, toast, message log) styles from the theme, and errors stay longer. |
| 12 | Paging | Viewport-sized `PgUp`/`PgDn`; `Home`/`End`/`g`/`G` on every list taller than a screen; half-page `Ctrl-D`/`Ctrl-U` in pagers; scroll offsets clamped. |
| 13 | Theme | Every control takes `&Theme` and uses no raw `Color::` value. Muted text uses the `muted` and `inert` tokens instead of an ad hoc dim style. |
| 14 | Width | All truncation and padding measure terminal cells, not `char`s. `…` marks cut ends; paths and identifiers use a middle ellipsis. |
| 15 | Text editing | Editing is correct for cell width, bracketed paste is on (a multi-line paste cannot submit), and word motion works. |
| 16 | Path fields | A path field has a kind (directory or file), expands `~` on commit, and completes through a source behind a cache. |
| 17 | Labels | One vocabulary: the shorter form. A field carries the same short label in every control (`name`, not a qualified `<kind> name`). |
| 18 | Pagers | `Esc` steps back (help, then find, then leave) and `q` leaves. Confirming a find scrolls to the match. A `[ n/m ]` position chip in screen mode, a scrollbar in modal and embedded modes. |

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
  driver once it exists, and until then a ratatui `TestBackend` buffer or the
  host's own snapshot facility. Check a normal width and a narrow width, and
  the focused and unfocused states.
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
  a narrow width, empty and overfull, with wide characters. Snapshot tests may
  use `insta`, which is on the development allowlist.
- **No host:** tests in this crate import nothing from the host and use no
  host fixtures. A control that cannot be tested without them is not generic
  yet.

### With the test kit

The test kit (module `testkit`; see the module map for its status) packages
the repeated parts:

- buffer inspection helpers (the text of a row, the style of a cell);
- a key DSL that writes input as a short text string and replays it through a
  control;
- an ANSI-preserving one-frame driver that renders a frame to a string with
  its styling kept, for snapshots of layout and color;
- a convention conformance suite: `assert_conventions` takes a registry of
  overlays and checks them against the house rules (close keys, generated
  footers, selection look, wrap or clamp, paging), so a control that
  registers is held to the rules mechanically instead of by review;
- source-scan and documentation-table helpers that a host's hygiene tests
  build on.

A control that has an overlay form registers it with the conformance suite as
part of the change that adds it. Until the kit lands, use a ratatui
`TestBackend` buffer for the same checks, and expect to port them to the kit.

## Checks before you finish

Run from the workspace root. While iterating, run the narrowest relevant
test, for example `cargo test -p tuikit --test independence`.

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
git diff --check
```

If a guard fails, fix the design. Add an allowlist entry only through the
process above.
