use std::fs;

use super::*;
use crate::model::{AgentSessionId, NodeId, SlotTarget};

fn store() -> (tempfile::TempDir, SlotStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SlotStore::at(dir.path().join("conspectus").join("pins.toml"));
    (dir, store)
}

fn text(value: &str) -> FieldValue {
    FieldValue::Text(value.into())
}

fn mux_slot(id: &str) -> PinSlot {
    PinSlot {
        id: id.into(),
        kind: PinKind::Mux,
        target: Some(PinSlot::history_target("h_7f3a9c0e21d4")),
        pinned_at: Some("2026-10-19T12:00:00Z".into()),
        overrides: SlotFields::from([(
            "argv".to_string(),
            FieldValue::List(vec![
                "claude".into(),
                "--dangerously-skip-permissions".into(),
            ]),
        )]),
        snapshot: SlotFields::from([
            ("name".to_string(), text("conspectus-main")),
            ("cwd".to_string(), text("/home/me/src/conspectus")),
        ]),
        order: 0,
        legacy: false,
    }
}

fn ids(store: &SlotStore) -> Vec<String> {
    store.load().slots.into_iter().map(|slot| slot.id).collect()
}

#[test]
fn slots_round_trip_with_their_order_and_fields() {
    let (_dir, store) = store();
    store.add(mux_slot("a")).unwrap();
    store
        .add(PinSlot {
            kind: PinKind::Pr,
            target: Some("forge_pr:github:github.com/me/app#7".into()),
            overrides: SlotFields::new(),
            snapshot: SlotFields::from([("label".to_string(), text("#7 Ship it"))]),
            ..mux_slot("b")
        })
        .unwrap();
    store
        .add(PinSlot {
            target: None,
            ..mux_slot("planned")
        })
        .unwrap();

    let load = store.load();
    assert!(load.problems.is_empty(), "{:?}", load.problems);
    assert_eq!(ids(&store), ["a", "b", "planned"]);
    assert_eq!(load.slots[0].order, 0);
    assert_eq!(load.slots[2].order, 2);
    assert_eq!(load.slots[0], mux_slot("a"));
    assert_eq!(load.slots[1].label(), "#7 Ship it");
    assert_eq!(load.slots[2].parsed_target(), Ok(SlotTarget::Planned));

    let written = fs::read_to_string(store.path()).unwrap();
    assert!(written.contains("schema_version = 2"), "{written}");
    assert!(
        written.contains("pinned_at = 2026-10-19T12:00:00Z"),
        "{written}"
    );
    assert!(written.contains("[pins.slots.overrides]"), "{written}");
}

#[test]
fn a_target_is_a_history_record_a_node_or_nothing() {
    let slot = mux_slot("a");
    assert_eq!(
        slot.parsed_target(),
        Ok(SlotTarget::History("h_7f3a9c0e21d4".into()))
    );
    let node = PinSlot {
        target: Some("agent_session:claude-code:global:e44d01cb".into()),
        ..mux_slot("b")
    };
    assert_eq!(
        node.parsed_target(),
        Ok(SlotTarget::Node(NodeId::AgentSession(AgentSessionId::new(
            "claude-code",
            "global",
            "e44d01cb"
        ))))
    );
    let bad = PinSlot {
        target: Some("nonsense".into()),
        ..mux_slot("c")
    };
    assert!(validate_slot(&bad).is_err());
}

#[test]
fn comments_and_unknown_keys_survive_a_write() {
    let (_dir, store) = store();
    fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    fs::write(
        store.path(),
        r#"# my pins, hand-curated
[pins]
schema_version = 2

# the daily driver
[[pins.slots]]
id = "daily"
kind = "mux"
future_key = "kept"
target = "history:h_aaaaaaaaaaaa"

[pins.slots.overrides]
name = "daily"

# the scratch one
[[pins.slots]]
id = "scratch"
kind = "mux"
"#,
    )
    .unwrap();
    store
        .update("scratch", |slot| {
            slot.overrides.insert("cwd".into(), text("/tmp"));
            Ok(())
        })
        .unwrap();
    store.add(mux_slot("new")).unwrap();

    let written = fs::read_to_string(store.path()).unwrap();
    assert!(written.contains("# my pins, hand-curated"), "{written}");
    assert!(written.contains("# the daily driver"), "{written}");
    assert!(written.contains("# the scratch one"), "{written}");
    assert!(written.contains("future_key = \"kept\""), "{written}");
    assert_eq!(ids(&store), ["daily", "scratch", "new"]);
    let scratch = &store.load().slots[1];
    assert_eq!(scratch.overrides["cwd"], text("/tmp"));
}

#[test]
fn updates_that_change_nothing_do_not_write() {
    let (_dir, store) = store();
    store.add(mux_slot("a")).unwrap();
    let before = fs::read_to_string(store.path()).unwrap();
    let outcome = store.update("a", |_| Ok(())).unwrap();
    assert!(!outcome.changed);
    assert_eq!(fs::read_to_string(store.path()).unwrap(), before);
    assert!(!store.remove("absent").unwrap().changed);
}

#[test]
fn ids_are_unique_and_names_checked() {
    let (_dir, store) = store();
    store.add(mux_slot("a")).unwrap();
    assert!(matches!(
        store.add(mux_slot("a")),
        Err(SlotError::DuplicateId(_))
    ));
    assert!(store.add(mux_slot(" padded ")).is_err());
    assert!(store.add(mux_slot("")).is_err());
    assert!(matches!(
        store.update("absent", |_| Ok(())),
        Err(SlotError::NotFound(_))
    ));
}

#[test]
fn overrides_are_limited_to_the_kinds_fields_and_types() {
    let (_dir, store) = store();
    let mut slot = mux_slot("a");
    slot.overrides.insert("branch".into(), text("x"));
    let err = store.add(slot).unwrap_err().to_string();
    assert!(err.contains("no overridable field `branch`"), "{err}");

    let mut slot = mux_slot("a");
    slot.overrides.insert("argv".into(), text("claude"));
    assert!(
        store
            .add(slot)
            .unwrap_err()
            .to_string()
            .contains("must be a list")
    );

    let mut pr = mux_slot("pr");
    pr.kind = PinKind::Pr;
    pr.target = Some("forge_pr:github:github.com/me/app#7".into());
    pr.overrides = SlotFields::from([("label".to_string(), text("mine"))]);
    store.add(pr).unwrap();
}

#[test]
fn rename_keeps_the_slots_place_and_refuses_a_taken_name() {
    let (_dir, store) = store();
    for id in ["a", "b", "c"] {
        store.add(mux_slot(id)).unwrap();
    }
    store.rename("b", "renamed").unwrap();
    assert_eq!(ids(&store), ["a", "renamed", "c"]);
    assert!(matches!(
        store.rename("a", "c"),
        Err(SlotError::DuplicateId(_))
    ));
    assert!(!store.rename("a", "a").unwrap().changed);
}

#[test]
fn slots_move_to_a_place_in_the_order() {
    let (_dir, store) = store();
    for id in ["a", "b", "c", "d"] {
        store.add(mux_slot(id)).unwrap();
    }
    store.move_slot("d", &Placement::Top).unwrap();
    assert_eq!(ids(&store), ["d", "a", "b", "c"]);
    store.move_slot("d", &Placement::Bottom).unwrap();
    assert_eq!(ids(&store), ["a", "b", "c", "d"]);
    store.move_slot("a", &Placement::After("c".into())).unwrap();
    assert_eq!(ids(&store), ["b", "c", "a", "d"]);
    store
        .move_slot("d", &Placement::Before("c".into()))
        .unwrap();
    assert_eq!(ids(&store), ["b", "d", "c", "a"]);
    store.move_slot("c", &Placement::Step(-1)).unwrap();
    assert_eq!(ids(&store), ["b", "c", "d", "a"]);
    store.move_slot("b", &Placement::Step(-1)).unwrap();
    assert_eq!(ids(&store), ["b", "c", "d", "a"], "already at the top");
    assert!(
        store
            .move_slot("b", &Placement::Before("b".into()))
            .is_err()
    );
    assert!(matches!(
        store.move_slot("zzz", &Placement::Top),
        Err(SlotError::NotFound(_))
    ));
}

#[test]
fn a_file_with_another_schema_version_is_never_read_or_written() {
    let (_dir, store) = store();
    fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    let newer = "[pins]\nschema_version = 3\n\n[[pins.slots]]\nid = \"x\"\nkind = \"mux\"\n";
    fs::write(store.path(), newer).unwrap();
    let load = store.load();
    assert!(load.unsupported && load.slots.is_empty());
    assert!(matches!(
        store.add(mux_slot("a")),
        Err(SlotError::UnsupportedSchema(3))
    ));
    assert_eq!(fs::read_to_string(store.path()).unwrap(), newer);
}

#[test]
fn a_bad_slot_is_skipped_on_read_and_kept_by_writes() {
    let (_dir, store) = store();
    fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    fs::write(
        store.path(),
        "[pins]\nschema_version = 2\n\n[[pins.slots]]\nid = \"good\"\nkind = \"mux\"\n\n[[pins.slots]]\nid = \"odd\"\nkind = \"spaceship\"\n",
    )
    .unwrap();
    let load = store.load();
    assert_eq!(load.slots.len(), 1);
    assert!(load.problems[0].contains("unknown kind `spaceship`"));
    store.add(mux_slot("more")).unwrap();
    let written = fs::read_to_string(store.path()).unwrap();
    assert!(written.contains("spaceship"), "the bad slot is not dropped");
}

#[test]
fn a_missing_file_is_no_slots_and_reading_creates_nothing() {
    let (dir, store) = store();
    assert!(store.load().slots.is_empty());
    assert!(!dir.path().join("conspectus").exists());
}

#[test]
fn concurrent_gestures_do_not_lose_each_other() {
    let (_dir, store) = store();
    let handles: Vec<_> = (0..6)
        .map(|n| {
            let store = store.clone();
            std::thread::spawn(move || {
                for i in 0..5 {
                    store.add(mux_slot(&format!("t{n}-{i}"))).unwrap();
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let load = store.load();
    assert!(load.problems.is_empty(), "{:?}", load.problems);
    assert_eq!(load.slots.len(), 30);
}

#[test]
fn an_io_error_names_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("not-a-dir");
    std::fs::write(&blocker, "").unwrap();
    let store = SlotStore::at(blocker.join("pins.toml"));

    let err = store
        .add(PinSlot {
            id: "a".into(),
            kind: PinKind::Mux,
            target: None,
            pinned_at: None,
            overrides: Default::default(),
            snapshot: Default::default(),
            order: 0,
            legacy: false,
        })
        .unwrap_err();

    assert!(
        err.to_string().contains("pins.toml") || err.to_string().contains("not-a-dir"),
        "{err}"
    );
}

#[test]
fn a_malformed_file_is_reported_as_malformed_not_as_schema_zero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pins.toml");
    std::fs::write(&path, "[pins\nbroken").unwrap();
    let store = SlotStore::at(&path);

    let err = store.remove("x").unwrap_err().to_string();

    assert!(err.contains("malformed") || err.contains("TOML"), "{err}");
    assert!(!err.contains("schema_version 0"), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[pins\nbroken");
}
