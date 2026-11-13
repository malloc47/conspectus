//! The hook fixture corpora of the event-source harnesses, read and
//! replayed for their tests. A corpus is a directory of single payloads
//! (`*.json`) and ordered sequences (`seq-*.jsonl`); each corpus's
//! README says how a sequence is laid out. [`run_sequence`] drives one
//! through the whole writer path: the adapter's mapping, the store in a
//! scratch directory, the entry read back, and the state a reader shows,
//! checked after every step.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::discovery::harness::HarnessAdapter;
use crate::live_store::{LiveContext, LiveStore};
use crate::model::live::{LiveConfidence, LiveDetail, LiveState, Liveness};

/// The fixture directory of a corpus under `tests/fixtures/hooks/`.
pub(crate) fn corpus_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hooks")
        .join(name)
}

pub(crate) fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// The files in `dir` with `extension`, by stem.
pub(crate) fn fixture_names(dir: &Path, extension: &str) -> BTreeSet<String> {
    fs::read_dir(dir)
        .expect("fixture directory")
        .filter_map(|entry| {
            let path = entry.expect("fixture entry").path();
            (path.extension().and_then(|ext| ext.to_str()) == Some(extension)).then(|| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .expect("stem")
                    .to_string()
            })
        })
        .collect()
}

/// A sequence's header and steps.
pub(crate) fn sequence(dir: &Path, name: &str) -> (Value, Vec<Value>) {
    let path = dir.join(format!("{name}.jsonl"));
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    let mut lines = text.lines().map(|line| {
        serde_json::from_str::<Value>(line).unwrap_or_else(|err| panic!("{name}: {err}: {line}"))
    });
    let header = lines.next().expect("a header line");
    (header, lines.collect())
}

fn liveness(value: &Value) -> Liveness {
    match value.as_str() {
        None | Some("alive") => Liveness::Alive,
        Some("dead") => Liveness::Dead,
        Some("unknown") => Liveness::Unknown,
        Some(other) => panic!("unknown liveness {other}"),
    }
}

/// Check the effective state of `harness`'s `session` against `expect`
/// at `at_ms`.
pub(crate) fn assert_shown(
    store: &LiveStore,
    harness: &str,
    session: &str,
    at_ms: i64,
    expect: &Value,
    place: &str,
) {
    let file = store.read();
    let entry = file
        .get(harness, session)
        .unwrap_or_else(|| panic!("{place}: no entry for {session}"));
    let observation = entry.observation;
    let shown = observation.effective(at_ms, liveness(&expect["liveness"]));
    let state = expect["state"].as_str().expect("expected state");
    assert_eq!(
        Some(shown.state),
        LiveState::from_name(state),
        "{place}: state"
    );
    let detail = expect["detail"].as_str().map(|detail| {
        LiveDetail::from_name(detail).unwrap_or_else(|| panic!("{place}: no detail {detail}"))
    });
    assert_eq!(shown.detail, detail, "{place}: detail");
    let confidence = match expect["confidence"].as_str() {
        None | Some("fresh") => LiveConfidence::Fresh,
        Some("quiet") => LiveConfidence::Quiet,
        Some("stale") => LiveConfidence::Stale,
        Some(other) => panic!("{place}: unknown confidence {other}"),
    };
    assert_eq!(shown.confidence, confidence, "{place}: confidence");
    if let Some(since_ms) = expect["since_ms"].as_i64() {
        assert_eq!(observation.since_ms, since_ms, "{place}: since_ms");
    }
    let subagents = expect["subagents"].as_u64().unwrap_or(0);
    assert_eq!(
        u64::from(observation.subagents),
        subagents,
        "{place}: subagents"
    );
    let via_subagent = expect["via_subagent"].as_bool().unwrap_or(false);
    assert_eq!(
        observation.via_subagent, via_subagent,
        "{place}: via_subagent"
    );
}

/// Drive the sequence `name` of the corpus in `dir` through `adapter`'s
/// writer path and check every step.
pub(crate) fn run_sequence(adapter: &dyn HarnessAdapter, dir: &Path, name: &str) {
    let harness = adapter.harness_key();
    let scratch = tempfile::tempdir().expect("scratch state directory");
    let store = LiveStore::at(scratch.path());
    let (header, steps) = sequence(dir, name);
    let mut session: Option<String> = None;
    for (index, step) in steps.iter().enumerate() {
        let place = format!(
            "{name} step {} ({})",
            index + 1,
            step["note"].as_str().unwrap_or("")
        );
        let at_ms = step["at_ms"].as_i64().expect("at_ms");
        if let Some(payload) = step.get("payload") {
            let context = LiveContext {
                pid: step["pid"].as_i64().or_else(|| header["pid"].as_i64()),
                pid_start: step["pid_start"]
                    .as_u64()
                    .or_else(|| header["pid_start"].as_u64()),
                ..LiveContext::default()
            };
            let events = adapter.live_events_from_payload(payload, &context, at_ms);
            if !events.is_empty() {
                store
                    .apply_events(adapter.live_reducer(), &events)
                    .unwrap_or_else(|err| panic!("{place}: {err}"));
            }
            session = payload["session_id"].as_str().map(str::to_string);
        }
        let session = session
            .as_deref()
            .expect("a payload before the first check");
        let mut expect = step["expect"].clone();
        if let Some(liveness) = step.get("liveness") {
            expect["liveness"] = liveness.clone();
        }
        assert_shown(&store, harness, session, at_ms, &expect, &place);
        for other in step["also"].as_array().into_iter().flatten() {
            let id = other["session_id"].as_str().expect("session_id");
            assert_shown(&store, harness, id, at_ms, other, &format!("{place}, {id}"));
        }
    }
}
