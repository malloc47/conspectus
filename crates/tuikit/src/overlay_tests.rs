use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::widgets::{Clear, Widget};

use super::*;
use crate::frame::{Lines, ModalFrame, ModalSize};
use crate::testkit::keys::press;
use crate::theme::Theme;

/// The host context the test overlays read: a word that changes between
/// keys, so a test can tell a live read from a copy.
struct Shelf {
    word: &'static str,
}

/// A test overlay: a rect it clears and fills with its mark (`#` while a
/// step of its own is open over it), and keys that say what it asks for.
/// `Enter` commits the context's word, `s` stays with its own mark, `b`
/// steps back with it, `Esc` closes, `d` dismisses, anything else is
/// used.
#[derive(Debug, Clone, PartialEq)]
struct Tile {
    mark: &'static str,
    rect: Rect,
    fills: bool,
    keys: usize,
    drawn: usize,
    under: usize,
}

impl Tile {
    fn new(mark: &'static str, rect: Rect) -> Self {
        Self {
            mark,
            rect,
            fills: false,
            keys: 0,
            drawn: 0,
            under: 0,
        }
    }

    fn full(mark: &'static str) -> Self {
        Self {
            fills: true,
            ..Self::new(mark, Rect::default())
        }
    }
}

impl Overlay<String, Shelf> for Tile {
    fn handle(&mut self, key: KeyEvent, shelf: &Shelf) -> Outcome<String> {
        self.keys += 1;
        match key.code {
            KeyCode::Enter => Outcome::Commit(shelf.word.to_string()),
            KeyCode::Char('s') => Outcome::Stay(self.mark.to_string()),
            KeyCode::Char('b') => Outcome::Back(self.mark.to_string()),
            KeyCode::Esc => Outcome::Close,
            KeyCode::Char('d') => Outcome::Dismiss,
            _ => Outcome::Consumed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _shelf: &Shelf, _theme: &Theme) {
        self.drawn += 1;
        let rect = if self.fills { area } else { self.rect };
        if !self.fills {
            Clear.render(rect, buf);
        }
        for y in rect.top()..rect.bottom() {
            for x in rect.left()..rect.right() {
                buf[(x, y)].set_symbol(self.mark);
            }
        }
    }

    fn render_under_step(&mut self, area: Rect, buf: &mut Buffer, shelf: &Shelf, theme: &Theme) {
        self.render(area, buf, shelf, theme);
        self.under += 1;
        let rect = if self.fills { area } else { self.rect };
        for y in rect.top()..rect.bottom() {
            for x in rect.left()..rect.right() {
                buf[(x, y)].set_symbol("#");
            }
        }
    }

    fn fills_screen(&self) -> bool {
        self.fills
    }
}

fn row(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

fn draw(stack: &mut ModalStack<Tile>, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    for y in 0..height {
        for x in 0..width {
            buf[(x, y)].set_symbol(".");
        }
    }
    stack.draw(area, &mut buf, &Shelf { word: "w" }, &Theme::default());
    buf
}

#[test]
fn an_empty_stack_routes_nothing_and_draws_nothing() {
    let mut stack: ModalStack<Tile> = ModalStack::new();
    assert_eq!(
        stack.route(press(KeyCode::Enter), &Shelf { word: "w" }),
        None
    );
    let buf = draw(&mut stack, 4, 1);
    assert_eq!(row(&buf, 0), "....");
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
}

#[test]
fn a_key_goes_to_the_top_entry_only() {
    let mut stack = ModalStack::new();
    stack.push(Tile::new("a", Rect::new(0, 0, 4, 1)));
    stack.push(Tile::new("b", Rect::new(1, 0, 2, 1)));
    let outcome = stack.route(press(KeyCode::Char('j')), &Shelf { word: "w" });
    assert_eq!(outcome, Some(Outcome::Consumed));
    let keys: Vec<usize> = stack.iter().map(|tile| tile.keys).collect();
    assert_eq!(keys, [0, 1], "the bottom entry saw nothing");
    assert_eq!(stack.top().map(|tile| tile.mark), Some("b"));
}

#[test]
fn handling_reads_the_context_as_it_is_now() {
    let mut stack = ModalStack::new();
    stack.push(Tile::new("a", Rect::default()));
    let mut shelf = Shelf { word: "first" };
    assert_eq!(
        stack.route(press(KeyCode::Enter), &shelf),
        Some(Outcome::Commit("first".to_string()))
    );
    shelf.word = "second";
    assert_eq!(
        stack.route(press(KeyCode::Enter), &shelf),
        Some(Outcome::Commit("second".to_string())),
        "no copy of the context was kept"
    );
}

#[test]
fn settling_closes_the_top_for_close_and_commit_only() {
    let shelf = Shelf { word: "w" };
    let mut stack = ModalStack::new();
    stack.push(Tile::new("a", Rect::default()));
    stack.push(Tile::new("b", Rect::default()));

    let consumed = stack.route(press(KeyCode::Char('j')), &shelf).unwrap();
    let settled = stack.settle(consumed);
    assert_eq!((settled.closed, settled.message), (Vec::new(), None));
    assert_eq!(stack.len(), 2);

    let stay = stack.route(press(KeyCode::Char('s')), &shelf).unwrap();
    let settled = stack.settle(stay);
    assert!(settled.closed.is_empty(), "a stay leaves the entry open");
    assert_eq!(settled.message.as_deref(), Some("b"));

    let close = stack.route(press(KeyCode::Esc), &shelf).unwrap();
    let settled = stack.settle(close);
    assert_eq!(marks(&settled.closed), ["b"]);
    assert_eq!(settled.message, None, "closing is silent");

    let commit = stack.route(press(KeyCode::Enter), &shelf).unwrap();
    let settled = stack.settle(commit);
    assert_eq!(marks(&settled.closed), ["a"]);
    assert_eq!(settled.message.as_deref(), Some("w"));
    assert!(stack.is_empty());
}

fn marks(tiles: &[Tile]) -> Vec<&'static str> {
    tiles.iter().map(|tile| tile.mark).collect()
}

/// A stack with `a` on its own, then `b` with its step `c` over it.
fn flow() -> ModalStack<Tile> {
    let mut stack = ModalStack::new();
    stack.push(Tile::new("a", Rect::default()));
    stack.push(Tile::new("b", Rect::default()));
    stack.push_step(Tile::new("c", Rect::default()));
    stack
}

#[test]
fn closing_a_step_gives_the_keys_back_to_the_entry_under_it() {
    let shelf = Shelf { word: "w" };
    let mut stack = flow();
    assert!(stack.top_is_step());
    let close = stack.route(press(KeyCode::Esc), &shelf).unwrap();
    let settled = stack.settle(close);
    assert_eq!(marks(&settled.closed), ["c"], "only the step closed");
    assert_eq!(stack.top().map(|tile| tile.mark), Some("b"));
    assert!(!stack.top_is_step(), "b opened on its own");
    stack.route(press(KeyCode::Char('j')), &shelf);
    assert_eq!(stack.top().map(|tile| tile.keys), Some(1), "b has the keys");
}

#[test]
fn a_commit_or_a_dismissal_closes_the_whole_flow_and_nothing_under_it() {
    let shelf = Shelf { word: "w" };
    for (key, message) in [
        (press(KeyCode::Enter), Some("w")),
        (press(KeyCode::Char('d')), None),
    ] {
        let mut stack = flow();
        let outcome = stack.route(key, &shelf).unwrap();
        let settled = stack.settle(outcome);
        assert_eq!(marks(&settled.closed), ["c", "b"], "{key:?}: top first");
        assert_eq!(settled.message.as_deref(), message, "{key:?}");
        assert_eq!(marks(stack.iter().as_slice()), ["a"], "{key:?}");
    }
}

#[test]
fn stepping_back_with_a_message_closes_the_step_only() {
    let shelf = Shelf { word: "w" };
    let mut stack = flow();
    let back = stack.route(press(KeyCode::Char('b')), &shelf).unwrap();
    assert_eq!(back, Outcome::Back("c".to_string()));
    let settled = stack.settle(back);
    assert_eq!(marks(&settled.closed), ["c"], "only the step closed");
    assert_eq!(settled.message.as_deref(), Some("c"), "with its message");
    assert_eq!(marks(stack.iter().as_slice()), ["a", "b"]);
    // An entry that is no step closes alone, as a commit closes it.
    let back = stack.route(press(KeyCode::Char('b')), &shelf).unwrap();
    let settled = stack.settle(back);
    assert_eq!(marks(&settled.closed), ["b"]);
    assert_eq!(settled.message.as_deref(), Some("b"));
    assert_eq!(marks(stack.iter().as_slice()), ["a"]);
}

#[test]
fn an_entry_under_a_step_of_its_own_draws_as_under_a_step() {
    let mut stack = ModalStack::new();
    stack.push(Tile::new("a", Rect::new(0, 0, 8, 1)));
    stack.push(Tile::new("b", Rect::new(1, 0, 6, 1)));
    stack.push_step(Tile::new("c", Rect::new(3, 0, 2, 1)));
    let buf = draw(&mut stack, 8, 1);
    assert_eq!(row(&buf, 0), "a##cc##a", "b is drawn under its step");
    let under: Vec<usize> = stack.iter().map(|tile| tile.under).collect();
    assert_eq!(under, [0, 1, 0], "a has no step over it, c is on top");
    // An overlay opened on its own over another is no step of it.
    stack.pop();
    stack.push(Tile::new("d", Rect::new(3, 0, 2, 1)));
    let buf = draw(&mut stack, 8, 1);
    assert_eq!(row(&buf, 0), "abbddbba");
}

#[test]
fn a_flow_runs_through_every_step_of_it() {
    let shelf = Shelf { word: "w" };
    let mut stack = flow();
    stack.push_step(Tile::new("e", Rect::default()));
    let dismiss = stack.route(press(KeyCode::Char('d')), &shelf).unwrap();
    assert_eq!(marks(&stack.settle(dismiss).closed), ["e", "c", "b"]);
    // An overlay opened on its own over a flow is not part of it.
    let mut stack = flow();
    stack.push(Tile::new("f", Rect::default()));
    assert!(!stack.top_is_step());
    let dismiss = stack.route(press(KeyCode::Char('d')), &shelf).unwrap();
    assert_eq!(marks(&stack.settle(dismiss).closed), ["f"]);
    assert_eq!(marks(&stack.close_flow()), ["c", "b"]);
    assert_eq!(marks(&stack.close_flow()), ["a"]);
    assert!(stack.close_flow().is_empty());
}

#[test]
fn a_dismissal_closes_an_entry_that_is_no_step_as_a_close_does() {
    let shelf = Shelf { word: "w" };
    let mut stack = ModalStack::new();
    stack.push(Tile::new("a", Rect::default()));
    stack.push(Tile::new("b", Rect::default()));
    let dismiss = stack.route(press(KeyCode::Char('d')), &shelf).unwrap();
    assert_eq!(marks(&stack.settle(dismiss).closed), ["b"]);
    assert_eq!(stack.len(), 1);
}

#[test]
fn a_step_on_an_empty_stack_opens_as_an_overlay_of_its_own() {
    let mut stack = ModalStack::new();
    stack.push_step(Tile::new("a", Rect::default()));
    assert!(!stack.top_is_step());
    stack.push(Tile::new("b", Rect::default()));
    stack.push_step(Tile::new("c", Rect::default()));
    // Popping keeps every entry's place in its flow.
    assert_eq!(stack.pop().map(|tile| tile.mark), Some("c"));
    assert!(!stack.top_is_step());
    stack.push_step(Tile::new("d", Rect::default()));
    let popped = stack.pop_if(|tile| tile.mark == "d");
    assert_eq!(popped.map(|tile| tile.mark), Some("d"));
    assert_eq!(marks(&stack.close_all()), ["b", "a"]);
    stack.push_step(Tile::new("e", Rect::default()));
    assert!(!stack.top_is_step(), "close_all forgot the old steps");
}

#[test]
fn outcomes_say_whether_they_close_and_what_they_carry() {
    let dismiss: Outcome<&str> = Outcome::Dismiss;
    assert!(dismiss.closes());
    assert_eq!(dismiss.message(), None);
    assert_eq!(dismiss.map(str::len), Outcome::Dismiss);
    assert!(!Outcome::Stay("s").closes());
    assert!(Outcome::<()>::Close.closes());
    let back = Outcome::Back("b");
    assert!(back.closes());
    assert_eq!(back.map(str::len), Outcome::Back(1));
    assert_eq!(back.message(), Some("b"));
}

#[test]
fn every_entry_draws_bottom_to_top() {
    let mut stack = ModalStack::new();
    stack.push(Tile::new("a", Rect::new(1, 0, 6, 1)));
    stack.push(Tile::new("b", Rect::new(3, 0, 2, 1)));
    let buf = draw(&mut stack, 8, 1);
    assert_eq!(
        row(&buf, 0),
        ".aabbaa.",
        "the lower entry shows around the upper"
    );
    assert!(stack.iter().all(|tile| tile.drawn == 1));
}

#[test]
fn an_entry_that_fills_the_screen_hides_everything_under_it() {
    let mut stack = ModalStack::new();
    stack.push(Tile::new("a", Rect::new(0, 0, 8, 1)));
    stack.push(Tile::full("f"));
    stack.push(Tile::new("b", Rect::new(3, 0, 2, 1)));
    assert!(stack.fills_screen());
    let buf = draw(&mut stack, 8, 1);
    assert_eq!(row(&buf, 0), "fffbbfff");
    let drawn: Vec<usize> = stack.iter().map(|tile| tile.drawn).collect();
    assert_eq!(
        drawn,
        [0, 1, 1],
        "nothing under the full-screen entry is drawn"
    );
}

#[test]
fn a_full_screen_entry_draws_on_a_cleared_area() {
    // An entry that fills the screen may leave cells unwritten; what the
    // host drew before the stack must not show in them.
    struct Sparse;
    impl Overlay<(), ()> for Sparse {
        fn handle(&mut self, _key: KeyEvent, _cx: &()) -> Outcome<()> {
            Outcome::Consumed
        }
        fn render(&mut self, _area: Rect, buf: &mut Buffer, _cx: &(), _theme: &Theme) {
            buf[(0, 0)].set_symbol("s");
        }
        fn fills_screen(&self) -> bool {
            true
        }
    }
    let area = Rect::new(0, 0, 3, 1);
    let mut buf = Buffer::with_lines(["xyz"]);
    let mut stack: ModalStack<Sparse> = ModalStack::new();
    stack.push(Sparse);
    stack.draw(area, &mut buf, &(), &Theme::default());
    assert_eq!(row(&buf, 0), "s  ");
}

#[test]
fn a_framed_modal_over_another_shows_both_frames() {
    // Two real modal frames: the menu's border stays in view around the
    // smaller prompt drawn over it.
    struct Framed {
        size: ModalSize,
        title: &'static str,
    }
    impl Overlay<(), ()> for Framed {
        fn handle(&mut self, _key: KeyEvent, _cx: &()) -> Outcome<()> {
            Outcome::Consumed
        }
        fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &(), theme: &Theme) {
            let lines = (0..8).map(|n| format!("row {n}").into()).collect();
            ModalFrame::new(theme, self.size).title(self.title).render(
                area,
                buf,
                Lines::new(lines),
            );
        }
    }
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut stack: ModalStack<Framed> = ModalStack::new();
    stack.push(Framed {
        size: ModalSize::FORM,
        title: "Below",
    });
    stack.push(Framed {
        size: ModalSize::COMPACT,
        title: "Above",
    });
    stack.draw(area, &mut buf, &(), &Theme::default());
    let text: Vec<String> = (0..24).map(|y| row(&buf, y)).collect();
    assert!(text.iter().any(|line| line.contains("Below")), "{text:#?}");
    assert!(text.iter().any(|line| line.contains("Above")), "{text:#?}");
}

#[test]
fn pop_if_closes_only_a_matching_top() {
    let mut stack = ModalStack::new();
    stack.push(Tile::new("a", Rect::default()));
    stack.push(Tile::new("b", Rect::default()));
    assert_eq!(stack.pop_if(|tile| tile.mark == "a"), None);
    assert_eq!(stack.len(), 2);
    assert_eq!(
        stack.pop_if(|tile| tile.mark == "b").map(|t| t.mark),
        Some("b")
    );
    assert_eq!(stack.pop().map(|t| t.mark), Some("a"));
    assert_eq!(stack.pop(), None);
}

#[test]
fn close_all_returns_the_entries_top_first() {
    let mut stack = ModalStack::new();
    for mark in ["a", "b", "c"] {
        stack.push(Tile::new(mark, Rect::default()));
    }
    let closed: Vec<&str> = stack.close_all().into_iter().map(|t| t.mark).collect();
    assert_eq!(closed, ["c", "b", "a"]);
    assert!(stack.is_empty());
}

#[test]
fn a_stack_of_boxed_overlays_routes_and_draws() {
    let mut stack: ModalStack<Box<dyn Overlay<String, Shelf>>> = ModalStack::default();
    stack.push(Box::new(Tile::new("a", Rect::new(0, 0, 2, 1))));
    stack.push(Box::new(Tile::full("f")));
    let shelf = Shelf { word: "w" };
    assert_eq!(
        stack.route(press(KeyCode::Char('s')), &shelf),
        Some(Outcome::Stay("f".to_string()))
    );
    assert!(stack.fills_screen());
    let area = Rect::new(0, 0, 2, 1);
    let mut buf = Buffer::empty(area);
    stack.draw(area, &mut buf, &shelf, &Theme::default());
    assert_eq!(row(&buf, 0), "ff");
    if let Some(top) = stack.top_mut() {
        assert!(top.fills_screen());
    }
}

#[test]
fn an_outcome_maps_its_message_and_says_whether_it_closes() {
    let commit: Outcome<u8> = Outcome::Commit(2);
    assert_eq!(commit.map(|n| n * 2), Outcome::Commit(4));
    assert_eq!(Outcome::<u8>::Stay(1).map(u16::from), Outcome::Stay(1_u16));
    assert_eq!(Outcome::<u8>::Close.map(u16::from), Outcome::Close);
    assert_eq!(Outcome::<u8>::Consumed.map(u16::from), Outcome::Consumed);
    assert!(commit.closes() && Outcome::<u8>::Close.closes());
    assert!(!Outcome::Stay(1).closes() && !Outcome::<u8>::Consumed.closes());
    assert_eq!(commit.message(), Some(2));
    assert_eq!(Outcome::Stay(3).message(), Some(3));
    assert_eq!(Outcome::<u8>::Close.message(), None);
}

#[test]
fn only_the_top_entry_says_whether_a_paste_goes_in() {
    struct Field(bool);
    impl Overlay<(), ()> for Field {
        fn handle(&mut self, _key: KeyEvent, _cx: &()) -> Outcome<()> {
            Outcome::Consumed
        }
        fn render(&mut self, _area: Rect, _buf: &mut Buffer, _cx: &(), _theme: &Theme) {}
        fn takes_text(&self) -> bool {
            self.0
        }
    }
    let mut stack: ModalStack<Field> = ModalStack::new();
    assert!(!stack.takes_text(), "an empty stack takes no paste");
    stack.push(Field(true));
    assert!(stack.takes_text());
    stack.push(Field(false));
    assert!(!stack.takes_text(), "a menu over a prompt takes no paste");
    assert!(
        !Tile::new("a", Rect::default()).takes_text(),
        "false unless said"
    );
}
