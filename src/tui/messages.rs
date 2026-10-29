//! In-memory log of operation outcomes (ADR 0105).
//!
//! Launches, attaches, renames and other operations report here
//! instead of only flashing a status message. The log lives for the
//! life of the TUI process and is never written to disk: pane output
//! and harness stderr can carry conversation content (ADR 0086).
//!
//! The log itself (bounded, repeat-collapsing, counting unseen
//! failures) is the framework's `tuikit::feedback::Log`. What stays
//! here is Conspectus's payload for each entry, a [`Report`]: which
//! pin or mux the outcome concerns, the subprocess that ran, and any
//! detail text, plus the [`LogEntryExt`] builders and the plain-text
//! rendering the Messages overlay and the clipboard use. The repeat count
//! after a summary (`Entry::summary_with_repeats`) and the overlay's rows
//! are the framework's too (`tuikit::log_list`).

use std::process::Output;

use tuikit::feedback::{Entry, Log, Severity};

use crate::model::MuxSessionId;

/// Entries kept before the oldest is dropped.
pub const MESSAGE_LOG_CAPACITY: usize = 200;

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

/// Conspectus's part of a log entry: what it concerns and what ran.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    pub target: Option<LogTarget>,
    pub command: Option<CommandRecord>,
    /// Free text beyond the command record, such as a dead pane's
    /// output.
    pub detail: Option<String>,
}

/// One logged outcome: the framework's entry over a [`Report`].
pub type LogEntry = Entry<Report>;

/// The Messages log.
pub type MessageLog = Log<Report>;

/// A new, empty Messages log.
pub fn message_log() -> MessageLog {
    Log::new(MESSAGE_LOG_CAPACITY)
}

/// Builders and renderings for Conspectus's log entries. An entry is
/// dated by the wall clock when it is built.
pub trait LogEntryExt: Sized {
    fn of(severity: Severity, summary: impl Into<String>) -> Self;

    /// Something worth noting that is neither a success nor a failure.
    fn info(summary: impl Into<String>) -> Self {
        Self::of(Severity::Info, summary)
    }

    /// An operation that worked.
    fn success(summary: impl Into<String>) -> Self {
        Self::of(Severity::Success, summary)
    }

    /// An operation that fell back or was refused.
    fn warning(summary: impl Into<String>) -> Self {
        Self::of(Severity::Warning, summary)
    }

    /// An operation that failed.
    fn error(summary: impl Into<String>) -> Self {
        Self::of(Severity::Error, summary)
    }

    fn with_target(self, target: LogTarget) -> Self;
    fn with_command(self, command: CommandRecord) -> Self;
    fn with_detail(self, detail: impl Into<String>) -> Self;

    /// Full plain-text rendering for the overlay's detail pane and
    /// the clipboard.
    fn full_text(&self) -> String;

    /// The last `max_lines` lines of the most useful output: the
    /// detail text, else stderr, else stdout.
    fn output_tail(&self, max_lines: usize) -> Vec<String>;
}

impl LogEntryExt for LogEntry {
    fn of(severity: Severity, summary: impl Into<String>) -> Self {
        Entry::new(
            crate::discovery::current_epoch(),
            severity,
            summary,
            Report::default(),
        )
    }

    fn with_target(mut self, target: LogTarget) -> Self {
        self.item.target = Some(target);
        self
    }

    fn with_command(mut self, command: CommandRecord) -> Self {
        self.item.command = Some(command);
        self
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        if !detail.trim().is_empty() {
            self.item.detail = Some(detail);
        }
        self
    }

    fn full_text(&self) -> String {
        let mut out = format!(
            "{} {} {}\n",
            clock(self.at),
            self.severity.mark(),
            self.summary_with_repeats()
        );
        let report = &self.item;
        match &report.target {
            Some(LogTarget::Pin(id)) => out.push_str(&format!("target: pin {id}\n")),
            Some(LogTarget::Mux(mux)) => out.push_str(&format!("target: {}\n", mux.native_id)),
            None => {}
        }
        if let Some(command) = &report.command {
            out.push_str(&format!("argv: {}\n", command.argv.join(" ")));
            match command.exit_code {
                Some(code) => out.push_str(&format!("exit: {code}\n")),
                None => out.push_str("exit: none (signal or not started)\n"),
            }
            push_stream(&mut out, "stderr", &command.stderr);
            push_stream(&mut out, "stdout", &command.stdout);
        }
        if let Some(detail) = &report.detail {
            out.push('\n');
            out.push_str(detail.trim_end());
            out.push('\n');
        }
        out
    }

    fn output_tail(&self, max_lines: usize) -> Vec<String> {
        let report = &self.item;
        let source = report
            .detail
            .as_deref()
            .or_else(|| {
                report
                    .command
                    .as_ref()
                    .map(|c| c.stderr.as_str())
                    .filter(|s| !s.trim().is_empty())
            })
            .or_else(|| report.command.as_ref().map(|c| c.stdout.as_str()))
            .unwrap_or("");
        let lines: Vec<&str> = source.lines().filter(|l| !l.trim().is_empty()).collect();
        let start = lines.len().saturating_sub(max_lines);
        lines[start..].iter().map(|l| (*l).to_string()).collect()
    }
}

/// The newest entry about any of `targets`, when it is a warning or
/// error. A later success on the same target hides an older failure.
pub fn latest_failure_for<'a>(log: &'a MessageLog, targets: &[LogTarget]) -> Option<&'a LogEntry> {
    log.newest_where(|entry| {
        entry
            .item
            .target
            .as_ref()
            .is_some_and(|target| targets.contains(target))
    })
    .filter(|entry| entry.severity.needs_attention())
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

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
