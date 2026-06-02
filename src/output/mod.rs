//! Output rendering boundaries.

use anyhow::Result;

use crate::model::GraphSnapshot;

#[cfg(feature = "query")]
pub mod agent;
pub mod dot;
#[cfg(feature = "query")]
pub mod forks;
pub mod html;
#[cfg(feature = "query")]
pub mod mux;
pub mod node_show;
#[cfg(feature = "query")]
pub mod prs;
pub mod render;
pub mod table;
#[cfg(feature = "query")]
pub mod union;

pub fn render_graph_json(snapshot: &GraphSnapshot) -> Result<String> {
    Ok(serde_json::to_string_pretty(snapshot)?)
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
