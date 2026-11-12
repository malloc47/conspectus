//! Live agent state (ADR 0203): what a running agent is doing now, in a
//! closed vocabulary that no harness owns, and the rules that turn a
//! stored observation into the state a reader shows.
//!
//! An emitter maps its own signals onto a [`LiveState`] and a
//! [`LiveDetail`] and records a [`LiveObservation`]; a signal that fits
//! no state is dropped, not stretched. Readers never show the stored
//! state as it is: [`LiveObservation::effective`] applies the time and
//! process rules at read time, so nothing rewrites an observation as time
//! passes. A node with no observation has no evidence, which is not the
//! same as being idle.
//!
//! A mux has no state of its own. [`LiveCounts`] counts the states of the
//! sessions it hosts, and [`LiveCounts::dominant`] picks the one to show
//! by [`LiveState::default_precedence`], which a viewer may replace
//! through [`LiveCounts::dominant_by`].
//!
//! Every name here serializes in lowercase (`rate_limited` for the one
//! two-word detail) and stays stable. A reader that meets a state or
//! source name it does not know reads `unknown`, and an unknown detail
//! reads as none, so an observation written by a newer version still
//! loads.

use serde::{Deserialize, Deserializer, Serialize};

/// How long a `working`, `compacting`, or `starting` state may go without
/// evidence before it is marked quiet, for a source that sends no
/// heartbeat.
pub const QUIET_WINDOW_MS: i64 = 10 * 60 * 1000;

/// For a source that sends heartbeats, the quiet window is this many
/// heartbeat intervals.
pub const QUIET_HEARTBEATS: i64 = 3;

/// How long any state but `exited` holds without evidence when the
/// process cannot be checked. Past it the state reads `unknown/stale`.
pub const UNKNOWN_LIVENESS_WINDOW_MS: i64 = 15 * 60 * 1000;

/// How long a `starting` state holds without evidence before it reads
/// `unknown/stale`.
pub const STARTING_WINDOW_MS: i64 = 2 * 60 * 1000;

/// What an agent session is doing, as one of a closed set of states.
///
/// Serialized by [`Self::name`]. A reader that meets a name it does not
/// know reads [`Self::Unknown`] instead of failing.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LiveState {
    /// A turn is running.
    Working,
    /// The agent is blocked on the operator.
    Waiting,
    /// The turn is over and the agent accepts a prompt.
    Ready,
    /// Context compaction is running.
    Compacting,
    /// The turn ended on an error.
    Error,
    /// The process is up and has had no prompt yet.
    Starting,
    /// The session is over.
    Exited,
    /// No usable evidence, or evidence too old to trust.
    Unknown,
}

impl LiveState {
    /// Every state, in the order the vocabulary lists them.
    pub const ALL: [LiveState; 8] = [
        LiveState::Working,
        LiveState::Waiting,
        LiveState::Ready,
        LiveState::Compacting,
        LiveState::Error,
        LiveState::Starting,
        LiveState::Exited,
        LiveState::Unknown,
    ];

    /// The stable name the state serializes as.
    pub fn name(self) -> &'static str {
        match self {
            LiveState::Working => "working",
            LiveState::Waiting => "waiting",
            LiveState::Ready => "ready",
            LiveState::Compacting => "compacting",
            LiveState::Error => "error",
            LiveState::Starting => "starting",
            LiveState::Exited => "exited",
            LiveState::Unknown => "unknown",
        }
    }

    /// The state named `name`, or `None` for a name outside the set.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.name() == name)
    }

    /// The details this state can carry. `compacting` and `starting`
    /// carry none.
    pub fn details(self) -> &'static [LiveDetail] {
        match self {
            LiveState::Working => &[
                LiveDetail::Generating,
                LiveDetail::Tool,
                LiveDetail::Retrying,
                LiveDetail::RateLimited,
            ],
            LiveState::Waiting => &[
                LiveDetail::Permission,
                LiveDetail::Question,
                LiveDetail::Plan,
                LiveDetail::Input,
            ],
            LiveState::Ready => &[LiveDetail::Finished, LiveDetail::Interrupted],
            LiveState::Compacting => &[],
            LiveState::Error => &[
                LiveDetail::RateLimited,
                LiveDetail::Auth,
                LiveDetail::Overloaded,
                LiveDetail::Failed,
            ],
            LiveState::Starting => &[],
            LiveState::Exited => &[LiveDetail::Ended, LiveDetail::Killed],
            LiveState::Unknown => &[LiveDetail::Stale],
        }
    }

    /// Whether `detail` is one of this state's details.
    pub fn accepts(self, detail: LiveDetail) -> bool {
        self.details().contains(&detail)
    }

    /// Whether a session in this state has running child agents worth
    /// showing beside it: not once it has exited or its state can't be
    /// told, whatever count was last recorded.
    pub fn shows_subagents(self) -> bool {
        !matches!(self, LiveState::Exited | LiveState::Unknown)
    }

    /// The default rank of this state when one state has to stand for
    /// several, as on a mux row; lower ranks first. What needs the
    /// operator comes first: `waiting`, `error`, `ready`, then `working`,
    /// then `compacting`, `starting`, `unknown`, and `exited`.
    pub fn default_precedence(self) -> u8 {
        match self {
            LiveState::Waiting => 0,
            LiveState::Error => 1,
            LiveState::Ready => 2,
            LiveState::Working => 3,
            LiveState::Compacting => 4,
            LiveState::Starting => 5,
            LiveState::Unknown => 6,
            LiveState::Exited => 7,
        }
    }
}

impl<'de> Deserialize<'de> for LiveState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;
        Ok(Self::from_name(&name).unwrap_or(LiveState::Unknown))
    }
}

/// What kind of a state it is: why an agent waits, how a turn ended,
/// why it failed. Each detail belongs to the states listed on it, and
/// [`LiveState::details`] gives the pairs.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LiveDetail {
    /// `working`: the model is producing output.
    Generating,
    /// `working`: a tool is running.
    Tool,
    /// `working`: a failed request is being retried.
    Retrying,
    /// `working`: held back by a rate limit and still going. `error`:
    /// the turn ended on one.
    RateLimited,
    /// `waiting`: for approval of an action.
    Permission,
    /// `waiting`: for an answer to a question the agent asked.
    Question,
    /// `waiting`: for approval of a plan.
    Plan,
    /// `waiting`: for other input.
    Input,
    /// `ready`: the turn finished.
    Finished,
    /// `ready`: the operator interrupted the turn.
    Interrupted,
    /// `error`: authentication failed.
    Auth,
    /// `error`: the provider was overloaded.
    Overloaded,
    /// `error`: any other failure.
    Failed,
    /// `exited`: the session said it was ending.
    Ended,
    /// `exited`: the process vanished without saying so.
    Killed,
    /// `unknown`: the evidence is too old to trust.
    Stale,
}

impl LiveDetail {
    /// Every detail, grouped by the first state that carries it.
    pub const ALL: [LiveDetail; 16] = [
        LiveDetail::Generating,
        LiveDetail::Tool,
        LiveDetail::Retrying,
        LiveDetail::RateLimited,
        LiveDetail::Permission,
        LiveDetail::Question,
        LiveDetail::Plan,
        LiveDetail::Input,
        LiveDetail::Finished,
        LiveDetail::Interrupted,
        LiveDetail::Auth,
        LiveDetail::Overloaded,
        LiveDetail::Failed,
        LiveDetail::Ended,
        LiveDetail::Killed,
        LiveDetail::Stale,
    ];

    /// The stable name the detail serializes as.
    pub fn name(self) -> &'static str {
        match self {
            LiveDetail::Generating => "generating",
            LiveDetail::Tool => "tool",
            LiveDetail::Retrying => "retrying",
            LiveDetail::RateLimited => "rate_limited",
            LiveDetail::Permission => "permission",
            LiveDetail::Question => "question",
            LiveDetail::Plan => "plan",
            LiveDetail::Input => "input",
            LiveDetail::Finished => "finished",
            LiveDetail::Interrupted => "interrupted",
            LiveDetail::Auth => "auth",
            LiveDetail::Overloaded => "overloaded",
            LiveDetail::Failed => "failed",
            LiveDetail::Ended => "ended",
            LiveDetail::Killed => "killed",
            LiveDetail::Stale => "stale",
        }
    }

    /// The detail named `name`, or `None` for a name outside the set.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|detail| detail.name() == name)
    }
}

/// Where an observation came from.
///
/// A reader that meets a name it does not know reads [`Self::Unknown`]
/// instead of failing.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LiveSource {
    /// Events from a hook or plugin running inside the agent's harness.
    Hook,
    /// A source this version does not know, written by a newer one.
    Unknown,
}

impl LiveSource {
    /// Every source.
    pub const ALL: [LiveSource; 2] = [LiveSource::Hook, LiveSource::Unknown];

    /// The stable name the source serializes as.
    pub fn name(self) -> &'static str {
        match self {
            LiveSource::Hook => "hook",
            LiveSource::Unknown => "unknown",
        }
    }

    /// The source named `name`, or `None` for a name outside the set.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|source| source.name() == name)
    }
}

impl<'de> Deserialize<'de> for LiveSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;
        Ok(Self::from_name(&name).unwrap_or(LiveSource::Unknown))
    }
}

/// Whether the process behind an observation still runs, as a reader
/// found it when it checked. It is an input to
/// [`LiveObservation::effective`]. The live-state store never records
/// it; a graph's table keeps the answer its binder found beside each
/// observation ([`LiveBinding`]).
///
/// Serialized by [`Self::name`]. A reader that meets a name it does not
/// know, or none, reads [`Self::Unknown`].
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Liveness {
    /// The recorded process is running.
    Alive,
    /// The recorded process is gone, or its pid now belongs to another
    /// process.
    Dead,
    /// The reader could not check: no pid was recorded, or the platform
    /// offers no cheap way to look.
    #[default]
    Unknown,
}

impl<'de> Deserialize<'de> for Liveness {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;
        Ok(Self::from_name(&name).unwrap_or(Liveness::Unknown))
    }
}

impl Liveness {
    /// Every liveness answer.
    pub const ALL: [Liveness; 3] = [Liveness::Alive, Liveness::Dead, Liveness::Unknown];

    /// The stable name the answer serializes as.
    pub fn name(self) -> &'static str {
        match self {
            Liveness::Alive => "alive",
            Liveness::Dead => "dead",
            Liveness::Unknown => "unknown",
        }
    }

    /// The answer named `name`, or `None` for a name outside the set.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|liveness| liveness.name() == name)
    }

    /// The liveness of a recorded process whose pid is running now.
    /// `recorded_start` is the start time stored with the observation
    /// ([`LiveObservation::pid_start`]) and `current_start` the start
    /// time of the process that has the pid now. When both are known and
    /// differ, the pid was reused and the recorded process is dead;
    /// otherwise it is alive.
    pub fn of_running_pid(recorded_start: Option<u64>, current_start: Option<u64>) -> Self {
        match (recorded_start, current_start) {
            (Some(recorded), Some(current)) if recorded != current => Liveness::Dead,
            _ => Liveness::Alive,
        }
    }
}

/// How far a reader can trust an effective state.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LiveConfidence {
    /// The evidence is recent enough, or the state does not age.
    Fresh,
    /// A `working`, `compacting`, or `starting` state that has had no
    /// evidence for longer than its quiet window. It still holds: a long
    /// build is still work.
    Quiet,
    /// The evidence is too old to trust. The state reads `unknown`.
    Stale,
}

impl LiveConfidence {
    /// Every confidence, from most to least trusted.
    pub const ALL: [LiveConfidence; 3] = [
        LiveConfidence::Fresh,
        LiveConfidence::Quiet,
        LiveConfidence::Stale,
    ];

    /// The stable name the confidence serializes as.
    pub fn name(self) -> &'static str {
        match self {
            LiveConfidence::Fresh => "fresh",
            LiveConfidence::Quiet => "quiet",
            LiveConfidence::Stale => "stale",
        }
    }
}

/// One session's live state as recorded: enums, counts, ids, and times,
/// never text. The state a reader shows comes from [`Self::effective`].
///
/// Times are Unix epoch milliseconds.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct LiveObservation {
    pub state: LiveState,
    /// One of [`LiveState::details`] for `state`. A detail that does not
    /// fit the state is ignored when the state is read.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_detail"
    )]
    pub detail: Option<LiveDetail>,
    /// Active child agents.
    #[serde(default, skip_serializing_if = "is_default")]
    pub subagents: u16,
    /// The state comes from a child agent and is raised to this, the
    /// root session (a child blocked on the operator makes its root
    /// `waiting`).
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub via_subagent: bool,
    /// When this state began.
    pub since_ms: i64,
    /// When the latest evidence for this state arrived. Every window in
    /// [`Self::effective`] is measured from it.
    pub observed_ms: i64,
    pub source: LiveSource,
    /// The agent's process.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i64>,
    /// The start time of `pid` as the platform reports it (on Linux, the
    /// clock ticks since boot in `/proc/<pid>/stat`). Only compared for
    /// equality, to tell a reused pid from the recorded process.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_start: Option<u64>,
    /// Seconds between the source's heartbeats; 0 when it sends none.
    #[serde(default, skip_serializing_if = "is_default")]
    pub heartbeat_s: u32,
}

impl LiveObservation {
    /// An observation of `state` from `source` at `observed_ms`, which is
    /// also when the state began. No detail, subagents, pid, or
    /// heartbeat.
    pub fn new(state: LiveState, source: LiveSource, observed_ms: i64) -> Self {
        Self {
            state,
            detail: None,
            subagents: 0,
            via_subagent: false,
            since_ms: observed_ms,
            observed_ms,
            source,
            pid: None,
            pid_start: None,
            heartbeat_s: 0,
        }
    }

    /// With `detail` when the state accepts it; a detail the state does
    /// not carry leaves the observation without one.
    pub fn with_detail(mut self, detail: LiveDetail) -> Self {
        self.detail = Some(detail).filter(|detail| self.state.accepts(*detail));
        self
    }

    pub fn with_since_ms(mut self, since_ms: i64) -> Self {
        self.since_ms = since_ms;
        self
    }

    pub fn with_subagents(mut self, subagents: u16) -> Self {
        self.subagents = subagents;
        self
    }

    pub fn with_pid(mut self, pid: i64, pid_start: Option<u64>) -> Self {
        self.pid = Some(pid);
        self.pid_start = pid_start;
        self
    }

    pub fn with_heartbeat_s(mut self, heartbeat_s: u32) -> Self {
        self.heartbeat_s = heartbeat_s;
        self
    }

    /// How long a `working`, `compacting`, or `starting` state may go
    /// without evidence before it is quiet: [`QUIET_HEARTBEATS`]
    /// heartbeats for a source that sends them, else
    /// [`QUIET_WINDOW_MS`].
    pub fn quiet_window_ms(&self) -> i64 {
        if self.heartbeat_s > 0 {
            QUIET_HEARTBEATS * i64::from(self.heartbeat_s) * 1000
        } else {
            QUIET_WINDOW_MS
        }
    }

    /// Milliseconds since the latest evidence at `now_ms`; evidence
    /// stamped after `now_ms` counts as just arrived.
    pub fn silent_ms(&self, now_ms: i64) -> i64 {
        now_ms.saturating_sub(self.observed_ms).max(0)
    }

    /// The state a reader shows at `now_ms`, given what it found about
    /// the process. A state passes a window only when its evidence is
    /// strictly older than the window. In order:
    ///
    /// 1. `exited` stays as recorded; time and liveness never revive it.
    /// 2. A dead process (see [`Liveness::of_running_pid`] for a reused
    ///    pid) is `exited/killed`.
    /// 3. With liveness [`Liveness::Unknown`], every state is
    ///    `unknown/stale` past [`UNKNOWN_LIVENESS_WINDOW_MS`].
    /// 4. `starting` is `unknown/stale` past [`STARTING_WINDOW_MS`].
    /// 5. `working`, `compacting`, and `starting` past
    ///    [`Self::quiet_window_ms`] keep their state and detail and are
    ///    [`LiveConfidence::Quiet`]; they never decay to `ready`.
    /// 6. `waiting`, `ready`, and `error` do not age while the process
    ///    lives: an operator can leave a prompt for hours.
    ///
    /// A recorded detail that does not fit the state is dropped, and an
    /// `unknown/stale` state is always [`LiveConfidence::Stale`].
    pub fn effective(&self, now_ms: i64, liveness: Liveness) -> EffectiveLive {
        let detail = self.detail.filter(|detail| self.state.accepts(*detail));
        if self.state == LiveState::Exited {
            return EffectiveLive {
                state: LiveState::Exited,
                detail,
                confidence: LiveConfidence::Fresh,
            };
        }
        if liveness == Liveness::Dead {
            return EffectiveLive {
                state: LiveState::Exited,
                detail: Some(LiveDetail::Killed),
                confidence: LiveConfidence::Fresh,
            };
        }
        let silent_ms = self.silent_ms(now_ms);
        if liveness == Liveness::Unknown && silent_ms > UNKNOWN_LIVENESS_WINDOW_MS {
            return EffectiveLive::STALE;
        }
        if self.state == LiveState::Starting && silent_ms > STARTING_WINDOW_MS {
            return EffectiveLive::STALE;
        }
        let confidence = match self.state {
            LiveState::Working | LiveState::Compacting | LiveState::Starting
                if silent_ms > self.quiet_window_ms() =>
            {
                LiveConfidence::Quiet
            }
            LiveState::Unknown if detail == Some(LiveDetail::Stale) => LiveConfidence::Stale,
            _ => LiveConfidence::Fresh,
        };
        EffectiveLive {
            state: self.state,
            detail,
            confidence,
        }
    }
}

/// An observation bound to a graph node, with whether its process ran
/// when it was bound: the value of a snapshot's `node_live` table (ADR
/// 0205). The binder checks the recorded pid and start time against the
/// process table once, so a reader of the table needs no process check
/// of its own; the time rules still apply at read time, through
/// [`Self::effective`].
///
/// Serializes as the observation's fields beside `liveness`; a binding
/// without `liveness` reads it as [`Liveness::Unknown`].
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct LiveBinding {
    #[serde(flatten)]
    pub observation: LiveObservation,
    /// What the binder found about the observation's process.
    #[serde(default)]
    pub liveness: Liveness,
}

impl LiveBinding {
    pub fn new(observation: LiveObservation, liveness: Liveness) -> Self {
        Self {
            observation,
            liveness,
        }
    }

    /// The state a reader shows at `now_ms`: the observation read
    /// through [`LiveObservation::effective`] with the bound liveness.
    pub fn effective(&self, now_ms: i64) -> EffectiveLive {
        self.observation.effective(now_ms, self.liveness)
    }
}

/// The state a reader shows for an observation at a given time, from
/// [`LiveObservation::effective`]. Computed, never stored.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct EffectiveLive {
    pub state: LiveState,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_detail"
    )]
    pub detail: Option<LiveDetail>,
    pub confidence: LiveConfidence,
}

impl EffectiveLive {
    /// `unknown/stale`: evidence too old to trust.
    pub const STALE: EffectiveLive = EffectiveLive {
        state: LiveState::Unknown,
        detail: Some(LiveDetail::Stale),
        confidence: LiveConfidence::Stale,
    };
}

/// How many sessions are in each state, for a mux or any other group.
/// A mux has these counts instead of a state of its own; which state its
/// row shows is the viewer's choice ([`Self::dominant`] gives the
/// default).
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(default)]
pub struct LiveCounts {
    pub working: u32,
    pub waiting: u32,
    pub ready: u32,
    pub compacting: u32,
    pub error: u32,
    pub starting: u32,
    pub exited: u32,
    pub unknown: u32,
}

impl LiveCounts {
    /// The count for `state`.
    pub fn get(&self, state: LiveState) -> u32 {
        match state {
            LiveState::Working => self.working,
            LiveState::Waiting => self.waiting,
            LiveState::Ready => self.ready,
            LiveState::Compacting => self.compacting,
            LiveState::Error => self.error,
            LiveState::Starting => self.starting,
            LiveState::Exited => self.exited,
            LiveState::Unknown => self.unknown,
        }
    }

    /// Count one more session in `state`.
    pub fn add(&mut self, state: LiveState) {
        let count = match state {
            LiveState::Working => &mut self.working,
            LiveState::Waiting => &mut self.waiting,
            LiveState::Ready => &mut self.ready,
            LiveState::Compacting => &mut self.compacting,
            LiveState::Error => &mut self.error,
            LiveState::Starting => &mut self.starting,
            LiveState::Exited => &mut self.exited,
            LiveState::Unknown => &mut self.unknown,
        };
        *count = count.saturating_add(1);
    }

    /// Sessions counted in every state together.
    pub fn total(&self) -> u32 {
        LiveState::ALL
            .into_iter()
            .fold(0u32, |total, state| total.saturating_add(self.get(state)))
    }

    /// Whether no session is counted.
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// The counted state that ranks first by
    /// [`LiveState::default_precedence`], or `None` when nothing is
    /// counted.
    pub fn dominant(&self) -> Option<LiveState> {
        self.dominant_by(LiveState::default_precedence)
    }

    /// The counted state with the lowest `rank`, or `None` when nothing
    /// is counted. States of equal rank go by [`LiveState::ALL`] order.
    pub fn dominant_by<K: Ord>(&self, mut rank: impl FnMut(LiveState) -> K) -> Option<LiveState> {
        LiveState::ALL
            .into_iter()
            .filter(|state| self.get(*state) > 0)
            .min_by_key(|state| rank(*state))
    }
}

impl Extend<LiveState> for LiveCounts {
    fn extend<I: IntoIterator<Item = LiveState>>(&mut self, states: I) {
        for state in states {
            self.add(state);
        }
    }
}

impl FromIterator<LiveState> for LiveCounts {
    fn from_iter<I: IntoIterator<Item = LiveState>>(states: I) -> Self {
        let mut counts = Self::default();
        counts.extend(states);
        counts
    }
}

/// Reads an optional detail by name; a name outside the set reads as no
/// detail rather than failing the observation.
fn lenient_detail<'de, D>(deserializer: D) -> Result<Option<LiveDetail>, D::Error>
where
    D: Deserializer<'de>,
{
    let name: Option<String> = Option::deserialize(deserializer)?;
    Ok(name.as_deref().and_then(LiveDetail::from_name))
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
