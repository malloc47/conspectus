//! Taking over the terminal and handing it back.
//!
//! `ratatui::init` enters raw mode and the alternate screen and installs
//! a panic hook that undoes both; `ratatui::restore` undoes them. On top
//! of that the TUI turns on bracketed paste, so a terminal delivers a
//! paste as one `Event::Paste` instead of keystrokes, where a line break
//! in the pasted text would arrive as Enter and submit whatever field it
//! landed in. Every take-over goes through [`enter`] and every hand-back
//! (exit, and each subprocess that gets the terminal: attach, the
//! external viewer) through [`leave`], so the paste mode never outlives
//! the TUI and is back on when the TUI returns.

use std::io::Write;
use std::sync::Once;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use ratatui::crossterm::execute;

/// Raw mode, the alternate screen, and bracketed paste.
pub(super) fn enter() -> DefaultTerminal {
    let terminal = ratatui::init();
    let _ = set_bracketed_paste(&mut std::io::stdout(), true);
    disable_paste_on_panic();
    terminal
}

/// Bracketed paste off, then the screen and mode `enter` changed.
pub(super) fn leave() {
    let _ = set_bracketed_paste(&mut std::io::stdout(), false);
    ratatui::restore();
}

/// Write the sequence that turns bracketed paste on or off.
fn set_bracketed_paste(out: &mut impl Write, on: bool) -> std::io::Result<()> {
    if on {
        execute!(out, EnableBracketedPaste)
    } else {
        execute!(out, DisableBracketedPaste)
    }
}

/// The panic hook `ratatui::init` installs restores the screen and raw
/// mode but not the paste mode, which would leave the shell receiving
/// paste markers. One extra hook, installed once, turns it off too.
fn disable_paste_on_panic() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = set_bracketed_paste(&mut std::io::stdout(), false);
            previous(info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::set_bracketed_paste;

    #[test]
    fn bracketed_paste_is_turned_on_and_back_off() {
        let mut on = Vec::new();
        set_bracketed_paste(&mut on, true).expect("write");
        assert_eq!(on, b"\x1b[?2004h");
        let mut off = Vec::new();
        set_bracketed_paste(&mut off, false).expect("write");
        assert_eq!(off, b"\x1b[?2004l");
    }
}
