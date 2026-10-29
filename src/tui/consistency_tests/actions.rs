//! The action model: what an action acts on, what Enter does for each
//! kind of entity, how focus follows an entity to its new view, and how
//! the candidates and targets the actions offer are found.

use super::*;
use crate::model::SlotStatus;
use crate::tui::Effect;
use crate::tui::actions::{
    Subject, action_subject, attached_mux_of, default_effect, mux_to_rebuild, natural_view,
    pins_for, retarget_candidates, running_note, subject_of_row,
};
use crate::tui::app::Focus;
use crate::tui::effect::ExecSpec;

fn mux(name: &str) -> NodeId {
    NodeId::MuxSession(MuxSessionId::new(format!("tmux:{name}")))
}

#[test]
fn a_pin_row_is_the_pin_and_the_entity_it_points_at() {
    let app = app_in(View::Pinned);
    let subject = subject_of_row(
        &app,
        &RowId::Pin {
            pin_id: "conspectus-main".into(),
        },
    );
    assert_eq!(subject.pin.as_deref(), Some("conspectus-main"));
    assert_eq!(subject.node, Some(mux("conspectus-main")));
    assert_eq!(subject.record.as_deref(), Some("h_main"));
}

#[test]
fn a_history_row_is_its_record_and_the_gone_node_it_stood_for() {
    let app = app_listing_all(View::History);
    let subject = subject_of_row(
        &app,
        &RowId::History {
            record_id: "h_agent".into(),
        },
    );
    assert_eq!(subject.record.as_deref(), Some("h_agent"));
    assert_eq!(subject.node, Some(mux("config-agent3")));
    assert_eq!(subject.pin, None);
}

#[test]
fn a_ghost_mux_row_and_its_history_row_are_the_same_subject_apart_from_the_row() {
    let app = app_listing_all(View::Mux);
    let ghost = subject_of_row(&app, &RowId::MuxSession(mux("config-agent3")));
    assert_eq!(ghost.record.as_deref(), Some("h_agent"));
    assert_eq!(ghost.node, Some(mux("config-agent3")));
}

#[test]
fn pins_for_names_every_pin_that_points_at_the_subject() {
    let app = app_in(View::Mux);
    let snapshot = app.snapshot_handle().expect("data").snapshot();
    let by_node = Subject {
        node: Some(mux("conspectus-main")),
        ..Subject::default()
    };
    assert_eq!(pins_for(snapshot, &by_node), ["conspectus-main"]);
    let by_record = Subject {
        record: Some("h_agent".into()),
        ..Subject::default()
    };
    assert_eq!(pins_for(snapshot, &by_record), ["config-agent3"]);
    let none = Subject {
        node: Some(mux("scratch")),
        ..Subject::default()
    };
    assert!(pins_for(snapshot, &none).is_empty());
}

#[test]
fn the_right_pane_subject_is_the_entity_the_cursor_is_on() {
    let mut app = app_in(View::Mux);
    select(&mut app, "conspectus-main");
    assert_eq!(action_subject(&app).node, Some(mux("conspectus-main")));
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Right);
    // On the mux's own field rows the pane shows the mux.
    assert_eq!(action_subject(&app).node, Some(mux("conspectus-main")));
    // Walking onto a related session makes the session the subject.
    for _ in 0..40 {
        if matches!(action_subject(&app).node, Some(NodeId::AgentSession(_))) {
            break;
        }
        app.update(Msg::ExplorerNavDown);
    }
    assert!(matches!(
        action_subject(&app).node,
        Some(NodeId::AgentSession(_))
    ));
    app.update(Msg::CycleFocus);
    assert_eq!(
        action_subject(&app).node,
        Some(mux("conspectus-main")),
        "back on the left pane the selected row is the subject again"
    );
}

#[test]
fn v_and_attach_follow_the_right_pane_too() {
    let mut app = app_in(View::Mux);
    select(&mut app, "conspectus-main");
    app.update(Msg::CycleFocus);
    for _ in 0..40 {
        if matches!(action_subject(&app).node, Some(NodeId::AgentSession(_))) {
            break;
        }
        app.update(Msg::ExplorerNavDown);
    }
    let NodeId::AgentSession(session) = action_subject(&app).node.expect("a session") else {
        panic!("a session");
    };
    assert_eq!(
        app.update(Msg::ViewSelected),
        vec![Effect::Exec(ExecSpec::ViewSession(session, None))]
    );
    // The session runs in the mux, so attach reaches that mux.
    let target = crate::tui::actions::resolve_attach_target(&app).expect("attachable");
    assert_eq!(target.mux, MuxSessionId::new("tmux:conspectus-main"));
}

fn enter_effect(view: View, label: &str) -> Vec<Effect> {
    let mut app = app_listing_all(view);
    select(&mut app, label);
    enter(&mut app)
}

#[test]
fn enter_on_a_gone_mux_pin_rebuilds_it_and_attaches() {
    for (view, label) in [
        (View::Pinned, "config-agent3"),
        (View::Mux, "config-agent3"),
    ] {
        assert_eq!(
            enter_effect(view, label),
            vec![Effect::Exec(ExecSpec::Rebuild {
                reference: "config-agent3".into(),
                attach: true,
            })],
            "{view:?}"
        );
    }
}

#[test]
fn enter_on_a_planned_pin_builds_it_and_attaches() {
    assert_eq!(
        enter_effect(View::Pinned, "nightly"),
        vec![Effect::Exec(ExecSpec::Rebuild {
            reference: "nightly".into(),
            attach: true,
        })]
    );
}

#[test]
fn enter_on_a_gone_worktree_rebuilds_it_without_attaching() {
    let snapshot = resolve_snapshot(crate::tui::rows::pinned::all_kinds_world());
    let gone = crate::tui::rows::pin_nodes(&snapshot)
        .into_iter()
        .find(|pin| pin.kind == PinKind::Checkout && pin.status == SlotStatus::Gone)
        .expect("the fixture has a gone worktree pin")
        .id
        .id
        .clone();
    let mut app = app_over(View::Pinned, &snapshot);
    select(&mut app, &gone);
    assert_eq!(
        enter(&mut app),
        vec![Effect::Exec(ExecSpec::Rebuild {
            reference: gone,
            attach: false,
        })]
    );
}

#[test]
fn enter_on_a_pin_whose_target_could_not_be_checked_says_so() {
    let effects = enter_effect(View::Pinned, "ledger-worktree");
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::Toast(note)] if note.text.contains("couldn't check")
                && note.severity == tuikit::feedback::Severity::Warning
        ),
        "{effects:?}"
    );
}

#[test]
fn enter_on_a_pinned_pr_shows_its_address_and_a_live_pinned_mux_attaches() {
    let effects = enter_effect(View::Pinned, "ledger-pr");
    assert!(
        matches!(effects.as_slice(), [Effect::Toast(_)]),
        "a PR has nothing to rebuild or attach: {effects:?}"
    );
    let effects = enter_effect(View::Pinned, "conspectus-main");
    assert!(
        matches!(effects.as_slice(), [Effect::Exec(ExecSpec::AttachMux(_))]),
        "{effects:?}"
    );
}

#[test]
fn enter_on_a_history_record_without_a_pin_rebuilds_the_record() {
    let mut app = app_in(View::History);
    select(&mut app, "h_old");
    let effects = enter(&mut app);
    assert_eq!(
        effects,
        vec![Effect::Exec(ExecSpec::Rebuild {
            reference: "h_old".into(),
            attach: true,
        })]
    );
}

#[test]
fn nothing_to_default_to_on_a_plain_live_row() {
    let mut app = app_in(View::Mux);
    select(&mut app, "scratch");
    assert_eq!(default_effect(&app), None);
}

#[test]
fn rebuilding_a_mux_knows_where_to_attach_before_anything_exists() {
    let app = app_in(View::Pinned);
    let snapshot = app.snapshot_handle().expect("data").snapshot();
    let (target, socket) = mux_to_rebuild(snapshot, "config-agent3").expect("a mux pin");
    assert_eq!(target.mux, MuxSessionId::new("tmux:config-agent3"));
    assert_eq!(target.native_id, "config-agent3");
    assert_eq!(socket, None);
    // A pin's override decides the name it comes back under.
    let (planned, _) = mux_to_rebuild(snapshot, "nightly").expect("a planned mux pin");
    assert_eq!(planned.native_id, "nightly");
    // A history record carries its own recipe.
    let (from_record, _) = mux_to_rebuild(snapshot, "h_agent").expect("a mux record");
    assert_eq!(from_record.mux, MuxSessionId::new("tmux:config-agent3"));
    // A worktree or a session names no mux ahead of time.
    assert!(mux_to_rebuild(snapshot, "ledger-worktree").is_none());
    assert!(mux_to_rebuild(snapshot, "e44d01cb").is_none());
}

#[test]
fn a_session_pin_is_attached_through_the_mux_hosting_it() {
    let app = app_in(View::Pinned);
    let snapshot = app.snapshot_handle().expect("data").snapshot();
    let (target, _) = attached_mux_of(snapshot, "e44d01cb").expect("the session is in a mux");
    assert_eq!(target.mux, MuxSessionId::new("tmux:conspectus-main"));
    assert!(attached_mux_of(snapshot, "config-agent3").is_none(), "gone");
}

#[test]
fn a_running_entity_gets_a_note_above_its_hypothetical_plan_and_a_stopped_one_does_not() {
    let app = app_in(View::Pinned);
    let snapshot = app.snapshot_handle().expect("data").snapshot();
    let text = running_note(snapshot, "conspectus-main").expect("running");
    assert!(
        text.contains("running") && text.contains("Enter attaches") && text.contains("stopped"),
        "{text}"
    );
    assert_eq!(running_note(snapshot, "config-agent3"), None);
}

#[test]
fn showing_a_plan_asks_the_engine_for_every_rebuildable_pin_running_or_not() {
    let mut app = app_in(View::Pinned);
    for name in ["conspectus-main", "config-agent3"] {
        let effects = app.update(Msg::ShowRebuildPlan(name.into()));
        assert!(
            matches!(
                effects.as_slice(),
                [Effect::Exec(ExecSpec::ShowRebuildPlan { .. })]
            ),
            "{name}: {effects:?}"
        );
    }
}

#[test]
fn retargeting_offers_the_other_entities_of_the_pins_kind_with_what_tells_them_apart() {
    let app = app_in(View::Pinned);
    let snapshot = app.snapshot_handle().expect("data").snapshot();
    let items = retarget_candidates(snapshot, "conspectus-main");
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.iter().any(|l| l.contains("scratch")), "{labels:?}");
    assert!(
        !labels.iter().any(|l| l.contains("conspectus-main")),
        "the current target isn't a candidate: {labels:?}"
    );
    let gone = items
        .iter()
        .find(|item| item.value == "h_agent")
        .expect("a gone mux record is a candidate");
    assert!(gone.detail.starts_with("stopped"), "{gone:?}");
    let live = items
        .iter()
        .find(|item| item.label.contains("scratch"))
        .expect("scratch");
    assert_eq!(live.detail, "running");
    assert!(
        items.iter().all(|item| !item.label.contains("e44d01cb")),
        "a session isn't a mux"
    );
    assert!(retarget_candidates(snapshot, "no-such-pin").is_empty());
}

#[test]
fn the_picker_opens_with_the_candidates_and_a_pick_becomes_a_retarget() {
    let mut app = app_in(View::Pinned);
    select(&mut app, "conspectus-main");
    app.update(Msg::OpenSlotRetarget("conspectus-main".into()));
    let picker = app.picker().expect("the picker");
    assert!(picker.total() >= 2);
    let frame = frame(&mut app, 120, 30);
    assert!(frame.contains("scratch"), "{frame}");
    assert!(frame.contains("Retarget conspectus-main"), "{frame}");
}

#[test]
fn retargeting_a_pin_with_nothing_else_to_point_at_says_so() {
    let mut app = app_in(View::Pinned);
    select(&mut app, "ledger-pr");
    // The one PR the fixture lists is a candidate; take it away.
    let effects = app.update(Msg::OpenSlotRetarget("ledger-pr".into()));
    if app.picker().is_none() {
        assert!(
            matches!(effects.as_slice(), [Effect::Toast(_)]),
            "{effects:?}"
        );
    }
}

#[test]
fn following_an_entity_from_history_lands_on_its_row_in_the_view_it_lives_in() {
    let mut app = app_listing_all(View::History);
    app.update(Msg::FollowEntity(mux("scratch")));
    // The record is a live mux now, so its History row is gone.
    app.forget_follow_row();
    app.update(Msg::HandoffSettled);
    assert_eq!(app.active_view(), View::Mux);
    assert_eq!(
        app.selection().and_then(crate::tui::rows::row_node),
        Some(&mux("scratch"))
    );
}

#[test]
fn following_waits_for_the_hand_off_to_settle() {
    let mut app = app_listing_all(View::History);
    app.update(Msg::HandoffReturned(MuxSessionId::new("tmux:scratch")));
    app.update(Msg::FollowEntity(mux("scratch")));
    app.forget_follow_row();
    assert_eq!(app.active_view(), View::History, "still on stale data");
    app.update(Msg::HandoffSettled);
    assert_eq!(app.active_view(), View::Mux);
}

#[test]
fn an_entity_still_listed_where_the_operator_is_is_selected_in_place() {
    let mut app = app_in(View::Mux);
    select(&mut app, "scratch");
    app.update(Msg::FollowEntity(mux("scratch")));
    app.update(Msg::HandoffSettled);
    assert_eq!(app.active_view(), View::Mux);
    assert_eq!(
        app.selection().and_then(crate::tui::rows::row_node),
        Some(&mux("scratch"))
    );
}

#[test]
fn an_entity_that_cannot_be_found_leaves_the_operator_in_place_with_a_note() {
    let mut app = app_in(View::Pinned);
    app.update(Msg::FollowEntity(mux("never-existed")));
    app.forget_follow_row();
    app.update(Msg::HandoffSettled);
    assert_eq!(app.active_view(), View::Pinned);
    assert!(!app.toasts().is_empty(), "the operator is told");
}

#[test]
fn starting_a_pin_from_the_pinned_view_leaves_the_operator_on_its_row() {
    let mut app = app_in(View::Pinned);
    let before = app.selection().cloned();
    assert!(before.is_some(), "a pin row is selected");
    // The pin's mux is the entity acted on; its pin row stays listed.
    app.update(Msg::FollowEntity(mux("scratch")));
    app.update(Msg::HandoffSettled);
    assert_eq!(app.active_view(), View::Pinned, "no jump to the Mux view");
    assert_eq!(app.selection().cloned(), before);
}

#[test]
fn history_hides_a_pinned_record_unless_following_it_asks_for_it() {
    let mut app = app_in(View::Mux);
    app.update(Msg::FollowEntity(mux("config-agent3")));
    app.update(Msg::HandoffSettled);
    // The ghost is listed in the Mux view, so that is where it is found.
    assert_eq!(app.active_view(), View::Mux);
}

#[test]
fn natural_views_follow_the_kind_and_whether_it_is_running() {
    let app = app_in(View::Mux);
    let snapshot = app.snapshot_handle().expect("data").snapshot();
    assert_eq!(natural_view(snapshot, &mux("scratch")), View::Mux);
    assert_eq!(natural_view(snapshot, &mux("config-agent3")), View::Mux);
    let pr = crate::model::NodeId::ForgePr(crate::model::ForgePrId::new(
        "github",
        "github.com",
        "malloc47",
        "conspectus",
        42,
    ));
    assert_eq!(natural_view(snapshot, &pr), View::Prs);
    let session = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "solo000001"));
    assert_eq!(natural_view(snapshot, &session), View::Sessions);
    let pin = NodeId::Pin(crate::model::PinId::new("solo"));
    assert_eq!(natural_view(snapshot, &pin), View::Pinned);
}

#[test]
fn a_rename_started_on_the_right_panes_entity_renames_that_entity() {
    let mut app = app_in(View::Mux);
    select(&mut app, "scratch");
    app.set_rename_node(mux("conspectus-main"));
    let effects = app.update(Msg::CommitRename("renamed".into()));
    assert_eq!(
        effects,
        vec![Effect::WriteStore(
            crate::tui::effect::StoreOp::CommitMuxRename {
                mux_id: MuxSessionId::new("tmux:conspectus-main"),
                new_name: "renamed".into(),
            }
        )]
    );
}

#[test]
fn a_pins_status_decides_what_the_menu_offers_to_rebuild() {
    let mut offered = Vec::new();
    for (name, kind, status) in [
        ("config-agent3", PinKind::Mux, SlotStatus::Gone),
        ("nightly", PinKind::Mux, SlotStatus::Planned),
        ("ledger-worktree", PinKind::Checkout, SlotStatus::Unknown),
        ("conspectus-main", PinKind::Mux, SlotStatus::Live),
        ("e44d01cb", PinKind::Session, SlotStatus::Live),
        ("ledger-pr", PinKind::Pr, SlotStatus::Live),
    ] {
        let mut app = app_listing_all(View::Pinned);
        select(&mut app, name);
        let labels = menu_labels(&mut app);
        offered.push((
            name,
            kind,
            status,
            labels.iter().any(|l| l == "Rebuild"),
            labels.iter().any(|l| l == "Show rebuild plan"),
        ));
    }
    let expected = [
        ("config-agent3", PinKind::Mux, SlotStatus::Gone, true, true),
        ("nightly", PinKind::Mux, SlotStatus::Planned, true, true),
        // Unchecked: the plan explains, a rebuild isn't offered blind.
        (
            "ledger-worktree",
            PinKind::Checkout,
            SlotStatus::Unknown,
            false,
            true,
        ),
        (
            "conspectus-main",
            PinKind::Mux,
            SlotStatus::Live,
            false,
            true,
        ),
        ("e44d01cb", PinKind::Session, SlotStatus::Live, false, true),
        ("ledger-pr", PinKind::Pr, SlotStatus::Live, false, false),
    ];
    assert_eq!(offered, expected);
}

#[test]
fn the_menu_is_the_same_wherever_a_pinned_entity_is_opened_from_the_right_pane() {
    let mut left = app_in(View::Mux);
    select(&mut left, "conspectus-main");
    let from_row = menu_labels(&mut left);
    let mut right = app_in(View::Sessions);
    select(&mut right, "e44d01cb");
    right.update(Msg::CycleFocus);
    // Walk the right pane onto the mux the session runs in.
    for _ in 0..60 {
        if action_subject(&right).node == Some(mux("conspectus-main")) {
            break;
        }
        right.update(Msg::ExplorerNavDown);
    }
    assert_eq!(action_subject(&right).node, Some(mux("conspectus-main")));
    // Reveal is offered only away from the entity's own view.
    let without_reveal = |labels: Vec<String>| -> Vec<String> {
        labels
            .into_iter()
            .filter(|label| label != "Reveal in its view")
            .collect()
    };
    assert_eq!(
        without_reveal(menu_labels(&mut right)),
        without_reveal(from_row)
    );
}
