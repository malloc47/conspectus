//! The shared reducer: how a [`LiveEvent`] changes a session's entry.
//!
//! The entry's private block keeps what one event cannot say on its own:
//! the main agent's state under any wait, each agent's pending wait, the
//! child agents still running, and the state compaction interrupted. The
//! observation readers see is derived from it after every event:
//!
//! - a wait of the main agent shows as `waiting` with its kind;
//! - else the earliest wait of a child agent shows as `waiting`, raised
//!   to the session with `via_subagent`;
//! - else the main agent's own state shows.
//!
//! `since_ms` is when the shown state began. A wait shows its own start;
//! once it ends, the main agent's state shows again with the start it
//! had, and so does a state compaction interrupted, so a finished turn
//! does not look new because the session compacted while idle.
//!
//! An entry that has `exited` takes no more events from its process, so
//! a late writer cannot revive it. A session start, or any event from
//! another process, opens it afresh.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize};

use crate::model::live::{LiveDetail, LiveObservation, LiveSource, LiveState};

use super::event::{LiveEvent, LiveEventKind, WaitKind};
use super::file::LiveEntry;

/// Folds an event into a session's entry. [`fold`] holds the rules every
/// event-source harness shares, and [`SharedReducer`] uses them as they
/// are. A harness whose events need a transition of their own implements
/// this, handles that case, and hands the rest to [`fold`].
pub trait Reducer {
    /// The entry after `event`; `entry` is `None` for a session the
    /// store has no entry for. An event that changes nothing returns the
    /// entry as it was.
    fn fold(&self, entry: Option<&LiveEntry>, event: &LiveEvent) -> LiveEntry {
        fold(entry, event)
    }
}

/// The reducer for a harness that needs nothing beyond the shared rules.
#[derive(Clone, Copy, Debug, Default)]
pub struct SharedReducer;

impl Reducer for SharedReducer {}

/// The entry after `event`, by the shared rules. Pure: the store assigns
/// the revision and the change time afterwards.
pub fn fold(entry: Option<&LiveEntry>, event: &LiveEvent) -> LiveEntry {
    let mut next = match entry {
        Some(entry) if entry.observation.state == LiveState::Exited => {
            if !reopens(entry, event) {
                return entry.clone();
            }
            LiveEntry::new(&event.harness, &event.session_id, event.observed_ms)
        }
        Some(entry) => entry.clone(),
        None => LiveEntry::new(&event.harness, &event.session_id, event.observed_ms),
    };
    next.merge_context(&event.context);
    next.instance = None;
    next.seq = None;
    next.reducer.apply(&event.kind, event.observed_ms);
    next.observation = next.reducer.observe(&next.observation, event.observed_ms);
    next
}

/// Whether `event` opens an `exited` entry again: a session start, or an
/// event from a process other than the recorded one.
fn reopens(entry: &LiveEntry, event: &LiveEvent) -> bool {
    if matches!(event.kind, LiveEventKind::SessionStarted { .. }) {
        return true;
    }
    let Some(pid) = event.context.pid else {
        return false;
    };
    let recorded = &entry.observation;
    recorded.pid != Some(pid)
        || matches!(
            (recorded.pid_start, event.context.pid_start),
            (Some(recorded), Some(current)) if recorded != current
        )
}

/// What the reducer keeps between events. Internal: it may change
/// between versions, no output shows it, and a block this version cannot
/// read is treated as empty.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReducerBlock {
    /// The main agent's own state, under any wait.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    main: Option<Phase>,
    /// Pending waits, at most one per agent, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    waits: Vec<Wait>,
    /// Ids of the child agents still running.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    subagents: BTreeSet<String>,
    /// The state to resume when compaction ends; set only while the main
    /// agent compacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resume: Option<Phase>,
}

/// A state with its detail and when it began.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Phase {
    state: LiveState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    detail: Option<LiveDetail>,
    since_ms: i64,
}

/// One agent's pending wait.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Wait {
    /// `None` for the main agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    kind: WaitKind,
    since_ms: i64,
}

impl ReducerBlock {
    pub(crate) fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// A block for an entry that ended: `exited` with `detail`, nothing
    /// pending.
    pub(crate) fn ended(detail: LiveDetail, at_ms: i64) -> Self {
        let mut block = Self::default();
        block.set_main(LiveState::Exited, Some(detail), at_ms);
        block
    }

    fn main_state(&self) -> Option<LiveState> {
        self.main.map(|phase| phase.state)
    }

    /// Make `state` the main agent's state. It keeps its start when the
    /// state stays the same, and any planned resume is dropped once the
    /// agent is no longer compacting.
    fn set_main(&mut self, state: LiveState, detail: Option<LiveDetail>, at_ms: i64) {
        let since_ms = match self.main {
            Some(phase) if phase.state == state => phase.since_ms,
            _ => at_ms,
        };
        self.main = Some(Phase {
            state,
            detail,
            since_ms,
        });
        if state != LiveState::Compacting {
            self.resume = None;
        }
    }

    /// The main agent leaves compaction for the state it interrupted, or
    /// for `ready` when that was neither a turn nor a ready prompt.
    fn end_compaction(&mut self, at_ms: i64) {
        match self.resume.take() {
            Some(phase) => self.main = Some(phase),
            None => self.set_main(LiveState::Ready, None, at_ms),
        }
    }

    fn end_wait(&mut self, agent: Option<&str>) {
        self.waits.retain(|wait| wait.agent.as_deref() != agent);
    }

    fn start_wait(&mut self, agent: Option<&String>, kind: WaitKind, at_ms: i64) {
        match self
            .waits
            .iter_mut()
            .find(|wait| wait.agent.as_ref() == agent)
        {
            Some(wait) if wait.kind == kind => {}
            Some(wait) => {
                wait.kind = kind;
                wait.since_ms = at_ms;
            }
            None => self.waits.push(Wait {
                agent: agent.cloned(),
                kind,
                since_ms: at_ms,
            }),
        }
    }

    fn apply(&mut self, kind: &LiveEventKind, at_ms: i64) {
        use LiveState::{Compacting, Error, Exited, Ready, Starting, Unknown, Working};
        match kind {
            LiveEventKind::SessionStarted {
                after_compaction: false,
            } => {
                *self = Self::default();
                self.set_main(Ready, None, at_ms);
            }
            LiveEventKind::SessionStarted {
                after_compaction: true,
            } => match self.main_state() {
                Some(Compacting) => self.end_compaction(at_ms),
                None => self.set_main(Ready, None, at_ms),
                Some(_) => {}
            },
            LiveEventKind::TurnStarted => {
                self.end_wait(None);
                self.set_main(Working, Some(LiveDetail::Generating), at_ms);
            }
            LiveEventKind::Retrying { rate_limited } => {
                let detail = if *rate_limited {
                    LiveDetail::RateLimited
                } else {
                    LiveDetail::Retrying
                };
                self.set_main(Working, Some(detail), at_ms);
            }
            LiveEventKind::WaitStarted { agent, kind } => {
                self.start_wait(agent.as_ref(), *kind, at_ms);
            }
            LiveEventKind::WaitEnded { agent } => self.end_wait(agent.as_deref()),
            LiveEventKind::ToolFinished { agent: None } => {
                self.end_wait(None);
                // A tool finishing means a turn is open, unless the turn
                // already closed and this writer lost the race to the
                // lock.
                if matches!(self.main_state(), None | Some(Working | Unknown)) {
                    self.set_main(Working, Some(LiveDetail::Generating), at_ms);
                }
            }
            LiveEventKind::ToolFinished { agent: Some(agent) } => self.end_wait(Some(agent)),
            LiveEventKind::SubagentStarted { id } => {
                self.subagents.insert(id.clone());
            }
            LiveEventKind::SubagentStopped { id } => {
                self.subagents.remove(id);
                self.end_wait(Some(id));
            }
            LiveEventKind::BackgroundWork { subagents } => {
                self.subagents = subagents.iter().cloned().collect();
                let running = &self.subagents;
                self.waits
                    .retain(|wait| wait.agent.as_ref().is_none_or(|id| running.contains(id)));
            }
            LiveEventKind::CompactionStarted => {
                if self.main_state() != Some(Compacting) {
                    let resume = self
                        .main
                        .filter(|phase| matches!(phase.state, Working | Ready));
                    self.set_main(Compacting, None, at_ms);
                    self.resume = resume;
                }
            }
            LiveEventKind::CompactionEnded => {
                if self.main_state() == Some(Compacting) {
                    self.end_compaction(at_ms);
                }
            }
            LiveEventKind::TurnEnded { interrupted } => {
                self.end_wait(None);
                let detail = if *interrupted {
                    LiveDetail::Interrupted
                } else {
                    LiveDetail::Finished
                };
                self.set_main(Ready, Some(detail), at_ms);
            }
            LiveEventKind::TurnFailed { class } => {
                self.end_wait(None);
                self.set_main(Error, Some(class.detail()), at_ms);
            }
            LiveEventKind::IdleConfirmed => {
                self.end_wait(None);
                if matches!(
                    self.main_state(),
                    None | Some(Working | Compacting | Starting | Unknown)
                ) {
                    self.set_main(Ready, None, at_ms);
                }
            }
            LiveEventKind::SessionEnded => *self = Self::ended(LiveDetail::Ended, at_ms),
        }
        if self.main_state() == Some(Exited) {
            self.waits.clear();
            self.subagents.clear();
        }
    }

    /// The observation this block shows, from the `previous` one: the
    /// process and the latest evidence carry over, `at_ms` counts as
    /// evidence, and the state, detail, start, and child agents come
    /// from the block.
    pub(crate) fn observe(&self, previous: &LiveObservation, at_ms: i64) -> LiveObservation {
        let main_wait = self.waits.iter().find(|wait| wait.agent.is_none());
        let child_wait = self.waits.iter().find(|wait| wait.agent.is_some());
        let (state, detail, since_ms, via_subagent) = match (main_wait, child_wait, self.main) {
            (Some(wait), _, _) => (
                LiveState::Waiting,
                Some(wait.kind.detail()),
                wait.since_ms,
                false,
            ),
            (None, Some(wait), _) => (
                LiveState::Waiting,
                Some(wait.kind.detail()),
                wait.since_ms,
                true,
            ),
            (None, None, Some(phase)) => (phase.state, phase.detail, phase.since_ms, false),
            (None, None, None) => (LiveState::Unknown, None, previous.since_ms, false),
        };
        LiveObservation {
            state,
            detail: detail.filter(|detail| state.accepts(*detail)),
            subagents: u16::try_from(self.subagents.len()).unwrap_or(u16::MAX),
            via_subagent,
            since_ms,
            observed_ms: previous.observed_ms.max(at_ms),
            source: LiveSource::Hook,
            heartbeat_s: 0,
            ..*previous
        }
    }
}

/// Reads a reducer block, or an empty one when this version cannot read
/// what a newer one wrote: the block is internal, and the next events
/// rebuild it.
pub(crate) fn tolerant_block<'de, D>(deserializer: D) -> Result<ReducerBlock, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

#[cfg(test)]
#[path = "reducer_tests.rs"]
mod tests;
