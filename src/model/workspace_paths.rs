//! Which workspace a path sits in.
//!
//! Discovery associates agent sessions and roots muxes in workspaces
//! by path, and the TUI places pinned rows the same way, so the
//! index lives here where both can share it.

use std::collections::{BTreeSet, HashMap};

use super::{GraphSnapshot, NodeId, RelationKind, source_field};

/// Workspaces indexed by every path inside their visible tree: the
/// workspace `root` itself (so a session or mux at the multi-repo
/// composite directory attributes correctly, the agent-deck launch
/// shape) and every member's `logical_path` (so one inside a specific
/// member subdir attributes via the deeper match, the Atelier
/// composite-tree shape).
///
/// **Crucially excludes `canonical_checkout_root`** even though that
/// field is also present on the membership link: when a workspace
/// member is a symlink pointing outside the workspace tree, the
/// canonical path is the *target* checkout's location, and work there
/// is *not* workspace-context work; it just touches a repo that is
/// also a workspace member. Indexing the canonical root would collapse
/// the (A) workspace-rooted and (B) repo-shared classes that
/// `docs/plans/workspace-view-redesign.md` keeps separate.
///
/// Paths are normalized (no trailing `/`) and matched component-wise.
#[derive(Clone, Debug, Default)]
pub struct WorkspacePaths {
    roots: Vec<(NodeId, String)>,
}

impl WorkspacePaths {
    pub fn from_snapshot(snapshot: &GraphSnapshot) -> Self {
        let mut roots = BTreeSet::new();

        for link in &snapshot.candidate_links {
            if link.relation != RelationKind::WorkspaceContainsRepo {
                continue;
            }

            if let NodeId::Workspace(ws) = &link.source {
                roots.insert((link.source.clone(), normalize_path(&ws.root)));
            }

            if let Some(path) = link
                .source_metadata
                .fields
                .get(source_field::LOGICAL_PATH)
                .and_then(serde_json::Value::as_str)
            {
                roots.insert((link.source.clone(), normalize_path(path)));
            }
        }

        Self {
            roots: roots.into_iter().collect(),
        }
    }

    /// Every workspace whose visible tree contains `path`, each with
    /// the deepest of its roots that matched, so a path in a member
    /// subdir reports the member rather than the workspace root.
    pub fn matching(&self, path: &str) -> Vec<(&NodeId, &str)> {
        let path = normalize_path(path);
        let mut deepest_by_workspace: HashMap<&NodeId, &str> = HashMap::new();

        for (workspace, root) in &self.roots {
            if !path_at_or_under(&path, root) {
                continue;
            }

            let current = deepest_by_workspace
                .entry(workspace)
                .or_insert(root.as_str());
            if path_depth(root) > path_depth(current) {
                *current = root;
            }
        }

        deepest_by_workspace.into_iter().collect()
    }

    /// The single workspace `path` sits most specifically inside: the
    /// one whose matching root is deepest, so with nested workspaces
    /// the inner one wins. Ties go to the lower workspace id so the
    /// answer doesn't depend on map order.
    pub fn innermost(&self, path: &str) -> Option<(&NodeId, &str)> {
        self.matching(path)
            .into_iter()
            .max_by(|(left_ws, left_root), (right_ws, right_root)| {
                path_depth(left_root)
                    .cmp(&path_depth(right_root))
                    .then_with(|| right_ws.cmp(left_ws))
            })
    }
}

/// Strip trailing `/`s, keeping the filesystem root as `/`.
pub fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

/// `child` is `parent` or inside it. Component-wise, so `/a/barbecue`
/// is not under `/a/bar`. Both paths should already be normalized.
pub fn path_at_or_under(child: &str, parent: &str) -> bool {
    if child == parent {
        return true;
    }
    if parent == "/" {
        return child.starts_with('/');
    }
    child.starts_with(&format!("{parent}/"))
}

/// Number of non-empty path components.
pub fn path_depth(path: &str) -> usize {
    path.split('/').filter(|part| !part.is_empty()).count()
}

#[cfg(test)]
#[path = "workspace_paths_tests.rs"]
mod tests;
