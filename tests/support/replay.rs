use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use conspectus::discovery::cross_link;
use conspectus::discovery::harness::claude_code::HARNESS_KEY as CLAUDE_CODE_HARNESS_KEY;
use conspectus::discovery::harness::codex::HARNESS_KEY as CODEX_HARNESS_KEY;
use conspectus::discovery::harness::fixtures::{
    ClaudeCodeSessionRecord, CodexSessionRecord, HarnessFixture,
};
use conspectus::discovery::tmux::FakeTmux;
use conspectus::discovery::{LocalDiscoveryConfig, discover_local_with};
use conspectus::filter::RowFilter;
use conspectus::hook::{HookRecord, HookStore};
use conspectus::model::GraphSnapshot;
use conspectus::resolve::resolve_snapshot;
use conspectus::tui::SessionsGrouping;
use conspectus::tui::rows::{RowTree, SessionsBuildInputs, build_sessions_tree};

pub struct ReplayWorld {
    temp: tempfile::TempDir,
    harness: HarnessFixture,
    hook_root: PathBuf,
    tmux_rows: Vec<TmuxReplayRow>,
    fd_paths_by_pid: BTreeMap<i64, Vec<String>>,
}

impl ReplayWorld {
    pub fn new() -> Self {
        let temp = tempfile::TempDir::new().expect("replay temp dir");
        let harness = HarnessFixture::at(temp.path().join("harness"));
        let hook_root = temp.path().join("hooks");
        Self {
            temp,
            harness,
            hook_root,
            tmux_rows: Vec::new(),
            fd_paths_by_pid: BTreeMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        self.temp.path()
    }

    pub fn hook_root(&self) -> &Path {
        &self.hook_root
    }

    pub fn mkdir(&self, relative: impl AsRef<Path>) -> PathBuf {
        let path = self.root().join(relative);
        fs::create_dir_all(&path).expect("create replay directory");
        path
    }

    pub fn write_codex_session(&self, session_key: &str, cwd: impl AsRef<Path>) -> PathBuf {
        self.harness
            .write_codex_session(
                &CodexSessionRecord::new(session_key)
                    .with_cwd(path_string(cwd.as_ref()))
                    .with_timestamp("2026-01-02T03:04:05Z"),
            )
            .expect("write codex replay session")
    }

    pub fn write_claude_code_session(&self, session_key: &str, cwd: impl AsRef<Path>) -> PathBuf {
        self.harness
            .write_claude_code_session(
                &ClaudeCodeSessionRecord::new(session_key, path_string(cwd.as_ref()))
                    .with_timestamp("2026-01-02T03:04:05Z"),
            )
            .expect("write claude-code replay session")
    }

    pub fn write_hook_record(&self, record: HookRecord) {
        HookStore::new(&self.hook_root)
            .write_record(&record)
            .expect("write replay hook record");
    }

    pub fn add_tmux_row(&mut self, row: TmuxReplayRow) {
        self.tmux_rows.push(row);
    }

    pub fn add_fd_paths(&mut self, pid: i64, paths: impl IntoIterator<Item = impl Into<String>>) {
        self.fd_paths_by_pid
            .entry(pid)
            .or_default()
            .extend(paths.into_iter().map(Into::into));
    }

    pub fn run(&self) -> ReplayResult {
        self.run_from([self.root().to_path_buf()])
    }

    pub fn run_from(&self, roots: impl IntoIterator<Item = impl Into<PathBuf>>) -> ReplayResult {
        let mut config = LocalDiscoveryConfig::empty()
            .with_harness_state_root(CODEX_HARNESS_KEY, self.harness.codex_state_root())
            .with_harness_state_root(
                CLAUDE_CODE_HARNESS_KEY,
                self.harness.claude_code_state_root(),
            )
            .with_hook_sidecar_root(&self.hook_root);

        if !self.tmux_rows.is_empty() {
            config = config.with_tmux_runner(FakeTmux::with_sessions(self.tmux_stdout()));
        }

        let mut snapshot = discover_local_with(roots, config).expect("replay discovery");
        if !self.fd_paths_by_pid.is_empty() {
            cross_link::infer_with_fd_paths(&mut snapshot, &self.fd_paths_by_pid);
        }
        let resolved = resolve_snapshot(snapshot.clone());
        let sessions = build_sessions_tree(SessionsBuildInputs {
            snapshot: &resolved,
            grouping: SessionsGrouping::Graph,
            home: Some(self.root()),
            now: Some(1_700_000_600),
            cwd: None,
            filter: RowFilter::default(),
        });

        ReplayResult {
            snapshot,
            resolved,
            sessions,
        }
    }

    pub fn normalize(&self, text: impl AsRef<str>) -> String {
        text.as_ref()
            .replace(&path_string(self.root()), "/fixture")
            .replace(&path_string(self.harness.root()), "/fixture/harness")
            .replace(&path_string(&self.hook_root), "/fixture/hooks")
    }

    /// Run discovery + resolve, then write the resolved
    /// `GraphSnapshot` as a JSON fixture that
    /// `conspectus tui --fixture` and `conspectus tui --snapshot
    /// --snapshot-fixture` can consume (ADRs 0068 / 0069).
    /// Temp-directory paths are rewritten to `/fixture` via
    /// [`ReplayWorld::normalize`] so the JSON is stable across
    /// machines and safe to check into `tests/fixtures/`.
    pub fn write_snapshot_fixture(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let result = self.run();
        let raw = serde_json::to_string_pretty(&result.resolved)
            .expect("serialize replay snapshot to JSON");
        fs::write(path.as_ref(), self.normalize(raw))
    }

    fn tmux_stdout(&self) -> String {
        self.tmux_rows
            .iter()
            .map(TmuxReplayRow::to_line)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }
}

impl Default for ReplayWorld {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ReplayResult {
    pub snapshot: GraphSnapshot,
    pub resolved: GraphSnapshot,
    pub sessions: RowTree,
}

#[derive(Clone, Debug, Default)]
pub struct TmuxReplayRow {
    name: String,
    cwd: Option<String>,
    activity_epoch: Option<i64>,
    created_epoch: Option<i64>,
    active_pane_command: Option<String>,
    active_pane_pid: Option<i64>,
    active_pane_current_path: Option<String>,
    active_pane_start_command: Option<String>,
    client_attached: Option<bool>,
}

impl TmuxReplayRow {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn with_cwd(mut self, cwd: impl AsRef<Path>) -> Self {
        self.cwd = Some(path_string(cwd.as_ref()));
        self
    }

    pub fn with_activity(mut self, epoch: i64) -> Self {
        self.activity_epoch = Some(epoch);
        self
    }

    pub fn with_created(mut self, epoch: i64) -> Self {
        self.created_epoch = Some(epoch);
        self
    }

    pub fn with_active_pane(
        mut self,
        command: impl Into<String>,
        pid: i64,
        cwd: impl AsRef<Path>,
        start_command: impl Into<String>,
    ) -> Self {
        self.active_pane_command = Some(command.into());
        self.active_pane_pid = Some(pid);
        self.active_pane_current_path = Some(path_string(cwd.as_ref()));
        self.active_pane_start_command = Some(start_command.into());
        self
    }

    fn to_line(&self) -> String {
        [
            self.name.clone(),
            self.cwd.clone().unwrap_or_default(),
            self.activity_epoch
                .map(|epoch| epoch.to_string())
                .unwrap_or_default(),
            self.created_epoch
                .map(|epoch| epoch.to_string())
                .unwrap_or_default(),
            self.active_pane_command.clone().unwrap_or_default(),
            self.active_pane_pid
                .map(|pid| pid.to_string())
                .unwrap_or_default(),
            self.active_pane_current_path.clone().unwrap_or_default(),
            self.active_pane_start_command.clone().unwrap_or_default(),
            self.client_attached
                .map(|attached| if attached { "1" } else { "0" }.to_string())
                .unwrap_or_default(),
        ]
        .join("\t")
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
