# ADR 0164: The Test Kit Drives Frames, Keeps Colors In Goldens, And Holds Every Modal To The Conventions

## Status

Accepted. Extends [ADR 0150](0150-tui-framework-workspace-crate.md) with
the framework's `testkit` module, the last layer-2 module of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md). Amends
[ADR 0067](0067-tui-snapshot-mode-for-agent-iteration.md): the key-script
notation, the one-frame driver, and the ANSI writer of snapshot mode move
into the framework, and every key an open overlay takes reaches the
reducer in snapshot mode, its outside effects skipped with a note.
Carries out ADR 0150's answer that the hygiene tests' mechanisms become
helpers of the test kit while their rule sets stay with the repository.
Amended by [ADR 0176](0176-tui-settings-panels-apply-as-they-change-and-editors-are-steps.md):
a `Case` can be a `step`, whose `Contained` check accepts a change of the
selection look of the entry under it outside its frame.

## Context

The test and preview machinery of the TUI is reusable and was partly
duplicated. The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/06-interaction-conventions.md`,
section 6; `04-menus-pickers-dialogs.md`, section 5) found:

- **Twenty-six test files defined their own key helper** (`key(code)`,
  `press(code)`, `plain`, `ctrl`, `char_key`, `key_event`), several of them
  verbose `KeyEvent { code, modifiers, kind, state }` literals, and the
  preview example repeated the literal five times.
- **Four copies of `strip_ansi`**: one in the tmux discovery, which the
  content search shares, and three in integration tests that read
  `--snapshot` output, each a little different.
- **Two Rust scanners for string literals and a third copy of the lexer.**
  The text-leak scan, the overlay-keymap hygiene scan (a naive one that
  read strings inside comments), and the framework's source guards each
  stripped comments, literals, and test code their own way, beside the
  glyph scanner's lexer in the framework.
- **The convention checks were written against the App.** The layout
  suite's three geometric checks (nothing escapes the frame at five widths
  by two heights, moving the selection moves no text, no word is cut at
  the edge), the frame and title check, the key-convention check by role,
  and the registry's close checks are true of any modal of any program,
  but each lived in Conspectus's consistency suite over its own list of
  modals, and the list of modals with their key tables was kept by hand in
  four places.
- **Goldens dropped styles.** The 37 `insta` snapshots store characters
  only, so a control that starts drawing its selection in the wrong color
  passes; the ANSI path was covered only by the README screenshot test.
- **Snapshot mode could not drive picks and submits.** After the modal
  stack (ADR 0161) every key reached every modal, so the cursor of every
  menu, form, and prompt moved, but the snapshot driver dropped what ten
  of the sixteen modals sent when they picked or submitted (`snapshot
  mode skipped a pick or submit`), so a frame of what a pick does could
  not be captured. The ANSI writer left out the blink, hidden, and
  crossed-out modifiers the theme grammar accepts.

## Decision

1. **`tuikit::testkit`, behind a `testkit` cargo feature.** The feature is
   off by default, so a production build carries none of it; the host
   turns it on from its `[dev-dependencies]` (tests and the preview
   example) and from its own dev-only `snapshot` feature, and the
   framework depends on itself by path in its own dev-dependencies so its
   integration tests and doctests have it. The independence guard accepts
   that one self-dependency (the crate's own name, by path `.`, as a
   dev-dependency) and nothing else new. The kit adds no dependency.

2. **Key scripts.** `testkit::keys` owns the notation ADR 0067 defined for
   `--snapshot-keys` (literals with an implicit Shift on uppercase letters,
   `<Name>` keys, nested `C-`, `A-`, `S-` prefixes, one bracketed character
   as itself) with typed errors whose messages are the old ones, and the
   press constructors tests use (`press`, `chord`, `typed`, `typed_key`,
   `ctrl`, `alt`, `release`). The host keeps what a key means.

3. **One-frame driver.** `testkit::render::FrameDriver` draws a frame off
   screen through the host's draw callback, and `drive` presses a run of
   keys with a frame drawn before each, as a live loop does; `render` and
   `render_widget` are the one-call forms. A frame is what the off-screen
   terminal received, and a check draws its frame on a fresh driver.

4. **Buffer probes.** `testkit::buffer` reads a frame: its text, the token
   columns of a row, where a piece of text starts and the style of that
   cell, a count of matching cells, the rectangle of a modal's frame from
   its corners among the cells that differ from the same screen without
   it, the text inside a frame, and the rows a moving selection repainted.

5. **ANSI text and one `strip_ansi`.** `testkit::ansi::to_ansi` is the
   snapshot writer, byte-for-byte, plus the four modifiers it lacked.
   `tuikit::text::strip_ansi` is the one stripper (control sequences,
   operating-system commands, two-character escapes), in `text` because
   production code (captured pane text) needs it; the kit re-exports it.
   The writer names every color to encode it, so the raw-style guard lists
   `src/testkit/ansi.rs` as a reviewed exception for `Color::`; modifiers
   are encoded by name, so it needs no other.

6. **Style-aware goldens.** `testkit::golden::styled` writes a frame as a
   header, a text row and a style row per row (one key per cell, a space
   for the default style), and a legend of each key's colors and
   modifiers. Keys follow first appearance, so a recoloring changes one
   legend line and a color-only change fails a comparison that a text
   golden passes. `assert_golden` checks a frame against a file; with
   `TUIKIT_ACCEPT_GOLDENS=1` it hands the new text to a writer the test
   passes in, since the framework writes no files. An underline color is
   not kept.

7. **The conformance suite.** `testkit::conformance` has a `Registry` of
   `Case`s over a host-supplied `Rig` (draw the screen, close the top
   modal, press a key as the loop does, count the open modals, read the
   feedback shown, put a hint on the status line, the theme). A case opens
   one modal in a fresh host state and says whether it fills the screen,
   whether its first level types, how its selection is checked, its key
   tables per level with their convention rows, and the table its footer
   comes from. `assert_conventions` runs eight checks: drawn; contained
   (a frame at every size, nothing outside it, no text in its sides);
   title (accent corners, one space each side, regular weight outside
   badge-styled chips); selection (the frame stays, no text moves above
   the last repainted row outside the cursor gutter); words (no row ends in
   the start of a word the wide screen shows whole); keys (the convention
   by role, `Ctrl-C` first to dismissing, and the table checks once per
   table); footer (every generated segment is drawn); close (`Ctrl-C` and
   `Esc` close, `q` too unless the level types, silently). Failures are
   values (`check_conventions`), and `assert_conventions_of` runs some
   checks so a host can give each its own test. The framework's own tests
   run the suite over a well-behaved menu and prompt and a deliberately
   broken modal that fails seven of the eight checks.

8. **Conspectus's registry.** `consistency_tests/conventions.rs` registers
   every modal once (the layout builders and the two viewers) with its key
   tables and footer, and fails when a registered modal or a row of
   `MODAL_CONVENTION` is not in it. The lifted tests leave `layout.rs`,
   `keys.rs`, and `registry.rs`, which keep what is Conspectus's own (the
   search modal's sizing and title, a prompt over a modal, routing, the
   levels inside a modal). The help pages' frame parity uses the kit's
   `same_chrome`.

9. **Source-scan helpers.** `testkit::source` exposes the crate's one
   lexer (now `lex`, shared with the glyph scanner): `code`,
   `without_comments`, `strip_test_items`, `production_code`,
   `string_literals` with escapes resolved, `contains_at_word_start`, and a
   read-only `files`/`rust_files` walk. The framework's guards, the host's
   tree helper, the text-leak and mark scans, the list-cursor scan, and the
   overlay-keymap hygiene scan use it; each keeps its own rules.

10. **Snapshot mode drives every overlay key.** What a modal sends goes
    through the reducer, as in every loop. The effects that would reach
    outside the snapshot (a copy, an attach, a resume, a launch, a
    rebuild, a mux command, a store write) are skipped with a stderr note
    naming them; a viewer opens from the fixture world as before, and the
    rest run on the pure executor. A consistency test presses every key
    every registered modal binds through the snapshot driver.

## Consequences

- **No visible change in the product.** The representative snapshot
  frames are byte-identical before and after. In snapshot mode a pick or
  submit now shows what it does (the pane viewer's `a` closes it and
  selects the mux before the skipped attach), and the stderr note names
  the skipped effect instead of `skipped a pick or submit`; an effect a
  pager or search used to refuse with a toast in snapshot mode is skipped
  with a note instead.
- A new modal is registered once, in `conventions.rs`, and is then held to
  every convention; a new key table is checked when a case consults it.
- A golden that should keep color can now; the existing `insta` goldens
  stay text, and moving them is a later choice per test.
- The crate's tree grows by about twenty files, all scanned by the guards;
  the guards' minimums rise with it.
- The host's list-cursor scan (ADR 0159 decision 7) drops its reviewed
  exception for the snapshot writer, which no longer lives in the host; the
  framework's raw-style guard reviews the writer instead.

## Alternatives Considered

- **A public module, no feature.** Simpler to use and to document, but the
  kit would be part of every production build's API and dead code there;
  the feature keeps it to builds that ask for it.
- **A separate crate (`tuikit-testkit`).** A clean dependency edge, but a
  second package to keep, a manifest allowlist entry, and an awkward split
  for the lexer the glyph scanner shares. A feature gives the same
  separation in builds.
- **ANSI text as the golden format.** Exact and already produced by the
  snapshot writer, but unreadable in a review diff. The legend form shows
  a recoloring as one changed line.
- **Writing goldens from the framework.** The usual accept step, but the
  framework writes no files; the host's writer is one closure.
- **Keep the convention checks in Conspectus.** They would keep working,
  but a second host would copy them, and the list of modals would stay in
  four places.
- **Keep dropping picks in snapshot mode** for the modals whose picks have
  outside effects. Safe, but the frame after a pick could not be seen; the
  reducer is pure, so running it and skipping only the outside effects is
  as safe.
- **Name the raw colors through `Display` strings** to avoid the guard
  exception. It would read ratatui's formatting instead of its enum to get
  around the scan; one reviewed exception says what the file does.

## Open Questions Answered

- *A feature or a public module?* A feature, `testkit`, enabled by the
  host's dev-dependency and its `snapshot` feature.
- *Which golden format keeps color?* Text and style rows with a legend,
  checked by `assert_golden`, accepted with `TUIKIT_ACCEPT_GOLDENS=1`.
- *Where does `strip_ansi` live?* In `tuikit::text`, since production code
  needs it; the kit re-exports it.
- *Do the host's source scans share the framework's lexer?* Yes, through
  `testkit::source`; the host's tree list and tripwires stay in
  `tests/support/source_tree.rs`, which walks with the kit.
