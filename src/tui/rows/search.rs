//! The Search view's row tree (ADR 0147).
//!
//! Lists the sessions that match the active content search, ranked.

use std::path::Path;

use crate::model::GraphSnapshot;
use crate::tui::SearchGrouping;

use super::{RowTree, ViewLabel};

pub struct SearchBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub grouping: SearchGrouping,
}

pub fn build_search_tree(_inputs: SearchBuildInputs<'_>) -> RowTree {
    RowTree {
        rows: Vec::new(),
        view: ViewLabel::Search,
    }
}
