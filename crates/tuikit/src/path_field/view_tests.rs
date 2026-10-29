use std::path::{Path, PathBuf};
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;

use super::*;
use crate::glyphs::marks;
use crate::path_field::{Candidate, PathKind, Validity};
use crate::testkit::golden::assert_golden;
use crate::testkit::paths::MemoryPaths;

fn paths() -> Arc<MemoryPaths> {
    Arc::new(
        MemoryPaths::new()
            .with_home("/home/me")
            .dir("/home/me/projects/alpha")
            .file("/home/me/notes.txt"),
    )
}

fn field(value: &str) -> PathField {
    PathField::new(value, PathKind::Dir, paths())
}

fn text(spans: &[Span<'_>]) -> String {
    spans.iter().map(|span| span.content.as_ref()).collect()
}

/// Where the mark sits: the cells before it.
fn mark_column(spans: &[Span<'_>], mark: &str) -> usize {
    let at = spans
        .iter()
        .position(|span| span.content == mark)
        .expect("the mark is drawn");
    text::spans_width(&spans[..at])
}

#[test]
fn the_row_takes_exactly_its_width() {
    let theme = Theme::default();
    for value in ["", "~/pro", "/home/me/projects", "/nowhere"] {
        let field = field(value);
        for focused in [true, false] {
            for width in 0..90 {
                let spans = field
                    .view()
                    .focused(focused)
                    .marks(true)
                    .spans(width, &theme);
                assert_eq!(
                    text::spans_width(&spans),
                    width,
                    "{value:?} focused={focused} width={width}"
                );
            }
        }
    }
}

#[test]
fn a_focused_field_with_a_completion_shows_what_tab_adds() {
    let theme = Theme::default();
    let field = field("~/pro");
    let spans = field.view().spans(60, &theme);
    let shown = text(&spans);
    assert!(shown.contains(" Tab: jects"), "{shown:?}");
    let hint = spans
        .iter()
        .find(|span| span.content.contains("Tab:"))
        .expect("a hint");
    assert_eq!(hint.style, Style::default().fg(theme.accent));
    assert_eq!(
        mark_column(&spans, marks::WARNING),
        60 - 2 - 30 + 1,
        "the hint takes 30 cells after the mark"
    );
}

#[test]
fn the_hint_and_its_cells_go_while_the_field_is_unfocused_or_has_nothing_to_add() {
    let theme = Theme::default();
    let unfocused = field("~/pro");
    let spans = unfocused.view().focused(false).spans(60, &theme);
    assert!(!text(&spans).contains("Tab:"));
    assert_eq!(mark_column(&spans, marks::WARNING), 60 - 2 + 1);

    let complete = field("/home/me/projects/alpha");
    assert!(complete.completion().is_none());
    let spans = complete.view().spans(60, &theme);
    assert!(!text(&spans).contains("Tab:"));
    assert_eq!(mark_column(&spans, marks::OK), 60 - 2 + 1);
}

#[test]
fn the_hint_takes_at_most_half_the_row_and_none_when_too_narrow() {
    let theme = Theme::default();
    let field = field("~/pro");
    let spans = field.view().spans(24, &theme);
    assert!(text(&spans).contains(" Tab: jec"), "{:?}", text(&spans));
    assert_eq!(mark_column(&spans, marks::WARNING), 24 - 2 - 12 + 1);
    let spans = field.view().spans(15, &theme);
    assert!(!text(&spans).contains("Tab"), "{:?}", text(&spans));
}

#[test]
fn a_long_remainder_is_cut_in_the_middle() {
    let theme = Theme::default();
    let field = field("/srv/").with_candidates(vec![Candidate::new(
        "/srv/a-rather-long-directory-name/with/its/last-part",
        1,
    )]);
    let shown = text(&field.view().spans(80, &theme));
    let hint = shown.split("Tab: ").nth(1).expect("a hint").trim_end();
    assert!(hint.starts_with("a-rather"), "{hint:?}");
    assert!(hint.ends_with("last-part"), "{hint:?}");
    assert!(hint.contains(marks::ELLIPSIS), "{hint:?}");
}

#[test]
fn the_mark_says_what_the_last_look_found() {
    let theme = Theme::default();
    for (value, validity) in [
        ("", Validity::Unknown),
        ("~/projects", Validity::Exists),
        ("~/notes.txt", Validity::Advisory),
        ("/nowhere", Validity::Advisory),
    ] {
        let field = field(value);
        assert_eq!(field.validity(), validity);
        let spans = field.view().focused(false).spans(30, &theme);
        let mark = spans
            .iter()
            .find(|span| span.content == validity.mark())
            .expect("the mark");
        assert_eq!(mark.style, validity.style(&theme), "{value:?}");
    }
}

#[test]
fn a_row_too_narrow_for_the_mark_is_the_text_alone() {
    let theme = Theme::default();
    let field = field("/nowhere");
    let spans = field.view().spans(2, &theme);
    assert!(!text(&spans).contains(marks::WARNING));
    assert_eq!(text::spans_width(&spans), 2);
}

#[test]
fn the_hint_names_the_key_its_table_binds() {
    assert_eq!(complete_label(), "Tab");
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/path_field/goldens")
        .join(name)
}

#[test]
fn the_field_looks_as_its_golden_says() {
    let theme = Theme::default();
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
    let rows = [
        (field("~/pro"), true),
        (field("~/pro"), false),
        (field("~/projects"), true),
        (field("~/notes.txt"), true),
        (field(""), false),
    ];
    for (y, (field, focused)) in (0u16..).zip(&rows) {
        let line = field
            .view()
            .focused(*focused)
            .marks(true)
            .placeholder("-")
            .style(theme.field.into_style())
            .line(40, &theme);
        buf.set_line(0, y, &line, 40);
    }
    assert_golden(&golden_path("view.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
}
