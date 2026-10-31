# House rules

The design language every control follows. A program built on the framework
feels like one program because each of its lists, modals, readers, and fields
answers the same keys the same way and looks the same doing it. These eighteen
rules say how; each one below gives the rule, why it exists, the pages of the
modules that carry it out, and what checks it.

A change that adds or alters a control applies every rule that fits, and the
control's page lists, under "House rules followed", the rules it follows and
how, and the ones that do not apply and why. Where a rule changes what a user
sees, the change catalogs the difference (see
[contributing.md](contributing.md#cataloging-visible-changes)).

The rules every modal shares are checked mechanically by the test kit's
conformance suite ([testkit.md](controls/testkit.md)): `Selection` (rule 1),
`Keys` (rules 4 and 8), `Close` (rules 4 and 5), `Footer` and `Hints` (rule
8), `Title` (rule 13), and `Words` and `Contained` (rule 14). The source guards
check rule 13 in the framework's own code
([contributing.md](contributing.md#the-raw-style-guard)). The rest are each
control's own tests and review.

| # | Topic | Rule |
| --- | --- | --- |
| 1 | [List cursor and highlight](#1-list-cursor-and-highlight) | Full-row highlight from the selection pair, and the pointer in a 2-cell gutter, in every list |
| 2 | [List ends](#2-list-ends) | Short choice lists wrap; scrollable content clamps |
| 3 | [Confirmation](#3-confirmation) | An inline second `Enter` when there is nothing more to say, a dialog when a body helps, nothing for a reversible action |
| 4 | [Close keys](#4-close-keys) | `Esc` steps back one level; `Ctrl-C` and `q` dismiss (`q` only where nothing is typed) |
| 5 | [Close feedback](#5-close-feedback) | Cancelling is silent |
| 6 | [Forms](#6-forms) | `Enter` submits from any field; one ring of stops |
| 7 | [Validation errors](#7-validation-errors) | One error line in the footer slot, set by a refused confirm and cleared by the next edit; nothing refused silently |
| 8 | [Footers and hints](#8-footers-and-hints) | Every footer and status hint is generated from key tables, in one grammar |
| 9 | [Focus cue](#9-focus-cue) | The focused pane's border and title marker take the focus accent |
| 10 | [Unfocused cursor](#10-unfocused-cursor) | Every list shows its cursor, in the inactive look when it lacks focus |
| 11 | [Feedback severity](#11-feedback-severity) | Four severities; every channel styles from the theme; errors stay longest |
| 12 | [Paging](#12-paging) | Pages are the rows shown; ends on every long list; half pages in readers; offsets clamped |
| 13 | [Theme](#13-theme) | Every control takes `&Theme`; no raw colors; two muted looks |
| 14 | [Width](#14-width) | Measure terminal cells; `…` marks a cut; paths and identifiers cut in the middle |
| 15 | [Text editing](#15-text-editing) | One editor: cell-correct, safe to paste into, with word motion |
| 16 | [Path fields](#16-path-fields) | A kind, `~` expanded on commit, completion through a source whose answers are kept |
| 17 | [Labels](#17-labels) | One vocabulary, the shorter form |
| 18 | [Pagers](#18-pagers) | One reader: `Esc` steps back, `q` leaves, a find lands on its match, the position always shows |

## 1. List cursor and highlight

**Rule.** Every list draws its cursor row whole in the theme's selection pair
(`selection_active` while the list has focus, `selection_inactive` while it
does not) and puts the pointer mark (`marks::POINTER`) in a 2-cell gutter
before it, two blank cells on every other row.

**Why.** A program that grows its lists one at a time ends up with several
cursor looks (bold, a hand-reversed row, a colored glyph, a third-party
widget's own), and a user has to learn each. One look is one cue. The marker
keeps the cursor visible on a terminal with no color or no reverse video, and
because every row has the gutter, nothing on the row moves as the cursor does.
A highlight across the whole row shows what `Enter` acts on.

**Carried out by.** [selection.md](controls/selection.md) (`selection::row`,
the one call a list makes per row), and through it
[action_menu.md](controls/action_menu.md),
[checklist.md](controls/checklist.md), [palette.md](controls/palette.md),
[settings_panel.md](controls/settings_panel.md),
[log_list.md](controls/log_list.md), [tree_view.md](controls/tree_view.md),
[property_grid.md](controls/property_grid.md),
[explorer.md](controls/explorer.md), and [form.md](controls/form.md). The
theme supplies the pair ([theme.md](controls/theme.md)) and the glyph table the
mark ([glyphs.md](controls/glyphs.md)).

**Checked by.** The conformance suite's `Selection` check (moving the cursor
moves no text outside the gutter) and the raw-style guard, which forbids a
hand-written reverse.

## 2. List ends

**Rule.** A short choice list (a menu, a picker, a checklist, a palette, a
settings list, a form's ring of fields) wraps from its last row to its first.
Scrollable content (a tree, a property list, a pager, a log) stops at its
first and last rows. Page keys stop at the ends in either kind.

**Why.** In a short list the far end is one step away, so wrapping is the
quick way there. In long content, wrapping loses the reader's place: a held
key runs off the end and lands back at the top. The rule goes by what a list
is, not how long it happens to be, so a list does not change behavior as it
grows.

**Carried out by.** [selection.md](controls/selection.md) (`Ends::Wrap`,
`Ends::Clamp`) and [scroll.md](controls/scroll.md) (`wrap_step`,
`clamp_step`); the lists named in rule 1 each pick their kind.

## 3. Confirmation

**Rule.** An action that needs a second thought and has nothing more to say
arms inline: the first `Enter` on the menu entry says what the next will do,
and the second does it. An action whose consequences need explaining (what
it will affect, a choice of how) asks a dialog: `y` or `Enter` confirms, `n`
or `Esc` cancels. A reversible action asks nothing.

**Why.** Four ways to confirm teach a user none of them. An inline arm costs
one key and no context switch; a dialog is worth its interruption only when
it shows something. Confirming what can be undone trains people to confirm
without reading, which spends the confirmation's value where it matters.

**Carried out by.** [action_menu.md](controls/action_menu.md)
(`Confirm::Inline`, and entries that open a step) and
[dialog.md](controls/dialog.md) (one key table for every form of dialog),
with the dialog keys in [keys.md](controls/keys.md).

## 4. Close keys

**Rule.** `Esc` steps back one level: out of a find, a help page, an editor,
a step, and at an overlay's first level, out of the overlay. `Ctrl-C`
dismisses the whole overlay, every step of it included, from any level, and
is the first row of every overlay's key table. `q` dismisses too wherever
nothing is typed, and types where text is in play.

**Why.** A user who knows one rule can get out of anything: `Esc` never loses
more than one level, and `Ctrl-C` always gets out. Letting each overlay pick
its own close semantics keeps the rule unlearnable. `q` is the reader's habit
from pagers, but in a field it must type.

**Carried out by.** [overlay.md](controls/overlay.md) (`Close` for a step
back, `Dismiss` for the whole flow, and the stack that settles each),
[keys.md](controls/keys.md) (the standard tables, `ctrl_c_first`), and every
overlay's page.

**Checked by.** The conformance suite's `Keys` (`Ctrl-C` first, bound to
dismissing) and `Close` (`Ctrl-C`, `Esc`, and `q` close a modal at its first
level, `q` only where it types nothing) checks.

## 5. Close feedback

**Rule.** Closing or cancelling says nothing: no status message, no toast.

**Why.** Cancelling is not an event. A "cancelled" message after every `Esc`
is noise that pushes real reports off the status line, and a user who
cancelled already knows.

**Carried out by.** [overlay.md](controls/overlay.md): `Close` and `Dismiss`
carry no message, so a host has nothing to post. Every overlay returns them
for its close keys.

**Checked by.** The conformance suite's `Close` check: closing leaves nothing
on the status line and posts no toast.

## 6. Forms

**Rule.** `Enter` submits a form from whichever field has the focus. `Tab`
and `Down` go to the next stop and `Shift-Tab` and `Up` to the previous, over
one ring that wraps; a read-only line is no stop.

**Why.** A user who has filled in what they need should not walk to a button.
Two orders (one for the arrows, one for `Tab`) make a form unpredictable, and
an `Enter` that moves in one form and submits in another is a trap.

**Carried out by.** [form.md](controls/form.md), on `FORM_KEYS`
([keys.md](controls/keys.md)).

## 7. Validation errors

**Rule.** A refused confirm puts one line saying why in the modal's footer
slot, directly above its keys, and moves the focus to what it is about; the
next edit clears it. No input is refused silently.

**Why.** A key that does nothing looks broken. A refusal belongs where the
user is looking (by the keys they just pressed), not in a status line the
modal covers or a toast that goes before it is read, and it should go away
once the user acts on it.

**Carried out by.** [form.md](controls/form.md) (`Submit::Reject`, field
checks) and [text_prompt.md](controls/text_prompt.md)
(`PromptResult::Reject`); the explorer reports every refusal as an outcome
for its host to word ([explorer.md](controls/explorer.md)).

## 8. Footers and hints

**Rule.** Every modal's footer and every status-bar hint is generated from the
key tables the keys are resolved with, in one grammar: the key, a space, and
a short imperative verb with no "to" (`Esc close`, `Enter pick`). A footer
lists the keys that leave last, and a footer too tall for its frame keeps
those.

**Why.** A hint written by hand drifts from the keys: it names a key that does
nothing, or leaves out one that works. Generated from the same table that
resolves the key, it cannot. One grammar reads alike everywhere, and the keys
that leave are the ones a lost user needs most.

**Carried out by.** [keys.md](controls/keys.md) (`Footer`,
`Footer::leaving_last`, the scoped keymap's verbs),
[frame.md](controls/frame.md) (`ModalFooter`, one look and one fit),
[status_bar.md](controls/status_bar.md) (`KeyHintBar`), and
[help.md](controls/help.md) (pages generated from the same tables).

**Checked by.** The conformance suite's `Footer` (the drawn footer is the
generated one, in its look and place, the leaving keys last and kept on a
short frame), `Hints` (the status line names only keys the modal takes), and
`Keys` (labels, shadowed rows) checks.

## 9. Focus cue

**Rule.** The pane with focus draws its border and its title's marker in the
theme's `panel_focus_accent`; the other panes keep a plain border and a blank
where the marker goes.

**Why.** With two panes on the screen, a user has to see which one their keys
go to. Color on the border shows it at a glance, and the marker in the title
shows it without color. The blank keeps the title from moving as focus does.

**Carried out by.** [frame.md](controls/frame.md) (`PaneFrame`), with the
token in [theme.md](controls/theme.md); [tree_view.md](controls/tree_view.md)
draws itself in a pane frame, and [app_frame.md](controls/app_frame.md) lays
out the panes a host frames.

## 10. Unfocused cursor

**Rule.** A list without focus still shows its cursor: the marker stays, and
the row takes the inactive selection look.

**Why.** A cursor that vanishes when its pane loses focus hides what the other
pane is showing details of, and the user has to find their place again when
they come back.

**Carried out by.** [selection.md](controls/selection.md)
(`Cursor::Inactive`), [tree_view.md](controls/tree_view.md),
[property_grid.md](controls/property_grid.md), and
[explorer.md](controls/explorer.md); an overlay under a step of its own draws
its cursor inactive too ([overlay.md](controls/overlay.md),
`render_under_step`, as [action_menu.md](controls/action_menu.md) and
[settings_panel.md](controls/settings_panel.md) do).

## 11. Feedback severity

**Rule.** Every report has one of four severities (info, success, warning,
error). Each channel (the status line, a toast, the message log) styles it
from the theme by its severity, and an error's toast stays longest.

**Why.** A failure must never look like a success. One report type with a
severity, fed to three channels, keeps the channels agreeing; an error stays
longer because it needs reading, while a confirmation only needs noticing.

**Carried out by.** [feedback.md](controls/feedback.md) (`Severity`,
`Feedback`, `Toasts`, `Log`), [status_bar.md](controls/status_bar.md), and
[log_list.md](controls/log_list.md); the chip tones agree
([chip.md](controls/chip.md)).

## 12. Paging

**Rule.** `PgUp` and `PgDn` move by the rows the last frame showed. Every list
taller than a screen has `Home` and `End` (and `g` and `G` where nothing is
typed). Readers have half pages on `Ctrl-D` and `Ctrl-U`. Every scroll offset
is clamped, on every frame.

**Why.** A page that moves by the terminal's height, or by a fixed count,
skips or repeats rows a user just saw. An offset that is never clamped leaves
blank rows under a list that shrank while rows above it are scrolled away,
and makes the next key up do nothing.

**Carried out by.** [scroll.md](controls/scroll.md) (`ScrollWindow`, the
window a control keeps and returns), [pager.md](controls/pager.md),
[tree_view.md](controls/tree_view.md), [checklist.md](controls/checklist.md),
[settings_panel.md](controls/settings_panel.md),
[palette.md](controls/palette.md), [explorer.md](controls/explorer.md), and
[log_list.md](controls/log_list.md).

## 13. Theme

**Rule.** Every control takes `&Theme` and names no raw color; a role only the
host knows is a named style the host registers. Text that recedes uses one of
two looks, `muted` and `inert`, not a dim of its own.

**Why.** One place changes the look of every control, so a host's users can
re-theme the whole program, and a terminal's own palette is respected. Muted
looks invented per surface drift apart; two named ones do not.

**Carried out by.** [theme.md](controls/theme.md) and every control's "Theme
tokens" section.

**Checked by.** The raw-style guard over the framework's source
([contributing.md](contributing.md#the-raw-style-guard)) and the conformance
suite's `Title` check (a modal's frame and title take the accent).

## 14. Width

**Rule.** All cutting, padding, and wrapping measures terminal cells, not
characters. A cut ends in `…` inside its room; paths and identifiers are cut
in the middle, so both ends stay readable. Nothing runs past its room for a
border to clip.

**Why.** A terminal gives a wide character or an emoji two cells and an accent
none, so code that counts characters misaligns every column after a wide
name. A cut without a mark hides that text is missing; a path cut at its end
loses the part that tells two apart.

**Carried out by.** [text.md](controls/text.md) (the one set of cut, pad,
wrap, and give-way rules), [glyphs.md](controls/glyphs.md) (every drawn glyph
is one measured cell), [chip.md](controls/chip.md),
[columns.md](controls/columns.md), [breadcrumb.md](controls/breadcrumb.md),
[tab_bar.md](controls/tab_bar.md), and
[priority_line.md](controls/priority_line.md).

**Checked by.** The conformance suite's `Words` (no word cut at a modal's
edge) and `Contained` (nothing escapes the frame, at ten sizes) checks, and the
glyph guard.

## 15. Text editing

**Rule.** Every text field is the one line editor: editing and windowing are
correct for cell widths, word motion works on `Alt` and `Ctrl`, and a paste
is typed as one line, so a pasted line break can never submit the field. The
host turns bracketed paste on and types a paste only into an overlay whose
text field has the keys.

**Why.** Several editors with different key subsets make every field a new
thing to learn. A caret on the wrong cell after a wide character, or a paste
whose newline presses `Enter`, loses the user's text or submits it half
typed.

**Carried out by.** [text_edit.md](controls/text_edit.md) (`LineEdit`,
`TextField`, `single_line_paste`), [overlay.md](controls/overlay.md)
(`takes_text`), and the fields built on them:
[text_prompt.md](controls/text_prompt.md), [form.md](controls/form.md),
[path_field.md](controls/path_field.md), [palette.md](controls/palette.md),
and the pager's find ([pager.md](controls/pager.md)).

## 16. Path fields

**Rule.** A path field asks for a directory or a file, completes and judges
what is typed by that kind, expands `~` on commit, and completes through a
read-only source its host supplies, whose answers it keeps so drawing does no
I/O.

**Why.** A field that does not know what it wants suggests files where a
directory is needed and calls them fine. A `~` committed as typed reaches a
command that does not expand it. A listing read on every frame makes drawing
slow and makes the field untestable without a real disk.

**Carried out by.** [path_field.md](controls/path_field.md) (`PathField`,
`PathSource`, `PathKind`), the in-memory source in
[testkit.md](controls/testkit.md) (`MemoryPaths`), and
[form.md](controls/form.md).

## 17. Labels

**Rule.** One vocabulary, the shorter form: a field carries the same short
label in every control (`name`, not a qualified `<kind> name`), and a switch
reads `name·state (key)` wherever it shows.

**Why.** The same thing named two ways reads as two things. Short labels scan
faster and fit a narrow screen.

**Carried out by.** The host's copy, which every control takes as given; the
framework's part is the shapes that keep labels alike:
[form.md](controls/form.md) (one label column),
[chip.md](controls/chip.md) (`ToggleChip`), [tab_bar.md](controls/tab_bar.md),
and [breadcrumb.md](controls/breadcrumb.md) (a suffix only where two tags
would read alike).

## 18. Pagers

**Rule.** Every reader is the one pager: `Esc` steps back (the help page, then
the find, then the reader) and `q` leaves; confirming a find always scrolls to
the match, even one already in view; a full-screen reader shows a `[ n/m ]`
position chip, and a framed or embedded one a scrollbar.

**Why.** A program grows several readers (a document, a captured screen, a
help page, a long value, a log's detail), and each grows its own keys and
quirks. One reader behaves alike everywhere, a find never leaves the user
hunting for the match, and the position always says where they are.

**Carried out by.** [pager.md](controls/pager.md), and the readers built on
it: [help.md](controls/help.md) and [log_list.md](controls/log_list.md); the
position chip is [chip.md](controls/chip.md)'s `PositionChip` and the
scrollbar [scroll.md](controls/scroll.md)'s `ScrollGutter`.
