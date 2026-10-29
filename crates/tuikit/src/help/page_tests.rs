use super::*;
use crate::keys::{PAGER_KEYS, ch, include, shown, with_availability};

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

fn text_of(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn page_text(page: &HelpPage<'_>, width: usize) -> Vec<String> {
    page.lines(width).iter().map(text_of).collect()
}

/// A tall page: `rows` key rows in one section.
fn tall(rows: usize) -> Vec<HelpSection> {
    let mut section = HelpSection::new("Rows");
    for n in 0..rows {
        section = section.keys(format!("{n}"), format!("row {n}"));
    }
    vec![section]
}

fn render(sections: &[HelpSection], area: Rect, scroll: &HelpScroll) -> (Buffer, ScrollWindow) {
    let theme = Theme::default();
    let mut buf = Buffer::empty(area);
    let window = HelpPage::new(&theme, sections)
        .footer(CLOSE_KEYS)
        .scrolled(scroll)
        .render(area, &mut buf);
    (buf, window)
}

fn row_text(buf: &Buffer, y: u16, from: u16, to: u16) -> String {
    (from..to).map(|x| buf[(x, y)].symbol()).collect()
}

#[test]
fn a_key_row_puts_the_keys_in_the_accent_column_and_the_description_beside_them() {
    let theme = Theme::default();
    let sections = sections();
    let lines = HelpPage::new(&theme, &sections).lines(80);
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
    let theme = Theme::default();
    let sections = sections();
    let text = page_text(&HelpPage::new(&theme, &sections), 60);
    let start = text
        .iter()
        .position(|t| t.starts_with("  k / ↑"))
        .expect("k row");
    assert!(text[start + 1].starts_with(&" ".repeat(KEY_COLUMN)));
    assert!(!text[start + 1][KEY_COLUMN..].starts_with(' '));
}

#[test]
fn keys_that_leave_no_space_before_the_description_take_a_line_of_their_own() {
    let theme = Theme::default();
    let sections = [HelpSection::new("Edge")
        .keys("a".repeat(13), "fits")
        .keys("b".repeat(14), "below")];
    let text = page_text(&HelpPage::new(&theme, &sections), 80);
    assert_eq!(text[1], format!("  {} fits", "a".repeat(13)));
    assert_eq!(text[2], format!("  {}", "b".repeat(14)));
    assert_eq!(text[3], format!("{}below", " ".repeat(KEY_COLUMN)));
}

#[test]
fn a_heading_wraps_rather_than_running_past_the_edge() {
    let theme = Theme::default();
    let sections = [HelpSection::new("A heading longer than the page is wide").keys("x", "y")];
    let lines = HelpPage::new(&theme, &sections).lines(24);
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
    let theme = Theme::default();
    let sections = sections();
    let text = page_text(&HelpPage::new(&theme, &sections), 80);
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
    let text = page_text(&HelpPage::new(&theme, &long), 80);
    assert_eq!(text[1], "  * A name of twenty one blurb");
}

#[test]
fn a_legend_mark_keeps_its_style_and_the_name_is_bold() {
    let theme = Theme::default();
    let mark = Span::styled("*", Style::default().fg(theme.warning));
    let sections = [HelpSection::new("Marks").legend(mark, "Warned", "careful")];
    let lines = HelpPage::new(&theme, &sections).lines(80);
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
    let theme = Theme::default();
    let sections = sections();
    let page = HelpPage::new(&theme, &sections).footer(CLOSE_KEYS);
    let words = |lines: Vec<String>| -> Vec<String> {
        lines
            .iter()
            .flat_map(|l| l.split_whitespace().map(str::to_string))
            .collect()
    };
    let source = words(page_text(&page, usize::MAX));
    for width in [30, 40, 55, 80, 120] {
        let text = page_text(&page, width);
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
    let lines = HelpPage::new(&theme, &sections).lines(50);
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
fn the_footer_is_generated_from_its_table_and_recedes() {
    let theme = Theme::default();
    let sections = sections();
    let page = HelpPage::new(&theme, &sections).footer(CLOSE_KEYS);
    let lines = page.footer_lines(80);
    assert_eq!(lines.len(), 1);
    assert_eq!(text_of(&lines[0]), crate::keys::footer(CLOSE_KEYS));
    assert!(
        lines[0]
            .spans
            .iter()
            .all(|span| span.style.add_modifier.contains(theme.muted))
    );
    // Wrapped only between segments, and unwrapped on a tiny page.
    assert_eq!(page.footer_lines(20).len(), 2);
    assert_eq!(page.footer_lines(8).len(), 1);
    // No table, no footer.
    assert!(HelpPage::new(&theme, &sections).footer_lines(80).is_empty());
    // A key that cannot act now is dimmed in the footer as well.
    let rows = with_availability(CLOSE_KEYS, |key| key != PagerKey::Up);
    let dimmed = HelpPage::new(&theme, &sections).footer(&rows);
    let unavailable = unavailable_style(&theme);
    assert!(
        dimmed.footer_lines(80)[0]
            .spans
            .iter()
            .all(|span| span.style.fg != unavailable.fg),
        "j still scrolls, so the segment stays available"
    );
}

#[test]
fn the_page_draws_in_the_reader_frame_titled_help() {
    let area = Rect::new(0, 0, 80, 24);
    let (buf, _) = render(&sections(), area, &HelpScroll::new());
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
    let sections = tall(40);
    let rect = HelpPage::SIZE.rect(area, 0);
    let footer = crate::keys::footer(CLOSE_KEYS);
    for offset in [0, 5, usize::MAX] {
        let (buf, _) = render(&sections, area, &HelpScroll::at(offset));
        let last = row_text(
            &buf,
            rect.y + rect.height - 2,
            rect.x + 1,
            rect.x + rect.width - 1,
        );
        assert!(last.starts_with(&footer), "{last:?} at {offset}");
    }
}

#[test]
fn an_overflowing_page_gets_a_scrollbar_and_its_text_a_column_less() {
    let area = Rect::new(0, 0, 60, 16);
    let rect = HelpPage::SIZE.rect(area, 0);
    let bar = rect.x + rect.width - 2;
    let (buf, window) = render(&tall(40), area, &HelpScroll::new());
    assert!(window.overflows());
    let column: String = (rect.y + 1..rect.y + rect.height - 2)
        .map(|y| buf[(bar, y)].symbol().to_string())
        .collect();
    assert!(!column.trim().is_empty(), "the bar is drawn: {column:?}");
    // A page that fits keeps every column and draws no bar.
    let (buf, window) = render(&tall(3), area, &HelpScroll::new());
    assert!(!window.overflows());
    let column: String = (rect.y + 1..rect.y + rect.height - 2)
        .map(|y| buf[(bar, y)].symbol().to_string())
        .collect();
    assert!(column.trim().is_empty(), "{column:?}");
}

#[test]
fn scrolling_past_the_end_shows_the_last_page_not_blank_rows() {
    let area = Rect::new(0, 0, 60, 16);
    let sections = tall(40);
    let rect = HelpPage::SIZE.rect(area, 0);
    let (buf, window) = render(&sections, area, &HelpScroll::at(usize::MAX));
    assert_eq!(window.offset(), window.last_offset());
    // The last row of the page sits right above the blank line and the
    // footer: no blank tail.
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
    let theme = Theme::default();
    let sections = tall(2);
    let page = HelpPage::new(&theme, &sections).footer(CLOSE_KEYS);
    // Two rows high: the footer's last row stays, and the rows above it
    // scroll with the page, which overflows and gives a column to the bar.
    let layout = page.layout(20, 2);
    let footer = page.footer_lines(19);
    let spill = footer.len() - 1;
    assert!(spill > 0);
    assert_eq!(layout.text_width, 19);
    assert_eq!(layout.body_height, 1);
    assert_eq!(layout.footer, footer[spill..]);
    assert_eq!(layout.body[layout.body.len() - spill..], footer[..spill]);
}

#[test]
fn page_keys_move_by_the_rows_the_page_showed() {
    let area = Rect::new(0, 0, 60, 16);
    let sections = tall(40);
    let mut scroll = HelpScroll::new();
    assert_eq!(scroll.page(), 1, "before any draw a page is a row");
    let (_, window) = render(&sections, area, &scroll);
    scroll.keep(window);
    assert_eq!(scroll.page(), window.viewport());
    scroll.page_down();
    assert_eq!(scroll.offset(), window.viewport());
    scroll.half_page_up();
    assert_eq!(scroll.offset(), window.viewport() - window.viewport() / 2);
    scroll.half_page_down();
    scroll.page_up();
    scroll.page_up();
    assert_eq!(scroll.offset(), 0, "stops at the top");
}

#[test]
fn keys_past_the_end_are_taken_back_by_the_next_draw() {
    let area = Rect::new(0, 0, 60, 16);
    let sections = tall(40);
    let mut scroll = HelpScroll::new();
    scroll.to_bottom();
    for _ in 0..5 {
        scroll.down(1);
    }
    let (_, window) = render(&sections, area, &scroll);
    scroll.keep(window);
    assert_eq!(scroll.offset(), window.last_offset());
    scroll.up(1);
    let (_, moved) = render(&sections, area, &scroll);
    assert_eq!(
        moved.offset(),
        window.last_offset() - 1,
        "one key up moves up"
    );
}

#[test]
fn the_standard_pager_keys_scroll_and_leave_the_rest_to_the_control() {
    let mut scroll = HelpScroll::new();
    scroll.keep(ScrollWindow::new(10, 100));
    for (key, offset) in [
        (PagerKey::Down, 1),
        (PagerKey::PageDown, 11),
        (PagerKey::Up, 10),
        (PagerKey::HalfPageDown, 15),
        (PagerKey::HalfPageUp, 10),
        (PagerKey::PageUp, 0),
        (PagerKey::Bottom, usize::MAX),
        (PagerKey::Top, 0),
    ] {
        assert!(scroll.apply(key), "{key:?}");
        assert_eq!(scroll.offset(), offset, "{key:?}");
    }
    for key in [
        PagerKey::Abort,
        PagerKey::Close,
        PagerKey::Find,
        PagerKey::FindNext,
        PagerKey::FindPrev,
        PagerKey::Help,
        PagerKey::Copy,
    ] {
        assert!(!scroll.apply(key), "{key:?}");
        assert_eq!(scroll.offset(), 0);
    }
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
    for seed in 1..=24u64 {
        let mut rng = Gen(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let sections = tall(5 + rng.below(60));
        let mut scroll = HelpScroll::new();
        for step in 0..60 {
            match rng.below(8) {
                0 => scroll.down(1 + rng.below(4)),
                1 => scroll.up(1 + rng.below(4)),
                2 => scroll.page_down(),
                3 => scroll.page_up(),
                4 => scroll.half_page_down(),
                5 => scroll.half_page_up(),
                6 => scroll.to_bottom(),
                _ => scroll.to_top(),
            }
            let area = Rect::new(0, 0, 30 + rng.below(90) as u16, 10 + rng.below(30) as u16);
            let (buf, window) = render(&sections, area, &scroll);
            scroll.keep(window);
            assert!(
                scroll.offset() <= window.last_offset(),
                "seed {seed} step {step}: {scroll:?} past {window:?}"
            );
            assert_eq!(scroll.page(), window.viewport().max(1));
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
