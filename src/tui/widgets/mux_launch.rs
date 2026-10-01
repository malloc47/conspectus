//! Ephemeral harness launch form (ADR 0096).
//!
//! Multi-field modal that commits a [`Msg::CommitMuxLaunch`] the
//! runtime turns into a subprocess re-exec of
//! `conspectus mux launch <harness>`. Distinct from the pin-create
//! form because no pin is written: on submit the mux and its
//! attributed harness session appear in discovery next refresh; the
//! operator can `pin adopt` later if they change their mind.
//!
//! Mode is fixed at open time — there is no persistence toggle
//! (operator preference, ADR 0096 intake). The title reads
//! "Launch harness in new mux — no pin" and the submit label reads
//! "Launch" so the outcome is obvious before the operator commits.
//!
//! ADR 0097: the shared fields (harness, cwd,
//! mux name / socket, launch argv, worktree toggle + branch, known-
//! harness + known-live-mux collections, error) live on a
//! [`LaunchSpecFormState`] container this widget owns by composition.
//! Only the caller-specific `worktree_repo` field and the cursor +
//! renderer stay on this wrapper.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::tui::Msg;
use crate::tui::modal::{Overlay, OverlayOutcome};
use crate::tui::theme::Theme;
use crate::tui::widgets::input::{InputOutcome, TextInputState};
use crate::tui::widgets::launch_spec_form::{
    LaunchSpecFormState, LaunchSpecInit, SpecTextField, optional_string, parse_launch_argv,
};
use crate::tui::widgets::path_omnibox::PathOmniboxOutcome;
use crate::tui::widgets::popup_frame;

const HARNESS_LABEL: &str = "Harness";
const CWD_LABEL: &str = "Cwd";
const MUX_NAME_LABEL: &str = "Mux name";
const MUX_SOCKET_LABEL: &str = "Mux socket";
const LAUNCH_ARGV_LABEL: &str = "Launch argv";
const WORKTREE_BRANCH_LABEL: &str = "Worktree branch";
const WORKTREE_REPO_LABEL: &str = "Worktree repo";

/// Ordered field identifiers for cursor placement. `WorktreeBranch`
/// and `WorktreeRepo` are only visible when the primitive's
/// worktree toggle is on.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MuxLaunchField {
    Harness,
    Cwd,
    MuxName,
    MuxSocket,
    LaunchArgv,
    WorktreeToggle,
    WorktreeBranch,
    WorktreeRepo,
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
}

/// Ephemeral harness launch form state. Owns the shared launch-spec
/// primitive plus the mux-launch-only `worktree_repo` field and the
/// cursor.
#[derive(Debug, Clone)]
pub struct MuxLaunchFormState {
    spec: LaunchSpecFormState,
    worktree_repo: TextInputState,
    focus: MuxLaunchField,
}

impl MuxLaunchFormState {
    /// New form with pre-populated defaults. Focus starts on the
    /// harness field so the operator types the harness key
    /// immediately.
    pub fn new(
        harness: impl Into<String>,
        cwd: impl Into<String>,
        mux_name: impl Into<String>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
    ) -> Self {
        let spec = LaunchSpecFormState::new(LaunchSpecInit {
            harness_label: HARNESS_LABEL,
            harness_value: harness.into(),
            cwd_label: CWD_LABEL,
            cwd_value: cwd.into(),
            mux_name_label: MUX_NAME_LABEL,
            mux_name_value: mux_name.into(),
            mux_socket_label: MUX_SOCKET_LABEL,
            launch_argv_label: LAUNCH_ARGV_LABEL,
            worktree_branch_label: WORKTREE_BRANCH_LABEL,
            worktree_branch_value: String::new(),
            known_harness_keys,
            known_mux_names,
        });
        Self {
            spec,
            worktree_repo: TextInputState::new(WORKTREE_REPO_LABEL, String::new()),
            focus: MuxLaunchField::Harness,
        }
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

    fn visible_fields(&self) -> Vec<MuxLaunchField> {
        let mut fields = vec![
            MuxLaunchField::Harness,
            MuxLaunchField::Cwd,
            MuxLaunchField::MuxName,
            MuxLaunchField::MuxSocket,
            MuxLaunchField::LaunchArgv,
            MuxLaunchField::WorktreeToggle,
        ];
        if self.spec.worktree_enabled() {
            fields.push(MuxLaunchField::WorktreeBranch);
            fields.push(MuxLaunchField::WorktreeRepo);
        }
        fields
    }

    fn advance_focus(&mut self, delta: i32) {
        let fields = self.visible_fields();
        let idx = fields.iter().position(|f| *f == self.focus).unwrap_or(0);
        self.focus = fields[crate::tui::cursor::wrap_step(idx, fields.len(), delta)];
    }

    fn try_commit(&mut self) -> OverlayOutcome {
        match self.build_request() {
            Ok(req) => OverlayOutcome::Commit(Box::new(Msg::CommitMuxLaunch(req))),
            Err(msg) => {
                self.spec.set_error(msg);
                OverlayOutcome::Consumed
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
        let cwd = self.spec.cwd().value().trim().to_string();
        if cwd.is_empty() {
            return Err("mux launch: cwd is required".to_string());
        }
        let mux_socket = optional_string(self.spec.mux_socket().value());
        let launch_argv = parse_launch_argv(self.spec.launch_argv().value().trim(), "mux launch")?;
        let (worktree_branch, worktree_repo) = if self.spec.worktree_enabled() {
            let branch = self.spec.worktree_branch().value().trim().to_string();
            if branch.is_empty() {
                return Err("mux launch: worktree branch is required".to_string());
            }
            let repo = self.worktree_repo.value().trim().to_string();
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
        })
    }
}

impl Overlay for MuxLaunchFormState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> OverlayOutcome {
        match key.code {
            KeyCode::Esc => return OverlayOutcome::Close,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return OverlayOutcome::Close;
            }
            KeyCode::Tab => {
                self.advance_focus(1);
                return OverlayOutcome::Consumed;
            }
            KeyCode::BackTab => {
                self.advance_focus(-1);
                return OverlayOutcome::Consumed;
            }
            KeyCode::Enter => return self.try_commit(),
            _ => {}
        }
        // Per-field handling for the two non-text-input rows.
        match self.focus {
            MuxLaunchField::Harness => {
                if matches!(
                    key.code,
                    KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                ) {
                    let delta = if matches!(key.code, KeyCode::Left) {
                        -1
                    } else {
                        1
                    };
                    self.spec.cycle_harness(delta, HARNESS_LABEL);
                    self.spec.clear_error();
                    return OverlayOutcome::Consumed;
                }
            }
            MuxLaunchField::WorktreeToggle => {
                if matches!(
                    key.code,
                    KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                ) {
                    self.spec.toggle_worktree();
                    self.spec.clear_error();
                    return OverlayOutcome::Consumed;
                }
            }
            MuxLaunchField::Cwd => {
                let outcome = self.spec.cwd_handle_key(key);
                if matches!(
                    outcome,
                    PathOmniboxOutcome::Changed | PathOmniboxOutcome::Completed
                ) {
                    self.spec.clear_error();
                }
                return OverlayOutcome::Consumed;
            }
            _ => {}
        }
        let input_outcome = match self.focus {
            MuxLaunchField::Harness => Some(self.spec.handle_text_key(SpecTextField::Harness, key)),
            MuxLaunchField::MuxName => Some(self.spec.handle_text_key(SpecTextField::MuxName, key)),
            MuxLaunchField::MuxSocket => {
                Some(self.spec.handle_text_key(SpecTextField::MuxSocket, key))
            }
            MuxLaunchField::LaunchArgv => {
                Some(self.spec.handle_text_key(SpecTextField::LaunchArgv, key))
            }
            MuxLaunchField::WorktreeBranch => Some(
                self.spec
                    .handle_text_key(SpecTextField::WorktreeBranch, key),
            ),
            MuxLaunchField::WorktreeRepo => Some(self.worktree_repo.handle_key(key)),
            MuxLaunchField::WorktreeToggle | MuxLaunchField::Cwd => None,
        };
        match input_outcome {
            Some(InputOutcome::Continue) => {
                self.spec.clear_error();
                OverlayOutcome::Consumed
            }
            Some(InputOutcome::Cancel) => OverlayOutcome::Close,
            Some(InputOutcome::Confirm(_)) => self.try_commit(),
            None => OverlayOutcome::Consumed,
        }
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
        let spec = self.state.spec();
        let mut lines: Vec<Line> = vec![
            Line::styled(
                "Launch harness in new mux — no pin",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            format_field_line(
                "harness",
                spec.harness().value(),
                self.state.focus == MuxLaunchField::Harness,
            ),
            format_field_line(
                "cwd",
                spec.cwd().value(),
                self.state.focus == MuxLaunchField::Cwd,
            ),
            format_field_line(
                "mux name",
                spec.mux_name().value(),
                self.state.focus == MuxLaunchField::MuxName,
            ),
            format_field_line(
                "mux socket",
                spec.mux_socket().value(),
                self.state.focus == MuxLaunchField::MuxSocket,
            ),
            format_field_line(
                "launch argv",
                spec.launch_argv().value(),
                self.state.focus == MuxLaunchField::LaunchArgv,
            ),
            format_field_line(
                "worktree",
                if spec.worktree_enabled() {
                    "[x] realize at launch"
                } else {
                    "[ ] off"
                },
                self.state.focus == MuxLaunchField::WorktreeToggle,
            ),
        ];
        if spec.worktree_enabled() {
            lines.push(format_field_line(
                "  branch",
                spec.worktree_branch().value(),
                self.state.focus == MuxLaunchField::WorktreeBranch,
            ));
            lines.push(format_field_line(
                "  repo",
                self.state.worktree_repo.value(),
                self.state.focus == MuxLaunchField::WorktreeRepo,
            ));
        }
        if let Some(err) = spec.error() {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                err.to_string(),
                Style::default().fg(self.theme.error),
            ));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(
            "Tab/Shift-Tab move · ←/→ toggle · Enter launch · Esc cancel",
        ));

        let height = u16::try_from(lines.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .clamp(10, area.height);
        let width = 72_u16.min(area.width);
        let rect = popup_frame::centered_rect(area, width, height);
        Clear.render(rect, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Mux ▸ Launch harness ")
            .border_style(Style::default().fg(self.theme.panel_focus_accent));
        let inner = block.inner(rect);
        block.render(rect, buf);
        Paragraph::new(lines).render(inner, buf);
    }
}

fn format_field_line(label: &str, value: &str, focused: bool) -> Line<'static> {
    let marker = if focused { "▸" } else { " " };
    let text = format!("{marker} {label}: {value}");
    if focused {
        Line::styled(text, Style::default().add_modifier(Modifier::BOLD))
    } else {
        Line::from(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn commits_request_when_required_fields_present() {
        let mut state = MuxLaunchFormState::new(
            "codex",
            "/home/op/repo",
            "adhoc",
            vec!["codex".to_string(), "claude-code".to_string()],
            Vec::new(),
        );
        let outcome = state.handle((), key(KeyCode::Enter));
        match outcome {
            OverlayOutcome::Commit(msg) => match *msg {
                Msg::CommitMuxLaunch(req) => {
                    assert_eq!(req.harness, "codex");
                    assert_eq!(req.name, "adhoc");
                    assert_eq!(req.cwd, "/home/op/repo");
                    assert!(req.worktree_branch.is_none());
                    assert!(req.launch_argv.is_empty());
                }
                other => panic!("unexpected msg: {other:?}"),
            },
            other => panic!("expected Commit, got {other:?}"),
        }
    }

    #[test]
    fn empty_harness_produces_error() {
        let mut state =
            MuxLaunchFormState::new("", "/tmp", "adhoc", vec!["codex".to_string()], Vec::new());
        let outcome = state.handle((), key(KeyCode::Enter));
        assert_eq!(outcome, OverlayOutcome::Consumed);
        assert!(state.error().unwrap().contains("harness"));
    }

    #[test]
    fn name_collision_with_known_mux_produces_error() {
        let mut state = MuxLaunchFormState::new(
            "codex",
            "/tmp",
            "conflict",
            vec!["codex".to_string()],
            vec!["conflict".to_string()],
        );
        let outcome = state.handle((), key(KeyCode::Enter));
        assert_eq!(outcome, OverlayOutcome::Consumed);
        assert!(state.error().unwrap().contains("already live"));
    }

    #[test]
    fn worktree_toggle_requires_branch_and_repo() {
        let mut state = MuxLaunchFormState::new(
            "codex",
            "/tmp",
            "adhoc",
            vec!["codex".to_string()],
            Vec::new(),
        );
        state.spec_mut().set_worktree_enabled(true);
        let outcome = state.handle((), key(KeyCode::Enter));
        assert_eq!(outcome, OverlayOutcome::Consumed);
        assert!(state.error().unwrap().contains("worktree"));
    }

    #[test]
    fn tab_cycles_focus_through_visible_fields() {
        let mut state = MuxLaunchFormState::new(
            "codex",
            "/tmp",
            "adhoc",
            vec!["codex".to_string()],
            Vec::new(),
        );
        assert_eq!(state.focus(), MuxLaunchField::Harness);
        state.handle((), key(KeyCode::Tab));
        assert_eq!(state.focus(), MuxLaunchField::Cwd);
        state.handle((), key(KeyCode::Tab));
        assert_eq!(state.focus(), MuxLaunchField::MuxName);
    }

    #[test]
    fn launch_argv_parses_shell_quoted() {
        let mut state = MuxLaunchFormState::new(
            "codex",
            "/tmp",
            "adhoc",
            vec!["codex".to_string()],
            Vec::new(),
        );
        state
            .spec_mut()
            .set_launch_argv(LAUNCH_ARGV_LABEL, "codex --model 'gpt-5 pro'".to_string());
        let req = state.build_request().expect("parse ok");
        assert_eq!(req.launch_argv, vec!["codex", "--model", "gpt-5 pro"]);
    }
}
