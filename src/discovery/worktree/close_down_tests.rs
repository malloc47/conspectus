// Extracted alongside close_down.rs — see #[path = "close_down_tests.rs"].
use super::*;
use crate::discovery::tmux::FakeTmux;
use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, CheckoutId, CheckoutNode, MuxSessionId,
    MuxSessionNode, PinId, PinMuxRef, PinNode, Provenance, RepoId, WorktreeMeta,
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

fn pin(id: &str, cwd: &str, store: &str) -> GraphNode {
    GraphNode::Pin(PinNode {
        id: PinId::new(id),
        display_name: id.to_string(),
        harness: "codex".to_string(),
        cwd: cwd.to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: id.to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: store.to_string(),
        binding: None,
    })
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
fn plan_resolves_worktree_live_sessions_and_pins() {
    let mut snap = snapshot();
    snap.nodes
        .push(pin("p", "/wt/feature/src", "/wt/feature/.conspectus.toml"));
    snap.nodes
        .push(pin("out", "/elsewhere", "/elsewhere/.conspectus.toml"));

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
        plan.pins.len(),
        1,
        "only the in-worktree pin: {:?}",
        plan.pins
    );
    assert_eq!(plan.pins[0].id, "p");
}

#[test]
fn plan_is_none_without_a_matching_worktree() {
    let snap = snapshot();
    assert!(plan_close_down(&snap, PathBuf::from("/app"), "nope").is_none());
}

// ---- execution ----

#[test]
fn discard_removes_worktree_kills_sessions_and_drops_pins() {
    let mut snap = snapshot();
    snap.nodes
        .push(pin("p", "/wt/feature", "/wt/feature/.conspectus.toml"));
    // Point the pin store at a real temp file so the drop can write.
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join(".conspectus.toml");
    std::fs::write(
        &store,
        "[pins]\nschema_version = 1\n\n[[pins.entry]]\nid = \"p\"\ndisplay_name = \"p\"\nharness = \"codex\"\ncwd = \"/wt/feature\"\n\n[pins.entry.mux]\nbackend = \"tmux\"\nname = \"p\"\n",
    )
    .unwrap();
    // Replace the pin's store_path with the real file.
    if let GraphNode::Pin(p) = snap.nodes.last_mut().unwrap() {
        p.store_path = store.to_string_lossy().to_string();
    }

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
    // Pin dropped.
    assert_eq!(report.dropped_pins, vec!["p".to_string()]);
    assert!(report.pin_errors.is_empty());
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
    )
    .expect("ok");

    assert!(report.landed);
    assert_eq!(
        *backend.merges.lock().unwrap(),
        vec![Some("main".to_string())]
    );
    assert!(backend.removes.lock().unwrap().is_empty());
}

#[test]
fn failed_worktree_step_skips_pin_drop() {
    let mut snap = snapshot();
    snap.nodes
        .push(pin("p", "/wt/feature", "/nonexistent/.conspectus.toml"));
    let plan = plan_close_down(&snap, PathBuf::from("/app"), "feature").expect("plan");
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
    )
    .expect("ok");

    // Sessions were still terminated, but no pin was touched because the
    // merge failed — the stream is intact.
    assert!(report.dropped_pins.is_empty());
    assert!(report.pin_errors.is_empty());
    assert!(matches!(
        report.worktree,
        WorktreeMutationOutcome::Failed { .. }
    ));
}
