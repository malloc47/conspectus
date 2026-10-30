//! Side-effecting actions the TUI can dispatch in response to a key
//! press, beyond what the pure reducer handles.
//!
//! v1 (this commit) covers attach-to-mux. Resume, fork, picker-
//! driven attach, and the rest of the action surface land in later
//! stories per `docs/implementation/phase-08-interactive-tui.md`.

use crate::model::{
    AgentSessionId, GraphLink, GraphSnapshot, MuxSessionId, MuxSessionNode, NodeId, RelationKind,
};
use crate::tui::app::App;
use crate::tui::rows::RowId;
use crate::tui::viewer::ViewerDisabled;
use tuikit::action_menu::{ActionMenu, Confirm, Entry};

/// Resolved target for an attach action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachTarget {
    /// The mux session's graph id. Carries the backend-prefixed
    /// `native_id` (e.g. `tmux:editor`) used as a stable key
    /// inside the graph and the preview cache.
    pub mux: MuxSessionId,
    /// Backend label, e.g. `"tmux"`. Drives the exec invocation in
    /// the runtime.
    pub backend: String,
    /// **Raw** backend-native session name, e.g. `editor`. This is
    /// what gets passed to `tmux attach-session -t` and
    /// `tmux capture-pane -t`. Tracked separately from
    /// `mux.native_id` because the graph form is backend-prefixed
    /// (`tmux:editor`) to keep ids unique across backends, but
    /// the tmux binary needs the unprefixed name.
    pub native_id: String,
}

impl AttachTarget {
    /// `<backend>:<native_id>` for status lines and log entries, e.g.
    /// `tmux:editor`, matching what the backend itself lists.
    pub fn label(&self) -> String {
        format!("{}:{}", self.backend, self.native_id)
    }
}

/// Why an attach attempt cannot proceed. The runtime surfaces these
/// in the status bar; the operator stays in the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachDisabled {
    /// No selection (empty tree).
    NoSelection,
    /// Selected row has no mux link at all.
    UnmuxedSession,
    /// Selected row kind doesn't support attach (e.g. workspace,
    /// repo, worktree group rows).
    UnsupportedRow,
    /// Selected row resolves a mux candidate but no `MuxSessionNode`
    /// exists in the snapshot — should not happen in practice; we
    /// surface a one-line reason rather than panic.
    MuxNodeMissing,
    /// The mux backend is not supported by this build (today only
    /// `tmux`). Carries the backend name for the status bar.
    UnsupportedBackend(String),
    /// The selected mux is the tmux session currently hosting the
    /// Conspectus TUI. Attaching it would nest the TUI inside
    /// itself.
    CurrentTmuxSession(String),
    /// The selected group row (workspace / repo / checkout) has
    /// more than one ambiguous mux scoped to it (ADR 0071), so `a`
    /// doesn't have a sensible default pick. The renderer surfaces
    /// the count as a status hint pointing at the detail section.
    AmbiguousGroupMuxes(usize),
    /// The mux is only in history (ADR 0140); rebuilding it, not
    /// attaching, brings it back.
    MuxGone(String),
}

/// What an action acts on: the entity the operator is looking at,
/// whichever view or pane shows it. Every accelerator and menu resolves
/// its subject here, so a gesture means the same thing wherever it is
/// pressed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Subject {
    /// The graph node the subject is, live or gone. For a pin, the node
    /// it points at.
    pub node: Option<NodeId>,
    /// The history record behind the subject, when it has one.
    pub record: Option<String>,
    /// The pin, when the subject is a pin itself (a Pinned-view row, or
    /// a pin shown in the right pane).
    pub pin: Option<String>,
}

/// The subject of an action: the entity shown in the right pane while
/// it has focus, else the selected left-pane row.
pub fn action_subject(app: &App) -> Subject {
    if app.focus() == crate::tui::app::Focus::Right
        && let Some(node) = right_pane_node(app)
    {
        return subject_of_node(app, &node);
    }
    app.selection()
        .map(|id| subject_of_row(app, id))
        .unwrap_or_default()
}

/// The entity the right pane shows: the link the cursor is on, else the
/// node the pane is focused on.
fn right_pane_node(app: &App) -> Option<NodeId> {
    let explorer = app.explorer()?;
    app.explorer_cursor_row()
        .and_then(|row| explorer.content().drill_target(&row))
        .or_else(|| Some(explorer.id().clone()))
}

fn pin_named<'a>(snapshot: &'a GraphSnapshot, name: &str) -> Option<&'a crate::model::PinNode> {
    crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .find(|pin| pin.id.id == name)
}

/// The history record a gone node stands for.
fn record_of_gone_node(snapshot: &GraphSnapshot, node: &NodeId) -> Option<String> {
    if snapshot.is_live(node) {
        return None;
    }
    if let NodeId::MuxSession(mux) = node {
        return snapshot
            .history
            .muxes
            .iter()
            .find(|record| record.recipe.native_id() == mux.native_id)
            .map(|record| record.meta.id.clone());
    }
    snapshot
        .history
        .by_key(&node.to_string())
        .map(|(_, meta)| meta.id.clone())
}

/// The subject for a graph node. A pin node is the pin itself.
pub fn subject_of_node(app: &App, node: &NodeId) -> Subject {
    let Some(handle) = app.snapshot_handle() else {
        return Subject {
            node: Some(node.clone()),
            ..Subject::default()
        };
    };
    let snapshot = handle.snapshot();
    if let NodeId::Pin(pin) = node {
        return subject_of_pin(snapshot, &pin.id);
    }
    Subject {
        node: Some(node.clone()),
        record: record_of_gone_node(snapshot, node),
        pin: None,
    }
}

fn subject_of_pin(snapshot: &GraphSnapshot, name: &str) -> Subject {
    let pin = pin_named(snapshot, name);
    let node = pin.and_then(|pin| pin.target_node.clone());
    let record = pin
        .and_then(|pin| pin.target.as_deref())
        .and_then(|target| target.strip_prefix(crate::model::PinSlot::HISTORY_PREFIX))
        .map(str::to_string)
        .or_else(|| {
            node.as_ref()
                .and_then(|node| record_of_gone_node(snapshot, node))
        });
    Subject {
        node,
        record,
        pin: Some(name.to_string()),
    }
}

/// The subject for a left-pane row.
pub fn subject_of_row(app: &App, id: &RowId) -> Subject {
    let Some(handle) = app.snapshot_handle() else {
        return Subject::default();
    };
    let snapshot = handle.snapshot();
    match id {
        RowId::Pin { pin_id } => subject_of_pin(snapshot, pin_id),
        RowId::History { record_id } => Subject {
            node: snapshot
                .history
                .meta(record_id)
                .and_then(|(_, meta)| NodeId::parse(&meta.key)),
            record: Some(record_id.clone()),
            pin: None,
        },
        RowId::Hit { session, .. } => subject_of_node(app, session),
        RowId::PaneHit { mux, .. } => subject_of_node(app, mux),
        other => crate::tui::rows::row_node(other)
            .map(|node| subject_of_node(app, node))
            .unwrap_or_default(),
    }
}

/// The pin kind of the subject's entity.
fn subject_kind(snapshot: &GraphSnapshot, subject: &Subject) -> Option<crate::model::PinKind> {
    use crate::model::{HistoryKind, PinKind};
    if let Some(pin) = subject
        .pin
        .as_deref()
        .and_then(|name| pin_named(snapshot, name))
    {
        return Some(pin.kind);
    }
    if let Some(node) = &subject.node
        && let Some(kind) = PinKind::of_node(node)
    {
        return Some(kind);
    }
    let (kind, _) = snapshot.history.meta(subject.record.as_deref()?)?;
    Some(match kind {
        HistoryKind::Mux => PinKind::Mux,
        HistoryKind::Checkout => PinKind::Checkout,
        HistoryKind::Repo => PinKind::Repo,
        HistoryKind::Pr => PinKind::Pr,
    })
}

/// The names of the pins that point at the subject, the subject's own
/// pin first.
pub fn pins_for(snapshot: &GraphSnapshot, subject: &Subject) -> Vec<String> {
    let mut names: Vec<String> = subject.pin.iter().cloned().collect();
    let history_target = subject
        .record
        .as_deref()
        .map(crate::model::PinSlot::history_target);
    for pin in crate::tui::rows::pin_nodes(snapshot) {
        let points_here = subject
            .node
            .as_ref()
            .is_some_and(|node| pin.target_node.as_ref() == Some(node))
            || (history_target.is_some() && pin.target == history_target);
        if points_here && !names.contains(&pin.id.id) {
            names.push(pin.id.id.clone());
        }
    }
    names
}

/// The attach target an entity resolves to. Decides, for the subject
/// the operator is acting on, which mux to attach to; the checks that
/// follow (is it running, is it this very session) are the same for
/// every entity.
fn mux_for_subject(
    snapshot: &GraphSnapshot,
    subject: &Subject,
) -> Result<MuxSessionId, AttachDisabled> {
    if let Some(pin) = &subject.pin {
        // A pin attaches to the live mux it points at, or to the mux
        // hosting the live session it points at.
        return pin_live_mux(snapshot, pin).ok_or_else(|| match pin_status_hint(snapshot, pin) {
            Some(_) => AttachDisabled::MuxGone(pin.clone()),
            None => AttachDisabled::UnmuxedSession,
        });
    }
    let Some(node) = &subject.node else {
        return Err(AttachDisabled::UnsupportedRow);
    };
    match node {
        NodeId::MuxSession(id) => Ok(id.clone()),
        session @ NodeId::AgentSession(_) => {
            preferred_mux_for_session(snapshot, session).ok_or(AttachDisabled::UnmuxedSession)
        }
        // `a` on a workspace, repo, or checkout attaches to the single
        // ambiguous mux scoped to that group, if any.
        group @ (NodeId::Workspace(_) | NodeId::Repo(_) | NodeId::Checkout(_)) => {
            let muxes = crate::tui::rows::ambiguous_muxes_for_group(snapshot, group);
            match muxes.as_slice() {
                [] => Err(AttachDisabled::UnmuxedSession),
                [NodeId::MuxSession(id)] => Ok(id.clone()),
                [_] => Err(AttachDisabled::UnsupportedRow),
                many => Err(AttachDisabled::AmbiguousGroupMuxes(many.len())),
            }
        }
        _ => Err(AttachDisabled::UnsupportedRow),
    }
}

/// Decide whether the current subject is attachable. Pure: works
/// against [`App`] state, does not touch the terminal or the
/// network.
pub fn resolve_attach_target(app: &App) -> Result<AttachTarget, AttachDisabled> {
    let Some(handle) = app.snapshot_handle().cloned() else {
        return Err(AttachDisabled::NoSelection);
    };
    let snapshot = handle.snapshot();
    let subject = action_subject(app);
    if subject == Subject::default() {
        return Err(AttachDisabled::NoSelection);
    }
    let mux_id = mux_for_subject(snapshot, &subject)?;
    attach_target_for_mux(app, &mux_id)
}

/// The attach target for the mux `mux_id`, or why it can't be attached.
pub fn attach_target_for_mux(
    app: &App,
    mux_id: &MuxSessionId,
) -> Result<AttachTarget, AttachDisabled> {
    let Some(handle) = app.snapshot_handle() else {
        return Err(AttachDisabled::NoSelection);
    };
    let snapshot = handle.snapshot();
    let mux_node = snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            crate::model::GraphNode::MuxSession(mux) if &mux.id == mux_id => Some(mux),
            _ => None,
        })
        .ok_or(AttachDisabled::MuxNodeMissing)?;
    if !snapshot.is_live(&NodeId::MuxSession(mux_node.id.clone())) {
        return Err(AttachDisabled::MuxGone(mux_node.session_name().to_string()));
    }

    let target = AttachTarget {
        mux: mux_node.id.clone(),
        backend: mux_node.backend.clone(),
        native_id: mux_node.native_id.clone(),
    };
    // Check backend against the compile-time registered backend
    // list. The self-attach check below stays tmux-specific because
    // `RunConfig::current_tmux_session` is the `$TMUX`-derived
    // name.
    if !crate::discovery::tmux::KNOWN_MUX_BACKENDS.contains(&target.backend.as_str()) {
        return Err(AttachDisabled::UnsupportedBackend(target.backend));
    }
    if target.backend == crate::discovery::tmux::TMUX_BACKEND
        && app
            .config()
            .current_tmux_session
            .as_deref()
            .is_some_and(|current| current == target.native_id)
    {
        return Err(AttachDisabled::CurrentTmuxSession(target.native_id));
    }
    Ok(target)
}

/// The session `v` opens for `node`: the session itself, or the
/// resolver-preferred session linked to a mux.
fn viewer_session_for(snapshot: &GraphSnapshot, node: &NodeId) -> Option<AgentSessionId> {
    match node {
        NodeId::AgentSession(id) => Some(id.clone()),
        mux @ NodeId::MuxSession(_) => preferred_session_for_mux(snapshot, mux),
        _ => None,
    }
}

/// Where the viewer opens for the selected row of the Search view: the
/// session's best hit, or the hit row's own turn. `None` outside the
/// Search view, with the right pane focused, or without a search to
/// show.
pub fn viewer_open_target(app: &App) -> Option<(AgentSessionId, crate::viewer::state::ViewerOpen)> {
    if app.active_view() != crate::tui::View::Search || app.focus() != crate::tui::app::Focus::Left
    {
        return None;
    }
    let content = app.viewed_search()?;
    let (node, turn_index) = match app.selection()? {
        RowId::Hit { session, turn } => (session, *turn),
        RowId::AgentSession(node @ NodeId::AgentSession(id)) => {
            let hits = content.hits().get(&id.harness_key, &id.session_key)?;
            (node, hits.best.turn_index)
        }
        _ => return None,
    };
    let session = viewer_session_for(app.snapshot_handle()?.snapshot(), node)?;
    Some((
        session,
        crate::viewer::state::ViewerOpen {
            query: content.text.clone(),
            turn_index,
        },
    ))
}

/// Resolve the viewer target for the current subject. Mirrors
/// [`resolve_attach_target`] so `Enter` and the `v` accelerator can
/// pick the right session id whether the subject is an agent session, a
/// mux, or a pin on either, in any view or in the right pane.
///
/// For agent sessions the session id wins directly. For muxes we walk
/// active `LinkedToMux` candidates and pick the resolver-preferred
/// linked session (highest provenance precedence, breaking ties on
/// confidence then link id). Subjects with no session report
/// [`ViewerDisabled::UnsupportedRow`] — the caller surfaces a one-line
/// status bar reason.
pub fn resolve_view_session(app: &App) -> Result<AgentSessionId, ViewerDisabled> {
    let subject = action_subject(app);
    if subject == Subject::default() {
        return Err(ViewerDisabled::NoSelection);
    }
    let handle = app
        .snapshot_handle()
        .ok_or(ViewerDisabled::UnsupportedRow)?;
    subject
        .node
        .as_ref()
        .and_then(|node| viewer_session_for(handle.snapshot(), node))
        .ok_or(ViewerDisabled::UnsupportedRow)
}

/// The live mux the pin `pin_id` points at, if any.
pub fn pin_live_mux(snapshot: &GraphSnapshot, pin_id: &str) -> Option<MuxSessionId> {
    let pin = pin_named(snapshot, pin_id)?;
    match crate::tui::rows::live_pin_target(snapshot, pin)? {
        NodeId::MuxSession(id) => Some(id),
        session @ NodeId::AgentSession(_) => preferred_mux_for_session(snapshot, &session),
        _ => None,
    }
}

/// The history record behind the selected left-pane row, when it has
/// one: a History row, or a ghost mux in the Mux view.
pub fn selected_record(app: &App) -> Option<String> {
    let id = app.selection()?;
    matches!(
        id,
        RowId::History { .. } | RowId::MuxSession(_) | RowId::Nested { .. }
    )
    .then(|| subject_of_row(app, id).record)
    .flatten()
}

/// What the subject can be pinned as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinSubject {
    /// A node in the graph (live, or a ghost without a record).
    Node(NodeId),
    /// A history record, by id.
    Record(String),
}

/// The entity a pin could point at, or the reason it can't be pinned.
pub fn pin_subject(app: &App) -> Result<PinSubject, String> {
    if app.selection().is_none() && app.focus() == crate::tui::app::Focus::Left {
        return Err("nothing is selected".to_string());
    }
    let subject = action_subject(app);
    if let Some(record) = subject.record {
        return Ok(PinSubject::Record(record));
    }
    let node = subject.node.ok_or("this row can't be pinned")?;
    match crate::model::PinKind::of_node(&node) {
        Some(_) => Ok(PinSubject::Node(node)),
        None => Err("a runtime process is ephemeral and can't be pinned".to_string()),
    }
}

/// The pins that already point at `subject`.
pub fn slots_pinning(snapshot: &GraphSnapshot, subject: &PinSubject) -> Vec<String> {
    let subject = match subject {
        PinSubject::Record(id) => Subject {
            record: Some(id.clone()),
            ..Subject::default()
        },
        PinSubject::Node(node) => Subject {
            node: Some(node.clone()),
            ..Subject::default()
        },
    };
    pins_for(snapshot, &subject)
}

/// The view an entity lives in: the one that lists it, where a gone
/// entity is only listed in History. An entity no view lists (a live
/// fork, say) answers Sessions, where revealing it reports that it
/// isn't there.
pub fn natural_view(snapshot: &GraphSnapshot, node: &NodeId) -> crate::tui::View {
    use crate::tui::View;
    let live = snapshot.is_live(node);
    match node {
        NodeId::MuxSession(_) => View::Mux,
        NodeId::Pin(_) => View::Pinned,
        NodeId::ForgePr(_) if live => View::Prs,
        _ if !live => View::History,
        _ => View::Sessions,
    }
}

/// The entities pin `name` could point at instead: the other live
/// entities of its kind, then the gone records of that kind, each with
/// what the operator needs to tell them apart.
pub fn retarget_candidates(
    snapshot: &GraphSnapshot,
    name: &str,
) -> Vec<crate::tui::widgets::picker::PickerItem> {
    use crate::model::{HistoryKind, PinKind};
    use crate::tui::widgets::picker::PickerItem;
    let Some(pin) = pin_named(snapshot, name) else {
        return Vec::new();
    };
    let current = pin.target_node.clone();
    let current_record = pin
        .target
        .as_deref()
        .and_then(|target| target.strip_prefix(crate::model::PinSlot::HISTORY_PREFIX));
    let mut items = Vec::new();
    for node in snapshot.live_nodes() {
        let id = node.id();
        if PinKind::of_node(&id) != Some(pin.kind) || Some(&id) == current.as_ref() {
            continue;
        }
        items.push(
            PickerItem::new(
                crate::tui::explorer::title_line(snapshot, node),
                id.to_string(),
            )
            .detail("running"),
        );
    }
    let history_kind = match pin.kind {
        PinKind::Mux => Some(HistoryKind::Mux),
        PinKind::Checkout => Some(HistoryKind::Checkout),
        PinKind::Repo => Some(HistoryKind::Repo),
        _ => None,
    };
    if let Some(history_kind) = history_kind {
        for (kind, meta) in snapshot.history.metas() {
            if kind != history_kind || Some(meta.id.as_str()) == current_record {
                continue;
            }
            // A record whose entity is running is already offered above.
            if snapshot.history.presence_of(&meta.id).is_none() {
                continue;
            }
            let label = snapshot
                .history
                .label_of(&meta.id)
                .unwrap_or_else(|| meta.display.label.clone());
            let detail = match &meta.display.context {
                Some(context) => format!(
                    "{} · {context}",
                    crate::model::gone_word(history_kind.node_kind())
                ),
                None => crate::model::gone_word(history_kind.node_kind()).to_string(),
            };
            items.push(PickerItem::new(label, meta.id.clone()).detail(detail));
        }
    }
    items
}

/// The mux that rebuilding `reference` (a pin name or a history record
/// id) brings back, when it names one: where to attach once it is up,
/// and its tmux socket. Read from the pin's effective recipe or the
/// record, so it is known before anything has been rebuilt.
pub fn mux_to_rebuild(
    snapshot: &GraphSnapshot,
    reference: &str,
) -> Option<(AttachTarget, Option<String>)> {
    let recipe = if let Some(pin) = pin_named(snapshot, reference) {
        if pin.kind != crate::model::PinKind::Mux {
            return None;
        }
        crate::model::MuxRecipe {
            backend: crate::discovery::tmux::TMUX_BACKEND.to_string(),
            socket: pin.text("socket").map(str::to_string),
            name: pin.text("name")?.to_string(),
            ..Default::default()
        }
    } else {
        snapshot.history.mux(reference)?.recipe.clone()
    };
    let socket = crate::model::effective_socket(recipe.socket.as_deref()).map(str::to_string);
    Some((
        AttachTarget {
            mux: MuxSessionId::new(recipe.native_id()),
            backend: recipe.backend.clone(),
            native_id: recipe.name,
        },
        socket,
    ))
}

/// The live mux `reference` (a pin name or history record id) is in
/// now: the pin's mux, or the mux hosting the pin's session.
pub fn attached_mux_of(
    snapshot: &GraphSnapshot,
    reference: &str,
) -> Option<(AttachTarget, Option<String>)> {
    let id = if pin_named(snapshot, reference).is_some() {
        pin_live_mux(snapshot, reference)?
    } else {
        let record = snapshot.history.mux(reference)?;
        match crate::history::project::live_mux_for(snapshot, record)? {
            NodeId::MuxSession(id) => id,
            _ => return None,
        }
    };
    let mux = snapshot.nodes.iter().find_map(|node| match node {
        crate::model::GraphNode::MuxSession(mux) if mux.id == id => Some(mux),
        _ => None,
    })?;
    Some((
        AttachTarget {
            mux: mux.id.clone(),
            backend: mux.backend.clone(),
            native_id: mux.native_id.clone(),
        },
        mux.socket.clone().filter(|socket| socket != "default"),
    ))
}

/// What a rebuild menu offers for an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RebuildOffer {
    /// Nothing to rebuild: a PR, or an entity with no plan at all.
    Nothing,
    /// Only the plan: the entity is running (the plan is what a rebuild
    /// would take if it had stopped), or couldn't be checked.
    PlanOnly,
    /// Bring it back, or see what that would run.
    Full,
}

fn rebuild_offer(
    kind: Option<crate::model::PinKind>,
    status: crate::model::SlotStatus,
) -> RebuildOffer {
    use crate::model::{PinKind, SlotStatus};
    if !matches!(
        kind,
        Some(PinKind::Mux | PinKind::Session | PinKind::Checkout | PinKind::Repo)
    ) {
        return RebuildOffer::Nothing;
    }
    match status {
        SlotStatus::Live | SlotStatus::Unknown => RebuildOffer::PlanOnly,
        SlotStatus::Gone | SlotStatus::Missing | SlotStatus::Planned => RebuildOffer::Full,
    }
}

/// The line that tops a rebuild plan for an entity that is running:
/// what Enter does, and that the plan below is hypothetical. `None`
/// for an entity that isn't running. `reference` is what the plan was
/// asked for: a pin name, a history record id, or a node id.
pub fn running_note(snapshot: &GraphSnapshot, reference: &str) -> Option<String> {
    use crate::model::{PinKind, SlotStatus};
    let (kind, live) = if let Some(pin) = pin_named(snapshot, reference) {
        (pin.kind, pin.status == SlotStatus::Live)
    } else if let Some(node) = NodeId::parse(reference) {
        (PinKind::of_node(&node)?, snapshot.is_live(&node))
    } else {
        let (_, meta) = snapshot.history.meta(reference)?;
        let node = NodeId::parse(&meta.key)?;
        (PinKind::of_node(&node)?, snapshot.is_live(&node))
    };
    if !live {
        return None;
    }
    Some(match kind {
        PinKind::Mux => format!(
            "`{reference}` is running, so Enter attaches to it. This is what rebuilding it \
             would take if it had stopped."
        ),
        PinKind::Session => format!(
            "`{reference}` is a live session. This is how it would be resumed into a new \
             mux if its mux had stopped."
        ),
        _ => format!(
            "`{reference}` exists, so there is nothing to rebuild. This is what rebuilding it \
             would take if it were gone."
        ),
    })
}

/// The `.` menu for the subject, or `None` when it has no actions. The
/// entries depend on the entity and whether a pin points at it, never on
/// the view that opened the menu, so the same entity offers the same
/// verbs everywhere. The History view always has a menu: even with
/// nothing selected it offers Clean up.
pub fn row_menu_for(app: &App) -> Option<ActionMenu<crate::tui::Msg>> {
    use crate::model::SlotStatus;
    use crate::tui::Msg;
    let snapshot = app.snapshot_handle()?.snapshot();
    let in_history = app.active_view() == crate::tui::View::History;
    let subject = action_subject(app);
    let pins = pins_for(snapshot, &subject);
    let kind = subject_kind(snapshot, &subject);
    let mut entries = Vec::new();

    // What a rebuild would do depends on the pin's status, or on the
    // entity being gone when only a record is left.
    let (status, reference) =
        if let Some(pin) = pins.first().and_then(|name| pin_named(snapshot, name)) {
            (pin.status, Some(pin.id.id.clone()))
        } else {
            let live = subject
                .node
                .as_ref()
                .is_some_and(|node| snapshot.is_live(node));
            (
                if live {
                    SlotStatus::Live
                } else {
                    SlotStatus::Gone
                },
                // A record names it; failing that, a live node does.
                subject.record.clone().or_else(|| {
                    subject
                        .node
                        .as_ref()
                        .filter(|_| live)
                        .map(ToString::to_string)
                }),
            )
        };
    if pins.is_empty()
        && let Ok(pinnable) = pin_subject(app)
    {
        let msg = match pinnable {
            PinSubject::Node(node) => Msg::PinNode(node),
            PinSubject::Record(id) => Msg::PinRecord(id),
        };
        // `p` pins from the menu as it does from the list.
        entries.push(
            Entry::new("Pin", msg)
                .detail("pin this")
                .shortcut(tuikit::keys::ch('p')),
        );
    }
    if let Some(reference) = reference {
        let offer = rebuild_offer(kind, status);
        if offer == RebuildOffer::Full {
            entries.push(
                Entry::new("Rebuild", Msg::RebuildReference(reference.clone()))
                    .detail("bring it back without attaching (Enter also attaches)"),
            );
        }
        if offer != RebuildOffer::Nothing {
            let note = if offer == RebuildOffer::PlanOnly {
                "the commands that would recreate it, as if it had stopped"
            } else {
                "what rebuild would run, without running it"
            };
            entries.push(
                Entry::new("Show rebuild plan", Msg::ShowRebuildPlan(reference)).detail(note),
            );
        }
    }
    if let Some(name) = pins.first() {
        pin_management_entries(snapshot, &subject, name, app.active_view(), &mut entries);
        for extra in pins.iter().skip(1) {
            entries.push(unpin_entry(extra));
        }
    }
    if let Some(id) = subject.record.clone().filter(|_| subject.pin.is_none()) {
        let note = if pins.is_empty() {
            "remove the record from history".to_string()
        } else {
            format!(
                "remove the record; {} would lose its target",
                pins.join(", ")
            )
        };
        entries.push(
            Entry::new("Forget", Msg::ForgetRecord(id))
                .detail(note)
                .confirm(Confirm::Inline),
        );
    }
    if in_history {
        entries.push(
            Entry::new("Clean up…", Msg::OpenCleanUp)
                .detail("forget gone records past their reap age"),
        );
    }
    let title = menu_title(app, &subject, &pins);
    (!entries.is_empty()).then(|| ActionMenu::new(title, entries))
}

/// The entry that removes the pin `name`, in the row menu and in the menu
/// `p` opens when several pins point at the row.
pub(crate) fn unpin_entry(name: &str) -> Entry<crate::tui::Msg> {
    Entry::new(
        format!("Unpin {name}"),
        crate::tui::Msg::Unpin(name.to_string()),
    )
    .detail("remove the pin; the entity and its history stay")
}

/// The `m` menu: the two ways to start a new mux, each opening its form.
/// Their shortcuts are the keys that open the same forms from the list.
pub fn mux_menu() -> ActionMenu<crate::tui::Msg> {
    use crate::tui::Msg;
    use tuikit::keys::ch;
    ActionMenu::new(
        "Mux",
        vec![
            Entry::new("New tmux session (bare shell)", Msg::OpenNewMuxForm)
                .detail("shell only, no harness")
                .shortcut(ch('n')),
            Entry::new("Launch harness in new mux…", Msg::OpenMuxLaunchForm)
                .detail("spawn harness; optionally pin it")
                .shortcut(ch('N')),
        ],
    )
}

/// The verbs on a pin: edit, rename, retarget, refresh its snapshot,
/// reveal its entity, unpin.
fn pin_management_entries(
    snapshot: &GraphSnapshot,
    subject: &Subject,
    name: &str,
    current_view: crate::tui::View,
    entries: &mut Vec<Entry<crate::tui::Msg>>,
) {
    use crate::tui::Msg;
    entries.push(
        Entry::new("Edit…", Msg::OpenSlotEdit(name.to_string()))
            .detail("change the name, cwd, harness, arguments, and more"),
    );
    entries.push(
        Entry::new("Rename…", Msg::OpenSlotRename(name.to_string()))
            .detail("change the pin name (a live mux is renamed with it)"),
    );
    entries.push(
        Entry::new("Retarget…", Msg::OpenSlotRetarget(name.to_string()))
            .detail("point it at another entity of the same kind"),
    );
    entries.push(
        Entry::new("Refresh snapshot", Msg::RefreshSlot(name.to_string()))
            .detail("copy the entity's current recipe into the pin"),
    );
    let elsewhere = subject
        .node
        .as_ref()
        .is_some_and(|node| natural_view(snapshot, node) != current_view);
    if elsewhere {
        entries.push(
            Entry::new("Reveal in its view", Msg::RevealSlot(name.to_string()))
                .detail("jump to the entity in Sessions, Mux, PRs, or History"),
        );
    }
    entries.push(unpin_entry(name));
}

fn menu_title(app: &App, subject: &Subject, pins: &[String]) -> String {
    let Some(handle) = app.snapshot_handle() else {
        return "Actions".to_string();
    };
    let snapshot = handle.snapshot();
    if let Some(name) = &subject.pin {
        return format!("pin {name}");
    }
    if let Some(id) = &subject.record
        && let Some((kind, meta)) = snapshot.history.meta(id)
    {
        let label = snapshot
            .history
            .label_of(id)
            .unwrap_or_else(|| meta.display.label.clone());
        return format!("{} {label}", kind.as_str());
    }
    match &subject.node {
        Some(NodeId::MuxSession(mux)) => format!("mux {}", mux.native_id),
        Some(NodeId::AgentSession(session)) => {
            format!(
                "session {}",
                session.session_key.chars().take(8).collect::<String>()
            )
        }
        Some(NodeId::ForgePr(pr)) => format!("PR #{}", pr.number),
        _ => pins
            .first()
            .map_or_else(|| "Actions".to_string(), |name| format!("pin {name}")),
    }
}

/// What `Enter` does on a pin, a History record, or a ghost mux, where
/// attaching isn't the answer. `None` means the subject attaches (or is
/// viewed) like any native row.
///
/// A gone mux or session comes back and is attached, the way a launch
/// always attached; a gone worktree or repo comes back and the
/// operator stays put.
pub fn default_effect(app: &App) -> Option<crate::tui::Effect> {
    use crate::model::{PinKind, SlotStatus};
    use crate::tui::Effect;
    use crate::tui::effect::ExecSpec;
    use tuikit::feedback::Feedback;
    let snapshot = app.snapshot_handle()?.snapshot();
    let subject = action_subject(app);
    let rebuild = |reference: String, kind: PinKind| {
        Some(Effect::Exec(ExecSpec::Rebuild {
            reference,
            attach: matches!(kind, PinKind::Mux | PinKind::Session),
        }))
    };
    // An entity a pin points at comes back the way the pin says (its
    // overrides included), from whichever view Enter was pressed in.
    let pin_name = subject
        .pin
        .clone()
        .or_else(|| pins_for(snapshot, &subject).into_iter().next());
    if let Some(name) = &pin_name {
        let pin = pin_named(snapshot, name)?;
        return match (pin.status, pin.kind) {
            (SlotStatus::Unknown, _) => Some(Effect::Toast(Feedback::warning(format!(
                "pin `{name}`: couldn't check whether its target exists; refresh and try again"
            )))),
            (SlotStatus::Live, PinKind::Mux) => None,
            (SlotStatus::Live, PinKind::Session) => {
                // A session in a mux attaches; one outside any mux is
                // opened in the viewer, as in the Sessions view.
                let node = pin.target_node.as_ref()?;
                if preferred_mux_for_session(snapshot, node).is_some() {
                    None
                } else {
                    viewer_session_for(snapshot, node)
                        .map(|session| Effect::Exec(ExecSpec::ViewSession(session, None)))
                }
            }
            (SlotStatus::Live, PinKind::Pr) => Some(Effect::Toast(match pin.text("url") {
                Some(url) => Feedback::info(format!("pin `{name}`: {url}")),
                None => Feedback::warning(format!("pin `{name}`: this PR has no URL on record")),
            })),
            (SlotStatus::Live, kind) => Some(Effect::Toast(Feedback::warning(format!(
                "pin `{name}` is a live {}; nothing to rebuild",
                kind.as_str()
            )))),
            (
                SlotStatus::Gone | SlotStatus::Missing | SlotStatus::Planned,
                kind @ (PinKind::Mux | PinKind::Session | PinKind::Checkout | PinKind::Repo),
            ) => rebuild(name.clone(), kind),
            (_, kind) => Some(Effect::Toast(Feedback::warning(format!(
                "pin `{name}`: a {} can't be rebuilt",
                kind.as_str()
            )))),
        };
    }
    let kind = subject_kind(snapshot, &subject);
    let record = subject.record?;
    match kind {
        Some(kind @ (PinKind::Mux | PinKind::Checkout | PinKind::Repo)) => rebuild(record, kind),
        _ => Some(Effect::Toast(Feedback::warning(
            "a PR record can't be rebuilt; it only remembers the PR",
        ))),
    }
}

/// One-line status for the slot `pin_id` when it needs the operator's
/// attention: the target is gone, unchecked, unresolved, or not built
/// yet. `None` for a live slot.
pub fn pin_status_hint(snapshot: &GraphSnapshot, pin_id: &str) -> Option<String> {
    let pin = crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .find(|pin| pin.id.id == pin_id)?;
    let name = crate::invocation::invoked_name();
    let rebuild = format!("{name} rebuild {pin_id}");
    match pin.status {
        crate::model::SlotStatus::Live => match pin.kind {
            crate::model::PinKind::Mux | crate::model::PinKind::Session => None,
            crate::model::PinKind::Pr => {
                Some(format!("pin `{pin_id}` is a live PR · Enter shows its URL"))
            }
            kind => Some(format!(
                "pin `{pin_id}` is a live {} · nothing to rebuild",
                kind.as_str()
            )),
        },
        crate::model::SlotStatus::Gone => Some(format!(
            "pin `{pin_id}` is {} · Enter runs `{rebuild}` to bring it back",
            crate::model::SlotStatus::Gone.label_for(pin.kind)
        )),
        crate::model::SlotStatus::Unknown => Some(format!(
            "pin `{pin_id}`: couldn't check whether its target exists"
        )),
        crate::model::SlotStatus::Missing => Some(format!(
            "pin `{pin_id}`: target not found · `{name} pin retarget {pin_id} <target>`, or Enter runs `{rebuild}`"
        )),
        crate::model::SlotStatus::Planned => Some(format!(
            "pin `{pin_id}` isn't built yet · Enter runs `{rebuild}`"
        )),
    }
}

/// Inverse of [`preferred_mux_for_session`]: find the resolver-
/// preferred agent session linked to `mux`. Used by the `v`
/// accelerator and the default-action dispatcher so a mux row's
/// "view" complements its "attach".
fn preferred_session_for_mux(snapshot: &GraphSnapshot, mux: &NodeId) -> Option<AgentSessionId> {
    let mut candidates: Vec<&GraphLink> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && matches!(link.state, crate::model::LinkState::Active)
                && matches!(link.target_node_id(), Some(t) if t == mux)
        })
        .collect();
    candidates.sort_by(|left, right| {
        right
            .provenance
            .precedence()
            .cmp(&left.provenance.precedence())
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.id.cmp(&right.id))
    });
    let chosen = candidates.into_iter().next()?;
    match &chosen.source {
        NodeId::AgentSession(id) => Some(id.clone()),
        _ => None,
    }
}

/// Walk active `LinkedToMux` candidates for `session`, returning the
/// resolver-preferred mux id. Mirrors the row-tree's "preferred"
/// choice so the row glyph and the attach target agree.
pub(crate) fn preferred_mux_for_session(
    snapshot: &GraphSnapshot,
    session: &NodeId,
) -> Option<MuxSessionId> {
    snapshot.preferred_mux_for(session)
}

/// Human-readable reason text for [`AttachDisabled`]. The runtime
/// pipes this into the status bar.
pub fn attach_disabled_reason(reason: &AttachDisabled) -> String {
    match reason {
        AttachDisabled::NoSelection => "attach: no row selected".to_string(),
        AttachDisabled::UnsupportedRow => {
            "attach: selected row has no mux to attach to".to_string()
        }
        AttachDisabled::UnmuxedSession => "attach: session is not attached to any mux".to_string(),
        AttachDisabled::MuxNodeMissing => {
            "attach: mux node missing from snapshot — try `r` to refresh".to_string()
        }
        AttachDisabled::UnsupportedBackend(name) => {
            // Report the registered backend set
            // instead of a hardcoded "only tmux."
            let registered = crate::discovery::tmux::KNOWN_MUX_BACKENDS.join(", ");
            format!("attach: mux backend `{name}` not registered (available: {registered})")
        }
        AttachDisabled::CurrentTmuxSession(name) => {
            format!("attach: refusing to attach current tmux session `{name}`")
        }
        AttachDisabled::AmbiguousGroupMuxes(n) => {
            format!("attach: {n} ambiguous muxes here — pick one in the detail pane")
        }
        AttachDisabled::MuxGone(name) => {
            format!(
                "attach: `{name}` isn't running · `{} rebuild {name}` brings it back",
                crate::invocation::invoked_name()
            )
        }
    }
}

/// Pull the mux node out for callers that need its full row (e.g.
/// status bar messages).
pub fn lookup_mux_node<'a>(
    snapshot: &'a GraphSnapshot,
    target: &AttachTarget,
) -> Option<&'a MuxSessionNode> {
    snapshot.nodes.iter().find_map(|node| match node {
        crate::model::GraphNode::MuxSession(mux) if mux.id == target.mux => Some(mux),
        _ => None,
    })
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;
