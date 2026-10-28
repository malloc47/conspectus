use ratatui::style::Color;

use super::*;
use crate::tui::theme::OverrideValue::Text as OverrideText;

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

#[test]
fn a_long_value_scrolls_around_the_cursor_with_edge_marks() {
    let value = "abcdefghijklmnopqrstuvwxyz";
    let w = window(value, 20, 12, true, &theme());
    assert_eq!(w.left, "< ", "the start is out of view");
    assert_eq!(w.right, " >", "the end is out of view");
    assert_eq!(w.cursor_text, "u");
    let w = window(value, 0, 12, true, &theme());
    assert_eq!(w.left, "  ");
    assert_eq!(w.right, " >");
}

#[test]
fn an_empty_value_shows_a_dash() {
    let line = value_row("cwd", "", 0, false, Cols::new(40), None, &theme());
    assert!(text(&line).contains('-'), "{}", text(&line));
}

#[test]
fn the_label_column_widens_for_long_labels() {
    let narrow = value_row("cwd", "x", 0, false, Cols::new(60), None, &theme());
    let wide = value_row(
        "worktree_branch",
        "x",
        0,
        false,
        Cols::new(60).with_label(15),
        None,
        &theme(),
    );
    let at = |line: &Line<'_>| text(line).find('x').expect("value");
    assert_eq!(at(&narrow), 2 + 11 + 2 + 2, "marker, label, gap, edge");
    assert_eq!(at(&wide), 2 + 15 + 2 + 2);
}

#[test]
fn completion_remainder_names_what_tab_would_add() {
    assert_eq!(
        completion_remainder("/tmp/al", "/tmp/alpha"),
        Some("pha".into())
    );
    assert_eq!(completion_remainder("", "/tmp"), Some("/tmp".into()));
    assert_eq!(completion_remainder("/tmp/alpha", "/tmp/alpha"), None);
    assert_eq!(
        completion_remainder("~/sr", "/home/me/src"),
        Some("c".into()),
        "falls back to the basename"
    );
}

#[test]
fn the_harness_row_marks_the_selected_chip_and_flags_a_custom_value() {
    let known: Vec<String> = ["claude-code", "codex"].map(String::from).to_vec();
    let line = harness_row(
        "harness",
        &TextInputState::new("harness", "codex"),
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

    let custom = harness_row(
        "harness",
        &TextInputState::new("harness", "my-agent"),
        &known,
        false,
        Cols::new(70),
        &theme(),
    );
    assert!(text(&custom).contains("[custom]"));
}

#[test]
fn launch_options_show_checked_when_the_argv_carries_the_flags() {
    let options = crate::discovery::harness::launch_options_for("claude-code");
    let off = options_row(
        "options",
        options,
        &["claude".into()],
        false,
        Cols::new(60),
        &theme(),
    );
    assert!(
        text(&off).contains("[ ] skip permissions"),
        "{}",
        text(&off)
    );
    let on = options_row(
        "options",
        options,
        &["claude".into(), "--dangerously-skip-permissions".into()],
        false,
        Cols::new(60),
        &theme(),
    );
    assert!(text(&on).contains("[x] skip permissions"), "{}", text(&on));
}

#[test]
fn the_footer_wraps_at_separators_without_breaking_a_hint() {
    let footer =
        "↑/↓ move · Tab complete/next · S-Tab back · ←/→/Space option · Enter launch · Esc cancel";
    let lines = footer_lines(footer, 40, &theme());
    assert!(lines.len() >= 2);
    for line in &lines {
        assert!(text(line).chars().count() <= 40, "{}", text(line));
    }
    let joined: Vec<String> = lines.iter().map(text).collect();
    assert_eq!(joined.join(" · "), footer, "nothing lost or reordered");
}

#[test]
fn display_argv_quotes_what_a_shell_would() {
    assert_eq!(
        display_argv(&["claude".into(), "--model".into(), "opus 4".into()]),
        "claude --model 'opus 4'"
    );
    assert_eq!(display_argv(&["a".into(), String::new()]), "a ''");
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
    let spans = value_cells("abcdefghijklmnopqrstuvwxyz", 20, 12, true, &theme());
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
    let idle = value_cells("abc", 0, 12, false, &theme());
    assert_eq!(style_of(&idle, "bc"), Style::default().fg(Color::Gray));
}

#[test]
fn a_custom_theme_restyles_the_field_its_marks_and_its_cursor() {
    let theme = custom();
    let spans = value_cells("abcdefghijklmnopqrstuvwxyz", 20, 12, true, &theme);
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
    let idle = value_cells("abc", 0, 12, false, &theme);
    assert_eq!(style_of(&idle, "bc"), Style::default().fg(Color::Green));
}

#[test]
fn chips_notes_footers_and_warnings_follow_the_theme() {
    let theme = custom();
    let known: Vec<String> = ["claude-code", "codex"].map(String::from).to_vec();
    let chips = harness_chips("codex", &known, 40, &theme);
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

    let note = note_row("pin", "a note", Cols::new(40), &theme);
    assert_eq!(
        style_of(&note.spans, "a note"),
        Style::default().add_modifier(Modifier::ITALIC)
    );
    let footer = footer_lines("Enter save · Esc cancel", 40, &theme);
    assert_eq!(
        footer[0].style,
        Style::default().add_modifier(Modifier::ITALIC)
    );
    let derived = derived_row("command", "nothing", true, Cols::new(40), &theme);
    assert_eq!(
        style_of(&derived.spans, "nothing"),
        Style::default()
            .fg(Color::LightRed)
            .add_modifier(Modifier::ITALIC)
    );
    let custom_harness = harness_row(
        "harness",
        &TextInputState::new("harness", "my-agent"),
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
    let empty = PathOmniboxState::new("cwd", "");
    assert_eq!(
        path_mark(&empty, &theme)[1].style,
        Style::default().fg(Color::Blue)
    );
    let missing = PathOmniboxState::new("cwd", "/no/such/place/for/this/test");
    assert_eq!(
        path_mark(&missing, &theme)[1].style,
        Style::default().fg(Color::LightRed)
    );
    let here = tempfile::tempdir().expect("temp dir");
    let exists = PathOmniboxState::new("cwd", here.path().display().to_string());
    assert_eq!(
        path_mark(&exists, &theme)[1].style,
        Style::default().fg(Color::LightCyan)
    );
}
