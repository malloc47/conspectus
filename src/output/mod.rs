//! Output rendering boundaries.

use anyhow::Result;

use crate::model::GraphSnapshot;

pub mod agent;
pub mod dot;
pub mod forks;
pub mod html;
pub mod mux;
pub mod node_show;
pub mod prs;
pub mod render;
pub mod table;
pub mod union;

pub fn render_graph_json(snapshot: &GraphSnapshot) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(snapshot)
}

pub use dot::{DotOptions, Inclusion, render_graph_dot};
pub use html::{HtmlOptions, render_graph_html};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_renders_top_level_arrays() {
        let rendered = render_graph_json(&GraphSnapshot::empty()).expect("render graph json");

        assert_eq!(
            rendered,
            "{\n  \"nodes\": [],\n  \"candidate_links\": [],\n  \"resolved_relationships\": [],\n  \"diagnostics\": []\n}"
        );
    }
}
