use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget};

use super::*;
use crate::glyphs::marks;
use crate::keys::{
    Convention, EnterMeans, KeyRole, SpaceMeans, ctrl, ctrl_c_first, footer, include, key,
    loose_characters, mislabeled, shown, unresolved,
};
use crate::overlay::ModalStack;
use crate::testkit::buffer::{find, rows, style_at};
use crate::testkit::conformance::{self, Case, Registry, Rig};
use crate::testkit::golden::assert_golden;
use crate::testkit::keys::{ctrl as ctrl_key, press, typed};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Pick(&'static str),
    Kept(String),
}

type Rows = Palette<Item<&'static str>, Msg>;

fn item(label: &str, detail: &str, value: &'static str) -> Item<&'static str> {
    Item::new(label, value).detail(detail)
}

/// Three candidates filtered by the word filter, as a picker lists them.
fn three() -> Rows {
    Palette::new(
        "Pick one",
        vec![
            item("alpha", "running", "a"),
            item("beta", "gone · stopped", "b"),
            item("alphabet", "running", "c"),
        ],
        |row: &Item<&'static str>| Outcome::Commit(Msg::Pick(row.value)),
    )
    .ranked_by(every_word)
}

/// `count` candidates, `Item 0` to `Item {count - 1}`.
fn many(count: usize) -> Rows {
    let rows = (0..count)
        .map(|n| Item::new(format!("Item {n}"), "n"))
        .collect();
    Palette::new("Many", rows, |row: &Item<&'static str>| {
        Outcome::Commit(Msg::Pick(row.value))
    })
    .ranked_by(every_word)
}

fn key_in<R: PaletteRow, A: PaletteAction>(
    palette: &mut Palette<R, Msg, A>,
    event: KeyEvent,
) -> Outcome<Msg> {
    palette.handle(event, &())
}

fn type_in<R: PaletteRow, A: PaletteAction>(palette: &mut Palette<R, Msg, A>, text: &str) {
    for c in text.chars() {
        key_in(palette, typed(c));
    }
}

fn labels(palette: &Rows) -> Vec<&str> {
    palette.listed().map(|row| row.label.as_str()).collect()
}

fn current(palette: &Rows) -> Option<&str> {
    palette.current().map(|row| row.label.as_str())
}

/// The palette drawn alone on a `width` by `height` screen.
fn drawn<R: PaletteRow, A: PaletteAction>(
    palette: &mut Palette<R, Msg, A>,
    width: u16,
    height: u16,
) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    palette.draw(buf.area, &mut buf, &Theme::default());
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

// ---- filtering ----------------------------------------------------------------

#[test]
fn typing_narrows_the_list_to_rows_matching_every_word() {
    let mut palette = three();
    assert_eq!(labels(&palette), ["alpha", "beta", "alphabet"]);
    type_in(&mut palette, "alph");
    assert_eq!(labels(&palette), ["alpha", "alphabet"]);
    type_in(&mut palette, "abe");
    assert_eq!(labels(&palette), ["alphabet"]);
    type_in(&mut palette, " RUN");
    assert_eq!(labels(&palette), ["alphabet"], "words match ignoring case");
}

#[test]
fn the_detail_is_searchable_too() {
    let mut palette = three();
    type_in(&mut palette, "gone");
    assert_eq!(labels(&palette), ["beta"]);
}

#[test]
fn letters_the_lists_of_other_controls_bind_type_into_the_query() {
    let mut palette = three();
    type_in(&mut palette, "qjk ");
    assert_eq!(palette.query(), "qjk ");
    assert!(palette.listed().len() == 0);
}

#[test]
fn deleting_widens_the_list_again() {
    let mut palette = three();
    type_in(&mut palette, "beta");
    assert_eq!(labels(&palette), ["beta"]);
    for _ in 0..4 {
        key_in(&mut palette, press(KeyCode::Backspace));
    }
    assert_eq!(labels(&palette), ["alpha", "beta", "alphabet"]);
}

#[test]
fn a_palette_without_a_ranker_lists_its_rows_as_given() {
    let mut palette: Rows = Palette::new(
        "Results",
        vec![item("one", "", "1"), item("two", "", "2")],
        |row: &Item<&'static str>| Outcome::Commit(Msg::Pick(row.value)),
    );
    type_in(&mut palette, "zzz");
    assert_eq!(labels(&palette), ["one", "two"], "the host ranks them");
}

#[test]
fn a_host_ranker_orders_the_results() {
    let mut palette = three().ranked_by(|query: &str, rows: &[Item<&'static str>]| {
        // Shortest label first, among those that start with the query.
        let mut hits: Vec<usize> = (0..rows.len())
            .filter(|&i| rows[i].label.starts_with(query))
            .collect();
        hits.sort_by_key(|&i| std::cmp::Reverse(rows[i].label.len()));
        hits
    });
    type_in(&mut palette, "al");
    assert_eq!(labels(&palette), ["alphabet", "alpha"]);
    assert_eq!(palette.ranked(), [2, 0]);
}

#[test]
fn every_word_ignores_case_and_keeps_the_hosts_order() {
    let rows = [
        item("North Gate", "open", ""),
        item("south gate", "closed", ""),
    ];
    assert_eq!(every_word("GATE", &rows), [0, 1]);
    assert_eq!(every_word("gate  CLOSED", &rows), [1]);
    assert_eq!(every_word("   ", &rows), [0, 1]);
    assert!(every_word("west", &rows).is_empty());
}

// ---- the cursor ---------------------------------------------------------------

#[test]
fn the_cursor_moves_with_the_arrows_and_wraps() {
    let mut palette = three();
    key_in(&mut palette, press(KeyCode::Down));
    assert_eq!(current(&palette), Some("beta"));
    key_in(&mut palette, press(KeyCode::Up));
    key_in(&mut palette, press(KeyCode::Up));
    assert_eq!(current(&palette), Some("alphabet"));
    key_in(&mut palette, press(KeyCode::Down));
    assert_eq!(current(&palette), Some("alpha"), "wraps past the end");
}

#[test]
fn ctrl_n_and_ctrl_p_move_as_in_every_list() {
    let mut palette = three();
    key_in(&mut palette, ctrl_key('n'));
    assert_eq!(palette.cursor(), 1);
    key_in(&mut palette, ctrl_key('p'));
    key_in(&mut palette, ctrl_key('p'));
    assert_eq!(palette.cursor(), 2);
    assert_eq!(palette.query(), "", "the chords type nothing");
}

#[test]
fn narrowing_keeps_the_cursor_on_the_list() {
    let mut palette = three();
    key_in(&mut palette, press(KeyCode::Down));
    key_in(&mut palette, press(KeyCode::Down));
    type_in(&mut palette, "beta");
    assert_eq!(palette.cursor(), 0);
    assert_eq!(current(&palette), Some("beta"));
}

#[test]
fn an_edit_puts_the_cursor_on_the_best_result() {
    let mut palette = three();
    key_in(&mut palette, press(KeyCode::Down));
    key_in(&mut palette, press(KeyCode::Down));
    type_in(&mut palette, "a");
    assert_eq!(labels(&palette), ["alpha", "beta", "alphabet"]);
    assert_eq!(palette.cursor(), 0, "the first result, not the third");
}

#[test]
fn the_cursor_can_start_on_a_given_row() {
    let palette = three().at(2);
    assert_eq!(current(&palette), Some("alphabet"));
    let unlisted = three().at(9);
    assert_eq!(unlisted.cursor(), 0);
}

#[test]
fn new_rows_keep_the_cursor_while_it_is_still_a_result() {
    let mut palette = three();
    key_in(&mut palette, press(KeyCode::Down));
    palette.set_rows(vec![
        item("one", "", "1"),
        item("two", "", "2"),
        item("six", "", "6"),
    ]);
    assert_eq!(palette.cursor(), 1);
    assert_eq!(current(&palette), Some("two"));
    palette.set_rows(vec![item("solo", "", "s")]);
    assert_eq!(palette.cursor(), 0, "past the end goes to the first");
    palette.set_rows(Vec::new());
    assert_eq!(palette.current(), None);
}

#[test]
fn new_rows_are_ranked_against_the_query() {
    let mut palette = three();
    type_in(&mut palette, "t");
    palette.set_rows(vec![item("ant", "", "1"), item("bee", "", "2")]);
    assert_eq!(labels(&palette), ["ant"]);
}

#[test]
fn page_keys_move_by_the_rows_shown_and_stop_at_the_ends() {
    let mut palette = many(30);
    let buf = drawn(&mut palette, 60, 16);
    let page = palette.window().page();
    assert!(page > 2 && page < 30, "{page}\n{}", text(&buf));
    key_in(&mut palette, press(KeyCode::PageDown));
    assert_eq!(palette.cursor(), page);
    for _ in 0..10 {
        key_in(&mut palette, press(KeyCode::PageDown));
    }
    assert_eq!(palette.cursor(), 29, "stops at the last result");
    key_in(&mut palette, press(KeyCode::PageUp));
    assert_eq!(palette.cursor(), 29 - page);
    for _ in 0..10 {
        key_in(&mut palette, press(KeyCode::PageUp));
    }
    assert_eq!(palette.cursor(), 0, "stops at the first");
}

#[test]
fn moving_in_an_empty_list_does_nothing() {
    let mut palette = three();
    type_in(&mut palette, "zzz");
    for code in [KeyCode::Up, KeyCode::Down, KeyCode::PageDown] {
        assert_eq!(key_in(&mut palette, press(code)), Outcome::Consumed);
        assert_eq!(palette.cursor(), 0);
    }
}

// ---- the outcome --------------------------------------------------------------

#[test]
fn enter_hands_the_row_under_the_cursor_to_the_host() {
    let mut palette = three();
    key_in(&mut palette, press(KeyCode::Down));
    assert_eq!(
        key_in(&mut palette, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Pick("b"))
    );
}

#[test]
fn enter_on_an_empty_list_just_closes() {
    let mut palette = three();
    type_in(&mut palette, "zzz");
    assert!(labels(&palette).is_empty());
    assert_eq!(key_in(&mut palette, press(KeyCode::Enter)), Outcome::Close);
}

#[test]
fn escape_closes_and_ctrl_c_dismisses_saying_nothing() {
    let mut palette = three();
    assert_eq!(key_in(&mut palette, press(KeyCode::Esc)), Outcome::Close);
    assert_eq!(key_in(&mut palette, ctrl_key('c')), Outcome::Dismiss);
}

#[test]
fn the_hosts_answer_is_what_the_palette_reports() {
    let mut palette = Palette::new(
        "Stay",
        vec![item("one", "", "1")],
        |row: &Item<&'static str>| Outcome::Stay(Msg::Pick(row.value)),
    );
    assert_eq!(
        key_in(&mut palette, press(KeyCode::Enter)),
        Outcome::Stay(Msg::Pick("1"))
    );
}

#[test]
fn a_clone_picks_as_the_original_does() {
    let mut palette = three();
    key_in(&mut palette, press(KeyCode::Down));
    let mut copy = palette.clone();
    assert_eq!(
        key_in(&mut copy, press(KeyCode::Enter)),
        key_in(&mut palette, press(KeyCode::Enter))
    );
}

#[test]
fn the_query_takes_a_paste() {
    let palette = three();
    assert!(Overlay::<Msg, ()>::takes_text(&palette));
    assert!(!Overlay::<Msg, ()>::fills_screen(&palette));
}

#[test]
fn the_query_can_be_filled_as_if_typed() {
    let mut palette = three().debounced(Duration::from_millis(50));
    key_in(&mut palette, press(KeyCode::Down));
    palette.set_query("bet");
    assert_eq!(palette.query(), "bet");
    assert_eq!(labels(&palette), ["beta", "alphabet"]);
    assert_eq!(palette.cursor(), 0);
    assert!(palette.debounce().is_some_and(Debounce::is_pending));
    let started = three().with_query("beta");
    assert_eq!(started.query(), "beta");
    assert_eq!(started.listed().len(), 1);
}

// ---- host keys and scopes -------------------------------------------------------

/// A host's own palette actions: the palette's, plus one it handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostKey {
    Own(PaletteKey),
    Keep,
}

impl KeyAction for HostKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Own(key) => key.role(),
            Self::Keep => KeyRole::Other,
        }
    }
}

impl PaletteAction for HostKey {
    fn palette_key(self) -> Option<PaletteKey> {
        match self {
            Self::Own(key) => Some(key),
            Self::Keep => None,
        }
    }
}

const HOST_KEYS: &[KeyRow<HostKey>] = &[
    crate::keys::alias(ctrl('c'), HostKey::Own(PaletteKey::Abort)),
    shown(
        key(KeyCode::Up),
        HostKey::Own(PaletteKey::Up),
        marks::ARROW_UP,
        "move",
    ),
    shown(
        key(KeyCode::Down),
        HostKey::Own(PaletteKey::Down),
        marks::ARROW_DOWN,
        "move",
    ),
    shown(
        key(KeyCode::Enter),
        HostKey::Own(PaletteKey::Pick),
        "Enter",
        "open",
    ),
    shown(
        key(KeyCode::Esc),
        HostKey::Own(PaletteKey::Cancel),
        "Esc",
        "close",
    ),
    shown(
        key(KeyCode::Tab),
        HostKey::Own(PaletteKey::Scope),
        "Tab",
        "look here/everywhere",
    ),
    shown(
        ctrl('o'),
        HostKey::Own(PaletteKey::Options),
        "Ctrl-O",
        "set options",
    ),
    shown(ctrl('s'), HostKey::Keep, "Ctrl-S", "keep"),
];

/// The standard rows with a scope key and an options key: a table that
/// builds on [`PALETTE_KEYS`].
const SCOPED_KEYS: &[KeyRow<PaletteKey>] = &[
    include(PALETTE_KEYS),
    shown(key(KeyCode::Tab), PaletteKey::Scope, "Tab", "switch scope"),
    shown(ctrl('o'), PaletteKey::Options, "Ctrl-O", "set options"),
];

fn scoped() -> Palette<Item<&'static str>, Msg, HostKey> {
    three()
        .keys(HOST_KEYS)
        .scopes(["Here", "Everywhere"])
        .scopes(["Coarse", "Fine"])
        .options(Options::new([("Hidden", false), ("Archived", true)]))
        .debounced(Duration::from_millis(200))
}

#[test]
fn a_host_table_drives_the_palette_and_keeps_its_own_actions() {
    let mut palette = scoped();
    let keep = ctrl_key('s');
    assert_eq!(palette.host_action(keep), Some(HostKey::Keep));
    assert_eq!(palette.host_action(press(KeyCode::Down)), None);
    assert_eq!(
        palette.action(press(KeyCode::Down)),
        Some(HostKey::Own(PaletteKey::Down))
    );
    // Handed to the palette, the host's own action is swallowed.
    assert_eq!(key_in(&mut palette, keep), Outcome::Consumed);
    assert_eq!(palette.query(), "");
    key_in(&mut palette, press(KeyCode::Down));
    assert_eq!(palette.cursor(), 1);
    assert_eq!(
        key_in(&mut palette, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Pick("b"))
    );
    assert_eq!(
        palette.footer().text(),
        "↑/↓ move · Enter open · Esc close · Tab look here/everywhere · Ctrl-O set options · Ctrl-S keep"
    );
}

#[test]
fn the_scope_key_switches_the_first_strip_and_starts_over() {
    let mut palette = scoped();
    key_in(&mut palette, press(KeyCode::Down));
    let before = Instant::now();
    key_in(&mut palette, press(KeyCode::Tab));
    assert_eq!(palette.scope(0), 1);
    assert_eq!(palette.scope(1), 0, "the host's strip stays");
    assert_eq!(palette.cursor(), 0);
    assert!(!palette.take_due(before, false), "still resting");
    assert!(palette.take_due(Instant::now(), true), "a search is owed");
    key_in(&mut palette, press(KeyCode::Tab));
    assert_eq!(palette.scope(0), 0, "wraps back to the first choice");
    palette.set_scope(1, 1);
    palette.set_scope(1, 5);
    palette.set_scope(7, 1);
    assert_eq!(palette.scope(1), 1);
    assert_eq!(palette.scope(7), 0);
}

#[test]
fn the_title_shows_its_strips_and_count() {
    let mut palette = scoped();
    key_in(&mut palette, press(KeyCode::Tab));
    let buf = drawn(&mut palette, 100, 20);
    assert!(
        text(&buf).contains("Pick one  Here [ Everywhere ]  [ Coarse ] Fine  (3/3)"),
        "{}",
        text(&buf)
    );
    let chosen = style_at(&buf, "[ Everywhere ]").expect("the chosen scope");
    let look = crate::chip::SegmentedTitle::chosen_look(&Theme::default());
    assert_eq!(chosen.fg, look.fg);
    assert!(chosen.add_modifier.contains(look.add_modifier));
}

#[test]
fn the_count_is_listed_of_total_listed_or_none() {
    let mut palette = three();
    type_in(&mut palette, "alph");
    assert!(text(&drawn(&mut palette, 80, 16)).contains("Pick one  (2/3) "));
    let mut listed = three().count(Count::Listed);
    type_in(&mut listed, "alph");
    assert!(text(&drawn(&mut listed, 80, 16)).contains("Pick one  (2) "));
    let mut none = three().count(Count::None);
    let buf = drawn(&mut none, 80, 16);
    let title = rows(&buf)
        .into_iter()
        .find(|row| row.contains("Pick one"))
        .expect("the title");
    assert!(title.contains(" Pick one ─"), "{title}");
    assert!(!title.contains('('), "{title}");
}

#[test]
fn a_scope_key_with_no_strips_does_nothing() {
    let mut palette = three().keys(SCOPED_KEYS);
    key_in(&mut palette, press(KeyCode::Down));
    assert_eq!(key_in(&mut palette, press(KeyCode::Tab)), Outcome::Consumed);
    assert_eq!(palette.cursor(), 1);
    assert_eq!(palette.scope(0), 0);
    assert_eq!(key_in(&mut palette, ctrl_key('o')), Outcome::Consumed);
    assert!(!palette.options_open(), "no options to open");
}

// ---- the options pane -----------------------------------------------------------

#[test]
fn the_options_pane_opens_toggles_and_goes_back() {
    let mut palette = scoped();
    key_in(&mut palette, press(KeyCode::Down));
    key_in(&mut palette, ctrl_key('o'));
    assert!(palette.options_open());
    assert!(
        !Overlay::<Msg, ()>::takes_text(&palette),
        "nothing is typed"
    );
    assert_eq!(palette.action(press(KeyCode::Down)), None);
    // j moves in the pane; Space turns the option on.
    key_in(&mut palette, typed('j'));
    assert_eq!(palette.options_pane().map(Options::cursor), Some(1));
    let before = Instant::now();
    key_in(&mut palette, typed(' '));
    assert!(!palette.options_pane().is_some_and(|o| o.is_on(1)));
    key_in(&mut palette, press(KeyCode::Enter));
    assert!(palette.options_pane().is_some_and(|o| o.is_on(1)));
    assert_eq!(palette.cursor(), 0, "the results start over");
    assert!(!palette.take_due(before, false));
    assert!(palette.take_due(Instant::now(), true), "a search is owed");
    assert_eq!(palette.query(), "", "nothing typed into the query");
    assert_eq!(
        palette.footer().text(),
        "↑/↓ move · Space toggle · Esc back"
    );
    // Esc goes back to the results; Ctrl-O opens the pane again on its
    // first option, and closes it.
    assert_eq!(key_in(&mut palette, press(KeyCode::Esc)), Outcome::Consumed);
    assert!(!palette.options_open());
    key_in(&mut palette, ctrl_key('o'));
    assert_eq!(palette.options_pane().map(Options::cursor), Some(0));
    key_in(&mut palette, ctrl_key('o'));
    assert!(!palette.options_open());
}

#[test]
fn q_and_ctrl_c_in_the_options_pane_dismiss_the_palette() {
    let mut palette = scoped();
    palette.open_options();
    assert_eq!(key_in(&mut palette, typed('q')), Outcome::Dismiss);
    let mut palette = scoped();
    palette.open_options();
    assert_eq!(key_in(&mut palette, ctrl_key('c')), Outcome::Dismiss);
}

#[test]
fn the_host_can_restore_and_read_its_options() {
    let mut palette = scoped();
    if let Some(options) = palette.options_mut() {
        options.set(0, true);
        options.set(9, true);
    }
    let options = palette.options_pane().expect("options");
    assert!(options.is_on(0) && options.is_on(1) && !options.is_on(9));
    assert_eq!(options.label(0), Some("Hidden"));
    assert!(!options.is_empty());
}

#[test]
fn the_open_pane_takes_the_lists_place_and_the_query_loses_its_caret() {
    let mut palette = scoped();
    palette.set_status(Some("2 places searched".into()));
    palette.open_options();
    let buf = drawn(&mut palette, 80, 20);
    let all = text(&buf);
    assert!(all.contains("► [ ] Hidden"), "{all}");
    assert!(all.contains("  [x] Archived"), "{all}");
    assert!(all.contains("2 places searched"), "the status stays: {all}");
    assert!(!all.contains("alpha"), "{all}");
    assert!(all.contains("↑/↓ move · Space toggle · Esc back"), "{all}");
    let caret = Theme::default().field_cursor.into_style();
    let at = find(&buf, "│/").expect("the query line");
    let after = &buf[(at.x + 2, at.y)];
    assert_ne!(
        Some(after.bg),
        caret.bg,
        "no caret while the pane has the keys"
    );
    palette.handle(press(KeyCode::Esc), &());
    let buf = drawn(&mut palette, 80, 20);
    let after = &buf[(at.x + 2, at.y)];
    assert_eq!(
        Some(after.bg),
        caret.bg,
        "the caret is back with the results"
    );
}

// ---- the debounce ---------------------------------------------------------------

#[test]
fn typing_waits_for_the_debounce_then_owes_one_search() {
    let mut palette = three().debounced(Duration::from_millis(200));
    let before = Instant::now();
    assert!(!palette.take_due(before, true), "nothing owed yet");
    type_in(&mut palette, "al");
    assert!(!palette.take_due(before, false), "still resting");
    type_in(&mut palette, "p");
    let later = Instant::now() + Duration::from_millis(205);
    assert!(palette.take_due(later, false));
    assert!(!palette.take_due(later, false), "asked once");
    palette.mark_edited(later);
    assert!(palette.take_due(later, true), "forcing skips the wait");
}

#[test]
fn moving_owes_no_search() {
    let mut palette = three().debounced(Duration::from_millis(10));
    key_in(&mut palette, press(KeyCode::Down));
    key_in(&mut palette, press(KeyCode::Right));
    assert!(!palette.take_due(Instant::now() + Duration::from_secs(1), true));
}

#[test]
fn a_palette_without_a_debounce_is_never_due() {
    let mut palette = three();
    type_in(&mut palette, "al");
    palette.mark_edited(Instant::now());
    assert!(!palette.take_due(Instant::now(), true));
    assert!(palette.debounce().is_none());
}

#[test]
fn the_debounce_timer_on_its_own() {
    let mut debounce = Debounce::new(Duration::from_millis(100));
    assert_eq!(debounce.delay(), Duration::from_millis(100));
    let at = Instant::now();
    assert!(!debounce.is_pending() && !debounce.is_due(at));
    debounce.mark(at);
    assert!(debounce.is_pending());
    assert!(!debounce.is_due(at));
    assert!(debounce.is_due(at + Duration::from_millis(100)));
    debounce.cancel();
    assert!(!debounce.take_due(at + Duration::from_secs(1), true));
}

// ---- drawing --------------------------------------------------------------------

#[test]
fn the_cursor_row_shows_the_marker_and_the_highlight() {
    let theme = Theme::default();
    let mut palette = three();
    key_in(&mut palette, press(KeyCode::Down));
    let buf = drawn(&mut palette, 80, 20);
    assert_eq!(marked(&buf).len(), 1, "one marked row");
    let at = find(&buf, marks::POINTER).expect("the marker");
    let label: String = (at.x + 2..at.x + 6)
        .map(|x| buf[(x, at.y)].symbol())
        .collect();
    assert_eq!(label, "beta");
    for x in at.x..at.x + 12 {
        assert!(
            buf[(x, at.y)].modifier.contains(theme.selection_active),
            "cell {x}"
        );
    }
    let above: String = (at.x..at.x + 7)
        .map(|x| buf[(x, at.y - 1)].symbol())
        .collect();
    assert_eq!(above, "  alpha", "other rows keep the label a gutter in");
}

#[test]
fn the_detail_recedes_after_the_separator_mark() {
    let theme = Theme::default();
    let mut palette = three();
    let buf = drawn(&mut palette, 80, 20);
    let all = text(&buf);
    assert!(all.contains("  alphabet  · running"), "{all}");
    let detail = style_at(&buf, "· gone").expect("a detail");
    assert!(detail.add_modifier.contains(theme.muted));
}

#[test]
fn the_query_line_draws_its_prompt_and_caret() {
    let theme = Theme::default();
    let mut palette = three();
    type_in(&mut palette, "東京 al");
    key_in(&mut palette, press(KeyCode::Home));
    key_in(&mut palette, press(KeyCode::Right));
    let buf = drawn(&mut palette, 60, 12);
    let at = find(&buf, "/東").expect("the query line");
    let caret = &buf[(at.x + 3, at.y)];
    assert_eq!(caret.symbol(), "京", "the caret is on the second character");
    let cue = theme.field_cursor.into_style();
    assert_eq!((Some(caret.fg), Some(caret.bg)), (cue.fg, cue.bg));
    let prompt = &buf[(at.x, at.y)];
    assert_eq!(prompt.fg, theme.accent);
    let mut other = three().prompt(">");
    assert!(find(&drawn(&mut other, 60, 12), "│>").is_some());
}

#[test]
fn an_empty_list_says_why() {
    let mut nothing: Rows = Palette::new("Nothing", Vec::new(), |row: &Item<&'static str>| {
        Outcome::Commit(Msg::Pick(row.value))
    })
    .ranked_by(every_word);
    assert!(text(&drawn(&mut nothing, 80, 16)).contains("(nothing to pick)"));
    type_in(&mut nothing, "x");
    assert!(text(&drawn(&mut nothing, 80, 16)).contains("(no matches)"));
    let mut told = three()
        .empty_note("(the shelf is bare)")
        .no_match_note("(no such book)");
    type_in(&mut told, "zzz");
    assert!(text(&drawn(&mut told, 80, 16)).contains("(no such book)"));
    told.set_no_match_note("(still looking…)");
    assert!(text(&drawn(&mut told, 80, 16)).contains("(still looking…)"));
    let mut bare: Rows = Palette::new("Bare", Vec::new(), |row: &Item<&'static str>| {
        Outcome::Commit(Msg::Pick(row.value))
    });
    bare.set_empty_note("(the shelf is bare)");
    let buf = drawn(&mut bare, 80, 16);
    assert!(text(&buf).contains("(the shelf is bare)"));
    let note = style_at(&buf, "(the shelf").expect("the note");
    assert!(note.add_modifier.contains(Theme::default().muted));
}

#[test]
fn the_status_row_sits_under_the_query_and_recedes() {
    let mut palette = three();
    palette.set_status(Some("3 rooms · 2 floors".into()));
    assert_eq!(palette.status(), Some("3 rooms · 2 floors"));
    let buf = drawn(&mut palette, 80, 20);
    let query = find(&buf, "│/").expect("the query");
    let status = find(&buf, "3 rooms").expect("the status");
    assert_eq!(status.y, query.y + 1);
    assert_eq!(find(&buf, "alpha").map(|at| at.y), Some(query.y + 2));
    let style = style_at(&buf, "3 rooms").expect("the status");
    assert!(style.add_modifier.contains(Theme::default().muted));
    palette.set_status(None);
    let buf = drawn(&mut palette, 80, 20);
    assert_eq!(find(&buf, "alpha").map(|at| at.y), Some(query.y + 1));
}

#[test]
fn a_long_row_ends_with_the_ellipsis_mark() {
    let mut palette: Rows = Palette::new(
        "Long",
        vec![item(
            "a rather long label that runs on",
            "and a detail longer still than the row",
            "x",
        )],
        |row: &Item<&'static str>| Outcome::Commit(Msg::Pick(row.value)),
    );
    let buf = drawn(&mut palette, 40, 12);
    let row = rows(&buf)
        .into_iter()
        .find(|row| row.contains("a rather"))
        .expect("the row");
    assert!(row.contains(&format!("{}│", marks::ELLIPSIS)), "{row}");
}

#[test]
fn the_footer_is_generated_and_recedes() {
    let theme = Theme::default();
    let mut palette = three();
    let buf = drawn(&mut palette, 80, 20);
    assert!(text(&buf).contains(&footer(PALETTE_KEYS)), "{}", text(&buf));
    let style = style_at(&buf, "Enter pick").expect("the footer");
    assert!(style.add_modifier.contains(theme.muted));
}

#[test]
fn the_palette_takes_the_palette_size_and_keeps_it_as_the_list_narrows() {
    let mut palette = three();
    let wide = drawn(&mut palette, 100, 30);
    let frame = |buf: &Buffer| {
        let top = find(buf, "┌").expect("the frame");
        let width = (top.x..buf.area.width)
            .take_while(|&x| buf[(x, top.y)].symbol() != "┐")
            .count()
            + 1;
        let height = (top.y..buf.area.height)
            .take_while(|&y| buf[(top.x, y)].symbol() != "└")
            .count()
            + 1;
        (top.x, top.y, width, height)
    };
    let expected = PALETTE_SIZE.rect(Rect::new(0, 0, 100, 30), 0);
    assert_eq!(
        frame(&wide),
        (
            expected.x,
            expected.y,
            usize::from(expected.width),
            usize::from(expected.height)
        )
    );
    type_in(&mut palette, "beta");
    assert_eq!(frame(&drawn(&mut palette, 100, 30)), frame(&wide));
    let mut compact = three().size(ModalSize::COMPACT);
    let small = drawn(&mut compact, 100, 30);
    let (_, _, width, height) = frame(&small);
    assert_eq!(width, 60);
    // Query, three rows, and one footer row inside the border.
    assert_eq!(height, 7, "{}", text(&small));
}

#[test]
fn an_overflowing_list_scrolls_with_the_scrollbar_and_the_cursor_stays_in_view() {
    let mut palette = many(40);
    let buf = drawn(&mut palette, 60, 16);
    assert!(text(&buf).contains("Item 0"));
    assert!(!text(&buf).contains("Item 39"));
    key_in(&mut palette, press(KeyCode::Up));
    let buf = drawn(&mut palette, 60, 16);
    assert_eq!(marked(&buf).len(), 1);
    assert!(marked(&buf)[0].contains("Item 39"), "{}", text(&buf));
    // The bar's thumb sits at the bottom of its track.
    let window = palette.window();
    assert_eq!(window.offset(), window.last_offset());
    assert!(window.overflows());
    let fits = drawn(&mut three(), 60, 16);
    assert!(!text(&fits).contains('█'), "no bar when the rows fit");
}

#[test]
fn scrolling_moves_no_further_than_the_cursor_needs() {
    let mut palette = many(40);
    drawn(&mut palette, 60, 16);
    let page = palette.window().page();
    for _ in 0..page + 2 {
        key_in(&mut palette, press(KeyCode::Down));
        drawn(&mut palette, 60, 16);
    }
    let window = palette.window();
    assert_eq!(window.offset(), 3, "the cursor rides the last row");
    key_in(&mut palette, press(KeyCode::Up));
    drawn(&mut palette, 60, 16);
    assert_eq!(
        palette.window().offset(),
        3,
        "moving up stays in the window"
    );
}

#[test]
fn the_smallest_frames_keep_the_query_the_cursor_and_the_footer() {
    for (width, height) in [(40, 10), (30, 8), (20, 6)] {
        let mut palette = many(20);
        key_in(&mut palette, press(KeyCode::Up));
        let buf = drawn(&mut palette, width, height);
        let all = text(&buf);
        assert!(find(&buf, "│/").is_some(), "{width}x{height}: {all}");
        assert_eq!(marked(&buf).len(), 1, "{width}x{height}: {all}");
        assert!(all.contains("move"), "{width}x{height}: {all}");
    }
}

#[test]
fn a_one_row_frame_draws_only_the_query() {
    let mut palette = three().size(ModalSize {
        height: crate::frame::Extent::new(3, 3, 0),
        ..ModalSize::PALETTE
    });
    let buf = drawn(&mut palette, 80, 3);
    assert!(find(&buf, "│/").is_some());
    assert!(marked(&buf).is_empty(), "{}", text(&buf));
}

#[test]
fn a_row_is_told_its_width_cursor_and_query() {
    struct Probe;
    impl PaletteRow for Probe {
        fn cells(&self, row: &RowContext<'_>) -> Vec<ratatui::text::Span<'static>> {
            vec![ratatui::text::Span::raw(format!(
                "w{} {} q={}",
                row.width,
                if row.selected() { "on" } else { "off" },
                row.query
            ))]
        }
    }
    let mut palette = Palette::new("Probe", vec![Probe, Probe], |_: &Probe| {
        Outcome::Commit(Msg::Kept(String::new()))
    });
    type_in(&mut palette, "hi");
    let buf = drawn(&mut palette, 64, 16);
    let all = text(&buf);
    // The palette frame is 60 wide here: 58 inside, less the gutter.
    assert!(all.contains("► w56 on q=hi"), "{all}");
    assert!(all.contains("  w56 off q=hi"), "{all}");
    assert_eq!(Probe.haystack(), "", "a row need not say what to match");
}

// ---- hosts that steer the cursor and draw the rows --------------------------------

#[test]
fn select_puts_the_cursor_on_a_listed_row_and_ignores_the_rest() {
    let mut palette = three();
    type_in(&mut palette, "alph");
    assert_eq!(labels(&palette), ["alpha", "alphabet"]);
    palette.select(2);
    assert_eq!(
        current(&palette),
        Some("alphabet"),
        "row 2 is listed second"
    );
    palette.select(1);
    assert_eq!(current(&palette), Some("alphabet"), "beta is not listed");
    palette.select(0);
    assert_eq!(palette.cursor(), 0);
}

#[test]
fn a_host_can_keep_its_own_cursor_rule_across_an_edit() {
    let mut palette = many(10);
    for _ in 0..3 {
        key_in(&mut palette, press(KeyCode::Down));
    }
    let before = palette.cursor();
    type_in(&mut palette, "Item");
    assert_eq!(palette.cursor(), 0, "the palette's rule: the best result");
    palette.select(palette.ranked()[before]);
    assert_eq!(
        current(&palette),
        Some("Item 3"),
        "the host kept its position"
    );
}

#[test]
fn draw_with_lays_out_only_the_rows_on_screen_with_the_hosts_cells() {
    let mut palette = many(40);
    let laid_out = std::cell::Cell::new(0);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 16));
    palette.draw_with(buf.area, &mut buf, &Theme::default(), |row, context| {
        laid_out.set(laid_out.get() + 1);
        vec![ratatui::text::Span::raw(format!(
            "{} of the host's, {} wide",
            row.label, context.width
        ))]
    });
    let all = text(&buf);
    assert!(all.contains("► Item 0 of the host's, 55 wide"), "{all}");
    assert!(
        !all.contains("Item 0  · n"),
        "the row's own cells are not drawn"
    );
    assert_eq!(
        laid_out.get(),
        palette.window().viewport(),
        "only the rows shown"
    );
}

#[test]
fn rows_the_host_draws_need_not_lay_themselves_out() {
    // Plain numbers: the host routes the keys and draws every row.
    let mut palette: Palette<u32, u32> =
        Palette::new("Numbers", vec![10, 20, 30], |n: &u32| Outcome::Commit(*n));
    assert_eq!(palette.handle_key(press(KeyCode::Down)), Outcome::Consumed);
    assert_eq!(
        palette.handle_key(press(KeyCode::Enter)),
        Outcome::Commit(20)
    );
    assert_eq!(palette.handle_key(typed('x')), Outcome::Consumed);
    assert_eq!(palette.query(), "x");
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 16));
    palette.draw_with(buf.area, &mut buf, &Theme::default(), |n, _| {
        vec![ratatui::text::Span::raw(format!("number {n}"))]
    });
    assert!(text(&buf).contains("► number 10"), "{}", text(&buf));
}

/// A row that says how wide it was told it is and fills that width.
struct Told;

impl PaletteRow for Told {
    fn cells(&self, row: &RowContext<'_>) -> Vec<ratatui::text::Span<'static>> {
        let said = format!("w{}", row.width);
        vec![ratatui::text::Span::raw(format!(
            "{said:=<width$}",
            width = row.width
        ))]
    }
}

fn told(count: usize) -> Palette<Told, Msg> {
    let rows = (0..count).map(|_| Told).collect();
    Palette::new("Told", rows, |_: &Told| Outcome::Close)
}

/// The cell just inside the frame's right border on the cursor's row.
fn last_inside(buf: &Buffer) -> String {
    let at = find(buf, marks::POINTER).expect("the cursor row");
    let corner = find(buf, "┐").expect("the frame's corner");
    buf[(corner.x - 1, at.y)].symbol().to_string()
}

#[test]
fn a_reserved_scrollbar_keeps_one_row_width_whether_or_not_the_list_overflows() {
    // The palette frame is 60 wide at 64 columns: 58 inside, less the
    // gutter, less the scrollbar's column.
    let fits = drawn(&mut told(2).reserve_scrollbar(), 64, 16);
    let overflows = drawn(&mut told(40).reserve_scrollbar(), 64, 16);
    assert!(text(&fits).contains("► w55="), "{}", text(&fits));
    assert!(text(&overflows).contains("► w55="), "{}", text(&overflows));
    assert_eq!(
        last_inside(&fits),
        " ",
        "the column stays blank while they fit"
    );
    assert!(!text(&fits).contains('█'), "no bar while the rows fit");
    assert!(text(&overflows).contains('█'), "the bar once they overflow");
    // Without the reservation the rows take the column while they fit.
    let plain = drawn(&mut told(2), 64, 16);
    assert!(text(&plain).contains("► w56="), "{}", text(&plain));
    assert_eq!(last_inside(&plain), "=");
}

#[test]
fn progress_says_how_far_a_provider_has_got() {
    let progress = |scanned, total, skipped, done| Progress {
        scanned,
        total,
        skipped,
        done,
    };
    assert_eq!(
        progress(0, 5, 0, false).status("pages").as_deref(),
        Some("0/5 pages")
    );
    assert_eq!(
        progress(77, 77, 3, true).status("files").as_deref(),
        Some("77 files searched · 3 skipped")
    );
    assert_eq!(
        progress(0, 0, 2, true).status("files"),
        None,
        "nothing until the total is known"
    );
}

// ---- goldens --------------------------------------------------------------------

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/palette/goldens")
        .join(name)
}

#[test]
fn the_palette_looks_as_its_goldens_say() {
    let mut palette = three();
    type_in(&mut palette, "al");
    key_in(&mut palette, press(KeyCode::Down));
    let buf = drawn(&mut palette, 70, 14);
    assert_golden(&golden_path("palette.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
    let mut scoped = scoped();
    scoped.set_status(Some("3 found".into()));
    key_in(&mut scoped, press(KeyCode::Up));
    let narrow = drawn(&mut scoped, 40, 10);
    assert_golden(&golden_path("narrow.golden"), &narrow, |path, text| {
        std::fs::write(path, text)
    });
}

// ---- the conventions ------------------------------------------------------------

/// A host of one stack of palettes over a screen of text.
struct Host(Theme);

impl Rig for Host {
    type State = ModalStack<Rows>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        Paragraph::new("the screen under the palette").render(area, &mut buf);
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
fn a_palette_keeps_the_conventions_every_modal_keeps() {
    let open = |palette: fn() -> Rows| {
        move || {
            let mut stack = ModalStack::new();
            stack.push(palette());
            stack
        }
    };
    let convention = Convention::new("Palette", EnterMeans::Activates, SpaceMeans::Types, "");
    let keys = || [conformance::table("PALETTE_KEYS", PALETTE_KEYS)];
    let registry = Registry::new(Host(Theme::default()))
        .case(
            Case::new("Palette", open(three))
                .typed()
                .moves()
                .keys(convention, keys())
                .footer(PALETTE_KEYS),
        )
        .case(
            Case::new("Palette", open(|| many(30)))
                .variant("long")
                .typed()
                .moves()
                .keys(convention, keys())
                .footer(PALETTE_KEYS),
        );
    conformance::assert_conventions(&registry);
}

#[test]
fn the_tables_a_host_builds_on_the_palettes_pass_the_table_checks() {
    for problems in [mislabeled(SCOPED_KEYS), mislabeled(HOST_KEYS)] {
        assert!(problems.is_empty(), "{problems:?}");
    }
    assert!(unresolved(SCOPED_KEYS).is_empty());
    assert!(unresolved(HOST_KEYS).is_empty());
    assert!(loose_characters(HOST_KEYS).is_empty());
    assert!(ctrl_c_first(SCOPED_KEYS) && ctrl_c_first(HOST_KEYS));
    assert_eq!(
        footer(SCOPED_KEYS),
        "↑/↓ move · Enter pick · Esc close · Tab switch scope · Ctrl-O set options"
    );
}
