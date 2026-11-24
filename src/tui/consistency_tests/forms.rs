//! Forms keep the capabilities the forms before them had: arrow keys
//! move between fields, Tab completes a directory in a cwd field, the
//! harness row offers the harness options (including skipping
//! permissions), a pin's editable fields are listed, not guessed, and a
//! new mux's name starts as the next free one in the series of the mux
//! the operator is looking at. And they follow the form house rules:
//! `Enter` submits from any field, and the keys stay in view on a short
//! terminal.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

use super::*;
use crate::tui::Theme;
use crate::tui::widgets::mux_launch::{self, MuxLaunchForm};
use crate::tui::widgets::new_mux::{self, CWD};
use tuikit::overlay::Overlay;
use tuikit::testkit::keys::press;

fn launch_form(cwd: &str) -> MuxLaunchForm {
    MuxLaunchForm::new(
        "claude-code",
        cwd,
        "fresh",
        vec!["claude-code".into(), "codex".into(), "opencode".into()],
        vec![],
    )
}

fn rendered(form: &MuxLaunchForm) -> String {
    let area = Rect::new(0, 0, 100, 30);
    let mut buffer = Buffer::empty(area);
    form.draw(area, &mut buffer, &Theme::default());
    buffer_to_string(&buffer)
}

/// The launch form's fields are one ring: `Down` visits every stop `Tab`
/// does, the server checkbox included (house rule 6).
#[test]
fn arrow_keys_move_between_the_launch_forms_fields() {
    let mut form = launch_form("/tmp");
    assert_eq!(form.focus(), Some(mux_launch::NAME), "the name is first");
    form.handle_key(press(KeyCode::Down));
    assert_eq!(
        form.focus(),
        Some(mux_launch::SERVER),
        "the server checkbox is a field of its own"
    );
    form.handle_key(press(KeyCode::Down));
    assert_eq!(form.focus(), Some(mux_launch::HARNESS));
    form.handle_key(press(KeyCode::Down));
    assert_eq!(
        form.focus(),
        Some(mux_launch::OPTIONS),
        "Down moves to the next field: the harness's launch options"
    );
    form.handle_key(press(KeyCode::Down));
    assert_eq!(form.focus(), Some(mux_launch::CWD));
    for _ in 0..4 {
        form.handle_key(press(KeyCode::Up));
    }
    assert_eq!(form.focus(), Some(mux_launch::NAME), "Up moves back");
}

#[test]
fn tab_completes_a_directory_in_the_launch_forms_cwd_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("alpha-dir")).expect("subdir");
    let typed = format!("{}/alp", dir.path().display());
    let mut form = launch_form(&typed);
    for _ in 0..4 {
        form.handle_key(press(KeyCode::Down));
    }
    assert_eq!(
        form.focus(),
        Some(mux_launch::CWD),
        "reach the cwd field first"
    );
    form.handle_key(press(KeyCode::Tab));
    assert_eq!(
        form.values().text(mux_launch::CWD),
        format!("{}/alpha-dir", dir.path().display()),
        "Tab completes the path while the cursor is in the cwd field"
    );
    assert_eq!(form.focus(), Some(mux_launch::CWD));
}

#[test]
fn the_new_mux_form_completes_paths_and_moves_with_arrows() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("alpha-dir")).expect("subdir");
    let typed = format!("{}/alp", dir.path().display());
    let mut form = new_mux::form("fresh", typed, Vec::new());
    form.handle(press(KeyCode::Down), &());
    assert_eq!(form.focus(), Some(CWD), "Down moves to the cwd field");
    form.handle(press(KeyCode::Tab), &());
    assert_eq!(
        form.values().text(CWD),
        format!("{}/alpha-dir", dir.path().display())
    );
}

/// The new mux form and the launch form submit from any field, so `Enter`
/// on the first field does what it does on the last (house rule 6).
#[test]
fn enter_submits_a_form_from_its_first_field() {
    let mut form = new_mux::form("fresh", "/tmp", Vec::new());
    assert!(
        matches!(
            form.handle(press(KeyCode::Enter), &()),
            tuikit::overlay::Outcome::Commit(Msg::CommitMuxNew { .. })
        ),
        "the new mux form creates from its name field"
    );
    let mut launch = launch_form("/tmp");
    assert_eq!(launch.focus(), Some(mux_launch::NAME));
    assert!(matches!(
        launch.handle_key(press(KeyCode::Enter)),
        tuikit::overlay::Outcome::Commit(Msg::CommitMuxLaunch(_))
    ));
}

/// A form's footer is fixed: on a terminal too short for the whole form,
/// its keys stay at the bottom and its rows scroll (house rule 8).
#[test]
fn a_forms_keys_stay_in_view_on_a_short_terminal() {
    let mut launch = launch_form("/tmp");
    // The last field, so the rows have scrolled to it.
    launch.handle_key(press(KeyCode::Up));
    let area = Rect::new(0, 0, 60, 12);
    let mut buffer = Buffer::empty(area);
    launch.draw(area, &mut buffer, &Theme::default());
    let frame = buffer_to_string(&buffer);
    for segment in
        tuikit::keys::Footer::new(crate::tui::keybindings::overlay::MUX_LAUNCH_KEYS).segments()
    {
        assert!(
            frame.contains(&segment.text),
            "{} scrolled away:\n{frame}",
            segment.text
        );
    }
    assert!(
        frame.contains("pin"),
        "the focused row is in view:\n{frame}"
    );
}

#[test]
fn the_harness_row_lists_its_options_and_the_permission_toggle() {
    let form = launch_form("/tmp");
    let text = rendered(&form).to_lowercase();
    for harness in ["claude-code", "codex", "opencode"] {
        assert!(
            text.contains(harness),
            "the harness choices include {harness}:\n{text}"
        );
    }
    assert!(
        text.contains("skip permissions") || text.contains("skip-permissions"),
        "a harness with launch options offers them:\n{text}"
    );
}

/// Each field shows under its label in the forms' shared vocabulary (the
/// worktree branch reads `branch`, as in the launch form).
#[test]
fn editing_a_pin_lists_every_field_the_pin_can_change() {
    let mut app = app_in(View::Pinned);
    select(&mut app, "conspectus-main");
    app.update(Msg::OpenSlotEdit("conspectus-main".into()));
    let frame = frame(&mut app, 160, 40);
    let missing: Vec<&str> = PinKind::Mux
        .overridable_fields()
        .iter()
        .map(|field| crate::tui::widgets::pin_edit::label_for(field))
        .filter(|label| !frame.contains(&format!(" {label} ")))
        .collect();
    assert!(
        missing.is_empty(),
        "the edit form must show {missing:?}:\n{frame}"
    );
}

#[test]
fn retargeting_offers_the_other_entities_of_the_same_kind() {
    let mut app = app_in(View::Pinned);
    select(&mut app, "conspectus-main");
    app.update(Msg::OpenSlotRetarget("conspectus-main".into()));
    let frame = frame(&mut app, 160, 40);
    assert!(
        frame.contains("scratch"),
        "the unpinned mux `scratch` is a candidate to retarget to:\n{frame}"
    );
}

fn command_row_cells(form: &MuxLaunchForm) -> Vec<(char, ratatui::style::Modifier)> {
    let area = Rect::new(0, 0, 100, 30);
    let mut buffer = Buffer::empty(area);
    form.draw(area, &mut buffer, &Theme::default());
    for y in 0..area.height {
        let text: String = (0..area.width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect();
        if text.contains("command") {
            return (0..area.width)
                .map(|x| {
                    let cell = &buffer[(x, y)];
                    (
                        cell.symbol().chars().next().unwrap_or(' '),
                        cell.style().add_modifier,
                    )
                })
                .collect();
        }
    }
    panic!("no command row in the form:\n{}", rendered(form));
}

#[test]
fn the_launch_forms_command_row_is_read_only_and_never_takes_focus() {
    use ratatui::style::Modifier;
    let mut form = launch_form("/tmp");
    // Walk the whole focus ring both ways with arrows and Tab: the
    // command row is not a field, so focus never rests on a row that
    // renders the command.
    for code in [KeyCode::Down, KeyCode::Tab, KeyCode::Up, KeyCode::BackTab] {
        for _ in 0..16 {
            form.handle_key(press(code));
            let cells = command_row_cells(&form);
            let marker: String = cells.iter().take(2).map(|(c, _)| *c).collect();
            assert!(
                !marker.contains('>'),
                "focus marker on the command row after {code:?}"
            );
        }
    }
    let cells = command_row_cells(&form);
    let text: String = cells.iter().map(|(c, _)| *c).collect();
    assert!(!text.contains("read only"), "no words spent on it: {text}");
    assert!(text.contains("└ command"), "led in by a dim corner: {text}");
    // Two cells deeper than the field labels, with its value in the
    // same column as theirs.
    let column = |needle: &str| {
        let frame = rendered(&form);
        let line = frame.lines().find(|l| l.contains(needle)).expect("row");
        line.find(needle).map(|byte| line[..byte].chars().count())
    };
    assert_eq!(
        column("command").map(|c| c - 2),
        column("harness  "),
        "the command label sits two cells inside the field labels"
    );
    let styled = cells
        .iter()
        .filter(|(c, _)| !c.is_whitespace() && !matches!(c, '│' | '┃' | '|'))
        .all(|(_, m)| m.contains(Modifier::ITALIC) && m.contains(Modifier::DIM));
    assert!(styled, "the command row is dim italic, unlike input rows");
}

// ---- the name a new mux starts with ---------------------------------------

/// The name each form opens with over the app `make` builds: the new
/// tmux session form's, then the launch form's.
fn seeded_names(make: impl Fn() -> App) -> [String; 2] {
    let mut app = make();
    app.update(Msg::OpenNewMuxForm);
    let bare = app
        .new_mux_form()
        .expect("the new tmux session form opens")
        .values()
        .text(new_mux::NAME)
        .to_string();
    let mut app = make();
    app.update(Msg::OpenMuxLaunchForm);
    let launch = app
        .mux_launch_form()
        .expect("the launch form opens")
        .values()
        .text(mux_launch::NAME)
        .to_string();
    [bare, launch]
}

/// Both forms open with `name`.
fn both(name: &str) -> [String; 2] {
    [name.to_string(), name.to_string()]
}

/// `view` over `world` with the row for `label` selected.
fn selected(view: View, world: &GraphSnapshot, label: &str) -> App {
    let mut app = app_over(view, world);
    select(&mut app, label);
    app
}

/// Live muxes named `names`; a session pin for each of `pin_ids`, so the
/// name is the pin's alone and no mux has it; and a mux in history for
/// each of `recorded`.
fn series_world(names: &[&str], pin_ids: &[&str], recorded: &[&str]) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    for name in names {
        let mut mux = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", *name);
        mux.cwd = Some("/home/op/src/conspectus".into());
        snapshot.nodes.push(GraphNode::MuxSession(mux));
    }
    for (order, id) in (0u32..).zip(pin_ids) {
        let key = format!("pinned{order:04}");
        snapshot
            .nodes
            .push(GraphNode::AgentSession(AgentSessionNode::new(
                AgentSessionId::new("claude-code", "/state", key.clone()),
                "claude-code",
            )));
        snapshot.pins.push(PinSlot {
            id: (*id).to_string(),
            kind: PinKind::Session,
            target: Some(format!("agent_session:claude-code:/state:{key}")),
            pinned_at: None,
            overrides: SlotFields::new(),
            snapshot: SlotFields::new(),
            order,
            legacy: false,
        });
    }
    for (n, name) in recorded.iter().enumerate() {
        snapshot.history.muxes.push(crate::model::MuxRecord {
            meta: crate::model::RecordMeta {
                id: format!("h_rec{n}"),
                key: format!("mux_session:tmux:{name}"),
                first_seen: NOW - 9_000,
                last_seen: NOW - 8_000,
                origin: Default::default(),
                display: Default::default(),
                rebuilt_from: None,
            },
            recipe: crate::model::MuxRecipe {
                backend: "tmux".into(),
                name: (*name).into(),
                ..Default::default()
            },
            extra: Default::default(),
        });
    }
    resolve_snapshot(snapshot)
}

/// Focus the right pane and walk its cursor onto the first link to an
/// entity `wanted` accepts.
fn focus_right_on(app: &mut App, wanted: impl Fn(&NodeId) -> bool) {
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), crate::tui::app::Focus::Right);
    let mut seen = Vec::new();
    for _ in 0..60 {
        let target = app.explorer().and_then(|state| {
            let row = app.explorer_cursor_row()?;
            state.content().drill_target(&row)
        });
        if target.as_ref().is_some_and(&wanted) {
            return;
        }
        seen.extend(target);
        // A folded section shows its links once opened.
        app.update(Msg::Explorer(tuikit::explorer::ExplorerKey::OpenSection));
        app.update(Msg::Explorer(tuikit::explorer::ExplorerKey::Down));
    }
    panic!("the right pane links to no such entity, only to {seen:?}");
}

/// Select the first group row of `app`'s list that stands for a repo,
/// checkout, or workspace.
fn select_a_group(app: &mut App) {
    let id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.id {
            RowId::Group(NodeId::Repo(_) | NodeId::Checkout(_) | NodeId::Workspace(_)) => {
                Some(row.id.clone())
            }
            _ => None,
        })
        .expect("the list has a group row");
    app.set_selection(id);
}

#[test]
fn a_numbered_mux_opens_the_forms_on_the_next_free_name_in_its_series() {
    let world = series_world(&["work-1", "work-2"], &[], &[]);
    assert_eq!(
        seeded_names(|| selected(View::Mux, &world, "work-1")),
        both("work-3"),
        "`work-2` is live, so `work-1` leads to `work-3`"
    );
}

#[test]
fn a_name_a_pin_or_history_holds_is_skipped() {
    let world = series_world(&["work-1"], &["work-2"], &["work-3"]);
    assert_eq!(
        seeded_names(|| selected(View::Mux, &world, "work-1")),
        both("work-4"),
        "a pin is named `work-2` and history has a `work-3`"
    );
}

#[test]
fn a_zero_padded_number_keeps_its_width() {
    let world = series_world(&["work-007", "work-099"], &[], &[]);
    assert_eq!(
        seeded_names(|| selected(View::Mux, &world, "work-007")),
        both("work-008")
    );
    assert_eq!(
        seeded_names(|| selected(View::Mux, &world, "work-099")),
        both("work-100")
    );
}

#[test]
fn a_name_without_a_number_gets_a_dash_suffix() {
    let world = world();
    assert_eq!(
        seeded_names(|| selected(View::Mux, &world, "scratch")),
        both("scratch-2")
    );
    assert_eq!(
        seeded_names(|| selected(View::Mux, &world, "conspectus-main")),
        both("conspectus-main-2")
    );
}

#[test]
fn a_session_opens_the_forms_on_its_muxs_series() {
    let world = world();
    assert_eq!(
        seeded_names(|| selected(View::Sessions, &world, "e44d01cb")),
        both("conspectus-main-2"),
        "the session runs in `conspectus-main`"
    );
}

/// A session whose mux has gone takes the one its row shows: the mux
/// history says hosted it last.
#[test]
fn a_session_whose_mux_is_gone_seeds_from_the_one_that_hosted_it_last() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::AgentSession(AgentSessionNode::new(
            AgentSessionId::new("claude-code", "/state", "hosted0001"),
            "claude-code",
        )));
    snapshot.history.muxes.push(crate::model::MuxRecord {
        meta: crate::model::RecordMeta {
            id: "h_work1".into(),
            key: "mux_session:tmux:work-1".into(),
            first_seen: NOW - 9_000,
            last_seen: NOW - 8_000,
            origin: Default::default(),
            display: Default::default(),
            rebuilt_from: None,
        },
        recipe: crate::model::MuxRecipe {
            backend: "tmux".into(),
            name: "work-1".into(),
            hosted: vec![crate::model::HostedSession {
                harness: "claude-code".into(),
                session_id: "hosted0001".into(),
                first_seen: NOW - 9_000,
                last_seen: NOW - 8_000,
            }],
            ..Default::default()
        },
        extra: Default::default(),
    });
    let world = resolve_snapshot(snapshot);
    assert_eq!(
        seeded_names(|| selected(View::Sessions, &world, "hosted")),
        both("work-2")
    );
}

#[test]
fn a_gone_mux_and_a_mux_pin_seed_from_the_muxs_name() {
    let world = world();
    for (view, label, seed) in [
        (View::Mux, "config-agent3", "config-agent4"),
        (View::History, "h_old", "old-experiment-2"),
        (View::Pinned, "config-agent3", "config-agent4"),
        (View::Pinned, "conspectus-main", "conspectus-main-2"),
        (View::Pinned, "nightly", "nightly-2"),
    ] {
        assert_eq!(
            seeded_names(|| selected(view, &world, label)),
            both(seed),
            "{label} in {view:?}"
        );
    }
}

#[test]
fn a_row_that_is_no_mux_and_runs_in_none_opens_the_forms_empty() {
    let world = world();
    let group = || {
        let mut app = app_over(View::Sessions, &world);
        select_a_group(&mut app);
        app
    };
    assert_eq!(seeded_names(group), both(""), "a group row");
    let pr = || {
        let mut app = app_over(View::Prs, &world);
        let id = app
            .visible_rows()
            .iter()
            .find(|row| matches!(row.id, RowId::Pr(_)))
            .map(|row| row.id.clone())
            .expect("a PR row");
        app.set_selection(id);
        app
    };
    assert_eq!(seeded_names(pr), both(""), "a PR row");
    assert_eq!(
        seeded_names(|| selected(View::Pinned, &world, "ledger-worktree")),
        both(""),
        "a checkout pin"
    );
    assert_eq!(
        seeded_names(|| selected(View::Sessions, &world, "solo")),
        both(""),
        "a session outside any mux"
    );
    let empty = GraphSnapshot::empty();
    assert_eq!(
        seeded_names(|| app_over(View::Mux, &empty)),
        both(""),
        "nothing selected"
    );
}

fn session_id(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", key))
}

/// A session `child00001` in the mux `work-1`, forked from the session
/// `parent0001` in the mux `build-3`.
fn lineage_world() -> GraphSnapshot {
    fn link(
        id: &str,
        source: NodeId,
        target: NodeId,
        relation: crate::model::RelationKind,
    ) -> crate::model::GraphLink {
        let mut link = crate::model::GraphLink::new(
            id.to_string(),
            source,
            crate::model::LinkEndpoint::Node { id: target },
            relation,
            crate::model::Provenance::Discovered,
        );
        link.state = crate::model::LinkState::Active;
        link
    }
    let mut snapshot = GraphSnapshot::empty();
    for (key, mux) in [("child00001", "work-1"), ("parent0001", "build-3")] {
        let id = MuxSessionId::new(format!("tmux:{mux}"));
        snapshot
            .nodes
            .push(GraphNode::MuxSession(MuxSessionNode::new(
                id.clone(),
                "tmux",
                mux,
            )));
        snapshot
            .nodes
            .push(GraphNode::AgentSession(AgentSessionNode::new(
                AgentSessionId::new("claude-code", "/state", key),
                "claude-code",
            )));
        snapshot.candidate_links.push(link(
            &format!("in-mux:{key}"),
            session_id(key),
            NodeId::MuxSession(id),
            crate::model::RelationKind::LinkedToMux,
        ));
    }
    snapshot.candidate_links.push(link(
        "lineage",
        session_id("child00001"),
        session_id("parent0001"),
        crate::model::RelationKind::ParentSession,
    ));
    resolve_snapshot(snapshot)
}

#[test]
fn with_the_right_pane_focused_the_entity_it_shows_is_the_base() {
    let world = lineage_world();
    assert_eq!(
        seeded_names(|| selected(View::Sessions, &world, "child")),
        both("work-2"),
        "the selected session runs in `work-1`"
    );
    let parent = session_id("parent0001");
    let make = || {
        let mut app = selected(View::Sessions, &world, "child");
        focus_right_on(&mut app, |node| *node == parent);
        app
    };
    assert_eq!(
        seeded_names(make),
        both("build-4"),
        "the right pane's cursor is on the parent session, which runs in `build-3`"
    );
}

// ----- The launch form starts as a copy of the row --------------------

const SKIP_PERMISSIONS: &str = "--dangerously-skip-permissions";
const CODEX_SKIP_PERMISSIONS: &str = "--dangerously-bypass-approvals-and-sandbox";

/// What the launch form holds right after it opens over an app.
#[derive(Debug, PartialEq, Eq)]
struct Opened {
    harness: String,
    argv: String,
    /// Each launch option's checkbox, in the harness's order.
    options: Vec<bool>,
    cwd: String,
    /// The pin row's choice: 0 `don't pin`, 1 `launch, then pin`, 2
    /// `pin, don't launch`.
    pin: usize,
}

fn open_launch_form(app: &mut App) -> Opened {
    app.update(Msg::OpenMuxLaunchForm);
    let form = app.mux_launch_form().expect("the launch form opens");
    let values = form.values();
    Opened {
        harness: values.text(mux_launch::HARNESS).to_string(),
        argv: values.text(mux_launch::ARGV).to_string(),
        options: values
            .check_group(mux_launch::OPTIONS)
            .map(tuikit::form::CheckGroup::checked)
            .unwrap_or_default(),
        cwd: values.text(mux_launch::CWD).to_string(),
        pin: values
            .chosen(mux_launch::PIN)
            .expect("the pin row has a choice"),
    }
}

/// A live mux, running `command` in `cwd`.
fn running_mux(name: &str, cwd: &str, command: &str) -> GraphNode {
    let mut mux = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", name);
    mux.cwd = Some(cwd.into());
    mux.active_pane_start_command = Some(command.into());
    GraphNode::MuxSession(mux)
}

/// A stopped mux's history record, with the recipe the operator launched
/// it by.
fn recorded_mux(
    id: &str,
    name: &str,
    cwd: &str,
    harness: &str,
    argv: &[&str],
) -> crate::model::MuxRecord {
    crate::model::MuxRecord {
        meta: crate::model::RecordMeta {
            id: id.into(),
            key: format!("mux_session:tmux:{name}"),
            first_seen: NOW - 9_000,
            last_seen: NOW - 8_000,
            origin: Default::default(),
            display: Default::default(),
            rebuilt_from: None,
        },
        recipe: crate::model::MuxRecipe {
            backend: "tmux".into(),
            name: name.into(),
            cwd: Some(cwd.into()),
            harness: Some(harness.into()),
            argv: Some(argv.iter().map(ToString::to_string).collect()),
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn mux_pin(id: &str, target: Option<&str>) -> PinSlot {
    PinSlot {
        id: id.into(),
        kind: PinKind::Mux,
        target: target.map(str::to_string),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    }
}

/// Every directory here exists except `/no/such/dir`:
///
/// - `work` runs Claude Code skipping permissions, hosts the session
///   `s-work`, and is pinned (`work-pin`);
/// - `plain` runs Claude Code without options and hosts the Codex
///   session `s-codex`;
/// - `gone-1` is stopped, recorded with Codex skipping permissions in a
///   directory that is gone, and unpinned;
/// - `gone-2` is stopped, recorded with Claude Code in `dir`, and pinned
///   (`gone-pin`);
/// - `planned` is a mux pin that has not been launched, with Codex and `dir`.
fn launch_world(dir: &str) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(running_mux(
        "work",
        dir,
        &format!("claude {SKIP_PERMISSIONS}"),
    ));
    snapshot.nodes.push(running_mux("plain", dir, "claude"));
    for (key, harness, mux) in [
        ("s-work", "claude-code", "work"),
        ("s-codex", "codex", "plain"),
    ] {
        let id = AgentSessionId::new(harness, "/state", key);
        let mut session = AgentSessionNode::new(id.clone(), harness);
        session.cwd = Some(dir.into());
        snapshot.nodes.push(GraphNode::AgentSession(session));
        let mut link = crate::model::GraphLink::new(
            format!("test:{key}-in-{mux}"),
            NodeId::AgentSession(id),
            crate::model::LinkEndpoint::Node {
                id: NodeId::MuxSession(MuxSessionId::new(format!("tmux:{mux}"))),
            },
            crate::model::RelationKind::LinkedToMux,
            crate::model::Provenance::StrongDiscovered,
        );
        link.state = crate::model::LinkState::Active;
        snapshot.candidate_links.push(link);
    }
    let mut history = crate::model::HistorySnapshot::default();
    history.muxes.push(recorded_mux(
        "h_gone1",
        "gone-1",
        "/no/such/dir",
        "codex",
        &["codex", CODEX_SKIP_PERMISSIONS],
    ));
    history.muxes.push(recorded_mux(
        "h_gone2",
        "gone-2",
        dir,
        "claude-code",
        &["claude"],
    ));
    for id in ["h_gone1", "h_gone2"] {
        history.presence.insert(
            id.to_string(),
            crate::model::NodePresence::Gone {
                last_seen: NOW - 8_000,
            },
        );
    }
    let mut planned = mux_pin("planned", None);
    planned.overrides = SlotFields::from([
        (
            "harness".to_string(),
            crate::model::FieldValue::Text("codex".into()),
        ),
        (
            "cwd".to_string(),
            crate::model::FieldValue::Text(dir.into()),
        ),
    ]);
    snapshot.pins = vec![
        mux_pin("work-pin", Some("mux_session:tmux:work")),
        mux_pin("gone-pin", Some("history:h_gone2")),
        planned,
    ];
    snapshot.mux_probes.insert(
        "tmux".to_string(),
        crate::model::MuxProbe::new(crate::model::MuxProbeStatus::Listed, NOW),
    );
    crate::history::project::project_into(&mut snapshot, &mut history, NOW);
    snapshot.history = history;
    resolve_snapshot(snapshot)
}

fn world_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("a directory for the world's muxes")
}

#[test]
fn a_running_mux_opens_the_form_with_its_harness_options_and_directory() {
    let dir = world_dir();
    let cwd = dir.path().display().to_string();
    let world = launch_world(&cwd);
    let opened = open_launch_form(&mut selected(View::Mux, &world, "work"));
    assert_eq!(opened.harness, "claude-code");
    assert!(opened.argv.contains(SKIP_PERMISSIONS), "{opened:?}");
    assert_eq!(opened.options, [true], "skip permissions starts checked");
    assert_eq!(opened.cwd, cwd);

    let opened = open_launch_form(&mut selected(View::Mux, &world, "plain"));
    assert_eq!(opened.harness, "claude-code");
    assert_eq!(
        opened.options,
        [false],
        "no option the mux was not started with"
    );
}

#[test]
fn the_pin_choice_follows_whether_the_row_is_pinned_and_running() {
    let dir = world_dir();
    let world = launch_world(&dir.path().display().to_string());
    for (view, label, pin) in [
        (View::Mux, "plain", 0),
        (View::Mux, "work", 1),
        (View::Mux, "gone-2", 2),
        (View::History, "h_gone1", 0),
        (View::Pinned, "work-pin", 1),
        (View::Pinned, "gone-pin", 2),
        (View::Pinned, "planned", 2),
    ] {
        let opened = open_launch_form(&mut selected(view, &world, label));
        assert_eq!(opened.pin, pin, "{label} in {view:?}: {opened:?}");
    }
}

#[test]
fn a_stopped_mux_opens_the_form_from_its_record() {
    let dir = world_dir();
    let cwd = dir.path().display().to_string();
    let world = launch_world(&cwd);
    let opened = open_launch_form(&mut selected(View::History, &world, "h_gone1"));
    assert_eq!(opened.harness, "codex");
    assert!(opened.argv.contains(CODEX_SKIP_PERMISSIONS), "{opened:?}");
    assert_eq!(opened.options, [true]);
    assert_ne!(
        opened.cwd, "/no/such/dir",
        "its directory is gone, so the form does not start there"
    );

    // Its directory exists: the form starts there. A pinned record is not
    // listed in History, so it is the Mux view's ghost row.
    let opened = open_launch_form(&mut selected(View::Mux, &world, "gone-2"));
    assert_eq!(opened.harness, "claude-code");
    assert_eq!(opened.cwd, cwd);
}

#[test]
fn a_directory_that_is_gone_falls_back_to_the_default_not_to_a_missing_path() {
    let dir = world_dir();
    let mut world = launch_world(&dir.path().display().to_string());
    let fallback = open_launch_form(&mut app_over(View::Mux, &GraphSnapshot::empty())).cwd;

    // A stopped mux whose recorded directory is gone.
    let opened = open_launch_form(&mut selected(View::History, &world, "h_gone1"));
    assert_eq!(opened.cwd, fallback, "the cwd a form seeds with no row");

    // A running mux whose directory was deleted under it.
    for node in &mut world.nodes {
        if let GraphNode::MuxSession(mux) = node
            && mux.session_name() == "plain"
        {
            mux.cwd = Some("/no/such/dir".into());
        }
    }
    let opened = open_launch_form(&mut selected(View::Mux, &world, "plain"));
    assert_eq!(opened.cwd, fallback);
    assert_eq!(
        opened.harness, "claude-code",
        "the rest of the row still seeds the form"
    );
}

#[test]
fn a_session_opens_the_form_with_its_harness_and_its_muxs_arguments() {
    let dir = world_dir();
    let cwd = dir.path().display().to_string();
    let world = launch_world(&cwd);
    let opened = open_launch_form(&mut selected(View::Sessions, &world, "s-work"));
    assert_eq!(opened.harness, "claude-code");
    assert_eq!(
        opened.options,
        [true],
        "its mux runs it skipping permissions"
    );
    assert_eq!(opened.cwd, cwd);
    assert_eq!(
        opened.pin, 0,
        "the session is not pinned, whatever its mux is"
    );

    // Its mux runs another harness: the arguments are that program's, not
    // this session's.
    let opened = open_launch_form(&mut selected(View::Sessions, &world, "s-codex"));
    assert_eq!(opened.harness, "codex");
    assert_eq!(opened.options, [false]);
    assert!(opened.argv.is_empty(), "{opened:?}");
}

#[test]
fn a_planned_pin_opens_the_form_with_what_it_says() {
    let dir = world_dir();
    let cwd = dir.path().display().to_string();
    let world = launch_world(&cwd);
    let opened = open_launch_form(&mut selected(View::Pinned, &world, "planned"));
    assert_eq!(opened.harness, "codex");
    assert_eq!(opened.cwd, cwd);
    assert_eq!(opened.pin, 2);
}

#[test]
fn a_row_that_names_no_harness_leaves_the_forms_defaults() {
    let dir = world_dir();
    let root = dir.path().display().to_string();
    let mut world = launch_world(&root);
    let repo = crate::model::RepoId::new(format!("{root}/.git"));
    world
        .nodes
        .push(GraphNode::Repo(crate::model::RepoNode::new(repo.clone())));
    world
        .nodes
        .push(GraphNode::Checkout(crate::model::CheckoutNode::new(
            crate::model::CheckoutId::new(repo, root.as_str()),
            root.as_str(),
        )));
    let world = resolve_snapshot(world);
    let bare = open_launch_form(&mut app_over(View::Mux, &GraphSnapshot::empty()));
    let mut app = app_over(View::Sessions, &world);
    select_a_group(&mut app);
    let opened = open_launch_form(&mut app);
    assert_eq!(opened.harness, bare.harness);
    assert!(opened.argv.is_empty(), "{opened:?}");
    assert_eq!(opened.pin, 0);
}

#[test]
fn a_harness_the_form_cannot_pick_is_not_seeded() {
    let dir = world_dir();
    let mut world = launch_world(&dir.path().display().to_string());
    world.history.muxes[1].recipe.harness = Some("mystery".into());
    world.history.muxes[1].recipe.argv = Some(vec!["mystery".into(), "--flag".into()]);
    // The pin's effective fields come from the history record.
    let world = resolve_snapshot(world);
    let bare = open_launch_form(&mut app_over(View::Mux, &GraphSnapshot::empty()));
    let opened = open_launch_form(&mut selected(View::Mux, &world, "gone-2"));
    assert_eq!(opened.harness, bare.harness);
    assert!(
        opened.argv.is_empty(),
        "flags of a program the form does not know are not carried: {opened:?}"
    );
}

#[test]
fn the_same_entity_opens_the_same_form_in_every_view() {
    let dir = world_dir();
    let world = launch_world(&dir.path().display().to_string());
    let open = |view, label| open_launch_form(&mut selected(view, &world, label));
    assert_eq!(
        open(View::Mux, "gone-2"),
        open(View::Pinned, "gone-pin"),
        "a pinned stopped mux, from the Mux view and from its pin"
    );
    assert_eq!(open(View::Mux, "work"), open(View::Pinned, "work-pin"));
    // An unpinned stopped mux is in History, and in the Mux view when
    // asked for.
    let filter = RowFilter {
        show_stopped_muxes: true,
        ..Default::default()
    };
    let mut in_mux_view = app_over_filtered(View::Mux, &world, filter);
    select(&mut in_mux_view, "gone-1");
    assert_eq!(
        open_launch_form(&mut in_mux_view),
        open(View::History, "h_gone1")
    );
}

#[test]
fn the_launch_form_seeds_the_same_with_the_right_pane_focused() {
    let dir = world_dir();
    let world = launch_world(&dir.path().display().to_string());
    // On a Codex session, with the right pane's cursor on the link to
    // its mux, which runs Claude Code: the entity the pane shows seeds
    // the form, not the row.
    let mux = NodeId::MuxSession(MuxSessionId::new("tmux:plain"));
    assert_eq!(
        open_launch_form(&mut selected(View::Sessions, &world, "s-codex")).harness,
        "codex",
        "without the right pane, the row seeds it"
    );
    let mut app = selected(View::Sessions, &world, "s-codex");
    focus_right_on(&mut app, |node| *node == mux);
    let opened = open_launch_form(&mut app);
    assert_eq!(opened.harness, "claude-code");
}
