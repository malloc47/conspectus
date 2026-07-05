//! Session-pin discovery.
//!
//! Reads `[[pins.entries]]` TOML sections (ADR 0057) from project-local
//! `.conspectus.toml` files and the user-level config and surfaces them
//! on [`GraphSnapshot::pins`] for downstream consumption.
//!
//! Parallel structure to [`crate::discovery::declared`]: enumerate
//! stores (user config first, then per-root project configs), parse
//! each via the [`crate::pins`] schema module, and emit one
//! [`PinCandidate`] per valid entry plus diagnostics for malformed
//! files. The resolver pass (H-PIN-004) consumes the resulting sidecar
//! to perform mux-anchored binding and to synthesize the matching
//! `LinkedToMux` candidates with `LocalPin`/`GlobalPin` provenance per
//! ADR 0057.
//!
//! This pass is strictly read-only: no config file is created or
//! mutated. Mirrors the ADR 0014 / ADR 0029 read-only invariant.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::ConfigLoader;
use crate::discovery::DiscoveryContext;
use crate::model::{Diagnostic, GraphNode, GraphSnapshot, PinCandidate, PinMuxRef, Provenance};
use crate::pins::{PinEntry, PinsDocument, parse_pins_document};

pub fn apply_pins(snapshot: &mut GraphSnapshot, context: &DiscoveryContext, loader: &ConfigLoader) {
    let mut stores: Vec<PinStore> = Vec::new();

    if let Some(path) = loader.user_config_path()
        && path.is_file()
    {
        stores.push(PinStore {
            path,
            provenance: Provenance::GlobalPin,
        });
    }

    for path in local_pin_store_paths(snapshot, context, loader) {
        stores.push(PinStore {
            path,
            provenance: Provenance::LocalPin,
        });
    }

    // Local-pin entries shadow global-pin entries with the same id.
    // We track ids per-effective-store: an `id` that wins at the
    // project tier suppresses a same-id entry that loads from the
    // user config in the same snapshot.
    let mut effective_ids: BTreeSet<String> = BTreeSet::new();

    // Iterate stores in order of weakest precedence first so the
    // strongest tier overwrites; the candidate vector itself is sorted
    // by `GraphSnapshot::canonicalize` at the end so order in the
    // input vec doesn't affect output stability.
    stores.sort_by_key(|store| std::cmp::Reverse(store.provenance.precedence()));

    for store in stores {
        let Some(document) = read_pins_document(&store, &mut snapshot.diagnostics) else {
            continue;
        };

        for entry in document.entries() {
            if !effective_ids.insert(entry.id.clone()) {
                snapshot
                    .diagnostics
                    .push(shadowed_pin_diagnostic(&store, &entry.id));
                continue;
            }

            snapshot.pins.push(pin_candidate_from(&store, entry));
        }
    }

    snapshot.sync_pin_nodes();
    snapshot.canonicalize();
}

fn local_pin_store_paths(
    snapshot: &GraphSnapshot,
    context: &DiscoveryContext,
    loader: &ConfigLoader,
) -> Vec<PathBuf> {
    let mut seen_project_paths = BTreeSet::new();
    let mut paths = Vec::new();
    for root in project_config_search_roots(snapshot, context) {
        if let Some(path) = loader.locate_project_config(&root)
            && seen_project_paths.insert(path.clone())
        {
            paths.push(path);
        }
    }
    paths
}

fn project_config_search_roots(
    snapshot: &GraphSnapshot,
    context: &DiscoveryContext,
) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = BTreeSet::new();
    for root in context.roots() {
        push_search_root(&mut roots, &mut seen, root);
    }
    for node in &snapshot.nodes {
        match node {
            GraphNode::Repo(repo) => {
                for path in &repo.source_paths {
                    push_search_root(&mut roots, &mut seen, Path::new(path));
                }
                if let Some(parent) = Path::new(&repo.common_dir).parent()
                    && Path::new(&repo.common_dir)
                        .file_name()
                        .is_some_and(|name| name == ".git")
                {
                    push_search_root(&mut roots, &mut seen, parent);
                }
            }
            GraphNode::Checkout(checkout) => {
                push_search_root(&mut roots, &mut seen, Path::new(&checkout.root));
            }
            GraphNode::Workspace(workspace) => {
                push_search_root(&mut roots, &mut seen, Path::new(&workspace.root));
            }
            GraphNode::AgentSession(session) => {
                if let Some(cwd) = &session.cwd {
                    push_search_root(&mut roots, &mut seen, Path::new(cwd));
                }
            }
            GraphNode::MuxSession(mux) => {
                if let Some(cwd) = &mux.cwd {
                    push_search_root(&mut roots, &mut seen, Path::new(cwd));
                }
                if let Some(cwd) = &mux.active_pane_current_path {
                    push_search_root(&mut roots, &mut seen, Path::new(cwd));
                }
            }
            GraphNode::RuntimeProcess(process) => {
                if let Some(cwd) = &process.cwd {
                    push_search_root(&mut roots, &mut seen, Path::new(cwd));
                }
            }
            GraphNode::Pin(_) => {}
            GraphNode::Branch(_) | GraphNode::ForgePr(_) | GraphNode::Fork(_) => {}
        }
    }
    roots
}

fn push_search_root(
    roots: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
    root: impl AsRef<Path>,
) {
    let root = root.as_ref();
    if root.as_os_str().is_empty() || !root.is_absolute() {
        return;
    }
    let root = root.to_path_buf();
    if seen.insert(root.clone()) {
        roots.push(root);
    }
}

#[derive(Clone, Debug)]
struct PinStore {
    path: PathBuf,
    provenance: Provenance,
}

fn read_pins_document(store: &PinStore, diagnostics: &mut Vec<Diagnostic>) -> Option<PinsDocument> {
    let text = match fs::read_to_string(&store.path) {
        Ok(text) => text,
        Err(err) => {
            diagnostics.push(config_diagnostic(
                &store.path,
                format!("failed to read pins: {err}"),
            ));
            return None;
        }
    };

    match parse_pins_document(&text) {
        Ok(document) => Some(document),
        Err(err) => {
            diagnostics.push(config_diagnostic(
                &store.path,
                format!("failed to parse pins: {err}"),
            ));
            None
        }
    }
}

fn pin_candidate_from(store: &PinStore, entry: &PinEntry) -> PinCandidate {
    PinCandidate {
        id: entry.id.clone(),
        display_name: entry.display_name.clone(),
        harness: entry.harness.clone(),
        cwd: entry.cwd.clone(),
        mux: PinMuxRef {
            backend: entry.mux.backend.clone(),
            name: entry.mux.name.clone(),
            socket_name: entry.mux.socket_name.clone(),
        },
        launch_argv: entry
            .launch
            .as_ref()
            .filter(|launch| !launch.argv.is_empty())
            .map(|launch| launch.argv.clone()),
        reason: entry.reason.clone(),
        provenance: store.provenance,
        store_path: store.path.to_string_lossy().into_owned(),
        // Loader leaves the binding state empty; the resolver pass
        // populates it.
        binding: None,
    }
}

fn config_diagnostic(path: &Path, message: String) -> Diagnostic {
    Diagnostic::Config {
        path: path.display().to_string(),
        message,
    }
}

fn shadowed_pin_diagnostic(store: &PinStore, id: &str) -> Diagnostic {
    // The shadow is a config concern; matches how
    // `discovery::declared` surfaces same-id collisions today.
    Diagnostic::Config {
        path: store.path.display().to_string(),
        message: format!(
            "pin `{id}` is shadowed by a same-id project-local pin (this entry skipped)"
        ),
    }
}

#[cfg(test)]
mod tests {
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
            Some(socket_name) => format!(
                r#"{{ backend = "tmux", name = "{mux_name}", socket_name = "{socket_name}" }}"#
            ),
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
        apply_pins(&mut snapshot, &context, &loader);

        assert!(snapshot.pins.is_empty());
        assert!(snapshot.diagnostics.is_empty());
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
        apply_pins(&mut snapshot, &context, &loader);

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

        apply_pins(&mut snapshot, &context, &loader);

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
        apply_pins(&mut snapshot, &context, &loader);

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
        apply_pins(&mut snapshot, &context, &loader);

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
        apply_pins(&mut snapshot, &context, &loader);

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
        apply_pins(&mut snapshot, &context, &loader);

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
        apply_pins(&mut snapshot, &context, &loader);

        assert_eq!(snapshot.pins.len(), 1);
    }

    #[test]
    fn xdg_relative_constant_matches_loader_layout() {
        // Sanity check: our test setup writes to the same path the
        // loader will read from. Lifts the magic strings out of
        // individual tests.
        assert!(USER_CONFIG_RELATIVE.ends_with("config.toml"));
    }
}
