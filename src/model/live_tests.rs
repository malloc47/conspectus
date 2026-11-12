use super::*;

use LiveConfidence::{Fresh, Quiet};
use LiveDetail::*;
use LiveState::*;

/// An arbitrary instant, in epoch milliseconds, for the latest evidence.
const T0: i64 = 1_800_000_000_000;
const SECOND: i64 = 1000;
const MINUTE: i64 = 60 * SECOND;
const DAY: i64 = 24 * 60 * MINUTE;

const KILLED: EffectiveLive = EffectiveLive {
    state: Exited,
    detail: Some(Killed),
    confidence: Fresh,
};

fn observed(state: LiveState) -> LiveObservation {
    LiveObservation::new(state, LiveSource::Hook, T0)
}

/// `observation` read `silent_ms` after its latest evidence.
fn after(observation: &LiveObservation, silent_ms: i64, liveness: Liveness) -> EffectiveLive {
    observation.effective(T0 + silent_ms, liveness)
}

fn shown(
    state: LiveState,
    detail: Option<LiveDetail>,
    confidence: LiveConfidence,
) -> EffectiveLive {
    EffectiveLive {
        state,
        detail,
        confidence,
    }
}

fn states_except_exited() -> impl Iterator<Item = LiveState> {
    LiveState::ALL.into_iter().filter(|state| *state != Exited)
}

// --- Vocabulary -----------------------------------------------------

#[test]
fn states_serialize_by_stable_lowercase_names() {
    let names: Vec<&str> = LiveState::ALL.iter().map(|state| state.name()).collect();
    assert_eq!(
        names,
        [
            "working",
            "waiting",
            "ready",
            "compacting",
            "error",
            "starting",
            "exited",
            "unknown"
        ]
    );
    for state in LiveState::ALL {
        assert_eq!(
            serde_json::to_value(state).expect("serialize state"),
            serde_json::Value::String(state.name().to_string())
        );
        assert_eq!(LiveState::from_name(state.name()), Some(state));
    }
}

#[test]
fn details_serialize_by_stable_lowercase_names() {
    let names: Vec<&str> = LiveDetail::ALL.iter().map(|detail| detail.name()).collect();
    assert_eq!(
        names,
        [
            "generating",
            "tool",
            "retrying",
            "rate_limited",
            "permission",
            "question",
            "plan",
            "input",
            "finished",
            "interrupted",
            "auth",
            "overloaded",
            "failed",
            "ended",
            "killed",
            "stale"
        ]
    );
    for detail in LiveDetail::ALL {
        assert!(
            detail
                .name()
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_')
        );
        assert_eq!(
            serde_json::to_value(detail).expect("serialize detail"),
            serde_json::Value::String(detail.name().to_string())
        );
        assert_eq!(LiveDetail::from_name(detail.name()), Some(detail));
    }
}

#[test]
fn sources_and_confidences_serialize_by_stable_lowercase_names() {
    let sources: Vec<&str> = LiveSource::ALL.iter().map(|source| source.name()).collect();
    assert_eq!(sources, ["hook", "unknown"]);
    for source in LiveSource::ALL {
        assert_eq!(
            serde_json::to_value(source).expect("serialize source"),
            serde_json::Value::String(source.name().to_string())
        );
        assert_eq!(LiveSource::from_name(source.name()), Some(source));
    }
    let confidences: Vec<&str> = LiveConfidence::ALL.iter().map(|c| c.name()).collect();
    assert_eq!(confidences, ["fresh", "quiet", "stale"]);
    for confidence in LiveConfidence::ALL {
        assert_eq!(
            serde_json::to_value(confidence).expect("serialize confidence"),
            serde_json::Value::String(confidence.name().to_string())
        );
    }
}

#[test]
fn liveness_serializes_by_stable_lowercase_names_and_defaults_to_unknown() {
    let names: Vec<&str> = Liveness::ALL.iter().map(|l| l.name()).collect();
    assert_eq!(names, ["alive", "dead", "unknown"]);
    for liveness in Liveness::ALL {
        assert_eq!(
            serde_json::to_value(liveness).expect("serialize liveness"),
            serde_json::Value::String(liveness.name().to_string())
        );
        assert_eq!(Liveness::from_name(liveness.name()), Some(liveness));
    }
    let undead: Liveness = serde_json::from_str(r#""undead""#).expect("lenient liveness");
    assert_eq!(undead, Liveness::Unknown);
    assert_eq!(Liveness::default(), Liveness::Unknown);
}

#[test]
fn a_binding_reads_its_observation_with_the_bound_liveness() {
    let waiting = observed(Waiting).with_detail(Permission);
    let hour_later = waiting.observed_ms + 60 * MINUTE;
    for liveness in Liveness::ALL {
        assert_eq!(
            LiveBinding::new(waiting, liveness).effective(hour_later),
            waiting.effective(hour_later, liveness),
            "{liveness:?}"
        );
    }
}

#[test]
fn a_binding_serializes_as_its_observation_beside_the_liveness() {
    let binding = LiveBinding::new(full_observation(), Liveness::Dead);
    let mut expected = serde_json::to_value(full_observation()).expect("serialize observation");
    expected["liveness"] = serde_json::json!("dead");
    assert_eq!(
        serde_json::to_value(binding).expect("serialize binding"),
        expected
    );
    let read: LiveBinding = serde_json::from_value(expected).expect("deserialize binding");
    assert_eq!(read, binding);

    let unchecked: LiveBinding =
        serde_json::from_value(serde_json::to_value(full_observation()).expect("observation"))
            .expect("a binding without liveness loads");
    assert_eq!(
        unchecked,
        LiveBinding::new(full_observation(), Liveness::Unknown)
    );

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&binding).expect("archive binding");
    let decoded =
        rkyv::from_bytes::<LiveBinding, rkyv::rancor::Error>(&bytes).expect("deserialize binding");
    assert_eq!(decoded, binding);
}

#[test]
fn each_state_carries_exactly_the_details_of_the_vocabulary() {
    let table: Vec<(&str, Vec<&str>)> = LiveState::ALL
        .into_iter()
        .map(|state| {
            let details = state.details().iter().map(|detail| detail.name()).collect();
            (state.name(), details)
        })
        .collect();
    assert_eq!(
        table,
        vec![
            (
                "working",
                vec!["generating", "tool", "retrying", "rate_limited"]
            ),
            ("waiting", vec!["permission", "question", "plan", "input"]),
            ("ready", vec!["finished", "interrupted"]),
            ("compacting", vec![]),
            (
                "error",
                vec!["rate_limited", "auth", "overloaded", "failed"]
            ),
            ("starting", vec![]),
            ("exited", vec!["ended", "killed"]),
            ("unknown", vec!["stale"]),
        ]
    );
}

#[test]
fn every_detail_belongs_to_a_state_and_only_rate_limited_to_two() {
    for detail in LiveDetail::ALL {
        let owners: Vec<LiveState> = LiveState::ALL
            .into_iter()
            .filter(|state| state.accepts(detail))
            .collect();
        let expected = if detail == RateLimited { 2 } else { 1 };
        assert_eq!(owners.len(), expected, "{detail:?} belongs to {owners:?}");
    }
}

#[test]
fn with_detail_keeps_only_a_detail_the_state_accepts() {
    assert_eq!(
        observed(Waiting).with_detail(Permission).detail,
        Some(Permission)
    );
    assert_eq!(
        observed(Error).with_detail(RateLimited).detail,
        Some(RateLimited)
    );
    assert_eq!(observed(Ready).with_detail(Permission).detail, None);
    assert_eq!(observed(Compacting).with_detail(Tool).detail, None);
    assert_eq!(observed(Starting).with_detail(Stale).detail, None);
}

#[test]
fn a_new_observation_begins_when_it_is_observed() {
    let observation = observed(Working);
    assert_eq!(observation.since_ms, T0);
    assert_eq!(observation.observed_ms, T0);
    assert_eq!(observation.detail, None);
    assert_eq!(observation.subagents, 0);
    assert!(!observation.via_subagent);
    assert_eq!(observation.pid, None);
    assert_eq!(observation.pid_start, None);
    assert_eq!(observation.heartbeat_s, 0);
}

// --- Effective state: process liveness -------------------------------

#[test]
fn a_dead_process_reads_exited_killed_whatever_the_state() {
    for state in states_except_exited() {
        let observation = observed(state);
        assert_eq!(after(&observation, 0, Liveness::Dead), KILLED, "{state:?}");
        assert_eq!(
            after(&observation, 30 * DAY, Liveness::Dead),
            KILLED,
            "{state:?}"
        );
    }
}

#[test]
fn a_changed_start_time_means_the_recorded_process_is_dead() {
    assert_eq!(Liveness::of_running_pid(Some(10), Some(11)), Liveness::Dead);
    assert_eq!(
        Liveness::of_running_pid(Some(10), Some(10)),
        Liveness::Alive
    );
    assert_eq!(Liveness::of_running_pid(None, Some(10)), Liveness::Alive);
    assert_eq!(Liveness::of_running_pid(Some(10), None), Liveness::Alive);
    assert_eq!(Liveness::of_running_pid(None, None), Liveness::Alive);

    let waiting = observed(Waiting)
        .with_detail(Permission)
        .with_pid(4242, Some(10));
    let reused = Liveness::of_running_pid(waiting.pid_start, Some(99));
    assert_eq!(after(&waiting, SECOND, reused), KILLED);
}

#[test]
fn exited_is_never_revived_by_time_or_liveness() {
    for detail in [Some(Ended), Some(Killed), None] {
        let mut observation = observed(Exited);
        observation.detail = detail;
        for liveness in [Liveness::Alive, Liveness::Dead, Liveness::Unknown] {
            for silent_ms in [0, UNKNOWN_LIVENESS_WINDOW_MS + 1, 365 * DAY] {
                assert_eq!(
                    after(&observation, silent_ms, liveness),
                    shown(Exited, detail, Fresh),
                    "{detail:?} {liveness:?} {silent_ms}"
                );
            }
        }
    }
}

// --- Effective state: the quiet window -------------------------------

#[test]
fn working_and_compacting_turn_quiet_just_past_ten_minutes_without_heartbeat() {
    let working = observed(Working).with_detail(Tool);
    assert_eq!(working.quiet_window_ms(), 10 * MINUTE);
    assert_eq!(
        after(&working, 10 * MINUTE, Liveness::Alive),
        shown(Working, Some(Tool), Fresh)
    );
    assert_eq!(
        after(&working, 10 * MINUTE + 1, Liveness::Alive),
        shown(Working, Some(Tool), Quiet)
    );

    let compacting = observed(Compacting);
    assert_eq!(
        after(&compacting, 10 * MINUTE, Liveness::Alive),
        shown(Compacting, None, Fresh)
    );
    assert_eq!(
        after(&compacting, 10 * MINUTE + 1, Liveness::Alive),
        shown(Compacting, None, Quiet)
    );
}

#[test]
fn a_quiet_state_never_decays_while_the_process_lives() {
    for state in [Working, Compacting] {
        assert_eq!(
            after(&observed(state), 30 * DAY, Liveness::Alive),
            shown(state, None, Quiet),
            "{state:?}"
        );
    }
}

#[test]
fn a_heartbeat_source_turns_quiet_just_past_three_heartbeats() {
    let working = observed(Working)
        .with_detail(Generating)
        .with_heartbeat_s(60);
    assert_eq!(working.quiet_window_ms(), 180 * SECOND);
    assert_eq!(
        after(&working, 180 * SECOND, Liveness::Alive),
        shown(Working, Some(Generating), Fresh)
    );
    assert_eq!(
        after(&working, 180 * SECOND + 1, Liveness::Alive),
        shown(Working, Some(Generating), Quiet)
    );

    let compacting = observed(Compacting).with_heartbeat_s(15);
    assert_eq!(
        after(&compacting, 45 * SECOND, Liveness::Alive),
        shown(Compacting, None, Fresh)
    );
    assert_eq!(
        after(&compacting, 45 * SECOND + 1, Liveness::Alive),
        shown(Compacting, None, Quiet)
    );
}

#[test]
fn a_slow_heartbeat_widens_the_quiet_window_past_ten_minutes() {
    let working = observed(Working).with_heartbeat_s(300);
    assert_eq!(working.quiet_window_ms(), 15 * MINUTE);
    assert_eq!(
        after(&working, 10 * MINUTE + 1, Liveness::Alive),
        shown(Working, None, Fresh)
    );
    assert_eq!(
        after(&working, 15 * MINUTE, Liveness::Alive),
        shown(Working, None, Fresh)
    );
    assert_eq!(
        after(&working, 15 * MINUTE + 1, Liveness::Alive),
        shown(Working, None, Quiet)
    );
}

#[test]
fn waiting_ready_and_error_do_not_age_while_the_process_lives() {
    let cases = [
        observed(Waiting).with_detail(Question),
        observed(Ready).with_detail(Finished),
        observed(Error).with_detail(Overloaded),
        observed(Waiting).with_heartbeat_s(60),
    ];
    for observation in cases {
        for silent_ms in [
            QUIET_WINDOW_MS + 1,
            UNKNOWN_LIVENESS_WINDOW_MS + 1,
            30 * DAY,
        ] {
            assert_eq!(
                after(&observation, silent_ms, Liveness::Alive),
                shown(observation.state, observation.detail, Fresh),
                "{observation:?} {silent_ms}"
            );
        }
    }
}

#[test]
fn windows_are_measured_from_the_latest_evidence() {
    // The state began an hour before its latest evidence; only the
    // evidence's age counts.
    let working = observed(Working).with_since_ms(T0 - 60 * MINUTE);
    assert_eq!(
        after(&working, MINUTE, Liveness::Alive),
        shown(Working, None, Fresh)
    );
    let starting = observed(Starting).with_since_ms(T0 - 60 * MINUTE);
    assert_eq!(
        after(&starting, MINUTE, Liveness::Alive),
        shown(Starting, None, Fresh)
    );
}

#[test]
fn evidence_stamped_after_now_counts_as_just_arrived() {
    let working = observed(Working);
    assert_eq!(working.silent_ms(T0 - MINUTE), 0);
    assert_eq!(
        working.effective(T0 - 30 * MINUTE, Liveness::Unknown),
        shown(Working, None, Fresh)
    );
}

// --- Effective state: starting ----------------------------------------

#[test]
fn starting_goes_stale_just_past_two_minutes() {
    let starting = observed(Starting);
    for liveness in [Liveness::Alive, Liveness::Unknown] {
        assert_eq!(
            after(&starting, 2 * MINUTE, liveness),
            shown(Starting, None, Fresh),
            "{liveness:?}"
        );
        assert_eq!(
            after(&starting, 2 * MINUTE + 1, liveness),
            EffectiveLive::STALE,
            "{liveness:?}"
        );
    }
}

#[test]
fn starting_with_a_short_heartbeat_turns_quiet_before_it_goes_stale() {
    let starting = observed(Starting).with_heartbeat_s(30);
    assert_eq!(
        after(&starting, 90 * SECOND, Liveness::Alive),
        shown(Starting, None, Fresh)
    );
    assert_eq!(
        after(&starting, 90 * SECOND + 1, Liveness::Alive),
        shown(Starting, None, Quiet)
    );
    assert_eq!(
        after(&starting, 2 * MINUTE, Liveness::Alive),
        shown(Starting, None, Quiet)
    );
    assert_eq!(
        after(&starting, 2 * MINUTE + 1, Liveness::Alive),
        EffectiveLive::STALE
    );
}

// --- Effective state: unknown liveness --------------------------------

#[test]
fn with_unknown_liveness_states_hold_until_fifteen_minutes() {
    let at_cutoff = |observation: LiveObservation| {
        after(&observation, UNKNOWN_LIVENESS_WINDOW_MS, Liveness::Unknown)
    };
    assert_eq!(UNKNOWN_LIVENESS_WINDOW_MS, 15 * MINUTE);
    assert_eq!(
        at_cutoff(observed(Waiting).with_detail(Plan)),
        shown(Waiting, Some(Plan), Fresh)
    );
    assert_eq!(
        at_cutoff(observed(Ready).with_detail(Interrupted)),
        shown(Ready, Some(Interrupted), Fresh)
    );
    assert_eq!(
        at_cutoff(observed(Error).with_detail(Auth)),
        shown(Error, Some(Auth), Fresh)
    );
    // Past the quiet window but not the cutoff: still working, quiet.
    assert_eq!(
        at_cutoff(observed(Working).with_detail(Retrying)),
        shown(Working, Some(Retrying), Quiet)
    );
    assert_eq!(
        at_cutoff(observed(Compacting)),
        shown(Compacting, None, Quiet)
    );
    assert_eq!(at_cutoff(observed(Unknown)), shown(Unknown, None, Fresh));
    // Fresh heartbeats keep a working state fresh with no pid to check.
    assert_eq!(
        after(
            &observed(Working).with_heartbeat_s(60),
            3 * MINUTE,
            Liveness::Unknown
        ),
        shown(Working, None, Fresh)
    );
}

#[test]
fn with_unknown_liveness_every_state_but_exited_goes_stale_just_past_fifteen_minutes() {
    for state in states_except_exited() {
        for heartbeat_s in [0, 60] {
            let observation = observed(state).with_heartbeat_s(heartbeat_s);
            assert_eq!(
                after(
                    &observation,
                    UNKNOWN_LIVENESS_WINDOW_MS + 1,
                    Liveness::Unknown
                ),
                EffectiveLive::STALE,
                "{state:?} heartbeat {heartbeat_s}"
            );
        }
    }
}

#[test]
fn the_fifteen_minute_cutoff_needs_unknown_liveness() {
    let working = observed(Working);
    assert_eq!(
        after(&working, UNKNOWN_LIVENESS_WINDOW_MS + 1, Liveness::Alive),
        shown(Working, None, Quiet)
    );
}

// --- Effective state: odd records -------------------------------------

#[test]
fn a_detail_that_does_not_fit_the_state_is_dropped_when_read() {
    let mut ready = observed(Ready);
    ready.detail = Some(Permission);
    assert_eq!(after(&ready, 0, Liveness::Alive), shown(Ready, None, Fresh));
}

#[test]
fn a_recorded_unknown_reads_unknown() {
    assert_eq!(
        after(&observed(Unknown), 0, Liveness::Alive),
        shown(Unknown, None, Fresh)
    );
    assert_eq!(
        after(&observed(Unknown).with_detail(Stale), 0, Liveness::Alive),
        EffectiveLive::STALE
    );
}

// --- Counts and precedence --------------------------------------------

#[test]
fn counts_tally_each_state() {
    let mut counts: LiveCounts = [Working, Working, Waiting, Exited].into_iter().collect();
    assert_eq!(counts.get(Working), 2);
    assert_eq!(counts.get(Waiting), 1);
    assert_eq!(counts.get(Exited), 1);
    assert_eq!(counts.get(Ready), 0);
    assert_eq!(counts.total(), 4);
    assert!(!counts.is_empty());

    counts.extend([Ready, Unknown]);
    counts.add(Compacting);
    assert_eq!(counts.total(), 7);
    assert_eq!(
        counts,
        LiveCounts {
            working: 2,
            waiting: 1,
            ready: 1,
            compacting: 1,
            error: 0,
            starting: 0,
            exited: 1,
            unknown: 1,
        }
    );

    let empty = LiveCounts::default();
    assert!(empty.is_empty());
    assert_eq!(empty.total(), 0);
    assert_eq!(empty.dominant(), None);
}

#[test]
fn the_default_precedence_puts_what_needs_the_operator_first() {
    let mut ranked = LiveState::ALL;
    ranked.sort_by_key(|state| state.default_precedence());
    assert_eq!(
        ranked,
        [
            Waiting, Error, Ready, Working, Compacting, Starting, Unknown, Exited
        ]
    );

    // Remove the dominant state one at a time: each step yields the next.
    let mut counts: LiveCounts = LiveState::ALL.into_iter().collect();
    for expected in ranked {
        assert_eq!(counts.dominant(), Some(expected));
        counts = LiveState::ALL
            .into_iter()
            .filter(|state| state.default_precedence() > expected.default_precedence())
            .collect();
    }
    assert_eq!(counts.dominant(), None);
}

#[test]
fn a_viewer_may_replace_the_precedence() {
    let counts: LiveCounts = [Waiting, Working, Compacting].into_iter().collect();
    assert_eq!(counts.dominant(), Some(Waiting));
    // A viewer that leads with activity.
    let busy_first = |state: LiveState| match state {
        Working | Compacting => 0,
        _ => 1,
    };
    assert_eq!(counts.dominant_by(busy_first), Some(Working));
}

// --- Serialization ----------------------------------------------------

fn full_observation() -> LiveObservation {
    let mut observation = observed(Waiting)
        .with_detail(Permission)
        .with_since_ms(T0 - MINUTE)
        .with_subagents(2)
        .with_pid(4242, Some(987_654))
        .with_heartbeat_s(60);
    observation.via_subagent = true;
    observation
}

#[test]
fn an_observation_round_trips_through_json() {
    let observation = full_observation();
    let json = serde_json::to_string(&observation).expect("serialize observation");
    let decoded: LiveObservation = serde_json::from_str(&json).expect("deserialize observation");
    assert_eq!(decoded, observation);
}

#[test]
fn an_observation_serializes_its_fields_in_a_stable_shape() {
    assert_eq!(
        serde_json::to_value(full_observation()).expect("serialize observation"),
        serde_json::json!({
            "state": "waiting",
            "detail": "permission",
            "subagents": 2,
            "via_subagent": true,
            "since_ms": T0 - MINUTE,
            "observed_ms": T0,
            "source": "hook",
            "pid": 4242,
            "pid_start": 987_654,
            "heartbeat_s": 60,
        })
    );
    // Defaults are left out.
    assert_eq!(
        serde_json::to_string(&observed(Ready)).expect("serialize observation"),
        format!(r#"{{"state":"ready","since_ms":{T0},"observed_ms":{T0},"source":"hook"}}"#)
    );
}

#[test]
fn effective_states_and_counts_round_trip_through_json() {
    for effective in [
        EffectiveLive::STALE,
        KILLED,
        shown(Working, Some(Tool), Quiet),
        shown(Compacting, None, Fresh),
    ] {
        let json = serde_json::to_string(&effective).expect("serialize effective");
        let decoded: EffectiveLive = serde_json::from_str(&json).expect("deserialize effective");
        assert_eq!(decoded, effective);
    }

    let counts: LiveCounts = [Waiting, Waiting, Error].into_iter().collect();
    let value = serde_json::to_value(counts).expect("serialize counts");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("counts serialize as an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let mut names: Vec<&str> = LiveState::ALL.iter().map(|state| state.name()).collect();
    names.sort_unstable();
    assert_eq!(keys, names, "one count per state, keyed by its name");
    let decoded: LiveCounts = serde_json::from_value(value).expect("deserialize counts");
    assert_eq!(decoded, counts);

    let partial: LiveCounts = serde_json::from_str(r#"{"waiting":2}"#).expect("partial counts");
    assert_eq!(partial.get(Waiting), 2);
    assert_eq!(partial.total(), 2);
}

#[test]
fn the_live_types_round_trip_through_rkyv() {
    let observation = full_observation();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&observation).expect("archive observation");
    let decoded = rkyv::from_bytes::<LiveObservation, rkyv::rancor::Error>(&bytes)
        .expect("deserialize observation");
    assert_eq!(decoded, observation);

    for state in LiveState::ALL {
        let observation = observed(state);
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&observation).expect("archive");
        let decoded =
            rkyv::from_bytes::<LiveObservation, rkyv::rancor::Error>(&bytes).expect("deserialize");
        assert_eq!(decoded, observation);
    }

    let effective = shown(Working, Some(RateLimited), Quiet);
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&effective).expect("archive effective");
    let decoded = rkyv::from_bytes::<EffectiveLive, rkyv::rancor::Error>(&bytes)
        .expect("deserialize effective");
    assert_eq!(decoded, effective);

    let counts: LiveCounts = LiveState::ALL.into_iter().chain([Ready]).collect();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&counts).expect("archive counts");
    let decoded =
        rkyv::from_bytes::<LiveCounts, rkyv::rancor::Error>(&bytes).expect("deserialize counts");
    assert_eq!(decoded, counts);
}

// --- Lenient reading --------------------------------------------------

#[test]
fn an_unknown_state_name_reads_as_unknown() {
    let state: LiveState = serde_json::from_str(r#""napping""#).expect("lenient state");
    assert_eq!(state, Unknown);

    let observation: LiveObservation = serde_json::from_str(&format!(
        r#"{{"state":"napping","detail":"permission","since_ms":{T0},"observed_ms":{T0},"source":"hook"}}"#
    ))
    .expect("an observation with an unknown state still loads");
    assert_eq!(observation.state, Unknown);
    // The detail no longer fits, so it is dropped when read.
    assert_eq!(
        after(&observation, 0, Liveness::Alive),
        shown(Unknown, None, Fresh)
    );
}

#[test]
fn an_unknown_detail_or_source_name_does_not_fail_the_observation() {
    let observation: LiveObservation = serde_json::from_str(&format!(
        r#"{{"state":"waiting","detail":"pondering","since_ms":{T0},"observed_ms":{T0},"source":"telepathy","future_field":[1,2]}}"#
    ))
    .expect("unknown names and fields still load");
    assert_eq!(observation.state, Waiting);
    assert_eq!(observation.detail, None);
    assert_eq!(observation.source, LiveSource::Unknown);

    let effective: EffectiveLive =
        serde_json::from_str(r#"{"state":"ready","detail":"pondering","confidence":"fresh"}"#)
            .expect("lenient effective detail");
    assert_eq!(effective, shown(Ready, None, Fresh));

    let explicit_null: LiveObservation = serde_json::from_str(&format!(
        r#"{{"state":"ready","detail":null,"since_ms":{T0},"observed_ms":{T0},"source":"hook"}}"#
    ))
    .expect("a null detail loads");
    assert_eq!(explicit_null.detail, None);
}

#[test]
fn a_state_that_is_not_a_name_still_fails() {
    assert!(serde_json::from_str::<LiveState>("3").is_err());
    assert!(serde_json::from_str::<LiveObservation>(r#"{"state":"ready"}"#).is_err());
}

// --- Provider neutrality ----------------------------------------------

#[test]
fn the_module_names_no_harness() {
    let source = include_str!("live.rs").to_ascii_lowercase();
    let words: Vec<&str> = source
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    // Each registered harness by the first word of its key, so
    // `claude_code` is caught as `claude`.
    for key in crate::discovery::harness::harness_keys() {
        let name = key.split('_').next().unwrap_or(key);
        assert!(
            !words.contains(&name),
            "src/model/live.rs names the harness `{name}`"
        );
    }
}
