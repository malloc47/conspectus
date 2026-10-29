use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use super::*;
use crate::keys::{PAGER_KEYS, ch, include, shown};
use crate::overlay::Outcome;
use crate::testkit::keys::{press, typed};

/// A pager closed by `?` too, as a help page opened by `?` is.
const CLOSE_KEYS: &[KeyRow<PagerKey>] = &[
    include(PAGER_KEYS),
    shown(ch('?'), PagerKey::Close, "?", "close"),
];

const LONG: &str = "Move the cursor down one row, or to the first row of the next group when \
                    the cursor is on the last row of its own";

fn sections() -> Vec<HelpSection> {
    vec![
        HelpSection::new("Moving")
            .keys("j / ↓", "Scroll one line down")
            .keys("k / ↑", LONG)
            .keys("Shift-Enter / Alt-Enter", "Open in place"),
        HelpSection::new("Marks")
            .legend(Span::raw("*"), "Marked", "the row is marked")
            .legend(Span::raw("+"), "Added", LONG),
    ]
}

fn page(sections: &[HelpSection]) -> HelpPage {
    HelpPage::new(sections.to_vec())
}

fn lines(sections: &[HelpSection], width: usize) -> Vec<Line<'static>> {
    page(sections).lines(&Theme::default(), width)
}

fn text_of(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn page_text(sections: &[HelpSection], width: usize) -> Vec<String> {
    lines(sections, width).iter().map(text_of).collect()
}

/// A tall page: `rows` key rows in one section.
fn tall(rows: usize) -> Vec<HelpSection> {
    let mut section = HelpSection::new("Rows");
    for n in 0..rows {
        section = section.keys(format!("{n}"), format!("row {n}"));
    }
    vec![section]
}

/// The page of `sections` in its pager, closed by `?` too.
fn reader(sections: &[HelpSection]) -> Pager<HelpPage> {
    page(sections).pager(CLOSE_KEYS)
}

fn draw(pager: &mut Pager<HelpPage>, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    pager.draw(area, &mut buf, &Theme::default());
    buf
}

fn row_text(buf: &Buffer, y: u16, from: u16, to: u16) -> String {
    (from..to).map(|x| buf[(x, y)].symbol()).collect()
}

#[test]
fn a_key_row_puts_the_keys_in_the_accent_column_and_the_description_beside_them() {
    let theme = Theme::default();
    let lines = lines(&sections(), 80);
    assert_eq!(text_of(&lines[0]), "Moving");
    assert!(
        lines[0].spans[0]
            .style
            .add_modifier
            .contains(Modifier::BOLD)
    );
    let row = &lines[1];
    assert_eq!(text_of(row), "  j / ↓         Scroll one line down");
    assert_eq!(row.spans[0].style.fg, Some(theme.accent));
    assert_eq!(text::width(&row.spans[0].content), KEY_COLUMN);
    assert_eq!(
        row.spans[1].style,
        Style::default(),
        "the description is plain"
    );
}

#[test]
fn a_description_wraps_under_its_hanging_indent() {
    let text = page_text(&sections(), 60);
    let start = text
        .iter()
        .position(|t| t.starts_with("  k / ↑"))
        .expect("k row");
    assert!(text[start + 1].starts_with(&" ".repeat(KEY_COLUMN)));
    assert!(!text[start + 1][KEY_COLUMN..].starts_with(' '));
}

#[test]
fn keys_that_leave_no_space_before_the_description_take_a_line_of_their_own() {
    let sections = [HelpSection::new("Edge")
        .keys("a".repeat(13), "fits")
        .keys("b".repeat(14), "below")];
    let text = page_text(&sections, 80);
    assert_eq!(text[1], format!("  {} fits", "a".repeat(13)));
    assert_eq!(text[2], format!("  {}", "b".repeat(14)));
    assert_eq!(text[3], format!("{}below", " ".repeat(KEY_COLUMN)));
}

#[test]
fn a_heading_wraps_rather_than_running_past_the_edge() {
    let sections = [HelpSection::new("A heading longer than the page is wide").keys("x", "y")];
    let lines = lines(&sections, 24);
    assert_eq!(text_of(&lines[0]), "A heading longer than");
    assert_eq!(text_of(&lines[1]), "the page is wide");
    assert!(
        lines[1].spans[0]
            .style
            .add_modifier
            .contains(Modifier::BOLD)
    );
}

#[test]
fn legend_rows_share_a_mark_column_and_a_name_column() {
    let text = page_text(&sections(), 80);
    let marked = text
        .iter()
        .position(|t| t.contains("Marked"))
        .expect("legend");
    assert_eq!(text[marked], "  * Marked          the row is marked");
    assert!(text[marked + 1].starts_with("  + Added           Move the cursor"));
    // The blurb wraps under the name column's end.
    assert!(text[marked + 2].starts_with(&" ".repeat(INDENT + 2 + NAME_COLUMN_MIN)));
    // A longer name widens its section's column, and only that.
    let long = [HelpSection::new("Long").legend(Span::raw("*"), "A name of twenty one", "blurb")];
    let text = page_text(&long, 80);
    assert_eq!(text[1], "  * A name of twenty one blurb");
}

#[test]
fn a_legend_mark_keeps_its_style_and_the_name_is_bold() {
    let theme = Theme::default();
    let mark = Span::styled("*", Style::default().fg(theme.warning));
    let sections = [HelpSection::new("Marks").legend(mark, "Warned", "careful")];
    let lines = lines(&sections, 80);
    assert_eq!(lines[1].spans[1].style.fg, Some(theme.warning));
    assert!(
        lines[1].spans[2]
            .style
            .add_modifier
            .contains(Modifier::BOLD)
    );
}

#[test]
fn no_line_is_wider_than_the_page_and_no_word_is_lost() {
    let words = |lines: Vec<String>| -> Vec<String> {
        lines
            .iter()
            .flat_map(|l| l.split_whitespace().map(str::to_string))
            .collect()
    };
    let source = words(page_text(&sections(), usize::MAX));
    for width in [30, 40, 55, 80, 120] {
        let text = page_text(&sections(), width);
        for line in &text {
            assert!(text::width(line) <= width, "{line:?} at {width}");
        }
        assert_eq!(words(text), source, "at {width}");
    }
}

#[test]
fn an_unavailable_row_is_drawn_in_the_unavailable_style_throughout() {
    let theme = Theme::default();
    let sections = [HelpSection::new("Rows")
        .row(HelpRow::keys("x", LONG).available(false))
        .keys("y", "Still here")];
    let lines = lines(&sections, 50);
    let dim = unavailable_style(&theme);
    let x_rows: Vec<&Line<'_>> = lines[1..]
        .iter()
        .take_while(|line| !text_of(line).contains("Still"))
        .collect();
    assert!(x_rows.len() > 1, "the row wraps");
    for line in x_rows {
        for span in &line.spans {
            assert_eq!(span.style.fg, dim.fg, "{:?}", span.content);
            assert!(span.style.add_modifier.contains(theme.muted));
        }
    }
    let y = lines.iter().find(|l| text_of(l).contains("Still")).unwrap();
    assert_eq!(y.spans[0].style.fg, Some(theme.accent));
}

#[test]
fn the_page_lays_out_once_per_width() {
    let theme = Theme::default();
    let mut page = page(&tall(10));
    let rows = page.layout(40, &theme);
    assert_eq!(rows, 12, "a heading, ten rows, and a blank line");
    assert_eq!(page.rows(1..2, &theme)[0], lines(&tall(10), 40)[1]);
    assert_eq!(page.header(40, &theme), Line::from("Help"));
    let titled = HelpPage::<()>::new(tall(1)).title("Keys");
    assert_eq!(titled.header(40, &theme), Line::from("Keys"));
}

#[test]
fn the_footer_is_generated_from_its_table_and_recedes() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut pager = reader(&sections());
    let buf = draw(&mut pager, area);
    let rect = HelpPage::SIZE.rect(area, 0);
    let footer_row = rect.y + rect.height - 2;
    let footer = row_text(&buf, footer_row, rect.x + 1, rect.x + rect.width - 1);
    assert!(
        footer.starts_with(&crate::keys::footer(CLOSE_KEYS)),
        "{footer:?}"
    );
    assert!((rect.x + 1..rect.x + 10).all(|x| buf[(x, footer_row)].modifier.contains(theme.muted)));
    assert_eq!(pager.footer().text(), crate::keys::footer(CLOSE_KEYS));
}

#[test]
fn the_page_draws_in_the_reader_frame_titled_help() {
    let area = Rect::new(0, 0, 80, 24);
    let buf = draw(&mut reader(&sections()), area);
    let rect = HelpPage::SIZE.rect(area, 0);
    assert_eq!(buf[(rect.x, rect.y)].symbol(), "┌");
    assert_eq!(row_text(&buf, rect.y, rect.x + 1, rect.x + 7), " Help ");
    assert_eq!(buf[(rect.x, rect.y)].fg, Theme::default().accent);
    assert_eq!(
        row_text(&buf, rect.y + 1, rect.x + 1, rect.x + 7),
        "Moving",
        "the first heading opens the page"
    );
}

#[test]
fn the_footer_stays_on_the_last_rows_whatever_the_scroll() {
    let area = Rect::new(0, 0, 60, 16);
    let rect = HelpPage::SIZE.rect(area, 0);
    let footer = crate::keys::footer(CLOSE_KEYS);
    let mut pager = reader(&tall(40));
    for keys in ["", "jjjjj", "G"] {
        for c in keys.chars() {
            pager.handle_key(typed(c));
        }
        let buf = draw(&mut pager, area);
        let last = row_text(
            &buf,
            rect.y + rect.height - 2,
            rect.x + 1,
            rect.x + rect.width - 1,
        );
        assert!(last.starts_with(&footer), "{last:?} after {keys:?}");
    }
}

#[test]
fn an_overflowing_page_gets_a_scrollbar_and_its_text_a_column_less() {
    let area = Rect::new(0, 0, 60, 16);
    let rect = HelpPage::SIZE.rect(area, 0);
    let bar = rect.x + rect.width - 2;
    let mut pager = reader(&tall(40));
    let buf = draw(&mut pager, area);
    assert!(pager.window().overflows());
    let column: String = (rect.y + 1..rect.y + rect.height - 2)
        .map(|y| buf[(bar, y)].symbol().to_string())
        .collect();
    assert!(!column.trim().is_empty(), "the bar is drawn: {column:?}");
    // A page that fits keeps every column and draws no bar.
    let mut pager = reader(&tall(3));
    let buf = draw(&mut pager, area);
    assert!(!pager.window().overflows());
    let column: String = (rect.y + 1..rect.y + rect.height - 2)
        .map(|y| buf[(bar, y)].symbol().to_string())
        .collect();
    assert!(column.trim().is_empty(), "{column:?}");
}

#[test]
fn the_end_key_shows_the_last_page_not_blank_rows() {
    let area = Rect::new(0, 0, 60, 16);
    let rect = HelpPage::SIZE.rect(area, 0);
    let mut pager = reader(&tall(40));
    pager.handle_key(typed('G'));
    let buf = draw(&mut pager, area);
    let window = pager.window();
    assert_eq!(window.offset(), window.last_offset());
    // The last row of the page sits right above the footer: no blank
    // tail.
    let footer_row = rect.y + rect.height - 2;
    let above: Vec<String> = (rect.y + 1..footer_row)
        .map(|y| {
            row_text(&buf, y, rect.x + 1, rect.x + 20)
                .trim_end()
                .to_string()
        })
        .collect();
    assert_eq!(
        above.last().map(String::as_str),
        Some(""),
        "the section's blank line"
    );
    assert!(above[above.len() - 2].starts_with("  39"), "{above:?}");
}

#[test]
fn footer_rows_that_do_not_fit_scroll_with_the_page() {
    // A frame two rows inside: the footer's last row stays, and the rows
    // above it scroll with the page, after its last row.
    let area = Rect::new(0, 0, 24, 4);
    let mut pager = reader(&tall(2));
    let buf = draw(&mut pager, area);
    let rect = HelpPage::SIZE.rect(area, 0);
    assert_eq!(rect.height, 4, "{rect:?}");
    let inside = |buf: &Buffer, y: u16| row_text(buf, y, rect.x + 1, rect.x + rect.width - 1);
    assert!(
        inside(&buf, rect.y + 2).starts_with("Esc/q/? close"),
        "{buf:?}"
    );
    pager.handle_key(typed('G'));
    let buf = draw(&mut pager, area);
    assert!(inside(&buf, rect.y + 1).contains("scroll"), "{buf:?}");
    assert!(
        inside(&buf, rect.y + 2).starts_with("Esc/q/? close"),
        "{buf:?}"
    );
}

#[test]
fn page_keys_move_by_the_rows_the_page_showed() {
    let area = Rect::new(0, 0, 60, 16);
    let mut pager = reader(&tall(40));
    draw(&mut pager, area);
    let shown = pager.window().viewport();
    pager.handle_key(press(KeyCode::PageDown));
    draw(&mut pager, area);
    assert_eq!(pager.window().offset(), shown);
    pager.handle_key(crate::testkit::keys::ctrl('u'));
    draw(&mut pager, area);
    assert_eq!(pager.window().offset(), shown - shown / 2);
    pager.handle_key(press(KeyCode::PageUp));
    pager.handle_key(press(KeyCode::PageUp));
    draw(&mut pager, area);
    assert_eq!(pager.window().offset(), 0, "stops at the top");
}

#[test]
fn keys_past_the_end_are_taken_back_by_the_next_draw() {
    let area = Rect::new(0, 0, 60, 16);
    let mut pager = reader(&tall(40));
    draw(&mut pager, area);
    pager.handle_key(typed('G'));
    draw(&mut pager, area);
    let end = pager.window();
    for _ in 0..5 {
        pager.handle_key(typed('j'));
    }
    draw(&mut pager, area);
    assert_eq!(pager.window().offset(), end.last_offset());
    pager.handle_key(typed('k'));
    draw(&mut pager, area);
    assert_eq!(
        pager.window().offset(),
        end.last_offset() - 1,
        "one key up moves up"
    );
}

#[test]
fn the_pages_table_scrolls_it_and_closes_it() {
    let mut pager = reader(&tall(40));
    for key in [typed('j'), press(KeyCode::PageDown), typed('/'), typed('x')] {
        assert_eq!(pager.handle_key(key), Outcome::Consumed, "{key:?}");
    }
    assert!(!pager.takes_text(), "a page with no find has no prompt");
    assert_eq!(pager.handle_key(typed('?')), Outcome::Close);
    assert_eq!(pager.handle_key(press(KeyCode::Esc)), Outcome::Close);
    assert_eq!(pager.handle_key(typed('q')), Outcome::Dismiss);
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

#[test]
fn after_any_keys_and_resizes_the_drawn_offset_is_inside_the_page() {
    let keys = [
        typed('j'),
        typed('k'),
        press(KeyCode::PageDown),
        press(KeyCode::PageUp),
        crate::testkit::keys::ctrl('d'),
        crate::testkit::keys::ctrl('u'),
        typed('G'),
        typed('g'),
    ];
    for seed in 1..=24u64 {
        let mut rng = Gen(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut pager = reader(&tall(5 + rng.below(60)));
        for step in 0..60 {
            for _ in 0..1 + rng.below(4) {
                pager.handle_key(keys[rng.below(keys.len())]);
            }
            let area = Rect::new(0, 0, 30 + rng.below(90) as u16, 10 + rng.below(30) as u16);
            let buf = draw(&mut pager, area);
            let window = pager.window();
            assert!(
                window.offset() <= window.last_offset(),
                "seed {seed} step {step}: {window:?}"
            );
            // The window shows rows whenever the page has any.
            let rect = HelpPage::SIZE.rect(area, 0);
            let first = row_text(&buf, rect.y + 1, rect.x + 1, rect.x + rect.width - 2);
            assert!(
                !first.trim().is_empty(),
                "seed {seed} step {step}: a blank top row"
            );
        }
    }
}
