//! Debug-only replay scenarios for manual graph/TUI inspection.
//!
//! This module is compiled only for tests and debug builds. It deliberately
//! keeps scenario materialization out of normal production discovery while
//! giving developers stable names for edge-case worlds.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use crate::config::Projection;
use crate::discovery::cross_link;
use crate::discovery::forge::FakeGh;
use crate::discovery::harness::aider::HARNESS_KEY as AIDER_HARNESS_KEY;
use crate::discovery::harness::claude_code::HARNESS_KEY as CLAUDE_CODE_HARNESS_KEY;
use crate::discovery::harness::codex::HARNESS_KEY as CODEX_HARNESS_KEY;
use crate::discovery::harness::fixtures::{
    ClaudeCodeSessionRecord, CodexSessionRecord, HarnessFixture, OpenCodeSessionRecord,
};
use crate::discovery::harness::opencode::HARNESS_KEY as OPENCODE_HARNESS_KEY;
use crate::discovery::tmux::FakeTmux;
use crate::discovery::{LocalDiscoveryConfig, discover_local_with, path_to_string};
use crate::filter::RowFilter;
use crate::hook::{HookRecord, HookStore, HookTmuxRecord, SCHEMA_VERSION};
use crate::model::GraphSnapshot;
use crate::output::render::RenderOptions;
use crate::output::table;
use crate::resolve::resolve_snapshot;
use crate::tui::rows::RowTree;
use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
use crate::tui::{RunConfig, SessionsGrouping, Sort, View};

#[derive(Clone, Copy, Debug)]
pub struct ScenarioDef {
    pub name: &'static str,
    pub description: &'static str,
    build: fn(&mut ScenarioWorld) -> Result<()>,
}

pub const SCENARIOS: &[ScenarioDef] = &[
    ScenarioDef {
        name: "empty",
        description: "empty non-repo world",
        build: build_empty,
    },
    ScenarioDef {
        name: "orphan-session",
        description: "one agent session with no mux or repo",
        build: build_orphan_session,
    },
    ScenarioDef {
        name: "exact-match",
        description: "one agent session exactly linked to one tmux session",
        build: build_exact_match,
    },
    ScenarioDef {
        name: "ambiguous-mux",
        description: "one agent session with two plausible tmux candidates",
        build: build_ambiguous_mux,
    },
    ScenarioDef {
        name: "hook-supersession",
        description: "same pane hook records where the freshest session wins",
        build: build_hook_supersession,
    },
    ScenarioDef {
        name: "codex-fd-current",
        description: "Codex fd evidence beats a stale launch command",
        build: build_codex_fd_current,
    },
    ScenarioDef {
        name: "process-cardinality",
        description: "one mux with two human agent runtime processes",
        build: build_process_cardinality,
    },
    ScenarioDef {
        name: "workspace-pr",
        description: "git workspace with a fake GitHub pull request",
        build: build_workspace_pr,
    },
    ScenarioDef {
        name: "fork-lineage",
        description: "Atelier fork lineage with unresolved harness lineage",
        build: build_fork_lineage,
    },
    ScenarioDef {
        name: "showcase",
        description: "comprehensive world exercising most conspectus surfaces (ADR 0070)",
        build: build_showcase,
    },
];

pub fn scenario_names() -> impl Iterator<Item = &'static str> {
    SCENARIOS.iter().map(|scenario| scenario.name)
}

pub fn scenario_def(name: &str) -> Option<ScenarioDef> {
    SCENARIOS
        .iter()
        .copied()
        .find(|scenario| scenario.name == name)
}

pub fn materialize(name: &str) -> Result<ScenarioWorld> {
    let Some(def) = scenario_def(name) else {
        let names = scenario_names().collect::<Vec<_>>().join(", ");
        bail!("unknown scenario `{name}`; expected one of {names}");
    };
    let mut world = ScenarioWorld::new(name)?;
    (def.build)(&mut world)?;
    Ok(world)
}

pub struct ScenarioWorld {
    name: String,
    root: PathBuf,
    harness: HarnessFixture,
    hook_root: PathBuf,
    scan_roots: Vec<PathBuf>,
    tmux_rows: Vec<TmuxReplayRow>,
    fd_paths_by_pid: BTreeMap<i64, Vec<String>>,
    gh_pull_requests: Option<String>,
    /// Agent-deck root path. When `Some`, discovery uses it for the
    /// agent-deck adapter (`multi-repo-worktrees/` subdir +
    /// `profiles/<name>/state.db` for titles, ADR 0066).
    agent_deck_root: Option<PathBuf>,
}

impl ScenarioWorld {
    fn new(name: &str) -> Result<Self> {
        let root = unique_root(name);
        fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
        let harness = HarnessFixture::at(root.join("harness"));
        let hook_root = root.join("hooks");
        Ok(Self {
            name: name.to_string(),
            scan_roots: vec![root.clone()],
            root,
            harness,
            hook_root,
            tmux_rows: Vec::new(),
            fd_paths_by_pid: BTreeMap::new(),
            gh_pull_requests: None,
            agent_deck_root: None,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn scan_roots(&self) -> &[PathBuf] {
        &self.scan_roots
    }

    pub fn snapshot(&self) -> Result<GraphSnapshot> {
        let mut snapshot = discover_local_with(self.scan_roots.clone(), self.discovery_config())?;
        if !self.fd_paths_by_pid.is_empty() {
            cross_link::infer_with_fd_paths(&mut snapshot, &self.fd_paths_by_pid);
            // `discover_local_with` already ran `cross_link::infer*`, and
            // `infer_with_fd_paths` re-runs the cwd-based pass. The
            // re-emitted links are byte-identical to the first pass; dedupe
            // by link id so downstream row builders see a canonical graph.
            let mut seen = std::collections::BTreeSet::new();
            snapshot
                .candidate_links
                .retain(|link| seen.insert(link.id.clone()));
        }
        Ok(resolve_snapshot(snapshot))
    }

    pub fn render_graph_json(&self) -> Result<String> {
        crate::output::render_graph_json(&self.snapshot()?)
    }

    pub fn render_table(
        &self,
        projection: Projection,
        mut options: RenderOptions,
    ) -> Result<String> {
        let snapshot = self.snapshot()?;
        options = options
            .with_filter(RowFilter::default())
            .with_now_epoch(Some(1_700_000_600));
        Ok(table::render_with(&snapshot, projection, &options))
    }

    pub fn render_node_show(&self, id: &str, color: bool) -> Result<String> {
        let snapshot = self.snapshot()?;
        let id = match crate::output::node_show::resolve_node_id(id, &snapshot) {
            Ok(id) => id,
            Err(err) => bail!("{err}"),
        };
        Ok(crate::output::node_show::render_node_show(
            &snapshot, &id, color,
        ))
    }

    pub fn sessions_tree(&self) -> Result<RowTree> {
        let snapshot = self.snapshot()?;
        Ok(build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(&self.root),
            now: Some(1_700_000_600),
            cwd: None,
            filter: RowFilter::default(),
        }))
    }

    pub fn tui_config(&self, view: View, color: bool) -> RunConfig {
        RunConfig {
            scan_roots: self.scan_roots.clone(),
            cwd: Some(self.root.clone()),
            default_view: view,
            default_sort: Sort::Hierarchy,
            sessions_grouping: SessionsGrouping::Graph,
            mux_grouping: crate::tui::MuxGrouping::Session,
            initial_filter: RowFilter::default(),
            explicit_filter: false,
            explicit_sort: false,
            explicit_grouping: false,
            refresh_interval: Duration::from_secs(24 * 60 * 60),
            mux_preview_interval: Duration::from_secs(24 * 60 * 60),
            live_preview_enabled: false,
            color,
            current_tmux_session: None,
            theme: crate::tui::Theme::default(),
            show_edge_meta: false,
            show_harness_chips: false,
            narrow_layout_threshold: crate::config::DEFAULT_NARROW_LAYOUT_THRESHOLD,
            intervals: crate::config::ServerIntervals::default(),
            no_cache: true,
            refresh: true,
        }
    }

    fn discovery_config(&self) -> LocalDiscoveryConfig {
        let mut config = LocalDiscoveryConfig::empty()
            .with_harness_state_root(CODEX_HARNESS_KEY, self.harness.codex_state_root())
            .with_harness_state_root(
                CLAUDE_CODE_HARNESS_KEY,
                self.harness.claude_code_state_root(),
            )
            .with_harness_state_root(OPENCODE_HARNESS_KEY, self.harness.opencode_state_root())
            .with_harness_state_root(AIDER_HARNESS_KEY, self.root.clone())
            .with_hook_sidecar_root(&self.hook_root)
            // H-EXT-007: codex's aux-attribution mutator pass
            // pulls from live `~/.codex/logs_*.sqlite`; skip it
            // in the scenario TUI so dev_scenarios stays
            // hermetic.
            .without_aux_harness(crate::discovery::harness::codex::HARNESS_KEY);
        if !self.tmux_rows.is_empty() {
            config = config.with_tmux_runner(FakeTmux::with_sessions(self.tmux_stdout()));
        }
        if let Some(body) = &self.gh_pull_requests {
            config = config.with_forge_runner(FakeGh::with_pull_requests(body.clone()));
        }
        if let Some(root) = &self.agent_deck_root {
            config = config.with_agent_deck_root(root.clone());
        }
        config
    }

    fn mkdir(&self, relative: &str) -> Result<PathBuf> {
        let path = self.root.join(relative);
        fs::create_dir_all(&path).with_context(|| format!("create {}", path.display()))?;
        Ok(path)
    }

    fn write_codex_session(&self, session_key: &str, cwd: &Path) -> Result<()> {
        self.harness
            .write_codex_session(
                &CodexSessionRecord::new(session_key)
                    .with_cwd(path_to_string(cwd))
                    .with_timestamp("2026-01-02T03:04:05Z"),
            )
            .map(|_| ())
    }

    fn write_claude_code_session(&self, session_key: &str, cwd: &Path) -> Result<()> {
        self.harness
            .write_claude_code_session(
                &ClaudeCodeSessionRecord::new(session_key, path_to_string(cwd))
                    .with_timestamp("2026-01-02T03:04:05Z"),
            )
            .map(|_| ())
    }

    fn write_hook_record(&self, record: HookRecord) -> Result<()> {
        HookStore::new(&self.hook_root)
            .write_record(&record)
            .map(|_| ())
    }

    fn add_tmux_row(&mut self, row: TmuxReplayRow) {
        self.tmux_rows.push(row);
    }

    fn add_fd_paths(&mut self, pid: i64, paths: impl IntoIterator<Item = String>) {
        self.fd_paths_by_pid.entry(pid).or_default().extend(paths);
    }

    fn init_repo(&self, relative: &str, remote: Option<&str>) -> Result<PathBuf> {
        let root = self.mkdir(relative)?;
        git(&root, &["init", "--initial-branch", "main"])?;
        git(&root, &["config", "user.name", "Conspectus Scenario"])?;
        git(
            &root,
            &["config", "user.email", "conspectus@example.invalid"],
        )?;
        if let Some(remote) = remote {
            git(&root, &["remote", "add", "origin", remote])?;
        }
        fs::write(root.join("README.md"), "scenario\n")?;
        git(&root, &["add", "README.md"])?;
        git(&root, &["commit", "-m", "initial"])?;
        Ok(root)
    }

    /// Create a bare repository plus a linked worktree on disk so
    /// the git discoverer sees a non-canonical checkout shape (the
    /// `.git/worktrees/<name>` form). The bare repo lives at
    /// `<relative>.git`; the worktree at `<worktree_relative>`.
    fn init_bare_repo_with_worktree(
        &self,
        bare_relative: &str,
        worktree_relative: &str,
        branch: &str,
    ) -> Result<(PathBuf, PathBuf)> {
        let bare = self.root.join(bare_relative);
        fs::create_dir_all(&bare).with_context(|| format!("create {}", bare.display()))?;
        git(&bare, &["init", "--bare", "--initial-branch", "main"])?;
        // Seed the bare repo with a commit by working through a
        // temporary clone — `git --bare commit` is not a thing.
        let seed = self.root.join("__bare_seed__");
        fs::create_dir_all(&seed)?;
        git(&seed, &["init", "--initial-branch", "main"])?;
        git(&seed, &["config", "user.name", "Conspectus Scenario"])?;
        git(
            &seed,
            &["config", "user.email", "conspectus@example.invalid"],
        )?;
        fs::write(seed.join("README.md"), "showcase bare\n")?;
        git(&seed, &["add", "README.md"])?;
        git(&seed, &["commit", "-m", "initial"])?;
        git(&seed, &["remote", "add", "bare", &path_to_string(&bare)])?;
        git(&seed, &["push", "bare", "main"])?;
        fs::remove_dir_all(&seed)?;

        let worktree = self.root.join(worktree_relative);
        fs::create_dir_all(worktree.parent().expect("worktree parent"))?;
        git(
            &bare,
            &[
                "worktree",
                "add",
                "-b",
                branch,
                &path_to_string(&worktree),
                "main",
            ],
        )?;
        Ok((bare, worktree))
    }

    /// Materialize an agent-deck workspace per ADR 0060/0066. Writes
    /// the `<agent_deck_root>/multi-repo-worktrees/<folder>/`
    /// directory with one symlink per `(name, target_checkout)`
    /// pair, plus a `profiles/<profile>/state.db` SQLite database
    /// carrying the operator-chosen title for the workspace so
    /// discovery's title lookup (ADR 0066) emits a meaningful
    /// `WorkspaceNode.name`. Sets `agent_deck_root` on the world so
    /// `discovery_config` wires the adapter.
    fn add_agent_deck_workspace(
        &mut self,
        folder: &str,
        title: &str,
        members: &[(&str, &Path)],
    ) -> Result<PathBuf> {
        let deck_root = self.root.join(".agent-deck");
        let worktrees_root = deck_root.join("multi-repo-worktrees");
        let workspace_dir = worktrees_root.join(folder);
        fs::create_dir_all(&workspace_dir).with_context(|| {
            format!("create agent-deck workspace at {}", workspace_dir.display())
        })?;
        for (name, target) in members {
            let link = workspace_dir.join(name);
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, &link)
                .with_context(|| format!("symlink {} -> {}", link.display(), target.display()))?;
        }

        // Extract the 8-hex conductor id agent-deck shares with the
        // instance row's `id` prefix. Same rule as the title-lookup
        // path in `src/discovery/agent_deck.rs`.
        let id_prefix = folder.rsplit('-').next().unwrap_or(folder);
        let profile_dir = deck_root.join("profiles").join("default");
        fs::create_dir_all(&profile_dir)
            .with_context(|| format!("create profile dir at {}", profile_dir.display()))?;
        let db_path = profile_dir.join("state.db");
        let conn = rusqlite::Connection::open(&db_path)
            .with_context(|| format!("open agent-deck state.db at {}", db_path.display()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS instances (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT ''
            );",
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO instances (id, title) VALUES (?1, ?2)",
            rusqlite::params![format!("{id_prefix}-1700000000"), title],
        )?;

        // Adapter expects the `multi-repo-worktrees` directory as its
        // root; profiles root is derived as `<root>/../profiles`.
        self.agent_deck_root = Some(worktrees_root);
        Ok(workspace_dir)
    }

    fn write_opencode_session(
        &self,
        session_id: &str,
        cwd: &Path,
        title: &str,
        epoch_ms: i64,
        assistant_message: Option<&str>,
    ) -> Result<()> {
        let mut record = OpenCodeSessionRecord::new(session_id)
            .with_directory(path_to_string(cwd))
            .with_title(title)
            .with_created(epoch_ms)
            .with_updated(epoch_ms + 1000);
        if let Some(text) = assistant_message {
            record = record.with_assistant_message(text);
        }
        self.harness.write_opencode_session(&record).map(|_| ())
    }

    fn write_aider_state(&self, repo: &Path) -> Result<()> {
        self.harness.write_aider_state(repo).map(|_| ())
    }

    fn write_atelier_config(&self, text: &str) -> Result<()> {
        self.write_atelier_config_at(&self.root, text)
    }

    fn write_atelier_config_at(&self, dir: &Path, text: &str) -> Result<()> {
        fs::write(dir.join("atelier.toml"), text).map_err(Into::into)
    }

    fn write_fork_index(&self, text: &str) -> Result<()> {
        self.write_fork_index_at(&self.root, text)
    }

    fn write_fork_index_at(&self, dir: &Path, text: &str) -> Result<()> {
        let path = dir.join(".atelier/forks/index.toml");
        fs::create_dir_all(path.parent().expect("fork index parent"))?;
        fs::write(path, text).map_err(Into::into)
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

impl Drop for ScenarioWorld {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone, Debug, Default)]
struct TmuxReplayRow {
    name: String,
    cwd: Option<String>,
    activity_epoch: Option<i64>,
    created_epoch: Option<i64>,
    active_pane_command: Option<String>,
    active_pane_pid: Option<i64>,
    active_pane_current_path: Option<String>,
    active_pane_start_command: Option<String>,
}

impl TmuxReplayRow {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    fn with_cwd(mut self, cwd: &Path) -> Self {
        self.cwd = Some(path_to_string(cwd));
        self
    }

    fn with_activity(mut self, epoch: i64) -> Self {
        self.activity_epoch = Some(epoch);
        self
    }

    fn with_active_pane(
        mut self,
        command: impl Into<String>,
        pid: i64,
        cwd: &Path,
        start_command: impl Into<String>,
    ) -> Self {
        self.active_pane_command = Some(command.into());
        self.active_pane_pid = Some(pid);
        self.active_pane_current_path = Some(path_to_string(cwd));
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
            String::new(),
        ]
        .join("\t")
    }
}

fn build_empty(world: &mut ScenarioWorld) -> Result<()> {
    world.mkdir("empty").map(|_| ())
}

fn build_orphan_session(world: &mut ScenarioWorld) -> Result<()> {
    let work = world.mkdir("work")?;
    world.write_codex_session("orphan-session", &work)
}

fn build_exact_match(world: &mut ScenarioWorld) -> Result<()> {
    let work = world.mkdir("work")?;
    world.write_codex_session("session-x", &work)?;
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_activity(1_700_000_500),
    );
    Ok(())
}

fn build_ambiguous_mux(world: &mut ScenarioWorld) -> Result<()> {
    let work = world.mkdir("work")?;
    world.write_codex_session("ambiguous", &work)?;
    world.add_tmux_row(
        TmuxReplayRow::new("editor-a")
            .with_cwd(&work)
            .with_activity(1_700_000_500),
    );
    world.add_tmux_row(
        TmuxReplayRow::new("editor-b")
            .with_cwd(&work)
            .with_activity(1_700_000_550),
    );
    Ok(())
}

fn build_hook_supersession(world: &mut ScenarioWorld) -> Result<()> {
    let work = world.mkdir("work")?;
    let session_a = "aaaaaaaa-1111-2222-3333-444444444444";
    let session_b = "bbbbbbbb-1111-2222-3333-444444444444";

    world.write_claude_code_session(session_a, &work)?;
    world.write_claude_code_session(session_b, &work)?;
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_active_pane("claude", 123, &work, "claude"),
    );

    for (session_key, observed_epoch) in [(session_a, 1_700_000_500), (session_b, 1_700_000_600)] {
        world.write_hook_record(HookRecord {
            schema_version: SCHEMA_VERSION,
            harness_key: "claude-code".to_string(),
            session_key: session_key.to_string(),
            cwd: Some(path_to_string(&work)),
            pid: Some(123),
            ppid: Some(456),
            tmux: Some(HookTmuxRecord {
                session_name: Some("editor".to_string()),
                native_id: None,
                pane_id: Some("%1".to_string()),
                socket_path: None,
            }),
            transcript_path: None,
            hook_event_name: Some("SessionStart".to_string()),
            observed_epoch,
            harness_version: Some("1.0.0".to_string()),
        })?;
    }
    Ok(())
}

fn build_codex_fd_current(world: &mut ScenarioWorld) -> Result<()> {
    let work = world.mkdir("work")?;
    let session_current = "b0000000-1111-2222-3333-444444444444";
    world.write_codex_session(session_current, &work)?;
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_active_pane(
                "codex",
                4242,
                &work,
                "codex resume a0000000-1111-2222-3333-444444444444",
            ),
    );
    world.add_fd_paths(
        4242,
        [format!(
            "{}/.codex/sessions/2026/05/26/rollout-{session_current}.jsonl",
            world.root.display()
        )],
    );
    Ok(())
}

fn build_process_cardinality(world: &mut ScenarioWorld) -> Result<()> {
    let work = world.mkdir("work")?;
    let session_a = "c0000000-1111-2222-3333-444444444444";
    let session_b = "d0000000-1111-2222-3333-444444444444";
    let live_pids = live_scenario_pids();

    world.write_claude_code_session(session_a, &work)?;
    world.write_claude_code_session(session_b, &work)?;
    world.add_tmux_row(
        TmuxReplayRow::new("pair-a")
            .with_cwd(&work)
            .with_active_pane("claude", live_pids[0], &work, "claude"),
    );
    world.add_tmux_row(
        TmuxReplayRow::new("pair-b")
            .with_cwd(&work)
            .with_active_pane("claude", live_pids[1], &work, "claude"),
    );

    for (session_key, pid, ppid, mux_name, observed_epoch) in [
        (
            session_a,
            live_pids[0],
            live_pids[1],
            "pair-a",
            1_700_000_500,
        ),
        (
            session_b,
            live_pids[1],
            live_pids[0],
            "pair-b",
            1_700_000_540,
        ),
    ] {
        world.write_hook_record(HookRecord {
            schema_version: SCHEMA_VERSION,
            harness_key: "claude-code".to_string(),
            session_key: session_key.to_string(),
            cwd: Some(path_to_string(&work)),
            pid: Some(pid),
            ppid: Some(ppid),
            tmux: Some(HookTmuxRecord {
                session_name: Some(mux_name.to_string()),
                native_id: None,
                pane_id: Some("%1".to_string()),
                socket_path: None,
            }),
            transcript_path: None,
            hook_event_name: Some("SessionStart".to_string()),
            observed_epoch,
            harness_version: Some("1.0.0".to_string()),
        })?;
    }
    Ok(())
}

fn live_scenario_pids() -> [i64; 2] {
    let current = i64::from(std::process::id());
    let parent = fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|stat| {
            let after_command = stat.rsplit_once(") ")?.1;
            let mut fields = after_command.split_whitespace();
            let _state = fields.next()?;
            fields.next()?.parse::<i64>().ok()
        })
        .filter(|pid| *pid > 0 && *pid != current)
        .unwrap_or(1);
    [current, parent]
}

fn build_workspace_pr(world: &mut ScenarioWorld) -> Result<()> {
    let repo = world.init_repo("repo", Some("git@github.com:octo/repo.git"))?;
    world.scan_roots = vec![repo.clone()];
    world.gh_pull_requests = Some(
        r#"[{"number":7,"state":"OPEN","url":"https://github.com/octo/repo/pull/7","headRefName":"main","baseRefName":"main","updatedAt":"2026-03-05T12:34:56Z","headRepositoryOwner":{"login":"octo"},"headRepository":{"name":"repo"},"isDraft":false}]"#
            .to_string(),
    );
    world.write_codex_session("pr-session", &repo)?;
    world.add_tmux_row(
        TmuxReplayRow::new("review")
            .with_cwd(&repo)
            .with_activity(1_700_000_520),
    );
    Ok(())
}

fn build_fork_lineage(world: &mut ScenarioWorld) -> Result<()> {
    world.init_repo("repo-a", None)?;
    world.init_repo("repo-b", None)?;
    world.write_atelier_config(
        r#"
[workspace]
name = "atelier-demo"

[[repos]]
name = "repo-a"
path = "/sources/repo-a"

[[repos]]
name = "repo-b"
path = "/sources/repo-b"
"#,
    )?;
    world.write_fork_index(
        r#"
[[forks]]
name = "alpha"
created-epoch = 1
mode = "worktree"
root = ".atelier/forks/alpha"
state = "isolated"

[[forks.repos]]
name = "repo-a"
source = "/sources/repo-a"
parent-worktree = "repo-a"
fork-worktree = ".atelier/forks/alpha/repo-a"
branch = "fork/alpha/repo-a"
forked = true

[[forks.harness]]
key = "codex"
source-session = "parent-session"
fork-session = "child-session"
capability = "native"

[[forks]]
name = "beta"
parent = "alpha"
created-epoch = 2
mode = "selected"
root = ".atelier/forks/beta"
"#,
    )?;
    let fork_cwd = world.mkdir(".atelier/forks/alpha/repo-a")?;
    world.write_codex_session("child-session", &fork_cwd)?;
    world.add_tmux_row(
        TmuxReplayRow::new("fork-alpha")
            .with_cwd(&fork_cwd)
            .with_activity(1_700_000_530),
    );
    Ok(())
}

/// Reference "now" the showcase's session/mux/PR timestamps are
/// anchored against (ADR 0070). Picked close to the showcase's
/// authoring date so sessions look recent when the checked-in
/// fixture is opened. When the showcase starts feeling stale (e.g.
/// every PR is "6 months ago"), bump this constant and re-run
/// `just regen-showcase-fixture` so the fresh JSON lands in one
/// commit.
const SHOWCASE_NOW_EPOCH: i64 = 1_781_611_200; // 2026-06-16T13:00:00Z (approx)

/// Comprehensive showcase scenario (ADR 0070). Built from modular
/// `add_*_to_showcase` helpers so a future edge case is one new
/// helper plus one call here.
fn build_showcase(world: &mut ScenarioWorld) -> Result<()> {
    let project = add_normal_repo_to_showcase(world)?;
    let worktree = add_bare_repo_with_worktree_to_showcase(world)?;
    let atelier = add_atelier_workspace_to_showcase(world)?;
    let deck_dir = add_agent_deck_workspace_to_showcase(world, &project, &atelier.member_repo_a)?;
    add_agent_sessions_to_showcase(world, &project, &worktree, &atelier, &deck_dir)?;
    add_mux_layout_to_showcase(world, &project, &worktree, &deck_dir)?;
    add_forge_prs_to_showcase(world, &project)?;
    add_hook_supersession_to_showcase(world, &project)?;
    Ok(())
}

struct AtelierLayout {
    member_repo_a: PathBuf,
    member_repo_b: PathBuf,
    fork_worktree: PathBuf,
}

fn add_normal_repo_to_showcase(world: &mut ScenarioWorld) -> Result<PathBuf> {
    let repo = world.init_repo(
        "repos/project",
        Some("git@github.com:conspectus/project.git"),
    )?;
    git(&repo, &["checkout", "-b", "feature/extra"])?;
    fs::write(repo.join("notes.md"), "feature work\n")?;
    git(&repo, &["add", "notes.md"])?;
    git(&repo, &["commit", "-m", "feature notes"])?;
    git(&repo, &["checkout", "main"])?;
    // The forge adapter scans each `DiscoveryContext` root for a git
    // repo with a remote, so add the project repo directly as a
    // scan-root sibling so PRs resolve while the scenario root still
    // catches the other workspaces.
    world.scan_roots.push(repo.clone());
    Ok(repo)
}

fn add_bare_repo_with_worktree_to_showcase(world: &mut ScenarioWorld) -> Result<PathBuf> {
    let (_bare, worktree) = world.init_bare_repo_with_worktree(
        "repos/bare-project.git",
        "checkouts/bare-project",
        "feature/bare",
    )?;
    Ok(worktree)
}

fn add_atelier_workspace_to_showcase(world: &mut ScenarioWorld) -> Result<AtelierLayout> {
    // Atelier discovery looks for `atelier.toml` at the scan-root and
    // resolves `repo.name` as `<workspace_root>/<name>`. House the
    // workspace under its own `atelier-demo/` subdirectory so the
    // `AssociatedWith` inference (`session cwd within workspace
    // root or member`) doesn't catch every unrelated session in the
    // scenario — without this, atelier-demo would claim every
    // showcase session because the workspace root would equal the
    // scenario root.
    let workspace_dir = world.mkdir("atelier-demo")?;
    world.scan_roots.push(workspace_dir.clone());
    let repo_a = world.init_repo("atelier-demo/repo-a", None)?;
    let repo_b = world.init_repo("atelier-demo/repo-b", None)?;
    world.write_atelier_config_at(
        &workspace_dir,
        &format!(
            r#"
[workspace]
name = "atelier-demo"

[[repos]]
name = "repo-a"
path = "{}"

[[repos]]
name = "repo-b"
path = "{}"
"#,
            path_to_string(&repo_a),
            path_to_string(&repo_b),
        ),
    )?;
    world.write_fork_index_at(
        &workspace_dir,
        r#"
[[forks]]
name = "alpha"
created-epoch = 1
mode = "worktree"
root = ".atelier/forks/alpha"
state = "isolated"

[[forks.repos]]
name = "repo-a"
source = "repo-a"
parent-worktree = "repo-a"
fork-worktree = ".atelier/forks/alpha/repo-a"
branch = "fork/alpha/repo-a"
forked = true

[[forks.harness]]
key = "codex"
source-session = "showcase-codex-parent"
fork-session = "showcase-codex-child"
capability = "native"
"#,
    )?;
    let fork_worktree = world.mkdir("atelier-demo/.atelier/forks/alpha/repo-a")?;
    Ok(AtelierLayout {
        member_repo_a: repo_a,
        member_repo_b: repo_b,
        fork_worktree,
    })
}

fn add_agent_deck_workspace_to_showcase(
    world: &mut ScenarioWorld,
    project: &Path,
    atelier_repo: &Path,
) -> Result<PathBuf> {
    world.add_agent_deck_workspace(
        "showcase-deck-c0debeef",
        "showcase-deck",
        &[("project", project), ("atelier-repo-a", atelier_repo)],
    )
}

fn add_agent_sessions_to_showcase(
    world: &mut ScenarioWorld,
    project: &Path,
    bare_worktree: &Path,
    atelier: &AtelierLayout,
    deck_dir: &Path,
) -> Result<()> {
    // Each session below sets a title (claude-code `summary`,
    // opencode `title`) and a last-message preview (transcript
    // assistant text for claude-code/codex, sqlite `part` row for
    // opencode) so the rendered showcase rows aren't blank — the
    // placeholder text names the example each session is meant to
    // illustrate (ADR 0070).
    // claude-code session in the normal project repo (1 hour ago).
    world.harness.write_claude_code_session(
        &ClaudeCodeSessionRecord::new("showcase-claude", path_to_string(project))
            .with_summary("claude-code in project repo")
            .with_assistant_message("Showcase: claude-code in project repo")
            .with_timestamp("2026-06-14T15:00:00Z"),
    )?;
    // claude-code session whose cwd is the agent-deck composite
    // directory itself — same shape as agent-deck's
    // multi-repo-worktrees launch (cwd at the composite dir, no
    // checkout) so the agent-deck workspace appears in the
    // Sessions / Graph view alongside atelier.
    world.harness.write_claude_code_session(
        &ClaudeCodeSessionRecord::new("showcase-deck-launcher", path_to_string(deck_dir))
            .with_summary("agent-deck composite launcher")
            .with_assistant_message("Showcase: agent-deck composite launcher")
            .with_timestamp("2026-06-14T15:30:00Z"),
    )?;
    // codex parent → child lineage chain (ADR 0018). Parent lives
    // in the atelier member repo, child is the fork worktree per
    // the fork index above. Spread across two days.
    world.harness.write_codex_session(
        &CodexSessionRecord::new("showcase-codex-parent")
            .with_cwd(path_to_string(&atelier.member_repo_a))
            .with_assistant_message("Showcase: codex parent in atelier fork lineage")
            .with_timestamp("2026-06-12T10:00:00Z"),
    )?;
    world.harness.write_codex_session(
        &CodexSessionRecord::new("showcase-codex-child")
            .with_cwd(path_to_string(&atelier.fork_worktree))
            .with_assistant_message("Showcase: codex child forked from parent")
            .with_timestamp("2026-06-13T11:00:00Z")
            .with_forked_from("showcase-codex-parent"),
    )?;
    // codex session in the bare repo's worktree (yesterday).
    world.harness.write_codex_session(
        &CodexSessionRecord::new("showcase-bare-codex")
            .with_cwd(path_to_string(bare_worktree))
            .with_assistant_message("Showcase: codex resume via /proc fd evidence")
            .with_timestamp("2026-06-13T14:30:00Z"),
    )?;
    // opencode session inside the second atelier member (3 hours
    // ago). epoch_ms is the field opencode's adapter consumes.
    world.write_opencode_session(
        "showcase-opencode",
        &atelier.member_repo_b,
        "opencode in atelier member repo",
        (SHOWCASE_NOW_EPOCH - 3 * 3600) * 1_000,
        Some("Showcase: opencode in atelier member repo"),
    )?;
    // aider state in the same member repo.
    world.write_aider_state(&atelier.member_repo_b)?;
    // Orphan session: cwd resolves to no checkout (a week ago).
    let orphan_dir = world.mkdir("orphan")?;
    world.harness.write_codex_session(
        &CodexSessionRecord::new("showcase-orphan")
            .with_cwd(path_to_string(&orphan_dir))
            .with_assistant_message("Showcase: orphan session with no checkout")
            .with_timestamp("2026-06-08T09:00:00Z"),
    )?;
    Ok(())
}

fn add_mux_layout_to_showcase(
    world: &mut ScenarioWorld,
    project: &Path,
    bare_worktree: &Path,
    deck_dir: &Path,
) -> Result<()> {
    // Attached mux for the claude-code session in `project` (5 min ago).
    world.add_tmux_row(
        TmuxReplayRow::new("project")
            .with_cwd(project)
            .with_activity(SHOWCASE_NOW_EPOCH - 5 * 60)
            .with_active_pane("claude", 1101, project, "claude --resume showcase-claude"),
    );
    // Mux that two harness sessions could plausibly attach to —
    // ambiguous candidate set (15 min ago).
    world.add_tmux_row(
        TmuxReplayRow::new("ambiguous")
            .with_cwd(project)
            .with_activity(SHOWCASE_NOW_EPOCH - 15 * 60)
            .with_active_pane("claude", 1102, project, "claude"),
    );
    world.harness.write_claude_code_session(
        &ClaudeCodeSessionRecord::new("showcase-claude-ambig-a", path_to_string(project))
            .with_summary("ambiguous mux candidate A")
            .with_assistant_message("Showcase: ambiguous mux candidate A")
            .with_timestamp("2026-06-14T15:40:00Z"),
    )?;
    world.harness.write_claude_code_session(
        &ClaudeCodeSessionRecord::new("showcase-claude-ambig-b", path_to_string(project))
            .with_summary("ambiguous mux candidate B")
            .with_assistant_message("Showcase: ambiguous mux candidate B")
            .with_timestamp("2026-06-14T15:42:00Z"),
    )?;
    // Mux for the bare-repo worktree with fd evidence so the
    // resolver picks the fresh session over the stale launch
    // argv (mirrors the codex-fd-current scenario, 2 hours ago).
    let bare_transcript = world
        .harness
        .codex_state_root()
        .join("sessions")
        .join("rollout-showcase-bare-codex.jsonl");
    world.add_tmux_row(
        TmuxReplayRow::new("bare-work")
            .with_cwd(bare_worktree)
            .with_activity(SHOWCASE_NOW_EPOCH - 2 * 3600)
            .with_active_pane(
                "codex",
                1103,
                bare_worktree,
                "codex resume an-older-session-id",
            ),
    );
    world.add_fd_paths(1103, [path_to_string(&bare_transcript)]);
    // Agent-deck-style mux launched from the workspace composite
    // dir (30 min ago).
    world.add_tmux_row(
        TmuxReplayRow::new("showcase-deck")
            .with_cwd(deck_dir)
            .with_activity(SHOWCASE_NOW_EPOCH - 30 * 60)
            .with_active_pane(
                "claude",
                1104,
                deck_dir,
                "claude --resume showcase-deck-launcher",
            ),
    );
    Ok(())
}

fn add_forge_prs_to_showcase(world: &mut ScenarioWorld, _project: &Path) -> Result<()> {
    // Two PRs anchored a few days before SHOWCASE_NOW_EPOCH so they
    // read as recent activity in the rendered TUI.
    world.gh_pull_requests = Some(
        r#"[
{"number":7,"state":"OPEN","url":"https://github.com/conspectus/project/pull/7","headRefName":"feature/extra","baseRefName":"main","updatedAt":"2026-06-13T15:00:00Z","headRepositoryOwner":{"login":"conspectus"},"headRepository":{"name":"project"},"isDraft":false},
{"number":8,"state":"OPEN","url":"https://github.com/conspectus/project/pull/8","headRefName":"main","baseRefName":"main","updatedAt":"2026-06-14T11:30:00Z","headRepositoryOwner":{"login":"conspectus"},"headRepository":{"name":"project"},"isDraft":true}
]"#
        .to_string(),
    );
    Ok(())
}

fn add_hook_supersession_to_showcase(world: &mut ScenarioWorld, project: &Path) -> Result<()> {
    // Hook record showing the fresh "current" session winning over
    // any stale launch-time argv pointing at an older session id —
    // mirrors the hook-supersession scenario shape, anchored a few
    // minutes ago.
    world.harness.write_claude_code_session(
        &ClaudeCodeSessionRecord::new("showcase-hook-current", path_to_string(project))
            .with_summary("hook-supersession current session")
            .with_assistant_message("Showcase: hook-supersession current session")
            .with_timestamp("2026-06-14T15:55:00Z"),
    )?;
    world.write_hook_record(HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "claude-code".to_string(),
        session_key: "showcase-hook-current".to_string(),
        cwd: Some(path_to_string(project)),
        pid: Some(1101),
        ppid: Some(1100),
        tmux: Some(HookTmuxRecord {
            session_name: Some("project".to_string()),
            native_id: None,
            pane_id: Some("%1".to_string()),
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("SessionStart".to_string()),
        observed_epoch: SHOWCASE_NOW_EPOCH - 2 * 60,
        harness_version: Some("1.0.0".to_string()),
    })?;
    Ok(())
}

fn unique_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "conspectus-scenario-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn git(root: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_scenario_materializes_and_renders() {
        for name in scenario_names() {
            let world = materialize(name).expect("materialize scenario");
            let graph = world.render_graph_json().expect("graph json");
            assert!(
                graph.contains("\"nodes\""),
                "scenario {name} should render graph JSON"
            );
            let table = world
                .render_table(Projection::Agent, RenderOptions::wide())
                .expect("session table");
            assert!(
                table.contains("ID") || table.is_empty(),
                "scenario {name} should render table output, got {table:?}"
            );
            let _tree = world.sessions_tree().expect("sessions tree");
        }
    }

    #[test]
    fn unknown_scenario_lists_valid_names() {
        let err = match materialize("missing") {
            Ok(_) => panic!("missing scenario should fail"),
            Err(err) => err.to_string(),
        };
        assert!(err.contains("unknown scenario `missing`"));
        assert!(err.contains("ambiguous-mux"));
    }

    #[test]
    fn process_cardinality_scenario_exposes_runtime_processes() {
        let world = materialize("process-cardinality").expect("materialize scenario");
        let snapshot = world.snapshot().expect("snapshot");
        let process_count = snapshot
            .nodes
            .iter()
            .filter(|node| matches!(node, crate::model::GraphNode::RuntimeProcess(_)))
            .count();
        assert_eq!(process_count, 2);
        assert!(
            snapshot
                .candidate_links
                .iter()
                .any(|link| { link.relation == crate::model::RelationKind::MuxContainsProcess })
        );
    }
}
