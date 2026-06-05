//! Side-effecting actions the TUI can dispatch in response to a key
//! press, beyond what the pure reducer handles.
//!
//! v1 (this commit) covers attach-to-mux. Resume, fork, picker-
//! driven attach, and the rest of the action surface land in later
//! stories per `docs/implementation/phase-08-interactive-tui.md`.

use crate::model::{
    AgentSessionId, Diagnostic, GraphLink, GraphSnapshot, MuxSessionId, MuxSessionNode, NodeId,
    RelationKind,
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
}

/// Decide whether the current selection is attachable. Pure: works
/// against [`App`] state, does not touch the terminal or the
/// network.
pub fn resolve_attach_target(app: &App) -> Result<AttachTarget, AttachDisabled> {
    let Some(database) = app.graph_db().cloned() else {
        return Err(AttachDisabled::NoSelection);
    };
    let snapshot =
        crate::query::read_snapshot(database.conn()).map_err(|_| AttachDisabled::MuxNodeMissing)?;
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
            match preferred_mux_for_session(&snapshot, &session_node) {
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
    match target.backend.as_str() {
        "tmux" => {
            if app
                .config()
                .current_tmux_session
                .as_deref()
                .is_some_and(|current| current == target.native_id)
            {
                return Err(AttachDisabled::CurrentTmuxSession(target.native_id));
            }
            Ok(target)
        }
        other => Err(AttachDisabled::UnsupportedBackend(other.to_string())),
    }
}

/// Resolve the viewer target for the current selection. Mirrors
/// [`resolve_attach_target`] so `Enter` (T8-043) and the `v`
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
            let database = app.graph_db().ok_or(ViewerDisabled::UnsupportedRow)?;
            let snapshot = crate::query::read_snapshot(database.conn())
                .map_err(|_| ViewerDisabled::UnsupportedRow)?;
            let mux_node_id = NodeId::MuxSession(mux_row.mux.clone());
            preferred_session_for_mux(&snapshot, &mux_node_id).ok_or(ViewerDisabled::UnsupportedRow)
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
        _ => None,
    };
    let Some(pin_id) = pin_id else {
        return Vec::new();
    };
    let Some(database) = app.graph_db() else {
        return Vec::new();
    };
    let Ok(snapshot) = crate::query::read_snapshot(database.conn()) else {
        return Vec::new();
    };
    pin_diagnostics_for_id(&snapshot, pin_id)
}

pub fn pin_diagnostics_for_id(snapshot: &GraphSnapshot, pin_id: &str) -> Vec<PinDiagnosticView> {
    snapshot
        .diagnostics
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            Diagnostic::PinUnbound {
                pin_id: id,
                expected_mux_native_id,
            } if id == pin_id => Some(PinDiagnosticView::Unbound {
                pin_id: id.clone(),
                expected_mux_native_id: expected_mux_native_id.clone(),
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
                } => Some(format!(
                    "pin `{pin_id}` unbound: expected {expected_mux_native_id} · Enter launch"
                )),
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
/// accelerator and T8-043's default-action dispatcher so a mux row's
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
            format!("attach: mux backend `{name}` not supported (only tmux)")
        }
        AttachDisabled::CurrentTmuxSession(name) => {
            format!("attach: refusing to attach current tmux session `{name}`")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, GraphLink,
        GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
        Provenance, RepoId, RepoNode,
    };
    use crate::resolve::resolve_snapshot;
    use crate::tui::app::{GraphDb, Msg};
    use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
    use crate::tui::{RunConfig, SessionsGrouping, View};

    fn build_app(snapshot: GraphSnapshot) -> App {
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

    fn session_node(harness: &str, scope: &str, key: &str, cwd: &str) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, scope, key),
            harness_key: harness.to_string(),
            cwd: Some(cwd.to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        })
    }

    /// Mirror real-world `TmuxDiscovery` shape: the node's `id`
    /// is backend-prefixed (`tmux:editor`) while `native_id` is
    /// the raw session name (`editor`).
    fn mux_node(backend: &str, native: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("{backend}:{native}")),
            backend: backend.to_string(),
            native_id: native.to_string(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn linked_to_mux(
        session: &NodeId,
        mux: &NodeId,
        provenance: Provenance,
        suffix: &str,
    ) -> GraphLink {
        GraphLink {
            id: format!("session-mux-{suffix}"),
            source: session.clone(),
            target: LinkEndpoint::Node { id: mux.clone() },
            relation: RelationKind::LinkedToMux,
            provenance,
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
        }));
    }

    #[test]
    fn empty_app_reports_no_selection() {
        let app = App::new(RunConfig::defaults());
        assert_eq!(
            resolve_attach_target(&app),
            Err(AttachDisabled::NoSelection)
        );
    }

    #[test]
    fn unmuxed_session_reports_unmuxed_disabled() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        let mut app = build_app(snapshot);
        // Step past the repo group to the session row.
        app.update(Msg::NavDown);
        assert_eq!(
            resolve_attach_target(&app),
            Err(AttachDisabled::UnmuxedSession)
        );
    }

    #[test]
    fn group_row_reports_unsupported_row() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        let app = build_app(snapshot);
        // Auto-selection lands on the repo group row first.
        assert_eq!(
            resolve_attach_target(&app),
            Err(AttachDisabled::UnsupportedRow)
        );
    }

    /// Mux node id matching the prefixed form `mux_node` emits.
    fn mux_node_id(backend: &str, native: &str) -> NodeId {
        NodeId::MuxSession(MuxSessionId::new(format!("{backend}:{native}")))
    }

    #[test]
    fn muxed_session_resolves_to_preferred_mux_target() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = mux_node_id("tmux", "editor");
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &mux_id,
            Provenance::Discovered,
            "1",
        ));
        let mut app = build_app(snapshot);
        app.update(Msg::NavDown);
        let target = resolve_attach_target(&app).expect("muxed session attachable");
        assert_eq!(target.backend, "tmux");
        assert_eq!(
            target.mux.native_id, "tmux:editor",
            "graph id retains the backend prefix"
        );
        assert_eq!(
            target.native_id, "editor",
            "raw native id strips the prefix — this is what tmux attach -t takes"
        );
    }

    #[test]
    fn current_tmux_session_is_not_attachable() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = mux_node_id("tmux", "editor");
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &mux_id,
            Provenance::Discovered,
            "1",
        ));
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
        cfg.current_tmux_session = Some("editor".to_string());
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app.update(Msg::NavDown);

        assert_eq!(
            resolve_attach_target(&app),
            Err(AttachDisabled::CurrentTmuxSession("editor".to_string()))
        );
    }

    #[test]
    fn ambiguous_session_attaches_to_resolver_preferred_candidate() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.nodes.push(mux_node("tmux", "scratch"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let editor = mux_node_id("tmux", "editor");
        let scratch = mux_node_id("tmux", "scratch");
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &editor,
            Provenance::StrongDiscovered,
            "1",
        ));
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &scratch,
            Provenance::Discovered,
            "2",
        ));
        let mut app = build_app(snapshot);
        app.update(Msg::NavDown);
        let target = resolve_attach_target(&app).expect("ambiguous session attachable");
        assert_eq!(
            target.native_id, "editor",
            "preferred (StrongDiscovered) candidate wins; raw native id forwarded"
        );
    }

    #[test]
    fn ambiguous_candidate_child_row_attaches_to_that_candidate() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.nodes.push(mux_node("tmux", "scratch"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let editor = mux_node_id("tmux", "editor");
        let scratch = mux_node_id("tmux", "scratch");
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &editor,
            Provenance::StrongDiscovered,
            "1",
        ));
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &scratch,
            Provenance::Discovered,
            "2",
        ));
        let mut app = build_app(snapshot);
        // Step from repo → session, expand its ambiguous children,
        // then step to the second candidate (scratch).
        app.update(Msg::NavDown);
        app.update(Msg::ToggleExpand);
        app.update(Msg::NavDown); // editor candidate
        app.update(Msg::NavDown); // scratch candidate
        let target = resolve_attach_target(&app).expect("candidate row attachable");
        assert_eq!(
            target.native_id, "scratch",
            "attach respects the candidate row override"
        );
    }

    #[test]
    fn mux_view_mux_row_resolves_attach_target() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("tmux", "editor"));
        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize snapshot");
        let tree = crate::tui::rows::mux::build_mux_tree_from_conn(
            crate::tui::rows::mux::MuxBuildInputsFromConn {
                conn: &conn,
                home: None,
                now: None,
                filter: RowFilter::default(),
                grouping: crate::tui::MuxGrouping::Session,
            },
        )
        .expect("build mux tree");
        let mut cfg = RunConfig::defaults();
        cfg.default_view = View::Mux;
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: GraphDb::new(conn),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });

        let target = resolve_attach_target(&app).expect("mux row attachable");
        assert_eq!(target.backend, "tmux");
        assert_eq!(target.native_id, "editor");
    }

    fn build_mux_view_app_with_attachments(snapshot: GraphSnapshot) -> App {
        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize snapshot");
        let tree = crate::tui::rows::mux::build_mux_tree_from_conn(
            crate::tui::rows::mux::MuxBuildInputsFromConn {
                conn: &conn,
                home: None,
                now: None,
                filter: RowFilter::default(),
                grouping: crate::tui::MuxGrouping::Session,
            },
        )
        .expect("build mux tree");
        let mut cfg = RunConfig::defaults();
        cfg.default_view = View::Mux;
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: GraphDb::new(conn),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app
    }

    #[test]
    fn view_resolves_agent_session_row_directly() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        let mut app = build_app(snapshot);
        app.update(Msg::NavDown);
        let session = resolve_view_session(&app).expect("agent session is viewable");
        assert_eq!(session.harness_key, "codex");
        assert_eq!(session.session_key, "abc");
    }

    #[test]
    fn view_resolves_mux_row_to_linked_session() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = mux_node_id("tmux", "editor");
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &mux_id,
            Provenance::Discovered,
            "1",
        ));

        let app = build_mux_view_app_with_attachments(snapshot);
        let session = resolve_view_session(&app).expect("mux row resolves linked session");
        assert_eq!(session.harness_key, "codex");
        assert_eq!(session.session_key, "abc");
    }

    #[test]
    fn view_on_mux_row_picks_preferred_when_multiple_sessions_linked() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot
            .nodes
            .push(session_node("claude-code", "/state", "def", "/p/proj"));
        let codex_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let claude_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "def"));
        let mux_id = mux_node_id("tmux", "editor");
        // codex link is the lower-precedence Discovered; claude link
        // is StrongDiscovered and should win.
        snapshot.candidate_links.push(linked_to_mux(
            &codex_id,
            &mux_id,
            Provenance::Discovered,
            "1",
        ));
        snapshot.candidate_links.push(linked_to_mux(
            &claude_id,
            &mux_id,
            Provenance::StrongDiscovered,
            "2",
        ));

        let app = build_mux_view_app_with_attachments(snapshot);
        let session = resolve_view_session(&app).expect("preferred session resolves");
        assert_eq!(
            session.harness_key, "claude-code",
            "StrongDiscovered candidate wins over Discovered"
        );
    }

    #[test]
    fn view_on_unattached_mux_row_reports_unsupported() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("tmux", "editor"));
        let app = build_mux_view_app_with_attachments(snapshot);
        assert_eq!(
            resolve_view_session(&app),
            Err(ViewerDisabled::UnsupportedRow)
        );
    }
}
