//! Discovery adapter boundaries.
//!
//! This module will hold providers for git, agent harnesses, tmux, forge, and
//! workspace metadata.

use crate::model::GraphSnapshot;

pub fn empty_graph() -> GraphSnapshot {
    GraphSnapshot::empty()
}
