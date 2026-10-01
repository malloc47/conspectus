//! Pins management overlay (ADR 0057).
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
//! apply a [`crate::tui::Msg`] (and either close or stay open).
//! Pin mutation side effects live in the reducer + executor per
//! ADR 0085 contract 2; this widget only describes intent.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::macros::span;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tui_popup::KnownSize;

use crate::discovery::harness::{
    HarnessLaunchOption, argv_contains_fragment, argv_with_fragment, argv_without_fragment,
    launch_argv_for, launch_options_for, strip_known_launch_option_fragments,
};
use crate::tui::Theme;
use crate::tui::widgets::input::TextInputState;
use crate::tui::widgets::launch_spec_form::{
    LaunchSpecFormState, LaunchSpecInit, SpecTextField, optional_string, parse_launch_argv,
};
use crate::tui::widgets::path_omnibox::{
    PathCandidate, PathOmniboxOutcome, PathOmniboxState, PathValidation,
};
use crate::tui::widgets::popup_frame::themed_popup;

mod bind;
mod create;
mod create_widget;
mod edit;
mod widgets;

use bind::*;
use create::*;
use create_widget::*;
use edit::*;
use widgets::*;

pub use widgets::PinsOverlayWidget;

const PIN_CREATE_LABEL_WIDTH: usize = 11;
const PIN_CREATE_VALUE_GAP: &str = "  ";
const PIN_CREATE_TEXT_GUTTER: &str = "  ";
const PIN_CREATE_CWD_SUFFIX_WIDTH: usize = 32;

/// Discoverable pin action group. Each entry maps 1:1 to a CLI
/// `conspectus pin <subcommand>` so the modal stays a thin
/// presentation of the underlying surface.
pub const PIN_ACTION_OPTIONS: &[&str] = &["create", "launch", "edit", "remove", "bind", "rebind"];

/// Read-only snapshot the pins overlay renders against. Borrowed
/// each frame so the overlay never holds a stale copy.
#[derive(Debug, Clone, Default)]
pub struct PinsContext {
    pub pin_create_defaults: PinCreateDefaults,
    pub pin_adopt_defaults: Option<PinCreateDefaults>,
    pub known_cwd_candidates: Vec<PathCandidate>,
    pub known_harness_keys: Vec<String>,
    pub known_mux_names: Vec<String>,
    pub known_pin_ids: Vec<String>,
    pub known_pin_mux_names: Vec<String>,
    pub selected_pin_id: Option<String>,
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

#[derive(Debug, Clone, Default)]
struct PinCreateOptions {
    adopt_defaults: Option<PinCreateDefaults>,
    known_cwd_candidates: Vec<PathCandidate>,
    known_harness_keys: Vec<String>,
    known_mux_names: Vec<String>,
    known_pin_ids: Vec<String>,
    known_pin_mux_names: Vec<String>,
    selected_pin_id: Option<String>,
    /// Open with the worktree toggle pre-enabled (the `N` new-stream
    /// entry point, ADR 0094).
    worktree_enabled: bool,
}

/// Build create options from a pins context, optionally pre-enabling
/// the worktree toggle (the `N` new-stream entry point).
fn create_options(ctx: &PinsContext, worktree_enabled: bool) -> PinCreateOptions {
    PinCreateOptions {
        adopt_defaults: ctx.pin_adopt_defaults.clone(),
        known_cwd_candidates: ctx.known_cwd_candidates.clone(),
        known_harness_keys: ctx.known_harness_keys.clone(),
        known_mux_names: ctx.known_mux_names.clone(),
        known_pin_ids: ctx.known_pin_ids.clone(),
        known_pin_mux_names: ctx.known_pin_mux_names.clone(),
        selected_pin_id: ctx.selected_pin_id.clone(),
        worktree_enabled,
    }
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
/// Committed variants carry a [`crate::tui::Msg`] the runtime
/// dispatches through the reducer (ADR 0085 contract 3), matching
/// the `ControlsOutcome` shape landed in Phase F.
#[derive(Debug, Clone, PartialEq)]
pub enum PinsOutcome {
    Continue,
    Close,
    ApplyAndStay(crate::tui::Msg),
    ApplyAndClose(crate::tui::Msg),
}

/// Format a placeholder-hint status message for a menu action
/// chosen without its prerequisites met. Shared between the
/// widget's outcome constructor and any direct caller wanting
/// to match the wording.
pub fn pin_placeholder_status(label: &str) -> String {
    format!(
        "pins: `{label}` needs a pin selection; press `p` for the picker or use `conspectus pin {label}`"
    )
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
    /// When `Some`, the pin is worktree-backed (ADR 0094): its worktree
    /// for this branch is created (if absent) and entered at launch.
    pub worktree_branch: Option<String>,
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
    /// Shared launch-spec fields (harness, cwd, mux name/socket,
    /// launch argv, worktree toggle + branch, known-harness /
    /// known-live-mux collections, error). See ADR 0097 and
    /// [`crate::tui::widgets::launch_spec_form`].
    spec: LaunchSpecFormState,
    store: PinCreateStore,
    id_overridden: bool,
    display_overridden: bool,
    mux_overridden: bool,
    known_pin_ids: Vec<String>,
    known_pin_mux_names: Vec<String>,
    selected_pin_id: Option<String>,
    adopt_auto_uncheck_armed: bool,
    adopt_auto_checked_by_collision: bool,
}

#[derive(Debug, Clone)]
pub struct PinEditState {
    target: PinMutationTarget,
    cursor: usize,
    id: TextInputState,
    display_name: TextInputState,
    harness: TextInputState,
    cwd: PathOmniboxState,
    mux_name: TextInputState,
    mux_socket: TextInputState,
    launch_argv: TextInputState,
    known_harness_keys: Vec<String>,
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
        Self::open_with_create_options(
            defaults,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
        )
    }

    pub fn open_with_create_options(
        defaults: PinCreateDefaults,
        adopt_defaults: Option<PinCreateDefaults>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
        known_pin_ids: Vec<String>,
        known_pin_mux_names: Vec<String>,
        selected_pin_id: Option<String>,
    ) -> Self {
        Self::open_with_create_initial(
            defaults,
            PinCreateOptions {
                adopt_defaults,
                known_harness_keys,
                known_mux_names,
                known_pin_ids,
                known_pin_mux_names,
                selected_pin_id,
                ..PinCreateOptions::default()
            },
        )
    }

    pub fn open_with_create_context(ctx: &PinsContext) -> Self {
        Self::open_with_create_initial(ctx.pin_create_defaults.clone(), create_options(ctx, false))
    }

    /// Open the create form with the worktree toggle pre-enabled — the
    /// `N` new-stream entry point (ADR 0094).
    pub fn open_with_new_stream_context(ctx: &PinsContext) -> Self {
        Self::open_with_create_initial(ctx.pin_create_defaults.clone(), create_options(ctx, true))
    }

    pub fn open_with_adopt_options(
        defaults: PinCreateDefaults,
        adopt_defaults: Option<PinCreateDefaults>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
        known_pin_ids: Vec<String>,
        known_pin_mux_names: Vec<String>,
        selected_pin_id: Option<String>,
    ) -> Self {
        Self::open_with_adopt_initial(
            defaults,
            PinCreateOptions {
                adopt_defaults,
                known_harness_keys,
                known_mux_names,
                known_pin_ids,
                known_pin_mux_names,
                selected_pin_id,
                ..PinCreateOptions::default()
            },
        )
    }

    pub fn open_with_adopt_context(ctx: &PinsContext) -> Self {
        Self::open_with_adopt_initial(ctx.pin_create_defaults.clone(), create_options(ctx, false))
    }

    fn open_with_adopt_initial(defaults: PinCreateDefaults, options: PinCreateOptions) -> Self {
        Self::open_with_create_initial(defaults, options)
    }

    fn open_with_create_initial(defaults: PinCreateDefaults, options: PinCreateOptions) -> Self {
        let initial = options
            .adopt_defaults
            .clone()
            .unwrap_or_else(|| defaults.clone());
        Self {
            cursor: PinsCursor::Action(0),
            sub_editor: Some(PinsSubEditor::Create(Box::new(
                PinCreateState::new_with_options(initial, options),
            ))),
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
            // "edit" is index 2 in PIN_ACTION_OPTIONS.
            cursor: PinsCursor::Action(2),
            sub_editor: Some(PinsSubEditor::Edit(Box::new(PinEditState::new(target)))),
        }
    }

    /// Open directly into the edit form seeded with `target`, pulling
    /// known cwd candidates and harness keys from `ctx` so the form
    /// can offer the same cycling / completion widgets as create.
    pub fn open_with_edit_context(ctx: &PinsContext, target: PinMutationTarget) -> Self {
        Self {
            cursor: PinsCursor::Action(2),
            sub_editor: Some(PinsSubEditor::Edit(Box::new(
                PinEditState::new_with_options(
                    target,
                    PinEditOptions {
                        known_cwd_candidates: ctx.known_cwd_candidates.clone(),
                        known_harness_keys: ctx.known_harness_keys.clone(),
                    },
                ),
            ))),
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
            self.sub_editor = Some(PinsSubEditor::Create(Box::new(
                PinCreateState::new_with_options(initial, create_options(ctx, false)),
            )));
            PinsOutcome::Continue
        } else if label == "launch" {
            // Launch has no sub-editor — the runtime takes the
            // pin id and re-execs into `conspectus pin launch`,
            // suspending the TUI. Requires a pin selection;
            // placeholder otherwise.
            if let Some(target) = ctx.pin_target.clone() {
                PinsOutcome::ApplyAndClose(crate::tui::Msg::LaunchPinById(target.id))
            } else {
                PinsOutcome::ApplyAndStay(crate::tui::Msg::SetStatus(Some(pin_placeholder_status(
                    label,
                ))))
            }
        } else if label == "edit" {
            if let Some(target) = ctx.pin_target.clone() {
                self.sub_editor = Some(PinsSubEditor::Edit(Box::new(
                    PinEditState::new_with_options(
                        target,
                        PinEditOptions {
                            known_cwd_candidates: ctx.known_cwd_candidates.clone(),
                            known_harness_keys: ctx.known_harness_keys.clone(),
                        },
                    ),
                )));
                PinsOutcome::Continue
            } else {
                PinsOutcome::ApplyAndStay(crate::tui::Msg::SetStatus(Some(pin_placeholder_status(
                    label,
                ))))
            }
        } else if label == "rebind" {
            if let Some(target) = ctx.pin_target.clone() {
                self.sub_editor =
                    Some(PinsSubEditor::Rebind(Box::new(PinRebindState::new(target))));
                PinsOutcome::Continue
            } else {
                PinsOutcome::ApplyAndStay(crate::tui::Msg::SetStatus(Some(pin_placeholder_status(
                    label,
                ))))
            }
        } else if label == "remove" {
            if let Some(target) = ctx.pin_target.clone() {
                self.sub_editor =
                    Some(PinsSubEditor::Remove(Box::new(PinRemoveState::new(target))));
                PinsOutcome::Continue
            } else {
                PinsOutcome::ApplyAndStay(crate::tui::Msg::SetStatus(Some(pin_placeholder_status(
                    label,
                ))))
            }
        } else if label == "bind" {
            if ctx.pin_bind_options.is_empty() {
                PinsOutcome::ApplyAndStay(crate::tui::Msg::SetStatus(Some(pin_placeholder_status(
                    label,
                ))))
            } else {
                self.sub_editor = Some(PinsSubEditor::Bind(PinBindState::new(
                    ctx.pin_bind_options.clone(),
                )));
                PinsOutcome::Continue
            }
        } else {
            PinsOutcome::ApplyAndStay(crate::tui::Msg::SetStatus(Some(pin_placeholder_status(
                label,
            ))))
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
                    PinsOutcome::ApplyAndClose(crate::tui::Msg::PinCreate(*request))
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
                    PinsOutcome::ApplyAndClose(crate::tui::Msg::PinEdit(*request))
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
                    PinsOutcome::ApplyAndClose(crate::tui::Msg::PinEdit(*request))
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
                    PinsOutcome::ApplyAndClose(crate::tui::Msg::PinBind(request))
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
                    PinsOutcome::ApplyAndClose(crate::tui::Msg::PinRemove(request))
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
    Confirm(Box<PinCreateRequest>),
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

/// Field labels for the shared launch-spec fields as pin-create
/// renders them. Kept as module-level constants so the primitive's
/// wholesale-replace helpers (harness cycle, mux-name derivation)
/// pass the same label back to the underlying `TextInputState`.
const PIN_HARNESS_LABEL: &str = " harness ";
const PIN_CWD_LABEL: &str = " cwd ";
const PIN_MUX_NAME_LABEL: &str = " mux ";
const PIN_MUX_SOCKET_LABEL: &str = " socket ";
const PIN_LAUNCH_ARGV_LABEL: &str = " launch argv ";
const PIN_WORKTREE_BRANCH_LABEL: &str = " worktree branch ";

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

impl crate::tui::Overlay for PinsOverlayState {
    type Ctx<'a> = &'a PinsContext;

    fn handle(&mut self, ctx: &PinsContext, key: KeyEvent) -> crate::tui::OverlayOutcome {
        match self.handle_key(ctx, key) {
            PinsOutcome::Continue => crate::tui::OverlayOutcome::Consumed,
            PinsOutcome::Close => crate::tui::OverlayOutcome::Close,
            PinsOutcome::ApplyAndStay(msg) => {
                crate::tui::OverlayOutcome::CommitAndStay(Box::new(msg))
            }
            PinsOutcome::ApplyAndClose(msg) => crate::tui::OverlayOutcome::Commit(Box::new(msg)),
        }
    }
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod tests;
