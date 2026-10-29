use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

use super::*;
use crate::glyphs::marks;
use crate::keys::{
    Convention, EnterMeans, Footer, SpaceMeans, ctrl_c_first, footer, loose_characters, mislabeled,
    unavailable_style, unresolved,
};
use crate::overlay::ModalStack;
use crate::testkit::buffer::{find, rows, style_at};
use crate::testkit::conformance::{self, Case, Registry, Rig};
use crate::testkit::golden::assert_golden;
use crate::testkit::keys::{ctrl, press, typed};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Keep(Vec<usize>),
}

/// Five toppings: two checked, one that cannot change (checked, with a
/// reason), and two more.
fn toppings() -> Checklist<Msg> {
    Checklist::new(
        "Toppings",
        vec![
            Item::new("cheese").checked(true),
            Item::new("onion"),
            Item::new("salt")
                .checked(true)
                .detail("always added")
                .disabled(),
            Item::new("pepper").checked(true),
            Item::new("olive"),
        ],
        |indices: Vec<usize>| {
            if indices.is_empty() {
                Outcome::Close
            } else {
                Outcome::Commit(Msg::Keep(indices))
            }
        },
    )
}

/// A list of `count` items, none checked.
fn long(count: usize) -> Checklist<Msg> {
    Checklist::new(
        "Many",
        (0..count).map(|n| Item::new(format!("Item {n}"))).collect(),
        |indices: Vec<usize>| Outcome::Commit(Msg::Keep(indices)),
    )
}

fn key(list: &mut Checklist<Msg>, event: KeyEvent) -> Outcome<Msg> {
    list.handle(event, &())
}

fn space(list: &mut Checklist<Msg>) {
    key(list, press(KeyCode::Char(' ')));
}

/// The checklist drawn alone on a `width` by `height` screen.
fn drawn(list: &mut Checklist<Msg>, width: u16, height: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    list.draw(buf.area, &mut buf, &Theme::default());
    buf
}

fn text(buf: &Buffer) -> String {
    rows(buf).join("\n")
}

/// The rows that carry the pointer mark.
fn marked(buf: &Buffer) -> Vec<String> {
    rows(buf)
        .into_iter()
        .filter(|row| row.contains(marks::POINTER))
        .collect()
}

// ---- checking -------------------------------------------------------------

#[test]
fn space_checks_and_unchecks_the_cursors_item() {
    let mut list = toppings();
    assert_eq!(list.cursor(), 0);
    space(&mut list);
    assert!(!list.is_checked(0));
    space(&mut list);
    assert!(list.is_checked(0));
    key(&mut list, press(KeyCode::Down));
    space(&mut list);
    assert_eq!(list.selected(), [0, 1, 2, 3]);
}

#[test]
fn items_start_as_the_host_checked_them() {
    let list = toppings();
    assert_eq!(list.selected(), [0, 2, 3]);
    assert!(list.is_checked(2) && !list.is_checked(4));
    assert!(
        !list.is_checked(99),
        "past the last item nothing is checked"
    );
}

#[test]
fn a_selects_every_item_that_can_change_and_n_none() {
    let mut list = toppings();
    key(&mut list, press(KeyCode::Char('a')));
    assert_eq!(list.selected(), [0, 1, 2, 3, 4]);
    key(&mut list, press(KeyCode::Char('n')));
    assert_eq!(
        list.selected(),
        [2],
        "the item that cannot change keeps its check"
    );
    // An item that cannot change keeps an unchecked state too.
    let mut fixed = Checklist::new(
        "Fixed",
        vec![Item::new("on"), Item::new("off").disabled()],
        |indices: Vec<usize>| Outcome::Commit(Msg::Keep(indices)),
    );
    key(&mut fixed, press(KeyCode::Char('a')));
    assert_eq!(fixed.selected(), [0]);
}

#[test]
fn select_all_and_none_bind_their_letters_as_typed() {
    let mut list = toppings();
    key(&mut list, ctrl('a'));
    assert_eq!(list.selected(), [0, 2, 3], "Ctrl-A is not `a`");
    key(&mut list, ctrl('n'));
    assert_eq!(list.cursor(), 1, "Ctrl-N moves, as in every list");
    assert_eq!(list.selected(), [0, 2, 3]);
}

#[test]
fn the_count_says_how_many_are_checked_on_the_bottom_border() {
    let mut list = toppings();
    assert_eq!(list.summary(), "3 of 5 selected");
    let buf = drawn(&mut list, 60, 20);
    let bottom = rows(&buf)
        .into_iter()
        .find(|row| row.contains('└'))
        .expect("a bottom border");
    assert!(bottom.contains("└ 3 of 5 selected ─"), "{bottom}");
    key(&mut list, press(KeyCode::Char('a')));
    assert_eq!(list.summary(), "5 of 5 selected");
    key(&mut list, press(KeyCode::Char('n')));
    let drawn = text(&drawn(&mut list, 60, 20));
    assert!(drawn.contains("└ 1 of 5 selected ─"), "{drawn}");
}

// ---- moving ---------------------------------------------------------------

#[test]
fn every_list_key_moves_the_cursor() {
    for (down, up) in [
        (press(KeyCode::Down), press(KeyCode::Up)),
        (press(KeyCode::Char('j')), press(KeyCode::Char('k'))),
        (ctrl('n'), ctrl('p')),
    ] {
        let mut list = long(4);
        key(&mut list, down);
        assert_eq!(list.cursor(), 1, "{down:?}");
        key(&mut list, up);
        assert_eq!(list.cursor(), 0, "{up:?}");
    }
}

#[test]
fn the_cursor_wraps_at_both_ends() {
    let mut list = long(3);
    key(&mut list, press(KeyCode::Up));
    assert_eq!(list.cursor(), 2, "up from the first item wraps");
    key(&mut list, press(KeyCode::Down));
    assert_eq!(list.cursor(), 0, "down from the last item wraps");
}

#[test]
fn the_ends_keys_go_to_the_first_and_last_items_and_stop() {
    for (bottom, top) in [
        (typed('G'), press(KeyCode::Char('g'))),
        (press(KeyCode::End), press(KeyCode::Home)),
    ] {
        let mut list = long(6);
        key(&mut list, bottom);
        assert_eq!(list.cursor(), 5, "{bottom:?}");
        key(&mut list, bottom);
        assert_eq!(list.cursor(), 5, "{bottom:?} stays at the end");
        key(&mut list, top);
        assert_eq!(list.cursor(), 0, "{top:?}");
        key(&mut list, top);
        assert_eq!(list.cursor(), 0, "{top:?} stays at the start");
    }
}

#[test]
fn page_keys_move_by_the_rows_shown_and_stop_at_the_ends() {
    let mut list = long(20);
    let _ = drawn(&mut list, 40, 10);
    let page = list.window().page();
    assert!(page > 1 && page < 20, "{:?}", list.window());
    key(&mut list, press(KeyCode::PageDown));
    assert_eq!(list.cursor(), page);
    for _ in 0..10 {
        key(&mut list, press(KeyCode::PageDown));
    }
    assert_eq!(list.cursor(), 19, "paging stops at the last item");
    key(&mut list, press(KeyCode::PageUp));
    assert_eq!(list.cursor(), 19 - page);
    for _ in 0..10 {
        key(&mut list, press(KeyCode::PageUp));
    }
    assert_eq!(list.cursor(), 0, "and at the first");
}

// ---- items that cannot change -----------------------------------------------

#[test]
fn the_cursor_steps_over_an_item_that_cannot_change() {
    let mut list = toppings();
    key(&mut list, press(KeyCode::Down));
    assert_eq!(list.cursor(), 1);
    key(&mut list, press(KeyCode::Down));
    assert_eq!(list.cursor(), 3, "salt is stepped over");
    key(&mut list, press(KeyCode::Up));
    assert_eq!(list.cursor(), 1);
}

#[test]
fn the_cursor_starts_on_and_lands_on_items_that_can_change() {
    let items = || {
        vec![
            Item::new("first").disabled(),
            Item::new("second"),
            Item::new("third"),
            Item::new("last").disabled(),
        ]
    };
    let mut list = Checklist::new("Some", items(), |indices: Vec<usize>| {
        Outcome::Commit(Msg::Keep(indices))
    });
    assert_eq!(list.cursor(), 1);
    key(&mut list, press(KeyCode::End));
    assert_eq!(list.cursor(), 2, "the last item that can change");
    key(&mut list, press(KeyCode::Home));
    assert_eq!(list.cursor(), 1);
    key(&mut list, press(KeyCode::PageDown));
    assert_eq!(list.cursor(), 2);
}

#[test]
fn an_item_that_cannot_change_recedes_keeps_its_reason_and_is_never_marked() {
    let theme = Theme::default();
    let mut list = toppings();
    let buf = drawn(&mut list, 60, 20);
    let drawn = text(&buf);
    assert!(
        drawn.contains(&format!("  {} salt · always added", marks::OK)),
        "{drawn}"
    );
    let receding = unavailable_style(&theme);
    let salt = style_at(&buf, "salt").expect("salt is drawn");
    assert_eq!(salt.fg, receding.fg);
    assert!(salt.add_modifier.contains(theme.muted));
    let reason = style_at(&buf, "always added").expect("the reason is drawn");
    assert_ne!(reason.fg, receding.fg, "the reason stays plain");
    assert!(!reason.add_modifier.contains(theme.muted), "{reason:?}");
    for _ in 0..10 {
        key(&mut list, press(KeyCode::Down));
        let shown = marked(&drawn_list(&mut list));
        assert!(shown.iter().all(|row| !row.contains("salt")), "{shown:?}");
    }
}

fn drawn_list(list: &mut Checklist<Msg>) -> Buffer {
    drawn(list, 60, 20)
}

#[test]
fn a_list_where_nothing_can_change_marks_no_row_and_changes_nothing() {
    let mut list = Checklist::new(
        "Fixed",
        vec![
            Item::new("one").checked(true).disabled(),
            Item::new("two").disabled(),
        ],
        |indices: Vec<usize>| Outcome::Commit(Msg::Keep(indices)),
    );
    for event in [
        press(KeyCode::Down),
        press(KeyCode::End),
        press(KeyCode::Char(' ')),
        press(KeyCode::Char('a')),
        press(KeyCode::Char('n')),
    ] {
        assert_eq!(key(&mut list, event), Outcome::Consumed);
    }
    assert_eq!(list.selected(), [0]);
    assert!(marked(&drawn(&mut list, 60, 20)).is_empty());
    assert_eq!(
        key(&mut list, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Keep(vec![0])),
        "a checked item that cannot change still counts"
    );
}

// ---- confirming and closing -------------------------------------------------

#[test]
fn enter_hands_the_checked_indices_to_the_host() {
    let mut list = toppings();
    assert_eq!(
        key(&mut list, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Keep(vec![0, 2, 3]))
    );
    // The host decides what nothing checked means: here, a close.
    let mut empty = long(3);
    let mut list = Checklist::new("Empty", vec![Item::new("one")], |indices: Vec<usize>| {
        if indices.is_empty() {
            Outcome::Close
        } else {
            Outcome::Commit(Msg::Keep(indices))
        }
    });
    assert_eq!(key(&mut list, press(KeyCode::Enter)), Outcome::Close);
    assert_eq!(
        key(&mut empty, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Keep(Vec::new()))
    );
}

#[test]
fn the_close_keys_close_the_checklist_saying_nothing() {
    let mut list = toppings();
    space(&mut list);
    assert_eq!(key(&mut list, press(KeyCode::Esc)), Outcome::Close);
    assert_eq!(key(&mut list, press(KeyCode::Char('q'))), Outcome::Dismiss);
    assert_eq!(key(&mut list, ctrl('c')), Outcome::Dismiss);
    assert_eq!(list.selected(), [2, 3], "closing takes nothing back");
}

#[test]
fn other_keys_are_swallowed() {
    let mut list = toppings();
    for event in [press(KeyCode::Tab), press(KeyCode::Char('x')), ctrl('j')] {
        assert_eq!(key(&mut list, event), Outcome::Consumed, "{event:?}");
    }
    assert_eq!(list.cursor(), 0);
    assert_eq!(list.selected(), [0, 2, 3]);
}

#[test]
fn a_clone_confirms_as_the_original_does() {
    let mut list = toppings();
    space(&mut list);
    let mut copy = list.clone();
    assert_eq!(copy.selected(), list.selected());
    assert_eq!(
        key(&mut copy, press(KeyCode::Enter)),
        key(&mut list, press(KeyCode::Enter))
    );
}

#[test]
fn an_empty_list_draws_its_footer_and_confirms_nothing() {
    let mut list = long(0);
    for event in [
        press(KeyCode::Down),
        press(KeyCode::PageDown),
        press(KeyCode::End),
        press(KeyCode::Char(' ')),
        press(KeyCode::Char('a')),
    ] {
        assert_eq!(key(&mut list, event), Outcome::Consumed);
    }
    let drawn = text(&drawn(&mut list, 60, 20));
    assert!(drawn.contains("Esc cancel"), "{drawn}");
    assert!(drawn.contains("0 of 0 selected"), "{drawn}");
    assert_eq!(
        key(&mut list, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Keep(Vec::new()))
    );
}

// ---- the look ---------------------------------------------------------------

#[test]
fn the_cursor_row_shows_the_marker_and_the_highlight_and_checks_keep_their_mark() {
    let theme = Theme::default();
    let mut list = toppings();
    key(&mut list, press(KeyCode::Down));
    let buf = drawn(&mut list, 60, 20);
    let drawn = text(&buf);
    assert!(
        drawn.contains(&format!("{} {} onion", marks::POINTER, marks::BULLET)),
        "{drawn}"
    );
    let onion = style_at(&buf, "onion").expect("onion is drawn");
    assert!(
        onion.add_modifier.contains(theme.selection_active),
        "{onion:?}"
    );
    assert!(
        drawn.contains(&format!("  {} cheese", marks::OK)),
        "a checked row off the cursor: {drawn}"
    );
    let cheese = style_at(&buf, "cheese").expect("cheese is drawn");
    assert_eq!(cheese.fg, Some(theme.accent));
    assert!(cheese.add_modifier.contains(Modifier::BOLD));
    assert!(!cheese.add_modifier.contains(Modifier::REVERSED));
}

#[test]
fn a_long_label_ends_with_the_ellipsis_mark() {
    let mut list = Checklist::new(
        "Long",
        vec![
            Item::new("a label far too long to fit in a narrow checklist frame"),
            Item::new("short").detail("with a detail that is far too long to fit beside it"),
        ],
        |indices: Vec<usize>| Outcome::Commit(Msg::Keep(indices)),
    );
    let buf = drawn(&mut list, 40, 12);
    let shown = rows(&buf);
    let label = shown
        .iter()
        .find(|row| row.contains("a label far"))
        .expect("the long label");
    assert!(
        label.contains(&format!("{}│", marks::ELLIPSIS)),
        "cut with `…` at the frame's edge: {label}"
    );
    let detail = shown
        .iter()
        .find(|row| row.contains("short"))
        .expect("the short label");
    assert!(
        detail.contains("short · with a detail")
            && detail.contains(&format!("{}│", marks::ELLIPSIS)),
        "the detail is cut, the label kept: {detail}"
    );
}

#[test]
fn a_list_that_fits_is_as_tall_as_its_rows() {
    let mut list = toppings();
    let buf = drawn(&mut list, 80, 24);
    let shown = rows(&buf);
    let top = shown.iter().position(|row| row.contains('┌')).expect("top");
    let bottom = shown
        .iter()
        .position(|row| row.contains('└'))
        .expect("bottom");
    // A blank row, five items, a blank row, the footer's two rows.
    assert_eq!(bottom - top - 1, 1 + 5 + 1 + 2, "{}", text(&buf));
    assert!(shown[top + 1].trim_matches(['│', ' ']).is_empty());
    assert!(!list.window().overflows());
}

#[test]
fn the_footer_is_the_standard_table_in_the_one_grammar() {
    let mut list = toppings();
    let drawn = text(&drawn(&mut list, 100, 30));
    assert!(
        drawn.contains("↑/↓ move · Space toggle · a select all · n select none"),
        "{drawn}"
    );
    assert!(drawn.contains("Enter confirm · Esc cancel"), "{drawn}");
}

// ---- scrolling ---------------------------------------------------------------

/// Steps through every item of a long list on a `width` by `height`
/// screen, both ways, and checks that the cursor's item, the whole
/// footer, and the count are on screen at every step.
fn walk(width: u16, height: u16) {
    let mut list = long(14);
    let segments: Vec<String> = Footer::new(CHECKLIST_KEYS)
        .segments()
        .iter()
        .map(|segment| segment.text.clone())
        .collect();
    for step in 0..30 {
        let buf = drawn(&mut list, width, height);
        let text = text(&buf);
        let label = format!("Item {}", list.cursor());
        let shown = marked(&buf);
        assert!(
            shown.len() == 1 && shown[0].contains(&label),
            "{width}x{height}, step {step}: the cursor's item `{label}` is shown\n{text}"
        );
        for segment in &segments {
            assert!(
                text.contains(segment.as_str()),
                "{width}x{height}, step {step}: `{segment}` is shown\n{text}"
            );
        }
        assert!(text.contains("0 of 14 selected"), "{text}");
        let forward = step < 15;
        key(
            &mut list,
            press(if forward { KeyCode::Down } else { KeyCode::Up }),
        );
    }
}

#[test]
fn a_long_list_scrolls_wraps_and_keeps_its_footer_at_40x10() {
    walk(40, 10);
}

#[test]
fn a_long_list_scrolls_wraps_and_keeps_its_footer_at_80x24() {
    walk(80, 24);
}

#[test]
fn a_short_screen_gives_up_the_blank_rows_first() {
    let mut list = toppings();
    let buf = drawn(&mut list, 40, 10);
    let shown = rows(&buf);
    let top = shown.iter().position(|row| row.contains('┌')).expect("top");
    assert!(shown[top + 1].contains("cheese"), "{}", text(&buf));
    assert!(text(&buf).contains("Esc cancel"), "{}", text(&buf));
}

#[test]
fn scrolling_moves_no_further_than_the_cursor_needs() {
    let mut list = long(14);
    let _ = drawn(&mut list, 40, 10);
    assert!(list.window().overflows(), "{:?}", list.window());
    // Up to the last item, then back up one: the window stays put while
    // the cursor is inside it.
    key(&mut list, press(KeyCode::Up));
    let _ = drawn(&mut list, 40, 10);
    let bottom = list.window();
    assert_eq!(bottom.offset(), bottom.last_offset(), "{bottom:?}");
    key(&mut list, press(KeyCode::Up));
    let _ = drawn(&mut list, 40, 10);
    assert_eq!(list.window(), bottom, "one item up stays in view");
}

#[test]
fn an_overflowing_list_shows_the_scrollbar_and_one_that_fits_does_not() {
    let bar = |list: &mut Checklist<Msg>, width, height| {
        let buf = drawn(list, width, height);
        rows(&buf)
            .iter()
            .any(|row| row.contains('█') || row.contains('║'))
    };
    assert!(bar(&mut long(14), 60, 16), "a long list on a short screen");
    assert!(!bar(&mut long(3), 80, 24), "a short list that fits");
}

#[test]
fn the_cursor_stays_in_view_on_the_smallest_frames() {
    for height in 5..=9 {
        let mut list = long(6);
        for _ in 0..6 {
            let buf = drawn(&mut list, 40, height);
            let label = format!("Item {}", list.cursor());
            assert!(
                marked(&buf).iter().any(|row| row.contains(&label)),
                "40x{height}: {label}\n{}",
                text(&buf)
            );
            key(&mut list, press(KeyCode::Down));
        }
    }
}

#[test]
fn the_window_follows_the_cursor_to_an_end_key() {
    let mut list = long(20);
    let _ = drawn(&mut list, 40, 10);
    key(&mut list, press(KeyCode::End));
    let buf = drawn(&mut list, 40, 10);
    assert!(
        marked(&buf).iter().any(|row| row.contains("Item 19")),
        "{}",
        text(&buf)
    );
    assert!(find(&buf, "Item 0").is_none(), "{}", text(&buf));
}

// ---- goldens ---------------------------------------------------------------

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/checklist/goldens")
        .join(name)
}

#[test]
fn the_checklist_looks_as_its_goldens_say() {
    let mut list = toppings();
    key(&mut list, press(KeyCode::Down));
    let buf = drawn(&mut list, 64, 16);
    assert_golden(&golden_path("checklist.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
    let mut scrolled = long(14);
    key(&mut scrolled, press(KeyCode::Char('a')));
    key(&mut scrolled, press(KeyCode::Up));
    let narrow = drawn(&mut scrolled, 40, 10);
    assert_golden(&golden_path("narrow.golden"), &narrow, |path, text| {
        std::fs::write(path, text)
    });
}

// ---- the conventions ---------------------------------------------------------

/// A host of one stack of checklists over a screen of text.
struct Host(Theme);

impl Rig for Host {
    type State = ModalStack<Checklist<Msg>>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        Paragraph::new("the screen under the checklist").render(area, &mut buf);
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
fn a_checklist_keeps_the_conventions_every_modal_keeps() {
    let open = || {
        let mut stack = ModalStack::new();
        stack.push(toppings());
        stack
    };
    let open_long = || {
        let mut stack = ModalStack::new();
        stack.push(long(14));
        stack
    };
    let convention = Convention::new("Checklist", EnterMeans::Submits, SpaceMeans::Toggles, "");
    let keys = || [conformance::table("CHECKLIST_KEYS", CHECKLIST_KEYS)];
    let registry = Registry::new(Host(Theme::default()))
        .case(
            Case::new("Checklist", open)
                .moves()
                .keys(convention, keys())
                .footer(CHECKLIST_KEYS),
        )
        .case(
            Case::new("Checklist", open_long)
                .variant("long")
                .moves()
                .keys(convention, keys())
                .footer(CHECKLIST_KEYS),
        );
    conformance::assert_conventions(&registry);
}

#[test]
fn the_checklist_table_passes_the_table_checks() {
    assert!(mislabeled(CHECKLIST_KEYS).is_empty());
    assert!(unresolved(CHECKLIST_KEYS).is_empty());
    assert!(loose_characters(CHECKLIST_KEYS).is_empty());
    assert!(ctrl_c_first(CHECKLIST_KEYS));
    assert!(footer(CHECKLIST_KEYS).contains("a select all · n select none"));
}
