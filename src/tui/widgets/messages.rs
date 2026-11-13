//! `!` Messages overlay (ADR 0105): the framework's log list
//! (`tuikit::log_list`) over the App's log of operation outcomes, newest
//! first, with the selected entry's full record (argv, exit status,
//! stderr, stdout) under the list.
//!
//! The log list owns the reading: the rows (the severity's mark, the age,
//! the summary and its repeat count), the selection that stops at the
//! newest and the oldest entry, the detail's scrolling and paging by the
//! rows it shows, the scrollbars, the footer, and the close keys. What is
//! Conspectus's is here: the title, the record each entry's detail reads
//! as (`full_text`), what the empty log says, and `y`, whose copy hook asks
//! the runtime to copy the record; the runtime does the clipboard write
//! and the toast. Keys are `MESSAGES_KEYS`: the standard log list table,
//! `y` to copy, and `!` closing as it opened.

use tuikit::log_list::LogList;

use crate::tui::keybindings::overlay::MESSAGES_KEYS;
use crate::tui::messages::{LogEntry, LogEntryExt, MessageLog, Report};
use crate::tui::{App, Msg};

/// The Messages overlay: the framework's log list over the App's log.
pub type MessagesList = LogList<Report, Msg, App>;

/// The overlay over the App's message log, aged against the clock the rows
/// use (which snapshot mode freezes).
pub fn open() -> MessagesList {
    over(App::messages, |_app: &App| {
        crate::tui::ui::current_unix_epoch_for_render()
    })
}

/// The Messages log list over the log `source` finds in a context, aged
/// against `clock`: the overlay's keys, record, copy, and empty note.
pub fn over<Cx: ?Sized>(
    source: impl Fn(&Cx) -> &MessageLog + Send + Sync + 'static,
    clock: impl Fn(&Cx) -> i64 + Send + Sync + 'static,
) -> LogList<Report, Msg, Cx> {
    LogList::new("Messages", source, clock)
        .keys(MESSAGES_KEYS)
        .detail(|entry: &LogEntry| entry.full_text())
        // The clipboard write is the executor's.
        .copy(|entry: &LogEntry| {
            Some(Msg::Copy {
                text: entry.full_text(),
                done: "copied message".to_string(),
            })
        })
        .empty(
            "No messages yet. Launches, attaches, renames and\n\
             failures from this session are listed here.",
        )
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
