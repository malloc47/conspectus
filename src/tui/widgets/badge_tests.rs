use super::*;
use ratatui::style::{Color, Modifier};

#[test]
fn harness_badge_pads_short_labels_inside_styled_span() {
    // codex is 5 chars; the badge pads to the default width
    // (8 characters) inside the styled span so the entire badge —
    // padding included — carries the chip background.
    let theme = Theme::default();
    let span = harness_badge("codex", &theme);
    assert_eq!(span.content, " codex    ");
    assert_eq!(span.content.chars().count(), theme.badge_width + 2);
    assert_eq!(span.style.fg, Some(theme.harness_color("codex")));
    assert!(span.style.add_modifier.contains(Modifier::REVERSED));
    assert!(span.style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn command_badge_matches_harness_badge_width_and_truncates() {
    let theme = Theme::default();
    let short = command_badge("npm", &theme);
    assert_eq!(short.content, " npm      ");
    let long = command_badge("conspectus", &theme);
    assert_eq!(long.content, " conspec… ");
    assert_eq!(long.content.chars().count(), theme.badge_width + 2);
    assert_eq!(long.style.bg, Some(theme.command_badge()));
    assert_eq!(long.style.fg, Some(Color::White));
}

#[test]
fn badges_follow_the_theme_badge_width() {
    let mut theme = Theme::default();
    theme.badge_width = 5;
    assert_eq!(harness_badge("codex", &theme).content, " codex ");
    assert_eq!(harness_badge("opencode", &theme).content, " open… ");
    assert_eq!(command_badge("npm", &theme).content, " npm   ");
    assert_eq!(no_agent_chip(&theme).content, " no a… ");

    theme.badge_width = 12;
    assert_eq!(
        command_badge("conspectus", &theme).content,
        " conspectus   "
    );
    assert_eq!(no_agent_chip(&theme).content, " no agent     ");
}

#[test]
fn harness_badge_matches_max_label_width_without_padding() {
    let theme = Theme::default();
    let span = harness_badge("opencode", &theme);
    assert_eq!(span.content, " opencode ");
    assert_eq!(span.content.chars().count(), theme.badge_width + 2);
}

#[test]
fn harness_badge_unknown_harness_uses_unknown_color() {
    let theme = Theme::default();
    let span = harness_badge("custom", &theme);
    assert_eq!(span.style.fg, Some(theme.harness_unknown()));
}

#[test]
fn harness_badge_truncates_labels_longer_than_the_badge() {
    let theme = Theme::default();
    let span = harness_badge("conspectus", &theme);
    assert_eq!(span.content, " conspec… ");
    assert_eq!(span.content.chars().count(), theme.badge_width + 2);
}

#[test]
fn harness_badge_picks_correct_color_per_known_harness() {
    let theme = Theme::default();
    assert_eq!(
        harness_badge("claude", &theme).style.fg,
        Some(Color::Magenta)
    );
    assert_eq!(harness_badge("codex", &theme).style.fg, Some(Color::Cyan));
    assert_eq!(
        harness_badge("opencode", &theme).style.fg,
        Some(Color::Green)
    );
    assert_eq!(harness_badge("aider", &theme).style.fg, Some(Color::Red));
    assert_eq!(harness_badge("pi", &theme).style.fg, Some(Color::LightBlue));
}

#[test]
fn the_no_agent_chip_is_muted_and_as_wide_as_a_badge() {
    let theme = Theme::default();
    let chip = no_agent_chip(&theme);
    assert_eq!(chip.content, " no agent ");
    assert_eq!(chip.style, Style::default().add_modifier(theme.muted));
    assert_eq!(
        tuikit::text::width(&chip.content),
        tuikit::text::width(&harness_badge("codex", &theme).content)
    );
}

#[test]
fn the_row_notes_recede_two_cells_after_the_label() {
    let theme = Theme::default();
    let note = Style::default().fg(theme.inert).add_modifier(theme.muted);
    assert_eq!(hits_chip(1, &theme).content, "  1 hit");
    assert_eq!(hits_chip(3, &theme).content, "  3 hits");
    assert_eq!(hits_chip(3, &theme).style, note);
    assert_eq!(
        was_in_chip("build", false, &theme).content,
        "  was in build"
    );
    assert_eq!(
        was_in_chip("build", true, &theme).content,
        "  (mux stopped)"
    );
    let long = was_in_chip("a-very-long-mux-name-indeed", false, &theme);
    assert_eq!(long.content, "  was in a-very-long-mux-nam…");
    assert_eq!(long.style, note);
}
