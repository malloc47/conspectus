//! `hook write` hands each adapter the payload parsed against
//! [`HarnessAdapter::hook_payload_fields`], not the whole payload. Every
//! adapter must make the same of both: the same attribution decision,
//! record, live events, and snapshot. Checked over every payload of the
//! fixture corpora, and over each single payload with every value in it,
//! at every depth, swapped for one of each kind or removed, so a field an
//! adapter reads but does not name, or names at the wrong depth, fails
//! here.

use std::path::Path;

use serde_json::{Value, json};

use super::live_sequences::{corpus_dir, fixture_names, read_json, sequence};
use super::{HarnessAdapter, registered_adapters};
use crate::live_store::LiveContext;

fn adapter(key: &str) -> &'static dyn HarnessAdapter {
    registered_adapters()
        .find(|adapter| adapter.harness_key() == key)
        .unwrap_or_else(|| panic!("no adapter {key}"))
}

/// What `adapter` makes of `payload`, as text to compare.
fn mapped(adapter: &dyn HarnessAdapter, payload: &Value) -> String {
    let context = LiveContext {
        pid: Some(4242),
        pid_start: Some(77),
        cwd: Some("/writer/cwd".to_string()),
        ..LiveContext::default()
    };
    let record = adapter
        .hook_record_from_payload(payload, Some(4242), Some(1), None, None, 1_800_000_000)
        .map_err(|err| err.to_string());
    format!(
        "attribution: {:?}\nrecord: {record:?}\nevents: {:?}\nsnapshot: {:?}",
        adapter.is_attribution_event(payload),
        adapter.live_events_from_payload(payload, &context, 1_800_000_000_000),
        adapter.live_snapshot_from_payload(payload, &context, 1_800_000_000_000),
    )
}

fn assert_same(adapter: &dyn HarnessAdapter, payload: &Value, place: &str) {
    let input = payload.to_string();
    let parsed = crate::hook::payload::parse(input.as_bytes(), adapter.hook_payload_fields())
        .expect("a valid payload parses");
    assert_eq!(
        mapped(adapter, &parsed),
        mapped(adapter, payload),
        "{place}: {input}"
    );
}

/// One value of each kind, to put where another was.
fn kinds() -> Vec<Value> {
    vec![
        json!({"status": "completed", "id": "x-1"}),
        json!([{"id": "x-1", "type": "subagent", "status": "running"}, 7]),
        json!("SessionStart"),
        json!(""),
        json!(7),
        json!(-1),
        json!(true),
        json!(null),
    ]
}

/// `value` with one point changed: itself swapped for each of
/// [`kinds`], and, `depth` levels down, each field removed or a value
/// inside it changed the same way.
fn mutations(value: &Value, depth: usize) -> Vec<Value> {
    let mut out = kinds();
    if depth == 0 {
        return out;
    }
    match value {
        Value::Object(fields) => {
            for (key, inner) in fields {
                let mut removed = fields.clone();
                removed.remove(key);
                out.push(Value::Object(removed));
                for changed in mutations(inner, depth - 1) {
                    let mut copy = fields.clone();
                    copy.insert(key.clone(), changed);
                    out.push(Value::Object(copy));
                }
            }
        }
        Value::Array(items) => {
            for (index, inner) in items.iter().enumerate() {
                for changed in mutations(inner, depth - 1) {
                    let mut copy = items.clone();
                    copy[index] = changed;
                    out.push(Value::Array(copy));
                }
            }
        }
        _ => {}
    }
    out
}

/// The single payloads of an event-source corpus.
fn single_payloads(dir: &Path) -> Vec<(String, Value)> {
    fixture_names(dir, "json")
        .into_iter()
        .map(|name| {
            let payload = read_json(&dir.join(format!("{name}.json")));
            (name, payload)
        })
        .collect()
}

/// The payloads of every sequence or script of a corpus: each step's
/// `payload`, and each payload a plugin script's step `emit`s.
fn sequence_payloads(dir: &Path) -> Vec<(String, Value)> {
    fixture_names(dir, "jsonl")
        .into_iter()
        .flat_map(|name| {
            let (_, steps) = sequence(dir, &name);
            steps
                .into_iter()
                .enumerate()
                .flat_map(move |(index, step)| {
                    let place = format!("{name} step {}", index + 1);
                    step.get("payload")
                        .cloned()
                        .into_iter()
                        .chain(step["emit"].as_array().cloned().unwrap_or_default())
                        .map(move |payload| (place.clone(), payload))
                })
        })
        .collect()
}

fn check_corpus(harness: &str, corpus: &str) -> usize {
    let adapter = adapter(harness);
    let dir = corpus_dir(corpus);
    let mut checked = 0;
    for (place, payload) in single_payloads(&dir) {
        for (index, variant) in mutations(&payload, 4).iter().enumerate() {
            assert_same(adapter, variant, &format!("{place} variant {index}"));
            checked += 1;
        }
    }
    for (place, payload) in sequence_payloads(&dir) {
        for (index, variant) in std::iter::once(payload.clone())
            .chain(mutations(&payload, 3))
            .enumerate()
        {
            assert_same(adapter, &variant, &format!("{place} variant {index}"));
            checked += 1;
        }
    }
    checked
}

#[test]
fn claude_code_maps_its_whitelisted_payloads_as_whole_ones() {
    assert!(check_corpus("claude-code", "claude-code") > 1_000);
}

#[test]
fn codex_maps_its_whitelisted_payloads_as_whole_ones() {
    assert!(check_corpus("codex", "codex") > 1_000);
}

#[test]
fn opencode_maps_its_whitelisted_payloads_as_whole_ones() {
    assert!(check_corpus("opencode", "opencode-live") > 1_000);
}

#[test]
fn pi_maps_its_whitelisted_payloads_as_whole_ones() {
    assert!(check_corpus("pi", "pi-live") > 1_000);
}

#[test]
fn every_adapter_maps_a_session_start_alike_both_ways() {
    // Aider and any adapter without a corpus reads the default fields.
    let payload = json!({
        "session_id": "0199f0a2-1111-7000-8000-000000000001",
        "cwd": "/work/project",
        "transcript_path": "/work/project/session.jsonl",
        "hook_event_name": "SessionStart",
        "prompt": "never read",
    });
    for adapter in registered_adapters() {
        for variant in mutations(&payload, 1) {
            assert_same(adapter, &variant, adapter.harness_key());
        }
    }
}
