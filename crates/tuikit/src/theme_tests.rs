use super::*;

fn text(value: &str) -> OverrideValue<'_> {
    OverrideValue::Text(value)
}

/// A theme with one named style and the aliases a host might keep after
/// merging two older keys into `muted`.
fn host_theme() -> Theme {
    let mut theme = Theme::default();
    theme.register("row_state", Color::Green, "a state color");
    theme.register_alias("faint", "muted");
    theme.register_alias("rule", "muted");
    theme.register_alias("old_state", "row_state");
    theme
}

#[test]
fn defaults_are_the_framework_palette() {
    let theme = Theme::default();
    assert_eq!(theme.accent, Color::Cyan);
    assert_eq!(theme.panel_focus_accent, Color::Cyan);
    assert_eq!(theme.muted, Modifier::DIM);
    assert_eq!(theme.inert, Color::DarkGray);
    assert_eq!(theme.selection_active, Modifier::REVERSED | Modifier::BOLD);
    assert_eq!(theme.selection_inactive, Modifier::BOLD);
    assert_eq!(theme.badge_width, DEFAULT_BADGE_WIDTH);
    assert_eq!(theme.surface, Color::Reset);
    assert_eq!(theme.field, StyleSpec::fg(Color::Gray));
    assert_eq!(
        theme.field_active,
        StyleSpec::on(Color::White, Color::DarkGray)
    );
    assert_eq!(
        theme.field_cursor,
        StyleSpec::on(Color::Black, Color::White)
    );
}

#[test]
fn match_style_is_the_highlight_color_bold_and_underlined() {
    let style = Theme::default().match_style();
    assert_eq!(style.fg, Some(Color::Yellow));
    assert!(
        style
            .add_modifier
            .contains(Modifier::BOLD | Modifier::UNDERLINED)
    );
}

#[test]
fn every_token_key_is_listed_once_and_reads_back_its_field() {
    let theme = Theme::default();
    let keys = theme.keys();
    let mut names: Vec<&str> = keys.iter().map(|key| key.name.as_str()).collect();
    let listed = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), listed, "a key is listed twice");
    for key in &keys {
        assert_eq!(key.origin, KeyOrigin::Token, "{}", key.name);
        assert!(!key.about.is_empty(), "{} has no description", key.name);
        let value = theme.value(&key.name).expect("a listed key stores a value");
        let kind_matches = match (key.kind, value) {
            (TokenKind::Width { .. }, TokenValue::Width(_)) => true,
            (kind, value) => kind == value.kind(),
        };
        assert!(
            kind_matches,
            "{} lists {:?} but stores {value:?}",
            key.name, key.kind
        );
    }
    assert_eq!(theme.value("accent"), Some(TokenValue::Color(theme.accent)));
    assert_eq!(theme.value("badge_width"), Some(TokenValue::Width(8)));
}

#[test]
fn every_listed_key_takes_a_value_from_a_table() {
    // One value per kind that differs from every default.
    let mut theme = Theme::default();
    for key in Theme::default().keys() {
        let (value, expected) = match key.kind {
            TokenKind::Color => (
                text("#123456"),
                TokenValue::Color(Color::Rgb(0x12, 0x34, 0x56)),
            ),
            TokenKind::Modifier => (
                text("crossed_out"),
                TokenValue::Modifier(Modifier::CROSSED_OUT),
            ),
            TokenKind::Style => (
                text("#123456 on #654321,hidden"),
                TokenValue::Style(StyleSpec {
                    fg: Some(Color::Rgb(0x12, 0x34, 0x56)),
                    bg: Some(Color::Rgb(0x65, 0x43, 0x21)),
                    modifier: Modifier::HIDDEN,
                }),
            ),
            TokenKind::Width { .. } => (OverrideValue::Integer(31), TokenValue::Width(31)),
        };
        let problems = theme.apply_overrides([(key.name.as_str(), value)]);
        assert_eq!(problems, [], "{}", key.name);
        assert_eq!(theme.value(&key.name), Some(expected), "{}", key.name);
    }
}

#[test]
fn a_registered_style_is_listed_looked_up_and_overridden() {
    let mut theme = host_theme();
    theme.register(
        "chip_fill",
        StyleSpec::on(Color::White, Color::Black),
        "chip fill",
    );
    let names: Vec<String> = theme
        .keys()
        .into_iter()
        .filter(|key| key.origin == KeyOrigin::Named)
        .map(|key| key.name)
        .collect();
    assert_eq!(names, ["chip_fill", "row_state"]);

    assert_eq!(theme.color("row_state"), Some(Color::Green));
    assert_eq!(theme.style("row_state").fg, Some(Color::Green));
    assert_eq!(theme.style("chip_fill").bg, Some(Color::Black));
    assert_eq!(theme.color("chip_fill"), Some(Color::White));

    let problems = theme.apply_overrides([("row_state", text("magenta"))]);
    assert_eq!(problems, []);
    assert_eq!(theme.color("row_state"), Some(Color::Magenta));
}

#[test]
fn an_unregistered_name_styles_nothing() {
    let theme = Theme::default();
    assert_eq!(theme.style("nothing_here"), Style::default());
    assert_eq!(theme.color("nothing_here"), None);
    assert_eq!(theme.modifier("nothing_here"), Modifier::empty());
    assert_eq!(theme.value("nothing_here"), None);
}

#[test]
fn lookups_by_name_also_reach_the_typed_tokens() {
    let theme = Theme::default();
    assert_eq!(theme.color("accent"), Some(Color::Cyan));
    assert_eq!(theme.modifier("muted"), Modifier::DIM);
    assert_eq!(theme.style("field_cursor").bg, Some(Color::White));
}

#[test]
fn set_refuses_a_value_of_another_kind() {
    let mut theme = host_theme();
    assert!(!theme.set("accent", TokenValue::Modifier(Modifier::BOLD)));
    assert!(!theme.set("row_state", TokenValue::Width(3)));
    assert!(!theme.set("faint", TokenValue::Modifier(Modifier::BOLD)));
    assert!(theme.set("row_state", TokenValue::Color(Color::Red)));
    assert_eq!(theme.color("row_state"), Some(Color::Red));
}

#[test]
#[should_panic(expected = "is a typed token")]
fn a_host_cannot_register_over_a_token() {
    Theme::default().register("accent", Color::Red, "no");
}

#[test]
#[should_panic(expected = "neither a token nor a registered name")]
fn an_alias_needs_a_target_that_exists() {
    Theme::default().register_alias("old", "not_a_token");
}

#[test]
fn an_alias_sets_its_target_when_the_target_key_is_absent() {
    let mut theme = host_theme();
    assert_eq!(theme.apply_overrides([("faint", text("italic"))]), []);
    assert_eq!(theme.muted, Modifier::ITALIC);
    assert_eq!(theme.apply_overrides([("old_state", text("blue"))]), []);
    assert_eq!(theme.color("row_state"), Some(Color::Blue));
}

#[test]
fn the_target_key_wins_over_its_aliases() {
    let mut theme = host_theme();
    let problems = theme.apply_overrides([("faint", text("italic")), ("muted", text("bold"))]);
    assert_eq!(theme.muted, Modifier::BOLD);
    assert_eq!(
        problems,
        [OverrideError {
            key: "faint".to_string(),
            problem: OverrideProblem::Shadowed {
                target: "muted".to_string(),
                by: "muted".to_string(),
            },
        }]
    );
}

#[test]
fn among_aliases_the_first_registered_wins() {
    let mut theme = host_theme();
    // Table order does not matter; registration order does.
    let problems = theme.apply_overrides([("rule", text("bold")), ("faint", text("italic"))]);
    assert_eq!(theme.muted, Modifier::ITALIC);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].key, "rule");
    assert_eq!(
        problems[0].problem.to_string(),
        "is ignored: it sets `muted`, which `faint` in the same table sets to something else"
    );
}

#[test]
fn aliases_that_agree_are_not_reported() {
    let mut theme = host_theme();
    let problems = theme.apply_overrides([
        ("muted", text("italic")),
        ("faint", text("italic")),
        ("rule", text(" Italic ")),
    ]);
    assert_eq!(problems, []);
    assert_eq!(theme.muted, Modifier::ITALIC);
}

#[test]
fn a_bad_value_is_reported_and_the_next_key_in_line_applies() {
    let mut theme = host_theme();
    let problems = theme.apply_overrides([("muted", text("sparkly")), ("faint", text("italic"))]);
    assert_eq!(theme.muted, Modifier::ITALIC);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].key, "muted");
    assert!(matches!(problems[0].problem, OverrideProblem::Invalid(_)));
}

#[test]
fn the_accent_also_sets_the_focus_cue_unless_the_table_does() {
    let mut theme = Theme::default();
    assert_eq!(theme.apply_overrides([("accent", text("magenta"))]), []);
    assert_eq!(theme.accent, Color::Magenta);
    assert_eq!(theme.panel_focus_accent, Color::Magenta);

    let mut theme = Theme::default();
    let problems = theme.apply_overrides([
        ("accent", text("magenta")),
        ("panel_focus_accent", text("red")),
    ]);
    assert_eq!(problems, []);
    assert_eq!(theme.accent, Color::Magenta);
    assert_eq!(theme.panel_focus_accent, Color::Red);

    let mut theme = Theme::default();
    assert_eq!(
        theme.apply_overrides([("panel_focus_accent", text("red"))]),
        []
    );
    assert_eq!(theme.accent, Color::Cyan, "the cue alone leaves the accent");
    assert_eq!(theme.panel_focus_accent, Color::Red);
}

#[test]
fn a_key_that_is_a_token_and_an_alias_sets_both() {
    let mut theme = Theme::default();
    theme.register_alias("panel_focus_accent", "accent");
    assert_eq!(
        theme.apply_overrides([("panel_focus_accent", text("red"))]),
        []
    );
    assert_eq!(theme.accent, Color::Red);
    assert_eq!(theme.panel_focus_accent, Color::Red);
}

#[test]
fn unknown_keys_and_wrong_types_are_reported_and_the_rest_apply() {
    let mut theme = Theme::default();
    let problems = theme.apply_overrides([
        ("accent", text("red")),
        ("no_such_key", text("red")),
        ("warning", OverrideValue::Integer(3)),
        ("error", OverrideValue::Other),
        ("badge_width", text("wide")),
    ]);
    assert_eq!(theme.accent, Color::Red);
    assert_eq!(theme.warning, Color::Yellow);
    let reported: Vec<(&str, &OverrideProblem)> = problems
        .iter()
        .map(|error| (error.key.as_str(), &error.problem))
        .collect();
    assert_eq!(
        reported,
        [
            ("no_such_key", &OverrideProblem::UnknownKey),
            ("warning", &OverrideProblem::NotText),
            ("error", &OverrideProblem::NotText),
            (
                "badge_width",
                &OverrideProblem::NotWidth {
                    min: MIN_BADGE_WIDTH
                }
            ),
        ]
    );
}

#[test]
fn a_width_below_its_minimum_keeps_the_default() {
    let mut theme = Theme::default();
    for width in [1, 0, -3] {
        let problems = theme.apply_overrides([("badge_width", OverrideValue::Integer(width))]);
        assert_eq!(problems.len(), 1, "{width}");
        assert_eq!(theme.badge_width, DEFAULT_BADGE_WIDTH);
    }
    assert_eq!(
        theme.apply_overrides([("badge_width", OverrideValue::Integer(2))]),
        []
    );
    assert_eq!(theme.badge_width, 2);
}

#[test]
fn a_key_given_twice_uses_its_last_value() {
    let mut theme = Theme::default();
    let problems = theme.apply_overrides([("link", text("red")), ("link", text("green"))]);
    assert_eq!(problems, []);
    assert_eq!(theme.link, Color::Green);
}

#[test]
fn a_later_table_overrides_an_earlier_one_whatever_key_it_used() {
    let mut theme = host_theme();
    assert_eq!(theme.apply_overrides([("muted", text("bold"))]), []);
    assert_eq!(theme.apply_overrides([("rule", text("italic"))]), []);
    assert_eq!(theme.muted, Modifier::ITALIC);
}

#[test]
fn the_default_aliases_are_listed() {
    let theme = host_theme();
    let aliases: Vec<(&str, &str)> = theme
        .aliases()
        .iter()
        .map(|alias| (alias.key.as_str(), alias.target.as_str()))
        .collect();
    assert_eq!(
        aliases,
        [
            ("accent", "panel_focus_accent"),
            ("faint", "muted"),
            ("rule", "muted"),
            ("old_state", "row_state"),
        ]
    );
}
