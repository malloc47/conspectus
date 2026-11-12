//! What a writer hands the store: a [`LiveEvent`] from an event source,
//! or a [`LiveSnapshot`] from a plugin that keeps its own state.
//!
//! Every field is an id, a path, a number, or a closed enum, so a
//! payload's text has nowhere to go. The kinds are the smallest set onto
//! which the research tables for the stateless hook harnesses map one
//! payload at a time; what each one means for the entry is in
//! [`super::fold`].
//!
//! - [`LiveEventKind::SessionStarted`] covers every session start; the
//!   start that follows a compaction is flagged, because it resumes the
//!   session rather than resetting it.
//! - Turns have one start ([`LiveEventKind::TurnStarted`]) and three
//!   ends, because a turn that ends is ready
//!   ([`LiveEventKind::TurnEnded`], finished or interrupted) or failed
//!   ([`LiveEventKind::TurnFailed`]), and a harness that tells neither can
//!   still confirm the prompt is idle ([`LiveEventKind::IdleConfirmed`]).
//! - Waits are per agent ([`LiveEventKind::WaitStarted`],
//!   [`LiveEventKind::WaitEnded`]): a child agent can ask the operator
//!   while the main agent works. Approval itself sends nothing in the
//!   harnesses surveyed, so a finished tool
//!   ([`LiveEventKind::ToolFinished`]) is what ends a wait.
//! - Child agents are tracked by id, so a repeated start counts once and
//!   a stop for an id never started is ignored;
//!   [`LiveEventKind::BackgroundWork`] replaces the set with the
//!   harness's own list when it sends one.
//! - Compaction has a start and an end, so the store can resume the state
//!   it interrupted.
//!
//! A harness signal that fits none of these is dropped by its adapter,
//! not stretched to fit.

use serde::{Deserialize, Serialize};

use crate::model::live::{LiveDetail, LiveState};

use super::file::LiveTmux;

/// One harness event in the neutral vocabulary, as its adapter mapped it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LiveEvent {
    /// The harness key, as an `AgentSession` id spells it.
    pub harness: String,
    /// The harness's own session id.
    pub session_id: String,
    /// When the writer received the event, Unix epoch milliseconds.
    pub observed_ms: i64,
    pub context: LiveContext,
    pub kind: LiveEventKind,
}

impl LiveEvent {
    /// An event with no attribution context.
    pub fn new(
        harness: impl Into<String>,
        session_id: impl Into<String>,
        observed_ms: i64,
        kind: LiveEventKind,
    ) -> Self {
        Self {
            harness: harness.into(),
            session_id: session_id.into(),
            observed_ms,
            context: LiveContext::default(),
            kind,
        }
    }

    pub fn with_context(mut self, context: LiveContext) -> Self {
        self.context = context;
        self
    }
}

/// What the writer knows about where a session runs. A field the writer
/// does not know is `None`, and the entry keeps what an earlier writer
/// recorded.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct LiveContext {
    /// The agent's working directory.
    pub cwd: Option<String>,
    /// The session's transcript file.
    pub transcript_path: Option<String>,
    /// The tmux pane the agent runs in.
    pub tmux: Option<LiveTmux>,
    /// The agent's process.
    pub pid: Option<i64>,
    /// The start time of `pid`, from [`super::process_start_time`].
    pub pid_start: Option<u64>,
}

/// What happened, in a closed set no harness owns. An `agent` of `None`
/// is the session's main agent; `Some` names a child agent by the id
/// its harness gives it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveEventKind {
    /// The session started, resumed, or was replaced in its process.
    /// `after_compaction` marks the start a harness sends when compaction
    /// finishes, which keeps the session's waits, child agents, and open
    /// turn.
    SessionStarted { after_compaction: bool },
    /// A turn began: a prompt was submitted, or the harness resumed one
    /// on its own.
    TurnStarted,
    /// A failed model request is being retried inside the turn;
    /// `rate_limited` when a rate limit holds it back.
    Retrying { rate_limited: bool },
    /// An agent is blocked on the operator for `kind`.
    WaitStarted {
        agent: Option<String>,
        kind: WaitKind,
    },
    /// The agent's wait ended without a tool finishing (a denial the
    /// harness reports, or a dialog it closed).
    WaitEnded { agent: Option<String> },
    /// One of the agent's tools finished, successfully or not, which ends
    /// any wait the agent had.
    ToolFinished { agent: Option<String> },
    /// A child agent started.
    SubagentStarted { id: String },
    /// A child agent stopped.
    SubagentStopped { id: String },
    /// The complete list of child agents still running in the
    /// background, from a harness that reports it. Send it only when the
    /// list is complete, as after a turn ends, when no foreground child
    /// still runs.
    BackgroundWork { subagents: Vec<String> },
    /// Context compaction began.
    CompactionStarted,
    /// Context compaction finished.
    CompactionEnded,
    /// The main agent's turn ended and the prompt is open again.
    TurnEnded { interrupted: bool },
    /// The main agent's turn ended on an error of `class`.
    TurnFailed { class: FailureClass },
    /// The harness confirms its prompt has sat idle: no turn runs and no
    /// dialog of the main agent is open.
    IdleConfirmed,
    /// The session ended. Its process may go on with another session.
    SessionEnded,
}

/// Why an agent waits on the operator.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitKind {
    /// Approval of an action.
    Permission,
    /// An answer to a question the agent asked.
    Question,
    /// Approval of a plan.
    Plan,
    /// Any other input.
    Input,
}

impl WaitKind {
    /// The `waiting` detail this kind shows as.
    pub fn detail(self) -> LiveDetail {
        match self {
            WaitKind::Permission => LiveDetail::Permission,
            WaitKind::Question => LiveDetail::Question,
            WaitKind::Plan => LiveDetail::Plan,
            WaitKind::Input => LiveDetail::Input,
        }
    }
}

/// Why a turn failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// A rate or usage limit.
    RateLimited,
    /// Authentication failed.
    Auth,
    /// The provider was overloaded.
    Overloaded,
    /// Any other failure.
    Failed,
}

impl FailureClass {
    /// The `error` detail this class shows as.
    pub fn detail(self) -> LiveDetail {
        match self {
            FailureClass::RateLimited => LiveDetail::RateLimited,
            FailureClass::Auth => LiveDetail::Auth,
            FailureClass::Overloaded => LiveDetail::Overloaded,
            FailureClass::Failed => LiveDetail::Failed,
        }
    }
}

/// A plugin's complete state for one session. The store replaces the
/// entry with it when `instance` matches the entry's and `seq` is
/// higher, or when `instance` is new and `observed_ms` is not older than
/// the entry's latest evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LiveSnapshot {
    pub harness: String,
    pub session_id: String,
    /// The plugin's instance: a fresh random id each time it loads.
    pub instance: String,
    /// Counts up with every snapshot one instance sends.
    pub seq: u64,
    /// When the plugin sent the snapshot, Unix epoch milliseconds.
    pub observed_ms: i64,
    pub context: LiveContext,
    pub state: LiveState,
    /// A detail the state does not carry is dropped.
    pub detail: Option<LiveDetail>,
    /// When the plugin's state began.
    pub since_ms: i64,
    /// Active child agents.
    pub subagents: u16,
    /// The state is a child agent's, raised to this session.
    pub via_subagent: bool,
    /// Seconds between the plugin's heartbeats; 0 when it sends none.
    pub heartbeat_s: u32,
}

/// The longest id the store accepts.
const MAX_ID_LEN: usize = 256;
/// The longest path the store accepts.
const MAX_PATH_LEN: usize = 4096;

/// Whether `id` looks like an identifier: short, with no whitespace or
/// control character. Text from a payload rarely does, so an id that
/// fails is refused rather than stored.
fn plausible_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && !id.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Whether `path` looks like an absolute path with no control character.
fn plausible_path(path: &str) -> bool {
    path.starts_with('/') && path.len() <= MAX_PATH_LEN && !path.chars().any(char::is_control)
}

/// Whether `name` looks like a short name (a tmux session or pane).
fn plausible_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_ID_LEN && !name.chars().any(char::is_control)
}

impl LiveContext {
    /// The context with every field that does not look like what it
    /// should hold dropped.
    pub(crate) fn sanitized(&self) -> Self {
        let keep = |value: &Option<String>, plausible: fn(&str) -> bool| {
            value.clone().filter(|value| plausible(value))
        };
        let tmux = self
            .tmux
            .as_ref()
            .map(|tmux| LiveTmux {
                socket: keep(&tmux.socket, plausible_path),
                session: keep(&tmux.session, plausible_name),
                pane: keep(&tmux.pane, plausible_id),
            })
            .filter(|tmux| !tmux.is_empty());
        Self {
            cwd: keep(&self.cwd, plausible_path),
            transcript_path: keep(&self.transcript_path, plausible_path),
            tmux,
            pid: self.pid.filter(|pid| *pid > 0),
            pid_start: self.pid.filter(|pid| *pid > 0).and(self.pid_start),
        }
    }
}

impl LiveEventKind {
    /// Whether every id the kind carries looks like one.
    fn plausible(&self) -> bool {
        let agent = |agent: &Option<String>| agent.as_deref().is_none_or(plausible_id);
        match self {
            LiveEventKind::WaitStarted { agent: who, .. }
            | LiveEventKind::WaitEnded { agent: who }
            | LiveEventKind::ToolFinished { agent: who } => agent(who),
            LiveEventKind::SubagentStarted { id } | LiveEventKind::SubagentStopped { id } => {
                plausible_id(id)
            }
            LiveEventKind::BackgroundWork { subagents } => {
                subagents.iter().all(|id| plausible_id(id))
            }
            LiveEventKind::SessionStarted { .. }
            | LiveEventKind::TurnStarted
            | LiveEventKind::Retrying { .. }
            | LiveEventKind::CompactionStarted
            | LiveEventKind::CompactionEnded
            | LiveEventKind::TurnEnded { .. }
            | LiveEventKind::TurnFailed { .. }
            | LiveEventKind::IdleConfirmed
            | LiveEventKind::SessionEnded => true,
        }
    }
}

impl LiveEvent {
    /// The event with implausible context fields dropped, or `None` when
    /// one of its ids does not look like an id.
    pub(crate) fn sanitized(&self) -> Option<Self> {
        (plausible_id(&self.harness) && plausible_id(&self.session_id) && self.kind.plausible())
            .then(|| Self {
                context: self.context.sanitized(),
                ..self.clone()
            })
    }
}

impl LiveSnapshot {
    /// The snapshot with implausible context fields dropped, or `None`
    /// when one of its ids does not look like an id.
    pub(crate) fn sanitized(&self) -> Option<Self> {
        (plausible_id(&self.harness)
            && plausible_id(&self.session_id)
            && plausible_id(&self.instance))
        .then(|| Self {
            context: self.context.sanitized(),
            ..self.clone()
        })
    }
}
