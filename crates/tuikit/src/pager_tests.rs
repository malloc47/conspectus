use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use super::*;
use crate::glyphs::marks;
use crate::keys::{
    Convention, EnterMeans, PAGER_KEYS, SpaceMeans, alias, ch, include, key_label, shown,
};
use crate::overlay::ModalStack;
use crate::testkit::buffer::{find as find_in_buffer, rows as buffer_rows, style_at};
use crate::testkit::conformance::{self, Case, Registry, Rig};
use crate::testkit::keys::{chord, ctrl, press, typed};
use crate::text_edit::LineEdit;

/// A document of lines, each a unit; laid out one row per line, or
/// wrapped at the width.
#[derive(Clone, Debug)]
struct Doc {
    lines: Vec<String>,
    wrap: bool,
    copyable: bool,
    chips: Vec<&'static str>,
    hidden: usize,
    rows: Vec<Line<'static>>,
    starts: Vec<usize>,
    built: Option<(u16, usize, bool)>,
}

impl Doc {
    fn new(lines: Vec<String>) -> Self {
        Self {
            lines,
            wrap: false,
            copyable: false,
            chips: Vec::new(),
            hidden: 0,
            rows: Vec::new(),
            starts: Vec::new(),
            built: None,
        }
    }

    /// `count` lines, `row 1` to `row count`, with `needle` on the lines
    /// (from 1) in `needles`.
    fn rows_of(count: usize, needles: &[usize]) -> Self {
        Self::new(
            (1..=count)
                .map(|n| {
                    if needles.contains(&n) {
                        format!("row {n} holds a needle")
                    } else {
                        format!("row {n}")
                    }
                })
                .collect(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocKey {
    Ask,
    Grow,
    Wrap,
    Leave,
}

impl KeyAction for DocKey {
    fn role(self) -> KeyRole {
        KeyRole::Other
    }
}

impl HelpAction for DocKey {
    const LISTED: &'static [Self] = &[DocKey::Ask, DocKey::Grow, DocKey::Wrap, DocKey::Leave];

    fn help(self) -> HelpEntry {
        match self {
            DocKey::Ask => HelpEntry::new("Document", "Ask the host"),
            DocKey::Grow => HelpEntry::new("Document", "Add a line"),
            DocKey::Wrap => HelpEntry::new("Document", "Wrap long lines"),
            DocKey::Leave => HelpEntry::new("Document", "Leave from the document"),
        }
    }
}

const DOC_KEYS: &[KeyRow<DocKey>] = &[
    shown(ch('a'), DocKey::Ask, "a", "ask"),
    alias(ch('+'), DocKey::Grow),
    alias(ch('w'), DocKey::Wrap),
    alias(ch('x'), DocKey::Leave),
];

impl PagerContent for Doc {
    type Message = String;
    type Action = DocKey;
    const KEYS: &'static [KeyRow<DocKey>] = DOC_KEYS;

    fn layout(&mut self, width: u16, _theme: &Theme) -> usize {
        let key = (width, self.lines.len(), self.wrap);
        if self.built != Some(key) {
            self.rows.clear();
            self.starts.clear();
            for line in &self.lines {
                self.starts.push(self.rows.len());
                let line = Line::from(line.clone());
                if self.wrap {
                    self.rows
                        .extend(crate::text::wrap_line(&line, usize::from(width), 0));
                } else {
                    self.rows.push(line);
                }
            }
            self.starts.push(self.rows.len());
            self.built = Some(key);
        }
        self.rows.len()
    }

    fn rows(&self, range: Range<usize>, _theme: &Theme) -> Vec<Line<'static>> {
        self.rows[range].to_vec()
    }

    fn header(&self, _width: usize, _theme: &Theme) -> Line<'static> {
        Line::from(format!(" document · {} lines", self.lines.len()))
    }

    fn chips(&self, _theme: &Theme) -> Vec<Span<'static>> {
        self.chips.iter().map(|chip| Span::raw(*chip)).collect()
    }

    fn find(&self, query: &str) -> Vec<usize> {
        let lines = self.lines.iter().enumerate();
        lines
            .filter(|(_, line)| crate::text::find_ci(line, query).is_some())
            .map(|(at, _)| at)
            .collect()
    }

    fn match_rows(&self, id: usize) -> Range<usize> {
        self.starts[id]..self.starts[id + 1]
    }

    fn paint(&self, rows: &mut [Line<'static>], first: usize, find: &Find, theme: &Theme) {
        for (at, row) in rows.iter_mut().enumerate() {
            let unit = self.starts.partition_point(|&start| start <= first + at) - 1;
            *row = find.paint(row, 0, find.current_unit() == Some(unit), theme);
        }
    }

    fn find_detail(&self, _query: &str) -> Option<String> {
        (self.hidden > 0).then(|| format!("{} hidden", self.hidden))
    }

    fn empty(&self) -> std::borrow::Cow<'_, str> {
        "nothing here".into()
    }

    fn apply(&mut self, action: DocKey) -> PagerEffect<String> {
        match action {
            DocKey::Ask => PagerEffect::Request("asked".into()),
            DocKey::Grow => {
                self.lines.push(format!("row {}", self.lines.len() + 1));
                PagerEffect::Relayout
            }
            DocKey::Wrap => {
                self.wrap = !self.wrap;
                PagerEffect::Relayout
            }
            DocKey::Leave => PagerEffect::Close,
        }
    }

    fn copy(&self) -> Option<String> {
        self.copyable.then(|| "copied".to_string())
    }
}

fn theme() -> Theme {
    Theme::default()
}

/// Draws `pager` at `width` by `height` and returns the screen's rows.
fn draw(pager: &mut Pager<Doc>, width: u16, height: u16) -> Vec<String> {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    pager.draw(area, &mut buf, &theme());
    buffer_rows(&buf)
}

fn screen(pager: &mut Pager<Doc>, width: u16, height: u16) -> String {
    draw(pager, width, height).join("\n")
}

fn keys(pager: &mut Pager<Doc>, script: &str) {
    for key in crate::testkit::keys::script(script) {
        pager.handle_key(key);
    }
}

fn shift(c: char) -> KeyEvent {
    chord(KeyModifiers::SHIFT, KeyCode::Char(c))
}

// ---- scrolling ------------------------------------------------------------

#[test]
fn it_scrolls_by_rows_pages_and_half_pages_and_goes_to_the_ends() {
    let mut pager = Pager::new(Doc::rows_of(100, &[]));
    // 14 rows less the header, two rules, and the footer: a body of 10.
    draw(&mut pager, 40, 14);
    assert_eq!(pager.window().viewport(), 10);
    let steps: [(KeyEvent, usize); 9] = [
        (typed('j'), 1),
        (press(KeyCode::PageDown), 11),
        (typed(' '), 21),
        (press(KeyCode::PageUp), 11),
        (ctrl('d'), 16),
        (ctrl('u'), 11),
        (press(KeyCode::Down), 12),
        (typed('g'), 0),
        (shift('G'), 90),
    ];
    for (key, offset) in steps {
        pager.handle_key(key);
        draw(&mut pager, 40, 14);
        assert_eq!(pager.window().offset(), offset, "{key:?}");
    }
    assert!(pager.follows_end());
    pager.handle_key(typed('k'));
    draw(&mut pager, 40, 14);
    assert_eq!(pager.window().offset(), 89, "one key up from the end moves");
    assert!(!pager.follows_end());
}

#[test]
fn the_offset_is_clamped_on_every_draw_and_kept() {
    let mut pager = Pager::new(Doc::rows_of(30, &[]));
    draw(&mut pager, 40, 14);
    for _ in 0..50 {
        pager.handle_key(typed('j'));
    }
    draw(&mut pager, 40, 14);
    assert_eq!(pager.window().offset(), 20, "never past the last page");
    pager.handle_key(typed('k'));
    draw(&mut pager, 40, 14);
    assert_eq!(pager.window().offset(), 19, "the next key up moves at once");
    // A taller screen shows everything from the top.
    draw(&mut pager, 40, 60);
    assert_eq!(pager.window().offset(), 0);
}

#[test]
fn keys_before_the_first_draw_count() {
    let mut pager = Pager::new(Doc::rows_of(100, &[])).at_row(40);
    draw(&mut pager, 40, 14);
    assert_eq!(pager.window().offset(), 40);
    let mut pager = Pager::new(Doc::rows_of(100, &[]));
    pager.handle_key(shift('G'));
    draw(&mut pager, 40, 14);
    assert_eq!(
        pager.window().offset(),
        90,
        "`G` before a draw lands at the end"
    );
}

#[test]
fn the_sticky_end_follows_the_content_as_it_grows() {
    let mut pager = Pager::new(Doc::rows_of(20, &[])).at_end();
    draw(&mut pager, 40, 14);
    assert_eq!(pager.window().offset(), 10);
    for _ in 0..5 {
        pager.handle_key(typed('+'));
    }
    let shown = draw(&mut pager, 40, 14);
    assert_eq!(pager.window().offset(), 15, "the newest rows stay in view");
    assert!(
        shown.iter().any(|row| row.starts_with("row 25")),
        "{shown:?}"
    );
    // A key that moves the window lets go of the end.
    pager.handle_key(typed('k'));
    pager.handle_key(typed('+'));
    draw(&mut pager, 40, 14);
    assert_eq!(pager.window().offset(), 14);
    assert!(!pager.follows_end());
    // A relayout that changes every row keeps a stuck window at the end.
    let mut pager = Pager::new(Doc::new(vec!["word ".repeat(30); 10])).at_end();
    draw(&mut pager, 40, 14);
    assert_eq!(pager.window().offset(), 0, "ten rows fit");
    pager.handle_key(typed('w'));
    draw(&mut pager, 40, 14);
    assert_eq!(pager.window().offset(), pager.window().last_offset());
    assert!(pager.window().last_offset() > 0, "wrapped rows overflow");
}

/// A fixed-seed xorshift, so a failure names its seed and reproduces.
struct Gen(u64);

impl Gen {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % bound as u64).unwrap_or(0)
    }
}

/// Over generated keys, sizes, and growth: after every draw the window is
/// inside the content, a stuck window is at the end, the position chip
/// names the top row, and a page key moves by the rows shown unless an
/// end stops it.
#[test]
fn the_window_stays_in_the_content_and_the_end_sticks_over_generated_keys() {
    let keys = [
        typed('j'),
        typed('k'),
        press(KeyCode::PageDown),
        press(KeyCode::PageUp),
        ctrl('d'),
        ctrl('u'),
        typed('g'),
        shift('G'),
        typed('+'),
        typed('w'),
    ];
    for seed in 1..=40_u64 {
        let mut generator = Gen(0x9e37_79b9_7f4a_7c15 ^ seed);
        let count = generator.below(80);
        let lines = (0..count)
            .map(|n| format!("row {n} {}", "word ".repeat(generator.below(20))))
            .collect();
        let mut pager = Pager::new(Doc::new(lines));
        if generator.below(2) == 0 {
            pager = pager.at_end();
        }
        let mut size = (
            20 + generator.below(80) as u16,
            5 + generator.below(30) as u16,
        );
        draw(&mut pager, size.0, size.1);
        for step in 0..60 {
            if generator.below(10) == 0 {
                size = (
                    20 + generator.below(80) as u16,
                    5 + generator.below(30) as u16,
                );
            }
            let key = keys[generator.below(keys.len())];
            let before = pager.window();
            pager.handle_key(key);
            let shown = draw(&mut pager, size.0, size.1);
            let window = pager.window();
            let label = format!("seed {seed} step {step} {key:?} at {size:?}");
            assert!(
                window.offset() <= window.last_offset(),
                "{label}: {window:?}"
            );
            if pager.follows_end() {
                assert_eq!(window.offset(), window.last_offset(), "{label}");
            }
            let chip = crate::chip::PositionChip::new(window.offset(), window.content()).text();
            assert!(
                shown.last().is_some_and(|row| row.starts_with(&chip)),
                "{label}: {chip} in {shown:?}"
            );
            let same =
                before.viewport() == window.viewport() && before.content() == window.content();
            if same && key == press(KeyCode::PageDown) {
                let expected = (before.offset() + before.page()).min(window.last_offset());
                assert_eq!(window.offset(), expected, "{label}");
            }
            if same && key == ctrl('u') {
                let half = (before.page() / 2).max(1);
                assert_eq!(
                    window.offset(),
                    before.offset().saturating_sub(half),
                    "{label}"
                );
            }
        }
    }
}

// ---- levels and leaving ---------------------------------------------------

#[test]
fn esc_steps_back_from_the_help_then_the_find_then_leaves() {
    let mut pager = Pager::new(Doc::rows_of(30, &[4]));
    draw(&mut pager, 60, 20);
    pager.start_find("needle");
    pager.handle_key(typed('?'));
    assert!(pager.help_is_open());
    assert_eq!(pager.handle_key(press(KeyCode::Esc)), Outcome::Consumed);
    assert!(!pager.help_is_open());
    assert!(pager.find().is_active());
    // The prompt is a level too: Esc there closes only the prompt.
    pager.handle_key(typed('/'));
    assert_eq!(pager.handle_key(press(KeyCode::Esc)), Outcome::Consumed);
    assert!(pager.find().prompt().is_none());
    assert!(
        pager.find().is_active(),
        "cancelling the prompt keeps the find"
    );
    assert_eq!(pager.handle_key(press(KeyCode::Esc)), Outcome::Consumed);
    assert!(!pager.find().is_active());
    assert_eq!(pager.handle_key(press(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn q_and_ctrl_c_leave_from_every_level_and_q_types_in_the_prompt() {
    for leave in [typed('q'), ctrl('c')] {
        let mut pager = Pager::new(Doc::rows_of(10, &[2]));
        pager.start_find("needle");
        assert_eq!(
            pager.handle_key(leave),
            Outcome::Dismiss,
            "with a find active"
        );
        pager.handle_key(typed('?'));
        assert_eq!(pager.handle_key(leave), Outcome::Dismiss, "from the help");
    }
    let mut pager = Pager::new(Doc::rows_of(10, &[]));
    pager.handle_key(typed('/'));
    assert!(pager.takes_text());
    assert_eq!(pager.handle_key(typed('q')), Outcome::Consumed);
    assert_eq!(pager.find().prompt().map(LineEdit::value), Some("q"));
    assert_eq!(pager.handle_key(ctrl('c')), Outcome::Dismiss);
}

#[test]
fn the_content_keys_come_after_the_pagers_and_say_what_they_ask() {
    let mut pager = Pager::new(Doc::rows_of(10, &[]));
    assert_eq!(
        pager.handle_key(typed('a')),
        Outcome::Stay("asked".to_string())
    );
    assert_eq!(pager.handle_key(typed('x')), Outcome::Close);
    assert_eq!(
        pager.handle_key(typed('z')),
        Outcome::Consumed,
        "unbound keys are swallowed"
    );
    assert_eq!(pager.handle_key(press(KeyCode::Enter)), Outcome::Consumed);
    // While the help is open the content's keys wait.
    pager.handle_key(typed('?'));
    assert_eq!(pager.handle_key(typed('a')), Outcome::Consumed);
    assert!(pager.help_is_open());
}

#[test]
fn the_copy_hook_answers_a_table_that_binds_copy() {
    const COPY_KEYS: &[KeyRow<PagerKey>] = &[
        include(PAGER_KEYS),
        shown(ch('y'), PagerKey::Copy, "y", "copy"),
    ];
    let mut doc = Doc::rows_of(5, &[]);
    doc.copyable = true;
    let mut pager = Pager::new(doc).keys(COPY_KEYS);
    assert_eq!(
        pager.handle_key(typed('y')),
        Outcome::Stay("copied".to_string())
    );
    let mut pager = Pager::new(Doc::rows_of(5, &[])).keys(COPY_KEYS);
    assert_eq!(
        pager.handle_key(typed('y')),
        Outcome::Consumed,
        "nothing to copy"
    );
    // That table has no find and no help: `/` and `?` do nothing.
    assert_eq!(pager.handle_key(typed('/')), Outcome::Consumed);
    assert!(pager.find().prompt().is_none());
    pager.handle_key(typed('?'));
    assert!(!pager.help_is_open());
}

#[test]
fn a_note_shows_until_the_next_key() {
    let mut pager = Pager::new(Doc::rows_of(5, &[]));
    pager.set_note(Feedback::warning("refused"));
    assert!(screen(&mut pager, 60, 10).contains("refused"));
    pager.handle_key(typed('j'));
    assert!(pager.note().is_none());
    assert!(!screen(&mut pager, 60, 10).contains("refused"));
}

// ---- find -----------------------------------------------------------------

#[test]
fn confirming_a_find_scrolls_to_the_match_even_when_it_shows() {
    let mut pager = Pager::new(Doc::rows_of(60, &[6, 30, 45]));
    draw(&mut pager, 60, 20);
    keys(&mut pager, "/needle<Enter>");
    draw(&mut pager, 60, 20);
    assert_eq!(pager.find().current_unit(), Some(5));
    assert_eq!(pager.window().offset(), 4, "the match a row below the top");
    // From further down, the first match at or below the top.
    keys(&mut pager, "<Esc>gjjjjjjjjjj/needle<Enter>");
    draw(&mut pager, 60, 20);
    assert_eq!(pager.find().current_unit(), Some(29));
    assert_eq!(pager.window().offset(), 28);
}

#[test]
fn n_and_shift_n_step_through_the_matches_and_wrap_with_a_note() {
    let mut pager = Pager::new(Doc::rows_of(60, &[6, 30, 45]));
    draw(&mut pager, 60, 20);
    keys(&mut pager, "/needle<Enter>");
    let screen_of = |pager: &mut Pager<Doc>| screen(pager, 80, 20);
    pager.handle_key(typed('n'));
    pager.handle_key(typed('n'));
    assert_eq!(pager.find().current_unit(), Some(44));
    assert!(screen_of(&mut pager).contains("find \"needle\" 3/3"));
    assert_eq!(
        pager.window().offset(),
        43,
        "a row of context above the match"
    );
    pager.handle_key(typed('n'));
    assert_eq!(pager.find().current_unit(), Some(5));
    let shown = screen_of(&mut pager);
    assert!(shown.contains("wrapped to the first match"), "{shown}");
    pager.handle_key(shift('N'));
    assert_eq!(pager.find().current_unit(), Some(44));
    assert!(screen_of(&mut pager).contains("wrapped to the last match"));
}

#[test]
fn stepping_with_no_find_says_how_to_start_one() {
    let mut pager = Pager::new(Doc::rows_of(10, &[]));
    pager.handle_key(typed('n'));
    assert_eq!(
        pager.note().map(|note| note.text.as_str()),
        Some("nothing to find yet: press / to search")
    );
    // With a find that matched nothing, `n` says nothing: the chip does.
    keys(&mut pager, "/zebra<Enter>n");
    assert!(pager.note().is_none());
    let shown = screen(&mut pager, 60, 10);
    assert!(shown.contains("find \"zebra\" no match"), "{shown}");
}

#[test]
fn an_empty_query_clears_the_find() {
    let mut pager = Pager::new(Doc::rows_of(10, &[3]));
    keys(&mut pager, "/needle<Enter>");
    assert!(pager.find().is_active());
    keys(&mut pager, "/<C-u><Enter>");
    assert!(!pager.find().is_active());
    assert!(!screen(&mut pager, 60, 10).contains("find \""));
}

#[test]
fn the_prompt_draws_in_the_footer_with_its_keys() {
    let mut pager = Pager::new(Doc::rows_of(10, &[3]));
    keys(&mut pager, "/nee");
    let shown = draw(&mut pager, 60, 10);
    let footer = shown.last().expect("a footer");
    assert!(footer.starts_with("/nee"), "{footer}");
    assert!(footer.contains("Enter find · Esc cancel"), "{footer}");
}

#[test]
fn the_match_the_pager_is_on_draws_in_the_selection_look() {
    let theme = theme();
    let mut pager = Pager::new(Doc::rows_of(20, &[2, 4]));
    keys(&mut pager, "/needle<Enter>");
    let area = Rect::new(0, 0, 60, 14);
    let mut buf = Buffer::empty(area);
    pager.draw(area, &mut buf, &theme);
    let current = find_in_buffer(&buf, "row 2 holds a needle").expect("the current row");
    let other = find_in_buffer(&buf, "row 4 holds a needle").expect("the other row");
    let at =
        |position: ratatui::layout::Position, x: u16| buf[(position.x + x, position.y)].style();
    // `needle` starts 14 cells into either row.
    assert!(at(current, 14).add_modifier.contains(Modifier::REVERSED));
    assert_eq!(at(other, 14).fg, theme.match_style().fg);
    assert!(!at(other, 14).add_modifier.contains(Modifier::REVERSED));
    assert_eq!(
        at(current, 0),
        at(other, 0),
        "the rest of the row is unchanged"
    );
}

#[test]
fn a_find_runs_again_when_the_content_lays_out_and_stays_on_its_unit() {
    let mut pager = Pager::new(Doc::rows_of(20, &[3, 9]));
    draw(&mut pager, 60, 10);
    keys(&mut pager, "/needle<Enter>n");
    assert_eq!(pager.find().current_unit(), Some(8));
    let offset = {
        draw(&mut pager, 60, 10);
        pager.window().offset()
    };
    pager.handle_key(typed('+'));
    assert_eq!(
        pager.find().current_unit(),
        Some(8),
        "still on the same unit"
    );
    draw(&mut pager, 60, 10);
    assert_eq!(
        pager.window().offset(),
        offset,
        "and not scrolled to it again"
    );
}

#[test]
fn land_goes_to_the_match_nearest_a_unit() {
    let mut pager = Pager::new(Doc::rows_of(80, &[5, 40, 70]));
    assert!(pager.land("needle", 50));
    draw(&mut pager, 60, 20);
    assert_eq!(pager.find().current_unit(), Some(39));
    assert_eq!(pager.window().offset(), 38);
    let mut pager = Pager::new(Doc::rows_of(80, &[])).at_row(30);
    assert!(!pager.land("needle", 50));
    assert!(pager.find().is_active(), "the find is active with no match");
    draw(&mut pager, 60, 20);
    assert_eq!(
        pager.window().offset(),
        30,
        "and the pager stays where it was"
    );
}

#[test]
fn the_find_chip_carries_the_contents_detail_when_nothing_matches() {
    let mut doc = Doc::rows_of(10, &[]);
    doc.hidden = 3;
    let mut pager = Pager::new(doc);
    keys(&mut pager, "/needle<Enter>");
    let shown = screen(&mut pager, 80, 10);
    assert!(
        shown.contains("find \"needle\" no match · 3 hidden"),
        "{shown}"
    );
}

// ---- help -----------------------------------------------------------------

#[test]
fn the_help_page_lists_the_contents_keys_before_the_keys_that_leave() {
    let pager = Pager::new(Doc::rows_of(10, &[]));
    let titles: Vec<String> = pager
        .help_sections()
        .iter()
        .map(|section| section.title.to_string())
        .collect();
    assert_eq!(titles, ["Moving", "Finding", "Document", "Viewer"]);
    // Every key either table binds is described.
    assert!(crate::help::undescribed(READER_KEYS).is_empty());
    assert!(crate::help::undescribed(DOC_KEYS).is_empty());
}

#[test]
fn the_help_page_scrolls_inside_itself_and_has_its_own_footer() {
    let mut pager = Pager::new(Doc::rows_of(100, &[]));
    draw(&mut pager, 60, 14);
    pager.handle_key(typed('?'));
    let shown = screen(&mut pager, 60, 14);
    assert!(shown.contains("Help"), "{shown}");
    assert!(
        shown.contains("j/k/PgDn/PgUp scroll · Esc/? back · q close"),
        "{shown}"
    );
    pager.handle_key(shift('G'));
    screen(&mut pager, 60, 14);
    assert!(
        pager
            .help_scroll()
            .is_some_and(|scroll| scroll.offset() > 0)
    );
    assert_eq!(pager.window().offset(), 0, "the body did not move");
    pager.handle_key(typed('?'));
    assert!(!pager.help_is_open(), "`?` closes it again");
}

// ---- drawing --------------------------------------------------------------

#[test]
fn a_full_screen_pager_has_a_header_rules_the_body_and_a_position_chip() {
    let mut pager = Pager::new(Doc::rows_of(40, &[]));
    let shown = draw(&mut pager, 100, 10);
    assert_eq!(shown[0].trim_end(), " document · 40 lines");
    assert!(shown[1].starts_with(&marks::RULE.repeat(100)));
    assert_eq!(shown[2].trim_end(), "row 1");
    assert!(shown[8].starts_with(&marks::RULE.repeat(100)));
    assert_eq!(
        shown[9].trim_end(),
        "[ 1/40 ] · j/k/PgDn/PgUp scroll · / find · ? help · a ask · Esc/q close"
    );
}

#[test]
fn a_narrow_footer_drops_the_scroll_keys_then_the_others_then_chips_and_keeps_leaving() {
    let mut doc = Doc::rows_of(40, &[]);
    doc.chips = vec!["one·on", "two·off"];
    let mut pager = Pager::new(doc);
    let footer = |pager: &mut Pager<Doc>, width| draw(pager, width, 10)[9].trim_end().to_string();
    assert_eq!(
        footer(&mut pager, 100),
        "[ 1/40 ] · one·on · two·off · j/k/PgDn/PgUp scroll · / find · ? help · a ask · Esc/q close"
    );
    assert_eq!(
        footer(&mut pager, 70),
        "[ 1/40 ] · one·on · two·off · / find · ? help · a ask · Esc/q close"
    );
    assert_eq!(
        footer(&mut pager, 55),
        "[ 1/40 ] · one·on · two·off · a ask · Esc/q close"
    );
    assert_eq!(
        footer(&mut pager, 42),
        "[ 1/40 ] · one·on · a ask · Esc/q close"
    );
    assert_eq!(footer(&mut pager, 32), "[ 1/40 ] · a ask · Esc/q close");
    assert_eq!(footer(&mut pager, 24), "[ 1/40 ] · Esc/q close");
    assert_eq!(footer(&mut pager, 14), "[ 1/40 ] · Es…");
}

#[test]
fn a_header_too_wide_is_cut_with_the_ellipsis() {
    let mut pager = Pager::new(Doc::rows_of(40, &[]));
    let shown = draw(&mut pager, 12, 8);
    assert_eq!(shown[0], " document ·…");
}

#[test]
fn a_framed_pager_is_titled_scrolls_with_a_bar_and_keeps_its_footer_down() {
    let mut pager =
        Pager::new(Doc::rows_of(60, &[])).mode(PagerMode::Modal(crate::frame::ModalSize::WIDE));
    let shown = draw(&mut pager, 80, 20);
    assert!(shown[1].contains("┌ document · 60 lines "), "{shown:?}");
    let footer_row = &shown[shown.len() - 3];
    assert!(
        footer_row.contains("j/k/PgDn/PgUp scroll · / find · ? help · a ask · Esc/q close"),
        "{shown:?}"
    );
    // The bar takes the body's last column, inside the frame.
    let bar_column = shown[3].chars().count() - 4;
    assert!(
        shown[3..10]
            .iter()
            .any(|row| row.chars().nth(bar_column).is_some_and(|c| c != ' ')),
        "{shown:?}"
    );
    assert_eq!(
        pager.window().viewport(),
        15,
        "the body less the footer row"
    );
    assert!(!Overlay::<String, ()>::fills_screen(&pager));
}

#[test]
fn a_framed_pager_shows_its_find_above_the_footer() {
    let mut pager =
        Pager::new(Doc::rows_of(60, &[30])).mode(PagerMode::Modal(crate::frame::ModalSize::WIDE));
    keys(&mut pager, "/needle<Enter>");
    let shown = draw(&mut pager, 80, 20);
    let status = &shown[shown.len() - 4];
    assert!(status.contains("find \"needle\" 1/1"), "{shown:?}");
    keys(&mut pager, "/x");
    let shown = draw(&mut pager, 80, 20);
    assert!(shown[shown.len() - 4].contains("/needlex"), "{shown:?}");
}

#[test]
fn an_embedded_pager_draws_its_body_and_a_scrollbar_and_nothing_else() {
    let mut pager = Pager::new(Doc::rows_of(60, &[])).mode(PagerMode::Embedded);
    let shown = draw(&mut pager, 30, 8);
    assert!(shown[0].starts_with("row 1 "), "{shown:?}");
    assert!(shown.iter().all(|row| !row.contains("scroll")));
    let bar = |row: &String| row.chars().nth(29).is_some_and(|c| c != ' ');
    assert!(
        shown.iter().any(bar),
        "a scrollbar in the last column: {shown:?}"
    );
    assert_eq!(pager.window().viewport(), 8);
    let mut pager = Pager::new(Doc::rows_of(3, &[])).mode(PagerMode::Embedded);
    let shown = draw(&mut pager, 30, 8);
    assert!(!shown.iter().any(bar), "content that fits has no bar");
}

#[test]
fn an_empty_content_says_so() {
    let mut pager = Pager::new(Doc::new(Vec::new()));
    let shown = draw(&mut pager, 40, 8);
    assert_eq!(shown[2].trim_end(), "nothing here");
    assert!(shown[7].starts_with("[ 1/1 ]"));
}

#[test]
fn a_full_screen_pager_fills_the_screen_and_takes_text_while_finding() {
    let mut pager = Pager::new(Doc::rows_of(3, &[]));
    assert!(Overlay::<String, ()>::fills_screen(&pager));
    assert!(!Overlay::<String, ()>::takes_text(&pager));
    pager.handle_key(typed('/'));
    assert!(Overlay::<String, ()>::takes_text(&pager));
    assert_eq!(
        pager.footer().text(),
        "j/k/PgDn/PgUp scroll · / find · ? help · a ask · Esc/q close"
    );
}

#[test]
fn styles_come_from_the_theme() {
    let theme = theme();
    let mut pager = Pager::new(Doc::rows_of(40, &[]));
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    pager.draw(area, &mut buf, &theme);
    assert_eq!(
        style_at(&buf, "[ 1/40 ]").map(|style| style.add_modifier),
        Some(Modifier::BOLD)
    );
    assert_eq!(
        style_at(&buf, "j/k").and_then(|style| style.fg),
        Some(theme.inert)
    );
    assert_eq!(
        buf[(0, 1)].style().fg,
        Some(theme.inert),
        "the rule recedes"
    );
}

// ---- conformance ----------------------------------------------------------

/// A host with nothing on screen but its stack of pagers.
struct Host(Theme);

impl Rig for Host {
    type State = ModalStack<Pager<Doc>>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        ratatui::widgets::Widget::render(
            ratatui::widgets::Paragraph::new("the screen under the pager"),
            area,
            &mut buf,
        );
        stack.draw::<String, ()>(area, &mut buf, &(), &self.0);
        buf
    }

    fn close_top(&self, stack: &mut Self::State) {
        stack.pop();
    }

    fn press(&self, stack: &mut Self::State, key: KeyEvent) {
        if let Some(outcome) = stack.route::<String, ()>(key, &()) {
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
fn a_pager_keeps_the_conventions_every_modal_keeps() {
    let screen = || {
        let mut stack = ModalStack::new();
        stack.push(Pager::new(Doc::rows_of(80, &[3, 50])));
        stack
    };
    let framed = || {
        let mut stack = ModalStack::new();
        stack.push(
            Pager::new(Doc::rows_of(80, &[3, 50]))
                .mode(PagerMode::Modal(crate::frame::ModalSize::WIDE)),
        );
        stack
    };
    let reader = Convention::new("Pager", EnterMeans::Nothing, SpaceMeans::Pages, "a pager");
    let find = Convention::new("Find", EnterMeans::Submits, SpaceMeans::Types, "");
    let tables = || {
        [
            conformance::table("READER_KEYS", READER_KEYS),
            conformance::part("DOC_KEYS", DOC_KEYS),
        ]
    };
    let footer = Pager::new(Doc::rows_of(1, &[])).footer();
    let registry = Registry::new(Host(theme()))
        .case(
            Case::new("Pager", screen)
                .full_screen()
                .keys(reader, tables())
                .keys(find, [conformance::table("FIND_KEYS", FIND_KEYS)])
                .footer(footer.clone()),
        )
        .case(
            Case::new("Pager", framed)
                .variant("framed")
                .unchecked_selection("its whole body scrolls with the arrow keys")
                .keys(reader, tables())
                .footer(footer),
        );
    conformance::assert_conventions(&registry);
}

#[test]
fn the_pagers_tables_label_their_keys_as_the_help_page_writes_them() {
    let pager = Pager::new(Doc::rows_of(1, &[]));
    assert_eq!(pager.label_of(PagerKey::Find), "/");
    assert_eq!(
        pager.label_of(PagerKey::Abort),
        key_label(&crate::keys::ctrl('c'))
    );
    assert_eq!(pager.table(), READER_KEYS);
    assert_eq!(pager.draw_mode(), PagerMode::Screen);
}
