use std::collections::BTreeMap;

use super::*;
use crate::config::HistoryConfig;
use crate::history::{HistoryStore, HistoryWriter};
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphNode, MuxProbe, MuxProbeStatus, MuxSessionId,
    MuxSessionNode, RuntimeProcessId, RuntimeProcessNode,
};
use crate::pin_slots::SlotStore;

fn mux_node(name: &str) -> GraphNode {
    let mut mux = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", name);
    mux.cwd = Some("/src/app".into());
    mux.created_epoch = Some(900);
    // Distinct incarnations: history tells muxes apart by session id.
    mux.backend_session_id = Some(format!("${name}"));
    GraphNode::MuxSession(mux)
}

fn snapshot(nodes: Vec<GraphNode>) -> GraphSnapshot {
    GraphSnapshot {
        nodes,
        mux_probes: BTreeMap::from([(
            "tmux".to_string(),
            MuxProbe {
                status: MuxProbeStatus::Listed,
                epoch: 1_000,
                exited: Vec::new(),
            },
        )]),
        ..GraphSnapshot::empty()
    }
}

struct Env {
    _dir: tempfile::TempDir,
    store: SlotStore,
    history: HistoryWriter,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    Env {
        store: SlotStore::at(dir.path().join("pins.toml")),
        history: HistoryWriter {
            store: HistoryStore::at(dir.path().join("history")),
            config: HistoryConfig::default(),
            home: None,
        },
        _dir: dir,
    }
}

fn runtime_process(id: RuntimeProcessId) -> RuntimeProcessNode {
    RuntimeProcessNode {
        observation_key: id.observation_key.clone(),
        id,
        pid: Some(1),
        parent_pid: None,
        root_pane_pid: None,
        command: Some("sleep".into()),
        cwd: None,
        harness_key: None,
        role: None,
        depth: None,
        observed_epoch: None,
    }
}

fn session_node() -> (AgentSessionId, GraphNode) {
    let id = AgentSessionId::new("claude-code", "scope", "abcdef1234567890");
    let mut node = AgentSessionNode::new(id.clone(), "claude-code");
    node.title = Some("Fix the parser".into());
    node.cwd = Some("/src/app".into());
    (id, GraphNode::AgentSession(node))
}

#[test]
fn pinning_a_live_mux_records_it_and_points_the_slot_at_the_record() {
    let env = env();
    let snap = snapshot(vec![mux_node("work")]);
    let what = find_pinnable(&snap, "work").unwrap();

    let slot = pin(&snap, &env.store, Some(&env.history), &what, None).unwrap();

    assert_eq!(slot.id, "work");
    assert_eq!(slot.kind, PinKind::Mux);
    let history = env.history.store.load().history;
    let record = &history.muxes[0];
    assert_eq!(slot.target, Some(PinSlot::history_target(&record.meta.id)));
    assert_eq!(
        slot.snapshot.get("name").and_then(FieldValue::as_text),
        Some("work")
    );
    assert!(slot.overrides.is_empty(), "pinning adds no overrides");
    assert!(slot.pinned_at.is_some());
    assert_eq!(env.store.load().slots.len(), 1);
}

#[test]
fn a_second_pin_of_the_same_name_gets_a_unique_slot_name() {
    let env = env();
    let snap = snapshot(vec![mux_node("work")]);
    let what = find_pinnable(&snap, "work").unwrap();
    pin(&snap, &env.store, Some(&env.history), &what, None).unwrap();

    let second = pin(&snap, &env.store, Some(&env.history), &what, None).unwrap();

    assert_eq!(second.id, "work-2");
    assert_eq!(
        env.history.store.load().history.muxes.len(),
        1,
        "both slots share the one record"
    );
}

#[test]
fn an_explicit_name_is_validated_and_kept() {
    let env = env();
    let snap = snapshot(vec![mux_node("work")]);
    let what = find_pinnable(&snap, "work").unwrap();

    let slot = pin(&snap, &env.store, Some(&env.history), &what, Some("daily")).unwrap();
    assert_eq!(slot.id, "daily");

    let err = pin(&snap, &env.store, Some(&env.history), &what, Some("daily")).unwrap_err();
    assert!(
        matches!(err, PinError::Slot(SlotError::DuplicateId(_))),
        "{err}"
    );
    let err = pin(&snap, &env.store, Some(&env.history), &what, Some("")).unwrap_err();
    assert!(matches!(err, PinError::Slot(_)), "{err}");
}

#[test]
fn a_mux_cannot_be_pinned_without_a_history_store() {
    let env = env();
    let snap = snapshot(vec![mux_node("work")]);
    let what = find_pinnable(&snap, "work").unwrap();

    let err = pin(&snap, &env.store, None, &what, None).unwrap_err();

    assert!(matches!(err, PinError::NoHistory(_)), "{err}");
    assert!(
        env.store.load().slots.is_empty(),
        "nothing is written on failure"
    );
}

#[test]
fn a_session_is_pinned_by_node_id_with_no_history_record() {
    let env = env();
    let (id, node) = session_node();
    let snap = snapshot(vec![node]);
    let what = find_pinnable(&snap, &NodeId::AgentSession(id.clone()).to_string()).unwrap();

    let slot = pin(&snap, &env.store, None, &what, None).unwrap();

    assert_eq!(slot.kind, PinKind::Session);
    assert_eq!(slot.target, Some(NodeId::AgentSession(id).to_string()));
    assert_eq!(slot.id, "Fix-the-parser", "named from its title");
}

#[test]
fn a_history_record_is_pinned_by_its_id() {
    let env = env();
    let live = snapshot(vec![mux_node("work")]);
    let what = find_pinnable(&live, "work").unwrap();
    pin(&live, &env.store, Some(&env.history), &what, Some("first")).unwrap();

    // The mux is gone now; only its record remains.
    let gone = GraphSnapshot {
        history: env.history.store.load().history,
        ..GraphSnapshot::empty()
    };
    let record_id = gone.history.muxes[0].meta.id.clone();
    let what = find_pinnable(&gone, &record_id).unwrap();
    assert_eq!(what, Pinnable::Record(record_id.clone()));

    let slot = pin(&gone, &env.store, None, &what, Some("again")).unwrap();

    assert_eq!(slot.kind, PinKind::Mux);
    assert_eq!(slot.target, Some(PinSlot::history_target(&record_id)));
}

#[test]
fn a_runtime_process_cannot_be_pinned() {
    let env = env();
    let id = RuntimeProcessId::new("proc:1");
    let snap = snapshot(vec![GraphNode::RuntimeProcess(runtime_process(id.clone()))]);
    let what = find_pinnable(&snap, &NodeId::RuntimeProcess(id).to_string()).unwrap();

    let err = pin(&snap, &env.store, Some(&env.history), &what, None).unwrap_err();

    assert!(matches!(err, PinError::NotPinnable(_)), "{err}");
}

#[test]
fn find_pinnable_reports_nothing_and_ambiguous_matches() {
    let snap = snapshot(vec![mux_node("work")]);
    assert!(matches!(
        find_pinnable(&snap, "nope"),
        Err(PinError::NotFound(_))
    ));
    assert!(matches!(
        find_pinnable(&snap, "h_000000000000"),
        Err(PinError::NotFound(_))
    ));

    let mut a = MuxSessionNode::new(MuxSessionId::new("tmux:a:work"), "tmux", "work");
    a.socket = Some("a".into());
    let GraphNode::MuxSession(b) = mux_node("work") else {
        unreachable!()
    };
    let both = snapshot(vec![GraphNode::MuxSession(a), GraphNode::MuxSession(b)]);
    let err = find_pinnable(&both, "work").unwrap_err();
    assert!(
        matches!(err, PinError::Invalid(ref m) if m.contains("matches 2 muxes")),
        "{err}"
    );
}

#[test]
fn overrides_parse_by_kind_and_list_fields_split_like_a_shell() {
    let (field, value) = parse_override(PinKind::Mux, r#"argv=claude --model "opus 4""#).unwrap();
    assert_eq!(field, "argv");
    assert_eq!(
        value,
        FieldValue::List(vec!["claude".into(), "--model".into(), "opus 4".into()])
    );

    let (field, value) = parse_override(PinKind::Mux, "cwd=/tmp/x").unwrap();
    assert_eq!(
        (field.as_str(), value),
        ("cwd", FieldValue::Text("/tmp/x".into()))
    );

    let (field, _) = parse_override(PinKind::Mux, "label=Daily").unwrap();
    assert_eq!(field, "label");
}

#[test]
fn bad_overrides_are_refused_with_the_allowed_fields() {
    let err = parse_override(PinKind::Mux, "nonsense").unwrap_err();
    assert!(err.to_string().contains("field=value"), "{err}");

    let err = parse_override(PinKind::Pr, "argv=x").unwrap_err();
    let message = err.to_string();
    assert!(message.contains("no overridable field `argv`"), "{message}");
    assert!(message.contains("label"), "{message}");
}

#[test]
fn an_unfinished_list_is_refused_and_a_text_field_is_taken_as_written() {
    let err = parse_override(PinKind::Mux, "argv=claude --model 'opus").unwrap_err();
    assert_eq!(err.to_string(), "`argv` has an unclosed `'` quote");
    let err = parse_override(PinKind::Mux, r"argv=claude \").unwrap_err();
    assert_eq!(err.to_string(), "`argv` has a trailing escape");

    let (_, value) = parse_override(PinKind::Mux, r#"argv=claude -p "a\b""#).unwrap();
    assert_eq!(
        value,
        FieldValue::List(vec!["claude".into(), "-p".into(), r"a\b".into()]),
        "a backslash before an ordinary character stays inside double quotes"
    );
    let (_, value) = parse_override(PinKind::Mux, "label=it's 'odd").unwrap();
    assert_eq!(value, FieldValue::Text("it's 'odd".into()));
}

#[test]
fn edit_sets_and_clears_overrides_and_reports_no_ops() {
    let env = env();
    let snap = snapshot(vec![mux_node("work")]);
    let what = find_pinnable(&snap, "work").unwrap();
    pin(&snap, &env.store, Some(&env.history), &what, None).unwrap();

    let set = vec![("cwd".to_string(), FieldValue::Text("/elsewhere".into()))];
    assert!(edit(&env.store, "work", &set, &[]).unwrap());
    assert!(
        !edit(&env.store, "work", &set, &[]).unwrap(),
        "same value is a no-op"
    );
    let slot = env.store.load().slots.remove(0);
    assert_eq!(
        slot.overrides.get("cwd"),
        Some(&FieldValue::Text("/elsewhere".into()))
    );

    assert!(edit(&env.store, "work", &[], &["cwd".to_string()]).unwrap());
    assert!(env.store.load().slots[0].overrides.is_empty());

    let err = edit(&env.store, "missing", &set, &[]).unwrap_err();
    assert!(
        matches!(err, PinError::Slot(SlotError::NotFound(_))),
        "{err}"
    );
}

#[test]
fn retarget_moves_a_slot_to_another_entity_of_its_kind() {
    let env = env();
    let snap = snapshot(vec![mux_node("work"), mux_node("play")]);
    let work = find_pinnable(&snap, "work").unwrap();
    pin(&snap, &env.store, Some(&env.history), &work, Some("slot")).unwrap();
    let before = env.store.load().slots[0].clone();

    let play = find_pinnable(&snap, "play").unwrap();
    let updated = retarget(&snap, &env.store, Some(&env.history), "slot", &play).unwrap();

    assert_ne!(updated.target, before.target);
    assert_eq!(
        updated.snapshot.get("name").and_then(FieldValue::as_text),
        Some("play")
    );
    assert_eq!(env.store.load().slots[0], updated);
    assert_eq!(env.history.store.load().history.muxes.len(), 2);
}

#[test]
fn retarget_refuses_a_different_kind_and_keeps_the_slot() {
    let env = env();
    let (sid, node) = session_node();
    let snap = snapshot(vec![mux_node("work"), node]);
    let work = find_pinnable(&snap, "work").unwrap();
    pin(&snap, &env.store, Some(&env.history), &work, Some("slot")).unwrap();
    let before = env.store.load().slots[0].clone();

    let session = Pinnable::Node(NodeId::AgentSession(sid));
    let err = retarget(&snap, &env.store, Some(&env.history), "slot", &session).unwrap_err();

    assert!(
        matches!(
            err,
            PinError::KindMismatch {
                slot: PinKind::Mux,
                target: PinKind::Session
            }
        ),
        "{err}"
    );
    assert_eq!(env.store.load().slots[0], before);
}

#[test]
fn a_planned_slot_has_no_target_and_keeps_its_overrides() {
    let env = env();
    let overrides = SlotFields::from([("cwd".to_string(), FieldValue::Text("/src/app".into()))]);

    let slot = pin_planned(&env.store, "later", PinKind::Mux, overrides.clone()).unwrap();

    assert_eq!(slot.target, None);
    assert_eq!(slot.overrides, overrides);
    assert_eq!(env.store.load().slots[0], slot);
    let err = pin_planned(&env.store, "later", PinKind::Mux, SlotFields::new()).unwrap_err();
    assert!(
        matches!(err, PinError::Slot(SlotError::DuplicateId(_))),
        "{err}"
    );
}

#[test]
fn refresh_records_the_entity_not_the_overrides() {
    let env = env();
    let snap = snapshot(vec![mux_node("work")]);
    let what = find_pinnable(&snap, "work").unwrap();
    pin(&snap, &env.store, Some(&env.history), &what, None).unwrap();
    edit(
        &env.store,
        "work",
        &[("cwd".to_string(), FieldValue::Text("/override".into()))],
        &[],
    )
    .unwrap();

    // The mux moved on; refresh copies where it is now.
    let GraphNode::MuxSession(mut moved) = mux_node("work") else {
        unreachable!()
    };
    moved.cwd = Some("/src/moved".into());
    let snap = snapshot(vec![GraphNode::MuxSession(moved)]);
    let mut snap = snap;
    snap.history = env.history.store.load().history;

    assert!(refresh_snapshot(&snap, &env.store, "work").unwrap());

    let slot = env.store.load().slots.remove(0);
    assert_eq!(
        slot.snapshot.get("cwd").and_then(FieldValue::as_text),
        Some("/src/moved")
    );
    assert_eq!(
        slot.overrides.get("cwd").and_then(FieldValue::as_text),
        Some("/override"),
        "overrides are untouched"
    );
    assert!(
        !refresh_snapshot(&snap, &env.store, "work").unwrap(),
        "idempotent"
    );
}

#[test]
fn renaming_carries_a_label_that_was_only_the_old_name() {
    let env = env();
    pin_planned(&env.store, "alpha", PinKind::Mux, SlotFields::new()).unwrap();
    pin_planned(&env.store, "custom", PinKind::Mux, SlotFields::new()).unwrap();
    env.store
        .update("custom", |slot| {
            slot.snapshot
                .insert("label".into(), FieldValue::Text("Nice name".into()));
            Ok(())
        })
        .unwrap();

    assert!(rename(&env.store, "alpha", "beta").unwrap());
    assert!(rename(&env.store, "custom", "other").unwrap());

    let slots = env.store.load().slots;
    let label = |id: &str| slots.iter().find(|s| s.id == id).unwrap().label();
    assert_eq!(label("beta"), "beta");
    assert_eq!(label("other"), "Nice name");
    let err = rename(&env.store, "beta", "other").unwrap_err();
    assert!(
        matches!(err, PinError::Slot(SlotError::DuplicateId(_))),
        "{err}"
    );
}

#[test]
fn pinning_fails_when_history_cannot_take_the_record() {
    let env = env();
    let dir = env.history.store.dir().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("muxes.json"),
        r#"{"schema_version": 99, "records": []}"#,
    )
    .unwrap();
    let snap = snapshot(vec![mux_node("work")]);
    let what = find_pinnable(&snap, "work").unwrap();

    let err = pin(&snap, &env.store, Some(&env.history), &what, None).unwrap_err();

    assert!(
        matches!(err, PinError::Invalid(ref m) if m.contains("can't be written")),
        "{err}"
    );
    assert!(
        env.store.load().slots.is_empty(),
        "no slot points at nothing"
    );
}
