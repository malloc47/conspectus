//! In-memory log of operation outcomes (ADR 0105).
//!
//! Launches, attaches, renames and other operations report here
//! instead of only flashing a status message. The log lives for the
//! life of the TUI process and is never written to disk: pane output
//! and harness stderr can carry conversation content (ADR 0086).

use std::collections::VecDeque;
use std::process::Output;

use crate::model::MuxSessionId;

/// Entries kept before the oldest is dropped.
pub const MESSAGE_LOG_CAPACITY: usize = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Info => "✓",
            Self::Warning => "⚠",
            Self::Error => "✗",
        }
    }

    /// Whether the operator should be told about the entry until
    /// they look at it.
    pub fn needs_attention(self) -> bool {
        self >= Self::Warning
    }
}

/// What an entry is about, so a row can show its own failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LogTarget {
    Pin(String),
    Mux(MuxSessionId),
}

/// A subprocess Conspectus ran on the operator's behalf.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandRecord {
    pub argv: Vec<String>,
    /// Exit code; `None` when the process was killed by a signal or
    /// never started.
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandRecord {
    pub fn from_output(argv: Vec<String>, output: &Output) -> Self {
        Self {
            argv,
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogEntry {
    pub at_epoch: i64,
    pub level: LogLevel,
    /// One line, shown in the status bar and the overlay list.
    pub summary: String,
    pub target: Option<LogTarget>,
    pub command: Option<CommandRecord>,
    /// Free text beyond the command record, such as a dead pane's
    /// output.
    pub detail: Option<String>,
    /// How many times this entry was logged back to back; see
    /// [`MessageLog::push`].
    pub repeats: u32,
}

impl LogEntry {
    pub fn new(level: LogLevel, summary: impl Into<String>) -> Self {
        Self {
            at_epoch: crate::discovery::current_epoch(),
            level,
            summary: summary.into(),
            target: None,
            command: None,
            detail: None,
            repeats: 1,
        }
    }

    /// The summary, with a `(×N)` suffix once it repeated.
    pub fn summary_with_repeats(&self) -> String {
        if self.repeats > 1 {
            format!("{} (×{})", self.summary, self.repeats)
        } else {
            self.summary.clone()
        }
    }

    pub fn info(summary: impl Into<String>) -> Self {
        Self::new(LogLevel::Info, summary)
    }

    pub fn warning(summary: impl Into<String>) -> Self {
        Self::new(LogLevel::Warning, summary)
    }

    pub fn error(summary: impl Into<String>) -> Self {
        Self::new(LogLevel::Error, summary)
    }

    pub fn with_target(mut self, target: LogTarget) -> Self {
        self.target = Some(target);
        self
    }

    pub fn with_command(mut self, command: CommandRecord) -> Self {
        self.command = Some(command);
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        if !detail.trim().is_empty() {
            self.detail = Some(detail);
        }
        self
    }

    /// Full plain-text rendering for the overlay's detail pane and
    /// the clipboard.
    pub fn full_text(&self) -> String {
        let mut out = format!(
            "{} {} {}\n",
            clock(self.at_epoch),
            self.level.glyph(),
            self.summary_with_repeats()
        );
        match &self.target {
            Some(LogTarget::Pin(id)) => out.push_str(&format!("target: pin {id}\n")),
            Some(LogTarget::Mux(mux)) => out.push_str(&format!("target: {}\n", mux.native_id)),
            None => {}
        }
        if let Some(command) = &self.command {
            out.push_str(&format!("argv: {}\n", command.argv.join(" ")));
            match command.exit_code {
                Some(code) => out.push_str(&format!("exit: {code}\n")),
                None => out.push_str("exit: none (signal or not started)\n"),
            }
            push_stream(&mut out, "stderr", &command.stderr);
            push_stream(&mut out, "stdout", &command.stdout);
        }
        if let Some(detail) = &self.detail {
            out.push('\n');
            out.push_str(detail.trim_end());
            out.push('\n');
        }
        out
    }

    /// The last `max_lines` lines of the most useful output: the
    /// detail text, else stderr, else stdout.
    pub fn output_tail(&self, max_lines: usize) -> Vec<String> {
        let source = self
            .detail
            .as_deref()
            .or_else(|| {
                self.command
                    .as_ref()
                    .map(|c| c.stderr.as_str())
                    .filter(|s| !s.trim().is_empty())
            })
            .or_else(|| self.command.as_ref().map(|c| c.stdout.as_str()))
            .unwrap_or("");
        let lines: Vec<&str> = source.lines().filter(|l| !l.trim().is_empty()).collect();
        let start = lines.len().saturating_sub(max_lines);
        lines[start..].iter().map(|l| (*l).to_string()).collect()
    }
}

fn push_stream(out: &mut String, label: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    out.push_str(&format!("--- {label} ---\n"));
    out.push_str(text.trim_end());
    out.push('\n');
}

/// `HH:MM:SS UTC`. Relative ages (`2m ago`) are what the overlay
/// list shows; the clock in the detail pane pins the moment down.
pub fn clock(epoch: i64) -> String {
    let secs = epoch.rem_euclid(86_400);
    format!(
        "{:02}:{:02}:{:02} UTC",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

/// Bounded, newest-last log plus a count of attention-worthy entries
/// the operator hasn't opened the overlay for yet.
#[derive(Clone, Debug, Default)]
pub struct MessageLog {
    entries: VecDeque<LogEntry>,
    unseen: usize,
}

impl MessageLog {
    /// Append `entry`. An entry identical to the newest one apart
    /// from its time (a refresh that keeps failing the same way) bumps
    /// that entry's repeat count instead of flooding the log, and
    /// doesn't count as a new unseen failure.
    pub fn push(&mut self, entry: LogEntry) {
        if let Some(last) = self.entries.back_mut()
            && last.level == entry.level
            && last.summary == entry.summary
            && last.target == entry.target
            && last.command == entry.command
            && last.detail == entry.detail
        {
            last.at_epoch = entry.at_epoch;
            last.repeats = last.repeats.saturating_add(1);
            return;
        }
        if entry.level.needs_attention() {
            self.unseen += 1;
        }
        if self.entries.len() == MESSAGE_LOG_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries newest first.
    pub fn newest_first(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().rev()
    }

    /// Warnings and errors logged since the overlay was last opened.
    pub fn unseen(&self) -> usize {
        self.unseen
    }

    pub fn mark_seen(&mut self) {
        self.unseen = 0;
    }

    /// The newest entry about any of `targets`, when it is a warning
    /// or error. A later success on the same target hides an older
    /// failure.
    pub fn latest_failure_for(&self, targets: &[LogTarget]) -> Option<&LogEntry> {
        self.newest_first()
            .find(|entry| {
                entry
                    .target
                    .as_ref()
                    .is_some_and(|target| targets.contains(target))
            })
            .filter(|entry| entry.level.needs_attention())
    }
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
