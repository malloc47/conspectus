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

/// A mux session to rename, with everything the backend needs to find
/// it. `target` is the backend-native session name, not the graph id:
/// tmux reads the prefixed `tmux:<name>` as session `tmux`, window
/// `<name>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MuxNativeRename {
    pub mux: MuxSessionId,
    pub backend: String,
    pub target: String,
    /// Non-default tmux socket, when the session lives on one.
    pub socket: Option<String>,
    pub new_name: String,
}

impl MuxNativeRename {
    fn for_node(mux: &crate::model::MuxSessionNode, new_name: String) -> Self {
        let (socket, target) = split_socket_and_name(&mux.native_id);
        Self {
            mux: mux.id.clone(),
            backend: mux.backend.clone(),
            target: target.to_string(),
            socket: socket.map(str::to_string),
            new_name,
        }
    }
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
                 resolve the ambiguity (e.g. `{} declared confirm`) \
                 or pass --no-mux to rename only the alias",
                crate::invocation::invoked_name()
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
                // A link to a mux discovery no longer sees has nothing to
                // rename; the alias still lands.
                1 => mux_node(snapshot, &candidates[0])
                    .map(|mux| MuxNativeRename::for_node(mux, name.clone())),
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

/// Graph-aware plan for renaming a mux session. Beyond the tmux
/// native-name change, this cascades to any pin whose `mux.name`
/// matches the current native id + socket so pin bindings survive
/// the rename.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MuxRenamePlan {
    pub mux_rename: MuxNativeRename,
    pub pin_slot_updates: Vec<PinSlotNameUpdate>,
}

/// A pin slot whose `name` override must follow a mux rename (ADR
/// 0141): the slot points at the mux by history record, which follows
/// the rename on its own, but an override would otherwise rebuild the
/// mux under its old name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinSlotNameUpdate {
    pub slot_id: String,
    pub new_name: String,
}

/// Write each update to the slot store. Returns, per update, whether
/// the slot changed or why it couldn't.
pub fn apply_pin_slot_updates(
    store: &crate::pin_slots::SlotStore,
    updates: &[PinSlotNameUpdate],
) -> Vec<(String, Result<bool, String>)> {
    updates
        .iter()
        .map(|update| {
            let result = store
                .update(&update.slot_id, |slot| {
                    slot.overrides.insert(
                        "name".to_string(),
                        crate::model::FieldValue::Text(update.new_name.clone()),
                    );
                    Ok(())
                })
                .map(|write| write.changed)
                .map_err(|err| err.to_string());
            (update.slot_id.clone(), result)
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MuxRenamePlanError {
    /// The desired new name is empty (or whitespace-only).
    EmptyName,
    /// The desired new name is identical to the current one — the
    /// plan would produce no change. Callers report this to the
    /// operator as a no-op rather than firing tmux.
    NoOp,
    /// The mux id doesn't resolve to a discovered mux node in the
    /// snapshot. Distinguishes "mux vanished mid-session" from other
    /// failures.
    MuxNotFound,
    /// The mux is only in history (ADR 0140): there is no session to
    /// rename until it is rebuilt.
    MuxGone,
}

impl std::fmt::Display for MuxRenamePlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => write!(f, "mux rename requires a non-empty name"),
            Self::NoOp => write!(f, "mux rename: new name matches current name"),
            Self::MuxNotFound => write!(f, "mux not present in the discovered snapshot"),
            Self::MuxGone => write!(f, "the mux is gone; rebuild it before renaming"),
        }
    }
}

impl std::error::Error for MuxRenamePlanError {}

/// Build a [`MuxRenamePlan`] for `mux_id` with `new_name`.
///
/// `new_name` is the bare tmux session name — no `tmux:` prefix, no
/// `<socket>:` prefix. Callers who want to change the socket use the
/// separate pin-rebind form, not this plan.
///
/// Every pin whose `mux.name` matches the mux's current bare name
/// (and whose `mux.socket_name` matches the mux's socket) picks up a
/// [`PinMuxNameUpdate`] so the executor rewrites the pin store in
/// lockstep with the tmux rename.
pub fn plan_mux_rename(
    snapshot: &GraphSnapshot,
    mux_id: &MuxSessionId,
    new_name: String,
) -> Result<MuxRenamePlan, MuxRenamePlanError> {
    if new_name.trim().is_empty() {
        return Err(MuxRenamePlanError::EmptyName);
    }

    let mux = mux_node(snapshot, mux_id).ok_or(MuxRenamePlanError::MuxNotFound)?;
    if !snapshot.is_live(&crate::model::NodeId::MuxSession(mux_id.clone())) {
        return Err(MuxRenamePlanError::MuxGone);
    }

    // `MuxSessionNode.native_id` drops the backend prefix (e.g.
    // `editor` for a default-socket tmux session, `scratch:editor`
    // for a non-default socket). Split off the leading `<socket>:`
    // if present so we compare against the bare tmux name — which
    // is what `PinMux.name` stores.
    let (_mux_socket, bare_current) = split_socket_and_name(&mux.native_id);

    if bare_current == new_name {
        return Err(MuxRenamePlanError::NoOp);
    }

    let mux_node_id = crate::model::NodeId::MuxSession(mux_id.clone());
    let pin_slot_updates = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            crate::model::GraphNode::Pin(pin)
                if pin.kind == crate::model::PinKind::Mux
                    && !pin.legacy
                    && pin.target_node.as_ref() == Some(&mux_node_id)
                    && pin
                        .overrides
                        .get("name")
                        .and_then(crate::model::FieldValue::as_text)
                        == Some(bare_current) =>
            {
                Some(PinSlotNameUpdate {
                    slot_id: pin.id.id.clone(),
                    new_name: new_name.clone(),
                })
            }
            _ => None,
        })
        .collect();

    Ok(MuxRenamePlan {
        mux_rename: MuxNativeRename::for_node(mux, new_name),
        pin_slot_updates,
    })
}

fn mux_node<'a>(
    snapshot: &'a GraphSnapshot,
    mux_id: &MuxSessionId,
) -> Option<&'a crate::model::MuxSessionNode> {
    snapshot.nodes.iter().find_map(|node| match node {
        crate::model::GraphNode::MuxSession(m) if &m.id == mux_id => Some(m),
        _ => None,
    })
}

/// Split a `MuxSessionNode.native_id` like `scratch:editor` into
/// `(Some("scratch"), "editor")`, or `editor` into `(None, "editor")`.
/// The bare tmux name is the second element — what
/// `PinMux.name` stores and what the operator types in the rename
/// modal.
fn split_socket_and_name(native_id: &str) -> (Option<&str>, &str) {
    match native_id.split_once(':') {
        Some((socket, name)) => (Some(socket), name),
        None => (None, native_id),
    }
}

#[cfg(test)]
#[path = "rename_tests.rs"]
mod tests;
