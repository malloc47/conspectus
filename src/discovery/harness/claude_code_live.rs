//! Claude Code hook payloads as live-state events (ADR 0204, ADR 0206).
//!
//! `conspectus hook init claude-code` installs a command hook for each of
//! [`HOOK_SUBSCRIPTIONS`]. Claude Code runs `conspectus hook write
//! claude-code` once per event with the payload on stdin and waits for
//! it, and [`live_events`] says what the payload means in the neutral
//! vocabulary of [`LiveEventKind`]:
//!
//! | Event | Fields it reads | Live events |
//! | :- | :- | :- |
//! | `SessionStart` | `source` | session started, flagged after compaction when `source` is `compact` |
//! | `UserPromptSubmit` | | turn started |
//! | `PermissionRequest` | `tool_name` | wait started: a question for `AskUserQuestion`, a plan for `ExitPlanMode`, else permission |
//! | `PreToolUse` | `tool_name` | wait started: a question for `AskUserQuestion`, a plan for `ExitPlanMode`; nothing for another tool |
//! | `PostToolUse` | `tool_name`; for the subagent tool, `tool_response.status` and `tool_response.agentId` | tool finished; a child started when the tool launched one in the background (`async_launched`), a child stopped when it returned one's result (`completed`) |
//! | `PostToolUseFailure` | `is_interrupt` | tool finished; the main agent's turn ended, interrupted, when the operator interrupted the tool |
//! | `Notification` | `notification_type` | `permission_prompt`: wait confirmed (permission); `elicitation_dialog`, `elicitation_url_dialog`, `agent_needs_input`: wait confirmed (input); `idle_prompt`, `quota_auto_resume_disabled`: idle confirmed; `quota_auto_resume_fired`: turn started; `quota_auto_resume_stale`: the main agent waits for input |
//! | `SubagentStart`, `SubagentStop` | | child started, child stopped |
//! | `Stop` | `background_tasks[]`: `id`, `type`, `status` | turn ended (finished), then the background list of the subagent tasks still running when the payload has a list |
//! | `StopFailure` | `error` | turn failed: rate limited (`rate_limit`), auth (`authentication_failed`, `oauth_org_not_allowed`, `cloud_credential_error`), overloaded (`overloaded`), else failed |
//! | `PreCompact`, `PostCompact` | | compaction started, compaction ended |
//! | `SessionEnd` | | session ended |
//!
//! Every event also reads `hook_event_name`, `session_id`, and
//! `agent_id`, and an event of the main agent reads `cwd` and
//! `transcript_path` for the entry. An event that fires inside a child
//! agent carries `agent_id`: its waits and tools are that child's, its
//! paths describe the child, and the events about a turn (a prompt, a
//! stop, a failure, compaction) are the child's own turn, which the
//! vocabulary has no state for, so they are dropped. `Notification`s
//! come from the main session and confirm whichever wait is pending.
//!
//! Each field is looked up by name in the parsed payload and a field of
//! another type reads as absent, so one odd field never costs the event,
//! and a field the table does not name (`prompt`, `tool_input`,
//! `message`, `title`, `last_assistant_message`, error text, the rest of
//! `tool_response`, background-task descriptions) is never read, however
//! large (ADR 0206 decisions 10 and 11). `tool_name` and a task's `type`
//! and `status` only classify and are not kept, and `agent_type` is not
//! read (decision 12).
//!
//! Some gaps come with the hooks, and readers have to live with them:
//!
//! - An Esc interrupt fires no `Stop`, so the turn reads `working` until
//!   the next prompt or an `idle_prompt` notification, and past the
//!   quiet window it reads `working`, quiet. A tool the interrupt fails
//!   with `is_interrupt` does end the turn.
//! - Approving a prompt fires nothing. The wait lasts until the approved
//!   tool finishes, so a long approved tool reads `waiting`.
//! - Denying a prompt, or dismissing a question or a plan, interrupts
//!   the turn and fires nothing either: the wait lasts until the next
//!   prompt or idle notification.
//! - A parallel tool that finishes while another awaits approval ends
//!   the main agent's wait early; the `permission_prompt` notification
//!   about 6 s later, which fires only while the prompt is unanswered,
//!   restores it.
//! - A killed process fires no `SessionEnd`. Readers find its pid gone
//!   and show `exited/killed`.
//! - Hooks do not run before the operator accepts a folder's workspace
//!   trust dialog, nor under `disableAllHooks`, `--bare`, or managed
//!   hooks only. Such a session has no entry, which is no evidence, not
//!   idle.
//! - A `Stop` that another hook blocks still reads as a finished turn
//!   while the continuation it forces runs, until that continuation's
//!   own `Stop`. `stop_hook_active` says only that the response that
//!   just finished was such a continuation, so it changes nothing here.
//! - Before Claude Code 2.1.311, `SubagentStop` fired about twice a
//!   minute for every running child, so the child count reads low until
//!   a `Stop` lists the background children again. The main agent's
//!   state is not affected.

use serde_json::{Map, Value};

use crate::live_store::{
    FailureClass, LiveContext, LiveEvent, LiveEventKind, WaitKind, plausible_id,
};

use super::HARNESS_KEY;

/// One hook event `hook init claude-code` installs, with the matcher its
/// entry needs; `None` matches every occurrence of the event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HookSubscription {
    pub(crate) event: &'static str,
    pub(crate) matcher: Option<&'static str>,
}

const SESSION_START: &str = "SessionStart";
const USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
const PERMISSION_REQUEST: &str = "PermissionRequest";
const PRE_TOOL_USE: &str = "PreToolUse";
const POST_TOOL_USE: &str = "PostToolUse";
const POST_TOOL_USE_FAILURE: &str = "PostToolUseFailure";
const NOTIFICATION: &str = "Notification";
const SUBAGENT_START: &str = "SubagentStart";
const SUBAGENT_STOP: &str = "SubagentStop";
const STOP: &str = "Stop";
const STOP_FAILURE: &str = "StopFailure";
const PRE_COMPACT: &str = "PreCompact";
const POST_COMPACT: &str = "PostCompact";
const SESSION_END: &str = "SessionEnd";

/// The tools whose `PreToolUse` opens a dialog, and the wait each is.
const QUESTION_TOOL: &str = "AskUserQuestion";
const PLAN_TOOL: &str = "ExitPlanMode";

/// The subagent tool, under its current name and the one it had before.
const SUBAGENT_TOOLS: [&str; 2] = ["Agent", "Task"];

/// The events Conspectus subscribes to (ADR 0206 decision 1), in the
/// order `hook init` installs them. The session start matcher lists every
/// documented source; `PreToolUse` fires only for the two tools that
/// open a dialog, and `Notification` only for the types
/// [`live_events`] maps.
pub(crate) const HOOK_SUBSCRIPTIONS: [HookSubscription; 14] = [
    HookSubscription {
        event: SESSION_START,
        matcher: Some("startup|resume|clear|compact|fork"),
    },
    HookSubscription {
        event: USER_PROMPT_SUBMIT,
        matcher: None,
    },
    HookSubscription {
        event: PERMISSION_REQUEST,
        matcher: None,
    },
    HookSubscription {
        event: PRE_TOOL_USE,
        matcher: Some("AskUserQuestion|ExitPlanMode"),
    },
    HookSubscription {
        event: POST_TOOL_USE,
        matcher: None,
    },
    HookSubscription {
        event: POST_TOOL_USE_FAILURE,
        matcher: None,
    },
    HookSubscription {
        event: NOTIFICATION,
        matcher: Some(
            "permission_prompt|idle_prompt|elicitation_dialog|elicitation_url_dialog|\
             agent_needs_input|quota_auto_resume_fired|quota_auto_resume_stale|\
             quota_auto_resume_disabled",
        ),
    },
    HookSubscription {
        event: SUBAGENT_START,
        matcher: None,
    },
    HookSubscription {
        event: SUBAGENT_STOP,
        matcher: None,
    },
    HookSubscription {
        event: STOP,
        matcher: None,
    },
    HookSubscription {
        event: STOP_FAILURE,
        matcher: None,
    },
    HookSubscription {
        event: PRE_COMPACT,
        matcher: None,
    },
    HookSubscription {
        event: POST_COMPACT,
        matcher: None,
    },
    HookSubscription {
        event: SESSION_END,
        matcher: None,
    },
];

/// Whether `payload` names the session a process runs, which only a
/// session start does. A payload without an event name comes from a
/// caller older than per-event hooks, all of which ran on session start.
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
    // A child agent's paths describe the child (it may run in a worktree
    // of its own), so only the main agent's events record them.
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
        text_in(self.fields, key)
    }

    fn flag(&self, key: &str) -> bool {
        self.fields
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// The child agent the hook fired in; `None` for the main agent.
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
            SESSION_START => vec![K::SessionStarted {
                after_compaction: self.text("source") == Some("compact"),
            }],
            USER_PROMPT_SUBMIT if main => vec![K::TurnStarted],
            PERMISSION_REQUEST => vec![K::WaitStarted {
                agent,
                kind: self.dialog_tool().unwrap_or(WaitKind::Permission),
            }],
            PRE_TOOL_USE => self
                .dialog_tool()
                .map(|kind| K::WaitStarted { agent, kind })
                .into_iter()
                .collect(),
            POST_TOOL_USE => {
                let mut kinds = vec![K::ToolFinished { agent }];
                kinds.extend(self.subagent_result());
                kinds
            }
            POST_TOOL_USE_FAILURE => {
                let mut kinds = vec![K::ToolFinished { agent }];
                if main && self.flag("is_interrupt") {
                    kinds.push(K::TurnEnded { interrupted: true });
                }
                kinds
            }
            NOTIFICATION => self.notification().into_iter().collect(),
            SUBAGENT_START => agent
                .map(|id| K::SubagentStarted { id })
                .into_iter()
                .collect(),
            SUBAGENT_STOP => agent
                .map(|id| K::SubagentStopped { id })
                .into_iter()
                .collect(),
            STOP if main => {
                let mut kinds = vec![K::TurnEnded { interrupted: false }];
                if let Some(subagents) = self.background_subagents() {
                    kinds.push(K::BackgroundWork { subagents });
                }
                kinds
            }
            STOP_FAILURE if main => vec![K::TurnFailed {
                class: failure_class(self.text("error")),
            }],
            PRE_COMPACT if main => vec![K::CompactionStarted],
            POST_COMPACT if main => vec![K::CompactionEnded],
            SESSION_END => vec![K::SessionEnded],
            _ => Vec::new(),
        }
    }

    /// The wait a tool's dialog is, for the two tools that open one.
    fn dialog_tool(&self) -> Option<WaitKind> {
        match self.text("tool_name")? {
            QUESTION_TOOL => Some(WaitKind::Question),
            PLAN_TOOL => Some(WaitKind::Plan),
            _ => None,
        }
    }

    /// The child agent a subagent tool's result names: started when the
    /// tool launched it in the background, stopped when the tool returned
    /// its result. Only the result's `status` and `agentId` are read.
    fn subagent_result(&self) -> Option<LiveEventKind> {
        if !SUBAGENT_TOOLS.contains(&self.text("tool_name")?) {
            return None;
        }
        let response = self.fields.get("tool_response")?.as_object()?;
        let id = text_in(response, "agentId")
            .filter(|id| plausible_id(id))?
            .to_string();
        match text_in(response, "status")? {
            "async_launched" => Some(LiveEventKind::SubagentStarted { id }),
            "completed" => Some(LiveEventKind::SubagentStopped { id }),
            _ => None,
        }
    }

    fn notification(&self) -> Option<LiveEventKind> {
        use LiveEventKind as K;
        // The dialog types arrive seconds after the dialog shows, and only
        // while it is still open, without saying whose dialog it is.
        Some(match self.text("notification_type")? {
            "permission_prompt" => K::WaitConfirmed {
                kind: WaitKind::Permission,
            },
            "elicitation_dialog" | "elicitation_url_dialog" | "agent_needs_input" => {
                K::WaitConfirmed {
                    kind: WaitKind::Input,
                }
            }
            // The prompt has sat idle; a usage-limit wait that gave up
            // leaves the session at its prompt too.
            "idle_prompt" | "quota_auto_resume_disabled" => K::IdleConfirmed,
            // The turn a usage limit stopped goes on by itself.
            "quota_auto_resume_fired" => K::TurnStarted,
            // The limit reset while the machine slept, and the session
            // waits for Enter before it goes on.
            "quota_auto_resume_stale" => K::WaitStarted {
                agent: None,
                kind: WaitKind::Input,
            },
            _ => return None,
        })
    }

    /// The ids of the subagent tasks a `Stop` lists as still running, or
    /// `None` when the payload has no list (Claude Code before 2.1.145).
    /// Shell, monitor, and other tasks are not child agents.
    fn background_subagents(&self) -> Option<Vec<String>> {
        let tasks = self.fields.get("background_tasks")?.as_array()?;
        Some(
            tasks
                .iter()
                .filter_map(Value::as_object)
                .filter(|task| text_in(task, "type") == Some("subagent"))
                .filter(|task| !text_in(task, "status").is_some_and(task_is_over))
                .filter_map(|task| text_in(task, "id").filter(|id| plausible_id(id)))
                .map(str::to_string)
                .collect(),
        )
    }
}

fn text_in<'a>(fields: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    fields
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

/// Whether a background task's status says it no longer runs. A status
/// this list does not know counts as running, so a child is not dropped
/// for a word a newer version added.
fn task_is_over(status: &str) -> bool {
    matches!(
        status,
        "completed" | "failed" | "killed" | "stopped" | "cancelled" | "canceled" | "done" | "error"
    )
}

/// The failure class of a `StopFailure` error type.
fn failure_class(error: Option<&str>) -> FailureClass {
    match error {
        Some("rate_limit") => FailureClass::RateLimited,
        Some("authentication_failed" | "oauth_org_not_allowed" | "cloud_credential_error") => {
            FailureClass::Auth
        }
        Some("overloaded") => FailureClass::Overloaded,
        _ => FailureClass::Failed,
    }
}

#[cfg(test)]
#[path = "claude_code_live_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "claude_code_sequence_tests.rs"]
mod sequence_tests;
