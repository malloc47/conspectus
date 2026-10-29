//! Shared launch-spec form primitive (ADR 0097).
//!
//! Container for the fields the mux-launch form uses:
//! harness, cwd (with autocomplete), mux name + socket, launch argv
//! override, worktree toggle + branch, plus known-harness and known-
//! live-mux collections used for autocomplete + collision detection.
//!
//! Wrappers ([`crate::tui::widgets::mux_launch::MuxLaunchFormState`])
//! hold this by composition, own their own cursor sequence +
//! rendering, and decide which of the primitive's fields are visible
//! in their layout. A wrapper reads or writes the shared fields only
//! through this struct's accessors.
//!
//! No cursor lives here. No Msg dispatch lives here. The primitive is
//! pure state + a handful of validation helpers that both wrappers
//! call from their own `Enter`/commit paths.

use ratatui::crossterm::event::KeyEvent;

use crate::discovery::harness::{
    HarnessLaunchOption, argv_contains_fragment, argv_with_fragment, argv_without_fragment,
    launch_options_for, strip_known_launch_option_fragments,
};
use crate::tui::widgets::form::display_argv;
use tuikit::path_field::{Candidate, PathField};
use tuikit::text_edit::{Edit, LineEdit};

/// Owned state for the launch-spec form's shared fields.
/// `MuxLaunchFormState` embeds this exactly once and routes every
/// read/write of a shared field through the accessors.
#[derive(Debug, Clone)]
pub struct LaunchSpecFormState {
    harness: LineEdit,
    cwd: PathField,
    mux_name: LineEdit,
    mux_socket: LineEdit,
    launch_argv: LineEdit,
    worktree_enabled: bool,
    worktree_branch: LineEdit,
    known_harness_keys: Vec<String>,
    known_mux_names: Vec<String>,
    error: Option<String>,
}

/// Constructor input for [`LaunchSpecFormState::new`]. Grouped into a
/// single struct so the primitive stays under the clippy 7-argument
/// budget and readers can spot each field without matching positional
/// arguments to parameter names.
#[derive(Debug, Clone)]
pub struct LaunchSpecInit {
    pub harness_value: String,
    pub cwd_value: String,
    pub mux_name_value: String,
    pub worktree_branch_value: String,
    pub known_harness_keys: Vec<String>,
    pub known_mux_names: Vec<String>,
}

impl LaunchSpecFormState {
    /// Build a spec with pre-populated defaults. The field labels are
    /// the wrapper's: they show up on its rendered rows, not inside the
    /// primitive.
    pub fn new(init: LaunchSpecInit) -> Self {
        Self {
            harness: LineEdit::new(init.harness_value),
            cwd: crate::tui::paths::directory_field(init.cwd_value, Vec::new()),
            mux_name: LineEdit::new(init.mux_name_value),
            mux_socket: LineEdit::default(),
            launch_argv: LineEdit::default(),
            worktree_enabled: false,
            worktree_branch: LineEdit::new(init.worktree_branch_value),
            known_harness_keys: init.known_harness_keys,
            known_mux_names: init.known_mux_names,
            error: None,
        }
    }

    // -- read accessors ---------------------------------------------

    pub fn harness(&self) -> &LineEdit {
        &self.harness
    }
    pub fn cwd(&self) -> &PathField {
        &self.cwd
    }
    pub fn mux_name(&self) -> &LineEdit {
        &self.mux_name
    }
    pub fn mux_socket(&self) -> &LineEdit {
        &self.mux_socket
    }
    pub fn launch_argv(&self) -> &LineEdit {
        &self.launch_argv
    }
    pub fn worktree_enabled(&self) -> bool {
        self.worktree_enabled
    }
    pub fn worktree_branch(&self) -> &LineEdit {
        &self.worktree_branch
    }
    pub fn known_harness_keys(&self) -> &[String] {
        &self.known_harness_keys
    }
    pub fn known_mux_names(&self) -> &[String] {
        &self.known_mux_names
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    // -- mutable field access ---------------------------------------

    pub fn harness_mut(&mut self) -> &mut LineEdit {
        &mut self.harness
    }
    pub fn cwd_mut(&mut self) -> &mut PathField {
        &mut self.cwd
    }
    pub fn mux_name_mut(&mut self) -> &mut LineEdit {
        &mut self.mux_name
    }
    pub fn mux_socket_mut(&mut self) -> &mut LineEdit {
        &mut self.mux_socket
    }
    pub fn launch_argv_mut(&mut self) -> &mut LineEdit {
        &mut self.launch_argv
    }
    pub fn worktree_branch_mut(&mut self) -> &mut LineEdit {
        &mut self.worktree_branch
    }

    // -- state mutation helpers -------------------------------------

    /// Replace the harness field wholesale. Used when a wrapper cycles
    /// through `known_harness_keys` or when a mode change re-seeds
    /// the harness default.
    pub fn set_harness(&mut self, value: impl Into<String>) {
        self.harness = LineEdit::new(value);
    }

    /// Replace the mux-name field wholesale.
    pub fn set_mux_name(&mut self, value: impl Into<String>) {
        self.mux_name = LineEdit::new(value);
    }

    /// Replace the launch-argv field wholesale (harness cycling
    /// rebuilds this when the previous value matched the old
    /// harness's default).
    pub fn set_launch_argv(&mut self, value: impl Into<String>) {
        self.launch_argv = LineEdit::new(value);
    }

    /// Toggle the worktree flag. Returns the new value.
    pub fn toggle_worktree(&mut self) -> bool {
        self.worktree_enabled = !self.worktree_enabled;
        self.worktree_enabled
    }

    pub fn set_worktree_enabled(&mut self, enabled: bool) {
        self.worktree_enabled = enabled;
    }

    pub fn set_cwd_candidates(&mut self, candidates: Vec<Candidate>) {
        self.cwd.set_candidates(candidates);
    }

    pub fn set_known_harness_keys(&mut self, keys: Vec<String>) {
        self.known_harness_keys = keys;
    }

    pub fn set_known_mux_names(&mut self, names: Vec<String>) {
        self.known_mux_names = names;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    // -- shared validation / lookup ---------------------------------

    /// Cycle to the next (delta > 0) or previous (delta < 0) known
    /// harness key. Returns the newly-selected key when the cycle
    /// moved, `None` when the known-key list is empty.
    pub fn cycle_harness(&mut self, delta: i32) -> Option<String> {
        if self.known_harness_keys.is_empty() {
            return None;
        }
        let value = self.harness.value().trim();
        let idx = match self
            .known_harness_keys
            .iter()
            .position(|known| known == value)
        {
            Some(idx) => tuikit::scroll::wrap_step(idx, self.known_harness_keys.len(), delta),
            None if delta < 0 => self.known_harness_keys.len().saturating_sub(1),
            None => 0,
        };
        let next = self.known_harness_keys[idx].clone();
        self.harness = LineEdit::new(next.clone());
        Some(next)
    }

    // -- harness launch options -------------------------------------

    /// The launch options (such as skipping permissions) the chosen
    /// harness offers.
    pub fn launch_options(&self) -> &'static [HarnessLaunchOption] {
        launch_options_for(self.harness.value().trim())
    }

    /// The argv the form would launch with: the typed override, or
    /// `default` when the field is empty.
    pub fn effective_argv(&self, default: &[String]) -> Result<Vec<String>, String> {
        let typed = parse_launch_argv(self.launch_argv.value().trim(), "launch")?;
        Ok(if typed.is_empty() {
            default.to_vec()
        } else {
            typed
        })
    }

    /// Whether `option`'s flags are in the effective argv.
    pub fn launch_option_selected(&self, option: HarnessLaunchOption, default: &[String]) -> bool {
        self.effective_argv(default)
            .is_ok_and(|argv| argv_contains_fragment(&argv, option.argv))
    }

    /// Replace the launch-argv field with `argv`, leaving it empty when
    /// that is just the default so the default keeps following the
    /// harness.
    pub fn set_effective_argv(&mut self, argv: &[String], default: &[String]) {
        let value = if argv.is_empty() || argv == default {
            String::new()
        } else {
            display_argv(argv)
        };
        self.launch_argv = LineEdit::new(value);
    }

    /// Flip a launch option. `delta` steps through the options when
    /// there are several; with one option it just toggles it.
    pub fn toggle_launch_option(&mut self, default: &[String], delta: i32) {
        let options = self.launch_options();
        if options.is_empty() {
            return;
        }
        let Ok(argv) = self.effective_argv(default) else {
            return;
        };
        let selected: Vec<usize> = options
            .iter()
            .enumerate()
            .filter(|(_, option)| argv_contains_fragment(&argv, option.argv))
            .map(|(idx, _)| idx)
            .collect();
        let idx = match (selected.as_slice(), delta) {
            ([only], -1 | 1) if options.len() > 1 => {
                tuikit::scroll::wrap_step(*only, options.len(), delta)
            }
            ([first, ..], _) => *first,
            ([], _) => 0,
        };
        let option = options[idx];
        let next = if argv_contains_fragment(&argv, option.argv) {
            argv_without_fragment(argv, option.argv)
        } else {
            argv_with_fragment(argv, option.argv)
        };
        self.set_effective_argv(&next, default);
    }

    /// Cycle the harness like [`Self::cycle_harness`], carrying a typed
    /// argv across: the old harness's launch option flags come off, and
    /// an argv that was only the old default is cleared.
    pub fn cycle_harness_keeping_argv(
        &mut self,
        delta: i32,
        default_for: &dyn Fn(&str) -> Vec<String>,
    ) {
        if self.known_harness_keys.is_empty() {
            return;
        }
        let old_default = default_for(self.harness.value().trim());
        let typed = parse_launch_argv(self.launch_argv.value().trim(), "launch").ok();
        self.cycle_harness(delta);
        let Some(typed) = typed.filter(|argv| !argv.is_empty()) else {
            return;
        };
        let stripped = strip_known_launch_option_fragments(typed);
        let new_default = default_for(self.harness.value().trim());
        if stripped == old_default {
            self.set_effective_argv(&[], &new_default);
        } else {
            self.set_effective_argv(&stripped, &new_default);
        }
    }

    /// `true` when the currently-typed mux name matches any known
    /// live mux (used by mux-launch's hard-error collision check).
    pub fn mux_name_collides_with_known(&self) -> bool {
        let name = self.mux_name.value().trim();
        if name.is_empty() {
            return false;
        }
        self.known_mux_names.iter().any(|known| known == name)
    }

    /// The known-mux entry whose native name matches the typed mux
    /// name, if any.
    pub fn matching_known_mux_name(&self) -> Option<&str> {
        let name = self.mux_name.value().trim();
        if name.is_empty() {
            return None;
        }
        self.known_mux_names
            .iter()
            .find(|known| known.as_str() == name)
            .map(String::as_str)
    }

    /// Route a key event into the cwd field and say what it did (`Tab`
    /// completes the path). Wrappers use this from their per-field
    /// dispatch when the cursor is on the cwd row.
    pub fn cwd_handle_key(&mut self, event: KeyEvent) -> Edit {
        self.cwd.handle(event)
    }

    /// Route a key event into whichever text field the caller names,
    /// and say what it did. Convenience for wrappers that switch on
    /// their own cursor enum and want to avoid boilerplate at each arm.
    pub fn handle_text_key(&mut self, field: SpecTextField, event: KeyEvent) -> Edit {
        match field {
            SpecTextField::Harness => self.harness.handle(event),
            SpecTextField::MuxName => self.mux_name.handle(event),
            SpecTextField::MuxSocket => self.mux_socket.handle(event),
            SpecTextField::LaunchArgv => self.launch_argv.handle(event),
            SpecTextField::WorktreeBranch => self.worktree_branch.handle(event),
        }
    }
}

/// Identifier for one of the primitive's text-input fields.
/// [`LaunchSpecFormState::handle_text_key`] uses this to route
/// per-field key events without exposing the underlying
/// `LineEdit` mutable borrow at every caller.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SpecTextField {
    Harness,
    MuxName,
    MuxSocket,
    LaunchArgv,
    WorktreeBranch,
}

// -- shared helpers ---------------------------------------------------

/// Trim + `Some`-ify a string, returning `None` when the trimmed
/// result is empty. Both wrappers use this when building their
/// commit request from optional fields (mux socket, worktree branch).
pub fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Shell-style tokenizer for the launch-argv override field. Same
/// grammar both wrappers used before extraction: double / single
/// quotes group, `\` escapes the next character, unquoted whitespace
/// separates tokens. `error_prefix` is prepended to any diagnostic so
/// each caller's error messages stay recognizable.
pub fn parse_launch_argv(raw: &str, error_prefix: &str) -> Result<Vec<String>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = raw.chars();
    let mut quote: Option<char> = None;
    let mut in_arg = false;
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(q), c) if c == q => {
                quote = None;
                in_arg = true;
            }
            (Some('"'), '\\') => match chars.next() {
                Some(next @ ('"' | '\\' | '$' | '`')) => {
                    current.push(next);
                    in_arg = true;
                }
                Some(next) => {
                    current.push('\\');
                    current.push(next);
                    in_arg = true;
                }
                None => return Err(format!("{error_prefix}: launch argv has a trailing escape")),
            },
            (Some(_), c) => {
                current.push(c);
                in_arg = true;
            }
            (None, '"' | '\'') => {
                quote = Some(ch);
                in_arg = true;
            }
            (None, '\\') => match chars.next() {
                Some(next) => {
                    current.push(next);
                    in_arg = true;
                }
                None => return Err(format!("{error_prefix}: launch argv has a trailing escape")),
            },
            (None, c) if c.is_whitespace() => {
                if in_arg {
                    args.push(std::mem::take(&mut current));
                    in_arg = false;
                }
            }
            (None, c) => {
                current.push(c);
                in_arg = true;
            }
        }
    }
    if let Some(q) = quote {
        return Err(format!(
            "{error_prefix}: launch argv has an unclosed `{q}` quote"
        ));
    }
    if in_arg {
        args.push(current);
    }
    Ok(args)
}

#[cfg(test)]
#[path = "launch_spec_form_tests.rs"]
mod tests;
