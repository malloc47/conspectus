use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, ArgvSource, CheckoutNode, CheckoutRecipe, CheckoutRecord,
    ForgePrNode, HistorySnapshot, MuxIncarnation, MuxRecipe, MuxSessionNode, PrRecipe, PrRecord,
    RecordMeta, RecordedRemote, RepoNode, RepoRecipe, RepoRecord, SlotFields, WorktreeMeta,
};

fn text(value: &str) -> FieldValue {
    FieldValue::Text(value.into())
}

fn list(values: &[&str]) -> FieldValue {
    FieldValue::List(values.iter().map(|v| (*v).to_string()).collect())
}

fn meta(id: &str) -> RecordMeta {
    RecordMeta {
        id: id.into(),
        key: String::new(),
        first_seen: 1,
        last_seen: 100,
        origin: Default::default(),
        display: Default::default(),
        rebuilt_from: None,
    }
}

fn slot(kind: PinKind, target: Option<String>) -> PinSlot {
    PinSlot {
        id: "my-slot".into(),
        kind,
        target,
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    }
}

fn mux_record(id: &str, session_id: &str) -> MuxRecord {
    MuxRecord {
        meta: meta(id),
        recipe: MuxRecipe {
            backend: "tmux".into(),
            name: "work".into(),
            cwd: Some("/history/cwd".into()),
            incarnation: Some(MuxIncarnation {
                session_id: Some(session_id.into()),
                created: Some(50),
            }),
            argv: Some(vec!["claude".into(), "--exact".into()]),
            argv_source: Some(ArgvSource::Conspectus),
            harness: Some("claude-code".into()),
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn live_mux(session_id: &str) -> GraphNode {
    let mut mux = MuxSessionNode::new(MuxSessionId::new("tmux:work"), "tmux", "work");
    mux.cwd = Some("/live/cwd".into());
    mux.backend_session_id = Some(session_id.into());
    mux.created_epoch = Some(50);
    mux.active_pane_start_command = Some("claude --observed".into());
    GraphNode::MuxSession(mux)
}

fn world(nodes: Vec<GraphNode>, history: HistorySnapshot) -> GraphSnapshot {
    GraphSnapshot {
        nodes,
        history,
        ..GraphSnapshot::empty()
    }
}

fn field(resolved: &ResolvedSlot, name: &str) -> (FieldValue, FieldSource) {
    let field = resolved
        .effective
        .iter()
        .find(|field| field.name == name)
        .unwrap_or_else(|| panic!("no field {name}: {:?}", resolved.effective));
    (field.value.clone(), field.source)
}

#[test]
fn each_mux_field_comes_from_the_highest_layer_that_has_it() {
    let history = HistorySnapshot {
        muxes: vec![mux_record("h_1", "$1")],
        ..Default::default()
    };
    let snapshot = world(vec![live_mux("$1")], history);
    let mut slot = slot(PinKind::Mux, Some("history:h_1".into()));
    slot.overrides = SlotFields::from([
        ("cwd".to_string(), text("/override/cwd")),
        ("worktree_branch".to_string(), text("feature/x")),
    ]);
    slot.snapshot = SlotFields::from([
        ("cwd".to_string(), text("/snapshot/cwd")),
        ("harness".to_string(), text("codex")),
        ("socket".to_string(), text("old-socket")),
    ]);
    let resolved = resolve_slot(&snapshot, &slot);

    assert_eq!(resolved.status, SlotStatus::Live);
    assert_eq!(
        field(&resolved, "cwd"),
        (text("/override/cwd"), FieldSource::Override)
    );
    assert_eq!(
        field(&resolved, "worktree_branch"),
        (text("feature/x"), FieldSource::Override)
    );
    assert_eq!(field(&resolved, "name"), (text("work"), FieldSource::Live));
    // The live harness is read from tmux's start command, above the
    // recorded and snapshot ones.
    assert_eq!(
        field(&resolved, "harness"),
        (text("claude-code"), FieldSource::Live)
    );
    // The snapshot only fills what nothing fresher knows.
    assert_eq!(
        field(&resolved, "socket"),
        (text("old-socket"), FieldSource::Snapshot)
    );
    // Exact launched tokens beat tmux's re-quoted rendering.
    assert_eq!(
        field(&resolved, "argv"),
        (list(&["claude", "--exact"]), FieldSource::History)
    );
}

#[test]
fn a_gone_mux_resolves_from_history_then_snapshot_then_defaults() {
    let mut record = mux_record("h_1", "$1");
    record.recipe.cwd = None;
    record.recipe.argv = None;
    let mut history = HistorySnapshot {
        muxes: vec![record],
        ..Default::default()
    };
    history
        .presence
        .insert("h_1".into(), NodePresence::Gone { last_seen: 100 });
    let snapshot = world(Vec::new(), history);
    let mut slot = slot(PinKind::Mux, Some("history:h_1".into()));
    slot.snapshot = SlotFields::from([("cwd".to_string(), text("/snapshot/cwd"))]);
    let resolved = resolve_slot(&snapshot, &slot);

    assert_eq!(resolved.status, SlotStatus::Gone);
    assert_eq!(
        resolved.target_node,
        Some(NodeId::MuxSession(MuxSessionId::new("tmux:work")))
    );
    assert_eq!(
        field(&resolved, "name"),
        (text("work"), FieldSource::History)
    );
    assert_eq!(
        field(&resolved, "cwd"),
        (text("/snapshot/cwd"), FieldSource::Snapshot)
    );
    // No recorded or live argv: the harness default.
    assert_eq!(
        field(&resolved, "argv"),
        (list(&["claude"]), FieldSource::Default)
    );
    assert_eq!(
        field(&resolved, "label"),
        (text("my-slot"), FieldSource::Default)
    );
}

#[test]
fn an_unprobed_mux_is_unknown_and_a_deleted_record_is_missing() {
    let history = HistorySnapshot {
        muxes: vec![mux_record("h_1", "$1")],
        ..Default::default()
    };
    let snapshot = world(Vec::new(), history);
    let unknown = resolve_slot(&snapshot, &slot(PinKind::Mux, Some("history:h_1".into())));
    assert_eq!(unknown.status, SlotStatus::Unknown);

    let missing = resolve_slot(
        &snapshot,
        &slot(PinKind::Mux, Some("history:h_gone".into())),
    );
    assert_eq!(missing.status, SlotStatus::Missing);
    assert_eq!(missing.target_node, None);
}

#[test]
fn a_dangling_slot_still_renders_from_its_snapshot() {
    let snapshot = world(Vec::new(), HistorySnapshot::default());
    let mut slot = slot(PinKind::Mux, Some("history:h_lost".into()));
    slot.snapshot = SlotFields::from([
        ("name".to_string(), text("lost")),
        ("label".to_string(), text("The lost one")),
    ]);
    let resolved = resolve_slot(&snapshot, &slot);
    assert_eq!(resolved.status, SlotStatus::Missing);
    assert_eq!(
        field(&resolved, "name"),
        (text("lost"), FieldSource::Snapshot)
    );
    assert_eq!(
        field(&resolved, "label"),
        (text("The lost one"), FieldSource::Snapshot)
    );
}

#[test]
fn a_planned_slot_is_planned_until_a_mux_of_its_name_runs() {
    let mut planned = slot(PinKind::Mux, None);
    planned.overrides = SlotFields::from([("name".to_string(), text("work"))]);
    let empty = world(Vec::new(), HistorySnapshot::default());
    let resolved = resolve_slot(&empty, &planned);
    assert_eq!(resolved.status, SlotStatus::Planned);
    assert_eq!(
        field(&resolved, "name"),
        (text("work"), FieldSource::Override)
    );

    let running = world(vec![live_mux("$1")], HistorySnapshot::default());
    let resolved = resolve_slot(&running, &planned);
    assert_eq!(resolved.status, SlotStatus::Live);
    assert_eq!(
        resolved.target_node,
        Some(NodeId::MuxSession(MuxSessionId::new("tmux:work")))
    );

    let nameless = slot(PinKind::Mux, None);
    assert_eq!(
        resolve_slot(&running, &nameless).status,
        SlotStatus::Planned
    );
}

#[test]
fn a_session_slot_follows_its_node() {
    let id = AgentSessionId::new("claude-code", "global", "e44d01cb");
    let mut session = AgentSessionNode::new(id.clone(), "claude-code");
    session.title = Some("Fix the ledger".into());
    let snapshot = world(
        vec![GraphNode::AgentSession(session)],
        HistorySnapshot::default(),
    );
    let mut slot = slot(
        PinKind::Session,
        Some(NodeId::AgentSession(id.clone()).to_string()),
    );
    slot.overrides = SlotFields::from([("resume_into".to_string(), text("work"))]);
    let resolved = resolve_slot(&snapshot, &slot);
    assert_eq!(resolved.status, SlotStatus::Live);
    assert_eq!(resolved.target_node, Some(NodeId::AgentSession(id)));
    assert_eq!(
        field(&resolved, "title"),
        (text("Fix the ledger"), FieldSource::Live)
    );
    assert_eq!(
        field(&resolved, "session_id"),
        (text("e44d01cb"), FieldSource::Live)
    );
    assert_eq!(
        field(&resolved, "resume_into"),
        (text("work"), FieldSource::Override)
    );

    let missing = resolve_slot(
        &world(Vec::new(), HistorySnapshot::default()),
        &PinSlot {
            target: Some("agent_session:claude-code:global:zzz".into()),
            ..slot
        },
    );
    assert_eq!(missing.status, SlotStatus::Missing);
}

#[test]
fn a_checkout_slot_resolves_from_history_when_its_worktree_is_gone() {
    let repo = RepoId::new("/src/app/.git");
    let record = CheckoutRecord {
        meta: meta("h_c"),
        recipe: CheckoutRecipe {
            repo: NodeId::Repo(repo.clone()).to_string(),
            root: "/wt/app.feature".into(),
            linked: true,
            branch: Some("feature/x".into()),
            head: Some("abc123".into()),
            upstream: None,
            extra: Default::default(),
        },
        extra: Default::default(),
    };
    let mut history = HistorySnapshot {
        checkouts: vec![record],
        ..Default::default()
    };
    history
        .presence
        .insert("h_c".into(), NodePresence::Gone { last_seen: 100 });
    let mut slot = slot(PinKind::Checkout, Some("history:h_c".into()));
    slot.overrides = SlotFields::from([("branch".to_string(), text("feature/y"))]);
    let resolved = resolve_slot(&world(Vec::new(), history), &slot);

    assert_eq!(resolved.status, SlotStatus::Gone);
    assert_eq!(
        resolved.target_node,
        Some(NodeId::Checkout(CheckoutId::new(repo, "/wt/app.feature")))
    );
    assert_eq!(
        field(&resolved, "branch"),
        (text("feature/y"), FieldSource::Override)
    );
    assert_eq!(
        field(&resolved, "head"),
        (text("abc123"), FieldSource::History)
    );
    assert_eq!(
        field(&resolved, "root"),
        (text("/wt/app.feature"), FieldSource::History)
    );
}

#[test]
fn a_live_checkout_reads_the_graph_first() {
    let repo = RepoId::new("/src/app/.git");
    let id = CheckoutId::new(repo, "/src/app");
    let mut node = CheckoutNode::new(id.clone(), "/src/app").with_worktree(WorktreeMeta::primary());
    node.head_commit = Some("live-head".into());
    let slot = slot(PinKind::Checkout, Some(NodeId::Checkout(id).to_string()));
    let resolved = resolve_slot(
        &world(vec![GraphNode::Checkout(node)], HistorySnapshot::default()),
        &slot,
    );
    assert_eq!(resolved.status, SlotStatus::Live);
    assert_eq!(
        field(&resolved, "head"),
        (text("live-head"), FieldSource::Live)
    );
}

#[test]
fn a_repo_slot_offers_its_remote_and_path() {
    let common_dir = "/src/old/.git";
    let record = RepoRecord {
        meta: meta("h_r"),
        recipe: RepoRecipe {
            common_dir: common_dir.into(),
            remotes: vec![RecordedRemote {
                name: "origin".into(),
                url: "git@github.com:me/old.git".into(),
            }],
            primary_root: Some("/src/old".into()),
            default_branch: None,
            extra: Default::default(),
        },
        extra: Default::default(),
    };
    let mut history = HistorySnapshot {
        repos: vec![record],
        ..Default::default()
    };
    history
        .presence
        .insert("h_r".into(), NodePresence::Gone { last_seen: 100 });
    let mut slot = slot(PinKind::Repo, Some("history:h_r".into()));
    slot.overrides = SlotFields::from([("path".to_string(), text("/src/elsewhere"))]);
    let resolved = resolve_slot(&world(Vec::new(), history), &slot);
    assert_eq!(resolved.status, SlotStatus::Gone);
    assert_eq!(
        field(&resolved, "remote"),
        (text("git@github.com:me/old.git"), FieldSource::History)
    );
    assert_eq!(
        field(&resolved, "path"),
        (text("/src/elsewhere"), FieldSource::Override)
    );
}

fn pr_id() -> ForgePrId {
    ForgePrId::new("github", "github.com", "me", "app", 7)
}

#[test]
fn a_pr_that_left_the_listing_lives_on_as_its_history_record() {
    let record = PrRecord {
        meta: RecordMeta {
            key: NodeId::ForgePr(pr_id()).to_string(),
            ..meta("h_p")
        },
        recipe: PrRecipe {
            provider: "github".into(),
            host: "github.com".into(),
            owner: "me".into(),
            repo: "app".into(),
            number: 7,
            title: Some("Ship it".into()),
            state: Some("merged".into()),
            ..Default::default()
        },
        extra: Default::default(),
    };
    let history = HistorySnapshot {
        prs: vec![record],
        ..Default::default()
    };
    let snapshot = world(Vec::new(), history);
    let by_node = slot(PinKind::Pr, Some(NodeId::ForgePr(pr_id()).to_string()));
    let by_record = slot(PinKind::Pr, Some("history:h_p".into()));
    for slot in [by_node, by_record] {
        let resolved = resolve_slot(&snapshot, &slot);
        assert_eq!(resolved.status, SlotStatus::Live);
        assert_eq!(
            field(&resolved, "state"),
            (text("merged"), FieldSource::History)
        );
        assert_eq!(
            field(&resolved, "label"),
            (text("#7 Ship it"), FieldSource::History)
        );
    }
}

#[test]
fn a_listed_pr_reads_the_graph() {
    let node = ForgePrNode {
        id: pr_id(),
        provider: "github".into(),
        host: "github.com".into(),
        owner: "me".into(),
        repo: "app".into(),
        number: 7,
        state: Some("open".into()),
        title: Some("Ship it".into()),
        url: None,
        updated_epoch: None,
        is_draft: false,
        head_ref: None,
    };
    let slot = slot(PinKind::Pr, Some(NodeId::ForgePr(pr_id()).to_string()));
    let resolved = resolve_slot(
        &world(vec![GraphNode::ForgePr(node)], HistorySnapshot::default()),
        &slot,
    );
    assert_eq!(field(&resolved, "state"), (text("open"), FieldSource::Live));
    assert_eq!(resolved.text("state"), Some("open"));
}

#[test]
fn a_repo_node_in_the_graph_is_live() {
    let node = RepoNode::new(RepoId::new("/src/app/.git"));
    let slot = slot(PinKind::Repo, Some("repo:/src/app/.git".into()));
    let resolved = resolve_slot(
        &world(vec![GraphNode::Repo(node)], HistorySnapshot::default()),
        &slot,
    );
    assert_eq!(resolved.status, SlotStatus::Live);
    assert_eq!(resolved.text("common_dir"), Some("/src/app/.git"));
}

fn with_slots(mut snapshot: GraphSnapshot, slots: Vec<PinSlot>) -> GraphSnapshot {
    snapshot.pins = slots;
    apply_slots(&mut snapshot);
    snapshot
}

fn pin_nodes(snapshot: &GraphSnapshot) -> Vec<&PinNode> {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Pin(pin) => Some(pin),
            _ => None,
        })
        .collect()
}

fn pin_links(snapshot: &GraphSnapshot) -> Vec<&GraphLink> {
    snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::PinTargets)
        .collect()
}

#[test]
fn every_slot_becomes_a_pin_node_with_a_targets_link_to_a_live_node() {
    let history = HistorySnapshot {
        muxes: vec![mux_record("h_m", "$7")],
        ..Default::default()
    };
    let snapshot = with_slots(
        world(vec![live_mux("$7")], history),
        vec![slot(PinKind::Mux, Some("history:h_m".into()))],
    );

    let [pin] = pin_nodes(&snapshot)[..] else {
        panic!("one pin node");
    };
    assert_eq!(pin.id.id, "my-slot");
    assert_eq!(pin.status, SlotStatus::Live);
    assert_eq!(pin.text("name"), Some("work"));
    assert_eq!(
        pin.target_node,
        Some(NodeId::MuxSession(MuxSessionId::new("tmux:work")))
    );
    let [link] = pin_links(&snapshot)[..] else {
        panic!("one pin_targets link");
    };
    assert_eq!(link.source, NodeId::Pin(PinId::new("my-slot")));
    assert!(matches!(
        &link.target,
        LinkEndpoint::Node { id } if *id == NodeId::MuxSession(MuxSessionId::new("tmux:work"))
    ));
    assert_eq!(link.source_metadata.adapter, PINS_ADAPTER);
}

#[test]
fn a_planned_slot_is_a_node_without_a_link() {
    let snapshot = with_slots(
        world(Vec::new(), HistorySnapshot::default()),
        vec![slot(PinKind::Mux, None)],
    );

    let [pin] = pin_nodes(&snapshot)[..] else {
        panic!("one pin node");
    };
    assert_eq!(pin.status, SlotStatus::Planned);
    assert_eq!(pin.target_node, None);
    assert!(pin_links(&snapshot).is_empty());
}

#[test]
fn applying_twice_does_not_duplicate_nodes_or_links() {
    let history = HistorySnapshot {
        muxes: vec![mux_record("h_m", "$7")],
        ..Default::default()
    };
    let mut snapshot = with_slots(
        world(vec![live_mux("$7")], history),
        vec![slot(PinKind::Mux, Some("history:h_m".into()))],
    );
    let once = snapshot.clone();

    apply_slots(&mut snapshot);

    assert_eq!(snapshot, once);
    assert_eq!(pin_nodes(&snapshot).len(), 1);
    assert_eq!(pin_links(&snapshot).len(), 1);
}

#[test]
fn removing_a_slot_removes_its_node_and_link_on_the_next_pass() {
    let history = HistorySnapshot {
        muxes: vec![mux_record("h_m", "$7")],
        ..Default::default()
    };
    let mut snapshot = with_slots(
        world(vec![live_mux("$7")], history),
        vec![slot(PinKind::Mux, Some("history:h_m".into()))],
    );
    snapshot.pins.clear();

    apply_slots(&mut snapshot);

    assert!(pin_nodes(&snapshot).is_empty());
    assert!(pin_links(&snapshot).is_empty());
    assert!(
        snapshot
            .node_provenance
            .keys()
            .all(|id| !matches!(id, NodeId::Pin(_)))
    );
}

#[test]
fn a_slot_for_a_gone_mux_links_to_its_ghost_node() {
    let history = HistorySnapshot {
        muxes: vec![mux_record("h_m", "$7")],
        ..Default::default()
    };
    // Nothing live and the mux unprobed: the slot is Unknown but still
    // names the node the history record stands for.
    let snapshot = with_slots(
        world(Vec::new(), history),
        vec![slot(PinKind::Mux, Some("history:h_m".into()))],
    );

    let [pin] = pin_nodes(&snapshot)[..] else {
        panic!("one pin node");
    };
    assert_eq!(pin.status, SlotStatus::Unknown);
    assert_eq!(pin.text("name"), Some("work"), "recipe comes from history");
    assert!(pin.target_node.is_some());
}

#[test]
fn a_dangling_slot_renders_from_its_snapshot_with_no_link() {
    let mut dangling = slot(PinKind::Mux, Some("history:h_deleted".into()));
    dangling.snapshot = SlotFields::from([
        ("name".to_string(), text("old")),
        ("label".to_string(), text("Old work")),
    ]);
    let snapshot = with_slots(
        world(Vec::new(), HistorySnapshot::default()),
        vec![dangling],
    );

    let [pin] = pin_nodes(&snapshot)[..] else {
        panic!("one pin node");
    };
    assert_eq!(pin.status, SlotStatus::Missing);
    assert_eq!(pin.label, "Old work");
    assert_eq!(pin.text("name"), Some("old"));
    assert!(pin_links(&snapshot).is_empty());
}

#[test]
fn a_pr_slot_projects_its_pr_node_from_history_once_it_leaves_the_listing() {
    let record = PrRecord {
        meta: RecordMeta {
            key: NodeId::ForgePr(pr_id()).to_string(),
            ..meta("h_p")
        },
        recipe: PrRecipe {
            provider: "github".into(),
            host: "github.com".into(),
            owner: "me".into(),
            repo: "app".into(),
            number: 7,
            title: Some("Ship it".into()),
            state: Some("merged".into()),
            ..Default::default()
        },
        extra: Default::default(),
    };
    let history = HistorySnapshot {
        prs: vec![record],
        ..Default::default()
    };
    let snapshot = with_slots(
        world(Vec::new(), history),
        vec![slot(
            PinKind::Pr,
            Some(NodeId::ForgePr(pr_id()).to_string()),
        )],
    );

    let projected = snapshot
        .find_node(&NodeId::ForgePr(pr_id()))
        .expect("PR node");
    let GraphNode::ForgePr(pr) = projected else {
        panic!("a PR node");
    };
    assert_eq!(pr.state.as_deref(), Some("merged"));
    assert_eq!(pr.title.as_deref(), Some("Ship it"));
    assert_eq!(pin_links(&snapshot).len(), 1);
}

#[test]
fn slots_keep_their_file_order_among_pin_nodes() {
    let mut a = slot(PinKind::Mux, None);
    a.id = "a".into();
    a.order = 1;
    let mut b = slot(PinKind::Mux, None);
    b.id = "b".into();
    b.order = 0;
    let snapshot = with_slots(world(Vec::new(), HistorySnapshot::default()), vec![a, b]);

    let mut pins = pin_nodes(&snapshot);
    pins.sort_by_key(|pin| pin.order);
    assert_eq!(
        pins.iter().map(|p| p.id.id.as_str()).collect::<Vec<_>>(),
        ["b", "a"]
    );
}
