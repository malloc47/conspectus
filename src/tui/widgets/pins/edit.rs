//! Pin-edit form state.

use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct PinEditOptions {
    pub(super) known_cwd_candidates: Vec<PathCandidate>,
    pub(super) known_harness_keys: Vec<String>,
}

impl PinEditState {
    pub(super) const FIELD_CWD: usize = 0;
    pub(super) const FIELD_HARNESS: usize = 1;
    pub(super) const FIELD_LAUNCH_OPTIONS: usize = 2;
    pub(super) const FIELD_LAUNCH_ARGV: usize = 3;
    pub(super) const FIELD_ID: usize = 4;
    pub(super) const FIELD_DISPLAY: usize = 5;
    pub(super) const FIELD_MUX_NAME: usize = 6;
    pub(super) const FIELD_MUX_SOCKET: usize = 7;

    pub(super) fn new(target: PinMutationTarget) -> Self {
        Self::new_with_options(target, PinEditOptions::default())
    }

    pub(super) fn new_with_options(target: PinMutationTarget, options: PinEditOptions) -> Self {
        let mut cwd = PathOmniboxState::new(" cwd ", target.cwd.clone());
        cwd.set_known_candidates(pin_edit_cwd_candidates(
            &target,
            options.known_cwd_candidates,
        ));
        let known_harness_keys =
            normalized_harness_keys(options.known_harness_keys, [&target.harness]);
        // Match create's "override vs default" semantics: if the pin
        // already carries the harness default argv, present the field
        // as empty so toggling harness naturally follows the new
        // default instead of pinning the previous binary.
        let launch_default = launch_argv_for(target.harness.trim())
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let launch_argv_initial =
            if !target.launch_argv.is_empty() && target.launch_argv != launch_default {
                display_launch_argv(&target.launch_argv)
            } else {
                String::new()
            };
        Self {
            cursor: 0,
            id: TextInputState::new(" id ", target.id.clone()),
            display_name: TextInputState::new(" display ", target.display_name.clone()),
            harness: TextInputState::new(" harness ", target.harness.clone()),
            cwd,
            mux_name: TextInputState::new(" mux ", target.mux_name.clone()),
            mux_socket: TextInputState::new(
                " socket ",
                target.mux_socket.clone().unwrap_or_default(),
            ),
            launch_argv: TextInputState::new(" launch argv ", launch_argv_initial),
            target,
            known_harness_keys,
            error: None,
        }
    }

    pub(super) fn handle_key(&mut self, event: KeyEvent) -> PinEditOutcome {
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
            KeyCode::Tab if self.logical_cursor() == Self::FIELD_CWD => {
                match self.cwd.handle_key(event) {
                    PathOmniboxOutcome::Completed | PathOmniboxOutcome::Changed => {
                        self.error = None;
                    }
                    PathOmniboxOutcome::NoCompletion | PathOmniboxOutcome::Continue => {}
                }
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
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.logical_cursor() == Self::FIELD_HARNESS =>
            {
                self.cycle_harness(if matches!(event.code, KeyCode::Left) {
                    -1
                } else {
                    1
                });
                self.error = None;
                PinEditOutcome::Continue
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.logical_cursor() == Self::FIELD_LAUNCH_OPTIONS =>
            {
                self.toggle_launch_option(if matches!(event.code, KeyCode::Left) {
                    -1
                } else {
                    1
                });
                self.error = None;
                PinEditOutcome::Continue
            }
            _ => {
                if event.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(event.code, KeyCode::Char('c'))
                {
                    return PinEditOutcome::Cancel;
                }
                let active = self.logical_cursor();
                if active == Self::FIELD_CWD {
                    let outcome = self.cwd.handle_key(event);
                    if matches!(
                        outcome,
                        PathOmniboxOutcome::Changed | PathOmniboxOutcome::Completed
                    ) {
                        self.error = None;
                    }
                } else if let Some(input) = self.active_input_mut() {
                    let _ = input.handle_key(event);
                    self.error = None;
                }
                PinEditOutcome::Continue
            }
        }
    }

    pub(super) fn move_cursor(&mut self, delta: i32) {
        self.cursor = crate::tui::cursor::wrap_step(self.cursor, self.field_count(), delta);
    }

    pub(super) fn field_count(&self) -> usize {
        self.visible_fields().len()
    }

    pub(super) fn has_launch_options(&self) -> bool {
        !self.launch_options().is_empty()
    }

    pub(super) fn visible_fields(&self) -> Vec<usize> {
        let mut fields = vec![Self::FIELD_CWD, Self::FIELD_HARNESS];
        if self.has_launch_options() {
            fields.push(Self::FIELD_LAUNCH_OPTIONS);
        }
        fields.extend([
            Self::FIELD_LAUNCH_ARGV,
            Self::FIELD_ID,
            Self::FIELD_DISPLAY,
            Self::FIELD_MUX_NAME,
            Self::FIELD_MUX_SOCKET,
        ]);
        fields
    }

    pub(super) fn logical_cursor(&self) -> usize {
        self.visible_fields()
            .get(self.cursor)
            .copied()
            .unwrap_or(Self::FIELD_CWD)
    }

    pub(super) fn active_input_mut(&mut self) -> Option<&mut TextInputState> {
        match self.logical_cursor() {
            Self::FIELD_HARNESS => Some(&mut self.harness),
            Self::FIELD_LAUNCH_ARGV => Some(&mut self.launch_argv),
            Self::FIELD_ID => Some(&mut self.id),
            Self::FIELD_DISPLAY => Some(&mut self.display_name),
            Self::FIELD_MUX_NAME => Some(&mut self.mux_name),
            Self::FIELD_MUX_SOCKET => Some(&mut self.mux_socket),
            _ => None,
        }
    }

    pub(super) fn launch_options(&self) -> &'static [HarnessLaunchOption] {
        launch_options_for(self.harness.value().trim())
    }

    pub(super) fn selected_launch_option_ids(&self) -> Vec<&'static str> {
        let argv = self.effective_launch_argv_list().unwrap_or_default();
        self.launch_options()
            .iter()
            .filter(|option| argv_contains_fragment(&argv, option.argv))
            .map(|option| option.id)
            .collect()
    }

    pub(super) fn launch_option_selected(&self, option: HarnessLaunchOption) -> bool {
        self.effective_launch_argv_list()
            .is_ok_and(|argv| argv_contains_fragment(&argv, option.argv))
    }

    pub(super) fn toggle_launch_option(&mut self, delta: i32) {
        let options = self.launch_options();
        if options.is_empty() {
            return;
        }
        let selected = self.selected_launch_option_ids();
        let current_idx = selected
            .first()
            .and_then(|selected| options.iter().position(|option| option.id == *selected))
            .unwrap_or(0);
        let idx = if matches!(delta, -1 | 1) && selected.len() == 1 {
            crate::tui::cursor::wrap_step(current_idx, options.len(), delta)
        } else {
            current_idx
        };
        let option = options[idx];
        let Ok(argv) = self.effective_launch_argv_list() else {
            return;
        };
        let argv = if self.launch_option_selected(option) {
            argv_without_fragment(argv, option.argv)
        } else {
            argv_with_fragment(argv, option.argv)
        };
        self.set_launch_argv_from_effective(argv);
    }

    pub(super) fn effective_launch_argv_list(&self) -> Result<Vec<String>, String> {
        let override_argv = self.launch_argv_override()?;
        if override_argv.is_empty() {
            Ok(self.default_launch_argv())
        } else {
            Ok(override_argv)
        }
    }

    pub(super) fn set_launch_argv_from_effective(&mut self, argv: Vec<String>) {
        let default = self.default_launch_argv();
        if argv.is_empty() || argv == default {
            self.launch_argv = TextInputState::new(" launch argv ", String::new());
        } else {
            self.launch_argv = TextInputState::new(" launch argv ", display_launch_argv(&argv));
        }
    }

    pub(super) fn launch_argv_override(&self) -> Result<Vec<String>, String> {
        parse_launch_argv(self.launch_argv.value().trim(), "pin edit")
    }

    pub(super) fn default_launch_argv(&self) -> Vec<String> {
        launch_argv_for(self.harness.value().trim())
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    pub(super) fn effective_launch_argv(&self) -> Result<LaunchArgvPreview, String> {
        let override_argv = self.launch_argv_override()?;
        if !override_argv.is_empty() {
            return Ok(LaunchArgvPreview {
                source: LaunchArgvSource::Override,
                argv: override_argv,
            });
        }
        Ok(LaunchArgvPreview {
            source: LaunchArgvSource::Default,
            argv: self.default_launch_argv(),
        })
    }

    pub(super) fn cycle_harness(&mut self, delta: i32) {
        if self.known_harness_keys.is_empty() {
            return;
        }
        let before = self.launch_argv_override().ok();
        let previous_default = self.default_launch_argv();
        let value = self.harness.value().trim();
        let idx = match self
            .known_harness_keys
            .iter()
            .position(|known| known == value)
        {
            Some(idx) => crate::tui::cursor::wrap_step(idx, self.known_harness_keys.len(), delta),
            None if delta < 0 => self.known_harness_keys.len().saturating_sub(1),
            None => 0,
        };
        self.harness = TextInputState::new(" harness ", self.known_harness_keys[idx].clone());
        if let Some(argv) = before {
            let stripped = strip_known_launch_option_fragments(argv);
            if stripped == previous_default {
                self.launch_argv = TextInputState::new(" launch argv ", String::new());
            } else {
                self.set_launch_argv_from_effective(stripped);
            }
        }
    }

    pub(super) fn request(&self) -> Result<PinEditRequest, String> {
        let id = required(self.id.value(), "id")?;
        let display_name = required(self.display_name.value(), "display")?;
        let harness = required(self.harness.value(), "harness")?;
        let cwd = required(&self.cwd.expanded_value(), "cwd")?;
        let mux_name = required(self.mux_name.value(), "mux.name")?;
        let mux_socket = optional(self.mux_socket.value());
        let launch_argv = self.launch_argv_override()?;
        let launch_argv = if launch_argv.is_empty() {
            self.default_launch_argv()
        } else {
            launch_argv
        };
        Ok(PinEditRequest {
            original_id: self.target.id.clone(),
            id,
            display_name,
            harness,
            cwd,
            mux_name,
            mux_socket,
            launch_argv,
            store_path: self.target.store_path.clone(),
        })
    }
}

pub(super) fn pin_edit_cwd_candidates(
    target: &PinMutationTarget,
    known: Vec<PathCandidate>,
) -> Vec<PathCandidate> {
    let mut candidates = Vec::new();
    if !target.cwd.trim().is_empty() {
        candidates.push(PathCandidate::new(target.cwd.clone(), "current", 1_000));
    }
    candidates.extend(known);
    candidates
}
