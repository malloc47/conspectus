# ADR 0197: The Main Screen's Characters Bind As Typed, As The Overlays' Do

## Status

Accepted. Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md)
decision 3 (the main view kept its own matchers for its characters: it
takes the overlay rule) and
[ADR 0192](0192-tui-the-main-screens-keys-are-one-scoped-keymap-and-the-help-page-is-generated-from-it.md)
decision 6 (the program's letters kept "any modifier but Control"). Applies
house rule 8 and the consistency-first rule of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md) to the main
screen's characters, and extends the framework's `keys` module with
`Keymap::loose_characters`. Carried out by CSP-797.

## Context

ADR 0160 gave every overlay one rule for characters: a character binds as
typed, alone or with Shift, never with Control or Alt, and a chord is
spelled out (`Ctrl-R`). The main screen kept its own matchers when its
keymap became one scoped `tuikit::keys::Keymap` (ADR 0192), because changing
them stops every chord of a main-screen letter and that story changed no
key. So one character still meant two things depending on what had the
keys:

- **Most letters fired with any modifier but Control** (`unless_ctrl`):
  `Alt-a` attached, `Alt-p` pinned, `Alt-/` opened quick select, and so on
  for `f ! ? a i o w v r n m p u ] [ / s < >` and the left tree's `h`, `l`,
  and `g`. The Control chord was left out so a binding of its own could
  take it (`Ctrl-R`, `Ctrl-G`, `Ctrl-F`, `Ctrl-O`, `Ctrl-L`).
- **The tree's `j`, `k`, and `G`, and the Preview's `J` and `K`, fired with
  any modifier at all** (`any_mod`), so `Ctrl-j` moved the tree. Many
  terminals send `Ctrl-j` (a line feed) for `Ctrl-Enter`, which ADR 0192
  had made do nothing on `Enter`'s own binding: it still moved the tree
  down.
- **The digits, `q`, and `.` fired with no modifier at all** (`plain`), a
  third rule.

The right pane's explorer already bound characters as typed (ADR 0188), so
`Alt-j` moved the tree on the left pane and nothing on the right. None of
these chords is documented: the operator guide, the help page, the design,
and the README name only the spelled-out chords (`Ctrl-R`, `Ctrl-G`,
`Ctrl-/` and its `Ctrl-_`/`Ctrl-7` forms, `Ctrl-L`, `Ctrl-O`, `Ctrl-F`,
`Ctrl-C`) and the reveal chords of `Enter` (`Shift-Enter`, `Alt-Enter`),
none of which is a chord of a character binding. The keymap's help page and
status bar write `a`, not `Alt-a`.

## Decision

1. **The main screen binds characters as the overlays do.** Every
   character binding of `keybindings::keymap()` is `keys::ch`: the
   character alone or with Shift (terminals differ on reporting Shift with
   `G` or `?`), never with Control, Alt, or any other modifier a terminal
   reports. A chord is a binding of its own, spelled out (`ctrl('r')`),
   and keys that are not characters keep ignoring their modifiers (an
   arrow, `Tab`, `PgDn`) or name them (`Enter` with none, `Shift-Enter`,
   `Alt-Enter`). The host's `unless_ctrl`, `any_mod`, and `plain` matchers
   go, so one rule covers the program, both panes, and every overlay.

2. **No chord is bound on purpose in their place.** None was documented,
   the status bar and the help page never named one, and a free chord is
   room for a later binding (a remap, ADR 0192 decision 3, would bind one
   by id). `Ctrl-j` and `Ctrl-k` stay free rather than becoming the
   readline-style moves the overlays' lists take (`Ctrl-N`/`Ctrl-P`, ADR
   0160 decision 4): the tree and the explorer are panes, and they bind
   neither (ADR 0188).

3. **The framework checks a keymap as it checks a table.**
   `Keymap::loose_characters` lists the keys that bind a character but
   ignore its Control or Alt chords, by binding id, as
   `keys::loose_characters` does for a table. The framework does not refuse
   them (a host may keep its own rule, as the keys page's seam says); the
   host holds its keymap to the overlay rule in a test that also presses
   every chord of every bound character on both panes and requires that
   it reaches nothing, or a binding that names that chord.

## Consequences

- **Keys that stop working** (with no modal open):
  - On either pane: the Alt chord (and a Super, Hyper, or Meta chord where
    the terminal reports one) of `f` (Controls), `!` (Messages), `?`
    (help), `a` (attach), `i` (copy id), `o` (open the value), `w`
    (worktree menu), `v` (view), `r` (refresh), `n` (new mux), `m` (mux
    menu), `p` (pin), `u` (undo), `]` and `[` (next and previous view),
    `/` (quick select), `s` (search contents), and `<` and `>` (older and
    newer search); every chord but Shift of `J` and `K` (scroll the
    Preview), Control included.
  - On the left pane: the Alt chord of `h` and `l` (fold) and `g` (first
    row); every chord but Shift of `j` and `k` (move) and `G` (last row),
    Control included, so `Ctrl-j`, which many terminals send for
    `Ctrl-Enter`, no longer moves the tree.
- **Keys that start working:** `q`, `.`, and `1` to `6` when a terminal
  reports them with Shift (few do: on most layouts Shift makes them other
  characters).
- **Nothing else changes.** The spelled-out chords keep their bindings,
  the help page and the status bar read the same (a key's label was
  already the plain character), and the right pane's keys are as before.
- A new main-screen character is one `ch` row; a matcher that ignores a
  character's chords fails the keymap's test.

## Alternatives Considered

- **Keep the main screen's matchers, as a stated exception.** No key
  changes, but a letter keeps meaning one thing in a menu and another on
  the tree (`Alt-g` went to the top of the tree and did nothing in a
  pager), the left and right panes keep disagreeing on `Alt-j`, and
  `Ctrl-Enter` keeps moving the tree through `Ctrl-j`. The chords it keeps
  are ones nobody was told about.
- **Characters ignore every modifier but Control everywhere** (the main
  screen's rule for the overlays too). ADR 0160 rejected it: `Alt-G` would
  stay an accidental end key, and one plain rule is easier to learn.
- **Keep `plain` for `q`, `.`, and the digits.** It is stricter than the
  rule only for Shift, which a terminal reports with these keys so rarely
  that the difference is a third matcher for nothing.
- **Bind `Ctrl-j`/`Ctrl-k` on purpose to move the tree.** It would keep a
  habit some operators may have formed, but no document or hint ever
  offered it, and the overlays' lists move with `Ctrl-N`/`Ctrl-P`, which
  the panes do not bind either.
- **Refuse loose characters in `KeymapBuilder::build`.** One rule enforced
  for every host, but a host with its own rule (a game, an editor) could
  not build; the check stays a listing, as `keys::loose_characters` is.

## Open Questions Answered

- *Do the program's keys adopt the overlay rule?* Yes: one rule for every
  character on every surface.
- *Which chords stop working?* Every chord of a bound character that no
  binding spells out; the list is under Consequences.
- *Should any be bound on purpose?* None.
