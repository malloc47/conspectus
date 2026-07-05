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
#[path = "rename_tests.rs"]
mod tests;
