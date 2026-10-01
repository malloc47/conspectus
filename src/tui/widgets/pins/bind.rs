//! Bind, remove, and rebind sub-editors.

use super::*;

impl PinBindState {
    pub(super) fn new(options: Vec<PinBindOption>) -> Self {
        Self { cursor: 0, options }
    }

    pub(super) fn handle_key(&mut self, event: KeyEvent) -> PinBindOutcome {
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

    pub(super) fn move_cursor(&mut self, delta: i32) {
        if self.options.is_empty() {
            return;
        }
        self.cursor = crate::tui::cursor::wrap_step(self.cursor, self.options.len(), delta);
    }
}

impl PinRemoveState {
    pub(super) fn new(target: PinMutationTarget) -> Self {
        Self { target }
    }

    pub(super) fn handle_key(&mut self, event: KeyEvent) -> PinRemoveOutcome {
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
    pub(super) const FIELD_COUNT: usize = 2;

    pub(super) fn new(target: PinMutationTarget) -> Self {
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

    pub(super) fn move_cursor(&mut self, delta: i32) {
        self.cursor = crate::tui::cursor::wrap_step(self.cursor, Self::FIELD_COUNT, delta);
    }

    pub(super) fn active_input_mut(&mut self) -> Option<&mut TextInputState> {
        match self.cursor {
            0 => Some(&mut self.mux_name),
            1 => Some(&mut self.mux_socket),
            _ => None,
        }
    }

    pub(super) fn request(&self) -> Result<PinEditRequest, String> {
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

pub(super) fn required(raw: &str, label: &str) -> Result<String, String> {
    optional(raw).ok_or_else(|| format!("pin create: {label} is required"))
}

pub(super) fn optional(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
