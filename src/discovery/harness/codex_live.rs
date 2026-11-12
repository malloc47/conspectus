//! Codex hook payloads as live-state events (ADR 0204, ADR 0206).
//!
//! `conspectus hook init codex` installs a command hook for each of
//! [`HOOK_EVENTS`] (ADR 0206 decision 2). Codex runs `conspectus hook
//! write codex` once per event with the payload on stdin and waits for
//! it, and [`live_events`] says what the payload means in the neutral
//! vocabulary of [`LiveEventKind`]:
//!
//! | Event | Fields it reads | Live events |
//! | :- | :- | :- |
//! | `SessionStart` | `source` | session started, flagged after compaction when `source` is `compact` |
//! | `UserPromptSubmit` | | turn started |
//! | `PermissionRequest` | | wait started (permission) |
//! | `PostToolUse` | | tool finished |
//! | `Stop` | | turn ended (finished), in plan mode as in any other |
//! | `Interrupt` | | turn ended (interrupted) |
//! | `SubagentStart`, `SubagentStop` | | child started, child stopped |
//! | `PreCompact`, `PostCompact` | | compaction started, compaction ended |
//! | `SessionEnd` | | session ended |
//!
//! Every event also reads `hook_event_name`, `session_id`, and
//! `agent_id`, and an event of the main agent reads `cwd` and
//! `transcript_path` for the entry. A subagent's hooks carry its
//! parent's `session_id`, so they reach the parent's entry; one that
//! carries `agent_id` is the child's. Its waits and tools count for the
//! child, its paths describe the child and are not recorded, and its
//! turn events (a session start, a prompt, a stop, an interrupt,
//! compaction, a session end) are dropped, since the vocabulary has no
//! state for a child's turn. Any other event, such as `PreToolUse`,
//! `PreModelRequest`, `PostModelRequest`, or one a later Codex adds,
//! yields nothing.
//!
//! Each field is looked up by name in the parsed payload, and a field of
//! another type reads as absent, so one odd field never costs the event.
//! A field the table does not name is never read, however large:
//! `prompt`, `tool_input`, `tool_response`, `last_assistant_message`,
//! `agent_transcript_path`, a model request's `error`, and also
//! `tool_name`, `agent_type`, `permission_mode`, `turn_id`, `trigger`,
//! and `reason`, which no state here needs (ADR 0206 decisions 10 to
//! 12).
//!
//! Some gaps come with the hooks, and readers have to live with them:
//!
//! - Approving a prompt fires nothing. The wait lasts until the approved
//!   tool finishes, so a long approved command reads `waiting`, and a
//!   declined one reads `waiting` until a later tool, the turn's `Stop`,
//!   or an `Interrupt`.
//! - A turn that ends in plan mode may have proposed a plan the operator
//!   now has to accept, or only answered; only the assistant's message
//!   could tell, and it is never read, so both read as a finished turn.
//! - MCP elicitations and sign-in prompts fire no hook, so a session
//!   waiting on one reads `working`, and past the quiet window `working`,
//!   quiet.
//! - A crashed or killed process fires no `SessionEnd`. Readers find its
//!   pid gone and show `exited/killed`.
//! - Current Codex skips a hook it does not manage until the operator
//!   trusts it in `/hooks`, and none run with hooks turned off or under
//!   managed hooks only. Such a session has no entry, which is no
//!   evidence, not idle.
//! - Codex documents `agent_id` on a subagent's permission request and
//!   on the subagent events only. Should a child's finished tool come
//!   without it, it counts for the main agent and ends that agent's
//!   wait, if one is pending.
//! - Whether `SubagentStop` fires once per child or after each of its
//!   turns is undocumented; a child idle between turns may read as
//!   stopped.

use serde_json::{Map, Value};

use crate::live_store::{LiveContext, LiveEvent, LiveEventKind, WaitKind};

use super::HARNESS_KEY;

const SESSION_START: &str = "SessionStart";
const USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
const PERMISSION_REQUEST: &str = "PermissionRequest";
const POST_TOOL_USE: &str = "PostToolUse";
const STOP: &str = "Stop";
const INTERRUPT: &str = "Interrupt";
const SESSION_END: &str = "SessionEnd";
const SUBAGENT_START: &str = "SubagentStart";
const SUBAGENT_STOP: &str = "SubagentStop";
const PRE_COMPACT: &str = "PreCompact";
const POST_COMPACT: &str = "PostCompact";

/// The events Conspectus subscribes to (ADR 0206 decision 2), in the
/// order `hook init codex` installs them. None needs a matcher: each
/// counts on every occurrence.
pub(crate) const HOOK_EVENTS: [&str; 11] = [
    SESSION_START,
    USER_PROMPT_SUBMIT,
    PERMISSION_REQUEST,
    POST_TOOL_USE,
    STOP,
    INTERRUPT,
    SESSION_END,
    SUBAGENT_START,
    SUBAGENT_STOP,
    PRE_COMPACT,
    POST_COMPACT,
];

/// Whether `payload` names the session a process runs, which only a
/// session start does. A payload without an event name comes from a
/// caller older than per-event hooks, which ran on session start only.
pub(super) fn is_attribution_event(payload: &Value) -> bool {
    payload
        .get("hook_event_name")
        .is_none_or(|name| name.as_str() == Some(SESSION_START))
}

/// The live events `payload` says, in order, each carrying `context`
/// with the cwd and transcript path the payload names. An event this
/// mapping does not cover, or a payload without a session id, yields
/// nothing.
pub(super) fn live_events(
    payload: &Value,
    context: &LiveContext,
    observed_ms: i64,
) -> Vec<LiveEvent> {
    let Some(hook) = Hook::of(payload) else {
        return Vec::new();
    };
    let Some(session_id) = hook.text("session_id") else {
        return Vec::new();
    };
    let kinds = hook.kinds();
    if kinds.is_empty() {
        return Vec::new();
    }
    // A child's paths describe the child (its own rollout), so only the
    // main agent's events record them.
    let path = |key: &str, fallback: &Option<String>| match hook.agent() {
        None => hook
            .text(key)
            .map(str::to_string)
            .or_else(|| fallback.clone()),
        Some(_) => fallback.clone(),
    };
    let context = LiveContext {
        cwd: path("cwd", &context.cwd),
        transcript_path: path("transcript_path", &context.transcript_path),
        ..context.clone()
    };
    kinds
        .into_iter()
        .map(|kind| {
            LiveEvent::new(HARNESS_KEY, session_id, observed_ms, kind).with_context(context.clone())
        })
        .collect()
}

/// A hook payload, read one named field at a time. Nothing walks the
/// payload, so a field no method names is never looked at.
struct Hook<'a> {
    fields: &'a Map<String, Value>,
}

impl<'a> Hook<'a> {
    fn of(payload: &'a Value) -> Option<Self> {
        payload.as_object().map(|fields| Self { fields })
    }

    /// The string at `key`, or `None` when it is missing, empty, or not a
    /// string.
    fn text(&self, key: &str) -> Option<&'a str> {
        self.fields
            .get(key)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
    }

    /// The child agent the hook fired for; `None` for the main agent.
    fn agent(&self) -> Option<&'a str> {
        self.text("agent_id")
    }

    fn kinds(&self) -> Vec<LiveEventKind> {
        use LiveEventKind as K;
        let agent = self.agent().map(str::to_string);
        let main = agent.is_none();
        let Some(event) = self.text("hook_event_name") else {
            return Vec::new();
        };
        match event {
            // A start after compaction resumes the session; every other
            // source (startup, resume, clear, fork) begins one.
            SESSION_START if main => vec![K::SessionStarted {
                after_compaction: self.text("source") == Some("compact"),
            }],
            USER_PROMPT_SUBMIT if main => vec![K::TurnStarted],
            // Codex asks only for approvals here (a command, a patch, a
            // network or MCP call); a question to the operator is a tool
            // of its own.
            PERMISSION_REQUEST => vec![K::WaitStarted {
                agent,
                kind: WaitKind::Permission,
            }],
            POST_TOOL_USE => vec![K::ToolFinished { agent }],
            STOP if main => vec![K::TurnEnded { interrupted: false }],
            INTERRUPT if main => vec![K::TurnEnded { interrupted: true }],
            SUBAGENT_START => agent
                .map(|id| K::SubagentStarted { id })
                .into_iter()
                .collect(),
            SUBAGENT_STOP => agent
                .map(|id| K::SubagentStopped { id })
                .into_iter()
                .collect(),
            PRE_COMPACT if main => vec![K::CompactionStarted],
            POST_COMPACT if main => vec![K::CompactionEnded],
            SESSION_END if main => vec![K::SessionEnded],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
#[path = "codex_live_tests.rs"]
mod tests;
