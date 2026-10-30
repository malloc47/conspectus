# ADR 0192: The Main Screen's Keys Are One Scoped Keymap, And The Help Page Is Generated From It

## Status

Accepted. Extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's scoped keymap (`tuikit::keys::Keymap`) and keymap help pages
(`tuikit::help::keymap_sections`), and applies house rules 8 and 12 of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md) to the main
screen's own keys. Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md)
decision 3 (the main view's keymap keeps its own matchers until the scoped
keymap replaces it: it is replaced, and keeps them, decision 6 here);
[ADR 0162](0162-tui-help-pages-one-page-generated-from-key-tables.md)
decision 6 (the keymap page's hand-written rows become rows generated from
the keymap, with annotations for what a binding cannot say);
[ADR 0188](0188-tui-the-explorer-is-a-framework-control-over-a-host-source.md)
decision 5 (the left pane's meanings of `F`, `Enter`, and `Backspace` and the
right pane's are now scopes of one table, and the focus guard is gone); and
[ADR 0191](0191-tui-the-status-bar-names-the-keys-of-the-scope-that-has-them.md)
decisions 5 and 6 (`MAIN_VIEW_KEYS` and the test that kept it in step with
the keymap are replaced by the keymap itself). Decides open question 2 of
the interaction-conventions inventory (a scoped keymap in this cut, its
rows carrying help text that renders).

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/06-interaction-conventions.md`,
section 1) found the main screen's keymap the weak half of the key model:

- **One mode.** `KEYBINDINGS` had a `KeyMode` with one variant, `Global`.
  What a key meant by focus was code: the right pane's table was resolved
  first by its own function, and a focus guard dropped the left tree's
  actions on the right pane. The keys that change meaning by focus (`F`,
  `Enter`, `Backspace`, the moves and folds) were declared in two places.
- **Help text nothing drew.** Its 58 `help_text` strings were read only by
  a test's failure message. The `?` page was 48 hand-written rows (40 once
  the right pane's section was generated, ADR 0188) in `widgets/help.rs`,
  held to the table by a one-way substring test with a map of equivalent
  spellings.
- **Keys outside the table.** `PgDn`, `PgUp`, `o`, `i`, `Tab`, `J`, and
  `K` were matched by hand in `keymap::translate`, invisible to the drift
  test.
- **A duplicate check that missed shadowing.** It compared exact matchers
  only. Matching was first row wins, so `g` (any modifier) after `Ctrl-G`
  (grouping) lost `Ctrl-G` without a word, and `Enter` (any modifier)
  overlapped `Shift-Enter` and `Alt-Enter` (reveal), which worked only
  because they came first.
- **A second table for the status bar.** CSP-759 generated the bar's hints
  from `MAIN_VIEW_KEYS`, a table beside the keymap kept in step by a test
  (ADR 0191 decision 6), because `KEYBINDINGS` rows produced `Action`,
  which holds boxed messages and cannot be a `const` key table.

## Decision

1. **The framework owns a scoped keymap.** `tuikit::keys` gains `Scope`
   (`Global`, `Pane(name)`, `Overlay(name)`), `ActiveScopes` (the scopes
   that have the keys, looked up most particular first: an overlay mode,
   the pane with focus, then the program), `bind` declarations (`Bind`, a
   `const`), `Keymap` built by `KeymapBuilder`, `Binding` (a built row),
   and `Conflict`/`Conflicts`. A binding carries a stable id, its keys (the
   first is the one a hint names, the rest aliases), the action, a short
   verb (`.verb`, none keeps it out of hints), the help section and text
   its row says (`.help`), a status rank (`.rank`), and an optional
   predicate over the host's context saying whether it can act now
   (`.available_when`). The keymap is generic over the action `A` and the
   context `Cx`; it names nothing of the host.

2. **No key shadows another.** `KeymapBuilder::build` returns every
   conflict: two bindings sharing an id; an id that is not lower-case
   letters, digits, `.`, and `-`; a binding with no key or no help; and any
   two keys of one scope that one key event fires (`KeyMatcher::overlap`),
   in either order and within one binding. Keys of different scopes may
   overlap by design; `Keymap::overrides` lists where a pane's or a mode's
   binding takes a global one's key, so a host can hold the list to what it
   means. Because nothing shadows, the table's order decides nothing about
   resolution: it is the order the help page lists the bindings in.

3. **Stable ids, so a remap needs no table change.** Every binding is
   found by its id (`Keymap::get`), checked unique and well formed at
   build. A user remap, when it comes, replaces the keys of the bindings
   its configuration names before `build`, which checks the result like
   any table. No remap is built now (the framework's rule 10: leave seams
   open).

4. **A control's table becomes a pane's scope.** `KeymapBuilder::table`
   takes a control's own `KeyRow` table in, one binding per action it
   binds, in the order the action's help lists them, with every key bound
   to it (shown keys first) and its first shown row's verb and rank, and
   the host's id, scope, action, and help for each. The explorer's
   `EXPLORER_KEYS` (as `RIGHT_PANE_KEYS`) is the right pane's scope without
   a copy of its rows, and still what the explorer's footer and status hint
   are generated from.

5. **Availability: resolve ignores it, the bar hides, the page dims.** A
   binding that cannot act now still resolves, so the host can say why
   (ADR 0160 decision 7). The status bar, which names the keys that act
   now, leaves it out (`KeyHintBar::keymap`); a help page generated with a
   context dims it (`keymap_sections_in`). An annotation's `because`
   still dims a key that the reader expects but that cannot act on this
   row, with the reason.

6. **Conspectus adopts it.** `keybindings::keymap()` is the one table of
   every key the main screen takes with no modal open: `PROGRAM_KEYS` (the
   program's bindings in `Scope::Global` and the left pane's row tree in
   `LEFT_PANE`), the right pane's table in `RIGHT_PANE`, and `E` after it.
   `Command` (`Copy`, one variant per binding, explorer keys as
   `Command::Explorer`) replaces `MainKey`; `Command::action` maps it to
   the loops' `Action`. `KEYBINDINGS`, `KeyMode`, `translate_via_table`,
   the seven hand-matched keys, `right_pane_action`, and `guard_focus` are
   gone; `action_for_event` resolves an overlay's key first, then the
   keymap for the focused pane (`keymap::translate_in`). The left tree's
   keys are left-pane bindings, so on the right pane a chord its table
   does not bind (`Alt-j`) does nothing, as the guard made it. The program's
   letters keep their matchers (any modifier but Control, so `Alt-a`
   attaches); the overlay rule for characters is not applied here.

7. **The shadowing the build found is fixed.** `g` (first row) binds any
   modifier but Control, so `Ctrl-G` is only grouping's (no key changes:
   the grouping row already won). The left pane's `Enter` binds no
   modifier (`keys::exact`), as the right pane's does, so `Shift-Enter` and
   `Alt-Enter` are only reveal's; `Ctrl-Enter` no longer runs the default
   action. `o` is the one global key a pane takes (the right pane's inspect,
   the same action), which a test pins.

8. **The status bar reads the keymap.** `MAIN_VIEW_KEYS` and its parity
   test go. `status_hints` names the keymap's bindings in groups, so the
   bar reads as before: what acts on the selection (`Enter`, `a`, `v`, `S`,
   `.`, the arrows; on the right pane `a` and the explorer's footer), the
   focus key, the program's keys (`r`, `Ctrl-R`, `q`), then `Backspace`,
   whose binding can act only while a reveal can be returned from
   (`App::can_nav_back`). Each group is a call of `KeyHintBar::keymap`
   with an annotation that keeps it; hints join across calls. The check
   that every key the bar names does what it says stays, now pressing each
   through the loops and requiring the binding the keymap resolves.

9. **The `?` page is generated from the keymap.** `help::keymap_sections`
   lists one row per binding, under the section it names, its keys in the
   binding's order and its help text as the description. What a binding
   cannot carry is a `HelpAnnotation` anchored by id: more prose after a
   row (the Controls overlay's, the viewer's, quick select's, and the
   content search's own keys, which their tables bind), a row of its own
   after a row (`/ then Tab`, a key of the overlay `/` opens), and a
   section of its own (the four legends). `help::unanchored` finds an
   annotation whose binding is gone. The page keeps its sections and their
   order, and every row's prose where one row was one binding.

10. **A two-way test replaces the one-way one.** Every binding has exactly
    one row whose keys are all its keys and whose description starts with
    its help text; every key row is a binding's row, each of whose keys
    reaches that binding in its scope, or an annotation row that starts
    with the key of the binding it follows; and no annotation is
    unanchored.

## Consequences

- **Visible changes** (the CSP-760 summary lists them, checked with
  `conspectus tui --snapshot` at 100x30, 60x20, and 40x10 on every page of
  the help and the status bar in every view and focus):
  - The status bar is byte-identical in all 132 captured states.
  - The help page splits each row that described two keys into one row per
    binding: `1`, `2`, `3`, `4` (`1 / 2 / 3 / 4`), `]` and `[`, `<` and
    `>`, `↑ / k` and `↓ / j` (`j / k / ↓ / ↑`), `← / h` and `→ / l`
    (`l / → / h / ←`), `PgDn` and `PgUp`, `Home / g` and `End / G`
    (`g / G`), `J` and `K`, each with its half of the old sentence.
  - It gains rows for two bound keys it never listed: `o` in Actions and
    `Backspace` in Navigation.
  - The Navigation `Enter` row (a second description of the default action)
    goes; the Actions row stays.
  - The content search's keys read `s / Ctrl-/ / Ctrl-_ / Ctrl-7` (they
    read `Ctrl-/ (or Ctrl-_ / Ctrl-7) / s`).
  - The page is longer: 16 pages at 60x20 (15), 55 at 40x10 (52), six at
    100x30 as before.
  - `Ctrl-Enter` on the left pane does nothing.
- A new main-screen key is one `bind` row: it resolves, gets a help row,
  and can get a hint, with no other change; a key that overlaps another in
  its scope fails the build, and the keymap's tests build it.
- The program-wide keymap is a `LazyLock` built once; a conflict would
  panic at first use, which the tests reach first.
- The help page and the status bar can no longer disagree with the keys;
  hand-written prose remains only in annotations, each anchored to a
  binding that must exist.

## Alternatives Considered

- **Keep `KEYBINDINGS` and give its rows scopes** (the old struct with a
  real `KeyMode`). Smaller, but its rows hold `fn() -> Action` and help
  strings with no section or verb, so the status bar would still need a
  second table, and the check would still be the host's.
- **Rows over the host's `Action`.** `Action` holds boxed messages and is
  not `Copy`, so no `const` table; a `Copy` command enum mapped to
  `Action` keeps the table `const` and the dispatch one `match`.
- **Order-dependent resolution with a "shadowed" check only** (refuse a
  row every one of whose events an earlier row takes). It would allow
  `Shift-Enter` before a general `Enter`, but the table's order would keep
  deciding meaning, and a reorder for the help page could change what a
  key does. Refusing any overlap in a scope makes order free.
- **Copy the explorer's keys into the host's table.** One row type, but a
  copy of a framework table, the thing ADR 0160 removed; `table()` takes
  it in.
- **Order the status bar by the table and give the page another order.**
  A separate listing would be a parallel table again; grouping the bar by
  calls keeps the page's order in the table and the bar's in the host's
  status code.
- **Dim an unavailable binding in the status bar.** Consistent with
  footers, but `Backspace back` would show dimmed whenever there is no
  reveal; the bar names what acts now, and an annotation dims a key that
  cannot act on this row.
- **Apply the one character rule to the program's letters.** One rule
  everywhere, but every `Alt` chord of a main-screen letter would stop
  working, which this story does not decide; filed as follow-up work.
- **Generate the page with every prose clause as annotations.** The page
  would read the same; one sentence per binding is help text, and only
  what describes other tables' keys, sequences, and marks is an
  annotation.

## Open Questions Answered

- *Scoped keymap in the first cut, or overlay tables only?* Scoped, with
  rows carrying help text that renders.
- *What is an overlay scope for?* A mode that lets the program's keys
  through but gives some keys a meaning of its own; modals keep their own
  tables, resolved first.
- *What does availability do in a keymap?* Resolution ignores it; the
  status bar hides, a context-aware page dims.
- *Where does hand-written help prose live?* In annotations anchored to a
  binding id, checked by `unanchored`.
