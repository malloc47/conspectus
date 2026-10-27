//! Every state-changing action ends the same way: its keys reach an
//! executor that can run them, the acted-on row freezes, the views
//! refresh without losing data, and focus follows the entity.

use super::*;
use crate::tui::Effect;
use crate::tui::effect::{ExecSpec, StoreOp, lifecycle};
use crate::tui::keybindings::KEYBINDINGS;
use crate::tui::keymap::Action;

/// Every key bound to a message runs on an executor that can run what
/// the message emits: the plain executor cannot do store writes,
/// launches, or mux ops, so a key whose message emits one must be routed
/// to the live executor, and anything the plain executor is handed
/// anyway is reported to the operator rather than dropped.
#[test]
fn every_key_runs_on_an_executor_that_can_run_what_it_emits() {
    use crate::tui::runtime::{ExecutorKind, live_executor_for, pure_executor_runs};
    let mut checked = 0;
    let mut live_only = 0;
    for binding in KEYBINDINGS {
        let action = (binding.action)();
        let Action::Msg(msg) = &action else {
            continue;
        };
        let mut app = app_in(View::Mux);
        select(&mut app, "scratch");
        let effects = app.update((**msg).clone());
        checked += 1;
        for effect in effects {
            if pure_executor_runs(&effect) {
                continue;
            }
            live_only += 1;
            assert_eq!(
                live_executor_for(&action),
                ExecutorKind::Live,
                "{:?} emits {effect:?}, which only the live executor can run",
                binding.key
            );
        }
    }
    assert!(
        checked > 10,
        "the key table has message bindings: {checked}"
    );
    assert!(
        live_only > 0,
        "`p` and `.`-style keys do emit live-only effects"
    );
}

#[test]
fn the_plain_executor_says_so_when_handed_what_it_cannot_run() {
    let mut app = app_in(View::Mux);
    let node = NodeId::MuxSession(MuxSessionId::new("tmux:scratch"));
    crate::tui::runtime::execute_effects(
        &mut app,
        vec![Effect::WriteStore(StoreOp::PinNode { node })],
    );
    assert!(
        app.toast().has_toast(),
        "dropping a gesture silently looks like it did nothing"
    );
}

pub(super) fn every_store_op() -> Vec<StoreOp> {
    let node = NodeId::MuxSession(MuxSessionId::new("tmux:scratch"));
    vec![
        StoreOp::PinRecord {
            record_id: "h_main".into(),
        },
        StoreOp::PinNode { node },
        StoreOp::Unpin { slot: "x".into() },
        StoreOp::RenameSlot {
            slot: "x".into(),
            new_name: "y".into(),
        },
        StoreOp::SavePin {
            request: crate::tui::widgets::pin_edit::PinEditRequest {
                pin: "x".into(),
                rename: None,
                set: Vec::new(),
                unset: Vec::new(),
            },
        },
        StoreOp::RetargetSlot {
            slot: "x".into(),
            target: "y".into(),
        },
        StoreOp::RefreshSlot { slot: "x".into() },
        StoreOp::ForgetRecord {
            record_id: "h_main".into(),
        },
        StoreOp::ReapRecords {
            ids: vec!["h_main".into()],
        },
        StoreOp::WorktreePrune {
            repo_root: "/r".into(),
        },
    ]
}

#[test]
fn every_store_write_refreshes_the_views() {
    for op in every_store_op() {
        let effect = Effect::WriteStore(op);
        assert!(
            lifecycle(&effect).refreshes,
            "{effect:?} must refresh afterwards"
        );
    }
}

#[test]
fn attaching_hands_over_the_terminal_freezes_the_row_and_refreshes() {
    let mut app = app_in(View::Mux);
    select(&mut app, "conspectus-main");
    let effects = app.update(Msg::AttachSelected);
    let [effect] = effects.as_slice() else {
        panic!("{effects:?}")
    };
    let policy = lifecycle(effect);
    assert!(
        policy.attaches && policy.freezes_row && policy.refreshes,
        "{policy:?}"
    );
}

#[test]
fn a_rebuild_freezes_the_row_refreshes_and_follows_the_entity() {
    let mut app = app_listing_all(View::History);
    let effects = app.update(Msg::RebuildReference("h_agent".into()));
    let [effect] = effects.as_slice() else {
        panic!("{effects:?}")
    };
    let policy = lifecycle(effect);
    assert!(
        policy.refreshes && policy.freezes_row && policy.follows_entity,
        "a record that comes back as a live mux leaves History: {policy:?}"
    );
}

#[test]
fn launching_attaches_refreshes_and_selects_the_new_mux() {
    let effect = Effect::Exec(ExecSpec::MuxNew {
        name: "fresh".into(),
        cwd: "/tmp".into(),
    });
    let policy = lifecycle(&effect);
    assert!(
        policy.attaches && policy.refreshes && policy.follows_entity,
        "{policy:?}"
    );
}

/// The rows, in any view, that stand for the mux `name`.
fn rows_standing_for_mux(app: &App, name: &str) -> Vec<RowId> {
    let mux = NodeId::MuxSession(MuxSessionId::new(format!("tmux:{name}")));
    app.tree()
        .rows
        .iter()
        .filter(|row| match &row.id {
            RowId::MuxSession(node) | RowId::Group(node) => *node == mux,
            RowId::Nested { node, .. } => *node == mux,
            RowId::Pin { pin_id } => pin_id == name,
            _ => false,
        })
        .map(|row| row.id.clone())
        .collect()
}

#[test]
fn returning_from_an_attach_freezes_every_row_that_shows_the_mux() {
    let mux = MuxSessionId::new("tmux:conspectus-main");
    let mut frozen_rows = 0;
    // Not the Search view: it lists sessions that have matches, and never
    // a mux or a pin, so no row there stands for the mux.
    for view in [View::Mux, View::Pinned, View::Sessions] {
        let mut app = app_in(view);
        app.update(Msg::HandoffReturned(mux.clone()));
        assert!(
            app.take_handoff_refresh_request(),
            "{view:?} asks to refresh"
        );
        for id in rows_standing_for_mux(&app, "conspectus-main") {
            let row = app
                .tree()
                .rows
                .iter()
                .find(|row| row.id == id)
                .expect("row");
            assert!(
                app.row_is_frozen(row),
                "{}: the row must dim with a spinner until the refresh lands",
                describe(view, &id)
            );
            frozen_rows += 1;
        }
    }
    assert!(frozen_rows >= 2, "{frozen_rows}");
}

#[test]
fn a_store_write_refresh_keeps_the_pr_list() {
    let mut config = RunConfig::defaults();
    config.forge_fetch = crate::discovery::forge::ForgeFetch::CachedOnly;
    let app = App::new(config);
    let refresh = crate::tui::runtime::store_write_refresh_config(&app);
    let bypasses_the_daemon = refresh.refresh;
    let reads_only_a_cold_cache =
        refresh.forge_fetch == crate::discovery::forge::ForgeFetch::CachedOnly;
    assert!(
        !(bypasses_the_daemon && reads_only_a_cold_cache),
        "a forced local rebuild with a cached-only forge policy drops every PR the daemon had"
    );
}

#[test]
fn an_effect_that_refreshes_asks_for_it_unless_a_handoff_refresh_is_on_its_way() {
    use crate::tui::runtime::wants_refresh_request;
    let write = Effect::WriteStore(StoreOp::Unpin { slot: "x".into() });
    let toast = Effect::Toast("hi".into());
    let mut app = app_in(View::Mux);
    assert!(wants_refresh_request(&lifecycle(&write), &app));
    assert!(!wants_refresh_request(&lifecycle(&toast), &app));
    // Returning from a mux already requests the refresh that settles it.
    app.update(Msg::HandoffReturned(MuxSessionId::new("tmux:scratch")));
    assert!(app.handoff_refresh_pending());
    assert!(!wants_refresh_request(&lifecycle(&write), &app));
}

#[test]
fn a_refresh_request_is_taken_once() {
    let mut app = app_in(View::Mux);
    assert!(!app.take_refresh_request());
    app.request_refresh();
    app.request_refresh();
    assert!(app.take_refresh_request());
    assert!(!app.take_refresh_request());
}

#[test]
fn a_rebuild_declares_that_it_attaches_only_when_asked_to() {
    let rebuild = |attach| {
        lifecycle(&Effect::Exec(ExecSpec::Rebuild {
            reference: "x".into(),
            attach,
        }))
    };
    assert!(rebuild(true).attaches && rebuild(true).follows_entity);
    assert!(!rebuild(false).attaches && rebuild(false).follows_entity);
    assert!(rebuild(false).freezes_row && rebuild(false).refreshes);
}
