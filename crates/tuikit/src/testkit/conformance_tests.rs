//! The suite against a tiny host: a screen of text, a stack of modals, and
//! a status line. Two well-behaved modals pass every check; a broken one,
//! which spills out of its frame, cuts its words, hand-writes its footer,
//! leaves on Enter, ignores Ctrl-C, and says "cancelled" when it closes,
//! fails each check it breaks.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use super::*;
use crate::frame::{Lines, ModalFrame, ModalSize};
use crate::glyphs::marks;
use crate::keys::{
    EnterMeans, Footer, KeyRow, MENU_KEYS, MenuKey, SpaceMeans, TEXT_INPUT_KEYS, TextInputKey,
    alias, key, shown, typed_char,
};
use crate::overlay::{ModalStack, Outcome, Overlay};
use crate::selection::{self, Cursor};

/// The host's state: its modals and its status line.
struct Screen {
    stack: ModalStack<Box<dyn Overlay<String, ()>>>,
    status: Option<String>,
}

/// The host's rig. Closing a modal clears the status line; a message a
/// modal sends goes on it.
struct Host {
    theme: Theme,
}

impl Rig for Host {
    type State = Screen;

    fn render(&self, screen: &mut Screen, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        let rows: Vec<Line> = (0..area.height)
            .map(|row| Line::from(format!("the screen under the modals, row {row}")))
            .collect();
        Paragraph::new(rows).render(area, &mut buf);
        if let Some(status) = &screen.status {
            let last = Rect::new(area.x, area.bottom() - 1, area.width, 1);
            Paragraph::new(status.as_str()).render(last, &mut buf);
        }
        screen
            .stack
            .draw::<String, ()>(area, &mut buf, &(), &self.theme);
        buf
    }

    fn close_top(&self, screen: &mut Screen) {
        screen.stack.pop();
    }

    fn press(&self, screen: &mut Screen, key: KeyEvent) {
        let Some(outcome) = screen.stack.route::<String, ()>(key, &()) else {
            return;
        };
        let settled = screen.stack.settle(outcome);
        if settled.closed.is_some() {
            screen.status = None;
        }
        if let Some(message) = settled.message {
            screen.status = Some(message);
        }
    }

    fn depth(&self, screen: &Screen) -> usize {
        screen.stack.len()
    }

    fn feedback(&self, screen: &Screen) -> Vec<String> {
        screen.status.iter().cloned().collect()
    }

    fn note(&self, screen: &mut Screen, text: &str) {
        screen.status = Some(text.to_string());
    }

    fn theme(&self) -> &Theme {
        &self.theme
    }
}

fn open(modal: impl Fn() -> Box<dyn Overlay<String, ()>> + 'static) -> impl Fn() -> Screen {
    move || {
        let mut stack = ModalStack::new();
        stack.push(modal());
        Screen {
            stack,
            status: None,
        }
    }
}

/// A menu that keeps the conventions.
struct Menu {
    items: [&'static str; 3],
    cursor: usize,
}

impl Menu {
    fn boxed() -> Box<dyn Overlay<String, ()>> {
        Box::new(Self {
            items: ["Open the file", "Rename it", "Remove it"],
            cursor: 0,
        })
    }
}

impl Overlay<String, ()> for Menu {
    fn handle(&mut self, key: KeyEvent, (): &()) -> Outcome<String> {
        match crate::keys::resolve(MENU_KEYS, key) {
            Some(MenuKey::Abort | MenuKey::Close) => Outcome::Close,
            Some(MenuKey::Pick) => Outcome::Commit(self.items[self.cursor].to_string()),
            Some(MenuKey::Down) => {
                self.cursor = (self.cursor + 1) % self.items.len();
                Outcome::Consumed
            }
            Some(MenuKey::Up) => {
                self.cursor = (self.cursor + self.items.len() - 1) % self.items.len();
                Outcome::Consumed
            }
            None => Outcome::Consumed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, (): &(), theme: &Theme) {
        let frame = ModalFrame::new(theme, ModalSize::MENU).title("Actions");
        let width = usize::from(frame.inner_width(area));
        let mut lines: Vec<Line> = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let cursor = if index == self.cursor {
                    Cursor::Active
                } else {
                    Cursor::Off
                };
                selection::row([Span::raw(*item)], cursor, theme)
            })
            .collect();
        lines.push(Line::from(""));
        lines.extend(Footer::new(MENU_KEYS).lines(width, theme));
        frame.render(
            area,
            buf,
            Lines::new(lines).following(self.cursor..self.cursor + 1),
        );
    }
}

/// A one-line prompt that keeps the conventions: its level types, so `q`
/// is a letter.
struct Prompt {
    text: String,
}

impl Overlay<String, ()> for Prompt {
    fn handle(&mut self, key: KeyEvent, (): &()) -> Outcome<String> {
        match crate::keys::resolve(TEXT_INPUT_KEYS, key) {
            Some(TextInputKey::Abort | TextInputKey::Cancel) => Outcome::Close,
            Some(TextInputKey::Confirm) => Outcome::Commit(self.text.clone()),
            Some(_) => Outcome::Consumed,
            None => {
                if let Some(c) = typed_char(key) {
                    self.text.push(c);
                }
                Outcome::Consumed
            }
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, (): &(), theme: &Theme) {
        let frame = ModalFrame::new(theme, ModalSize::COMPACT).title("Name");
        let width = usize::from(frame.inner_width(area));
        let mut lines = vec![Line::from(self.text.clone()), Line::from("")];
        lines.extend(Footer::new(TEXT_INPUT_KEYS).lines(width, theme));
        frame.render(area, buf, Lines::new(lines));
    }

    fn takes_text(&self) -> bool {
        true
    }
}

/// What the broken modal's keys do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BrokenKey {
    Close,
    Down,
    Up,
}

impl KeyAction for BrokenKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Close => KeyRole::Leave,
            Self::Down | Self::Up => KeyRole::Move,
        }
    }
}

/// A table with no `Ctrl-C` and an `Enter` that leaves.
const BROKEN_KEYS: &[KeyRow<BrokenKey>] = &[
    shown(key(KeyCode::Esc), BrokenKey::Close, "Esc", "close"),
    shown(key(KeyCode::Enter), BrokenKey::Close, "Enter", "leave"),
    shown(
        key(KeyCode::Down),
        BrokenKey::Down,
        marks::ARROW_DOWN,
        "move",
    ),
    alias(key(KeyCode::Up), BrokenKey::Up),
];

/// A modal that breaks every convention it can.
struct Broken {
    cursor: usize,
}

impl Overlay<String, ()> for Broken {
    fn handle(&mut self, key: KeyEvent, (): &()) -> Outcome<String> {
        match crate::keys::resolve(BROKEN_KEYS, key) {
            // Closing announces itself.
            Some(BrokenKey::Close) => Outcome::Commit("cancelled".to_string()),
            Some(BrokenKey::Down) => {
                self.cursor = (self.cursor + 1) % 3;
                Outcome::Consumed
            }
            Some(BrokenKey::Up) => {
                self.cursor = (self.cursor + 2) % 3;
                Outcome::Consumed
            }
            // Ctrl-C and `q` do nothing.
            None => Outcome::Consumed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, (): &(), _theme: &Theme) {
        let width = area.width.saturating_sub(4).min(100);
        let height = 9.min(area.height.saturating_sub(2));
        let rect = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        Clear.render(rect, buf);
        // A frame of its own, in the default color, with a bold title.
        let bold = Style::new().add_modifier(Modifier::BOLD);
        let block = Block::bordered().title(Span::styled(" Broken ", bold));
        let inner = block.inner(rect);
        block.render(rect, buf);
        // The cursor row moves its text, the long line is clipped by the
        // frame, and the footer is written by hand.
        let mut lines: Vec<Line> = ["alpha", "beta", "gamma"]
            .iter()
            .enumerate()
            .map(|(index, item)| {
                if index == self.cursor {
                    Line::styled(format!("> {item}"), bold)
                } else {
                    Line::from(*item)
                }
            })
            .collect();
        lines.push(Line::from(""));
        lines.push(Line::from(
            "this sentence is long enough that a narrow frame cuts its words in the middle of them",
        ));
        lines.push(Line::from(""));
        lines.push(Line::from("Esc to close, Enter to pick"));
        Paragraph::new(lines).render(inner, buf);
        // And one mark lands under the frame.
        if rect.bottom() < area.bottom() {
            buf[(rect.x + 2, rect.bottom())].set_symbol("!");
        }
    }
}

fn host() -> Host {
    Host {
        theme: Theme::default(),
    }
}

fn menu_convention() -> Convention {
    Convention::new("Menu", EnterMeans::Activates, SpaceMeans::Types, "")
}

fn sound() -> Registry<Host> {
    Registry::new(host())
        .case(
            Case::new("Menu", open(Menu::boxed))
                .moves()
                .keys(menu_convention(), [table("MENU_KEYS", MENU_KEYS)])
                .footer(MENU_KEYS),
        )
        .case(
            Case::new(
                "Prompt",
                open(|| {
                    Box::new(Prompt {
                        text: "draft".to_string(),
                    })
                }),
            )
            .typed()
            .keys(
                Convention::new("Prompt", EnterMeans::Submits, SpaceMeans::Types, ""),
                [table("TEXT_INPUT_KEYS", TEXT_INPUT_KEYS)],
            )
            .footer(TEXT_INPUT_KEYS),
        )
}

fn broken() -> Registry<Host> {
    Registry::new(host()).case(
        Case::new("Broken", open(|| Box::new(Broken { cursor: 0 })))
            .moves()
            .keys(
                Convention::new("Broken", EnterMeans::Activates, SpaceMeans::Types, ""),
                [table("BROKEN_KEYS", BROKEN_KEYS)],
            )
            .footer(BROKEN_KEYS),
    )
}

fn failed(failures: &[Failure], check: Check) -> Vec<&str> {
    failures
        .iter()
        .filter(|failure| failure.check == check)
        .map(|failure| failure.message.as_str())
        .collect()
}

#[test]
fn modals_that_keep_the_conventions_pass_every_check() {
    let failures = check_conventions(&sound());
    assert!(
        failures.is_empty(),
        "{}",
        failures
            .iter()
            .map(Failure::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_conventions(&sound());
    assert_eq!(sound().names(), ["Menu", "Prompt"]);
}

#[test]
fn a_broken_modal_fails_every_check_it_breaks() {
    let failures = check_conventions(&broken());
    for check in [
        Check::Contained,
        Check::Title,
        Check::Selection,
        Check::Words,
        Check::Keys,
        Check::Footer,
        Check::Close,
    ] {
        assert!(
            !failed(&failures, check).is_empty(),
            "{check:?} did not fail:\n{}",
            failures
                .iter()
                .map(Failure::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    assert!(failed(&failures, Check::Drawn).is_empty(), "it is drawn");
    assert!(failures.iter().all(|failure| failure.case == "Broken"));
}

#[test]
fn each_failure_says_what_broke_and_where() {
    let failures = check_conventions(&broken());
    let contained = failed(&failures, Check::Contained);
    assert!(
        contained
            .iter()
            .any(|message| message.contains("outside the frame")),
        "{contained:?}"
    );
    let title = failed(&failures, Check::Title);
    assert!(
        title
            .iter()
            .any(|message| message.contains("not the accent")),
        "{title:?}"
    );
    assert!(
        title
            .iter()
            .any(|message| message.contains("regular weight")),
        "{title:?}"
    );
    let selection = failed(&failures, Check::Selection);
    assert!(
        selection
            .iter()
            .any(|message| message.contains("changed when only the selection moved")),
        "{selection:?}"
    );
    let words = failed(&failures, Check::Words);
    assert!(
        words
            .iter()
            .any(|message| message.contains("the start of a longer word")),
        "{words:?}"
    );
    let keys = failed(&failures, Check::Keys);
    for expected in [
        "Enter is bound to [Leave], not to applying",
        "Enter leaves",
        "Ctrl-C is bound to []",
        "BROKEN_KEYS does not start with Ctrl-C",
    ] {
        assert!(
            keys.iter().any(|message| message.contains(expected)),
            "{expected}: {keys:?}"
        );
    }
    let footer = failed(&failures, Check::Footer);
    assert!(
        footer.iter().any(|message| message.contains("`Esc close`")),
        "{footer:?}"
    );
    let close = failed(&failures, Check::Close);
    for expected in [
        "Ctrl-C did not close it",
        "q did not close it",
        "closing with Esc said",
    ] {
        assert!(
            close.iter().any(|message| message.contains(expected)),
            "{expected}: {close:?}"
        );
    }
}

#[test]
#[should_panic(expected = "modal convention failure")]
fn assert_conventions_fails_for_a_broken_modal() {
    assert_conventions(&broken());
}

#[test]
fn a_chosen_check_runs_alone() {
    let failures = check_conventions_of(&broken(), &[Check::Footer]);
    assert!(!failures.is_empty());
    assert!(
        failures
            .iter()
            .all(|failure| failure.check == Check::Footer)
    );
    assert_conventions_of(&sound(), &[Check::Footer, Check::Close]);
}

/// Draws nothing at all.
struct Invisible;

impl Overlay<String, ()> for Invisible {
    fn handle(&mut self, _: KeyEvent, (): &()) -> Outcome<String> {
        Outcome::Close
    }

    fn render(&mut self, _: Rect, _: &mut Buffer, (): &(), _: &Theme) {}
}

#[test]
fn a_modal_that_draws_nothing_fails_drawn_and_has_no_frame() {
    let registry = Registry::new(host()).case(Case::new("Invisible", open(|| Box::new(Invisible))));
    let failures = check_conventions_of(&registry, &[Check::Drawn, Check::Contained]);
    assert!(!failed(&failures, Check::Drawn).is_empty());
    assert!(
        failed(&failures, Check::Contained)
            .iter()
            .all(|message| message.starts_with("draws no frame"))
    );
    let full = Registry::new(host())
        .case(Case::new("Invisible", open(|| Box::new(Invisible))).full_screen());
    assert!(
        check_conventions_of(&full, &[Check::Contained, Check::Words, Check::Title]).is_empty()
    );
}

#[test]
fn a_table_is_checked_once_however_many_cases_consult_it() {
    let registry = Registry::new(host())
        .case(
            Case::new("Broken", open(|| Box::new(Broken { cursor: 0 })))
                .variant("first")
                .keys(menu_convention(), [table("BROKEN_KEYS", BROKEN_KEYS)]),
        )
        .case(
            Case::new("Broken", open(|| Box::new(Broken { cursor: 0 })))
                .variant("second")
                .keys(menu_convention(), [table("BROKEN_KEYS", BROKEN_KEYS)]),
        );
    let failures = check_conventions_of(&registry, &[Check::Keys]);
    let table_problems: Vec<&Failure> = failures
        .iter()
        .filter(|failure| failure.message.starts_with("BROKEN_KEYS"))
        .collect();
    assert_eq!(table_problems.len(), 1, "{table_problems:?}");
    assert_eq!(table_problems[0].case, "Broken (first)");
    assert_eq!(registry.names(), ["Broken"]);
}

#[test]
fn same_chrome_compares_the_frame_and_not_the_body() {
    let theme = Theme::default();
    let draw = |body: &str, title: &str| {
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        ModalFrame::new(&theme, ModalSize::MENU)
            .title(title.to_string())
            .render(
                area,
                &mut buf,
                Lines::new(vec![Line::from(body.to_string())]),
            );
        buf
    };
    let rect = ModalSize::MENU.rect(Rect::new(0, 0, 40, 10), 1);
    assert!(same_chrome(&draw("one", "Same"), &draw("two", "Same"), rect).is_empty());
    let differ = same_chrome(&draw("one", "Same"), &draw("one", "Other"), rect);
    assert!(
        !differ.is_empty() && differ[0].starts_with('('),
        "{differ:?}"
    );
}
