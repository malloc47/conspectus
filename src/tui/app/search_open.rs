//! Opening a quick-select result: what Enter does for an entity, and the
//! word that says so before it is pressed.

use super::tree::row_matches;
use super::*;
use crate::model::NodeKind;
use crate::tui::relation_path::{MAX_HOPS, Reachability};

/// What opening an entity does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenPlan {
    /// The current view lists it: select its row in the left pane.
    Here,
    /// It is related to what is focused: follow the relationships in the
    /// right pane. Every hop after the start, ending with the entity.
    Path(Vec<NodeId>),
    /// Somewhere else: switch to the view that lists it.
    View(crate::tui::View),
}

impl OpenPlan {
    /// The outcome word shown at the right edge of a result.
    pub fn word(&self) -> String {
        match self {
            Self::Here => "here".to_string(),
            Self::Path(path) if path.len() < MAX_HOPS => {
                let first = path.first().map_or("related", kind_word);
                format!("via {first}")
            }
            Self::Path(path) => format!("{} hops", path.len()),
            Self::View(view) => {
                format!(
                    "in {} view",
                    crate::tui::widgets::controls::view_label(*view)
                )
            }
        }
    }
}

fn kind_word(node: &NodeId) -> &'static str {
    match NodeKind::from(node) {
        NodeKind::Workspace => "workspace",
        NodeKind::Repo => "repo",
        NodeKind::Checkout => "checkout",
        NodeKind::AgentSession => "session",
        NodeKind::MuxSession => "mux",
        NodeKind::Pin => "pin",
        NodeKind::RuntimeProcess => "process",
        NodeKind::Branch => "branch",
        NodeKind::Fork => "fork",
        NodeKind::ForgePr => "pr",
    }
}

impl App {
    /// Where paths start: the node the right pane is focused on, or the
    /// selected row's node when nothing is drilled.
    fn search_start(&self) -> Option<NodeId> {
        self.explorer
            .as_ref()
            .map(|explorer| explorer.id().clone())
            .or_else(|| {
                self.tree_state
                    .selected()
                    .and_then(|id| crate::tui::rows::row_node(id).cloned())
            })
    }

    /// The nodes within a few hops of the search start, walked once so
    /// each result can be classified without another walk.
    pub(crate) fn search_reachability(&self) -> Option<Reachability> {
        let start = self.search_start()?;
        let handle = self.snapshot_handle()?;
        Some(Reachability::from(handle.snapshot(), &start, MAX_HOPS))
    }

    /// The row in the current view's tree that lists `node`, collapsed
    /// rows included.
    fn row_index_for_node(&self, node: &NodeId) -> Option<usize> {
        let record = self.snapshot_handle().and_then(|handle| {
            handle
                .snapshot()
                .history
                .by_key(&node.to_string())
                .map(|(_, meta)| meta.id.clone())
        });
        self.tree.rows.iter().position(|row| {
            row_matches(row, node)
                || matches!(&row.id, RowId::History { record_id } if Some(record_id) == record.as_ref())
        })
    }

    /// What opening `node` would do now. `reach` comes from
    /// [`Self::search_reachability`], computed once for many results.
    pub(crate) fn open_plan(&self, reach: Option<&Reachability>, node: &NodeId) -> OpenPlan {
        if self.row_index_for_node(node).is_some() {
            return OpenPlan::Here;
        }
        if let Some(path) = reach.and_then(|reach| reach.path_to(node)) {
            return OpenPlan::Path(path.to_vec());
        }
        let view = self
            .snapshot_handle()
            .map_or(crate::tui::View::Sessions, |handle| {
                crate::tui::actions::natural_view(handle.snapshot(), node)
            });
        OpenPlan::View(view)
    }

    /// Open a quick-select result by the plan its outcome word promised.
    pub(crate) fn open_entity(&mut self, node: &NodeId) {
        let reach = self.search_reachability();
        match self.open_plan(reach.as_ref(), node) {
            OpenPlan::Here => {
                if let Some(index) = self.row_index_for_node(node) {
                    self.expand_to_and_select(index);
                }
            }
            OpenPlan::Path(path) => {
                if !self.follow_path(&path) {
                    self.reveal_entity(node);
                }
            }
            OpenPlan::View(_) => self.reveal_entity(node),
        }
    }

    /// Switch to the view that lists `node`, saying so when it isn't.
    fn reveal_entity(&mut self, node: &NodeId) {
        if let Err(why) = self.reveal_node(node, None) {
            let label = self
                .snapshot_handle()
                .and_then(|handle| {
                    let snapshot = handle.snapshot();
                    snapshot
                        .find_node(node)
                        .map(|found| crate::tui::explorer::title_line(snapshot, found))
                })
                .unwrap_or_else(|| node.to_string());
            self.status_message = Some(Feedback::warning(format!("{label} {why}")));
        }
    }

    /// Focus the right pane and drill along `path`, leaving the cursor on
    /// the entity's row in the last hop's relationships. Each drill is a
    /// breadcrumb hop, so Backspace retraces it. Undoes itself and says
    /// `false` when a hop can't be followed.
    fn follow_path(&mut self, path: &[NodeId]) -> bool {
        let Some((target, hops)) = path.split_last() else {
            return false;
        };
        if self.explorer.is_none() {
            return false;
        }
        let was_focus = self.focus;
        let mut made = 0;
        let mut ok = true;
        for hop in hops {
            let Some(outcome) =
                self.with_explorer(|explorer, source| explorer.drill_to(hop.clone(), source))
            else {
                ok = false;
                break;
            };
            self.explorer_outcome(outcome);
            if self.explorer.as_ref().map(GraphExplorer::id) != Some(hop) {
                ok = false;
                break;
            }
            made += 1;
        }
        if ok {
            ok = self.put_cursor_on_neighbor(target);
        }
        if !ok {
            for _ in 0..made {
                if let Some(outcome) = self.with_explorer(|explorer, source| explorer.back(source))
                {
                    self.explorer_outcome(outcome);
                }
            }
            self.focus = was_focus;
            return false;
        }
        self.focus = Focus::Right;
        if let Some(explorer) = self.explorer.as_mut() {
            explorer.disarm();
        }
        self.status_message = None;
        true
    }

    /// Put the explorer cursor on the row relating to `target`, opening
    /// the `Other` zone when the row sits there: a winner can sit among
    /// the alternates.
    fn put_cursor_on_neighbor(&mut self, target: &NodeId) -> bool {
        self.with_explorer(|explorer, source| {
            explorer.seek(source, |view, entry| {
                view.drill_target(&entry.host).as_ref() == Some(target)
            })
        })
        .unwrap_or(false)
    }
}
