use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::tui::keybindings::overlay::HELP_KEYS;

fn render_text(width: u16, height: u16, scroll: usize) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    render(area, &mut buf, &Theme::default(), scroll);
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

#[test]
fn every_key_the_viewer_binds_is_listed_exactly_once() {
    let listed: Vec<String> = sections_for(VIEWER_KEYS)
        .iter()
        .flat_map(|s| s.rows.iter())
        .flat_map(|r| r.keys.split(" / ").map(str::to_string).collect::<Vec<_>>())
        .collect();
    for row in VIEWER_KEYS {
        let KeyRow::Bind { key, .. } = row else {
            continue;
        };
        let label = tuikit::keys::key_label(key);
        let count = listed.iter().filter(|k| **k == label).count();
        assert_eq!(count, 1, "`{label}` should be listed exactly once");
    }
}

#[test]
fn every_message_with_a_key_has_a_row_and_a_description() {
    let sections = sections_for(VIEWER_KEYS);
    for msg in ViewerMsg::ALL {
        let bound = VIEWER_KEYS
            .iter()
            .any(|row| matches!(row, KeyRow::Bind { action, .. } if *action == msg));
        let described = sections
            .iter()
            .flat_map(|s| s.rows.iter())
            .any(|r| r.description == msg.help().description);
        assert_eq!(bound, described, "{msg:?}");
    }
}

#[test]
fn a_message_with_no_key_is_left_out() {
    let table = [KeyRow::Bind {
        key: tuikit::keys::KeyMatcher::Exact {
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
            code: ratatui::crossterm::event::KeyCode::Char('j'),
        },
        action: ViewerMsg::ScrollDown,
        hint: tuikit::keys::Hint::Alias,
        available: true,
    }];
    let sections = sections_for(&table);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].rows.len(), 1);
    assert_eq!(sections[0].rows[0].keys, "j");
}

#[test]
fn keys_use_the_keymap_help_notation() {
    let all: String = sections_for(VIEWER_KEYS)
        .iter()
        .flat_map(|s| s.rows.iter().map(|r| r.keys.clone()))
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
        for line in help_lines(&Theme::default(), Some(width)) {
            let w = UnicodeWidthStr::width(text(&line).as_str());
            assert!(w <= width, "{w} > {width}: {:?}", text(&line));
        }
    }
}

#[test]
fn wrapping_keeps_every_word_of_every_description() {
    for width in [40usize, 50, 80] {
        let page: Vec<String> = help_lines(&Theme::default(), Some(width))
            .iter()
            .map(text)
            .collect();
        let flat = page.join(" ");
        for section in sections_for(VIEWER_KEYS) {
            for row in section.rows {
                for word in row.description.split_whitespace() {
                    assert!(flat.contains(word), "`{word}` lost at width {width}");
                }
            }
        }
    }
}

#[test]
fn the_footer_comes_from_the_scroll_and_close_key_table() {
    let footer = tuikit::keys::footer(HELP_KEYS);
    let page = help_lines(&Theme::default(), None)
        .iter()
        .map(text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(page.contains(&footer), "{footer} not in\n{page}");
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
            PagerKey::Close if code == ratatui::crossterm::event::KeyCode::Char('?') => {
                ViewerMsg::ToggleHelp
            }
            // Esc steps back; `q` and Ctrl-C leave at once.
            PagerKey::Abort | PagerKey::Close
                if code == ratatui::crossterm::event::KeyCode::Esc =>
            {
                ViewerMsg::Close
            }
            PagerKey::Abort | PagerKey::Close => ViewerMsg::Quit,
            PagerKey::Copy => continue,
        };
        expect(event, viewer);
    }
}

/// How far the page can scroll in `area`, as its render reports it.
fn max_scroll(area: Rect, theme: &Theme) -> usize {
    render(area, &mut Buffer::empty(area), theme, 0).last_offset()
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
    state.help_scroll = usize::MAX;
    terminal
        .draw(|frame| {
            let area = frame.area();
            draw(&mut state, &Theme::default(), frame, area);
        })
        .unwrap();
    assert_eq!(
        state.help_scroll,
        max_scroll(Rect::new(0, 0, 60, 12), &Theme::default())
    );
    // The page keys move by the help's own rows: the modal's 8 inner
    // rows less its one-row footer, not the transcript's viewport.
    assert_eq!(state.help_page, 7);
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
