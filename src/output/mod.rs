//! Output rendering boundaries.

use anyhow::Result;

use crate::model::GraphSnapshot;

#[cfg(feature = "query")]
pub mod agent;
pub mod node_show;
pub mod render;
pub mod table;

pub fn render_graph_json(snapshot: &GraphSnapshot) -> Result<String> {
    Ok(serde_json::to_string_pretty(snapshot)?)
}

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
