use super::*;
use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

/// Every `KEYBINDINGS` entry's key label
/// must appear somewhere in `keymap_sections`'s rendered
/// output. Guards against the pre-H-HYG-007 dispatcher /
/// help-overlay drift the audit called out — a new binding
/// added to the table stays reachable at render time only
/// if the help section names it too. Failing this test
/// signals that a wave-1..4 migrated binding is missing
/// from the operator-facing help.
#[test]
fn every_keybindings_entry_appears_in_help_sections() {
    use crate::tui::keybindings::{KEYBINDINGS, KeyMode, key_label};
    // Concatenate every section's binding key strings.
    let sections = keymap_sections();
    let mut haystack = String::new();
    for section in &sections {
        for binding in &section.bindings {
            haystack.push_str(binding.key());
            haystack.push('\n');
        }
    }
    // Ratatui-cheese `Binding::key(&self)` returns the key
    // string; the haystack now holds every operator-facing
    // key label. Check each KEYBINDINGS entry.
    //
    // Multi-key help rows collapse several bindings into one
    // string (e.g. "1 – 5", "j / k / ↓ / ↑"). The test
    // maps each KEYBINDINGS entry to one or more equivalent
    // string forms the help output uses, and asserts at
    // least one of those forms is present.
    fn equivalent_forms(label: &str) -> Vec<String> {
        match label {
            "1" | "2" | "3" | "4" | "5" => vec![label.to_string(), "1 – 5".to_string()],
            "Ctrl-c" => vec!["Ctrl-C".to_string()],
            "Ctrl-g" => vec!["Ctrl-G".to_string()],
            "Down" | "Up" => vec!["↓".to_string(), "↑".to_string()],
            "Right" | "Left" => vec!["→".to_string(), "←".to_string()],
            "Home" | "End" => vec!["g".to_string(), "G".to_string()],
            other => vec![other.to_string()],
        }
    }

    let mut missing: Vec<String> = Vec::new();
    for binding in KEYBINDINGS {
        if binding.mode != KeyMode::Global {
            continue;
        }
        let label = key_label(&binding.key);
        let forms = equivalent_forms(&label);
        if !forms.iter().any(|f| haystack.contains(f)) {
            missing.push(format!(
                "binding {label:?} (help: {help:?}) not in help sections",
                help = binding.help_text,
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "H-HYG-007 wave 5 drift: KEYBINDINGS entries missing from help sections:\n{}",
        missing.join("\n")
    );
}

#[test]
fn help_body_includes_node_kind_icon_legend() {
    // H-UI-002 slice: pressing `?` should surface a built-in
    // legend for the ADR 0073 glyph slate so operators learn
    // the symbol vocabulary without cross-referencing the
    // docs. Every NodeKind in canonical display order must
    // appear with its glyph, its operator-facing display
    // name, and a short blurb.
    let theme = Theme::default();
    let lines = body_lines(&theme);
    let plain_lines: Vec<String> = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect();
    let body_text = plain_lines.join("\n");
    assert!(
        body_text.contains("Node kind icons"),
        "expected section header in:\n{body_text}",
    );
    for kind in NodeKind::ALL {
        let glyph = node_kind_style(kind, &theme).glyph;
        let name = node_kind_display_name(kind);
        assert!(
            plain_lines
                .iter()
                .any(|line| line.contains(&glyph) && line.contains(name)),
            "expected legend row for {kind:?} ({glyph} {name}) in:\n{body_text}",
        );
    }
}

#[test]
fn esc_closes_help_overlay() {
    let mut state = HelpOverlayState::new();
    assert_eq!(state.handle_key(key(KeyCode::Esc)), HelpOutcome::Close);
}

#[test]
fn q_and_question_mark_close_help_overlay() {
    let mut state = HelpOverlayState::new();
    assert_eq!(
        state.handle_key(key(KeyCode::Char('q'))),
        HelpOutcome::Close
    );
    assert_eq!(
        state.handle_key(key(KeyCode::Char('?'))),
        HelpOutcome::Close
    );
}

#[test]
fn ctrl_c_closes_help_overlay() {
    let mut state = HelpOverlayState::new();
    let event = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(state.handle_key(event), HelpOutcome::Close);
}

#[test]
fn other_keys_are_swallowed_not_propagated() {
    let mut state = HelpOverlayState::new();
    assert_eq!(
        state.handle_key(key(KeyCode::Char('j'))),
        HelpOutcome::Continue
    );
    assert_eq!(state.handle_key(key(KeyCode::Enter)), HelpOutcome::Continue);
}

#[test]
fn body_lines_cover_every_documented_key() {
    let theme = Theme::default();
    let rendered: String = body_lines(&theme)
        .iter()
        .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
        .collect::<Vec<_>>()
        .join(" ");
    // Every key-literal reference used in a bind() call must
    // appear in the rendered help text.
    for needle in [
        "f ",
        "? ",
        "Enter ",
        "a ",
        "i ",
        "R ",
        "v ",
        "r ",
        "S ",
        "b ",
        "Delete ",
        "q / Ctrl-C",
        "1 / 2",
        "] / [",
        "F ",
        "Ctrl-G",
        "/ ",
        "j / k / ↓ / ↑",
        "l / → / h / ←",
        "PgDn / PgUp",
        "g / G",
        "Tab",
        "J / K",
        "j / k",
        "e ",
        "Backspace",
        "E ",
        "o ",
    ] {
        assert!(
            rendered.contains(needle),
            "help text missing `{needle}` reference; full text:\n{rendered}"
        );
    }
    // Every action description must also be visible so the
    // operator knows what each binding does.
    for desc in [
        "Open the controls overlay",
        "This help",
        "Default action on the selected row",
        "Attach to the selected mux",
        "Copy the selected agent or mux session's full id",
        "Rename the selected agent session (alias), mux (tmux + pin cascade), or pin's display name",
        "Open the selected session's transcript",
        "Refresh discovery now",
        "Resume the selected un-muxed agent session",
        "Quit",
        "Open the pins overlay (menu listing every action)",
        "New stream — opens the create form",
        "Launch the selected pin",
        "Rebind the selected pin's mux target",
        "Bind picker for the selected PinAmbiguous row",
        "Adopt the selected live mux row as a new pin",
        "Remove the selected pin (two-press confirmation)",
        "Switch directly to the sessions / mux view",
        "Cycle to next / previous view",
        "Clear all active filters",
        "Cycle grouping forward",
        "Open the search overlay",
        "Move selection down / up",
        "Expand / collapse the selected left-tree row",
        "Page through the row tree",
        "First / last row",
        "Cycle focus",
        "Scroll the right-panel preview",
        "Move the explorer cursor",
        "Toggle expand/collapse on a multi-link",
        "Back out of the most recent drilldown",
        "Toggle Expanded Node Detail",
        "Toggle edge meta",
        "Open the full untruncated value",
    ] {
        assert!(
            rendered.contains(desc),
            "help text missing description `{desc}`; full text:\n{rendered}"
        );
    }
}

fn line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn narrow_width_wraps_descriptions_under_a_hanging_indent() {
    let theme = Theme::default();
    let width = 60;
    let lines = body_lines_for_width(&theme, Some(width));
    // The icon legend is a fixed-width table; only the keymap wraps.
    let keymap_end = lines
        .iter()
        .position(|l| line_text(l) == "Node kind icons")
        .expect("legend header");
    for line in &lines[..keymap_end] {
        assert!(
            unicode_width::UnicodeWidthStr::width(line_text(line).as_str()) <= width,
            "line exceeds {width} cells: {:?}",
            line_text(line)
        );
    }
    // The long `w` description continues on an indented line.
    let texts: Vec<String> = lines.iter().map(line_text).collect();
    let w_idx = texts
        .iter()
        .position(|t| t.starts_with("  w "))
        .expect("w binding");
    assert!(texts[w_idx + 1].starts_with(&" ".repeat(KEY_COLUMN_WIDTH)));
    assert!(body_lines(&theme).len() < lines.len());
}

#[test]
fn jumping_to_the_end_shows_the_last_lines_not_blank_space() {
    let theme = Theme::default();
    let mut state = HelpOverlayState::new();
    state.handle_key(key(KeyCode::Char('G')));
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    HelpOverlayWidget::new(&state, &theme).render(area, &mut buf);
    let rendered: String = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("to close"),
        "end of keymap should be visible after G:\n{rendered}"
    );
}
