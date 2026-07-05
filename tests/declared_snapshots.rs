//! End-to-end JSON and table snapshots for declared-link discovery.
//!
//! Each scenario sets up a temp directory holding either a project
//! `.conspectus.toml`, a user config under
//! `<temp>/.config/conspectus/config.toml`, or both, then runs
//! `discover_local_with` and snapshots the resolved graph or the
//! rendered table. Path-normalization rewrites the temp tree to
//! `/fixture` so reruns are byte-stable.

use std::fs;
use std::path::Path;

use conspectus::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
use conspectus::discovery::harness::codex::HARNESS_KEY as CODEX_HARNESS_KEY;
use conspectus::discovery::harness::fixtures::{CodexSessionRecord, HarnessFixture};
use conspectus::discovery::tmux::FakeTmux;
use conspectus::discovery::{LocalDiscoveryConfig, discover_local_with};
use conspectus::model::{GraphNode, GraphSnapshot, LinkEndpoint, NodeId};
use conspectus::output::render::Projection;
use conspectus::output::render_graph_json;
use conspectus::output::table;
use conspectus::resolve::resolve_snapshot;

mod support;

#[test]
fn local_declared_link_with_matched_target_snapshot() {
    let fixture = DeclaredFixture::with_session_and_mux();
    fixture.write_project_config(&active_session_mux("local-link"));

    let snapshot = run(&fixture);
    insta::assert_snapshot!("local_declared_session_to_mux", snapshot);
}

#[test]
fn global_declared_link_with_matched_target_snapshot() {
    let fixture = DeclaredFixture::with_session_and_mux();
    fixture.write_user_config(&active_session_mux("global-link"));

    let snapshot = run(&fixture);
    insta::assert_snapshot!("global_declared_session_to_mux", snapshot);
}

#[test]
fn local_declared_overrides_user_declared_in_resolution_snapshot() {
    let fixture = DeclaredFixture::with_session_and_mux();
    // Both configs reference the same source/target pair so the resolver
    // has to pick between them. Local wins per ADR 0014.
    fixture.write_project_config(&active_session_mux("local-link"));
    fixture.write_user_config(&active_session_mux("global-link"));

    let snapshot = run(&fixture);
    insta::assert_snapshot!("local_declared_overrides_user", snapshot);
}

#[test]
fn ignored_declared_state_snapshot() {
    let fixture = DeclaredFixture::with_session_and_mux();
    fixture.write_project_config(&ignored_session_mux("ignore-noisy", "stale"));

    let snapshot = run(&fixture);
    insta::assert_snapshot!("ignored_declared_session_to_mux", snapshot);
}

#[test]
fn overridden_declared_state_snapshot() {
    let fixture = DeclaredFixture::with_session_and_mux();
    fixture.write_project_config(&overridden_session_mux(
        "old-link",
        "new-link",
        "replaced by user",
    ));

    let snapshot = run(&fixture);
    insta::assert_snapshot!("overridden_declared_session_to_mux", snapshot);
}

#[test]
fn unresolved_declared_endpoint_snapshot() {
    let fixture = DeclaredFixture::without_discovery();
    // No harness or tmux providers run; the declared target therefore
    // has nothing to match against in the graph and surfaces as
    // unresolved endpoint evidence.
    fixture.write_project_config(&active_session_mux("orphan-link"));

    let snapshot = run(&fixture);
    insta::assert_snapshot!("unresolved_declared_endpoint", snapshot);
}

#[test]
fn agent_table_with_declared_mux_relationship_snapshot() {
    let fixture = DeclaredFixture::with_session_and_mux();
    fixture.write_project_config(&active_session_mux("declared-editor"));

    let table = run_table(&fixture, Projection::Agent);
    insta::assert_snapshot!("agent_table_declared_mux_relationship", table);
}

fn active_session_mux(id: &str) -> String {
    declared_link_toml(id, "active", None, None)
}

fn ignored_session_mux(id: &str, reason: &str) -> String {
    declared_link_toml(id, "ignored", Some(reason), None)
}

fn overridden_session_mux(id: &str, replacement: &str, reason: &str) -> String {
    declared_link_toml(id, "overridden", Some(reason), Some(replacement))
}

fn declared_link_toml(
    id: &str,
    state: &str,
    reason: Option<&str>,
    overridden_by: Option<&str>,
) -> String {
    let mut text = format!(
        r#"[declared]
schema_version = 1

[[declared.links]]
id = "{id}"
relation = "linked_to_mux"
state = "{state}"
source = {{ type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "session-x" }}
target = {{ type = "mux_session", native_id = "tmux:editor" }}
"#
    );
    if let Some(reason) = reason {
        text.push_str(&format!("reason = \"{reason}\"\n"));
    }
    if let Some(overridden_by) = overridden_by {
        text.push_str(&format!("overridden_by = \"{overridden_by}\"\n"));
    }
    text
}

fn run(fixture: &DeclaredFixture) -> String {
    let snapshot = discover_local_with([fixture.path()], fixture.config()).expect("discover");
    let rendered = render_graph_json(&resolve_snapshot(snapshot)).expect("render");
    let normalized = fixture.normalize(&rendered);
    support::redact_freshness_epoch(&normalized)
}

fn run_table(fixture: &DeclaredFixture, projection: Projection) -> String {
    let snapshot = discover_local_with([fixture.path()], fixture.config()).expect("discover");
    let mut resolved = resolve_snapshot(snapshot);
    fixture.normalize_snapshot_paths(&mut resolved);
    let rendered = table::render(&resolved, projection);
    fixture.normalize(&rendered)
}

/// Test fixture for declared-link snapshots.
///
/// `with_session_and_mux` sets up a Codex session record + a fake tmux
/// session that share a `cwd`, so a declared `agent_session ↔
/// mux_session` link has both endpoints present in the discovered
/// graph. `without_discovery` configures `LocalDiscoveryConfig::empty()`
/// so declared endpoints surface as `Unresolved`.
struct DeclaredFixture {
    temp: tempfile::TempDir,
    user_config_path: std::path::PathBuf,
    project_root: std::path::PathBuf,
    with_session: bool,
}

impl DeclaredFixture {
    fn with_session_and_mux() -> Self {
        let temp = tempfile::TempDir::new().expect("temp");
        let project_root = temp.path().join("project");
        fs::create_dir_all(&project_root).expect("project dir");
        let state_dir = temp.path().join(".state");
        fs::create_dir_all(&state_dir).expect("state dir");
        HarnessFixture::at(&state_dir)
            .write_codex_session(&CodexSessionRecord::new("session-x").with_cwd("/work"))
            .expect("codex session");
        Self {
            user_config_path: temp.path().join(".config/conspectus/config.toml"),
            project_root,
            temp,
            with_session: true,
        }
    }

    fn without_discovery() -> Self {
        let temp = tempfile::TempDir::new().expect("temp");
        let project_root = temp.path().join("project");
        fs::create_dir_all(&project_root).expect("project dir");
        Self {
            user_config_path: temp.path().join(".config/conspectus/config.toml"),
            project_root,
            temp,
            with_session: false,
        }
    }

    fn path(&self) -> &Path {
        &self.project_root
    }

    fn write_project_config(&self, body: &str) {
        fs::write(self.project_root.join(PROJECT_CONFIG_FILENAME), body).expect("project config");
    }

    fn write_user_config(&self, body: &str) {
        fs::create_dir_all(self.user_config_path.parent().expect("parent")).expect("xdg dir");
        fs::write(&self.user_config_path, body).expect("user config");
    }

    fn config(&self) -> LocalDiscoveryConfig {
        let loader = ConfigLoader::new()
            .with_home(self.temp.path())
            .with_xdg_config_home(self.temp.path().join(".config"));
        let mut config = LocalDiscoveryConfig::empty();
        config.declared_config_loader = Some(loader);
        if self.with_session {
            let state_dir = self.temp.path().join(".state");
            config = config
                .with_harness_state_root(CODEX_HARNESS_KEY, state_dir.join("codex"))
                .with_tmux_runner(FakeTmux::with_sessions(
                    "editor\t/work\t1700000500\t1700000000\n",
                ));
        }
        config
    }

    fn normalize(&self, rendered: &str) -> String {
        let mut out = rendered.replace(&self.temp.path().to_string_lossy().to_string(), "/fixture");
        if let Some(name) = self.temp.path().file_name().and_then(|s| s.to_str()) {
            out = out.replace(name, "fixture");
        }
        out
    }

    /// Rewrite path-derived fields on the snapshot so the rendered short
    /// row identifier (H-TBL-002) — which hashes the full `NodeId`,
    /// including `AgentSessionId.state_scope` — is stable across runs.
    fn normalize_snapshot_paths(&self, snapshot: &mut GraphSnapshot) {
        for node in &mut snapshot.nodes {
            if let GraphNode::AgentSession(node) = node {
                node.cwd = node.cwd.as_ref().map(|path| self.normalize(path));
                node.id.state_scope = self.normalize(&node.id.state_scope);
            }
        }
        for link in &mut snapshot.candidate_links {
            self.normalize_node_id(&mut link.source);
            if let LinkEndpoint::Node { id } = &mut link.target {
                self.normalize_node_id(id);
            }
        }
    }

    fn normalize_node_id(&self, id: &mut NodeId) {
        if let NodeId::AgentSession(agent_id) = id {
            agent_id.state_scope = self.normalize(&agent_id.state_scope);
        }
    }
}
