//! Pin-create form state: fields, defaults, completion, and submission.

use super::*;

impl PinCreateState {
    pub(super) const FIELD_NAME: usize = 0;
    pub(super) const FIELD_MODE: usize = 1;
    pub(super) const FIELD_CWD: usize = 2;
    pub(super) const FIELD_HARNESS: usize = 3;
    pub(super) const FIELD_LAUNCH_OPTIONS: usize = 4;
    pub(super) const FIELD_LAUNCH_ARGV: usize = 5;
    pub(super) const FIELD_ID: usize = 6;
    pub(super) const FIELD_DISPLAY: usize = 7;
    pub(super) const FIELD_MUX_NAME: usize = 8;
    pub(super) const FIELD_MUX_SOCKET: usize = 9;
    pub(super) const FIELD_STORE: usize = 10;
    /// Worktree toggle (ADR 0094): when on, the pin is worktree-backed
    /// and the branch field below it becomes visible.
    pub(super) const FIELD_WORKTREE_TOGGLE: usize = 11;
    pub(super) const FIELD_WORKTREE_BRANCH: usize = 12;

    /// Read-only access to the shared launch-spec fields. Renderers,
    /// tests, and any code outside the reducer's edit path go through
    /// this borrow.
    pub fn spec(&self) -> &LaunchSpecFormState {
        &self.spec
    }

    /// Mutable access to the shared launch-spec fields. Used by
    /// wrapper-internal helpers that need to poke a shared field
    /// without going through a dedicated setter.
    pub fn spec_mut(&mut self) -> &mut LaunchSpecFormState {
        &mut self.spec
    }

    #[cfg(test)]
    pub(super) fn new(
        defaults: PinCreateDefaults,
        adopt_defaults: Option<PinCreateDefaults>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
    ) -> Self {
        Self::new_with_guards(
            defaults,
            adopt_defaults,
            known_harness_keys,
            known_mux_names,
            Vec::new(),
            Vec::new(),
            None,
        )
    }

    #[cfg(test)]
    pub(super) fn new_with_guards(
        defaults: PinCreateDefaults,
        adopt_defaults: Option<PinCreateDefaults>,
        known_harness_keys: Vec<String>,
        known_mux_names: Vec<String>,
        known_pin_ids: Vec<String>,
        known_pin_mux_names: Vec<String>,
        selected_pin_id: Option<String>,
    ) -> Self {
        Self::new_with_options(
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

    pub(super) fn new_with_options(defaults: PinCreateDefaults, options: PinCreateOptions) -> Self {
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
            defaults.id.clone()
        };
        let display_name = if defaults.display_name.is_empty() {
            name.clone()
        } else {
            defaults.display_name.clone()
        };
        let derived_mux_name = derived_mux_name_for_mode(defaults.mode, &name, &derived_id);
        let mux_name = if defaults.mux_name.is_empty() {
            derived_mux_name.clone()
        } else {
            defaults.mux_name.clone()
        };
        let id_overridden = id != derived_id;
        let display_overridden = display_name != name;
        let mux_overridden = mux_name != derived_mux_name;
        let known_harness_keys =
            normalized_harness_keys(options.known_harness_keys, [&defaults.harness]);
        let cwd_candidates = pin_create_cwd_candidates(
            &defaults,
            options.adopt_defaults.as_ref(),
            options.known_cwd_candidates,
        );
        let mut spec = LaunchSpecFormState::new(LaunchSpecInit {
            harness_label: PIN_HARNESS_LABEL,
            harness_value: defaults.harness,
            cwd_label: PIN_CWD_LABEL,
            cwd_value: defaults.cwd,
            mux_name_label: PIN_MUX_NAME_LABEL,
            mux_name_value: mux_name,
            mux_socket_label: PIN_MUX_SOCKET_LABEL,
            launch_argv_label: PIN_LAUNCH_ARGV_LABEL,
            worktree_branch_label: PIN_WORKTREE_BRANCH_LABEL,
            // Branch defaults to the derived id; independently editable.
            worktree_branch_value: derived_id,
            known_harness_keys,
            known_mux_names: options.known_mux_names,
        });
        spec.set_cwd_candidates(cwd_candidates);
        spec.set_worktree_enabled(options.worktree_enabled);
        Self {
            mode: defaults.mode,
            cursor: 0,
            adopt_defaults: options.adopt_defaults,
            name: TextInputState::new(" name ", name),
            id: TextInputState::new(" id ", id),
            display_name: TextInputState::new(" display ", display_name),
            spec,
            store: PinCreateStore::Auto,
            id_overridden,
            display_overridden,
            mux_overridden,
            known_pin_ids: options.known_pin_ids,
            known_pin_mux_names: options.known_pin_mux_names,
            selected_pin_id: options.selected_pin_id,
            adopt_auto_uncheck_armed: defaults.mode == PinCreateMode::AdoptSelected,
            adopt_auto_checked_by_collision: false,
        }
    }

    pub(super) fn handle_key(&mut self, event: KeyEvent) -> PinCreateOutcome {
        match event.code {
            KeyCode::Esc => PinCreateOutcome::Cancel,
            KeyCode::Enter => match self.request() {
                Ok(request) => PinCreateOutcome::Confirm(Box::new(request)),
                Err(err) => {
                    self.spec.set_error(err);
                    PinCreateOutcome::Continue
                }
            },
            KeyCode::Up => {
                self.move_cursor(-1);
                PinCreateOutcome::Continue
            }
            KeyCode::Tab if self.logical_cursor() == Self::FIELD_CWD => {
                match self.spec.cwd_handle_key(event) {
                    PathOmniboxOutcome::Completed | PathOmniboxOutcome::Changed => {
                        self.spec.clear_error();
                    }
                    PathOmniboxOutcome::NoCompletion | PathOmniboxOutcome::Continue => {}
                }
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
                if self.logical_cursor() == Self::FIELD_MODE && self.can_toggle_mode() =>
            {
                self.toggle_mode();
                PinCreateOutcome::Continue
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.logical_cursor() == Self::FIELD_HARNESS =>
            {
                self.cycle_harness(if matches!(event.code, KeyCode::Left) {
                    -1
                } else {
                    1
                });
                self.spec.clear_error();
                PinCreateOutcome::Continue
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.logical_cursor() == Self::FIELD_LAUNCH_OPTIONS =>
            {
                self.toggle_launch_option(if matches!(event.code, KeyCode::Left) {
                    -1
                } else {
                    1
                });
                self.spec.clear_error();
                PinCreateOutcome::Continue
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.logical_cursor() == Self::FIELD_STORE =>
            {
                self.cycle_store(if matches!(event.code, KeyCode::Left) {
                    -1
                } else {
                    1
                });
                PinCreateOutcome::Continue
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.logical_cursor() == Self::FIELD_WORKTREE_TOGGLE =>
            {
                self.spec.toggle_worktree();
                self.spec.clear_error();
                PinCreateOutcome::Continue
            }
            _ => {
                if event.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(event.code, KeyCode::Char('c'))
                {
                    return PinCreateOutcome::Cancel;
                }
                let before = self.active_input_value().unwrap_or_default();
                let active = self.logical_cursor();
                if active == Self::FIELD_CWD {
                    let outcome = self.spec.cwd_handle_key(event);
                    if matches!(
                        outcome,
                        PathOmniboxOutcome::Changed | PathOmniboxOutcome::Completed
                    ) {
                        self.spec.clear_error();
                    }
                } else {
                    let handled = self.forward_to_active_input(active, event);
                    if handled {
                        let after = self.active_input_value().unwrap_or_default();
                        if after != before {
                            self.after_active_input_changed(active);
                        }
                        self.spec.clear_error();
                    }
                }
                PinCreateOutcome::Continue
            }
        }
    }

    /// Read the current value of the cursor's active text input, or
    /// `None` when the cursor is on a non-text row (mode / store /
    /// worktree toggle / launch-options carousel). Used by the
    /// change-detection path so pin-specific `sync_*` hooks fire when
    /// the operator actually typed something.
    pub(super) fn active_input_value(&self) -> Option<String> {
        match self.logical_cursor() {
            Self::FIELD_NAME => Some(self.name.value().to_string()),
            Self::FIELD_HARNESS => Some(self.spec.harness().value().to_string()),
            Self::FIELD_LAUNCH_ARGV => Some(self.spec.launch_argv().value().to_string()),
            Self::FIELD_ID => Some(self.id.value().to_string()),
            Self::FIELD_DISPLAY => Some(self.display_name.value().to_string()),
            Self::FIELD_MUX_NAME => Some(self.spec.mux_name().value().to_string()),
            Self::FIELD_MUX_SOCKET => Some(self.spec.mux_socket().value().to_string()),
            Self::FIELD_WORKTREE_BRANCH => Some(self.spec.worktree_branch().value().to_string()),
            _ => None,
        }
    }

    /// Forward the key event into the text input for `active`, if one
    /// applies. Returns `true` when a text input handled the key.
    pub(super) fn forward_to_active_input(&mut self, active: usize, event: KeyEvent) -> bool {
        match active {
            Self::FIELD_NAME => {
                let _ = self.name.handle_key(event);
                true
            }
            Self::FIELD_HARNESS => {
                self.spec.handle_text_key(SpecTextField::Harness, event);
                true
            }
            Self::FIELD_LAUNCH_ARGV => {
                self.spec.handle_text_key(SpecTextField::LaunchArgv, event);
                true
            }
            Self::FIELD_ID => {
                let _ = self.id.handle_key(event);
                true
            }
            Self::FIELD_DISPLAY => {
                let _ = self.display_name.handle_key(event);
                true
            }
            Self::FIELD_MUX_NAME => {
                self.spec.handle_text_key(SpecTextField::MuxName, event);
                true
            }
            Self::FIELD_MUX_SOCKET => {
                self.spec.handle_text_key(SpecTextField::MuxSocket, event);
                true
            }
            Self::FIELD_WORKTREE_BRANCH => {
                self.spec
                    .handle_text_key(SpecTextField::WorktreeBranch, event);
                true
            }
            _ => false,
        }
    }

    pub(super) fn move_cursor(&mut self, delta: i32) {
        self.cursor = crate::tui::cursor::wrap_step(self.cursor, self.field_count(), delta);
    }

    pub(super) fn field_count(&self) -> usize {
        self.visible_fields().len()
    }

    pub(super) fn can_toggle_mode(&self) -> bool {
        self.adopt_defaults.is_some()
    }

    pub(super) fn has_launch_options(&self) -> bool {
        !self.launch_options().is_empty()
    }

    pub(super) fn visible_fields(&self) -> Vec<usize> {
        let mut fields = vec![Self::FIELD_NAME];
        if self.can_toggle_mode() {
            fields.push(Self::FIELD_MODE);
        }
        fields.push(Self::FIELD_CWD);
        fields.push(Self::FIELD_WORKTREE_TOGGLE);
        if self.spec.worktree_enabled() {
            fields.push(Self::FIELD_WORKTREE_BRANCH);
        }
        fields.push(Self::FIELD_HARNESS);
        if self.has_launch_options() {
            fields.push(Self::FIELD_LAUNCH_OPTIONS);
        }
        fields.extend([
            Self::FIELD_LAUNCH_ARGV,
            Self::FIELD_ID,
            Self::FIELD_DISPLAY,
            Self::FIELD_MUX_NAME,
            Self::FIELD_MUX_SOCKET,
            Self::FIELD_STORE,
        ]);
        fields
    }

    pub(super) fn logical_cursor(&self) -> usize {
        self.visible_fields()
            .get(self.cursor)
            .copied()
            .unwrap_or(Self::FIELD_NAME)
    }

    pub(super) fn render_cursor(&self) -> usize {
        self.logical_cursor()
    }

    pub(super) fn toggle_mode(&mut self) {
        match self.mode {
            PinCreateMode::NewVariation => {
                if self.adopt_defaults.is_none() {
                    return;
                }
                self.mode = PinCreateMode::AdoptSelected;
                self.adopt_auto_uncheck_armed = false;
                self.adopt_auto_checked_by_collision = false;
                if !self.mux_overridden {
                    self.spec.set_mux_name(
                        PIN_MUX_NAME_LABEL,
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
                    self.spec.set_mux_name(
                        PIN_MUX_NAME_LABEL,
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

    pub(super) fn cycle_store(&mut self, delta: i32) {
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

    pub(super) fn cycle_harness(&mut self, delta: i32) {
        if self.spec.known_harness_keys().is_empty() {
            return;
        }
        let before = self.launch_argv_override().ok();
        let previous_default = self.default_launch_argv();
        self.spec.cycle_harness(delta, PIN_HARNESS_LABEL);
        if let Some(argv) = before {
            let stripped = strip_known_launch_option_fragments(argv);
            if stripped == previous_default {
                self.spec
                    .set_launch_argv(PIN_LAUNCH_ARGV_LABEL, String::new());
            } else {
                self.set_launch_argv_from_effective(stripped);
            }
        }
    }

    pub(super) fn launch_options(&self) -> &'static [HarnessLaunchOption] {
        launch_options_for(self.spec.harness().value().trim())
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
        let value = if argv.is_empty() || argv == default {
            String::new()
        } else {
            display_launch_argv(&argv)
        };
        self.spec.set_launch_argv(PIN_LAUNCH_ARGV_LABEL, value);
    }

    pub(super) fn after_active_input_changed(&mut self, active: usize) {
        match active {
            Self::FIELD_NAME => {
                if self.mode == PinCreateMode::AdoptSelected && self.adopt_auto_uncheck_armed {
                    self.mode = PinCreateMode::NewVariation;
                    self.adopt_auto_uncheck_armed = false;
                }
                self.sync_from_name();
                self.sync_mode_from_mux_collision();
            }
            Self::FIELD_ID => self.id_overridden = !self.id.value().is_empty(),
            Self::FIELD_DISPLAY => self.display_overridden = !self.display_name.value().is_empty(),
            Self::FIELD_MUX_NAME => {
                self.mux_overridden = !self.spec.mux_name().value().is_empty();
                self.sync_mode_from_mux_collision();
            }
            _ => {}
        }
    }

    pub(super) fn sync_mode_from_mux_collision(&mut self) {
        let matches_known_mux = self.matching_known_mux_name().is_some();
        if matches_known_mux {
            self.mode = PinCreateMode::AdoptSelected;
            self.adopt_auto_uncheck_armed = false;
            self.adopt_auto_checked_by_collision = true;
        } else if self.adopt_auto_checked_by_collision {
            self.mode = PinCreateMode::NewVariation;
            self.adopt_auto_checked_by_collision = false;
            if !self.mux_overridden {
                self.spec.set_mux_name(
                    PIN_MUX_NAME_LABEL,
                    derived_mux_name_for_mode(
                        self.mode,
                        self.name.value(),
                        &pin_id_candidate(self.name.value()),
                    ),
                );
            }
        }
    }

    pub(super) fn sync_from_name(&mut self) {
        let name = self.name.value().to_string();
        let derived_id = pin_id_candidate(&name);
        if !self.id_overridden {
            self.id = TextInputState::new(" id ", derived_id.clone());
        }
        if !self.display_overridden {
            self.display_name = TextInputState::new(" display ", name);
        }
        if !self.mux_overridden {
            self.spec.set_mux_name(
                PIN_MUX_NAME_LABEL,
                derived_mux_name_for_mode(self.mode, self.name.value(), &derived_id),
            );
        }
    }

    pub(super) fn request(&self) -> Result<PinCreateRequest, String> {
        let id = required(self.id.value(), "id")?;
        let harness = required(self.spec.harness().value(), "harness")?;
        let cwd = required(&self.spec.cwd().expanded_value(), "cwd")?;
        let display_name = optional(self.display_name.value()).unwrap_or_else(|| id.clone());
        let mux_name =
            optional_string(self.spec.mux_name().value()).unwrap_or_else(|| display_name.clone());
        let mux_socket = optional_string(self.spec.mux_socket().value());
        if self.known_pin_ids.iter().any(|known| known == &id) {
            return Err(format!("pin create: pin `{id}` already exists"));
        }
        if self.mode == PinCreateMode::AdoptSelected && self.selected_pin_id.is_some() {
            let pin_id = self.selected_pin_id.as_deref().unwrap_or_default();
            return Err(format!("pin create: `{pin_id}` is already pinned"));
        }
        if self.mode == PinCreateMode::NewVariation && self.mux_name_collides(&mux_name) {
            return Err(format!(
                "pin create: mux name `{mux_name}` is already used; choose a new name or edit the existing pin"
            ));
        }
        let launch_argv = self.launch_argv_override()?;
        if launch_argv.is_empty() && self.default_launch_argv().is_empty() {
            return Err(format!(
                "pin create: launch argv is required for unknown harness `{harness}`"
            ));
        }
        let worktree_branch = if self.spec.worktree_enabled() {
            Some(required(
                self.spec.worktree_branch().value(),
                "worktree branch",
            )?)
        } else {
            None
        };
        Ok(PinCreateRequest {
            id,
            display_name,
            harness,
            cwd,
            mux_name,
            mux_socket,
            adopt_source_mux_name: self.adopt_source_mux_name(),
            launch_argv,
            worktree_branch,
            store: self.store,
        })
    }

    pub(super) fn launch_argv_override(&self) -> Result<Vec<String>, String> {
        parse_launch_argv(self.spec.launch_argv().value().trim(), "pin create")
    }

    pub(super) fn default_launch_argv(&self) -> Vec<String> {
        launch_argv_for(self.spec.harness().value().trim())
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

    pub(super) fn adopt_source_mux_name(&self) -> Option<String> {
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

    pub(super) fn matching_known_mux_name(&self) -> Option<&str> {
        self.spec.matching_known_mux_name()
    }

    pub(super) fn mux_name_collides(&self, mux_name: &str) -> bool {
        self.spec
            .known_mux_names()
            .iter()
            .any(|known| known == mux_name)
            || self
                .known_pin_mux_names
                .iter()
                .any(|known| known == mux_name)
    }

    pub(super) fn mux_name_display(&self) -> String {
        let value = self.spec.mux_name().value();
        match self.adopt_source_mux_name() {
            Some(source) if source != value => format!("{value} (rename of: {source})"),
            _ => value.to_string(),
        }
    }

    #[cfg(test)]
    pub(super) fn harness_warning(&self) -> Option<String> {
        let value = self.spec.harness().value().trim();
        if value.is_empty()
            || self
                .spec
                .known_harness_keys()
                .iter()
                .any(|known| known == value)
        {
            None
        } else {
            Some(format!("custom harness `{value}` will be saved as typed"))
        }
    }
}

pub(super) fn completion_remainder(typed: &str, suggestion: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Some(suggestion.to_string());
    }
    suggestion
        .strip_prefix(typed)
        .filter(|remainder| !remainder.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let typed_basename = typed.rsplit('/').next().unwrap_or(typed);
            let suggestion_basename = suggestion.rsplit('/').next().unwrap_or(suggestion);
            suggestion_basename
                .strip_prefix(typed_basename)
                .filter(|remainder| !remainder.is_empty())
                .map(str::to_string)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LaunchArgvSource {
    Default,
    Override,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LaunchArgvPreview {
    pub(super) source: LaunchArgvSource,
    pub(super) argv: Vec<String>,
}

pub(super) fn normalized_harness_keys<'a>(
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

pub(super) fn pin_create_cwd_candidates(
    defaults: &PinCreateDefaults,
    adopt_defaults: Option<&PinCreateDefaults>,
    known: Vec<PathCandidate>,
) -> Vec<PathCandidate> {
    let mut candidates = Vec::new();
    if !defaults.cwd.trim().is_empty() {
        candidates.push(PathCandidate::new(defaults.cwd.clone(), "selected", 1_000));
    }
    if let Some(adopt) = adopt_defaults
        && !adopt.cwd.trim().is_empty()
        && adopt.cwd != defaults.cwd
    {
        candidates.push(PathCandidate::new(adopt.cwd.clone(), "adopt", 900));
    }
    candidates.extend(known);
    candidates
}

pub(super) fn display_launch_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| shell_display_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn shell_display_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '=' | '+')
    }) {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\\''"))
}

pub(super) fn pin_id_candidate(raw: &str) -> String {
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

pub(super) fn derived_mux_name_for_mode(
    mode: PinCreateMode,
    name: &str,
    derived_id: &str,
) -> String {
    match mode {
        PinCreateMode::NewVariation => derived_id.to_string(),
        PinCreateMode::AdoptSelected => name.to_string(),
    }
}
