use super::*;
use ratatui::crossterm::event::KeyEvent;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use tuikit::explorer::ExplorerKey;
use tuikit::testkit::keys::chord;

fn msg(action: Option<Action>) -> Option<Msg> {
    // Action::Msg now boxes its payload (large enum variant);
    // unwrap for the test assertions.
    match action {
        Some(Action::Msg(msg)) => Some(*msg),
        _ => None,
    }
}

#[test]
fn translate_q_quits() {
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Char('q')
        )))),
        Some(Msg::Quit)
    );
}

#[test]
fn translate_ctrl_c_quits() {
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::CONTROL,
            KeyCode::Char('c')
        )))),
        Some(Msg::Quit)
    );
}

#[test]
fn translate_r_requests_refresh() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('r')))),
        Some(Action::Refresh)
    );
}

#[test]
fn translate_shift_r_opens_rename_overlay() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::SHIFT, KeyCode::Char('R')))),
        Some(Action::OpenRename)
    );
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('R')))),
        Some(Action::OpenRename)
    );
}

#[test]
fn translate_lowercase_r_still_refreshes() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('r')))),
        Some(Action::Refresh)
    );
}

#[test]
fn a_refresh_after_an_action_reads_the_daemons_snapshot() {
    let mut config = RunConfig::defaults();
    config.refresh = true;
    config.no_cache = true;
    config.default_view = View::Mux;
    let app = App::new(config);

    let refresh_config = store_write_refresh_config(&app);

    // Forcing a local rebuild would bypass the daemon and with it every
    // PR the daemon had fetched.
    assert!(!refresh_config.refresh);
    assert!(refresh_config.no_cache);
    assert_eq!(refresh_config.default_view, View::Mux);
}

#[test]
fn translate_a_requests_attach() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('a')))),
        Some(Action::Attach)
    );
}

#[test]
fn translate_i_requests_copy_session_id() {
    // `i` resolves the selected agent or mux session's
    // full id and routes it through the OSC 52 clipboard
    // primitive (ADR 0056) at the main-loop boundary.
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('i')))),
        Some(Action::CopySessionId)
    );
    // Ctrl-I is the terminal alias for Tab; the binding must not
    // claim it.
    assert_ne!(
        translate(Event::Key(chord(KeyModifiers::CONTROL, KeyCode::Char('i')))),
        Some(Action::CopySessionId)
    );
}

/// What `event` does with `focus` on that pane and no modal open, as every
/// loop resolves it (`action_for_event`).
fn resolved(focus: crate::tui::app::Focus, event: KeyEvent) -> Option<Action> {
    let mut app = App::new(RunConfig::defaults());
    if focus == crate::tui::app::Focus::Right {
        app.update(Msg::CycleFocus);
    }
    action_for_event(&app, Event::Key(event))
}

/// The action of an explorer key.
fn explorer(key: ExplorerKey) -> Action {
    Action::Msg(Box::new(Msg::Explorer(key)))
}

/// A key action that sends `msg`.
fn sends(msg: Msg) -> Action {
    Action::Msg(Box::new(msg))
}

#[test]
fn e_folds_the_explorer_other_zone_on_the_right_pane_only() {
    use crate::tui::app::Focus;
    let e = chord(KeyModifiers::NONE, KeyCode::Char('e'));
    assert_eq!(
        resolved(Focus::Right, e),
        Some(explorer(ExplorerKey::ToggleSection)),
        "`e` has one meaning: the explorer's Other zone"
    );
    assert_eq!(
        resolved(Focus::Left, e),
        None,
        "the left tree has no Other zone, so the key does nothing there"
    );
    assert_eq!(
        translate(Event::Key(e)),
        None,
        "only the right pane's table binds it"
    );
}

#[test]
fn translate_v_opens_viewer() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('v')))),
        Some(Action::View)
    );
}

#[test]
fn translate_f_opens_controls_overlay() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('f')))),
        Some(Action::OpenControls)
    );
}

#[test]
fn translate_upper_t_no_longer_bound_to_view() {
    // `v` owns View; `T` is unbound and falls through to None.
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('T')))),
        None,
    );
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::SHIFT, KeyCode::Char('T')))),
        None,
    );
}

// Overlay routing coverage lives in
// `tui::runtime::tests::overlay_routing`.

#[test]
fn translate_shift_f_clears_filters() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::SHIFT, KeyCode::Char('F')))),
        Some(Action::ClearFilters)
    );
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('F')))),
        Some(Action::ClearFilters)
    );
}

#[test]
fn translate_shift_e_toggles_edge_meta() {
    // `E` toggles the explorer's edge-meta visibility.
    // Focus-agnostic — the binding is global so the operator
    // can flip it without first tabbing into the right pane.
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::SHIFT, KeyCode::Char('E')))),
        Some(Action::Msg(Box::new(Msg::ToggleEdgeMeta)))
    );
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('E')))),
        Some(Action::Msg(Box::new(Msg::ToggleEdgeMeta)))
    );
}

#[test]
fn translate_ctrl_g_cycles_grouping() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::CONTROL, KeyCode::Char('g')))),
        Some(Action::CycleGrouping(1))
    );
}

#[test]
fn translate_digits_switch_views_directly() {
    // Sessions, Mux, PRs, Pinned, History, and Search each have a digit.
    let cases = [
        ('1', View::Sessions),
        ('2', View::Mux),
        ('3', View::Prs),
        ('4', View::Pinned),
        ('5', View::History),
        ('6', View::Search),
    ];
    for (ch, view) in cases {
        assert_eq!(
            translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char(ch)))),
            Some(Action::SwitchView(view)),
            "char {ch}"
        );
    }
    for ch in ['7', '8'] {
        assert_eq!(
            translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char(ch)))),
            None,
            "no view on char {ch}"
        );
    }
}

#[test]
fn translate_brackets_cycle_views() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char(']')))),
        Some(Action::CycleView(1))
    );
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('[')))),
        Some(Action::CycleView(-1))
    );
}

#[test]
fn cycle_view_wraps_in_both_directions() {
    // VIEW_OPTIONS is [Sessions, Mux, Prs, Pinned, History, Search], so
    // the cycle wraps across those six.
    assert_eq!(cycle_view(View::Sessions, -1), View::Search);
    assert_eq!(cycle_view(View::Search, 1), View::Sessions);
    assert_eq!(cycle_view(View::History, 1), View::Search);
    assert_eq!(cycle_view(View::Prs, 1), View::Pinned);
    assert_eq!(cycle_view(View::Pinned, 1), View::History);
    assert_eq!(cycle_view(View::Sessions, 1), View::Mux);
    assert_eq!(cycle_view(View::Mux, 1), View::Prs);
    assert_eq!(cycle_view(View::Mux, -1), View::Sessions);
}

#[test]
fn translate_ignores_control_a() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::CONTROL, KeyCode::Char('a')))),
        None
    );
}

#[test]
fn control_r_is_a_hard_refresh_and_r_a_plain_one() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::CONTROL, KeyCode::Char('r')))),
        Some(Action::HardRefresh)
    );
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('r')))),
        Some(Action::Refresh)
    );
}

#[test]
fn translate_maps_navigation_keys() {
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Char('j')
        )))),
        Some(Msg::NavDown)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Char('k')
        )))),
        Some(Msg::NavUp)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Down
        )))),
        Some(Msg::NavDown)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Up
        )))),
        Some(Msg::NavUp)
    );
    // Enter no longer maps to a Msg directly. It is
    // resolved against the selected row's kind by the dispatcher
    // at the call site (and remapped to the explorer's `Activate`
    // when the right pane has focus).
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Enter))),
        Some(Action::DefaultAction)
    );
    // Vi-style tree fold keys: `l` / `→` expand, `h` / `←`
    // collapse the selected left-tree row.
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Char('l')
        )))),
        Some(Msg::ExpandRow)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Right
        )))),
        Some(Msg::ExpandRow)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Char('h')
        )))),
        Some(Msg::CollapseRow)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Left
        )))),
        Some(Msg::CollapseRow)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Tab
        )))),
        Some(Msg::CycleFocus)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Char('g')
        )))),
        Some(Msg::Home)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Char('G')
        )))),
        Some(Msg::End)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::PageDown
        )))),
        Some(Msg::PageDown)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::PageUp
        )))),
        Some(Msg::PageUp)
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Char('J')
        )))),
        Some(Msg::ScrollPreviewBy(1))
    );
    assert_eq!(
        msg(translate(Event::Key(chord(
            KeyModifiers::NONE,
            KeyCode::Char('K')
        )))),
        Some(Msg::ScrollPreviewBy(-1))
    );
}

#[test]
fn the_left_pane_keeps_the_program_wide_keymap() {
    use crate::tui::app::Focus;
    let key = |code| chord(KeyModifiers::NONE, code);
    assert_eq!(
        resolved(Focus::Left, key(KeyCode::Char('j'))),
        Some(sends(Msg::NavDown))
    );
    assert_eq!(
        resolved(Focus::Left, key(KeyCode::PageDown)),
        Some(sends(Msg::PageDown))
    );
    assert_eq!(
        resolved(Focus::Left, key(KeyCode::Enter)),
        Some(Action::DefaultAction)
    );
    assert_eq!(
        resolved(Focus::Left, chord(KeyModifiers::SHIFT, KeyCode::Char('F'))),
        Some(Action::ClearFilters)
    );
    assert_eq!(
        resolved(Focus::Left, key(KeyCode::Backspace)),
        Some(sends(Msg::NavBack))
    );
    assert_eq!(
        resolved(Focus::Left, key(KeyCode::Char('h'))),
        Some(sends(Msg::CollapseRow))
    );
    assert_eq!(
        resolved(Focus::Left, key(KeyCode::Esc)),
        None,
        "Esc means nothing here"
    );
}

#[test]
fn the_right_pane_moves_and_pages_the_explorer() {
    use crate::tui::app::Focus;
    // With the right pane focused, j/k and the arrows move the explorer
    // cursor, and the page keys page it by the rows its zone shows (they
    // moved one row). J/K (uppercase) keep their preview-scroll role.
    let key = |code| resolved(Focus::Right, chord(KeyModifiers::NONE, code));
    assert_eq!(key(KeyCode::Char('j')), Some(explorer(ExplorerKey::Down)));
    assert_eq!(key(KeyCode::Down), Some(explorer(ExplorerKey::Down)));
    assert_eq!(key(KeyCode::Char('k')), Some(explorer(ExplorerKey::Up)));
    assert_eq!(key(KeyCode::Up), Some(explorer(ExplorerKey::Up)));
    assert_eq!(
        key(KeyCode::PageDown),
        Some(explorer(ExplorerKey::PageDown))
    );
    assert_eq!(key(KeyCode::PageUp), Some(explorer(ExplorerKey::PageUp)));
    assert_eq!(
        resolved(Focus::Right, chord(KeyModifiers::SHIFT, KeyCode::Char('J'))),
        Some(sends(Msg::ScrollPreviewBy(1)))
    );
}

#[test]
fn the_right_pane_takes_enter_f_and_backspace_and_leaves_the_rest_global() {
    use crate::tui::app::Focus;
    let none = KeyModifiers::NONE;
    // Locked decision 8: Enter is the universal "do the obvious thing"
    // key on the explorer cursor. The explorer reads the row (Node-zone
    // fields copy; other rows drill or fold), and the outcome says which.
    assert_eq!(
        resolved(Focus::Right, chord(none, KeyCode::Enter)),
        Some(explorer(ExplorerKey::Activate))
    );
    // The reveal chords are not the explorer's Enter: they reach the
    // program-wide keymap on either pane.
    for modifiers in [KeyModifiers::SHIFT, KeyModifiers::ALT] {
        assert_eq!(
            resolved(Focus::Right, chord(modifiers, KeyCode::Enter)),
            Some(sends(Msg::RevealSelected)),
            "{modifiers:?}"
        );
    }
    // F clears filters on the left tree but toggles the Expanded Node
    // Detail view on the right pane.
    assert_eq!(
        resolved(Focus::Right, chord(KeyModifiers::SHIFT, KeyCode::Char('F'))),
        Some(explorer(ExplorerKey::ToggleFull))
    );
    assert_eq!(
        resolved(Focus::Right, chord(none, KeyCode::Backspace)),
        Some(explorer(ExplorerKey::Back))
    );
    // `o` opens the right pane's cursor value from either pane.
    for focus in [Focus::Left, Focus::Right] {
        assert_eq!(
            resolved(focus, chord(none, KeyCode::Char('o'))),
            Some(explorer(ExplorerKey::Inspect)),
            "{focus:?}"
        );
    }
    // Keys the right pane's table leaves alone mean what they mean
    // everywhere.
    assert_eq!(
        resolved(Focus::Right, chord(none, KeyCode::Tab)),
        Some(sends(Msg::CycleFocus))
    );
    assert_eq!(
        resolved(Focus::Right, chord(none, KeyCode::Char('q'))),
        Some(sends(Msg::Quit))
    );
    assert_eq!(
        resolved(Focus::Right, chord(none, KeyCode::Char('r'))),
        Some(Action::Refresh)
    );
    assert_eq!(
        resolved(Focus::Right, chord(KeyModifiers::SHIFT, KeyCode::Char('E'))),
        Some(sends(Msg::ToggleEdgeMeta))
    );
    assert_eq!(
        resolved(Focus::Right, chord(KeyModifiers::SHIFT, KeyCode::Char('O'))),
        Some(sends(Msg::RevealSelected))
    );
}

#[test]
fn the_right_pane_folds_its_section_with_h_l_and_the_arrows() {
    // `h` / `l` / `←` / `→` fold the left tree's row on the left pane.
    // On the right pane they used to do nothing (the focus remap dropped
    // them so they could not mutate the tree the operator wasn't
    // driving); now they close and open the explorer's Other zone, the
    // tree's fold gesture.
    use crate::tui::app::Focus;
    let key = |focus, code| resolved(focus, chord(KeyModifiers::NONE, code));
    assert_eq!(
        key(Focus::Right, KeyCode::Char('h')),
        Some(explorer(ExplorerKey::CloseSection))
    );
    assert_eq!(
        key(Focus::Right, KeyCode::Left),
        Some(explorer(ExplorerKey::CloseSection))
    );
    assert_eq!(
        key(Focus::Right, KeyCode::Char('l')),
        Some(explorer(ExplorerKey::OpenSection))
    );
    assert_eq!(
        key(Focus::Right, KeyCode::Right),
        Some(explorer(ExplorerKey::OpenSection))
    );
    assert_eq!(
        key(Focus::Left, KeyCode::Char('l')),
        Some(sends(Msg::ExpandRow))
    );
    assert_eq!(
        key(Focus::Left, KeyCode::Right),
        Some(sends(Msg::ExpandRow))
    );
    assert_eq!(
        key(Focus::Left, KeyCode::Char('h')),
        Some(sends(Msg::CollapseRow))
    );
    assert_eq!(
        key(Focus::Left, KeyCode::Left),
        Some(sends(Msg::CollapseRow))
    );
}

#[test]
fn the_right_pane_goes_to_its_ends_and_steps_back_with_backspace_and_esc() {
    // `g`/`Home` and `G`/`End` go to the explorer's first and last row on
    // right-pane focus instead of bleeding into the left tree's ends.
    // `Esc` steps back one level, as `Backspace` does. `Tab` is the focus
    // toggle on either side.
    use crate::tui::app::Focus;
    let none = KeyModifiers::NONE;
    let key = |focus, modifiers, code| resolved(focus, chord(modifiers, code));
    assert_eq!(
        key(Focus::Right, none, KeyCode::Home),
        Some(explorer(ExplorerKey::First))
    );
    assert_eq!(
        key(Focus::Right, none, KeyCode::Char('g')),
        Some(explorer(ExplorerKey::First))
    );
    assert_eq!(
        key(Focus::Right, none, KeyCode::End),
        Some(explorer(ExplorerKey::Last))
    );
    assert_eq!(
        key(Focus::Right, KeyModifiers::SHIFT, KeyCode::Char('G')),
        Some(explorer(ExplorerKey::Last))
    );
    assert_eq!(
        key(Focus::Right, none, KeyCode::Esc),
        Some(explorer(ExplorerKey::Back))
    );
    assert_eq!(
        key(Focus::Left, none, KeyCode::Home),
        Some(sends(Msg::Home))
    );
    assert_eq!(key(Focus::Left, none, KeyCode::End), Some(sends(Msg::End)));
    for focus in [Focus::Left, Focus::Right] {
        assert_eq!(key(focus, none, KeyCode::Tab), Some(sends(Msg::CycleFocus)));
    }
}

#[test]
fn the_right_panes_table_declares_what_each_shared_key_means_on_either_pane() {
    // The table on `RIGHT_PANE_KEYS`: the keys that mean one thing on the
    // left pane (the program-wide keymap) and another on the right (the
    // right pane's table).
    use crate::tui::app::Focus;
    let none = KeyModifiers::NONE;
    let cases = [
        (
            chord(none, KeyCode::Enter),
            Some(Action::DefaultAction),
            Some(explorer(ExplorerKey::Activate)),
        ),
        (
            chord(KeyModifiers::SHIFT, KeyCode::Char('F')),
            Some(Action::ClearFilters),
            Some(explorer(ExplorerKey::ToggleFull)),
        ),
        (
            chord(none, KeyCode::Backspace),
            Some(sends(Msg::NavBack)),
            Some(explorer(ExplorerKey::Back)),
        ),
        (
            chord(none, KeyCode::Char('h')),
            Some(sends(Msg::CollapseRow)),
            Some(explorer(ExplorerKey::CloseSection)),
        ),
        (
            chord(none, KeyCode::Right),
            Some(sends(Msg::ExpandRow)),
            Some(explorer(ExplorerKey::OpenSection)),
        ),
        (
            chord(none, KeyCode::PageDown),
            Some(sends(Msg::PageDown)),
            Some(explorer(ExplorerKey::PageDown)),
        ),
        (
            chord(none, KeyCode::PageUp),
            Some(sends(Msg::PageUp)),
            Some(explorer(ExplorerKey::PageUp)),
        ),
        (
            chord(none, KeyCode::Esc),
            None,
            Some(explorer(ExplorerKey::Back)),
        ),
        (
            chord(none, KeyCode::Char('e')),
            None,
            Some(explorer(ExplorerKey::ToggleSection)),
        ),
        (
            chord(none, KeyCode::Char('o')),
            Some(explorer(ExplorerKey::Inspect)),
            Some(explorer(ExplorerKey::Inspect)),
        ),
    ];
    for (event, left, right) in cases {
        assert_eq!(resolved(Focus::Left, event), left, "left {event:?}");
        assert_eq!(resolved(Focus::Right, event), right, "right {event:?}");
    }
}

#[test]
fn the_inspect_key_the_cut_hint_names_is_the_right_panes_inspect_key() {
    use crate::tui::keybindings::RIGHT_PANE_KEYS;
    use crate::tui::keymap::INSPECT_KEY;
    use tuikit::keys::{Hint, KeyRow, ch, rows};
    let bound: Vec<_> = rows(RIGHT_PANE_KEYS)
        .filter_map(|row| match row {
            KeyRow::Bind {
                key,
                action: ExplorerKey::Inspect,
                hint,
                ..
            } => Some((*key, *hint)),
            _ => None,
        })
        .collect();
    assert_eq!(bound.len(), 1, "one inspect key");
    let (key, hint) = bound[0];
    assert_eq!(key, ch(INSPECT_KEY));
    assert!(
        matches!(hint, Hint::Shown { label, .. } if label == INSPECT_KEY.to_string()),
        "{hint:?}"
    );
}

#[test]
fn chords_the_right_pane_does_not_bind_leave_the_left_tree_alone() {
    // The left pane's tree keys match `j` with any modifier, so `Alt-j`
    // used to move the explorer through the remap. The right pane's
    // table binds characters as typed, and the tree's keys are the left
    // pane's bindings, so these chords do nothing on the right pane.
    use crate::tui::app::Focus;
    use crate::tui::keybindings::{Command, LEFT_PANE, keymap};
    use tuikit::keys::{Binding, Scope};
    for (modifiers, c) in [
        (KeyModifiers::ALT, 'j'),
        (KeyModifiers::ALT, 'k'),
        (KeyModifiers::CONTROL, 'j'),
        (KeyModifiers::CONTROL, 'k'),
        (KeyModifiers::ALT, 'g'),
        (KeyModifiers::ALT | KeyModifiers::SHIFT, 'G'),
        (KeyModifiers::ALT, 'h'),
        (KeyModifiers::ALT, 'l'),
    ] {
        let event = chord(modifiers, KeyCode::Char(c));
        assert_eq!(resolved(Focus::Right, event), None, "{modifiers:?} {c}");
        assert!(
            resolved(Focus::Left, event).is_some(),
            "the left tree keeps {c}"
        );
    }
    // The tree's keys are the left pane's bindings, so none of them is
    // looked up with the right pane focused; the program's are.
    for command in [
        Command::Default,
        Command::ClearFilters,
        Command::Down,
        Command::Up,
        Command::PageDown,
        Command::PageUp,
        Command::First,
        Command::Last,
        Command::Expand,
        Command::Collapse,
        Command::Back,
    ] {
        let scopes: Vec<Scope> = keymap()
            .bindings()
            .iter()
            .filter(|binding| binding.action() == command)
            .map(Binding::scope)
            .collect();
        assert_eq!(scopes, [LEFT_PANE], "{command:?}");
    }
    for command in [
        Command::Refresh,
        Command::Focus,
        Command::Explorer(ExplorerKey::Inspect),
    ] {
        assert!(
            keymap()
                .bindings()
                .iter()
                .any(|binding| binding.action() == command && binding.scope() == Scope::Global),
            "{command:?}"
        );
    }
}

mod selected_default_action_tests {
    use super::*;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, GraphLink,
        GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
        Provenance, RelationKind, RepoId, RepoNode,
    };
    use crate::resolve::resolve_snapshot;
    use crate::tui::SessionsGrouping;
    use crate::tui::app::{Msg, SnapshotHandle};
    use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};

    fn session_node(harness: &str, scope: &str, key: &str, cwd: &str) -> GraphNode {
        GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new(harness, scope, key),
                harness.to_string(),
            )
            .with_cwd(cwd.to_string()),
        )
    }

    fn mux_node(backend: &str, native: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode::new(
            MuxSessionId::new(format!("{backend}:{native}")),
            backend.to_string(),
            native.to_string(),
        ))
    }

    fn linked_to_mux(session: &NodeId, mux: &NodeId, suffix: &str) -> GraphLink {
        GraphLink {
            id: format!("session-mux-{suffix}"),
            source: session.clone(),
            target: LinkEndpoint::Node { id: mux.clone() },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn add_repo_and_worktree(snapshot: &mut GraphSnapshot, common_dir: &str) {
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(common_dir))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new(common_dir), common_dir.to_string()),
            root: common_dir.to_string(),
            git_dir: None,
            current_branch: None,
            worktree: None,
            head_commit: None,
        }));
    }

    fn build_sessions_app(snapshot: GraphSnapshot) -> App {
        let snapshot = resolve_snapshot(snapshot);
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: None,
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });
        let mut cfg = RunConfig::defaults();
        cfg.default_view = View::Sessions;
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: SnapshotHandle::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app
    }

    #[test]
    fn empty_selection_falls_back_to_toggle_expand() {
        let app = App::new(RunConfig::defaults());
        assert_eq!(selected_default_action(&app), SelectedDefault::ToggleExpand);
    }

    #[test]
    fn group_row_resolves_to_toggle_expand() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        // Auto-selection lands on the repo group row.
        let app = build_sessions_app(snapshot);
        assert_eq!(selected_default_action(&app), SelectedDefault::ToggleExpand);
    }

    #[test]
    fn unmuxed_session_resolves_to_view() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        let mut app = build_sessions_app(snapshot);
        // Step past the group row to the session row.
        app.update(Msg::NavDown);
        assert_eq!(selected_default_action(&app), SelectedDefault::View);
    }

    #[test]
    fn muxed_session_resolves_to_attach() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        snapshot
            .candidate_links
            .push(linked_to_mux(&session_id, &mux_id, "1"));
        let mut app = build_sessions_app(snapshot);
        app.update(Msg::NavDown);
        assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
    }

    #[test]
    fn mux_candidate_child_resolves_to_attach() {
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        snapshot
            .nodes
            .push(session_node("codex", "/state", "abc", "/p/proj"));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.nodes.push(mux_node("tmux", "scratch"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let editor = NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        let scratch = NodeId::MuxSession(MuxSessionId::new("tmux:scratch"));
        snapshot
            .candidate_links
            .push(linked_to_mux(&session_id, &editor, "1"));
        snapshot
            .candidate_links
            .push(linked_to_mux(&session_id, &scratch, "2"));
        let mut app = build_sessions_app(snapshot);
        // Group → ambiguous session → expand → candidate child.
        app.update(Msg::NavDown);
        app.update(Msg::ToggleExpand);
        app.update(Msg::NavDown);
        assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
    }

    #[test]
    fn mux_view_mux_row_resolves_to_attach() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("tmux", "editor"));
        let snapshot = resolve_snapshot(snapshot);
        let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: None,
            filter: RowFilter::default(),
            grouping: crate::tui::MuxGrouping::Session,
            sort: crate::tui::Sort::Hierarchy,
            mux_recency: crate::tui::MuxRecency::default(),
        });
        let mut cfg = RunConfig::defaults();
        cfg.default_view = View::Mux;
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: SnapshotHandle::new(snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
    }

    #[test]
    fn unmuxed_session_resolves_to_view_regardless_of_viewer_support() {
        // The dispatcher routes every un-muxed agent
        // session through `SelectedDefault::View`. Harnesses
        // without a registered viewer (or whose viewer binary is
        // missing from `$PATH`) still resolve to `View` here —
        // the runtime's `view_action` is what surfaces the
        // `viewer_disabled_reason` status message after the
        // operator presses Enter. Keeping the dispatcher
        // uniform keeps the keymap consistent across harnesses
        // and lets the fallback message stay actionable.
        let mut snapshot = GraphSnapshot::empty();
        add_repo_and_worktree(&mut snapshot, "/p/proj");
        // `aider` has no viewer registered, so this is the
        // unsupported-viewer surface for the dispatcher.
        snapshot
            .nodes
            .push(session_node("aider", "/state", "abc", "/p/proj"));
        let mut app = build_sessions_app(snapshot);
        app.update(Msg::NavDown);
        assert_eq!(selected_default_action(&app), SelectedDefault::View);
    }
}

#[test]
fn translate_ignores_unbound_keys() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('z')))),
        None
    );
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::CONTROL, KeyCode::Char('a')))),
        None
    );
}

#[test]
fn translate_ignores_release_kind_keys() {
    let mut key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    key.kind = KeyEventKind::Release;
    assert_eq!(translate(Event::Key(key)), None);
}

/// ADR 0085 contract 4: view / grouping / filter /
/// sort changes must not trigger discovery. The signal we lean
/// on is `SnapshotHandle` identity — `refresh` builds a fresh
/// `SnapshotHandle::new(...)` (a distinct `Rc`), so if the projection
/// path had run discovery the post-action `snapshot_handle()` would
/// point at a different allocation than the pre-action one.
mod projection_zero_discovery {
    use super::*;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, GraphNode, GraphSnapshot,
        RepoId, RepoNode,
    };
    use crate::resolve::resolve_snapshot;
    use crate::tui::app::SnapshotHandle;
    use crate::tui::{Grouping, MuxGrouping, SessionsGrouping, View};

    fn seeded_app() -> App {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new("/p/proj"))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new("/p/proj"), "/p/proj".to_string()),
            root: "/p/proj".to_string(),
            git_dir: None,
            current_branch: None,
            worktree: None,
            head_commit: None,
        }));
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("codex", "/state", "abc"),
                "codex".to_string(),
            )
            .with_cwd("/p/proj".to_string()),
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs {
            snapshot: &snapshot,
            view: View::Sessions,
            grouping: Grouping::Sessions(SessionsGrouping::Graph),
            filter: RowFilter::default(),
            sort: super::super::super::Sort::Hierarchy,
            mux_recency: crate::tui::MuxRecency::default(),
            cwd: None,
        });
        let mut cfg = RunConfig::defaults();
        cfg.default_view = View::Sessions;
        let mut app = App::new(cfg);
        app.update(Msg::SetData {
            snapshot: SnapshotHandle::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app
    }

    fn same_snapshot(a: &App, b_ptr: *const crate::model::GraphSnapshot) -> bool {
        std::ptr::eq(
            a.snapshot_handle().expect("snapshot loaded").snapshot() as *const _,
            b_ptr,
        )
    }

    #[test]
    fn view_switch_does_not_run_discovery() {
        let mut app = seeded_app();
        let before = app.snapshot_handle().expect("snapshot loaded").snapshot() as *const _;
        let _ = app.update(Msg::SwitchView(View::Mux));
        assert_eq!(app.active_view(), View::Mux);
        assert!(
            same_snapshot(&app, before),
            "view switch must re-derive from the held snapshot, not re-run discovery",
        );
    }

    #[test]
    fn grouping_change_does_not_run_discovery() {
        let mut app = seeded_app();
        let before = app.snapshot_handle().expect("snapshot loaded").snapshot() as *const _;
        let _ = app.update(Msg::SetGrouping(Grouping::Sessions(
            SessionsGrouping::Workspace,
        )));
        assert_eq!(
            app.grouping(),
            Grouping::Sessions(SessionsGrouping::Workspace)
        );
        assert!(
            same_snapshot(&app, before),
            "grouping change must re-derive from the held snapshot, not re-run discovery",
        );
    }

    #[test]
    fn filter_change_does_not_run_discovery() {
        let mut app = seeded_app();
        let before = app.snapshot_handle().expect("snapshot loaded").snapshot() as *const _;
        let filter = RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
            ..RowFilter::default()
        };
        let _ = app.update(Msg::SetFilter(filter.clone()));
        assert_eq!(app.filter(), &filter);
        assert!(
            same_snapshot(&app, before),
            "filter change must re-derive from the held snapshot, not re-run discovery",
        );
    }

    #[test]
    fn view_switch_leaves_run_config_untouched() {
        // ADR 0085 contract 4: RunConfig is initial-values-only.
        // The projection change must not write back into config.
        let mut app = seeded_app();
        let before_view = app.config().default_view;
        let before_grouping = app.config().sessions_grouping;
        let before_mux_grouping = app.config().mux_grouping;
        let _ = app.update(Msg::SwitchView(View::Mux));
        let _ = app.update(Msg::SetGrouping(Grouping::Mux(MuxGrouping::Host)));
        assert_eq!(app.config().default_view, before_view);
        assert_eq!(app.config().sessions_grouping, before_grouping);
        assert_eq!(app.config().mux_grouping, before_mux_grouping);
    }
}

/// ADR 0085 contract 2: mux ops flow through
/// the executor. The reducer emits `Effect::RunMux(...)` and the
/// executor is the only code holding a `MuxBackend` reference.
mod mux_effect_executor {
    use super::*;
    use crate::discovery::tmux::{FakeTmux, TmuxCaptureOutcome};
    use crate::model::MuxSessionId;
    use crate::tui::app::SnapshotHandle;
    use crate::tui::effect::MuxOp;
    use crate::tui::preview::PreviewContent;

    #[test]
    fn capture_preview_effect_lands_in_app_preview_store() {
        let mut app = App::new(RunConfig::defaults());
        // No snapshot needed — the executor branch just calls
        // capture_via on the runner and dispatches
        // Msg::SetMuxPreview, so App only needs to be alive.
        let _ = SnapshotHandle::from_snapshot(&crate::model::GraphSnapshot::empty());
        let mux = MuxSessionId::new("tmux:editor");
        let tmux = FakeTmux::with_sessions("").with_capture(
            "editor",
            TmuxCaptureOutcome::captured("hello from pane".to_string()),
        );

        execute_mux_op(
            &mut app,
            &tmux,
            MuxOp::CapturePreview {
                mux: mux.clone(),
                native_id: "editor".to_string(),
            },
        );

        let entry = app.mux_preview(&mux).expect("preview cached");
        assert!(
            matches!(entry.content, PreviewContent::Text(ref capture) if capture.text.contains("hello from pane"))
        );
    }

    #[test]
    fn plan_preview_load_returns_none_with_no_selection() {
        let app = App::new(RunConfig::defaults());
        // Empty app: no selection, so no source to load regardless of
        // what was shown or whether a refresh was asked for.
        assert!(plan_preview_load(&app, app.config(), None, true).is_none());
    }
}

/// `overlay_key_from_event` is the one place a key press reaches the
/// modal stack: an open modal takes every key press, whatever its kind,
/// and the fallback returns None when no modal is open.
mod overlay_routing {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    #[test]
    fn empty_stack_returns_none() {
        let app = App::new(RunConfig::defaults());
        assert!(
            overlay_key_from_event(
                &app,
                &Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('j')))
            )
            .is_none()
        );
    }

    #[test]
    fn help_on_stack_routes_key_press() {
        let mut app = App::new(RunConfig::defaults());
        app.open_help_overlay();
        let action = overlay_key_from_event(
            &app,
            &Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('j'))),
        );
        assert!(matches!(action, Some(Action::ModalKey(_))));
    }

    #[test]
    fn controls_on_stack_routes_key_press() {
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        let action =
            overlay_key_from_event(&app, &Event::Key(chord(KeyModifiers::NONE, KeyCode::Enter)));
        assert!(matches!(action, Some(Action::ModalKey(_))));
    }

    #[test]
    fn help_on_top_of_controls_routes_to_help() {
        // Multi-overlay: the top variant wins, matching the
        // modal stack's LIFO input-owner contract.
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        app.open_help_overlay();
        let action = overlay_key_from_event(
            &app,
            &Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('j'))),
        );
        assert!(matches!(action, Some(Action::ModalKey(_))));
    }

    #[test]
    fn non_key_events_return_none_even_when_modal_is_open() {
        let mut app = App::new(RunConfig::defaults());
        app.open_help_overlay();
        assert!(overlay_key_from_event(&app, &Event::FocusGained).is_none());
    }

    #[test]
    fn key_release_events_return_none_even_when_modal_is_open() {
        let mut app = App::new(RunConfig::defaults());
        app.open_help_overlay();
        let mut key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        assert!(overlay_key_from_event(&app, &Event::Key(key)).is_none());
    }
}

mod mux_launch_argv {
    //! The argv the TUI hands `conspectus mux new` / `mux launch`.

    use super::*;

    #[test]
    fn mux_new_argv_carries_name_cwd_and_no_attach() {
        // ADR 0095: the TUI re-execs into
        // `conspectus mux new` with the operator-supplied fields;
        // `--no-attach` is always present so the TUI's own attach
        // path (not the subprocess) hands the terminal off.
        let argv = mux_new_argv("dev", "/home/op/proj");
        assert_eq!(
            argv,
            vec![
                "mux".to_string(),
                "new".to_string(),
                "dev".to_string(),
                "--cwd".to_string(),
                "/home/op/proj".to_string(),
                "--no-attach".to_string(),
            ]
        );
    }

    fn request(
        pin: crate::tui::widgets::mux_launch::PinChoice,
    ) -> crate::tui::widgets::mux_launch::MuxLaunchRequest {
        crate::tui::widgets::mux_launch::MuxLaunchRequest {
            harness: "codex".into(),
            name: "w".into(),
            cwd: "/src".into(),
            mux_socket: None,
            launch_argv: vec!["codex".into(), "--x".into()],
            worktree_branch: None,
            worktree_repo: None,
            pin,
        }
    }

    #[test]
    fn launch_argv_carries_the_pin_choice_before_the_argv() {
        use crate::tui::widgets::mux_launch::PinChoice;
        let off = mux_launch_argv(&request(PinChoice::Off));
        assert!(!off.contains(&"--pin".to_string()), "{off:?}");

        let pin = mux_launch_argv(&request(PinChoice::Pin));
        assert!(pin.contains(&"--pin".to_string()), "{pin:?}");
        assert!(!pin.contains(&"--no-launch".to_string()), "{pin:?}");

        let plan = mux_launch_argv(&request(PinChoice::PlanOnly));
        let pin_at = plan.iter().position(|a| a == "--pin").unwrap();
        assert_eq!(plan[pin_at + 1], "--no-launch");
        let argv_at = plan.iter().position(|a| a == "--argv").unwrap();
        assert!(pin_at < argv_at, "flags precede the argv tokens: {plan:?}");
    }

    #[test]
    fn a_launch_with_hyphen_tokens_parses_with_the_real_cli() {
        use crate::tui::widgets::mux_launch::PinChoice;
        use clap::Parser;
        for (harness, flag) in [
            ("claude-code", "--dangerously-skip-permissions"),
            ("codex", "--dangerously-bypass-approvals-and-sandbox"),
        ] {
            let mut req = request(PinChoice::PlanOnly);
            req.harness = harness.into();
            req.launch_argv = vec!["tool".into(), flag.into(), "-p".into(), "two words".into()];
            let argv = mux_launch_argv(&req);
            assert_eq!(
                &argv[argv.len() - 4..],
                ["tool", flag, "-p", "two words"],
                "the tokens come last, untouched"
            );
            let parsed = crate::cli::Cli::try_parse_from(
                std::iter::once("conspectus".to_string()).chain(argv),
            );
            assert!(parsed.is_ok(), "{harness}: {:?}", parsed.err());
        }
    }
}

#[test]
fn translate_bang_opens_messages() {
    assert_eq!(
        translate(Event::Key(chord(KeyModifiers::SHIFT, KeyCode::Char('!')))),
        Some(Action::OpenMessages)
    );
}

#[test]
fn launch_entry_level_follows_outcome_and_stderr() {
    use crate::tui::messages::{CommandRecord, LogTarget};
    use tuikit::feedback::Severity;
    let summary = |success| LaunchSummary {
        success,
        message: "pin `w1` launched".to_string(),
    };
    let record = |stderr: &str| CommandRecord {
        argv: vec!["conspectus".into()],
        exit_code: Some(0),
        stdout: String::new(),
        stderr: stderr.to_string(),
    };
    let target = || LogTarget::Pin("w1".to_string());

    assert_eq!(
        launch_entry(&summary(true), record(""), target()).severity,
        Severity::Success
    );
    assert_eq!(
        launch_entry(
            &summary(true),
            record("resume failed; launched fresh"),
            target()
        )
        .severity,
        Severity::Warning
    );
    let failed = launch_entry(&summary(false), record("boom"), target());
    assert_eq!(failed.severity, Severity::Error);
    assert_eq!(
        failed.item.command.map(|c| c.stderr),
        Some("boom".to_string())
    );
}

mod worker_ledger {
    use super::*;

    const APPLY: WorkerVerdict = WorkerVerdict {
        apply: true,
        settles_handoff: false,
    };
    const SETTLE: WorkerVerdict = WorkerVerdict {
        apply: true,
        settles_handoff: true,
    };
    const DROP: WorkerVerdict = WorkerVerdict {
        apply: false,
        settles_handoff: false,
    };

    #[test]
    fn older_result_landing_after_a_newer_one_is_dropped() {
        let mut ledger = WorkerLedger::default();
        let timer = ledger.start();
        let handoff = ledger.start_for_handoff();
        assert_eq!(ledger.finish(handoff), SETTLE);
        assert!(!ledger.idle());
        assert_eq!(ledger.finish(timer), DROP);
        assert!(ledger.idle());
    }

    #[test]
    fn worker_spawned_before_the_handoff_does_not_settle_it() {
        let mut ledger = WorkerLedger::default();
        let timer = ledger.start();
        let handoff = ledger.start_for_handoff();
        assert_eq!(ledger.finish(timer), APPLY);
        assert_eq!(ledger.finish(handoff), SETTLE);
    }

    #[test]
    fn a_later_handoff_moves_the_settle_point() {
        let mut ledger = WorkerLedger::default();
        let first = ledger.start_for_handoff();
        let second = ledger.start_for_handoff();
        assert_eq!(ledger.finish(first), APPLY);
        assert_eq!(ledger.finish(second), SETTLE);
    }

    #[test]
    fn worker_spawned_after_the_handoff_settles_it() {
        let mut ledger = WorkerLedger::default();
        let handoff = ledger.start_for_handoff();
        let refresh = ledger.start();
        assert_eq!(ledger.finish(refresh), SETTLE);
        assert_eq!(ledger.finish(handoff), DROP);
        assert!(ledger.idle());
    }
}

/// ADR 0112 §5: when the selection's preview loads.
mod preview_loading {
    use super::*;
    use crate::discovery::tmux::FakeTmux;
    use crate::model::{AgentSessionId, AgentSessionNode, GraphNode, GraphSnapshot};
    use crate::resolve::resolve_snapshot;
    use crate::tui::app::SnapshotHandle;
    use crate::tui::preview::{PreviewContent, PreviewSource};
    use crate::tui::rows::RowKind;

    /// An un-muxed codex session whose state root doesn't exist,
    /// selected.
    fn session_app(config: RunConfig) -> App {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("codex", "/nonexistent/.codex", "abc"),
                "codex".to_string(),
            )
            .with_cwd("/p/proj".to_string()),
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs {
            snapshot: &snapshot,
            view: View::Sessions,
            grouping: crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::Graph),
            filter: crate::filter::RowFilter::default(),
            sort: crate::tui::Sort::Hierarchy,
            mux_recency: crate::tui::MuxRecency::default(),
            cwd: None,
        });
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: SnapshotHandle::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let session = app
            .tree()
            .rows
            .iter()
            .find(|row| matches!(row.kind, RowKind::AgentSession(_)))
            .map(|row| row.id.clone())
            .expect("session row");
        app.set_selection(session);
        app
    }

    fn transcript() -> PreviewSource {
        PreviewSource::Transcript(AgentSessionId::new("codex", "/nonexistent/.codex", "abc"))
    }

    fn loaded(app: &mut App) {
        app.update(Msg::SetPreview {
            source: transcript(),
            content: PreviewContent::Unavailable("gone".to_string()),
        });
    }

    /// The PRs view with PR `#7` selected.
    fn pr_app(config: RunConfig) -> App {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::ForgePr(crate::model::ForgePrNode {
                id: crate::model::ForgePrId::new("github", "github.com", "o", "r", 7),
                provider: "github".to_string(),
                host: "github.com".to_string(),
                owner: "o".to_string(),
                repo: "r".to_string(),
                number: 7,
                state: Some("OPEN".to_string()),
                title: None,
                url: None,
                updated_epoch: None,
                is_draft: false,
                head_ref: Some("feature".to_string()),
            }));
        let snapshot = resolve_snapshot(snapshot);
        let mut config = config;
        config.default_view = View::Prs;
        let mut app = App::new(config);
        let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
            &snapshot, &app,
        ));
        app.update(Msg::SetData {
            snapshot: SnapshotHandle::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let row = app
            .tree()
            .rows
            .iter()
            .find(|row| matches!(row.kind, RowKind::Pr(_)))
            .map(|row| row.id.clone())
            .expect("PR row");
        app.set_selection(row);
        app
    }

    fn pr() -> PreviewSource {
        PreviewSource::PullRequest(crate::tui::preview::PrQuery {
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            number: 7,
        })
    }

    fn fetched(app: &mut App) {
        app.update(Msg::SetPreview {
            source: pr(),
            content: PreviewContent::Lines(vec!["Title".into()]),
        });
    }

    #[test]
    fn a_fetched_pr_is_reused_across_reselection_and_refresh() {
        let mut app = pr_app(RunConfig::defaults());
        let config = app.config().clone();
        assert_eq!(plan_preview_load(&app, &config, None, false), Some(pr()));
        fetched(&mut app);
        let elsewhere = transcript();
        assert_eq!(
            plan_preview_load(&app, &config, Some(&elsewhere), false),
            None
        );
        assert_eq!(plan_preview_load(&app, &config, Some(&pr()), true), None);
    }

    #[test]
    fn a_hard_refresh_or_the_forge_interval_reloads_a_pr() {
        let mut app = pr_app(RunConfig::defaults());
        fetched(&mut app);
        app.update(Msg::ForgeRefreshRequested);
        let config = app.config().clone();
        assert_eq!(
            plan_preview_load(&app, &config, Some(&pr()), false),
            Some(pr())
        );

        let mut config = RunConfig::defaults();
        config.intervals.forge = Duration::ZERO;
        let mut app = pr_app(config.clone());
        fetched(&mut app);
        assert_eq!(
            plan_preview_load(&app, &config, Some(&transcript()), false),
            Some(pr())
        );
    }

    #[test]
    fn a_pr_that_failed_to_load_retries_when_selected_again() {
        let mut app = pr_app(RunConfig::defaults());
        app.update(Msg::SetPreview {
            source: pr(),
            content: PreviewContent::Unavailable("offline".to_string()),
        });
        let config = app.config().clone();
        assert_eq!(plan_preview_load(&app, &config, Some(&pr()), false), None);
        assert_eq!(
            plan_preview_load(&app, &config, Some(&transcript()), false),
            Some(pr())
        );
    }

    #[test]
    fn a_transcript_not_yet_loaded_loads() {
        let app = session_app(RunConfig::defaults());
        let config = app.config().clone();
        assert_eq!(
            plan_preview_load(&app, &config, None, false),
            Some(transcript())
        );
    }

    #[test]
    fn a_loaded_transcript_still_on_screen_does_not_reload() {
        let mut app = session_app(RunConfig::defaults());
        loaded(&mut app);
        let config = app.config().clone();
        assert_eq!(
            plan_preview_load(&app, &config, Some(&transcript()), false),
            None
        );
    }

    #[test]
    fn reselecting_a_session_rereads_its_transcript() {
        let mut app = session_app(RunConfig::defaults());
        loaded(&mut app);
        let config = app.config().clone();
        let elsewhere = PreviewSource::Transcript(AgentSessionId::new("codex", "/s", "other"));
        assert_eq!(
            plan_preview_load(&app, &config, Some(&elsewhere), false),
            Some(transcript())
        );
    }

    #[test]
    fn refresh_rereads_the_transcript_on_screen() {
        let mut app = session_app(RunConfig::defaults());
        loaded(&mut app);
        let config = app.config().clone();
        assert_eq!(
            plan_preview_load(&app, &config, Some(&transcript()), true),
            Some(transcript())
        );
    }

    #[test]
    fn a_load_in_flight_is_not_started_again() {
        let mut app = session_app(RunConfig::defaults());
        app.update(Msg::PreviewLoadStarted(transcript()));
        let config = app.config().clone();
        assert_eq!(plan_preview_load(&app, &config, None, true), None);
    }

    #[test]
    fn no_live_preview_loads_nothing() {
        let mut config = RunConfig::defaults();
        config.live_preview_enabled = false;
        let app = session_app(config.clone());
        assert_eq!(plan_preview_load(&app, &config, None, true), None);
    }

    #[test]
    fn loading_a_missing_transcript_stores_why() {
        let mut app = session_app(RunConfig::defaults());
        app.update(Msg::PreviewLoadStarted(transcript()));
        load_preview_now(&mut app, &FakeTmux::with_sessions(""), transcript());
        assert!(!app.previews().is_loading(&transcript()));
        let entry = app.previews().get(&transcript()).expect("stored");
        assert_eq!(
            entry.content,
            PreviewContent::Unavailable("transcript file not found".to_string())
        );
    }
}

/// ADR 0115: forge discovery's failures reach the header's forge chip.
mod forge_status {
    use super::*;
    use crate::tui::app::SnapshotHandle;

    #[test]
    fn a_forge_failure_diagnostic_sets_the_forge_chip_reason() {
        let mut snapshot = crate::model::GraphSnapshot::empty();
        snapshot
            .diagnostics
            .push(crate::model::Diagnostic::ProviderUnavailable {
                provider: "github".to_string(),
                reason: "gh is not authenticated".to_string(),
            });
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: SnapshotHandle::from_snapshot(&snapshot),
            tree: crate::tui::rows::RowTree::default(),
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let config = app.config().clone();
        populate_provider_status(&mut app, &config);
        assert_eq!(app.provider_status().forge_available, Some(false));
        assert_eq!(
            app.provider_status().forge_reason.as_deref(),
            Some("gh is not authenticated")
        );
    }

    #[test]
    fn a_clean_forge_run_leaves_the_forge_chip_off() {
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::SetData {
            snapshot: SnapshotHandle::from_snapshot(&crate::model::GraphSnapshot::empty()),
            tree: crate::tui::rows::RowTree::default(),
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        let config = app.config().clone();
        populate_provider_status(&mut app, &config);
        assert_eq!(app.provider_status().forge_available, Some(true));
    }
}

#[test]
fn attach_routes_each_backend_to_its_own_runner() {
    assert!(matches!(attach_route("tmux"), AttachRoute::Tmux));
    match attach_route("zellij") {
        AttachRoute::Backend(runner) => assert_eq!(runner.backend_key(), "zellij"),
        _ => panic!("a zellij target must not attach through tmux"),
    }
    assert!(matches!(attach_route("screen"), AttachRoute::Unknown));
}

#[test]
fn attach_outcome_names_the_backend_that_failed() {
    use crate::discovery::tmux::TmuxAttachOutcome;
    match attach_outcome("zellij", Ok(TmuxAttachOutcome::Unsupported)) {
        AttachOutcome::Failed(message) => assert!(message.contains("zellij"), "{message}"),
        AttachOutcome::Detached => panic!("unsupported attach is a failure"),
    }
    assert!(matches!(
        attach_outcome("zellij", Ok(TmuxAttachOutcome::Detached)),
        AttachOutcome::Detached
    ));
}

#[test]
fn attach_return_entry_checks_pane_status_through_the_targets_backend() {
    let target = AttachTarget {
        mux: MuxSessionId::new("zellij:zp"),
        backend: "zellij".to_string(),
        native_id: "zp".to_string(),
    };
    let entry = attach_return_entry(
        "pin `zp`",
        AttachOutcome::Detached,
        crate::tui::messages::LogTarget::Mux(target.mux.clone()),
        &target,
        None,
    );
    assert!(
        entry.summary.contains("attached/detached"),
        "zellij reports no pane status, so the entry is a plain detach: {}",
        entry.summary
    );
}

fn native_rename(backend: &str, target: &str) -> crate::rename::MuxNativeRename {
    crate::rename::MuxNativeRename {
        mux: MuxSessionId::new(format!("{backend}:{target}")),
        backend: backend.to_string(),
        target: target.to_string(),
        socket: None,
        new_name: "renamed".to_string(),
    }
}

#[test]
fn tui_mux_rename_passes_the_bare_name_to_tmux() {
    let tmux = crate::discovery::tmux::FakeTmux::with_sessions("");
    rename_mux_via_backend(&tmux, &native_rename("tmux", "editor")).expect("renamed");
    assert_eq!(
        tmux.rename_calls(),
        vec![(None, "editor".to_string(), "renamed".to_string())]
    );
}

#[test]
fn tui_mux_rename_never_sends_a_zellij_rename_to_tmux() {
    let tmux = crate::discovery::tmux::FakeTmux::with_sessions("");
    let err = rename_mux_via_backend(&tmux, &native_rename("zellij", "notes"))
        .expect_err("zellij cannot rename sessions");
    assert!(err.contains("zellij"), "{err}");
    assert!(tmux.rename_calls().is_empty());
}

#[test]
fn spinner_chip_shows_while_work_runs_and_clears_after() {
    let mut app = App::new(RunConfig::defaults());
    let (release, gate) = std::sync::mpsc::channel::<()>();
    let mut frames = 0;
    let mut chips_during_draw = None;

    let result = run_with_spinner(
        &mut app,
        "Launching demo".to_string(),
        |app| {
            frames += 1;
            chips_during_draw = Some(
                app.in_flight_ops()
                    .iter()
                    .map(|op| (op.kind.clone(), op.label.clone()))
                    .collect::<Vec<_>>(),
            );
            if frames == 3 {
                release.send(()).unwrap();
            }
        },
        move || {
            gate.recv().unwrap();
            42
        },
    );

    assert_eq!(result, 42);
    assert!(
        frames >= 3,
        "kept redrawing until the work finished: {frames}"
    );
    assert_eq!(
        chips_during_draw,
        Some(vec![(
            crate::tui::app::InFlightKind::Launch,
            "Launching demo".to_string()
        )])
    );
    assert!(app.in_flight_ops().is_empty(), "chip clears when done");
}

#[test]
fn spinner_draws_at_least_one_frame_for_instant_work() {
    let mut app = App::new(RunConfig::defaults());
    let mut frames = 0;
    let result = run_with_spinner(&mut app, "x".to_string(), |_| frames += 1, || "done");
    assert_eq!(result, "done");
    assert!(frames >= 1);
    assert!(app.in_flight_ops().is_empty());
}

#[test]
fn spinner_worker_panic_propagates_to_the_caller() {
    let result = std::panic::catch_unwind(|| {
        let mut app = App::new(RunConfig::defaults());
        run_with_spinner(
            &mut app,
            "x".to_string(),
            |_| {},
            || -> () { panic!("boom") },
        );
    });
    assert!(result.is_err());
}

mod handoff_guard {
    use super::*;

    #[test]
    fn row_mutating_actions_are_guarded_and_read_only_ones_are_not() {
        for action in [
            Action::OpenRename,
            Action::OpenWorktreeMenu,
            Action::OpenWorktreeCloseDown,
        ] {
            assert!(acts_on_selected_row(&action), "{action:?}");
        }
        for action in [
            Action::Refresh,
            Action::View,
            Action::CopySessionId,
            Action::OpenControls,
            Action::OpenSearch,
            Action::DefaultAction,
            Action::Attach,
        ] {
            assert!(!acts_on_selected_row(&action), "{action:?}");
        }
    }
}

mod lazy_forge {
    use super::*;
    use crate::discovery::forge::ForgeFetch;

    #[test]
    fn only_a_hard_refresh_lets_a_live_refresh_fetch() {
        for nudge in [DaemonNudge::None, DaemonNudge::LiveClasses] {
            assert_eq!(
                worker_forge_fetch(nudge, ForgeFetch::CachedOnly),
                ForgeFetch::CachedOnly,
                "{nudge:?}"
            );
        }
        assert_eq!(
            worker_forge_fetch(DaemonNudge::LiveAndForge, ForgeFetch::CachedOnly),
            ForgeFetch::Fetch
        );
    }

    #[test]
    fn the_background_fetch_runs_only_without_a_daemon_one_at_a_time() {
        let now = Instant::now();
        let mut worker = ForgeWorker::new();
        assert!(!worker.due(now), "a daemon fetches for a daemon-backed TUI");

        worker.daemonless = true;
        assert!(worker.due(now));

        worker.running = true;
        assert!(!worker.due(now), "one fetch at a time");

        worker.running = false;
        worker.last_started = Some(now);
        assert!(!worker.due(now));
        assert!(worker.due(now + FORGE_POLL));
    }
}

#[test]
fn keys_of_the_retired_pin_surfaces_are_unbound() {
    // The pins overlay and its direct shortcuts left with the old pin
    // model; pins are slots managed from the row menu.
    for (code, mods) in [
        (KeyCode::Char('B'), KeyModifiers::SHIFT),
        (KeyCode::Char('L'), KeyModifiers::SHIFT),
        (KeyCode::Char('A'), KeyModifiers::SHIFT),
        (KeyCode::Char('b'), KeyModifiers::NONE),
        (KeyCode::Delete, KeyModifiers::NONE),
    ] {
        assert_eq!(translate(Event::Key(chord(mods, code))), None, "{code:?}");
    }
}

#[test]
fn p_pins_and_capital_n_launches_with_a_worktree() {
    assert!(matches!(
        translate(Event::Key(chord(KeyModifiers::NONE, KeyCode::Char('p')))),
        Some(Action::Msg(msg)) if *msg == Msg::TogglePin
    ));
    assert!(matches!(
        translate(Event::Key(chord(KeyModifiers::SHIFT, KeyCode::Char('N')))),
        Some(Action::Msg(msg)) if *msg == Msg::OpenMuxLaunchForm
    ));
}

#[test]
fn saving_the_pin_edit_form_sets_and_drops_overrides_in_the_store() {
    use crate::model::{FieldValue, PinKind, SlotFields};
    use crate::tui::widgets::pin_edit::PinEditRequest;
    let dir = tempfile::tempdir().unwrap();
    let store = crate::pin_slots::SlotStore::at(dir.path().join("pins.toml"));
    crate::pin_ops::pin_planned(
        &store,
        "work",
        PinKind::Mux,
        SlotFields::from([("cwd".to_string(), FieldValue::Text("/old".into()))]),
    )
    .unwrap();

    let request = PinEditRequest {
        pin: "work".into(),
        rename: None,
        set: vec![
            ("socket".to_string(), FieldValue::Text("s1".into())),
            (
                "argv".to_string(),
                FieldValue::List(vec!["claude".into(), "--x".into()]),
            ),
        ],
        unset: vec!["cwd".to_string()],
    };
    assert!(executor::apply_pin_overrides(&store, &request).unwrap());

    let slot = store.load().slots.remove(0);
    assert_eq!(
        slot.overrides.get("socket"),
        Some(&FieldValue::Text("s1".into()))
    );
    assert_eq!(
        slot.overrides.get("argv"),
        Some(&FieldValue::List(vec!["claude".into(), "--x".into()]))
    );
    assert!(
        !slot.overrides.contains_key("cwd"),
        "the override was dropped"
    );

    // Saving the same thing again changes nothing; an empty request is a no-op.
    assert!(!executor::apply_pin_overrides(&store, &request).unwrap());
    let none = PinEditRequest {
        set: Vec::new(),
        unset: Vec::new(),
        ..request
    };
    assert!(!executor::apply_pin_overrides(&store, &none).unwrap());
}

mod settle {
    use std::cell::RefCell;
    use std::collections::BTreeSet;

    use super::*;
    use crate::model::{GraphNode, GraphSnapshot, MuxSessionId, MuxSessionNode, NodeId};

    fn world(names: &[&str]) -> GraphSnapshot {
        GraphSnapshot {
            nodes: names
                .iter()
                .map(|name| {
                    GraphNode::MuxSession(MuxSessionNode::new(
                        MuxSessionId::new(format!("tmux:{name}")),
                        "tmux",
                        *name,
                    ))
                })
                .collect(),
            ..GraphSnapshot::empty()
        }
    }

    fn ids(names: &[&str]) -> BTreeSet<NodeId> {
        live_mux_ids(&world(names))
    }

    /// Probe results in order; the last repeats.
    fn run(
        before: Option<&BTreeSet<NodeId>>,
        results: &[&[&str]],
    ) -> (usize, usize, GraphSnapshot) {
        let calls = RefCell::new(0usize);
        let sleeps = RefCell::new(0usize);
        let (snapshot, ()) = probe_until_changed(
            before,
            2,
            Duration::from_millis(150),
            |_| *sleeps.borrow_mut() += 1,
            || {
                let i = (*calls.borrow()).min(results.len() - 1);
                *calls.borrow_mut() += 1;
                (Ok(world(results[i])), ())
            },
        );
        (calls.into_inner(), sleeps.into_inner(), snapshot.unwrap())
    }

    #[test]
    fn a_changed_mux_set_is_taken_at_once() {
        let before = ids(&["a"]);
        let (calls, sleeps, snapshot) = run(Some(&before), &[&["a", "b"]]);
        assert_eq!((calls, sleeps), (1, 0));
        assert_eq!(live_mux_ids(&snapshot), ids(&["a", "b"]));
    }

    #[test]
    fn an_unchanged_set_is_probed_again_until_it_changes() {
        let before = ids(&["a", "b"]);
        // The first probe still sees the session that just ended.
        let (calls, sleeps, snapshot) = run(Some(&before), &[&["a", "b"], &["a"]]);
        assert_eq!((calls, sleeps), (2, 1));
        assert_eq!(live_mux_ids(&snapshot), ids(&["a"]));
    }

    #[test]
    fn a_set_that_never_changes_stops_after_the_bounded_retries() {
        let before = ids(&["a"]);
        let (calls, sleeps, _) = run(Some(&before), &[&["a"]]);
        assert_eq!((calls, sleeps), (3, 2));
    }

    #[test]
    fn without_a_hand_off_set_it_probes_once() {
        let (calls, sleeps, _) = run(None, &[&["a"]]);
        assert_eq!((calls, sleeps), (1, 0));
    }

    #[test]
    fn a_failed_probe_is_returned_without_retrying() {
        let before = ids(&["a"]);
        let calls = RefCell::new(0);
        let (result, ()) = probe_until_changed(
            Some(&before),
            2,
            Duration::ZERO,
            |_| {},
            || {
                *calls.borrow_mut() += 1;
                (Err(anyhow::anyhow!("boom")), ())
            },
        );
        assert!(result.is_err());
        assert_eq!(calls.into_inner(), 1);
    }
}

mod viewer_find_prompt {
    use super::*;
    use crate::viewer::model::{TranscriptDocument, TranscriptTurn, TurnKind, TurnRole};

    fn app_with_viewer() -> App {
        let mut document = TranscriptDocument::default();
        for body in ["alpha beta", "gamma needle", "delta"] {
            document.turns.push(TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: body.to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            });
        }
        let mut app = App::new(RunConfig::defaults());
        app.open_viewer_modal(crate::tui::viewer_bridge::viewer(document, None));
        app
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            modal_key_pure(app, chord(KeyModifiers::NONE, KeyCode::Char(c)));
        }
    }

    #[test]
    fn slash_opens_the_prompt_and_every_key_types_into_it() {
        let mut app = app_with_viewer();
        modal_key_pure(&mut app, chord(KeyModifiers::NONE, KeyCode::Char('/')));
        // `n`, `q`, and Space are viewer keys, but the prompt owns them now.
        type_text(&mut app, "n q");
        let prompt = app.viewer_modal().and_then(|s| s.find().prompt());
        assert_eq!(prompt.map(tuikit::text_edit::LineEdit::value), Some("n q"));
        assert!(app.viewer_modal().is_some(), "q did not close the viewer");
    }

    #[test]
    fn enter_runs_the_find_and_n_steps_afterwards() {
        let mut app = app_with_viewer();
        modal_key_pure(&mut app, chord(KeyModifiers::NONE, KeyCode::Char('/')));
        type_text(&mut app, "a");
        modal_key_pure(&mut app, chord(KeyModifiers::NONE, KeyCode::Enter));
        let state = app.viewer_modal().expect("viewer");
        assert_eq!(state.find().matches(), [0, 1, 2]);
        assert!(state.find().prompt().is_none());
        modal_key_pure(&mut app, chord(KeyModifiers::NONE, KeyCode::Char('n')));
        assert_eq!(app.viewer_modal().and_then(|s| s.find().current()), Some(1));
        modal_key_pure(&mut app, chord(KeyModifiers::SHIFT, KeyCode::Char('N')));
        assert_eq!(app.viewer_modal().and_then(|s| s.find().current()), Some(0));
    }

    #[test]
    fn escape_leaves_the_prompt_without_closing_the_viewer() {
        let mut app = app_with_viewer();
        modal_key_pure(&mut app, chord(KeyModifiers::NONE, KeyCode::Char('/')));
        type_text(&mut app, "x");
        modal_key_pure(&mut app, chord(KeyModifiers::NONE, KeyCode::Esc));
        assert!(
            app.viewer_modal()
                .is_some_and(|s| s.find().prompt().is_none())
        );
        modal_key_pure(&mut app, chord(KeyModifiers::NONE, KeyCode::Esc));
        assert!(app.viewer_modal().is_none(), "a second Esc closes it");
    }

    #[test]
    fn control_chords_do_not_type() {
        let mut app = app_with_viewer();
        modal_key_pure(&mut app, chord(KeyModifiers::NONE, KeyCode::Char('/')));
        modal_key_pure(&mut app, chord(KeyModifiers::CONTROL, KeyCode::Char('a')));
        let prompt = app.viewer_modal().and_then(|s| s.find().prompt());
        assert_eq!(prompt.map(tuikit::text_edit::LineEdit::value), Some(""));
    }
}
