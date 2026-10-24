//! Choosing and building the resume of a session (ADR 0018, 0098).

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

use crate::discovery::harness::{resume_argv_for, splice_resume_argv};
use crate::model::{
    AgentSessionId, GraphNode, GraphSnapshot, LinkState, MuxRecord, NodeId, RelationKind,
};

/// Where following a session's lineage forward ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineageOutcome {
    /// The head: the newest session in the chain.
    Head(AgentSessionId),
    /// The session isn't in the snapshot.
    SessionMissing,
    /// The chain forks, so there is no single head.
    Fork {
        at: AgentSessionId,
        successors: Vec<AgentSessionId>,
    },
}

/// Follow `parent_session` links forward from a session to the newest
/// one, so a session that compacted or resumed since is resumed at its
/// head. A cycle in a malformed graph stops the walk where it is.
pub fn lineage_head(snapshot: &GraphSnapshot, harness: &str, session_key: &str) -> LineageOutcome {
    let Some(start) = snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::AgentSession(s)
            if s.id.harness_key == harness && s.id.session_key == session_key =>
        {
            Some(s.id.clone())
        }
        _ => None,
    }) else {
        return LineageOutcome::SessionMissing;
    };

    let mut current = start;
    let mut visited: BTreeSet<AgentSessionId> = BTreeSet::from([current.clone()]);
    loop {
        let target = NodeId::AgentSession(current.clone());
        let mut successors: Vec<AgentSessionId> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::ParentSession
                    && matches!(link.state, LinkState::Active)
                    && link.target_node_id() == Some(&target)
            })
            .filter_map(|link| match &link.source {
                NodeId::AgentSession(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        successors.sort();
        successors.dedup();
        match successors.len() {
            0 => return LineageOutcome::Head(current),
            1 => {
                let next = successors.remove(0);
                if !visited.insert(next.clone()) {
                    return LineageOutcome::Head(current);
                }
                current = next;
            }
            _ => {
                return LineageOutcome::Fork {
                    at: current,
                    successors,
                };
            }
        }
    }
}

/// A session to resume and the argv that resumes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resume {
    pub harness: String,
    /// The lineage head actually resumed.
    pub session_key: String,
    /// `base` with the resume tokens spliced in.
    pub argv: Vec<OsString>,
}

/// Why no resume was built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoResume {
    /// Nothing to resume: the mux hosted no session of this harness.
    NoSession,
    Missing(String),
    Forked {
        at: String,
        count: usize,
    },
    HarnessCannotResume(String),
    /// The launch argv never runs the harness binary, so the resume
    /// tokens have nowhere to go (ADR 0098).
    CannotSplice {
        binary: String,
        session: String,
    },
}

impl NoResume {
    /// A line for the plan, or `None` when there is simply nothing to
    /// say.
    pub fn note(&self) -> Option<String> {
        Some(match self {
            Self::NoSession => return None,
            Self::Missing(session) => {
                format!("session `{session}` no longer exists; launching fresh")
            }
            Self::Forked { at, count } => format!(
                "session `{at}` has {count} compacted successors; launching fresh \
                 (resume the one you want with `--session`)"
            ),
            Self::HarnessCannotResume(harness) => {
                format!("harness `{harness}` has no resume command; launching fresh")
            }
            Self::CannotSplice { binary, session } => format!(
                "the launch argv doesn't run `{binary}`, so session `{session}` can't be \
                 resumed; launching fresh"
            ),
        })
    }
}

/// The session a mux record should resume: the one asked for, else the
/// newest of the harness it hosted.
pub fn session_to_resume(
    record: Option<&MuxRecord>,
    harness: &str,
    requested: Option<&str>,
) -> Option<String> {
    if let Some(requested) = requested {
        return Some(requested.to_string());
    }
    record?
        .recipe
        .hosted
        .iter()
        .filter(|hosted| hosted.harness == harness)
        .max_by_key(|hosted| hosted.last_seen)
        .map(|hosted| hosted.session_id.clone())
}

/// Build the resume of `session` for `harness` into `base`.
pub fn build_resume(
    snapshot: &GraphSnapshot,
    harness: &str,
    session: &str,
    cwd: &Path,
    base: &[OsString],
) -> Result<Resume, NoResume> {
    let head = match lineage_head(snapshot, harness, session) {
        LineageOutcome::Head(head) => head,
        LineageOutcome::SessionMissing => return Err(NoResume::Missing(session.to_string())),
        LineageOutcome::Fork { at, successors } => {
            return Err(NoResume::Forked {
                at: at.session_key,
                count: successors.len(),
            });
        }
    };
    let resume = resume_argv_for(harness, &head.session_key, cwd)
        .ok_or_else(|| NoResume::HarnessCannotResume(harness.to_string()))?;
    let argv = splice_resume_argv(base, &resume).ok_or_else(|| NoResume::CannotSplice {
        binary: resume[0].to_string_lossy().into_owned(),
        session: head.session_key.clone(),
    })?;
    Ok(Resume {
        harness: harness.to_string(),
        session_key: head.session_key,
        argv,
    })
}
