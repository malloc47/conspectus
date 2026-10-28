//! The modal stack on the App: lending it out with the App as the
//! entries' context, routing a key to its top, drawing it, and the open,
//! close, and peek accessors of each overlay.

use super::*;

use crate::tui::Modal;
use crate::tui::modal::ModalStack;
use tuikit::overlay::Settled;

impl App {
    /// Lends the modal stack out for one call, with the rest of the App as
    /// the entries' context, and puts it back after. While it is lent out
    /// the App's own stack is empty, so an entry reads its own state from
    /// itself, never through the App's modal accessors; nothing opens a
    /// modal from inside the call.
    pub fn with_modals<R>(&mut self, f: impl FnOnce(&mut ModalStack, &App) -> R) -> R {
        let mut stack = std::mem::take(&mut self.modal_stack);
        let out = f(&mut stack, self);
        debug_assert!(
            self.modal_stack.is_empty(),
            "a modal opened while the stack was lent out"
        );
        self.modal_stack = stack;
        out
    }

    /// One key for the modal on top: what it asked for, with the stack's
    /// part done (the top closed for a close or a commit). `None` when no
    /// modal is open, so the key is the main screen's.
    pub fn press_modal_key(
        &mut self,
        key: ratatui::crossterm::event::KeyEvent,
    ) -> Option<Settled<Modal, Msg>> {
        self.with_modals(|stack, app| {
            let outcome = stack.route(key, app)?;
            Some(stack.settle(outcome))
        })
    }

    /// The bookkeeping as a key closes `closed`, before what it committed
    /// is handled: the status line drops what the modal showed while it
    /// was open (closing says nothing), and a closing search remembers how
    /// it was set. Returns a message to send first, if any: a search that
    /// was not kept drops its preview.
    pub(crate) fn modal_closed(&mut self, closed: Modal, committed: Option<&Msg>) -> Option<Msg> {
        self.status_message = None;
        let Modal::Search(search) = closed else {
            return None;
        };
        self.search_closed(&search);
        // Keeping the search is the only way out that keeps its preview.
        let kept = matches!(
            committed,
            Some(Msg::ApplyContentSearch { .. } | Msg::KeepContentSearch { .. })
        );
        (!kept).then_some(Msg::DiscardContentPreview)
    }

    /// Draws every open modal over `area`, bottom to top.
    pub(crate) fn draw_modals(
        &mut self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
    ) {
        self.with_modals(|stack, app| stack.draw(area, buf, app, app.theme()));
    }

    /// Open `modal` on top of the stack; it takes the keys from now on.
    /// The per-overlay openers below do this with the state they build.
    pub fn open_modal(&mut self, modal: Modal) {
        self.modal_stack.push(modal);
    }

    /// Whether any modal is open; an open modal owns every key.
    pub fn has_modal(&self) -> bool {
        !self.modal_stack.is_empty()
    }

    /// Whether an open modal fills the screen (a full-screen viewer), so
    /// nothing under it is drawn.
    pub fn modals_fill_screen(&self) -> bool {
        self.modal_stack.fills_screen()
    }

    /// The name of the modal on top, as `Modal::NAMES` lists it.
    pub fn top_modal_name(&self) -> Option<&'static str> {
        self.modal_stack.top().map(Modal::name)
    }

    /// Whether the overlay on top is typing into a text field: a prompt,
    /// a form row that takes text, a filter editor, a query, or a find
    /// prompt. A paste goes only there; in a menu, a list, or a choice
    /// row its characters would act as keys.
    pub fn text_field_has_keys(&self) -> bool {
        self.modal_stack.takes_text()
    }

    /// Active rename-overlay state, if any. Lives on the modal
    /// stack (ADR 0085 contract 3); this accessor peeks the top
    /// entry.
    pub fn rename_overlay(&self) -> Option<&crate::tui::widgets::input::TextInputState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::Rename(state) => Some(state.inner()),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn rename_overlay_mut(&mut self) -> Option<&mut crate::tui::RenameOverlayState> {
        match self.modal_stack.top_mut()? {
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
        self.slot_prompt = None;
        self.modal_stack.push(crate::tui::Modal::Rename(
            crate::tui::RenameOverlayState::new(state),
        ));
    }

    /// Fix the entity the open rename prompt renames.
    pub(crate) fn set_rename_node(&mut self, node: NodeId) {
        self.rename_node = Some(node);
    }

    /// Open the text prompt that edits a pin slot.
    pub(crate) fn open_slot_prompt(
        &mut self,
        prompt: super::SlotPrompt,
        title: &str,
        initial: &str,
    ) {
        self.open_rename_overlay(crate::tui::widgets::input::TextInputState::new(
            title, initial,
        ));
        self.slot_prompt = Some(prompt);
    }

    /// Open a menu that unpins whichever of `slots` is chosen.
    pub(crate) fn open_pin_menu(&mut self, slots: Vec<String>) {
        use crate::tui::Msg;
        use crate::tui::widgets::row_menu::{RowMenuEntry, RowMenuState};
        let entries: Vec<RowMenuEntry> = slots
            .into_iter()
            .map(|slot| {
                RowMenuEntry::new(
                    format!("Unpin {slot}"),
                    "remove the pin; the entity and its history stay",
                    Msg::Unpin(slot),
                )
            })
            .collect();
        if !entries.is_empty() {
            self.open_row_menu(RowMenuState::new("Pinned", entries));
        }
    }

    /// Switch to the native view of the entity pin `slot` points at and
    /// select its row.
    pub(crate) fn reveal_slot(&mut self, slot: &str) -> Result<(), String> {
        let node = self
            .snapshot_handle()
            .and_then(|handle| {
                crate::tui::rows::pin_nodes(handle.snapshot())
                    .into_iter()
                    .find(|pin| pin.id.id == slot)
                    .and_then(|pin| pin.target_node.clone())
            })
            .ok_or_else(|| format!("pin `{slot}` has no entity to reveal"))?;
        let record = self.snapshot_handle().and_then(|handle| {
            crate::tui::rows::pin_nodes(handle.snapshot())
                .into_iter()
                .find(|pin| pin.id.id == slot)
                .and_then(|pin| pin.target.as_deref())
                .and_then(|target| target.strip_prefix(crate::model::PinSlot::HISTORY_PREFIX))
                .map(str::to_string)
        });
        self.reveal_node(&node, record)
            .map_err(|why| format!("`{slot}` {why}"))
    }

    /// Switch to the view that lists `node` and select its row. A gone
    /// entity is listed only where history is: the Mux view keeps its
    /// ghosts, and everything else is in History. On failure the
    /// operator stays where they were.
    pub(crate) fn reveal_node(
        &mut self,
        node: &NodeId,
        record_hint: Option<String>,
    ) -> Result<(), String> {
        let origin = self.nav_origin();
        self.reveal_node_quiet(node, record_hint)?;
        self.push_nav(origin);
        Ok(())
    }

    /// Where the operator stands now, as a place Backspace can return to.
    pub(crate) fn nav_origin(&self) -> NavEntry {
        NavEntry {
            view: self.active_view,
            selection: self.selection.clone(),
        }
    }

    /// Remember `origin` as the place Backspace returns to, unless the
    /// operator is still standing on it. The oldest entries fall off
    /// past the limit.
    pub(crate) fn push_nav(&mut self, origin: NavEntry) {
        if origin == self.nav_origin() || self.nav_back.last() == Some(&origin) {
            return;
        }
        self.nav_back.push(origin);
        if self.nav_back.len() > NAV_BACK_LIMIT {
            self.nav_back.remove(0);
        }
    }

    /// Whether Backspace on the left pane has somewhere to go back to.
    pub fn can_nav_back(&self) -> bool {
        !self.nav_back.is_empty()
    }

    /// Return to the view and row the last jump left, saying so when the
    /// row is gone.
    pub(crate) fn nav_pop(&mut self) -> Option<String> {
        let entry = self.nav_back.pop()?;
        self.switch_to_view(entry.view);
        self.rebuild_tree_in_place();
        match entry.selection {
            Some(id) if self.tree.rows.iter().any(|row| row.id == id) => {
                self.select_row_expanding(id);
                None
            }
            Some(_) => Some("that row is gone".to_string()),
            None => None,
        }
    }

    /// [`Self::reveal_node`] without a way back, for follow-ups the
    /// operator didn't ask to jump to.
    pub(crate) fn reveal_node_quiet(
        &mut self,
        node: &NodeId,
        record_hint: Option<String>,
    ) -> Result<(), String> {
        let Some(handle) = self.snapshot_handle() else {
            return Err("isn't listed: nothing is loaded yet".to_string());
        };
        let snapshot = handle.snapshot();
        let view = crate::tui::actions::natural_view(snapshot, node);
        // History lists a gone entity by its record rather than by node.
        let record = record_hint
            .or_else(|| {
                snapshot
                    .history
                    .by_key(&node.to_string())
                    .map(|(_, meta)| meta.id.clone())
            })
            .or_else(|| match node {
                NodeId::MuxSession(mux) => snapshot
                    .history
                    .muxes
                    .iter()
                    .find(|record| record.recipe.native_id() == mux.native_id)
                    .map(|record| record.meta.id.clone()),
                _ => None,
            });
        let previous = self.active_view;
        let previous_filter = self.filter.clone();
        self.switch_to_view(view);
        // A pin's own target is a pinned record, which History hides
        // unless asked; revealing it asks.
        if view == crate::tui::View::History {
            self.filter.show_pinned_history = true;
        }
        self.rebuild_tree_in_place();
        let Some(index) = self.tree.rows.iter().position(|row| {
            crate::tui::rows::row_node(&row.id) == Some(node)
                || matches!(&row.id, RowId::History { record_id } if Some(record_id) == record.as_ref())
        }) else {
            // Stay where the operator was.
            self.filter = previous_filter;
            self.switch_to_view(previous);
            self.rebuild_tree_in_place();
            return Err(format!("isn't listed in the {view:?} view"));
        };
        self.expand_to_and_select(index);
        Ok(())
    }

    /// Select a row, expanding the groups above it when it sits under
    /// collapsed ones.
    pub(super) fn select_row_expanding(&mut self, id: RowId) {
        if self.visible_rows().iter().any(|row| row.id == id) {
            self.set_selection(id);
        } else if let Some(index) = self.tree.rows.iter().position(|row| row.id == id) {
            self.expand_to_and_select(index);
        }
    }

    /// Expand the ancestors of the row at `index` and select it.
    pub(super) fn expand_to_and_select(&mut self, index: usize) {
        let mut wanted = self.tree.rows[index].depth;
        let ancestors: Vec<RowId> = self.tree.rows[..index]
            .iter()
            .rev()
            .filter(|row| {
                let keep = row.depth < wanted;
                if keep {
                    wanted = row.depth;
                }
                keep
            })
            .map(|row| row.id.clone())
            .collect();
        self.expanded.extend(ancestors);
        let id = self.tree.rows[index].id.clone();
        self.set_selection(id);
    }

    /// Select what an undo restored, now that its refresh has landed.
    /// It never leaves the view: a row that isn't listed here is left
    /// alone.
    pub(crate) fn apply_restore_focus(&mut self) {
        use crate::tui::undo::RestoreFocus;
        let Some(focus) = self.pending_restore_focus.take() else {
            return;
        };
        let index = self.tree.rows.iter().position(|row| match &focus {
            RestoreFocus::Row(id) => &row.id == id,
            RestoreFocus::Node(node) => crate::tui::rows::row_node(&row.id) == Some(node),
        });
        if let Some(index) = index {
            self.expand_to_and_select(index);
        }
    }

    /// Test hook: the row the action started on has disappeared from the
    /// refreshed tree, as a History record does once it is a live mux.
    #[cfg(test)]
    pub(crate) fn forget_follow_row(&mut self) {
        self.pending_follow_row = None;
    }

    /// Select the entity the last action acted on, now that the refresh
    /// after it has landed. Stays on the selected row when it is still
    /// listed, then in the current view when the entity is listed there;
    /// otherwise moves to the view where it now
    /// lives, and tells the operator when it can't be found.
    pub(crate) fn apply_pending_follow(&mut self) {
        if self.handoff.is_some() {
            return;
        }
        let Some(node) = self.pending_follow.take() else {
            return;
        };
        // Stay where the operator was while their row is still listed:
        // starting a pin leaves it in the Pinned view, and only a row that
        // disappears sends them to where the entity went.
        if let Some(row) = self.pending_follow_row.take()
            && let Some(index) = self.tree.rows.iter().position(|r| r.id == row)
        {
            self.expand_to_and_select(index);
            return;
        }
        if let Some(index) = self
            .tree
            .rows
            .iter()
            .position(|row| crate::tui::rows::row_node(&row.id) == Some(&node))
        {
            self.expand_to_and_select(index);
            return;
        }
        if let Err(why) = self.reveal_node_quiet(&node, None) {
            self.post_toast(format!("couldn't find the entity: it {why}"));
        }
    }

    /// The open picker, if it is on top of the stack.
    pub fn picker(&self) -> Option<&crate::tui::widgets::picker::PickerState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::Picker(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn picker_mut(&mut self) -> Option<&mut crate::tui::widgets::picker::PickerState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::Picker(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    pub fn open_picker(&mut self, state: crate::tui::widgets::picker::PickerState) {
        self.modal_stack
            .push(crate::tui::Modal::Picker(Box::new(state)));
    }

    pub fn close_picker(&mut self) {
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::Picker(_))) {
            self.modal_stack.pop();
        }
    }

    /// Pop the rename overlay if it's on top; no-op otherwise.
    pub fn close_rename_overlay(&mut self) {
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::Rename(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active worktree action menu, if on top of the stack.
    pub fn worktree_menu(&self) -> Option<&crate::tui::widgets::worktree_menu::WorktreeMenuState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::WorktreeMenu(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn worktree_menu_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::worktree_menu::WorktreeMenuState> {
        match self.modal_stack.top_mut()? {
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
            self.modal_stack.top(),
            Some(crate::tui::Modal::WorktreeMenu(_))
        ) {
            self.modal_stack.pop();
        }
    }

    /// Active bare mux form, if on top of the stack.
    pub fn new_mux_form(&self) -> Option<&crate::tui::widgets::new_mux::NewMuxFormState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::NewMux(state) => Some(state),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn new_mux_form_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::new_mux::NewMuxFormState> {
        match self.modal_stack.top_mut()? {
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
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::NewMux(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active mux action menu, if on top of stack.
    pub fn mux_menu(&self) -> Option<&crate::tui::widgets::mux_menu::MuxMenuState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::MuxMenu(state) => Some(state),
            _ => None,
        }
    }

    pub fn mux_menu_mut(&mut self) -> Option<&mut crate::tui::widgets::mux_menu::MuxMenuState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::MuxMenu(state) => Some(state),
            _ => None,
        }
    }

    pub fn open_mux_menu(&mut self, state: crate::tui::widgets::mux_menu::MuxMenuState) {
        self.modal_stack.push(crate::tui::Modal::MuxMenu(state));
    }

    pub fn close_mux_menu(&mut self) {
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::MuxMenu(_))) {
            self.modal_stack.pop();
        }
    }

    /// Open the full-value modal on `state`.
    pub fn open_value_modal(&mut self, state: crate::tui::widgets::value_modal::ValueModalState) {
        self.modal_stack.push(crate::tui::Modal::ValueModal(state));
        self.status_message = None;
    }

    /// Active row action menu, if on top of stack.
    pub fn row_menu(&self) -> Option<&crate::tui::widgets::row_menu::RowMenuState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::RowMenu(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    pub fn row_menu_mut(&mut self) -> Option<&mut crate::tui::widgets::row_menu::RowMenuState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::RowMenu(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    pub fn open_row_menu(&mut self, state: crate::tui::widgets::row_menu::RowMenuState) {
        self.modal_stack
            .push(crate::tui::Modal::RowMenu(Box::new(state)));
    }

    pub fn close_row_menu(&mut self) {
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::RowMenu(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active Clean up modal, if on top of stack.
    pub fn clean_up(&self) -> Option<&crate::tui::widgets::clean_up::CleanUpState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::CleanUp(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    pub fn clean_up_mut(&mut self) -> Option<&mut crate::tui::widgets::clean_up::CleanUpState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::CleanUp(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    pub fn open_clean_up(&mut self, state: crate::tui::widgets::clean_up::CleanUpState) {
        self.modal_stack
            .push(crate::tui::Modal::CleanUp(Box::new(state)));
    }

    pub fn close_clean_up(&mut self) {
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::CleanUp(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active mux-launch form, if on top of stack.
    pub fn mux_launch_form(&self) -> Option<&crate::tui::widgets::mux_launch::MuxLaunchFormState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::MuxLaunch(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    pub fn mux_launch_form_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::mux_launch::MuxLaunchFormState> {
        match self.modal_stack.top_mut()? {
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
            self.modal_stack.top(),
            Some(crate::tui::Modal::MuxLaunch(_))
        ) {
            self.modal_stack.pop();
        }
    }

    /// What the pin edit form needs to know about pin `slot`: its fields
    /// in effect, what they would be without its overrides, and the
    /// choices the fields offer. `None` when no such pin is in the graph.
    pub(crate) fn pin_edit_init(
        &self,
        slot: &str,
    ) -> Option<crate::tui::widgets::pin_edit::PinEditInit> {
        use crate::model::{GraphNode, SlotStatus};
        let snapshot = self.snapshot_handle()?.snapshot();
        let pin = snapshot.nodes.iter().find_map(|node| match node {
            GraphNode::Pin(pin) if pin.id.id == slot => Some(pin),
            _ => None,
        })?;
        let stored = snapshot.pins.iter().find(|stored| stored.id == slot)?;
        let mut probe = stored.clone();
        probe.overrides.clear();
        let inherited = crate::resolve::slots::resolve_slot(snapshot, &probe).effective;
        let cwd = pin.text("cwd").map(str::to_string);
        Some(crate::tui::widgets::pin_edit::PinEditInit {
            pin: slot.to_string(),
            kind: pin.kind,
            status: pin.status.label_for(pin.kind).to_string(),
            live_mux: pin.kind == crate::model::PinKind::Mux && pin.status == SlotStatus::Live,
            effective: pin.effective.clone(),
            overrides: pin.overrides.clone(),
            inherited,
            known_pin_names: snapshot.pins.iter().map(|p| p.id.clone()).collect(),
            known_harness_keys: crate::tui::runtime::known_harness_keys(),
            cwd_candidates: crate::tui::runtime::known_cwd_candidates(self, cwd.as_deref()),
            defaults: Some(crate::launch_default::LaunchDefaults::new(
                self.config().launch.clone(),
            )),
        })
    }

    /// Active pin edit form, if on top of stack.
    pub fn pin_edit_form(&self) -> Option<&crate::tui::widgets::pin_edit::PinEditFormState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::PinEdit(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    pub fn pin_edit_form_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::pin_edit::PinEditFormState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::PinEdit(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    pub fn open_pin_edit_form(&mut self, state: crate::tui::widgets::pin_edit::PinEditFormState) {
        self.modal_stack
            .push(crate::tui::Modal::PinEdit(Box::new(state)));
    }

    pub fn close_pin_edit_form(&mut self) {
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::PinEdit(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active controls-overlay state (ADR 0031), if any.
    /// Lives on the modal stack (ADR 0085 contract 3); this
    /// accessor peeks the top entry.
    pub fn controls_overlay(&self) -> Option<&crate::tui::widgets::controls::ControlsOverlayState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::Controls(state) => Some(state),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn controls_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::controls::ControlsOverlayState> {
        match self.modal_stack.top_mut()? {
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
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::Controls(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active `/` search overlay, if any. Lives on the
    /// modal stack (ADR 0085 contract 3); this accessor peeks the
    /// top entry.
    pub fn search_overlay(&self) -> Option<&crate::tui::widgets::search::SearchOverlayState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::Search(state) => Some(state),
            _ => None,
        }
    }

    pub fn search_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::search::SearchOverlayState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::Search(state) => Some(state),
            _ => None,
        }
    }

    /// What `/` opens: quick select, on the current view's rows. Returns
    /// the status hint to show.
    pub fn open_search_entry(&mut self) -> String {
        use crate::tui::keybindings::overlay::SEARCH_KEYS;
        use tuikit::keys::footer;
        self.open_search_overlay();
        format!("quick select: type to find · {}", footer(SEARCH_KEYS))
    }

    /// What `s` opens: content search, with the query filled in when a
    /// search is active. Returns the status hint to show.
    pub fn open_content_search_entry(&mut self) -> String {
        use crate::tui::keybindings::overlay::SEARCH_CONTENT_KEYS;
        use tuikit::keys::footer;
        if !self.config.live_preview_enabled {
            return crate::tui::widgets::search::CONTENT_SEARCH_OFF.to_string();
        }
        self.open_content_search_overlay();
        format!(
            "search contents: type a query · {}",
            footer(SEARCH_CONTENT_KEYS)
        )
    }

    /// Push a quick-select overlay onto the modal stack, on the current
    /// reach, with the content options as the operator last left them.
    pub fn open_search_overlay(&mut self) {
        use crate::tui::widgets::search::{SearchMode, SearchOverlayState};
        let (group, kinds) = self.search_options();
        let state = SearchOverlayState::configured(
            SearchMode::Quick,
            group,
            kinds,
            self.config.live_preview_enabled,
            "",
        );
        self.modal_stack.push(crate::tui::Modal::Search(state));
    }

    /// Push a content-search overlay onto the modal stack. It always opens
    /// with an empty query on the wider reach; earlier search strings are
    /// a key away, newest first.
    pub fn open_content_search_overlay(&mut self) {
        use crate::tui::widgets::search::{PastSearch, Reach, SearchMode, SearchOverlayState};
        let enabled = self.config.live_preview_enabled;
        let (group, kinds) = self.search_options();
        let past = self
            .search_history
            .entries()
            .iter()
            .rev()
            .map(|entry| PastSearch {
                text: entry.search.text.clone(),
                reach: entry.search.reach,
                kinds: entry.search.kinds,
                panes: entry.search.with_panes,
            })
            .collect();
        let state = SearchOverlayState::configured(SearchMode::Content, group, kinds, enabled, "")
            .with_reach(Reach::All)
            .with_panes(self.search_memory.is_none_or(|memory| memory.panes))
            .with_past(past);
        self.modal_stack.push(crate::tui::Modal::Search(state));
    }

    /// The content grouping and kinds the overlay opens with: as it was
    /// last left, or the `[search]` defaults the first time.
    fn search_options(
        &self,
    ) -> (
        crate::viewer::search::Grouping,
        crate::viewer::search::ContentKinds,
    ) {
        match self.search_memory {
            Some(memory) => (memory.group, memory.kinds),
            None => (
                self.config.search.group.granularity(),
                self.config.search.kinds,
            ),
        }
    }

    /// Pop the search overlay if it's on top; no-op otherwise.
    pub fn close_search_overlay(&mut self) {
        if let Some(Modal::Search(search)) = self
            .modal_stack
            .pop_if(|top| matches!(top, Modal::Search(_)))
        {
            self.search_closed(&search);
        }
    }

    /// Active `?` help overlay, if any. Lives on the modal
    /// stack (ADR 0085 contract 3); this accessor peeks the top
    /// entry.
    pub fn help_overlay(&self) -> Option<&crate::tui::widgets::help::HelpOverlayState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::Help(state) => Some(state),
            _ => None,
        }
    }

    pub fn help_overlay_mut(&mut self) -> Option<&mut crate::tui::widgets::help::HelpOverlayState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::Help(state) => Some(state),
            _ => None,
        }
    }

    /// Open the Messages overlay and mark its warnings and errors
    /// seen.
    pub fn open_messages_overlay(&mut self) {
        self.messages.mark_seen();
        self.modal_stack.push(crate::tui::Modal::Messages(
            crate::tui::widgets::messages::MessagesOverlayState::new(),
        ));
    }

    pub fn messages_overlay(&self) -> Option<&crate::tui::widgets::messages::MessagesOverlayState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::Messages(state) => Some(state),
            _ => None,
        }
    }

    /// The open Messages overlay, when it is on top.
    pub fn messages_overlay_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::messages::MessagesOverlayState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::Messages(state) => Some(state),
            _ => None,
        }
    }

    pub fn close_messages_overlay(&mut self) {
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::Messages(_))) {
            self.modal_stack.pop();
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
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::Help(_))) {
            self.modal_stack.pop();
        }
    }

    /// Pop the top modal, for tests that compare a frame with and
    /// without it.
    #[cfg(test)]
    pub(crate) fn pop_top_modal(&mut self) {
        self.modal_stack.pop();
    }

    /// The modal stack, for tests that look at more than the top entry;
    /// overlay-specific code uses the per-overlay accessors like
    /// `help_overlay()`.
    #[cfg(test)]
    pub(crate) fn modal_stack(&self) -> &ModalStack {
        &self.modal_stack
    }

    /// Active `o` full-value modal, if any. Lives on the
    /// modal stack (ADR 0085 contract 3); this accessor peeks the
    /// top entry.
    pub fn value_modal(&self) -> Option<&crate::tui::widgets::value_modal::ValueModalState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::ValueModal(state) => Some(state),
            _ => None,
        }
    }

    pub fn value_modal_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::value_modal::ValueModalState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::ValueModal(state) => Some(state),
            _ => None,
        }
    }

    /// Pop the value modal if it's on top; no-op otherwise.
    pub fn close_value_modal(&mut self) {
        if matches!(
            self.modal_stack.top(),
            Some(crate::tui::Modal::ValueModal(_))
        ) {
            self.modal_stack.pop();
        }
    }

    /// Read-only access to the active transcript viewer modal.
    /// Lives on the modal stack
    /// (ADR 0085 contract 3); this accessor peeks the top entry.
    pub fn viewer_modal(&self) -> Option<&crate::viewer::state::ViewerState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::Viewer(state) => Some(state),
            _ => None,
        }
    }

    /// Mutable access to the viewer on top.
    pub fn viewer_modal_mut(&mut self) -> Option<&mut crate::viewer::state::ViewerState> {
        match self.modal_stack.top_mut()? {
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

    /// The pane scrollback viewer, when it is on top.
    pub fn pane_viewer(&self) -> Option<&crate::tui::widgets::pane_viewer::PaneViewerState> {
        match self.modal_stack.top()? {
            crate::tui::Modal::PaneViewer(state) => Some(state),
            _ => None,
        }
    }

    pub fn pane_viewer_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::pane_viewer::PaneViewerState> {
        match self.modal_stack.top_mut()? {
            crate::tui::Modal::PaneViewer(state) => Some(state),
            _ => None,
        }
    }

    pub fn open_pane_viewer(&mut self, state: crate::tui::widgets::pane_viewer::PaneViewerState) {
        self.modal_stack
            .push(crate::tui::Modal::PaneViewer(Box::new(state)));
    }

    pub fn close_pane_viewer(&mut self) {
        if matches!(
            self.modal_stack.top(),
            Some(crate::tui::Modal::PaneViewer(_))
        ) {
            self.modal_stack.pop();
        }
    }

    /// Pop the viewer modal if it's on top; no-op otherwise.
    pub fn close_viewer_modal(&mut self) {
        if matches!(self.modal_stack.top(), Some(crate::tui::Modal::Viewer(_))) {
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
        let toast = crate::tui::widgets::toast::builder_for(label, self.theme());
        self.toast.show_toast(toast);
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
