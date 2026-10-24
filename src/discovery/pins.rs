//! Load pin slots (ADR 0141) onto [`GraphSnapshot::pins`].
//!
//! The slots come from `$XDG_DATA_HOME/conspectus/pins.toml`. Until
//! the operator runs `conspectus pin migrate`, any legacy
//! `[[pins.entries]]` in the user config, the project configs under
//! the scan roots and the observed paths, and the old store registry
//! also load, each as an in-memory mux slot after the real ones, with a
//! `LegacyPins` diagnostic naming the command.
//!
//! This pass is strictly read-only: no file is created or changed.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::ConfigLoader;
use crate::discovery::DiscoveryContext;
use crate::legacy_pins::{entry_to_slot, parse_pins_document};
use crate::model::{Diagnostic, GraphNode, GraphSnapshot};
use crate::pin_slots::SlotStore;

/// What the loader reads from.
pub struct PinSources<'a> {
    pub slots: Option<&'a SlotStore>,
    /// Config files that may hold legacy entries are found through the
    /// loader; `None` skips the legacy pass.
    pub legacy_loader: Option<&'a ConfigLoader>,
    /// Project stores the old registry recorded.
    pub legacy_registry_stores: &'a [PathBuf],
}

pub fn apply_pins(
    snapshot: &mut GraphSnapshot,
    context: &DiscoveryContext,
    sources: &PinSources<'_>,
) {
    snapshot.pins.clear();
    let mut taken: BTreeSet<String> = BTreeSet::new();

    if let Some(store) = sources.slots {
        let load = store.load();
        for problem in load.problems {
            snapshot.diagnostics.push(Diagnostic::Config {
                path: store.path().display().to_string(),
                message: problem,
            });
        }
        for slot in load.slots {
            taken.insert(slot.id.clone());
            snapshot.pins.push(slot);
        }
    }

    if let Some(loader) = sources.legacy_loader {
        let mut legacy_ids = Vec::new();
        let mut diagnostics = Vec::new();
        for LegacyEntry { path, entry } in legacy_entries(
            snapshot,
            context,
            loader,
            sources.legacy_registry_stores,
            &mut diagnostics,
        ) {
            if !taken.insert(entry.id.clone()) {
                diagnostics.push(Diagnostic::Config {
                    path: path.display().to_string(),
                    message: format!(
                        "pin `{}` is already a pin (this legacy entry skipped)",
                        entry.id
                    ),
                });
                continue;
            }
            let order = u32::try_from(snapshot.pins.len()).unwrap_or(u32::MAX);
            legacy_ids.push(entry.id.clone());
            snapshot.pins.push(entry_to_slot(&entry, order));
        }
        snapshot.diagnostics.extend(diagnostics);
        if !legacy_ids.is_empty() {
            snapshot.diagnostics.push(Diagnostic::LegacyPins {
                count: u32::try_from(legacy_ids.len()).unwrap_or(u32::MAX),
                ids: legacy_ids,
            });
        }
    }
    snapshot.canonicalize();
}

/// A legacy entry and the config file it lives in.
#[derive(Clone, Debug)]
pub struct LegacyEntry {
    pub path: PathBuf,
    pub entry: crate::legacy_pins::PinEntry,
}

/// Every legacy `[[pins.entries]]` entry the config files reach, project
/// stores before the user store, an entry shadowed by an earlier one
/// with the same id left out. Read problems go to `diagnostics`.
pub fn legacy_entries(
    snapshot: &GraphSnapshot,
    context: &DiscoveryContext,
    loader: &ConfigLoader,
    registry_stores: &[PathBuf],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<LegacyEntry> {
    let mut seen_ids = BTreeSet::new();
    let mut entries = Vec::new();
    for path in legacy_store_paths(snapshot, context, loader, registry_stores) {
        let Some(document) = read_pins_document(&path, diagnostics) else {
            continue;
        };
        for entry in document.entries() {
            if seen_ids.insert(entry.id.clone()) {
                entries.push(LegacyEntry {
                    path: path.clone(),
                    entry: entry.clone(),
                });
            } else {
                diagnostics.push(Diagnostic::Config {
                    path: path.display().to_string(),
                    message: format!(
                        "pin `{}` is shadowed by an earlier legacy pin (this entry skipped)",
                        entry.id
                    ),
                });
            }
        }
    }
    entries
}

/// The user config, then the project configs, that may hold legacy
/// entries, in the order the old loader read them (project before user,
/// so a project entry shadows a user one with the same id).
fn legacy_store_paths(
    snapshot: &GraphSnapshot,
    context: &DiscoveryContext,
    loader: &ConfigLoader,
    registry_stores: &[PathBuf],
) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut paths = Vec::new();
    for root in project_config_search_roots(snapshot, context) {
        if let Some(path) = loader.locate_project_config(&root)
            && seen.insert(path.clone())
        {
            paths.push(path);
        }
    }
    for path in registry_stores {
        if path.is_file() && seen.insert(path.clone()) {
            paths.push(path.clone());
        }
    }
    if let Some(path) = loader.user_config_path()
        && path.is_file()
        && seen.insert(path.clone())
    {
        paths.push(path);
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
    for node in snapshot.live_nodes() {
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
            GraphNode::Pin(_)
            | GraphNode::Branch(_)
            | GraphNode::ForgePr(_)
            | GraphNode::Fork(_) => {}
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

fn read_pins_document(
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<crate::legacy_pins::PinsDocument> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            diagnostics.push(Diagnostic::Config {
                path: path.display().to_string(),
                message: format!("failed to read pins: {err}"),
            });
            return None;
        }
    };
    match parse_pins_document(&text) {
        Ok(document) => Some(document),
        Err(err) => {
            diagnostics.push(Diagnostic::Config {
                path: path.display().to_string(),
                message: format!("failed to parse pins: {err}"),
            });
            None
        }
    }
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod tests;
