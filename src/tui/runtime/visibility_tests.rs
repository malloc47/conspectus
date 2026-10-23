use super::*;

#[test]
fn a_pane_is_visible_only_when_attached_and_active() {
    assert_eq!(parse_visibility("1:1:1\n"), Some(true));
    assert_eq!(parse_visibility("2:1:1"), Some(true));
    assert_eq!(
        parse_visibility("0:1:1"),
        Some(false),
        "no client on the session"
    );
    assert_eq!(
        parse_visibility("1:0:1"),
        Some(false),
        "another window is showing"
    );
    assert_eq!(
        parse_visibility("1:1:0"),
        Some(false),
        "another pane is active"
    );
}

#[test]
fn unreadable_output_says_nothing() {
    assert_eq!(parse_visibility(""), None);
    assert_eq!(parse_visibility("garbage"), None);
    assert_eq!(parse_visibility("1:1"), None);
}

#[test]
fn only_a_hidden_to_visible_change_fires() {
    assert_eq!(step(true, Some(true)), (true, false), "staying in view");
    assert_eq!(step(true, Some(false)), (false, false), "leaving view");
    assert_eq!(step(false, Some(false)), (false, false), "still away");
    assert_eq!(step(false, Some(true)), (true, true), "back in view");
}

#[test]
fn a_failed_query_neither_fires_nor_changes_state() {
    assert_eq!(step(false, None), (false, false));
    assert_eq!(step(true, None), (true, false));
}
