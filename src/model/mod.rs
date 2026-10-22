//! Core graph model boundaries.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod rkyv_adapters;
pub mod workspace_paths;
use rkyv_adapters::MetadataAsJson;

mod graph;
mod history;
mod ids;
mod link;
mod node;
mod resolution;
mod slot;

pub use graph::*;
pub use history::*;
pub use ids::*;
pub use link::*;
pub use node::*;
pub use resolution::*;
pub use slot::*;

pub type Metadata = BTreeMap<String, Value>;

/// Component-wise prefix match so `/a/b` does **not** count as an ancestor
/// of `/a/barbecue`.
pub fn path_is_ancestor_of(ancestor: &Path, descendant: &Path) -> bool {
    let mut anc_iter = ancestor.components();
    let mut desc_iter = descendant.components();
    loop {
        match (anc_iter.next(), desc_iter.next()) {
            (Some(a), Some(d)) if a == d => continue,
            (Some(_), Some(_)) => return false,
            (Some(_), None) => return false,
            (None, _) => return true,
        }
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
