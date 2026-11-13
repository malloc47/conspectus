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
//! `unknown`, and `exited` never change, and a poll never binds an agent
//! the store has no entry for.

use std::collections::BTreeMap;

use crate::discovery::live_poller::{PollCandidate, SETTLE_MS};
use crate::live_store::{LiveEntry, LiveFile, PidProbe};
use crate::model::live::{LiveBinding, LiveObservation, LivePoll, LiveSource, LiveState, Liveness};
use crate::model::{GraphSnapshot, LivePane, NodeId};

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

#[cfg(test)]
#[path = "live_poll_tests.rs"]
mod tests;
