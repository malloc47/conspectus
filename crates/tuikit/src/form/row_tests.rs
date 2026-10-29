use std::sync::Arc;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::*;
use crate::keys::{FORM_KEYS, FormKey, include, key, shown};
use crate::path_field::PathKind;
use crate::testkit::paths::MemoryPaths;

fn theme() -> Theme {
    Theme::default()
}

fn text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn spans_text(spans: &[Span<'_>]) -> String {
    spans.iter().map(|span| span.content.as_ref()).collect()
}

/// The style of the first span whose text contains `needle`.
fn style_of(line: &Line<'_>, needle: &str) -> Style {
    line.spans
        .iter()
        .find(|span| span.content.contains(needle))
        .unwrap_or_else(|| panic!("no span holds {needle:?} in {:?}", text(line)))
        .style
}

/// The cell column `needle` starts in.
fn column(line: &Line<'_>, needle: &str) -> usize {
    let drawn = text(line);
    let byte = drawn.find(needle).expect("needle on the line");
    text::width(&drawn[..byte])
}

fn options(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn the_label_column_fits_the_longest_label_within_its_bounds() {
    assert_eq!(Cols::for_labels(60, ["a", "bb"]).label, LABEL_MIN);
    assert_eq!(Cols::for_labels(60, ["fourteen cells"]).label, 14);
    assert_eq!(
        Cols::for_labels(60, ["a label far wider than the column"]).label,
        LABEL_MAX
    );
    assert_eq!(Cols::for_labels(60, Vec::<&str>::new()).label, LABEL_MIN);
    let cols = Cols::new(40).with_label(13);
    assert_eq!(cols.lead(), 2 + 13 + LABEL_GAP);
    assert_eq!(cols.value(), 40 - 17);
    assert_eq!(Cols::new(5).value(), 1, "a value always gets a cell");
}

#[test]
fn a_label_wider_than_its_column_is_cut_not_pushed() {
    assert_eq!(label("name", 6), "name    ");
    assert_eq!(label("directory", 6), "direc…  ");
    assert_eq!(text::width(&label("directory", 6)), 6 + LABEL_GAP);
}

#[test]
fn every_row_starts_its_value_in_one_column() {
    let theme = theme();
    let cols = Cols::new(60);
    let edit = LineEdit::new("draft");
    let path = PathField::new(
        "/srv",
        PathKind::Dir,
        Arc::new(MemoryPaths::new().dir("/srv")),
    );
    let text_line = text_row("name", &edit, false, cols, &theme);
    let path_line = path_row("where", &path, false, cols, &theme);
    let toggle = toggle_row("copy", true, "keep a copy", false, cols, &theme);
    let note = read_only_row("about", "a note", Tone::Note, cols, &theme);
    let derived = read_only_row("result", "an output", Tone::Derived, cols, &theme);
    // A field's text starts after its 2-cell scroll-mark slot; every
    // other row indents its text to the same column.
    let start = cols.lead() + 2;
    assert_eq!(column(&text_line, "draft"), start);
    assert_eq!(column(&path_line, "/srv"), start);
    assert_eq!(column(&toggle, marks::CHECKED), start);
    assert_eq!(column(&note, "a note"), start);
    assert_eq!(column(&derived, "an output"), start);
    assert_eq!(column(&text_line, "name"), 2, "labels follow the gutter");
}

#[test]
fn a_focused_row_carries_the_cursor_and_its_fill_keeps_its_look() {
    let theme = theme();
    let cols = Cols::new(40);
    let edit = LineEdit::new("draft");
    let focused = text_row("name", &edit, true, cols, &theme);
    let unfocused = text_row("name", &edit, false, cols, &theme);
    assert!(text(&focused).starts_with(marks::POINTER));
    assert!(text(&unfocused).starts_with("  name"));
    assert_eq!(
        style_of(&focused, "name").add_modifier,
        theme.selection_active,
        "the label takes the cursor's highlight"
    );
    assert_eq!(style_of(&focused, "draft"), theme.field_active.into_style());
    assert_eq!(style_of(&unfocused, "draft"), theme.field.into_style());
    assert_eq!(text::width(&text(&focused)), 40, "a field fills its column");
}

#[test]
fn an_empty_field_shows_the_dash() {
    let theme = theme();
    let line = text_row("name", &LineEdit::new(""), false, Cols::new(40), &theme);
    assert_eq!(column(&line, EMPTY), Cols::new(40).lead() + 2);
}

#[test]
fn a_choice_highlights_the_chosen_chip_and_marks_a_custom_value() {
    let theme = theme();
    let names = options(&["alpha", "beta", "gamma"]);
    let chosen = choice_cells(&LineEdit::new("beta"), &names, 60, false, &theme);
    let drawn = spans_text(&chosen);
    assert!(drawn.contains(" [alpha] [beta] [gamma]"), "{drawn}");
    let beta = chosen
        .iter()
        .find(|span| span.content == " [beta]")
        .expect("the chosen chip");
    assert_eq!(beta.style.fg, Some(theme.accent));
    assert!(beta.style.add_modifier.contains(Modifier::BOLD));
    let alpha = chosen
        .iter()
        .find(|span| span.content == " [alpha]")
        .expect("another chip");
    assert_eq!(alpha.style.add_modifier, theme.muted);
    assert_eq!(text::spans_width(&chosen[..chosen.len() - 3]), CHOICE_SLOT);

    let custom = choice_cells(&LineEdit::new("delta"), &names, 40, false, &theme);
    let mark = custom.last().expect("the custom mark");
    assert_eq!(mark.content, format!(" {CUSTOM}"));
    assert_eq!(mark.style.fg, Some(theme.warning));
    assert_eq!(text::spans_width(&custom), 40, "the value takes the rest");
}

#[test]
fn chips_that_do_not_fit_end_in_the_ellipsis_mark() {
    let theme = theme();
    let names = options(&["alpha", "beta", "gamma"]);
    let spans = chips("alpha", &names, 18, &theme);
    assert_eq!(
        spans_text(&spans),
        format!(" [alpha] [beta] {}", marks::ELLIPSIS)
    );
    assert!(chips("alpha", &names, 2, &theme).is_empty());
}

#[test]
fn a_one_of_row_never_looks_like_a_row_of_checkboxes() {
    let theme = theme();
    let names = options(&["off", "on", "later"]);
    let line = one_of_row("when", &names, 1, false, Cols::new(60), &theme);
    let drawn = text(&line);
    assert!(drawn.contains("( ) off (•) on ( ) later"), "{drawn}");
    assert!(
        !drawn.contains('['),
        "no checkbox in a one-of-N row: {drawn}"
    );
    assert_eq!(option("on", true), format!("{} on", marks::CHOSEN));
    assert_eq!(checkbox("on", true), "[x] on");
    assert_eq!(checkbox("on", false), "[ ] on");
}

#[test]
fn a_row_of_text_is_cut_with_the_ellipsis_mark() {
    let theme = theme();
    let cols = Cols::new(30);
    let line = static_row("label", "a value much too long to fit", false, cols, &theme);
    let drawn = text(&line);
    assert!(drawn.ends_with(marks::ELLIPSIS), "{drawn}");
    assert_eq!(text::width(&drawn), 30);
}

#[test]
fn a_check_row_marks_the_item_under_its_cursor_only_when_it_needs_one() {
    let theme = theme();
    let cols = Cols::new(60);
    let caret = theme.field_cursor.into_style();
    let items = vec![("first".to_string(), true), ("second".to_string(), false)];
    let focused = check_row("parts", &items, Some(1), true, cols, &theme);
    assert!(text(&focused).contains("[x] first [ ] second"));
    assert_eq!(style_of(&focused, "second"), caret);
    assert_eq!(style_of(&focused, "first"), theme.field_active.into_style());
    let unfocused = check_row("parts", &items, Some(1), false, cols, &theme);
    assert_eq!(style_of(&unfocused, "second"), theme.field.into_style());
    let one = vec![("only".to_string(), false)];
    let single = check_row("parts", &one, Some(0), true, cols, &theme);
    assert_eq!(
        style_of(&single, "only"),
        theme.field_active.into_style(),
        "one item needs no cursor of its own"
    );
}

#[test]
fn read_only_lines_recede_and_a_warning_takes_the_warning_color() {
    let theme = theme();
    let cols = Cols::new(60);
    let recede = Style::default().add_modifier(theme.muted | Modifier::ITALIC);
    let note = read_only_row("about", "some text", Tone::Note, cols, &theme);
    assert!(text(&note).starts_with("  about"));
    assert_eq!(style_of(&note, "about"), recede);
    assert_eq!(style_of(&note, "some text"), recede);

    let derived = read_only_row("output", "a result", Tone::Derived, cols, &theme);
    assert!(
        text(&derived).starts_with(&format!("  {} output", marks::DERIVED)),
        "{}",
        text(&derived)
    );
    assert_eq!(style_of(&derived, "a result"), recede);
    assert_eq!(
        column(&derived, "output"),
        column(&note, "about") + 2,
        "the derived label sits two cells deeper"
    );

    let warning = read_only_row("output", "nothing to run", Tone::Warning, cols, &theme);
    let style = style_of(&warning, "nothing to run");
    assert_eq!(style.fg, Some(theme.warning));
    assert!(style.add_modifier.contains(Modifier::ITALIC));
    assert!(!text(&warning).contains(marks::POINTER), "never focused");
}

const KEYS: &[crate::keys::KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    shown(
        key(ratatui::crossterm::event::KeyCode::Enter),
        FormKey::Submit,
        "Enter",
        "save",
    ),
    shown(
        key(ratatui::crossterm::event::KeyCode::Esc),
        FormKey::Cancel,
        "Esc",
        "cancel",
    ),
];

#[test]
fn the_footer_is_generated_muted_and_breaks_only_between_hints() {
    let theme = theme();
    let lines = footer_lines(KEYS, 40, &theme);
    let drawn: Vec<String> = lines.iter().map(text).collect();
    assert_eq!(
        drawn,
        vec![
            "↑/↓ move · Tab complete/next".to_string(),
            "S-Tab back · Enter save · Esc cancel".to_string(),
        ]
    );
    for line in &lines {
        assert_eq!(line.style.add_modifier, theme.muted);
    }
}

#[test]
fn the_error_line_is_in_the_error_color_and_cut_to_its_width() {
    let theme = theme();
    let line = error_line("a reason that is longer than the row", 20, &theme);
    assert_eq!(text::width(&text(&line)), 20);
    assert!(text(&line).ends_with(marks::ELLIPSIS));
    assert_eq!(line.spans[0].style.fg, Some(theme.error));
}
