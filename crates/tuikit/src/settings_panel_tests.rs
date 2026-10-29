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
    unresolved,
};
use crate::overlay::ModalStack;
use crate::testkit::buffer::{find, rows, style_at};
use crate::testkit::conformance::{self, Case, Registry, Rig};
use crate::testkit::golden::assert_golden;
use crate::testkit::keys::{ctrl, press};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    View(usize),
    Wrap(bool),
    Mode,
    EditName,
    Find,
    Clear,
    Sort(&'static str),
    Pick(usize, usize),
}

/// The context the test panels read: the settings as they stand.
#[derive(Clone, Debug)]
struct Prefs {
    view: usize,
    wrap: bool,
    mode: &'static str,
    name: String,
    sort: &'static str,
    /// Extra sections of choices, to make a panel taller than a screen.
    extra: usize,
    /// Whether the panel shows a section of its own ahead of the views.
    lead: bool,
}

impl Prefs {
    fn new() -> Self {
        Self {
            view: 0,
            wrap: false,
            mode: "fast",
            name: "ada".into(),
            sort: "name",
            extra: 0,
            lead: false,
        }
    }

    fn long() -> Self {
        Self {
            extra: 5,
            ..Self::new()
        }
    }
}

const VIEWS: [&str; 3] = ["Alpha", "Beta", "Gamma"];

/// The rows of every test panel: views (a choice that does nothing when
/// it is in force), display settings (a checkbox, a cycle, an editor),
/// actions (a button and an editor that leaves), sorts (a choice that
/// always applies), and any extra sections.
fn sections(prefs: &Prefs) -> Vec<Section<&'static str, Msg>> {
    let mut sections = Vec::new();
    if prefs.lead {
        sections.push(Section::new(
            "Lead",
            vec![Row::button("lead", "Lead row", None)],
        ));
    }
    let views = VIEWS
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let active = prefs.view == index;
            let label = format!("{view} [{}]", index + 1);
            Row::choice(*view, label, active, (!active).then_some(Msg::View(index)))
        })
        .collect();
    sections.push(Section::new("View", views));
    sections.push(Section::new(
        "Display",
        vec![
            Row::checkbox(
                "wrap",
                "Wrap long lines",
                prefs.wrap,
                Msg::Wrap(!prefs.wrap),
            ),
            Row::cycle("mode", "mode", prefs.mode, Msg::Mode),
            Row::editor("name", "name", prefs.name.clone(), Msg::EditName),
        ],
    ));
    let clear = (prefs.wrap || prefs.name != "ada").then_some(Msg::Clear);
    sections.push(Section::new(
        "Actions",
        vec![
            Row::button("clear", "Clear all", clear),
            Row::editor("find", "find", "Enter searches", Msg::Find).leaves(),
        ],
    ));
    let sorts = [("by-name", "name"), ("by-date", "date")]
        .into_iter()
        .map(|(id, sort)| Row::choice(id, sort, prefs.sort == sort, Some(Msg::Sort(sort))))
        .collect();
    sections.push(Section::new("Sort", sorts));
    for (section, ids) in EXTRA_IDS.iter().enumerate().take(prefs.extra) {
        let rows = ids
            .iter()
            .zip(EXTRA_LABELS)
            .enumerate()
            .map(|(row, (id, label))| {
                Row::choice(
                    *id,
                    format!("Extra {section} {label}"),
                    row == 0,
                    Some(Msg::Pick(section, row)),
                )
            })
            .collect();
        sections.push(Section::new(format!("Extra {section}"), rows));
    }
    sections
}

/// The labels and ids of the extra sections' rows.
const EXTRA_LABELS: [&str; 4] = ["one", "two", "three", "four"];
const EXTRA_IDS: [[&str; 4]; 5] = [
    ["a0", "a1", "a2", "a3"],
    ["b0", "b1", "b2", "b3"],
    ["c0", "c1", "c2", "c3"],
    ["d0", "d1", "d2", "d3"],
    ["e0", "e1", "e2", "e3"],
];

/// A step that draws nothing, to open over a panel.
struct Blank;

impl Overlay<Msg, Prefs> for Blank {
    fn handle(&mut self, _key: KeyEvent, _prefs: &Prefs) -> Outcome<Msg> {
        Outcome::Consumed
    }

    fn render(&mut self, _area: Rect, _buf: &mut Buffer, _prefs: &Prefs, _theme: &Theme) {}
}

fn panel() -> SettingsPanel<&'static str, Msg, Prefs> {
    SettingsPanel::new("Settings", sections)
}

fn key(
    panel: &mut SettingsPanel<&'static str, Msg, Prefs>,
    prefs: &Prefs,
    event: KeyEvent,
) -> Outcome<Msg> {
    panel.handle(event, prefs)
}

/// The id of the row the cursor is on.
fn at(panel: &SettingsPanel<&'static str, Msg, Prefs>) -> &'static str {
    panel.cursor().copied().unwrap_or(VIEWS[0])
}

/// The panel drawn alone on a `width` by `height` screen.
fn drawn(
    panel: &mut SettingsPanel<&'static str, Msg, Prefs>,
    prefs: &Prefs,
    width: u16,
    height: u16,
) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    panel.draw(buf.area, &mut buf, prefs, &Theme::default());
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

/// Every row id of `prefs`'s panel, in order.
fn ids(prefs: &Prefs) -> Vec<&'static str> {
    sections(prefs)
        .into_iter()
        .flat_map(|section| section.rows)
        .map(|row| row.id)
        .collect()
}

// ---- the cursor --------------------------------------------------------------

#[test]
fn the_cursor_starts_on_the_first_row_or_where_the_host_opens_it() {
    let prefs = Prefs::new();
    let mut panel = panel();
    assert_eq!(panel.cursor(), None);
    assert!(marked(&drawn(&mut panel, &prefs, 80, 30))[0].contains("Alpha [1]"));
    let mut opened = panel.clone().at("mode");
    assert_eq!(opened.cursor(), Some(&"mode"));
    assert!(marked(&drawn(&mut opened, &prefs, 80, 30))[0].contains("mode"));
    // A row the model does not build leaves the cursor on the first row.
    let mut gone = panel.at("missing");
    assert!(marked(&drawn(&mut gone, &prefs, 80, 30))[0].contains("Alpha [1]"));
    key(&mut gone, &prefs, press(KeyCode::Down));
    assert_eq!(gone.cursor(), Some(&"Beta"));
}

#[test]
fn the_cursor_steps_over_the_headings_and_wraps_at_both_ends() {
    let prefs = Prefs::new();
    let order = ids(&prefs);
    let mut panel = panel();
    for expected in order.iter().skip(1) {
        key(&mut panel, &prefs, press(KeyCode::Down));
        assert_eq!(at(&panel), *expected);
    }
    key(&mut panel, &prefs, press(KeyCode::Down));
    assert_eq!(at(&panel), order[0], "Down wraps to the first row");
    key(&mut panel, &prefs, press(KeyCode::Up));
    assert_eq!(
        at(&panel),
        *order.last().expect("rows"),
        "Up wraps to the last"
    );
}

#[test]
fn every_list_key_moves_the_cursor() {
    let prefs = Prefs::new();
    for (forward, back) in [
        (press(KeyCode::Down), press(KeyCode::Up)),
        (press(KeyCode::Char('j')), press(KeyCode::Char('k'))),
        (ctrl('n'), ctrl('p')),
    ] {
        let mut panel = panel();
        key(&mut panel, &prefs, forward);
        assert_eq!(at(&panel), "Beta", "{forward:?}");
        key(&mut panel, &prefs, back);
        assert_eq!(at(&panel), "Alpha", "{back:?}");
    }
}

#[test]
fn the_ends_keys_go_to_the_first_and_last_rows_and_stop() {
    let prefs = Prefs::new();
    let last = *ids(&prefs).last().expect("rows");
    for (end, home) in [
        (press(KeyCode::End), press(KeyCode::Home)),
        (press(KeyCode::Char('G')), press(KeyCode::Char('g'))),
    ] {
        let mut panel = panel();
        key(&mut panel, &prefs, end);
        assert_eq!(at(&panel), last, "{end:?}");
        key(&mut panel, &prefs, end);
        assert_eq!(at(&panel), last, "{end:?} stops at the end");
        key(&mut panel, &prefs, home);
        assert_eq!(at(&panel), "Alpha", "{home:?}");
        key(&mut panel, &prefs, home);
        assert_eq!(at(&panel), "Alpha", "{home:?} stops at the start");
    }
}

#[test]
fn page_keys_move_by_the_rows_shown_and_stop_at_the_ends() {
    let prefs = Prefs::long();
    let mut panel = panel();
    let _ = drawn(&mut panel, &prefs, 60, 16);
    let page = panel.window().page();
    assert!(panel.window().overflows(), "{:?}", panel.window());
    let lines = |panel: &SettingsPanel<&'static str, Msg, Prefs>| {
        let places = body::places(&sections(&prefs));
        let ids = ids(&prefs);
        let index = ids.iter().position(|id| *id == at(panel)).expect("a row");
        places[index].line
    };
    key(&mut panel, &prefs, press(KeyCode::PageDown));
    let first = lines(&panel);
    assert!(
        (page + 1..=page + 3).contains(&first),
        "a page down lands on the row nearest a page away: line {first}, page {page}"
    );
    for _ in 0..20 {
        key(&mut panel, &prefs, press(KeyCode::PageDown));
    }
    assert_eq!(at(&panel), *ids(&prefs).last().expect("rows"), "stops");
    for _ in 0..20 {
        key(&mut panel, &prefs, press(KeyCode::PageUp));
    }
    assert_eq!(at(&panel), "Alpha", "stops at the first row");
}

#[test]
fn the_cursor_keeps_its_row_when_rows_appear_above_it() {
    let mut prefs = Prefs::new();
    let mut panel = panel().at("wrap");
    prefs.lead = true;
    key(&mut panel, &prefs, press(KeyCode::Down));
    assert_eq!(
        at(&panel),
        "mode",
        "the cursor moved from its row, not its index"
    );
}

// ---- acting on a row ---------------------------------------------------------

#[test]
fn space_reports_each_rows_message_at_once_and_the_panel_stays_open() {
    let prefs = Prefs::new();
    let space = |id| {
        let mut panel = panel().at(id);
        key(&mut panel, &prefs, press(KeyCode::Char(' ')))
    };
    assert_eq!(space("Beta"), Outcome::Stay(Msg::View(1)));
    assert_eq!(space("Alpha"), Outcome::Consumed, "the view in force");
    assert_eq!(space("wrap"), Outcome::Stay(Msg::Wrap(true)));
    assert_eq!(space("mode"), Outcome::Stay(Msg::Mode));
    assert_eq!(
        space("name"),
        Outcome::Stay(Msg::EditName),
        "opens the editor"
    );
    assert_eq!(space("clear"), Outcome::Consumed, "nothing to clear");
    assert_eq!(
        space("find"),
        Outcome::Commit(Msg::Find),
        "a row that leaves"
    );
    assert_eq!(space("by-date"), Outcome::Stay(Msg::Sort("date")));
    assert_eq!(
        space("by-name"),
        Outcome::Stay(Msg::Sort("name")),
        "a choice the host applies again"
    );
}

#[test]
fn enter_applies_a_choice_and_closes_and_otherwise_acts_or_closes() {
    let prefs = Prefs::new();
    let enter = |id| {
        let mut panel = panel().at(id);
        key(&mut panel, &prefs, press(KeyCode::Enter))
    };
    assert_eq!(enter("Beta"), Outcome::Commit(Msg::View(1)));
    assert_eq!(enter("Alpha"), Outcome::Close, "the view in force");
    assert_eq!(enter("by-date"), Outcome::Commit(Msg::Sort("date")));
    assert_eq!(enter("by-name"), Outcome::Commit(Msg::Sort("name")));
    assert_eq!(enter("wrap"), Outcome::Close, "Space applied it already");
    assert_eq!(enter("mode"), Outcome::Close, "Space applied it already");
    assert_eq!(enter("name"), Outcome::Stay(Msg::EditName), "as on Space");
    assert_eq!(enter("clear"), Outcome::Consumed, "nothing to clear");
    assert_eq!(enter("find"), Outcome::Commit(Msg::Find));
    let changed = Prefs {
        wrap: true,
        ..Prefs::new()
    };
    let mut clear = panel().at("clear");
    assert_eq!(
        key(&mut clear, &changed, press(KeyCode::Enter)),
        Outcome::Stay(Msg::Clear),
        "a button presses on Enter as on Space"
    );
}

#[test]
fn the_rows_are_read_from_the_context_on_every_key() {
    let mut prefs = Prefs::new();
    let mut panel = panel().at("wrap");
    assert_eq!(
        key(&mut panel, &prefs, press(KeyCode::Char(' '))),
        Outcome::Stay(Msg::Wrap(true))
    );
    prefs.wrap = true;
    assert_eq!(
        key(&mut panel, &prefs, press(KeyCode::Char(' '))),
        Outcome::Stay(Msg::Wrap(false)),
        "no copy of the context was kept"
    );
}

#[test]
fn the_close_keys_close_the_panel_saying_nothing() {
    let prefs = Prefs::new();
    let mut panel = panel();
    assert_eq!(key(&mut panel, &prefs, press(KeyCode::Esc)), Outcome::Close);
    assert_eq!(
        key(&mut panel, &prefs, press(KeyCode::Char('q'))),
        Outcome::Dismiss
    );
    assert_eq!(key(&mut panel, &prefs, ctrl('c')), Outcome::Dismiss);
}

#[test]
fn other_keys_are_swallowed() {
    let prefs = Prefs::new();
    let mut panel = panel().at("wrap");
    for code in [
        KeyCode::Char('x'),
        KeyCode::Tab,
        KeyCode::Left,
        KeyCode::F(2),
    ] {
        assert_eq!(key(&mut panel, &prefs, press(code)), Outcome::Consumed);
        assert_eq!(at(&panel), "wrap");
    }
    assert_eq!(
        key(&mut panel, &prefs, ctrl('g')),
        Outcome::Consumed,
        "not `g`"
    );
    assert_eq!(at(&panel), "wrap");
}

#[test]
fn an_empty_panel_draws_its_footer_and_closes_on_enter() {
    let mut empty: SettingsPanel<&'static str, Msg, ()> =
        SettingsPanel::new("Empty", |(): &()| Vec::new());
    assert_eq!(empty.handle(press(KeyCode::Down), &()), Outcome::Consumed);
    assert_eq!(
        empty.handle(press(KeyCode::Char(' ')), &()),
        Outcome::Consumed
    );
    assert_eq!(empty.handle(press(KeyCode::Enter), &()), Outcome::Close);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
    empty.draw(buf.area, &mut buf, &(), &Theme::default());
    assert!(text(&buf).contains("Esc close"), "{}", text(&buf));
    assert!(marked(&buf).is_empty());
}

#[test]
fn a_clone_shares_the_hosts_row_model() {
    let prefs = Prefs::new();
    let mut original = panel().at("wrap");
    let mut copy = original.clone();
    assert_eq!(
        key(&mut copy, &prefs, press(KeyCode::Char(' '))),
        key(&mut original, &prefs, press(KeyCode::Char(' ')))
    );
    assert_eq!(copy.title(), "Settings");
    assert_eq!(copy.sections(&prefs).len(), 4);
}

#[test]
fn debug_names_the_state_and_not_the_model() {
    let debug = format!("{:?}", panel().at("wrap"));
    assert!(
        debug.contains("Settings") && debug.contains("wrap"),
        "{debug}"
    );
    assert!(!debug.contains("model"), "{debug}");
}

// ---- drawing -----------------------------------------------------------------

#[test]
fn each_kind_of_row_reads_as_it_should() {
    let prefs = Prefs {
        wrap: true,
        ..Prefs::new()
    };
    let mut panel = panel();
    let buf = drawn(&mut panel, &prefs, 80, 30);
    let drawn = text(&buf);
    for expected in [
        "View",
        &format!("{} Alpha [1]  active", marks::POINTER),
        "  Beta [2]",
        &format!("  {} Wrap long lines", marks::CHECKED),
        "  mode fast  [ Cycle ]",
        "  name ada  [ Edit ]",
        "  find Enter searches  [ Edit ]",
        "  Clear all",
        "  name  active",
    ] {
        assert!(drawn.contains(expected), "`{expected}` in\n{drawn}");
    }
    let theme = Theme::default();
    let heading = style_at(&buf, "Display").expect("a heading");
    assert!(heading.add_modifier.contains(Modifier::BOLD));
    let wrap = style_at(&buf, "Wrap long lines").expect("the checkbox");
    assert!(
        wrap.add_modifier.contains(Modifier::BOLD),
        "checked is bold"
    );
    let beta = style_at(&buf, "Beta").expect("a choice");
    assert!(!beta.add_modifier.contains(Modifier::BOLD));
    let alpha = style_at(&buf, "Alpha").expect("the cursor's row");
    assert!(
        alpha.add_modifier.contains(theme.selection_active),
        "{alpha:?}"
    );
    let off = Prefs::new();
    let unchecked = text(&drawn_at(&off));
    assert!(
        unchecked.contains(&format!("  {} Wrap long lines", marks::UNCHECKED)),
        "{unchecked}"
    );
}

fn drawn_at(prefs: &Prefs) -> Buffer {
    drawn(&mut panel(), prefs, 80, 30)
}

#[test]
fn the_name_column_lines_the_values_up() {
    let prefs = Prefs::new();
    let mut wide = panel().name_width(8);
    let drawn = text(&drawn(&mut wide, &prefs, 80, 30));
    assert!(drawn.contains("  mode     fast  [ Cycle ]"), "{drawn}");
    assert!(drawn.contains("  name     ada  [ Edit ]"), "{drawn}");
    assert!(drawn.contains("  find     Enter searches"), "{drawn}");
}

#[test]
fn under_a_step_the_cursor_row_takes_the_inactive_look() {
    let theme = Theme::default();
    let prefs = Prefs::new();
    let mut panel = panel().at("name");
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 30));
    panel.draw_under_step(buf.area, &mut buf, &prefs, &theme);
    assert!(marked(&buf)[0].contains("name ada"), "the marker stays");
    let name = style_at(&buf, "name ada").expect("the cursor's row");
    assert!(
        name.add_modifier.contains(theme.selection_inactive),
        "{name:?}"
    );
    assert!(
        !name.add_modifier.contains(theme.selection_active),
        "{name:?}"
    );
    // On a stack, the panel under a step of its own draws that way, and
    // under an overlay opened on its own it draws as it does alone.
    let cursor_row = |step: bool| {
        let mut stack: ModalStack<Box<dyn Overlay<Msg, Prefs>>> = ModalStack::new();
        stack.push(Box::new(panel.clone()));
        if step {
            stack.push_step(Box::new(Blank));
        } else {
            stack.push(Box::new(Blank));
        }
        let mut both = Buffer::empty(Rect::new(0, 0, 80, 30));
        stack.draw(both.area, &mut both, &prefs, &theme);
        style_at(&both, "name ada").expect("the panel's cursor row")
    };
    assert!(
        cursor_row(true)
            .add_modifier
            .contains(theme.selection_inactive)
    );
    assert!(
        cursor_row(false)
            .add_modifier
            .contains(theme.selection_active)
    );
}

#[test]
fn a_long_row_ends_with_the_ellipsis_mark() {
    let prefs = Prefs {
        name: "a value far too long to fit beside its name in a narrow panel".into(),
        ..Prefs::new()
    };
    let mut panel = panel();
    let shown = rows(&drawn(&mut panel, &prefs, 40, 30));
    let row = shown
        .iter()
        .find(|row| row.contains("a value far"))
        .expect("the long row");
    assert!(
        row.contains(&format!("{}│", marks::ELLIPSIS)),
        "cut with `…` at the frame's edge: {row}"
    );
}

#[test]
fn a_panel_that_fits_is_as_tall_as_its_rows_and_its_footer() {
    let prefs = Prefs::new();
    let mut panel = panel();
    let buf = drawn(&mut panel, &prefs, 100, 40);
    let shown = rows(&buf);
    let top = shown.iter().position(|row| row.contains('┌')).expect("top");
    let bottom = shown
        .iter()
        .position(|row| row.contains('└'))
        .expect("bottom");
    // Four headings, ten rows, three blank lines between sections, a
    // blank row and the footer's two rows.
    assert_eq!(bottom - top - 1, 4 + 10 + 3 + 1 + 2, "{}", text(&buf));
    assert!(
        shown[top + 1].contains("View"),
        "no blank row above: {}",
        text(&buf)
    );
    assert!(!panel.window().overflows());
}

#[test]
fn the_footer_is_the_standard_table_in_the_one_grammar() {
    let prefs = Prefs::new();
    let drawn = text(&drawn(&mut panel(), &prefs, 100, 40));
    assert!(
        drawn.contains("↑/↓ move · Space select/toggle · Enter apply & close"),
        "{drawn}"
    );
    assert!(
        drawn.contains("Esc close · changes apply as you make them"),
        "broken between hints: {drawn}"
    );
}

// ---- scrolling ---------------------------------------------------------------

/// Steps through every row of a long panel on a `width` by `height`
/// screen, both ways, and checks that the cursor's row and the whole
/// footer are on screen at every step.
fn walk(width: u16, height: u16) {
    let prefs = Prefs::long();
    let order = ids(&prefs);
    let segments: Vec<String> = Footer::new(SETTINGS_KEYS)
        .segments()
        .iter()
        .map(|segment| segment.text.clone())
        .collect();
    let mut panel = panel();
    let steps = order.len() * 2 + 2;
    for step in 0..steps {
        let buf = drawn(&mut panel, &prefs, width, height);
        let shown = text(&buf);
        let index = order.iter().position(|id| *id == at(&panel)).expect("row");
        let marked = marked(&buf);
        assert_eq!(marked.len(), 1, "{width}x{height}, step {step}\n{shown}");
        let row = sections(&prefs)
            .into_iter()
            .flat_map(|section| section.rows)
            .nth(index)
            .expect("the row");
        let label = row.label;
        assert!(
            marked[0].contains(&label),
            "{width}x{height}, step {step}: the cursor's row `{label}` is shown\n{shown}"
        );
        for segment in &segments {
            assert!(
                shown.contains(segment.as_str()),
                "{width}x{height}, step {step}: `{segment}` is shown\n{shown}"
            );
        }
        let forward = step < steps / 2;
        key(
            &mut panel,
            &prefs,
            press(if forward { KeyCode::Down } else { KeyCode::Up }),
        );
    }
}

#[test]
fn a_long_panel_scrolls_wraps_and_keeps_its_footer_at_40x10() {
    walk(40, 10);
}

#[test]
fn a_long_panel_scrolls_wraps_and_keeps_its_footer_at_80x24() {
    walk(80, 24);
}

#[test]
fn a_sections_first_row_brings_its_heading_into_view() {
    let prefs = Prefs::long();
    let mut panel = panel();
    let _ = drawn(&mut panel, &prefs, 60, 16);
    key(&mut panel, &prefs, press(KeyCode::End));
    let _ = drawn(&mut panel, &prefs, 60, 16);
    // Back up to the top by wrapping: the first row's heading shows.
    key(&mut panel, &prefs, press(KeyCode::Down));
    let buf = drawn(&mut panel, &prefs, 60, 16);
    let shown = rows(&buf);
    let top = shown.iter().position(|row| row.contains('┌')).expect("top");
    assert!(shown[top + 1].contains("View"), "{}", text(&buf));
    // Down into a section that starts below the window: its heading
    // comes into view with its first row.
    let mut panel = panel_on_the_first_action(&prefs);
    let buf = drawn(&mut panel, &prefs, 60, 12);
    let shown = text(&buf);
    assert!(shown.contains("Actions"), "{shown}");
}

/// A long panel on a 60x12 screen, moved down a row at a time, drawing
/// as it goes, onto the first row of the Actions section.
fn panel_on_the_first_action(prefs: &Prefs) -> SettingsPanel<&'static str, Msg, Prefs> {
    let mut panel = panel();
    let _ = drawn(&mut panel, prefs, 60, 12);
    for _ in 0..6 {
        key(&mut panel, prefs, press(KeyCode::Down));
        let _ = drawn(&mut panel, prefs, 60, 12);
    }
    assert_eq!(at(&panel), "clear");
    panel
}

#[test]
fn scrolling_moves_no_further_than_the_cursor_needs() {
    let prefs = Prefs::long();
    let mut panel = panel();
    let _ = drawn(&mut panel, &prefs, 40, 10);
    assert!(panel.window().overflows(), "{:?}", panel.window());
    key(&mut panel, &prefs, press(KeyCode::Up));
    let _ = drawn(&mut panel, &prefs, 40, 10);
    let bottom = panel.window();
    assert_eq!(bottom.offset(), bottom.last_offset(), "{bottom:?}");
    key(&mut panel, &prefs, press(KeyCode::Up));
    let _ = drawn(&mut panel, &prefs, 40, 10);
    assert_eq!(panel.window(), bottom, "one row up stays in view");
}

#[test]
fn an_overflowing_panel_shows_the_scrollbar_and_one_that_fits_does_not() {
    let bar = |prefs: &Prefs, width, height| {
        let buf = drawn(&mut panel(), prefs, width, height);
        rows(&buf)
            .iter()
            .any(|row| row.contains('█') || row.contains('║'))
    };
    assert!(
        bar(&Prefs::long(), 60, 20),
        "a long panel on a short screen"
    );
    assert!(!bar(&Prefs::new(), 100, 40), "a panel that fits");
}

#[test]
fn the_cursor_stays_in_view_on_the_smallest_frames() {
    let prefs = Prefs::new();
    for height in 5..=9 {
        let mut panel = panel();
        for _ in 0..12 {
            let buf = drawn(&mut panel, &prefs, 40, height);
            assert_eq!(marked(&buf).len(), 1, "40x{height}\n{}", text(&buf));
            key(&mut panel, &prefs, press(KeyCode::Down));
        }
    }
}

#[test]
fn the_window_follows_the_cursor_to_an_end_key() {
    let prefs = Prefs::long();
    let mut panel = panel();
    let _ = drawn(&mut panel, &prefs, 40, 10);
    key(&mut panel, &prefs, press(KeyCode::End));
    let buf = drawn(&mut panel, &prefs, 40, 10);
    assert!(
        marked(&buf).iter().any(|row| row.contains("Extra 4 four")),
        "{}",
        text(&buf)
    );
    assert!(find(&buf, "Alpha").is_none(), "{}", text(&buf));
}

// ---- goldens ---------------------------------------------------------------

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/settings_panel/goldens")
        .join(name)
}

#[test]
fn the_panel_looks_as_its_goldens_say() {
    let prefs = Prefs {
        wrap: true,
        ..Prefs::new()
    };
    let mut panel = panel().at("mode");
    let buf = drawn(&mut panel, &prefs, 72, 30);
    assert_golden(&golden_path("panel.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
    let mut under = Buffer::empty(Rect::new(0, 0, 72, 30));
    panel.draw_under_step(under.area, &mut under, &prefs, &Theme::default());
    assert_golden(&golden_path("under_step.golden"), &under, |path, text| {
        std::fs::write(path, text)
    });
    let long = Prefs::long();
    let mut scrolled = panel_on_the_first_action(&long);
    let narrow = drawn(&mut scrolled, &long, 40, 10);
    assert_golden(&golden_path("narrow.golden"), &narrow, |path, text| {
        std::fs::write(path, text)
    });
}

// ---- the conventions ---------------------------------------------------------

/// A host of one stack of settings panels over a screen of text, whose
/// settings stay as they are.
struct Host(Theme, Prefs);

impl Rig for Host {
    type State = ModalStack<SettingsPanel<&'static str, Msg, Prefs>>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        Paragraph::new("the screen under the panel").render(area, &mut buf);
        stack.draw::<Msg, Prefs>(area, &mut buf, &self.1, &self.0);
        buf
    }

    fn close_top(&self, stack: &mut Self::State) {
        stack.pop();
    }

    fn press(&self, stack: &mut Self::State, key: KeyEvent) {
        if let Some(outcome) = stack.route::<Msg, Prefs>(key, &self.1) {
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
fn a_settings_panel_keeps_the_conventions_every_modal_keeps() {
    let open = || {
        let mut stack = ModalStack::new();
        stack.push(panel());
        stack
    };
    let convention = Convention::new(
        "Settings",
        EnterMeans::Activates,
        SpaceMeans::Toggles,
        "changes apply as they are made, so Esc keeps them",
    );
    let keys = || [conformance::table("SETTINGS_KEYS", SETTINGS_KEYS)];
    let registry = Registry::new(Host(Theme::default(), Prefs::new())).case(
        Case::new("Settings", open)
            .moves()
            .keys(convention, keys())
            .footer(SETTINGS_KEYS),
    );
    conformance::assert_conventions(&registry);
    let long = Registry::new(Host(Theme::default(), Prefs::long())).case(
        Case::new("Settings", open)
            .variant("long")
            .moves()
            .keys(convention, keys())
            .footer(SETTINGS_KEYS),
    );
    conformance::assert_conventions(&long);
}

#[test]
fn the_settings_table_passes_the_table_checks() {
    assert!(mislabeled(SETTINGS_KEYS).is_empty());
    assert!(unresolved(SETTINGS_KEYS).is_empty());
    assert!(loose_characters(SETTINGS_KEYS).is_empty());
    assert!(ctrl_c_first(SETTINGS_KEYS));
    assert!(footer(SETTINGS_KEYS).ends_with("changes apply as you make them"));
}

/// A host whose stack holds a panel with an editor open over it as a
/// step.
struct StepHost(Theme, Prefs);

impl Rig for StepHost {
    type State = ModalStack<Box<dyn Overlay<Msg, Prefs>>>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        Paragraph::new("the screen under the panel").render(area, &mut buf);
        stack.draw::<Msg, Prefs>(area, &mut buf, &self.1, &self.0);
        buf
    }

    fn close_top(&self, stack: &mut Self::State) {
        stack.pop();
    }

    fn press(&self, stack: &mut Self::State, key: KeyEvent) {
        if let Some(outcome) = stack.route::<Msg, Prefs>(key, &self.1) {
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

/// The panel on a row the prompt does not cover, with an editor (a
/// prompt) open over it as a step.
fn with_editor() -> ModalStack<Box<dyn Overlay<Msg, Prefs>>> {
    let mut stack: ModalStack<Box<dyn Overlay<Msg, Prefs>>> = ModalStack::new();
    stack.push(Box::new(panel().at("Gamma")));
    let prompt = crate::text_prompt::TextPrompt::new("name", "ada", |text: &str| {
        crate::text_prompt::PromptResult::Back(Msg::Pick(text.len(), 0))
    });
    stack.push_step(Box::new(prompt));
    stack
}

#[test]
fn an_editor_over_the_panel_keeps_the_conventions_as_a_step() {
    use crate::testkit::conformance::Check;
    let convention = Convention::new("Prompt", EnterMeans::Submits, SpaceMeans::Types, "");
    let keys = || {
        [conformance::table(
            "TEXT_INPUT_KEYS",
            crate::keys::TEXT_INPUT_KEYS,
        )]
    };
    let step = Registry::new(StepHost(Theme::default(), Prefs::new())).case(
        Case::new("Prompt", with_editor)
            .typed()
            .step()
            .keys(convention, keys())
            .footer(crate::keys::TEXT_INPUT_KEYS),
    );
    conformance::assert_conventions(&step);
    // Not marked as a step, the panel's receding cursor row is a change
    // outside the prompt's frame.
    let plain = Registry::new(StepHost(Theme::default(), Prefs::new())).case(
        Case::new("Prompt", with_editor)
            .typed()
            .keys(convention, keys())
            .footer(crate::keys::TEXT_INPUT_KEYS),
    );
    let failures = conformance::check_conventions_of(&plain, &[Check::Contained]);
    assert!(
        failures
            .iter()
            .any(|failure| failure.to_string().contains("outside the frame")),
        "{failures:?}"
    );
}
