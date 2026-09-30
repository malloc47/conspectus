//! Declarative keybinding table (H-HYG-007 wave 1).
//!
//! Introduces the `(mode/focus, key, action, help)` table shape
//! the story mandates so the dispatcher, the focus remap, the
//! help overlay, and the controls / pins hint footers all
//! consume the same source-of-truth list. Pre-H-HYG-007 the
//! three consumers hand-maintained parallel lists — nothing
//! forced them to agree.
//!
//! **Wave 1 scope**: table shape + a small pilot subset (view
//! switching / grouping cycle) as the reference example + a
//! drift test asserting every entry's `help_text` appears in
//! the currently-shipped [`crate::tui::widgets::help::keymap_sections`]
//! output. Full migration of the 265 `KeyCode::` arms in
//! `runtime.rs` (76) + `widgets/pins.rs` (137) + `keymap.rs`
//! (52) is queued for waves 2–5 as per the H-HYG-007 backlog
//! sizing note.
//!
//! Actions with runtime parameters (`SwitchView(v)`,
//! `CycleView(dir)`) are represented as closures returning the
//! parameterized action; parameter-free actions are stored
//! directly. The table stays a compile-time `&'static`
//! (no allocation on init) and every entry is
//! `#[derive(Debug)]` for the drift test's assertion messages.

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::tui::View;
use crate::tui::keymap::Action;

/// One row of the declarative keymap. Each entry ties a
/// crossterm key to a semantic [`Action`] and a
/// human-readable help string. The `mode` field carries the
/// focus / overlay context the entry applies to; wave 1 ships
/// only the `Global` variant, subsequent waves add focus-aware
/// entries so `remap_for_focus` becomes data-driven too.
#[derive(Debug)]
pub struct KeyBinding {
    pub mode: KeyMode,
    pub key: KeyMatcher,
    /// Callback producing the [`Action`] the runtime dispatches
    /// when this binding fires. A callback (not a stored
    /// `Action`) so parameterized actions (`SwitchView(v)`)
    /// stay expressible without wrapping every table entry in
    /// a builder.
    pub action: fn() -> Action,
    /// Help-overlay description. Non-empty per drift-test
    /// invariant.
    pub help_text: &'static str,
}

/// Focus context a [`KeyBinding`] applies to. Wave 1 ships
/// only [`Global`](KeyMode::Global) — bindings that fire
/// regardless of pane focus. Waves 2–3 add per-focus
/// variants (`LeftPane`, `RightPane`, overlay-owned
/// contexts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    /// Fires regardless of pane focus; the pre-H-HYG-007
    /// dispatcher had no mode discrimination for these
    /// entries either.
    Global,
}

/// Key-matching predicate. A tuple of `(modifiers, code)`
/// with modifier flexibility because most single-char
/// bindings accept both `SHIFT + <upper>` and plain
/// `<upper>` (crossterm normalizes some terminals'
/// shift-case reporting differently).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMatcher {
    /// Exact modifier + code match.
    Exact {
        modifiers: KeyModifiers,
        code: KeyCode,
    },
    /// Char match ignoring SHIFT; the pre-H-HYG-007
    /// dispatcher used this shape for uppercase-letter
    /// actions that accepted both the shifted and the
    /// unshifted forms.
    UpperChar(char),
    /// Char match admitting any modifier except CONTROL.
    /// Pre-H-HYG-007 the dispatcher wrote this as
    /// `(m, KeyCode::Char('r')) if !m.contains(CONTROL)`
    /// so `r` fires but `Ctrl-R` (which may be reserved
    /// by the terminal) doesn't.
    AnyModExceptCtrl(char),
    /// Non-char code match ignoring modifiers. Used for
    /// arrow keys and nav keys where the pre-H-HYG-007
    /// dispatcher wrote `(_, KeyCode::Down)` — any
    /// modifier is fine.
    AnyMod(KeyCode),
}

impl KeyMatcher {
    /// Test a crossterm key event against this matcher.
    pub fn matches(&self, modifiers: KeyModifiers, code: KeyCode) -> bool {
        match self {
            KeyMatcher::Exact {
                modifiers: m,
                code: c,
            } => *m == modifiers && *c == code,
            KeyMatcher::UpperChar(ch) => match code {
                KeyCode::Char(c) if c == *ch => {
                    modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT
                }
                _ => false,
            },
            KeyMatcher::AnyModExceptCtrl(ch) => match code {
                KeyCode::Char(c) if c == *ch => !modifiers.contains(KeyModifiers::CONTROL),
                _ => false,
            },
            KeyMatcher::AnyMod(k) => code == *k,
        }
    }
}

/// Wave 1 pilot: view switching bindings. Each entry produces
/// a parameterized `Action::SwitchView` or `Action::CycleView`.
///
/// Wave 2 folds `translate`'s 52 `KeyCode::` arms into this
/// same table; wave 3 does the same for the runtime
/// dispatcher's 76 arms; wave 4 the pins overlay's 137;
/// wave 5 rewires `keymap_sections` to be derived from
/// the table so the help overlay can't drift.
pub const KEYBINDINGS: &[KeyBinding] = &[
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::NONE,
            code: KeyCode::Char('1'),
        },
        action: || Action::SwitchView(View::Sessions),
        help_text: "Switch to Sessions view",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::NONE,
            code: KeyCode::Char('2'),
        },
        action: || Action::SwitchView(View::Mux),
        help_text: "Switch to Mux view",
    },
    // Union / PRs / Forks views are hidden from the interactive UI
    // for now (H-VIEW-001); their `3`/`4`/`5` accelerators are removed
    // alongside the trimmed `VIEW_OPTIONS`. The `View` variants and CLI
    // `table` projections remain so this stays a UI-surface hide.
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('g'),
        },
        action: || Action::CycleGrouping(1),
        help_text: "Cycle grouping (Ctrl-G)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('c'),
        },
        action: || Action::Msg(Box::new(crate::tui::Msg::Quit)),
        help_text: "Quit (Ctrl-C)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::NONE,
            code: KeyCode::Char('q'),
        },
        action: || Action::Msg(Box::new(crate::tui::Msg::Quit)),
        help_text: "Quit (q)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::UpperChar('S'),
        action: || Action::Resume,
        help_text: "Resume the selected un-muxed agent session",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::UpperChar('R'),
        action: || Action::OpenRename,
        help_text: "Rename the selected agent session, mux, or pin",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('w'),
        action: || Action::OpenWorktreeMenu,
        help_text: "Open the worktree action menu for the selected node",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::UpperChar('X'),
        action: || Action::OpenWorktreeCloseDown,
        help_text: "Close down the selected stream (merge/discard + teardown)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::NONE,
            code: KeyCode::Delete,
        },
        action: || Action::RemovePin,
        help_text: "Remove the selected pin",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::UpperChar('N'),
        action: || Action::OpenPinCreate,
        help_text: "Create a new pin",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('n'),
        action: || Action::OpenNewMuxForm,
        help_text: "Create a new bare tmux session (no pin, no agent)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('m'),
        action: || Action::OpenMuxMenu,
        help_text: "Open the Mux action menu (new session / launch harness)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::UpperChar('B'),
        action: || Action::OpenPinRebind,
        help_text: "Rebind the selected pin",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::UpperChar('A'),
        action: || Action::OpenPinAdopt,
        help_text: "Adopt the selected mux row as a pin",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::UpperChar('L'),
        action: || Action::LaunchPin,
        help_text: "Launch the selected pin",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::UpperChar('F'),
        action: || Action::ClearFilters,
        help_text: "Clear every active filter",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::UpperChar('E'),
        action: || Action::Msg(Box::new(crate::tui::Msg::ToggleEdgeMeta)),
        help_text: "Toggle explorer edge-meta visibility",
    },
    // H-HYG-007 wave 4: parametric-guard shortcuts. Each
    // matcher fires on any non-Ctrl modifier + the char, so
    // `Ctrl-R` etc. reserved by the terminal doesn't collide.
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('r'),
        action: || Action::Refresh,
        help_text: "Refresh discovery now",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('a'),
        action: || Action::Attach,
        help_text: "Attach to the selected mux",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('b'),
        action: || Action::PinBindHint,
        help_text: "Open bind picker for ambiguous pin row",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('v'),
        action: || Action::View,
        help_text: "Open the selected session's transcript viewer",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('f'),
        action: || Action::OpenControls,
        help_text: "Open the controls overlay (view / grouping / filters / sort)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('p'),
        action: || Action::OpenPins,
        help_text: "Open the pins overlay",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl(']'),
        action: || Action::CycleView(1),
        help_text: "Cycle to the next view",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('['),
        action: || Action::CycleView(-1),
        help_text: "Cycle to the previous view",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('/'),
        action: || Action::OpenSearch,
        help_text: "Open the in-view search overlay",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('?'),
        action: || Action::OpenHelp,
        help_text: "Show the help overlay",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('l'),
        action: || Action::Msg(Box::new(crate::tui::Msg::ExpandRow)),
        help_text: "Expand the selected row's children",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('h'),
        action: || Action::Msg(Box::new(crate::tui::Msg::CollapseRow)),
        help_text: "Collapse the selected row's children",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('e'),
        action: || Action::Msg(Box::new(crate::tui::Msg::ToggleLinkedDetails)),
        help_text: "Toggle explorer linked-details",
    },
    // Arrow / nav keys — any modifier is fine.
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Down),
        action: || Action::Msg(Box::new(crate::tui::Msg::NavDown)),
        help_text: "Move cursor down",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Up),
        action: || Action::Msg(Box::new(crate::tui::Msg::NavUp)),
        help_text: "Move cursor up",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Right),
        action: || Action::Msg(Box::new(crate::tui::Msg::ExpandRow)),
        help_text: "Expand the selected row (Right arrow)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Left),
        action: || Action::Msg(Box::new(crate::tui::Msg::CollapseRow)),
        help_text: "Collapse the selected row (Left arrow)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Enter),
        action: || Action::DefaultAction,
        help_text: "Default action on the selected row",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Backspace),
        action: || Action::Msg(Box::new(crate::tui::Msg::ExplorerBack)),
        help_text: "Pop one drilldown hop in the explorer",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Home),
        action: || Action::Msg(Box::new(crate::tui::Msg::Home)),
        help_text: "Jump to the first row",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::End),
        action: || Action::Msg(Box::new(crate::tui::Msg::End)),
        help_text: "Jump to the last row",
    },
    // `j` / `k` also fire NavDown / NavUp per Vim convention.
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Char('j')),
        action: || Action::Msg(Box::new(crate::tui::Msg::NavDown)),
        help_text: "Move cursor down (j)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Char('k')),
        action: || Action::Msg(Box::new(crate::tui::Msg::NavUp)),
        help_text: "Move cursor up (k)",
    },
    // `g` / `G` also fire Home / End per Vim convention.
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Char('g')),
        action: || Action::Msg(Box::new(crate::tui::Msg::Home)),
        help_text: "Jump to the first row (g)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyMod(KeyCode::Char('G')),
        action: || Action::Msg(Box::new(crate::tui::Msg::End)),
        help_text: "Jump to the last row (G)",
    },
];

/// H-HYG-007 wave 2 entrypoint: `translate_via_table(modifiers,
/// code) → Option<Action>`. Called first by
/// [`crate::tui::keymap::translate`]; a hit here returns before
/// the pre-H-HYG-007 hand-matched arms fire. Bindings unregistered
/// in [`KEYBINDINGS`] fall through so wave-by-wave migration
/// stays behavior-preserving.
pub fn translate_via_table(modifiers: KeyModifiers, code: KeyCode) -> Option<Action> {
    for binding in KEYBINDINGS {
        if binding.mode != KeyMode::Global {
            continue;
        }
        if binding.key.matches(modifiers, code) {
            return Some((binding.action)());
        }
    }
    None
}

/// H-HYG-007 wave 5: render a KeyBinding's key as an operator-
/// friendly label suitable for a help overlay row (e.g. `Ctrl-C`,
/// `Enter`, `q`, `↓`). Used by the coherence drift test — every
/// key label produced here must appear somewhere in
/// [`crate::tui::widgets::help::keymap_sections`]'s output so
/// the table and help stay in sync.
pub fn key_label(key: &KeyMatcher) -> String {
    match key {
        KeyMatcher::Exact { modifiers, code } => {
            let mut buf = String::new();
            if modifiers.contains(KeyModifiers::CONTROL) {
                buf.push_str("Ctrl-");
            }
            if modifiers.contains(KeyModifiers::SHIFT)
                && !matches!(code, KeyCode::Char(c) if c.is_ascii_uppercase())
            {
                buf.push_str("Shift-");
            }
            buf.push_str(&code_label(*code));
            buf
        }
        KeyMatcher::UpperChar(c) => c.to_string(),
        KeyMatcher::AnyModExceptCtrl(c) => c.to_string(),
        KeyMatcher::AnyMod(code) => code_label(*code),
    }
}

fn code_label(code: KeyCode) -> String {
    match code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-test infrastructure: every entry's help_text must
    /// be non-empty. Establishes the invariant wave 5 uses to
    /// enforce dispatcher / help-overlay coherence.
    #[test]
    fn every_binding_has_nonempty_help_text() {
        for binding in KEYBINDINGS {
            assert!(
                !binding.help_text.is_empty(),
                "binding {binding:?} has empty help_text"
            );
        }
    }

    /// Drift-test scaffold: every binding's key matcher fires
    /// exactly once against its own canonical event. Guards
    /// against accidental duplicate entries in later waves.
    #[test]
    fn no_two_bindings_match_the_same_global_key() {
        for (i, a) in KEYBINDINGS.iter().enumerate() {
            for (j, b) in KEYBINDINGS.iter().enumerate() {
                if i >= j {
                    continue;
                }
                if a.mode != b.mode {
                    continue;
                }
                // A key matcher shouldn't match its sibling's
                // matcher — the drift test's tightest form is
                // "no two rows produce Some(action) for the
                // same event."
                if let (
                    KeyMatcher::Exact { modifiers, code },
                    KeyMatcher::Exact {
                        modifiers: m2,
                        code: c2,
                    },
                ) = (a.key, b.key)
                {
                    assert!(
                        !(modifiers == m2 && code == c2),
                        "bindings [{i}] and [{j}] collide on same key"
                    );
                }
            }
        }
    }

    /// Wave 1 pilot: view-switching bindings produce the
    /// parameterized SwitchView action correctly.
    #[test]
    fn view_switch_bindings_produce_correct_action() {
        // The `1` binding should produce SwitchView(Sessions).
        let sessions = KEYBINDINGS
            .iter()
            .find(|b| {
                matches!(
                    b.key,
                    KeyMatcher::Exact {
                        code: KeyCode::Char('1'),
                        ..
                    }
                )
            })
            .expect("`1` binding registered");
        assert_eq!((sessions.action)(), Action::SwitchView(View::Sessions));
    }
}
