//! Repair polls in the live tables (ADR 0218): the latest poll of a
//! harness's own list of its sessions, which the snapshot carries in
//! `live_polls`, folded into the bindings of that harness's store
//! entries.
//!
//! Hooks miss the end of two states: an interrupted turn fires no event,
//! so `working` lingers, and an approval fires none, so `waiting`
//! lingers. A binding gives way to the poll's reading of the same agent
//! ([`combine`]) when all of these hold:
//!
//! 1. it still holds its store entry's hook observation (no pane title
//!    replaced it);
//! 2. the entry recorded `working` or `waiting`, and its process was not
//!    found dead;
//! 3. the poll ran at least [`SETTLE_MS`] after the entry's latest event,
//!    so a hook event newer than the poll, or just before it, wins;
//! 4. the poll lists the same agent, by pid or else session id
//!    ([`crate::model::live::PolledAgent::is`]), in another state.
//!
//! The binding then reads the poll's state with source `harness_poll`.
//! The store is not rewritten, so the entry's next event takes over
//! again. A hook entry's `ready`, `error`, `compacting`, `starting`,
//! `unknown`, and `exited` never change.
//!
//! An agent the store has no entry for binds from the poll alone (ADR
//! 0222) only when discovery knows its process: a running main agent of
//! the harness that no entry reports ([`unreported`]), which is also what
//! makes the poller run every thirty seconds. It binds to the session the
//! graph says that process runs, else to the session the poll names, and
//! only where no other binding sits ([`fold_poll_only`]). It records no
//! pid, so the read rule ages it as it ages any state whose process
//! cannot be checked: quiet after ten minutes of work, stale fifteen
//! minutes after the last poll that listed it. It is gone at once when
//! its process ends or a newer poll no longer lists it.

use std::collections::{BTreeMap, BTreeSet};

use crate::discovery::live_poller::{PollCandidate, SETTLE_MS};
use crate::live_store::{LiveEntry, LiveFile, PidProbe, PidStatus};
use crate::model::live::{LiveBinding, LiveObservation, LivePoll, LiveSource, LiveState, Liveness};
use crate::model::{GraphNode, GraphSnapshot, LivePane, NodeId, RelationKind, RuntimeProcessRole};

use super::{BoundLive, Winners};

/// Fold every poll `snapshot` carries into `bound`, the tables of the
/// store's entries, whose winners per session and per pane are
/// `sessions` and `panes`.
pub(super) fn fold_polls(
    snapshot: &GraphSnapshot,
    sessions: &BTreeMap<NodeId, &LiveEntry>,
    panes: &BTreeMap<LivePane, &LiveEntry>,
    bound: &mut BoundLive,
) {
    if snapshot.live_polls.is_empty() {
        return;
    }
    let repair = |entry: &LiveEntry, binding: Option<&LiveBinding>| {
        let poll = snapshot.live_polls.get(&entry.harness)?;
        let binding = binding.filter(|binding| binding.observation == entry.observation)?;
        combine(binding, &entry.session_id, poll)
    };
    for (node, entry) in sessions {
        if let Some(next) = repair(entry, bound.nodes.get(node)) {
            bound.nodes.insert(node.clone(), next);
        }
    }
    for (pane, entry) in panes {
        if let Some(next) = repair(entry, bound.panes.get(pane)) {
            bound.panes.insert(pane.clone(), next);
        }
    }
}

/// What a hook binding of session `session_id` reads once `poll` is
/// folded in, by the module's rules: `None` leaves it as it is.
pub(crate) fn combine(
    hook: &LiveBinding,
    session_id: &str,
    poll: &LivePoll,
) -> Option<LiveBinding> {
    let recorded = &hook.observation;
    if recorded.source != LiveSource::Hook
        || !matches!(recorded.state, LiveState::Working | LiveState::Waiting)
        || hook.liveness == Liveness::Dead
        || poll.observed_ms.saturating_sub(recorded.observed_ms) < SETTLE_MS
    {
        return None;
    }
    let agent = poll.agent_for(recorded.pid, recorded.pid_start, Some(session_id))?;
    if agent.state == recorded.state {
        return None;
    }
    // A run that began before the entry's latest event says nothing of
    // the time since; this poll is then the first evidence.
    let since_ms = if agent.since_ms > recorded.observed_ms {
        agent.since_ms
    } else {
        poll.observed_ms
    };
    let mut observation =
        LiveObservation::new(agent.state, LiveSource::HarnessPoll, poll.observed_ms)
            .with_since_ms(since_ms)
            .with_subagents(recorded.subagents);
    if let Some(detail) = agent.detail {
        observation = observation.with_detail(detail);
    }
    observation.pid = recorded.pid;
    observation.pid_start = recorded.pid_start;
    Some(LiveBinding::new(observation, hook.liveness))
}

/// The bound store entries of `harness` that a poll might repair: those
/// that recorded `working` or `waiting` from hook events and whose
/// process `probe` does not find dead.
pub(super) fn candidates(
    snapshot: &GraphSnapshot,
    file: &LiveFile,
    probe: &dyn PidProbe,
    harness: &str,
) -> Vec<PollCandidate> {
    let winners = Winners::of(snapshot, file);
    winners
        .entries()
        .filter(|entry| {
            let recorded = &entry.observation;
            entry.harness == harness
                && recorded.source == LiveSource::Hook
                && matches!(recorded.state, LiveState::Working | LiveState::Waiting)
                && probe.liveness(recorded) != Liveness::Dead
        })
        .map(|entry| PollCandidate {
            session_id: entry.session_id.clone(),
            pid: entry.observation.pid,
            pid_start: entry.observation.pid_start,
            state: entry.observation.state,
            observed_ms: entry.observation.observed_ms,
        })
        .collect()
}

/// A running main agent of a harness, as a graph shows it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RunningAgent {
    pub pid: i64,
    /// The native ids of the sessions the graph says the process runs:
    /// those its links identify, or its only candidate.
    pub sessions: BTreeSet<String>,
}

impl RunningAgent {
    /// Whether store entry `entry` describes this agent: it recorded the
    /// agent's pid, or is one of the sessions the agent runs.
    pub fn reported_by(&self, entry: &LiveEntry) -> bool {
        entry.observation.pid == Some(self.pid) || self.sessions.contains(&entry.session_id)
    }
}

/// The distinct agent processes of `harness` the graph `snapshot`
/// recorded that `probe` finds running now. Only main agents count, not
/// their subagents, background helpers, or shells.
pub fn running_agents(
    snapshot: &GraphSnapshot,
    harness: &str,
    probe: &dyn PidProbe,
) -> Vec<RunningAgent> {
    let mut agents: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    for node in &snapshot.nodes {
        let GraphNode::RuntimeProcess(process) = node else {
            continue;
        };
        let Some(pid) = process.pid else {
            continue;
        };
        if process.harness_key.as_deref() != Some(harness)
            || process.role != Some(RuntimeProcessRole::HumanAgent)
        {
            continue;
        }
        agents.entry(pid).or_default().extend(sessions_of(
            snapshot,
            &NodeId::RuntimeProcess(process.id.clone()),
        ));
    }
    agents
        .into_iter()
        .filter(|(pid, _)| matches!(probe.status(*pid), PidStatus::Running { .. }))
        .map(|(pid, sessions)| RunningAgent { pid, sessions })
        .collect()
}

/// The native ids of the sessions `process` runs, by the graph's links:
/// every session a link says it identifies, or the only session a link
/// names it a candidate for, as the resolver reads them.
fn sessions_of(snapshot: &GraphSnapshot, process: &NodeId) -> BTreeSet<String> {
    let session = |relation: RelationKind| {
        snapshot
            .candidate_links
            .iter()
            .filter(move |link| link.relation == relation && link.source == *process)
            .filter_map(|link| match link.target_node_id() {
                Some(NodeId::AgentSession(session)) => Some(session.session_key.clone()),
                _ => None,
            })
            .collect::<BTreeSet<String>>()
    };
    let mut sessions = session(RelationKind::ProcessIdentifiesSession);
    let candidates = session(RelationKind::ProcessCandidatesSession);
    if candidates.len() == 1 {
        sessions.extend(candidates);
    }
    sessions
}

/// The running main agents of `harness` that no entry of the store
/// `file` reports: the agents a poll may show on its own, and whose being
/// known keeps the poller running.
pub fn unreported(
    snapshot: &GraphSnapshot,
    file: &LiveFile,
    probe: &dyn PidProbe,
    harness: &str,
) -> Vec<RunningAgent> {
    let entries: Vec<&LiveEntry> = file
        .entries
        .values()
        .filter(|entry| entry.harness == harness)
        .collect();
    running_agents(snapshot, harness, probe)
        .into_iter()
        .filter(|agent| !entries.iter().any(|entry| agent.reported_by(entry)))
        .collect()
}

/// Bind, from each poll `snapshot` carries, the agents of the poll's
/// harness that the store `file` has no entry for and that discovery
/// knows as running ([`unreported`]), each to its session when that
/// session's slot in `bound` is empty. See the module's rules.
pub(super) fn fold_poll_only(
    snapshot: &GraphSnapshot,
    file: &LiveFile,
    probe: &dyn PidProbe,
    bound: &mut BoundLive,
) {
    for (harness, poll) in &snapshot.live_polls {
        let known = unreported(snapshot, file, probe, harness);
        if known.is_empty() {
            continue;
        }
        let sessions = live_sessions(snapshot, harness);
        for agent in &poll.agents {
            let Some(pid) = agent.pid else {
                continue;
            };
            let Some(process) = known.iter().find(|known| known.pid == pid) else {
                continue;
            };
            // The listed process must still be the one the poll saw.
            let start = match probe.status(pid) {
                PidStatus::Running { start } => start,
                PidStatus::Gone => continue,
                PidStatus::Unknown => None,
            };
            if matches!((agent.pid_start, start), (Some(listed), Some(now)) if listed != now) {
                continue;
            }
            let session = match process.sessions.len() {
                1 => process.sessions.iter().next(),
                _ => agent.session_id.as_ref(),
            };
            let Some(node) = session.and_then(|session| sessions.get(session)) else {
                continue;
            };
            if bound.nodes.contains_key(node) {
                continue;
            }
            bound
                .nodes
                .insert(node.clone(), poll_only(agent, poll.observed_ms));
        }
    }
}

/// The live agent sessions of `harness`, by native id. Two nodes for one
/// session keep the lower id, as the store's binding does.
fn live_sessions(snapshot: &GraphSnapshot, harness: &str) -> BTreeMap<String, NodeId> {
    let mut sessions: BTreeMap<String, NodeId> = BTreeMap::new();
    for node in snapshot.live_nodes() {
        let GraphNode::AgentSession(session) = node else {
            continue;
        };
        if session.harness_key != harness {
            continue;
        }
        let id = node.id();
        sessions
            .entry(session.id.session_key.clone())
            .and_modify(|kept| {
                if id < *kept {
                    *kept = id.clone();
                }
            })
            .or_insert(id);
    }
    sessions
}

/// What a poll at `observed_ms` says of `agent` on its own: its state and
/// detail since its run began, with no pid, so its process counts as
/// unchecked and the read rule ages it once polls stop.
fn poll_only(agent: &crate::model::live::PolledAgent, observed_ms: i64) -> LiveBinding {
    let mut observation = LiveObservation::new(agent.state, LiveSource::HarnessPoll, observed_ms)
        .with_since_ms(agent.since_ms);
    if let Some(detail) = agent.detail {
        observation = observation.with_detail(detail);
    }
    LiveBinding::new(observation, Liveness::Unknown)
}

#[cfg(test)]
#[path = "live_poll_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "live_poll_only_tests.rs"]
mod poll_only_tests;
