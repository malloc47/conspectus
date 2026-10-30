# `AppFrame`

| | |
| --- | --- |
| Module | `tuikit::app_frame` |
| Layer | 1 (chrome control) |
| Type parameters | none; the host's screen is any type that implements `Screen`, whose `Report` is what the body's draw measured |
| Cargo feature | none |
| Outcomes | none: the frame handles no keys; drawing returns the body's report, or the report's default when an overlay hid the body |

## Purpose

`AppFrame` is the frame a full-screen program's screen is drawn in. Every
frame of such a program draws the same layers in the same order, whatever
it shows:

1. **The chrome**: a header row at the top, a footer row at the bottom (a
   status line, key hints), and the body between them, which holds the
   program's panes.
2. **The overlays**, over the whole screen, from the bottom of the host's
   modal stack to its top, each clearing what it covers. While an open
   overlay fills the screen, the chrome is not drawn at all: nothing of it
   would show.
3. **The toasts**, last, over everything, so a report shows over a
   full-screen overlay too.

The host supplies what each part draws by implementing `Screen`, usually
on a small adapter over its own state; the frame works out the areas and
calls the parts in that order. `AppFrame::areas` gives the same layout to
anything that needs one part's cells again: a dev tool that prints one
region of a frame, a check that reads one pane.

The body's draw returns what it measured (the windows its scrolled panes
showed), and `render` hands that back to the caller, which keeps it for
the next key and the next frame. The frame sends no message and keeps no
state, so a draw stays a function of the host's state and the area.

It replaces a draw function each program writes for itself: a vertical
split into three rows, a branch that skips the chrome under a full-screen
overlay, the stack's draw, and the toasts, in an order a later change
could get wrong, with a copy of the row split wherever a tool needs a
region. The host keeps what the header and footer say, which view fills
the body (and how the body splits, usually with a
[`SplitPane`](frame.md#splitpane)), what its overlays are, and its toasts.

## Usage example

A program with a header, two panes, a status line, a modal stack whose
entry may fill the screen, and toasts. The adapter borrows the program's
parts so the stack can be drawn with the program's state as its context.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget};
use std::time::Instant;
use tuikit::app_frame::{AppFrame, Screen};
use tuikit::feedback::{Feedback, Toasts};
use tuikit::frame::{PaneFrame, SplitPane};
use tuikit::overlay::{ModalStack, Outcome, Overlay};
use tuikit::testkit::buffer::rows;
use tuikit::theme::Theme;

/// What the program knows.
struct State {
    left_focused: bool,
}

/// A page that covers the whole screen.
struct Page;

impl Overlay<(), State> for Page {
    fn handle(&mut self, _key: KeyEvent, _cx: &State) -> Outcome<()> {
        Outcome::Close
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &State, _theme: &Theme) {
        Paragraph::new("a page").render(area, buf);
    }

    fn fills_screen(&self) -> bool {
        true
    }
}

struct Program {
    theme: Theme,
    state: State,
    modals: ModalStack<Box<dyn Overlay<(), State>>>,
    toasts: Toasts,
}

impl Screen for Program {
    /// The body reports the panes it drew.
    type Report = Option<[Rect; 2]>;

    fn header(&mut self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("3 items").render(area, buf);
    }

    fn body(&mut self, area: Rect, buf: &mut Buffer) -> Self::Report {
        let panes = SplitPane::new(40).areas(area);
        let focus = [self.state.left_focused, !self.state.left_focused];
        for ((pane, title), focused) in panes.into_iter().zip(["list", "detail"]).zip(focus) {
            PaneFrame::new(&self.theme)
                .title(title)
                .focused(focused)
                .render(pane, buf);
        }
        Some(panes)
    }

    fn footer(&mut self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Tab focus · q quit").render(area, buf);
    }

    fn overlays_fill_screen(&self) -> bool {
        self.modals.fills_screen()
    }

    fn overlays(&mut self, area: Rect, buf: &mut Buffer) {
        self.modals.draw(area, buf, &self.state, &self.theme);
    }

    fn toasts(&mut self, area: Rect, buf: &mut Buffer) {
        self.toasts.render(area, buf, &self.theme);
    }
}

let mut program = Program {
    theme: Theme::default(),
    state: State { left_focused: true },
    modals: ModalStack::new(),
    toasts: Toasts::new(),
};
let frame = AppFrame::new();
let area = Rect::new(0, 0, 60, 10);

// The chrome, the panes the body reports, and the areas a tool reads.
let mut buf = Buffer::empty(area);
let drawn = frame.render(area, &mut buf, &mut program);
assert_eq!(drawn, Some([Rect::new(0, 1, 30, 8), Rect::new(30, 1, 30, 8)]));
let areas = frame.areas(area);
assert_eq!((areas.header.height, areas.body, areas.footer.y), (1, Rect::new(0, 1, 60, 8), 9));
assert!(rows(&buf)[1].starts_with("┌ ► list "));

// A page that fills the screen hides the chrome; a toast still shows.
program.modals.push(Box::new(Page));
program.toasts.show(Feedback::success("saved"), Instant::now());
let mut buf = Buffer::empty(area);
assert_eq!(frame.render(area, &mut buf, &mut program), None);
let text = rows(&buf);
assert!(text[0].starts_with("a page"));
assert!(!text.iter().any(|row| row.contains("Tab focus")));
assert!(text.iter().any(|row| row.contains("saved")));
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `AppFrame::new()` | a frame with a one-row header and a one-row footer | also `Default` |
| `.header_rows(rows)`, `.footer_rows(rows)` | set the header's and the footer's rows | 0 leaves the screen without one |
| `AppFrame::MIN_BODY` | the rows the body keeps on a short screen | 3; the header and the footer give way first |
| `.areas(area)` | where the parts go | an `AppAreas { header, body, footer }`, top to bottom |
| `.render(area, buf, &mut screen)` | draws the layers in order | returns the body's report, or its default when an overlay filled the screen |
| `Screen::Report` | what the body measured | `Default` is what a hidden body reports |
| `Screen::body(area, buf)` | draws the body | the only part a host must supply |
| `Screen::header`, `footer` | draw the header and the footer rows | nothing unless implemented |
| `Screen::overlays_fill_screen()` | whether an open overlay covers the screen | `false` unless implemented; usually `ModalStack::fills_screen` |
| `Screen::overlays(area, buf)` | draws the overlays over the whole area | usually `ModalStack::draw`, bottom to top |
| `Screen::toasts(area, buf)` | draws the toasts over everything | usually `Toasts::render` |

### The layers

`render` clips the area to the buffer and then:

1. unless `overlays_fill_screen()`, draws the header, the body (keeping
   its report), and the footer in the areas `areas` gives;
2. calls `overlays` with the whole area;
3. calls `toasts` with the whole area;
4. returns the body's report, or `Report::default()` when the body was
   not drawn.

The header and the footer take their rows from the top and the bottom;
the body takes the rest. On a screen shorter than the header, the
footer, and `MIN_BODY` together, the body keeps its rows first: at four
rows the body has three and the header one, and at three or fewer the
body has them all.

A host whose stack lives in its own state, beside the rest of it, lends
the stack out for the overlays' call so the entries can read the state
as their context (see [overlay.md](overlay.md)); the frame only says when
the call happens.

## Key table

None: the frame handles no keys. The panes, the overlays, and the host's
keymap own every key; the frame only draws what they leave on the screen.

## Theme tokens

None: the frame draws nothing of its own. Each part styles itself from the
theme the host passes it: the panes their frames
([frame.md](frame.md#paneframe)), the overlays their own frames, the toasts
their borders by severity.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 9 | Focus cue | The frame draws nothing over the panes; the body draws each pane in a `PaneFrame` whose focus the host passes, so the focused pane is the one framed in the focus accent |
| 13 | Theme | The frame names no color; every part reads the theme the host passes it |

Not applicable: 1 to 8, 10 to 12, and 14 to 18 govern lists, overlays,
keys, feedback, text, and pagers, which the parts the frame calls follow;
the frame only places them and orders their draws.

## Seams left open

- **A minimum screen.** Below `MIN_BODY` plus the header and footer, the
  parts get what is left. A "the screen is too small" body the frame draws
  itself under a size the host names would be a parameter here: one
  centered `EmptyState` ([empty_state.md](empty_state.md)) over the whole
  area, saying how much room the screen needs, in place of every layer.
- **More chrome rows.** A tab bar or a second status row is a header or
  footer of more rows today; named rows (a tab strip, a hint bar) would be
  further parts of `Screen`, each with its area in `AppAreas`.
- **A dimmed screen under a modal.** The chrome draws as it is under an
  overlay. A host-chosen look for the chrome while a modal is open (dimmed,
  or with no focus cue) would be a flag the frame passes the parts.
- **Mouse hit testing.** `areas` is the layout a click is tested against;
  a click in the body goes to the panes the host's split placed.
