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
use crate::tui::rows::{RowId, RowKind};
use crate::tui::viewer::ViewerDisabled;

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

/// Decide whether the current selection is attachable. Pure: works
/// against [`App`] state, does not touch the terminal or the
/// network.
pub fn resolve_attach_target(app: &App) -> Result<AttachTarget, AttachDisabled> {
    let Some(handle) = app.snapshot_handle().cloned() else {
        return Err(AttachDisabled::NoSelection);
    };
    let snapshot = handle.snapshot();
    let Some(selection) = app.selection() else {
        return Err(AttachDisabled::NoSelection);
    };
    let row = app
        .tree()
        .rows
        .iter()
        .find(|r| &r.id == selection)
        .ok_or(AttachDisabled::NoSelection)?;

    let mux_id = match (&row.kind, selection) {
        // A slot row attaches to the live mux it points at, or to the
        // mux hosting the live session it points at.
        (_, RowId::Pin { pin_id }) => match pin_live_mux(snapshot, pin_id) {
            Some(id) => id,
            None => {
                return Err(match pin_status_hint(snapshot, pin_id) {
                    Some(_) => AttachDisabled::MuxGone(pin_id.clone()),
                    None => AttachDisabled::UnmuxedSession,
                });
            }
        },
        // An entity listed under a slot attaches like its own row.
        (_, RowId::Nested { node, .. }) => match node {
            NodeId::MuxSession(id) => id.clone(),
            session @ NodeId::AgentSession(_) => match preferred_mux_for_session(snapshot, session)
            {
                Some(id) => id,
                None => return Err(AttachDisabled::UnmuxedSession),
            },
            _ => return Err(AttachDisabled::UnsupportedRow),
        },
        (RowKind::AgentSessionMuxCandidate(candidate), _) => candidate.mux.clone(),
        (RowKind::AgentSession(session), _) => {
            let session_node = NodeId::AgentSession(session.session.clone());
            match preferred_mux_for_session(snapshot, &session_node) {
                Some(id) => id,
                None => return Err(AttachDisabled::UnmuxedSession),
            }
        }
        (RowKind::Group(group), RowId::Group(NodeId::MuxSession(id))) => {
            // The sessions view doesn't have mux group rows in v1,
            // but other views will; honor a mux node id directly.
            let _ = group;
            id.clone()
        }
        (RowKind::MuxSession(mux), RowId::MuxSession(NodeId::MuxSession(id))) => {
            let _ = mux;
            id.clone()
        }
        // A ghost mux row: the liveness check below refuses it by
        // name instead of the row being unsupported.
        (RowKind::Mixed(_), RowId::MuxSession(NodeId::MuxSession(id))) => id.clone(),
        // ADR 0071: `a` on a workspace/repo/checkout row attaches
        // to the single ambiguous mux scoped to that group, if any.
        // Zero → UnmuxedSession (no attach possible). Multiple →
        // AmbiguousGroupMuxes so the status bar points the operator
        // at the detail section.
        (
            RowKind::Group(_),
            RowId::Group(node @ (NodeId::Workspace(_) | NodeId::Repo(_) | NodeId::Checkout(_))),
        ) => {
            let muxes = crate::tui::detail::ambiguous_muxes_for_group(snapshot, node);
            match muxes.len() {
                0 => return Err(AttachDisabled::UnmuxedSession),
                1 => match &muxes[0] {
                    NodeId::MuxSession(id) => id.clone(),
                    _ => return Err(AttachDisabled::UnsupportedRow),
                },
                n => return Err(AttachDisabled::AmbiguousGroupMuxes(n)),
            }
        }
        _ => return Err(AttachDisabled::UnsupportedRow),
    };

    let mux_node = snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            crate::model::GraphNode::MuxSession(mux) if mux.id == mux_id => Some(mux),
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

/// Resolve the viewer target for the current selection. Mirrors
/// [`resolve_attach_target`] so `Enter` and the `v`
/// accelerator can pick the right session id whether the cursor
/// sits on an agent session row or a mux row.
///
/// For agent session rows the selection's session id wins directly.
/// For mux rows we walk active `LinkedToMux` candidates and pick the
/// resolver-preferred linked session (highest provenance precedence,
/// breaking ties on confidence then link id). Mux rows with no
/// linked sessions report [`ViewerDisabled::UnsupportedRow`] — the
/// caller surfaces a one-line status bar reason.
pub fn resolve_view_session(app: &App) -> Result<AgentSessionId, ViewerDisabled> {
    let Some(selection) = app.selection() else {
        return Err(ViewerDisabled::NoSelection);
    };
    let row = app
        .tree()
        .rows
        .iter()
        .find(|r| &r.id == selection)
        .ok_or(ViewerDisabled::NoSelection)?;
    match &row.kind {
        RowKind::AgentSession(session) => Ok(session.session.clone()),
        RowKind::MuxSession(mux_row) => {
            let handle = app
                .snapshot_handle()
                .ok_or(ViewerDisabled::UnsupportedRow)?;
            let snapshot = handle.snapshot();
            let mux_node_id = NodeId::MuxSession(mux_row.mux.clone());
            preferred_session_for_mux(snapshot, &mux_node_id).ok_or(ViewerDisabled::UnsupportedRow)
        }
        _ => Err(ViewerDisabled::UnsupportedRow),
    }
}

/// The live mux the slot `pin_id` points at, if any.
pub fn pin_live_mux(snapshot: &GraphSnapshot, pin_id: &str) -> Option<MuxSessionId> {
    let pin = crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .find(|pin| pin.id.id == pin_id)?;
    match crate::tui::rows::live_pin_target(snapshot, pin)? {
        NodeId::MuxSession(id) => Some(id),
        session @ NodeId::AgentSession(_) => preferred_mux_for_session(snapshot, &session),
        _ => None,
    }
}

/// The history record behind the selected row, when it has one: a
/// History row, or a ghost mux in the Mux view.
pub fn selected_record(app: &App) -> Option<String> {
    let selection = app.selection()?;
    if let RowId::History { record_id } = selection {
        return Some(record_id.clone());
    }
    let snapshot = app.snapshot_handle()?.snapshot();
    let RowId::MuxSession(node @ NodeId::MuxSession(mux)) = selection else {
        return None;
    };
    if snapshot.is_live(node) {
        return None;
    }
    snapshot
        .history
        .muxes
        .iter()
        .find(|record| record.recipe.native_id() == mux.native_id)
        .map(|record| record.meta.id.clone())
}

/// What the selected row can be pinned as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinSubject {
    /// A node in the graph (live, or a ghost without a record).
    Node(NodeId),
    /// A history record, by id.
    Record(String),
}

/// The entity under the cursor that a slot could point at, or the
/// reason it can't be pinned.
pub fn pin_subject(app: &App) -> Result<PinSubject, String> {
    let selection = app.selection().ok_or("nothing is selected")?;
    if let Some(record) = selected_record(app) {
        return Ok(PinSubject::Record(record));
    }
    let node = crate::tui::rows::row_node(selection).ok_or("this row can't be pinned")?;
    match crate::model::PinKind::of_node(node) {
        Some(_) => Ok(PinSubject::Node(node.clone())),
        None => Err("a runtime process is ephemeral and can't be pinned".to_string()),
    }
}

/// The slots that already point at `subject`.
pub fn slots_pinning(snapshot: &GraphSnapshot, subject: &PinSubject) -> Vec<String> {
    match subject {
        PinSubject::Record(id) => crate::tui::rows::history::pinned_by(snapshot, id),
        PinSubject::Node(node) => crate::tui::rows::pin_nodes(snapshot)
            .into_iter()
            .filter(|pin| {
                pin.target_node.as_ref() == Some(node)
                    || pin.target.as_deref() == Some(node.to_string().as_str())
            })
            .map(|pin| pin.id.id.clone())
            .collect(),
    }
}

/// The row menu entries that pin or unpin `subject`.
fn pin_entries(
    snapshot: &GraphSnapshot,
    subject: &PinSubject,
) -> Vec<crate::tui::widgets::row_menu::RowMenuEntry> {
    use crate::tui::Msg;
    use crate::tui::widgets::row_menu::RowMenuEntry;
    let slots = slots_pinning(snapshot, subject);
    if slots.is_empty() {
        let msg = match subject {
            PinSubject::Node(node) => Msg::PinNode(node.clone()),
            PinSubject::Record(id) => Msg::PinRecord(id.clone()),
        };
        return vec![RowMenuEntry::new(
            "Pin",
            "add a slot that points at this (p)",
            msg,
        )];
    }
    slots
        .into_iter()
        .map(|slot| {
            RowMenuEntry::new(
                format!("Unpin {slot}"),
                "remove the slot; the entity and its history stay",
                Msg::Unpin(slot),
            )
            .confirming()
        })
        .collect()
}

/// The `.` menu for the selected row, or `None` when the row has no
/// actions. The History view always has one: even with nothing
/// selected it offers Clean up.
pub fn row_menu_for(app: &App) -> Option<crate::tui::widgets::row_menu::RowMenuState> {
    use crate::tui::Msg;
    use crate::tui::widgets::row_menu::{RowMenuEntry, RowMenuState};
    let snapshot = app.snapshot_handle()?.snapshot();
    let in_history = app.active_view() == crate::tui::View::History;
    let mut entries = Vec::new();
    let mut title = "Actions".to_string();
    if let Some(RowId::Pin { pin_id }) = app.selection() {
        return slot_menu(snapshot, pin_id);
    }
    if let Some(id) = selected_record(app)
        && let Some((kind, meta)) = snapshot.history.meta(&id)
    {
        title = format!(
            "{} {}",
            kind.as_str(),
            snapshot
                .history
                .label_of(&id)
                .unwrap_or_else(|| meta.display.label.clone())
        );
        let pins = crate::tui::rows::history::pinned_by(snapshot, &id);
        let forget_note = if pins.is_empty() {
            "remove the record from history".to_string()
        } else {
            format!(
                "remove the record; {} would lose its target",
                pins.join(", ")
            )
        };
        entries.push(RowMenuEntry::new(
            "Rebuild",
            "bring it back (conspectus rebuild)",
            Msg::RebuildReference(id.clone()),
        ));
        entries.push(RowMenuEntry::new(
            "Show rebuild plan",
            "what rebuild would run, without running it",
            Msg::ShowRebuildPlan(id.clone()),
        ));
        entries.push(RowMenuEntry::new("Forget", forget_note, Msg::ForgetRecord(id)).confirming());
    } else if let Some(row) = app
        .tree()
        .rows
        .iter()
        .find(|r| Some(&r.id) == app.selection())
    {
        title = row_title(row);
    }
    if let Ok(subject) = pin_subject(app) {
        let mut pin = pin_entries(snapshot, &subject);
        // Pin/Unpin leads the menu where it is the main verb.
        if entries.is_empty() {
            pin.append(&mut entries);
            entries = pin;
        } else {
            entries.splice(1..1, pin);
        }
    }
    if in_history {
        entries.push(RowMenuEntry::new(
            "Clean up…",
            "forget gone records past their reap age",
            Msg::OpenCleanUp,
        ));
    }
    (!entries.is_empty()).then(|| RowMenuState::new(title, entries))
}

fn row_title(row: &crate::tui::rows::Row) -> String {
    match &row.kind {
        RowKind::MuxSession(mux) => format!("mux {}", mux.native_id),
        RowKind::AgentSession(session) => format!("session {}", session.short_id),
        RowKind::Pr(pr) => format!("PR #{}", pr.pr_number),
        _ => "row".to_string(),
    }
}

/// The menu of a slot row in the Pinned view.
fn slot_menu(
    snapshot: &GraphSnapshot,
    slot: &str,
) -> Option<crate::tui::widgets::row_menu::RowMenuState> {
    use crate::tui::Msg;
    use crate::tui::widgets::row_menu::{RowMenuEntry, RowMenuState};
    crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .find(|pin| pin.id.id == slot)?;
    let slot = slot.to_string();
    let entries = vec![
        RowMenuEntry::new(
            "Rebuild",
            "bring it back, or attach if it is up (conspectus rebuild)",
            Msg::RebuildReference(slot.clone()),
        ),
        RowMenuEntry::new(
            "Show rebuild plan",
            "what rebuild would run, without running it",
            Msg::ShowRebuildPlan(slot.clone()),
        ),
        RowMenuEntry::new(
            "Edit overrides…",
            "set or clear a field: name=value",
            Msg::OpenSlotEdit(slot.clone()),
        ),
        RowMenuEntry::new(
            "Rename…",
            "change the slot name (a live mux is renamed with it)",
            Msg::OpenSlotRename(slot.clone()),
        ),
        RowMenuEntry::new(
            "Retarget…",
            "point it at another entity of the same kind",
            Msg::OpenSlotRetarget(slot.clone()),
        ),
        RowMenuEntry::new(
            "Refresh snapshot",
            "copy the entity's current recipe into the slot",
            Msg::RefreshSlot(slot.clone()),
        ),
        RowMenuEntry::new(
            "Reveal in its view",
            "jump to the entity in Sessions, Mux, or PRs",
            Msg::RevealSlot(slot.clone()),
        ),
        RowMenuEntry::new(
            "Unpin",
            "remove the slot; the entity and its history stay",
            Msg::Unpin(slot.clone()),
        )
        .confirming(),
    ];
    Some(RowMenuState::new(format!("slot {slot}"), entries))
}

/// What `Enter` does on a History row or a ghost mux: rebuild the
/// record with `conspectus rebuild <record id>`. `None` on any other
/// row.
pub fn history_primary_effect(app: &App) -> Option<crate::tui::Effect> {
    let reference = selected_record(app)?;
    Some(crate::tui::Effect::Exec(
        crate::tui::effect::ExecSpec::Rebuild { reference },
    ))
}

/// What `Enter` does on a slot row when attaching isn't it: run
/// `conspectus rebuild <slot>` for a slot whose target is gone, missing,
/// planned, or a session that isn't in a mux, and explain otherwise.
/// `None` means the slot attaches.
pub fn pinned_primary_effect(app: &App) -> Option<crate::tui::Effect> {
    use crate::model::{PinKind, SlotStatus};
    use crate::tui::Effect;
    use crate::tui::effect::ExecSpec;
    let RowId::Pin { pin_id } = app.selection()? else {
        return None;
    };
    let snapshot = app.snapshot_handle()?.snapshot();
    let pin = crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .find(|pin| pin.id.id == *pin_id)?;
    if pin_live_mux(snapshot, pin_id).is_some() {
        return None;
    }
    let rebuild = || {
        Some(Effect::Exec(ExecSpec::Rebuild {
            reference: pin_id.clone(),
        }))
    };
    match (pin.status, pin.kind) {
        (SlotStatus::Unknown, _) => Some(Effect::Toast(format!(
            "pin `{pin_id}`: couldn't check whether its target exists; refresh and try again"
        ))),
        (
            SlotStatus::Gone | SlotStatus::Missing | SlotStatus::Planned,
            PinKind::Mux | PinKind::Session | PinKind::Checkout | PinKind::Repo,
        ) => rebuild(),
        (SlotStatus::Live, PinKind::Session) => rebuild(),
        (SlotStatus::Live, PinKind::Pr) => Some(Effect::Toast(match pin.text("url") {
            Some(url) => format!("pin `{pin_id}`: {url}"),
            None => format!("pin `{pin_id}`: this PR has no URL on record"),
        })),
        (SlotStatus::Live, kind) => Some(Effect::Toast(format!(
            "pin `{pin_id}` is a live {}; nothing to launch",
            kind.as_str()
        ))),
        (_, kind) => Some(Effect::Toast(format!(
            "pin `{pin_id}`: a {} can't be rebuilt",
            kind.as_str()
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
    let rebuild = format!("{} rebuild {pin_id}", crate::invocation::invoked_name());
    match pin.status {
        crate::model::SlotStatus::Live => None,
        crate::model::SlotStatus::Gone => Some(format!(
            "pin `{pin_id}` is gone · Enter runs `{rebuild}` to bring it back"
        )),
        crate::model::SlotStatus::Unknown => Some(format!(
            "pin `{pin_id}`: couldn't check whether its target exists"
        )),
        crate::model::SlotStatus::Missing => Some(format!(
            "pin `{pin_id}`: target not found · `pin retarget {pin_id} <target>`, or Enter runs `{rebuild}`"
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
    let mut candidates: Vec<&GraphLink> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.source == *session
                && link.relation == RelationKind::LinkedToMux
                && matches!(link.state, crate::model::LinkState::Active)
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
    match chosen.target_node_id()? {
        NodeId::MuxSession(id) => Some(id.clone()),
        _ => None,
    }
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
