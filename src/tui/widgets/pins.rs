//! Pins management overlay (ADR 0057 / H-PIN-022..024).
//!
//! Dedicated modal — separate from the view/grouping/filter
//! [`controls`](super::controls) overlay — that fronts every pin
//! CRUD flow: create, rename, remove, bind (PinAmbiguous override),
//! rebind (external-rename recovery), adopt (agent-deck migration).
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
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::widgets::input::TextInputState;

/// Discoverable pin action group. Each entry maps 1:1 to a CLI
/// `conspectus pin <subcommand>` so the modal stays a thin
/// presentation of the underlying surface.
pub const PIN_ACTION_OPTIONS: &[&str] = &[
    "create", "launch", "rename", "remove", "bind", "rebind", "adopt",
];

/// Read-only snapshot the pins overlay renders against. Borrowed
/// each frame so the overlay never holds a stale copy.
#[derive(Debug, Clone, Default)]
pub struct PinsContext {
    pub pin_create_defaults: PinCreateDefaults,
    pub pin_target: Option<PinMutationTarget>,
    pub pin_bind_options: Vec<PinBindOption>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinCreateDefaults {
    pub id: String,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux_name: String,
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
    Create(PinCreateState),
    Edit(PinEditState),
    Rebind(PinRebindState),
    Bind(PinBindState),
    Remove(PinRemoveState),
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
    cursor: usize,
    id: TextInputState,
    display_name: TextInputState,
    harness: TextInputState,
    cwd: TextInputState,
    mux_name: TextInputState,
    mux_socket: TextInputState,
    launch_argv: TextInputState,
    store: PinCreateStore,
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
        Self {
            cursor: PinsCursor::Action(0),
            sub_editor: Some(PinsSubEditor::Create(PinCreateState::new(defaults))),
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
            sub_editor: Some(PinsSubEditor::Remove(PinRemoveState::new(target))),
        }
    }

    /// Open directly into the edit form for `target`. Used by the
    /// rename direct shortcut.
    pub fn open_with_edit(target: PinMutationTarget) -> Self {
        Self {
            // "rename" is index 2 in PIN_ACTION_OPTIONS.
            cursor: PinsCursor::Action(2),
            sub_editor: Some(PinsSubEditor::Edit(PinEditState::new(target))),
        }
    }

    /// Open directly into the mux-only rebind form. Backed by the
    /// `B` direct shortcut so external tmux renames recover in one
    /// keystroke without paging through the full edit form.
    pub fn open_with_rebind(target: PinMutationTarget) -> Self {
        Self {
            // "rebind" is index 5 in PIN_ACTION_OPTIONS.
            cursor: PinsCursor::Action(5),
            sub_editor: Some(PinsSubEditor::Rebind(PinRebindState::new(target))),
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
        if label == "create" || label == "adopt" {
            self.sub_editor = Some(PinsSubEditor::Create(PinCreateState::new(
                ctx.pin_create_defaults.clone(),
            )));
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
                self.sub_editor = Some(PinsSubEditor::Edit(PinEditState::new(target)));
                PinsOutcome::Continue
            } else {
                PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder(label))
            }
        } else if label == "rebind" {
            if let Some(target) = ctx.pin_target.clone() {
                self.sub_editor = Some(PinsSubEditor::Rebind(PinRebindState::new(target)));
                PinsOutcome::Continue
            } else {
                PinsOutcome::ApplyAndStay(PinsAction::PinPlaceholder(label))
            }
        } else if label == "remove" {
            if let Some(target) = ctx.pin_target.clone() {
                self.sub_editor = Some(PinsSubEditor::Remove(PinRemoveState::new(target)));
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
    const FIELD_COUNT: usize = 8;

    fn new(defaults: PinCreateDefaults) -> Self {
        let id = if defaults.id.is_empty() {
            "new-pin".to_string()
        } else {
            defaults.id
        };
        let display_name = if defaults.display_name.is_empty() {
            id.clone()
        } else {
            defaults.display_name
        };
        let mux_name = if defaults.mux_name.is_empty() {
            display_name.clone()
        } else {
            defaults.mux_name
        };
        Self {
            cursor: 0,
            id: TextInputState::new(" id ", id),
            display_name: TextInputState::new(" display ", display_name),
            harness: TextInputState::new(" harness ", defaults.harness),
            cwd: TextInputState::new(" cwd ", defaults.cwd),
            mux_name: TextInputState::new(" mux ", mux_name),
            mux_socket: TextInputState::new(" socket ", String::new()),
            launch_argv: TextInputState::new(" launch argv ", String::new()),
            store: PinCreateStore::Auto,
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
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.cursor == Self::FIELD_COUNT - 1 =>
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
                if let Some(input) = self.active_input_mut() {
                    let _ = input.handle_key(event);
                    self.error = None;
                }
                PinCreateOutcome::Continue
            }
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        let len = Self::FIELD_COUNT as i32;
        let next = ((self.cursor as i32 + delta) % len + len) % len;
        self.cursor = next as usize;
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

    fn active_input_mut(&mut self) -> Option<&mut TextInputState> {
        match self.cursor {
            0 => Some(&mut self.id),
            1 => Some(&mut self.display_name),
            2 => Some(&mut self.harness),
            3 => Some(&mut self.cwd),
            4 => Some(&mut self.mux_name),
            5 => Some(&mut self.mux_socket),
            6 => Some(&mut self.launch_argv),
            _ => None,
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
            launch_argv,
            store: self.store,
        })
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
}

impl<'a> PinsOverlayWidget<'a> {
    pub fn new(state: &'a PinsOverlayState) -> Self {
        Self { state }
    }
}

impl Widget for PinsOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = centered_modal_rect(area);
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Pins "));
        let inner = block.inner(modal);
        block.render(modal, buf);

        let cursor = self.state.cursor();
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(section_header("Pins"));
        for (idx, label) in PIN_ACTION_OPTIONS.iter().enumerate() {
            let row = PinsCursor::Action(idx);
            lines.push(row_line((*label).to_string(), cursor == row));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "↑/↓ move · Enter pick · Esc close",
            Style::default().add_modifier(Modifier::DIM),
        )));

        Paragraph::new(lines).render(inner, buf);

        if let Some(editor) = self.state.sub_editor() {
            render_sub_editor(editor, area, buf);
        }
    }
}

fn render_sub_editor(editor: &PinsSubEditor, area: Rect, buf: &mut Buffer) {
    match editor {
        PinsSubEditor::Create(state) => PinCreateWidget::new(state).render(area, buf),
        PinsSubEditor::Edit(state) => PinEditWidget::new(state).render(area, buf),
        PinsSubEditor::Rebind(state) => PinRebindWidget::new(state).render(area, buf),
        PinsSubEditor::Bind(state) => PinBindWidget::new(state).render(area, buf),
        PinsSubEditor::Remove(state) => PinRemoveWidget::new(state).render(area, buf),
    }
}

struct PinCreateWidget<'a> {
    state: &'a PinCreateState,
}

impl<'a> PinCreateWidget<'a> {
    fn new(state: &'a PinCreateState) -> Self {
        Self { state }
    }
}

impl Widget for PinCreateWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = pin_create_modal_rect(area);
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Create Pin "));
        let inner = block.inner(modal);
        block.render(modal, buf);

        let mut lines = vec![
            pin_create_field(0, "id", self.state.id.value(), self.state.cursor),
            pin_create_field(
                1,
                "display",
                self.state.display_name.value(),
                self.state.cursor,
            ),
            pin_create_field(2, "harness", self.state.harness.value(), self.state.cursor),
            pin_create_field(3, "cwd", self.state.cwd.value(), self.state.cursor),
            pin_create_field(
                4,
                "mux.name",
                self.state.mux_name.value(),
                self.state.cursor,
            ),
            pin_create_field(
                5,
                "mux.socket",
                self.state.mux_socket.value(),
                self.state.cursor,
            ),
            pin_create_field(
                6,
                "launch argv",
                self.state.launch_argv.value(),
                self.state.cursor,
            ),
            pin_create_field(7, "store", self.state.store.label(), self.state.cursor),
        ];
        if let Some(error) = &self.state.error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                error.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Up/Down field · type to edit · Space cycles store · Enter create · Esc cancel",
            Style::default().add_modifier(Modifier::DIM),
        )));

        Paragraph::new(lines).render(inner, buf);
    }
}

fn pin_create_field(idx: usize, label: &'static str, value: &str, cursor: usize) -> Line<'static> {
    let marker = if cursor == idx { "> " } else { "  " };
    let style = if cursor == idx {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    let value = if value.trim().is_empty() { "-" } else { value };
    Line::from(Span::styled(format!("{marker}{label:11} {value}"), style))
}

fn pin_create_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(76, area.width.saturating_sub(4)).max(44);
    let height = std::cmp::min(14, area.height.saturating_sub(2)).max(10);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

struct PinEditWidget<'a> {
    state: &'a PinEditState,
}

impl<'a> PinEditWidget<'a> {
    fn new(state: &'a PinEditState) -> Self {
        Self { state }
    }
}

impl Widget for PinEditWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = pin_edit_modal_rect(area);
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Edit Pin "));
        let inner = block.inner(modal);
        block.render(modal, buf);

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
            Line::from(format!("  harness     {}", self.state.target.harness)),
            Line::from(format!("  cwd         {}", self.state.target.cwd)),
            Line::from(format!("  store       {}", self.state.target.store_path)),
        ];
        if let Some(error) = &self.state.error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                error.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Up/Down field · type to edit · Enter save · Esc cancel",
            Style::default().add_modifier(Modifier::DIM),
        )));

        Paragraph::new(lines).render(inner, buf);
    }
}

struct PinRebindWidget<'a> {
    state: &'a PinRebindState,
}

impl<'a> PinRebindWidget<'a> {
    fn new(state: &'a PinRebindState) -> Self {
        Self { state }
    }
}

impl Widget for PinRebindWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = pin_rebind_modal_rect(area);
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Rebind Pin "));
        let inner = block.inner(modal);
        block.render(modal, buf);

        let mut lines = vec![
            Line::from(format!("  id          {}", self.state.target.id)),
            Line::from(format!("  display     {}", self.state.target.display_name)),
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
            Line::from(format!("  store       {}", self.state.target.store_path)),
        ];
        if let Some(error) = &self.state.error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                error.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Up/Down field · type to edit · Enter save · Esc cancel",
            Style::default().add_modifier(Modifier::DIM),
        )));

        Paragraph::new(lines).render(inner, buf);
    }
}

fn pin_rebind_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(70, area.width.saturating_sub(4)).max(44);
    let height = std::cmp::min(12, area.height.saturating_sub(2)).max(9);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

fn pin_edit_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(78, area.width.saturating_sub(4)).max(46);
    let height = std::cmp::min(14, area.height.saturating_sub(2)).max(10);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

struct PinBindWidget<'a> {
    state: &'a PinBindState,
}

impl<'a> PinBindWidget<'a> {
    fn new(state: &'a PinBindState) -> Self {
        Self { state }
    }
}

impl Widget for PinBindWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = pin_bind_modal_rect(area);
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Bind Pin "));
        let inner = block.inner(modal);
        block.render(modal, buf);

        let mut lines = Vec::new();
        if let Some(first) = self.state.options.first() {
            lines.push(Line::from(format!("pin       {}", first.pin_id)));
        }
        for (idx, option) in self.state.options.iter().enumerate() {
            let marker = if idx == self.state.cursor { "> " } else { "  " };
            let style = if idx == self.state.cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{marker}{}", option.label),
                style,
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Up/Down choose · Enter bind · Esc cancel",
            Style::default().add_modifier(Modifier::DIM),
        )));
        Paragraph::new(lines).render(inner, buf);
    }
}

fn pin_bind_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(76, area.width.saturating_sub(4)).max(44);
    let height = std::cmp::min(12, area.height.saturating_sub(2)).max(7);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

struct PinRemoveWidget<'a> {
    state: &'a PinRemoveState,
}

impl<'a> PinRemoveWidget<'a> {
    fn new(state: &'a PinRemoveState) -> Self {
        Self { state }
    }
}

impl Widget for PinRemoveWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = pin_remove_modal_rect(area);
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Remove Pin "));
        let inner = block.inner(modal);
        block.render(modal, buf);

        let lines = vec![
            Line::from(vec![
                Span::raw("id       "),
                Span::styled(
                    self.state.target.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(format!("display  {}", self.state.target.display_name)),
            Line::from(format!("store    {}", self.state.target.store_path)),
            Line::from(""),
            Line::from(Span::styled(
                "Enter remove · Esc cancel",
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        Paragraph::new(lines).render(inner, buf);
    }
}

fn pin_remove_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(76, area.width.saturating_sub(4)).max(44);
    let height = std::cmp::min(8, area.height.saturating_sub(2)).max(7);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

fn section_header(label: &str) -> Line<'static> {
    Line::from(Span::styled(
        label.to_string(),
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

fn row_line(label: String, cursored: bool) -> Line<'static> {
    let marker = if cursored { "> " } else { "  " };
    let mut style = Style::default();
    if cursored {
        style = style.add_modifier(Modifier::REVERSED);
    }
    Line::from(Span::styled(format!("{marker}{label}"), style))
}

fn centered_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(50, area.width.saturating_sub(4)).max(32);
    let max_height = area.height.saturating_sub(2);
    let desired = 14;
    let height = (desired as u16).clamp(8, max_height.max(8));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
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
            },
            ..PinsContext::default()
        };
        let mut state = PinsOverlayState::new();
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, PinsOutcome::Continue);
        assert!(matches!(state.sub_editor(), Some(PinsSubEditor::Create(_))));
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
                launch_argv: Vec::new(),
                store: PinCreateStore::Auto,
            }))
        );
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
}
