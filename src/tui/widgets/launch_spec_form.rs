//! Shared launch-spec form primitive (H-MUX-LAUNCH-002 / ADR 0097).
//!
//! Container for the fields that pin-create and mux-launch share:
//! harness, cwd (with autocomplete), mux name + socket, launch argv
//! override, worktree toggle + branch, plus known-harness and known-
//! live-mux collections used for autocomplete + collision detection.
//!
//! Wrappers ([`crate::tui::widgets::pins::PinCreateState`],
//! [`crate::tui::widgets::mux_launch::MuxLaunchFormState`]) hold this
//! by composition, own their own cursor sequence + rendering, and
//! decide which of the primitive's fields are visible in their layout.
//! Neither wrapper reads or writes the shared fields except through
//! this struct's accessors.
//!
//! No cursor lives here. No Msg dispatch lives here. The primitive is
//! pure state + a handful of validation helpers that both wrappers
//! call from their own `Enter`/commit paths.

use ratatui::crossterm::event::KeyEvent;

use crate::tui::widgets::input::{InputOutcome, TextInputState};
use crate::tui::widgets::path_omnibox::{PathCandidate, PathOmniboxOutcome, PathOmniboxState};

/// Owned state for the launch-spec form's shared fields. Both
/// `PinCreateState` and `MuxLaunchFormState` embed this exactly once
/// and route every read/write of a shared field through the accessors.
#[derive(Debug, Clone)]
pub struct LaunchSpecFormState {
    harness: TextInputState,
    cwd: PathOmniboxState,
    mux_name: TextInputState,
    mux_socket: TextInputState,
    launch_argv: TextInputState,
    worktree_enabled: bool,
    worktree_branch: TextInputState,
    known_harness_keys: Vec<String>,
    known_mux_names: Vec<String>,
    error: Option<String>,
}

/// Constructor input for [`LaunchSpecFormState::new`]. Grouped into a
/// single struct so the primitive stays under the clippy 7-argument
/// budget and readers can spot each field without matching positional
/// arguments to parameter names.
#[derive(Debug, Clone)]
pub struct LaunchSpecInit<'a> {
    pub harness_label: &'a str,
    pub harness_value: String,
    pub cwd_label: &'a str,
    pub cwd_value: String,
    pub mux_name_label: &'a str,
    pub mux_name_value: String,
    pub mux_socket_label: &'a str,
    pub launch_argv_label: &'a str,
    pub worktree_branch_label: &'a str,
    pub worktree_branch_value: String,
    pub known_harness_keys: Vec<String>,
    pub known_mux_names: Vec<String>,
}

impl LaunchSpecFormState {
    /// Build a spec with pre-populated defaults. The caller decides
    /// the field labels (they show up on the wrapper's rendered rows,
    /// not inside the primitive) — pass the same labels the previous
    /// standalone widgets used so snapshot tests stay green.
    pub fn new(init: LaunchSpecInit<'_>) -> Self {
        Self {
            harness: TextInputState::new(init.harness_label.to_string(), init.harness_value),
            cwd: PathOmniboxState::new(init.cwd_label.to_string(), init.cwd_value),
            mux_name: TextInputState::new(init.mux_name_label.to_string(), init.mux_name_value),
            mux_socket: TextInputState::new(init.mux_socket_label.to_string(), String::new()),
            launch_argv: TextInputState::new(init.launch_argv_label.to_string(), String::new()),
            worktree_enabled: false,
            worktree_branch: TextInputState::new(
                init.worktree_branch_label.to_string(),
                init.worktree_branch_value,
            ),
            known_harness_keys: init.known_harness_keys,
            known_mux_names: init.known_mux_names,
            error: None,
        }
    }

    // -- read accessors ---------------------------------------------

    pub fn harness(&self) -> &TextInputState {
        &self.harness
    }
    pub fn cwd(&self) -> &PathOmniboxState {
        &self.cwd
    }
    pub fn mux_name(&self) -> &TextInputState {
        &self.mux_name
    }
    pub fn mux_socket(&self) -> &TextInputState {
        &self.mux_socket
    }
    pub fn launch_argv(&self) -> &TextInputState {
        &self.launch_argv
    }
    pub fn worktree_enabled(&self) -> bool {
        self.worktree_enabled
    }
    pub fn worktree_branch(&self) -> &TextInputState {
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

    pub fn harness_mut(&mut self) -> &mut TextInputState {
        &mut self.harness
    }
    pub fn cwd_mut(&mut self) -> &mut PathOmniboxState {
        &mut self.cwd
    }
    pub fn mux_name_mut(&mut self) -> &mut TextInputState {
        &mut self.mux_name
    }
    pub fn mux_socket_mut(&mut self) -> &mut TextInputState {
        &mut self.mux_socket
    }
    pub fn launch_argv_mut(&mut self) -> &mut TextInputState {
        &mut self.launch_argv
    }
    pub fn worktree_branch_mut(&mut self) -> &mut TextInputState {
        &mut self.worktree_branch
    }

    // -- state mutation helpers -------------------------------------

    /// Replace the harness field wholesale. Used when a wrapper cycles
    /// through `known_harness_keys` or when a mode change re-seeds
    /// the harness default.
    pub fn set_harness(&mut self, title: impl Into<String>, value: impl Into<String>) {
        self.harness = TextInputState::new(title.into(), value);
    }

    /// Replace the mux-name field wholesale — the pin-create adopt
    /// flow re-derives this from the current mode / id, so the
    /// wholesale-replace shape matches the existing behavior.
    pub fn set_mux_name(&mut self, title: impl Into<String>, value: impl Into<String>) {
        self.mux_name = TextInputState::new(title.into(), value);
    }

    /// Replace the launch-argv field wholesale (harness cycling in
    /// pin-create rebuilds this when the previous value matched the
    /// old harness's default).
    pub fn set_launch_argv(&mut self, title: impl Into<String>, value: impl Into<String>) {
        self.launch_argv = TextInputState::new(title.into(), value);
    }

    /// Toggle the worktree flag. Returns the new value.
    pub fn toggle_worktree(&mut self) -> bool {
        self.worktree_enabled = !self.worktree_enabled;
        self.worktree_enabled
    }

    pub fn set_worktree_enabled(&mut self, enabled: bool) {
        self.worktree_enabled = enabled;
    }

    pub fn set_cwd_candidates(&mut self, candidates: Vec<PathCandidate>) {
        self.cwd.set_known_candidates(candidates);
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
    pub fn cycle_harness(&mut self, delta: i32, harness_label: &str) -> Option<String> {
        if self.known_harness_keys.is_empty() {
            return None;
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
        let next = self.known_harness_keys[idx].clone();
        self.harness = TextInputState::new(harness_label.to_string(), next.clone());
        Some(next)
    }

    /// `true` when the currently-typed mux name matches any known
    /// live mux (used by pin-create's adopt trigger and mux-launch's
    /// hard-error collision check).
    pub fn mux_name_collides_with_known(&self) -> bool {
        let name = self.mux_name.value().trim();
        if name.is_empty() {
            return false;
        }
        self.known_mux_names.iter().any(|known| known == name)
    }

    /// The known-mux entry whose native name matches the typed mux
    /// name, if any. Used by pin-create's adopt provenance.
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

    /// Route a key event into the cwd omnibox and return the
    /// omnibox's outcome. Wrappers use this from their per-field
    /// dispatch when the cursor is on the cwd row.
    pub fn cwd_handle_key(&mut self, event: KeyEvent) -> PathOmniboxOutcome {
        self.cwd.handle_key(event)
    }

    /// Route a key event into whichever `TextInputState` field the
    /// caller names. Convenience for wrappers that switch on their
    /// own cursor enum and want to avoid boilerplate at each arm.
    pub fn handle_text_key(&mut self, field: SpecTextField, event: KeyEvent) -> InputOutcome {
        match field {
            SpecTextField::Harness => self.harness.handle_key(event),
            SpecTextField::MuxName => self.mux_name.handle_key(event),
            SpecTextField::MuxSocket => self.mux_socket.handle_key(event),
            SpecTextField::LaunchArgv => self.launch_argv.handle_key(event),
            SpecTextField::WorktreeBranch => self.worktree_branch.handle_key(event),
        }
    }
}

/// Identifier for one of the primitive's text-input fields.
/// [`LaunchSpecFormState::handle_text_key`] uses this to route
/// per-field key events without exposing the underlying
/// `TextInputState` mutable borrow at every caller.
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
mod tests {
    use super::*;

    fn state() -> LaunchSpecFormState {
        LaunchSpecFormState::new(LaunchSpecInit {
            harness_label: "Harness",
            harness_value: "codex".to_string(),
            cwd_label: "Cwd",
            cwd_value: "/tmp".to_string(),
            mux_name_label: "Mux name",
            mux_name_value: "adhoc".to_string(),
            mux_socket_label: "Mux socket",
            launch_argv_label: "Launch argv",
            worktree_branch_label: "Worktree branch",
            worktree_branch_value: String::new(),
            known_harness_keys: vec!["codex".to_string(), "claude-code".to_string()],
            known_mux_names: vec!["existing".to_string()],
        })
    }

    #[test]
    fn cycle_harness_advances_and_wraps() {
        let mut s = state();
        assert_eq!(s.harness().value(), "codex");
        let next = s.cycle_harness(1, "Harness").expect("cycled");
        assert_eq!(next, "claude-code");
        assert_eq!(s.harness().value(), "claude-code");
        // Wrap around from last back to first.
        let next = s.cycle_harness(1, "Harness").expect("cycled");
        assert_eq!(next, "codex");
    }

    #[test]
    fn cycle_harness_returns_none_when_known_set_is_empty() {
        let mut s = LaunchSpecFormState::new(LaunchSpecInit {
            harness_label: "Harness",
            harness_value: "codex".to_string(),
            cwd_label: "Cwd",
            cwd_value: "/tmp".to_string(),
            mux_name_label: "Mux name",
            mux_name_value: "adhoc".to_string(),
            mux_socket_label: "Mux socket",
            launch_argv_label: "Launch argv",
            worktree_branch_label: "Worktree branch",
            worktree_branch_value: String::new(),
            known_harness_keys: Vec::new(),
            known_mux_names: Vec::new(),
        });
        assert!(s.cycle_harness(1, "Harness").is_none());
        assert_eq!(s.harness().value(), "codex");
    }

    #[test]
    fn mux_name_collides_flags_known_names() {
        let mut s = state();
        s.set_mux_name("Mux name", "existing");
        assert!(s.mux_name_collides_with_known());
        assert_eq!(s.matching_known_mux_name(), Some("existing"));
    }

    #[test]
    fn toggle_worktree_flips_the_flag() {
        let mut s = state();
        assert!(!s.worktree_enabled());
        assert!(s.toggle_worktree());
        assert!(s.worktree_enabled());
        assert!(!s.toggle_worktree());
    }

    #[test]
    fn parse_launch_argv_handles_single_quotes() {
        let out = parse_launch_argv("codex --model 'gpt-5 pro'", "test").expect("parse");
        assert_eq!(out, vec!["codex", "--model", "gpt-5 pro"]);
    }

    #[test]
    fn parse_launch_argv_rejects_unclosed_quote() {
        let err = parse_launch_argv("codex --flag 'oops", "test").expect_err("unclosed");
        assert!(err.contains("unclosed"));
    }

    #[test]
    fn parse_launch_argv_empty_input_is_empty_vec() {
        assert!(parse_launch_argv("", "test").unwrap().is_empty());
    }

    #[test]
    fn optional_string_trims_and_filters_empty() {
        assert!(optional_string("  ").is_none());
        assert_eq!(optional_string(" foo ").as_deref(), Some("foo"));
    }
}
