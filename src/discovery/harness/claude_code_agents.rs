//! Claude Code's own list of its sessions, `claude agents --json`, read
//! as live state (ADR 0218).
//!
//! The command prints a JSON array, one object per active session,
//! interactive or background. This module reads each object against a
//! whitelist of five fields and keeps only what they reduce to:
//!
//! | Field | Kept as |
//! | :- | :- |
//! | `pid` | the pid, when it is a number above 1 |
//! | `sessionId` | the session id, when it is a plain id token |
//! | `status`, `state`, `waitingFor` | the live state and detail of the table below |
//!
//! Every other field (`cwd`, `name`, `kind`, `startedAt`, `id`, and any a
//! later version adds) is skipped by the parser without being kept: `name`
//! can be a title generated from the operator's prompt. A session with no
//! state this module can read, or with neither a pid nor a session id to
//! match it by, is dropped.
//!
//! | Field | Value | State |
//! | :- | :- | :- |
//! | `status` | `busy` | `working`, `generating` |
//! | `status` | `waiting` | `waiting`, detail from `waitingFor` |
//! | `status` | `idle` | `ready`, no detail |
//! | `state` | `working` | `working`, `generating` |
//! | `state` | `blocked` | `waiting`, detail from `waitingFor` |
//! | `state` | `done` | `ready`, `finished` |
//! | `state` | `failed` | `error`, `failed` |
//! | `state` | `stopped` | `exited`, `ended` |
//! | `waitingFor` | `permission prompt`, `sandbox request` | `permission` |
//! | `waitingFor` | anything else, or absent | `input` |
//!
//! `status` decides when it holds one of its values; `state`, which only
//! background sessions carry, decides otherwise. Values compare trimmed
//! and in lowercase. `idle` has no detail because the list cannot tell a
//! finished turn from an interrupted one.
//!
//! The field names and values come from Claude Code's documentation (read
//! for 2.1.322), not from a recorded session.

use std::process::Command;
use std::time::Duration;

use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer};

use crate::discovery::live_poller::{LivePoller, PollRunner, RunOutcome, run_bounded};
use crate::model::live::{LiveDetail, LivePoll, LiveState, PolledAgent};

use super::HARNESS_KEY;

/// The most output a poll reads. A longer listing is no listing.
pub const MAX_OUTPUT_BYTES: usize = 1 << 20;

/// The most sessions a poll keeps.
pub const MAX_AGENTS: usize = 256;

/// The longest session id a poll keeps.
const MAX_SESSION_ID_LEN: usize = 128;

/// How long `claude agents --json` may run before it is killed.
pub const TIMEOUT: Duration = Duration::from_secs(3);

/// The poller for Claude Code: `claude agents --json` from `PATH`.
pub fn poller() -> LivePoller {
    LivePoller::new(HARNESS_KEY, Box::new(AgentList::system()), parse)
}

/// What the listing at `observed_ms` says, or `None` when it is not a
/// JSON array of objects or is longer than [`MAX_OUTPUT_BYTES`]. Each
/// session's run starts at `observed_ms`.
pub fn parse(output: &[u8], observed_ms: i64) -> Option<LivePoll> {
    if output.len() > MAX_OUTPUT_BYTES {
        return None;
    }
    let listed: Vec<Listed> = serde_json::from_slice(output).ok()?;
    let agents = listed
        .into_iter()
        .filter_map(|listed| listed.agent(observed_ms))
        .take(MAX_AGENTS)
        .collect();
    Some(LivePoll {
        observed_ms,
        agents,
    })
}

/// The state and detail a session's `status`, `state`, and `waitingFor`
/// say, by the table in the module documentation, or `None` when they
/// say nothing this module reads.
pub fn reading(
    status: Option<&str>,
    state: Option<&str>,
    waiting_for: Option<&str>,
) -> Option<(LiveState, Option<LiveDetail>)> {
    let waiting = || (LiveState::Waiting, Some(waiting_detail(waiting_for)));
    let by_status = normalized(status).and_then(|status| match status.as_str() {
        "busy" => Some((LiveState::Working, Some(LiveDetail::Generating))),
        "waiting" => Some(waiting()),
        "idle" => Some((LiveState::Ready, None)),
        _ => None,
    });
    by_status.or_else(|| {
        normalized(state).and_then(|state| match state.as_str() {
            "working" => Some((LiveState::Working, Some(LiveDetail::Generating))),
            "blocked" => Some(waiting()),
            "done" => Some((LiveState::Ready, Some(LiveDetail::Finished))),
            "failed" => Some((LiveState::Error, Some(LiveDetail::Failed))),
            "stopped" => Some((LiveState::Exited, Some(LiveDetail::Ended))),
            _ => None,
        })
    })
}

/// What a waiting session waits for: an approval, or other input.
fn waiting_detail(waiting_for: Option<&str>) -> LiveDetail {
    match normalized(waiting_for).as_deref() {
        Some("permission prompt" | "sandbox request") => LiveDetail::Permission,
        _ => LiveDetail::Input,
    }
}

fn normalized(value: Option<&str>) -> Option<String> {
    value.map(|value| value.trim().to_ascii_lowercase())
}

/// Whether `id` is a plain id token: ASCII letters, digits, `-`, `_`,
/// `.`, and `:`, at most [`MAX_SESSION_ID_LEN`] long. Anything else is
/// not kept, so no text reaches the snapshot through it.
fn plain_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_SESSION_ID_LEN
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

/// The whitelisted fields of one listed session. A field of the wrong
/// type reads as absent; a field not named here is skipped unread.
#[derive(Deserialize)]
struct Listed {
    #[serde(default, deserialize_with = "lenient")]
    pid: Option<i64>,
    #[serde(default, rename = "sessionId", deserialize_with = "lenient")]
    session_id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    status: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    state: Option<String>,
    #[serde(default, rename = "waitingFor", deserialize_with = "lenient")]
    waiting_for: Option<String>,
}

impl Listed {
    fn agent(self, observed_ms: i64) -> Option<PolledAgent> {
        let (state, detail) = reading(
            self.status.as_deref(),
            self.state.as_deref(),
            self.waiting_for.as_deref(),
        )?;
        let mut agent = PolledAgent::new(state, detail, observed_ms);
        agent.pid = self.pid.filter(|pid| *pid > 1);
        agent.session_id = self.session_id.filter(|id| plain_id(id));
        (agent.pid.is_some() || agent.session_id.is_some()).then_some(agent)
    }
}

/// A field read as `T`, or `None` when it holds something else.
fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(T::deserialize(value).ok())
}

/// Runs `claude agents --json` (ADR 0218): stdin closed, stderr dropped
/// unread, stdout read up to [`MAX_OUTPUT_BYTES`], killed after
/// [`TIMEOUT`], and with Claude Code's nonessential traffic turned off so
/// the call reaches no network.
#[derive(Clone, Debug)]
pub struct AgentList {
    binary: std::path::PathBuf,
    timeout: Duration,
}

impl AgentList {
    /// The `claude` on `PATH`.
    pub fn system() -> Self {
        Self::at("claude")
    }

    /// The program at `binary`, a name looked up on `PATH` or a path.
    pub fn at(binary: impl Into<std::path::PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            timeout: TIMEOUT,
        }
    }

    /// Kill the program after `timeout` instead of [`TIMEOUT`].
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn command(&self) -> Command {
        let mut command = crate::exec::command(crate::exec::Program::Harness, &self.binary);
        command
            .args(["agents", "--json"])
            .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");
        command
    }
}

impl PollRunner for AgentList {
    fn run(&self) -> RunOutcome {
        run_bounded(self.command(), self.timeout, MAX_OUTPUT_BYTES)
    }
}

#[cfg(test)]
#[path = "claude_code_agents_tests.rs"]
mod tests;
