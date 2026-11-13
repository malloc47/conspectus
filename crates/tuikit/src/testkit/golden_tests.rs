use std::cell::RefCell;
use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::*;
use crate::testkit::buffer;
use crate::testkit::render::render_widget;

/// A two-row picker: a title, a highlighted choice, and a plain one.
fn picker(title: Color, highlight: Style) -> Buffer {
    let lines = vec![
        Line::from(vec![
            Span::styled("Pick", Style::new().fg(title).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled("one", Style::new().fg(title)),
        ]),
        Line::from(Span::styled("> first", highlight)),
        Line::from("  second"),
    ];
    render_widget(Paragraph::new(lines), 12, 3)
}

fn standard() -> Buffer {
    picker(
        Color::Yellow,
        Style::new().fg(Color::Black).bg(Color::White),
    )
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/testkit/goldens")
        .join(name)
}

#[test]
fn the_form_keeps_text_styles_and_a_legend() {
    assert_eq!(
        styled(&standard()),
        "frame 12x3
text  |Pick one    |
style |aaaa bbb    |
text  |> first     |
style |ccccccc     |
text  |  second    |
style |            |
legend
  a  fg Yellow, bold
  b  fg Yellow
  c  fg Black, bg White
"
    );
}

#[test]
fn a_change_of_color_alone_fails_where_a_text_golden_passes() {
    let recolored = picker(Color::Red, Style::new().fg(Color::Black).bg(Color::White));
    assert_eq!(buffer::text(&standard()), buffer::text(&recolored));
    let mismatch = compare(&styled(&standard()), &styled(&recolored)).unwrap_err();
    let report = mismatch.to_string();
    assert!(
        report.contains("- ") && report.contains("a  fg Red, bold"),
        "{report}"
    );
    // Only the legend changed: the keys stay where they were.
    assert!(!report.contains("style |"), "{report}");
}

#[test]
fn a_lost_modifier_or_a_moved_highlight_fails_too() {
    let mut lost = standard();
    lost.set_style(
        Rect::new(0, 0, 4, 1),
        Style::new().remove_modifier(Modifier::BOLD),
    );
    assert!(compare(&styled(&standard()), &styled(&lost)).is_err());

    let mut moved = standard();
    moved.set_style(Rect::new(0, 1, 12, 1), Style::reset());
    moved.set_style(
        Rect::new(0, 2, 8, 1),
        Style::new().fg(Color::Black).bg(Color::White),
    );
    assert!(compare(&styled(&standard()), &styled(&moved)).is_err());
}

#[test]
fn an_unstyled_cell_and_a_reset_one_read_alike() {
    let mut reset = standard();
    reset.set_style(Rect::new(8, 2, 4, 1), Style::reset());
    assert_eq!(styled(&standard()), styled(&reset));
}

#[test]
fn a_wide_character_is_written_once_over_the_cell_it_covers() {
    let wide = Span::styled("\u{6771}x", Style::new().fg(Color::Cyan));
    let frame = render_widget(Paragraph::new(wide), 4, 1);
    let form = styled(&frame);
    assert!(form.contains("text  |\u{6771}x |"), "{form}");
    assert!(form.contains("style |a a |"), "{form}");
}

#[test]
fn a_region_is_written_alone() {
    let form = styled_region(&standard(), Rect::new(0, 1, 7, 1));
    assert!(form.starts_with("frame 7x1\ntext  |> first|\n"), "{form}");
}

#[test]
fn compare_ignores_how_the_last_line_ends() {
    assert!(compare("frame 1x1\n", "frame 1x1").is_ok());
    assert!(compare("frame 1x1", "frame 2x1").is_err());
}

#[test]
fn the_golden_file_matches() {
    assert_golden(&golden_path("picker.golden"), &standard(), |path, text| {
        std::fs::write(path, text)
    });
}

#[test]
fn a_missing_or_different_golden_says_how_to_accept() {
    let missing = check(&golden_path("absent.golden"), "x", false, |_, _| Ok(()));
    assert!(missing.unwrap_err().contains(ACCEPT_VAR));
    let different = check(
        &golden_path("picker.golden"),
        "frame 1x1\n",
        false,
        |_, _| Ok(()),
    );
    let report = different.unwrap_err();
    assert!(
        report.contains("picker.golden") && report.contains(ACCEPT_VAR),
        "{report}"
    );
}

#[test]
fn accepting_hands_the_frame_to_the_writer_and_reads_nothing() {
    let written = RefCell::new(None);
    let accepted = check(
        &golden_path("absent.golden"),
        "new text",
        true,
        |path, text| {
            *written.borrow_mut() = Some((path.to_path_buf(), text.to_string()));
            Ok(())
        },
    );
    assert_eq!(accepted, Ok(()));
    let (path, text) = written.into_inner().expect("written");
    assert!(path.ends_with("absent.golden"));
    assert_eq!(text, "new text");
    let failed = check(&golden_path("absent.golden"), "x", true, |_, _| {
        Err(std::io::Error::other("read-only"))
    });
    assert!(failed.unwrap_err().contains("read-only"));
}
