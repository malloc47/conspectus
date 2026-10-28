//! A generated page drawn whole: every key a table binds reaches the
//! page, and one with no description does not, which `undescribed` says.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::*;
use crate::keys::{KeyRow, PAGER_KEYS, alias, ch, ctrl, key, key_label, rows};
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Viewer {
    Down,
    Up,
    HalfDown,
    Help,
    Leave,
    Quit,
    /// Bound by [`GAP_KEYS`] and never listed.
    Unlisted,
}

impl HelpAction for Viewer {
    const LISTED: &'static [Self] = &[
        Viewer::Down,
        Viewer::Up,
        Viewer::HalfDown,
        Viewer::Help,
        Viewer::Leave,
        Viewer::Quit,
    ];

    fn help(self) -> HelpEntry {
        match self {
            Viewer::Down => HelpEntry::new("Moving", "Scroll one line down"),
            Viewer::Up => HelpEntry::new("Moving", "Scroll one line up"),
            Viewer::HalfDown => HelpEntry::new("Moving", "Half a page down"),
            Viewer::Help => HelpEntry::new("Viewer", "Show or hide this help"),
            Viewer::Leave => HelpEntry::new("Viewer", "Step back one level"),
            Viewer::Quit => HelpEntry::new("Viewer", "Leave at once"),
            Viewer::Unlisted => HelpEntry::new("Viewer", "Not on the page"),
        }
    }
}

const VIEWER_KEYS: &[KeyRow<Viewer>] = &[
    alias(ctrl('c'), Viewer::Quit),
    alias(ctrl('d'), Viewer::HalfDown),
    alias(key(KeyCode::Esc), Viewer::Leave),
    alias(ch('q'), Viewer::Quit),
    alias(ch('j'), Viewer::Down),
    alias(key(KeyCode::Down), Viewer::Down),
    alias(ch('k'), Viewer::Up),
    alias(key(KeyCode::Up), Viewer::Up),
    alias(ch('?'), Viewer::Help),
];

const GAP_KEYS: &[KeyRow<Viewer>] = &[
    alias(ctrl('c'), Viewer::Quit),
    alias(ch('j'), Viewer::Down),
    alias(ch('x'), Viewer::Unlisted),
];

/// The page's text as drawn at `width` by `height`, scrolled through
/// from the top to the end, one row per drawn line.
fn drawn_page<A: HelpAction>(table: &[KeyRow<A>], width: u16, height: u16) -> Vec<String> {
    let theme = Theme::default();
    let sections = sections_for(table);
    let area = Rect::new(0, 0, width, height);
    let mut scroll = HelpScroll::new();
    let mut seen = Vec::new();
    loop {
        let mut buf = Buffer::empty(area);
        let window = HelpPage::new(&theme, &sections)
            .footer(PAGER_KEYS)
            .scrolled(&scroll)
            .render(area, &mut buf);
        scroll.keep(window);
        for y in 0..height {
            seen.push((0..width).map(|x| buf[(x, y)].symbol()).collect::<String>());
        }
        if window.offset() == window.last_offset() {
            return seen;
        }
        scroll.page_down();
    }
}

/// The keys `table` binds that no drawn row of its page lists beside the
/// start of a description.
fn missing_from_page<A: HelpAction>(table: &[KeyRow<A>], width: u16, height: u16) -> Vec<String> {
    let page = drawn_page(table, width, height);
    rows(table)
        .filter_map(|row| match row {
            KeyRow::Bind { key, action, .. } => {
                let label = key_label(key);
                let first_word = action.help().description.split_whitespace().next();
                let listed = page.iter().any(|line| {
                    let words: Vec<&str> = line.split_whitespace().collect();
                    words.contains(&label.as_str())
                        && first_word.is_some_and(|word| words.contains(&word))
                });
                (!listed).then_some(label)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn every_key_a_table_binds_is_on_its_page_beside_its_description() {
    for (width, height) in [(100, 30), (60, 20), (40, 12)] {
        assert_eq!(
            missing_from_page(VIEWER_KEYS, width, height),
            Vec::<String>::new(),
            "at {width}x{height}"
        );
    }
    assert!(undescribed(VIEWER_KEYS).is_empty());
}

#[test]
fn a_key_with_no_description_is_missing_and_reported() {
    assert_eq!(missing_from_page(GAP_KEYS, 100, 30), ["x"]);
    assert_eq!(undescribed(GAP_KEYS), ["x"]);
}

#[test]
fn a_generated_page_groups_rows_and_ends_with_its_footer() {
    let theme = Theme::default();
    let sections = sections_for(VIEWER_KEYS);
    let page = HelpPage::new(&theme, &sections).footer(PAGER_KEYS);
    let text: Vec<String> = page
        .lines(80)
        .iter()
        .map(|line: &Line<'_>| line.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect();
    assert_eq!(
        text,
        [
            "Moving",
            "  j / ↓         Scroll one line down",
            "  k / ↑         Scroll one line up",
            "  Ctrl-D        Half a page down",
            "",
            "Viewer",
            "  ?             Show or hide this help",
            "  Esc           Step back one level",
            "  q / Ctrl-C    Leave at once",
            "",
        ]
    );
    let footer: String = page.footer_lines(80)[0]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(footer, crate::keys::footer(PAGER_KEYS));
}
