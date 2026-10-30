//! The frame around the rows reads right in every view and at every
//! width: the left pane's tab strip never cuts a view's name, the header
//! never cuts one of its parts and its session count agrees with what the
//! active view lists, the focused pane is the one framed in the focus
//! accent, and every key the status bar names does what it says.
//!
//! Each is checked on the rendered frame, the way the operator reads
//! it, so it holds whichever control draws the strip, the header, or the
//! panes.

use ratatui::crossterm::event::Event;
use tuikit::keys::{KeyRow, key_label, resolve};
use tuikit::status_bar::KeyHintBar;

use super::*;
use crate::filter::{HarnessFilter, PrStateFilter, PrStateKey};
use crate::tui::app::Focus;
use crate::tui::keybindings::{MAIN_VIEW_KEYS, MainKey, RIGHT_PANE_KEYS};
use crate::tui::keymap::Action;
use crate::tui::rows::RowKind;

/// The word the tab strip uses for each view, in strip order.
const TAB_LABELS: [(View, &str); 6] = [
    (View::Sessions, "sessions"),
    (View::Mux, "mux"),
    (View::Prs, "prs"),
    (View::Pinned, "pinned"),
    (View::History, "history"),
    (View::Search, "search"),
];

fn tab_label(view: View) -> &'static str {
    TAB_LABELS
        .iter()
        .find(|(candidate, _)| *candidate == view)
        .map(|(_, label)| *label)
        .expect("every view has a tab")
}

/// The words of the left pane's title: what lies between its top-left
/// corner and the next corner, without the border, the focus marker,
/// and the separators.
fn tab_strip_words(frame: &str) -> Vec<String> {
    let line = frame.lines().nth(1).expect("the panes' top border");
    let start = line.find('┌').expect("the left pane's top-left corner") + '┌'.len_utf8();
    let end = line[start..]
        .find('┐')
        .map_or(line.len(), |offset| start + offset);
    line[start..end]
        .replace(['─', '►'], " ")
        .split_whitespace()
        .filter(|word| *word != "·")
        .map(str::to_string)
        .collect()
}

/// Every width from 40 to 120 columns, both layouts (stacked below the
/// default threshold of 100, side by side from it), every view, and both
/// focus states: the strip shows whole view names in order, always the
/// active one, and ends with `…` when it leaves any out.
#[test]
fn the_tab_strip_never_cuts_a_view_name_at_any_width() {
    let labels: Vec<&str> = TAB_LABELS.iter().map(|(_, label)| *label).collect();
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        for focus_right in [false, true] {
            if focus_right {
                app.update(Msg::CycleFocus);
            }
            for width in 40..=120 {
                let shown = frame(&mut app, width, 12);
                let words = tab_strip_words(&shown);
                let context = format!(
                    "{view:?} at {width} columns, right pane focused: {focus_right}: {words:?}\n{}",
                    shown.lines().nth(1).unwrap_or_default()
                );
                let shortened = words.last().is_some_and(|word| word == "…");
                let names = &words[..words.len() - usize::from(shortened)];
                for name in names {
                    assert!(
                        labels.contains(&name.as_str()),
                        "`{name}` is not a whole view name: {context}"
                    );
                }
                assert!(
                    names.iter().any(|name| name == tab_label(view)),
                    "the active view's name is missing: {context}"
                );
                let order: Vec<usize> = names
                    .iter()
                    .map(|name| labels.iter().position(|label| label == name).unwrap())
                    .collect();
                assert!(
                    order.windows(2).all(|pair| pair[0] < pair[1]),
                    "the names are out of order: {context}"
                );
                assert_eq!(
                    shortened,
                    names.len() < labels.len(),
                    "the strip ends with `…` exactly when it leaves a view out: {context}"
                );
            }
        }
    }
}

/// Below 40 columns the strip keeps shortening, down to a cut active
/// name, without the frame failing to draw.
#[test]
fn the_tab_strip_draws_in_any_narrow_pane() {
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        for width in 1..40 {
            let shown = frame(&mut app, width, 12);
            assert!(shown.lines().count() > 1, "{view:?} at {width} columns");
        }
    }
}

/// A pane wide enough for the whole strip names every view; one a few
/// cells short of it drops the names after the active one first.
#[test]
fn the_tab_strip_names_every_view_when_they_fit() {
    let mut app = app_in(View::Sessions);
    // Side by side at 108 columns the left pane is 54 wide.
    let words = tab_strip_words(&frame(&mut app, 108, 12));
    assert_eq!(words, TAB_LABELS.map(|(_, label)| label.to_string()));
    // At 100 columns it is 50 wide: `search` gives way.
    let words = tab_strip_words(&frame(&mut app, 100, 12));
    assert_eq!(
        words,
        ["sessions", "mux", "prs", "pinned", "history", "…"].map(String::from)
    );
    // With Search active, `history` gives way instead.
    let mut app = app_in(View::Search);
    let words = tab_strip_words(&frame(&mut app, 100, 12));
    assert_eq!(
        words,
        ["sessions", "mux", "prs", "pinned", "search", "…"].map(String::from)
    );
}

/// The header row of `app` drawn `width` columns wide, without the
/// blanks after it.
fn header_row(app: &mut App, width: u16) -> String {
    let shown = frame(app, width, 12);
    shown
        .lines()
        .next()
        .expect("the header line")
        .trim_end()
        .to_string()
}

/// Between two parts of the header's counts, and before a chip section.
const COUNT_GAP: &str = " · ";
const SECTION_GAP: &str = "  ·  ";

/// The parts of a header that shows them all, each with what goes before
/// it when something precedes it: the counts joined by ` · `, a chip
/// section set apart by `  ·  `.
fn header_parts(full: &str) -> Vec<(&'static str, String)> {
    let mut parts = Vec::new();
    for (section, text) in full.split(SECTION_GAP).enumerate() {
        for (index, part) in text.split(COUNT_GAP).enumerate() {
            let gap = if section > 0 && index == 0 {
                SECTION_GAP
            } else {
                COUNT_GAP
            };
            parts.push((gap, part.to_string()));
        }
    }
    parts
}

/// Every width from 40 to 120 columns, both layouts, every view: the
/// header shows whole parts of the header a wide screen shows, in order,
/// always the session count, and ends with `…` exactly when it leaves a
/// part out. It never runs past the screen for the edge to cut.
#[test]
fn the_header_never_cuts_a_part_at_any_width() {
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        let full = header_row(&mut app, 300);
        let parts = header_parts(&full);
        assert!(parts.len() >= 3, "{view:?}: {full}");
        for width in 40..=120 {
            let shown = header_row(&mut app, width);
            let context = format!("{view:?} at {width} columns: {shown:?} (whole: {full:?})");
            assert!(
                tuikit::text::width(&shown) <= usize::from(width),
                "{context}"
            );
            let (body, shortened) = match shown.strip_suffix(tuikit::glyphs::marks::MORE) {
                Some(body) => (body, true),
                None => (shown.as_str(), false),
            };
            let mut rest = body;
            let mut kept = Vec::new();
            for (gap, part) in &parts {
                let drawn = if kept.is_empty() {
                    part.clone()
                } else {
                    format!("{gap}{part}")
                };
                if let Some(after) = rest.strip_prefix(drawn.as_str()) {
                    rest = after;
                    kept.push(part.as_str());
                }
            }
            assert!(rest.is_empty(), "a part is cut: {context}");
            assert!(
                kept.iter().any(|part| part.ends_with(" sessions")),
                "the session count is missing: {context}"
            );
            assert_eq!(
                shortened,
                kept.len() < parts.len(),
                "the header ends with `…` exactly when it leaves a part out: {context}"
            );
        }
    }
}

/// The header gives way by importance, not from its end: at 40 columns
/// the freshness, the session count, and the mux count stay, and what
/// follows them goes.
#[test]
fn the_header_keeps_its_most_important_parts_when_narrow() {
    let mut app = app_in(View::Sessions);
    let full = header_row(&mut app, 300);
    assert!(full.starts_with("updated "), "{full}");
    assert!(
        tuikit::text::width(&full) > 40,
        "the fixture's header is wider: {full}"
    );
    let narrow = header_row(&mut app, 40);
    let kept: Vec<&str> = narrow
        .trim_end_matches(tuikit::glyphs::marks::MORE)
        .split(COUNT_GAP)
        .collect();
    assert_eq!(kept.len(), 3, "{narrow}");
    assert!(kept[0].starts_with("updated "), "{narrow}");
    assert!(kept[1].ends_with(" sessions"), "{narrow}");
    assert!(kept[2].ends_with(" mux"), "{narrow}");
    assert!(narrow.ends_with(tuikit::glyphs::marks::MORE), "{narrow}");
}

/// The cells of a pane's border other than its top row, which carries
/// the title: the bottom row and the two sides.
fn border_cells(pane: Rect) -> Vec<(u16, u16)> {
    let mut cells: Vec<(u16, u16)> = (pane.x..pane.right())
        .map(|x| (x, pane.bottom() - 1))
        .collect();
    for y in pane.y..pane.bottom() - 1 {
        cells.push((pane.x, y));
        cells.push((pane.right() - 1, y));
    }
    cells
}

/// The focus cue (house rule 9): the pane with focus draws its border and
/// the `►` before its title in the theme's focus accent, and the other
/// keeps the plain border and a blank where the marker goes, in every
/// view, side by side and stacked, whichever pane has focus. The titles'
/// own text keeps its look.
#[test]
fn only_the_focused_pane_is_framed_in_the_focus_accent() {
    use ratatui::style::Color;

    let theme = crate::tui::Theme::default();
    for (width, height) in [(160_u16, 30_u16), (80, 30)] {
        let areas = panes(width, height);
        for view in ALL_VIEWS {
            let mut app = app_in(view);
            for _ in 0..2 {
                let buf = buffer_of(&mut app, width, height);
                let label = format!("{view:?} at {width}x{height}, {:?} focused", app.focus());
                let (focused, other) = match app.focus() {
                    crate::tui::app::Focus::Left => (areas.left, areas.right),
                    crate::tui::app::Focus::Right => (areas.right, areas.left),
                };
                for (x, y) in border_cells(focused) {
                    assert_eq!(
                        buf[(x, y)].fg,
                        theme.panel_focus_accent,
                        "{label}: ({x}, {y})"
                    );
                }
                for (x, y) in border_cells(other) {
                    assert_eq!(buf[(x, y)].fg, Color::Reset, "{label}: ({x}, {y})");
                }
                for (pane, mark) in [(focused, tuikit::glyphs::marks::POINTER), (other, " ")] {
                    let cell = &buf[(pane.x + 2, pane.y)];
                    assert_eq!(cell.symbol(), mark, "{label}: the marker");
                    assert_eq!(cell.fg, theme.panel_focus_accent, "{label}: the marker");
                    for x in [pane.x, pane.right() - 1] {
                        let corner = buf[(x, pane.y)].fg;
                        let tinted = pane == focused;
                        assert_eq!(corner == theme.panel_focus_accent, tinted, "{label}: ({x})");
                    }
                }
                app.update(Msg::CycleFocus);
            }
        }
    }
    // The right pane's title names the node's kind in bold, in no color
    // of the border's.
    let mut app = app_in(View::Mux);
    select(&mut app, "conspectus-main");
    app.update(Msg::CycleFocus);
    let buf = buffer_of(&mut app, 160, 30);
    let title = Rect {
        height: 1,
        ..panes(160, 30).right
    };
    let at = tuikit::testkit::buffer::find_in(&buf, title, "mux").expect("the right title");
    assert_eq!(buf[(at.x, at.y)].fg, Color::Reset);
    assert!(
        buf[(at.x, at.y)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
}

/// The header's `N/M sessions` and `M sessions` cell, as numbers.
fn header_count(app: &mut App) -> (Option<usize>, usize) {
    let shown = frame(app, 160, 12);
    let header = shown.lines().next().expect("the header line").to_string();
    let cell = header
        .split(" · ")
        .find_map(|segment| segment.trim().strip_suffix(" sessions"))
        .unwrap_or_else(|| panic!("no session count: {header}"));
    let cell = cell.rsplit(' ').next().unwrap_or(cell);
    match cell.split_once('/') {
        Some((shown, total)) => (
            Some(shown.parse().expect("a count")),
            total.parse().expect("a total"),
        ),
        None => (None, cell.parse().expect("a total")),
    }
}

fn sessions_in(snapshot: &GraphSnapshot) -> usize {
    snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::AgentSession(_)))
        .count()
}

fn session_rows(app: &App) -> usize {
    app.tree()
        .rows
        .iter()
        .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .count()
}

/// With nothing narrowing it, no view reads as if sessions were filtered
/// out: Mux and PRs list only the sessions with their muxes and PRs,
/// Pinned and History list entities of every kind, and Search lists
/// nothing before a search, and each shows the plain total.
#[test]
fn the_header_counts_every_session_when_nothing_narrows_the_view() {
    let total = sessions_in(&world());
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        assert_eq!(
            header_count(&mut app),
            (None, total),
            "{view:?} lists {} session rows",
            session_rows(&app)
        );
    }
}

/// A filter narrows the count where the view honours it, to the sessions
/// the view still lists, and leaves it whole where the view ignores
/// filters.
#[test]
fn the_header_count_follows_a_filter_in_the_views_that_apply_it() {
    let total = sessions_in(&world());
    let filter = RowFilter {
        harness: Some(HarnessFilter::from_values(["codex"])),
        ..RowFilter::default()
    };
    for view in ALL_VIEWS {
        let mut app = app_over_filtered(view, &world(), filter.clone());
        let expected = match view {
            View::Sessions | View::Prs => Some(session_rows(&app)),
            View::Mux => Some(
                app.tree()
                    .rows
                    .iter()
                    .filter_map(|row| match &row.kind {
                        RowKind::MuxSession(mux) => Some(mux.attached_count),
                        _ => None,
                    })
                    .sum(),
            ),
            View::Pinned | View::History | View::Search => None,
        };
        let expected = expected.filter(|shown| *shown != total);
        assert_eq!(header_count(&mut app), (expected, total), "{view:?}");
        if matches!(view, View::Sessions | View::Mux | View::Prs) {
            assert!(
                expected.is_some(),
                "{view:?}: the codex filter leaves sessions out of the world"
            );
        }
    }
}

/// A PR state filter narrows only the PRs view, and its count follows.
#[test]
fn a_pr_filter_narrows_the_prs_count() {
    let total = sessions_in(&world());
    let filter = RowFilter {
        pr_state: Some(PrStateFilter::from_values([PrStateKey::Merged])),
        ..RowFilter::default()
    };
    let mut app = app_over_filtered(View::Prs, &world(), filter);
    let listed = session_rows(&app);
    let expected = (listed != total).then_some(listed);
    assert_eq!(header_count(&mut app), (expected, total));
}

/// The Search view counts the sessions its search found, whether it
/// lists them as session rows or, flat, as the hits that name them.
#[test]
fn the_header_counts_the_sessions_a_search_found() {
    let total = sessions_in(&world());
    let mut app = searched(View::Search, RowFilter::default());
    assert_eq!(header_count(&mut app), (Some(1), total));
    // Pinned and History ignore the search the lists are filtered by.
    for view in [View::Pinned, View::History] {
        let mut app = searched(view, RowFilter::default());
        assert_eq!(header_count(&mut app), (None, total), "{view:?}");
    }
}

/// What the program-wide keymap does with each of the main view's status
/// bar keys: the action a hint that names the key promises.
fn promised(key: MainKey) -> Action {
    let msg = |msg: Msg| Action::Msg(Box::new(msg));
    match key {
        MainKey::Default => Action::DefaultAction,
        MainKey::Attach => Action::Attach,
        MainKey::View => Action::View,
        MainKey::Resume => Action::Resume,
        MainKey::Menu => msg(Msg::OpenRowMenu),
        MainKey::Up => msg(Msg::NavUp),
        MainKey::Down => msg(Msg::NavDown),
        MainKey::Collapse => msg(Msg::CollapseRow),
        MainKey::Expand => msg(Msg::ExpandRow),
        MainKey::Focus => msg(Msg::CycleFocus),
        MainKey::Refresh => Action::Refresh,
        MainKey::HardRefresh => Action::HardRefresh,
        MainKey::Quit => msg(Msg::Quit),
        MainKey::Back => msg(Msg::NavBack),
        MainKey::Messages => Action::OpenMessages,
    }
}

/// The keys `hints` names that do not do what they say in `app`'s scope,
/// as findings: [`unresolved_in`] with the main view's table.
fn unresolved_hints(app: &App, hints: &KeyHintBar) -> Vec<String> {
    unresolved_in(app, hints, MAIN_VIEW_KEYS)
}

/// The keys `hints` names that do not do what they say in `app`'s scope.
/// Each key is pressed the way the loops press it (`action_for_event`):
/// with the right pane focused, a key of the right pane's table must
/// reach the explorer as that table's action, under one of the right
/// pane's own hints; any other key must be a row of `main`, the main
/// view's table, that the program-wide keymap turns into the action the
/// row stands for.
fn unresolved_in(app: &App, hints: &KeyHintBar, main: &[KeyRow<MainKey>]) -> Vec<String> {
    let pane_hints: Vec<String> = match (app.explorer(), app.graph_source()) {
        (Some(explorer), Some(source)) => explorer.footer(RIGHT_PANE_KEYS, &source),
        _ => tuikit::explorer::hint(RIGHT_PANE_KEYS),
    }
    .segments()
    .iter()
    .map(|segment| segment.text.clone())
    .collect();
    let mut findings = Vec::new();
    for hint in hints.hints() {
        for key in hint.keys() {
            let event = key.example();
            let routed = crate::tui::runtime::action_for_event(app, Event::Key(event));
            let pane = (app.focus() == Focus::Right)
                .then(|| resolve(RIGHT_PANE_KEYS, event))
                .flatten();
            if pane.is_some() && !pane_hints.contains(&hint.text()) {
                findings.push(format!(
                    "`{}` ({}) is the right pane's key, which says {pane_hints:?}",
                    key_label(key),
                    hint.text()
                ));
                continue;
            }
            let expected = match pane {
                Some(explorer) => Some(Action::Msg(Box::new(Msg::Explorer(explorer)))),
                None => resolve(main, event).map(promised),
            };
            if expected.is_none() || routed != expected {
                findings.push(format!(
                    "`{}` ({}) is {routed:?}, not {expected:?}",
                    key_label(key),
                    hint.text()
                ));
            }
        }
    }
    findings
}

/// In every view, on every row, with either pane focused, and with a way
/// back from a reveal, every key the status bar names does what its hint
/// says: the status bar is generated from the tables of the scope that
/// has the keys, so it cannot advertise a key that does something else
/// there. (Over a modal it names the modal's keys, which the conformance
/// suite's `Hints` check holds to the modal's footer.)
#[test]
fn every_key_the_status_bar_names_does_what_it_says() {
    let mut checked = 0;
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        let ids: Vec<RowId> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
        for id in ids {
            app.set_selection(id.clone());
            for _ in 0..2 {
                let hints = crate::tui::ui::status_hints(&app);
                assert!(!hints.is_empty(), "{}: no hints", describe(view, &id));
                let findings = unresolved_hints(&app, &hints);
                assert!(
                    findings.is_empty(),
                    "{} ({:?} focused): {}\n{}",
                    describe(view, &id),
                    app.focus(),
                    hints.text(),
                    findings.join("\n")
                );
                checked += hints.keys().count();
                app.update(Msg::CycleFocus);
            }
        }
    }
    assert!(checked > 100, "only {checked} keys checked");
    // A reveal leaves a way back, which the bar names.
    let mut app = app_in(View::Pinned);
    // A reveal that fails leaves the rows as they were.
    let rows = app.tree().rows.len();
    let revealed = (0..rows).any(|index| {
        let id = app.tree().rows[index].id.clone();
        app.set_selection(id);
        app.update(Msg::RevealSelected);
        app.can_nav_back()
    });
    assert!(revealed, "no Pinned row reveals its entity");
    let hints = crate::tui::ui::status_hints(&app);
    assert!(hints.text().contains("Backspace back"), "{}", hints.text());
    assert_eq!(unresolved_hints(&app, &hints), Vec::<String>::new());
}

/// The check fails a hint that names a key no table of its scope binds,
/// one whose key does something else, and one of the left pane's tree
/// keys named while the right pane has the keys.
#[test]
fn a_hint_that_does_not_do_what_it_says_is_caught() {
    use tuikit::keys::{ch, key, shown};
    const UNBOUND: &[KeyRow<MainKey>] = &[shown(ch('z'), MainKey::Quit, "z", "quit")];
    const ELSEWHERE: &[KeyRow<MainKey>] = &[shown(ch('q'), MainKey::Refresh, "q", "refresh")];
    const TREE: &[KeyRow<MainKey>] = &[shown(
        key(ratatui::crossterm::event::KeyCode::Down),
        MainKey::Down,
        "↓",
        "move",
    )];
    let mut app = app_in(View::Sessions);
    // A table whose row names a key the keymap leaves unbound, or binds
    // to something else.
    for table in [UNBOUND, ELSEWHERE] {
        let bar = KeyHintBar::new().table(table, 1);
        assert_eq!(unresolved_in(&app, &bar, table).len(), 1, "{}", bar.text());
    }
    let tree = KeyHintBar::new().table(TREE, 1);
    assert!(
        unresolved_hints(&app, &tree).is_empty(),
        "the left pane moves"
    );
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Right);
    assert_eq!(
        unresolved_hints(&app, &tree).len(),
        1,
        "on the right pane the explorer takes the down arrow, not the tree"
    );
}
