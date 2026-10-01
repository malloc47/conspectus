//! Side-effecting actions the TUI can dispatch in response to a key
//! press, beyond what the pure reducer handles.
//!
//! v1 (this commit) covers attach-to-mux. Resume, fork, picker-
//! driven attach, and the rest of the action surface land in later
//! stories per `docs/implementation/phase-08-interactive-tui.md`.

use crate::model::{
    AgentSessionId, Diagnostic, GraphLink, GraphSnapshot, MuxSessionId, MuxSessionNode, NodeId,
    PinLastSession, RelationKind,
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

/// Pin diagnostic attached to the currently selected row. Mirrors
/// ADR 0057's resolver diagnostics but keeps only the fields needed
/// by the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinDiagnosticView {
    Unbound {
        pin_id: String,
        expected_mux_native_id: String,
        /// Optional last-recorded session from the pin-bindings
        /// sidecar (ADR 0058). When present, the launch path can
        /// advertise a "resume `<id>`" affordance instead of a plain
        /// "launch" hint.
        last_session: Option<PinLastSession>,
    },
    StaleMux {
        pin_id: String,
        mux: MuxSessionId,
    },
    Ambiguous {
        pin_id: String,
        chosen: AgentSessionId,
        competing: Vec<AgentSessionId>,
    },
    Drift {
        pin_id: String,
        declared_cwd: String,
        observed_cwd: String,
    },
}

impl PinDiagnosticView {
    pub fn pin_id(&self) -> &str {
        match self {
            Self::Unbound { pin_id, .. }
            | Self::StaleMux { pin_id, .. }
            | Self::Ambiguous { pin_id, .. }
            | Self::Drift { pin_id, .. } => pin_id,
        }
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

/// Diagnostics for the selected pin-bearing row. Unbound/stale pin
/// rows carry the pin id directly; bound pin rows surface as regular
/// agent-session rows with `pin_id` set.
pub fn selected_pin_diagnostics(app: &App) -> Vec<PinDiagnosticView> {
    let Some(selection) = app.selection() else {
        return Vec::new();
    };
    let Some(row) = app.tree().rows.iter().find(|r| &r.id == selection) else {
        return Vec::new();
    };
    let pin_id = match &row.kind {
        RowKind::Pin(pin) => Some(pin.pin_id.as_str()),
        RowKind::AgentSession(session) => session.pin_id.as_deref(),
        RowKind::MuxSession(mux) => mux.pin_id.as_deref(),
        _ => None,
    };
    let Some(pin_id) = pin_id else {
        return Vec::new();
    };
    let Some(handle) = app.snapshot_handle() else {
        return Vec::new();
    };
    pin_diagnostics_for_id(handle.snapshot(), pin_id)
}

pub fn pin_diagnostics_for_id(snapshot: &GraphSnapshot, pin_id: &str) -> Vec<PinDiagnosticView> {
    snapshot
        .diagnostics
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            Diagnostic::PinUnbound {
                pin_id: id,
                expected_mux_native_id,
                last_session,
            } if id == pin_id => Some(PinDiagnosticView::Unbound {
                pin_id: id.clone(),
                expected_mux_native_id: expected_mux_native_id.clone(),
                last_session: last_session.clone(),
            }),
            Diagnostic::PinStaleMux { pin_id: id, mux } if id == pin_id => {
                Some(PinDiagnosticView::StaleMux {
                    pin_id: id.clone(),
                    mux: mux.clone(),
                })
            }
            Diagnostic::PinAmbiguous {
                pin_id: id,
                chosen,
                competing,
            } if id == pin_id => Some(PinDiagnosticView::Ambiguous {
                pin_id: id.clone(),
                chosen: chosen.clone(),
                competing: competing.clone(),
            }),
            Diagnostic::PinDrift {
                pin_id: id,
                declared_cwd,
                observed_cwd,
            } if id == pin_id => Some(PinDiagnosticView::Drift {
                pin_id: id.clone(),
                declared_cwd: declared_cwd.clone(),
                observed_cwd: observed_cwd.clone(),
            }),
            _ => None,
        })
        .collect()
}

pub fn pin_status_hint(diagnostics: &[PinDiagnosticView]) -> Option<String> {
    diagnostics
        .iter()
        .find_map(|diagnostic| match diagnostic {
            PinDiagnosticView::Ambiguous {
                pin_id,
                chosen,
                competing,
            } => Some(format!(
                "pin `{pin_id}` ambiguous: chosen {} · {} competing · b bind",
                session_label(chosen),
                competing.len()
            )),
            _ => None,
        })
        .or_else(|| {
            diagnostics.iter().find_map(|diagnostic| match diagnostic {
                PinDiagnosticView::Drift {
                    pin_id,
                    declared_cwd,
                    observed_cwd,
                } => Some(format!(
                    "pin `{pin_id}` cwd drift: declared {declared_cwd} · observed {observed_cwd}"
                )),
                _ => None,
            })
        })
        .or_else(|| {
            diagnostics.iter().find_map(|diagnostic| match diagnostic {
                PinDiagnosticView::StaleMux { pin_id, mux } => Some(format!(
                    "pin `{pin_id}` stale mux {}: Enter relaunch",
                    mux.native_id
                )),
                PinDiagnosticView::Unbound {
                    pin_id,
                    expected_mux_native_id,
                    last_session,
                } => Some(match last_session {
                    Some(last) => format!(
                        "pin `{pin_id}` unbound: expected {expected_mux_native_id} · \
                         Enter resume `{}`",
                        last.session_id
                    ),
                    None => format!(
                        "pin `{pin_id}` unbound: expected {expected_mux_native_id} · Enter launch"
                    ),
                }),
                _ => None,
            })
        })
}

pub fn pin_bind_hint(diagnostics: &[PinDiagnosticView]) -> Option<String> {
    diagnostics.iter().find_map(|diagnostic| {
        let PinDiagnosticView::Ambiguous {
            pin_id,
            chosen,
            competing,
        } = diagnostic
        else {
            return None;
        };
        let candidates = std::iter::once(chosen)
            .chain(competing.iter())
            .map(session_label)
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "pin bind `{pin_id}`: choose one of [{candidates}] with `conspectus pin bind {pin_id} --to <session-id>`"
        ))
    })
}

fn session_label(id: &AgentSessionId) -> String {
    format!("{}:{}", id.harness_key, id.session_key)
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
fn preferred_mux_for_session(snapshot: &GraphSnapshot, session: &NodeId) -> Option<MuxSessionId> {
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
            format!("attach: {n} ambiguous muxes here — pick one in detail, or `b` to bind",)
        }
    }
}

/// Helper: format the target's display label for diagnostics (e.g.
/// `tmux:editor`). Uses the **raw** native_id so the label matches
/// what the operator would see in `tmux list-sessions`.
pub fn target_label(target: &AttachTarget) -> String {
    format!("{}:{}", target.backend, target.native_id)
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

/// Resolved tmux launch target for a pin (ADR 0057). Carries the
/// pin's declared mux name and optional socket so the executor can
/// attach to the session once `conspectus pin launch <id>`
/// finishes. `None` when the pin has no attachable target yet
/// (e.g. the launch synthesized a mux the operator asked us to
/// leave detached).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinLaunchTarget {
    pub mux_name: String,
    pub mux_socket: Option<String>,
}

/// Reasons `Msg::LaunchSelectedPin` refuses to fire in the reducer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinLaunchDisabled {
    /// No selection to derive a pin from.
    NoSelection,
    /// The selected row is not a pin, a pin-bound session, or a
    /// pin-bound mux — nothing to launch.
    NotAPinRow,
}

/// Format `PinLaunchDisabled` as an operator-facing status hint.
pub fn pin_launch_disabled_reason(reason: &PinLaunchDisabled) -> String {
    match reason {
        PinLaunchDisabled::NoSelection => "launch: nothing selected".to_string(),
        PinLaunchDisabled::NotAPinRow => "launch: select a pin or placeholder row".to_string(),
    }
}

/// Look up a pin's launch target from the loaded snapshot. Public
/// so both the reducer (via `resolve_launch_pin`) and the Pins-
/// overlay dispatch (which already has the `pin_id` in hand from
/// the modal) can share the same lookup.
pub fn pin_launch_target_from_snapshot(app: &App, pin_id: &str) -> Option<PinLaunchTarget> {
    app.snapshot_handle()?
        .snapshot()
        .pins
        .iter()
        .find(|pin| pin.id == pin_id)
        .map(|pin| PinLaunchTarget {
            mux_name: pin.mux.name.clone(),
            mux_socket: pin.mux.socket_name.clone(),
        })
}

/// Resolve the launch-pin action against `App` state. Pure. Returns
/// the pin id plus its optional attach target on success. The
/// executor is the only code that runs the launch subprocess.
pub fn resolve_launch_pin(
    app: &App,
) -> Result<(String, Option<PinLaunchTarget>), PinLaunchDisabled> {
    let selection = app.selection().ok_or(PinLaunchDisabled::NoSelection)?;
    let row = app
        .tree()
        .rows
        .iter()
        .find(|row| &row.id == selection)
        .ok_or(PinLaunchDisabled::NoSelection)?;
    let pin_id = match &row.kind {
        RowKind::Pin(pin) => Some(pin.pin_id.as_str()),
        RowKind::AgentSession(session) => session.pin_id.as_deref(),
        RowKind::MuxSession(mux) => mux.pin_id.as_deref(),
        _ => None,
    };
    let pin_id = pin_id.ok_or(PinLaunchDisabled::NotAPinRow)?.to_string();
    let target = match &row.kind {
        RowKind::Pin(pin) => Some(PinLaunchTarget {
            mux_name: pin.mux_name.clone(),
            mux_socket: pin.mux_socket.clone(),
        }),
        RowKind::AgentSession(_) | RowKind::MuxSession(_) => {
            pin_launch_target_from_snapshot(app, &pin_id)
        }
        _ => None,
    };
    Ok((pin_id, target))
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;
