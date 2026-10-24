use super::*;
use crate::model::{
    CheckoutRecipe, CheckoutRecord, FieldValue, MuxRecipe, MuxRecord, PinKind, PrRecipe, PrRecord,
    RecordDisplay, RecordMeta, RepoRecipe, RepoRecord, SlotFields,
};

const DAY: i64 = 86_400;
const NOW: i64 = 100 * DAY;

fn meta(id: &str, key: &str, last_seen: i64) -> RecordMeta {
    RecordMeta {
        id: id.into(),
        key: key.into(),
        first_seen: 1,
        last_seen,
        origin: Default::default(),
        display: RecordDisplay {
            label: id.into(),
            context: None,
        },
        rebuilt_from: None,
    }
}

fn mux(id: &str, last_seen: i64) -> MuxRecord {
    MuxRecord {
        meta: meta(id, &format!("mux_session:tmux:{id}"), last_seen),
        recipe: MuxRecipe {
            backend: "tmux".into(),
            name: id.into(),
            cwd: Some(format!("/work/{id}")),
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn checkout(id: &str, root: &str, last_seen: i64) -> CheckoutRecord {
    CheckoutRecord {
        meta: meta(id, &format!("checkout:{id}"), last_seen),
        recipe: CheckoutRecipe {
            repo: "repo:/r/.git".into(),
            root: root.into(),
            linked: true,
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn repo(id: &str, last_seen: i64) -> RepoRecord {
    RepoRecord {
        meta: meta(id, &format!("repo:/{id}/.git"), last_seen),
        recipe: RepoRecipe {
            common_dir: format!("/{id}/.git"),
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn pr(id: &str, last_seen: i64) -> PrRecord {
    PrRecord {
        meta: meta(id, "forge_pr:github:github.com/o/r#1", last_seen),
        recipe: PrRecipe {
            provider: "github".into(),
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
            number: 1,
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn gone(last_seen: i64) -> NodePresence {
    NodePresence::Gone { last_seen }
}

fn snapshot(history: HistorySnapshot) -> GraphSnapshot {
    GraphSnapshot {
        history,
        ..GraphSnapshot::empty()
    }
}

fn slot(target: &str) -> PinSlot {
    PinSlot {
        id: "s".into(),
        kind: PinKind::Mux,
        target: Some(target.into()),
        pinned_at: None,
        overrides: SlotFields::from([("name".to_string(), FieldValue::Text("x".into()))]),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    }
}

fn ids(found: &[Candidate]) -> Vec<&str> {
    found.iter().map(|c| c.id.as_str()).collect()
}

fn find(snapshot: &GraphSnapshot) -> Vec<Candidate> {
    candidates(snapshot, &HistoryConfig::default(), NOW, None, None)
}

#[test]
fn a_gone_mux_qualifies_once_it_is_past_the_threshold() {
    let mut history = HistorySnapshot {
        muxes: vec![mux("old", 0), mux("recent", 0), mux("live", 0)],
        ..Default::default()
    };
    history.presence.insert("old".into(), gone(NOW - 15 * DAY));
    history
        .presence
        .insert("recent".into(), gone(NOW - 13 * DAY));

    let found = find(&snapshot(history));

    assert_eq!(
        ids(&found),
        ["old"],
        "14 days for muxes; live records have no presence entry"
    );
    assert_eq!(found[0].last_seen, NOW - 15 * DAY);
}

#[test]
fn the_threshold_is_inclusive_and_per_kind() {
    let mut history = HistorySnapshot {
        muxes: vec![mux("m", 0)],
        checkouts: vec![checkout("c", "/wt/c", 0)],
        ..Default::default()
    };
    history.presence.insert("m".into(), gone(NOW - 14 * DAY));
    history.presence.insert("c".into(), gone(NOW - 29 * DAY));

    let found = find(&snapshot(history));

    assert_eq!(ids(&found), ["m"], "a checkout waits 30 days");
}

#[test]
fn unknown_records_are_never_candidates() {
    let mut history = HistorySnapshot {
        muxes: vec![mux("m", 0)],
        ..Default::default()
    };
    history
        .presence
        .insert("m".into(), NodePresence::Unknown { last_seen: 0 });

    assert!(find(&snapshot(history)).is_empty());
}

#[test]
fn a_record_a_slot_targets_is_never_a_candidate() {
    let mut history = HistorySnapshot {
        muxes: vec![mux("pinned", 0), mux("loose", 0)],
        ..Default::default()
    };
    history.presence.insert("pinned".into(), gone(0));
    history.presence.insert("loose".into(), gone(0));
    let mut snap = snapshot(history);
    snap.pins = vec![slot("history:pinned")];

    assert_eq!(ids(&find(&snap)), ["loose"]);
}

#[test]
fn a_record_whose_node_a_slot_targets_is_protected_too() {
    let history = HistorySnapshot {
        prs: vec![pr("pr", 0)],
        ..Default::default()
    };
    let mut snap = snapshot(history);
    snap.pins = vec![slot("forge_pr:github:github.com/o/r#1")];

    assert!(find(&snap).is_empty());
}

#[test]
fn repos_have_no_default_threshold_but_older_than_applies_to_every_kind() {
    let mut history = HistorySnapshot {
        repos: vec![repo("r", 0)],
        muxes: vec![mux("m", 0)],
        ..Default::default()
    };
    history.presence.insert("r".into(), gone(NOW - 90 * DAY));
    history.presence.insert("m".into(), gone(NOW - 2 * DAY));
    let snap = snapshot(history);

    assert_eq!(ids(&find(&snap)), Vec::<&str>::new());

    let found = candidates(
        &snap,
        &HistoryConfig::default(),
        NOW,
        None,
        Some(Duration::from_secs(u64::try_from(DAY).unwrap())),
    );
    assert_eq!(ids(&found), ["r", "m"], "oldest first");
}

#[test]
fn the_kind_filter_limits_the_candidates() {
    let mut history = HistorySnapshot {
        muxes: vec![mux("m", 0)],
        checkouts: vec![checkout("c", "/wt/c", 0)],
        ..Default::default()
    };
    history.presence.insert("m".into(), gone(0));
    history.presence.insert("c".into(), gone(0));

    let found = candidates(
        &snapshot(history),
        &HistoryConfig::default(),
        NOW,
        Some(HistoryKind::Checkout),
        None,
    );

    assert_eq!(ids(&found), ["c"]);
}

#[test]
fn a_pr_qualifies_when_it_left_the_listing_long_enough_ago() {
    let history = HistorySnapshot {
        prs: vec![pr("stale", NOW - 31 * DAY)],
        ..Default::default()
    };
    let found = find(&snapshot(history));
    assert_eq!(ids(&found), ["stale"]);

    let history = HistorySnapshot {
        prs: vec![pr("fresh", NOW - 29 * DAY)],
        ..Default::default()
    };
    assert!(find(&snapshot(history)).is_empty());
}

#[test]
fn a_pr_still_in_the_graph_is_not_a_candidate() {
    let history = HistorySnapshot {
        prs: vec![pr("listed", 0)],
        ..Default::default()
    };
    let mut snap = snapshot(history);
    snap.nodes.push(crate::model::GraphNode::ForgePr(
        crate::model::ForgePrNode {
            id: crate::model::ForgePrId::new("github", "github.com", "o", "r", 1),
            provider: "github".into(),
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
            number: 1,
            state: None,
            title: None,
            url: None,
            updated_epoch: None,
            is_draft: false,
            head_ref: None,
        },
    ));

    assert!(find(&snap).is_empty());
}

#[test]
fn reap_removes_exactly_the_named_records() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::at(dir.path());
    store
        .update(|history| {
            history.muxes = vec![mux("a", 1), mux("b", 1), mux("c", 1)];
            crate::history::MergeOutcome::changed(HistoryKind::Mux)
        })
        .unwrap();

    let removed = reap(&store, &BTreeSet::from(["a".to_string(), "c".to_string()])).unwrap();

    assert_eq!(removed, 2);
    let left: Vec<_> = store
        .load()
        .history
        .muxes
        .iter()
        .map(|m| m.meta.id.clone())
        .collect();
    assert_eq!(left, ["b"]);
}

#[test]
fn forgetting_rooted_records_spares_the_protected_ones() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::at(dir.path());
    store
        .update(|history| {
            history.muxes = vec![mux("in", 1), mux("kept", 1), mux("elsewhere", 1)];
            history.muxes[0].recipe.cwd = Some("/wt/feature/sub".into());
            history.muxes[1].recipe.cwd = Some("/wt/feature".into());
            history.muxes[2].recipe.cwd = Some("/wt/other".into());
            history.checkouts = vec![checkout("co", "/wt/feature", 1)];
            let mut outcome = crate::history::MergeOutcome::changed(HistoryKind::Mux);
            outcome.changed.insert(HistoryKind::Checkout);
            outcome
        })
        .unwrap();

    let removed = forget_rooted_in(
        &store,
        Path::new("/wt/feature"),
        &BTreeSet::from(["kept".to_string()]),
    )
    .unwrap();

    assert_eq!(removed, 2, "the nested mux and the checkout");
    let history = store.load().history;
    let left: Vec<_> = history.muxes.iter().map(|m| m.meta.id.as_str()).collect();
    assert_eq!(left, ["kept", "elsewhere"]);
    assert!(history.checkouts.is_empty());
}
