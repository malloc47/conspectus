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
use crate::discovery::harness::claude_code::HARNESS_KEY as CLAUDE_CODE_HARNESS_KEY;
use crate::discovery::harness::codex::HARNESS_KEY as CODEX_HARNESS_KEY;
use crate::discovery::harness::fixtures::{
    ClaudeCodeSessionRecord, CodexSessionRecord, HarnessFixture,
};
use crate::discovery::tmux::FakeTmux;
use crate::discovery::{LocalDiscoveryConfig, discover_local_with};
use crate::filter::RowFilter;
use crate::hook::{HookRecord, HookStore, HookTmuxRecord, SCHEMA_VERSION};
use crate::model::GraphSnapshot;
use crate::output::table::{self, RenderOptions};
use crate::resolve::resolve_snapshot;
use crate::tui::rows::RowTree;
use crate::tui::rows::sessions::{SessionsBuildInputsFromConn, build_sessions_tree_from_conn};
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
        let conn = crate::query::materialize_snapshot(&snapshot)?;
        Ok(table::render_with_conn(&conn, projection, &options)?)
    }

    pub fn render_node_show(&self, id: &str, color: bool) -> Result<String> {
        let snapshot = self.snapshot()?;
        let conn = crate::query::materialize_snapshot(&snapshot)?;
        let id = match crate::output::node_show::resolve_node_id_from_conn(&conn, id)? {
            Ok(id) => id,
            Err(err) => bail!("{err}"),
        };
        Ok(crate::output::node_show::render_node_show_from_conn(
            &conn, &id, color,
        )?)
    }

    pub fn sessions_tree(&self) -> Result<RowTree> {
        let snapshot = self.snapshot()?;
        let conn = crate::query::materialize_snapshot(&snapshot)?;
        Ok(build_sessions_tree_from_conn(
            SessionsBuildInputsFromConn {
                conn: &conn,
                grouping: SessionsGrouping::Graph,
                home: Some(&self.root),
                now: Some(1_700_000_600),
                cwd: None,
                filter: RowFilter::default(),
            },
        )?)
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
            refresh_interval: Duration::from_secs(24 * 60 * 60),
            mux_preview_interval: Duration::from_secs(24 * 60 * 60),
            live_preview_enabled: false,
            color,
            current_tmux_session: None,
            theme: crate::tui::Theme::default(),
        }
    }

    fn discovery_config(&self) -> LocalDiscoveryConfig {
        let mut config = LocalDiscoveryConfig::empty()
            .with_harness_state_root(CODEX_HARNESS_KEY, self.harness.codex_state_root())
            .with_harness_state_root(
                CLAUDE_CODE_HARNESS_KEY,
                self.harness.claude_code_state_root(),
            )
            .with_hook_sidecar_root(&self.hook_root)
            .without_codex_log();
        if !self.tmux_rows.is_empty() {
            config = config.with_tmux_runner(FakeTmux::with_sessions(self.tmux_stdout()));
        }
        if let Some(body) = &self.gh_pull_requests {
            config = config.with_forge_runner(FakeGh::with_pull_requests(body.clone()));
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
                    .with_cwd(path_string(cwd))
                    .with_timestamp("2026-01-02T03:04:05Z"),
            )
            .map(|_| ())
    }

    fn write_claude_code_session(&self, session_key: &str, cwd: &Path) -> Result<()> {
        self.harness
            .write_claude_code_session(
                &ClaudeCodeSessionRecord::new(session_key, path_string(cwd))
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

    fn write_atelier_config(&self, text: &str) -> Result<()> {
        fs::write(self.root.join("atelier.toml"), text).map_err(Into::into)
    }

    fn write_fork_index(&self, text: &str) -> Result<()> {
        let path = self.root.join(".atelier/forks/index.toml");
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
        self.cwd = Some(path_string(cwd));
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
        self.active_pane_current_path = Some(path_string(cwd));
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
            cwd: Some(path_string(&work)),
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

    world.write_claude_code_session(session_a, &work)?;
    world.write_claude_code_session(session_b, &work)?;
    world.add_tmux_row(
        TmuxReplayRow::new("pair")
            .with_cwd(&work)
            .with_active_pane("claude", 6101, &work, "claude"),
    );

    for (session_key, pid, ppid, pane_id, observed_epoch) in [
        (session_a, 6101, 6000, "%1", 1_700_000_500),
        (session_b, 6201, 6000, "%2", 1_700_000_540),
    ] {
        world.write_hook_record(HookRecord {
            schema_version: SCHEMA_VERSION,
            harness_key: "claude-code".to_string(),
            session_key: session_key.to_string(),
            cwd: Some(path_string(&work)),
            pid: Some(pid),
            ppid: Some(ppid),
            tmux: Some(HookTmuxRecord {
                session_name: Some("pair".to_string()),
                native_id: None,
                pane_id: Some(pane_id.to_string()),
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

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
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
