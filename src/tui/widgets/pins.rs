//! Pins management overlay (ADR 0057 / H-PIN-022..024).
//!
//! Dedicated modal — separate from the view/grouping/filter
//! [`controls`](super::controls) overlay — that fronts every pin
//! CRUD flow: create, rename, remove, bind (PinAmbiguous override),
//! and rebind (external-rename recovery).
//!
//! ## Architecture
//!
//! The overlay holds only what is unique to its UI: a cursor over a
//! flat action list plus an optional sub-editor (create form, edit
//! form, bind picker, remove confirmation). Live app state — the
//! selected row's defaults, the resolver's ambiguous-binding options
//! — lives on [`crate::tui::app::App`] and is borrowed each frame as
//! a [`PinsContext`].
//!
//! Key dispatch returns a [`PinsOutcome`]: keep open, close, or
//! apply a [`PinsAction`] (and either close or stay open). Pin
//! mutation side effects live in the runtime so the file write path
//! is shared with direct-shortcut openers; this widget only
//! describes intent.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::macros::{line, span};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tui_popup::KnownSize;

use crate::tui::Theme;
use crate::tui::widgets::input::TextInputState;
use crate::tui::widgets::popup_frame::themed_popup;

/// Discoverable pin action group. Each entry maps 1:1 to a CLI
/// `conspectus pin <subcommand>` so the modal stays a thin
/// presentation of the underlying surface.
pub const PIN_ACTION_OPTIONS: &[&str] = &["create", "launch", "rename", "remove", "bind", "rebind"];

/// Read-only snapshot the pins overlay renders against. Borrowed
/// each frame so the overlay never holds a stale copy.
#[derive(Debug, Clone, Default)]
pub struct PinsContext {
    pub pin_create_defaults: PinCreateDefaults,
    pub pin_adopt_defaults: Option<PinCreateDefaults>,
    pub known_harness_keys: Vec<String>,
    pub known_mux_names: Vec<String>,
    pub pin_target: Option<PinMutationTarget>,
    pub pin_bind_options: Vec<PinBindOption>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinCreateDefaults {
    pub mode: PinCreateMode,
    pub id: String,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux_name: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PinCreateMode {
    #[default]
    NewVariation,
    AdoptSelected,
}

impl PinCreateMode {
    fn label(self) -> &'static str {
        match self {
            Self::NewVariation => "new",
            Self::AdoptSelected => "adopt selected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinMutationTarget {
    pub id: String,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux_name: String,
    pub mux_socket: Option<String>,
    pub launch_argv: Vec<String>,
    pub store_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinBindOption {
    pub pin_id: String,
    pub session_key: String,
    pub label: String,
}

/// One landable row in the pins overlay. Only the action list rows
/// are landable today; sub-editors take over key dispatch when open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinsCursor {
    Action(usize),
}

/// Sub-editor that owns key input while open. The host overlay
/// suspends its own bindings until the sub-editor commits or cancels.
#[derive(Debug, Clone)]
pub enum PinsSubEditor {
    Create(Box<PinCreateState>),
    Edit(Box<PinEditState>),
    Rebind(Box<PinRebindState>),
    Bind(PinBindState),
    Remove(Box<PinRemoveState>),
}

/// What the pins overlay returned from a single key event.
#[derive(Debug, Clone, PartialEq)]
pub enum PinsOutcome {
    Continue,
    Close,
    ApplyAndStay(PinsAction),
    ApplyAndClose(PinsAction),
}

/// Side-effecting outcome the runtime applies. The overlay never
/// touches the app directly.
#[derive(Debug, Clone, PartialEq)]
pub enum PinsAction {
    CreatePin(PinCreateRequest),
    EditPin(PinEditRequest),
    BindPin(PinBindRequest),
    RemovePin(PinRemoveRequest),
    /// Launch the named pin via the CLI's `pin launch` path (ADR
    /// 0057 / ADR 0058). The runtime suspends the TUI, re-execs
    /// into the binary, and refreshes on return — same code path
    /// the row-level `Enter` / `L` shortcuts use.
    LaunchPin {
        pin_id: String,
    },
    /// Action chosen from the menu without the prerequisites met
    /// (e.g. `rename` with no pin row selected). The runtime surfaces
    /// a status hint instead of mutating.
    PinPlaceholder(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinCreateRequest {
    pub id: String,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux_name: String,
    pub mux_socket: Option<String>,
    pub adopt_source_mux_name: Option<String>,
    pub launch_argv: Vec<String>,
    pub store: PinCreateStore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinRemoveRequest {
    pub id: String,
    pub display_name: String,
    pub store_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinEditRequest {
    pub original_id: String,
    pub id: String,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux_name: String,
    pub mux_socket: Option<String>,
    pub launch_argv: Vec<String>,
    pub store_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinBindRequest {
    pub pin_id: String,
    pub session_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinCreateStore {
    Auto,
    Project,
    User,
}

impl PinCreateStore {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Project => "project",
            Self::User => "user",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PinCreateState {
    mode: PinCreateMode,
    cursor: usize,
    adopt_defaults: Option<PinCreateDefaults>,
    name: TextInputState,
    id: TextInputState,
    display_name: TextInputState,
    harness: TextInputState,
    cwd: TextInputState,
    mux_name: TextInputState,
    mux_socket: TextInputState,
    launch_argv: TextInputState,
    store: PinCreateStore,
    id_overridden: bool,
    display_overridden: bool,
    mux_overridden: bool,
    known_harness_keys: Vec<String>,
    known_mux_names: Vec<String>,
    adopt_auto_uncheck_armed: bool,
    adopt_auto_checked_by_collision: bool,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PinEditState {
    target: PinMutationTarget,
    cursor: usize,
    id: TextInputState,
    display_name: TextInputState,
    mux_name: TextInputState,
    mux_socket: TextInputState,
    launch_argv: TextInputState,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PinBindState {
    cursor: usize,
    options: Vec<PinBindOption>,
}

#[derive(Debug, Clone)]
pub struct PinRemoveState {
    target: PinMutationTarget,
}

/// Mux-only rebind form used by the `B` direct shortcut. Surfaces
/// only `mux.name` and `mux.socket_name` for editing; every other
/// field is carried through from the target unchanged. The runtime
/// still serializes as a [`PinEditRequest`] so the write path stays
/// shared with the full edit form.
#[derive(Debug, Clone)]
pub struct PinRebindState {
    target: PinMutationTarget,
    cursor: usize,
    mux_name: TextInputState,
    mux_socket: TextInputState,
    error: Option<String>,
}

/// Pure state for the pins overlay: cursor position plus the active
/// sub-editor (if any).
#[derive(Debug, Clone)]
pub struct PinsOverlayState {
    cursor: PinsCursor,
    sub_editor: Option<PinsSubEditor>,
}

impl PinsOverlayState {
    /// Open at the top of the action list.
    pub fn new() -> Self {
        Self {
            cursor: PinsCursor::Action(0),
            sub_editor: None,
        }
    }

    /// Open directly into the create sub-editor seeded with
    /// `defaults`. Used by the direct shortcut so the operator
    /// skips the menu step.
    pub fn open_with_create(defaults: PinCreateDefaults) -> Self {
        Self::open_with_create_options(defaults, None, Vec::new(), Vec::new())
    }

    pub fn open_with_create_options(
        defaults: PinCreateDefaults,
        adopt_defaults: Option<PinCreateDefaults>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
    ) -> Self {
        let initial = adopt_defaults.clone().unwrap_or(defaults);
        Self::open_with_create_initial(initial, adopt_defaults, known_harness_keys, known_mux_names)
    }

    pub fn open_with_adopt_options(
        defaults: PinCreateDefaults,
        adopt_defaults: Option<PinCreateDefaults>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
    ) -> Self {
        let initial = adopt_defaults.clone().unwrap_or_else(|| defaults.clone());
        Self::open_with_create_initial(initial, adopt_defaults, known_harness_keys, known_mux_names)
    }

    fn open_with_create_initial(
        initial: PinCreateDefaults,
        adopt_defaults: Option<PinCreateDefaults>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
    ) -> Self {
        Self {
            cursor: PinsCursor::Action(0),
            sub_editor: Some(PinsSubEditor::Create(Box::new(PinCreateState::new(
                initial,
                adopt_defaults,
                known_harness_keys,
                known_mux_names,
            )))),
        }
    }

    /// Open directly into the bind picker. Returns `None` when
    /// there are no competing options to choose between.
    pub fn open_with_bind(options: Vec<PinBindOption>) -> Option<Self> {
        if options.is_empty() {
            return None;
        }
        Some(Self {
            // "bind" is index 4 in PIN_ACTION_OPTIONS.
            cursor: PinsCursor::Action(4),
            sub_editor: Some(PinsSubEditor::Bind(PinBindState::new(options))),
        })
    }

    /// Open directly into the remove confirmation for `target`.
    pub fn open_with_remove(target: PinMutationTarget) -> Self {
        Self {
            // "remove" is index 3 in PIN_ACTION_OPTIONS.
            cursor: PinsCursor::Action(3),
            sub_editor: Some(PinsSubEditor::Remove(Box::new(PinRemoveState::new(target)))),
        }
    }

    /// Open directly into the edit form for `target`. Used by the
    /// rename direct shortcut.
    pub fn open_with_edit(target: PinMutationTarget) -> Self {
        Self {
            // "rename" is index 2 in PIN_ACTION_OPTIONS.
            cursor: PinsCursor::Action(2),
            sub_editor: Some(PinsSubEditor::Edit(Box::new(PinEditState::new(target)))),
        }
    }

    /// Open directly into the mux-only rebind form. Backed by the
    /// `B` direct shortcut so external tmux renames recover in one
    /// keystroke without paging through the full edit form.
    pub fn open_with_rebind(target: PinMutationTarget) -> Self {
        Self {
            // "rebind" is index 5 in PIN_ACTION_OPTIONS.
            cursor: PinsCursor::Action(5),
            sub_editor: Some(PinsSubEditor::Rebind(Box::new(PinRebindState::new(target)))),
        }
    }

    pub fn cursor(&self) -> PinsCursor {
        self.cursor
    }

    pub fn sub_editor(&self) -> Option<&PinsSubEditor> {
        self.sub_editor.as_ref()
    }

    /// Dispatch a crossterm key event. Returns the outcome the caller
    /// should apply.
    pub fn handle_key(&mut self, ctx: &PinsContext, event: KeyEvent) -> PinsOutcome {
        if event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(event.code, KeyCode::Char('c'))
        {
            return PinsOutcome::Close;
        }

        if self.sub_editor.is_some() {
            return Self::dispatch_sub_editor(&mut self.sub_editor, event);
        }

        match event.code {
            KeyCode::Esc => PinsOutcome::Close,
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = move_cursor(self.cursor, -1);
                PinsOutcome::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = move_cursor(self.cursor, 1);
                PinsOutcome::Continue
            }
            KeyCode::Enter => self.activate(ctx),
            _ => PinsOutcome::Continue,
        }
    }

    fn activate(&mut self, ctx: &PinsContext) -> PinsOutcome {
        let PinsCursor::Action(idx) = self.cursor;
        let label = PIN_ACTION_OPTIONS.get(idx).copied().unwrap_or("help");
        if label == "create" {
            let initial = ctx
                .pin_adopt_defaults
                .clone()
                .unwrap_or_else(|| ctx.pin_create_defaults.clone());
            self.sub_editor = Some(PinsSubEditor::Create(Box::new(PinCreateState::new(
                initial,
                ctx.pin_adopt_defaults.clone(),
                ctx.known_harness_keys.clone(),
                ctx.known_mux_names.clone(),
            ))));
            PinsOutcome::Continue
        } else if label == "launch" {
            // Launch has no sub-editor — the runtime takes the
            // pin id and re-execs into `conspectus pin launch`,
            // suspending the TUI. Requires a pin selection;
            // placeholder otherwise.
            if let Some(target) = ctx.pin_target.clone() {
                PinsOutcome::ApplyAndClose(PinsAction::LaunchPin { pin_id: target.id })
            } else {
                PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder(label))
            }
        } else if label == "rename" {
            if let Some(target) = ctx.pin_target.clone() {
                self.sub_editor = Some(PinsSubEditor::Edit(Box::new(PinEditState::new(target))));
                PinsOutcome::Continue
            } else {
                PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder(label))
            }
        } else if label == "rebind" {
            if let Some(target) = ctx.pin_target.clone() {
                self.sub_editor =
                    Some(PinsSubEditor::Rebind(Box::new(PinRebindState::new(target))));
                PinsOutcome::Continue
            } else {
                PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder(label))
            }
        } else if label == "remove" {
            if let Some(target) = ctx.pin_target.clone() {
                self.sub_editor =
                    Some(PinsSubEditor::Remove(Box::new(PinRemoveState::new(target))));
                PinsOutcome::Continue
            } else {
                PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder(label))
            }
        } else if label == "bind" {
            if ctx.pin_bind_options.is_empty() {
                PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder(label))
            } else {
                self.sub_editor = Some(PinsSubEditor::Bind(PinBindState::new(
                    ctx.pin_bind_options.clone(),
                )));
                PinsOutcome::Continue
            }
        } else {
            PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder(label))
        }
    }

    fn dispatch_sub_editor(slot: &mut Option<PinsSubEditor>, event: KeyEvent) -> PinsOutcome {
        let editor = slot.as_mut().expect("dispatch called with empty slot");
        match editor {
            PinsSubEditor::Create(state) => match state.handle_key(event) {
                PinCreateOutcome::Continue => PinsOutcome::Continue,
                PinCreateOutcome::Cancel => {
                    *slot = None;
                    PinsOutcome::Continue
                }
                PinCreateOutcome::Confirm(request) => {
                    *slot = None;
                    PinsOutcome::ApplyAndClose(PinsAction::CreatePin(request))
                }
            },
            PinsSubEditor::Edit(state) => match state.handle_key(event) {
                PinEditOutcome::Continue => PinsOutcome::Continue,
                PinEditOutcome::Cancel => {
                    *slot = None;
                    PinsOutcome::Continue
                }
                PinEditOutcome::Confirm(request) => {
                    *slot = None;
                    PinsOutcome::ApplyAndClose(PinsAction::EditPin(*request))
                }
            },
            PinsSubEditor::Rebind(state) => match state.handle_key(event) {
                PinEditOutcome::Continue => PinsOutcome::Continue,
                PinEditOutcome::Cancel => {
                    *slot = None;
                    PinsOutcome::Continue
                }
                PinEditOutcome::Confirm(request) => {
                    *slot = None;
                    PinsOutcome::ApplyAndClose(PinsAction::EditPin(*request))
                }
            },
            PinsSubEditor::Bind(state) => match state.handle_key(event) {
                PinBindOutcome::Continue => PinsOutcome::Continue,
                PinBindOutcome::Cancel => {
                    *slot = None;
                    PinsOutcome::Continue
                }
                PinBindOutcome::Confirm(request) => {
                    *slot = None;
                    PinsOutcome::ApplyAndClose(PinsAction::BindPin(request))
                }
            },
            PinsSubEditor::Remove(state) => match state.handle_key(event) {
                PinRemoveOutcome::Continue => PinsOutcome::Continue,
                PinRemoveOutcome::Cancel => {
                    *slot = None;
                    PinsOutcome::Continue
                }
                PinRemoveOutcome::Confirm(request) => {
                    *slot = None;
                    PinsOutcome::ApplyAndClose(PinsAction::RemovePin(request))
                }
            },
        }
    }
}

impl Default for PinsOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

fn move_cursor(cursor: PinsCursor, delta: i32) -> PinsCursor {
    let PinsCursor::Action(idx) = cursor;
    let len = PIN_ACTION_OPTIONS.len() as i32;
    if len == 0 {
        return cursor;
    }
    let next = ((idx as i32 + delta) % len + len) % len;
    PinsCursor::Action(next as usize)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PinCreateOutcome {
    Continue,
    Confirm(PinCreateRequest),
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PinEditOutcome {
    Continue,
    Confirm(Box<PinEditRequest>),
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PinBindOutcome {
    Continue,
    Confirm(PinBindRequest),
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PinRemoveOutcome {
    Continue,
    Confirm(PinRemoveRequest),
    Cancel,
}

impl PinCreateState {
    const LOGICAL_STORE_FIELD: usize = 9;

    fn new(
        defaults: PinCreateDefaults,
        adopt_defaults: Option<PinCreateDefaults>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
    ) -> Self {
        let name = if !defaults.display_name.is_empty() {
            defaults.display_name.clone()
        } else if !defaults.id.is_empty() {
            defaults.id.clone()
        } else {
            "new pin".to_string()
        };
        let derived_id = pin_id_candidate(&name);
        let id = if defaults.id.is_empty() {
            derived_id.clone()
        } else {
            defaults.id
        };
        let display_name = if defaults.display_name.is_empty() {
            name.clone()
        } else {
            defaults.display_name
        };
        let derived_mux_name = derived_mux_name_for_mode(defaults.mode, &name, &derived_id);
        let mux_name = if defaults.mux_name.is_empty() {
            derived_mux_name.clone()
        } else {
            defaults.mux_name
        };
        let id_overridden = id != derived_id;
        let display_overridden = display_name != name;
        let mux_overridden = mux_name != derived_mux_name;
        let known_harness_keys = normalized_harness_keys(known_harness_keys, [&defaults.harness]);
        Self {
            mode: defaults.mode,
            cursor: 0,
            adopt_defaults,
            name: TextInputState::new(" name ", name),
            id: TextInputState::new(" id ", id),
            display_name: TextInputState::new(" display ", display_name),
            harness: TextInputState::new(" harness ", defaults.harness),
            cwd: TextInputState::new(" cwd ", defaults.cwd),
            mux_name: TextInputState::new(" mux ", mux_name),
            mux_socket: TextInputState::new(" socket ", String::new()),
            launch_argv: TextInputState::new(" launch argv ", String::new()),
            store: PinCreateStore::Auto,
            id_overridden,
            display_overridden,
            mux_overridden,
            known_harness_keys,
            known_mux_names,
            adopt_auto_uncheck_armed: defaults.mode == PinCreateMode::AdoptSelected,
            adopt_auto_checked_by_collision: false,
            error: None,
        }
    }

    fn handle_key(&mut self, event: KeyEvent) -> PinCreateOutcome {
        match event.code {
            KeyCode::Esc => PinCreateOutcome::Cancel,
            KeyCode::Enter => match self.request() {
                Ok(request) => PinCreateOutcome::Confirm(request),
                Err(err) => {
                    self.error = Some(err);
                    PinCreateOutcome::Continue
                }
            },
            KeyCode::Up => {
                self.move_cursor(-1);
                PinCreateOutcome::Continue
            }
            KeyCode::Down | KeyCode::Tab => {
                self.move_cursor(1);
                PinCreateOutcome::Continue
            }
            KeyCode::BackTab => {
                self.move_cursor(-1);
                PinCreateOutcome::Continue
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.logical_cursor() == 1 && self.can_toggle_mode() =>
            {
                self.toggle_mode();
                PinCreateOutcome::Continue
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if self.logical_cursor() == 3 => {
                self.cycle_harness(if matches!(event.code, KeyCode::Left) {
                    -1
                } else {
                    1
                });
                self.error = None;
                PinCreateOutcome::Continue
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.logical_cursor() == Self::LOGICAL_STORE_FIELD =>
            {
                self.cycle_store(if matches!(event.code, KeyCode::Left) {
                    -1
                } else {
                    1
                });
                PinCreateOutcome::Continue
            }
            _ => {
                if event.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(event.code, KeyCode::Char('c'))
                {
                    return PinCreateOutcome::Cancel;
                }
                let before = self
                    .active_input()
                    .map(|input| input.value().to_string())
                    .unwrap_or_default();
                let active = self.cursor;
                if let Some(input) = self.active_input_mut() {
                    let _ = input.handle_key(event);
                    let changed = input.value() != before;
                    if changed {
                        self.after_active_input_changed(active);
                    }
                    self.error = None;
                }
                PinCreateOutcome::Continue
            }
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        let len = self.field_count() as i32;
        let next = ((self.cursor as i32 + delta) % len + len) % len;
        self.cursor = next as usize;
    }

    fn field_count(&self) -> usize {
        if self.can_toggle_mode() { 10 } else { 9 }
    }

    fn can_toggle_mode(&self) -> bool {
        self.adopt_defaults.is_some()
    }

    fn logical_cursor(&self) -> usize {
        if self.can_toggle_mode() || self.cursor == 0 {
            self.cursor
        } else {
            self.cursor + 1
        }
    }

    fn render_cursor(&self) -> usize {
        self.logical_cursor()
    }

    fn toggle_mode(&mut self) {
        match self.mode {
            PinCreateMode::NewVariation => {
                if self.adopt_defaults.is_none() {
                    return;
                }
                self.mode = PinCreateMode::AdoptSelected;
                self.adopt_auto_uncheck_armed = false;
                self.adopt_auto_checked_by_collision = false;
                if !self.mux_overridden {
                    self.mux_name = TextInputState::new(
                        " mux ",
                        derived_mux_name_for_mode(
                            self.mode,
                            self.name.value(),
                            &pin_id_candidate(self.name.value()),
                        ),
                    );
                }
            }
            PinCreateMode::AdoptSelected => {
                self.mode = PinCreateMode::NewVariation;
                self.adopt_auto_uncheck_armed = false;
                self.adopt_auto_checked_by_collision = false;
                if !self.mux_overridden {
                    self.mux_name = TextInputState::new(
                        " mux ",
                        derived_mux_name_for_mode(
                            self.mode,
                            self.name.value(),
                            &pin_id_candidate(self.name.value()),
                        ),
                    );
                }
            }
        }
    }

    fn cycle_store(&mut self, delta: i32) {
        let idx = match self.store {
            PinCreateStore::Auto => 0,
            PinCreateStore::Project => 1,
            PinCreateStore::User => 2,
        };
        self.store = match ((idx + delta) % 3 + 3) % 3 {
            0 => PinCreateStore::Auto,
            1 => PinCreateStore::Project,
            _ => PinCreateStore::User,
        };
    }

    fn cycle_harness(&mut self, delta: i32) {
        if self.known_harness_keys.is_empty() {
            return;
        }
        let value = self.harness.value().trim();
        let idx = match self
            .known_harness_keys
            .iter()
            .position(|known| known == value)
        {
            Some(idx) => {
                let len = self.known_harness_keys.len() as i32;
                ((idx as i32 + delta) % len + len) % len
            }
            None if delta < 0 => self.known_harness_keys.len().saturating_sub(1) as i32,
            None => 0,
        } as usize;
        self.harness = TextInputState::new(" harness ", self.known_harness_keys[idx].clone());
    }

    fn active_input_mut(&mut self) -> Option<&mut TextInputState> {
        match self.logical_cursor() {
            0 => Some(&mut self.name),
            2 => Some(&mut self.cwd),
            3 => Some(&mut self.harness),
            4 => Some(&mut self.launch_argv),
            5 => Some(&mut self.id),
            6 => Some(&mut self.display_name),
            7 => Some(&mut self.mux_name),
            8 => Some(&mut self.mux_socket),
            _ => None,
        }
    }

    fn active_input(&self) -> Option<&TextInputState> {
        match self.logical_cursor() {
            0 => Some(&self.name),
            2 => Some(&self.cwd),
            3 => Some(&self.harness),
            4 => Some(&self.launch_argv),
            5 => Some(&self.id),
            6 => Some(&self.display_name),
            7 => Some(&self.mux_name),
            8 => Some(&self.mux_socket),
            _ => None,
        }
    }

    fn after_active_input_changed(&mut self, active: usize) {
        match if self.can_toggle_mode() || active == 0 {
            active
        } else {
            active + 1
        } {
            0 => {
                if self.mode == PinCreateMode::AdoptSelected && self.adopt_auto_uncheck_armed {
                    self.mode = PinCreateMode::NewVariation;
                    self.adopt_auto_uncheck_armed = false;
                }
                self.sync_from_name();
                self.sync_mode_from_mux_collision();
            }
            5 => self.id_overridden = !self.id.value().is_empty(),
            6 => self.display_overridden = !self.display_name.value().is_empty(),
            7 => {
                self.mux_overridden = !self.mux_name.value().is_empty();
                self.sync_mode_from_mux_collision();
            }
            _ => {}
        }
    }

    fn sync_mode_from_mux_collision(&mut self) {
        let matches_known_mux = self.matching_known_mux_name().is_some();
        if matches_known_mux {
            self.mode = PinCreateMode::AdoptSelected;
            self.adopt_auto_uncheck_armed = false;
            self.adopt_auto_checked_by_collision = true;
        } else if self.adopt_auto_checked_by_collision {
            self.mode = PinCreateMode::NewVariation;
            self.adopt_auto_checked_by_collision = false;
            if !self.mux_overridden {
                self.mux_name = TextInputState::new(
                    " mux ",
                    derived_mux_name_for_mode(
                        self.mode,
                        self.name.value(),
                        &pin_id_candidate(self.name.value()),
                    ),
                );
            }
        }
    }

    fn sync_from_name(&mut self) {
        let name = self.name.value().to_string();
        let derived_id = pin_id_candidate(&name);
        if !self.id_overridden {
            self.id = TextInputState::new(" id ", derived_id.clone());
        }
        if !self.display_overridden {
            self.display_name = TextInputState::new(" display ", name);
        }
        if !self.mux_overridden {
            self.mux_name = TextInputState::new(
                " mux ",
                derived_mux_name_for_mode(self.mode, self.name.value(), &derived_id),
            );
        }
    }

    fn request(&self) -> Result<PinCreateRequest, String> {
        let id = required(self.id.value(), "id")?;
        let harness = required(self.harness.value(), "harness")?;
        let cwd = required(self.cwd.value(), "cwd")?;
        let display_name = optional(self.display_name.value()).unwrap_or_else(|| id.clone());
        let mux_name = optional(self.mux_name.value()).unwrap_or_else(|| display_name.clone());
        let mux_socket = optional(self.mux_socket.value());
        let launch_argv = optional(self.launch_argv.value())
            .map(|raw| raw.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        Ok(PinCreateRequest {
            id,
            display_name,
            harness,
            cwd,
            mux_name,
            mux_socket,
            adopt_source_mux_name: self.adopt_source_mux_name(),
            launch_argv,
            store: self.store,
        })
    }

    fn adopt_source_mux_name(&self) -> Option<String> {
        if self.mode != PinCreateMode::AdoptSelected {
            return None;
        }
        if let Some(mux_name) = self.matching_known_mux_name() {
            return Some(mux_name.to_string());
        }
        self.adopt_defaults
            .as_ref()
            .map(|defaults| defaults.mux_name.clone())
            .filter(|source| !source.trim().is_empty())
    }

    fn matching_known_mux_name(&self) -> Option<&str> {
        let mux_name = self.mux_name.value().trim();
        if mux_name.is_empty() {
            return None;
        }
        self.known_mux_names
            .iter()
            .find(|known| known.as_str() == mux_name)
            .map(String::as_str)
    }

    fn mux_name_display(&self) -> String {
        let value = self.mux_name.value();
        match self.adopt_source_mux_name() {
            Some(source) if source != value => format!("{value} (rename of: {source})"),
            _ => value.to_string(),
        }
    }

    fn harness_warning(&self) -> Option<String> {
        let value = self.harness.value().trim();
        if value.is_empty() || self.known_harness_keys.iter().any(|known| known == value) {
            None
        } else {
            Some(format!("custom harness `{value}` will be saved as typed"))
        }
    }
}

fn normalized_harness_keys<'a>(
    known_harness_keys: Vec<String>,
    extra_keys: impl IntoIterator<Item = &'a String>,
) -> Vec<String> {
    let mut keys: Vec<String> = known_harness_keys
        .into_iter()
        .chain(extra_keys.into_iter().cloned())
        .filter_map(|key| {
            let key = key.trim();
            (!key.is_empty()).then(|| key.to_string())
        })
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

fn pin_id_candidate(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "new-pin".to_string()
    } else {
        trimmed
    }
}

fn derived_mux_name_for_mode(mode: PinCreateMode, name: &str, derived_id: &str) -> String {
    match mode {
        PinCreateMode::NewVariation => derived_id.to_string(),
        PinCreateMode::AdoptSelected => name.to_string(),
    }
}

impl PinEditState {
    const FIELD_COUNT: usize = 5;

    fn new(target: PinMutationTarget) -> Self {
        Self {
            cursor: 0,
            id: TextInputState::new(" id ", target.id.clone()),
            display_name: TextInputState::new(" display ", target.display_name.clone()),
            mux_name: TextInputState::new(" mux ", target.mux_name.clone()),
            mux_socket: TextInputState::new(
                " socket ",
                target.mux_socket.clone().unwrap_or_default(),
            ),
            launch_argv: TextInputState::new(" launch argv ", target.launch_argv.join(" ")),
            target,
            error: None,
        }
    }

    fn handle_key(&mut self, event: KeyEvent) -> PinEditOutcome {
        match event.code {
            KeyCode::Esc => PinEditOutcome::Cancel,
            KeyCode::Enter => match self.request() {
                Ok(request) => PinEditOutcome::Confirm(Box::new(request)),
                Err(err) => {
                    self.error = Some(err);
                    PinEditOutcome::Continue
                }
            },
            KeyCode::Up => {
                self.move_cursor(-1);
                PinEditOutcome::Continue
            }
            KeyCode::Down | KeyCode::Tab => {
                self.move_cursor(1);
                PinEditOutcome::Continue
            }
            KeyCode::BackTab => {
                self.move_cursor(-1);
                PinEditOutcome::Continue
            }
            _ => {
                if event.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(event.code, KeyCode::Char('c'))
                {
                    return PinEditOutcome::Cancel;
                }
                if let Some(input) = self.active_input_mut() {
                    let _ = input.handle_key(event);
                    self.error = None;
                }
                PinEditOutcome::Continue
            }
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        let len = Self::FIELD_COUNT as i32;
        let next = ((self.cursor as i32 + delta) % len + len) % len;
        self.cursor = next as usize;
    }

    fn active_input_mut(&mut self) -> Option<&mut TextInputState> {
        match self.cursor {
            0 => Some(&mut self.id),
            1 => Some(&mut self.display_name),
            2 => Some(&mut self.mux_name),
            3 => Some(&mut self.mux_socket),
            4 => Some(&mut self.launch_argv),
            _ => None,
        }
    }

    fn request(&self) -> Result<PinEditRequest, String> {
        let id = required(self.id.value(), "id")?;
        let display_name = required(self.display_name.value(), "display")?;
        let mux_name = required(self.mux_name.value(), "mux.name")?;
        let mux_socket = optional(self.mux_socket.value());
        let launch_argv = optional(self.launch_argv.value())
            .map(|raw| raw.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        Ok(PinEditRequest {
            original_id: self.target.id.clone(),
            id,
            display_name,
            harness: self.target.harness.clone(),
            cwd: self.target.cwd.clone(),
            mux_name,
            mux_socket,
            launch_argv,
            store_path: self.target.store_path.clone(),
        })
    }
}

impl PinBindState {
    fn new(options: Vec<PinBindOption>) -> Self {
        Self { cursor: 0, options }
    }

    fn handle_key(&mut self, event: KeyEvent) -> PinBindOutcome {
        match event.code {
            KeyCode::Esc => PinBindOutcome::Cancel,
            KeyCode::Enter => {
                let Some(option) = self.options.get(self.cursor) else {
                    return PinBindOutcome::Continue;
                };
                PinBindOutcome::Confirm(PinBindRequest {
                    pin_id: option.pin_id.clone(),
                    session_key: option.session_key.clone(),
                })
            }
            KeyCode::Up => {
                self.move_cursor(-1);
                PinBindOutcome::Continue
            }
            KeyCode::Down | KeyCode::Tab => {
                self.move_cursor(1);
                PinBindOutcome::Continue
            }
            KeyCode::BackTab => {
                self.move_cursor(-1);
                PinBindOutcome::Continue
            }
            _ if event.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(event.code, KeyCode::Char('c')) =>
            {
                PinBindOutcome::Cancel
            }
            _ => PinBindOutcome::Continue,
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        if self.options.is_empty() {
            return;
        }
        let len = self.options.len() as i32;
        let next = ((self.cursor as i32 + delta) % len + len) % len;
        self.cursor = next as usize;
    }
}

impl PinRemoveState {
    fn new(target: PinMutationTarget) -> Self {
        Self { target }
    }

    fn handle_key(&mut self, event: KeyEvent) -> PinRemoveOutcome {
        match event.code {
            KeyCode::Esc => PinRemoveOutcome::Cancel,
            KeyCode::Enter => PinRemoveOutcome::Confirm(PinRemoveRequest {
                id: self.target.id.clone(),
                display_name: self.target.display_name.clone(),
                store_path: self.target.store_path.clone(),
            }),
            _ if event.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(event.code, KeyCode::Char('c')) =>
            {
                PinRemoveOutcome::Cancel
            }
            _ => PinRemoveOutcome::Continue,
        }
    }
}

impl PinRebindState {
    const FIELD_COUNT: usize = 2;

    fn new(target: PinMutationTarget) -> Self {
        Self {
            cursor: 0,
            mux_name: TextInputState::new(" mux ", target.mux_name.clone()),
            mux_socket: TextInputState::new(
                " socket ",
                target.mux_socket.clone().unwrap_or_default(),
            ),
            error: None,
            target,
        }
    }

    fn handle_key(&mut self, event: KeyEvent) -> PinEditOutcome {
        match event.code {
            KeyCode::Esc => PinEditOutcome::Cancel,
            KeyCode::Enter => match self.request() {
                Ok(request) => PinEditOutcome::Confirm(Box::new(request)),
                Err(err) => {
                    self.error = Some(err);
                    PinEditOutcome::Continue
                }
            },
            KeyCode::Up => {
                self.move_cursor(-1);
                PinEditOutcome::Continue
            }
            KeyCode::Down | KeyCode::Tab => {
                self.move_cursor(1);
                PinEditOutcome::Continue
            }
            KeyCode::BackTab => {
                self.move_cursor(-1);
                PinEditOutcome::Continue
            }
            _ => {
                if event.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(event.code, KeyCode::Char('c'))
                {
                    return PinEditOutcome::Cancel;
                }
                if let Some(input) = self.active_input_mut() {
                    let _ = input.handle_key(event);
                    self.error = None;
                }
                PinEditOutcome::Continue
            }
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        let len = Self::FIELD_COUNT as i32;
        let next = ((self.cursor as i32 + delta) % len + len) % len;
        self.cursor = next as usize;
    }

    fn active_input_mut(&mut self) -> Option<&mut TextInputState> {
        match self.cursor {
            0 => Some(&mut self.mux_name),
            1 => Some(&mut self.mux_socket),
            _ => None,
        }
    }

    fn request(&self) -> Result<PinEditRequest, String> {
        let mux_name = required(self.mux_name.value(), "mux.name")?;
        let mux_socket = optional(self.mux_socket.value());
        // Rebind only swaps the mux target; every other field is
        // preserved verbatim so the runtime's shared write path
        // can treat this as an ordinary edit.
        Ok(PinEditRequest {
            original_id: self.target.id.clone(),
            id: self.target.id.clone(),
            display_name: self.target.display_name.clone(),
            harness: self.target.harness.clone(),
            cwd: self.target.cwd.clone(),
            mux_name,
            mux_socket,
            launch_argv: self.target.launch_argv.clone(),
            store_path: self.target.store_path.clone(),
        })
    }
}

fn required(raw: &str, label: &str) -> Result<String, String> {
    optional(raw).ok_or_else(|| format!("pin create: {label} is required"))
}

fn optional(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Centered modal widget for the pins overlay.
pub struct PinsOverlayWidget<'a> {
    state: &'a PinsOverlayState,
    theme: &'a Theme,
}

impl<'a> PinsOverlayWidget<'a> {
    pub fn new(state: &'a PinsOverlayState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinsOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // H-WIDG-004: framing through `tui_popup::Popup`.
        let cursor = self.state.cursor();
        let mut lines: Vec<Line<'static>> = Vec::new();
        for (idx, label) in PIN_ACTION_OPTIONS.iter().enumerate() {
            let row = PinsCursor::Action(idx);
            lines.push(row_line((*label).to_string(), cursor == row));
        }
        lines.push(line![""]);
        lines.push(line![
            span!(Modifier::DIM; "↑/↓ move · Enter pick · Esc close")
        ]);
        let modal = centered_modal_rect(area);

        let sub_editor_to_render = self.state.sub_editor();
        let theme = self.theme;
        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                pins_menu_cursor_line(cursor),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        let popup = themed_popup(body, line![" Pins "], theme);
        popup.render(area, buf);

        if let Some(editor) = sub_editor_to_render {
            render_sub_editor(editor, area, buf, theme);
        }
    }
}

fn render_sub_editor(editor: &PinsSubEditor, area: Rect, buf: &mut Buffer, theme: &Theme) {
    match editor {
        PinsSubEditor::Create(state) => PinCreateWidget::new(state, theme).render(area, buf),
        PinsSubEditor::Edit(state) => PinEditWidget::new(state, theme).render(area, buf),
        PinsSubEditor::Rebind(state) => PinRebindWidget::new(state, theme).render(area, buf),
        PinsSubEditor::Bind(state) => PinBindWidget::new(state, theme).render(area, buf),
        PinsSubEditor::Remove(state) => PinRemoveWidget::new(state, theme).render(area, buf),
    }
}

struct PinCreateWidget<'a> {
    state: &'a PinCreateState,
    theme: &'a Theme,
}

impl<'a> PinCreateWidget<'a> {
    fn new(state: &'a PinCreateState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinCreateWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // H-WIDG-004: framing through `tui_popup::Popup`.
        let cursor = self.state.render_cursor();
        let content_lines = 14 + usize::from(self.state.error.is_some()) * 2;
        let modal = pin_create_modal_rect(area, content_lines);
        let inner_width = modal.width.saturating_sub(2) as usize;
        let mut lines = vec![
            pin_create_input_field(0, "name", &self.state.name, cursor, inner_width),
            pin_create_mode_field(self.state, cursor, inner_width),
            pin_create_input_field(2, "cwd", &self.state.cwd, cursor, inner_width),
            pin_create_harness_field(self.state, cursor, inner_width),
            pin_create_input_field(
                4,
                "launch argv",
                &self.state.launch_argv,
                cursor,
                inner_width,
            ),
            line![""],
            line![span!(Modifier::DIM; "Advanced identity")],
            pin_create_input_field(5, "id", &self.state.id, cursor, inner_width),
            pin_create_input_field(6, "display", &self.state.display_name, cursor, inner_width),
            pin_create_value_field(
                7,
                "mux.name",
                &self.state.mux_name_display(),
                self.state.mux_name.cursor(),
                cursor,
                inner_width,
            ),
            pin_create_input_field(8, "mux.socket", &self.state.mux_socket, cursor, inner_width),
            pin_create_store_field(self.state.store, cursor, inner_width),
        ];
        if let Some(error) = &self.state.error {
            lines.push(line![""]);
            lines.push(line![span!(Modifier::BOLD; "{}", error.clone())]);
        }
        lines.push(line![""]);
        lines.push(line![span!(
            Modifier::DIM;
            "↑/↓/Tab move · S-Tab back · ←/→/Space option · Enter create · Esc cancel"
        )]);

        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                Some(cursor),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Create Pin "], self.theme).render(area, buf);
    }
}

fn pin_create_input_field(
    idx: usize,
    label: &'static str,
    input: &TextInputState,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    pin_create_value_field(
        idx,
        label,
        input.value(),
        input.cursor(),
        cursor,
        inner_width,
    )
}

fn pin_create_value_field(
    idx: usize,
    label: &'static str,
    value: &str,
    value_cursor: usize,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    let active = cursor == idx;
    let marker = if active { "> " } else { "  " };
    let label_style = if active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let prefix = format!("{marker}{label:11} ");
    let value_width = inner_width.saturating_sub(prefix.chars().count()).max(1);
    let display = pin_field_visible_window(value, value_cursor, value_width, active);
    line![
        span!(label_style; "{prefix}"),
        span!(display.edge_style; "{}", display.left_indicator),
        span!("{}", display.before_cursor),
        span!(display.cursor_style; "{}", display.cursor_text),
        span!("{}", display.after_cursor),
        span!(display.edge_style; "{}", display.right_indicator),
    ]
}

fn pin_create_mode_field(
    state: &PinCreateState,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    if state.can_toggle_mode() {
        option_pair_line(
            cursor == 1,
            "mode",
            "new",
            state.mode == PinCreateMode::NewVariation,
            "adopt selected",
            state.mode == PinCreateMode::AdoptSelected,
            inner_width,
        )
    } else {
        pin_create_static_field(1, "mode", state.mode.label(), cursor, inner_width)
    }
}

fn pin_create_harness_field(
    state: &PinCreateState,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    const HARNESS_VALUE_SLOT_WIDTH: usize = 18;

    let active = cursor == 3;
    let marker = if active { "> " } else { "  " };
    let label_style = if active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let prefix = format!("{marker}{:11} ", "harness");
    let available = inner_width.saturating_sub(prefix.chars().count());
    if state.harness_warning().is_some() {
        return pin_create_harness_custom_field(state, active, label_style, prefix, available);
    }
    let value_width = HARNESS_VALUE_SLOT_WIDTH.min(available).max(1);
    let suffix_budget = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(value_width)
        .min(48);
    let suffix = pin_create_harness_suffix(state, suffix_budget);
    let display = pin_field_visible_window(
        state.harness.value(),
        state.harness.cursor(),
        value_width,
        active,
    );
    let mut spans = vec![
        span!(label_style; "{prefix}"),
        span!(display.edge_style; "{}", display.left_indicator),
        span!("{}", display.before_cursor),
        span!(display.cursor_style; "{}", display.cursor_text),
        span!("{}", display.after_cursor),
        span!(display.edge_style; "{}", display.right_indicator),
    ];
    let display_width = display.left_indicator.chars().count()
        + display.before_cursor.chars().count()
        + display.cursor_text.chars().count()
        + display.after_cursor.chars().count()
        + display.right_indicator.chars().count();
    let padding = value_width.saturating_sub(display_width);
    if padding > 0 {
        spans.push(span!("{}", " ".repeat(padding)));
    }
    spans.extend(suffix);
    Line::from(spans)
}

fn pin_create_harness_custom_field(
    state: &PinCreateState,
    active: bool,
    label_style: Style,
    prefix: String,
    available: usize,
) -> Line<'static> {
    let suffix = vec![span!(Style::default().fg(Color::Yellow); " [custom]")];
    let suffix_width = span_width(&suffix);
    let value_width = available.saturating_sub(suffix_width).max(1);
    let display = pin_field_visible_window(
        state.harness.value(),
        state.harness.cursor(),
        value_width,
        active,
    );
    let mut spans = vec![
        span!(label_style; "{prefix}"),
        span!(display.edge_style; "{}", display.left_indicator),
        span!("{}", display.before_cursor),
        span!(display.cursor_style; "{}", display.cursor_text),
        span!("{}", display.after_cursor),
        span!(display.edge_style; "{}", display.right_indicator),
    ];
    spans.extend(suffix);
    Line::from(spans)
}

fn pin_create_harness_suffix(state: &PinCreateState, width: usize) -> Vec<Span<'static>> {
    if width < 3 {
        return Vec::new();
    }
    if state.known_harness_keys.is_empty() {
        return Vec::new();
    }
    let selected = state.harness.value().trim();
    let mut spans = Vec::new();
    let mut remaining = width;
    for key in &state.known_harness_keys {
        let token = format!(" [{key}]");
        let needed = token.chars().count();
        if needed > remaining {
            if remaining >= 2 {
                spans.push(span!(Modifier::DIM; " …"));
            }
            break;
        }
        if key == selected {
            spans.push(
                span!(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD); "{}", token),
            );
        } else {
            spans.push(span!(Modifier::DIM; "{}", token));
        }
        remaining = remaining.saturating_sub(needed);
    }
    spans
}

fn span_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

fn pin_create_store_field(
    store: PinCreateStore,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    let marker = if cursor == 9 { "> " } else { "  " };
    let label_style = if cursor == 9 {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let raw = format!(
        "{marker}{:11} {} {} {}",
        "store",
        option_token("auto", store == PinCreateStore::Auto),
        option_token("project", store == PinCreateStore::Project),
        option_token("user", store == PinCreateStore::User),
    );
    line![span!(label_style; "{}", truncate_chars(&raw, inner_width))]
}

fn pin_create_static_field(
    idx: usize,
    label: &'static str,
    value: &str,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    let marker = if cursor == idx { "> " } else { "  " };
    let style = if cursor == idx {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let value = if value.trim().is_empty() { "-" } else { value };
    let raw = format!("{marker}{label:11} {value}");
    line![span!(style; "{}", truncate_chars(&raw, inner_width))]
}

fn option_pair_line(
    active: bool,
    label: &'static str,
    left: &'static str,
    left_selected: bool,
    right: &'static str,
    right_selected: bool,
    inner_width: usize,
) -> Line<'static> {
    let marker = if active { "> " } else { "  " };
    let style = if active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let raw = format!(
        "{marker}{label:11} {} {}",
        option_token(left, left_selected),
        option_token(right, right_selected)
    );
    line![span!(style; "{}", truncate_chars(&raw, inner_width))]
}

fn option_token(label: &str, selected: bool) -> String {
    if selected {
        format!("[x] {label}")
    } else {
        format!("[ ] {label}")
    }
}

fn pin_create_field(idx: usize, label: &'static str, value: &str, cursor: usize) -> Line<'static> {
    pin_create_static_field(idx, label, value, cursor, usize::MAX / 2)
}

struct PinFieldVisibleWindow {
    left_indicator: &'static str,
    before_cursor: String,
    cursor_text: String,
    after_cursor: String,
    right_indicator: &'static str,
    edge_style: Style,
    cursor_style: Style,
}

fn pin_field_visible_window(
    value: &str,
    cursor: usize,
    width: usize,
    active: bool,
) -> PinFieldVisibleWindow {
    let width = width.max(1);
    let edge_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let cursor_style = if active {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    let value = if value.trim().is_empty() { "-" } else { value };
    let chars: Vec<char> = value.chars().collect();
    let total = chars.len();
    let cursor = cursor.min(total);

    if width == 1 {
        let ch = chars.get(cursor).copied().unwrap_or(' ');
        return PinFieldVisibleWindow {
            left_indicator: if cursor > 0 { "<" } else { " " },
            before_cursor: String::new(),
            cursor_text: ch.to_string(),
            after_cursor: String::new(),
            right_indicator: if cursor + usize::from(cursor < total) < total {
                ">"
            } else {
                " "
            },
            edge_style,
            cursor_style,
        };
    }

    let content_width = width.saturating_sub(4).max(1);
    let start = if total <= content_width {
        0
    } else if cursor >= content_width {
        cursor.saturating_sub(content_width.saturating_sub(1))
    } else {
        0
    };
    let end = (start + content_width).min(total);
    let cursor_offset = cursor.saturating_sub(start).min(content_width);
    let before_cursor: String = chars[start..(start + cursor_offset).min(end)]
        .iter()
        .collect();
    let cursor_text = if cursor < end {
        chars[start + cursor_offset].to_string()
    } else if active {
        " ".to_string()
    } else {
        String::new()
    };
    let after_start = (start + cursor_offset + usize::from(cursor < end)).min(end);
    let after_cursor: String = chars[after_start..end].iter().collect();

    PinFieldVisibleWindow {
        left_indicator: if start > 0 { "< " } else { "  " },
        before_cursor,
        cursor_text,
        after_cursor,
        right_indicator: if end < total { " >" } else { "  " },
        edge_style,
        cursor_style,
    }
}

fn truncate_chars(value: &str, width: usize) -> String {
    if width == usize::MAX / 2 {
        return value.to_string();
    }
    value.chars().take(width).collect()
}

fn pin_create_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(76, area.width.saturating_sub(4)).max(44);
    modal_rect_for_content(area, width, content_lines, 10)
}

struct PinEditWidget<'a> {
    state: &'a PinEditState,
    theme: &'a Theme,
}

impl<'a> PinEditWidget<'a> {
    fn new(state: &'a PinEditState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinEditWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // H-WIDG-004: framing through `tui_popup::Popup`.
        let mut lines = vec![
            pin_create_field(0, "id", self.state.id.value(), self.state.cursor),
            pin_create_field(
                1,
                "display",
                self.state.display_name.value(),
                self.state.cursor,
            ),
            pin_create_field(
                2,
                "mux.name",
                self.state.mux_name.value(),
                self.state.cursor,
            ),
            pin_create_field(
                3,
                "mux.socket",
                self.state.mux_socket.value(),
                self.state.cursor,
            ),
            pin_create_field(
                4,
                "launch argv",
                self.state.launch_argv.value(),
                self.state.cursor,
            ),
            line![format!("  harness     {}", self.state.target.harness)],
            line![format!("  cwd         {}", self.state.target.cwd)],
            line![format!("  store       {}", self.state.target.store_path)],
        ];
        if let Some(error) = &self.state.error {
            lines.push(line![""]);
            lines.push(line![span!(Modifier::BOLD; "{}", error.clone())]);
        }
        lines.push(line![""]);
        lines.push(line![span!(
            Modifier::DIM;
            "Up/Down field · type to edit · Enter save · Esc cancel"
        )]);
        let modal = pin_edit_modal_rect(area, lines.len());

        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                Some(self.state.cursor),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Edit Pin "], self.theme).render(area, buf);
    }
}

struct PinRebindWidget<'a> {
    state: &'a PinRebindState,
    theme: &'a Theme,
}

impl<'a> PinRebindWidget<'a> {
    fn new(state: &'a PinRebindState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinRebindWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // H-WIDG-004: framing through `tui_popup::Popup`.
        let mut lines = vec![
            line![format!("  id          {}", self.state.target.id)],
            line![format!("  display     {}", self.state.target.display_name)],
            pin_create_field(
                0,
                "mux.name",
                self.state.mux_name.value(),
                self.state.cursor,
            ),
            pin_create_field(
                1,
                "mux.socket",
                self.state.mux_socket.value(),
                self.state.cursor,
            ),
            line![format!("  store       {}", self.state.target.store_path)],
        ];
        if let Some(error) = &self.state.error {
            lines.push(line![""]);
            lines.push(line![span!(Modifier::BOLD; "{}", error.clone())]);
        }
        lines.push(line![""]);
        lines.push(line![span!(
            Modifier::DIM;
            "Up/Down field · type to edit · Enter save · Esc cancel"
        )]);
        let modal = pin_rebind_modal_rect(area, lines.len());

        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                Some(2 + self.state.cursor),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Rebind Pin "], self.theme).render(area, buf);
    }
}

fn pin_rebind_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(70, area.width.saturating_sub(4)).max(44);
    modal_rect_for_content(area, width, content_lines, 9)
}

fn pin_edit_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(78, area.width.saturating_sub(4)).max(46);
    modal_rect_for_content(area, width, content_lines, 10)
}

struct PinBindWidget<'a> {
    state: &'a PinBindState,
    theme: &'a Theme,
}

impl<'a> PinBindWidget<'a> {
    fn new(state: &'a PinBindState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinBindWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // H-WIDG-004: framing through `tui_popup::Popup`.
        let mut lines = Vec::new();
        if let Some(first) = self.state.options.first() {
            lines.push(line![format!("pin       {}", first.pin_id)]);
        }
        for (idx, option) in self.state.options.iter().enumerate() {
            let marker = if idx == self.state.cursor { "> " } else { "  " };
            let style = if idx == self.state.cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(line![span!(style; "{marker}{}", option.label)]);
        }
        lines.push(line![""]);
        lines.push(line![span!(
            Modifier::DIM;
            "Up/Down choose · Enter bind · Esc cancel"
        )]);
        let modal = pin_bind_modal_rect(area, lines.len());
        let option_start = usize::from(!self.state.options.is_empty());

        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                Some(option_start + self.state.cursor),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Bind Pin "], self.theme).render(area, buf);
    }
}

fn pin_bind_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(76, area.width.saturating_sub(4)).max(44);
    modal_rect_for_content(area, width, content_lines, 7)
}

struct PinRemoveWidget<'a> {
    state: &'a PinRemoveState,
    theme: &'a Theme,
}

impl<'a> PinRemoveWidget<'a> {
    fn new(state: &'a PinRemoveState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinRemoveWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // H-WIDG-004: framing through `tui_popup::Popup`.
        let lines = vec![
            line![
                "id       ",
                span!(Modifier::BOLD; "{}", self.state.target.id.clone()),
            ],
            line![format!("display  {}", self.state.target.display_name)],
            line![format!("store    {}", self.state.target.store_path)],
            line![""],
            line![span!(Modifier::DIM; "Enter remove · Esc cancel")],
        ];
        let modal = pin_remove_modal_rect(area, lines.len());
        let body = ScrollLinesBody {
            scroll_offset: 0,
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Remove Pin "], self.theme).render(area, buf);
    }
}

fn pin_remove_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(76, area.width.saturating_sub(4)).max(44);
    modal_rect_for_content(area, width, content_lines, 7)
}

fn row_line(label: String, cursored: bool) -> Line<'static> {
    let marker = if cursored { "> " } else { "  " };
    let mut style = Style::default();
    if cursored {
        style = style.add_modifier(Modifier::REVERSED);
    }
    line![span!(style; "{marker}{label}")]
}

fn centered_modal_rect(area: Rect) -> Rect {
    centered_modal_rect_for_content(area, pins_menu_content_lines())
}

fn centered_modal_rect_for_content(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(50, area.width.saturating_sub(4)).max(32);
    modal_rect_for_content(area, width, content_lines, 8)
}

fn modal_rect_for_content(area: Rect, width: u16, content_lines: usize, min_height: u16) -> Rect {
    let max_height = area.height;
    let desired = content_lines.saturating_add(2) as u16;
    let height = desired.clamp(min_height, max_height.max(min_height));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn pins_menu_content_lines() -> usize {
    PIN_ACTION_OPTIONS.len() + 2
}

fn pins_menu_cursor_line(cursor: PinsCursor) -> Option<usize> {
    let PinsCursor::Action(idx) = cursor;
    Some(idx)
}

struct ScrollLinesBody {
    lines: Vec<Line<'static>>,
    inner_width: usize,
    inner_height: usize,
    scroll_offset: u16,
}

impl KnownSize for ScrollLinesBody {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for ScrollLinesBody {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines)
            .scroll((self.scroll_offset, 0))
            .render(area, buf);
    }
}

fn scroll_offset_for_cursor(
    cursor_line: Option<usize>,
    inner_height: usize,
    content_height: usize,
) -> u16 {
    let Some(cursor_line) = cursor_line else {
        return 0;
    };
    if inner_height == 0 || cursor_line < inner_height {
        return 0;
    }
    let max_scroll = content_height.saturating_sub(inner_height);
    cursor_line
        .saturating_sub(inner_height.saturating_sub(1))
        .min(max_scroll) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn shift_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn pin_target() -> PinMutationTarget {
        PinMutationTarget {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest-mux".to_string(),
            mux_socket: Some("scratch".to_string()),
            launch_argv: vec!["codex".to_string(), "--resume".to_string()],
            store_path: "/workspace/project/.conspectus.toml".to_string(),
        }
    }

    #[test]
    fn opens_at_first_action() {
        let state = PinsOverlayState::new();
        assert_eq!(state.cursor(), PinsCursor::Action(0));
    }

    #[test]
    fn arrow_keys_wrap_around_action_list() {
        let ctx = PinsContext::default();
        let mut state = PinsOverlayState::new();
        for _ in 0..PIN_ACTION_OPTIONS.len() {
            state.handle_key(&ctx, key(KeyCode::Down));
        }
        assert_eq!(state.cursor(), PinsCursor::Action(0));
    }

    #[test]
    fn pins_menu_content_count_tracks_rendered_body() {
        let mut lines: Vec<Line<'static>> = Vec::new();
        for label in PIN_ACTION_OPTIONS {
            lines.push(row_line((*label).to_string(), false));
        }
        lines.push(Line::default());
        lines.push(Line::from(span!(
            Modifier::DIM;
            "↑/↓ move · Enter pick · Esc close"
        )));
        assert_eq!(pins_menu_content_lines(), lines.len());
    }

    #[test]
    fn pins_menu_has_single_create_entry_without_adopt_peer() {
        assert!(PIN_ACTION_OPTIONS.contains(&"create"));
        assert!(!PIN_ACTION_OPTIONS.contains(&"adopt"));
    }

    #[test]
    fn selected_last_pin_action_scrolls_into_short_menu_body() {
        let cursor_line = pins_menu_cursor_line(PinsCursor::Action(PIN_ACTION_OPTIONS.len() - 1));
        let inner_height = 4;
        let offset = scroll_offset_for_cursor(cursor_line, inner_height, pins_menu_content_lines());
        let cursor_line = cursor_line.unwrap();
        assert!(offset > 0, "short pins menu should scroll");
        assert!(cursor_line >= offset as usize);
        assert!(cursor_line < offset as usize + inner_height);
    }

    #[test]
    fn esc_at_top_level_closes_overlay() {
        let ctx = PinsContext::default();
        let mut state = PinsOverlayState::new();
        let outcome = state.handle_key(&ctx, key(KeyCode::Esc));
        assert_eq!(outcome, PinsOutcome::Close);
    }

    #[test]
    fn enter_on_create_opens_create_editor() {
        let ctx = PinsContext {
            pin_create_defaults: PinCreateDefaults {
                id: "ingest".to_string(),
                display_name: "Ingest".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "ingest".to_string(),
                ..PinCreateDefaults::default()
            },
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, PinsOutcome::Continue);
        assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Create(_))));
    }

    #[test]
    fn create_form_starts_adopt_checked_then_auto_unchecks_on_first_name_edit() {
        let ctx = PinsContext {
            pin_create_defaults: PinCreateDefaults {
                id: "agentdeck-conspectus-2".to_string(),
                display_name: "agentdeck-conspectus-2".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "agentdeck-conspectus-2".to_string(),
                mode: PinCreateMode::NewVariation,
            },
            pin_adopt_defaults: Some(PinCreateDefaults {
                id: "agentdeck-conspectus".to_string(),
                display_name: "agentdeck_conspectus".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "agentdeck_conspectus".to_string(),
                mode: PinCreateMode::AdoptSelected,
            }),
            known_mux_names: vec!["agentdeck_conspectus".to_string()],
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.handle_key(&ctx, key(KeyCode::Enter));

        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
                assert_eq!(editor.name.value(), "agentdeck_conspectus");
                assert_eq!(editor.mux_name.value(), "agentdeck_conspectus");
                assert_eq!(editor.mux_name_display(), "agentdeck_conspectus");
                let request = editor.request().expect("valid create request");
                assert_eq!(request.mux_name, "agentdeck_conspectus");
                assert_eq!(
                    request.adopt_source_mux_name.as_deref(),
                    Some("agentdeck_conspectus")
                );
            }
            other => panic!("unexpected editor: {other:?}"),
        }

        for _ in 0.."agentdeck_conspectus".len() {
            state.handle_key(&ctx, key(KeyCode::Backspace));
        }
        for ch in "agentdeck-conspectus-v2".chars() {
            state.handle_key(&ctx, key(KeyCode::Char(ch)));
        }

        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(editor.mode, PinCreateMode::NewVariation);
                assert_eq!(editor.name.value(), "agentdeck-conspectus-v2");
                assert_eq!(editor.id.value(), "agentdeck-conspectus-v2");
                assert_eq!(editor.display_name.value(), "agentdeck-conspectus-v2");
                assert_eq!(editor.mux_name.value(), "agentdeck-conspectus-v2");
                assert_eq!(editor.mux_name_display(), "agentdeck-conspectus-v2");
                assert_eq!(
                    editor
                        .request()
                        .expect("valid request")
                        .adopt_source_mux_name,
                    None
                );
            }
            other => panic!("unexpected editor: {other:?}"),
        }

        state.handle_key(&ctx, key(KeyCode::Down));
        state.handle_key(&ctx, key(KeyCode::Char(' ')));
        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
                assert_eq!(
                    editor.mux_name_display(),
                    "agentdeck-conspectus-v2 (rename of: agentdeck_conspectus)"
                );
            }
            other => panic!("unexpected editor: {other:?}"),
        }

        state.handle_key(&ctx, key(KeyCode::Up));
        for _ in 0.."agentdeck-conspectus-v2".len() {
            state.handle_key(&ctx, key(KeyCode::Backspace));
        }
        for ch in "agentdeck-conspectus-v3".chars() {
            state.handle_key(&ctx, key(KeyCode::Char(ch)));
        }
        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
                assert_eq!(
                    editor.mux_name_display(),
                    "agentdeck-conspectus-v3 (rename of: agentdeck_conspectus)"
                );
            }
            other => panic!("unexpected editor: {other:?}"),
        }
    }

    #[test]
    fn direct_adopt_opens_create_form_with_adopt_selected_and_toggleable() {
        let defaults = PinCreateDefaults {
            id: "work-2".to_string(),
            display_name: "work-2".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "work-2".to_string(),
            mode: PinCreateMode::NewVariation,
        };
        let adopt = PinCreateDefaults {
            id: "work".to_string(),
            display_name: "work".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "work".to_string(),
            mode: PinCreateMode::AdoptSelected,
        };
        let mut state =
            PinsOverlayState::open_with_adopt_options(defaults, Some(adopt), vec![], vec![]);

        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
                assert!(editor.can_toggle_mode());
            }
            other => panic!("unexpected editor: {other:?}"),
        }

        state.handle_key(&PinsContext::default(), key(KeyCode::Down));
        state.handle_key(&PinsContext::default(), key(KeyCode::Char(' ')));
        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(editor.mode, PinCreateMode::NewVariation);
                assert_eq!(editor.mux_name.value(), "work");
            }
            other => panic!("unexpected editor: {other:?}"),
        }

        let outcome = state.handle_key(&PinsContext::default(), key(KeyCode::Enter));
        assert!(matches!(
            outcome,
            PinsOutcome::ApplyAndClose(PinsAction::CreatePin(_))
        ));
    }

    #[test]
    fn create_form_backtab_moves_to_previous_field() {
        let mut editor = PinCreateState::new(
            PinCreateDefaults {
                id: "scratch".to_string(),
                display_name: "scratch".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "scratch".to_string(),
                mode: PinCreateMode::NewVariation,
            },
            None,
            vec![],
            vec![],
        );

        editor.handle_key(key(KeyCode::Down));
        assert_eq!(editor.render_cursor(), 2);
        editor.handle_key(key(KeyCode::BackTab));
        assert_eq!(editor.render_cursor(), 0);
        editor.handle_key(shift_key(KeyCode::BackTab));
        assert_eq!(editor.render_cursor(), 9);
    }

    #[test]
    fn create_form_forwards_home_and_end_to_active_text_field() {
        let mut editor = PinCreateState::new(
            PinCreateDefaults {
                id: "scratch".to_string(),
                display_name: "scratch".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "scratch".to_string(),
                mode: PinCreateMode::NewVariation,
            },
            None,
            vec![],
            vec![],
        );

        editor.handle_key(key(KeyCode::Home));
        editor.handle_key(key(KeyCode::Char('x')));
        assert_eq!(editor.name.value(), "xscratch");
        editor.handle_key(key(KeyCode::End));
        editor.handle_key(key(KeyCode::Char('y')));
        assert_eq!(editor.name.value(), "xscratchy");
    }

    #[test]
    fn create_form_visible_window_marks_hidden_text_and_keeps_cursor_visible() {
        let display = pin_field_visible_window("abcdefghijklmnopqrstuvwxyz", 25, 12, true);
        assert_eq!(display.left_indicator, "< ");
        assert_eq!(display.right_indicator, "  ");
        assert_eq!(display.cursor_text, "z");
        let rendered = format!(
            "{}{}{}",
            display.before_cursor, display.cursor_text, display.after_cursor
        );
        assert!(rendered.len() <= 8);

        let display = pin_field_visible_window("abcdefghijklmnopqrstuvwxyz", 2, 12, true);
        assert_eq!(display.left_indicator, "  ");
        assert_eq!(display.right_indicator, " >");
        assert_eq!(display.cursor_text, "c");
    }

    #[test]
    fn create_form_cycles_known_harness_choices_from_harness_field() {
        let mut editor = PinCreateState::new(
            PinCreateDefaults {
                id: "scratch".to_string(),
                display_name: "scratch".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "scratch".to_string(),
                mode: PinCreateMode::NewVariation,
            },
            None,
            vec!["claude-code".to_string(), "codex".to_string()],
            vec![],
        );

        editor.handle_key(key(KeyCode::Down));
        editor.handle_key(key(KeyCode::Down));
        assert_eq!(editor.render_cursor(), 3);
        editor.handle_key(key(KeyCode::Right));
        assert_eq!(editor.harness.value(), "claude-code");
        editor.handle_key(key(KeyCode::Left));
        assert_eq!(editor.harness.value(), "codex");
        editor.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(editor.harness.value(), "claude-code");
    }

    #[test]
    fn create_form_harness_choices_keep_stable_column_when_focused() {
        let editor = PinCreateState::new(
            PinCreateDefaults {
                id: "scratch".to_string(),
                display_name: "scratch".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "scratch".to_string(),
                mode: PinCreateMode::NewVariation,
            },
            None,
            vec!["claude-code".to_string(), "codex".to_string()],
            vec![],
        );

        let inactive = line_content(pin_create_harness_field(&editor, 0, 74));
        let active = line_content(pin_create_harness_field(&editor, 3, 74));
        assert_eq!(inactive.find("[claude-code]"), active.find("[claude-code]"));
        assert_eq!(inactive.find("[codex]"), active.find("[codex]"));
    }

    #[test]
    fn create_form_allows_freeform_harness_with_warning() {
        let mut editor = PinCreateState::new(
            PinCreateDefaults {
                id: "scratch".to_string(),
                display_name: "scratch".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "scratch".to_string(),
                mode: PinCreateMode::NewVariation,
            },
            None,
            vec!["codex".to_string()],
            vec![],
        );

        editor.handle_key(key(KeyCode::Down));
        editor.handle_key(key(KeyCode::Down));
        for _ in 0.."codex".len() {
            editor.handle_key(key(KeyCode::Backspace));
        }
        for ch in "custom-harness".chars() {
            editor.handle_key(key(KeyCode::Char(ch)));
        }

        assert_eq!(
            editor.harness_warning().as_deref(),
            Some("custom harness `custom-harness` will be saved as typed")
        );
        assert_eq!(
            editor.request().expect("valid request").harness,
            "custom-harness"
        );
    }

    fn line_content(line: Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn create_form_auto_checks_adopt_for_known_mux_name_collisions() {
        let ctx = PinsContext {
            pin_create_defaults: PinCreateDefaults {
                id: "scratch".to_string(),
                display_name: "scratch".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "scratch".to_string(),
                mode: PinCreateMode::NewVariation,
            },
            pin_adopt_defaults: Some(PinCreateDefaults {
                id: "selected".to_string(),
                display_name: "selected".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "selected".to_string(),
                mode: PinCreateMode::AdoptSelected,
            }),
            known_mux_names: vec!["selected".to_string(), "busy-mux".to_string()],
            ..PinsContext::default()
        };
        let mut editor = PinCreateState::new(
            ctx.pin_create_defaults.clone(),
            ctx.pin_adopt_defaults.clone(),
            ctx.known_harness_keys.clone(),
            ctx.known_mux_names.clone(),
        );

        for _ in 0.."scratch".len() {
            editor.handle_key(key(KeyCode::Backspace));
        }
        for ch in "busy_mux".chars() {
            editor.handle_key(key(KeyCode::Char(ch)));
        }
        assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
        assert_eq!(editor.mux_name.value(), "busy-mux");
        assert_eq!(editor.mux_name_display(), "busy-mux");
        assert_eq!(
            editor
                .request()
                .expect("valid request")
                .adopt_source_mux_name
                .as_deref(),
            Some("busy-mux")
        );

        editor.handle_key(key(KeyCode::Char('x')));
        assert_eq!(editor.mode, PinCreateMode::NewVariation);
        assert_eq!(editor.name.value(), "busy_muxx");
        assert_eq!(editor.mux_name.value(), "busy-muxx");
        assert_eq!(
            editor
                .request()
                .expect("valid request")
                .adopt_source_mux_name,
            None
        );

        editor.handle_key(key(KeyCode::Down));
        editor.handle_key(key(KeyCode::Char(' ')));
        editor.handle_key(key(KeyCode::Up));
        editor.handle_key(key(KeyCode::Char('y')));
        assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
        assert_eq!(editor.name.value(), "busy_muxxy");
        assert_eq!(
            editor.mux_name_display(),
            "busy_muxxy (rename of: selected)"
        );

        for _ in 0.."busy_muxxy".len() {
            editor.handle_key(key(KeyCode::Backspace));
        }
        for ch in "busy-mux".chars() {
            editor.handle_key(key(KeyCode::Char(ch)));
        }
        assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
        assert_eq!(editor.mux_name_display(), "busy-mux");

        editor.handle_key(key(KeyCode::Char('z')));
        assert_eq!(editor.mode, PinCreateMode::NewVariation);
        assert_eq!(editor.name.value(), "busy-muxz");
    }

    #[test]
    fn create_form_collision_tracking_uses_mux_name_not_primary_name() {
        let ctx = PinsContext {
            pin_create_defaults: PinCreateDefaults {
                id: "scratch".to_string(),
                display_name: "scratch".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "scratch".to_string(),
                mode: PinCreateMode::NewVariation,
            },
            pin_adopt_defaults: Some(PinCreateDefaults {
                id: "selected".to_string(),
                display_name: "selected".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "selected".to_string(),
                mode: PinCreateMode::AdoptSelected,
            }),
            known_mux_names: vec!["live-mux".to_string()],
            ..PinsContext::default()
        };
        let mut editor = PinCreateState::new(
            ctx.pin_create_defaults.clone(),
            ctx.pin_adopt_defaults.clone(),
            ctx.known_harness_keys.clone(),
            ctx.known_mux_names.clone(),
        );

        for _ in 0.."scratch".len() {
            editor.handle_key(key(KeyCode::Backspace));
        }
        for ch in "live_mux".chars() {
            editor.handle_key(key(KeyCode::Char(ch)));
        }
        assert_eq!(editor.name.value(), "live_mux");
        assert_eq!(editor.mux_name.value(), "live-mux");
        assert_eq!(editor.mode, PinCreateMode::AdoptSelected);

        editor.handle_key(key(KeyCode::Down));
        editor.handle_key(key(KeyCode::Char(' ')));
        for _ in 0..6 {
            editor.handle_key(key(KeyCode::Down));
        }
        for _ in 0.."live_mux".len() {
            editor.handle_key(key(KeyCode::Backspace));
        }
        for ch in "not-live".chars() {
            editor.handle_key(key(KeyCode::Char(ch)));
        }
        assert_eq!(editor.name.value(), "live_mux");
        assert_eq!(editor.mux_name.value(), "not-live");
        assert_eq!(editor.mode, PinCreateMode::NewVariation);

        for _ in 0.."not-live".len() {
            editor.handle_key(key(KeyCode::Backspace));
        }
        for ch in "live-mux".chars() {
            editor.handle_key(key(KeyCode::Char(ch)));
        }
        assert_eq!(editor.name.value(), "live_mux");
        assert_eq!(editor.mux_name.value(), "live-mux");
        assert_eq!(editor.mode, PinCreateMode::AdoptSelected);
        assert_eq!(
            editor
                .request()
                .expect("valid request")
                .adopt_source_mux_name
                .as_deref(),
            Some("live-mux")
        );
    }

    #[test]
    fn enter_on_rename_without_target_emits_placeholder() {
        let ctx = PinsContext::default();
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(2); // rename
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder("rename"))
        );
    }

    #[test]
    fn enter_on_rename_with_target_opens_edit_form() {
        let ctx = PinsContext {
            pin_target: Some(pin_target()),
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(2); // rename
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, PinsOutcome::Continue);
        assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Edit(_))));
    }

    #[test]
    fn edit_form_confirms_target_fields() {
        let ctx = PinsContext {
            pin_target: Some(pin_target()),
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(2); // rename
        state.handle_key(&ctx, key(KeyCode::Enter));

        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            PinsOutcome::ApplyAndClose(PinsAction::EditPin(PinEditRequest {
                original_id: "ingest".to_string(),
                id: "ingest".to_string(),
                display_name: "Ingest".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "ingest-mux".to_string(),
                mux_socket: Some("scratch".to_string()),
                launch_argv: vec!["codex".to_string(), "--resume".to_string()],
                store_path: "/workspace/project/.conspectus.toml".to_string(),
            }))
        );
    }

    #[test]
    fn edit_form_cancel_does_not_emit_action() {
        let ctx = PinsContext {
            pin_target: Some(pin_target()),
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(2); // rename
        state.handle_key(&ctx, key(KeyCode::Enter));

        let outcome = state.handle_key(&ctx, key(KeyCode::Esc));
        assert_eq!(outcome, PinsOutcome::Continue);
        assert!(state.sub_editor().is_none());
    }

    #[test]
    fn enter_on_remove_with_target_opens_confirmation() {
        let ctx = PinsContext {
            pin_target: Some(pin_target()),
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(3); // remove
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, PinsOutcome::Continue);
        assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Remove(_))));
    }

    #[test]
    fn remove_confirmation_emits_remove_action() {
        let ctx = PinsContext {
            pin_target: Some(pin_target()),
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(3); // remove
        state.handle_key(&ctx, key(KeyCode::Enter));

        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            PinsOutcome::ApplyAndClose(PinsAction::RemovePin(PinRemoveRequest {
                id: "ingest".to_string(),
                display_name: "Ingest".to_string(),
                store_path: "/workspace/project/.conspectus.toml".to_string(),
            }))
        );
        assert!(state.sub_editor().is_none());
    }

    #[test]
    fn enter_on_bind_opens_picker_for_ambiguous_options() {
        let ctx = PinsContext {
            pin_bind_options: vec![
                PinBindOption {
                    pin_id: "ingest".to_string(),
                    session_key: "a".to_string(),
                    label: "codex:a".to_string(),
                },
                PinBindOption {
                    pin_id: "ingest".to_string(),
                    session_key: "b".to_string(),
                    label: "codex:b".to_string(),
                },
            ],
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(4); // bind
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, PinsOutcome::Continue);
        assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Bind(_))));
    }

    #[test]
    fn bind_picker_confirms_selected_session() {
        let ctx = PinsContext {
            pin_bind_options: vec![
                PinBindOption {
                    pin_id: "ingest".to_string(),
                    session_key: "a".to_string(),
                    label: "codex:a".to_string(),
                },
                PinBindOption {
                    pin_id: "ingest".to_string(),
                    session_key: "b".to_string(),
                    label: "codex:b".to_string(),
                },
            ],
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(4); // bind
        state.handle_key(&ctx, key(KeyCode::Enter));
        state.handle_key(&ctx, key(KeyCode::Down));

        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            PinsOutcome::ApplyAndClose(PinsAction::BindPin(PinBindRequest {
                pin_id: "ingest".to_string(),
                session_key: "b".to_string(),
            }))
        );
    }

    #[test]
    fn bind_with_no_options_emits_placeholder() {
        let ctx = PinsContext::default();
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(4); // bind
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder("bind"))
        );
    }

    #[test]
    fn create_form_confirms_defaults() {
        let ctx = PinsContext {
            pin_create_defaults: PinCreateDefaults {
                id: "ingest".to_string(),
                display_name: "Ingest".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "ingest-mux".to_string(),
                ..PinCreateDefaults::default()
            },
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.handle_key(&ctx, key(KeyCode::Enter));

        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            PinsOutcome::ApplyAndClose(PinsAction::CreatePin(PinCreateRequest {
                id: "ingest".to_string(),
                display_name: "Ingest".to_string(),
                harness: "codex".to_string(),
                cwd: "/workspace/project".to_string(),
                mux_name: "ingest-mux".to_string(),
                mux_socket: None,
                adopt_source_mux_name: None,
                launch_argv: Vec::new(),
                store: PinCreateStore::Auto,
            }))
        );
    }

    #[test]
    fn create_form_name_drives_default_identity_fields() {
        let ctx = PinsContext::default();
        let mut state = PinsOverlayState::open_with_create(PinCreateDefaults::default());

        // Cursor starts on the primary name field. Replace the
        // default name; id/display/mux stay synchronized because the
        // operator has not edited the advanced identity fields.
        for _ in 0.."new pin".len() {
            state.handle_key(&ctx, key(KeyCode::Backspace));
        }
        for ch in "Client Sandbox".chars() {
            state.handle_key(&ctx, key(KeyCode::Char(ch)));
        }

        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(editor.name.value(), "Client Sandbox");
                assert_eq!(editor.id.value(), "client-sandbox");
                assert_eq!(editor.display_name.value(), "Client Sandbox");
                assert_eq!(editor.mux_name.value(), "client-sandbox");
            }
            other => panic!("unexpected editor: {other:?}"),
        }
    }

    #[test]
    fn create_form_preserves_explicit_identity_overrides_after_name_edit() {
        let ctx = PinsContext::default();
        let mut state = PinsOverlayState::open_with_create(PinCreateDefaults::default());

        // Move to mux.name, edit it, then return to name and change
        // the primary value. The explicit mux override must survive.
        for _ in 0..6 {
            state.handle_key(&ctx, key(KeyCode::Down));
        }
        for _ in 0.."new-pin".len() {
            state.handle_key(&ctx, key(KeyCode::Backspace));
        }
        for ch in "kept-mux".chars() {
            state.handle_key(&ctx, key(KeyCode::Char(ch)));
        }
        for _ in 0..6 {
            state.handle_key(&ctx, key(KeyCode::Up));
        }
        for _ in 0.."new pin".len() {
            state.handle_key(&ctx, key(KeyCode::Backspace));
        }
        for ch in "Renamed Pin".chars() {
            state.handle_key(&ctx, key(KeyCode::Char(ch)));
        }

        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(editor.id.value(), "renamed-pin");
                assert_eq!(editor.display_name.value(), "Renamed Pin");
                assert_eq!(editor.mux_name.value(), "kept-mux");
            }
            other => panic!("unexpected editor: {other:?}"),
        }
    }

    #[test]
    fn create_form_cleared_identity_field_rejoins_name_derivation() {
        let ctx = PinsContext::default();
        let mut state = PinsOverlayState::open_with_create(PinCreateDefaults::default());

        // Override mux.name first.
        for _ in 0..6 {
            state.handle_key(&ctx, key(KeyCode::Down));
        }
        for _ in 0.."new-pin".len() {
            state.handle_key(&ctx, key(KeyCode::Backspace));
        }
        for ch in "kept-mux".chars() {
            state.handle_key(&ctx, key(KeyCode::Char(ch)));
        }

        // Clearing the field entirely makes it derived again. The
        // next name edit should fill it from the new name.
        for _ in 0.."kept-mux".len() {
            state.handle_key(&ctx, key(KeyCode::Backspace));
        }
        for _ in 0..6 {
            state.handle_key(&ctx, key(KeyCode::Up));
        }
        for _ in 0.."new pin".len() {
            state.handle_key(&ctx, key(KeyCode::Backspace));
        }
        for ch in "Client Sandbox".chars() {
            state.handle_key(&ctx, key(KeyCode::Char(ch)));
        }

        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(editor.mux_name.value(), "client-sandbox");
            }
            other => panic!("unexpected editor: {other:?}"),
        }
    }

    #[test]
    fn create_form_keeps_validation_errors_open() {
        let ctx = PinsContext {
            pin_create_defaults: PinCreateDefaults {
                id: "ingest".to_string(),
                display_name: "Ingest".to_string(),
                mux_name: "ingest".to_string(),
                ..PinCreateDefaults::default()
            },
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.handle_key(&ctx, key(KeyCode::Enter));

        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, PinsOutcome::Continue);
        match state.sub_editor() {
            Some(PinsSubEditor::Create(editor)) => {
                assert_eq!(
                    editor.error.as_deref(),
                    Some("pin create: harness is required")
                );
            }
            other => panic!("unexpected editor: {other:?}"),
        }
    }

    #[test]
    fn open_with_create_skips_menu() {
        let defaults = PinCreateDefaults {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace/project".to_string(),
            mux_name: "ingest-mux".to_string(),
            ..PinCreateDefaults::default()
        };
        let state = PinsOverlayState::open_with_create(defaults);
        assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Create(_))));
    }

    #[test]
    fn open_with_bind_skips_menu_when_options_present() {
        let options = vec![PinBindOption {
            pin_id: "ingest".to_string(),
            session_key: "a".to_string(),
            label: "codex:a".to_string(),
        }];
        let state = PinsOverlayState::open_with_bind(options).expect("options present");
        assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Bind(_))));
    }

    #[test]
    fn open_with_bind_returns_none_for_empty_options() {
        assert!(PinsOverlayState::open_with_bind(Vec::new()).is_none());
    }

    #[test]
    fn open_with_rebind_opens_mux_only_form() {
        let state = PinsOverlayState::open_with_rebind(pin_target());
        assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Rebind(_))));
    }

    #[test]
    fn rebind_form_preserves_unchanged_target_fields() {
        let target = pin_target();
        let mut state = PinRebindState::new(target.clone());
        // Enter without editing should round-trip the target's
        // mux fields and carry the rest through verbatim.
        let outcome = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match outcome {
            PinEditOutcome::Confirm(request) => {
                assert_eq!(request.original_id, target.id);
                assert_eq!(request.id, target.id);
                assert_eq!(request.display_name, target.display_name);
                assert_eq!(request.harness, target.harness);
                assert_eq!(request.cwd, target.cwd);
                assert_eq!(request.mux_name, target.mux_name);
                assert_eq!(request.mux_socket, target.mux_socket);
                assert_eq!(request.launch_argv, target.launch_argv);
                assert_eq!(request.store_path, target.store_path);
            }
            other => panic!("expected confirm, got {other:?}"),
        }
    }

    #[test]
    fn menu_rebind_uses_mux_only_form() {
        // The Pins menu's `rebind` entry routes through the
        // narrower form, not the full edit form.
        let ctx = PinsContext {
            pin_target: Some(pin_target()),
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(5); // rebind
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, PinsOutcome::Continue);
        assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Rebind(_))));
    }

    // ----- launch menu entry -----

    #[test]
    fn enter_on_launch_without_target_emits_placeholder() {
        let ctx = PinsContext::default();
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(1); // launch
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder("launch"))
        );
    }

    #[test]
    fn enter_on_launch_with_target_emits_launch_pin_action() {
        // The launch entry has no sub-editor — it commits the pin
        // id straight to the runtime so the TUI can suspend and
        // re-exec into `conspectus pin launch <id>`.
        let ctx = PinsContext {
            pin_target: Some(pin_target()),
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        state.cursor = PinsCursor::Action(1); // launch
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            PinsOutcome::ApplyAndClose(PinsAction::LaunchPin {
                pin_id: "ingest".to_string()
            })
        );
        assert!(state.sub_editor().is_none());
    }

    #[test]
    fn bind_picker_selected_last_option_scrolls_into_short_body() {
        let options: Vec<PinBindOption> = (0..8)
            .map(|idx| PinBindOption {
                pin_id: "ingest".to_string(),
                session_key: format!("session-{idx}"),
                label: format!("codex:session-{idx}"),
            })
            .collect();
        let state = PinBindState::new(options);
        let cursor_line = 1 + state.options.len() - 1;
        let content_height = 1 + state.options.len() + 2;
        let inner_height = 5;
        let offset = scroll_offset_for_cursor(Some(cursor_line), inner_height, content_height);
        assert!(offset > 0, "short bind picker should scroll");
        assert!(cursor_line >= offset as usize);
        assert!(cursor_line < offset as usize + inner_height);
    }
}
