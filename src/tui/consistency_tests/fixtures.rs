//! The shared world and drivers for the consistency suite: one graph
//! with live, gone, planned, and unpinned entities, rendered through
//! every view the same way.

use super::*;

pub(super) const NOW: i64 = 1_700_000_000;

/// Every view the operator can switch to.
pub(super) const ALL_VIEWS: [View; 7] = [
    View::Sessions,
    View::Mux,
    View::Union,
    View::Prs,
    View::Forks,
    View::Pinned,
    View::History,
];

/// The views that list the entities themselves (and their pins).
pub(super) const ENTITY_VIEWS: [View; 3] = [View::Sessions, View::Mux, View::Prs];

fn probe() -> crate::model::MuxProbe {
    crate::model::MuxProbe {
        status: crate::model::MuxProbeStatus::Listed,
        epoch: NOW,
        exited: Vec::new(),
    }
}

fn slot(id: &str, kind: PinKind, target: Option<&str>, order: u32) -> PinSlot {
    PinSlot {
        id: id.into(),
        kind,
        target: target.map(str::to_string),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::from([("label".to_string(), FieldValue::Text(id.into()))]),
        order,
        legacy: false,
    }
}

/// The pinned-view fixture plus: ghost muxes projected from history, an
/// unpinned mux, an unpinned live PR, and a live session outside any mux
/// with a pin of its own. `extra` adds that many loose muxes, sessions,
/// and planned pins so trees outgrow a short viewport.
pub(super) fn world_with(extra: usize) -> GraphSnapshot {
    let mut snapshot = crate::tui::rows::pinned::fixture_world();
    snapshot.mux_probes.insert("tmux".to_string(), probe());
    let mut history = std::mem::take(&mut snapshot.history);
    crate::history::project::project_into(&mut snapshot, &mut history, NOW);
    snapshot.history = history;

    let mut loose = MuxSessionNode::new(MuxSessionId::new("tmux:scratch"), "tmux", "scratch");
    loose.cwd = Some("/home/op/src/conspectus".into());
    loose.backend_session_id = Some("$9".into());
    loose.created_epoch = Some(NOW - 10_000);
    snapshot.nodes.push(GraphNode::MuxSession(loose));

    let mut solo = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", "solo000001"),
        "claude-code",
    );
    solo.cwd = Some("/home/op/src/conspectus".into());
    solo.title = Some("Untitled walk".into());
    solo.last_active_epoch = Some(NOW - 60);
    snapshot.nodes.push(GraphNode::AgentSession(solo));
    snapshot.pins.push(slot(
        "solo",
        PinKind::Session,
        Some("agent_session:claude-code:/state:solo000001"),
        7,
    ));

    snapshot.nodes.push(GraphNode::ForgePr(ForgePrNode {
        id: ForgePrId::new("github", "github.com", "malloc47", "conspectus", 42),
        provider: "github".into(),
        host: "github.com".into(),
        owner: "malloc47".into(),
        repo: "conspectus".into(),
        number: 42,
        state: Some("open".into()),
        title: Some("Follow-up fixes".into()),
        url: Some("https://github.com/malloc47/conspectus/pull/42".into()),
        updated_epoch: Some(NOW - 600),
        is_draft: false,
        head_ref: Some("follow-up".into()),
    }));

    for n in 0..extra {
        let name = format!("loose-{n:02}");
        let mut mux = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", &name);
        mux.cwd = Some("/home/op/src/conspectus".into());
        mux.backend_session_id = Some(format!("$1{n:02}"));
        mux.created_epoch = Some(NOW - 5_000);
        snapshot.nodes.push(GraphNode::MuxSession(mux));
        let mut session = AgentSessionNode::new(
            AgentSessionId::new("claude-code", "/state", format!("bulk{n:02}")),
            "claude-code",
        );
        session.cwd = Some("/home/op/src/conspectus".into());
        session.title = Some(format!("Bulk session {n:02}"));
        session.last_active_epoch = Some(NOW - 100 - n as i64);
        snapshot.nodes.push(GraphNode::AgentSession(session));
        snapshot.pins.push(slot(
            &format!("planned-{n:02}"),
            PinKind::Mux,
            None,
            100 + n as u32,
        ));
    }
    resolve_snapshot(snapshot)
}

pub(super) fn world() -> GraphSnapshot {
    world_with(0)
}

pub(super) fn app_over(view: View, snapshot: &GraphSnapshot) -> App {
    let mut config = RunConfig::defaults();
    config.default_view = view;
    let mut app = App::new(config);
    app.update(Msg::SwitchView(view));
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs {
        snapshot,
        view,
        grouping: Grouping::default_for(view),
        filter: RowFilter::default(),
        sort: Sort::Hierarchy,
        mux_recency: MuxRecency::default(),
        cwd: None,
    });
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(snapshot),
        tree,
        loaded_at_epoch: NOW,
        initial_selection_hint: None,
    });
    app
}

pub(super) fn app_in(view: View) -> App {
    app_over(view, &world())
}

pub(super) fn buffer_of(app: &mut App, width: u16, height: u16) -> ratatui::buffer::Buffer {
    render_to_buffer(app, Rect::new(0, 0, width, height))
}

pub(super) fn frame(app: &mut App, width: u16, height: u16) -> String {
    buffer_to_string(&buffer_of(app, width, height))
}

/// The text of the left pane's rows only, one string per terminal row.
pub(super) fn left_pane_lines(frame: &str, width: u16) -> Vec<String> {
    let half = usize::from(width) / 2;
    frame
        .lines()
        .map(|line| line.chars().take(half).collect())
        .collect()
}

pub(super) fn row_id_for(app: &App, label: &str) -> Option<RowId> {
    app.tree()
        .rows
        .iter()
        .find(|row| match &row.id {
            RowId::Pin { pin_id } => pin_id == label,
            RowId::History { record_id } => record_id == label,
            RowId::MuxSession(NodeId::MuxSession(id)) => id.native_id == format!("tmux:{label}"),
            RowId::AgentSession(NodeId::AgentSession(id)) => id.session_key.starts_with(label),
            _ => false,
        })
        .map(|row| row.id.clone())
}

pub(super) fn select(app: &mut App, label: &str) {
    let id = row_id_for(app, label).unwrap_or_else(|| panic!("no row for {label}"));
    app.set_selection(id);
}

/// What `Enter` does on the selected row, the way the live loop routes it.
pub(super) fn enter(app: &mut App) -> Vec<crate::tui::Effect> {
    match crate::tui::keymap::selected_default_action(app) {
        crate::tui::keymap::SelectedDefault::ToggleExpand => app.update(Msg::ToggleExpand),
        crate::tui::keymap::SelectedDefault::Attach => app.update(Msg::AttachSelected),
        crate::tui::keymap::SelectedDefault::View => app.update(Msg::ViewSelected),
    }
}

pub(super) fn menu_labels(app: &mut App) -> Vec<String> {
    app.update(Msg::OpenRowMenu);
    let labels = app
        .row_menu()
        .map(|menu| menu.entries().iter().map(|e| e.label.clone()).collect())
        .unwrap_or_default();
    app.close_row_menu();
    labels
}

/// A row's entity for display in failure messages.
pub(super) fn describe(view: View, id: &RowId) -> String {
    format!("{view:?}/{id:?}")
}

/// The toast texts the pin-specific messages produce for the selection.
pub(super) fn menu_toasts(app: &mut App) -> Vec<String> {
    let mut out = Vec::new();
    for msg in [Msg::TogglePin, Msg::AttachSelected, Msg::ViewSelected] {
        for effect in app.update(msg) {
            if let crate::tui::Effect::Toast(text) = effect {
                out.push(text);
            }
        }
    }
    out
}
