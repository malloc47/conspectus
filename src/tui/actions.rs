//! Side-effecting actions the TUI can dispatch in response to a key
//! press, beyond what the pure reducer handles.
//!
//! v1 (this commit) covers attach-to-mux. Resume, fork, picker-
//! driven attach, and the rest of the action surface land in later
//! stories per `docs/implementation/phase-08-interactive-tui.md`.

use crate::model::{GraphLink, GraphSnapshot, MuxSessionId, MuxSessionNode, NodeId, RelationKind};
use crate::tui::app::App;
use crate::tui::rows::{RowId, RowKind};

/// Resolved target for an attach action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachTarget {
    /// The mux session to hand control to.
    pub mux: MuxSessionId,
    /// Backend label, e.g. `"tmux"`. Drives the exec invocation in
    /// the runtime.
    pub backend: String,
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
}

/// Decide whether the current selection is attachable. Pure: works
/// against [`App`] state, does not touch the terminal or the
/// network.
pub fn resolve_attach_target(app: &App) -> Result<AttachTarget, AttachDisabled> {
    let Some(snapshot) = app.snapshot().cloned() else {
        return Err(AttachDisabled::NoSelection);
    };
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
            match preferred_mux_for_session(snapshot.as_ref(), &session_node) {
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
        _ => return Err(AttachDisabled::UnsupportedRow),
    };

    let mux_node = snapshot
        .as_ref()
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
    };
    match target.backend.as_str() {
        "tmux" => Ok(target),
        other => Err(AttachDisabled::UnsupportedBackend(other.to_string())),
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
        AttachDisabled::UnmuxedSession => {
            "attach: session is not attached to any mux (R reserved for resume)".to_string()
        }
        AttachDisabled::MuxNodeMissing => {
            "attach: mux node missing from snapshot — try `r` to refresh".to_string()
        }
        AttachDisabled::UnsupportedBackend(name) => {
            format!("attach: mux backend `{name}` not supported (only tmux)")
        }
    }
}

/// Helper: format the target's display label for diagnostics (e.g.
/// `tmux:editor`). Borrowed from the row tree so the messages stay
/// consistent.
pub fn target_label(_snapshot: &GraphSnapshot, target: &AttachTarget) -> String {
    format!("{}:{}", target.backend, target.mux.native_id)
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
    use crate::model::{
        AgentSessionId, AgentSessionNode, Confidence, GraphLink, GraphNode, GraphSnapshot,
        LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance, RepoId, RepoNode,
        WorktreeId, WorktreeNode,
    };
    use crate::resolve::resolve_snapshot;
    use crate::tui::app::Msg;
    use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
    use crate::tui::{RunConfig, SessionsGrouping, View};
    use std::sync::Arc;

    fn build_app(snapshot: GraphSnapshot) -> App {
        let snapshot = resolve_snapshot(snapshot);
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: None,
            now: None,
        });
        let mut cfg = RunConfig::defaults();
        cfg.default_view = View::Sessions;
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: Arc::new(snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
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
        })
    }

    fn mux_node(backend: &str, native: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(native),
            backend: backend.to_string(),
            native_id: native.to_string(),
            cwd: None,
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
        snapshot.nodes.push(GraphNode::Worktree(WorktreeNode {
            id: WorktreeId::new(RepoId::new(common_dir), common_dir.to_string()),
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

    #[test]
    fn muxed_session_resolves_to_preferred_mux_target() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
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
        assert_eq!(target.mux.native_id, "editor");
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
        let editor = NodeId::MuxSession(MuxSessionId::new("editor"));
        let scratch = NodeId::MuxSession(MuxSessionId::new("scratch"));
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
            target.mux.native_id, "editor",
            "preferred (StrongDiscovered) candidate wins"
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
        let editor = NodeId::MuxSession(MuxSessionId::new("editor"));
        let scratch = NodeId::MuxSession(MuxSessionId::new("scratch"));
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
            target.mux.native_id, "scratch",
            "attach respects the candidate row override"
        );
    }
}
