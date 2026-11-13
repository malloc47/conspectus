use std::path::PathBuf;

use super::*;
use crate::model::live::{LiveDetail as D, LiveState as S};

/// Planted in every text field of `sentinel.json`.
const SENTINEL: &str = "sentinel-poll-7d31aa";

const AT: i64 = 1_794_546_100_000;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude-agents")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

/// The one session a fixture lists: its pid, session id, state, detail.
fn only(name: &str) -> (Option<i64>, Option<String>, S, Option<D>) {
    let poll = parse(&fixture(name), AT).unwrap_or_else(|| panic!("{name} parses"));
    assert_eq!(poll.observed_ms, AT);
    let [agent] = poll.agents.as_slice() else {
        panic!("{name} lists one session: {poll:?}");
    };
    assert_eq!(agent.since_ms, AT, "a run starts at its first poll");
    assert_eq!(agent.pid_start, None, "the parser reads no process table");
    (
        agent.pid,
        agent.session_id.clone(),
        agent.state,
        agent.detail,
    )
}

#[test]
fn every_documented_shape_reads_its_state() {
    let interactive = Some("8c1d2f0e-5b7a-4c39-9e21-3a6f0b9d4e17".to_string());
    let cases = [
        (
            "interactive-busy.json",
            (
                Some(41234),
                interactive.clone(),
                S::Working,
                Some(D::Generating),
            ),
        ),
        (
            "interactive-waiting-permission.json",
            (
                Some(41234),
                interactive.clone(),
                S::Waiting,
                Some(D::Permission),
            ),
        ),
        (
            "interactive-waiting-input.json",
            (Some(41234), interactive.clone(), S::Waiting, Some(D::Input)),
        ),
        (
            "interactive-idle.json",
            (Some(41234), interactive, S::Ready, None),
        ),
        (
            "background-blocked.json",
            (
                Some(52001),
                Some("1f4e7a2b-9c3d-4b6e-8f10-2d5c7e9a0b31".to_string()),
                S::Waiting,
                Some(D::Permission),
            ),
        ),
        (
            "background-done.json",
            (
                None,
                Some("6a0b3c9d-2e4f-4a71-b8c5-9d1e3f5a7b02".to_string()),
                S::Ready,
                Some(D::Finished),
            ),
        ),
        (
            "background-failed.json",
            (Some(52003), None, S::Error, Some(D::Failed)),
        ),
        (
            "background-stopped.json",
            (
                None,
                Some("c3d5e7f9-0a2b-4c6d-8e0f-1a3b5c7d9e11".to_string()),
                S::Exited,
                Some(D::Ended),
            ),
        ),
    ];
    for (name, expected) in cases {
        assert_eq!(only(name), expected, "{name}");
    }
}

/// `status`, `state`, and `waitingFor`, and what they read as.
type Case = (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
    Option<(S, Option<D>)>,
);

/// A listed session's pid, session id, state, and detail.
type Read<'a> = (Option<i64>, Option<&'a str>, S, Option<D>);

#[test]
fn the_mapping_table_reads_status_first_then_state() {
    let cases: &[Case] = &[
        (
            Some("busy"),
            None,
            None,
            Some((S::Working, Some(D::Generating))),
        ),
        (
            Some("waiting"),
            None,
            None,
            Some((S::Waiting, Some(D::Input))),
        ),
        (
            Some("waiting"),
            None,
            Some("permission prompt"),
            Some((S::Waiting, Some(D::Permission))),
        ),
        (
            Some("waiting"),
            None,
            Some("sandbox request"),
            Some((S::Waiting, Some(D::Permission))),
        ),
        (
            Some("waiting"),
            None,
            Some("input needed"),
            Some((S::Waiting, Some(D::Input))),
        ),
        (
            Some("waiting"),
            None,
            Some("worker request"),
            Some((S::Waiting, Some(D::Input))),
        ),
        (
            Some("waiting"),
            None,
            Some("dialog open"),
            Some((S::Waiting, Some(D::Input))),
        ),
        (Some("idle"), None, None, Some((S::Ready, None))),
        (
            None,
            Some("working"),
            None,
            Some((S::Working, Some(D::Generating))),
        ),
        (
            None,
            Some("blocked"),
            None,
            Some((S::Waiting, Some(D::Input))),
        ),
        (
            None,
            Some("blocked"),
            Some("permission prompt"),
            Some((S::Waiting, Some(D::Permission))),
        ),
        (
            None,
            Some("done"),
            None,
            Some((S::Ready, Some(D::Finished))),
        ),
        (
            None,
            Some("failed"),
            None,
            Some((S::Error, Some(D::Failed))),
        ),
        (
            None,
            Some("stopped"),
            None,
            Some((S::Exited, Some(D::Ended))),
        ),
        // `status` decides when it holds one of its values.
        (Some("idle"), Some("working"), None, Some((S::Ready, None))),
        // Otherwise `state` does.
        (
            Some("paused"),
            Some("working"),
            None,
            Some((S::Working, Some(D::Generating))),
        ),
        // Trimmed and lowercased.
        (
            Some("  BUSY "),
            None,
            None,
            Some((S::Working, Some(D::Generating))),
        ),
        (
            Some("Waiting"),
            None,
            Some(" Permission Prompt "),
            Some((S::Waiting, Some(D::Permission))),
        ),
        // Nothing this table lists.
        (Some("thinking"), None, None, None),
        (None, Some("queued"), None, None),
        (Some("thinking"), Some("queued"), None, None),
        (None, None, Some("permission prompt"), None),
        (None, None, None, None),
        (Some(""), Some(""), None, None),
    ];
    for (status, state, waiting_for, expected) in cases {
        assert_eq!(
            reading(*status, *state, *waiting_for),
            *expected,
            "status {status:?}, state {state:?}, waitingFor {waiting_for:?}"
        );
    }
}

#[test]
fn values_no_version_documents_assert_nothing_or_fall_back() {
    let poll = parse(&fixture("future-values.json"), AT).expect("parses");
    let read: Vec<(Option<i64>, S, Option<D>)> = poll
        .agents
        .iter()
        .map(|agent| (agent.pid, agent.state, agent.detail))
        .collect();
    // Dropped: an unknown status with no state (60001); a text pid and a
    // number session id, which read as absent and leave nothing to match
    // the session by; a status and a state of the wrong types (60005); no
    // pid or session id at all; and a pid of 1, which is no agent's.
    assert_eq!(
        read,
        [
            // An unknown status with a known state falls back to it.
            (Some(60002), S::Working, Some(D::Generating)),
            // An unknown `waitingFor` is other input.
            (Some(60003), S::Waiting, Some(D::Input)),
            // Trimmed and lowercased; an unknown field is skipped.
            (Some(60008), S::Ready, None),
        ]
    );
}

#[test]
fn an_empty_list_is_a_poll_with_no_sessions() {
    let poll = parse(&fixture("empty.json"), AT).expect("parses");
    assert_eq!(poll.observed_ms, AT);
    assert!(poll.agents.is_empty());
}

#[test]
fn output_that_is_not_a_list_of_objects_is_no_poll() {
    assert_eq!(parse(&fixture("malformed.json"), AT), None);
    assert_eq!(parse(&fixture("not-a-list.json"), AT), None);
    assert_eq!(parse(b"", AT), None);
    assert_eq!(parse(b"null", AT), None);
    assert_eq!(
        parse(br#"[{"pid": 2, "status": "idle"}, "text"]"#, AT),
        None
    );
    assert_eq!(parse(&[0xff, 0xfe, 0x00], AT), None);
}

#[test]
fn output_past_the_cap_is_no_poll() {
    let mut big = br#"[{"pid": 41234, "status": "idle", "name": ""#.to_vec();
    big.resize(MAX_OUTPUT_BYTES, b'x');
    big.extend_from_slice(br#""}]"#);
    assert!(big.len() > MAX_OUTPUT_BYTES);
    assert_eq!(parse(&big, AT), None);

    let mut fits = br#"[{"pid": 41234, "status": "idle", "name": ""#.to_vec();
    fits.resize(MAX_OUTPUT_BYTES - 3, b'x');
    fits.extend_from_slice(br#""}]"#);
    assert_eq!(fits.len(), MAX_OUTPUT_BYTES);
    let poll = parse(&fits, AT).expect("a listing at the cap parses");
    assert_eq!(poll.agents.len(), 1);
}

#[test]
fn a_poll_keeps_at_most_the_cap_of_sessions() {
    let listed: Vec<serde_json::Value> = (0..MAX_AGENTS + 10)
        .map(|index| serde_json::json!({"pid": 1000 + index, "status": "busy"}))
        .collect();
    let bytes = serde_json::to_vec(&listed).unwrap();
    let poll = parse(&bytes, AT).expect("parses");
    assert_eq!(poll.agents.len(), MAX_AGENTS);
    assert_eq!(poll.agents[0].pid, Some(1000));
}

#[test]
fn only_a_plain_session_id_is_kept() {
    let id = |session_id: &str| {
        let bytes = serde_json::to_vec(&serde_json::json!([
            {"pid": 41234, "sessionId": session_id, "status": "idle"}
        ]))
        .unwrap();
        parse(&bytes, AT).expect("parses").agents[0]
            .session_id
            .clone()
    };
    for kept in [
        "8c1d2f0e-5b7a-4c39-9e21-3a6f0b9d4e17",
        "ses_01HZX.part:2",
        &"a".repeat(128),
    ] {
        assert_eq!(id(kept).as_deref(), Some(kept));
    }
    for dropped in [
        "",
        "two words",
        "line\nbreak",
        "tab\tbed",
        "quote\"d",
        "slash/ed",
        "ünïcode",
        &"a".repeat(129),
    ] {
        assert_eq!(id(dropped), None, "{dropped:?}");
    }
}

#[test]
fn only_a_pid_above_one_is_kept() {
    let pid = |value: serde_json::Value| {
        let bytes = serde_json::to_vec(&serde_json::json!([
            {"pid": value, "sessionId": "s-1", "status": "idle"}
        ]))
        .unwrap();
        parse(&bytes, AT).expect("parses").agents[0].pid
    };
    assert_eq!(pid(serde_json::json!(2)), Some(2));
    assert_eq!(pid(serde_json::json!(4_194_304)), Some(4_194_304));
    for dropped in [
        serde_json::json!(1),
        serde_json::json!(0),
        serde_json::json!(-7),
        serde_json::json!(12.5),
        serde_json::json!("41234"),
        serde_json::json!(null),
        serde_json::json!(true),
    ] {
        assert_eq!(pid(dropped.clone()), None, "{dropped}");
    }
}

#[test]
fn nothing_but_enums_ids_and_times_leaves_the_parser() {
    let poll = parse(&fixture("sentinel.json"), AT).expect("parses");
    let read: Vec<Read<'_>> = poll
        .agents
        .iter()
        .map(|agent| {
            (
                agent.pid,
                agent.session_id.as_deref(),
                agent.state,
                agent.detail,
            )
        })
        .collect();
    assert_eq!(
        read,
        [
            (Some(41234), None, S::Ready, None),
            (
                Some(41235),
                Some("2b4d6f8a-1c3e-4a5b-9d7f-0e2a4c6b8d10"),
                S::Waiting,
                Some(D::Input)
            ),
        ]
    );
    let debug = format!("{poll:?}");
    let json = serde_json::to_string(&poll).unwrap();
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&poll).unwrap();
    assert!(!debug.contains(SENTINEL), "{debug}");
    assert!(!json.contains(SENTINEL), "{json}");
    assert!(
        !archived
            .windows(SENTINEL.len())
            .any(|window| window == SENTINEL.as_bytes()),
        "the archived poll holds the sentinel"
    );
}

#[test]
fn the_poller_runs_claude_agents_json_from_path() {
    let poller = poller();
    assert_eq!(poller.harness(), HARNESS_KEY);
    let command = AgentList::system().command();
    assert_eq!(command.get_program(), "claude");
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, ["agents", "--json"]);
    let envs: Vec<_> = command.get_envs().collect();
    assert_eq!(
        envs,
        [(
            std::ffi::OsStr::new("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"),
            Some(std::ffi::OsStr::new("1"))
        )]
    );
}

#[test]
fn the_claude_code_adapter_offers_the_poller() {
    let adapter = crate::discovery::harness::registered_adapters()
        .find(|adapter| adapter.harness_key() == HARNESS_KEY)
        .expect("claude-code is registered");
    let poller = adapter.live_poller().expect("claude-code polls");
    assert_eq!(poller.harness(), HARNESS_KEY);
    let others: Vec<&str> = crate::discovery::harness::registered_adapters()
        .filter(|adapter| adapter.live_poller().is_some())
        .map(crate::discovery::harness::HarnessAdapter::harness_key)
        .collect();
    assert_eq!(others, [HARNESS_KEY], "no other harness polls");
}
