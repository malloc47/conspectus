//! Every modal's [`Overlay<Msg, App>`](Overlay) impl: how it takes a key
//! and draws itself with the App as its context.
//!
//! A widget keeps its own key handling (`handle_key`) and its own widget
//! for drawing; the impl here hands each the App data it reads. A scrolled
//! entry keeps the window it drew, so its next page key moves by the rows
//! that were on screen. Every entry draws through the shared modal frame,
//! which clears what it covers, or fills the screen.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tuikit::keys::resolve;
use tuikit::overlay::{Outcome, Overlay};
use tuikit::theme::Theme as KitTheme;

use super::RenameOverlayState;
use crate::tui::widgets::{
    clean_up::{CleanUpState, CleanUpWidget},
    controls::{ControlsOutcome, ControlsOverlayState, ControlsOverlayWidget},
    help::{HelpOutcome, HelpOverlayState, HelpOverlayWidget},
    input::TextInputWidget,
    messages::{MessagesOverlayState, MessagesWidget},
    mux_launch::{MuxLaunchFormState, MuxLaunchFormWidget},
    mux_menu::{MuxMenuState, MuxMenuWidget},
    new_mux::{NewMuxFormState, NewMuxFormWidget},
    pane_viewer::{PaneViewerOutcome, PaneViewerState},
    picker::{PickerState, PickerWidget},
    pin_edit::{PinEditFormState, PinEditFormWidget},
    row_menu::{RowMenuState, RowMenuWidget},
    search::SearchOverlayState,
    value_modal::{ValueModalOutcome, ValueModalState, ValueModalWidget},
    worktree_menu::{WorktreeMenuState, WorktreeMenuWidget},
};
use crate::tui::{App, Msg};

impl Overlay<Msg, App> for HelpOverlayState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        match self.handle_key(key) {
            HelpOutcome::Continue => Outcome::Consumed,
            HelpOutcome::Close => Outcome::Close,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        let window = HelpOverlayWidget::new(self, app.theme()).draw(area, buf);
        self.fit(window);
    }
}

impl Overlay<Msg, App> for ControlsOverlayState {
    fn handle(&mut self, key: KeyEvent, app: &App) -> Outcome<Msg> {
        match self.handle_key(&app.controls_context(), key) {
            ControlsOutcome::Continue => Outcome::Consumed,
            ControlsOutcome::Close => Outcome::Close,
            ControlsOutcome::ApplyAndStay(msg) => Outcome::Stay(msg),
            ControlsOutcome::ApplyAndClose(msg) => Outcome::Commit(msg),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        ControlsOverlayWidget::new(self, app.controls_context(), app.theme()).render(area, buf);
    }

    fn takes_text(&self) -> bool {
        self.editor_takes_text()
    }
}

impl Overlay<Msg, App> for RenameOverlayState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        TextInputWidget::new(self.inner(), app.theme()).render(area, buf);
    }

    fn takes_text(&self) -> bool {
        true
    }
}

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
        // Widening for the first time needs every entity indexed.
        let entities = widens
            .then(|| app.snapshot_handle())
            .flatten()
            .map(|handle| EntityIndex::build(handle.snapshot(), app.config().live_preview_enabled));
        let cx = SearchContext {
            items: &items,
            content: &content,
            entities,
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
        let outcome = |node: &crate::model::NodeId| app.open_plan(reach.as_ref(), node).word();
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
}

impl Overlay<Msg, App> for ValueModalState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        use crate::tui::keybindings::overlay::VALUE_MODAL_KEYS;
        use tuikit::keys::PagerKey;
        // `y` copies; the clipboard write is the executor's.
        if resolve(VALUE_MODAL_KEYS, key) == Some(PagerKey::Copy) {
            let (text, what) = self.copy_payload();
            return Outcome::Stay(Msg::Copy {
                text: text.to_string(),
                done: format!("Copied {what}"),
            });
        }
        match self.handle_key(key) {
            ValueModalOutcome::Continue => Outcome::Consumed,
            ValueModalOutcome::Close => Outcome::Close,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        let window = ValueModalWidget::new(self, app.theme()).draw(area, buf);
        self.fit(window);
    }
}

impl Overlay<Msg, App> for MessagesOverlayState {
    fn handle(&mut self, key: KeyEvent, app: &App) -> Outcome<Msg> {
        self.handle_key(app.messages(), key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        // Ages read against the clock the rows use, which snapshot mode
        // freezes.
        let now = crate::tui::ui::current_unix_epoch_for_render();
        let window = MessagesWidget::new(self, app.messages(), app.theme(), now).draw(area, buf);
        self.fit(window);
    }
}

impl Overlay<Msg, App> for crate::viewer::state::ViewerState {
    /// A nested reducer: the key becomes a viewer message, which the App's
    /// reducer applies to this entry (closing it when the viewer says to
    /// leave). While the find prompt is open it owns the keys: bound keys
    /// edit or leave it, and anything else types.
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        use crate::tui::keybindings::overlay::{VIEWER_FIND_KEYS, VIEWER_KEYS};
        use tuikit::keys::typed_char;
        if self.find.prompt.is_some() {
            return resolve(VIEWER_FIND_KEYS, key)
                .or_else(|| typed_char(key).map(crate::viewer::find::ViewerFindMsg::Insert))
                .map_or(Outcome::Consumed, |msg| Outcome::Stay(Msg::ViewerFind(msg)));
        }
        resolve(VIEWER_KEYS, key).map_or(Outcome::Consumed, |msg| Outcome::Stay(Msg::Viewer(msg)))
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        crate::viewer::widget::draw(self, app.theme(), buf, area);
        if self.show_help {
            crate::tui::widgets::viewer_help::draw(self, app.theme(), buf, area);
        }
    }

    fn fills_screen(&self) -> bool {
        true
    }

    fn takes_text(&self) -> bool {
        self.find.prompt.is_some()
    }
}

impl Overlay<Msg, App> for PaneViewerState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        match self.handle_key(key) {
            PaneViewerOutcome::Continue => Outcome::Consumed,
            PaneViewerOutcome::Close => Outcome::Close,
            // Whether the pane's mux can be attached is the reducer's call.
            PaneViewerOutcome::Attach => Outcome::Stay(Msg::AttachPane),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        crate::tui::widgets::pane_viewer::draw(self, app.theme(), buf, area);
    }

    fn fills_screen(&self) -> bool {
        true
    }

    fn takes_text(&self) -> bool {
        self.prompt.is_some()
    }
}

impl Overlay<Msg, App> for WorktreeMenuState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        WorktreeMenuWidget::new(self, app.theme()).render(area, buf);
    }

    fn takes_text(&self) -> bool {
        self.editing_branch()
    }
}

impl Overlay<Msg, App> for NewMuxFormState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        NewMuxFormWidget::new(self, app.theme()).render(area, buf);
    }

    fn takes_text(&self) -> bool {
        true
    }
}

impl Overlay<Msg, App> for MuxMenuState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        MuxMenuWidget::new(self, app.theme()).render(area, buf);
    }
}

impl Overlay<Msg, App> for MuxLaunchFormState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        MuxLaunchFormWidget::new(self, app.theme()).render(area, buf);
    }

    fn takes_text(&self) -> bool {
        self.focus().takes_text()
    }
}

impl Overlay<Msg, App> for PinEditFormState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        PinEditFormWidget::new(self, app.theme()).render(area, buf);
    }

    fn takes_text(&self) -> bool {
        self.focused_label().is_some()
    }
}

impl Overlay<Msg, App> for RowMenuState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        RowMenuWidget::new(self, app.theme()).render(area, buf);
    }
}

impl Overlay<Msg, App> for CleanUpState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        CleanUpWidget::new(self, app.theme()).render(area, buf);
    }
}

impl Overlay<Msg, App> for PickerState {
    fn handle(&mut self, key: KeyEvent, _app: &App) -> Outcome<Msg> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, app: &App, _theme: &KitTheme) {
        PickerWidget::new(self, app.theme()).render(area, buf);
    }

    fn takes_text(&self) -> bool {
        true
    }
}
