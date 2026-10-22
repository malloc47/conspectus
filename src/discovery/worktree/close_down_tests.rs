// Extracted alongside close_down.rs — see #[path = "close_down_tests.rs"].
use super::*;
use crate::discovery::tmux::FakeTmux;
use crate::history::{HistoryStore, MuxObservation, Observations};
use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, CheckoutId, CheckoutNode, MuxRecipe, MuxSessionId,
    MuxSessionNode, NodeId, PinKind, PinNode, PinSlot, RecordDisplay, RecordOrigin, RepoId,
    SlotStatus, WorktreeMeta,
};
use std::sync::Mutex;

// ---- fakes ----

struct FakeSignaller {
    terminated: Mutex<Vec<i64>>,
}
impl FakeSignaller {
    fn new() -> Self {
        Self {
            terminated: Mutex::new(Vec::new()),
        }
    }
}
impl ProcessSignaller for FakeSignaller {
    fn terminate(&self, pid: i64) -> Result<()> {
        self.terminated.lock().unwrap().push(pid);
        Ok(())
    }
    // Report the process gone immediately so the grace poll exits fast.
    fn is_alive(&self, _pid: i64) -> bool {
        false
    }
    fn sleep(&self, _dur: Duration) {}
}

/// Records which mutation ran and returns a canned outcome.
struct RecordingBackend {
    outcome: WorktreeMutationOutcome,
    merges: Mutex<Vec<Option<String>>>,
    removes: Mutex<Vec<(String, bool)>>,
}
impl RecordingBackend {
    fn new(outcome: WorktreeMutationOutcome) -> Self {
        Self {
            outcome,
            merges: Mutex::new(Vec::new()),
            removes: Mutex::new(Vec::new()),
        }
    }
}
impl WorktreeBackend for RecordingBackend {
    fn backend_key(&self) -> &'static str {
        "recording"
    }
    fn capabilities(&self) -> crate::discovery::worktree::WorktreeCaps {
        crate::discovery::worktree::WorktreeCaps {
            can_create: true,
            can_remove: true,
            can_merge: true,
            can_prune: true,
        }
    }
    fn list(&self, _repo_root: &Path) -> Result<Vec<crate::discovery::worktree::WorktreeRecord>> {
        Ok(Vec::new())
    }
    fn merge(&self, req: &WorktreeMergeRequest) -> Result<WorktreeMutationOutcome> {
        self.merges.lock().unwrap().push(req.target.clone());
        Ok(self.outcome.clone())
    }
    fn remove(&self, req: &WorktreeRemoveRequest) -> Result<WorktreeMutationOutcome> {
        self.removes
            .lock()
            .unwrap()
            .push((req.branch.clone(), req.force));
        Ok(self.outcome.clone())
    }
}

// ---- snapshot fixtures ----

fn linked_checkout(repo: &str, path: &str, branch: &str) -> GraphNode {
    let repo_id = RepoId::new(repo);
    let mut c = CheckoutNode::new(CheckoutId::new(repo_id.clone(), path), path)
        .with_worktree(WorktreeMeta::linked());
    c.current_branch = Some(BranchId::new(repo_id, branch));
    GraphNode::Checkout(c)
}

/// A mux slot node pointing at the history record `record`.
fn slot_for(id: &str, record: &str) -> GraphNode {
    let mut pin = PinNode::for_test(id, PinKind::Mux, None, SlotStatus::Gone, &[("name", id)]);
    pin.target = Some(PinSlot::history_target(record));
    GraphNode::Pin(pin)
}

/// A mux record observed running in `cwd`.
fn mux_in(name: &str, cwd: &str) -> MuxObservation {
    MuxObservation {
        seen_at: 1_000,
        key: format!("mux_session:tmux:{name}"),
        recipe: MuxRecipe {
            backend: "tmux".into(),
            name: name.into(),
            cwd: Some(cwd.into()),
            ..Default::default()
        },
        display: RecordDisplay {
            label: name.into(),
            context: None,
        },
        origin: RecordOrigin::Observed,
    }
}

/// History holding one mux per `(name, cwd)`; returns each record's id
/// in the same order.
fn history_with(store: &HistoryStore, muxes: &[(&str, &str)]) -> Vec<String> {
    let (_, outcome) = store
        .record(&Observations {
            muxes: muxes.iter().map(|(name, cwd)| mux_in(name, cwd)).collect(),
            ..Default::default()
        })
        .unwrap();
    outcome.mux_ids
}

fn snapshot() -> GraphSnapshot {
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(linked_checkout(
        "/app/.git",
        "/wt/feature",
        "refs/heads/feature",
    ));
    snap.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(AgentSessionId::new("codex", "/s", "s1"), "codex")
            .with_cwd("/wt/feature/src".to_string()),
    ));
    snap.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(MuxSessionId::new("tmux:feat"), "tmux", "feat")
            .with_active_pane_current_path("/wt/feature".to_string())
            .with_active_pane_pid(4242),
    ));
    snap
}

// ---- planning ----

#[test]
fn plan_resolves_worktree_live_sessions_and_protected_records() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::at(dir.path());
    let ids = history_with(&store, &[("a", "/wt/feature"), ("b", "/x"), ("c", "/y")]);
    let mut snap = snapshot();
    snap.nodes.push(slot_for("p", &ids[0]));
    snap.nodes.push(slot_for("q", &ids[1]));
    // A slot that targets a live node protects no history record.
    snap.nodes.push(GraphNode::Pin(PinNode::for_test(
        "live",
        PinKind::Mux,
        Some(NodeId::MuxSession(MuxSessionId::new("tmux:feat"))),
        SlotStatus::Live,
        &[],
    )));

    let plan = plan_close_down(&snap, PathBuf::from("/app"), "feature").expect("plan");
    assert_eq!(plan.worktree_root, "/wt/feature");
    assert_eq!(plan.branch, "feature");
    assert!(plan.has_live());
    assert_eq!(
        plan.live_labels.len(),
        2,
        "agent + mux: {:?}",
        plan.live_labels
    );
    assert_eq!(plan.mux_targets.len(), 1);
    assert_eq!(plan.mux_targets[0].native_id, "feat");
    assert_eq!(plan.mux_targets[0].pane_pid, Some(4242));
    assert_eq!(
        plan.protected_records,
        BTreeSet::from([ids[0].clone(), ids[1].clone()])
    );
}

#[test]
fn plan_is_none_without_a_matching_worktree() {
    let snap = snapshot();
    assert!(plan_close_down(&snap, PathBuf::from("/app"), "nope").is_none());
}

// ---- execution ----

#[test]
fn discard_removes_worktree_kills_sessions_and_forgets_unpinned_history() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::at(dir.path());
    let ids = history_with(
        &store,
        &[
            ("scratch", "/wt/feature/src"),
            ("pinned", "/wt/feature"),
            ("elsewhere", "/other"),
        ],
    );
    let mut snap = snapshot();
    snap.nodes.push(slot_for("keep", &ids[1]));

    let plan = plan_close_down(&snap, PathBuf::from("/app"), "feature").expect("plan");
    let backend = RecordingBackend::new(WorktreeMutationOutcome::Succeeded { path: None });
    let mux = FakeTmux::with_sessions("");
    let sig = FakeSignaller::new();

    let report = execute_close_down(
        &plan,
        true, // discard
        None,
        &backend,
        &mux,
        &sig,
        Duration::from_secs(3),
        Some(&store),
    )
    .expect("ok");

    assert!(!report.landed);
    // Session terminated (SIGTERM + hard kill).
    assert_eq!(*sig.terminated.lock().unwrap(), vec![4242]);
    assert_eq!(mux.kill_calls(), vec![(None, "feat".to_string())]);
    // Removed (not merged), forced.
    assert_eq!(
        *backend.removes.lock().unwrap(),
        vec![("feature".to_string(), true)]
    );
    assert!(backend.merges.lock().unwrap().is_empty());
    // The record rooted in the worktree went; the pinned one and the
    // one elsewhere stayed.
    assert_eq!(report.forgotten_records, 1);
    assert!(report.history_error.is_none());
    let left: Vec<_> = store
        .load()
        .history
        .muxes
        .iter()
        .map(|record| record.recipe.name.clone())
        .collect();
    assert_eq!(left, ["pinned", "elsewhere"]);
}

#[test]
fn merge_lands_branch_and_passes_target() {
    let plan = plan_close_down(&snapshot(), PathBuf::from("/app"), "feature").expect("plan");
    let backend = RecordingBackend::new(WorktreeMutationOutcome::Succeeded { path: None });
    let mux = FakeTmux::with_sessions("");
    let sig = FakeSignaller::new();

    let report = execute_close_down(
        &plan,
        false, // merge
        Some("main".to_string()),
        &backend,
        &mux,
        &sig,
        Duration::from_secs(3),
        None,
    )
    .expect("ok");

    assert!(report.landed);
    assert_eq!(
        *backend.merges.lock().unwrap(),
        vec![Some("main".to_string())]
    );
    assert!(backend.removes.lock().unwrap().is_empty());
    assert_eq!(report.forgotten_records, 0);
}

#[test]
fn failed_worktree_step_leaves_history_alone() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::at(dir.path());
    history_with(&store, &[("scratch", "/wt/feature")]);
    let plan = plan_close_down(&snapshot(), PathBuf::from("/app"), "feature").expect("plan");
    let backend = RecordingBackend::new(WorktreeMutationOutcome::Failed {
        code: Some(1),
        message: "merge conflict".to_string(),
    });
    let mux = FakeTmux::with_sessions("");
    let sig = FakeSignaller::new();

    let report = execute_close_down(
        &plan,
        false,
        None,
        &backend,
        &mux,
        &sig,
        Duration::from_secs(3),
        Some(&store),
    )
    .expect("ok");

    // Sessions were still terminated, but no record was forgotten
    // because the merge failed: the stream is intact.
    assert_eq!(report.forgotten_records, 0);
    assert_eq!(store.load().history.muxes.len(), 1);
    assert!(matches!(
        report.worktree,
        WorktreeMutationOutcome::Failed { .. }
    ));
}
