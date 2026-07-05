// Extracted from theme.rs H-HYG-011 rolling wave via #[path = "theme_tests.rs"] mod tests;
use super::*;

#[test]
fn default_edge_state_colors_match_adr_0075() {
    let theme = Theme::default();
    // AltOf defaults to the secondary-text hue; Conflict
    // defaults to the warning hue. The pair is independently
    // themable so operators can paint conflict in a louder
    // color than the global warning palette without touching
    // group-level chip styling.
    assert_eq!(theme.edge_alt_of, Color::DarkGray);
    assert_eq!(theme.edge_conflict, Color::Yellow);
}

#[test]
fn default_node_kind_colors_match_adr_0073_slate() {
    let theme = Theme::default();
    assert_eq!(theme.node_workspace, Color::LightBlue);
    assert_eq!(theme.node_repo, Color::Blue);
    assert_eq!(theme.node_checkout, Color::Cyan);
    assert_eq!(theme.node_agent_session, Color::LightGreen);
    assert_eq!(theme.node_mux_session, Color::Magenta);
    assert_eq!(theme.node_runtime_process, Color::DarkGray);
    assert_eq!(theme.node_branch, Color::Green);
    assert_eq!(theme.node_fork, Color::LightMagenta);
    assert!(
        theme.icons.is_empty(),
        "default icon overrides should be empty so the geometric slate ships unchanged",
    );
}

#[test]
fn default_preserves_inline_literals() {
    let theme = Theme::default();
    // H-EXT-003: both the display label and the harness key
    // resolve to the same entry in `harness_colors`.
    assert_eq!(theme.harness_color("claude"), Color::Magenta);
    assert_eq!(theme.harness_color("claude-code"), Color::Magenta);
    assert_eq!(theme.harness_color("codex"), Color::Cyan);
    assert_eq!(theme.harness_color("opencode"), Color::Green);
    assert_eq!(theme.harness_color("aider"), Color::Red);
    assert_eq!(theme.harness_color("unknown-harness"), Color::White);
    assert_eq!(theme.mux_attached, Color::Green);
    assert_eq!(theme.mux_ambiguous, Color::Yellow);
    assert_eq!(theme.mux_unmuxed, Modifier::DIM);
    assert_eq!(
        theme.selection_active,
        Modifier::REVERSED.union(Modifier::BOLD)
    );
}

#[test]
fn style_spec_into_style_combines_color_and_modifier() {
    let spec = StyleSpec::fg_mod(Color::Green, Modifier::BOLD);
    let style = spec.into_style();
    assert_eq!(style.fg, Some(Color::Green));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn style_spec_modifier_only_omits_fg() {
    let spec = StyleSpec::modifier(Modifier::DIM);
    let style = spec.into_style();
    assert_eq!(style.fg, None);
    assert!(style.add_modifier.contains(Modifier::DIM));
}

#[test]
fn parse_color_accepts_named_ansi() {
    assert_eq!(parse_color("magenta").unwrap(), Color::Magenta);
    assert_eq!(parse_color("Bright_Green").unwrap(), Color::LightGreen);
    assert_eq!(parse_color("default").unwrap(), Color::Reset);
}

#[test]
fn parse_color_accepts_ansi256_and_hex() {
    assert_eq!(parse_color("ansi256:42").unwrap(), Color::Indexed(42));
    assert_eq!(
        parse_color("#ff8800").unwrap(),
        Color::Rgb(0xff, 0x88, 0x00)
    );
    assert_eq!(parse_color("#FFFFFF").unwrap(), Color::Rgb(255, 255, 255));
}

#[test]
fn parse_color_rejects_garbage() {
    assert!(parse_color("").is_err());
    assert!(parse_color("not-a-color").is_err());
    assert!(parse_color("ansi256:999").is_err());
    assert!(parse_color("#zzzzzz").is_err());
    assert!(parse_color("#abc").is_err());
}

#[test]
fn parse_modifier_accepts_comma_list() {
    let m = parse_modifier("bold,italic").unwrap();
    assert!(m.contains(Modifier::BOLD));
    assert!(m.contains(Modifier::ITALIC));
    assert_eq!(parse_modifier("").unwrap(), Modifier::empty());
    assert_eq!(
        parse_modifier("reverse").unwrap(),
        Modifier::REVERSED,
        "`reverse` is an accepted alias for `reversed`"
    );
}

#[test]
fn parse_modifier_rejects_unknown_token() {
    assert!(parse_modifier("bold,not-a-mod").is_err());
}

#[test]
fn parse_style_spec_handles_color_plus_modifier() {
    let spec = parse_style_spec("green,bold").unwrap();
    assert_eq!(spec.color, Some(Color::Green));
    assert!(spec.modifier.contains(Modifier::BOLD));
}

#[test]
fn parse_style_spec_handles_modifier_only() {
    let spec = parse_style_spec("dim").unwrap();
    assert_eq!(spec.color, None);
    assert!(spec.modifier.contains(Modifier::DIM));
}

#[test]
fn parse_style_spec_handles_color_only() {
    let spec = parse_style_spec("#ff8800").unwrap();
    assert_eq!(spec.color, Some(Color::Rgb(0xff, 0x88, 0x00)));
    assert_eq!(spec.modifier, Modifier::empty());
}

#[test]
fn known_keys_cover_every_settable_field() {
    // If a field is in known_keys with kind Color, set_color must
    // accept its name; same for Modifier and StyleSpec. Catches
    // drift when fields are added to the struct but missed in
    // known_keys / set_*.
    let mut theme = Theme::default();
    for entry in Theme::known_keys() {
        let accepted = match entry.kind {
            ThemeKeyKind::Color => theme.set_color(entry.name, Color::Black),
            ThemeKeyKind::Modifier => theme.set_modifier(entry.name, Modifier::BOLD),
            ThemeKeyKind::StyleSpec => {
                theme.set_style_spec(entry.name, StyleSpec::fg(Color::Black))
            }
        };
        assert!(accepted, "set_* refused known key `{}`", entry.name);
    }
}
