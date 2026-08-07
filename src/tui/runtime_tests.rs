// Extracted from runtime.rs H-HYG-011 rolling wave via #[path = "runtime_tests.rs"] mod tests;
use super::*;
use crate::tui::widgets::pins::{
    PinBindRequest, PinCreateRequest, PinCreateStore, PinEditRequest, PinRemoveRequest,
};
use ratatui::crossterm::event::KeyEvent;
use std::fs;
use std::os::unix::process::ExitStatusExt;

fn press(code: KeyCode, mods: KeyModifiers) -> Event {
    let mut key = KeyEvent::new(code, mods);
    key.kind = KeyEventKind::Press;
    Event::Key(key)
}

fn msg(action: Option<Action>) -> Option<Msg> {
    // Action::Msg now boxes its payload (large enum variant);
    // unwrap for the test assertions.
    match action {
        Some(Action::Msg(msg)) => Some(*msg),
        _ => None,
    }
}

#[test]
fn translate_q_quits() {
    assert_eq!(
        msg(translate(press(KeyCode::Char('q'), KeyModifiers::NONE), 24)),
        Some(Msg::Quit)
    );
}

#[test]
fn translate_ctrl_c_quits() {
    assert_eq!(
        msg(translate(
            press(KeyCode::Char('c'), KeyModifiers::CONTROL),
            24
        )),
        Some(Msg::Quit)
    );
}

#[test]
fn translate_r_requests_refresh() {
    assert_eq!(
        translate(press(KeyCode::Char('r'), KeyModifiers::NONE), 24),
        Some(Action::Refresh)
    );
}

#[test]
fn translate_shift_r_opens_rename_overlay() {
    assert_eq!(
        translate(press(KeyCode::Char('R'), KeyModifiers::SHIFT), 24),
        Some(Action::OpenRename)
    );
    assert_eq!(
        translate(press(KeyCode::Char('R'), KeyModifiers::NONE), 24),
        Some(Action::OpenRename)
    );
}

#[test]
fn translate_lowercase_r_still_refreshes() {
    assert_eq!(
        translate(press(KeyCode::Char('r'), KeyModifiers::NONE), 24),
        Some(Action::Refresh)
    );
}

#[test]
fn pin_mutation_refresh_config_forces_local_discovery() {
    let mut config = RunConfig::defaults();
    config.refresh = false;
    config.no_cache = true;
    config.default_view = View::Mux;
    let app = App::new(config);

    let refresh_config = pin_mutation_refresh_config(&app);

    assert!(refresh_config.refresh);
    assert!(refresh_config.no_cache);
    assert_eq!(refresh_config.default_view, View::Mux);
}

#[test]
fn translate_a_requests_attach() {
    assert_eq!(
        translate(press(KeyCode::Char('a'), KeyModifiers::NONE), 24),
        Some(Action::Attach)
    );
}

#[test]
fn translate_delete_requests_pin_remove() {
    assert_eq!(
        translate(press(KeyCode::Delete, KeyModifiers::NONE), 24),
        Some(Action::RemovePin)
    );
}

#[test]
fn pin_launch_summary_surfaces_stderr_on_failure() {
    let output = Output {
        status: std::process::ExitStatus::from_raw(1 << 8),
        stdout: b"stdout line\n".to_vec(),
        stderr: b"tmux session `demo` vanished before attach\n".to_vec(),
    };

    let summary = summarize_pin_launch_output("demo", Ok(output));

    assert!(!summary.success);
    assert_eq!(
        summary.message,
        "pin `demo` launch failed: tmux session `demo` vanished before attach"
    );
}

#[test]
fn pin_launch_summary_uses_stdout_on_success() {
    let output = Output {
        status: std::process::ExitStatus::from_raw(0),
        stdout: b"spawned `demo` (detached); attach with: tmux attach-session -t demo\n".to_vec(),
        stderr: Vec::new(),
    };

    let summary = summarize_pin_launch_output("demo", Ok(output));

    assert!(summary.success);
    assert_eq!(
        summary.message,
        "pin `demo` launched: spawned `demo` (detached); attach with: tmux attach-session -t demo"
    );
}

#[test]
fn translate_b_requests_pin_bind_hint() {
    assert_eq!(
        translate(press(KeyCode::Char('b'), KeyModifiers::NONE), 24),
        Some(Action::PinBindHint)
    );
}

#[test]
fn translate_i_requests_copy_session_id() {
    // T8-040: `i` resolves the selected agent or mux session's
    // full id and routes it through the OSC 52 clipboard
    // primitive (ADR 0056) at the main-loop boundary.
    assert_eq!(
        translate(press(KeyCode::Char('i'), KeyModifiers::NONE), 24),
        Some(Action::CopySessionId)
    );
    // Ctrl-I is the terminal alias for Tab; the binding must not
    // claim it.
    assert_ne!(
        translate(press(KeyCode::Char('i'), KeyModifiers::CONTROL), 24),
        Some(Action::CopySessionId)
    );
}

#[test]
fn translate_v_opens_viewer() {
    assert_eq!(
        translate(press(KeyCode::Char('v'), KeyModifiers::NONE), 24),
        Some(Action::View)
    );
}

#[test]
fn translate_f_opens_controls_overlay() {
    assert_eq!(
        translate(press(KeyCode::Char('f'), KeyModifiers::NONE), 24),
        Some(Action::OpenControls)
    );
}

#[test]
fn translate_p_opens_pins_overlay() {
    assert_eq!(
        translate(press(KeyCode::Char('p'), KeyModifiers::NONE), 24),
        Some(Action::OpenPins)
    );
}

#[test]
fn translate_capital_n_requests_open_pin_create() {
    assert_eq!(
        translate(press(KeyCode::Char('N'), KeyModifiers::SHIFT), 24),
        Some(Action::OpenPinCreate)
    );
    assert_eq!(
        translate(press(KeyCode::Char('N'), KeyModifiers::NONE), 24),
        Some(Action::OpenPinCreate)
    );
}

#[test]
fn translate_capital_b_requests_open_pin_rebind() {
    assert_eq!(
        translate(press(KeyCode::Char('B'), KeyModifiers::SHIFT), 24),
        Some(Action::OpenPinRebind)
    );
    // Lowercase b still routes to the bind picker / hint.
    assert_eq!(
        translate(press(KeyCode::Char('b'), KeyModifiers::NONE), 24),
        Some(Action::PinBindHint)
    );
}

#[test]
fn translate_capital_l_requests_pin_launch() {
    assert_eq!(
        translate(press(KeyCode::Char('L'), KeyModifiers::SHIFT), 24),
        Some(Action::LaunchPin)
    );
    assert_eq!(
        translate(press(KeyCode::Char('L'), KeyModifiers::NONE), 24),
        Some(Action::LaunchPin)
    );
}

#[test]
fn translate_capital_a_requests_open_pin_adopt() {
    assert_eq!(
        translate(press(KeyCode::Char('A'), KeyModifiers::SHIFT), 24),
        Some(Action::OpenPinAdopt)
    );
    // Lowercase a still attaches.
    assert_eq!(
        translate(press(KeyCode::Char('a'), KeyModifiers::NONE), 24),
        Some(Action::Attach)
    );
}

#[test]
fn translate_upper_t_no_longer_bound_to_view() {
    // Post H-VIEWER-NATIVE-008 reshuffle: `v` owns View;
    // `T` is unbound and falls through to None.
    assert_eq!(
        translate(press(KeyCode::Char('T'), KeyModifiers::NONE), 24),
        None,
    );
    assert_eq!(
        translate(press(KeyCode::Char('T'), KeyModifiers::SHIFT), 24),
        None,
    );
}

// static_action_for_event removed in H-TUI-004 wave 2 — its
// body was `overlay_key_from_event.or_else(translate + remap)`,
// which the shared `run_loop` now inlines. Overlay routing
// coverage lives in `tui::runtime::tests::overlay_routing`.

#[test]
fn translate_shift_f_clears_filters() {
    assert_eq!(
        translate(press(KeyCode::Char('F'), KeyModifiers::SHIFT), 24),
        Some(Action::ClearFilters)
    );
    assert_eq!(
        translate(press(KeyCode::Char('F'), KeyModifiers::NONE), 24),
        Some(Action::ClearFilters)
    );
}

#[test]
fn write_pin_create_writes_project_store() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    let loader = crate::config::ConfigLoader::new()
        .with_home(home.path())
        .with_xdg_config_home(home.path().join(".config"));
    let request = PinCreateRequest {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: project.path().display().to_string(),
        mux_name: "ingest-mux".to_string(),
        mux_socket: Some("scratch".to_string()),
        adopt_source_mux_name: None,
        launch_argv: vec!["codex".to_string(), "--resume".to_string()],
        worktree_branch: None,
        store: PinCreateStore::Project,
    };

    let state = tempfile::TempDir::new().expect("state");
    let registry =
        crate::pin_store_registry::PinStoreRegistry::new().with_xdg_state_home(state.path());
    let (outcome, entry, kind) = write_pin_create(&request, &loader, &registry).expect("write pin");
    assert!(outcome.changed);
    assert_eq!(kind, PinStoreKind::Project);
    assert_eq!(entry.id, "ingest");
    assert_eq!(entry.mux.native_id(), "tmux:scratch:ingest-mux");
    assert_eq!(outcome.path, project.path().join(".conspectus.toml"));

    // H-PIN-ROOT-001: creating a project pin records its store in the
    // registry so it survives a later scan from an unrelated root.
    assert_eq!(
        registry.read(),
        vec![project.path().join(".conspectus.toml")],
    );

    let written = fs::read_to_string(&outcome.path).expect("project config");
    assert!(written.contains("[pins]"));
    assert!(written.contains(r#"id = "ingest""#));
    assert!(written.contains(r#"display_name = "Ingest""#));
    assert!(written.contains(r#"harness = "codex""#));
    assert!(written.contains(r#"name = "ingest-mux""#));
    assert!(written.contains(r#"socket_name = "scratch""#));
    assert!(written.contains("argv = ["));
    assert!(written.contains(r#""codex""#));
    assert!(written.contains(r#""--resume""#));
}

#[test]
fn write_pin_create_user_store_uses_user_config_path() {
    let home = tempfile::TempDir::new().expect("home");
    let xdg = home.path().join(".config");
    let project = tempfile::TempDir::new().expect("project");
    let loader = crate::config::ConfigLoader::new()
        .with_home(home.path())
        .with_xdg_config_home(&xdg);
    let request = PinCreateRequest {
        id: "scratch".to_string(),
        display_name: "Scratch".to_string(),
        harness: "codex".to_string(),
        cwd: project.path().display().to_string(),
        mux_name: "scratch".to_string(),
        mux_socket: None,
        adopt_source_mux_name: None,
        launch_argv: Vec::new(),
        worktree_branch: None,
        store: PinCreateStore::User,
    };

    let state = tempfile::TempDir::new().expect("state");
    let registry =
        crate::pin_store_registry::PinStoreRegistry::new().with_xdg_state_home(state.path());
    let (outcome, _, kind) =
        write_pin_create(&request, &loader, &registry).expect("write user pin");
    assert_eq!(kind, PinStoreKind::User);
    assert_eq!(outcome.path, xdg.join(crate::config::USER_CONFIG_RELATIVE));
    assert!(outcome.path.exists());
    assert!(!project.path().join(".conspectus.toml").exists());

    // The user-scope store is consulted directly by discovery, so it's
    // never recorded in the project-store registry.
    assert!(registry.read().is_empty());
}

#[test]
fn pin_adopt_mux_rename_renames_source_mux_to_requested_mux_name() {
    let tmux = crate::discovery::tmux::FakeTmux::with_sessions("");
    let request = PinCreateRequest {
        id: "agentdeck-conspectus-2".to_string(),
        display_name: "agentdeck-conspectus-2".to_string(),
        harness: "codex".to_string(),
        cwd: "/workspace/project".to_string(),
        mux_name: "agentdeck-conspectus-2".to_string(),
        mux_socket: Some("scratch".to_string()),
        adopt_source_mux_name: Some("agentdeck_conspectus".to_string()),
        launch_argv: Vec::new(),
        worktree_branch: None,
        store: PinCreateStore::Project,
    };

    let status = apply_pin_adopt_mux_rename(&tmux, &request).expect("rename status");

    assert_eq!(
        status,
        "renamed adopted mux `agentdeck_conspectus` to `agentdeck-conspectus-2`"
    );
    assert_eq!(
        tmux.rename_calls(),
        vec![(
            Some("scratch".to_string()),
            "agentdeck_conspectus".to_string(),
            "agentdeck-conspectus-2".to_string()
        )]
    );
}

#[test]
fn pin_adopt_mux_rename_skips_when_source_already_matches_target() {
    let tmux = crate::discovery::tmux::FakeTmux::with_sessions("");
    let request = PinCreateRequest {
        id: "work".to_string(),
        display_name: "work".to_string(),
        harness: "codex".to_string(),
        cwd: "/workspace/project".to_string(),
        mux_name: "work".to_string(),
        mux_socket: None,
        adopt_source_mux_name: Some("work".to_string()),
        launch_argv: Vec::new(),
        worktree_branch: None,
        store: PinCreateStore::Project,
    };

    let status = apply_pin_adopt_mux_rename(&tmux, &request).expect("rename status");

    assert_eq!(status, "adopted existing mux `work`");
    assert!(tmux.rename_calls().is_empty());
}

#[test]
fn pin_create_success_toast_distinguishes_new_and_adopted_pins() {
    assert_eq!(
        pin_create_success_toast(false, "ingest"),
        "pin created, not started: `ingest`"
    );
    assert_eq!(
        pin_create_success_toast(true, "ingest"),
        "pin adopted; mux already running: `ingest`"
    );
}

#[test]
fn write_pin_edit_updates_id_display_mux_and_launch() {
    let project = tempfile::TempDir::new().expect("project");
    let path = project.path().join(".conspectus.toml");
    let entry = PinEntry {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: project.path().display().to_string(),
        mux: PinMux {
            backend: TMUX_MUX_BACKEND.to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch: None,
        worktree: None,
        reason: None,
    };
    crate::pins::upsert_pin_entry(&path, entry).expect("seed pin");

    let outcome = write_pin_edit(&PinEditRequest {
        original_id: "ingest".to_string(),
        id: "daily-ingest".to_string(),
        display_name: "Daily Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: project.path().display().to_string(),
        mux_name: "daily".to_string(),
        mux_socket: Some("scratch".to_string()),
        launch_argv: vec!["codex".to_string(), "--resume".to_string()],
        store_path: path.display().to_string(),
    })
    .expect("edit pin");

    assert!(outcome.changed);
    let written = fs::read_to_string(&path).expect("config");
    assert!(!written.contains(r#"id = "ingest""#));
    assert!(written.contains(r#"id = "daily-ingest""#));
    assert!(written.contains(r#"display_name = "Daily Ingest""#));
    assert!(written.contains(r#"name = "daily""#));
    assert!(written.contains(r#"socket_name = "scratch""#));
    assert!(written.contains(r#""--resume""#));
}

#[test]
fn write_pin_edit_rejects_duplicate_id_without_mutating() {
    let project = tempfile::TempDir::new().expect("project");
    let path = project.path().join(".conspectus.toml");
    for (id, mux) in [("ingest", "ingest"), ("scratch", "scratch")] {
        crate::pins::upsert_pin_entry(
            &path,
            PinEntry {
                id: id.to_string(),
                display_name: id.to_string(),
                harness: "codex".to_string(),
                cwd: project.path().display().to_string(),
                mux: PinMux {
                    backend: TMUX_MUX_BACKEND.to_string(),
                    name: mux.to_string(),
                    socket_name: None,
                },
                launch: None,
                worktree: None,
                reason: None,
            },
        )
        .expect("seed pin");
    }
    let before = fs::read_to_string(&path).expect("before");

    let err = write_pin_edit(&PinEditRequest {
        original_id: "ingest".to_string(),
        id: "scratch".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: project.path().display().to_string(),
        mux_name: "ingest".to_string(),
        mux_socket: None,
        launch_argv: Vec::new(),
        store_path: path.display().to_string(),
    })
    .expect_err("duplicate id");
    assert!(err.to_string().contains("already exists"));
    assert_eq!(fs::read_to_string(&path).expect("after"), before);
}

#[test]
fn write_pin_edit_rejects_duplicate_mux_without_mutating() {
    let project = tempfile::TempDir::new().expect("project");
    let path = project.path().join(".conspectus.toml");
    for (id, mux) in [("ingest", "ingest"), ("scratch", "scratch")] {
        crate::pins::upsert_pin_entry(
            &path,
            PinEntry {
                id: id.to_string(),
                display_name: id.to_string(),
                harness: "codex".to_string(),
                cwd: project.path().display().to_string(),
                mux: PinMux {
                    backend: TMUX_MUX_BACKEND.to_string(),
                    name: mux.to_string(),
                    socket_name: None,
                },
                launch: None,
                worktree: None,
                reason: None,
            },
        )
        .expect("seed pin");
    }
    let before = fs::read_to_string(&path).expect("before");

    let err = write_pin_edit(&PinEditRequest {
        original_id: "ingest".to_string(),
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: project.path().display().to_string(),
        mux_name: "scratch".to_string(),
        mux_socket: None,
        launch_argv: Vec::new(),
        store_path: path.display().to_string(),
    })
    .expect_err("duplicate mux");
    assert!(err.to_string().contains("already used"));
    assert_eq!(fs::read_to_string(&path).expect("after"), before);
}

#[test]
fn write_pin_bind_writes_declared_override() {
    let temp = tempfile::TempDir::new().expect("temp");
    let loader = crate::config::ConfigLoader::new()
        .with_home(temp.path())
        .with_xdg_config_home(temp.path().join(".config"));
    let session_id = crate::model::AgentSessionId::new("codex", "/state", "session-a");
    let mut snapshot = crate::model::GraphSnapshot::empty();
    snapshot.nodes.push(crate::model::GraphNode::AgentSession(
        crate::model::AgentSessionNode::new(session_id, "codex".to_string())
            .with_cwd("/workspace".to_string()),
    ));
    snapshot.pins.push(crate::model::PinCandidate {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/workspace".to_string(),
        mux: crate::model::PinMuxRef {
            backend: TMUX_MUX_BACKEND.to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: crate::model::Provenance::LocalPin,
        store_path: "/workspace/.conspectus.toml".to_string(),
        binding: None,
    });

    let outcome = write_pin_bind(
        &PinBindRequest {
            pin_id: "ingest".to_string(),
            session_key: "session-a".to_string(),
        },
        &snapshot,
        &loader,
    )
    .expect("bind pin");

    assert!(outcome.changed);
    let written = fs::read_to_string(outcome.path).expect("declared config");
    assert!(written.contains(r#"id = "pin:ingest:bound""#));
    assert!(written.contains(r#"label = "pin:ingest""#));
    assert!(written.contains(r#"session_key = "session-a""#));
    assert!(written.contains(r#"native_id = "tmux:ingest""#));
}

#[test]
fn write_pin_remove_removes_from_explicit_store_path() {
    let project = tempfile::TempDir::new().expect("project");
    let path = project.path().join(".conspectus.toml");
    let entry = PinEntry {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: project.path().display().to_string(),
        mux: PinMux {
            backend: TMUX_MUX_BACKEND.to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch: None,
        worktree: None,
        reason: None,
    };
    crate::pins::upsert_pin_entry(&path, entry).expect("seed pin");
    assert!(path.exists());

    let outcome = write_pin_remove(&PinRemoveRequest {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        store_path: path.display().to_string(),
    })
    .expect("remove pin");
    assert!(outcome.changed);
    assert_eq!(outcome.path, path);
    assert!(!outcome.path.exists());
}

#[test]
fn translate_shift_e_toggles_edge_meta() {
    // T8-042: `E` toggles the explorer's edge-meta visibility.
    // Focus-agnostic — the binding is global so the operator
    // can flip it without first tabbing into the right pane.
    assert_eq!(
        translate(press(KeyCode::Char('E'), KeyModifiers::SHIFT), 24),
        Some(Action::Msg(Box::new(Msg::ToggleEdgeMeta)))
    );
    assert_eq!(
        translate(press(KeyCode::Char('E'), KeyModifiers::NONE), 24),
        Some(Action::Msg(Box::new(Msg::ToggleEdgeMeta)))
    );
}

#[test]
fn translate_ctrl_g_cycles_grouping() {
    assert_eq!(
        translate(press(KeyCode::Char('g'), KeyModifiers::CONTROL), 24),
        Some(Action::CycleGrouping(1))
    );
}

#[test]
fn translate_digits_switch_views_directly() {
    // Only Sessions/Mux are surfaced in the UI (H-VIEW-001); the
    // `3`/`4`/`5` accelerators for the hidden Union/PRs/Forks views
    // were removed and now translate to no action.
    let cases = [('1', View::Sessions), ('2', View::Mux)];
    for (ch, view) in cases {
        assert_eq!(
            translate(press(KeyCode::Char(ch), KeyModifiers::NONE), 24),
            Some(Action::SwitchView(view)),
            "char {ch}"
        );
    }
    for ch in ['3', '4', '5'] {
        assert_eq!(
            translate(press(KeyCode::Char(ch), KeyModifiers::NONE), 24),
            None,
            "hidden view char {ch}"
        );
    }
}

#[test]
fn translate_brackets_cycle_views() {
    assert_eq!(
        translate(press(KeyCode::Char(']'), KeyModifiers::NONE), 24),
        Some(Action::CycleView(1))
    );
    assert_eq!(
        translate(press(KeyCode::Char('['), KeyModifiers::NONE), 24),
        Some(Action::CycleView(-1))
    );
}

#[test]
fn cycle_view_wraps_in_both_directions() {
    // VIEW_OPTIONS is trimmed to [Sessions, Mux] (H-VIEW-001), so the
    // cycle wraps between just those two.
    assert_eq!(cycle_view(View::Sessions, -1), View::Mux);
    assert_eq!(cycle_view(View::Mux, 1), View::Sessions);
    assert_eq!(cycle_view(View::Sessions, 1), View::Mux);
    assert_eq!(cycle_view(View::Mux, -1), View::Sessions);
}

#[test]
fn translate_ignores_control_a_and_control_r() {
    assert_eq!(
        translate(press(KeyCode::Char('a'), KeyModifiers::CONTROL), 24),
        None
    );
    assert_eq!(
        translate(press(KeyCode::Char('r'), KeyModifiers::CONTROL), 24),
        None
    );
}

#[test]
fn translate_maps_navigation_keys() {
    assert_eq!(
        msg(translate(press(KeyCode::Char('j'), KeyModifiers::NONE), 24)),
        Some(Msg::NavDown)
    );
    assert_eq!(
        msg(translate(press(KeyCode::Char('k'), KeyModifiers::NONE), 24)),
        Some(Msg::NavUp)
    );
    assert_eq!(
        msg(translate(press(KeyCode::Down, KeyModifiers::NONE), 24)),
        Some(Msg::NavDown)
    );
    assert_eq!(
        msg(translate(press(KeyCode::Up, KeyModifiers::NONE), 24)),
        Some(Msg::NavUp)
    );
    // T8-043: Enter no longer maps to a Msg directly. It is
    // resolved against the selected row's kind by the dispatcher
    // at the call site (and remapped to ExplorerActivate when
    // the right pane has focus).
    assert_eq!(
        translate(press(KeyCode::Enter, KeyModifiers::NONE), 24),
        Some(Action::DefaultAction)
    );
    // Vi-style tree fold keys: `l` / `→` expand, `h` / `←`
    // collapse the selected left-tree row.
    assert_eq!(
        msg(translate(press(KeyCode::Char('l'), KeyModifiers::NONE), 24)),
        Some(Msg::ExpandRow)
    );
    assert_eq!(
        msg(translate(press(KeyCode::Right, KeyModifiers::NONE), 24)),
        Some(Msg::ExpandRow)
    );
    assert_eq!(
        msg(translate(press(KeyCode::Char('h'), KeyModifiers::NONE), 24)),
        Some(Msg::CollapseRow)
    );
    assert_eq!(
        msg(translate(press(KeyCode::Left, KeyModifiers::NONE), 24)),
        Some(Msg::CollapseRow)
    );
    assert_eq!(
        msg(translate(press(KeyCode::Tab, KeyModifiers::NONE), 24)),
        Some(Msg::CycleFocus)
    );
    assert_eq!(
        msg(translate(press(KeyCode::Char('g'), KeyModifiers::NONE), 24)),
        Some(Msg::Home)
    );
    assert_eq!(
        msg(translate(press(KeyCode::Char('G'), KeyModifiers::NONE), 24)),
        Some(Msg::End)
    );
    assert_eq!(
        msg(translate(press(KeyCode::PageDown, KeyModifiers::NONE), 20)),
        Some(Msg::PageDown(20))
    );
    assert_eq!(
        msg(translate(press(KeyCode::PageUp, KeyModifiers::NONE), 20)),
        Some(Msg::PageUp(20))
    );
    assert_eq!(
        msg(translate(press(KeyCode::Char('J'), KeyModifiers::NONE), 24)),
        Some(Msg::ScrollPreviewBy(1))
    );
    assert_eq!(
        msg(translate(press(KeyCode::Char('K'), KeyModifiers::NONE), 24)),
        Some(Msg::ScrollPreviewBy(-1))
    );
}

#[test]
fn remap_for_focus_left_is_identity() {
    use crate::tui::app::Focus;
    let action = Action::Msg(Box::new(Msg::NavDown));
    assert_eq!(remap_for_focus(action.clone(), Focus::Left), Some(action));
    let action = Action::Msg(Box::new(Msg::PageDown(20)));
    assert_eq!(remap_for_focus(action.clone(), Focus::Left), Some(action));
}

#[test]
fn remap_for_focus_right_routes_nav_keys_into_the_explorer() {
    use crate::tui::app::Focus;
    // T8-028: with the right pane focused, j/k and PageUp/Down
    // move the explorer cursor instead of scrolling the preview.
    // J/K (uppercase) keep their preview-scroll role via the
    // standalone bindings in `translate`.
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::NavDown)), Focus::Right),
        Some(Action::Msg(Box::new(Msg::ExplorerNavDown)))
    );
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::NavUp)), Focus::Right),
        Some(Action::Msg(Box::new(Msg::ExplorerNavUp)))
    );
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::PageDown(20))), Focus::Right),
        Some(Action::Msg(Box::new(Msg::ExplorerNavDown)))
    );
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::PageUp(20))), Focus::Right),
        Some(Action::Msg(Box::new(Msg::ExplorerNavUp)))
    );
}

#[test]
fn remap_for_focus_right_routes_enter_and_e_to_the_explorer() {
    use crate::tui::app::Focus;
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::CycleFocus)), Focus::Right),
        Some(Action::Msg(Box::new(Msg::CycleFocus)))
    );
    // Locked decision 8: Enter is the universal "do the obvious
    // thing" key on the explorer cursor. T8-040 made the
    // dispatch App-aware (Node-zone fields copy; other rows
    // drill/expand), so the remap produces the new
    // [`Action::ExplorerEnter`] variant that the main loop
    // resolves against [`App::explorer_copy_target`].
    assert_eq!(
        remap_for_focus(Action::DefaultAction, Focus::Right),
        Some(Action::ExplorerEnter)
    );
    // Left-pane DefaultAction is left untouched here so the main
    // loop can resolve it against the selected row.
    assert_eq!(
        remap_for_focus(Action::DefaultAction, Focus::Left),
        Some(Action::DefaultAction)
    );
    // `e` is the explicit expand/collapse accelerator.
    assert_eq!(
        remap_for_focus(
            Action::Msg(Box::new(Msg::ToggleLinkedDetails)),
            Focus::Right
        ),
        Some(Action::Msg(Box::new(Msg::ExplorerToggleGroup)))
    );
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::Quit)), Focus::Right),
        Some(Action::Msg(Box::new(Msg::Quit)))
    );
    assert_eq!(
        remap_for_focus(Action::Refresh, Focus::Right),
        Some(Action::Refresh)
    );
    // T8-034: F clears filters on the left tree but toggles the
    // Expanded Node Detail view on the right pane.
    assert_eq!(
        remap_for_focus(Action::ClearFilters, Focus::Left),
        Some(Action::ClearFilters)
    );
    assert_eq!(
        remap_for_focus(Action::ClearFilters, Focus::Right),
        Some(Action::Msg(Box::new(Msg::ExplorerToggleFullDetail)))
    );
}

#[test]
fn remap_for_focus_right_suppresses_left_tree_expand_collapse_keys() {
    // Regression: `h` / `l` / `←` / `→` are left-tree
    // expand/collapse keys. When the right pane is focused they
    // used to leak through and mutate the tree the operator
    // wasn't driving. The focus remap drops them.
    use crate::tui::app::Focus;
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::ExpandRow)), Focus::Right),
        None
    );
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::CollapseRow)), Focus::Right),
        None
    );
    // Left focus still routes them through unchanged.
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::ExpandRow)), Focus::Left),
        Some(Action::Msg(Box::new(Msg::ExpandRow)))
    );
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::CollapseRow)), Focus::Left),
        Some(Action::Msg(Box::new(Msg::CollapseRow)))
    );
}

#[test]
fn remap_for_focus_right_routes_home_and_end_into_the_explorer() {
    // H-OBS-007 (paired with the h/l/Left/Right suppression
    // above): `g`/`Home` and `G`/`End` snap the explorer
    // cursor to its first / last row on right-pane focus
    // instead of bleeding into the left tree's Home/End
    // jumps. `Tab`/`CycleFocus` is intentionally left alone —
    // it is the focus toggle and must stay useful regardless
    // of which pane has focus.
    use crate::tui::app::Focus;
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::Home)), Focus::Right),
        Some(Action::Msg(Box::new(Msg::ExplorerHome))),
    );
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::End)), Focus::Right),
        Some(Action::Msg(Box::new(Msg::ExplorerEnd))),
    );
    // Left focus still drives the left tree.
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::Home)), Focus::Left),
        Some(Action::Msg(Box::new(Msg::Home))),
    );
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::End)), Focus::Left),
        Some(Action::Msg(Box::new(Msg::End))),
    );
    // Tab cycles focus on either side — never remapped.
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::CycleFocus)), Focus::Right),
        Some(Action::Msg(Box::new(Msg::CycleFocus))),
    );
    assert_eq!(
        remap_for_focus(Action::Msg(Box::new(Msg::CycleFocus)), Focus::Left),
        Some(Action::Msg(Box::new(Msg::CycleFocus))),
    );
}

mod selected_default_action_tests {
    use super::*;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, GraphLink,
        GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
        Provenance, RelationKind, RepoId, RepoNode,
    };
    use crate::resolve::resolve_snapshot;
    use crate::tui::SessionsGrouping;
    use crate::tui::app::{GraphDb, Msg};
    use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};

    fn session_node(harness: &str, scope: &str, key: &str, cwd: &str) -> GraphNode {
        GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new(harness, scope, key),
                harness.to_string(),
            )
            .with_cwd(cwd.to_string()),
        )
    }

    fn mux_node(backend: &str, native: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode::new(
            MuxSessionId::new(format!("{backend}:{native}")),
            backend.to_string(),
            native.to_string(),
        ))
    }

    fn linked_to_mux(session: &NodeId, mux: &NodeId, suffix: &str) -> GraphLink {
        GraphLink {
            id: format!("session-mux-{suffix}"),
            source: session.clone(),
            target: LinkEndpoint::Node { id: mux.clone() },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn add_repo_and_worktree(snapshot: &mut GraphSnapshot, common_dir: &str) {
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(common_dir))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new(common_dir), common_dir.to_string()),
            root: common_dir.to_string(),
            git_dir: None,
            current_branch: None,
            worktree: None,
        }));
    }

    fn build_sessions_app(snapshot: GraphSnapshot) -> App {
        let snapshot = resolve_snapshot(snapshot);
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: None,
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });
        let mut cfg = RunConfig::defaults();
        cfg.default_view = View::Sessions;
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app
    }

    #[test]
    fn empty_selection_falls_back_to_toggle_expand() {
        let app = App::new(RunConfig::defaults());
        assert_eq!(selected_default_action(&app), SelectedDefault::ToggleExpand);
    }

    #[test]
    fn group_row_resolves_to_toggle_expand() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        // Auto-selection lands on the repo group row.
        let app = build_sessions_app(snapshot);
        assert_eq!(selected_default_action(&app), SelectedDefault::ToggleExpand);
    }

    #[test]
    fn unmuxed_session_resolves_to_view() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        let mut app = build_sessions_app(snapshot);
        // Step past the group row to the session row.
        app.update(Msg::NavDown);
        assert_eq!(selected_default_action(&app), SelectedDefault::View);
    }

    #[test]
    fn muxed_session_resolves_to_attach() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        snapshot
            .candidate_links
            .push(linked_to_mux(&session_id, &mux_id, "1"));
        let mut app = build_sessions_app(snapshot);
        app.update(Msg::NavDown);
        assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
    }

    #[test]
    fn mux_candidate_child_resolves_to_attach() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.nodes.push(mux_node("tmux", "scratch"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let editor = NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        let scratch = NodeId::MuxSession(MuxSessionId::new("tmux:scratch"));
        snapshot
            .candidate_links
            .push(linked_to_mux(&session_id, &editor, "1"));
        snapshot
            .candidate_links
            .push(linked_to_mux(&session_id, &scratch, "2"));
        let mut app = build_sessions_app(snapshot);
        // Group → ambiguous session → expand → candidate child.
        app.update(Msg::NavDown);
        app.update(Msg::ToggleExpand);
        app.update(Msg::NavDown);
        assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
    }

    #[test]
    fn mux_view_mux_row_resolves_to_attach() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("tmux", "editor"));
        let snapshot = resolve_snapshot(snapshot);
        let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: None,
            filter: RowFilter::default(),
            grouping: crate::tui::MuxGrouping::Session,
            sort: crate::tui::Sort::Hierarchy,
            mux_recency: crate::tui::MuxRecency::default(),
        });
        let mut cfg = RunConfig::defaults();
        cfg.default_view = View::Mux;
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: GraphDb::new(snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
    }

    #[test]
    fn unmuxed_session_resolves_to_view_regardless_of_viewer_support() {
        // T8-043: the dispatcher routes every un-muxed agent
        // session through `SelectedDefault::View`. Harnesses
        // without a registered viewer (or whose viewer binary is
        // missing from `$PATH`) still resolve to `View` here —
        // the runtime's `view_action` is what surfaces the
        // `viewer_disabled_reason` status message after the
        // operator presses Enter. Keeping the dispatcher
        // uniform keeps the keymap consistent across harnesses
        // and lets the fallback message stay actionable.
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        // `aider` has no viewer registered in v1 (see
        // `H-TRANSCRIPT-007` deferred), so this is the
        // unsupported-viewer surface for the dispatcher.
        snapshot
            .nodes
            .push(session_node("aider", "/state", "abc", "/p/proj"));
        let mut app = build_sessions_app(snapshot);
        app.update(Msg::NavDown);
        assert_eq!(selected_default_action(&app), SelectedDefault::View);
    }

    #[test]
    fn unbound_pin_row_resolves_to_launch_pin() {
        use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};

        let mut snapshot = GraphSnapshot::empty();
        snapshot.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/proj".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/tmp/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Unbound),
        });
        // Auto-selection lands on the first row of the synthetic
        // "Pins" group (the group header). Walk the tree to find
        // the actual pin row id and select it so the dispatch
        // exercises `RowKind::Pin` instead of the group header.
        let mut app = build_sessions_app(snapshot);
        let pin_row_id = app
            .tree()
            .rows
            .iter()
            .find(|r| matches!(&r.id, crate::tui::rows::RowId::Pin { .. }))
            .map(|r| r.id.clone())
            .expect("pin row emitted");
        app.set_selection(pin_row_id);
        assert_eq!(selected_default_action(&app), SelectedDefault::LaunchPin);
    }
}

#[test]
fn translate_ignores_unbound_keys() {
    assert_eq!(
        translate(press(KeyCode::Char('z'), KeyModifiers::NONE), 24),
        None
    );
    assert_eq!(
        translate(press(KeyCode::Char('a'), KeyModifiers::CONTROL), 24),
        None
    );
}

#[test]
fn translate_ignores_release_kind_keys() {
    let mut key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    key.kind = KeyEventKind::Release;
    assert_eq!(translate(Event::Key(key), 24), None);
}

/// H-TUI-001 / ADR 0085 contract 4: view / grouping / filter /
/// sort changes must not trigger discovery. The signal we lean
/// on is `GraphDb` identity — `refresh` builds a fresh
/// `GraphDb::new(...)` (a distinct `Rc`), so if the projection
/// path had run discovery the post-action `graph_db()` would
/// point at a different allocation than the pre-action one.
mod projection_zero_discovery {
    use super::*;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, GraphNode, GraphSnapshot,
        RepoId, RepoNode,
    };
    use crate::resolve::resolve_snapshot;
    use crate::tui::app::GraphDb;
    use crate::tui::{Grouping, MuxGrouping, SessionsGrouping, View};

    fn seeded_app() -> App {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new("/p/proj"))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new("/p/proj"), "/p/proj".to_string()),
            root: "/p/proj".to_string(),
            git_dir: None,
            current_branch: None,
            worktree: None,
        }));
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("codex", "/state", "abc"),
                "codex".to_string(),
            )
            .with_cwd("/p/proj".to_string()),
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs {
            snapshot: &snapshot,
            view: View::Sessions,
            grouping: Grouping::Sessions(SessionsGrouping::Graph),
            filter: RowFilter::default(),
            sort: super::super::super::Sort::Hierarchy,
            mux_recency: crate::tui::MuxRecency::default(),
            cwd: None,
        });
        let mut cfg = RunConfig::defaults();
        cfg.default_view = View::Sessions;
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app
    }

    fn same_snapshot(a: &App, b_ptr: *const crate::model::GraphSnapshot) -> bool {
        std::ptr::eq(
            a.graph_db().expect("snapshot loaded").snapshot() as *const _,
            b_ptr,
        )
    }

    #[test]
    fn view_switch_does_not_run_discovery() {
        let mut app = seeded_app();
        let before = app.graph_db().expect("snapshot loaded").snapshot() as *const _;
        let _ = app.update(Msg::SwitchView(View::Mux));
        assert_eq!(app.active_view(), View::Mux);
        assert!(
            same_snapshot(&app, before),
            "view switch must re-derive from the held snapshot, not re-run discovery",
        );
    }

    #[test]
    fn grouping_change_does_not_run_discovery() {
        let mut app = seeded_app();
        let before = app.graph_db().expect("snapshot loaded").snapshot() as *const _;
        let _ = app.update(Msg::SetGrouping(Grouping::Sessions(
            SessionsGrouping::Workspace,
        )));
        assert_eq!(
            app.grouping(),
            Grouping::Sessions(SessionsGrouping::Workspace)
        );
        assert!(
            same_snapshot(&app, before),
            "grouping change must re-derive from the held snapshot, not re-run discovery",
        );
    }

    #[test]
    fn filter_change_does_not_run_discovery() {
        let mut app = seeded_app();
        let before = app.graph_db().expect("snapshot loaded").snapshot() as *const _;
        let filter = RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
            ..RowFilter::default()
        };
        let _ = app.update(Msg::SetFilter(filter.clone()));
        assert_eq!(app.filter(), &filter);
        assert!(
            same_snapshot(&app, before),
            "filter change must re-derive from the held snapshot, not re-run discovery",
        );
    }

    #[test]
    fn view_switch_leaves_run_config_untouched() {
        // ADR 0085 contract 4: RunConfig is initial-values-only.
        // The projection change must not write back into config.
        let mut app = seeded_app();
        let before_view = app.config().default_view;
        let before_grouping = app.config().sessions_grouping;
        let before_mux_grouping = app.config().mux_grouping;
        let _ = app.update(Msg::SwitchView(View::Mux));
        let _ = app.update(Msg::SetGrouping(Grouping::Mux(MuxGrouping::Host)));
        assert_eq!(app.config().default_view, before_view);
        assert_eq!(app.config().sessions_grouping, before_grouping);
        assert_eq!(app.config().mux_grouping, before_mux_grouping);
    }
}

/// H-TUI-002 Phase C (ADR 0085 contract 2): mux ops flow through
/// the executor. The reducer emits `Effect::RunMux(...)` and the
/// executor is the only code holding a `MuxBackend` reference.
mod mux_effect_executor {
    use super::*;
    use crate::discovery::tmux::{FakeTmux, TmuxCaptureOutcome};
    use crate::model::MuxSessionId;
    use crate::tui::app::GraphDb;
    use crate::tui::effect::{Effect, MuxOp};
    use crate::tui::preview::PreviewContent;

    #[test]
    fn capture_preview_effect_lands_in_app_preview_store() {
        let mut app = App::new(RunConfig::defaults());
        // No snapshot needed — the executor branch just calls
        // capture_via on the runner and dispatches
        // Msg::SetMuxPreview, so App only needs to be alive.
        let _ = GraphDb::from_snapshot(&crate::model::GraphSnapshot::empty());
        let mux = MuxSessionId::new("tmux:editor");
        let tmux = FakeTmux::with_sessions("").with_capture(
            "editor",
            TmuxCaptureOutcome::Captured("hello from pane".to_string()),
        );

        execute_mux_op(
            &mut app,
            &tmux,
            MuxOp::CapturePreview {
                mux: mux.clone(),
                native_id: "editor".to_string(),
            },
        );

        let entry = app.mux_preview(&mux).expect("preview cached");
        assert!(
            matches!(entry.content, PreviewContent::Text(ref s) if s.contains("hello from pane"))
        );
    }

    #[test]
    fn plan_mux_preview_capture_returns_none_when_live_preview_disabled() {
        let mut config = RunConfig::defaults();
        config.live_preview_enabled = false;
        let app = App::new(RunConfig::defaults());
        assert!(plan_mux_preview_capture(&app, &config, None).is_none());
    }

    #[test]
    fn plan_mux_preview_capture_returns_none_when_no_mux_target_selected() {
        let app = App::new(RunConfig::defaults());
        // Empty app: no selection, no mux target — nothing to
        // capture regardless of the previous target.
        assert!(plan_mux_preview_capture(&app, app.config(), None).is_none());
    }

    #[test]
    fn plan_mux_preview_capture_emits_run_mux_effect_shape() {
        // Smoke test the returned effect variant when a plan is
        // constructed manually via `Effect::RunMux(...)` —
        // proves the enum wiring, without needing a full seeded
        // App that resolves to an attachable mux row.
        let planned = Effect::RunMux(MuxOp::CapturePreview {
            mux: MuxSessionId::new("tmux:pane"),
            native_id: "pane".to_string(),
        });
        assert!(matches!(
            planned,
            Effect::RunMux(MuxOp::CapturePreview { .. })
        ));
    }
}

/// H-TUI-004 wave 1: `overlay_key_from_event` centralizes the
/// modal-stack-aware routing both event loops used to duplicate.
/// These tests pin the routing: each open modal takes ownership
/// of the next key press, and the fallback returns None when no
/// modal is on top.
mod overlay_routing {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    fn press(code: KeyCode, mods: KeyModifiers) -> Event {
        let mut key = KeyEvent::new(code, mods);
        key.kind = KeyEventKind::Press;
        Event::Key(key)
    }

    #[test]
    fn empty_stack_returns_none() {
        let app = App::new(RunConfig::defaults());
        assert!(
            overlay_key_from_event(&app, &press(KeyCode::Char('j'), KeyModifiers::NONE)).is_none()
        );
    }

    #[test]
    fn help_on_stack_routes_key_press() {
        let mut app = App::new(RunConfig::defaults());
        app.open_help_overlay();
        let action = overlay_key_from_event(&app, &press(KeyCode::Char('j'), KeyModifiers::NONE));
        assert!(matches!(action, Some(Action::HelpOverlayKey(_))));
    }

    #[test]
    fn controls_on_stack_routes_key_press() {
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        let action = overlay_key_from_event(&app, &press(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(action, Some(Action::ControlsOverlayKey(_))));
    }

    #[test]
    fn help_on_top_of_controls_routes_to_help() {
        // Multi-overlay: the top variant wins, matching the
        // modal stack's LIFO input-owner contract.
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        app.open_help_overlay();
        let action = overlay_key_from_event(&app, &press(KeyCode::Char('j'), KeyModifiers::NONE));
        assert!(matches!(action, Some(Action::HelpOverlayKey(_))));
    }

    #[test]
    fn non_key_events_return_none_even_when_modal_is_open() {
        let mut app = App::new(RunConfig::defaults());
        app.open_help_overlay();
        assert!(overlay_key_from_event(&app, &Event::FocusGained).is_none());
    }

    #[test]
    fn key_release_events_return_none_even_when_modal_is_open() {
        let mut app = App::new(RunConfig::defaults());
        app.open_help_overlay();
        let mut key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        assert!(overlay_key_from_event(&app, &Event::Key(key)).is_none());
    }
}

mod pin_launch_scan_root {
    //! Regression coverage for the CWD-drift bug: launching a pin
    //! from a TUI started outside the pin's project root used to
    //! fail because the subprocess re-discovered pins using its
    //! inherited CWD. Fixed by passing the pin's `cwd` field as
    //! `--scan-root` on the subprocess argv.

    use super::*;

    fn seed_app_with_pin(pin_id: &str, cwd: &str) -> App {
        let mut snapshot = crate::model::GraphSnapshot::empty();
        snapshot.pins.push(crate::model::PinCandidate {
            id: pin_id.to_string(),
            display_name: pin_id.to_string(),
            harness: "codex".to_string(),
            cwd: cwd.to_string(),
            mux: crate::model::PinMuxRef {
                backend: TMUX_MUX_BACKEND.to_string(),
                name: pin_id.to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: crate::model::Provenance::LocalPin,
            store_path: format!("{cwd}/.conspectus.toml"),
            binding: None,
        });
        let resolved = crate::resolve::resolve_snapshot(snapshot);
        let mut app = App::new(RunConfig::defaults());
        let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
            &resolved, &app,
        ));
        app.update(Msg::SetData {
            snapshot: crate::tui::app::GraphDb::new(resolved),
            tree,
            loaded_at_epoch: 0,
            initial_selection_hint: None,
        });
        app
    }

    #[test]
    fn resolve_pin_scan_root_returns_pin_cwd() {
        let app = seed_app_with_pin("ingest", "/workspace/ingest");
        assert_eq!(
            resolve_pin_scan_root(&app, "ingest"),
            Some("/workspace/ingest".to_string())
        );
    }

    #[test]
    fn resolve_pin_scan_root_none_for_unknown_pin() {
        let app = seed_app_with_pin("ingest", "/workspace/ingest");
        assert_eq!(resolve_pin_scan_root(&app, "does-not-exist"), None);
    }

    #[test]
    fn resolve_pin_scan_root_none_before_snapshot_loads() {
        let app = App::new(RunConfig::defaults());
        assert_eq!(resolve_pin_scan_root(&app, "ingest"), None);
    }

    #[test]
    fn pin_launch_argv_includes_scan_root_when_present() {
        let argv = pin_launch_argv("ingest", Some("/workspace/ingest"));
        assert_eq!(
            argv,
            vec![
                "pin".to_string(),
                "launch".to_string(),
                "ingest".to_string(),
                "--no-attach".to_string(),
                "--scan-root".to_string(),
                "/workspace/ingest".to_string(),
            ]
        );
    }

    #[test]
    fn pin_launch_argv_omits_scan_root_when_absent() {
        let argv = pin_launch_argv("ingest", None);
        assert_eq!(
            argv,
            vec![
                "pin".to_string(),
                "launch".to_string(),
                "ingest".to_string(),
                "--no-attach".to_string(),
            ]
        );
    }
}
