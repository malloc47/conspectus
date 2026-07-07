// Extracted from pins.rs H-HYG-009 wave 2 via #[path = "pins_tests.rs"] mod tests;
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

fn shift_key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn move_to_create_field(editor: &mut PinCreateState, target: usize) {
    for _ in 0..editor.field_count() {
        if editor.render_cursor() == target {
            return;
        }
        editor.handle_key(key(KeyCode::Down));
    }
    panic!("field {target} is not visible");
}

fn pin_target() -> PinMutationTarget {
    PinMutationTarget {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/workspace/project".to_string(),
        mux_name: "ingest-mux".to_string(),
        mux_socket: Some("scratch".to_string()),
        launch_argv: vec!["codex".to_string(), "--resume".to_string()],
        store_path: "/workspace/project/.conspectus.toml".to_string(),
    }
}

#[test]
fn opens_at_first_action() {
    let state = PinsOverlayState::new();
    assert_eq!(state.cursor(), PinsCursor::Action(0));
}

#[test]
fn arrow_keys_wrap_around_action_list() {
    let ctx = PinsContext::default();
    let mut state = PinsOverlayState::new();
    for _ in 0..PIN_ACTION_OPTIONS.len() {
        state.handle_key(&ctx, key(KeyCode::Down));
    }
    assert_eq!(state.cursor(), PinsCursor::Action(0));
}

#[test]
fn pins_menu_content_count_tracks_rendered_body() {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for label in PIN_ACTION_OPTIONS {
        lines.push(row_line((*label).to_string(), false));
    }
    lines.push(Line::default());
    lines.push(Line::from(span!(
        Modifier::DIM;
        "↑/↓ move · Enter pick · Esc close"
    )));
    assert_eq!(pins_menu_content_lines(), lines.len());
}

#[test]
fn pins_menu_has_single_create_entry_without_adopt_peer() {
    assert!(PIN_ACTION_OPTIONS.contains(&"create"));
    assert!(!PIN_ACTION_OPTIONS.contains(&"adopt"));
}

#[test]
fn selected_last_pin_action_scrolls_into_short_menu_body() {
    let cursor_line = pins_menu_cursor_line(PinsCursor::Action(PIN_ACTION_OPTIONS.len() - 1));
    let inner_height = 4;
    let offset = scroll_offset_for_cursor(cursor_line, inner_height, pins_menu_content_lines());
    let cursor_line = cursor_line.unwrap();
    assert!(offset > 0, "short pins menu should scroll");
    assert!(cursor_line >= offset as usize);
    assert!(cursor_line < offset as usize + inner_height);
}

#[test]
fn esc_at_top_level_closes_overlay() {
    let ctx = PinsContext::default();
    let mut state = PinsOverlayState::new();
    let outcome = state.handle_key(&ctx, key(KeyCode::Esc));
    assert_eq!(outcome, PinsOutcome::Close);
}

#[test]
fn enter_on_create_opens_create_editor() {
    let ctx = PinsContext {
        pin_create_defaults: PinCreateDefaults {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest".to_string(),
            ..PinCreateDefaults::default()
        },
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, PinsOutcome::Continue);
    assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Create(_))));
}

#[test]
fn create_form_starts_adopt_checked_then_auto_unchecks_on_first_name_edit() {
    let ctx = PinsContext {
        pin_create_defaults: PinCreateDefaults {
            id: "agentdeck-conspectus-2".to_string(),
            display_name: "agentdeck-conspectus-2".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "agentdeck-conspectus-2".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        pin_adopt_defaults: Some(PinCreateDefaults {
            id: "agentdeck-conspectus".to_string(),
            display_name: "agentdeck_conspectus".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "agentdeck_conspectus".to_string(),
            mode: PinCreateMode::AdoptSelected,
        }),
        known_mux_names: vec!["agentdeck_conspectus".to_string()],
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.handle_key(&ctx, key(KeyCode::Enter));

    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
            assert_eq!(editor.name.value(), "agentdeck_conspectus");
            assert_eq!(editor.mux_name.value(), "agentdeck_conspectus");
            assert_eq!(editor.mux_name_display(), "agentdeck_conspectus");
            let request = editor.request().expect("valid create request");
            assert_eq!(request.mux_name, "agentdeck_conspectus");
            assert_eq!(
                request.adopt_source_mux_name.as_deref(),
                Some("agentdeck_conspectus")
            );
        }
        other => panic!("unexpected editor: {other:?}"),
    }

    for _ in 0.."agentdeck_conspectus".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for ch in "agentdeck-conspectus-v2".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(ch)));
    }

    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(editor.mode, PinCreateMode::NewVariation);
            assert_eq!(editor.name.value(), "agentdeck-conspectus-v2");
            assert_eq!(editor.id.value(), "agentdeck-conspectus-v2");
            assert_eq!(editor.display_name.value(), "agentdeck-conspectus-v2");
            assert_eq!(editor.mux_name.value(), "agentdeck-conspectus-v2");
            assert_eq!(editor.mux_name_display(), "agentdeck-conspectus-v2");
            assert_eq!(
                editor
                    .request()
                    .expect("valid request")
                    .adopt_source_mux_name,
                None
            );
        }
        other => panic!("unexpected editor: {other:?}"),
    }

    state.handle_key(&ctx, key(KeyCode::Down));
    state.handle_key(&ctx, key(KeyCode::Char(' ')));
    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
            assert_eq!(
                editor.mux_name_display(),
                "agentdeck-conspectus-v2 (rename of: agentdeck_conspectus)"
            );
        }
        other => panic!("unexpected editor: {other:?}"),
    }

    state.handle_key(&ctx, key(KeyCode::Up));
    for _ in 0.."agentdeck-conspectus-v2".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for ch in "agentdeck-conspectus-v3".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(ch)));
    }
    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
            assert_eq!(
                editor.mux_name_display(),
                "agentdeck-conspectus-v3 (rename of: agentdeck_conspectus)"
            );
        }
        other => panic!("unexpected editor: {other:?}"),
    }
}

#[test]
fn direct_adopt_opens_create_form_with_adopt_selected_and_toggleable() {
    let defaults = PinCreateDefaults {
        id: "work-2".to_string(),
        display_name: "work-2".to_string(),
        harness: "codex".to_string(),
        cwd: "/workspace/project".to_string(),
        mux_name: "work-2".to_string(),
        mode: PinCreateMode::NewVariation,
    };
    let adopt = PinCreateDefaults {
        id: "work".to_string(),
        display_name: "work".to_string(),
        harness: "codex".to_string(),
        cwd: "/workspace/project".to_string(),
        mux_name: "work".to_string(),
        mode: PinCreateMode::AdoptSelected,
    };
    let mut state = PinsOverlayState::open_with_adopt_options(
        defaults,
        Some(adopt),
        vec![],
        vec![],
        vec![],
        vec![],
        None,
    );

    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
            assert!(editor.can_toggle_mode());
        }
        other => panic!("unexpected editor: {other:?}"),
    }

    state.handle_key(&PinsContext::default(), key(KeyCode::Down));
    state.handle_key(&PinsContext::default(), key(KeyCode::Char(' ')));
    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(editor.mode, PinCreateMode::NewVariation);
            assert_eq!(editor.mux_name.value(), "work");
        }
        other => panic!("unexpected editor: {other:?}"),
    }

    let outcome = state.handle_key(&PinsContext::default(), key(KeyCode::Enter));
    assert!(matches!(
        outcome,
        PinsOutcome::ApplyAndClose(crate::tui::Msg::PinCreate(_))
    ));
}

#[test]
fn create_form_backtab_moves_to_previous_field() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec![],
        vec![],
    );

    editor.handle_key(key(KeyCode::Down));
    assert_eq!(editor.render_cursor(), 2);
    editor.handle_key(key(KeyCode::BackTab));
    assert_eq!(editor.render_cursor(), 0);
    editor.handle_key(shift_key(KeyCode::BackTab));
    assert_eq!(editor.render_cursor(), PinCreateState::FIELD_STORE);
}

#[test]
fn create_form_forwards_home_and_end_to_active_text_field() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec![],
        vec![],
    );

    editor.handle_key(key(KeyCode::Home));
    editor.handle_key(key(KeyCode::Char('x')));
    assert_eq!(editor.name.value(), "xscratch");
    editor.handle_key(key(KeyCode::End));
    editor.handle_key(key(KeyCode::Char('y')));
    assert_eq!(editor.name.value(), "xscratchy");
}

#[test]
fn create_form_visible_window_marks_hidden_text_and_keeps_cursor_visible() {
    let display = pin_field_visible_window("abcdefghijklmnopqrstuvwxyz", 25, 12, true);
    assert_eq!(display.left_indicator, "< ");
    assert_eq!(display.right_indicator, "  ");
    assert_eq!(display.cursor_text, "z");
    let rendered = format!(
        "{}{}{}",
        display.before_cursor, display.cursor_text, display.after_cursor
    );
    assert!(rendered.len() <= 8);

    let display = pin_field_visible_window("abcdefghijklmnopqrstuvwxyz", 2, 12, true);
    assert_eq!(display.left_indicator, "  ");
    assert_eq!(display.right_indicator, " >");
    assert_eq!(display.cursor_text, "c");
}

#[test]
fn create_form_cycles_known_harness_choices_from_harness_field() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec!["claude-code".to_string(), "codex".to_string()],
        vec![],
    );

    editor.handle_key(key(KeyCode::Down));
    editor.handle_key(key(KeyCode::Down));
    assert_eq!(editor.render_cursor(), 3);
    editor.handle_key(key(KeyCode::Right));
    assert_eq!(editor.harness.value(), "claude-code");
    editor.handle_key(key(KeyCode::Left));
    assert_eq!(editor.harness.value(), "codex");
    editor.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(editor.harness.value(), "claude-code");
}

#[test]
fn create_form_harness_choices_keep_stable_column_when_focused() {
    let editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec!["claude-code".to_string(), "codex".to_string()],
        vec![],
    );

    let inactive = line_content(pin_create_harness_field(&editor, 0, 74));
    let active = line_content(pin_create_harness_field(&editor, 3, 74));
    assert_eq!(inactive.find("[claude-code]"), active.find("[claude-code]"));
    assert_eq!(inactive.find("[codex]"), active.find("[codex]"));
}

#[test]
fn create_form_launch_preview_uses_default_when_override_is_blank() {
    let editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec!["codex".to_string()],
        vec![],
    );

    let preview = editor.effective_launch_argv().expect("preview");
    assert_eq!(preview.source, LaunchArgvSource::Default);
    assert_eq!(preview.argv, vec!["codex".to_string()]);
    assert!(line_content(pin_create_launch_preview_field(&editor, 74)).contains("default: codex"));
}

#[test]
fn create_form_launch_preview_uses_shell_parsed_override() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec!["codex".to_string()],
        vec![],
    );

    editor.launch_argv =
        TextInputState::new(" launch argv ", "sandbox --name 'two words'".to_string());

    let preview = editor.effective_launch_argv().expect("preview");
    assert_eq!(preview.source, LaunchArgvSource::Override);
    assert_eq!(
        preview.argv,
        vec![
            "sandbox".to_string(),
            "--name".to_string(),
            "two words".to_string()
        ]
    );
    assert!(
        line_content(pin_create_launch_preview_field(&editor, 74))
            .contains("override: sandbox --name 'two words'")
    );
    assert_eq!(
        editor.request().expect("valid request").launch_argv,
        preview.argv
    );
}

#[test]
fn create_form_launch_option_toggles_codex_skip_permissions_into_argv() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec![],
        vec![],
    );

    move_to_create_field(&mut editor, PinCreateState::FIELD_LAUNCH_OPTIONS);
    editor.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(
        editor.launch_argv.value(),
        "codex --dangerously-bypass-approvals-and-sandbox"
    );
    assert_eq!(
        editor.request().expect("valid request").launch_argv,
        vec![
            "codex".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string()
        ]
    );

    editor.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(editor.launch_argv.value(), "");
    assert_eq!(
        editor.request().expect("valid request").launch_argv,
        Vec::<String>::new()
    );
}

#[test]
fn create_form_launch_option_toggles_claude_skip_permissions_into_argv() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "claude-code".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec![],
        vec![],
    );

    move_to_create_field(&mut editor, PinCreateState::FIELD_LAUNCH_OPTIONS);
    editor.handle_key(key(KeyCode::Char(' ')));

    assert_eq!(
        editor.launch_argv.value(),
        "claude --dangerously-skip-permissions"
    );
}

#[test]
fn create_form_launch_option_preserves_manual_argv_tokens() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec![],
        vec![],
    );
    editor.launch_argv = TextInputState::new(" launch argv ", "sandbox run codex".to_string());

    move_to_create_field(&mut editor, PinCreateState::FIELD_LAUNCH_OPTIONS);
    editor.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(
        editor.launch_argv.value(),
        "sandbox run codex --dangerously-bypass-approvals-and-sandbox"
    );
    editor.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(editor.launch_argv.value(), "sandbox run codex");
}

#[test]
fn create_form_switching_harness_removes_incompatible_option_flags() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec!["claude-code".to_string(), "codex".to_string()],
        vec![],
    );
    move_to_create_field(&mut editor, PinCreateState::FIELD_LAUNCH_OPTIONS);
    editor.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(
        editor.launch_argv.value(),
        "codex --dangerously-bypass-approvals-and-sandbox"
    );

    move_to_create_field(&mut editor, PinCreateState::FIELD_HARNESS);
    editor.handle_key(key(KeyCode::Left));
    assert_eq!(editor.harness.value(), "claude-code");
    assert_eq!(editor.launch_argv.value(), "");
}

#[test]
fn create_form_clearing_launch_override_returns_to_default() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec!["codex".to_string()],
        vec![],
    );

    editor.launch_argv = TextInputState::new(" launch argv ", "sandbox codex".to_string());
    assert_eq!(
        editor.effective_launch_argv().expect("preview").source,
        LaunchArgvSource::Override
    );
    editor.launch_argv = TextInputState::new(" launch argv ", String::new());
    let preview = editor.effective_launch_argv().expect("preview");
    assert_eq!(preview.source, LaunchArgvSource::Default);
    assert_eq!(preview.argv, vec!["codex".to_string()]);
    assert_eq!(
        editor.request().expect("valid request").launch_argv,
        Vec::<String>::new()
    );
}

#[test]
fn create_form_requires_launch_override_for_unknown_harness() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "custom-harness".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec!["custom-harness".to_string()],
        vec![],
    );

    assert_eq!(
        editor.request().expect_err("missing launch argv"),
        "pin create: launch argv is required for unknown harness `custom-harness`"
    );
    editor.launch_argv = TextInputState::new(" launch argv ", "custom run".to_string());
    assert_eq!(
        editor.request().expect("valid request").launch_argv,
        vec!["custom".to_string(), "run".to_string()]
    );
}

#[test]
fn create_form_cwd_tab_completes_ranked_path_candidate() {
    let mut editor = PinCreateState::new_with_options(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: String::new(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        PinCreateOptions {
            known_cwd_candidates: vec![
                PathCandidate::new("/workspace/low", "agent", 20),
                PathCandidate::new("/workspace/high", "selected", 100),
            ],
            ..PinCreateOptions::default()
        },
    );

    move_to_create_field(&mut editor, PinCreateState::FIELD_CWD);
    for ch in "/work".chars() {
        editor.handle_key(key(KeyCode::Char(ch)));
    }
    assert_eq!(editor.render_cursor(), PinCreateState::FIELD_CWD);
    assert_eq!(
        line_content(pin_create_path_omnibox_field(&editor, 2, 74)),
        "> cwd            /work    ! Tab: space/high              "
    );

    editor.handle_key(key(KeyCode::Tab));

    assert_eq!(editor.render_cursor(), PinCreateState::FIELD_CWD);
    assert_eq!(editor.cwd.value(), "/workspace/high");
}

#[test]
fn completion_remainder_shows_only_untyped_suffix() {
    assert_eq!(
        completion_remainder("/work", "/workspace/high").as_deref(),
        Some("space/high")
    );
    assert_eq!(
        completion_remainder("~/src/co", "~/src/conspectus").as_deref(),
        Some("nspectus")
    );
    assert_eq!(
        completion_remainder("co", "/home/op/src/conspectus").as_deref(),
        Some("nspectus")
    );
}

#[test]
fn create_form_cwd_tab_without_completion_stays_on_cwd_field() {
    let mut editor = PinCreateState::new_with_options(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/no/completion".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        PinCreateOptions::default(),
    );

    move_to_create_field(&mut editor, PinCreateState::FIELD_CWD);
    editor.handle_key(key(KeyCode::Tab));

    assert_eq!(editor.render_cursor(), PinCreateState::FIELD_CWD);
    assert_eq!(editor.cwd.value(), "/no/completion");
}

#[test]
fn create_form_expands_tilde_cwd_in_request() {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .expect("HOME must be set for tilde request test");
    let editor = PinCreateState::new_with_options(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "~/src/conspectus".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        PinCreateOptions::default(),
    );

    assert_eq!(
        editor.request().expect("valid request").cwd,
        home.join("src/conspectus").to_string_lossy()
    );
}

#[test]
fn create_form_rejects_existing_pin_id() {
    let editor = PinCreateState::new_with_guards(
        PinCreateDefaults {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest-new".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec![],
        vec![],
        vec!["ingest".to_string()],
        vec![],
        None,
    );

    assert_eq!(
        editor.request().expect_err("duplicate id should fail"),
        "pin create: pin `ingest` already exists"
    );
}

#[test]
fn create_form_rejects_non_adopt_mux_name_collisions() {
    let editor = PinCreateState::new_with_guards(
        PinCreateDefaults {
            id: "ingest-copy".to_string(),
            display_name: "Ingest Copy".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec![],
        vec![],
        vec![],
        vec!["ingest".to_string()],
        None,
    );

    assert_eq!(
        editor
            .request()
            .expect_err("duplicate mux name should fail"),
        "pin create: mux name `ingest` is already used; choose a new name or edit the existing pin"
    );
}

#[test]
fn create_form_rejects_adopt_when_selected_row_is_already_pinned() {
    let editor = PinCreateState::new_with_guards(
        PinCreateDefaults {
            id: "ingest-copy".to_string(),
            display_name: "Ingest Copy".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest".to_string(),
            mode: PinCreateMode::AdoptSelected,
        },
        Some(PinCreateDefaults {
            id: "ingest-copy".to_string(),
            display_name: "Ingest Copy".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest".to_string(),
            mode: PinCreateMode::AdoptSelected,
        }),
        vec![],
        vec!["ingest".to_string()],
        vec![],
        vec!["ingest".to_string()],
        Some("ingest".to_string()),
    );

    assert_eq!(
        editor
            .request()
            .expect_err("already-pinned adopt should fail"),
        "pin create: `ingest` is already pinned"
    );
}

#[test]
fn create_form_reports_invalid_launch_argv_quotes() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec!["codex".to_string()],
        vec![],
    );

    editor.launch_argv = TextInputState::new(" launch argv ", "codex 'unterminated".to_string());

    assert_eq!(
        editor.request().expect_err("invalid argv"),
        "pin create: launch argv has an unclosed `'` quote"
    );
    assert!(
        line_content(pin_create_launch_preview_field(&editor, 74)).contains("unclosed `'` quote")
    );
}

#[test]
fn create_form_allows_freeform_harness_with_warning() {
    let mut editor = PinCreateState::new(
        PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        None,
        vec!["codex".to_string()],
        vec![],
    );

    editor.handle_key(key(KeyCode::Down));
    editor.handle_key(key(KeyCode::Down));
    for _ in 0.."codex".len() {
        editor.handle_key(key(KeyCode::Backspace));
    }
    for ch in "custom-harness".chars() {
        editor.handle_key(key(KeyCode::Char(ch)));
    }
    editor.launch_argv = TextInputState::new(" launch argv ", "custom run".to_string());

    assert_eq!(
        editor.harness_warning().as_deref(),
        Some("custom harness `custom-harness` will be saved as typed")
    );
    assert_eq!(
        editor.request().expect("valid request").harness,
        "custom-harness"
    );
}

fn line_content(line: Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

#[test]
fn create_form_auto_checks_adopt_for_known_mux_name_collisions() {
    let ctx = PinsContext {
        pin_create_defaults: PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        pin_adopt_defaults: Some(PinCreateDefaults {
            id: "selected".to_string(),
            display_name: "selected".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "selected".to_string(),
            mode: PinCreateMode::AdoptSelected,
        }),
        known_mux_names: vec!["selected".to_string(), "busy-mux".to_string()],
        ..PinsContext::default()
    };
    let mut editor = PinCreateState::new(
        ctx.pin_create_defaults.clone(),
        ctx.pin_adopt_defaults.clone(),
        ctx.known_harness_keys.clone(),
        ctx.known_mux_names,
    );

    for _ in 0.."scratch".len() {
        editor.handle_key(key(KeyCode::Backspace));
    }
    for ch in "busy_mux".chars() {
        editor.handle_key(key(KeyCode::Char(ch)));
    }
    assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
    assert_eq!(editor.mux_name.value(), "busy-mux");
    assert_eq!(editor.mux_name_display(), "busy-mux");
    assert_eq!(
        editor
            .request()
            .expect("valid request")
            .adopt_source_mux_name
            .as_deref(),
        Some("busy-mux")
    );

    editor.handle_key(key(KeyCode::Char('x')));
    assert_eq!(editor.mode, PinCreateMode::NewVariation);
    assert_eq!(editor.name.value(), "busy_muxx");
    assert_eq!(editor.mux_name.value(), "busy-muxx");
    assert_eq!(
        editor
            .request()
            .expect("valid request")
            .adopt_source_mux_name,
        None
    );

    editor.handle_key(key(KeyCode::Down));
    editor.handle_key(key(KeyCode::Char(' ')));
    editor.handle_key(key(KeyCode::Up));
    editor.handle_key(key(KeyCode::Char('y')));
    assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
    assert_eq!(editor.name.value(), "busy_muxxy");
    assert_eq!(
        editor.mux_name_display(),
        "busy_muxxy (rename of: selected)"
    );

    for _ in 0.."busy_muxxy".len() {
        editor.handle_key(key(KeyCode::Backspace));
    }
    for ch in "busy-mux".chars() {
        editor.handle_key(key(KeyCode::Char(ch)));
    }
    assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
    assert_eq!(editor.mux_name_display(), "busy-mux");

    editor.handle_key(key(KeyCode::Char('z')));
    assert_eq!(editor.mode, PinCreateMode::NewVariation);
    assert_eq!(editor.name.value(), "busy-muxz");
}

#[test]
fn create_form_collision_tracking_uses_mux_name_not_primary_name() {
    let ctx = PinsContext {
        pin_create_defaults: PinCreateDefaults {
            id: "scratch".to_string(),
            display_name: "scratch".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "scratch".to_string(),
            mode: PinCreateMode::NewVariation,
        },
        pin_adopt_defaults: Some(PinCreateDefaults {
            id: "selected".to_string(),
            display_name: "selected".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "selected".to_string(),
            mode: PinCreateMode::AdoptSelected,
        }),
        known_mux_names: vec!["live-mux".to_string()],
        ..PinsContext::default()
    };
    let mut editor = PinCreateState::new(
        ctx.pin_create_defaults.clone(),
        ctx.pin_adopt_defaults.clone(),
        ctx.known_harness_keys.clone(),
        ctx.known_mux_names,
    );

    for _ in 0.."scratch".len() {
        editor.handle_key(key(KeyCode::Backspace));
    }
    for ch in "live_mux".chars() {
        editor.handle_key(key(KeyCode::Char(ch)));
    }
    assert_eq!(editor.name.value(), "live_mux");
    assert_eq!(editor.mux_name.value(), "live-mux");
    assert_eq!(editor.mode, PinCreateMode::AdoptSelected);

    editor.handle_key(key(KeyCode::Down));
    editor.handle_key(key(KeyCode::Char(' ')));
    for _ in 0..7 {
        editor.handle_key(key(KeyCode::Down));
    }
    for _ in 0.."live_mux".len() {
        editor.handle_key(key(KeyCode::Backspace));
    }
    for ch in "not-live".chars() {
        editor.handle_key(key(KeyCode::Char(ch)));
    }
    assert_eq!(editor.name.value(), "live_mux");
    assert_eq!(editor.mux_name.value(), "not-live");
    assert_eq!(editor.mode, PinCreateMode::NewVariation);

    for _ in 0.."not-live".len() {
        editor.handle_key(key(KeyCode::Backspace));
    }
    for ch in "live-mux".chars() {
        editor.handle_key(key(KeyCode::Char(ch)));
    }
    assert_eq!(editor.name.value(), "live_mux");
    assert_eq!(editor.mux_name.value(), "live-mux");
    assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
    assert_eq!(
        editor
            .request()
            .expect("valid request")
            .adopt_source_mux_name
            .as_deref(),
        Some("live-mux")
    );
}

#[test]
fn enter_on_edit_without_target_emits_placeholder() {
    let ctx = PinsContext::default();
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(2); // edit
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        PinsOutcome::ApplyAndStay(crate::tui::Msg::SetStatus(Some(pin_placeholder_status(
            "edit"
        ))))
    );
}

#[test]
fn enter_on_edit_with_target_opens_edit_form() {
    let ctx = PinsContext {
        pin_target: Some(pin_target()),
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(2); // edit
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, PinsOutcome::Continue);
    assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Edit(_))));
}

#[test]
fn edit_form_confirms_target_fields() {
    let ctx = PinsContext {
        pin_target: Some(pin_target()),
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(2); // edit
    state.handle_key(&ctx, key(KeyCode::Enter));

    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        PinsOutcome::ApplyAndClose(crate::tui::Msg::PinEdit(PinEditRequest {
            original_id: "ingest".to_string(),
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest-mux".to_string(),
            mux_socket: Some("scratch".to_string()),
            launch_argv: vec!["codex".to_string(), "--resume".to_string()],
            store_path: "/workspace/project/.conspectus.toml".to_string(),
        }))
    );
}

#[test]
fn edit_form_lets_operator_change_harness_and_cwd() {
    // H-PIN-EDIT: harness + cwd were unreachable in the edit form
    // before this wave (carried through from target as read-only).
    // Now they're position 2 and 3 in the field list; operators
    // can retype them and the resulting PinEditRequest carries the
    // new values.
    let ctx = PinsContext {
        pin_target: Some(pin_target()),
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(2); // edit
    state.handle_key(&ctx, key(KeyCode::Enter));

    // Advance cursor from id (0) → display (1) → harness (2).
    state.handle_key(&ctx, key(KeyCode::Down));
    state.handle_key(&ctx, key(KeyCode::Down));
    // Clear existing harness text and type a new value. TextInputState
    // handles Backspace one char at a time.
    for _ in 0.."codex".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for ch in "opencode".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(ch)));
    }
    // Advance to cwd (3) and rewrite it too.
    state.handle_key(&ctx, key(KeyCode::Down));
    for _ in 0.."/workspace/project".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for ch in "/new/root".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(ch)));
    }

    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    match outcome {
        PinsOutcome::ApplyAndClose(crate::tui::Msg::PinEdit(request)) => {
            assert_eq!(request.harness, "opencode");
            assert_eq!(request.cwd, "/new/root");
            // Untouched fields stay put.
            assert_eq!(request.id, "ingest");
            assert_eq!(request.display_name, "Ingest");
        }
        other => panic!("expected PinEdit outcome, got {other:?}"),
    }
}

#[test]
fn edit_form_requires_harness_and_cwd_to_be_non_empty() {
    // Blank harness → validation error, stay in form. Same for cwd.
    let ctx = PinsContext {
        pin_target: Some(pin_target()),
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(2); // edit
    state.handle_key(&ctx, key(KeyCode::Enter));

    // Advance to harness (position 2) and clear it entirely.
    state.handle_key(&ctx, key(KeyCode::Down));
    state.handle_key(&ctx, key(KeyCode::Down));
    for _ in 0.."codex".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }

    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, PinsOutcome::Continue);
    // The sub_editor stays open so the operator can fix the error.
    assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Edit(_))));
}

#[test]
fn edit_form_cancel_does_not_emit_action() {
    let ctx = PinsContext {
        pin_target: Some(pin_target()),
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(2); // edit
    state.handle_key(&ctx, key(KeyCode::Enter));

    let outcome = state.handle_key(&ctx, key(KeyCode::Esc));
    assert_eq!(outcome, PinsOutcome::Continue);
    assert!(state.sub_editor().is_none());
}

#[test]
fn enter_on_remove_with_target_opens_confirmation() {
    let ctx = PinsContext {
        pin_target: Some(pin_target()),
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(3); // remove
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, PinsOutcome::Continue);
    assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Remove(_))));
}

#[test]
fn remove_confirmation_emits_remove_action() {
    let ctx = PinsContext {
        pin_target: Some(pin_target()),
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(3); // remove
    state.handle_key(&ctx, key(KeyCode::Enter));

    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        PinsOutcome::ApplyAndClose(crate::tui::Msg::PinRemove(PinRemoveRequest {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            store_path: "/workspace/project/.conspectus.toml".to_string(),
        }))
    );
    assert!(state.sub_editor().is_none());
}

#[test]
fn enter_on_bind_opens_picker_for_ambiguous_options() {
    let ctx = PinsContext {
        pin_bind_options: vec![
            PinBindOption {
                pin_id: "ingest".to_string(),
                session_key: "a".to_string(),
                label: "codex:a".to_string(),
            },
            PinBindOption {
                pin_id: "ingest".to_string(),
                session_key: "b".to_string(),
                label: "codex:b".to_string(),
            },
        ],
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(4); // bind
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, PinsOutcome::Continue);
    assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Bind(_))));
}

#[test]
fn bind_picker_confirms_selected_session() {
    let ctx = PinsContext {
        pin_bind_options: vec![
            PinBindOption {
                pin_id: "ingest".to_string(),
                session_key: "a".to_string(),
                label: "codex:a".to_string(),
            },
            PinBindOption {
                pin_id: "ingest".to_string(),
                session_key: "b".to_string(),
                label: "codex:b".to_string(),
            },
        ],
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(4); // bind
    state.handle_key(&ctx, key(KeyCode::Enter));
    state.handle_key(&ctx, key(KeyCode::Down));

    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        PinsOutcome::ApplyAndClose(crate::tui::Msg::PinBind(PinBindRequest {
            pin_id: "ingest".to_string(),
            session_key: "b".to_string(),
        }))
    );
}

#[test]
fn bind_with_no_options_emits_placeholder() {
    let ctx = PinsContext::default();
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(4); // bind
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        PinsOutcome::ApplyAndStay(crate::tui::Msg::SetStatus(Some(pin_placeholder_status(
            "bind"
        ))))
    );
}

#[test]
fn create_form_confirms_defaults() {
    let ctx = PinsContext {
        pin_create_defaults: PinCreateDefaults {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest-mux".to_string(),
            ..PinCreateDefaults::default()
        },
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.handle_key(&ctx, key(KeyCode::Enter));

    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        PinsOutcome::ApplyAndClose(crate::tui::Msg::PinCreate(PinCreateRequest {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest-mux".to_string(),
            mux_socket: None,
            adopt_source_mux_name: None,
            launch_argv: Vec::new(),
            store: PinCreateStore::Auto,
        }))
    );
}

#[test]
fn create_form_name_drives_default_identity_fields() {
    let ctx = PinsContext::default();
    let mut state = PinsOverlayState::open_with_create(PinCreateDefaults::default());

    // Cursor starts on the primary name field. Replace the
    // default name; id/display/mux stay synchronized because the
    // operator has not edited the advanced identity fields.
    for _ in 0.."new pin".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for ch in "Client Sandbox".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(ch)));
    }

    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(editor.name.value(), "Client Sandbox");
            assert_eq!(editor.id.value(), "client-sandbox");
            assert_eq!(editor.display_name.value(), "Client Sandbox");
            assert_eq!(editor.mux_name.value(), "client-sandbox");
        }
        other => panic!("unexpected editor: {other:?}"),
    }
}

#[test]
fn create_form_preserves_explicit_identity_overrides_after_name_edit() {
    let ctx = PinsContext::default();
    let mut state = PinsOverlayState::open_with_create(PinCreateDefaults::default());

    // Move to mux.name, edit it, then return to name and change
    // the primary value. The explicit mux override must survive.
    for _ in 0..6 {
        state.handle_key(&ctx, key(KeyCode::Down));
    }
    for _ in 0.."new-pin".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for ch in "kept-mux".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(ch)));
    }
    for _ in 0..6 {
        state.handle_key(&ctx, key(KeyCode::Up));
    }
    for _ in 0.."new pin".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for ch in "Renamed Pin".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(ch)));
    }

    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(editor.id.value(), "renamed-pin");
            assert_eq!(editor.display_name.value(), "Renamed Pin");
            assert_eq!(editor.mux_name.value(), "kept-mux");
        }
        other => panic!("unexpected editor: {other:?}"),
    }
}

#[test]
fn create_form_cleared_identity_field_rejoins_name_derivation() {
    let ctx = PinsContext::default();
    let mut state = PinsOverlayState::open_with_create(PinCreateDefaults::default());

    // Override mux.name first.
    for _ in 0..6 {
        state.handle_key(&ctx, key(KeyCode::Down));
    }
    for _ in 0.."new-pin".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for ch in "kept-mux".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(ch)));
    }

    // Clearing the field entirely makes it derived again. The
    // next name edit should fill it from the new name.
    for _ in 0.."kept-mux".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for _ in 0..6 {
        state.handle_key(&ctx, key(KeyCode::Up));
    }
    for _ in 0.."new pin".len() {
        state.handle_key(&ctx, key(KeyCode::Backspace));
    }
    for ch in "Client Sandbox".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(ch)));
    }

    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(editor.mux_name.value(), "client-sandbox");
        }
        other => panic!("unexpected editor: {other:?}"),
    }
}

#[test]
fn create_form_keeps_validation_errors_open() {
    let ctx = PinsContext {
        pin_create_defaults: PinCreateDefaults {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            mux_name: "ingest".to_string(),
            ..PinCreateDefaults::default()
        },
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.handle_key(&ctx, key(KeyCode::Enter));

    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, PinsOutcome::Continue);
    match state.sub_editor() {
        Some(PinsSubEditor::Create(editor)) => {
            assert_eq!(
                editor.error.as_deref(),
                Some("pin create: harness is required")
            );
        }
        other => panic!("unexpected editor: {other:?}"),
    }
}

#[test]
fn open_with_create_skips_menu() {
    let defaults = PinCreateDefaults {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/workspace/project".to_string(),
        mux_name: "ingest-mux".to_string(),
        ..PinCreateDefaults::default()
    };
    let state = PinsOverlayState::open_with_create(defaults);
    assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Create(_))));
}

#[test]
fn open_with_bind_skips_menu_when_options_present() {
    let options = vec![PinBindOption {
        pin_id: "ingest".to_string(),
        session_key: "a".to_string(),
        label: "codex:a".to_string(),
    }];
    let state = PinsOverlayState::open_with_bind(options).expect("options present");
    assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Bind(_))));
}

#[test]
fn open_with_bind_returns_none_for_empty_options() {
    assert!(PinsOverlayState::open_with_bind(Vec::new()).is_none());
}

#[test]
fn open_with_rebind_opens_mux_only_form() {
    let state = PinsOverlayState::open_with_rebind(pin_target());
    assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Rebind(_))));
}

#[test]
fn rebind_form_preserves_unchanged_target_fields() {
    let target = pin_target();
    let mut state = PinRebindState::new(target.clone());
    // Enter without editing should round-trip the target's
    // mux fields and carry the rest through verbatim.
    let outcome = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match outcome {
        PinEditOutcome::Confirm(request) => {
            assert_eq!(request.original_id, target.id);
            assert_eq!(request.id, target.id);
            assert_eq!(request.display_name, target.display_name);
            assert_eq!(request.harness, target.harness);
            assert_eq!(request.cwd, target.cwd);
            assert_eq!(request.mux_name, target.mux_name);
            assert_eq!(request.mux_socket, target.mux_socket);
            assert_eq!(request.launch_argv, target.launch_argv);
            assert_eq!(request.store_path, target.store_path);
        }
        other => panic!("expected confirm, got {other:?}"),
    }
}

#[test]
fn menu_rebind_uses_mux_only_form() {
    // The Pins menu's `rebind` entry routes through the
    // narrower form, not the full edit form.
    let ctx = PinsContext {
        pin_target: Some(pin_target()),
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(5); // rebind
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, PinsOutcome::Continue);
    assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Rebind(_))));
}

// ----- launch menu entry -----

#[test]
fn enter_on_launch_without_target_emits_placeholder() {
    let ctx = PinsContext::default();
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(1); // launch
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        PinsOutcome::ApplyAndStay(crate::tui::Msg::SetStatus(Some(pin_placeholder_status(
            "launch"
        ))))
    );
}

#[test]
fn enter_on_launch_with_target_emits_launch_pin_action() {
    // The launch entry has no sub-editor — it commits the pin
    // id straight to the runtime so the TUI can suspend and
    // re-exec into `conspectus pin launch <id>`.
    let ctx = PinsContext {
        pin_target: Some(pin_target()),
        ..PinsContext::default()
    };
    let mut state = PinsOverlayState::new();
    state.cursor = PinsCursor::Action(1); // launch
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        PinsOutcome::ApplyAndClose(crate::tui::Msg::LaunchPinById("ingest".to_string()))
    );
    assert!(state.sub_editor().is_none());
}

#[test]
fn bind_picker_selected_last_option_scrolls_into_short_body() {
    let options: Vec<PinBindOption> = (0..8)
        .map(|idx| PinBindOption {
            pin_id: "ingest".to_string(),
            session_key: format!("session-{idx}"),
            label: format!("codex:session-{idx}"),
        })
        .collect();
    let state = PinBindState::new(options);
    let cursor_line = 1 + state.options.len() - 1;
    let content_height = 1 + state.options.len() + 2;
    let inner_height = 5;
    let offset = scroll_offset_for_cursor(Some(cursor_line), inner_height, content_height);
    assert!(offset > 0, "short bind picker should scroll");
    assert!(cursor_line >= offset as usize);
    assert!(cursor_line < offset as usize + inner_height);
}
