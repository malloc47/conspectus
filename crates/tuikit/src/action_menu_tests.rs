use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget};

use super::*;
use crate::glyphs::marks;
use crate::keys::{
    Convention, EnterMeans, SpaceMeans, ch, ctrl as ctrl_key, footer, unavailable_style,
};
use crate::overlay::ModalStack;
use crate::testkit::buffer::{find, rows, style_at};
use crate::testkit::conformance::{self, Case, Registry, Rig};
use crate::testkit::golden::assert_golden;
use crate::testkit::keys::{ctrl, press};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Open,
    Rename,
    Copy,
    Remove,
    Item(usize),
}

/// A menu of four entries: a plain one with a detail and a shortcut, a
/// plain one, one that cannot run, and one that asks twice.
fn menu() -> ActionMenu<Msg> {
    ActionMenu::new(
        "notes.txt",
        vec![
            Entry::new("Open", Msg::Open)
                .detail("show the file")
                .shortcut(ch('o')),
            Entry::new("Rename…", Msg::Rename).detail("give the file another name"),
            Entry::new("Copy", Msg::Copy)
                .detail("nothing to copy to")
                .disabled(),
            Entry::new("Remove", Msg::Remove)
                .detail("delete the file")
                .shortcut(ch('x'))
                .confirm(Confirm::Inline),
        ],
    )
}

/// A menu of `count` entries, each with a detail line.
fn long(count: usize) -> ActionMenu<Msg> {
    ActionMenu::new(
        "Many",
        (0..count)
            .map(|n| Entry::new(format!("Item {n}"), Msg::Item(n)).detail(format!("detail {n}")))
            .collect(),
    )
}

fn key(menu: &mut ActionMenu<Msg>, event: KeyEvent) -> Outcome<Msg> {
    menu.handle(event, &())
}

fn down(menu: &mut ActionMenu<Msg>) {
    key(menu, press(KeyCode::Down));
}

/// The menu drawn alone on a `width` by `height` screen.
fn drawn(menu: &mut ActionMenu<Msg>, width: u16, height: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    menu.draw(buf.area, &mut buf, &Theme::default());
    buf
}

/// The rows that carry the pointer mark.
fn marked(buf: &Buffer) -> Vec<String> {
    rows(buf)
        .into_iter()
        .filter(|row| row.contains(marks::POINTER))
        .collect()
}

// ---- picking ---------------------------------------------------------------

#[test]
fn enter_commits_the_entry_under_the_cursor() {
    let mut menu = menu();
    assert_eq!(
        key(&mut menu, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Open)
    );
    down(&mut menu);
    assert_eq!(
        key(&mut menu, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Rename)
    );
}

#[test]
fn every_list_key_moves_the_cursor() {
    for (forward, back) in [
        (press(KeyCode::Down), press(KeyCode::Up)),
        (press(KeyCode::Char('j')), press(KeyCode::Char('k'))),
        (ctrl('n'), ctrl('p')),
    ] {
        let mut menu = menu();
        key(&mut menu, forward);
        assert_eq!(menu.cursor(), 1, "{forward:?}");
        key(&mut menu, back);
        assert_eq!(menu.cursor(), 0, "{back:?}");
    }
}

#[test]
fn the_cursor_wraps_at_both_ends() {
    let mut menu = menu();
    key(&mut menu, press(KeyCode::Up));
    assert_eq!(menu.cursor(), 3, "up from the first entry is the last");
    down(&mut menu);
    assert_eq!(menu.cursor(), 0, "down from the last entry is the first");
}

#[test]
fn the_close_keys_close_the_menu_saying_nothing() {
    for close in [press(KeyCode::Esc), press(KeyCode::Char('q')), ctrl('c')] {
        let mut menu = menu();
        assert_eq!(key(&mut menu, close), Outcome::Close, "{close:?}");
    }
}

#[test]
fn other_keys_are_swallowed() {
    let mut menu = menu();
    for other in [press(KeyCode::Char('z')), press(KeyCode::Tab), ctrl('o')] {
        assert_eq!(key(&mut menu, other), Outcome::Consumed, "{other:?}");
    }
    assert_eq!(menu.cursor(), 0);
}

// ---- inline confirmation ---------------------------------------------------

#[test]
fn an_inline_entry_arms_on_the_first_enter_and_commits_on_the_second() {
    let mut menu = menu();
    key(&mut menu, press(KeyCode::Up));
    assert_eq!(key(&mut menu, press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(menu.armed(), Some(3));
    assert_eq!(
        key(&mut menu, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Remove)
    );
}

#[test]
fn an_armed_entry_says_what_enter_will_do() {
    let mut menu = menu();
    key(&mut menu, press(KeyCode::Up));
    let before = rows(&drawn(&mut menu, 80, 24)).join("\n");
    assert!(!before.contains("again"), "{before}");
    key(&mut menu, press(KeyCode::Enter));
    let armed = marked(&drawn(&mut menu, 80, 24));
    assert_eq!(armed.len(), 1);
    assert!(
        armed[0].contains("Remove · press Enter again to confirm"),
        "{armed:?}"
    );
    assert_eq!(confirm_note(), "press Enter again to confirm");
}

#[test]
fn an_armed_entry_keeps_the_highlight_in_the_error_color() {
    let theme = Theme::default();
    let mut menu = menu();
    key(&mut menu, press(KeyCode::Up));
    key(&mut menu, press(KeyCode::Enter));
    let buf = drawn(&mut menu, 80, 24);
    let style = style_at(&buf, "Remove ·").expect("the armed row");
    assert_eq!(style.fg, Some(theme.error));
    assert!(style.add_modifier.contains(theme.selection_active));
}

#[test]
fn moving_disarms_a_pending_confirmation() {
    let mut menu = menu();
    key(&mut menu, press(KeyCode::Up));
    key(&mut menu, press(KeyCode::Enter));
    key(&mut menu, press(KeyCode::Up));
    key(&mut menu, press(KeyCode::Down));
    assert_eq!(menu.armed(), None);
    assert_eq!(key(&mut menu, press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(menu.armed(), Some(3));
}

#[test]
fn the_close_keys_close_even_an_armed_menu() {
    for close in [press(KeyCode::Esc), press(KeyCode::Char('q')), ctrl('c')] {
        let mut menu = menu();
        key(&mut menu, press(KeyCode::Up));
        key(&mut menu, press(KeyCode::Enter));
        assert_eq!(key(&mut menu, close), Outcome::Close, "{close:?}");
    }
}

// ---- disabled entries ------------------------------------------------------

#[test]
fn the_cursor_steps_over_an_entry_that_cannot_run() {
    let mut menu = menu();
    down(&mut menu);
    down(&mut menu);
    assert_eq!(menu.cursor(), 3, "Copy is skipped going down");
    key(&mut menu, press(KeyCode::Up));
    assert_eq!(menu.cursor(), 1, "and going up");
}

#[test]
fn the_cursor_starts_on_the_first_entry_that_can_run() {
    let mut menu = ActionMenu::new(
        "Later",
        vec![
            Entry::new("Copy", Msg::Copy).disabled(),
            Entry::new("Open", Msg::Open),
        ],
    );
    assert_eq!(menu.cursor(), 1);
    assert_eq!(
        menu.selected().map(|entry| &entry.outcome),
        Some(&Msg::Open)
    );
    key(&mut menu, press(KeyCode::Down));
    assert_eq!(
        menu.cursor(),
        1,
        "the only entry that can run wraps to itself"
    );
}

#[test]
fn a_disabled_entry_recedes_and_is_never_marked() {
    let theme = Theme::default();
    let mut menu = menu();
    let buf = drawn(&mut menu, 80, 24);
    let style = style_at(&buf, "Copy").expect("the disabled row");
    let dim = unavailable_style(&theme);
    assert_eq!(style.fg, dim.fg);
    assert!(style.add_modifier.contains(theme.muted));
    // Its detail, which says why, reads as plainly as any other.
    assert_eq!(
        style_at(&buf, "nothing to copy to"),
        style_at(&buf, "show the file")
    );
    for _ in 0..8 {
        down(&mut menu);
        let shown = marked(&drawn(&mut menu, 80, 24));
        assert!(!shown[0].contains("Copy"), "{shown:?}");
    }
}

#[test]
fn a_menu_where_nothing_can_run_marks_no_row_and_picks_nothing() {
    let mut menu = ActionMenu::new(
        "Nothing",
        vec![
            Entry::new("Copy", Msg::Copy).disabled(),
            Entry::new("Open", Msg::Open).disabled().shortcut(ch('o')),
        ],
    );
    assert!(menu.selected().is_none());
    down(&mut menu);
    assert_eq!(key(&mut menu, press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(key(&mut menu, press(KeyCode::Char('o'))), Outcome::Consumed);
    assert!(marked(&drawn(&mut menu, 80, 24)).is_empty());
    assert_eq!(key(&mut menu, press(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn an_empty_menu_draws_its_footer_and_picks_nothing() {
    let mut menu: ActionMenu<Msg> = ActionMenu::new("Empty", Vec::new());
    assert_eq!(key(&mut menu, press(KeyCode::Enter)), Outcome::Consumed);
    down(&mut menu);
    let shown = rows(&drawn(&mut menu, 80, 24)).join("\n");
    assert!(shown.contains(&footer(MENU_KEYS)), "{shown}");
}

// ---- shortcuts ---------------------------------------------------------------

#[test]
fn a_shortcut_picks_its_entry_from_anywhere() {
    let mut menu = menu();
    down(&mut menu);
    assert_eq!(
        key(&mut menu, press(KeyCode::Char('o'))),
        Outcome::Commit(Msg::Open)
    );
    assert_eq!(menu.cursor(), 0, "the cursor moved to it");
}

#[test]
fn a_shortcut_to_an_inline_entry_arms_it_first() {
    let mut menu = menu();
    assert_eq!(key(&mut menu, press(KeyCode::Char('x'))), Outcome::Consumed);
    assert_eq!((menu.cursor(), menu.armed()), (3, Some(3)));
    assert_eq!(
        key(&mut menu, press(KeyCode::Char('x'))),
        Outcome::Commit(Msg::Remove),
        "the shortcut again confirms, as Enter does"
    );
}

#[test]
fn a_shortcut_binds_its_character_as_typed() {
    let mut menu = menu();
    assert_eq!(
        key(&mut menu, ctrl('x')),
        Outcome::Consumed,
        "not a chord of it"
    );
    assert_eq!(menu.armed(), None);
}

#[test]
fn a_disabled_entrys_shortcut_does_nothing() {
    let mut menu = ActionMenu::new(
        "Later",
        vec![
            Entry::new("Open", Msg::Open),
            Entry::new("Copy", Msg::Copy).shortcut(ch('c')).disabled(),
        ],
    );
    assert_eq!(key(&mut menu, press(KeyCode::Char('c'))), Outcome::Consumed);
    assert_eq!(menu.cursor(), 0, "the cursor stays");
}

#[test]
fn the_key_column_lines_the_labels_up() {
    let mut menu = ActionMenu::new(
        "Keys",
        vec![
            Entry::new("Open", Msg::Open).shortcut(ch('o')),
            Entry::new("Rename", Msg::Rename).shortcut(ctrl_key('r')),
            Entry::new("Copy", Msg::Copy).detail("copy it"),
        ],
    );
    let buf = drawn(&mut menu, 80, 24);
    let open = find(&buf, "Open").expect("Open");
    let rename = find(&buf, "Rename").expect("Rename");
    let copy = find(&buf, "Copy").expect("Copy");
    assert_eq!(open.x, rename.x);
    assert_eq!(open.x, copy.x);
    let key = find(&buf, "Ctrl-R").expect("the chord's label");
    assert_eq!(rename.x, key.x + 6 + 2, "the widest key, then two cells");
    let detail = find(&buf, "copy it").expect("the detail");
    assert_eq!(detail.x, copy.x + 3, "details sit past the labels");
}

#[test]
fn a_menu_without_shortcuts_has_no_key_column() {
    let mut menu = long(2);
    let buf = drawn(&mut menu, 80, 24);
    let marker = find(&buf, marks::POINTER).expect("the cursor");
    let label = find(&buf, "Item 0").expect("the label");
    assert_eq!(label.x, marker.x + 2, "the label follows the gutter");
}

#[test]
fn shadowed_shortcuts_are_reported() {
    assert!(menu().shadowed_shortcuts().is_empty());
    let menu = ActionMenu::new(
        "Clash",
        vec![
            Entry::new("Open", Msg::Open).shortcut(ch('j')),
            Entry::new("Rename", Msg::Rename).shortcut(ch('r')),
            Entry::new("Remove", Msg::Remove).shortcut(ch('r')),
            Entry::new("Copy", Msg::Copy).shortcut(ch('q')),
        ],
    );
    assert_eq!(menu.shadowed_shortcuts(), ["j", "r", "q"]);
}

// ---- scrolling ---------------------------------------------------------------

/// Steps through every entry of a long menu on a `width` by `height`
/// screen, both ways, and checks the cursor's entry and the whole footer
/// are on screen at every step.
fn walk(width: u16, height: u16) {
    let mut menu = long(14);
    let footer = footer(MENU_KEYS);
    for step in 0..30 {
        let buf = drawn(&mut menu, width, height);
        let text = rows(&buf).join("\n");
        let label = format!("Item {}", menu.cursor());
        let shown = marked(&buf);
        assert!(
            shown.len() == 1 && shown[0].contains(&label),
            "{width}x{height}, step {step}: the cursor's entry `{label}` is shown\n{text}"
        );
        assert!(
            text.contains(&footer),
            "{width}x{height}, step {step}: the footer is shown\n{text}"
        );
        let forward = step < 15;
        key(
            &mut menu,
            press(if forward { KeyCode::Down } else { KeyCode::Up }),
        );
    }
}

#[test]
fn a_long_menu_scrolls_and_keeps_its_footer_at_40x10() {
    walk(40, 10);
}

#[test]
fn a_long_menu_scrolls_and_keeps_its_footer_at_80x24() {
    walk(80, 24);
}

#[test]
fn a_menu_that_fits_is_as_tall_as_its_rows() {
    let mut menu = menu();
    let buf = drawn(&mut menu, 80, 24);
    let shown = rows(&buf);
    let top = shown.iter().position(|row| row.contains("┌")).expect("top");
    let bottom = shown
        .iter()
        .position(|row| row.contains("└"))
        .expect("bottom");
    // A blank row, four labels and their details, a blank row, the footer.
    assert_eq!(bottom - top - 1, 1 + 8 + 1 + 1);
    assert!(menu.window().visible() == (0..9), "{:?}", menu.window());
}

#[test]
fn scrolling_moves_no_further_than_the_cursor_needs() {
    let mut menu = long(14);
    let _ = drawn(&mut menu, 40, 10);
    let viewport = menu.window().viewport();
    assert!(viewport < 29, "the menu overflows: {:?}", menu.window());
    // Down to the last entry, then back up one: the window stays put
    // while the cursor is inside it.
    key(&mut menu, press(KeyCode::Up));
    let _ = drawn(&mut menu, 40, 10);
    let bottom = menu.window();
    assert_eq!(bottom.offset(), bottom.last_offset(), "{bottom:?}");
    key(&mut menu, press(KeyCode::Up));
    let _ = drawn(&mut menu, 40, 10);
    assert_eq!(menu.window(), bottom, "one entry up stays in view");
}

#[test]
fn an_overflowing_menu_shows_the_scrollbar_and_one_that_fits_does_not() {
    let bar = |menu: &mut ActionMenu<Msg>, width, height| {
        let buf = drawn(menu, width, height);
        rows(&buf)
            .iter()
            .any(|row| row.contains('█') || row.contains('║'))
    };
    // The bar's track keeps a row clear above and below it, so it draws
    // once the entries have five rows; on a 40x10 screen they have four.
    assert!(bar(&mut long(14), 60, 16), "a long menu on a short screen");
    assert!(!bar(&mut long(3), 80, 24), "a short menu that fits");
}

#[test]
fn the_cursor_stays_in_view_on_the_smallest_frames() {
    for height in 5..=9 {
        let mut menu = long(6);
        for _ in 0..6 {
            let buf = drawn(&mut menu, 40, height);
            let label = format!("Item {}", menu.cursor());
            assert!(
                marked(&buf).iter().any(|row| row.contains(&label)),
                "40x{height}: {label}\n{}",
                rows(&buf).join("\n")
            );
            down(&mut menu);
        }
    }
}

// ---- the look -------------------------------------------------------------

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/action_menu/goldens")
        .join(name)
}

#[test]
fn the_menu_looks_as_its_goldens_say() {
    let mut armed = menu();
    key(&mut armed, press(KeyCode::Up));
    key(&mut armed, press(KeyCode::Enter));
    let buf = drawn(&mut armed, 64, 16);
    assert_golden(&golden_path("menu.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
    let mut scrolled = long(14);
    key(&mut scrolled, press(KeyCode::Up));
    let narrow = drawn(&mut scrolled, 40, 10);
    assert_golden(&golden_path("narrow.golden"), &narrow, |path, text| {
        std::fs::write(path, text)
    });
}

// ---- the conventions -----------------------------------------------------

/// A host of one stack of menus over a screen of text.
struct Host(Theme);

impl Rig for Host {
    type State = ModalStack<ActionMenu<Msg>>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        Paragraph::new("the screen under the menu").render(area, &mut buf);
        stack.draw::<Msg, ()>(area, &mut buf, &(), &self.0);
        buf
    }

    fn close_top(&self, stack: &mut Self::State) {
        stack.pop();
    }

    fn press(&self, stack: &mut Self::State, key: KeyEvent) {
        if let Some(outcome) = stack.route::<Msg, ()>(key, &()) {
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

#[test]
fn a_menu_keeps_the_conventions_every_modal_keeps() {
    let open = || {
        let mut stack = ModalStack::new();
        stack.push(menu());
        stack
    };
    let open_long = || {
        let mut stack = ModalStack::new();
        stack.push(long(14));
        stack
    };
    let convention = Convention::new("ActionMenu", EnterMeans::Activates, SpaceMeans::Types, "");
    let registry = Registry::new(Host(Theme::default()))
        .case(
            Case::new("ActionMenu", open)
                .moves()
                .keys(convention, [conformance::table("MENU_KEYS", MENU_KEYS)])
                .footer(MENU_KEYS),
        )
        .case(
            Case::new("ActionMenu", open_long)
                .variant("long")
                .moves()
                .keys(convention, [conformance::table("MENU_KEYS", MENU_KEYS)])
                .footer(MENU_KEYS),
        );
    conformance::assert_conventions(&registry);
}
