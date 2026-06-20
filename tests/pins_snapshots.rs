use std::fs;
use std::path::Path;

use conspectus::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
use conspectus::discovery::{LocalDiscoveryConfig, discover_local_with};
use conspectus::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId, PinCandidate,
    PinMuxRef, Provenance, RelationKind, SourceMetadata,
};
use conspectus::output::render_graph_json;
use conspectus::output::table::{Projection, RenderOptions, render_with};
use conspectus::resolve::resolve_snapshot;

fn agent(key: &str, cwd: &str) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new("codex", "/state", key),
        harness_key: "codex".to_string(),
        cwd: Some(cwd.to_string()),
        title: None,
        last_message_preview: None,
        last_active_epoch: None,
        session_kind: None,
    })
}

fn mux(native_id: &str, cwd: &str) -> GraphNode {
    // Callers pass the fully-prefixed form like "tmux:bound" for
    // ergonomics, but `MuxSessionNode.native_id` should hold the
    // bare post-backend portion (`bound`) per production
    // discovery. Strip the `tmux:` prefix here so the fixture
    // matches what discovery emits and the resolver's lookup
    // (`mux_index`) reconstructs the same prefixed key on the
    // way out.
    let bare = native_id.strip_prefix("tmux:").unwrap_or(native_id);
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(native_id),
        backend: "tmux".to_string(),
        native_id: bare.to_string(),
        cwd: Some(cwd.to_string()),
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    })
}

fn linked_to_mux(
    id: &str,
    session_key: &str,
    mux_native_id: &str,
    provenance: Provenance,
) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source: NodeId::AgentSession(AgentSessionId::new("codex", "/state", session_key)),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(MuxSessionId::new(mux_native_id)),
        },
        relation: RelationKind::LinkedToMux,
        provenance,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn declared_pin_bind(id: &str, pin_id: &str, session_key: &str, mux_native_id: &str) -> GraphLink {
    let mut fields = Metadata::new();
    fields.insert(
        "label".to_string(),
        serde_json::Value::String(format!("pin:{pin_id}")),
    );
    GraphLink {
        source_metadata: SourceMetadata {
            adapter: "declared".to_string(),
            evidence: Some("pin bind override".to_string()),
            fields,
            freshness_epoch: None,
        },
        ..linked_to_mux(id, session_key, mux_native_id, Provenance::LocalDeclared)
    }
}

fn pin(
    id: &str,
    cwd: &str,
    mux_name: &str,
    socket_name: Option<&str>,
    provenance: Provenance,
) -> PinCandidate {
    PinCandidate {
        id: id.to_string(),
        display_name: id.to_string(),
        harness: "codex".to_string(),
        cwd: cwd.to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: mux_name.to_string(),
            socket_name: socket_name.map(str::to_string),
        },
        launch_argv: None,
        reason: None,
        provenance,
        store_path: match provenance {
            Provenance::GlobalPin => "/home/op/.config/conspectus/config.toml",
            _ => "/home/op/src/work/.conspectus.toml",
        }
        .to_string(),
        binding: None,
    }
}

fn pin_state_matrix() -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.extend([
        agent("bound-alpha", "/home/op/src/work"),
        agent("ambig-strong", "/home/op/src/work"),
        agent("ambig-weak", "/home/op/src/work"),
        agent("drift-alpha", "/home/op/src/elsewhere"),
        agent("socket-alpha", "/home/op/src/work"),
        mux("tmux:bound", "/home/op/src/work"),
        mux("tmux:stale", "/home/op/src/work"),
        mux("tmux:ambig", "/home/op/src/work"),
        mux("tmux:drift", "/home/op/src/work"),
        mux("tmux:work:socketed", "/home/op/src/work"),
    ]);
    snapshot.candidate_links.extend([
        linked_to_mux(
            "bound-discovered",
            "bound-alpha",
            "tmux:bound",
            Provenance::Discovered,
        ),
        linked_to_mux(
            "ambig-strong",
            "ambig-strong",
            "tmux:ambig",
            Provenance::StrongDiscovered,
        ),
        linked_to_mux(
            "ambig-weak",
            "ambig-weak",
            "tmux:ambig",
            Provenance::Discovered,
        ),
        linked_to_mux(
            "drift-discovered",
            "drift-alpha",
            "tmux:drift",
            Provenance::Discovered,
        ),
        linked_to_mux(
            "socket-discovered",
            "socket-alpha",
            "tmux:work:socketed",
            Provenance::Discovered,
        ),
    ]);
    snapshot.pins.extend([
        pin(
            "bound-local",
            "/home/op/src/work",
            "bound",
            None,
            Provenance::LocalPin,
        ),
        pin(
            "stale-local",
            "/home/op/src/work",
            "stale",
            None,
            Provenance::LocalPin,
        ),
        pin(
            "unbound-local",
            "/home/op/src/work",
            "missing",
            None,
            Provenance::LocalPin,
        ),
        pin(
            "ambig-local",
            "/home/op/src/work",
            "ambig",
            None,
            Provenance::LocalPin,
        ),
        pin(
            "drift-local",
            "/home/op/src/work",
            "drift",
            None,
            Provenance::LocalPin,
        ),
        pin(
            "socket-local",
            "/home/op/src/work",
            "socketed",
            Some("work"),
            Provenance::LocalPin,
        ),
    ]);
    resolve_snapshot(snapshot)
}

fn declared_bind_override() -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.extend([
        agent("chosen-by-declared", "/home/op/src/work"),
        agent("strong-discovered", "/home/op/src/work"),
        mux("tmux:bound-by-declared", "/home/op/src/work"),
    ]);
    snapshot.candidate_links.extend([
        linked_to_mux(
            "strong-discovered",
            "strong-discovered",
            "tmux:bound-by-declared",
            Provenance::StrongDiscovered,
        ),
        declared_pin_bind(
            "pin-bind-override",
            "override-local",
            "chosen-by-declared",
            "tmux:bound-by-declared",
        ),
    ]);
    snapshot.pins.push(pin(
        "override-local",
        "/home/op/src/work",
        "bound-by-declared",
        None,
        Provenance::LocalPin,
    ));
    resolve_snapshot(snapshot)
}

fn pins_toml(entries: &[(&str, &str, &str)]) -> String {
    let mut text = "[pins]\nschema_version = 1\n".to_string();
    for (id, display_name, mux_name) in entries {
        text.push_str(&format!(
            r#"
[[pins.entries]]
id = "{id}"
display_name = "{display_name}"
harness = "codex"
cwd = "/home/op/src/work"
mux = {{ backend = "tmux", name = "{mux_name}" }}
"#
        ));
    }
    text
}

struct PinDiscoveryFixture {
    temp: tempfile::TempDir,
    project_root: std::path::PathBuf,
    user_config_path: std::path::PathBuf,
}

impl PinDiscoveryFixture {
    fn new() -> Self {
        let temp = tempfile::TempDir::new().expect("temp");
        let project_root = temp.path().join("project");
        fs::create_dir_all(&project_root).expect("project dir");
        Self {
            user_config_path: temp.path().join(".config/conspectus/config.toml"),
            project_root,
            temp,
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
        LocalDiscoveryConfig::empty().with_declared_config_loader(
            ConfigLoader::new()
                .with_home(self.temp.path())
                .with_xdg_config_home(self.temp.path().join(".config")),
        )
    }

    fn run_discovered_graph_json(&self) -> String {
        let snapshot = discover_local_with([self.path()], self.config()).expect("discover");
        let rendered = render_graph_json(&snapshot).expect("render");
        self.normalize(&rendered)
    }

    fn normalize(&self, rendered: &str) -> String {
        let mut out = rendered.replace(&self.temp.path().to_string_lossy().to_string(), "/fixture");
        if let Some(name) = self.temp.path().file_name().and_then(|s| s.to_str()) {
            out = out.replace(name, "fixture");
        }
        out
    }
}

#[test]
fn pin_state_matrix_graph_json_snapshot() {
    let rendered = render_graph_json(&pin_state_matrix()).expect("render graph json");

    insta::assert_snapshot!("pin_state_matrix_graph_json", rendered);
}

#[test]
fn pin_state_matrix_agent_table_snapshot() {
    let rendered = render_with(
        &pin_state_matrix(),
        Projection::Agent,
        &RenderOptions::wide(),
    );

    insta::assert_snapshot!("pin_state_matrix_agent_table", rendered);
}

#[test]
fn pin_local_over_global_discovery_shadow_graph_json_snapshot() {
    let fixture = PinDiscoveryFixture::new();
    fixture.write_user_config(&pins_toml(&[("ingest", "global ingest", "global-mux")]));
    fixture.write_project_config(&pins_toml(&[("ingest", "project ingest", "project-mux")]));

    insta::assert_snapshot!(
        "pin_local_over_global_discovery_shadow_graph_json",
        fixture.run_discovered_graph_json()
    );
}

#[test]
fn pin_duplicate_config_graph_json_snapshot() {
    let fixture = PinDiscoveryFixture::new();
    fixture.write_user_config(&pins_toml(&[("global-only", "global only", "global-mux")]));
    fixture.write_project_config(&pins_toml(&[
        ("duplicate", "first duplicate", "first-mux"),
        ("duplicate", "second duplicate", "second-mux"),
    ]));

    insta::assert_snapshot!(
        "pin_duplicate_config_graph_json",
        fixture.run_discovered_graph_json()
    );
}

#[test]
fn pin_declared_bind_override_graph_json_snapshot() {
    let rendered = render_graph_json(&declared_bind_override()).expect("render graph json");

    insta::assert_snapshot!("pin_declared_bind_override_graph_json", rendered);
}
