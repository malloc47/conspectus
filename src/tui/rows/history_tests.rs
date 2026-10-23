use super::*;
use crate::model::{
    CheckoutRecipe, CheckoutRecord, GraphNode, HistorySnapshot, MuxRecipe, MuxRecord, NodePresence,
    PrRecipe, PrRecord, RecordDisplay, RecordMeta, RepoId, RepoNode, RepoRecipe, RepoRecord,
};

const NOW: i64 = 1_700_000_000;

fn meta(id: &str, key: &str, label: &str, last_seen: i64) -> RecordMeta {
    RecordMeta {
        id: id.into(),
        key: key.into(),
        first_seen: 1,
        last_seen,
        origin: Default::default(),
        display: RecordDisplay {
            label: label.into(),
            context: None,
        },
        rebuilt_from: None,
    }
}

fn mux(id: &str, name: &str, cwd: &str, last_seen: i64) -> MuxRecord {
    MuxRecord {
        meta: meta(id, &format!("mux_session:tmux:{name}"), name, last_seen),
        recipe: MuxRecipe {
            backend: "tmux".into(),
            name: name.into(),
            cwd: Some(cwd.into()),
            harness: Some("claude-code".into()),
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn world() -> GraphSnapshot {
    let mut repo = RepoNode::new(RepoId::new("/home/op/src/app/.git"));
    repo.source_paths = vec!["/home/op/src/app".into()];
    let history = HistorySnapshot {
        muxes: vec![
            mux("h_old", "old-agent", "/home/op/src/app", NOW - 86_400 * 3),
            mux("h_new", "new-agent", "/home/op/src/app", NOW - 600),
            mux("h_live", "live-agent", "/home/op/src/app", NOW - 10),
            mux("h_stray", "stray", "/tmp/elsewhere", NOW - 7_200),
            mux("h_unknown", "unsure", "/home/op/src/app", NOW - 60),
        ],
        checkouts: vec![CheckoutRecord {
            meta: meta(
                "h_wt",
                "checkout:repo:/home/op/src/app/.git@/home/op/src/app-feature",
                "app-feature",
                NOW - 3_600,
            ),
            recipe: CheckoutRecipe {
                repo: "repo:/home/op/src/app/.git".into(),
                root: "/home/op/src/app-feature".into(),
                linked: true,
                branch: Some("feature".into()),
                ..Default::default()
            },
            extra: Default::default(),
        }],
        repos: vec![RepoRecord {
            meta: meta("h_repo", "repo:/home/op/src/lost/.git", "lost", NOW - 5_000),
            recipe: RepoRecipe {
                common_dir: "/home/op/src/lost/.git".into(),
                primary_root: Some("/home/op/src/lost".into()),
                ..Default::default()
            },
            extra: Default::default(),
        }],
        prs: vec![PrRecord {
            meta: meta("h_pr", "forge_pr:github:github.com/o/r#9", "", NOW - 90_000),
            recipe: PrRecipe {
                provider: "github".into(),
                host: "github.com".into(),
                owner: "o".into(),
                repo: "r".into(),
                number: 9,
                title: Some("Ship it".into()),
                state: Some("merged".into()),
                ..Default::default()
            },
            extra: Default::default(),
        }],
        presence: std::collections::BTreeMap::from([
            (
                "h_old".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 86_400 * 3,
                },
            ),
            (
                "h_new".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 600,
                },
            ),
            (
                "h_stray".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 7_200,
                },
            ),
            (
                "h_unknown".to_string(),
                NodePresence::Unknown {
                    last_seen: NOW - 60,
                },
            ),
            (
                "h_wt".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 3_600,
                },
            ),
            (
                "h_repo".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 5_000,
                },
            ),
        ]),
    };
    GraphSnapshot {
        nodes: vec![GraphNode::Repo(repo)],
        history,
        ..GraphSnapshot::empty()
    }
}

fn build(snapshot: &GraphSnapshot, grouping: HistoryGrouping) -> RowTree {
    build_history_tree(HistoryBuildInputs {
        snapshot,
        home: Some(Path::new("/home/op")),
        now: Some(NOW),
        grouping,
        show_pinned: true,
    })
}

fn labels(tree: &RowTree) -> Vec<String> {
    tree.rows
        .iter()
        .map(|row| match &row.kind {
            RowKind::Mixed(m) => m.label.clone(),
            RowKind::Group(g) => format!("[{}]", g.display_path),
            _ => unreachable!(),
        })
        .collect()
}

#[test]
fn only_gone_records_are_listed_newest_sighting_first() {
    let snapshot = world();
    let tree = build(&snapshot, HistoryGrouping::None);
    assert_eq!(
        labels(&tree),
        [
            "new-agent",
            "app-feature",
            "lost",
            "stray",
            "#9",
            "old-agent"
        ]
    );
    assert!(
        tree.rows
            .iter()
            .all(|row| row.depth == 0 && !row.expandable),
        "flat, with nothing to expand"
    );
}

#[test]
fn rows_say_what_each_record_was() {
    let snapshot = world();
    let tree = build(&snapshot, HistoryGrouping::None);
    let rows: Vec<&MixedRow> = tree
        .rows
        .iter()
        .map(|row| match &row.kind {
            RowKind::Mixed(m) => m,
            _ => unreachable!(),
        })
        .collect();
    let mux = rows[0];
    assert_eq!(mux.node_kind, NodeKind::MuxSession);
    assert_eq!(mux.status, "stopped");
    assert_eq!(mux.tone, MixedTone::Dim);
    assert_eq!(mux.age.as_deref(), Some("10m"));
    assert_eq!(mux.chip, Some(MixedChip::agent("claude")));
    assert_eq!(mux.content, "~/src/app");
    let checkout = rows[1];
    assert_eq!(checkout.node_kind, NodeKind::Checkout);
    assert_eq!(checkout.content, "app · feature");
    let repo = rows[2];
    assert_eq!(repo.node_kind, NodeKind::Repo);
    assert_eq!(repo.content, "~/src/lost");
    let pr = rows[4];
    assert_eq!(pr.node_kind, NodeKind::ForgePr);
    assert_eq!(pr.status, "merged");
    assert_eq!(pr.content, "o/r · Ship it");
    assert_eq!(pr.pr_state.as_deref(), Some("merged"));
}

#[test]
fn project_grouping_puts_stray_records_in_other() {
    let snapshot = world();
    let tree = build(&snapshot, HistoryGrouping::Project);
    assert_eq!(
        labels(&tree),
        [
            "[~/src/app]",
            "new-agent",
            "app-feature",
            "old-agent",
            "[~/src/lost]",
            "lost",
            "[Other]",
            "stray",
            "#9",
        ]
    );
    assert!(matches!(tree.rows[0].id, RowId::Bucket(_)));
    assert_eq!(tree.rows[1].depth, 1);
}

#[test]
fn kind_grouping_follows_the_record_kinds() {
    let snapshot = world();
    let tree = build(&snapshot, HistoryGrouping::Kind);
    assert_eq!(
        labels(&tree),
        [
            "[Muxes]",
            "new-agent",
            "stray",
            "old-agent",
            "[Worktrees]",
            "app-feature",
            "[Repos]",
            "lost",
            "[PRs]",
            "#9",
        ]
    );
}

#[test]
fn rows_are_keyed_on_the_record_id_and_point_at_the_ghost_node() {
    let snapshot = world();
    let tree = build(&snapshot, HistoryGrouping::None);
    let RowId::History { record_id } = &tree.rows[0].id else {
        panic!("a history row");
    };
    assert_eq!(record_id, "h_new");
    let RowKind::Mixed(row) = &tree.rows[0].kind else {
        unreachable!()
    };
    assert_eq!(
        row.primary_node,
        NodeId::MuxSession(MuxSessionId::new("tmux:new-agent"))
    );
}

#[test]
fn a_pr_back_in_the_graph_or_an_unreadable_forge_is_not_gone() {
    let mut snapshot = world();
    snapshot
        .nodes
        .push(GraphNode::ForgePr(crate::model::ForgePrNode {
            id: crate::model::ForgePrId::new("github", "github.com", "o", "r", 9),
            provider: "github".into(),
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
            number: 9,
            state: Some("open".into()),
            title: None,
            url: None,
            updated_epoch: None,
            is_draft: false,
            head_ref: None,
        }));
    assert!(!labels(&build(&snapshot, HistoryGrouping::None)).contains(&"#9".to_string()));

    let mut snapshot = world();
    snapshot
        .diagnostics
        .push(crate::model::Diagnostic::ProviderUnavailable {
            provider: crate::discovery::providers::GITHUB.to_string(),
            reason: "gh missing".into(),
        });
    assert!(
        !labels(&build(&snapshot, HistoryGrouping::None)).contains(&"#9".to_string()),
        "an unread forge proves nothing about a PR"
    );
}

#[test]
fn an_empty_history_builds_an_empty_tree() {
    let snapshot = GraphSnapshot::empty();
    assert!(build(&snapshot, HistoryGrouping::Project).rows.is_empty());
}

#[test]
fn slots_pointing_at_a_record_are_found_by_id_or_key() {
    use crate::model::{PinKind, PinSlot, SlotFields};
    let mut snapshot = world();
    let slot = |id: &str, target: &str| PinSlot {
        id: id.into(),
        kind: PinKind::Mux,
        target: Some(target.into()),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    };
    snapshot.pins = vec![
        slot("by-id", "history:h_new"),
        slot("by-key", "mux_session:tmux:new-agent"),
        slot("other", "history:h_old"),
    ];
    assert_eq!(pinned_by(&snapshot, "h_new"), ["by-id", "by-key"]);
    assert!(pinned_by(&snapshot, "h_stray").is_empty());
}
