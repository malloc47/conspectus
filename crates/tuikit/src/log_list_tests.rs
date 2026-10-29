use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::{Paragraph, Widget};

use super::*;
use crate::feedback::Severity;
use crate::keys::{
    Convention, EnterMeans, SpaceMeans, alias, ch, ctrl_c_first, footer, include, loose_characters,
    mislabeled, shown, unresolved,
};
use crate::overlay::ModalStack;
use crate::testkit::buffer::{find, rows};
use crate::testkit::conformance::{self, Case, Registry, Rig};
use crate::testkit::keys::{ctrl, press, typed};

/// The host's state in these tests: a log whose entries carry what ran,
/// and the clock.
#[derive(Clone, Debug)]
struct Events {
    log: Log<String>,
    now: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Copy(String),
}

/// The standard table, `y` copying, and `!` closing as it opened.
const EVENT_KEYS: &[KeyRow<LogListKey>] = &[
    include(LOG_LIST_KEYS),
    shown(ch('y'), LogListKey::Copy, "y", "copy"),
    alias(ch('!'), LogListKey::Close),
];

impl Events {
    /// `count` entries, `entry 1` (the oldest) to `entry count`, logged a
    /// minute apart, read a minute after the last.
    fn of(count: usize) -> Self {
        let mut log = Log::default();
        for n in 1..=count {
            let at = i64::try_from(n).expect("a small count") * 60;
            let severity = Severity::ALL[n % Severity::ALL.len()];
            log.push(Entry::new(
                at,
                severity,
                format!("entry {n}"),
                format!("output of {n}"),
            ));
        }
        let now = i64::try_from(count + 1).expect("a small count") * 60;
        Self { log, now }
    }

    /// One entry whose detail is `lines` numbered lines.
    fn long(lines: usize) -> Self {
        let mut log = Log::default();
        let detail: Vec<String> = (1..=lines).map(|n| format!("line {n}")).collect();
        log.push(Entry::new(
            0,
            Severity::Error,
            "launch failed",
            detail.join("\n"),
        ));
        Self { log, now: 0 }
    }
}

/// The list the tests open: its detail is the entry's output.
fn list() -> LogList<String, Msg, Events> {
    LogList::new(
        "Events",
        |events: &Events| &events.log,
        |events: &Events| events.now,
    )
    .keys(EVENT_KEYS)
    .detail(|entry: &Entry<String>| entry.item.clone())
    .copy(|entry: &Entry<String>| Some(Msg::Copy(entry.item.clone())))
    .empty("Nothing has happened yet.\nWhat happens is listed here.")
}

fn draw(list: &mut LogList<String, Msg, Events>, events: &Events, w: u16, h: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    list.draw(buf.area, &mut buf, events, &Theme::default());
    buf
}

fn screen(buf: &Buffer) -> String {
    rows(buf).join("\n")
}

/// The numbers of the `line N` rows on screen, in order.
fn shown_lines(buf: &Buffer) -> Vec<usize> {
    rows(buf)
        .iter()
        .filter_map(|row| {
            let at = row.find("line ")?;
            row[at + 5..]
                .split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse()
                .ok()
        })
        .collect()
}

fn key(list: &mut LogList<String, Msg, Events>, events: &Events, key: KeyEvent) -> Outcome<Msg> {
    list.handle(key, events)
}

// ---- selection ---------------------------------------------------------------

#[test]
fn the_selection_starts_at_the_newest_entry_and_stops_at_the_ends() {
    let events = Events::of(3);
    let mut list = list();
    let summary = |list: &LogList<String, Msg, Events>| {
        list.selected_entry(&events.log)
            .map(|entry| entry.summary.clone())
    };
    assert_eq!(summary(&list).as_deref(), Some("entry 3"), "newest first");
    key(&mut list, &events, press(KeyCode::Char('k')));
    assert_eq!(list.selected(), 0, "a log does not wrap at the top");
    for _ in 0..5 {
        key(&mut list, &events, press(KeyCode::Char('j')));
    }
    assert_eq!(list.selected(), 2, "nor at the bottom");
    assert_eq!(summary(&list).as_deref(), Some("entry 1"));
    key(&mut list, &events, press(KeyCode::Char('g')));
    assert_eq!(list.selected(), 0, "g selects the newest");
    key(&mut list, &events, typed('G'));
    assert_eq!(list.selected(), 2, "G selects the oldest");
    key(&mut list, &events, press(KeyCode::Home));
    assert_eq!(list.selected(), 0);
    key(&mut list, &events, press(KeyCode::End));
    assert_eq!(list.selected(), 2);
    key(&mut list, &events, press(KeyCode::Up));
    key(&mut list, &events, ctrl('p'));
    assert_eq!(list.selected(), 0, "the arrows and Ctrl-P move it too");
    key(&mut list, &events, ctrl('n'));
    assert_eq!(list.selected(), 1);
}

#[test]
fn moving_the_selection_shows_the_entry_s_detail_from_its_top() {
    let mut events = Events::long(40);
    events
        .log
        .push(Entry::new(1, Severity::Success, "launched", "ok".into()));
    let mut list = list();
    key(&mut list, &events, press(KeyCode::Char('j')));
    draw(&mut list, &events, 80, 24);
    for _ in 0..3 {
        key(&mut list, &events, typed('J'));
    }
    let scrolled = draw(&mut list, &events, 80, 24);
    assert_eq!(list.detail_window().offset(), 3);
    assert_eq!(
        shown_lines(&scrolled).first(),
        Some(&4),
        "{}",
        screen(&scrolled)
    );
    key(&mut list, &events, typed('K'));
    draw(&mut list, &events, 80, 24);
    assert_eq!(list.detail_window().offset(), 2, "K scrolls back a row");

    key(&mut list, &events, press(KeyCode::Char('k')));
    let other = draw(&mut list, &events, 80, 24);
    assert!(screen(&other).contains("ok"), "{}", screen(&other));
    key(&mut list, &events, press(KeyCode::Char('j')));
    let back = draw(&mut list, &events, 80, 24);
    assert_eq!(list.detail_window().offset(), 0, "back at its top");
    assert_eq!(shown_lines(&back).first(), Some(&1));
}

#[test]
fn an_entry_logged_while_the_list_is_open_shows_at_the_next_draw() {
    let mut events = Events::of(2);
    let mut list = list();
    draw(&mut list, &events, 80, 24);
    events.log.push(Entry::new(
        500,
        Severity::Error,
        "late failure",
        "why".into(),
    ));
    let buf = draw(&mut list, &events, 80, 24);
    let text = screen(&buf);
    assert!(text.contains("Events · 3"), "{text}");
    assert!(text.contains("late failure"), "{text}");
}

// ---- keys --------------------------------------------------------------------

#[test]
fn esc_closes_and_q_and_ctrl_c_dismiss_saying_nothing() {
    let events = Events::of(2);
    for (press_, outcome) in [
        (press(KeyCode::Esc), Outcome::Close),
        (press(KeyCode::Char('!')), Outcome::Close),
        (press(KeyCode::Char('q')), Outcome::Dismiss),
        (ctrl('c'), Outcome::Dismiss),
        (press(KeyCode::Enter), Outcome::Consumed),
        (press(KeyCode::Char('x')), Outcome::Consumed),
    ] {
        assert_eq!(key(&mut list(), &events, press_), outcome, "{press_:?}");
    }
}

#[test]
fn y_asks_the_host_to_copy_the_selected_entry() {
    let events = Events::of(2);
    let mut list = list();
    key(&mut list, &events, press(KeyCode::Char('j')));
    assert_eq!(
        key(&mut list, &events, press(KeyCode::Char('y'))),
        Outcome::Stay(Msg::Copy("output of 1".into()))
    );
    let empty = Events::of(0);
    assert_eq!(
        key(&mut self::list(), &empty, press(KeyCode::Char('y'))),
        Outcome::Consumed,
        "an empty log has nothing to copy"
    );
    let mut plain: LogList<String, Msg, Events> =
        LogList::new("Events", |e: &Events| &e.log, |e: &Events| e.now).keys(EVENT_KEYS);
    assert_eq!(
        key(&mut plain, &events, press(KeyCode::Char('y'))),
        Outcome::Consumed,
        "a list with no copy hook copies nothing"
    );
}

#[test]
fn opening_the_list_marks_the_log_seen() {
    let mut events = Events::of(4);
    assert_eq!(events.log.unseen(), 2, "the warning and the error");
    let list = list().mark_seen(&mut events.log);
    assert_eq!(events.log.unseen(), 0);
    assert_eq!(events.log.len(), 4, "seeing keeps the entries");
    assert_eq!(list.title(), "Events");
    assert_eq!(list.table(), EVENT_KEYS);
}

#[test]
fn the_log_list_table_passes_the_table_checks() {
    for table in [LOG_LIST_KEYS, EVENT_KEYS] {
        assert!(mislabeled(table).is_empty());
        assert!(unresolved(table).is_empty());
        assert!(loose_characters(table).is_empty());
        assert!(ctrl_c_first(table));
    }
    assert_eq!(
        footer(LOG_LIST_KEYS),
        "j/k select · J/K scroll detail · Esc close"
    );
    assert_eq!(
        list().footer().text(),
        "j/k select · J/K scroll detail · y copy · Esc close",
        "the keys that leave come last"
    );
    let ctrl_g = KeyEvent::new(
        KeyCode::Char('g'),
        ratatui::crossterm::event::KeyModifiers::CONTROL,
    );
    assert_eq!(resolve(LOG_LIST_KEYS, ctrl_g), None, "g binds as typed");
}

// ---- drawing -----------------------------------------------------------------

#[test]
fn a_row_is_the_mark_the_age_and_the_summary_under_the_title_and_count() {
    let events = Events::of(3);
    let mut list = list();
    let buf = draw(&mut list, &events, 80, 24);
    let text = screen(&buf);
    assert!(text.contains("Events · 3"), "{text}");
    // Entry 3's severity is the fourth of the cycle.
    let newest = Severity::Error.mark();
    assert!(
        text.contains(&format!("{} {newest}   1m ago  entry 3", marks::POINTER)),
        "{text}"
    );
    assert!(text.contains("   3m ago  entry 1"), "{text}");
    assert!(
        text.contains(&marks::RULE.repeat(10)),
        "a rule under the list"
    );
    assert!(text.contains("output of 3"), "the newest entry's detail");
    let footer_row = rows(&buf)
        .iter()
        .position(|row| row.contains("j/k select"))
        .expect("a footer");
    assert_eq!(footer_row, 21, "the footer keeps the frame's last row");
}

#[test]
fn each_severity_takes_its_own_theme_color() {
    let mut theme = Theme::default();
    theme.info = Color::Blue;
    theme.success = Color::Magenta;
    theme.warning = Color::Yellow;
    theme.error = Color::Red;
    let mut log = Log::default();
    for severity in Severity::ALL {
        log.push(Entry::new(
            0,
            severity,
            format!("{severity:?} entry"),
            String::new(),
        ));
    }
    let events = Events { log, now: 0 };
    let mut list = list();
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    list.draw(buf.area, &mut buf, &events, &theme);
    for severity in Severity::ALL {
        let at = find(&buf, &format!("{severity:?} entry")).expect("its row");
        let mark = (0..at.x)
            .find(|&x| buf[(x, at.y)].symbol() == severity.mark())
            .expect("its mark before its summary");
        assert_eq!(buf[(mark, at.y)].fg, severity.color(&theme), "{severity:?}");
    }
}

#[test]
fn the_selected_row_shows_the_marker_and_the_highlight() {
    let events = Events::of(3);
    let mut list = list();
    key(&mut list, &events, press(KeyCode::Char('j')));
    let buf = draw(&mut list, &events, 80, 24);
    let theme = Theme::default();
    let marked = crate::testkit::buffer::find_all(&buf, marks::POINTER);
    assert_eq!(marked.len(), 1, "one marked row");
    let (x, y) = (marked[0].x, marked[0].y);
    assert!(rows(&buf)[usize::from(y)].contains("entry 2"));
    assert!(buf[(x, y)].modifier.contains(theme.selection_active));
    assert_eq!(
        buf[(x, y - 1)].symbol(),
        " ",
        "other rows keep the gutter blank"
    );
    assert!(!buf[(x, y - 1)].modifier.contains(theme.selection_active));
}

#[test]
fn a_repeated_entry_shows_its_count_and_a_long_summary_ends_in_an_ellipsis() {
    let mut log = Log::default();
    for at in 0..3 {
        log.push(Entry::new(
            at,
            Severity::Error,
            "refresh failed",
            String::new(),
        ));
    }
    log.push(Entry::new(
        3,
        Severity::Info,
        "a summary far longer than the forty columns this frame can hold",
        String::new(),
    ));
    let events = Events { log, now: 3 };
    let mut list = list();
    let buf = draw(&mut list, &events, 40, 12);
    let text = screen(&buf);
    assert!(
        text.contains(&format!("refresh failed ({}3)", marks::TIMES)),
        "{text}"
    );
    let cut = rows(&buf)
        .into_iter()
        .find(|row| row.contains("a summary"))
        .expect("the long row");
    assert!(
        cut.trim_end()
            .trim_end_matches('│')
            .ends_with(marks::ELLIPSIS),
        "{cut}"
    );
}

#[test]
fn the_list_scrolls_no_further_than_the_selection_needs_with_a_scrollbar() {
    let events = Events::of(30);
    let mut list = list();
    draw(&mut list, &events, 80, 24);
    let rows_shown = list.window().viewport();
    assert_eq!(rows_shown, 8, "two fifths of the 20 rows inside the frame");
    for _ in 0..12 {
        key(&mut list, &events, press(KeyCode::Char('j')));
    }
    let down = draw(&mut list, &events, 80, 24);
    assert_eq!(
        list.window().offset(),
        12 + 1 - rows_shown,
        "it follows down"
    );
    assert!(
        screen(&down).contains("║") || screen(&down).contains("█"),
        "a scrollbar\n{}",
        screen(&down)
    );
    key(&mut list, &events, press(KeyCode::Char('k')));
    draw(&mut list, &events, 80, 24);
    assert_eq!(
        list.window().offset(),
        12 + 1 - rows_shown,
        "one row up stays inside the window: it does not move"
    );
    key(&mut list, &events, typed('G'));
    draw(&mut list, &events, 80, 24);
    assert_eq!(list.window().offset(), 30 - rows_shown);
}

// ---- the detail ----------------------------------------------------------------

#[test]
fn the_detail_pages_by_the_rows_it_shows_and_stops_at_its_end() {
    let events = Events::long(60);
    let mut list = list();
    let first = draw(&mut list, &events, 80, 24);
    let shown = shown_lines(&first);
    let page = list.detail_window().viewport();
    assert_eq!(shown.len(), page);
    key(&mut list, &events, press(KeyCode::PageDown));
    let paged = draw(&mut list, &events, 80, 24);
    assert_eq!(
        shown_lines(&paged).first(),
        Some(&(shown.last().expect("lines") + 1)),
        "the next page starts below the last line shown\n{}",
        screen(&paged)
    );
    key(&mut list, &events, press(KeyCode::Char(' ')));
    key(&mut list, &events, press(KeyCode::PageUp));
    let back = draw(&mut list, &events, 80, 24);
    assert_eq!(shown_lines(&back).first(), shown_lines(&paged).first());
    for _ in 0..20 {
        key(&mut list, &events, press(KeyCode::PageDown));
    }
    let end = draw(&mut list, &events, 80, 24);
    assert_eq!(
        shown_lines(&end).last(),
        Some(&60),
        "the end, not a blank page"
    );
    key(&mut list, &events, typed('K'));
    let up = draw(&mut list, &events, 80, 24);
    assert_eq!(
        shown_lines(&up).last(),
        Some(&59),
        "K after the end moves at once"
    );
    assert!(
        screen(&up).contains("█"),
        "the detail shows the shared scrollbar\n{}",
        screen(&up)
    );
}

#[test]
fn a_short_detail_stays_put_and_draws_no_scrollbar() {
    let events = Events::long(2);
    let mut list = list();
    key(&mut list, &events, press(KeyCode::PageDown));
    let buf = draw(&mut list, &events, 80, 24);
    assert_eq!(shown_lines(&buf), [1, 2]);
    assert!(!screen(&buf).contains("█"), "{}", screen(&buf));
}

// ---- the empty list and the footer ----------------------------------------------

#[test]
fn an_empty_log_says_so_over_the_footer() {
    let events = Events::of(0);
    let mut list = list();
    let buf = draw(&mut list, &events, 40, 10);
    let all = rows(&buf);
    assert!(all[0].contains("Events · 0"), "{all:#?}");
    assert!(all[1].contains("Nothing has happened yet."), "{all:#?}");
    assert!(all[2].contains("What happens is listed here."), "{all:#?}");
    assert!(
        all[8].contains("Esc close"),
        "the footer keeps the last row\n{all:#?}"
    );
    let default: LogList<String, Msg, Events> =
        LogList::new("Events", |e: &Events| &e.log, |e: &Events| e.now);
    let mut default = default;
    let buf = draw(&mut default, &events, 40, 10);
    assert!(screen(&buf).contains(EMPTY_NOTE), "{}", screen(&buf));
}

#[test]
fn a_narrow_footer_wraps_between_its_segments() {
    let events = Events::of(3);
    let mut list = list();
    let buf = draw(&mut list, &events, 40, 10);
    let all = rows(&buf);
    assert!(
        all[7].contains("j/k select · J/K scroll detail"),
        "{all:#?}"
    );
    assert!(all[8].contains("y copy · Esc close"), "{all:#?}");
    assert!(
        shown_rows_between(&all, 5, 7) >= 1,
        "the detail keeps a row\n{all:#?}"
    );
}

/// How many of rows `from..to` draw something inside the frame.
fn shown_rows_between(all: &[String], from: usize, to: usize) -> usize {
    all[from..to]
        .iter()
        .filter(|row| !row.trim_matches(|c| c == '│' || c == ' ').is_empty())
        .count()
}

#[test]
fn a_footer_too_tall_for_the_frame_keeps_its_last_rows_and_scrolls_the_rest() {
    let events = Events::long(1);
    let mut list = list();
    // Two rows inside the frame for the detail and the footer: the footer's
    // last row (the keys that leave) stays, its first scrolls with the
    // detail.
    let buf = draw(&mut list, &events, 40, 6);
    let all = rows(&buf);
    assert!(all[4].contains("y copy · Esc close"), "{all:#?}");
    assert!(!screen(&buf).contains("j/k select"), "{all:#?}");
    for _ in 0..3 {
        key(&mut list, &events, typed('J'));
    }
    let scrolled = draw(&mut list, &events, 40, 6);
    assert!(
        screen(&scrolled).contains("j/k select"),
        "scrolled into view after the detail\n{}",
        screen(&scrolled)
    );
}

#[test]
fn a_list_clones_and_debugs_without_its_closures() {
    let events = Events::of(2);
    let mut list = list();
    key(&mut list, &events, press(KeyCode::Char('j')));
    let copy = list.clone();
    assert_eq!(copy.selected(), 1);
    let debug = format!("{copy:?}");
    assert!(debug.starts_with("LogList"), "{debug}");
    assert!(debug.contains("Events"), "{debug}");
}

// ---- the conventions -----------------------------------------------------------

/// A host of one stack of log lists over a screen of text.
struct Host(Theme, Events);

impl Rig for Host {
    type State = ModalStack<LogList<String, Msg, Events>>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        Paragraph::new("the screen under the list").render(area, &mut buf);
        stack.draw::<Msg, Events>(area, &mut buf, &self.1, &self.0);
        buf
    }

    fn close_top(&self, stack: &mut Self::State) {
        stack.pop();
    }

    fn press(&self, stack: &mut Self::State, key: KeyEvent) {
        if let Some(outcome) = stack.route::<Msg, Events>(key, &self.1) {
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
fn a_log_list_keeps_the_conventions_every_modal_keeps() {
    let open = || {
        let mut stack = ModalStack::new();
        stack.push(list());
        stack
    };
    let convention = Convention::new(
        "LogList",
        EnterMeans::Nothing,
        SpaceMeans::Pages,
        "a log: Space pages the detail",
    );
    let keys = || [conformance::table("EVENT_KEYS", EVENT_KEYS)];
    for events in [
        Events::of(3),
        Events::of(30),
        Events::long(80),
        Events::of(0),
    ] {
        let registry = Registry::new(Host(Theme::default(), events)).case(
            Case::new("LogList", open)
                .unchecked_selection("the detail under the list shows the selected entry")
                .keys(convention, keys())
                .footer(list().footer()),
        );
        conformance::assert_conventions(&registry);
    }
}
