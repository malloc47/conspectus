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
    // Both the display label and the harness key
    // resolve to the same entry in `harness_colors`.
    assert_eq!(theme.harness_color("claude"), Color::Magenta);
    assert_eq!(theme.harness_color("claude-code"), Color::Magenta);
    assert_eq!(theme.harness_color("codex"), Color::Cyan);
    assert_eq!(theme.harness_color("opencode"), Color::Green);
    assert_eq!(theme.harness_color("aider"), Color::Red);
    assert_eq!(theme.harness_color("pi"), Color::LightBlue);
    assert_eq!(theme.harness_color("unknown-harness"), Color::White);
    assert_eq!(theme.mux_attached, Color::Green);
    assert_eq!(theme.mux_unmuxed, Modifier::DIM);
    assert_eq!(
        theme.selection_active,
        Modifier::REVERSED.union(Modifier::BOLD)
    );
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
