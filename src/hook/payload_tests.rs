use serde_json::json;

use super::*;

const TASK: [PayloadField; 2] = [PayloadField::scalar("id"), PayloadField::scalar("status")];

const FIELDS: [PayloadField; 4] = [
    PayloadField::scalar("session_id"),
    PayloadField::scalar("flag"),
    PayloadField::nested("result", &TASK),
    PayloadField::nested("tasks", &TASK),
];

fn parsed(payload: &Value) -> Value {
    parse(payload.to_string().as_bytes(), &FIELDS).expect("valid JSON")
}

#[test]
fn only_the_named_fields_are_kept_at_every_depth() {
    let payload = json!({
        "session_id": "s1",
        "flag": true,
        "prompt": "never kept",
        "result": {"id": "a1", "status": "done", "content": "never kept"},
        "tasks": [
            {"id": "t1", "status": "running", "description": "never kept"},
            {"id": "t2", "command": "never kept"},
        ],
        "tool_input": {"command": "never kept", "nested": [{"deep": true}]},
    });
    assert_eq!(
        parsed(&payload),
        json!({
            "session_id": "s1",
            "flag": true,
            "result": {"id": "a1", "status": "done"},
            "tasks": [{"id": "t1", "status": "running"}, {"id": "t2"}],
        })
    );
}

#[test]
fn a_named_field_of_the_wrong_kind_reads_as_null() {
    let payload = json!({
        "session_id": {"not": "a string"},
        "flag": [true],
        "result": "a string where an object is read",
        "tasks": [7, "text", {"id": "t1"}, [{"id": "nested list"}]],
    });
    assert_eq!(
        parsed(&payload),
        json!({
            "session_id": null,
            "flag": null,
            "result": null,
            "tasks": [null, null, {"id": "t1"}, [{"id": "nested list"}]],
        })
    );
}

#[test]
fn scalars_keep_their_values() {
    let payload = json!({
        "session_id": "line\nbreak \"quoted\" é",
        "flag": -42,
        "result": {"id": 18_446_744_073_709_551_615_u64, "status": 1.5},
        "tasks": [{"id": null}],
    });
    assert_eq!(parsed(&payload), payload);
}

#[test]
fn an_escaped_key_still_names_its_field_and_the_last_duplicate_wins() {
    let input = br#"{"session_id": "first", "session_id": "second", "flag": false}"#;
    assert_eq!(
        parse(input, &FIELDS),
        Some(json!({"session_id": "second", "flag": false}))
    );
}

#[test]
fn a_payload_that_is_not_an_object_reads_as_null_or_a_list() {
    assert_eq!(parse(b"\"text\"", &FIELDS), Some(Value::Null));
    assert_eq!(parse(b"12", &FIELDS), Some(Value::Null));
    assert_eq!(parse(b"null", &FIELDS), Some(Value::Null));
    assert_eq!(
        parse(br#"[{"session_id": "s1", "other": 1}]"#, &FIELDS),
        Some(json!([{"session_id": "s1"}]))
    );
}

#[test]
fn what_a_whole_value_parse_refuses_is_refused() {
    for input in [
        &b""[..],
        b"{",
        b"{\"session_id\": \"s1\"} trailing",
        b"{\"prompt\": \"unterminated}",
        b"{\"prompt\": [1, 2}",
        b"\xff\xfe",
    ] {
        assert_eq!(
            parse(input, &FIELDS),
            None,
            "{}",
            String::from_utf8_lossy(input)
        );
        assert!(serde_json::from_slice::<Value>(input).is_err());
    }
}

#[test]
fn an_unknown_field_is_skipped_however_deep() {
    let deep = (0..100).fold(json!("bottom"), |inner, _| json!([inner]));
    let payload = json!({"session_id": "s1", "skipped": deep});
    assert_eq!(parsed(&payload), json!({"session_id": "s1"}));
}

#[test]
fn the_session_fields_are_what_the_default_record_reads() {
    let names: Vec<&str> = SESSION_FIELDS.iter().map(|field| field.name).collect();
    assert_eq!(
        names,
        ["session_id", "cwd", "transcript_path", "hook_event_name"]
    );
    assert!(SESSION_FIELDS.iter().all(|field| field.fields.is_empty()));
}
