//! Session-rename planning.
//!
//! Given a target agent session, a desired display name (or a clear
//! request), and a `--no-mux` flag, this module computes a
//! [`RenamePlan`] that the CLI rename command and the TUI rename
//! action execute. Planning is pure — it does not write the alias
//! overlay or invoke tmux; callers do that. Centralizing the policy
//! here keeps ADR 0029's lockstep contract in one place.

use std::collections::BTreeMap;

use crate::model::{
    AgentSessionId, GraphLink, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, NodeId,
    RelationKind,
};

/// Resolved actions for renaming a single agent session, plus the
/// optional lockstep mux rename. Either action can be `None`: an
/// alias clear with no mux lockstep produces a plan where only the
/// alias removal runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenamePlan {
    pub agent_alias_write: AgentAliasWrite,
    pub mux_native_rename: Option<MuxNativeRename>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentAliasWrite {
    pub session: AgentSessionId,
    /// `Some` upserts the alias to this display name. `None`
    /// removes any existing alias entry for the session.
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MuxNativeRename {
    pub mux: MuxSessionId,
    pub new_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenamePlanError {
    /// The session has more than one active `LinkedToMux` candidate
    /// and the operator did not pass `--no-mux`. Per ADR 0029
    /// lockstep refuses to pick a mux when ambiguity exists.
    AmbiguousMux { candidate_count: usize },
    /// Display name was supplied but trimmed to empty. Use the
    /// `clear` form instead.
    EmptyDisplayName,
}

impl std::fmt::Display for RenamePlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AmbiguousMux { candidate_count } => write!(
                f,
                "session has {candidate_count} active mux candidates; \
                 resolve the ambiguity (e.g. `conspectus declared confirm`) \
                 or pass --no-mux to rename only the alias"
            ),
            Self::EmptyDisplayName => {
                write!(
                    f,
                    "display name must not be empty; use --clear to remove an alias"
                )
            }
        }
    }
}

impl std::error::Error for RenamePlanError {}

/// Build a [`RenamePlan`] for `session_id`.
///
/// `new_display_name`:
/// - `Some(name)` upserts the alias to `name` and, when lockstep
///   applies, renames the linked mux to `name` too.
/// - `None` clears the alias for the session. Mux native names are
///   left untouched even when lockstep would otherwise apply —
///   clearing the alias does not synthesize an empty tmux name.
///
/// `no_mux` suppresses the mux rename regardless of ambiguity, per
/// ADR 0029.
pub fn plan_session_rename(
    snapshot: &GraphSnapshot,
    session_id: &AgentSessionId,
    new_display_name: Option<String>,
    no_mux: bool,
) -> Result<RenamePlan, RenamePlanError> {
    if let Some(name) = &new_display_name
        && name.trim().is_empty()
    {
        return Err(RenamePlanError::EmptyDisplayName);
    }

    let agent_alias_write = AgentAliasWrite {
        session: session_id.clone(),
        display_name: new_display_name.clone(),
    };

    let mux_native_rename = match (&new_display_name, no_mux) {
        (Some(name), false) => {
            let session_node = NodeId::AgentSession(session_id.clone());
            let candidates = active_mux_candidates(snapshot, &session_node);
            match candidates.len() {
                0 => None,
                1 => Some(MuxNativeRename {
                    mux: candidates[0].clone(),
                    new_name: name.clone(),
                }),
                n => return Err(RenamePlanError::AmbiguousMux { candidate_count: n }),
            }
        }
        _ => None,
    };

    Ok(RenamePlan {
        agent_alias_write,
        mux_native_rename,
    })
}

/// Collect distinct active `LinkedToMux` target mux ids for
/// `session`. Mirrors the de-duplication that the TUI row builder
/// uses so the lockstep decision matches the indicator the operator
/// sees.
fn active_mux_candidates(snapshot: &GraphSnapshot, session: &NodeId) -> Vec<MuxSessionId> {
    let mut by_target: BTreeMap<MuxSessionId, Vec<&GraphLink>> = BTreeMap::new();
    for link in &snapshot.candidate_links {
        if link.source != *session
            || link.relation != RelationKind::LinkedToMux
            || !matches!(link.state, LinkState::Active)
        {
            continue;
        }
        let Some(target_id) = link.target_node_id() else {
            continue;
        };
        let LinkEndpoint::Node {
            id: NodeId::MuxSession(mux_id),
        } = &link.target
        else {
            // Unresolved endpoints don't have a native name to rename.
            let _ = target_id;
            continue;
        };
        by_target.entry(mux_id.clone()).or_default().push(link);
    }
    by_target.into_keys().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionNode, Confidence, Freshness, GraphNode, MuxSessionNode, Provenance,
        SourceMetadata,
    };

    fn session(harness: &str, key: &str) -> AgentSessionNode {
        AgentSessionNode::new(
            AgentSessionId::new(harness, "/state", key),
            harness.to_string(),
        )
    }

    fn mux(name: &str) -> MuxSessionNode {
        MuxSessionNode::new(
            MuxSessionId::new(name),
            "tmux".to_string(),
            name.to_string(),
        )
    }

    fn linked_to_mux(id: &str, source: &AgentSessionId, target: &MuxSessionId) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source: NodeId::AgentSession(source.clone()),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(target.clone()),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    #[test]
    fn rename_with_no_mux_link_writes_alias_only() {
        let mut snapshot = GraphSnapshot::empty();
        let node = session("codex", "alpha");
        let id = node.id.clone();
        snapshot.nodes.push(GraphNode::AgentSession(node));

        let plan =
            plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");

        assert_eq!(plan.agent_alias_write.session, id);
        assert_eq!(
            plan.agent_alias_write.display_name.as_deref(),
            Some("ingest")
        );
        assert!(plan.mux_native_rename.is_none());
    }

    #[test]
    fn rename_with_single_mux_link_renames_both() {
        let mut snapshot = GraphSnapshot::empty();
        let node = session("codex", "alpha");
        let id = node.id.clone();
        let mux_node = mux("editor");
        snapshot.nodes.push(GraphNode::AgentSession(node));
        snapshot
            .candidate_links
            .push(linked_to_mux("link-1", &id, &mux_node.id));
        snapshot.nodes.push(GraphNode::MuxSession(mux_node));

        let plan =
            plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");

        let mux_rename = plan.mux_native_rename.expect("mux rename");
        assert_eq!(mux_rename.mux.native_id, "editor");
        assert_eq!(mux_rename.new_name, "ingest");
    }

    #[test]
    fn ambiguous_mux_refuses_lockstep() {
        let mut snapshot = GraphSnapshot::empty();
        let node = session("codex", "alpha");
        let id = node.id.clone();
        let mux_a = mux("editor");
        let mux_b = mux("shell");
        snapshot.nodes.push(GraphNode::AgentSession(node));
        snapshot
            .candidate_links
            .push(linked_to_mux("link-a", &id, &mux_a.id));
        snapshot
            .candidate_links
            .push(linked_to_mux("link-b", &id, &mux_b.id));
        snapshot.nodes.push(GraphNode::MuxSession(mux_a));
        snapshot.nodes.push(GraphNode::MuxSession(mux_b));

        let err = plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false)
            .expect_err("ambiguous");
        assert_eq!(err, RenamePlanError::AmbiguousMux { candidate_count: 2 });
    }

    #[test]
    fn ambiguous_mux_with_no_mux_flag_skips_mux_rename() {
        let mut snapshot = GraphSnapshot::empty();
        let node = session("codex", "alpha");
        let id = node.id.clone();
        let mux_a = mux("editor");
        let mux_b = mux("shell");
        snapshot.nodes.push(GraphNode::AgentSession(node));
        snapshot
            .candidate_links
            .push(linked_to_mux("link-a", &id, &mux_a.id));
        snapshot
            .candidate_links
            .push(linked_to_mux("link-b", &id, &mux_b.id));
        snapshot.nodes.push(GraphNode::MuxSession(mux_a));
        snapshot.nodes.push(GraphNode::MuxSession(mux_b));

        let plan =
            plan_session_rename(&snapshot, &id, Some("ingest".to_string()), true).expect("plan");
        assert!(plan.mux_native_rename.is_none());
    }

    #[test]
    fn no_mux_flag_suppresses_lockstep_even_with_single_candidate() {
        let mut snapshot = GraphSnapshot::empty();
        let node = session("codex", "alpha");
        let id = node.id.clone();
        let mux_node = mux("editor");
        snapshot.nodes.push(GraphNode::AgentSession(node));
        snapshot
            .candidate_links
            .push(linked_to_mux("link", &id, &mux_node.id));
        snapshot.nodes.push(GraphNode::MuxSession(mux_node));

        let plan =
            plan_session_rename(&snapshot, &id, Some("ingest".to_string()), true).expect("plan");
        assert!(plan.mux_native_rename.is_none());
    }

    #[test]
    fn clearing_alias_skips_mux_rename_even_with_single_candidate() {
        let mut snapshot = GraphSnapshot::empty();
        let node = session("codex", "alpha");
        let id = node.id.clone();
        let mux_node = mux("editor");
        snapshot.nodes.push(GraphNode::AgentSession(node));
        snapshot
            .candidate_links
            .push(linked_to_mux("link", &id, &mux_node.id));
        snapshot.nodes.push(GraphNode::MuxSession(mux_node));

        let plan = plan_session_rename(&snapshot, &id, None, false).expect("plan");
        assert!(plan.agent_alias_write.display_name.is_none());
        assert!(plan.mux_native_rename.is_none());
    }

    #[test]
    fn duplicate_linked_to_mux_candidates_collapse_to_one_mux() {
        // Multiple evidence rows pointing at the same mux are
        // resolved-single from the operator's perspective; they
        // should not trigger ambiguity.
        let mut snapshot = GraphSnapshot::empty();
        let node = session("codex", "alpha");
        let id = node.id.clone();
        let mux_node = mux("editor");
        snapshot.nodes.push(GraphNode::AgentSession(node));
        snapshot
            .candidate_links
            .push(linked_to_mux("evidence-1", &id, &mux_node.id));
        snapshot
            .candidate_links
            .push(linked_to_mux("evidence-2", &id, &mux_node.id));
        snapshot.nodes.push(GraphNode::MuxSession(mux_node));

        let plan =
            plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");
        let mux_rename = plan.mux_native_rename.expect("mux rename");
        assert_eq!(mux_rename.mux.native_id, "editor");
    }

    #[test]
    fn ignored_linked_to_mux_candidates_are_skipped() {
        let mut snapshot = GraphSnapshot::empty();
        let node = session("codex", "alpha");
        let id = node.id.clone();
        let mux_node = mux("editor");
        snapshot.nodes.push(GraphNode::AgentSession(node));
        let mut link = linked_to_mux("ignored", &id, &mux_node.id);
        link.state = LinkState::Ignored { reason: None };
        snapshot.candidate_links.push(link);
        snapshot.nodes.push(GraphNode::MuxSession(mux_node));

        let plan =
            plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");
        assert!(plan.mux_native_rename.is_none());
    }

    #[test]
    fn empty_display_name_is_rejected() {
        let snapshot = GraphSnapshot::empty();
        let id = AgentSessionId::new("codex", "/state", "alpha");
        let err =
            plan_session_rename(&snapshot, &id, Some("   ".to_string()), false).expect_err("empty");
        assert_eq!(err, RenamePlanError::EmptyDisplayName);
    }
}
