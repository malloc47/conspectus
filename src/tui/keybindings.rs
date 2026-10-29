//! Declarative keybinding table.
//!
//! A `(mode/focus, key, action, help)` table so the dispatcher,
//! the focus remap, the help overlay, and the controls hint
//! footers all consume the same source-of-truth list. A drift test
//! asserts every entry's `help_text` appears in
//! `crate::tui::widgets::help::keymap_sections`. Keys the table
//! doesn't hold stay as match arms in `keymap::translate`
//! (`PageUp` / `PageDown`, which the help page documents as one
//! pair, among them).
//!
//! Actions with runtime parameters (`SwitchView(v)`,
//! `CycleView(dir)`) are represented as closures returning the
//! parameterized action; parameter-free actions are stored
//! directly. The table stays a compile-time `&'static`
//! (no allocation on init) and every entry is
//! `#[derive(Debug)]` for the drift test's assertion messages.

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

pub mod overlay;

use tuikit::keys::KeyMatcher;

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

/// Focus context a [`KeyBinding`] applies to. Keys inside an
/// overlay live in that overlay's [`overlay`] table instead
/// (ADR 0123).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    /// Fires regardless of pane focus.
    Global,
}

/// The undo key. Toasts that offer an undo name it from here.
pub const UNDO_KEY: char = 'u';

/// Wave 1 pilot: view switching bindings. Each entry produces
/// a parameterized `Action::SwitchView` or `Action::CycleView`.
///
/// Wave 2 folds `translate`'s 52 `KeyCode::` arms into this
/// same table; wave 3 does the same for the runtime
/// dispatcher's 76 arms; wave 4 rewires `keymap_sections` to be derived from
/// the table so the help overlay can't drift.
/// Help text for `!`, shared with the help overlay's
/// "Discoverable controls" section.
pub const MESSAGES_HELP: &str =
    "Open the messages log: launch, attach and rename outcomes with full output";

/// Help text for Ctrl-R, shared with the help overlay.
pub const HARD_REFRESH_HELP: &str =
    "Hard refresh: rescan and re-query forges (gh) for pull requests";

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
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::NONE,
            code: KeyCode::Char('3'),
        },
        action: || Action::SwitchView(View::Prs),
        help_text: "Switch to PRs view",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::NONE,
            code: KeyCode::Char('4'),
        },
        action: || Action::SwitchView(View::Pinned),
        help_text: "Switch to Pinned view",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::NONE,
            code: KeyCode::Char('5'),
        },
        action: || Action::SwitchView(View::History),
        help_text: "Switch to History view",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::NONE,
            code: KeyCode::Char('6'),
        },
        action: || Action::SwitchView(View::Search),
        help_text: "Switch to the Search view",
    },
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
        key: KeyMatcher::Char('S'),
        action: || Action::Resume,
        help_text: "Resume the selected un-muxed agent session",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Char('R'),
        action: || Action::OpenRename,
        help_text: "Rename the selected agent session or mux",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('w'),
        action: || Action::OpenWorktreeMenu,
        help_text: "Open the worktree action menu for the selected node",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Char('X'),
        action: || Action::OpenWorktreeCloseDown,
        help_text: "Close down the selected stream (merge/discard + teardown)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('n'),
        action: || Action::OpenNewMuxForm,
        help_text: "Create a new bare tmux session (no agent)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Char('N'),
        action: || Action::Msg(Box::new(crate::tui::Msg::OpenMuxLaunchForm)),
        help_text: "Launch a harness in a new mux (the form can add a worktree)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('m'),
        action: || Action::OpenMuxMenu,
        help_text: "Open the Mux action menu (new session / launch harness)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::NONE,
            code: KeyCode::Char('.'),
        },
        action: || Action::Msg(Box::new(crate::tui::Msg::OpenRowMenu)),
        help_text: "Open the row action menu (pin, rebuild, forget, clean up)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('p'),
        action: || Action::Msg(Box::new(crate::tui::Msg::TogglePin)),
        help_text: "Pin the selected row, or unpin it when it already is",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl(UNDO_KEY),
        action: || Action::Msg(Box::new(crate::tui::Msg::Undo)),
        help_text: "Undo the last pin or unpin",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Char('F'),
        action: || Action::ClearFilters,
        help_text: "Clear every active filter",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Char('E'),
        action: || Action::Msg(Box::new(crate::tui::Msg::ToggleEdgeMeta)),
        help_text: "Toggle explorer edge-meta visibility",
    },
    // Parametric-guard shortcuts. Each
    // matcher fires on any non-Ctrl modifier + the char, so Ctrl
    // chords (`Ctrl-R`, the hard refresh) stay distinct.
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('r'),
        action: || Action::Refresh,
        help_text: "Refresh discovery now",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('r'),
        },
        action: || Action::HardRefresh,
        help_text: HARD_REFRESH_HELP,
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('a'),
        action: || Action::Attach,
        help_text: "Attach to the selected mux",
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
        help_text: "Open the controls overlay (view / grouping / filters / sort / preview wrap)",
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
        help_text: "Quick select: find a row in this view, or Tab to find anything",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('s'),
        action: || Action::OpenContentSearch,
        help_text: "Search the contents of sessions and mux scrollback",
    },
    // Ctrl-/ reaches the app as one of three keys, depending on the
    // terminal: a terminal using the kitty keyboard protocol sends it as
    // such; the rest send the byte 0x1F, which decodes as Ctrl-7 (and
    // which is also what Ctrl-_ sends).
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('/'),
        },
        action: || Action::OpenContentSearch,
        help_text: "Search the contents of sessions and mux scrollback (Ctrl-/)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('7'),
        },
        action: || Action::OpenContentSearch,
        help_text: "Ctrl-/ as most terminals send it (Ctrl-7)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('_'),
        },
        action: || Action::OpenContentSearch,
        help_text: "Ctrl-/ as some terminals send it (Ctrl-_)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('<'),
        action: || Action::Msg(Box::new(crate::tui::Msg::PageSearchHistory(-1))),
        help_text: "Search view: show the previous (older) search",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('>'),
        action: || Action::Msg(Box::new(crate::tui::Msg::PageSearchHistory(1))),
        help_text: "Search view: show the next (newer) search",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('f'),
        },
        action: || Action::Msg(Box::new(crate::tui::Msg::ApplySearchEntry)),
        help_text: "Search view: keep the shown search as the filter on the lists",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('l'),
        },
        action: || Action::Msg(Box::new(crate::tui::Msg::ToggleSearchGranularity)),
        help_text: "Search view: list sessions, or the matching messages",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('o'),
        },
        action: || Action::Msg(Box::new(crate::tui::Msg::OpenSearchOptions)),
        help_text: "Search view: choose what a search looks in (tools, thinking, panes)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('?'),
        action: || Action::OpenHelp,
        help_text: "Show the help overlay",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::AnyModExceptCtrl('!'),
        action: || Action::OpenMessages,
        help_text: MESSAGES_HELP,
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
        action: || Action::Msg(Box::new(crate::tui::Msg::ExplorerToggleGroup)),
        help_text: "Toggle the explorer's Other zone (right pane)",
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
    // Shift+Enter needs a terminal that reports it (kitty protocol);
    // Alt+Enter arrives everywhere.
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::SHIFT,
            code: KeyCode::Enter,
        },
        action: || Action::Msg(Box::new(crate::tui::Msg::RevealSelected)),
        help_text: "Reveal the row's entity in its own view (Backspace returns)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::ALT,
            code: KeyCode::Enter,
        },
        action: || Action::Msg(Box::new(crate::tui::Msg::RevealSelected)),
        help_text: "Reveal the row's entity in its own view (Backspace returns)",
    },
    KeyBinding {
        mode: KeyMode::Global,
        key: KeyMatcher::Char('O'),
        action: || Action::Msg(Box::new(crate::tui::Msg::RevealSelected)),
        help_text: "Reveal the row's entity in its own view (Backspace returns)",
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

/// Look up `(modifiers, code)` in [`KEYBINDINGS`]. Called first by
/// [`crate::tui::keymap::translate`]; keys the table doesn't hold
/// fall through to its remaining match arms.
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
