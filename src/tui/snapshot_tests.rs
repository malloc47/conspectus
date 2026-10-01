use super::*;

/// Default reflow threshold used by the `pane_rect` layout tests.
const DEFAULT: u16 = crate::config::DEFAULT_NARROW_LAYOUT_THRESHOLD;

#[test]
fn parse_key_script_handles_literals_and_named_keys() {
    let keys = parse_key_script("vj<Enter>").expect("parse");
    assert_eq!(keys.len(), 3);
    assert_eq!(keys[0].code, KeyCode::Char('v'));
    assert_eq!(keys[1].code, KeyCode::Char('j'));
    assert_eq!(keys[2].code, KeyCode::Enter);
}

#[test]
fn parse_key_script_uppercase_carries_shift_modifier() {
    let keys = parse_key_script("G").expect("parse");
    assert!(keys[0].modifiers.contains(KeyModifiers::SHIFT));
}

#[test]
fn parse_key_script_supports_modifier_prefixes() {
    let keys = parse_key_script("<C-r>").expect("parse");
    assert_eq!(keys[0].code, KeyCode::Char('r'));
    assert!(keys[0].modifiers.contains(KeyModifiers::CONTROL));

    let keys = parse_key_script("<C-A-Delete>").expect("parse");
    assert_eq!(keys[0].code, KeyCode::Delete);
    assert!(keys[0].modifiers.contains(KeyModifiers::CONTROL));
    assert!(keys[0].modifiers.contains(KeyModifiers::ALT));
}

#[test]
fn parse_key_script_unterminated_bracket_errors() {
    let err = parse_key_script("<Enter").unwrap_err();
    assert!(err.to_string().contains("unterminated"));
}

#[test]
fn parse_key_script_unknown_name_errors() {
    let err = parse_key_script("<Bogus>").unwrap_err();
    assert!(err.to_string().contains("unknown key name"));
}

#[test]
fn buffer_to_ansi_emits_styled_text_with_reset_per_line() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 2));
    buffer.set_string(0, 0, "abc", Style::default().fg(Color::Red));
    buffer.set_string(0, 1, "xyz", Style::default());

    let out = buffer_to_ansi(&buffer, Rect::new(0, 0, 3, 2));
    assert!(out.contains("abc"));
    assert!(out.contains("xyz"));
    // Red fg = 31m
    assert!(out.contains("\x1b[31m"));
    // Reset at end of line
    assert!(out.matches("\x1b[0m").count() >= 2);
}

#[test]
fn pane_rect_returns_full_area_for_all() {
    let area = Rect::new(0, 0, 160, 40);
    assert_eq!(pane_rect(area, SnapshotPane::All, DEFAULT), area);
}

#[test]
fn pane_rect_carves_header_and_status_as_single_rows() {
    let area = Rect::new(0, 0, 160, 40);
    let header = pane_rect(area, SnapshotPane::Header, DEFAULT);
    assert_eq!(header.height, 1);
    assert_eq!(header.y, 0);
    let status = pane_rect(area, SnapshotPane::Status, DEFAULT);
    assert_eq!(status.height, 1);
    assert_eq!(status.y, 39);
}

#[test]
fn pane_rect_splits_body_horizontally_when_wide() {
    let area = Rect::new(0, 0, 160, 40);
    let left = pane_rect(area, SnapshotPane::Left, DEFAULT);
    let right = pane_rect(area, SnapshotPane::Right, DEFAULT);
    assert_eq!(left.x, 0);
    assert!(left.width > 0);
    assert!(right.x >= left.width);
    assert_eq!(left.y, 1);
    assert_eq!(left.height, 38);
}

#[test]
fn fixture_round_trip_preserves_empty_snapshot() {
    // Empty graph serializes to a minimal JSON and deserializes
    // back to the same value; covers the simplest fixture shape
    // an agent might hand-craft.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("empty.json");
    let snapshot = GraphSnapshot::empty();
    write_fixture(&path, &snapshot).expect("write");
    let loaded = load_fixture(&path).expect("load");
    assert_eq!(loaded, snapshot);
}

#[test]
fn fixture_load_error_reports_path() {
    let err =
        load_fixture(std::path::Path::new("/no/such/file.json")).expect_err("missing file errors");
    assert!(
        err.to_string().contains("/no/such/file.json"),
        "error should mention the failing path: {err}",
    );
}

#[test]
fn pane_rect_splits_body_vertically_when_narrow() {
    // Below the default narrow_layout_threshold (100): body splits
    // top/bottom.
    let area = Rect::new(0, 0, 60, 40);
    let left = pane_rect(area, SnapshotPane::Left, DEFAULT);
    let right = pane_rect(area, SnapshotPane::Right, DEFAULT);
    assert_eq!(left.x, 0);
    assert_eq!(right.x, 0);
    assert_eq!(left.width, 60);
    assert!(right.y > left.y);
}

#[test]
fn pane_rect_split_direction_follows_configured_threshold() {
    // The reflow breakpoint is configurable. A 60-col
    // body stacks vertically at the default threshold (100) but stays
    // side-by-side once the threshold is lowered below the width.
    let area = Rect::new(0, 0, 60, 40);

    let stacked = pane_rect(area, SnapshotPane::Right, DEFAULT);
    assert!(stacked.y > 1, "default threshold stacks a 60-col body");

    let side_by_side = pane_rect(area, SnapshotPane::Right, 50);
    assert_eq!(side_by_side.y, 1, "lowered threshold keeps side-by-side");
    assert!(side_by_side.x > 0, "right pane sits beside the left pane");
}
