//! The mux launch form (ADR 0096): start a harness in a new tmux session,
//! optionally in a worktree, and optionally pin the session.
//!
//! The form is the framework's `Form`, so it moves, submits, and refuses
//! the way every form does: one ring of fields (`Tab`/`↓` next,
//! `S-Tab`/`↑` previous), `Enter` launches from any of them, and a field
//! that stops the launch says why on the error line and takes the focus.
//! The `server` checkbox is a field of its own; switching it on shows the
//! `socket` field under it, as the `worktree` toggle shows `branch` and
//! `repo`.
//!
//! This module keeps what only Conspectus knows: the fields, their order,
//! and their checks; the request `Enter` commits ([`MuxLaunchRequest`],
//! which the runtime turns into `conspectus mux launch`); and the fields
//! kept in step with the others after each key. Those are the harness's
//! launch options, shown as checkboxes that add or remove their flags in
//! the argv; a typed argv carried across a change of harness; and the
//! `command` row, which shows the typed argv or the command an empty one
//! means (a launch profile, `atelier exec`, or the harness default, ADR
//! 0142) and where it comes from. That default is decided when the form
//! opens and again only when the harness or the cwd changes, so drawing
//! the form reads no files.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use tuikit::form::{
    CheckGroup, Choice, Field, FieldKind, Form, OneOf, ReadOnly, Submit, Toggle, Tone, Values,
};
use tuikit::keys::{FormKey, resolve};
use tuikit::overlay::{Outcome, Overlay};
use tuikit::path_field::{Candidate, PathField};
use tuikit::text_edit::LineEdit;

use crate::argv::{parse_command_line, split_command_line};
use crate::discovery::harness::launch_options_for;
use crate::launch_default::LaunchDefaults;
use crate::tui::Msg;
use crate::tui::keybindings::overlay::MUX_LAUNCH_KEYS;
use crate::tui::widgets::launch_argv::{self, DefaultArgv};

/// The form's title.
const TITLE: &str = "Launch harness in new mux";

/// The mux name: the first field. Each field's id is also its label.
pub const NAME: &str = "name";
/// The checkbox that shows the socket field: off is the default server.
pub const SERVER: &str = "server";
/// The socket of a server other than the default, shown while the server
/// checkbox is on.
pub const SOCKET: &str = "socket";
/// The harness: one of the known harnesses, or one typed in.
pub const HARNESS: &str = "harness";
/// The harness's launch options as checkboxes, shown for a harness that
/// has some.
pub const OPTIONS: &str = "options";
/// The directory the harness starts in.
pub const CWD: &str = "cwd";
/// The argv to launch; empty for the default.
pub const ARGV: &str = "argv";
/// The command the form will run, derived from the fields above it.
pub const COMMAND: &str = "command";
/// Whether to create or reuse a worktree for a branch.
pub const WORKTREE: &str = "worktree";
/// The worktree's branch, shown while the worktree toggle is on.
pub const BRANCH: &str = "branch";
/// The repository the worktree belongs to, shown with the branch.
pub const REPO: &str = "repo";
/// Whether the launch also pins the new mux.
pub const PIN: &str = "pin";

/// What the server checkbox says after its box.
const SERVER_TEXT: &str = "own socket";

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
    /// The choices in the order the pin row shows them.
    const ORDER: [PinChoice; 3] = [PinChoice::Off, PinChoice::Pin, PinChoice::PlanOnly];

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

/// The launch form: the framework form and what keeps its derived fields
/// in step.
#[derive(Debug, Clone)]
pub struct MuxLaunchForm {
    form: Form<Msg>,
    /// Where an empty argv comes from (ADR 0142).
    defaults: Option<LaunchDefaults>,
    /// The default argv for the harness and cwd as they are now.
    default: DefaultArgv,
}

impl MuxLaunchForm {
    /// The form with its first values: `harness` among
    /// `known_harness_keys`, `cwd`, and `mux_name`, which must not be one
    /// of `known_mux_names` (the live sessions). The focus starts on the
    /// name.
    pub fn new(
        harness: impl Into<String>,
        cwd: impl Into<String>,
        mux_name: impl Into<String>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
    ) -> Self {
        let pin_choices = PinChoice::ORDER
            .iter()
            .map(|choice| choice.label().to_string())
            .collect();
        let form = Form::new(TITLE, MUX_LAUNCH_KEYS, request)
            .field(Field::text(NAME, NAME, mux_name).validate(name_check(known_mux_names)))
            .field(Field::toggle(
                SERVER,
                SERVER,
                Toggle::new(false, SERVER_TEXT),
            ))
            .field(Field::text(SOCKET, SOCKET, "").visible_if(|values| values.is_on(SERVER)))
            .field(
                Field::choice(
                    HARNESS,
                    HARNESS,
                    Choice::new(harness, known_harness_keys).custom(true),
                )
                .validate(required("harness")),
            )
            .field(
                Field::check_group(
                    OPTIONS,
                    OPTIONS,
                    CheckGroup::new(Vec::<(String, bool)>::new()),
                )
                .visible_if(|values| !launch_options_for(values.text(HARNESS).trim()).is_empty()),
            )
            .field(
                Field::path(
                    CWD,
                    CWD,
                    crate::tui::paths::directory_field(cwd, Vec::new()),
                )
                .validate(required("cwd")),
            )
            .field(Field::text(ARGV, ARGV, "").validate(argv_check))
            .field(Field::read_only(
                COMMAND,
                COMMAND,
                ReadOnly::new("", Tone::Derived),
            ))
            .field(Field::toggle(
                WORKTREE,
                WORKTREE,
                Toggle::new(false, "create or reuse a worktree for the branch")
                    .off_text("no worktree"),
            ))
            .field(
                Field::text(BRANCH, BRANCH, "")
                    .visible_if(worktree_on)
                    .validate(required("worktree branch")),
            )
            .field(
                Field::path(
                    REPO,
                    REPO,
                    crate::tui::paths::directory_field("", Vec::new()),
                )
                .visible_if(worktree_on)
                .validate(required("worktree repo")),
            )
            .field(Field::one_of(PIN, PIN, OneOf::new(pin_choices, 0)));
        let mut launch = Self {
            form,
            defaults: None,
            default: DefaultArgv::default(),
        };
        launch.default = launch.decide_default();
        launch.follow();
        launch
    }

    /// Where an empty argv comes from, so the command row can show what it
    /// launches.
    #[must_use]
    pub fn with_defaults(mut self, defaults: LaunchDefaults) -> Self {
        self.defaults = Some(defaults);
        self.default = self.decide_default();
        self.follow();
        self
    }

    /// Seeds the worktree repo without turning the toggle on, so
    /// switching it on later starts from the launch directory.
    #[must_use]
    pub fn with_worktree_repo(mut self, repo: impl Into<String>) -> Self {
        if let Some(FieldKind::Path(path)) = self.form.get_mut(REPO).map(Field::kind_mut) {
            path.set_value(repo);
        }
        self
    }

    /// Starts with the worktree toggle on and the repo seeded.
    #[must_use]
    pub fn with_worktree(mut self, repo: impl Into<String>) -> Self {
        if let Some(FieldKind::Toggle(toggle)) = self.form.get_mut(WORKTREE).map(Field::kind_mut) {
            toggle.set(true);
        }
        self.with_worktree_repo(repo)
    }

    /// Offers these directories when completing the cwd and the worktree
    /// repo.
    #[must_use]
    pub fn with_cwd_candidates(mut self, candidates: Vec<Candidate>) -> Self {
        for id in [CWD, REPO] {
            if let Some(FieldKind::Path(path)) = self.form.get_mut(id).map(Field::kind_mut) {
                path.set_candidates(candidates.clone());
            }
        }
        self
    }

    /// The framework form: its fields, values, focus, and error line.
    pub fn form(&self) -> &Form<Msg> {
        &self.form
    }

    /// The id of the field that has the focus.
    pub fn focus(&self) -> Option<&'static str> {
        self.form.focus()
    }

    /// The reason the last launch was refused, until the next edit.
    pub fn error(&self) -> Option<&str> {
        self.form.error()
    }

    /// What the form's fields hold.
    pub fn values(&self) -> Values<'_> {
        self.form.values()
    }

    /// The command row's text: the typed argv, or the default and where it
    /// comes from.
    pub fn command(&self) -> &str {
        match self.form.get(COMMAND).map(Field::kind) {
            Some(FieldKind::ReadOnly(row)) => row.text(),
            _ => "",
        }
    }

    /// The command an empty argv launches and where it comes from
    /// (`claude --x — from the project launch profile`), while the argv is
    /// empty and launch defaults decided it.
    pub fn default_hint(&self) -> Option<String> {
        self.text(ARGV)
            .trim()
            .is_empty()
            .then(|| self.default.hint())
            .flatten()
    }

    /// Whether typing edits the focused field, so a paste belongs to it.
    pub fn takes_text(&self) -> bool {
        self.form.takes_text()
    }

    /// Draws the form over `area`.
    pub fn draw(&self, area: Rect, buf: &mut Buffer, theme: &tuikit::theme::Theme) {
        self.form.draw(area, buf, theme);
    }

    /// One key: the form edits, moves, or submits; then the fields that
    /// follow others catch up. Stepping the harness carries a typed argv
    /// across, and flipping a launch option writes its flags into the
    /// argv.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<Msg> {
        let steps_harness = self.focus() == Some(HARNESS)
            && matches!(
                resolve(MUX_LAUNCH_KEYS, key),
                Some(FormKey::OptionPrev | FormKey::OptionNext | FormKey::Toggle)
            );
        let old_default = self.default.argv().to_vec();
        let outcome = Overlay::<Msg, ()>::handle(&mut self.form, key, &());
        match self.form.edited() {
            Some(HARNESS) if steps_harness => self.carry_argv(&old_default),
            Some(OPTIONS) => self.apply_options(),
            _ => {}
        }
        self.follow();
        outcome
    }

    /// The text of field `id`, as typed.
    fn text(&self, id: &str) -> &str {
        self.form.values().text(id)
    }

    /// The cwd as the launch will get it, `~` expanded.
    fn cwd(&self) -> String {
        self.form
            .values()
            .path(CWD)
            .map(PathField::expanded_value)
            .unwrap_or_default()
    }

    /// The default argv for the harness and cwd as they are now, decided
    /// afresh.
    fn decide_default(&self) -> DefaultArgv {
        DefaultArgv::resolve(self.defaults.as_ref(), self.text(HARNESS), &self.cwd())
    }

    /// The argv the form launches: as typed (read as far as it goes), or
    /// the default.
    fn effective_argv(&self) -> Vec<String> {
        launch_argv::effective(split_command_line(self.text(ARGV)), self.default.argv())
    }

    fn set_argv(&mut self, text: String) {
        if let Some(FieldKind::Text(edit)) = self.form.get_mut(ARGV).map(Field::kind_mut) {
            *edit = LineEdit::new(text);
        }
    }

    /// After a step of the harness: the typed argv loses the old
    /// harness's flags, and one that was only the old default empties so
    /// it follows the new one.
    fn carry_argv(&mut self, old_default: &[String]) {
        let typed = split_command_line(self.text(ARGV));
        if typed.is_empty() {
            return;
        }
        let (harness, cwd) = (self.text(HARNESS).to_string(), self.cwd());
        self.default.refresh(self.defaults.as_ref(), &harness, &cwd);
        let next = launch_argv::carried(typed, old_default, self.default.argv());
        let text = launch_argv::field_text(&next, self.default.argv());
        self.set_argv(text);
    }

    /// After a launch option flipped: the argv takes the options as the
    /// checkboxes now say.
    fn apply_options(&mut self) {
        let options = launch_options_for(self.text(HARNESS).trim());
        let Some(checked) = self
            .form
            .values()
            .check_group(OPTIONS)
            .map(CheckGroup::checked)
        else {
            return;
        };
        let mut argv = self.effective_argv();
        for (option, on) in options.iter().zip(checked) {
            argv = launch_argv::with_option(argv, *option, on);
        }
        let text = launch_argv::field_text(&argv, self.default.argv());
        self.set_argv(text);
    }

    /// Brings the fields that follow others up to date: the default argv
    /// (decided again only when the harness or the cwd changed), the
    /// launch options' checkboxes, and the command row.
    fn follow(&mut self) {
        let (harness, cwd) = (self.text(HARNESS).trim().to_string(), self.cwd());
        self.default.refresh(self.defaults.as_ref(), &harness, &cwd);
        let argv = self.effective_argv();
        let options = launch_options_for(&harness);
        let items: Vec<(String, bool)> = options
            .iter()
            .zip(launch_argv::options_on(options, &argv))
            .map(|(option, on)| (option.label.to_string(), on))
            .collect();
        if let Some(FieldKind::CheckGroup(group)) = self.form.get_mut(OPTIONS).map(Field::kind_mut)
        {
            group.set_items(items);
        }
        let (text, tone) = self.command_line();
        if let Some(FieldKind::ReadOnly(row)) = self.form.get_mut(COMMAND).map(Field::kind_mut) {
            row.set(text, tone);
        }
    }

    /// What the command row says: the typed argv; else the default and
    /// where it comes from; else that the harness has no default.
    fn command_line(&self) -> (String, Tone) {
        let typed = self.text(ARGV).trim();
        if !typed.is_empty() {
            return (typed.to_string(), Tone::Derived);
        }
        if let Some(hint) = self.default.hint() {
            return (hint, Tone::Derived);
        }
        if self.default.argv().is_empty() {
            ("no default for this harness".to_string(), Tone::Warning)
        } else {
            (launch_argv::display(self.default.argv()), Tone::Derived)
        }
    }
}

/// Whether the worktree toggle is on, which shows the branch and repo.
fn worktree_on(values: &Values<'_>) -> bool {
    values.is_on(WORKTREE)
}

/// Refuses an empty field, naming `what` is required.
fn required(what: &'static str) -> impl Fn(&str) -> Result<(), String> + Send + Sync {
    move |text: &str| {
        if text.trim().is_empty() {
            Err(format!("mux launch: {what} is required"))
        } else {
            Ok(())
        }
    }
}

/// Refuses an empty name and the name of a session that is already live.
fn name_check(live: Vec<String>) -> impl Fn(&str) -> Result<(), String> + Send + Sync {
    move |text: &str| {
        let name = text.trim();
        if name.is_empty() {
            Err("mux launch: mux name is required".to_string())
        } else if live.iter().any(|known| known == name) {
            Err(format!(
                "mux launch: tmux session `{name}` is already live; pick another name"
            ))
        } else {
            Ok(())
        }
    }
}

/// Refuses an argv that ends inside a quote or on a lone backslash.
fn argv_check(text: &str) -> Result<(), String> {
    parse_launch_argv(text).map(drop)
}

/// The argv field read strictly, as the launch takes it.
fn parse_launch_argv(text: &str) -> Result<Vec<String>, String> {
    parse_command_line(text.trim()).map_err(|why| format!("mux launch: launch argv has {why}"))
}

/// `text` trimmed, or `None` when nothing is left.
fn optional(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The launch the values ask for. The fields' checks have passed: the
/// name, harness, and cwd are set, the name is not live, the argv parses,
/// and a worktree has its branch and repo.
fn request(values: &Values<'_>) -> Submit<Msg> {
    let launch_argv = match parse_launch_argv(values.text(ARGV)) {
        Ok(argv) => argv,
        Err(reason) => return Submit::Reject(reason),
    };
    let expanded = |id: &str| {
        values
            .path(id)
            .map(PathField::expanded_value)
            .unwrap_or_default()
    };
    let (worktree_branch, worktree_repo) = if values.is_on(WORKTREE) {
        (optional(values.text(BRANCH)), Some(expanded(REPO)))
    } else {
        (None, None)
    };
    Submit::Commit(Msg::CommitMuxLaunch(MuxLaunchRequest {
        harness: values.text(HARNESS).trim().to_string(),
        name: values.text(NAME).trim().to_string(),
        cwd: expanded(CWD),
        mux_socket: values
            .is_on(SERVER)
            .then(|| optional(values.text(SOCKET)))
            .flatten(),
        launch_argv,
        worktree_branch,
        worktree_repo,
        pin: values
            .chosen(PIN)
            .and_then(|index| PinChoice::ORDER.get(index).copied())
            .unwrap_or_default(),
    }))
}

#[cfg(test)]
#[path = "mux_launch_tests.rs"]
mod tests;
