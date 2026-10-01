//! Modal-stack accessors: open, close, and peek each overlay.

use super::*;

impl App {
    /// Active rename-overlay state, if any. Lives on the modal
    /// stack (ADR 0085 contract 3); this accessor peeks the top
    /// entry.
    pub fn rename_overlay(&self) -> Option<&crate::tui::widgets::input::TextInputState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Rename(state) => Some(state.inner()),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn rename_overlay_mut(&mut self) -> Option<&mut crate::tui::RenameOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Rename(state) => Some(state),
            _ => None,
        }
    }

    /// Push a rename overlay onto the modal stack. Caller
    /// pre-populates the input with the current alias, harness
    /// title, or empty string per ADR 0030. The raw text-input
    /// state is wrapped in a `RenameOverlayState` so the
    /// overlay's Confirm(String) maps to Msg::CommitRename via
    /// the uniform Overlay trait.
    pub fn open_rename_overlay(&mut self, state: crate::tui::widgets::input::TextInputState) {
        self.modal_stack.push(crate::tui::Modal::Rename(
            crate::tui::RenameOverlayState::new(state),
        ));
    }

    /// Pop the rename overlay if it's on top; no-op otherwise.
    pub fn close_rename_overlay(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Rename(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active worktree action menu, if on top of the stack.
    pub fn worktree_menu(&self) -> Option<&crate::tui::widgets::worktree_menu::WorktreeMenuState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::WorktreeMenu(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn worktree_menu_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::worktree_menu::WorktreeMenuState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::WorktreeMenu(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    /// Push a worktree action menu onto the modal stack.
    pub fn open_worktree_menu(
        &mut self,
        state: crate::tui::widgets::worktree_menu::WorktreeMenuState,
    ) {
        self.modal_stack
            .push(crate::tui::Modal::WorktreeMenu(Box::new(state)));
    }

    /// Pop the worktree menu if it's on top; no-op otherwise.
    pub fn close_worktree_menu(&mut self) {
        if matches!(
            self.modal_stack.last(),
            Some(crate::tui::Modal::WorktreeMenu(_))
        ) {
            self.modal_stack.pop();
        }
    }

    /// Active bare mux form, if on top of the stack.
    pub fn new_mux_form(&self) -> Option<&crate::tui::widgets::new_mux::NewMuxFormState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::NewMux(state) => Some(state),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn new_mux_form_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::new_mux::NewMuxFormState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::NewMux(state) => Some(state),
            _ => None,
        }
    }

    /// Push the bare mux form onto the modal stack.
    pub fn open_new_mux_form(&mut self, state: crate::tui::widgets::new_mux::NewMuxFormState) {
        self.modal_stack.push(crate::tui::Modal::NewMux(state));
    }

    /// Pop the bare mux form if it's on top; no-op otherwise.
    pub fn close_new_mux_form(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::NewMux(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active mux action menu, if on top of stack.
    pub fn mux_menu(&self) -> Option<&crate::tui::widgets::mux_menu::MuxMenuState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::MuxMenu(state) => Some(state),
            _ => None,
        }
    }

    pub fn mux_menu_mut(&mut self) -> Option<&mut crate::tui::widgets::mux_menu::MuxMenuState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::MuxMenu(state) => Some(state),
            _ => None,
        }
    }

    pub fn open_mux_menu(&mut self, state: crate::tui::widgets::mux_menu::MuxMenuState) {
        self.modal_stack.push(crate::tui::Modal::MuxMenu(state));
    }

    pub fn close_mux_menu(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::MuxMenu(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active mux-launch form, if on top of stack.
    pub fn mux_launch_form(&self) -> Option<&crate::tui::widgets::mux_launch::MuxLaunchFormState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::MuxLaunch(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    pub fn mux_launch_form_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::mux_launch::MuxLaunchFormState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::MuxLaunch(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    pub fn open_mux_launch_form(
        &mut self,
        state: crate::tui::widgets::mux_launch::MuxLaunchFormState,
    ) {
        self.modal_stack
            .push(crate::tui::Modal::MuxLaunch(Box::new(state)));
    }

    pub fn close_mux_launch_form(&mut self) {
        if matches!(
            self.modal_stack.last(),
            Some(crate::tui::Modal::MuxLaunch(_))
        ) {
            self.modal_stack.pop();
        }
    }

    pub fn pending_pin_remove(&self) -> Option<&str> {
        self.pending_pin_remove.as_deref()
    }

    pub fn set_pending_pin_remove(&mut self, pin_id: Option<String>) {
        self.pending_pin_remove = pin_id;
    }

    /// Active controls-overlay state (ADR 0031), if any.
    /// Lives on the modal stack (ADR 0085 contract 3); this
    /// accessor peeks the top entry.
    pub fn controls_overlay(&self) -> Option<&crate::tui::widgets::controls::ControlsOverlayState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Controls(state) => Some(state),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn controls_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::controls::ControlsOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Controls(state) => Some(state),
            _ => None,
        }
    }

    /// Push a fresh controls overlay onto the modal stack, cursor
    /// on the active view row.
    pub fn open_controls_overlay(&mut self) {
        let ctx = self.controls_context();
        self.modal_stack.push(crate::tui::Modal::Controls(
            crate::tui::widgets::controls::ControlsOverlayState::new(&ctx),
        ));
    }

    /// Pop the controls overlay if it's on top; no-op otherwise.
    pub fn close_controls_overlay(&mut self) {
        if matches!(
            self.modal_stack.last(),
            Some(crate::tui::Modal::Controls(_))
        ) {
            self.modal_stack.pop();
        }
    }

    /// Active pins-overlay state (ADR 0057), if any. Lives on the
    /// modal stack (ADR 0085 contract 3); this accessor peeks the
    /// top entry.
    pub fn pins_overlay(&self) -> Option<&crate::tui::widgets::pins::PinsOverlayState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Pins(state) => Some(state),
            _ => None,
        }
    }

    pub fn pins_overlay_mut(&mut self) -> Option<&mut crate::tui::widgets::pins::PinsOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Pins(state) => Some(state),
            _ => None,
        }
    }

    /// Push a fresh pins overlay onto the modal stack at the top
    /// of the action list.
    pub fn open_pins_overlay(&mut self) {
        self.modal_stack.push(crate::tui::Modal::Pins(
            crate::tui::widgets::pins::PinsOverlayState::new(),
        ));
    }

    /// Push a pre-configured pins overlay state — used by direct
    /// shortcuts (`N`/`B`/`A`/`b`) that skip the menu and open a
    /// sub-editor directly.
    pub fn set_pins_overlay(&mut self, state: crate::tui::widgets::pins::PinsOverlayState) {
        self.modal_stack.push(crate::tui::Modal::Pins(state));
    }

    /// Pop the pins overlay if it's on top; no-op otherwise.
    pub fn close_pins_overlay(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Pins(_))) {
            self.modal_stack.pop();
        }
    }

    /// Snapshot of the live pin state the pins overlay renders against.
    pub fn pins_context(&self) -> crate::tui::widgets::pins::PinsContext {
        crate::tui::widgets::pins::PinsContext {
            pin_create_defaults: self.pin_create_defaults(),
            pin_adopt_defaults: self.pin_adopt_defaults_if_available(),
            known_cwd_candidates: self.known_pin_cwd_candidates(),
            known_harness_keys: self.known_harness_keys().into_iter().collect(),
            known_mux_names: self.used_mux_names().into_iter().collect(),
            known_pin_ids: self.used_pin_ids().into_iter().collect(),
            known_pin_mux_names: self.used_pin_mux_names().into_iter().collect(),
            selected_pin_id: self.selected_pin_id(),
            pin_target: self.pin_mutation_target(),
            pin_bind_options: self.pin_bind_options(),
        }
    }

    /// Active `/` search overlay, if any. Lives on the
    /// modal stack (ADR 0085 contract 3); this accessor peeks the
    /// top entry.
    pub fn search_overlay(&self) -> Option<&crate::tui::widgets::search::SearchOverlayState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Search(state) => Some(state),
            _ => None,
        }
    }

    pub fn search_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::search::SearchOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Search(state) => Some(state),
            _ => None,
        }
    }

    /// Push a fresh search overlay onto the modal stack.
    pub fn open_search_overlay(&mut self) {
        self.modal_stack.push(crate::tui::Modal::Search(
            crate::tui::widgets::search::SearchOverlayState::new(),
        ));
    }

    /// Pop the search overlay if it's on top; no-op otherwise.
    pub fn close_search_overlay(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Search(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active `?` help overlay, if any. Lives on the modal
    /// stack (ADR 0085 contract 3); this accessor peeks the top
    /// entry.
    pub fn help_overlay(&self) -> Option<&crate::tui::widgets::help::HelpOverlayState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Help(state) => Some(state),
            _ => None,
        }
    }

    pub fn help_overlay_mut(&mut self) -> Option<&mut crate::tui::widgets::help::HelpOverlayState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Help(state) => Some(state),
            _ => None,
        }
    }

    /// Push a fresh help overlay onto the modal stack.
    pub fn open_help_overlay(&mut self) {
        self.modal_stack.push(crate::tui::Modal::Help(
            crate::tui::widgets::help::HelpOverlayState::new(),
        ));
    }

    /// Pop the help overlay if it's on top; no-op otherwise.
    pub fn close_help_overlay(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Help(_))) {
            self.modal_stack.pop();
        }
    }

    /// The modal stack (ADR 0085 contract 3). Reserved for
    /// generic stack-operating code (draw sweep, generic
    /// input-routing helper); overlay-specific consumers use the
    /// per-overlay accessors like `help_overlay()`.
    #[cfg(test)]
    pub(crate) fn modal_stack(&self) -> &[crate::tui::Modal] {
        &self.modal_stack
    }

    /// Active `o` full-value modal, if any. Lives on the
    /// modal stack (ADR 0085 contract 3); this accessor peeks the
    /// top entry.
    pub fn value_modal(&self) -> Option<&crate::tui::widgets::value_modal::ValueModalState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::ValueModal(state) => Some(state),
            _ => None,
        }
    }

    pub fn value_modal_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::value_modal::ValueModalState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::ValueModal(state) => Some(state),
            _ => None,
        }
    }

    /// Pop the value modal if it's on top; no-op otherwise.
    pub fn close_value_modal(&mut self) {
        if matches!(
            self.modal_stack.last(),
            Some(crate::tui::Modal::ValueModal(_))
        ) {
            self.modal_stack.pop();
        }
    }

    /// Read-only access to the active transcript viewer modal.
    /// Lives on the modal stack
    /// (ADR 0085 contract 3); this accessor peeks the top entry.
    pub fn viewer_modal(&self) -> Option<&crate::viewer::state::ViewerState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Viewer(state) => Some(state),
            _ => None,
        }
    }

    /// Mutable access for the draw path (the widget writes back
    /// viewport_height + total_lines metrics during render).
    pub fn viewer_modal_mut(&mut self) -> Option<&mut crate::viewer::state::ViewerState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Viewer(state) => Some(state),
            _ => None,
        }
    }

    /// Push a viewer modal onto the stack. Callers construct the
    /// initial state via `viewer_bridge::build_viewer_state` and
    /// pass ownership here.
    pub fn open_viewer_modal(&mut self, state: crate::viewer::state::ViewerState) {
        self.modal_stack.push(crate::tui::Modal::Viewer(state));
    }

    /// Pop the viewer modal if it's on top; no-op otherwise.
    pub fn close_viewer_modal(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Viewer(_))) {
            self.modal_stack.pop();
        }
    }

    /// Read accessor for the toast engine.
    /// Returns the engine itself so the renderer can call
    /// `(&engine).render_ref(...)` directly; `has_toast()` reports
    /// whether anything is queued.
    pub fn toast(&self) -> &ratatui_comfy_toaster::ToastEngine<()> {
        &self.toast
    }

    /// Post a transient toast that auto-dismisses after the widget's
    /// `TOAST_DURATION` window. Called from the runtime side (the
    /// `Cmd` boundary per ADR 0024). Drains any prior queued toast
    /// first so the newer feedback supersedes — matches the in-tree
    /// "replacement" contract the reducer test pins.
    pub fn post_toast(&mut self, label: impl Into<String>) {
        let label = label.into();
        crate::tui::widgets::toast::engine_dismiss_all(&mut self.toast);
        self.toast
            .show_toast(crate::tui::widgets::toast::builder_for(label));
    }

    /// Update the engine's frame area and retire expired toasts.
    /// Called by the runtime once per draw — handles terminal
    /// resize and drives the polled expiry that replaces the prior
    /// in-tree `is_expired()` check.
    pub fn prepare_toast_for_render(&mut self, area: ratatui::layout::Rect) {
        self.toast.set_area(area);
        self.toast.tick();
    }
}
