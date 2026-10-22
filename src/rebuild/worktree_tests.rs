use std::path::Path;

use super::super::resolve_subject;
use super::super::test_support::{FakeWorktrees, Harness, titles};
use super::*;
use crate::discovery::tmux::FakeTmux;
use crate::model::{
    BranchId, BranchNode, CheckoutId, CheckoutRecipe, CheckoutRecord, FieldValue, HistorySnapshot,
    NodePresence, RecordMeta, RecordedRemote, RepoId, RepoRecipe, RepoRecord,
};

pub(super) const NOW: i64 = 1_790_000_000;

fn meta(id: &str) -> RecordMeta {
    RecordMeta {
        id: id.into(),
        key: String::new(),
        first_seen: 1,
        last_seen: NOW - 3 * 86_400,
        origin: Default::default(),
        display: Default::default(),
        rebuilt_from: None,
    }
}

/// A repo on disk at `<tmp>/app`, and a recorded linked worktree of it
/// at `<tmp>/wt-feature` that is gone.
pub(super) struct World {
    pub tmp: tempfile::TempDir,
    pub snapshot: GraphSnapshot,
}

pub(super) fn checkout_record(tmp: &Path, linked: bool) -> CheckoutRecord {
    CheckoutRecord {
        meta: meta("h_wt"),
        recipe: CheckoutRecipe {
            repo: format!("repo:{}", tmp.join("app/.git").display()),
            root: if linked {
                tmp.join("wt-feature").display().to_string()
            } else {
                tmp.join("app").display().to_string()
            },
            linked,
            branch: Some("feature/x".into()),
            head: Some("abc123".into()),
            upstream: None,
            extra: Default::default(),
        },
        extra: Default::default(),
    }
}

pub(super) fn world(branch_exists: bool) -> World {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("app/.git")).unwrap();
    let repo = RepoId::new(tmp.path().join("app/.git").display().to_string());
    let mut nodes = vec![GraphNode::Checkout(
        crate::model::CheckoutNode::new(
            CheckoutId::new(repo.clone(), tmp.path().join("app").display().to_string()),
            tmp.path().join("app").display().to_string(),
        )
        .with_worktree(crate::model::WorktreeMeta::primary()),
    )];
    if branch_exists {
        nodes.push(GraphNode::Branch(BranchNode {
            id: BranchId::new(repo, "refs/heads/feature/x"),
            refname: "refs/heads/feature/x".into(),
            current_commit: None,
            upstream: None,
        }));
    }
    let mut history = HistorySnapshot {
        checkouts: vec![checkout_record(tmp.path(), true)],
        ..Default::default()
    };
    history.presence.insert(
        "h_wt".into(),
        NodePresence::Gone {
            last_seen: NOW - 3 * 86_400,
        },
    );
    let snapshot = GraphSnapshot {
        nodes,
        history,
        ..GraphSnapshot::empty()
    };
    World { tmp, snapshot }
}

fn harness(world: &World, lands_at: &str) -> Harness {
    Harness::new(FakeTmux::with_sessions(""))
        .with_worktrees(FakeWorktrees::landing_at(&world.tmp.path().join(lands_at)))
}

fn plan_for(h: &Harness, w: &World) -> Result<Plan, RebuildError> {
    let subject = resolve_subject(&w.snapshot, &[], "h_wt").unwrap();
    h.plan(&w.snapshot, &subject, &Options::default())
}

#[test]
fn a_gone_worktree_whose_branch_exists_is_switched_to() {
    let w = world(true);
    let h = harness(&w, "wt-feature");
    let plan = plan_for(&h, &w).unwrap();

    assert_eq!(plan.subject, "worktree `wt-feature` (gone)");
    assert_eq!(titles(&plan), ["rebuild worktree feature/x"]);
    assert!(
        plan.steps[0].command.ends_with("switch --no-cd feature/x"),
        "{}",
        plan.steps[0].command
    );

    let executed = h.run(&plan).unwrap();
    let switches = h
        .worktree
        .as_ref()
        .unwrap()
        .switches
        .lock()
        .unwrap()
        .clone();
    assert_eq!(switches.len(), 1);
    assert!(!switches[0].create && switches[0].base.is_none());
    assert_eq!(
        executed.landed[&w.tmp.path().join("wt-feature").display().to_string()],
        w.tmp.path().join("wt-feature")
    );
    // Same place: the record comes back to life, no link to an old one.
    let load = h.writer.store.load();
    assert_eq!(load.history.checkouts.len(), 1);
    assert!(load.history.checkouts[0].meta.rebuilt_from.is_none());
}

#[test]
fn a_deleted_branch_is_recreated_from_the_last_head() {
    let w = world(false);
    let h = harness(&w, "wt-feature");
    let plan = plan_for(&h, &w).unwrap();
    assert_eq!(titles(&plan), ["recreate branch feature/x"]);
    assert!(
        plan.steps[0]
            .command
            .ends_with("switch --no-cd --create --base abc123 feature/x"),
        "{}",
        plan.steps[0].command
    );
    h.run(&plan).unwrap();
    let switches = h
        .worktree
        .as_ref()
        .unwrap()
        .switches
        .lock()
        .unwrap()
        .clone();
    assert!(switches[0].create);
    assert_eq!(switches[0].base.as_deref(), Some("abc123"));
}

#[test]
fn a_worktree_that_lands_elsewhere_links_the_records_and_moves_the_slots() {
    let w = world(true);
    let slots = crate::pin_slots::SlotStore::at(w.tmp.path().join("pins.toml"));
    slots
        .add(crate::model::PinSlot {
            id: "ledger".into(),
            kind: crate::model::PinKind::Checkout,
            target: Some("history:h_wt".into()),
            pinned_at: None,
            overrides: Default::default(),
            snapshot: Default::default(),
            order: 0,
            legacy: false,
        })
        .unwrap();
    let h = harness(&w, "elsewhere/app.feature-x").with_slots(slots.clone());
    let plan = plan_for(&h, &w).unwrap();
    let executed = h.run(&plan).unwrap();

    let load = h.writer.store.load();
    let new = load
        .history
        .checkouts
        .iter()
        .find(|r| r.recipe.root.ends_with("elsewhere/app.feature-x"))
        .expect("a record for the new place");
    assert_eq!(new.meta.rebuilt_from.as_deref(), Some("h_wt"));
    assert_eq!(
        slots.load().slots[0].target,
        Some(format!("history:{}", new.meta.id)),
        "the slot follows the worktree"
    );
    assert_eq!(
        executed.retargeted,
        vec![("ledger".to_string(), new.meta.id.clone())]
    );
    assert!(
        executed
            .notes
            .iter()
            .any(|n| n.contains("instead of its recorded place")),
        "{:?}",
        executed.notes
    );
}

#[test]
fn a_worktree_in_a_deleted_repo_clones_the_repo_first() {
    let mut w = world(true);
    // The repo is gone: nothing at its common dir, no live checkout.
    std::fs::remove_dir_all(w.tmp.path().join("app")).unwrap();
    w.snapshot.nodes.clear();
    let common_dir = w.tmp.path().join("app/.git").display().to_string();
    w.snapshot.history.repos.push(RepoRecord {
        meta: RecordMeta {
            key: format!("repo:{common_dir}"),
            ..meta("h_repo")
        },
        recipe: RepoRecipe {
            common_dir,
            remotes: vec![RecordedRemote {
                name: "origin".into(),
                url: "git@github.com:me/app.git".into(),
            }],
            primary_root: Some(w.tmp.path().join("app").display().to_string()),
            default_branch: None,
            extra: Default::default(),
        },
        extra: Default::default(),
    });
    w.snapshot
        .history
        .presence
        .insert("h_repo".into(), NodePresence::Gone { last_seen: 1 });

    let h = harness(&w, "wt-feature");
    let plan = plan_for(&h, &w).unwrap();
    assert_eq!(
        titles(&plan),
        ["clone the repo", "rebuild worktree feature/x"]
    );
    h.run(&plan).unwrap();
    let clones = h.cloner.calls.lock().unwrap().clone();
    assert_eq!(
        clones,
        vec![(
            "git@github.com:me/app.git".to_string(),
            w.tmp.path().join("app")
        )]
    );
    // The branch comes from the clone, so it isn't created.
    let switches = h
        .worktree
        .as_ref()
        .unwrap()
        .switches
        .lock()
        .unwrap()
        .clone();
    assert!(!switches[0].create);
    assert_eq!(switches[0].repo_root, w.tmp.path().join("app"));
}

#[test]
fn a_gone_primary_checkout_is_the_repo_cloned_into_its_root() {
    let mut w = world(true);
    std::fs::remove_dir_all(w.tmp.path().join("app")).unwrap();
    w.snapshot.nodes.clear();
    w.snapshot.history.checkouts = vec![checkout_record(w.tmp.path(), false)];
    let common_dir = w.tmp.path().join("app/.git").display().to_string();
    w.snapshot.history.repos.push(RepoRecord {
        meta: RecordMeta {
            key: format!("repo:{common_dir}"),
            ..meta("h_repo")
        },
        recipe: RepoRecipe {
            common_dir,
            remotes: vec![RecordedRemote {
                name: "origin".into(),
                url: "git@github.com:me/app.git".into(),
            }],
            primary_root: None,
            default_branch: None,
            extra: Default::default(),
        },
        extra: Default::default(),
    });
    let h = harness(&w, "unused");
    let plan = plan_for(&h, &w).unwrap();
    assert_eq!(titles(&plan), ["clone the repo"]);
    h.run(&plan).unwrap();
    assert_eq!(
        h.cloner.calls.lock().unwrap()[0].1,
        w.tmp.path().join("app")
    );
}

#[test]
fn a_live_worktree_has_nothing_to_rebuild() {
    let w = world(true);
    std::fs::create_dir_all(w.tmp.path().join("wt-feature")).unwrap();
    let mut w = w;
    w.snapshot.history.presence.clear();
    let h = harness(&w, "wt-feature");
    let plan = plan_for(&h, &w).unwrap();
    assert!(plan.steps.is_empty());
    assert!(
        plan.warnings[0].contains("nothing to rebuild"),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn rebuilding_needs_a_worktree_backend() {
    let w = world(true);
    let h = Harness::new(FakeTmux::with_sessions(""));
    let err = plan_for(&h, &w).unwrap_err();
    assert!(err.to_string().contains("install worktrunk"), "{err}");
}

#[test]
fn a_failed_switch_reports_the_backends_message() {
    let w = world(true);
    let h = harness(&w, "wt-feature");
    *h.worktree.as_ref().unwrap().fail_with.lock().unwrap() = Some("branch is checked out".into());
    let plan = plan_for(&h, &w).unwrap();
    let failure = h.run(&plan).unwrap_err();
    assert_eq!(failure.step, 1);
    assert!(
        failure.to_string().contains("branch is checked out"),
        "{failure}"
    );
}

#[test]
fn a_location_override_is_called_out_as_advisory() {
    let w = world(true);
    let mut subject = resolve_subject(&w.snapshot, &[], "h_wt").unwrap();
    subject
        .slot
        .overrides
        .insert("location".into(), FieldValue::Text("/elsewhere".into()));
    let h = harness(&w, "wt-feature");
    let plan = h.plan(&w.snapshot, &subject, &Options::default()).unwrap();
    assert!(
        plan.warnings[0].contains("may not be honored"),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn a_branch_override_rebuilds_that_branch() {
    let w = world(true);
    let mut subject = resolve_subject(&w.snapshot, &[], "h_wt").unwrap();
    subject
        .slot
        .overrides
        .insert("branch".into(), FieldValue::Text("feature/y".into()));
    let h = harness(&w, "wt-feature");
    let plan = h.plan(&w.snapshot, &subject, &Options::default()).unwrap();
    assert_eq!(
        titles(&plan),
        ["recreate branch feature/y"],
        "feature/y has no branch node"
    );
}
