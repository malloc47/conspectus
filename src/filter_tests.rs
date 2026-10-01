use super::*;

fn inputs<'a>(
    harness: &'a str,
    mux: MuxStateKey,
    last_active: Option<i64>,
) -> SessionMatchInputs<'a> {
    SessionMatchInputs {
        harness_key: harness,
        now_epoch: Some(1_000_000),
        last_active_epoch: last_active,
        mux_state: mux,
    }
}

#[test]
fn empty_filter_admits_everything() {
    let filter = RowFilter::default();
    assert!(filter.is_empty());
    assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Attached, Some(1))));
}

#[test]
fn harness_filter_set_membership() {
    let filter = RowFilter {
        harness: Some(HarnessFilter::from_values(["claude-code", "codex"])),
        ..RowFilter::default()
    };
    assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Attached, Some(1))));
    assert!(filter.matches_session(&inputs("codex", MuxStateKey::Attached, Some(1))));
    assert!(!filter.matches_session(&inputs("opencode", MuxStateKey::Attached, Some(1))));
}

#[test]
fn harness_filter_is_case_insensitive_and_trimmed() {
    let filter = RowFilter {
        harness: Some(HarnessFilter::from_values(["  Claude-Code  "])),
        ..RowFilter::default()
    };
    assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Attached, Some(1))));
    assert!(filter.matches_session(&inputs("CLAUDE-CODE", MuxStateKey::Attached, Some(1))));
}

#[test]
fn harness_filter_canonicalizes_values() {
    let filter = HarnessFilter::from_values(["codex", "Claude-Code", "codex", "  "]);
    match filter {
        HarnessFilter::Any(v) => assert_eq!(v, vec!["claude-code", "codex"]),
    }
}

#[test]
fn max_age_filter_at_boundary() {
    // now = 1_000_000, last = 1_000_000 - 7 days = 395200
    let week = Duration::from_secs(7 * 24 * 60 * 60);
    let filter = RowFilter {
        max_age: Some(week),
        ..RowFilter::default()
    };
    // Exactly at the boundary admits the row.
    let just_in = SessionMatchInputs {
        harness_key: "claude-code",
        now_epoch: Some(1_000_000),
        last_active_epoch: Some(1_000_000 - week.as_secs() as i64),
        mux_state: MuxStateKey::Attached,
    };
    assert!(filter.matches_session(&just_in));
    // One second older fails.
    let just_out = SessionMatchInputs {
        last_active_epoch: Some(1_000_000 - week.as_secs() as i64 - 1),
        ..just_in
    };
    assert!(!filter.matches_session(&just_out));
}

#[test]
fn max_age_admits_when_timestamps_missing() {
    let filter = RowFilter {
        max_age: Some(Duration::from_secs(60)),
        ..RowFilter::default()
    };
    let no_last = SessionMatchInputs {
        harness_key: "claude-code",
        now_epoch: Some(1_000_000),
        last_active_epoch: None,
        mux_state: MuxStateKey::Attached,
    };
    assert!(filter.matches_session(&no_last));
    let no_now = SessionMatchInputs {
        now_epoch: None,
        last_active_epoch: Some(0),
        ..no_last
    };
    assert!(filter.matches_session(&no_now));
}

#[test]
fn max_age_admits_on_clock_skew() {
    let filter = RowFilter {
        max_age: Some(Duration::from_secs(60)),
        ..RowFilter::default()
    };
    let future = SessionMatchInputs {
        harness_key: "claude-code",
        now_epoch: Some(1_000),
        last_active_epoch: Some(2_000),
        mux_state: MuxStateKey::Attached,
    };
    assert!(filter.matches_session(&future));
}

#[test]
fn mux_state_filter_set_membership() {
    let filter = RowFilter {
        mux_state: Some(MuxStateFilter::from_values([
            MuxStateKey::Unmuxed,
            MuxStateKey::Ambiguous,
        ])),
        ..RowFilter::default()
    };
    assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Unmuxed, Some(1))));
    assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Ambiguous, Some(1))));
    assert!(!filter.matches_session(&inputs("claude-code", MuxStateKey::Attached, Some(1))));
}

#[test]
fn mux_state_key_round_trips() {
    for key in [
        MuxStateKey::Attached,
        MuxStateKey::Ambiguous,
        MuxStateKey::Unmuxed,
    ] {
        assert_eq!(MuxStateKey::from_str_ci(key.as_str()), Some(key));
    }
    assert_eq!(
        MuxStateKey::from_str_ci("ATTACHED"),
        Some(MuxStateKey::Attached)
    );
    assert_eq!(
        MuxStateKey::from_str_ci(" unmuxed "),
        Some(MuxStateKey::Unmuxed)
    );
    assert_eq!(MuxStateKey::from_str_ci("nope"), None);
}

#[test]
fn mux_state_from_candidate_count() {
    assert_eq!(MuxStateKey::from_candidate_count(0), MuxStateKey::Unmuxed);
    assert_eq!(MuxStateKey::from_candidate_count(1), MuxStateKey::Attached);
    assert_eq!(MuxStateKey::from_candidate_count(2), MuxStateKey::Ambiguous);
    assert_eq!(MuxStateKey::from_candidate_count(7), MuxStateKey::Ambiguous);
}

#[test]
fn intersection_of_all_dimensions() {
    let filter = RowFilter {
        harness: Some(HarnessFilter::from_values(["claude-code"])),
        max_age: Some(Duration::from_secs(60)),
        mux_state: Some(MuxStateFilter::from_values([MuxStateKey::Unmuxed])),
        ..RowFilter::default()
    };
    let good = SessionMatchInputs {
        harness_key: "claude-code",
        now_epoch: Some(1_000),
        last_active_epoch: Some(970),
        mux_state: MuxStateKey::Unmuxed,
    };
    assert!(filter.matches_session(&good));
    // Wrong harness.
    assert!(!filter.matches_session(&SessionMatchInputs {
        harness_key: "codex",
        ..good
    }));
    // Too old.
    assert!(!filter.matches_session(&SessionMatchInputs {
        last_active_epoch: Some(900),
        ..good
    }));
    // Wrong mux state.
    assert!(!filter.matches_session(&SessionMatchInputs {
        mux_state: MuxStateKey::Attached,
        ..good
    }));
}
