//! Rebuild (ADR 0142): turn a recipe from a pin slot, a history
//! record, or a live node back into a live entity.
//!
//! A rebuild is a [`Plan`]: an ordered list of [`Step`]s that is shown
//! before it runs and printed by `--dry-run`. Each kind contributes an
//! executor that plans its own steps and can put another rebuild's
//! steps ahead of its own (a mux whose worktree is gone). Running a
//! plan stops at the first failed step and reports what already ran;
//! nothing is rolled back, because every step leaves something the
//! operator can see and remove.

pub mod clone;
pub mod launch;
pub mod mux;
pub mod resume;
mod run;
pub mod session;
mod subject;
#[cfg(test)]
pub(crate) mod test_support;
pub mod worktree;

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use crate::discovery::tmux::MuxBackend;
use crate::discovery::worktree::WorktreeBackend;
use crate::history::HistoryWriter;
use crate::pin_slots::SlotStore;

pub use clone::{RepoCloner, SystemCloner};
pub use run::{Executed, RunFailure, execute};
pub use subject::{Subject, SubjectSource, resolve_subject};

/// Options that shape a plan.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Resume this session of a mux's hosted sessions instead of the
    /// most recent one.
    pub session: Option<String>,
    /// End the plan with an attach. Off for `--no-attach`.
    pub attach: bool,
}

/// Where a step that starts inside a rebuilt worktree should start: the
/// worktree `key` names, plus the path inside it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Landing {
    pub key: String,
    pub rel: PathBuf,
}

/// One thing a rebuild does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// End a mux session whose pane died (ADR 0103), so a fresh one
    /// can take its name.
    KillSession {
        backend: String,
        socket: Option<String>,
        name: String,
    },
    /// Create a mux session running `argv`, or `resume` when there is
    /// one (a resume that dies is replaced by `argv`, ADR 0103).
    NewSession {
        backend: String,
        socket: Option<String>,
        name: String,
        cwd: PathBuf,
        /// When an earlier step rebuilds the worktree `cwd` lives in,
        /// the session starts where that worktree actually landed.
        cwd_after: Option<Landing>,
        argv: Vec<OsString>,
        resume: Option<resume::Resume>,
        harness: Option<String>,
    },
    /// Bring a branch's worktree back through the worktree backend
    /// (ADR 0092), creating the branch from `base` when it is gone.
    WorktreeSwitch {
        /// Names the worktree for later steps that start inside it.
        key: String,
        repo_root: PathBuf,
        /// The repo's key (`repo:<common dir>`).
        repo: String,
        branch: String,
        create: bool,
        base: Option<String>,
        /// The checkout this replaces, when rebuilding a recorded one.
        recorded_root: Option<PathBuf>,
        old_record: Option<String>,
    },
    /// `git clone <remote> <dest>` into a path that doesn't exist
    /// (ADR 0143 category 6).
    Clone { remote: String, dest: PathBuf },
    /// Type the launch argv into a live mux whose agent exited. Only
    /// planned when nothing shows a harness running there (ADR 0028).
    SendKeys {
        backend: String,
        socket: Option<String>,
        name: String,
        argv: Vec<OsString>,
    },
    /// Attach the terminal to a mux session.
    Attach {
        backend: String,
        socket: Option<String>,
        name: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    /// What the step does, in a few words.
    pub title: String,
    /// The command it runs, as the operator would type it.
    pub command: String,
    pub action: Action,
}

/// A rebuild, planned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    /// One line naming what is being rebuilt and its state.
    pub subject: String,
    pub steps: Vec<Step>,
    /// Things the operator should know before running it.
    pub warnings: Vec<String>,
    /// The mux the plan ends with, for the attach and the caller's
    /// bookkeeping.
    pub target: Option<MuxTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MuxTarget {
    pub backend: String,
    pub socket: Option<String>,
    pub name: String,
}

impl Plan {
    /// The text `--dry-run` prints and the TUI confirms.
    pub fn render(&self) -> String {
        let mut out = format!("rebuild {}\n", self.subject);
        for (i, step) in self.steps.iter().enumerate() {
            out.push_str(&format!(
                "  {}. {:<22} {}\n",
                i + 1,
                step.title,
                step.command
            ));
        }
        for warning in &self.warnings {
            out.push_str(&format!("  note: {warning}\n"));
        }
        out
    }

    /// Put `first`'s steps and warnings ahead of this plan's. The
    /// subject stays this plan's.
    pub fn after(mut self, first: Plan) -> Plan {
        let mut steps = first.steps;
        steps.append(&mut self.steps);
        self.steps = steps;
        let mut warnings = first.warnings;
        warnings.append(&mut self.warnings);
        self.warnings = warnings;
        self
    }

    /// Whether the plan changes anything. Attaching alone doesn't.
    pub fn creates_something(&self) -> bool {
        self.steps
            .iter()
            .any(|step| !matches!(step.action, Action::Attach { .. }))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RebuildError {
    #[error("nothing matches `{0}`: pass a pin slot name, a history id (h_…), or a node id")]
    NotFound(String),
    #[error("`{input}` matches more than one thing: {candidates}")]
    Ambiguous { input: String, candidates: String },
    #[error("`{name}` is owned by {owner}, which manages its lifecycle; rebuild it there")]
    Owned { name: String, owner: String },
    #[error("a {0} can't be rebuilt")]
    Unsupported(String),
    #[error("{0}")]
    Cannot(String),
    #[error("the `{0}` mux backend isn't available")]
    NoBackend(String),
    #[error("{0}")]
    Backend(String),
}

/// Plan the rebuild of `subject`.
pub fn plan(
    snapshot: &crate::model::GraphSnapshot,
    subject: &Subject,
    options: &Options,
    env: &Env<'_>,
    now: i64,
) -> Result<Plan, RebuildError> {
    match subject.slot.kind {
        crate::model::PinKind::Mux => mux::plan(snapshot, subject, options, env, now),
        crate::model::PinKind::Session => session::plan(snapshot, subject, options, env, now),
        crate::model::PinKind::Checkout => worktree::plan(snapshot, subject, options, env, now),
        crate::model::PinKind::Repo => clone::plan(snapshot, subject, options, env, now),
        other => Err(RebuildError::Unsupported(other.as_str().to_string())),
    }
}

/// What a plan needs to talk to the world.
pub struct Env<'a> {
    pub backend_for: &'a dyn Fn(&str) -> Option<Box<dyn MuxBackend>>,
    /// Where a launched mux is recorded (ADR 0140). `None` records
    /// nothing.
    pub history: Option<&'a HistoryWriter>,
    /// How long a resume is watched before it counts as started.
    pub resume_watch: Duration,
    /// The backend that changes worktrees (worktrunk). `None` when
    /// none is configured, which makes a worktree rebuild an error.
    pub worktree: Option<&'a dyn WorktreeBackend>,
    /// Where pin slots live, so a rebuild that lands somewhere new can
    /// retarget the slots that pointed at the old place.
    pub slots: Option<&'a SlotStore>,
    pub cloner: &'a dyn RepoCloner,
}
