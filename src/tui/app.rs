//! Pure app state.
//!
//! Per ADR 0024, [`update`] is synchronous and free of I/O: the
//! runtime translates terminal events (and, later, background-task
//! results and timer ticks) into [`Msg`]s and feeds them in.
//!
//! v1 shell (P8-003) only handles the `Quit` message. Downstream
//! stories extend `App` and `Msg` without changing the loop's
//! shape.

use crate::tui::RunConfig;

/// Top-level state. Owns the resolved run configuration plus the
/// per-frame UI state (selection, focus, expanded rows, etc., as
/// later stories add them).
#[derive(Debug)]
pub struct App {
    config: RunConfig,
    should_quit: bool,
}

/// Every event the reducer can process. Keep variants narrow and
/// add as stories land; do not make the enum a kitchen sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    /// Operator asked to exit (`q`, Ctrl-C, or a fatal-error
    /// translation in a later story).
    Quit,
}

impl App {
    /// Build a fresh app at the start of the run.
    pub fn new(config: RunConfig) -> Self {
        Self {
            config,
            should_quit: false,
        }
    }

    /// Read-only access to the immutable run config.
    pub fn config(&self) -> &RunConfig {
        &self.config
    }

    /// True once the runtime should leave the event loop.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Apply a single [`Msg`] to the state. Pure: no I/O, no panics,
    /// no clock reads. Test by constructing an `App` and asserting
    /// the post-state.
    pub fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Quit => self.should_quit = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_msg_sets_should_quit() {
        let mut app = App::new(RunConfig::defaults());
        assert!(!app.should_quit());
        app.update(Msg::Quit);
        assert!(app.should_quit());
    }

    #[test]
    fn config_is_preserved() {
        let mut cfg = RunConfig::defaults();
        cfg.live_preview_enabled = false;
        let app = App::new(cfg);
        assert!(!app.config().live_preview_enabled);
    }
}
