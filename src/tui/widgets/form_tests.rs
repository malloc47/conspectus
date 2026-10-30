//! The framework's form rows as Conspectus draws them: with its theme
//! (the default look, and every form token a `[tui.theme]` override
//! reaches), its directory fields, its harnesses' launch options, and its
//! form key tables.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tuikit::form::{
    Cols, Tone, check_row, chips, choice_row, one_of_row, path_cells, path_row, read_only_row,
    text_cells, text_row,
};
use tuikit::frame::ModalFooter;
use tuikit::glyphs::marks;
use tuikit::keys::Footer;
use tuikit::text_edit::LineEdit;

use crate::tui::Theme;
use crate::tui::theme::OverrideValue::Text as OverrideText;
use crate::tui::widgets::launch_argv;

fn text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

fn theme() -> Theme {
    Theme::default()
}

/// A theme whose every form token differs from the default.
fn custom() -> Theme {
    let mut theme = Theme::default();
    let problems = theme.apply_overrides([
        ("field", OverrideText("green")),
        ("field_active", OverrideText("black on yellow")),
        ("field_cursor", OverrideText("white on red")),
        ("accent", OverrideText("magenta")),
        ("muted", OverrideText("italic")),
        ("inert", OverrideText("blue")),
        ("success", OverrideText("bright_cyan")),
        ("warning", OverrideText("bright_red")),
    ]);
    assert_eq!(problems, []);
    theme
}

/// A line holding `value` with the cursor after `cursor` characters.
fn edit(value: &str, cursor: usize) -> LineEdit {
    LineEdit::new(value).with_cursor(cursor)
}

const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz";

#[test]
fn a_long_value_scrolls_around_the_cursor_with_edge_marks() {
    let spans = text_cells(&edit(ALPHABET, 20), 12, true, &theme());
    let cells: String = spans.iter().map(|span| span.content.as_ref()).collect();
    assert!(
        cells.starts_with("< "),
        "the start is out of view: {cells:?}"
    );
    assert!(cells.ends_with(" >"), "the end is out of view: {cells:?}");
    assert_eq!(style_of(&spans, "u"), theme().field_cursor.into_style());
    let spans = text_cells(&edit(ALPHABET, 0), 12, true, &theme());
    let cells: String = spans.iter().map(|span| span.content.as_ref()).collect();
    assert_eq!(cells, "  abcdefgh >");
}

#[test]
fn wide_characters_fill_the_field_by_their_cells() {
    // Twelve cells of field: the marks take four, so four two-cell
    // characters and the caret do not fit, and the window scrolls.
    let spans = text_cells(&edit("東京タワーと富士山", 9), 12, true, &theme());
    assert_eq!(tuikit::text::spans_width(&spans), 12);
    let cells: String = spans.iter().map(|span| span.content.as_ref()).collect();
    assert_eq!(cells, "< 富士山    ", "the cursor rides the right edge");
    let spans = text_cells(&edit("東京タワーと富士山", 0), 12, true, &theme());
    let cells: String = spans.iter().map(|span| span.content.as_ref()).collect();
    assert_eq!(cells, "  東京タワ >");
}

#[test]
fn an_empty_value_shows_a_dash() {
    let line = text_row("cwd", &edit("", 0), false, Cols::new(40), &theme());
    assert!(text(&line).contains('-'), "{}", text(&line));
}

#[test]
fn the_label_column_widens_for_long_labels() {
    let narrow = text_row("cwd", &edit("x", 0), false, Cols::new(60), &theme());
    let wide = text_row(
        "worktree_branch",
        &edit("x", 0),
        false,
        Cols::new(60).with_label(15),
        &theme(),
    );
    let at = |line: &Line<'_>| text(line).find('x').expect("value");
    assert_eq!(at(&narrow), 2 + 11 + 2 + 2, "marker, label, gap, edge");
    assert_eq!(at(&wide), 2 + 15 + 2 + 2);
}

#[test]
fn a_path_row_shows_the_tab_hint_only_while_focused_with_a_completion() {
    let theme = theme();
    let here = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(here.path().join("alpha")).expect("mkdir");
    let typed = here.path().join("al").display().to_string();
    let field = crate::tui::paths::directory_field(typed, Vec::new());
    let cols = Cols::new(80);
    let focused = text(&path_row("cwd", &field, true, cols, &theme));
    assert!(focused.contains(" Tab: pha"), "{focused:?}");
    let idle = text(&path_row("cwd", &field, false, cols, &theme));
    assert!(!idle.contains("Tab:"), "{idle:?}");
    assert_eq!(
        tuikit::text::width(&focused),
        tuikit::text::width(&idle),
        "the row keeps its width with or without the hint"
    );
}

#[test]
fn the_harness_row_marks_the_selected_chip_and_flags_a_custom_value() {
    let known: Vec<String> = ["claude-code", "codex"].map(String::from).to_vec();
    let line = choice_row(
        "harness",
        &LineEdit::new("codex"),
        &known,
        false,
        Cols::new(70),
        &theme(),
    );
    let rendered = text(&line);
    assert!(
        rendered.contains("[claude-code]") && rendered.contains("[codex]"),
        "{rendered}"
    );
    let chip = line
        .spans
        .iter()
        .find(|s| s.content.contains("[codex]"))
        .unwrap();
    assert!(
        chip.style.add_modifier.contains(Modifier::BOLD),
        "selected chip is bold"
    );

    let custom = choice_row(
        "harness",
        &LineEdit::new("my-agent"),
        &known,
        false,
        Cols::new(70),
        &theme(),
    );
    assert!(text(&custom).contains("[custom]"));
}

/// A harness's launch options as the forms draw them: checkboxes,
/// checked when `argv` carries their flags.
fn options_row(harness: &str, argv: &[String]) -> Line<'static> {
    let options = crate::discovery::harness::launch_options_for(harness);
    let items: Vec<(String, bool)> = options
        .iter()
        .zip(launch_argv::options_on(options, argv))
        .map(|(option, on)| (option.label.to_string(), on))
        .collect();
    check_row("options", &items, None, false, Cols::new(60), &theme())
}

#[test]
fn launch_options_show_checked_when_the_argv_carries_the_flags() {
    let off = options_row("claude-code", &["claude".into()]);
    assert!(
        text(&off).contains("[ ] skip permissions"),
        "{}",
        text(&off)
    );
    let on = options_row(
        "claude-code",
        &["claude".into(), "--dangerously-skip-permissions".into()],
    );
    assert!(text(&on).contains("[x] skip permissions"), "{}", text(&on));
}

#[test]
fn the_footer_wraps_at_separators_without_breaking_a_hint() {
    let keys = crate::tui::keybindings::overlay::MUX_LAUNCH_KEYS;
    let footer = tuikit::keys::footer(keys);
    let theme = theme();
    let lines = ModalFooter::new(Footer::new(keys), &theme).keys(40);
    assert!(lines.len() >= 2);
    for line in &lines {
        assert!(tuikit::text::width(&text(line)) <= 40, "{}", text(line));
    }
    let joined: Vec<String> = lines.iter().map(text).collect();
    assert_eq!(joined.join(" · "), footer, "nothing lost or reordered");
}

#[test]
fn a_one_of_n_row_draws_choices_not_checkboxes() {
    let options = ["don't pin", "launch, then pin"].map(String::from);
    let line = one_of_row("pin", &options, 1, false, Cols::new(70), &theme());
    let rendered = text(&line);
    assert!(
        rendered.contains(&format!(
            "{} don't pin {} launch, then pin",
            marks::NOT_CHOSEN,
            marks::CHOSEN
        )),
        "{rendered}"
    );
    assert!(!rendered.contains(marks::CHECKED) && !rendered.contains(marks::UNCHECKED));
}

/// The style of the first span whose text contains `needle`.
fn style_of(spans: &[Span<'_>], needle: &str) -> Style {
    spans
        .iter()
        .find(|span| span.content.contains(needle))
        .unwrap_or_else(|| panic!("no span with {needle:?}"))
        .style
}

#[test]
fn the_default_theme_draws_the_form_as_it_always_has() {
    let spans = text_cells(&edit(ALPHABET, 20), 12, true, &theme());
    // The scroll marks, the fill, and the cursor cell keep the fixed
    // look the forms had before they took the theme.
    assert_eq!(
        style_of(&spans, "<"),
        Style::default()
            .fg(Color::Cyan)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    );
    assert_eq!(
        style_of(&spans, "q"),
        Style::default().fg(Color::White).bg(Color::DarkGray)
    );
    assert_eq!(
        style_of(&spans, "u"),
        Style::default().fg(Color::Black).bg(Color::White)
    );
    let idle = text_cells(&edit("abc", 0), 12, false, &theme());
    assert_eq!(style_of(&idle, "bc"), Style::default().fg(Color::Gray));
}

#[test]
fn a_custom_theme_restyles_the_field_its_marks_and_its_cursor() {
    let theme = custom();
    let spans = text_cells(&edit(ALPHABET, 20), 12, true, &theme);
    assert_eq!(
        style_of(&spans, "<"),
        Style::default()
            .fg(Color::Magenta)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    );
    assert_eq!(
        style_of(&spans, "q"),
        Style::default().fg(Color::Black).bg(Color::Yellow)
    );
    assert_eq!(
        style_of(&spans, "u"),
        Style::default().fg(Color::White).bg(Color::Red)
    );
    let idle = text_cells(&edit("abc", 0), 12, false, &theme);
    assert_eq!(style_of(&idle, "bc"), Style::default().fg(Color::Green));
}

#[test]
fn chips_notes_footers_and_warnings_follow_the_theme() {
    let theme = custom();
    let known: Vec<String> = ["claude-code", "codex"].map(String::from).to_vec();
    let chips = chips("codex", &known, 40, &theme);
    assert_eq!(
        style_of(&chips, "[codex]"),
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    );
    assert_eq!(
        style_of(&chips, "[claude-code]"),
        Style::default().add_modifier(Modifier::ITALIC)
    );

    let note = read_only_row("pin", "a note", Tone::Note, Cols::new(40), &theme);
    assert_eq!(
        style_of(&note.spans, "a note"),
        Style::default().add_modifier(Modifier::ITALIC)
    );
    let footer = ModalFooter::new(
        Footer::new(crate::tui::keybindings::overlay::PIN_EDIT_KEYS),
        &theme,
    )
    .keys(40);
    assert_eq!(
        footer[0].spans[0].style,
        Style::default().add_modifier(Modifier::ITALIC)
    );
    let derived = read_only_row("command", "nothing", Tone::Warning, Cols::new(40), &theme);
    assert_eq!(
        style_of(&derived.spans, "nothing"),
        Style::default()
            .fg(Color::LightRed)
            .add_modifier(Modifier::ITALIC)
    );
    let custom_harness = choice_row(
        "harness",
        &LineEdit::new("my-agent"),
        &known,
        false,
        Cols::new(70),
        &theme,
    );
    assert_eq!(
        style_of(&custom_harness.spans, "[custom]"),
        Style::default().fg(Color::LightRed)
    );
}

#[test]
fn path_marks_follow_the_theme() {
    let theme = custom();
    let mark_style = |value: String| {
        let field = crate::tui::paths::directory_field(value, Vec::new());
        let mark = field.validity().mark();
        path_cells(&field, 30, false, &theme)
            .into_iter()
            .find(|span| span.content == mark)
            .expect("the mark is drawn")
            .style
    };
    assert_eq!(mark_style(String::new()), Style::default().fg(Color::Blue));
    assert_eq!(
        mark_style("/no/such/place/for/this/test".to_string()),
        Style::default().fg(Color::LightRed)
    );
    let here = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        mark_style(here.path().display().to_string()),
        Style::default().fg(Color::LightCyan)
    );
}

#[test]
fn the_focused_row_carries_the_cursor_marker_and_highlight_and_its_value_keeps_its_fill() {
    let theme = theme();
    let cols = Cols::new(60);
    let focused = text_row("name", &edit("xyz", 1), true, cols, &theme);
    let idle = text_row("name", &edit("xyz", 1), false, cols, &theme);
    assert!(
        text(&focused).starts_with(&format!("{} name", marks::POINTER)),
        "{}",
        text(&focused)
    );
    assert!(text(&idle).starts_with("  name"), "{}", text(&idle));
    assert_eq!(
        tuikit::text::width(&text(&focused)),
        tuikit::text::width(&text(&idle)),
        "the marker moves no text"
    );
    let label = style_of(&focused.spans, "name");
    assert!(label.add_modifier.contains(theme.selection_active));
    assert!(style_of(&idle.spans, "name").add_modifier.is_empty());
    assert_eq!(
        style_of(&focused.spans, "x"),
        theme.field_active.into_style(),
        "the field's fill keeps its look inside the highlight"
    );
    assert_eq!(
        style_of(&focused.spans, "y"),
        theme.field_cursor.into_style()
    );
}
