//! The hook trace (ADR 0242): one redacted JSON line per `conspectus hook
//! write` call, for the operator who wants to see what a harness really
//! sends.
//!
//! `CONSPECTUS_HOOK_TRACE=<name>` turns it on. [`Trace::from_env`] is the
//! whole cost of a call without it. With it, the writer hands an [`Entry`]
//! to [`Trace::record`] after everything else it does, and the entry's
//! payload is redacted ([`super::redact`]) before it becomes text, so a
//! raw payload exists only in the writer's memory. The line is appended to
//! `hook-trace/<name>.jsonl` in the state directory, in one `write` on a
//! file opened with `O_APPEND`, with no lock.
//!
//! A line:
//!
//! ```json
//! {"v":1,"t_ms":1794400904859,"harness":"claude-code","payload":{...},
//!  "env":{"TMUX_PANE":true,"TMUX":true,"CLAUDE_PID":true},
//!  "events":["tool_finished"],"applied":"written",
//!  "after":{"state":"working","detail":"tool","subagents":0,
//!           "via_subagent":false,"since_ms":1794400900000,"rev":41}}
//! ```
//!
//! Nothing here may disturb the hook: the caller ignores the result of
//! [`Trace::record`], and nothing in this module prints, starts a process,
//! or opens a socket.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::declared::create_private_dir_all;

use super::redact::{redact_payload, size_placeholder};

/// The variable that names the trace.
pub const TRACE_ENV: &str = "CONSPECTUS_HOOK_TRACE";

/// The directory, inside the state directory, that holds the traces.
pub const TRACE_DIR: &str = "hook-trace";

/// The format version of a line.
pub const TRACE_VERSION: u32 = 1;

/// The longest name a trace may have.
const MAX_NAME_LEN: usize = 64;

/// The longest line, newline excluded. A line over it keeps its fields
/// and gives up its payload's structure.
pub const MAX_LINE_BYTES: usize = 64 * 1024;

/// Whether `name` is a trace name: `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`.
/// It is a file name, so it holds no separator and does not start with a
/// dot.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && name.len() <= MAX_NAME_LEN
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A trace the operator asked for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trace {
    name: String,
}

impl Trace {
    /// The trace `CONSPECTUS_HOOK_TRACE` names: `None` when it is unset
    /// or holds anything but a valid name. One environment lookup.
    pub fn from_env() -> Option<Self> {
        let value = std::env::var_os(TRACE_ENV)?;
        Self::named(value.to_str()?)
    }

    /// The trace called `name`, when `name` is valid.
    pub fn named(name: &str) -> Option<Self> {
        valid_name(name).then(|| Self {
            name: name.to_string(),
        })
    }

    /// The file this trace appends to, in the state directory `state`.
    pub fn path(&self, state: &Path) -> PathBuf {
        state.join(TRACE_DIR).join(format!("{}.jsonl", self.name))
    }

    /// The line for `entry`, with its newline. `None` only if the line
    /// cannot be serialized, which no value here can cause.
    pub fn render(&self, entry: &Entry<'_>) -> Option<String> {
        let raw = serde_json::from_slice::<Value>(entry.input).ok();
        let payload = match &raw {
            Some(raw) => redact_payload(raw, &self.name),
            None => Value::String(format!("<unparsed:{} bytes>", entry.input.len())),
        };
        let mut line = self.line(entry, &payload)?;
        if line.len() > MAX_LINE_BYTES
            && let Some(raw) = &raw
        {
            line = self.line(entry, &size_placeholder(raw))?;
        }
        line.push('\n');
        Some(line)
    }

    fn line(&self, entry: &Entry<'_>, payload: &Value) -> Option<String> {
        let harness = if valid_name(entry.harness) {
            entry.harness
        } else {
            "<invalid>"
        };
        let line = Line {
            v: TRACE_VERSION,
            t_ms: entry.t_ms,
            harness,
            payload,
            env: &entry.env,
            events: &entry.events,
            applied: entry.applied,
            after: entry.after.as_ref(),
        };
        serde_json::to_string(&line).ok()
    }

    /// Append `entry`'s line to the trace file under `state`, creating
    /// the directory (mode 0700) and the file (mode 0600) as needed. The
    /// caller ignores the result: a trace that cannot be written is not
    /// written.
    pub fn record(&self, state: &Path, entry: &Entry<'_>) -> io::Result<()> {
        let Some(line) = self.render(entry) else {
            return Ok(());
        };
        let dir = state.join(TRACE_DIR);
        create_private_dir_all(&dir)?;
        let mut options = fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(self.path(state))?;
        file.write_all(line.as_bytes())
    }
}

/// What the writer saw and did in one call.
#[derive(Debug)]
pub struct Entry<'a> {
    /// When the writer started, Unix epoch milliseconds.
    pub t_ms: i64,
    /// The harness named on the command line.
    pub harness: &'a str,
    /// The payload as the harness sent it. It is parsed and redacted when
    /// the line is rendered, and never written.
    pub input: &'a [u8],
    pub env: EnvFlags,
    /// The neutral events the adapter produced, by label, or `snapshot`.
    pub events: Vec<String>,
    /// What the store did: its [`Applied`](crate::live_store::Applied)
    /// word, `none` when nothing mapped, or `error`.
    pub applied: &'static str,
    /// The session's observation read back after the apply.
    pub after: Option<After>,
}

/// Which of the variables the writer reads were set, and nothing of their
/// values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct EnvFlags {
    #[serde(rename = "TMUX_PANE")]
    pub tmux_pane: bool,
    #[serde(rename = "TMUX")]
    pub tmux: bool,
    #[serde(rename = "CLAUDE_PID")]
    pub claude_pid: bool,
}

impl EnvFlags {
    pub fn from_env() -> Self {
        let set = |name: &str| std::env::var_os(name).is_some();
        Self {
            tmux_pane: set("TMUX_PANE"),
            tmux: set("TMUX"),
            claude_pid: set("CLAUDE_PID"),
        }
    }
}

/// A session's stored observation, as words and numbers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct After {
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<&'static str>,
    pub subagents: u16,
    pub via_subagent: bool,
    pub since_ms: i64,
    /// The store revision that last changed the entry.
    pub rev: u64,
}

/// The shape of a line, in the order its fields are written.
#[derive(Serialize)]
struct Line<'a> {
    v: u32,
    t_ms: i64,
    harness: &'a str,
    payload: &'a Value,
    env: &'a EnvFlags,
    events: &'a [String],
    applied: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<&'a After>,
}

#[cfg(test)]
#[path = "trace_tests.rs"]
mod tests;
