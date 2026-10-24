//! The Controls overlay's view of [`App`].

use super::*;

impl App {
    /// Snapshot of the live state the controls overlay renders
    /// against. Borrowed each frame so the overlay never lags
    /// behind the app.
    pub fn controls_context(&self) -> crate::tui::widgets::controls::ControlsContext<'_> {
        crate::tui::widgets::controls::ControlsContext {
            view: self.active_view,
            grouping: self.grouping,
            filter: &self.filter,
            sort: self.sort,
            mux_recency: self.mux_recency,
            preview_wrap: self.preview_wrap,
            session_preview: self.session_preview,
        }
    }
}
