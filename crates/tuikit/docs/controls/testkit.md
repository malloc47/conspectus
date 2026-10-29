# `testkit`

| | |
| --- | --- |
| Module | `tuikit::testkit` |
| Layer | 2 (convention) |
| Type parameters | `S`: the host state a case opens (its model with one modal on top), through the `Rig` trait's `State`; `R`: the host's `Rig` |
| Cargo feature | `testkit` (off by default; a host enables it from its dev-dependencies) |
| Outcomes | none: the kit drives and inspects, and its checks return failures or panic with them |

## Purpose

The test kit is what a host's tests and dev tools use to drive the screens
the framework draws and to check them. It replaces what each program grows
test file by test file: a key helper in every file, a hand-rolled ANSI
writer and stripper, a scanner per hygiene test, goldens that keep the
characters of a frame and drop its colors, and convention checks written
against one program's model with the list of its modals kept in several
places.

- **`keys`**: the key-script notation and single presses.
- **`render`**: `FrameDriver`, the one-frame driver over the host's draw
  callback, which can press a run of keys with a frame drawn before each.
- **`buffer`**: reading a frame (text, token columns, the cell and style
  under a piece of text, matching cells, a modal's frame and the text in
  it, the rows a selection repainted).
- **`ansi`**: a frame as ANSI text with its styles; `strip_ansi` (from
  `tuikit::text`) takes them out.
- **`golden`**: a style-aware text form of a frame and a check against a
  golden file.
- **`conformance`**: `assert_conventions` over a host's `Registry` of
  modals.
- **`source`**: the source-scan helpers hygiene tests build on.

The host keeps what a key means, how its screen is drawn and its keys
routed (its `Rig`), which modals it has and what each needs the checks to
know, its golden files and the writer that accepts them, and the rules its
hygiene tests enforce with the scan helpers.

## Usage example

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tuikit::frame::{Lines, ModalFrame, ModalSize};
use tuikit::keys::{self, Convention, EnterMeans, MENU_KEYS, MenuKey, SpaceMeans};
use tuikit::overlay::{ModalStack, Outcome, Overlay};
use tuikit::testkit::conformance::{self, Case, Registry, Rig};
use tuikit::testkit::{buffer, golden, keys as script, render};
use tuikit::theme::Theme;

// Key scripts: the notation a dev tool or a test writes presses in.
let presses = script::parse("j<Tab><C-r>").expect("a valid script");
assert_eq!(presses[2], script::ctrl('r'));

// One frame off screen, read back as text, and as a golden that keeps
// color: the two frames below have the same text, but not the same golden.
let draw = |color| {
    let word = Span::styled("ok", Style::new().fg(color));
    render::render_widget(Paragraph::new(word), 4, 1)
};
assert_eq!(buffer::text(&draw(Color::Green)), buffer::text(&draw(Color::Red)));
assert!(golden::compare(&golden::styled(&draw(Color::Green)), &golden::styled(&draw(Color::Red))).is_err());

// The conformance suite over a host with one menu.
struct Menu;

impl Overlay<(), ()> for Menu {
    fn handle(&mut self, key: KeyEvent, (): &()) -> Outcome<()> {
        match keys::resolve(MENU_KEYS, key) {
            Some(MenuKey::Abort | MenuKey::Close) => Outcome::Close,
            Some(MenuKey::Pick) => Outcome::Commit(()),
            _ => Outcome::Consumed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, (): &(), theme: &Theme) {
        let lines = vec![Line::from("  Open"), Line::from(""), Line::from(keys::footer(MENU_KEYS))];
        ModalFrame::new(theme, ModalSize::MENU)
            .title("Actions")
            .render(area, buf, Lines::new(lines));
    }
}

struct Host(Theme);

impl Rig for Host {
    type State = ModalStack<Menu>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        Paragraph::new("the screen under the modal").render(area, &mut buf);
        stack.draw::<(), ()>(area, &mut buf, &(), &self.0);
        buf
    }

    fn close_top(&self, stack: &mut Self::State) {
        stack.pop();
    }

    fn press(&self, stack: &mut Self::State, key: KeyEvent) {
        if let Some(outcome) = stack.route::<(), ()>(key, &()) {
            let _ = stack.settle(outcome);
        }
    }

    fn depth(&self, stack: &Self::State) -> usize {
        stack.len()
    }

    fn theme(&self) -> &Theme {
        &self.0
    }
}

let registry = Registry::new(Host(Theme::default())).case(
    Case::new("Menu", || {
        let mut stack = ModalStack::new();
        stack.push(Menu);
        stack
    })
    .keys(
        Convention::new("Menu", EnterMeans::Activates, SpaceMeans::Types, ""),
        [conformance::table("MENU_KEYS", MENU_KEYS)],
    )
    .footer(MENU_KEYS),
);
conformance::assert_conventions(&registry);
```

A host's real registry lists every modal it registers, one `Case` each (a
modal with several modes, such as a search, takes one case per mode with a
`variant`), and fails a test of its own when a modal it registers is
missing from it. Run the checks one per test (`assert_conventions_of` with
one `Check`) so they run in parallel.

## Key table

The kit binds no keys. Its key-script notation:

| Script | Press |
| --- | --- |
| `j`, `?`, `1` | That character, no modifier |
| `G` | An uppercase ASCII letter carries Shift, as a keyboard reports it |
| `<Up>` `<Down>` `<Left>` `<Right>` | The arrows |
| `<Enter>` `<Esc>` `<Tab>` `<BackTab>` `<Space>` `<Backspace>` | Those keys |
| `<Delete>` `<Insert>` `<Home>` `<End>` `<PageUp>` `<PageDown>` | Those keys |
| `<F1>` to `<F12>` | The function keys |
| `<<>`, `<v>` | One character in brackets is that character |
| `<C-x>`, `<A-x>`, `<S-Tab>` | Control, Alt, and Shift prefixes; they nest (`<C-A-Delete>`) |

An unclosed `<` or an unknown name is a `ScriptError` whose message says
which. The conformance suite presses `Ctrl-C`, `Esc`, and `q` to check the
close rules, and the down and up arrows to move a selection.

## Theme tokens

The kit draws nothing. The `Title` check reads `accent` (the frame's
corners and the title must be in it) and `badge` (title cells in that look
are chips and keep their own style); the host's `Rig` supplies the theme.

| Token | Used for |
| --- | --- |
| `accent` | The color the `Title` check expects of a modal's corners and title |
| `badge` | The look that marks a title cell as a chip the `Title` check leaves alone |

## House rules followed

The conformance suite is how the rules below are checked mechanically for
every modal a host registers.

| # | Rule | How the kit holds a modal to it |
| --- | --- | --- |
| 1 | List cursor and highlight | `Selection`: moving the cursor repaints rows and moves no text outside the two-cell cursor gutter |
| 4 | Close keys | `Close`: `Ctrl-C` and `Esc` close the modal at its first level, and `q` too unless the level types; `Keys`: `Ctrl-C` is bound first, to dismissing |
| 5 | Close feedback | `Close`: closing leaves nothing on the status line and posts no toast |
| 8 | Footers and hints | `Footer`: every segment the modal's table generates is drawn; `Keys`: the table checks (labels, unresolved keys) |
| 13 | Theme | `Title`: the frame and title take the theme's `accent`; the kit's own code reads no raw style except the ANSI writer, which encodes the colors it is given |
| 14 | Width | `Words`: no row ends at the frame's edge in the start of a word the modal shows whole on a wide screen; `Contained`: nothing escapes the frame at any of ten sizes |

Not applicable: 2, 3, 6, 7, 9 to 12, and 15 to 18 are each control's own
behavior, which its tests check; the kit's suite covers the rules every
modal shares. Rule 2's wrap is not checked by the suite because a host's
list may be scrollable content that clamps.

## Seams left open

- **More checks over the same registry.** A check that every list wraps or
  clamps by its kind, that paging keys move by the viewport, or that text a
  modal draws contains no developer artifacts can run over the cases a host
  already registers; the `Check` enum grows with them.
- **A preview gallery.** A case opens a modal from nothing, which is what a
  gallery entry needs; a gallery could take its entries from the same
  registry.
- **Documentation tables.** A helper that compares a table in a document
  with the set of values the code defines (a glyph table, a word list)
  belongs here when a second user appears; the host keeps its own today.
- **Underline color.** Goldens and the ANSI writer leave an underline color
  out; a host that draws one can add it to both.
