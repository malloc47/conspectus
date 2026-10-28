use ratatui::style::Modifier;

use super::*;

fn text(value: &str) -> OverrideValue<'_> {
    OverrideValue::Text(value)
}

/// Every `[tui.theme]` key the TUI accepted before its tokens moved into
/// the framework, with the default it had and the value a config might
/// give it. Each must still load.
const OLD_KEYS: &[(&str, &str)] = &[
    ("harness_claude", "magenta"),
    ("harness_codex", "cyan"),
    ("harness_opencode", "green"),
    ("harness_aider", "red"),
    ("harness_pi", "bright_blue"),
    ("harness_unknown", "white"),
    ("recency_fresh", "bright_green,bold"),
    ("recency_active", "green"),
    ("recency_recent", "yellow"),
    ("recency_cold", "dim"),
    ("mux_attached", "green"),
    ("mux_unmuxed", "dim"),
    ("selection_active", "reversed,bold"),
    ("selection_inactive", "bold"),
    ("panel_focus_accent", "cyan"),
    ("cwd_mark", "cyan"),
    ("link_id", "blue"),
    ("placeholder", "dim"),
    ("ghost", "dark_gray"),
    ("match_highlight", "yellow"),
    ("pin_placeholder", "bright_yellow"),
    ("command_badge", "black"),
    ("secondary_text", "dark_gray"),
    ("disclosure", "cyan"),
    ("divider", "dim"),
    ("warning", "yellow"),
    ("error", "red"),
    ("success", "green"),
    ("pr_open", "green"),
    ("pr_closed", "red"),
    ("pr_merged", "magenta"),
    ("pr_draft", "yellow"),
    ("badge", "reversed,bold"),
    ("node_workspace", "bright_blue"),
    ("node_repo", "blue"),
    ("node_checkout", "cyan"),
    ("node_agent_session", "bright_green"),
    ("node_mux_session", "magenta"),
    ("node_runtime_process", "dark_gray"),
    ("node_branch", "green"),
    ("node_fork", "bright_magenta"),
    ("edge_alt_of", "dark_gray"),
    ("edge_conflict", "yellow"),
];

#[test]
fn default_edge_state_colors_match_adr_0075() {
    let theme = Theme::default();
    // AltOf defaults to the inert hue and Conflict to the warning hue;
    // each can be themed apart from the broader palette.
    assert_eq!(theme.edge_alt_of(), theme.inert);
    assert_eq!(theme.edge_conflict(), theme.warning);
}

#[test]
fn default_node_kind_colors_match_adr_0073_slate() {
    let theme = Theme::default();
    assert_eq!(theme.node_workspace(), Color::LightBlue);
    assert_eq!(theme.node_repo(), Color::Blue);
    assert_eq!(theme.node_checkout(), Color::Cyan);
    assert_eq!(theme.node_agent_session(), Color::LightGreen);
    assert_eq!(theme.node_mux_session(), Color::Magenta);
    assert_eq!(theme.node_runtime_process(), Color::DarkGray);
    assert_eq!(theme.node_branch(), Color::Green);
    assert_eq!(theme.node_fork(), Color::LightMagenta);
    assert!(
        theme.icons.is_empty(),
        "default icon overrides should be empty so the geometric slate ships unchanged",
    );
}

#[test]
fn harness_colors_resolve_by_key_or_label() {
    let theme = Theme::default();
    assert_eq!(theme.harness_color("claude"), Color::Magenta);
    assert_eq!(theme.harness_color("claude-code"), Color::Magenta);
    assert_eq!(theme.harness_color("codex"), Color::Cyan);
    assert_eq!(theme.harness_color("opencode"), Color::Green);
    assert_eq!(theme.harness_color("aider"), Color::Red);
    assert_eq!(theme.harness_color("pi"), Color::LightBlue);
    assert_eq!(theme.harness_color("unknown-harness"), Color::White);
    assert_eq!(theme.harness_unknown(), Color::White);
}

#[test]
fn every_registered_harness_has_a_color_and_every_color_a_harness() {
    let mut registered: Vec<&str> = crate::discovery::harness::harness_keys().to_vec();
    let mut colored: Vec<&str> = HARNESS_COLORS.iter().map(|(key, _)| *key).collect();
    registered.sort_unstable();
    colored.sort_unstable();
    assert_eq!(colored, registered);
}

/// The value of each old key's default before the move, read through the
/// key that now holds it.
#[test]
fn the_default_theme_draws_what_it_drew_before() {
    let theme = Theme::default();
    let expect: &[(&str, TokenValue)] = &[
        ("muted", TokenValue::Modifier(Modifier::DIM)),
        ("inert", TokenValue::Color(Color::DarkGray)),
        ("accent", TokenValue::Color(Color::Cyan)),
        ("panel_focus_accent", TokenValue::Color(Color::Cyan)),
        ("link", TokenValue::Color(Color::Blue)),
        ("disclosure", TokenValue::Color(Color::Cyan)),
        ("match_highlight", TokenValue::Color(Color::Yellow)),
        ("warning", TokenValue::Color(Color::Yellow)),
        ("error", TokenValue::Color(Color::Red)),
        ("success", TokenValue::Color(Color::Green)),
        (
            "badge",
            TokenValue::Modifier(Modifier::REVERSED | Modifier::BOLD),
        ),
        (
            "selection_active",
            TokenValue::Modifier(Modifier::REVERSED | Modifier::BOLD),
        ),
        ("selection_inactive", TokenValue::Modifier(Modifier::BOLD)),
        ("badge_width", TokenValue::Width(8)),
        (
            "recency_fresh",
            TokenValue::Style(StyleSpec::fg_mod(Color::LightGreen, Modifier::BOLD)),
        ),
        (
            "recency_active",
            TokenValue::Style(StyleSpec::fg(Color::Green)),
        ),
        (
            "recency_recent",
            TokenValue::Style(StyleSpec::fg(Color::Yellow)),
        ),
        (
            "recency_cold",
            TokenValue::Style(StyleSpec::modifier(Modifier::DIM)),
        ),
        ("mux_attached", TokenValue::Color(Color::Green)),
        ("cwd_mark", TokenValue::Color(Color::Cyan)),
        ("command_badge", TokenValue::Color(Color::Black)),
        ("pin_placeholder", TokenValue::Color(Color::LightYellow)),
        ("pr_open", TokenValue::Color(Color::Green)),
        ("pr_closed", TokenValue::Color(Color::Red)),
        ("pr_merged", TokenValue::Color(Color::Magenta)),
        ("pr_draft", TokenValue::Color(Color::Yellow)),
        ("edge_alt_of", TokenValue::Color(Color::DarkGray)),
        ("edge_conflict", TokenValue::Color(Color::Yellow)),
    ];
    for (key, value) in expect {
        assert_eq!(theme.value(key), Some(*value), "{key}");
    }
    // The tokens new with the move reproduce the literals they replace.
    assert_eq!(theme.field, StyleSpec::fg(Color::Gray));
    assert_eq!(
        theme.field_active,
        StyleSpec::on(Color::White, Color::DarkGray)
    );
    assert_eq!(
        theme.field_cursor,
        StyleSpec::on(Color::Black, Color::White)
    );
    assert_eq!(theme.surface, Color::Reset);
    assert_eq!(theme.info, theme.success);
    assert_eq!(theme.command_badge_text(), Color::White);
}

#[test]
fn every_old_key_still_loads() {
    for (key, value) in OLD_KEYS {
        let mut theme = Theme::default();
        let problems = theme.apply_overrides([(*key, text(value))]);
        assert_eq!(problems, [], "`{key} = {value:?}`");
    }
}

#[test]
fn every_old_key_is_a_key_or_an_alias_today() {
    let theme = Theme::default();
    let keys: Vec<String> = theme.keys().into_iter().map(|key| key.name).collect();
    for (key, _) in OLD_KEYS {
        let known = keys.iter().any(|name| name == key)
            || theme.aliases().iter().any(|alias| alias.key == *key);
        assert!(known, "`{key}` is neither a key nor an alias");
    }
}

#[test]
fn each_alias_sets_its_target_and_loses_to_the_target_key() {
    for (alias, target) in ALIASES {
        let theme = Theme::default();
        let kind = theme.keys().into_iter().find(|key| key.name == *target);
        let kind = kind.expect("an alias targets a key").kind;
        let (mine, theirs, expected) = match kind {
            TokenKind::Color => (
                "#102030",
                "#405060",
                TokenValue::Color(Color::Rgb(0x10, 0x20, 0x30)),
            ),
            TokenKind::Modifier => ("italic", "bold", TokenValue::Modifier(Modifier::ITALIC)),
            other => panic!("`{alias}` targets a {other:?} key"),
        };

        let mut alone = theme.clone();
        assert_eq!(alone.apply_overrides([(*alias, text(mine))]), [], "{alias}");
        assert_eq!(alone.value(target), Some(expected), "{alias} sets {target}");

        let mut both = theme.clone();
        let problems = both.apply_overrides([(*alias, text(mine)), (*target, text(theirs))]);
        assert_ne!(both.value(target), Some(expected), "{target} beats {alias}");
        if theme.value(alias).is_some() {
            // A key that is a token of its own still sets that token, so
            // it is not ignored.
            assert_eq!(problems, [], "{alias}");
            assert_eq!(both.value(alias), Some(expected), "{alias} keeps its own");
            continue;
        }
        assert_eq!(problems.len(), 1, "{alias}: {problems:?}");
        assert_eq!(problems[0].key, *alias);
        assert!(
            matches!(&problems[0].problem, OverrideProblem::Shadowed { by, .. } if by == target),
            "{alias}: {problems:?}"
        );
    }
}

#[test]
fn placeholder_divider_and_mux_unmuxed_all_set_muted() {
    for key in ["placeholder", "divider", "mux_unmuxed"] {
        let mut theme = Theme::default();
        assert_eq!(theme.apply_overrides([(key, text("italic"))]), []);
        assert_eq!(theme.muted, Modifier::ITALIC, "{key}");
    }
}

#[test]
fn secondary_text_and_ghost_both_set_inert() {
    for key in ["secondary_text", "ghost"] {
        let mut theme = Theme::default();
        assert_eq!(theme.apply_overrides([(key, text("blue"))]), []);
        assert_eq!(theme.inert, Color::Blue, "{key}");
    }
}

#[test]
fn old_muted_keys_set_differently_merge_by_a_fixed_precedence() {
    // muted beats placeholder, which beats divider, which beats
    // mux_unmuxed; inert beats secondary_text, which beats ghost.
    let mut theme = Theme::default();
    let problems = theme.apply_overrides([
        ("divider", text("bold")),
        ("ghost", text("red")),
        ("mux_unmuxed", text("underline")),
        ("placeholder", text("italic")),
        ("secondary_text", text("blue")),
    ]);
    assert_eq!(theme.muted, Modifier::ITALIC);
    assert_eq!(theme.inert, Color::Blue);
    let ignored: Vec<&str> = problems.iter().map(|error| error.key.as_str()).collect();
    assert_eq!(ignored, ["divider", "ghost", "mux_unmuxed"]);

    let mut theme = Theme::default();
    theme.apply_overrides([
        ("divider", text("bold")),
        ("mux_unmuxed", text("underline")),
    ]);
    assert_eq!(theme.muted, Modifier::BOLD, "divider beats mux_unmuxed");
}

#[test]
fn panel_focus_accent_still_colors_the_accent_too() {
    let mut theme = Theme::default();
    assert_eq!(
        theme.apply_overrides([("panel_focus_accent", text("magenta"))]),
        []
    );
    assert_eq!(theme.accent, Color::Magenta);
    assert_eq!(theme.panel_focus_accent, Color::Magenta);

    let mut theme = Theme::default();
    let problems = theme.apply_overrides([
        ("accent", text("red")),
        ("panel_focus_accent", text("magenta")),
    ]);
    assert_eq!(problems, [], "both keys set a token of their own");
    assert_eq!(theme.accent, Color::Red);
    assert_eq!(theme.panel_focus_accent, Color::Magenta);
}

#[test]
fn a_harness_table_entry_beats_the_flat_alias() {
    let mut theme = Theme::default();
    let problems = theme.apply_overrides([
        ("harness_codex", text("red")),
        ("harness.codex", text("blue")),
    ]);
    assert_eq!(theme.harness_color("codex"), Color::Blue);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].key, "harness_codex");
}

#[test]
fn every_key_and_alias_is_unique() {
    let theme = Theme::default();
    let mut names: Vec<String> = theme.keys().into_iter().map(|key| key.name).collect();
    let keys = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), keys, "a key is listed twice");
    let mut aliases: Vec<&str> = ALIASES.iter().map(|(key, _)| *key).collect();
    aliases.sort_unstable();
    aliases.dedup();
    assert_eq!(aliases.len(), ALIASES.len(), "an alias is listed twice");
}
