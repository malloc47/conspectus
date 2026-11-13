//! Previews computed from a field's value alone (ADR 0112 §2): full
//! text, lists, command lines one argument per line, timestamps in
//! local time and UTC with their age, and glossary entries for values
//! from a fixed set. Nothing here reads the system beyond the clock and
//! time zone.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::explorer::FieldValue;

/// The lines a value previews, or `None` for a value whose preview is
/// loaded or names a node. Labels and notes take `muted`, the theme's
/// muted modifier.
pub fn value_lines(
    value: &FieldValue,
    now: Option<i64>,
    muted: Modifier,
) -> Option<Vec<Line<'static>>> {
    match value {
        FieldValue::Text(text) => Some(text.lines().map(|l| Line::raw(l.to_string())).collect()),
        FieldValue::List(items) => Some(items.iter().map(|i| Line::raw(i.clone())).collect()),
        FieldValue::Command(argv) => Some(command_lines(argv)),
        FieldValue::Timestamp(epoch) => Some(timestamp_lines(*epoch, now, muted)),
        FieldValue::Glossary { field, values } => Some(glossary_lines(field, values, muted)),
        FieldValue::Path(_)
        | FieldValue::Ref { .. }
        | FieldValue::Commit { .. }
        | FieldValue::Process(_)
        | FieldValue::Node(_) => None,
    }
}

fn bold(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::new().add_modifier(Modifier::BOLD))
}

fn dim(text: impl Into<String>, muted: Modifier) -> Span<'static> {
    Span::styled(text.into(), Style::new().add_modifier(muted))
}

/// The program, then each argument indented on its own line.
fn command_lines(argv: &[String]) -> Vec<Line<'static>> {
    argv.iter()
        .enumerate()
        .map(|(i, arg)| {
            if i == 0 {
                Line::from(bold(arg.clone()))
            } else {
                Line::raw(format!("  {arg}"))
            }
        })
        .collect()
}

pub use crate::argv::split_command_line;

fn timestamp_lines(epoch: i64, now: Option<i64>, muted: Modifier) -> Vec<Line<'static>> {
    let Some(utc) = chrono::DateTime::from_timestamp(epoch, 0) else {
        return vec![Line::raw(format!("{epoch} (out of range)"))];
    };
    let mut lines = Vec::new();
    if let Some((offset, zone)) = local_offset(epoch)
        && let Some(offset) = chrono::FixedOffset::east_opt(offset)
    {
        let local = utc.with_timezone(&offset);
        lines.push(Line::from(vec![
            dim("local  ", muted),
            Span::raw(format!("{} {zone}", local.format("%Y-%m-%d %H:%M:%S %:z"))),
        ]));
    }
    lines.push(Line::from(vec![
        dim("UTC    ", muted),
        Span::raw(utc.format("%Y-%m-%d %H:%M:%S").to_string()),
    ]));
    if let Some(age) = tuikit::age::Age::of(Some(epoch), now) {
        lines.push(Line::from(vec![
            dim("age    ", muted),
            Span::raw(age.ago()),
        ]));
    }
    lines
}

/// The local UTC offset in seconds and the zone's abbreviation at
/// `epoch`, from the C library's time-zone database.
#[cfg(unix)]
fn local_offset(epoch: i64) -> Option<(i32, String)> {
    let time = libc::time_t::try_from(epoch).ok()?;
    // SAFETY: `localtime_r` writes only into `tm`, which outlives the
    // call; a null return means it failed and `tm` is not read.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::localtime_r(&raw const time, &raw mut tm) };
    if result.is_null() {
        return None;
    }
    let zone = if tm.tm_zone.is_null() {
        String::new()
    } else {
        // SAFETY: a non-null `tm_zone` from `localtime_r` points at a
        // NUL-terminated abbreviation owned by the C library.
        unsafe { std::ffi::CStr::from_ptr(tm.tm_zone) }
            .to_string_lossy()
            .into_owned()
    };
    Some((i32::try_from(tm.tm_gmtoff).ok()?, zone))
}

#[cfg(not(unix))]
fn local_offset(_epoch: i64) -> Option<(i32, String)> {
    None
}

fn glossary_lines(field: &str, values: &[String], muted: Modifier) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for value in values {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(bold(value.clone())));
        lines.push(match glossary_entry(field, value) {
            Some(entry) => Line::raw(entry),
            None => Line::from(dim(format!("no glossary entry for `{value}`"), muted)),
        });
    }
    lines
}

/// What a value from a fixed set means. Kept beside the code that
/// produces the values; a value missing here previews as having no
/// entry rather than as nothing.
pub fn glossary_entry(field: &str, value: &str) -> Option<String> {
    let entry = match (field, value.to_ascii_lowercase().as_str()) {
        ("harness" | "harness_key", "claude-code") => "Anthropic's Claude Code CLI.",
        ("harness" | "harness_key", "codex") => "OpenAI's Codex CLI.",
        ("harness" | "harness_key", "opencode") => "The opencode CLI.",
        ("harness" | "harness_key", "pi") => "The Pi coding agent CLI.",
        ("harness" | "harness_key", "aider") => {
            "The aider CLI. The viewer can't read its transcripts."
        }
        ("backend", "tmux") => "A tmux session on the default socket.",
        ("backend", "zellij") => "A zellij session.",
        ("provider", "agent-deck" | "agent_deck") => {
            "Declared by agent-deck, which groups several repos into one workspace."
        }
        ("provider", "atelier") => "Declared by an Atelier workspace manifest.",
        ("provider", "generic_workspace") => "A directory Conspectus treats as a workspace.",
        ("provider", "github") => "Hosted on GitHub; read with `gh`.",
        ("provider", "gitlab") => "Hosted on GitLab.",
        ("status", "active") => {
            "The harness recorded activity for this session; the age is its last turn."
        }
        ("state", "open") => "Open for review.",
        ("state", "closed") => "Closed without merging.",
        ("state", "merged") => "Merged into its base branch.",
        // What an agent is doing, and the detail beside it.
        ("state", "waiting") => "The agent is blocked on you.",
        ("state", "working") => "A turn is running.",
        ("state", "ready") => "The turn is over, and the agent takes a prompt.",
        ("state", "compacting") => "The harness is compacting the conversation's context.",
        ("state", "error") => "The turn ended on an error. The agent takes a prompt again.",
        ("state", "starting") => "The agent's process is up and has said nothing more.",
        ("state", "exited") => "The session is over.",
        ("state", "unknown") => "There is no usable evidence of what the agent is doing.",
        ("state", "generating") => "The model is writing.",
        ("state", "tool") => "A tool is running.",
        ("state", "retrying") => "A request failed and is being retried.",
        ("state", "rate_limited") => {
            "The provider is limiting requests: a working agent waits out the limit to retry, and \
             an error is a turn that ended on it."
        }
        ("state", "permission") => "It asks your permission to go on.",
        ("state", "question") => "It asked you a question.",
        ("state", "plan") => "It waits for you to approve a plan.",
        ("state", "input") => {
            "It waits for other input, such as a form a tool asks you to fill in."
        }
        ("state", "finished") => "The turn ended by itself.",
        ("state", "interrupted") => "The turn was interrupted.",
        ("state", "auth") => "A sign-in or credential problem ended the turn.",
        ("state", "overloaded") => "The provider was overloaded.",
        ("state", "failed") => "The turn failed.",
        ("state", "ended") => "It quit, or its process moved on to a new session.",
        ("state", "killed") => {
            "Its process is gone without a word, or its pid now belongs to another process."
        }
        ("state", "stale") => {
            "Its process can't be checked, and nothing has arrived from it for fifteen minutes."
        }
        ("state", "unseen") => {
            "You haven't looked at this finished turn: no tmux client has attached to its mux \
             since it ended, or, for a session with no mux, the cursor hasn't rested on its row \
             in this TUI. Its mark is bold until you do."
        }
        ("state", "seen") => {
            "You have looked at this finished turn: a tmux client attached to its mux since it \
             ended, or, for a session with no mux, the cursor rested on its row in this TUI."
        }
        ("source", "hook") => "Reported by the harness's hooks or plugin.",
        ("source", "pane_title") => {
            "Read from the title the agent sets on its tmux pane, which says whether it waits on \
             you or works, but not what it waits for."
        }
        ("source", "harness_poll") => {
            "Read from the harness's own list of its sessions, which Conspectus asks for when \
             the hooks have gone quiet on an agent that was working or waiting."
        }
        ("source", "unknown") => "Reported by a source this version of Conspectus doesn't know.",
        ("evidence", "fresh") => {
            "Recent enough to trust, or a state that doesn't age while the agent runs."
        }
        ("evidence", "quiet") => {
            "Work with no news for longer than its source reports in: still under way as far as \
             anyone knows, but not confirmed."
        }
        ("evidence", "stale") => "Too old to trust, so the agent's state reads unknown.",
        ("draft", "true") => "A draft: not yet ready for review.",
        ("draft", "false") => "Ready for review.",
        ("attached", "yes") => "A tmux client is attached right now.",
        ("attached", "no") => "No tmux client is attached.",
        ("scope", "workspace") => "The fork belongs to a whole workspace.",
        ("capabilities", "mode:worktree") => "The fork works in its own git worktree.",
        ("capabilities", "mode:selected") => "The fork works on selected files.",
        ("capabilities", "mode:research") => "The fork researches without changing files.",
        ("capabilities", "read_only") => "The fork may not write.",
        ("capabilities", "state:inherit") => "The fork starts from its parent's state.",
        ("capabilities", "state:shared") => "The fork shares state with its parent.",
        ("capabilities", "state:isolated") => "The fork keeps its own state.",
        ("source", "local_declared") => "Declared in this project's `.conspectus.toml`.",
        ("source", "local_pin") => "A pin from this project's `.conspectus.toml`.",
        ("source", "global_declared") => "Declared in your user config.",
        ("source", "global_pin") => "A pin from your user config.",
        ("source", "strong_discovered") => "Discovered from direct evidence.",
        ("source", "discovered") => "Discovered from indirect evidence.",
        ("source", "convention") => "Inferred from a naming or layout convention.",
        ("source", "cached") => "Carried over from an earlier scan.",
        ("role", "human_agent") => "The agent harness a person is driving.",
        ("role", "subagent") => "An agent the harness started on its own.",
        ("role", "background") => "A helper process the harness started.",
        ("role", "shell") => "A shell in the pane.",
        ("role", "unknown") => "A process Conspectus couldn't classify.",
        ("session_kind", "human") => "A session a person started.",
        ("session_kind", "subagent") => "A session an agent started.",
        ("depth", "0") => "The pane's root process.",
        ("depth", depth) => {
            return depth
                .parse::<u32>()
                .ok()
                .map(|n| format!("{n} levels below the pane's root process."));
        }
        _ => return None,
    };
    Some(entry.to_string())
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
