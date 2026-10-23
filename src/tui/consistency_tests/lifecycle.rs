//! Every state-changing action ends the same way: its keys reach an
//! executor that can run them, the acted-on row freezes, the views
//! refresh without losing data, and focus follows the entity.

use super::*;
use crate::tui::Effect;
use crate::tui::effect::{ExecSpec, StoreOp, lifecycle};
use crate::tui::keybindings::KEYBINDINGS;
use crate::tui::keymap::Action;

/// Effects only the live-loop executor runs. The executor used for
/// plain messages silently drops them.
fn needs_the_live_executor(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::WriteStore(_) | Effect::Exec(_) | Effect::RunMux(_)
    )
}

#[test]
#[ignore = "backlog CSP-692.02: p pins nothing because key-bound messages run on the executor that drops store writes"]
fn keys_that_carry_a_message_never_emit_effects_that_executor_drops() {
    let mut checked = 0;
    let mut dropped = Vec::new();
    for binding in KEYBINDINGS {
        let Action::Msg(msg) = (binding.action)() else {
            continue;
        };
        let mut app = app_in(View::Mux);
        select(&mut app, "scratch");
        let described = format!("{:?}", binding.key);
        let effects = app.update(*msg);
        checked += 1;
        if effects.iter().any(needs_the_live_executor) {
            dropped.push(format!("{described}: {effects:?}"));
        }
    }
    assert!(
        checked > 10,
        "the key table has message bindings: {checked}"
    );
    assert!(
        dropped.is_empty(),
        "these keys emit effects the plain executor drops:\n{}",
        dropped.join("\n")
    );
}

fn every_store_op() -> Vec<StoreOp> {
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
        StoreOp::EditSlot {
            slot: "x".into(),
            assignment: "cwd=/".into(),
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
#[ignore = "backlog CSP-692.02: a rebuild from the menu does not freeze the row or follow the mux to its view"]
fn a_rebuild_freezes_the_row_refreshes_and_follows_the_entity() {
    let mut app = app_in(View::History);
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
#[ignore = "backlog CSP-692.02: a launch does not follow the new mux to its row"]
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
#[ignore = "backlog CSP-692.02: rows of the mixed-kind renderer never freeze after an attach"]
fn returning_from_an_attach_freezes_every_row_that_shows_the_mux() {
    let mux = MuxSessionId::new("tmux:conspectus-main");
    let mut frozen_rows = 0;
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
                app.row_awaits_handoff(&row.kind),
                "{}: the row must dim with a spinner until the refresh lands",
                describe(view, &id)
            );
            frozen_rows += 1;
        }
    }
    assert!(frozen_rows >= 2, "{frozen_rows}");
}

#[test]
#[ignore = "backlog CSP-692.02: a store write refreshes locally with cached-only forge data, which empties the PR list"]
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
