//! Noticing when the TUI's own tmux pane comes back into view.
//!
//! Inside tmux, attaching to a mux is `switch-client`: the call returns
//! at once, the TUI keeps running in a window nobody is looking at, and
//! the operator comes back later by switching the client back. The
//! refresh that follows the attach therefore ran while the mux was
//! still alive, and nothing refreshed when the operator returned, so a
//! mux they had exited stayed on screen as running until the next timer
//! tick or a manual refresh. The watcher reports the moment the pane
//! becomes visible again so the runtime can refresh then.

use std::sync::mpsc;
use std::time::Duration;

/// How often the pane's visibility is checked.
const POLL: Duration = Duration::from_millis(250);

/// A background thread that sends `()` each time the TUI's pane goes
/// from hidden to visible. Dropping the receiver ends the thread.
pub(super) struct VisibilityWatcher {
    pub(super) rx: mpsc::Receiver<()>,
}

impl VisibilityWatcher {
    /// Start watching when the TUI runs inside tmux; `None` anywhere
    /// else, where the terminal is always in view.
    pub(super) fn start() -> Option<Self> {
        let in_tmux = std::env::var_os("TMUX").is_some_and(|value| !value.is_empty());
        let pane = std::env::var("TMUX_PANE")
            .ok()
            .filter(|pane| !pane.is_empty())?;
        if !in_tmux {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut shown = true;
            loop {
                std::thread::sleep(POLL);
                let (next, fired) = step(shown, query(&pane));
                shown = next;
                if fired && tx.send(()).is_err() {
                    return;
                }
            }
        });
        Some(Self { rx })
    }
}

/// Ask tmux whether the pane is visible: some client is attached to
/// its session, and its window and pane are the active ones. `None`
/// when tmux can't say.
fn query(pane: &str) -> Option<bool> {
    let output = crate::exec::command(crate::exec::Program::Tmux, "tmux")
        .args([
            "display-message",
            "-p",
            "-t",
            pane,
            "#{session_attached}:#{window_active}:#{pane_active}",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_visibility(&String::from_utf8_lossy(&output.stdout))
}

/// `attached:window_active:pane_active` as tmux prints it.
fn parse_visibility(text: &str) -> Option<bool> {
    let mut parts = text.trim().split(':');
    let attached: u32 = parts.next()?.parse().ok()?;
    let window = parts.next()? == "1";
    let pane = parts.next()? == "1";
    Some(attached > 0 && window && pane)
}

/// Fold one observation into the watcher's state: the new state, and
/// whether the pane just came back into view. An observation that says
/// nothing leaves the state alone.
fn step(shown: bool, observed: Option<bool>) -> (bool, bool) {
    match observed {
        Some(now) => (now, now && !shown),
        None => (shown, false),
    }
}

#[cfg(test)]
#[path = "visibility_tests.rs"]
mod tests;
