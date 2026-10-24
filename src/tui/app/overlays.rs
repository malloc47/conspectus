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

    /// Expand the ancestors of the row at `index` and select it.
    fn expand_to_and_select(&mut self, index: usize) {
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
        if let Err(why) = self.reveal_node(&node, None) {
            self.post_toast(format!("couldn't find the entity: it {why}"));
        }
    }

    /// The open picker, if it is on top of the stack.
    pub fn picker(&self) -> Option<&crate::tui::widgets::picker::PickerState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Picker(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    /// Mutable access for the runtime's per-key forwarding.
    pub fn picker_mut(&mut self) -> Option<&mut crate::tui::widgets::picker::PickerState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Picker(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    pub fn open_picker(&mut self, state: crate::tui::widgets::picker::PickerState) {
        self.modal_stack
            .push(crate::tui::Modal::Picker(Box::new(state)));
    }

    pub fn close_picker(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Picker(_))) {
            self.modal_stack.pop();
        }
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

    /// Open the full-value modal on `state`.
    pub fn open_value_modal(&mut self, state: crate::tui::widgets::value_modal::ValueModalState) {
        self.modal_stack.push(crate::tui::Modal::ValueModal(state));
        self.status_message = None;
    }

    /// Active row action menu, if on top of stack.
    pub fn row_menu(&self) -> Option<&crate::tui::widgets::row_menu::RowMenuState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::RowMenu(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    pub fn row_menu_mut(&mut self) -> Option<&mut crate::tui::widgets::row_menu::RowMenuState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::RowMenu(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    pub fn open_row_menu(&mut self, state: crate::tui::widgets::row_menu::RowMenuState) {
        self.modal_stack
            .push(crate::tui::Modal::RowMenu(Box::new(state)));
    }

    pub fn close_row_menu(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::RowMenu(_))) {
            self.modal_stack.pop();
        }
    }

    /// Active Clean up modal, if on top of stack.
    pub fn clean_up(&self) -> Option<&crate::tui::widgets::clean_up::CleanUpState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::CleanUp(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    pub fn clean_up_mut(&mut self) -> Option<&mut crate::tui::widgets::clean_up::CleanUpState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::CleanUp(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    pub fn open_clean_up(&mut self, state: crate::tui::widgets::clean_up::CleanUpState) {
        self.modal_stack
            .push(crate::tui::Modal::CleanUp(Box::new(state)));
    }

    pub fn close_clean_up(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::CleanUp(_))) {
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
        match self.modal_stack.last()? {
            crate::tui::Modal::PinEdit(state) => Some(state.as_ref()),
            _ => None,
        }
    }

    pub fn pin_edit_form_mut(
        &mut self,
    ) -> Option<&mut crate::tui::widgets::pin_edit::PinEditFormState> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::PinEdit(state) => Some(state.as_mut()),
            _ => None,
        }
    }

    pub fn open_pin_edit_form(&mut self, state: crate::tui::widgets::pin_edit::PinEditFormState) {
        self.modal_stack
            .push(crate::tui::Modal::PinEdit(Box::new(state)));
    }

    pub fn close_pin_edit_form(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::PinEdit(_))) {
            self.modal_stack.pop();
        }
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

    /// What `/` opens: the Content prompt in the Search view, the search
    /// overlay elsewhere. Returns the status hint to show.
    pub fn open_search_entry(&mut self) -> String {
        use crate::tui::keybindings::overlay::{SEARCH_KEYS, footer};
        if self.active_view() == crate::tui::View::Search {
            let ctx = self.controls_context();
            self.modal_stack.push(crate::tui::Modal::Controls(
                crate::tui::widgets::controls::ControlsOverlayState::new_at_content(&ctx),
            ));
            return "search contents: type a query · Enter search · Esc cancel".to_string();
        }
        self.open_search_overlay();
        format!(
            "search: type to filter · Enter pick · Esc close · {}",
            footer(SEARCH_KEYS)
        )
    }

    /// Push a search overlay onto the modal stack: in the contents scope
    /// with the query filled in when a search is active, as the operator
    /// last left it otherwise, and on the rows with `[search]` defaults
    /// the first time.
    pub fn open_search_overlay(&mut self) {
        use crate::tui::widgets::search::{SearchOverlayState, SearchScope};
        let enabled = self.config.live_preview_enabled;
        let state = match (self.content(), self.search_memory) {
            (Some(content), _) => SearchOverlayState::configured(
                SearchScope::Contents,
                content.group,
                content.kinds,
                enabled,
                &content.text,
            ),
            (None, Some(memory)) => SearchOverlayState::configured(
                memory.scope,
                memory.group,
                memory.kinds,
                enabled,
                "",
            ),
            (None, None) => SearchOverlayState::configured(
                SearchScope::Rows,
                self.config.search.group,
                self.config.search.kinds,
                enabled,
                "",
            ),
        };
        self.modal_stack.push(crate::tui::Modal::Search(state));
    }

    /// Pop the search overlay if it's on top; no-op otherwise.
    pub fn close_search_overlay(&mut self) {
        if matches!(self.modal_stack.last(), Some(crate::tui::Modal::Search(_))) {
            self.remember_search_overlay();
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
    /// Open the Messages overlay and mark its warnings and errors
    /// seen.
    pub fn open_messages_overlay(&mut self) {
        self.messages.mark_seen();
        self.modal_stack.push(crate::tui::Modal::Messages(
            crate::tui::widgets::messages::MessagesOverlayState::new(),
        ));
    }

    pub fn messages_overlay(&self) -> Option<&crate::tui::widgets::messages::MessagesOverlayState> {
        match self.modal_stack.last()? {
            crate::tui::Modal::Messages(state) => Some(state),
            _ => None,
        }
    }

    /// The open Messages overlay together with the log it reads.
    pub fn messages_overlay_mut(
        &mut self,
    ) -> Option<(
        &mut crate::tui::widgets::messages::MessagesOverlayState,
        &crate::tui::messages::MessageLog,
    )> {
        match self.modal_stack.last_mut()? {
            crate::tui::Modal::Messages(state) => Some((state, &self.messages)),
            _ => None,
        }
    }

    pub fn close_messages_overlay(&mut self) {
        if matches!(
            self.modal_stack.last(),
            Some(crate::tui::Modal::Messages(_))
        ) {
            self.modal_stack.pop();
        }
    }

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

    /// Pop the top modal, for tests that compare a frame with and
    /// without it.
    #[cfg(test)]
    pub(crate) fn pop_top_modal(&mut self) {
        self.modal_stack.pop();
    }

    /// The top of the modal stack, for tests that drive an overlay's own
    /// key handling and then render it.
    #[cfg(test)]
    pub(crate) fn top_modal_mut(&mut self) -> Option<&mut crate::tui::Modal> {
        self.modal_stack.last_mut()
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
