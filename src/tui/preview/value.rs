//! Previews computed from a field's value alone (ADR 0112 §2): full
//! text, lists, command lines one argument per line, timestamps in
//! local time and UTC with their age, and glossary entries for values
//! from a fixed set. Nothing here reads the system beyond the clock and
//! time zone.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::explorer::FieldValue;

/// The lines a value previews, or `None` for a value whose preview is
/// loaded or names a node.
pub fn value_lines(value: &FieldValue, now: Option<i64>) -> Option<Vec<Line<'static>>> {
    match value {
        FieldValue::Text(text) => Some(text.lines().map(|l| Line::raw(l.to_string())).collect()),
        FieldValue::List(items) => Some(items.iter().map(|i| Line::raw(i.clone())).collect()),
        FieldValue::Command(argv) => Some(command_lines(argv)),
        FieldValue::Timestamp(epoch) => Some(timestamp_lines(*epoch, now)),
        FieldValue::Glossary { field, values } => Some(glossary_lines(field, values)),
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

fn dim(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::new().add_modifier(Modifier::DIM))
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

/// Split a command line into arguments the way a POSIX shell would for
/// plain words, single and double quotes, and backslash escapes. No
/// expansion happens; the preview only shows where arguments break.
pub fn split_command_line(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' => {
                in_word = true;
                for quoted in chars.by_ref() {
                    if quoted == '\'' {
                        break;
                    }
                    current.push(quoted);
                }
            }
            '"' => {
                in_word = true;
                while let Some(quoted) = chars.next() {
                    match quoted {
                        '"' => break,
                        '\\' => {
                            if let Some(escaped) = chars.next() {
                                current.push(escaped);
                            }
                        }
                        other => current.push(other),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                }
            }
            c if c.is_whitespace() => {
                if in_word {
                    args.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            other => {
                in_word = true;
                current.push(other);
            }
        }
    }
    if in_word {
        args.push(current);
    }
    args
}

fn timestamp_lines(epoch: i64, now: Option<i64>) -> Vec<Line<'static>> {
    let Some(utc) = chrono::DateTime::from_timestamp(epoch, 0) else {
        return vec![Line::raw(format!("{epoch} (out of range)"))];
    };
    let mut lines = Vec::new();
    if let Some((offset, zone)) = local_offset(epoch)
        && let Some(offset) = chrono::FixedOffset::east_opt(offset)
    {
        let local = utc.with_timezone(&offset);
        lines.push(Line::from(vec![
            dim("local  "),
            Span::raw(format!("{} {zone}", local.format("%Y-%m-%d %H:%M:%S %:z"))),
        ]));
    }
    lines.push(Line::from(vec![
        dim("UTC    "),
        Span::raw(utc.format("%Y-%m-%d %H:%M:%S").to_string()),
    ]));
    if let Some(age) = crate::tui::rows::format_recency(now, Some(epoch)) {
        lines.push(Line::from(vec![
            dim("age    "),
            Span::raw(format!("{age} ago")),
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

fn glossary_lines(field: &str, values: &[String]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for value in values {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(bold(value.clone())));
        lines.push(match glossary_entry(field, value) {
            Some(entry) => Line::raw(entry),
            None => Line::from(dim(format!("no glossary entry for `{value}`"))),
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
mod tests {
    use super::*;

    fn rows(value: &FieldValue, now: Option<i64>) -> Vec<String> {
        value_lines(value, now)
            .expect("lines")
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn command_lines_split_like_a_shell() {
        assert_eq!(
            split_command_line(r#"claude --resume abc -p 'two words' "a \"q\"" x\ y"#),
            [
                "claude",
                "--resume",
                "abc",
                "-p",
                "two words",
                r#"a "q""#,
                "x y"
            ]
        );
        assert_eq!(
            rows(
                &FieldValue::Command(vec!["claude".into(), "--resume".into(), "abc".into()]),
                None
            ),
            ["claude", "  --resume", "  abc"]
        );
    }

    #[test]
    fn a_timestamp_shows_utc_and_its_age() {
        let lines = rows(&FieldValue::Timestamp(1_700_000_000), Some(1_700_003_600));
        assert!(
            lines.contains(&"UTC    2023-11-14 22:13:20".to_string()),
            "{lines:?}"
        );
        assert_eq!(lines.last().map(String::as_str), Some("age    1h ago"));
        #[cfg(unix)]
        assert!(lines[0].starts_with("local  2023-11-1"), "{lines:?}");
    }

    #[test]
    fn a_fixed_set_value_shows_its_entry_or_says_it_has_none() {
        assert_eq!(
            rows(
                &FieldValue::Glossary {
                    field: "backend",
                    values: vec!["tmux".into()],
                },
                None
            ),
            ["tmux", "A tmux session on the default socket."]
        );
        assert_eq!(
            rows(
                &FieldValue::Glossary {
                    field: "backend",
                    values: vec!["screen".into()],
                },
                None
            ),
            ["screen", "no glossary entry for `screen`"]
        );
        assert_eq!(
            glossary_entry("depth", "2").as_deref(),
            Some("2 levels below the pane's root process.")
        );
    }

    #[test]
    fn text_and_lists_show_in_full() {
        let long = "x".repeat(300);
        assert_eq!(rows(&FieldValue::Text(long.clone()), None), [long]);
        assert_eq!(
            rows(&FieldValue::List(vec!["a".into(), "b".into()]), None),
            ["a", "b"]
        );
    }
}
