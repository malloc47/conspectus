# ADR 0191: The Status Bar Names The Keys Of The Scope That Has Them, Generated From Its Key Tables

## Status

Accepted. Carries out the `StatusBar` row of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md)'s control set
and its house rule 8 for the status bar, and extends
[ADR 0150](0150-tui-framework-workspace-crate.md) with the framework's
`status_bar` module. Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md):
a bound row carries an optional rank, and a footer segment lists the keys
it names and its highest rank. Amends
[ADR 0161](0161-tui-overlays-draw-themselves-on-one-host-owned-stack.md):
an overlay names the keys it takes now (`Overlay::hints`). Amends
[ADR 0164](0164-tui-test-kit-key-scripts-frame-driver-goldens-conformance.md):
the conformance suite leaves a host's status rows out of its
with-and-without comparisons and gains a `Hints` check. Carries out the
hint half of [ADR 0190](0190-tui-the-chrome-gives-way-by-one-rule-and-an-empty-slot-says-so-one-way.md)
decision 6 (the status bar's hints, still hand-written there, are
generated). Amends [ADR 0078](0078-tui-surface-division-of-labor.md)'s
focus scope: the `[left]`/`[right]` tag shows while a pane has the keys,
not under an overlay. Completes [ADR 0123](0123-overlay-keymap-tables.md)'s
consequence that a key's "status hint … follow[s] from that row", which
four hand-written hints did not. Amended by
[ADR 0192](0192-tui-the-main-screens-keys-are-one-scoped-keymap-and-the-help-page-is-generated-from-it.md)
(decisions 5 and 6): `MAIN_VIEW_KEYS` and the test that kept it in step
with the keymap are gone; the bar names the scoped keymap's bindings
(`KeyHintBar::keymap`) in the same groups and order. Decision 8's
status scan was widened by CSP-796: it also covers `app.rs`, `app/`,
and `actions.rs`, and flags a key named in a message's prose (a key
before a colon, a single character in backticks, "press" before a
named key); a status or refusal message names its key from the binding
that takes it (`keybindings::key_name`).

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/06-interaction-conventions.md`,
sections 1.3 and 9.3; `07-chrome-and-explorer.md`, sections 3.6 and 7)
found the status bar's hints written by hand, in three places, with no
check:

- **The main view's hints were strings.** `j/k move · h/l fold · Enter
  default`, `Tab focus`, `r refresh · ^R forges`, `q quit`, `Backspace
  back`, and about ten strings from `default_action_status_hint` (`Enter/a
  attach …`, `Enter/v view … · S resume`, `Enter/l expand · h collapse`,
  the pin hints, the attach-disabled reasons) were literals in
  `ui/header.rs`. Nothing tied them to the keymap. `^R` was a third key
  notation; `Enter` appeared twice on most rows (`Enter/a attach` and
  `Enter default`); an ambiguous row said `Tab inspect candidates · Tab
  focus`.
- **They described the left pane whatever had the keys.** With the right
  pane focused the bar still said `Enter/a attach …` or `Enter/l expand ·
  h collapse`, although the right pane's table takes `Enter`, `h`, and
  `l`; on an ambiguous row it pointed `Tab` at the right pane while the
  right pane had focus. With a modal open it kept the main view's hints,
  `q quit` and `Tab focus` among them, which the modal had taken.
- **Modal openers copied footers into the status line.** Opening the
  worktree menu or the mux menu posted a hand-written copy of the menu
  footer; Controls, quick select, and content search posted their table's
  footer behind a prefix (`controls: …`, `quick select: type to find ·
  …`). The copies sat in `runtime.rs` and `app/overlays.rs`, outside
  `tests/overlay_keymap_hygiene.rs`, and the snapshot driver, which
  opened Controls and the searches its own way, showed the main view's
  hints instead.

CSP-756 had already generated the right pane's hint from
`RIGHT_PANE_KEYS` (ADR 0188), and CSP-758 had put the bar's pieces on a
`PriorityLine` with named ranks, so what was left was the main view, the
overlays, and the check.

## Decision

1. **`tuikit::status_bar` draws the bottom row.** `StatusBar` shows
   either a message, a `Feedback` in its severity's style cut with `…`
   to the cells its lead leaves, or the host's segments and the hints on
   one `PriorityLine` in the cells its fixed spans leave: a lead in both
   modes (the unseen and busy chips), a scope shown with the keys only
   (the focus tag, the stale mark), and a trail (the provider chips).

2. **`KeyHintBar` generates the hints from key tables.** Each shown row
   is a hint of its label and verb in the footer's grammar, in table
   order; rows whose words match join their labels across the whole bar
   (`Enter/a attach tmux:main`); a note is a hint with no key. A bound
   row carries an optional rank (`KeyRow::rank`), the "priority per row"
   the `PriorityLine` page left as a seam; a row with none takes the rank
   the host gives its table. A footer segment now lists the keys it names
   and its highest rank, so a control's footer goes into the bar whole
   with its keys.

3. **The host words a row for where the reader is with an
   `Annotation`.** Built from its state each frame, an annotation leaves
   a row out, gives it another verb, a detail after the verb (what it
   acts on), a reason it cannot act (drawn dimmed in
   `keys::unavailable_style`, `a attach: no row selected`), or another
   rank. This is how `default_action_status_hint` becomes annotations on
   rows of a table instead of free text: the information a verb cannot
   carry (the mux a row attaches to, `at the match`, `(pin, forget, plan,
   clean up)`, why attaching is refused) is a field of the row's hint.

4. **An overlay names its own keys.** `Overlay::hints` returns the footer
   of the table an overlay resolves its keys with, at its current level;
   every framework control returns the footer it draws, and the host's
   three own entries return their palette's or form's. Empty by default.

5. **The scope that has the keys is the scope whose keys the bar shows.**
   In Conspectus:
   - **An overlay on top:** its `hints`, ranked above the view's settings
     (`status_rank::OVERLAY_KEYS`), with no `[left]`/`[right]` tag; the
     settings, the chips, and the stale mark stay.
   - **The left pane:** `MAIN_VIEW_KEYS`, the main view's table
     (`MainKey`: `Enter`, `a`, `v`, `S`, `.`, the arrows, `Tab`, `r`,
     `Ctrl-R`, `q`, `Backspace`, and `!`), each row annotated for the
     selection. `Enter`'s verb comes from the dispatcher's own
     classification (`selected_default_action`): a group or an
     expandable row folds (`Enter/←/→ fold`), a search hit opens at the
     match, a record rebuilds, a pin whose state decides says what
     `Enter` does (`Enter rebuild`, `Enter show URL`, dimmed where it
     refuses), a row that attaches names its mux, an un-muxed session
     names itself for the viewer and offers `S` when it resumes, and any
     other row says why `a` cannot attach. The arrows are shown, `j`/`k`
     and `h`/`l` stay bound (ADR 0160's "arrows shown, letters aliased").
     `Ctrl-R` shows in the PRs view, `Backspace` while a reveal can be
     returned from, `Tab inspect candidates` in place of `Tab focus` on
     an ambiguous row.
   - **The right pane:** `a attach …` when the right pane's entity
     attaches, the right pane's table (`Explorer::footer`, `Enter`'s verb
     the cursor row's), then `Tab`, `r`, `Ctrl-R`, and `q`. The left
     tree's keys, which the right pane's table takes, are left out.

6. **`MAIN_VIEW_KEYS` is checked against the keymap that resolves it.**
   The program-wide keymap (`KEYBINDINGS`) is not yet a key table; the
   scoped keymap (CSP-760) merges the two. Until then the status bar's
   table is held to it by a consistency test that, in every view, on
   every row, with either pane focused, presses every key the bar names
   through `action_for_event` (the one path every loop routes keys
   through) and requires the action the row stands for: the right pane's
   keys must reach the explorer under the right pane's own hints, the
   main view's keys the action their `MainKey` promises. The table is in
   the overlay table registry with `mislabeled`, `unresolved`, and
   `loose_characters`, exempt from `Ctrl-C` first as the panes are.

7. **Nothing writes footer text into the status line.** The worktree and
   mux menus, Controls, quick select, and content search no longer post
   a hint when they open; they clear the status line, as their copies
   used to replace what was there, and the bar shows the overlay's keys.
   The unseen-messages chip and a failure's `· ! details` name the
   Messages key from its row (`keybindings::messages_hint`); the chip
   drops it while an overlay has the keys. The snapshot driver opens
   Controls through the same function as the loops.

8. **Two checks guard it.** `tests/overlay_keymap_hygiene.rs` scans the
   code that writes the status line (`runtime.rs`, `runtime/executor.rs`,
   `app/overlays.rs`, `ui/header.rs`) for any hand-written hint, by a
   stricter rule than the overlay code's: a ` · `-separated piece that is
   keys and a verb (`Tab focus`, `q quit`, `^R forges`). The conformance
   suite's `Hints` check holds the keys the bar names with each
   registered modal open to the keys its footer names; its rig names the
   status row (`Rig::status_rows`), which the `Drawn` and `Contained`
   checks no longer count as the modal's.

## Consequences

- **Visible changes.** The bar's settings, chips, and fit rule are as
  before; the hints change as follows (the CSP-759 summary lists each
  frame).
  - The left pane's keys: `j/k move · h/l fold · Enter default` becomes
    `↑/↓ move · ←/→ fold`; `Enter`'s meaning is the selection's hint.
  - Group rows: `Enter/l expand · h collapse` becomes `Enter/←/→ fold`.
  - The PRs view: `r refresh · ^R forges` becomes `r refresh · Ctrl-R
    hard refresh`.
  - Pins: `pin `x` is stopped · Enter runs `conspectus rebuild x` to
    bring it back` (and the removed, planned, and missing forms) becomes
    `Enter rebuild`; `pin `x` is a live PR · Enter shows its URL` becomes
    `Enter show URL`; a pin whose target was not checked, or a live one
    with nothing to rebuild, shows `Enter rebuild` dimmed (with the
    reason for the first). The Preview keeps each pin's full sentence.
  - A row that cannot attach: `attach: selected row has no mux to attach
    to` becomes `a attach: selected row has no mux to attach to`, dimmed
    (`Enter/a attach: …` where `Enter` attaches too).
  - An ambiguous row names `Tab` once (`Tab inspect candidates`).
  - The right pane: `Enter/a attach X` becomes `a attach X`, before the
    right pane's keys; the left pane's `Enter`, view, resume, menu,
    fold, and disabled-attach hints go; `Tab` is `Tab focus`.
  - With an overlay open: its own keys, no scope tag, the settings kept;
    the menus' `mux: …` and `worktree: …` messages are hints now, so the
    settings and chips show beside them; `quick select: type to find ·`
    and `search contents: type a query ·` lose their prefixes, and
    `controls: …` (which only the live loop showed) goes.
  - The unseen chip reads `▲ N` while an overlay has the keys.
  - A status message wider than the bar ends with `…` instead of being
    cut at the edge.
  - Fit: each key group is a segment of its own, so a narrow bar keeps
    more hints than when the left pane's keys were one bundle (at 60
    columns `↑/↓ move` comes back beside `filter:` and `q quit`), and an
    overlay's first keys outlast the settings.
- A new main-view key the bar should name is a row of `MAIN_VIEW_KEYS`
  and an arm in the annotations; a new overlay's keys reach the bar
  through `Overlay::hints` with no host change.
- The scoped keymap (CSP-760) replaces `MAIN_VIEW_KEYS` and the
  hand-kept scope rule with scopes of one table; the bar's API is
  ready for it (`KeyHintBar::annotated` over the active scope's tables).

## Alternatives Considered

- **No hints under an overlay.** The modal draws its own footer, so the
  bar could show only the settings. It would remove the duplication, but
  the story's scopes include the top overlay, the menus and searches
  already showed their keys there, and an empty bar reads as nothing to
  do. The duplication is generated, so it cannot drift.
- **Name the overlay in the scope tag** (`[controls]`, `[menu]`). It would
  keep the old prefixes' information, but needs a word per modal that no
  table holds, and every modal's frame already names it.
- **Generate the main view's hints from `KEYBINDINGS`.** Its rows produce
  `Action`, which is not `Copy` and holds boxed messages, so it cannot
  be a `const` key table, and its help strings are sentences for the help
  page. A separate table checked against it is the smaller step; CSP-760
  merges them.
- **Rank by a closure over the action instead of on the row.** The ranks
  would sit beside the annotations; but a rank is a property of the
  row's hint the table can state once, and the `PriorityLine` page had
  named "ranks travel with the table rows" as its seam.
- **Keep the default-action hint on the right pane.** It described the
  left row; on the right pane its `Enter`, `h`, and `l` belong to the
  explorer. Only `a`, which acts on the right pane's entity, stays.
- **Keep the pin sentences.** They told the CLI equivalent
  (`conspectus rebuild x`); but they were key hints written as prose,
  long enough to push every other hint off a 160-column bar, and the
  Preview shows them in full.
- **`j/k move` and `h/l fold`.** The letters work, but ADR 0160 shows the
  arrows in every list's footer, and the right pane already said `↑/↓
  move`.

## Open Questions Answered

- *Where does a hint's priority live?* On its key row (`KeyRow::rank`),
  or on the host's annotation for a row when the context decides it.
- *What does the bar show with a modal open?* The modal's own keys
  (`Overlay::hints`), above the view's settings.
- *How does contextual information survive generation?* As annotation
  fields on rows: a verb, a detail, a reason, a rank.
- *How is "every advertised key resolves" checked?* By pressing every
  key the bar names through the loops' routing in every view, row, and
  focus, and, for modals, by the conformance `Hints` check.
