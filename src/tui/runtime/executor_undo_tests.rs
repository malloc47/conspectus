use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, FieldValue, GraphNode, GraphSnapshot, NodeId, PinKind,
    PinSlot, SlotFields,
};
use crate::pin_slots::SlotStore;
use crate::tui::effect::StoreOp;

fn store() -> (tempfile::TempDir, SlotStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SlotStore::at(dir.path().join("conspectus").join("pins.toml"));
    (dir, store)
}

fn mux_pin(id: &str, argv: &[&str]) -> PinSlot {
    PinSlot {
        id: id.into(),
        kind: PinKind::Mux,
        target: Some(PinSlot::history_target("h_7f3a9c0e21d4")),
        pinned_at: Some("2026-10-19T12:00:00Z".into()),
        overrides: SlotFields::from([
            (
                "argv".to_string(),
                FieldValue::List(argv.iter().map(|a| (*a).to_string()).collect()),
            ),
            ("cwd".to_string(), FieldValue::Text("/src/app".into())),
        ]),
        snapshot: SlotFields::from([("name".to_string(), FieldValue::Text(id.into()))]),
        order: 0,
        legacy: false,
    }
}

fn ids(store: &SlotStore) -> Vec<String> {
    store.load().slots.into_iter().map(|slot| slot.id).collect()
}

/// Run the write an undo entry reverses to, as the executor would.
fn apply(store: &SlotStore, undo: Undo) -> Result<String, String> {
    match undo.into_op() {
        StoreOp::RemovePin { id } => remove_pin_with(store, &id),
        StoreOp::RestorePin { slot, index } => restore_pin_with(store, &slot, index, false),
        other => panic!("not an undo write: {other:?}"),
    }
}

#[test]
fn unpinning_then_undoing_restores_the_pin_exactly_where_it_was() {
    let (_dir, store) = store();
    for id in ["a", "b", "c"] {
        store
            .add(mux_pin(id, &["claude", "--model", "opus"]))
            .unwrap();
    }
    let before = store.load().slots;

    let (outcome, undo) = unpin_with(&store, "b", None);

    assert!(
        outcome.unwrap().contains("to undo"),
        "the toast offers undo"
    );
    assert_eq!(ids(&store), ["a", "c"]);
    let restored = apply(&store, undo.expect("an undo was recorded")).unwrap();
    assert_eq!(restored, "restored pin `b`");
    assert_eq!(
        store.load().slots,
        before,
        "name, overrides, saved values, and place"
    );
}

#[test]
fn the_unpin_toast_names_the_undo_key_from_the_key_table() {
    let (_dir, store) = store();
    store.add(mux_pin("a", &["claude"])).unwrap();
    let (outcome, _) = unpin_with(&store, "a", None);
    let text = outcome.unwrap();
    assert!(text.ends_with(&format!("{UNDO_KEY} to undo")), "{text}");
    assert!(text.contains("its 2 overrides were removed"), "{text}");
}

#[test]
fn pinning_then_undoing_removes_the_pin() {
    let (_dir, store) = store();
    let id = AgentSessionId::new("claude-code", "scope", "abcdef1234567890");
    let mut node = AgentSessionNode::new(id.clone(), "claude-code");
    node.title = Some("Fix the parser".into());
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::AgentSession(node)],
        ..GraphSnapshot::empty()
    };
    let what = crate::pin_ops::Pinnable::Node(NodeId::AgentSession(id));

    let (outcome, undo) = pin_with(&store, &snapshot, None, &what);

    assert!(outcome.unwrap().contains("to undo"));
    assert_eq!(ids(&store).len(), 1);
    assert_eq!(
        apply(&store, undo.unwrap()).unwrap().as_str(),
        "removed pin `Fix-the-parser`"
    );
    assert!(ids(&store).is_empty());
}

#[test]
fn several_unpins_undo_in_reverse_order() {
    let (_dir, store) = store();
    for id in ["a", "b", "c", "d"] {
        store.add(mux_pin(id, &["claude"])).unwrap();
    }
    let before = store.load().slots;
    let mut stack = crate::tui::undo::UndoStack::default();
    for id in ["b", "d", "a"] {
        let (_, undo) = unpin_with(&store, id, None);
        stack.push(undo.unwrap());
    }
    assert_eq!(ids(&store), ["c"]);

    while let Some(undo) = stack.pop() {
        apply(&store, undo).unwrap();
    }

    assert_eq!(store.load().slots, before, "the order is exactly as before");
}

#[test]
fn undoing_after_the_name_was_reused_refuses_and_changes_nothing() {
    let (_dir, store) = store();
    store.add(mux_pin("a", &["claude"])).unwrap();
    let (_, undo) = unpin_with(&store, "a", None);
    store.add(mux_pin("a", &["codex"])).unwrap();

    let outcome = apply(&store, undo.unwrap());

    assert_eq!(outcome.unwrap_err(), "undo: a pin named `a` exists now");
    let slots = store.load().slots;
    assert_eq!(slots.len(), 1);
    assert_eq!(
        slots[0].overrides.get("argv"),
        Some(&FieldValue::List(vec!["codex".into()])),
        "the newer pin is untouched"
    );
}

#[test]
fn a_name_held_by_a_legacy_pin_refuses_the_restore() {
    let (_dir, store) = store();
    let outcome = restore_pin_with(&store, &mux_pin("legacy", &["claude"]), 0, true);
    assert_eq!(
        outcome.unwrap_err(),
        "undo: a pin named `legacy` exists now"
    );
    assert!(ids(&store).is_empty());
}

#[test]
fn undoing_a_pin_that_is_already_gone_says_so() {
    let (_dir, store) = store();
    let outcome = remove_pin_with(&store, "nope");
    assert_eq!(outcome.unwrap_err(), "undo: pin `nope` is already gone");
}

#[test]
fn a_pin_whose_record_was_reaped_still_comes_back() {
    let (_dir, store) = store();
    // The pin targets a history record that no longer exists anywhere;
    // restoring only writes the pin, which then shows as missing.
    let outcome = restore_pin_with(&store, &mux_pin("orphan", &["claude"]), 0, false);
    assert!(outcome.is_ok());
    assert_eq!(ids(&store), ["orphan"]);
}
