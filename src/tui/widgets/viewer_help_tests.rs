use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::text::Line;
use tuikit::help::{HelpPage, HelpRow, HelpSection, undescribed};
use tuikit::keys::{KeyRow, key_label, rows};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::tui::keybindings::overlay::{HELP_KEYS, PANE_VIEWER_KEYS, VIEWER_KEYS};
use crate::tui::widgets::pane_viewer::PaneViewerMsg;
use crate::viewer::input::ViewerMsg;

fn render_text(width: u16, height: u16, scroll: usize) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    render(
        VIEWER_KEYS,
        area,
        &mut buf,
        &Theme::default(),
        &HelpScroll::at(scroll),
    );
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// The page of `table` laid out `width` cells wide, footer last.
fn help_lines<A: HelpAction>(
    table: &[KeyRow<A>],
    theme: &Theme,
    width: usize,
) -> Vec<Line<'static>> {
    let sections = sections_for(table);
    let page = crate::tui::widgets::help::page(theme, &sections);
    let mut lines = page.lines(width);
    lines.extend(page.footer_lines(width));
    lines
}

/// Every key row of `sections`, as `(keys, description, available)`.
fn key_rows(sections: &[HelpSection]) -> Vec<(String, String, bool)> {
    sections
        .iter()
        .flat_map(|section| &section.rows)
        .filter_map(|row| match row {
            HelpRow::Keys {
                keys,
                description,
                available,
            } => Some((keys.clone(), description.to_string(), *available)),
            HelpRow::Legend { .. } => None,
        })
        .collect()
}

/// Every key `table` binds that its page does not list exactly once, on
/// a row that says what the key's action does.
fn keys_missing_from_the_page<A: HelpAction + std::fmt::Debug>(table: &[KeyRow<A>]) -> Vec<String> {
    let page = key_rows(&sections_for(table));
    rows(table)
        .filter_map(|row| match row {
            KeyRow::Bind { key, action, .. } => {
                let label = key_label(key);
                let listing: Vec<&(String, String, bool)> = page
                    .iter()
                    .filter(|(keys, _, _)| keys.split(" / ").any(|k| k == label))
                    .collect();
                let described = listing.len() == 1
                    && listing[0].1 == action.help().description
                    && !listing[0].1.trim().is_empty();
                (!described).then(|| format!("{label} ({action:?})"))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn every_key_either_viewer_binds_is_described_once_on_its_page() {
    assert_eq!(
        keys_missing_from_the_page(VIEWER_KEYS),
        Vec::<String>::new()
    );
    assert_eq!(
        keys_missing_from_the_page(PANE_VIEWER_KEYS),
        Vec::<String>::new()
    );
    assert!(undescribed(VIEWER_KEYS).is_empty());
    assert!(undescribed(PANE_VIEWER_KEYS).is_empty());
}

#[test]
fn a_key_with_no_description_fails_the_check() {
    // A table that binds a message its listing leaves out: the check that
    // guards the viewers' tables names the key.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Probe {
        Listed,
        Forgotten,
    }
    impl HelpAction for Probe {
        const LISTED: &'static [Self] = &[Probe::Listed];
        fn help(self) -> tuikit::help::HelpEntry {
            tuikit::help::HelpEntry::new("Moving", "Scroll")
        }
    }
    const PROBE_KEYS: &[KeyRow<Probe>] = &[
        tuikit::keys::alias(tuikit::keys::ch('j'), Probe::Listed),
        tuikit::keys::alias(tuikit::keys::ch('x'), Probe::Forgotten),
    ];
    assert_eq!(keys_missing_from_the_page(PROBE_KEYS), ["x (Forgotten)"]);
    assert_eq!(undescribed(PROBE_KEYS), ["x"]);
}

#[test]
fn the_pane_page_lists_attach_and_every_message_with_a_key() {
    let theme = Theme::default();
    let page: String = help_lines(PANE_VIEWER_KEYS, &theme, 80)
        .iter()
        .map(text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(page.contains("Attach to the pane's mux"), "{page}");
    for msg in PaneViewerMsg::ALL {
        assert!(
            page.contains(msg.help().description),
            "{msg:?} is missing from the help page"
        );
    }
}

#[test]
fn keys_use_the_one_key_notation() {
    let all: String = key_rows(&sections_for(VIEWER_KEYS))
        .iter()
        .map(|row| row.0.clone())
        .collect::<Vec<_>>()
        .join(" | ");
    for expected in [
        "Ctrl-D", "Ctrl-U", "PgDn", "PgUp", "Home", "End", "Esc", "↑", "↓",
    ] {
        assert!(all.contains(expected), "`{expected}` missing from {all}");
    }
    for word in ["Up", "Down", "Left", "Right"] {
        assert!(
            !all.split([' ', '|']).any(|token| token == word),
            "`{word}` should be an arrow: {all}"
        );
    }
}

#[test]
fn no_line_is_wider_than_the_page_at_any_width() {
    for width in [40usize, 50, 60, 80, 100] {
        for line in help_lines(VIEWER_KEYS, &Theme::default(), width)
            .into_iter()
            .chain(help_lines(PANE_VIEWER_KEYS, &Theme::default(), width))
        {
            let w = UnicodeWidthStr::width(text(&line).as_str());
            assert!(w <= width, "{w} > {width}: {:?}", text(&line));
        }
    }
}

#[test]
fn wrapping_keeps_every_word_of_every_description() {
    for width in [40usize, 50, 80] {
        let flat = help_lines(VIEWER_KEYS, &Theme::default(), width)
            .iter()
            .map(text)
            .collect::<Vec<_>>()
            .join(" ");
        for (_, description, _) in key_rows(&sections_for(VIEWER_KEYS)) {
            for word in description.split_whitespace() {
                assert!(flat.contains(word), "`{word}` lost at width {width}");
            }
        }
    }
}

#[test]
fn the_footer_comes_from_the_scroll_and_close_key_table() {
    let footer = tuikit::keys::footer(HELP_KEYS);
    for page in [
        help_lines(VIEWER_KEYS, &Theme::default(), usize::MAX),
        help_lines(PANE_VIEWER_KEYS, &Theme::default(), usize::MAX),
    ] {
        let page = page.iter().map(text).collect::<Vec<_>>().join("\n");
        assert!(
            page.ends_with(&footer),
            "{footer} not at the end of\n{page}"
        );
    }
}

#[test]
fn the_keys_the_footer_names_do_what_it_says_in_the_viewer() {
    use tuikit::keys::{PagerKey, resolve};
    let expect = |event: ratatui::crossterm::event::KeyEvent, viewer: ViewerMsg| {
        assert_eq!(resolve(VIEWER_KEYS, event), Some(viewer), "{event:?}");
    };
    for row in tuikit::keys::rows(HELP_KEYS) {
        let KeyRow::Bind { key, action, .. } = row else {
            continue;
        };
        let event = key.example();
        let code = event.code;
        let viewer = match action {
            PagerKey::Down => ViewerMsg::ScrollDown,
            PagerKey::Up => ViewerMsg::ScrollUp,
            PagerKey::PageDown => ViewerMsg::PageDown,
            PagerKey::PageUp => ViewerMsg::PageUp,
            PagerKey::Top => ViewerMsg::JumpToStart,
            PagerKey::Bottom => ViewerMsg::JumpToEnd,
            // `?` toggles the viewer's help; the rest close it.
            PagerKey::Close if code == KeyCode::Char('?') => ViewerMsg::ToggleHelp,
            // Esc steps back; `q` and Ctrl-C leave at once.
            PagerKey::Abort | PagerKey::Close if code == KeyCode::Esc => ViewerMsg::Close,
            PagerKey::Abort | PagerKey::Close => ViewerMsg::Quit,
            PagerKey::Copy => continue,
        };
        expect(event, viewer);
    }
}

#[test]
fn a_key_that_cannot_act_now_is_dimmed_on_the_help_page() {
    let theme = Theme::default();
    let rows = tuikit::keys::with_availability(VIEWER_KEYS, |msg| msg != ViewerMsg::ToggleThinking);
    let lines = help_lines(&rows, &theme, 100);
    let row_of = |msg: ViewerMsg| {
        let start: String = msg
            .help()
            .description
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        lines
            .iter()
            .find(|line| text(line).contains(&start))
            .unwrap_or_else(|| panic!("no row for {msg:?}"))
            .clone()
    };
    let dim = tuikit::keys::unavailable_style(&theme);
    let off = row_of(ViewerMsg::ToggleThinking);
    for span in off
        .spans
        .iter()
        .filter(|span| !span.content.trim().is_empty())
    {
        assert_eq!(span.style.fg, dim.fg, "{:?}", span.content);
        assert!(span.style.add_modifier.contains(theme.muted));
    }
    let on = row_of(ViewerMsg::ScrollDown);
    assert!(
        on.spans
            .iter()
            .any(|span| span.style.fg == Some(theme.accent)),
        "an available row keeps its accent key column"
    );
}

/// How far the page can scroll in `area`, as its render reports it.
fn max_scroll(area: Rect, theme: &Theme) -> usize {
    render(
        VIEWER_KEYS,
        area,
        &mut Buffer::empty(area),
        theme,
        &HelpScroll::new(),
    )
    .last_offset()
}

#[test]
fn a_short_terminal_scrolls_and_clamps() {
    let theme = Theme::default();
    let tall = Rect::new(0, 0, 100, 60);
    let short = Rect::new(0, 0, 60, 12);
    assert_eq!(max_scroll(tall, &theme), 0, "everything fits when tall");
    let max = max_scroll(short, &theme);
    assert!(max > 0, "a short terminal needs to scroll");

    let top = render_text(60, 12, 0);
    let bottom = render_text(60, 12, max);
    assert_ne!(top, bottom);
    assert_eq!(render_text(60, 12, usize::MAX), bottom, "scroll is clamped");
    assert!(
        bottom.contains("Esc/q/? close"),
        "the footer is reachable:\n{bottom}"
    );
}

#[test]
fn draw_clamps_the_viewers_scroll_position() {
    let backend = ratatui::backend::TestBackend::new(60, 12);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut state =
        crate::viewer::state::ViewerState::new(crate::viewer::model::TranscriptDocument::default());
    state.show_help = true;
    state.help_scroll.to_bottom();
    terminal
        .draw(|frame| {
            let area = frame.area();
            draw(
                VIEWER_KEYS,
                &mut state.help_scroll,
                &Theme::default(),
                frame.buffer_mut(),
                area,
            );
        })
        .unwrap();
    assert_eq!(
        state.help_scroll.offset(),
        max_scroll(Rect::new(0, 0, 60, 12), &Theme::default())
    );
    // The page keys move by the help's own rows: the modal's 8 inner
    // rows less its one-row footer, not the transcript's viewport.
    assert_eq!(state.help_scroll.page(), 7);
}

#[test]
fn both_viewers_draw_their_page_through_the_one_page_size() {
    // The pane viewer's page is the transcript viewer's shape: one frame
    // at the reader's size, whichever table generated it.
    let area = Rect::new(0, 0, 80, 24);
    let rect = HelpPage::SIZE.rect(area, 0);
    let mut pane = Buffer::empty(area);
    render(
        PANE_VIEWER_KEYS,
        area,
        &mut pane,
        &Theme::default(),
        &HelpScroll::new(),
    );
    assert_eq!(pane[(rect.x, rect.y)].symbol(), "┌");
    assert_eq!(
        pane[(rect.x + rect.width - 1, rect.y + rect.height - 1)].symbol(),
        "┘"
    );
}

#[test]
fn page_at_120_columns() {
    insta::assert_snapshot!(render_text(120, 30, 0));
}

#[test]
fn page_at_80_columns() {
    insta::assert_snapshot!(render_text(80, 24, 0));
}

#[test]
fn page_at_a_narrow_width_scrolled() {
    insta::assert_snapshot!(render_text(50, 14, 6));
}
