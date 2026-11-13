use super::*;

fn inputs<'a>(
    harness: &'a str,
    mux: MuxStateKey,
    last_active: Option<i64>,
) -> SessionMatchInputs<'a> {
    SessionMatchInputs {
        harness_key: harness,
        session_key: "session",
        now_epoch: Some(1_000_000),
        last_active_epoch: last_active,
        mux_state: mux,
        live_state: LiveStateKey::None,
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
        session_key: "session",
        now_epoch: Some(1_000_000),
        last_active_epoch: Some(1_000_000 - week.as_secs() as i64),
        mux_state: MuxStateKey::Attached,

        live_state: LiveStateKey::None,
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
        session_key: "session",
        now_epoch: Some(1_000_000),
        last_active_epoch: None,
        mux_state: MuxStateKey::Attached,

        live_state: LiveStateKey::None,
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
        session_key: "session",
        now_epoch: Some(1_000),
        last_active_epoch: Some(2_000),
        mux_state: MuxStateKey::Attached,

        live_state: LiveStateKey::None,
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
        session_key: "session",
        now_epoch: Some(1_000),
        last_active_epoch: Some(970),
        mux_state: MuxStateKey::Unmuxed,

        live_state: LiveStateKey::None,
    };
    assert!(filter.matches_session(&good));
    // Wrong harness.
    assert!(!filter.matches_session(&SessionMatchInputs {
        harness_key: "codex",
        session_key: "session",
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

#[test]
fn pr_branch_filter_admits_by_tracking() {
    assert!(PrBranchFilter::Tracked.admits(true));
    assert!(!PrBranchFilter::Tracked.admits(false));
    assert!(PrBranchFilter::Untracked.admits(false));
    assert!(!PrBranchFilter::Untracked.admits(true));
}

#[test]
fn pr_branch_filter_cycles_through_any_tracked_untracked() {
    let mut state = None;
    let mut seen = Vec::new();
    for _ in 0..4 {
        state = PrBranchFilter::cycle(state);
        seen.push(state);
    }
    assert_eq!(
        seen,
        [
            Some(PrBranchFilter::Tracked),
            Some(PrBranchFilter::Untracked),
            None,
            Some(PrBranchFilter::Tracked)
        ]
    );
}

#[test]
fn pr_branch_filter_round_trips_its_text_form() {
    for value in [PrBranchFilter::Tracked, PrBranchFilter::Untracked] {
        assert_eq!(PrBranchFilter::from_str_ci(value.as_str()), Some(value));
    }
    assert_eq!(
        PrBranchFilter::from_str_ci(" Tracked "),
        Some(PrBranchFilter::Tracked)
    );
    assert_eq!(PrBranchFilter::from_str_ci("both"), None);
}

#[test]
fn pr_settings_clear_with_the_filter_but_do_not_narrow_by_session() {
    // They are cleared by "Clear all filters" (not empty) yet must not
    // switch on the session-based narrowing of PR rows.
    let branch = RowFilter {
        pr_branch: Some(PrBranchFilter::Untracked),
        ..RowFilter::default()
    };
    let float = RowFilter {
        float_tracked_prs_top: true,
        ..RowFilter::default()
    };
    for filter in [branch, float] {
        assert!(!filter.is_empty());
        assert!(!filter.has_narrowing_predicates());
    }
}

#[test]
fn pr_settings_survive_a_serde_round_trip_and_default_to_off() {
    let filter = RowFilter {
        pr_branch: Some(PrBranchFilter::Tracked),
        float_tracked_prs_top: true,
        ..RowFilter::default()
    };
    let json = serde_json::to_string(&filter).expect("serialize");
    assert!(json.contains("\"pr_branch\":\"tracked\""), "{json}");
    assert_eq!(
        serde_json::from_str::<RowFilter>(&json).expect("read"),
        filter
    );

    // State written before the settings existed reads back as off.
    let old: RowFilter = serde_json::from_str("{}").expect("old state");
    assert_eq!(old, RowFilter::default());
    assert_eq!(
        serde_json::to_string(&RowFilter::default()).expect("serialize"),
        "{}"
    );
}

#[test]
fn pr_state_keys_match_forge_state_and_draft_flag_in_any_case() {
    assert!(PrStateKey::Open.matches(Some("open"), false));
    assert!(
        PrStateKey::Open.matches(Some("OPEN"), true),
        "open includes drafts"
    );
    assert!(!PrStateKey::Open.matches(Some("merged"), false));
    assert!(PrStateKey::Draft.matches(Some("open"), true));
    assert!(!PrStateKey::Draft.matches(Some("open"), false));
    assert!(
        !PrStateKey::Draft.matches(Some("merged"), true),
        "a merged draft is merged"
    );
    assert!(PrStateKey::Merged.matches(Some("Merged"), false));
    assert!(PrStateKey::Closed.matches(Some("closed"), false));
    for key in PrStateKey::ALL {
        assert!(!key.matches(None, false), "{key:?}");
        assert_eq!(PrStateKey::from_str_ci(key.as_str()), Some(key));
    }
    assert_eq!(
        PrStateKey::from_str_ci(" Merged "),
        Some(PrStateKey::Merged)
    );
    assert_eq!(PrStateKey::from_str_ci("other"), None);
}

#[test]
fn pr_state_filter_admits_any_listed_state() {
    let filter =
        PrStateFilter::from_values([PrStateKey::Merged, PrStateKey::Draft, PrStateKey::Merged]);
    assert_eq!(filter.values(), [PrStateKey::Draft, PrStateKey::Merged]);
    assert!(filter.admits(Some("merged"), false));
    assert!(filter.admits(Some("open"), true));
    assert!(!filter.admits(Some("open"), false));
    assert!(!filter.admits(Some("closed"), false));
}

#[test]
fn pr_state_clears_with_the_filter_but_does_not_narrow_by_session() {
    let filter = RowFilter {
        pr_state: Some(PrStateFilter::from_values([PrStateKey::Open])),
        ..RowFilter::default()
    };
    assert!(!filter.is_empty());
    assert!(!filter.has_narrowing_predicates());
    let json = serde_json::to_string(&filter).expect("serialize");
    assert_eq!(
        serde_json::from_str::<RowFilter>(&json).expect("read"),
        filter
    );
}

fn content_filter(matched: &[(&str, &str)]) -> ContentFilter {
    use crate::viewer::model::{TurnKind, TurnRole};
    use crate::viewer::search::{Hit, SessionHits};
    let mut hits = ContentHitMap::default();
    for (harness, session) in matched {
        hits.insert(
            (*harness).to_string(),
            (*session).to_string(),
            SessionHits {
                count: 1,
                best: Hit {
                    turn_index: 0,
                    role: TurnRole::User,
                    kind: TurnKind::Message,
                    timestamp: None,
                    snippet: "x".into(),
                    highlight: 0..1,
                },
                latest: None,
            },
        );
    }
    ContentFilter {
        text: "x".into(),
        hits: Arc::new(hits),
        complete: true,
        group: crate::viewer::search::Grouping::Session,
        details: Arc::default(),
        panes: Arc::default(),
    }
}

#[test]
fn a_content_filter_admits_only_the_sessions_that_matched() {
    let filter = RowFilter {
        content: Some(content_filter(&[("claude-code", "found")])),
        ..RowFilter::default()
    };
    let mut hit = inputs("claude-code", MuxStateKey::Unmuxed, Some(1));
    hit.session_key = "found";
    let mut miss = inputs("claude-code", MuxStateKey::Unmuxed, Some(1));
    miss.session_key = "other";
    assert!(filter.matches_session(&hit));
    assert!(!filter.matches_session(&miss));
}

#[test]
fn a_content_match_is_per_harness_not_just_per_session_key() {
    let filter = RowFilter {
        content: Some(content_filter(&[("claude-code", "shared-key")])),
        ..RowFilter::default()
    };
    let mut other_harness = inputs("codex", MuxStateKey::Unmuxed, Some(1));
    other_harness.session_key = "shared-key";
    assert!(!filter.matches_session(&other_harness));
}

#[test]
fn a_content_filter_counts_as_a_narrowing_predicate_and_as_something_to_clear() {
    let filter = RowFilter {
        content: Some(content_filter(&[])),
        ..RowFilter::default()
    };
    assert!(filter.has_narrowing_predicates());
    assert!(!filter.is_empty());
}

#[test]
fn a_content_filter_composes_with_the_other_predicates() {
    let filter = RowFilter {
        harness: Some(HarnessFilter::from_values(["codex"])),
        content: Some(content_filter(&[("claude-code", "found")])),
        ..RowFilter::default()
    };
    let mut hit = inputs("claude-code", MuxStateKey::Unmuxed, Some(1));
    hit.session_key = "found";
    assert!(
        !filter.matches_session(&hit),
        "a match in the wrong harness still fails the harness filter"
    );
}

#[test]
fn a_content_filter_is_never_serialized() {
    let filter = RowFilter {
        content: Some(content_filter(&[("claude-code", "found")])),
        ..RowFilter::default()
    };
    let json = serde_json::to_string(&filter).unwrap();
    assert_eq!(json, serde_json::to_string(&RowFilter::default()).unwrap());
    assert!(!json.contains("found"));
    let back: RowFilter = serde_json::from_str(&json).unwrap();
    assert!(back.content.is_none());
}

#[test]
fn every_live_state_reads_as_the_value_of_its_mark() {
    use crate::model::live::LiveState;
    let expected = [
        (LiveState::Waiting, LiveStateKey::Waiting),
        (LiveState::Ready, LiveStateKey::Ready),
        (LiveState::Working, LiveStateKey::Working),
        (LiveState::Compacting, LiveStateKey::Working),
        (LiveState::Starting, LiveStateKey::Working),
        (LiveState::Error, LiveStateKey::Error),
        (LiveState::Unknown, LiveStateKey::Unknown),
        (LiveState::Exited, LiveStateKey::None),
    ];
    assert_eq!(
        expected.len(),
        LiveState::ALL.len(),
        "every state is listed"
    );
    for (state, key) in expected {
        assert_eq!(LiveStateKey::of_state(state), key, "{state:?}");
        assert_eq!(LiveStateKey::of(Some(state)), key, "{state:?}");
    }
    assert_eq!(LiveStateKey::of(None), LiveStateKey::None, "no evidence");
}

#[test]
fn live_state_keys_round_trip_their_text_form() {
    for key in LiveStateKey::ALL {
        assert_eq!(LiveStateKey::from_str_ci(key.as_str()), Some(key));
    }
    assert_eq!(
        LiveStateKey::from_str_ci(" WAITING "),
        Some(LiveStateKey::Waiting)
    );
    assert_eq!(LiveStateKey::from_str_ci("unseen"), None);
    assert_eq!(
        LiveStateKey::spellings(),
        "waiting, ready, working, error, unknown, none"
    );
}

#[test]
fn a_live_state_filter_admits_the_sessions_in_its_states() {
    let filter = RowFilter {
        live_state: Some(LiveStateFilter::from_values([
            LiveStateKey::Error,
            LiveStateKey::Waiting,
            LiveStateKey::Error,
        ])),
        ..RowFilter::default()
    };
    assert_eq!(
        filter.live_state.as_ref().map(LiveStateFilter::values),
        Some([LiveStateKey::Waiting, LiveStateKey::Error].as_slice())
    );
    let with = |live_state| SessionMatchInputs {
        live_state,
        ..inputs("claude-code", MuxStateKey::Unmuxed, Some(1))
    };
    assert!(filter.matches_session(&with(LiveStateKey::Waiting)));
    assert!(filter.matches_session(&with(LiveStateKey::Error)));
    for other in [
        LiveStateKey::Ready,
        LiveStateKey::Working,
        LiveStateKey::Unknown,
        LiveStateKey::None,
    ] {
        assert!(!filter.matches_session(&with(other)), "{other:?}");
    }
    assert!(filter.has_narrowing_predicates());
    assert!(!filter.has_session_only_predicates());
    assert!(!filter.is_empty());
}

#[test]
fn the_agents_a_mux_counts_keep_it_only_under_a_lone_live_state_filter() {
    let states = |keys: &[LiveStateKey]| keys.iter().copied().collect::<BTreeSet<_>>();
    let waiting = RowFilter {
        live_state: Some(LiveStateFilter::from_values([LiveStateKey::Waiting])),
        ..RowFilter::default()
    };
    assert!(waiting.admits_mux_agents(&states(&[LiveStateKey::Working, LiveStateKey::Waiting])));
    assert!(!waiting.admits_mux_agents(&states(&[LiveStateKey::Working])));
    assert!(!waiting.admits_mux_agents(&states(&[])));
    let with_harness = RowFilter {
        harness: Some(HarnessFilter::from_values(["codex"])),
        ..waiting
    };
    assert!(
        !with_harness.admits_mux_agents(&states(&[LiveStateKey::Waiting])),
        "the counts can't say which harness waits"
    );
    assert!(!RowFilter::default().admits_mux_agents(&states(&[LiveStateKey::Waiting])));
}

#[test]
fn the_float_and_the_live_state_filter_are_saved_and_the_states_never_are() {
    use crate::model::live::{LiveBinding, LiveObservation, LiveSource, LiveState, Liveness};
    use crate::model::{AgentSessionId, GraphSnapshot, NodeId};

    let session = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "s1"));
    let mut snapshot = GraphSnapshot::default();
    snapshot.node_live.insert(
        session.clone(),
        LiveBinding::new(
            LiveObservation::new(LiveState::Waiting, LiveSource::Hook, 1_000),
            Liveness::Alive,
        ),
    );
    let live = LiveStates::read(&snapshot, snapshot.live_tables(), 2_000);
    assert_eq!(live.session(&session), LiveStateKey::Waiting);
    let filter = RowFilter {
        live_state: Some(LiveStateFilter::from_values([LiveStateKey::Waiting])),
        float_waiting_top: true,
        live,
        ..RowFilter::default()
    };
    assert!(!filter.is_empty());
    let json = serde_json::to_string(&filter).expect("serialize");
    assert_eq!(
        json,
        r#"{"live_state":{"any":["waiting"]},"float_waiting_top":true}"#
    );
    let back: RowFilter = serde_json::from_str(&json).expect("read");
    assert_eq!(back.live, LiveStates::default(), "the states are not saved");
    assert_eq!(
        back,
        RowFilter {
            live: LiveStates::default(),
            ..filter
        }
    );
    let float = RowFilter {
        float_waiting_top: true,
        ..RowFilter::default()
    };
    assert!(!float.is_empty(), "Clear all turns the float off");
    assert!(!float.has_narrowing_predicates(), "a float hides nothing");
}

#[test]
fn live_states_read_sessions_panes_and_the_muxes_an_agent_waits_on() {
    use crate::model::live::{LiveBinding, LiveObservation, LiveSource, LiveState, Liveness};
    use crate::model::{AgentSessionId, GraphSnapshot, LivePane, MuxSessionId, NodeId};

    let mux = MuxSessionId::new("tmux:scratch".to_string());
    let quiet = MuxSessionId::new("tmux:quiet".to_string());
    let binding = |state| {
        LiveBinding::new(
            LiveObservation::new(state, LiveSource::Hook, 1_000),
            Liveness::Alive,
        )
    };
    let mut snapshot = GraphSnapshot::default();
    snapshot.pane_live.insert(
        LivePane::new(mux.clone(), Some("%1".into())),
        binding(LiveState::Waiting),
    );
    snapshot.pane_live.insert(
        LivePane::new(mux.clone(), Some("%2".into())),
        binding(LiveState::Starting),
    );
    snapshot.pane_live.insert(
        LivePane::new(quiet.clone(), Some("%1".into())),
        binding(LiveState::Ready),
    );
    let live = LiveStates::read(&snapshot, snapshot.live_tables(), 2_000);
    assert_eq!(
        live.mux_agents(&mux).iter().copied().collect::<Vec<_>>(),
        [LiveStateKey::Waiting, LiveStateKey::Working]
    );
    assert!(live.mux_waits(&mux));
    assert!(!live.mux_waits(&quiet));
    let unknown = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "none"));
    assert_eq!(live.session(&unknown), LiveStateKey::None, "no evidence");
    assert!(
        live.mux_agents(&MuxSessionId::new("tmux:other".to_string()))
            .is_empty()
    );
}
