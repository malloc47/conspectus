use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

use super::*;
use crate::action_menu::{ActionMenu, Entry};
use crate::glyphs::marks;
use crate::keys::{Convention, EnterMeans, SpaceMeans, ch, ctrl as ctrl_key};
use crate::overlay::ModalStack;
use crate::testkit::buffer::{rows, style_at};
use crate::testkit::conformance::{self, Case, Registry, Rig};
use crate::testkit::golden::assert_golden;
use crate::testkit::keys::{ctrl, press};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Delete,
    Archive,
    Ask,
}

/// A confirmation with a body and a warning list.
fn delete() -> Dialog<Msg> {
    Dialog::confirm("Delete", Msg::Delete)
        .body(["Delete `plan.txt` and its history?"])
        .warning(
            "open in 2 windows:",
            ["the left window", "the right window"],
        )
}

/// A choice between two verbs.
fn close() -> Dialog<Msg> {
    Dialog::choice(
        "Close plan.txt",
        vec![
            Verb::new(ch('a'), "archive & close", Msg::Archive),
            Verb::new(ch('d'), "delete & close", Msg::Delete),
        ],
    )
    .body([
        "Close `plan.txt`",
        "Archives or deletes it, then closes every window it is open in.",
    ])
}

fn notice() -> Dialog<Msg> {
    Dialog::notice("Nothing to do").body(["The list is empty.", "", "Add an item first."])
}

/// A dialog whose body is many rows tall.
fn tall(items: usize) -> Dialog<Msg> {
    Dialog::confirm("Delete", Msg::Delete)
        .body(["Delete these?"])
        .warning(
            format!("{items} items:"),
            (0..items).map(|n| format!("item {n}")),
        )
}

fn key(dialog: &mut Dialog<Msg>, event: KeyEvent) -> Outcome<Msg> {
    dialog.handle(event, &())
}

/// The dialog drawn alone on a `width` by `height` screen.
fn drawn(dialog: &mut Dialog<Msg>, width: u16, height: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    dialog.draw(buf.area, &mut buf, &Theme::default());
    buf
}

fn text(buf: &Buffer) -> String {
    rows(buf).join("\n")
}

// ---- keys -----------------------------------------------------------------

#[test]
fn a_confirmation_goes_ahead_on_enter_or_y_and_cancels_on_esc_or_n() {
    for yes in [press(KeyCode::Enter), press(KeyCode::Char('y'))] {
        assert_eq!(key(&mut delete(), yes), Outcome::Commit(Msg::Delete));
    }
    // A letter binds as typed, with or without a Shift the terminal
    // reports beside it.
    let shifted = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::SHIFT);
    assert_eq!(key(&mut delete(), shifted), Outcome::Commit(Msg::Delete));
    for no in [press(KeyCode::Esc), press(KeyCode::Char('n'))] {
        assert_eq!(key(&mut delete(), no), Outcome::Close, "{no:?}");
    }
}

#[test]
fn ctrl_c_and_q_dismiss_every_form_saying_nothing() {
    for mut dialog in [delete(), close(), notice()] {
        for dismiss in [ctrl('c'), press(KeyCode::Char('q'))] {
            assert_eq!(key(&mut dialog, dismiss), Outcome::Dismiss, "{dismiss:?}");
        }
    }
}

#[test]
fn a_chord_of_a_dialog_letter_does_nothing() {
    for mut dialog in [delete(), close(), notice()] {
        for chord in [ctrl('y'), ctrl('n'), ctrl('a'), ctrl('d'), ctrl('q')] {
            assert_eq!(key(&mut dialog, chord), Outcome::Consumed, "{chord:?}");
        }
        assert_eq!(
            key(&mut dialog, press(KeyCode::Char('x'))),
            Outcome::Consumed
        );
    }
}

#[test]
fn a_choice_takes_its_verbs_keys_and_has_no_default() {
    assert_eq!(
        key(&mut close(), press(KeyCode::Char('a'))),
        Outcome::Commit(Msg::Archive)
    );
    assert_eq!(
        key(&mut close(), press(KeyCode::Char('d'))),
        Outcome::Commit(Msg::Delete)
    );
    for nothing in [press(KeyCode::Enter), press(KeyCode::Char('y'))] {
        assert_eq!(key(&mut close(), nothing), Outcome::Consumed, "{nothing:?}");
    }
    for no in [press(KeyCode::Esc), press(KeyCode::Char('n'))] {
        assert_eq!(key(&mut close(), no), Outcome::Close, "{no:?}");
    }
}

#[test]
fn every_dismiss_key_closes_a_notice_and_it_commits_nothing() {
    for close_key in [
        press(KeyCode::Enter),
        press(KeyCode::Char('y')),
        press(KeyCode::Esc),
        press(KeyCode::Char('n')),
    ] {
        assert_eq!(
            key(&mut notice(), close_key),
            Outcome::Close,
            "{close_key:?}"
        );
    }
}

#[test]
fn a_verb_on_a_key_the_table_binds_is_shadowed() {
    assert!(close().shadowed_verbs().is_empty());
    let clash = Dialog::choice(
        "Clash",
        vec![
            Verb::new(ch('y'), "yes please", Msg::Delete),
            Verb::new(ch('a'), "archive", Msg::Archive),
            Verb::new(ch('a'), "again", Msg::Ask),
            Verb::new(ch('j'), "jump", Msg::Ask),
        ],
    );
    assert_eq!(clash.shadowed_verbs(), ["y", "a", "j"]);
    assert!(
        delete().shadowed_verbs().is_empty(),
        "no verbs, none shadowed"
    );
}

// ---- footers --------------------------------------------------------------

#[test]
fn each_form_generates_its_footer_from_the_one_table() {
    assert_eq!(delete().footer().text(), "Enter/y confirm · Esc/n cancel");
    assert_eq!(
        close().footer().text(),
        "a archive & close · d delete & close · Esc/n cancel"
    );
    assert_eq!(notice().footer().text(), "Enter/Esc close");
}

// ---- layout ---------------------------------------------------------------

#[test]
fn the_body_wraps_under_a_blank_row_and_the_warning_list_follows() {
    let buf = drawn(&mut delete(), 100, 24);
    let lines = rows(&buf);
    let top = lines
        .iter()
        .position(|row| row.contains("┌ Delete "))
        .expect("the title is on the top border");
    let inside = |row: usize| {
        let line = &lines[top + row];
        let start = line.find('│').expect("a side") + '│'.len_utf8();
        line[start..]
            .trim_end()
            .trim_end_matches('│')
            .trim_end()
            .to_string()
    };
    assert_eq!(inside(1), "", "a blank row first");
    assert_eq!(inside(2), "Delete `plan.txt` and its history?");
    assert_eq!(inside(3), "");
    assert_eq!(inside(4), format!("{} open in 2 windows:", marks::WARNING));
    assert_eq!(inside(5), "  - the left window");
    assert_eq!(inside(6), "  - the right window");
    assert_eq!(inside(7), "");
    assert_eq!(inside(8), "Enter/y confirm · Esc/n cancel");
    let theme = Theme::default();
    let style = style_at(&buf, "open in 2 windows").expect("the heading");
    assert_eq!(style.fg, Some(theme.warning), "the heading takes `warning`");
    assert!(style.add_modifier.contains(Modifier::BOLD));
    let mark = style_at(&buf, marks::WARNING).expect("the mark");
    assert_eq!(mark.fg, Some(theme.warning), "and so does its mark");
    let item = style_at(&buf, "the left window").expect("an item");
    assert_ne!(item.fg, Some(theme.warning), "items are plain");
}

#[test]
fn the_dialog_is_the_menu_frame_and_as_tall_as_its_rows() {
    let buf = drawn(&mut notice(), 100, 24);
    let lines = rows(&buf);
    let top = lines.iter().position(|row| row.contains('┌')).expect("top");
    let bottom = lines
        .iter()
        .position(|row| row.contains('└'))
        .expect("bottom");
    let width = lines[top].trim().chars().count();
    assert_eq!(width, 64, "the menu frame's cap");
    // A blank row, three body rows, a blank row, the footer.
    assert_eq!(bottom - top - 1, 6, "{}", text(&buf));
}

#[test]
fn a_long_heading_and_items_wrap_under_their_first_words() {
    let mut dialog = Dialog::confirm("Delete", Msg::Delete).warning(
        "ends every window this file is open in, keeping the drafts:",
        ["a window whose title is long enough to wrap onto a second row"],
    );
    let buf = drawn(&mut dialog, 40, 20);
    let lines = rows(&buf);
    let drawn = text(&buf);
    let top = lines.iter().position(|row| row.contains('┌')).expect("top");
    let inner = |row: usize| {
        let line = &lines[top + row];
        let start = line.find('│').expect("a side") + '│'.len_utf8();
        line[start..].to_string()
    };
    let starts = |row: usize, indent: usize| {
        let text = inner(row);
        text.chars().take(indent).all(|c| c == ' ')
            && text
                .chars()
                .nth(indent)
                .is_some_and(|c| c != ' ' && c != '-')
    };
    // With no body, the heading follows the blank top row directly.
    assert!(inner(2).starts_with("▲ ends every"), "{drawn}");
    assert!(starts(3, 2), "the heading's second row: {drawn}");
    assert!(inner(4).starts_with("  - a window"), "{drawn}");
    assert!(starts(5, 4), "the item's second row: {drawn}");
}

#[test]
fn an_empty_warning_list_draws_nothing() {
    let mut bare = Dialog::confirm("Delete", Msg::Delete)
        .body(["Delete it?"])
        .warning("open in 0 windows:", Vec::<String>::new());
    assert!(bare.warning_list().is_none());
    let drawn = text(&drawn(&mut bare, 80, 20));
    assert!(!drawn.contains(marks::WARNING), "{drawn}");
}

#[test]
fn a_short_screen_gives_up_the_blank_row_above_the_body_and_keeps_the_keys() {
    // Eight rows of content in a frame of six: the blank row above the
    // body goes; the footer keeps the last row under its blank row, as in
    // every framed control, and the body scrolls in the four rows left.
    let mut dialog = delete();
    let buf = drawn(&mut dialog, 40, 10);
    let shown = text(&buf);
    let lines = rows(&buf);
    let top = lines.iter().position(|row| row.contains('┌')).expect("top");
    let bottom = lines
        .iter()
        .position(|row| row.contains('└'))
        .expect("bottom");
    assert!(lines[top + 1].contains("Delete `plan.txt`"), "{shown}");
    assert!(
        lines[bottom - 1].contains("Enter/y confirm · Esc/n cancel"),
        "{shown}"
    );
    assert_eq!(lines[bottom - 2].trim_matches(['│', ' ']), "", "{shown}");
    assert!(!shown.contains("the right window"), "{shown}");
    key(&mut dialog, press(KeyCode::Down));
    let scrolled = text(&drawn(&mut dialog, 40, 10));
    assert!(scrolled.contains("the right window"), "{scrolled}");
}

#[test]
fn a_body_taller_than_the_screen_scrolls_under_the_footer() {
    let mut dialog = tall(12);
    let first = drawn(&mut dialog, 40, 12);
    let drawn_first = text(&first);
    assert!(drawn_first.contains("Enter/y confirm"), "{drawn_first}");
    assert!(drawn_first.contains("Delete these?"), "{drawn_first}");
    assert!(!drawn_first.contains("item 11"), "{drawn_first}");
    assert!(dialog.window().overflows());
    for _ in 0..20 {
        key(&mut dialog, press(KeyCode::Down));
    }
    let last = drawn(&mut dialog, 40, 12);
    let drawn_last = text(&last);
    assert!(drawn_last.contains("item 11"), "{drawn_last}");
    assert!(drawn_last.contains("Enter/y confirm"), "{drawn_last}");
    assert_eq!(dialog.window().offset(), dialog.window().last_offset());
    key(&mut dialog, press(KeyCode::Char('k')));
    assert_eq!(dialog.window().offset() + 1, dialog.window().last_offset());
    // The shared scrollbar runs beside the body.
    assert!(
        drawn_last.contains('█') || drawn_last.contains('║'),
        "{drawn_last}"
    );
}

#[test]
fn the_dialog_keeps_its_keys_on_the_smallest_screens() {
    for height in 1..=8 {
        for mut dialog in [delete(), close(), notice(), tall(6)] {
            let buf = drawn(&mut dialog, 40, height);
            // From six rows on, the frame has two inner rows: the body
            // keeps one and the footer's last row, the keys that leave,
            // takes the other.
            if height >= 6 {
                let drawn = text(&buf);
                assert!(
                    drawn.contains("Esc/n cancel") || drawn.contains("Enter/Esc close"),
                    "40x{height}:\n{drawn}"
                );
            }
        }
    }
}

// ---- the look -------------------------------------------------------------

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/dialog/goldens")
        .join(name)
}

#[test]
fn the_dialog_looks_as_its_goldens_say() {
    let buf = drawn(&mut delete(), 64, 16);
    assert_golden(&golden_path("confirm.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
    let narrow = drawn(&mut close(), 40, 10);
    assert_golden(&golden_path("choice.golden"), &narrow, |path, text| {
        std::fs::write(path, text)
    });
}

// ---- on the stack ---------------------------------------------------------

/// A host's entries: a menu, or the dialog one of its entries asks.
#[derive(Clone, Debug)]
enum Modal {
    Menu(ActionMenu<Msg>),
    Ask(Dialog<Msg>),
}

impl Overlay<Msg, ()> for Modal {
    fn handle(&mut self, key: KeyEvent, cx: &()) -> Outcome<Msg> {
        match self {
            Self::Menu(menu) => menu.handle(key, cx),
            Self::Ask(dialog) => dialog.handle(key, cx),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &(), theme: &Theme) {
        match self {
            Self::Menu(menu) => menu.render(area, buf, cx, theme),
            Self::Ask(dialog) => dialog.render(area, buf, cx, theme),
        }
    }
}

fn menu() -> ActionMenu<Msg> {
    ActionMenu::new(
        "plan.txt",
        vec![
            Entry::new("Delete…", Msg::Ask).opens_step(),
            Entry::new("Archive", Msg::Archive),
        ],
    )
}

/// One key the way a host routes it: the dialog an entry asks for opens
/// as a step of the menu. Returns what the host handles.
fn route(stack: &mut ModalStack<Modal>, event: KeyEvent) -> Option<Msg> {
    let outcome = stack.route(event, &())?;
    let settled = stack.settle(outcome);
    match settled.message {
        Some(Msg::Ask) => {
            stack.push_step(Modal::Ask(delete()));
            None
        }
        other => other,
    }
}

fn menu_open() -> ModalStack<Modal> {
    let mut stack = ModalStack::new();
    stack.push(Modal::Menu(menu()));
    stack
}

#[test]
fn a_menu_entry_opens_the_dialog_as_a_step_and_esc_comes_back() {
    let mut stack = menu_open();
    assert_eq!(route(&mut stack, press(KeyCode::Enter)), None);
    assert_eq!(stack.len(), 2, "the menu stays under the dialog");
    assert!(stack.top_is_step());
    assert_eq!(route(&mut stack, press(KeyCode::Esc)), None);
    assert!(
        matches!(stack.top(), Some(Modal::Menu(_))),
        "back on the menu"
    );
    // The menu has the keys again, its cursor where it was.
    if let Some(Modal::Menu(menu)) = stack.top() {
        assert_eq!(menu.cursor(), 0);
    }
    assert_eq!(route(&mut stack, press(KeyCode::Down)), None);
    assert_eq!(route(&mut stack, press(KeyCode::Enter)), Some(Msg::Archive));
    assert!(stack.is_empty());
}

#[test]
fn going_ahead_or_dismissing_in_the_step_closes_the_menu_too() {
    let mut stack = menu_open();
    route(&mut stack, press(KeyCode::Enter));
    assert_eq!(
        route(&mut stack, press(KeyCode::Char('y'))),
        Some(Msg::Delete)
    );
    assert!(stack.is_empty(), "the flow is done");
    for dismiss in [ctrl('c'), press(KeyCode::Char('q'))] {
        let mut stack = menu_open();
        route(&mut stack, press(KeyCode::Enter));
        assert_eq!(route(&mut stack, dismiss), None, "{dismiss:?}");
        assert!(stack.is_empty(), "{dismiss:?}: the whole flow is dismissed");
    }
}

#[test]
fn a_menu_taller_than_its_step_shows_above_and_below_it() {
    let mut entries = vec![Entry::new("Delete…", Msg::Ask).opens_step()];
    entries.extend((0..12).map(|n| Entry::new(format!("Entry {n}"), Msg::Archive)));
    let mut stack = ModalStack::new();
    stack.push(Modal::Menu(ActionMenu::new("plan.txt", entries)));
    route(&mut stack, press(KeyCode::Enter));
    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
    stack.draw(buf.area, &mut buf, &(), &Theme::default());
    let lines = rows(&buf);
    let drawn = text(&buf);
    let menu_top = lines.iter().position(|row| row.contains("┌ plan.txt "));
    let dialog_top = lines.iter().position(|row| row.contains("┌ Delete "));
    let (Some(menu_top), Some(dialog_top)) = (menu_top, dialog_top) else {
        panic!("both frames show: {drawn}");
    };
    assert!(menu_top < dialog_top, "{drawn}");
    assert!(
        drawn.contains("↑/↓ move · Enter pick"),
        "the menu's footer: {drawn}"
    );
    // The two are one width, so no column of the menu shows beside it.
    let row = lines
        .iter()
        .find(|row| row.contains("the left window"))
        .expect("an item");
    assert_eq!(row.matches('│').count(), 2, "{drawn}");
}

// ---- the conventions -----------------------------------------------------

/// A host of one stack of menus and dialogs over a screen of text.
struct Host(Theme);

impl Rig for Host {
    type State = ModalStack<Modal>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        Paragraph::new("the screen under the dialog").render(area, &mut buf);
        stack.draw::<Msg, ()>(area, &mut buf, &(), &self.0);
        buf
    }

    fn close_top(&self, stack: &mut Self::State) {
        stack.pop();
    }

    fn press(&self, stack: &mut Self::State, event: KeyEvent) {
        route(stack, event);
    }

    fn depth(&self, stack: &Self::State) -> usize {
        stack.len()
    }

    fn theme(&self) -> &Theme {
        &self.0
    }
}

#[test]
fn every_form_keeps_the_conventions_every_modal_keeps() {
    let convention = Convention::new("Dialog", EnterMeans::Submits, SpaceMeans::Types, "");
    let keys = || [conformance::table("DIALOG_KEYS", DIALOG_KEYS)];
    let alone = |dialog: fn() -> Dialog<Msg>| {
        move || {
            let mut stack = ModalStack::new();
            stack.push(Modal::Ask(dialog()));
            stack
        }
    };
    let step = || {
        let mut stack = menu_open();
        route(&mut stack, press(KeyCode::Enter));
        stack
    };
    let registry = Registry::new(Host(Theme::default()))
        .case(
            Case::new("Dialog", alone(delete))
                .variant("confirm")
                .keys(convention, keys())
                .footer(delete().footer()),
        )
        .case(
            Case::new("Dialog", alone(close))
                .variant("choice")
                .keys(convention, keys())
                .footer(close().footer()),
        )
        .case(
            Case::new("Dialog", alone(notice))
                .variant("notice")
                .keys(convention, keys())
                .footer(notice().footer()),
        )
        .case(
            Case::new("Dialog", step)
                .variant("a step of a menu")
                .keys(convention, keys())
                .footer(delete().footer()),
        );
    conformance::assert_conventions(&registry);
}

#[test]
fn the_dialog_table_passes_the_table_checks() {
    use crate::keys::{ctrl_c_first, loose_characters, mislabeled, unresolved};
    assert!(mislabeled(DIALOG_KEYS).is_empty());
    assert!(unresolved(DIALOG_KEYS).is_empty());
    assert!(loose_characters(DIALOG_KEYS).is_empty());
    assert!(ctrl_c_first(DIALOG_KEYS));
    assert_eq!(
        resolve(DIALOG_KEYS, ctrl_key('c').example()),
        Some(DialogKey::Abort)
    );
}

/// The rows inside the one frame drawn on `buf`, trailing spaces trimmed.
fn inside(buf: &Buffer) -> Vec<String> {
    let rect = crate::testkit::buffer::frame_rect(buf, &Buffer::empty(buf.area)).expect("a frame");
    crate::testkit::buffer::inner_rows(buf, rect)
        .into_iter()
        .map(|row| row.trim_end().to_string())
        .collect()
}

#[test]
fn a_footer_too_tall_for_a_short_frame_keeps_the_keys_that_leave_and_scrolls_the_rest() {
    // At 16 by 6 the frame has two rows inside and the footer three: the
    // body keeps a row, the footer its last, the keys that leave. Its
    // first rows follow the body's last row, where scrolling reaches them.
    let mut dialog = delete();
    let buf = drawn(&mut dialog, 16, 6);
    let rows = inside(&buf);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert!(rows[0].starts_with("Delete"), "{rows:?}");
    assert_eq!(rows[1], "Esc/n cancel", "{rows:?}");
    assert!(!rows.iter().any(|row| row.contains("confirm")), "{rows:?}");
    for _ in 0..30 {
        key(&mut dialog, press(KeyCode::Down));
    }
    let rows = inside(&drawn(&mut dialog, 16, 6));
    assert_eq!(
        rows[0].trim_end_matches(['▼', '║', '█', '▲', ' ']),
        "confirm",
        "{rows:?}"
    );
    assert_eq!(rows[1], "Esc/n cancel", "{rows:?}");
}
