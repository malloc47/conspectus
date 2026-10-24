use super::*;

fn text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn a_long_value_scrolls_around_the_cursor_with_edge_marks() {
    let value = "abcdefghijklmnopqrstuvwxyz";
    let w = window(value, 20, 12, true);
    assert_eq!(w.left, "< ", "the start is out of view");
    assert_eq!(w.right, " >", "the end is out of view");
    assert_eq!(w.cursor_text, "u");
    let w = window(value, 0, 12, true);
    assert_eq!(w.left, "  ");
    assert_eq!(w.right, " >");
}

#[test]
fn an_empty_value_shows_a_dash() {
    let line = value_row("cwd", "", 0, false, Cols::new(40), None);
    assert!(text(&line).contains('-'), "{}", text(&line));
}

#[test]
fn the_label_column_widens_for_long_labels() {
    let narrow = value_row("cwd", "x", 0, false, Cols::new(60), None);
    let wide = value_row(
        "worktree_branch",
        "x",
        0,
        false,
        Cols::new(60).with_label(15),
        None,
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
    );
    assert!(text(&custom).contains("[custom]"));
}

#[test]
fn launch_options_show_checked_when_the_argv_carries_the_flags() {
    let options = crate::discovery::harness::launch_options_for("claude-code");
    let off = options_row("options", options, &["claude".into()], false, Cols::new(60));
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
    );
    assert!(text(&on).contains("[x] skip permissions"), "{}", text(&on));
}

#[test]
fn the_footer_wraps_at_separators_without_breaking_a_hint() {
    let footer =
        "↑/↓ move · Tab complete/next · S-Tab back · ←/→/Space option · Enter launch · Esc cancel";
    let lines = wrap_footer(footer, 40);
    assert!(lines.len() >= 2);
    for line in &lines {
        assert!(text(line).chars().count() <= 40, "{}", text(line));
    }
    let joined: Vec<String> = lines.iter().map(text).collect();
    assert_eq!(joined.join(" · "), footer, "nothing lost or reordered");
}

#[test]
fn the_view_scrolls_only_when_the_cursor_would_fall_off() {
    assert_eq!(scroll_offset_for_cursor(2, 10, 30), 0);
    assert_eq!(scroll_offset_for_cursor(12, 10, 30), 3);
    assert_eq!(
        scroll_offset_for_cursor(29, 10, 30),
        20,
        "never past the end"
    );
}

#[test]
fn display_argv_quotes_what_a_shell_would() {
    assert_eq!(
        display_argv(&["claude".into(), "--model".into(), "opus 4".into()]),
        "claude --model 'opus 4'"
    );
    assert_eq!(display_argv(&["a".into(), String::new()]), "a ''");
}
