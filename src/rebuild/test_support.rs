//! Fakes shared by the rebuild executors' tests.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;

use crate::discovery::tmux::{FakeTmux, MuxBackend};
use crate::discovery::worktree::{
    WorktreeBackend, WorktreeCaps, WorktreeMutationOutcome, WorktreeRecord, WorktreeSwitchRequest,
};
use crate::history::{HistoryStore, HistoryWriter};
use crate::model::GraphSnapshot;
use crate::model::WorktreeKind;
use crate::pin_slots::SlotStore;

use super::{Env, Executed, Options, Plan, RebuildError, RepoCloner, RunFailure, Subject};

/// Records the clones it was asked for and creates the destination.
#[derive(Clone, Default)]
pub struct RecordingCloner {
    pub calls: Arc<Mutex<Vec<(String, PathBuf)>>>,
}

impl RepoCloner for RecordingCloner {
    fn clone_repo(&self, remote: &str, dest: &Path) -> Result<()> {
        std::fs::create_dir_all(dest)?;
        self.calls
            .lock()
            .unwrap()
            .push((remote.to_string(), dest.to_path_buf()));
        Ok(())
    }
}

/// A worktree backend that answers `switch` by creating a directory at
/// `lands_at` (or the next free place) and listing it afterwards.
#[derive(Clone)]
pub struct FakeWorktrees {
    pub switches: Arc<Mutex<Vec<WorktreeSwitchRequest>>>,
    pub lands_at: Arc<Mutex<Option<PathBuf>>>,
    pub head: String,
    pub fail_with: Arc<Mutex<Option<String>>>,
}

impl FakeWorktrees {
    pub fn landing_at(path: &Path) -> Self {
        Self {
            switches: Arc::default(),
            lands_at: Arc::new(Mutex::new(Some(path.to_path_buf()))),
            head: "abc123".to_string(),
            fail_with: Arc::default(),
        }
    }
}

impl WorktreeBackend for FakeWorktrees {
    fn backend_key(&self) -> &'static str {
        "fake"
    }

    fn capabilities(&self) -> WorktreeCaps {
        WorktreeCaps {
            can_create: true,
            can_remove: true,
            can_merge: false,
            can_prune: false,
        }
    }

    fn list(&self, _repo_root: &Path) -> Result<Vec<WorktreeRecord>> {
        let switches = self.switches.lock().unwrap();
        let (Some(last), Some(path)) = (switches.last(), self.lands_at.lock().unwrap().clone())
        else {
            return Ok(Vec::new());
        };
        Ok(vec![WorktreeRecord {
            path: path.display().to_string(),
            branch: Some(format!("refs/heads/{}", last.branch)),
            head: Some(self.head.clone()),
            kind: WorktreeKind::Linked,
            locked: None,
            prunable: None,
            bare: false,
            detached: false,
        }])
    }

    fn switch(&self, req: &WorktreeSwitchRequest) -> Result<WorktreeMutationOutcome> {
        if let Some(message) = self.fail_with.lock().unwrap().clone() {
            return Ok(WorktreeMutationOutcome::Failed {
                code: Some(1),
                message,
            });
        }
        if let Some(path) = self.lands_at.lock().unwrap().as_ref() {
            std::fs::create_dir_all(path)?;
        }
        self.switches.lock().unwrap().push(req.clone());
        Ok(WorktreeMutationOutcome::Succeeded { path: None })
    }
}

/// Everything a plan and a run need, over fakes.
pub struct Harness {
    pub tmux: FakeTmux,
    _dir: tempfile::TempDir,
    pub writer: HistoryWriter,
    pub worktree: Option<FakeWorktrees>,
    pub cloner: RecordingCloner,
    pub slots: Option<SlotStore>,
    pub launch: crate::launch_default::LaunchDefaults,
}

impl Harness {
    pub fn new(tmux: FakeTmux) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let writer = HistoryWriter {
            store: HistoryStore::at(dir.path().join("history")),
            config: Default::default(),
            home: None,
        };
        Self {
            tmux,
            _dir: dir,
            writer,
            worktree: None,
            cloner: RecordingCloner::default(),
            slots: None,
            launch: crate::launch_default::LaunchDefaults::default(),
        }
    }

    pub fn with_launch(mut self, launch: crate::launch_default::LaunchDefaults) -> Self {
        self.launch = launch;
        self
    }

    pub fn with_worktrees(mut self, worktrees: FakeWorktrees) -> Self {
        self.worktree = Some(worktrees);
        self
    }

    pub fn with_slots(mut self, store: SlotStore) -> Self {
        self.slots = Some(store);
        self
    }

    fn with_env<T>(&self, f: impl FnOnce(&Env<'_>) -> T) -> T {
        let tmux = self.tmux.clone();
        let backend_for = move |backend: &str| -> Option<Box<dyn MuxBackend>> {
            (backend == "tmux").then(|| Box::new(tmux.clone()) as Box<dyn MuxBackend>)
        };
        let env = Env {
            backend_for: &backend_for,
            history: Some(&self.writer),
            resume_watch: Duration::ZERO,
            worktree: self.worktree.as_ref().map(|w| w as &dyn WorktreeBackend),
            slots: self.slots.as_ref(),
            cloner: &self.cloner,
            launch: &self.launch,
        };
        f(&env)
    }

    pub fn plan(
        &self,
        snapshot: &GraphSnapshot,
        subject: &Subject,
        options: &Options,
    ) -> Result<Plan, RebuildError> {
        self.with_env(|env| super::plan(snapshot, subject, options, env, 1_790_000_000))
    }

    pub fn run(&self, plan: &Plan) -> Result<Executed, RunFailure> {
        self.with_env(|env| super::execute(plan, env))
    }
}

/// `--attach` options.
pub fn attach() -> Options {
    Options {
        session: None,
        attach: true,
        ..Default::default()
    }
}

pub fn titles(plan: &Plan) -> Vec<&str> {
    plan.steps.iter().map(|s| s.title.as_str()).collect()
}
