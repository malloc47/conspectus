# `StatusBar`

| | |
| --- | --- |
| Module | `tuikit::status_bar` |
| Layer | 1 (component), built on the layer 2 key model |
| Type parameters | none on the types; `KeyHintBar::table` and `annotated` take any table's action enum `A`, and `KeyHintBar::keymap` a keymap's action `A` and the host's context `Cx` its availability reads |
| Cargo feature | none |
| Outcomes | none: `line` returns the bar's spans for the width it is given, `render` draws them |

## Purpose

The last row of a screen tells the reader one of two things: what just
happened (a message), or what the keys do now. A status bar whose key hints
are written by hand drifts from the keys: it names a key that a pane or an
overlay on top has taken for something else, words a key one way while the
footer and the help page word it another, and copies an overlay's footer
into itself when the overlay opens. `StatusBar` draws the row, and its
`KeyHintBar` generates the hints from the key tables, and the scoped
keymap, of the scope that has the keys, so a hint cannot name a key its
scope does not bind.

- **A message** is a [`Feedback`](feedback.md) in its severity's style
  (`Feedback::span`), after the host's lead, cut with `…` when it is wider
  than its room.
- **The keys** are the host's segments (settings in effect, a count) and
  the hints, on one [`PriorityLine`](priority_line.md): they give way lowest
  rank first, a narrower one comes back where a wider one did not fit, and
  the line ends in ` …` when something went.

A hint is a key table's shown row in the footer's grammar (`Enter open`):
rows that share their words join their labels (`↑/↓ move`), hints keep the
order of their tables, and a row's rank (`KeyRow::rank`) says how long it
holds on, or the rank the host gives its table. What a key does can depend
on where the reader is, so the host annotates rows each time it builds the
bar ([`Annotation`]): leave a row out, give it another verb, a detail after
the verb (`Enter/a open report.txt`), a reason it cannot act (drawn dimmed:
`d delete: read-only`), or another rank. An overlay names its own keys with
[`Overlay::hints`](overlay.md), its footer at its level, which goes in whole.
A program's own keys come from its [scoped keymap](keys.md#the-scoped-keymap):
`KeyHintBar::keymap` names the bindings with a verb that act in the scopes
that have the keys, in the keymap's order, leaving out a binding that cannot
act now (its availability predicate) and one whose key a more particular
scope takes.

The host keeps which scope has the keys and which tables are its, every
verb, detail, and reason (its words), the ranks, its own segments, and the
spans that must never move: a lead shown in both modes (a count that needs
attention, busy chips), a scope shown with the keys only (a tag naming whose
keys they are, a mark), and a trail after them (chips at the end).

## Usage example

A file list's status bar: the list's keys and the program's, annotated for
the row under the cursor, beside a setting; then a message in its place.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::text::Span;
use tuikit::feedback::Feedback;
use tuikit::keys::{Footer, KeyRow, MENU_KEYS, ch, ctrl, key, shown};
use tuikit::priority_line::Segment;
use tuikit::status_bar::{Annotation, KeyHintBar, StatusBar};
use tuikit::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListKey {
    Up,
    Down,
    Open,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgramKey {
    Edit,
    Reload,
    Quit,
}

const LIST_KEYS: &[KeyRow<ListKey>] = &[
    shown(key(KeyCode::Up), ListKey::Up, "↑", "move").rank(3),
    shown(key(KeyCode::Down), ListKey::Down, "↓", "move").rank(3),
    shown(key(KeyCode::Enter), ListKey::Open, "Enter", "open").rank(6),
    shown(ch('d'), ListKey::Delete, "d", "delete").rank(2),
];

const PROGRAM_KEYS: &[KeyRow<ProgramKey>] = &[
    shown(ch('e'), ProgramKey::Edit, "e", "edit").rank(6),
    shown(ch('r'), ProgramKey::Reload, "r", "reload").rank(1),
    shown(ctrl('q'), ProgramKey::Quit, "Ctrl-Q", "quit").rank(9),
];

let theme = Theme::default();
// The cursor is on a read-only text file: `Enter` edits it, as `e` does.
let file = "notes.txt";
let hints = KeyHintBar::new()
    .annotated(LIST_KEYS, 1, |key| match key {
        ListKey::Open => Some(Annotation::new().verb("edit").detail(file)),
        ListKey::Delete => Some(Annotation::new().because("read-only")),
        ListKey::Up | ListKey::Down => Some(Annotation::new()),
    })
    .annotated(PROGRAM_KEYS, 1, |key| match key {
        ProgramKey::Edit => Some(Annotation::new().detail(file)),
        ProgramKey::Reload | ProgramKey::Quit => Some(Annotation::new()),
    });
assert_eq!(
    hints.text(),
    "↑/↓ move · Enter/e edit notes.txt · d delete: read-only · r reload · Ctrl-Q quit"
);

let bar = StatusBar::new(&theme)
    .scope([Span::raw("[files] ")])
    .segment(Segment::new("sort:name").rank(4))
    .hints(&hints)
    .trail([Span::raw("  net:off")]);
assert_eq!(
    bar.line(120).to_string(),
    "[files] sort:name · ↑/↓ move · Enter/e edit notes.txt · d delete: read-only · \
     r reload · Ctrl-Q quit  net:off"
);
assert_eq!(
    bar.line(70).to_string(),
    "[files] sort:name · Enter/e edit notes.txt · Ctrl-Q quit …  net:off"
);
assert_eq!(
    bar.line(60).to_string(),
    "[files] Enter/e edit notes.txt · Ctrl-Q quit …  net:off"
);
let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
bar.render(buf.area, &mut buf);

// An overlay on top has the keys: its own footer, whole.
let menu = KeyHintBar::new().footer(&Footer::new(MENU_KEYS), 7);
assert!(menu.text().ends_with("Esc close"));

// A message takes the bar's place; the scope, the line, and the trail go.
let failed = Feedback::error("could not read notes.txt");
let shown = StatusBar::new(&theme).message(Some(&failed));
assert_eq!(shown.line(20).to_string(), "could not read note…");
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `KeyHintBar::new()` | a bar with no hints | |
| `.table(table, rank)` | every shown row of `table` and its notes as hints, as written | included tables spliced in; a row with no rank holds on at `rank` |
| `.annotated(table, rank, annotate)` | the rows `annotate` keeps, as it says | `annotate(&action)` returns `None` to leave a row out, or an `Annotation`; notes stay |
| `.footer(&footer, rank)` | a control's footer, each segment a hint, whole | what `Overlay::hints` returns; a segment never joins another; a segment with no rank holds on at `rank`, and a note (no keys) a step below, so it gives way before the keys |
| `.keymap(&keymap, active, &cx, rank, annotate)` | the bindings of a scoped keymap with a verb, in the scopes `active`, as `annotate` says | each names its first key; left out when `annotate` returns `None`, when it cannot act in `cx`, or when a scope looked up before its own takes the key; joins and ranks as `annotated` |
| `.hints()`, `.keys()`, `.text()`, `.is_empty()` | the hints, every key they name, the hints as one line | `keys` is what a check presses |
| `.segments(&theme)` | the hints as `PriorityLine` segments with their ranks | `muted`; a hint that cannot act in `keys::unavailable_style` |
| `KeyHint::text`, `keys`, `rank`, `is_available` | one hint | |
| `Annotation::new()` | the row as its table writes it | |
| `.verb(v)`, `.detail(d)` | another verb; words after the verb | |
| `.because(reason)`, `.unavailable()` | the key cannot act here: dimmed, with `: reason` after the words | the key still resolves; its scope says why when it is pressed |
| `.rank(n)` | another rank | |
| `.words(verb)`, `.is_available()` | what the annotation makes of a row with `verb` | |
| `StatusBar::new(&theme)` | an empty bar | its line is a `PriorityLine` with the default separator and mark |
| `.lead(spans)` | spans first, in both modes | never give way |
| `.scope(spans)` | spans after the lead, with the keys only | a tag naming whose keys they are, a mark |
| `.segment(segment)` | one of the host's segments on the line | with its rank |
| `.hints(&bar)` | the hints on the line, after the segments so far | |
| `.trail(spans)` | spans at the end, with the keys only | |
| `.message(Option<&Feedback>)` | show a message instead of the scope, the line, and the trail | in its severity's style, cut with `…` |
| `.shows_message()` | whether it shows a message | |
| `.spans(width)`, `.line(width)`, `.render(area, buf)` | the bar in `width` cells; drawn on the first row of `area` | only a lead and scope wider than `width` make it wider |

### How rows become hints

1. Rows are read in table order, included tables spliced in. An alias is
   never a hint; a note is a hint with no key.
2. A shown row the host keeps becomes `labels words`: its label, and its
   verb (the annotation's, or the table's), the detail, and `: reason`.
3. A row whose words a keyed hint of the bar already has joins it: its
   label after the others (`Enter/e edit notes.txt`), its key added, the
   higher rank, available when either is. This holds across tables, so
   one grammar covers the whole bar. A segment taken whole from a footer
   joins nothing.
4. A hint's rank is the annotation's, else the row's, else the table's.
5. The bar's line fits by `PriorityLine`'s rule: lowest rank first, the
   later of a tie first, the room filled back, ` …` when something went.

## Key table

None of its own: the bar handles no key. It names the keys of the tables
the host gives it, whose own rules (`keys`) hold: every label is
`key_label`'s, a shown key resolves to its row, characters bind as typed.

## Theme tokens

| Token | Used for |
| --- | --- |
| `muted` | the hints, and the line's separator and mark |
| `inert` with `muted` | a hint that cannot act now (`keys::unavailable_style`) |
| `info`, `success`, `warning`, `error` | a message, in its severity's color (`Feedback::span`) |

The host's lead, scope, segments, and trail keep the styles it gives them
(often a token, a named style, or a `StatusChip`'s tone).

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 8 | Footers and hints | every hint is generated from a key table in the footer's grammar; a control's footer goes in whole; the host words what a key does here through annotations on rows, never as free text |
| 11 | Feedback severity | a message is drawn in its severity's color |
| 13 | Theme | takes `&Theme`; hints in `muted`, a hint that cannot act in `inert` with `muted`; no color is named |
| 14 | Width | widths in terminal cells; hints and segments give way whole and the line ends in `marks::MORE`; a message is cut with `…` |

Not applicable: 1, 2, and 10 (no list or cursor), 3 to 7, 12, 15, 16, and
18 (no keys of its own, no overlay, no editing, no paging), 9 (the focus cue
is the pane frame's; a scope tag is the host's span), and 17 (the words are
the host's).

## Seams left open

- **Grouping a keymap's hints.** `keymap` names bindings in the keymap's
  order, which is the help page's. A host that wants the bar in another
  order (what acts on the selection, then the program's keys) calls it once
  per group with an `annotate` that keeps that group, as the hints join
  across calls; a group on each binding would let one call do it.
- **Showing why a dimmed key cannot act.** `because` puts the reason in the
  hint. A narrow bar could keep the dimmed key and drop its reason first.
- **Clicking a hint.** `PriorityLine::fit` reports the kept segments, and
  each hint names its keys, which is what a mouse hit test would press.
- **Right-aligned hints.** Hints that hug the right edge would be a second
  line laid out in the cells this one leaves.
