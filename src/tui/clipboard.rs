//! Clipboard write path for the TUI (ADR 0056).
//!
//! Emits an OSC 52 escape sequence on stdout so the host terminal (or
//! tmux, when `set-clipboard on` is configured) writes the payload to
//! the system clipboard. The sequence and its encoder are the
//! framework's (`tuikit::feedback::clipboard`), which writes to the
//! sink it is given; this adapter gives it the terminal.
//!
//! OSC 52 is fire-and-forget. Terminals that don't implement it drop
//! the sequence silently. Callers should still surface a "copied"
//! toast — this matches the behavior of every other tool that writes
//! the clipboard via terminal escapes (vim's `+y`, tmux copy-mode,
//! etc.).

use std::io;

/// Write `text` to the system clipboard via an OSC 52 sequence on
/// stdout. Returns the underlying I/O error if the write fails (rare
/// while the TUI owns the terminal).
pub fn copy_to_clipboard(text: &str) -> io::Result<()> {
    tuikit::feedback::clipboard::copy(&mut io::stdout().lock(), text)
}
