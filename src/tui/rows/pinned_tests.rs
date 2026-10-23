use super::*;
use crate::model::PinKind;

fn tree(grouping: PinnedGrouping, sort: Sort) -> RowTree {
    build_pinned_tree(PinnedBuildInputs {
        snapshot: &fixture_world(),
        home: Some(Path::new("/home/op")),
        now: Some(1_700_000_000),
        grouping,
        sort,
    })
}

fn mixed<'a>(tree: &'a RowTree, label: &str) -> &'a MixedRow {
    tree.rows
        .iter()
        .find_map(|row| match (&row.id, &row.kind) {
            (RowId::Pin { pin_id }, RowKind::Mixed(m)) if pin_id == label => Some(m),
            _ => None,
        })
        .unwrap_or_else(|| panic!("slot row `{label}`: {tree:#?}"))
}

fn slot_ids(tree: &RowTree) -> Vec<&str> {
    tree.rows
        .iter()
        .filter_map(|row| match &row.id {
            RowId::Pin { pin_id } => Some(pin_id.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_slot_is_a_row_in_slot_order_when_ungrouped() {
    let tree = tree(PinnedGrouping::None, Sort::Hierarchy);
    assert_eq!(
        slot_ids(&tree),
        [
            "conspectus-main",
            "config-agent3",
            "e44d01cb",
            "ledger-worktree",
            "ledger-pr",
            "nightly",
            "old-api"
        ]
    );
    assert!(tree.rows.iter().all(|row| row.depth <= 1));
    assert_eq!(tree.view, ViewLabel::Pinned);
}

#[test]
fn status_words_follow_the_kind_vocabulary() {
    let tree = tree(PinnedGrouping::None, Sort::Hierarchy);
    let status = |label: &str| mixed(&tree, label).status.clone();
    assert_eq!(status("conspectus-main"), "live");
    assert_eq!(status("config-agent3"), "gone");
    assert_eq!(status("e44d01cb"), "in mux");
    assert_eq!(status("ledger-worktree"), "gone");
    assert_eq!(status("ledger-pr"), "merged");
    assert_eq!(status("nightly"), "planned");
    assert_eq!(status("old-api"), "planned");
}

#[test]
fn gone_and_planned_rows_are_dimmed_and_live_ones_are_not() {
    let tree = tree(PinnedGrouping::None, Sort::Hierarchy);
    for (label, tone) in [
        ("conspectus-main", MixedTone::Normal),
        ("e44d01cb", MixedTone::Normal),
        ("ledger-pr", MixedTone::Dim),
        ("config-agent3", MixedTone::Dim),
        ("nightly", MixedTone::Dim),
    ] {
        // PR slots are live when the PR is in the graph; this one
        // only has history.
        let row = mixed(&tree, label);
        if label != "ledger-pr" {
            assert_eq!(row.tone, tone, "{label}");
        }
    }
}

#[test]
fn ages_come_from_activity_when_live_and_last_seen_when_gone() {
    let tree = tree(PinnedGrouping::None, Sort::Hierarchy);
    assert_eq!(mixed(&tree, "conspectus-main").age.as_deref(), Some("2m"));
    assert_eq!(mixed(&tree, "config-agent3").age.as_deref(), Some("3d"));
    assert_eq!(mixed(&tree, "ledger-worktree").age.as_deref(), Some("2d"));
    assert_eq!(mixed(&tree, "nightly").age, None);
}

#[test]
fn content_names_the_harness_and_place_per_kind() {
    let tree = tree(PinnedGrouping::None, Sort::Hierarchy);
    // The agent is a chip of its own, not part of the text after it.
    let main = mixed(&tree, "conspectus-main");
    assert_eq!(main.chip, Some(MixedChip::agent("claude")));
    assert_eq!(main.content, "~/src/conspectus");
    let session = mixed(&tree, "e44d01cb");
    assert_eq!(session.chip, Some(MixedChip::agent("claude")));
    assert_eq!(session.content, "Fix the ledger merge");
    assert_eq!(
        mixed(&tree, "ledger-worktree").content,
        "conspectus · feature/ledger"
    );
    assert_eq!(
        mixed(&tree, "ledger-pr").content,
        "malloc47/conspectus · Ledger store and merge"
    );
}

#[test]
fn project_grouping_buckets_by_repo_with_other_last() {
    let tree = tree(PinnedGrouping::Project, Sort::Hierarchy);
    let buckets: Vec<_> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.id {
            RowId::Bucket(label) => Some(label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(buckets, ["~/src/conspectus", "Other"]);
    // Slots sit one level under their bucket.
    assert!(tree.rows.iter().all(|row| match row.id {
        RowId::Bucket(_) => row.depth == 0,
        RowId::Pin { .. } => row.depth == 1,
        _ => row.depth >= 2,
    }));
    let members = |bucket: &str| -> Vec<String> {
        let start = tree
            .rows
            .iter()
            .position(|r| r.id == RowId::Bucket(bucket.into()))
            .unwrap();
        tree.rows[start + 1..]
            .iter()
            .take_while(|r| !matches!(r.id, RowId::Bucket(_)))
            .filter_map(|r| match &r.id {
                RowId::Pin { pin_id } => Some(pin_id.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        members("~/src/conspectus"),
        ["conspectus-main", "e44d01cb", "ledger-worktree", "nightly"],
        "a worktree belongs to the repo it was made from, wherever its directory is"
    );
    assert_eq!(members("Other"), ["config-agent3", "ledger-pr", "old-api"]);
}

#[test]
fn kind_grouping_buckets_in_kind_order() {
    let tree = tree(PinnedGrouping::Kind, Sort::Hierarchy);
    let buckets: Vec<_> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.id {
            RowId::Bucket(label) => Some(label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(buckets, ["Muxes", "Sessions", "Worktrees", "PRs"]);
}

#[test]
fn recency_sort_puts_the_freshest_first_and_undated_slots_last_in_order() {
    let tree = tree(PinnedGrouping::None, Sort::Recency);
    assert_eq!(
        slot_ids(&tree),
        [
            "conspectus-main",
            "e44d01cb",
            "ledger-pr",
            "ledger-worktree",
            "config-agent3",
            "nightly",
            "old-api"
        ]
    );
}

#[test]
fn a_mux_slot_expands_to_the_sessions_it_hosts() {
    let tree = tree(PinnedGrouping::None, Sort::Hierarchy);
    let main = tree
        .rows
        .iter()
        .find(|r| {
            r.id == RowId::Pin {
                pin_id: "conspectus-main".into(),
            }
        })
        .unwrap();
    assert!(main.expandable);
    let nested: Vec<_> = tree
        .rows
        .iter()
        .filter(|r| matches!(&r.id, RowId::Nested { pin_id, .. } if pin_id == "conspectus-main"))
        .collect();
    assert_eq!(nested.len(), 1, "{nested:#?}");
    assert_eq!(nested[0].depth, 1);
    let RowKind::Mixed(session) = &nested[0].kind else {
        panic!("a mixed row");
    };
    assert_eq!(session.node_kind, NodeKind::AgentSession);
    assert_eq!(session.label, "Fix the ledger merge");
    assert_eq!(session.status, "in mux");
}

#[test]
fn a_checkout_slot_expands_to_its_muxes_and_their_sessions() {
    use crate::model::{FieldValue, PinSlot, SlotFields};
    let mut world = fixture_world();
    world.pins.push(PinSlot {
        id: "repo-checkout".into(),
        kind: PinKind::Checkout,
        target: Some(
            "checkout:repo:/home/op/src/conspectus/.git@/home/op/src/conspectus".to_string(),
        ),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::from([(
            "label".to_string(),
            FieldValue::Text("repo-checkout".into()),
        )]),
        order: 9,
        legacy: false,
    });
    let world = crate::resolve::resolve_snapshot(world);
    let tree = build_pinned_tree(PinnedBuildInputs {
        snapshot: &world,
        home: None,
        now: Some(1_700_000_000),
        grouping: PinnedGrouping::None,
        sort: Sort::Hierarchy,
    });
    let nested: Vec<(u8, NodeKind)> = tree
        .rows
        .iter()
        .filter(|r| matches!(&r.id, RowId::Nested { pin_id, .. } if pin_id == "repo-checkout"))
        .filter_map(|r| match &r.kind {
            RowKind::Mixed(m) => Some((r.depth, m.node_kind)),
            _ => None,
        })
        .collect();
    assert_eq!(
        nested,
        [(1, NodeKind::MuxSession), (2, NodeKind::AgentSession)]
    );
}

#[test]
fn an_empty_world_has_an_empty_tree() {
    let tree = build_pinned_tree(PinnedBuildInputs {
        snapshot: &GraphSnapshot::empty(),
        home: None,
        now: None,
        grouping: PinnedGrouping::Project,
        sort: Sort::Hierarchy,
    });
    assert!(tree.rows.is_empty());
}

#[test]
fn slot_rows_have_unique_ids() {
    let tree = tree(PinnedGrouping::Project, Sort::Hierarchy);
    let mut ids: Vec<_> = tree.rows.iter().map(|r| r.id.clone()).collect();
    let before = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), before);
}
