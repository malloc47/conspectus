//! Ephemeral harness launch form (ADR 0096).
//!
//! Up and Down move between rows; the mux name comes first, with a
//! checkbox on its row that reveals the mux socket field. Tab visits every
//! stop, the checkbox included, completes a path in the cwd field, and
//! moves on elsewhere. Left, Right, and Space act on the harness,
//! launch option, worktree, and pin rows.
//!
//! Multi-field modal that commits a [`Msg::CommitMuxLaunch`] the
//! runtime turns into a subprocess re-exec of
//! `conspectus mux launch <harness>`. On submit the mux and its
//! attributed harness session appear in discovery next refresh; the
//! form can also pin the mux (or only plan a pin) in the same step.
//!
//! An empty launch argv resolves to the default for the harness
//! (profile, `atelier exec`, or the harness default, ADR 0142), shown
//! under the field with its source.
//!
//! ADR 0097: the shared fields (harness, cwd,
//! mux name / socket, launch argv, worktree toggle + branch, known-
//! harness + known-live-mux collections, error) live on a
//! [`LaunchSpecFormState`] container this widget owns by composition.
//! Only the caller-specific `worktree_repo` field and the cursor +
//! renderer stay on this wrapper.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::macros::line;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use tuikit::frame::Lines;

use crate::tui::Msg;
use crate::tui::theme::Theme;
use crate::tui::widgets::form::{
    self, choice_row, derived_row, footer_lines, harness_row, input_row, options_row, path_row,
    static_value_row,
};
use crate::tui::widgets::launch_spec_form::{
    LaunchSpecFormState, LaunchSpecInit, SpecTextField, optional_string, parse_launch_argv,
};
use crate::tui::widgets::path_omnibox::PathOmniboxOutcome;
use tuikit::overlay::Outcome;
use tuikit::text_edit::LineEdit;

const HARNESS_LABEL: &str = "harness";
const CWD_LABEL: &str = "cwd";
const MUX_NAME_LABEL: &str = "mux name";
const MUX_SOCKET_LABEL: &str = "mux socket";
const LAUNCH_ARGV_LABEL: &str = "launch argv";
const WORKTREE_BRANCH_LABEL: &str = "branch";
const WORKTREE_REPO_LABEL: &str = "repo";

/// Ordered field identifiers for cursor placement. `WorktreeBranch`
/// and `WorktreeRepo` are only visible when the primitive's
/// worktree toggle is on.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MuxLaunchField {
    Harness,
    /// The harness's launch options, such as skipping permissions;
    /// only visible for harnesses that offer some.
    LaunchOptions,
    Cwd,
    MuxName,
    /// The checkbox on the name row that reveals the socket field.
    MuxSocketToggle,
    MuxSocket,
    LaunchArgv,
    WorktreeToggle,
    WorktreeBranch,
    WorktreeRepo,
    Pin,
}

impl MuxLaunchField {
    /// Whether the field takes typed text; the others are choices and
    /// toggles.
    pub fn takes_text(self) -> bool {
        !matches!(
            self,
            Self::LaunchOptions | Self::MuxSocketToggle | Self::WorktreeToggle | Self::Pin
        )
    }
}

/// Whether the launch also pins the new mux (ADR 0141).
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum PinChoice {
    #[default]
    Off,
    /// `--pin`: launch, then pin the new mux.
    Pin,
    /// `--pin --no-launch`: add a planned pin and start nothing.
    PlanOnly,
}

impl PinChoice {
    fn cycled(self, delta: i32) -> Self {
        const ORDER: [PinChoice; 3] = [PinChoice::Off, PinChoice::Pin, PinChoice::PlanOnly];
        let idx = ORDER.iter().position(|c| *c == self).unwrap_or(0);
        ORDER[tuikit::scroll::wrap_step(idx, ORDER.len(), delta)]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Off => "don't pin",
            Self::Pin => "launch, then pin",
            Self::PlanOnly => "pin, don't launch",
        }
    }
}

/// Request shape the runtime translates into
/// `conspectus mux launch` argv. Pure data — no state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxLaunchRequest {
    pub harness: String,
    pub name: String,
    pub cwd: String,
    pub mux_socket: Option<String>,
    pub launch_argv: Vec<String>,
    pub worktree_branch: Option<String>,
    pub worktree_repo: Option<String>,
    pub pin: PinChoice,
}

/// Ephemeral harness launch form state. Owns the shared launch-spec
/// primitive plus the mux-launch-only `worktree_repo` field and the
/// cursor.
#[derive(Debug, Clone)]
pub struct MuxLaunchFormState {
    spec: LaunchSpecFormState,
    worktree_repo: LineEdit,
    focus: MuxLaunchField,
    pin: PinChoice,
    /// Whether the mux socket field is shown and used. Off means the
    /// default tmux server; the typed socket is kept while it is off.
    socket_enabled: bool,
    /// Where an empty launch argv comes from, so the form can show the
    /// command it will run (ADR 0142).
    defaults: Option<crate::launch_default::LaunchDefaults>,
}

impl MuxLaunchFormState {
    /// New form with pre-populated defaults. Focus starts on the
    /// mux name, the first field.
    pub fn new(
        harness: impl Into<String>,
        cwd: impl Into<String>,
        mux_name: impl Into<String>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
    ) -> Self {
        let spec = LaunchSpecFormState::new(LaunchSpecInit {
            harness_value: harness.into(),
            cwd_value: cwd.into(),
            mux_name_value: mux_name.into(),
            worktree_branch_value: String::new(),
            known_harness_keys,
            known_mux_names,
        });
        Self {
            spec,
            worktree_repo: LineEdit::default(),
            focus: MuxLaunchField::MuxName,
            pin: PinChoice::Off,
            socket_enabled: false,
            defaults: None,
        }
    }

    /// Show the argv an empty `launch argv` field resolves to.
    #[must_use]
    pub fn with_defaults(mut self, defaults: crate::launch_default::LaunchDefaults) -> Self {
        self.defaults = Some(defaults);
        self
    }

    /// Seed the worktree repo path without turning the toggle on, so
    /// switching it on later starts from the launch directory.
    #[must_use]
    pub fn with_worktree_repo(mut self, repo: impl Into<String>) -> Self {
        self.worktree_repo = LineEdit::new(repo);
        self
    }

    /// Start with the worktree toggle on.
    #[must_use]
    pub fn with_worktree(mut self, repo: impl Into<String>) -> Self {
        self.spec.set_worktree_enabled(true);
        self.worktree_repo = LineEdit::new(repo);
        self
    }

    /// The command an empty argv field launches, and where it comes
    /// from: `claude --x (project launch profile)`.
    pub fn default_hint(&self) -> Option<String> {
        let defaults = self.defaults.as_ref()?;
        if !self.spec.launch_argv().value().trim().is_empty() {
            return None;
        }
        let cwd = self.spec.cwd().expanded_value();
        let (argv, source) = defaults.default_argv(
            self.spec.harness().value().trim(),
            (!cwd.is_empty()).then(|| std::path::Path::new(&cwd)),
        )?;
        let command: Vec<String> = argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        Some(format!("{} — from {}", command.join(" "), source.label()))
    }

    /// Offer these directories when completing the cwd field.
    #[must_use]
    pub fn with_cwd_candidates(
        mut self,
        candidates: Vec<crate::tui::widgets::path_omnibox::PathCandidate>,
    ) -> Self {
        self.spec.set_cwd_candidates(candidates);
        self
    }

    fn default_argv(&self) -> Vec<String> {
        form::default_argv(
            self.defaults.as_ref(),
            self.spec.harness().value().trim(),
            &self.spec.cwd().expanded_value(),
        )
    }

    /// Borrow the shared spec (read-only).
    pub fn spec(&self) -> &LaunchSpecFormState {
        &self.spec
    }

    /// Borrow the shared spec (mutable) — used by wrapper-internal
    /// helpers and by tests that want to poke a shared field.
    pub fn spec_mut(&mut self) -> &mut LaunchSpecFormState {
        &mut self.spec
    }

    pub fn focus(&self) -> MuxLaunchField {
        self.focus
    }

    pub fn error(&self) -> Option<&str> {
        self.spec.error()
    }

    pub fn worktree_repo_value(&self) -> &str {
        self.worktree_repo.value()
    }

    /// The rows, top to bottom. The socket checkbox shares the name row,
    /// so it is not a row of its own.
    fn visible_fields(&self) -> Vec<MuxLaunchField> {
        let mut fields = vec![MuxLaunchField::MuxName];
        if self.socket_enabled {
            fields.push(MuxLaunchField::MuxSocket);
        }
        fields.push(MuxLaunchField::Harness);
        if !self.spec.launch_options().is_empty() {
            fields.push(MuxLaunchField::LaunchOptions);
        }
        fields.extend([
            MuxLaunchField::Cwd,
            MuxLaunchField::LaunchArgv,
            MuxLaunchField::WorktreeToggle,
        ]);
        if self.spec.worktree_enabled() {
            fields.push(MuxLaunchField::WorktreeBranch);
            fields.push(MuxLaunchField::WorktreeRepo);
        }
        fields.push(MuxLaunchField::Pin);
        fields
    }

    /// Up and Down: move between rows. On the socket checkbox, the row
    /// is the name row.
    fn advance_row(&mut self, delta: i32) {
        let fields = self.visible_fields();
        let at = if self.focus == MuxLaunchField::MuxSocketToggle {
            MuxLaunchField::MuxName
        } else {
            self.focus
        };
        let idx = fields.iter().position(|f| *f == at).unwrap_or(0);
        self.focus = fields[tuikit::selection::Ends::Wrap.step(idx, fields.len(), delta)];
    }

    /// Tab and Shift-Tab: every stop, the socket checkbox included
    /// (right after the name).
    fn advance_stop(&mut self, delta: i32) {
        let mut stops = self.visible_fields();
        stops.insert(1, MuxLaunchField::MuxSocketToggle);
        let idx = stops.iter().position(|f| *f == self.focus).unwrap_or(0);
        self.focus = stops[tuikit::selection::Ends::Wrap.step(idx, stops.len(), delta)];
    }

    fn try_commit(&mut self) -> Outcome<Msg> {
        match self.build_request() {
            Ok(req) => Outcome::Commit(Msg::CommitMuxLaunch(req)),
            Err(msg) => {
                self.spec.set_error(msg);
                Outcome::Consumed
            }
        }
    }

    fn build_request(&self) -> Result<MuxLaunchRequest, String> {
        let harness = self.spec.harness().value().trim().to_string();
        if harness.is_empty() {
            return Err("mux launch: harness is required".to_string());
        }
        let name = self.spec.mux_name().value().trim().to_string();
        if name.is_empty() {
            return Err("mux launch: mux name is required".to_string());
        }
        if self.spec.mux_name_collides_with_known() {
            return Err(format!(
                "mux launch: tmux session `{name}` is already live; pick another name"
            ));
        }
        let cwd = self.spec.cwd().expanded_value();
        if cwd.is_empty() {
            return Err("mux launch: cwd is required".to_string());
        }
        let mux_socket = if self.socket_enabled {
            optional_string(self.spec.mux_socket().value())
        } else {
            None
        };
        let launch_argv = parse_launch_argv(self.spec.launch_argv().value().trim(), "mux launch")?;
        let (worktree_branch, worktree_repo) = if self.spec.worktree_enabled() {
            let branch = self.spec.worktree_branch().value().trim().to_string();
            if branch.is_empty() {
                return Err("mux launch: worktree branch is required".to_string());
            }
            let repo = crate::tui::widgets::path_omnibox::expand_home_prefix(
                self.worktree_repo.value().trim(),
            );
            if repo.is_empty() {
                return Err("mux launch: worktree repo is required".to_string());
            }
            (Some(branch), Some(repo))
        } else {
            (None, None)
        };
        Ok(MuxLaunchRequest {
            harness,
            name,
            cwd,
            mux_socket,
            launch_argv,
            worktree_branch,
            worktree_repo,
            pin: self.pin,
        })
    }
}

impl MuxLaunchFormState {
    /// One key: edit, move between the rows, change an option, launch, or close.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<Msg> {
        use crate::tui::keybindings::overlay::MUX_LAUNCH_KEYS;
        use tuikit::keys::{FormKey, resolve};
        let option_delta = match resolve(MUX_LAUNCH_KEYS, key) {
            Some(FormKey::Abort | FormKey::Cancel) => return Outcome::Close,
            Some(FormKey::Up) => {
                self.advance_row(-1);
                return Outcome::Consumed;
            }
            Some(FormKey::Back) => {
                self.advance_stop(-1);
                return Outcome::Consumed;
            }
            Some(FormKey::Down) => {
                self.advance_row(1);
                return Outcome::Consumed;
            }
            Some(FormKey::Tab) => {
                // Tab completes a path in the cwd field, and moves on
                // when there is nothing to complete.
                if self.focus == MuxLaunchField::Cwd {
                    match self.spec.cwd_handle_key(key) {
                        PathOmniboxOutcome::Completed | PathOmniboxOutcome::Changed => {
                            self.spec.clear_error();
                            return Outcome::Consumed;
                        }
                        PathOmniboxOutcome::NoCompletion | PathOmniboxOutcome::Continue => {}
                    }
                }
                self.advance_stop(1);
                return Outcome::Consumed;
            }
            Some(FormKey::Submit) => return self.try_commit(),
            Some(FormKey::OptionPrev) => Some(-1),
            Some(FormKey::OptionNext) => Some(1),
            Some(FormKey::Inherit) | None => None,
        };
        // Option keys act on the choice and toggle rows; on a text
        // field they fall through and edit the text.
        match self.focus {
            MuxLaunchField::Harness => {
                if let Some(delta) = option_delta {
                    let cwd = self.spec.cwd().expanded_value();
                    let defaults = self.defaults.clone();
                    self.spec.cycle_harness_keeping_argv(delta, &|harness| {
                        form::default_argv(defaults.as_ref(), harness, &cwd)
                    });
                    self.spec.clear_error();
                    return Outcome::Consumed;
                }
            }
            MuxLaunchField::LaunchOptions => {
                if let Some(delta) = option_delta {
                    let default = self.default_argv();
                    self.spec.toggle_launch_option(&default, delta);
                    self.spec.clear_error();
                }
                return Outcome::Consumed;
            }
            MuxLaunchField::Pin => {
                if let Some(delta) = option_delta {
                    self.pin = self.pin.cycled(delta);
                    self.spec.clear_error();
                }
                return Outcome::Consumed;
            }
            MuxLaunchField::MuxSocketToggle => {
                if option_delta.is_some() {
                    self.socket_enabled = !self.socket_enabled;
                    self.spec.clear_error();
                }
                return Outcome::Consumed;
            }
            MuxLaunchField::WorktreeToggle => {
                if option_delta.is_some() {
                    self.spec.toggle_worktree();
                    self.spec.clear_error();
                }
                return Outcome::Consumed;
            }
            MuxLaunchField::Cwd => {
                let outcome = self.spec.cwd_handle_key(key);
                if matches!(
                    outcome,
                    PathOmniboxOutcome::Changed | PathOmniboxOutcome::Completed
                ) {
                    self.spec.clear_error();
                }
                return Outcome::Consumed;
            }
            _ => {}
        }
        // A key on a text field edits it; any key there clears the
        // error, as the form's other fields do when they act.
        let field = match self.focus {
            MuxLaunchField::Harness => Some(SpecTextField::Harness),
            MuxLaunchField::MuxName => Some(SpecTextField::MuxName),
            MuxLaunchField::MuxSocket => Some(SpecTextField::MuxSocket),
            MuxLaunchField::LaunchArgv => Some(SpecTextField::LaunchArgv),
            MuxLaunchField::WorktreeBranch => Some(SpecTextField::WorktreeBranch),
            MuxLaunchField::WorktreeRepo => {
                self.worktree_repo.handle(key);
                self.spec.clear_error();
                return Outcome::Consumed;
            }
            MuxLaunchField::LaunchOptions
            | MuxLaunchField::MuxSocketToggle
            | MuxLaunchField::WorktreeToggle
            | MuxLaunchField::Cwd
            | MuxLaunchField::Pin => None,
        };
        if let Some(field) = field {
            self.spec.handle_text_key(field, key);
            self.spec.clear_error();
        }
        Outcome::Consumed
    }
}

/// Renderer for the mux-launch form. Bordered popup with title,
/// per-field rows, worktree toggle indicator, and a footer hint.
pub struct MuxLaunchFormWidget<'a> {
    state: &'a MuxLaunchFormState,
    theme: &'a Theme,
}

impl<'a> MuxLaunchFormWidget<'a> {
    pub fn new(state: &'a MuxLaunchFormState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for MuxLaunchFormWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let state = self.state;
        let spec = state.spec();
        let frame = form::modal_frame(self.theme, "Launch harness in new mux");
        let inner_width = usize::from(frame.inner_width(area));
        let footer = footer_lines(
            &tuikit::keys::footer(crate::tui::keybindings::overlay::MUX_LAUNCH_KEYS),
            inner_width,
            self.theme,
        );
        let cols = form::Cols::new(inner_width);
        let focus = state.focus;
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut cursor_line = 0;
        let mut row = |lines: &mut Vec<Line<'static>>,
                       field: MuxLaunchField,
                       line: Line<'static>| {
            if field == focus
                || (field == MuxLaunchField::MuxName && focus == MuxLaunchField::MuxSocketToggle)
            {
                cursor_line = lines.len();
            }
            lines.push(line);
        };
        row(
            &mut lines,
            MuxLaunchField::MuxName,
            state.name_line(cols, self.theme),
        );
        if state.socket_enabled {
            row(
                &mut lines,
                MuxLaunchField::MuxSocket,
                input_row(
                    MUX_SOCKET_LABEL,
                    spec.mux_socket(),
                    focus == MuxLaunchField::MuxSocket,
                    cols,
                    self.theme,
                ),
            );
        }
        row(
            &mut lines,
            MuxLaunchField::Harness,
            harness_row(
                HARNESS_LABEL,
                spec.harness(),
                spec.known_harness_keys(),
                focus == MuxLaunchField::Harness,
                cols,
                self.theme,
            ),
        );
        if !spec.launch_options().is_empty() {
            let default = state.default_argv();
            let effective = spec.effective_argv(&default).unwrap_or_default();
            row(
                &mut lines,
                MuxLaunchField::LaunchOptions,
                options_row(
                    "options",
                    spec.launch_options(),
                    &effective,
                    focus == MuxLaunchField::LaunchOptions,
                    cols,
                    self.theme,
                ),
            );
        }
        row(
            &mut lines,
            MuxLaunchField::Cwd,
            path_row(
                CWD_LABEL,
                spec.cwd(),
                focus == MuxLaunchField::Cwd,
                cols,
                self.theme,
            ),
        );
        row(
            &mut lines,
            MuxLaunchField::LaunchArgv,
            input_row(
                LAUNCH_ARGV_LABEL,
                spec.launch_argv(),
                focus == MuxLaunchField::LaunchArgv,
                cols,
                self.theme,
            ),
        );
        lines.push(state.command_line(cols, self.theme));
        row(
            &mut lines,
            MuxLaunchField::WorktreeToggle,
            static_value_row(
                "worktree",
                if spec.worktree_enabled() {
                    "[x] create or reuse a worktree for the branch"
                } else {
                    "[ ] no worktree"
                },
                focus == MuxLaunchField::WorktreeToggle,
                cols,
                self.theme,
            ),
        );
        if spec.worktree_enabled() {
            row(
                &mut lines,
                MuxLaunchField::WorktreeBranch,
                input_row(
                    WORKTREE_BRANCH_LABEL,
                    spec.worktree_branch(),
                    focus == MuxLaunchField::WorktreeBranch,
                    cols,
                    self.theme,
                ),
            );
            row(
                &mut lines,
                MuxLaunchField::WorktreeRepo,
                input_row(
                    WORKTREE_REPO_LABEL,
                    &state.worktree_repo,
                    focus == MuxLaunchField::WorktreeRepo,
                    cols,
                    self.theme,
                ),
            );
        }
        row(
            &mut lines,
            MuxLaunchField::Pin,
            choice_row(
                "pin",
                &[
                    (PinChoice::Off.label(), state.pin == PinChoice::Off),
                    (PinChoice::Pin.label(), state.pin == PinChoice::Pin),
                    (
                        PinChoice::PlanOnly.label(),
                        state.pin == PinChoice::PlanOnly,
                    ),
                ],
                focus == MuxLaunchField::Pin,
                cols,
                self.theme,
            ),
        );
        if let Some(err) = spec.error() {
            lines.push(line![""]);
            lines.push(line![ratatui::text::Span::styled(
                err.to_string(),
                Style::default().fg(self.theme.error)
            )]);
        }
        lines.push(line![""]);
        for footer_line in footer {
            lines.push(footer_line);
        }

        frame.render(
            area,
            buf,
            Lines::new(lines).following(cursor_line..cursor_line + 1),
        );
    }
}

impl MuxLaunchFormState {
    /// The name row: the name field with the socket checkbox to its
    /// right. The checkbox takes the same cells checked or not, and the
    /// row carries the focus marker when either part has focus.
    fn name_line(&self, cols: form::Cols, theme: &Theme) -> Line<'static> {
        use ratatui::text::Span;
        let name_focused = self.focus == MuxLaunchField::MuxName;
        let toggle_focused = self.focus == MuxLaunchField::MuxSocketToggle;
        let text = form::checkbox("socket", self.socket_enabled);
        let suffix = form::Suffix {
            width: tuikit::text::width(&text) + 2,
            spans: vec![
                Span::raw("  "),
                Span::styled(text, form::entry_style(toggle_focused, theme)),
            ],
        };
        let line = form::value_row(
            MUX_NAME_LABEL,
            self.spec.mux_name(),
            name_focused,
            cols,
            Some(suffix),
            theme,
        );
        if toggle_focused {
            form::with_cursor(line, theme)
        } else {
            line
        }
    }

    /// The command row: what the form will run, and where an empty
    /// argv field gets it. Drawn as derived output, not as a field.
    fn command_line(&self, cols: form::Cols, theme: &Theme) -> Line<'static> {
        let typed = self.spec.launch_argv().value().trim();
        if !typed.is_empty() {
            return derived_row("command", typed, false, cols, theme);
        }
        if let Some(hint) = self.default_hint() {
            return derived_row("command", &hint, false, cols, theme);
        }
        let default = self.default_argv();
        if default.is_empty() {
            derived_row("command", "no default for this harness", true, cols, theme)
        } else {
            derived_row("command", &form::display_argv(&default), false, cols, theme)
        }
    }
}

#[cfg(test)]
#[path = "mux_launch_tests.rs"]
mod tests;
