//! Every modal's [`Overlay<Msg, App>`](Overlay) impl: how it takes a key
//! and draws itself with the App as its context.
//!
//! A widget keeps its own key handling (`handle_key`) and its own widget
//! for drawing; the impl here hands each the App data it reads. A scrolled
//! entry keeps the window it drew, so its next page key moves by the rows
//! that were on screen. Every entry draws through the shared modal frame,
//! which clears what it covers, or fills the screen. The rename and step
//! prompts, the new mux form, the row, mux, and worktree menus, the
//! dialogs, Clean up, the Controls panel and its editors, the picker, the
//! keymap help, the value modal, the transcript and pane viewers, and
//! Messages have no impl here: they are the framework's text prompt, form,
//! action menu, dialog, checklist, settings panel, palette, pager, and log
//! list, which implement the trait for their context.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tuikit::overlay::{Outcome, Overlay};
use tuikit::theme::Theme as KitTheme;

use crate::tui::widgets::{
    mux_launch::MuxLaunchForm, pin_edit::PinEditForm, search::SearchOverlayState,
};
use crate::tui::{App, Msg};

impl Overlay<Msg, App> for SearchOverlayState {
    /// The overlay reranks against the App's rows and content results on
    /// every key. The contents it lists are for its own query: the App's
    /// stack is lent out while the key is handled.
    fn handle(&mut self, key: KeyEvent, app: &App) -> Outcome<Msg> {
        use crate::tui::search::items_from_rows;
        use crate::tui::search_entities::EntityIndex;
        use crate::tui::widgets::search::{Reach, SearchContext, SearchMode};
        let widens = self.widens_with(key);
        // Quick select ranks every row of the view, collapsed ones
        // included.
        let items = if self.mode() == SearchMode::Quick && self.reach() == Reach::Current || widens
        {
            items_from_rows(&app.tree().rows)
        } else {
            Vec::new()
        };
        let content = if self.mode() == SearchMode::Content {
            app.content_view_for(&self.content_text()).rows
        } else {
            Vec::new()
        };
        // Widening for the first time needs every entity indexed, and what
        // each view lists for their outcome words.
        let entities = widens
            .then(|| app.snapshot_handle())
            .flatten()
            .map(|handle| EntityIndex::build(handle.snapshot(), app.config().live_preview_enabled));
        let listing = entities.is_some().then(|| app.listing());
        let cx = SearchContext {
            items: &items,
            content: &content,
            entities,
            listing,
        };
        self.handle_in(cx, key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        use crate::tui::search::items_from_rows;
        use crate::tui::widgets::search::{Reach, SearchMode, SearchOverlayWidget};
        // The items come from the live row tree each frame, so a label
        // never lags behind a refresh; only the scope that lists rows pays
        // for building them.
        let items = if self.mode() == SearchMode::Quick && self.reach() == Reach::Current {
            items_from_rows(&app.tree().rows)
        } else {
            Vec::new()
        };
        let view = (self.mode() == SearchMode::Content)
            .then(|| app.content_view_for(&self.content_text()));
        // Outcome words for the wider reach: one walk out from the focus,
        // then a lookup per row the list draws.
        let reach = (self.reach() == Reach::All && self.mode() == SearchMode::Quick)
            .then(|| app.search_reachability())
            .flatten();
        let listing = self.listing();
        let outcome = |node: &crate::model::NodeId| {
            app.open_plan(reach.as_ref(), listing.as_deref(), node)
                .word()
        };
        let mut widget =
            SearchOverlayWidget::new(self, &items, app.theme()).with_outcomes(&outcome);
        if let Some(view) = &view {
            widget = widget.with_content(view);
        }
        widget.render(area, buf);
    }

    fn takes_text(&self) -> bool {
        self.options_cursor().is_none()
    }

    fn hints(&self) -> tuikit::keys::Footer {
        self.footer()
    }
}

impl Overlay<Msg, App> for MuxLaunchForm {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _app: &App, theme: &KitTheme) {
        self.draw(area, buf, theme);
    }

    fn takes_text(&self) -> bool {
        MuxLaunchForm::takes_text(self)
    }

    fn hints(&self) -> tuikit::keys::Footer {
        Overlay::<Msg, App>::hints(self.form())
    }
}

impl Overlay<Msg, App> for PinEditForm {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _app: &App, theme: &KitTheme) {
        self.draw(area, buf, theme);
    }

    fn takes_text(&self) -> bool {
        PinEditForm::takes_text(self)
    }

    fn hints(&self) -> tuikit::keys::Footer {
        Overlay::<Msg, App>::hints(self.form())
    }
}
