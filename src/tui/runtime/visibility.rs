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
//!
//! The same poll tells which session the client came back from, for
//! the turns it watched there (ADR 0231).

use std::sync::mpsc;
use std::time::Duration;

use crate::tui::rows::seen::ClientReturn;

/// How often the pane's visibility is checked.
const POLL: Duration = Duration::from_millis(250);

/// What the watcher noticed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Signal {
    /// The TUI's pane went from hidden to visible.
    Regained,
    /// The operator's client came back to the TUI's session from
    /// another one, which the memory of seen turns reads (ADR 0231).
    Returned(ClientReturn),
}

/// A background thread that sends a [`Signal`] each time the TUI's pane
/// goes from hidden to visible, and each time the client returns to the
/// TUI's session. Dropping the receiver ends the thread.
pub(super) struct VisibilityWatcher {
    pub(super) rx: mpsc::Receiver<Signal>,
}

impl VisibilityWatcher {
    /// Start watching when the TUI runs inside tmux; `None` anywhere
    /// else, where the terminal is always in view.
    pub(super) fn start() -> Option<Self> {
        let tmux_env = std::env::var("TMUX")
            .ok()
            .filter(|value| !value.is_empty())?;
        let pane = std::env::var("TMUX_PANE")
            .ok()
            .filter(|pane| !pane.is_empty())?;
        let socket = server_socket(&tmux_env);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut watch = Watch::new(socket);
            loop {
                std::thread::sleep(POLL);
                // Taken before the query: the poll sees the client no
                // earlier than this.
                let at_ms = crate::tui::rows::tree_current_unix_ms();
                for signal in watch.step(query(&pane), at_ms) {
                    if tx.send(signal).is_err() {
                        return;
                    }
                }
            }
        });
        Some(Self { rx })
    }
}

/// The socket of the tmux server the TUI runs under: the first field of
/// `$TMUX` (`<socket path>,<server pid>,<session index>`).
fn server_socket(tmux_env: &str) -> Option<String> {
    let socket = tmux_env.split(',').next()?;
    (!socket.is_empty()).then(|| socket.to_string())
}

/// What one poll of tmux said about the TUI's pane.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    /// Clients attached to the pane's session.
    attached: u32,
    window_active: bool,
    pane_active: bool,
    /// The session the client was on before the one it is on now, as
    /// tmux expands it for the pane's session (`#{client_last_session}`);
    /// `None` when the client has not switched.
    last_session: Option<String>,
}

impl Observation {
    /// Some client is attached to the pane's session, and its window
    /// and pane are the active ones.
    fn visible(&self) -> bool {
        self.attached > 0 && self.window_active && self.pane_active
    }
}

/// Ask tmux about the pane: whether it is visible, and which session the
/// client came from. `None` when tmux can't say.
fn query(pane: &str) -> Option<Observation> {
    let output = crate::exec::command(crate::exec::Program::Tmux, "tmux")
        .args([
            "display-message",
            "-p",
            "-t",
            pane,
            "#{session_attached}:#{window_active}:#{pane_active}:#{client_last_session}",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_observation(&String::from_utf8_lossy(&output.stdout))
}

/// `attached:window_active:pane_active:last_session` as tmux prints it.
/// tmux never puts a colon in a session name, and a name is the rest of
/// the line anyway.
fn parse_observation(text: &str) -> Option<Observation> {
    let mut parts = text.trim().splitn(4, ':');
    let attached: u32 = parts.next()?.parse().ok()?;
    let window_active = parts.next()? == "1";
    let pane_active = parts.next()? == "1";
    let last_session = parts
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    Some(Observation {
        attached,
        window_active,
        pane_active,
        last_session,
    })
}

/// What the watcher remembers between polls.
#[derive(Debug)]
struct Watch {
    /// The socket of the server the TUI runs under, which names the
    /// session a return came from; `None` when `$TMUX` didn't say.
    socket: Option<String>,
    /// Whether the pane was in view at the last poll that could say.
    shown: bool,
    /// When the last poll that found no client on the TUI's session ran,
    /// until the client is next found back on it.
    away_ms: Option<i64>,
}

impl Watch {
    fn new(socket: Option<String>) -> Self {
        Self {
            socket,
            shown: true,
            away_ms: None,
        }
    }

    /// Fold one poll, which began at `at_ms` (Unix epoch milliseconds),
    /// into the state, and say what it noticed. A poll that says nothing
    /// leaves the state alone.
    ///
    /// The client's last session is read at the poll that finds a client
    /// on the TUI's session again after finding none, where it is the
    /// session the client came from. Read at a window change inside the
    /// session it would be stale: whichever session the client left
    /// before it came to this one, however long ago. The return is dated
    /// by the last poll that found the client away, so a turn that ended
    /// between that poll and the client's return is never credited.
    fn step(&mut self, observed: Option<Observation>, at_ms: i64) -> Vec<Signal> {
        let Some(observed) = observed else {
            return Vec::new();
        };
        let mut signals = Vec::new();
        if observed.attached == 0 {
            self.away_ms = Some(at_ms);
        } else if let Some(away_ms) = self.away_ms.take()
            && let (Some(socket), Some(session)) = (&self.socket, observed.last_session.clone())
        {
            signals.push(Signal::Returned(ClientReturn {
                socket: socket.clone(),
                session,
                at_ms: away_ms,
            }));
        }
        let visible = observed.visible();
        if visible && !self.shown {
            signals.push(Signal::Regained);
        }
        self.shown = visible;
        signals
    }
}

#[cfg(test)]
#[path = "visibility_tests.rs"]
mod tests;
