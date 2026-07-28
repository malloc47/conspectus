// Extracted from pins.rs H-HYG-011 rolling wave via #[path = "pins_tests.rs"] mod tests;
use super::*;
use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME, USER_CONFIG_RELATIVE};
use crate::model::{AgentSessionId, AgentSessionNode};
use tempfile::TempDir;

fn project_pin_toml(id: &str, mux_name: &str) -> String {
    format!(
        r#"
[pins]
schema_version = 1

[[pins.entries]]
id = "{id}"
display_name = "{id}"
harness = "codex"
cwd = "/home/me/work/repo"
mux = {{ backend = "tmux", name = "{mux_name}" }}
"#,
    )
}

fn user_pin_toml(id: &str, mux_name: &str, socket: Option<&str>) -> String {
    let mux_table = match socket {
        None => format!(r#"{{ backend = "tmux", name = "{mux_name}" }}"#),
        Some(socket_name) => {
            format!(r#"{{ backend = "tmux", name = "{mux_name}", socket_name = "{socket_name}" }}"#)
        }
    };
    format!(
        r#"
[pins]
schema_version = 1

[[pins.entries]]
id = "{id}"
display_name = "{id}"
harness = "codex"
cwd = "/home/me/scratch"
mux = {mux_table}
"#,
    )
}

fn loader_with(home: &Path, xdg_config: &Path) -> ConfigLoader {
    ConfigLoader::new()
        .with_home(home.to_path_buf())
        .with_xdg_config_home(xdg_config.to_path_buf())
}

fn context_at(roots: &[&Path]) -> DiscoveryContext {
    DiscoveryContext::from_roots(roots.iter().map(|p| p.to_path_buf())).expect("ctx")
}

#[test]
fn empty_when_no_config_files_exist() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("mkdir root");
    let loader = loader_with(&home, &xdg);
    let context = context_at(&[&root]);

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(&mut snapshot, &context, &loader, &[]);

    assert!(snapshot.pins.is_empty());
    assert!(snapshot.diagnostics.is_empty());
}

#[test]
fn loads_registry_recorded_pin_store_outside_every_scan_root() {
    // H-PIN-ROOT-001: a pin lives in a repo that is neither a scan root
    // nor referenced by any discovered node cwd. Without the registry
    // it would be invisible; recording its store path keeps it loading.
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let scan_root = tmp.path().join("scan");
    let outside_repo = tmp.path().join("elsewhere/repo");
    fs::create_dir_all(&scan_root).expect("mkdir scan");
    fs::create_dir_all(&outside_repo).expect("mkdir outside");
    let store = outside_repo.join(PROJECT_CONFIG_FILENAME);
    fs::write(&store, project_pin_toml("ingest", "ingest")).expect("write outside config");

    let loader = loader_with(&home, &xdg);
    // The context only knows about `scan_root`, which holds no config.
    let context = context_at(&[&scan_root]);

    // Baseline: without the registry the pin is invisible.
    let mut baseline = GraphSnapshot::empty();
    apply_pins(&mut baseline, &context, &loader, &[]);
    assert!(
        baseline.pins.is_empty(),
        "pin outside the scan root should be invisible without the registry",
    );

    // With the store recorded in the registry it loads as a local pin.
    let mut snapshot = GraphSnapshot::empty();
    apply_pins(
        &mut snapshot,
        &context,
        &loader,
        std::slice::from_ref(&store),
    );
    assert_eq!(snapshot.pins.len(), 1);
    assert_eq!(snapshot.pins[0].id, "ingest");
    assert_eq!(snapshot.pins[0].provenance, Provenance::LocalPin);
    assert!(snapshot.diagnostics.is_empty());
}

#[test]
fn registry_store_that_no_longer_exists_is_skipped() {
    // A stale registry entry (repo since deleted) must not error or
    // synthesize a phantom pin.
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let scan_root = tmp.path().join("scan");
    fs::create_dir_all(&scan_root).expect("mkdir scan");
    let missing = tmp.path().join("gone/repo").join(PROJECT_CONFIG_FILENAME);

    let loader = loader_with(&home, &xdg);
    let context = context_at(&[&scan_root]);

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(&mut snapshot, &context, &loader, &[missing]);
    assert!(snapshot.pins.is_empty());
    assert!(snapshot.diagnostics.is_empty());
}

#[test]
fn registry_store_already_covered_by_scan_root_is_not_double_loaded() {
    // When a registry entry names the same store a scan root already
    // locates, the pin loads exactly once (no same-id shadow diagnostic).
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("mkdir root");
    let store = root.join(PROJECT_CONFIG_FILENAME);
    fs::write(&store, project_pin_toml("ingest", "ingest")).expect("write project config");

    let loader = loader_with(&home, &xdg);
    let context = context_at(&[&root]);

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(&mut snapshot, &context, &loader, &[store]);
    assert_eq!(snapshot.pins.len(), 1, "no double load");
    assert!(
        snapshot.diagnostics.is_empty(),
        "no spurious shadow diagnostic: {:?}",
        snapshot.diagnostics,
    );
}

#[test]
fn loads_project_pin_as_local_pin() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("mkdir root");
    fs::write(
        root.join(PROJECT_CONFIG_FILENAME),
        project_pin_toml("ingest", "ingest"),
    )
    .expect("write project config");

    let loader = loader_with(&home, &xdg);
    let context = context_at(&[&root]);

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(&mut snapshot, &context, &loader, &[]);

    assert_eq!(snapshot.pins.len(), 1);
    let pin = &snapshot.pins[0];
    assert_eq!(pin.id, "ingest");
    assert_eq!(pin.harness, "codex");
    assert_eq!(pin.mux.native_id(), "tmux:ingest");
    assert_eq!(pin.provenance, Provenance::LocalPin);
    assert!(snapshot.diagnostics.is_empty());
}

#[test]
fn loads_project_pin_from_observed_session_cwd_outside_scan_root() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let scan_root = tmp.path().join("scan");
    let project = tmp.path().join("project");
    let nested = project.join("subdir");
    fs::create_dir_all(&scan_root).expect("mkdir scan root");
    fs::create_dir_all(&nested).expect("mkdir nested project dir");
    fs::write(
        project.join(PROJECT_CONFIG_FILENAME),
        project_pin_toml("observed", "observed"),
    )
    .expect("write project config");

    let loader = loader_with(&home, &xdg);
    let context = context_at(&[&scan_root]);

    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "s1"),
            "codex".to_string(),
        )
        .with_cwd(nested.to_string_lossy().into_owned()),
    ));

    apply_pins(&mut snapshot, &context, &loader, &[]);

    assert_eq!(snapshot.pins.len(), 1);
    assert_eq!(snapshot.pins[0].id, "observed");
    assert_eq!(snapshot.pins[0].provenance, Provenance::LocalPin);
    assert!(snapshot.diagnostics.is_empty());
}

#[test]
fn loads_user_pin_as_global_pin() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let user_dir = xdg.join("conspectus");
    fs::create_dir_all(&user_dir).expect("mkdir user");
    fs::write(
        user_dir.join("config.toml"),
        user_pin_toml("global-ingest", "ingest", None),
    )
    .expect("write user config");
    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("mkdir root");

    let loader = loader_with(&home, &xdg);
    let context = context_at(&[&root]);

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(&mut snapshot, &context, &loader, &[]);

    assert_eq!(snapshot.pins.len(), 1);
    let pin = &snapshot.pins[0];
    assert_eq!(pin.id, "global-ingest");
    assert_eq!(pin.provenance, Provenance::GlobalPin);
}

#[test]
fn local_pin_shadows_same_id_user_pin() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let user_dir = xdg.join("conspectus");
    fs::create_dir_all(&user_dir).expect("mkdir user");
    fs::write(
        user_dir.join("config.toml"),
        user_pin_toml("ingest", "global-mux", None),
    )
    .expect("write user config");
    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("mkdir root");
    fs::write(
        root.join(PROJECT_CONFIG_FILENAME),
        project_pin_toml("ingest", "project-mux"),
    )
    .expect("write project config");

    let loader = loader_with(&home, &xdg);
    let context = context_at(&[&root]);

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(&mut snapshot, &context, &loader, &[]);

    assert_eq!(snapshot.pins.len(), 1);
    let pin = &snapshot.pins[0];
    assert_eq!(pin.provenance, Provenance::LocalPin);
    assert_eq!(pin.mux.name, "project-mux");

    let shadow = snapshot
        .diagnostics
        .iter()
        .find_map(|d| match d {
            Diagnostic::Config { message, .. } if message.contains("shadowed") => {
                Some(message.clone())
            }
            _ => None,
        })
        .expect("shadow diagnostic");
    assert!(shadow.contains("ingest"));
}

#[test]
fn non_default_socket_round_trips_into_native_id() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let user_dir = xdg.join("conspectus");
    fs::create_dir_all(&user_dir).expect("mkdir user");
    fs::write(
        user_dir.join("config.toml"),
        user_pin_toml("scratch", "ingest", Some("scratch")),
    )
    .expect("write user config");
    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("mkdir root");

    let loader = loader_with(&home, &xdg);
    let context = context_at(&[&root]);

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(&mut snapshot, &context, &loader, &[]);

    let pin = snapshot.pins.first().expect("pin present");
    assert_eq!(pin.mux.socket_name.as_deref(), Some("scratch"));
    assert_eq!(pin.mux.native_id(), "tmux:scratch:ingest");
}

#[test]
fn malformed_config_emits_diagnostic_and_does_not_block_other_stores() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let user_dir = xdg.join("conspectus");
    fs::create_dir_all(&user_dir).expect("mkdir user");
    fs::write(
        user_dir.join("config.toml"),
        "this is = not valid toml = at all",
    )
    .expect("write malformed user config");

    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("mkdir root");
    fs::write(
        root.join(PROJECT_CONFIG_FILENAME),
        project_pin_toml("survivor", "survivor"),
    )
    .expect("write good project config");

    let loader = loader_with(&home, &xdg);
    let context = context_at(&[&root]);

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(&mut snapshot, &context, &loader, &[]);

    assert_eq!(snapshot.pins.len(), 1, "good project pin still loads");
    let pin = &snapshot.pins[0];
    assert_eq!(pin.id, "survivor");

    let parse_message = snapshot
        .diagnostics
        .iter()
        .find_map(|d| match d {
            Diagnostic::Config { message, .. } if message.contains("failed to parse pins") => {
                Some(message.clone())
            }
            _ => None,
        })
        .expect("parse diagnostic");
    assert!(parse_message.contains("failed to parse pins"));
}

#[test]
fn duplicate_project_root_is_only_scanned_once() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("mkdir root");
    fs::write(
        root.join(PROJECT_CONFIG_FILENAME),
        project_pin_toml("one", "one"),
    )
    .expect("write project config");

    let loader = loader_with(&home, &xdg);
    // Pass the same root twice.
    let context = context_at(&[&root, &root]);

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(&mut snapshot, &context, &loader, &[]);

    assert_eq!(snapshot.pins.len(), 1);
}

#[test]
fn xdg_relative_constant_matches_loader_layout() {
    // Sanity check: our test setup writes to the same path the
    // loader will read from. Lifts the magic strings out of
    // individual tests.
    assert!(USER_CONFIG_RELATIVE.ends_with("config.toml"));
}
