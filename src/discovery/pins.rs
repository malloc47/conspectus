//! Session-pin discovery.
//!
//! Reads `[[pins.entries]]` TOML sections (ADR 0057) from project-local
//! `.conspectus.toml` files and the user-level config and surfaces them
//! on [`GraphSnapshot::pins`] for downstream consumption.
//!
//! Parallel structure to [`crate::discovery::declared`]: enumerate
//! stores (user config first, then per-root project configs), parse
//! each via the `crate::pins` schema module, and emit one
//! [`PinCandidate`] per valid entry plus diagnostics for malformed
//! files. The resolver pass consumes the resulting sidecar
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

pub fn apply_pins(
    snapshot: &mut GraphSnapshot,
    context: &DiscoveryContext,
    loader: &ConfigLoader,
    registry_stores: &[PathBuf],
) {
    let mut stores: Vec<PinStore> = Vec::new();

    if let Some(path) = loader.user_config_path()
        && path.is_file()
    {
        stores.push(PinStore {
            path,
            provenance: Provenance::GlobalPin,
        });
    }

    for path in local_pin_store_paths(snapshot, context, loader, registry_stores) {
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
    registry_stores: &[PathBuf],
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
    // Registry-recorded project stores are already full
    // `.conspectus.toml` paths, so they're appended directly (not walked
    // up from a search root). Dedup shares the same set as the
    // root-derived paths, and the registry read already dropped stores
    // whose file no longer exists.
    for path in registry_stores {
        if path.is_file() && seen_project_paths.insert(path.clone()) {
            paths.push(path.clone());
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
#[path = "pins_tests.rs"]
mod tests;
