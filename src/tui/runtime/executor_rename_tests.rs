use super::*;
use crate::discovery::tmux::FakeTmux;
use crate::model::{GraphSnapshot, NodeId, PinKind, SlotStatus};
use crate::pin_slots::SlotStore;
use crate::tui::effect::StoreOp;
use ratatui::crossterm::event::KeyCode;
use tuikit::testkit::keys::{ctrl, press};

/// The live mux pin of the Pinned fixture world, and a pin of another
/// name beside it.
const LIVE_MUX_PIN: &str = "conspectus-main";
const OTHER_PIN: &str = "config-agent3";

/// The Pinned fixture world, and a store holding its pins (the legacy
/// one lives in the legacy store, not this one).
fn world() -> (tempfile::TempDir, SlotStore, GraphSnapshot) {
    let snapshot = crate::tui::rows::pinned::fixture_world();
    let dir = tempfile::tempdir().unwrap();
    let store = SlotStore::at(dir.path().join("conspectus").join("pins.toml"));
    for slot in snapshot.pins.iter().filter(|slot| !slot.legacy) {
        store.add(slot.clone()).unwrap();
    }
    (dir, store, snapshot)
}

fn app_over(snapshot: &GraphSnapshot) -> App {
    let mut config = RunConfig::defaults();
    config.default_view = View::Pinned;
    let mut app = App::new(config);
    let grouping = crate::tui::PinnedGrouping::None;
    app.update(Msg::SetGrouping(crate::tui::Grouping::Pinned(grouping)));
    let tree =
        crate::tui::rows::pinned::build_pinned_tree(crate::tui::rows::pinned::PinnedBuildInputs {
            snapshot,
            home: Some(std::path::Path::new("/home/op")),
            now: Some(1_700_000_000),
            grouping,
            sort: crate::tui::Sort::Hierarchy,
        });
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

/// Type `text` over the open prompt's value and confirm it; what it
/// commits goes through the reducer, and its store writes through the
/// executor's rename with `tmux` and `store`.
fn rename_through_the_prompt(
    app: &mut App,
    snapshot: &GraphSnapshot,
    store: &SlotStore,
    tmux: &FakeTmux,
    text: &str,
) -> Vec<Result<String, String>> {
    let _ = app.press_modal_key(ctrl('u'));
    for c in text.chars() {
        let _ = app.press_modal_key(press(KeyCode::Char(c)));
    }
    let settled = app
        .press_modal_key(press(KeyCode::Enter))
        .expect("the prompt is open");
    let effects = settled
        .message
        .map(|msg| app.update(msg))
        .unwrap_or_default();
    effects
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::WriteStore(StoreOp::RenameSlot { slot, new_name }) => {
                Some(rename_slot_with(snapshot, store, tmux, &slot, &new_name))
            }
            _ => None,
        })
        .collect()
}

fn assert_live_mux_pin(snapshot: &GraphSnapshot) {
    let pin = crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .find(|pin| pin.id.id == LIVE_MUX_PIN)
        .expect("the fixture pins the mux");
    assert_eq!(
        (pin.kind, pin.status),
        (PinKind::Mux, SlotStatus::Live),
        "the pin is a live mux pin, so a rename would rename its session"
    );
    assert!(matches!(pin.target_node, Some(NodeId::MuxSession(_))));
}

#[test]
fn a_live_mux_pin_renamed_through_the_prompt_to_a_taken_name_changes_nothing() {
    let (_dir, store, snapshot) = world();
    assert_live_mux_pin(&snapshot);
    let before = store.load().slots;
    let tmux = FakeTmux::with_sessions("");
    let mut app = app_over(&snapshot);
    app.update(Msg::OpenSlotRename(LIVE_MUX_PIN.into()));

    let writes = rename_through_the_prompt(&mut app, &snapshot, &store, &tmux, OTHER_PIN);

    assert_eq!(
        tmux.rename_calls(),
        Vec::new(),
        "the tmux session keeps its name"
    );
    assert_eq!(store.load().slots, before, "no override and no rename");
    assert_eq!(writes, Vec::new(), "nothing is written: {writes:?}");
    assert_eq!(
        app.rename_overlay()
            .and_then(tuikit::text_prompt::TextPrompt::error),
        Some("a pin named `config-agent3` already exists"),
        "the prompt stays open and says why, as the edit form does"
    );
}

#[test]
fn the_executor_checks_a_taken_name_before_it_renames_the_session() {
    // A name the prompt allowed can be taken by the time the write runs
    // (another pin added from the command line meanwhile).
    let (_dir, store, snapshot) = world();
    let before = store.load().slots;
    let tmux = FakeTmux::with_sessions("");

    let outcome = rename_slot_with(&snapshot, &store, &tmux, LIVE_MUX_PIN, OTHER_PIN);

    assert_eq!(
        outcome,
        Err("rename failed: a pin named `config-agent3` already exists".to_string())
    );
    assert_eq!(tmux.rename_calls(), Vec::new());
    assert_eq!(store.load().slots, before);
}

#[test]
fn a_free_name_renames_the_session_the_pin_and_pins_the_name() {
    let (_dir, store, snapshot) = world();
    let tmux = FakeTmux::with_sessions("");
    let mut app = app_over(&snapshot);
    app.update(Msg::OpenSlotRename(LIVE_MUX_PIN.into()));

    let writes = rename_through_the_prompt(&mut app, &snapshot, &store, &tmux, "ledger");

    assert_eq!(writes.len(), 1, "{writes:?}");
    let summary = writes[0].as_ref().expect("renamed");
    assert!(
        summary.starts_with("renamed pin `conspectus-main` to `ledger`"),
        "{summary}"
    );
    assert_eq!(
        tmux.rename_calls(),
        vec![(None, "conspectus-main".to_string(), "ledger".to_string())]
    );
    let renamed = store
        .load()
        .slots
        .into_iter()
        .find(|slot| slot.id == "ledger")
        .expect("the pin has its new name");
    assert_eq!(
        renamed.overrides.get("name"),
        Some(&crate::model::FieldValue::Text("ledger".into())),
        "the pin keeps following the renamed session"
    );
}
