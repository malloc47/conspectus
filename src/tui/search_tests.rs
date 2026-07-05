// Extracted from search.rs H-HYG-011 rolling wave via #[path = "search_tests.rs"] mod tests;
use super::*;
use crate::model::{AgentSessionId, NodeId};
use crate::tui::rows::{AgentSessionRow, MuxIndicator, RowKind};

fn agent_row(harness: &str, key: &str, alias: Option<&str>, preview: &str) -> Row {
    let session_id = AgentSessionId::new(harness, "/state", key);
    let primary = NodeId::AgentSession(session_id.clone());
    Row {
        id: RowId::AgentSession(primary.clone()),
        depth: 1,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: session_id,
            short_id: key.to_string(),
            harness_label: harness.to_string(),
            cwd_display: Some("~/proj".to_string()),
            project_display: None,
            recency: None,
            activity_epoch: None,
            mux_state: MuxIndicator::Unmuxed,
            preview: Some(preview.to_string()),
            title: None,
            alias: alias.map(str::to_string),
            title_disambiguates: false,
            primary_node: primary,
            pin_id: None,
        }),
    }
}

#[test]
fn substring_backend_returns_empty_for_blank_query() {
    let rows = vec![agent_row("claude-code", "abc", Some("puffin"), "hello")];
    let items = items_from_rows(&rows);
    let backend = SubstringBackend;
    assert!(backend.rank("", &items).is_empty());
    assert!(backend.rank("   ", &items).is_empty());
}

#[test]
fn substring_backend_matches_case_insensitively() {
    let rows = vec![
        agent_row("claude-code", "abc", Some("Puffin"), "hello"),
        agent_row("codex", "xyz", None, "nope"),
    ];
    let items = items_from_rows(&rows);
    let backend = SubstringBackend;
    let matches = backend.rank("PUFFIN", &items);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, rows[0].id);
}

#[test]
fn substring_backend_ranks_earlier_matches_higher() {
    // Two rows; one has the needle at the start, one near the
    // end. The earlier match outranks the later one.
    let rows = vec![
        agent_row("claude-code", "early", Some("puffin"), "x"),
        agent_row("codex", "late", None, "nothing here except puffin"),
    ];
    let items = items_from_rows(&rows);
    let backend = SubstringBackend;
    let matches = backend.rank("puffin", &items);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].id, rows[0].id);
    assert_eq!(matches[1].id, rows[1].id);
}

#[test]
fn substring_backend_marks_matched_range_inside_haystack() {
    let rows = vec![agent_row("claude-code", "a", Some("puffin"), "x")];
    let items = items_from_rows(&rows);
    let backend = SubstringBackend;
    let matches = backend.rank("ffi", &items);
    assert_eq!(matches.len(), 1);
    let range = matches[0].matched_range.clone().unwrap();
    assert_eq!(&items[0].haystack[range], "ffi");
}

#[test]
fn substring_backend_finds_match_in_preview_or_cwd() {
    let rows = vec![agent_row("claude-code", "a", None, "look at puffin here")];
    let items = items_from_rows(&rows);
    let backend = SubstringBackend;
    let matches = backend.rank("look at", &items);
    assert_eq!(matches.len(), 1);
}

#[test]
fn items_from_rows_uses_alias_then_title_then_short_id_for_label() {
    let rows = vec![agent_row("claude-code", "abc123", Some("nice-alias"), "x")];
    let items = items_from_rows(&rows);
    assert_eq!(items[0].label.as_ref(), "nice-alias");

    // Without an alias the label falls back to harness:short_id.
    let no_alias = vec![agent_row("claude-code", "abc123", None, "x")];
    let items = items_from_rows(&no_alias);
    assert_eq!(items[0].label.as_ref(), "claude-code:abc123");
}

#[test]
fn snippet_around_returns_haystack_unchanged_when_short() {
    let snippet = snippet_around("look at puffin here", 8..14, 80);
    assert_eq!(snippet.text, "look at puffin here");
    assert_eq!(snippet.highlight, Some(8..14));
}

#[test]
fn snippet_around_windows_long_haystack_with_ellipses() {
    let haystack: String = (0..200).map(|_| "x").collect::<String>()
        + "puffin"
        + &(0..200).map(|_| "y").collect::<String>();
    let needle_byte_start = 200;
    let needle_byte_end = needle_byte_start + "puffin".len();
    let snippet = snippet_around(&haystack, needle_byte_start..needle_byte_end, 30);
    assert!(snippet.text.starts_with('…'));
    assert!(snippet.text.ends_with('…'));
    assert!(snippet.text.contains("puffin"));
    // Highlight should locate the puffin substring inside the
    // rendered snippet.
    let range = snippet.highlight.expect("highlight present");
    assert_eq!(&snippet.text[range], "puffin");
    // Width budget honored within tolerance (chars; not bytes
    // because UTF-8 ellipsis is 3 bytes).
    let char_count = snippet.text.chars().count();
    assert!(char_count <= 30, "snippet too wide: {char_count}");
}

#[test]
fn snippet_around_unicode_safe_split() {
    let haystack = "naïve · approach · brûlée";
    let needle_byte_start = haystack.find("approach").unwrap();
    let needle_byte_end = needle_byte_start + "approach".len();
    let snippet = snippet_around(haystack, needle_byte_start..needle_byte_end, 15);
    // Should not panic and should keep the match visible.
    assert!(snippet.text.contains("approach"));
    let range = snippet.highlight.unwrap();
    assert_eq!(&snippet.text[range], "approach");
}

#[test]
fn snippet_around_falls_back_when_range_is_out_of_bounds() {
    let snippet = snippet_around("short text", 100..200, 20);
    // No highlight, but still a readable snippet (head-anchored).
    assert!(snippet.highlight.is_none());
    assert!(!snippet.text.is_empty());
}
