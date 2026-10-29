use super::*;

/// Default reflow threshold used by the `pane_rect` layout tests.
const DEFAULT: u16 = crate::config::DEFAULT_NARROW_LAYOUT_THRESHOLD;

#[test]
fn a_bad_key_script_fails_the_snapshot_with_the_reason() {
    let error = run(
        RunConfig::defaults(),
        SnapshotConfig {
            width: 10,
            height: 4,
            keys: "j<Bogus>".into(),
            pane: SnapshotPane::All,
            fixture: Some(std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/showcase.json"
            ))),
            export_fixture: None,
            fixture_root: None,
            now: None,
            messages: Vec::new(),
            toast: None,
            status: None,
        },
    )
    .expect_err("an unknown key name is an error");
    assert_eq!(error.to_string(), "unknown key name `<Bogus>`");
}

#[test]
fn pane_rect_returns_full_area_for_all() {
    let area = Rect::new(0, 0, 160, 40);
    assert_eq!(pane_rect(area, SnapshotPane::All, DEFAULT), area);
}

#[test]
fn pane_rect_carves_header_and_status_as_single_rows() {
    let area = Rect::new(0, 0, 160, 40);
    let header = pane_rect(area, SnapshotPane::Header, DEFAULT);
    assert_eq!(header.height, 1);
    assert_eq!(header.y, 0);
    let status = pane_rect(area, SnapshotPane::Status, DEFAULT);
    assert_eq!(status.height, 1);
    assert_eq!(status.y, 39);
}

#[test]
fn pane_rect_splits_body_horizontally_when_wide() {
    let area = Rect::new(0, 0, 160, 40);
    let left = pane_rect(area, SnapshotPane::Left, DEFAULT);
    let right = pane_rect(area, SnapshotPane::Right, DEFAULT);
    assert_eq!(left.x, 0);
    assert!(left.width > 0);
    assert!(right.x >= left.width);
    assert_eq!(left.y, 1);
    assert_eq!(left.height, 38);
}

#[test]
fn fixture_round_trip_preserves_empty_snapshot() {
    // Empty graph serializes to a minimal JSON and deserializes
    // back to the same value; covers the simplest fixture shape
    // an agent might hand-craft.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("empty.json");
    let snapshot = GraphSnapshot::empty();
    write_fixture(&path, &snapshot).expect("write");
    let loaded = load_fixture(&path).expect("load");
    assert_eq!(loaded, snapshot);
}

#[test]
fn fixture_load_error_reports_path() {
    let err =
        load_fixture(std::path::Path::new("/no/such/file.json")).expect_err("missing file errors");
    assert!(
        err.to_string().contains("/no/such/file.json"),
        "error should mention the failing path: {err}",
    );
}

#[test]
fn pane_rect_splits_body_vertically_when_narrow() {
    // Below the default narrow_layout_threshold (100): body splits
    // top/bottom.
    let area = Rect::new(0, 0, 60, 40);
    let left = pane_rect(area, SnapshotPane::Left, DEFAULT);
    let right = pane_rect(area, SnapshotPane::Right, DEFAULT);
    assert_eq!(left.x, 0);
    assert_eq!(right.x, 0);
    assert_eq!(left.width, 60);
    assert!(right.y > left.y);
}

#[test]
fn pane_rect_split_direction_follows_configured_threshold() {
    // The reflow breakpoint is configurable. A 60-col
    // body stacks vertically at the default threshold (100) but stays
    // side-by-side once the threshold is lowered below the width.
    let area = Rect::new(0, 0, 60, 40);

    let stacked = pane_rect(area, SnapshotPane::Right, DEFAULT);
    assert!(stacked.y > 1, "default threshold stacks a 60-col body");

    let side_by_side = pane_rect(area, SnapshotPane::Right, 50);
    assert_eq!(side_by_side.y, 1, "lowered threshold keeps side-by-side");
    assert!(side_by_side.x > 0, "right pane sits beside the left pane");
}

/// App loaded with one codex session attached to one tmux mux, on
/// the default sessions view.
fn session_mux_app() -> (App, RunConfig) {
    use crate::model::{
        AgentSessionId, AgentSessionNode, GraphLink, GraphNode, LinkEndpoint, MuxSessionId,
        MuxSessionNode, NodeId, Provenance, RelationKind,
    };
    let session = AgentSessionId::new("codex", "/state", "abc");
    let mux = MuxSessionId::new("editor");
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::AgentSession(AgentSessionNode::new(
            session.clone(),
            "codex".to_string(),
        )));
    snapshot
        .nodes
        .push(GraphNode::MuxSession(MuxSessionNode::new(
            mux.clone(),
            "tmux".to_string(),
            "editor".to_string(),
        )));
    snapshot.candidate_links.push(GraphLink::new(
        "session-mux",
        NodeId::AgentSession(session),
        LinkEndpoint::Node {
            id: NodeId::MuxSession(mux),
        },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    ));
    let config = RunConfig::defaults();
    let mut app = App::new(config.clone());
    refresh_from_snapshot(&mut app, &config, snapshot);
    (app, config)
}

fn drive(app: &mut App, config: &RunConfig, script: &str) -> Notes {
    let mut notes = Notes::new();
    drive_keys(
        app,
        config,
        tuikit::testkit::keys::script(script),
        Rect::new(0, 0, 120, 32),
        None,
        &mut notes,
    );
    notes
}

#[test]
fn key_prelude_routes_navigation_to_the_focused_right_pane() {
    let (mut app, config) = session_mux_app();
    drive(&mut app, &config, "j");
    let selection = app.selection().cloned();
    assert!(selection.is_some(), "left tree should have a selection");
    let cursor_before = app.explorer().expect("explorer").cursor;

    drive(&mut app, &config, "<Tab>jj");

    assert_eq!(app.focus(), crate::tui::app::Focus::Right);
    assert_eq!(
        app.selection().cloned(),
        selection,
        "j after <Tab> must not move the left tree"
    );
    assert_eq!(app.explorer().expect("explorer").cursor, cursor_before + 2);
}

#[test]
fn key_prelude_enter_on_a_related_row_drills_and_backspace_returns() {
    use crate::tui::explorer::ExplorerRow;
    let (mut app, config) = session_mux_app();
    drive(&mut app, &config, "j<Tab>");
    let link_row = app
        .explorer()
        .expect("explorer")
        .rows()
        .iter()
        .position(|row| matches!(row, ExplorerRow::ValidatedLink { .. }))
        .expect("related row");

    drive(&mut app, &config, &"j".repeat(link_row));
    drive(&mut app, &config, "<Enter>");
    let explorer = app.explorer().expect("explorer");
    assert_eq!(explorer.breadcrumb.len(), 1, "Enter should drill one hop");
    assert!(matches!(
        explorer.view.focused,
        crate::model::NodeId::MuxSession(_)
    ));

    drive(&mut app, &config, "<Backspace>");
    assert!(app.explorer().expect("explorer").breadcrumb.is_empty());
}

const LONG_MESSAGE: &str = "this message runs far past the width the Node zone shows for it";

/// App loaded with one session whose last message is too long for the
/// Node zone, on the default sessions view.
fn session_with_long_message_app() -> (App, RunConfig) {
    use crate::model::{AgentSessionId, AgentSessionNode, GraphNode};
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "abc"),
            "codex".to_string(),
        )
        .with_last_message_preview(LONG_MESSAGE),
    ));
    let config = RunConfig::defaults();
    let mut app = App::new(config.clone());
    refresh_from_snapshot(&mut app, &config, snapshot);
    (app, config)
}

#[test]
fn key_prelude_o_and_enter_reach_a_truncated_field_past_the_core_rows() {
    let (mut app, config) = session_with_long_message_app();
    drive(&mut app, &config, "j<Tab>F");
    let state = app.explorer().expect("explorer");
    let row = state
        .view
        .fields(true)
        .iter()
        .position(|field| field.label == "last_message_preview")
        .expect("expanded detail shows the last message");
    assert!(row >= state.view.core_fields.len());
    let cursor = state.cursor;

    drive(&mut app, &config, &"j".repeat(row - cursor));
    drive(&mut app, &config, "o");
    let modal = app.value_modal().expect("o opens the value modal");
    assert_eq!(modal.label, "last_message_preview");
    assert_eq!(modal.value, LONG_MESSAGE);

    drive(&mut app, &config, "o<Enter>");
    assert!(app.value_modal().is_none(), "o closes the modal again");
    assert_eq!(
        app.toasts().iter().last().map(|toast| toast.text.as_str()),
        Some("copied: last_message_preview"),
        "Enter copies the same field"
    );
}

#[test]
fn a_feedback_flag_names_its_severity_or_takes_the_default() {
    assert_eq!(
        parse_feedback("error:copy failed: broken pipe", Severity::Success),
        Feedback::error("copy failed: broken pipe")
    );
    assert_eq!(
        parse_feedback("warning:nothing to undo", Severity::Info),
        Feedback::warning("nothing to undo")
    );
    assert_eq!(
        parse_feedback("copied: cwd", Severity::Success),
        Feedback::success("copied: cwd")
    );
    assert_eq!(
        parse_feedback("note: not a severity", Severity::Info),
        Feedback::info("note: not a severity"),
        "only the four names are prefixes"
    );
}

#[test]
fn a_pane_hit_opens_the_pane_viewer_on_the_recorded_scrollback() {
    use crate::content_search::{MuxRef, PaneRef};
    use crate::tui::effect::{Effect, ExecSpec};
    let world = tempfile::tempdir().expect("world");
    let panes = world.path().join(crate::dev_scenarios::PANE_CAPTURES_DIR);
    fs::create_dir_all(&panes).expect("panes dir");
    fs::write(
        panes.join("shell.txt"),
        "first line\nthe match\nlast line\n",
    )
    .expect("capture");
    let pane = PaneRef {
        mux: MuxRef {
            backend: "tmux".into(),
            socket: None,
            session: "shell".into(),
        },
        // The recorded backend numbers the captures in name order.
        pane_id: "%1".into(),
        window_index: "0".into(),
        pane_index: "0".into(),
        command: Some("zsh".into()),
    };
    let effect = Effect::Exec(ExecSpec::PaneScrollback {
        pane,
        line: 2,
        text: "the match".into(),
    });
    let mut app = App::new(RunConfig::defaults());
    let rebase = Some((std::path::Path::new(FIXTURE_PATH_ROOT), world.path()));
    let mut notes = Notes::new();
    run_effects_opening_viewers(&mut app, vec![effect], rebase, &mut notes);
    assert!(notes.is_empty(), "{notes:?}");
    let viewer = app.pane_viewer().expect("the pane viewer opens");
    assert!(viewer.lines.iter().any(|line| line == "the match"));
    assert_eq!(viewer.matches.len(), 1);
}

#[test]
fn every_effect_runs_or_is_skipped_with_a_note_naming_it() {
    use crate::model::MuxSessionId;
    use crate::tui::effect::{Effect, ExecSpec, MuxOp, StoreOp};
    let effects = vec![
        Effect::Toast(Feedback::success("shown")),
        Effect::Copy {
            text: "id".into(),
            done: "copied: id".into(),
        },
        Effect::Exec(ExecSpec::MuxNew {
            name: "scratch".into(),
            cwd: "/tmp".into(),
        }),
        Effect::Exec(ExecSpec::Rebuild {
            reference: "gone".into(),
            attach: false,
        }),
        Effect::RunMux(MuxOp::CapturePreview {
            mux: MuxSessionId::new("editor"),
            native_id: "editor".into(),
        }),
        Effect::WriteStore(StoreOp::Unpin { slot: "one".into() }),
    ];
    let mut app = App::new(RunConfig::defaults());
    let mut notes = Notes::new();
    run_effects_opening_viewers(&mut app, effects, None, &mut notes);
    assert_eq!(
        notes,
        [
            "snapshot mode skipped a copy",
            "snapshot mode skipped a launch",
            "snapshot mode skipped a rebuild",
            "snapshot mode skipped a mux command",
            "snapshot mode skipped a store write",
        ]
    );
    let toasts: Vec<&str> = app
        .toasts()
        .iter()
        .map(|toast| toast.text.as_str())
        .collect();
    assert_eq!(
        toasts,
        ["shown"],
        "the pure effects ran; nothing was refused"
    );
}

#[test]
fn a_menu_pick_and_a_prompt_submit_reach_the_reducer() {
    // The row menu's first entry pins the session; the reducer asks for a
    // store write, which the snapshot skips and names.
    let (mut app, config) = session_mux_app();
    let notes = drive(&mut app, &config, "j.<Enter>");
    assert!(app.modal_stack().is_empty(), "the pick closed the menu");
    assert_eq!(notes, ["snapshot mode skipped a store write"]);

    // A rename commits through the reducer the same way.
    let (mut app, config) = session_mux_app();
    let notes = drive(&mut app, &config, "jRx<Enter>");
    assert!(app.modal_stack().is_empty(), "Enter closed the prompt");
    assert_eq!(notes, ["snapshot mode skipped a store write"]);
}

#[test]
fn the_mux_menus_shortcuts_open_what_their_keys_open_on_the_list() {
    // The menu shows each entry's shortcut so the operator learns the key
    // that opens the same form from the list.
    let menu = crate::tui::actions::mux_menu();
    let mut checked = 0;
    for entry in menu.entries() {
        let key = tuikit::keys::key_label(&entry.shortcut.expect("a shortcut"));
        let (mut through_menu, config) = session_mux_app();
        drive(&mut through_menu, &config, &format!("m{key}"));
        let (mut on_the_list, config) = session_mux_app();
        drive(&mut on_the_list, &config, &key);
        assert!(
            through_menu.top_modal_name().is_some(),
            "{key} opened a form"
        );
        assert_eq!(
            through_menu.top_modal_name(),
            on_the_list.top_modal_name(),
            "`m` then `{key}`"
        );
        checked += 1;
    }
    assert_eq!(checked, 2);
}
