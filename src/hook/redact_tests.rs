use serde_json::{Value, json};

use super::*;

const SALT: &str = "trace-a";

fn redacted(payload: &Value) -> Value {
    redact_payload(payload, SALT)
}

/// Every string in `value`, keys and values.
fn strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| strings(item, out)),
        Value::Object(map) => {
            for (key, item) in map {
                out.push(key.clone());
                strings(item, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[test]
fn numbers_booleans_and_nulls_are_kept() {
    let payload = json!({
        "count": 3,
        "ratio": 0.25,
        "negative": -7,
        "flag": true,
        "off": false,
        "nothing": null,
        "huge": 18446744073709551615u64,
    });
    assert_eq!(redacted(&payload), payload);
}

#[test]
fn a_string_is_its_length_in_characters() {
    let out = redacted(&json!({ "note": "hello", "wide": "h\u{e9}llo \u{1f600}", "empty": "" }));
    assert_eq!(out["note"], "<str:5>");
    assert_eq!(out["wide"], "<str:7>");
    assert_eq!(out["empty"], "<str:0>");
}

#[test]
fn a_payload_that_is_not_an_object_is_redacted_too() {
    assert_eq!(redacted(&json!("some prompt text")), "<str:16>");
    assert_eq!(redacted(&json!(12)), 12);
    assert_eq!(redacted(&json!(null)), json!(null));
    assert_eq!(
        redacted(&json!(["one", { "state": "ready" }])),
        json!(["<str:3>", { "state": "ready" }])
    );
}

#[test]
fn a_key_is_kept_only_in_the_shape_of_an_identifier() {
    let forty = "k".repeat(40);
    let forty_one = "k".repeat(41);
    let payload = json!({
        "plain_key": 1,
        "_underscore": 2,
        "Mixed9Case": 3,
        forty.clone(): 4,
        forty_one: 5,
        "9starts_with_digit": 6,
        "has space": 7,
        "has-dash": 8,
        "": 9,
        "caf\u{e9}": 10,
    });
    let out = redacted(&payload);
    let map = out.as_object().unwrap();
    for kept in ["plain_key", "_underscore", "Mixed9Case", forty.as_str()] {
        assert!(map.contains_key(kept), "{kept} was hidden: {out}");
    }
    // The six odd keys share one name, numbered so none is lost.
    let hidden = map.keys().filter(|key| key.starts_with("<key")).count();
    assert_eq!(hidden, 6, "{out}");
    assert!(
        map.contains_key("<key>") && map.contains_key("<key:6>"),
        "{out}"
    );
    assert_eq!(map.len(), 10, "{out}");
    // Hiding a key does not hide its number.
    let numbers: Vec<i64> = map
        .iter()
        .filter(|(key, _)| key.starts_with("<key"))
        .map(|(_, value)| value.as_i64().unwrap())
        .collect();
    assert_eq!(numbers.len(), 6);
    assert!(
        numbers.iter().all(|n| [5, 6, 7, 8, 9, 10].contains(n)),
        "{numbers:?}"
    );
}

#[test]
fn a_key_is_shown_to_depth_three_and_hidden_below() {
    let payload = json!({ "one": { "two": { "three": { "four": { "five": 5 } } } } });
    let out = redacted(&payload);
    assert_eq!(
        out,
        json!({ "one": { "two": { "three": { "<key>": { "<key>": 5 } } } } })
    );
}

#[test]
fn an_array_adds_no_depth() {
    let payload = json!({ "one": [ { "two": [ { "three": { "four": 1 } } ] } ] });
    let out = redacted(&payload);
    assert_eq!(
        out,
        json!({ "one": [ { "two": [ { "three": { "<key>": 1 } } ] } ] })
    );
}

#[test]
fn a_hidden_key_keeps_the_class_of_its_real_name() {
    let payload = json!({ "a": { "b": { "c": { "session_id": "raw-id", "cwd": "/work", "prompt": "text" } } } });
    let out = redacted(&payload);
    let mut found = Vec::new();
    strings(&out, &mut found);
    assert!(
        found
            .iter()
            .all(|text| !text.contains("raw-id") && !text.contains("/work")),
        "{out}"
    );
    let leaf = &out["a"]["b"]["c"];
    let mut values: Vec<&str> = leaf
        .as_object()
        .unwrap()
        .values()
        .map(|v| v.as_str().unwrap())
        .collect();
    values.sort_unstable();
    assert_eq!(
        values,
        ["<path>", "<str:4>", pseudonym(SALT, "raw-id").as_str()]
    );
    assert!(
        leaf.as_object()
            .unwrap()
            .keys()
            .all(|key| key.starts_with("<key"))
    );
}

#[test]
fn an_array_keeps_eight_elements_and_counts_the_rest() {
    let items: Vec<Value> = (0..11).map(|n| json!(format!("item {n}"))).collect();
    let out = redacted(&json!({ "list": items }));
    let list = out["list"].as_array().unwrap();
    assert_eq!(list.len(), 9, "{out}");
    assert!(list[..8].iter().all(|item| item == "<str:6>"), "{out}");
    assert_eq!(list[8], "<+3 more>");

    // Exactly eight is whole, with no marker.
    let eight: Vec<Value> = (0..8).map(|n| json!(n)).collect();
    let whole = redacted(&json!({ "list": eight }));
    assert_eq!(whole["list"].as_array().map(Vec::len), Some(8));
    assert_eq!(whole["list"][7], 7);
    // An empty one stays empty.
    assert_eq!(redacted(&json!({ "list": [] }))["list"], json!([]));
}

#[test]
fn an_array_element_is_redacted_by_its_arrays_key() {
    let out = redacted(&json!({
        "session_ids": ["raw-1", "raw-2"],
        "transcript_paths": ["/a", "/b"],
        "reason": ["clear", "a long sentence with spaces"],
        "notes": [{ "type": "text", "words": "some text" }],
    }));
    assert_eq!(
        out["session_ids"],
        json!([pseudonym(SALT, "raw-1"), pseudonym(SALT, "raw-2")])
    );
    assert_eq!(out["transcript_paths"], json!(["<path>", "<path>"]));
    assert_eq!(out["reason"], json!(["clear", "<str:27>"]));
    assert_eq!(
        out["notes"],
        json!([{ "type": "text", "words": "<str:9>" }])
    );
}

#[test]
fn an_object_keeps_sixty_four_entries_and_counts_the_rest() {
    let mut map = serde_json::Map::new();
    for n in 0..70 {
        map.insert(format!("key_{n:02}"), json!(n));
    }
    let out = redacted(&Value::Object(map));
    let out = out.as_object().unwrap();
    assert_eq!(out.len(), 65);
    assert_eq!(out["<more>"], 6);
}

#[test]
fn an_enum_key_keeps_a_word_and_not_a_sentence() {
    let out = redacted(&json!({
        "hook_event_name": "StopFailure",
        "source": "startup",
        "trigger": "auto",
        "reason": "prompt_input_exit",
        "notification_type": "permission_prompt",
        "permission_mode": "acceptEdits",
        "error": "rate_limit",
        "status": "async_launched",
        "state": "waiting",
        "detail": "permission",
        "kind": "form",
        "type": "subagent",
        "stop_reason": "end_turn",
        "error_class": "rate_limit",
        "mode": "tui",
        "level": "high",
        "behavior": "allow",
        "extension_version": "0.2.0",
        "version": "2.1.322",
        "harnessVersion": "1.18.32",
        "hookEventName": "session.status",
    }));
    let map = out.as_object().unwrap();
    assert_eq!(map["hook_event_name"], "StopFailure");
    assert_eq!(map["error"], "rate_limit");
    assert_eq!(map["reason"], "prompt_input_exit");
    assert_eq!(map["extension_version"], "0.2.0");
    assert_eq!(map["harnessVersion"], "1.18.32");
    assert_eq!(map["hookEventName"], "session.status");
    for (key, value) in map {
        assert!(
            !value.as_str().unwrap().starts_with('<'),
            "{key} was redacted"
        );
    }
}

#[test]
fn a_key_that_only_ends_like_a_version_is_not_one() {
    let out = redacted(&json!({ "conversion": "some_word", "inversion": "x", "versions": "y" }));
    assert_eq!(
        out,
        json!({ "conversion": "<str:9>", "inversion": "<str:1>", "versions": "<str:1>" })
    );
}

#[test]
fn error_is_a_word_only_in_a_stop_failure() {
    let failure = redacted(&json!({ "hook_event_name": "StopFailure", "error": "rate_limit" }));
    assert_eq!(failure["error"], "rate_limit");
    // A failed tool's error is its text, even when it is one word.
    for event in ["PostToolUseFailure", "PostModelRequest", "SomeFutureEvent"] {
        let out = redacted(&json!({ "hook_event_name": event, "error": "SENTINEL-oneword" }));
        assert_eq!(out["error"], "<str:16>", "{event}");
        assert_eq!(out["hook_event_name"], event);
    }
    assert_eq!(redacted(&json!({ "error": "timeout" }))["error"], "<str:7>");
    // Nested, the event name is the one beside the field.
    let nested = redacted(&json!({
        "hook_event_name": "StopFailure",
        "detail": { "error": "overloaded" },
    }));
    assert_eq!(nested["detail"]["error"], "<str:10>");
}

#[test]
fn an_enum_key_redacts_a_value_that_is_not_word_shaped() {
    let sentence = "stream disconnected before completion";
    let forty_eight = "w".repeat(48);
    let forty_nine = "w".repeat(49);
    let out = redacted(&json!({
        "hook_event_name": "StopFailure",
        "error": sentence,
        "status": forty_nine,
        "state": forty_eight,
        "reason": "",
        "detail": "has/slash",
        "kind": "caf\u{e9}",
        "type": "two words",
        "source": "multi\nline",
    }));
    assert_eq!(out["error"], "<str:37>");
    assert_eq!(out["status"], "<str:49>");
    assert_eq!(out["state"], json!(forty_eight));
    assert_eq!(out["reason"], "<str:0>");
    assert_eq!(out["detail"], "<str:9>");
    assert_eq!(out["kind"], "<str:4>");
    assert_eq!(out["type"], "<str:9>");
    assert_eq!(out["source"], "<str:10>");
}

#[test]
fn a_word_under_a_key_outside_the_vocabulary_is_a_placeholder() {
    let out = redacted(
        &json!({ "model": "gpt-5.4", "agent_type": "Explore", "mcp_server": { "name": "tracker" } }),
    );
    assert_eq!(out["model"], "<str:7>");
    assert_eq!(out["agent_type"], "<str:7>");
    assert_eq!(out["mcp_server"]["name"], "<str:7>");
}

#[test]
fn a_tool_name_is_kept_unless_it_is_an_mcp_tool() {
    let out = redacted(&json!({ "tool_name": "Bash" }));
    assert_eq!(out["tool_name"], "Bash");
    let out = redacted(&json!({ "tool_name": "mcp__tracker__create_issue" }));
    assert_eq!(out["tool_name"], "mcp__<redacted>");
    let out = redacted(&json!({ "tool_name": "mcp__" }));
    assert_eq!(out["tool_name"], "mcp__<redacted>");
    // Only the prefix makes an MCP tool: a similar name is just a name.
    assert_eq!(
        redacted(&json!({ "tool_name": "mcp_x" }))["tool_name"],
        "mcp_x"
    );
    assert_eq!(
        redacted(&json!({ "tool_name": "Bash(rm -rf /)" }))["tool_name"],
        "<str:14>"
    );
    // Only `tool_name` is a tool name: another spelling is a string like
    // any other.
    assert_eq!(
        redacted(&json!({ "toolName": "bash" }))["toolName"],
        "<str:4>"
    );
    assert_eq!(
        redacted(&json!({ "toolName": "mcp__docs__search" }))["toolName"],
        "<str:17>"
    );
}

#[test]
fn an_identifier_becomes_a_pseudonym() {
    let keys = [
        "session_id",
        "agent_id",
        "tool_use_id",
        "prompt_id",
        "turn_id",
        "request_id",
        "instance",
        "id",
        "agentId",
        "sessionID",
        "parentSessionId",
        "tool_use_ids",
        "uuid",
    ];
    let payload: Value = keys
        .iter()
        .map(|key| ((*key).to_string(), json!(format!("raw-{key}"))))
        .collect::<serde_json::Map<_, _>>()
        .into();
    let out = redacted(&payload);
    for key in keys {
        let pseudonym = out[key].as_str().unwrap();
        assert_eq!(pseudonym, super::pseudonym(SALT, &format!("raw-{key}")));
        assert!(
            pseudonym.starts_with("id:") && pseudonym.len() == 11,
            "{pseudonym}"
        );
        assert!(
            pseudonym[3..]
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "{pseudonym}"
        );
    }
}

#[test]
fn a_pseudonym_is_the_documented_hash() {
    // The one-liner the operator guide gives:
    // printf '%s\0%s' NAME VALUE | sha256sum | cut -c1-8
    assert_eq!(pseudonym("trace-a", "session-1"), "id:d5496ddf");
    assert_eq!(pseudonym("trace-b", "session-1"), "id:7652f7e4");
    assert_eq!(pseudonym("trace-a", "session-2"), "id:5a2e8e05");
    // The NUL keeps a salt and a value from running together.
    assert_eq!(pseudonym("a", "bc"), "id:40bb547d");
    assert_eq!(pseudonym("ab", "c"), "id:6c032e63");
}

#[test]
fn a_pseudonym_is_stable_within_a_trace_and_differs_across_traces() {
    let payload = json!({ "session_id": "3f2a9c1e-7b4d-4e8a-9c6f-1a2b3c4d5e6f", "agent_id": "a1" });
    let first = redact_payload(&payload, "trace-a");
    let again = redact_payload(&payload, "trace-a");
    let other = redact_payload(&payload, "trace-b");
    assert_eq!(first, again);
    assert_ne!(first["session_id"], other["session_id"]);
    assert_ne!(first["agent_id"], other["agent_id"]);
    // Two values in one trace have two pseudonyms; one value, one.
    assert_ne!(first["session_id"], first["agent_id"]);
    let twice = redact_payload(
        &json!({ "a": { "session_id": "x" }, "b": [{ "session_id": "x" }] }),
        "t",
    );
    assert_eq!(twice["a"]["session_id"], twice["b"][0]["session_id"]);
}

#[test]
fn an_identifier_that_is_a_number_is_a_number() {
    assert_eq!(
        redacted(&json!({ "session_id": 17, "pid": 4242 })),
        json!({ "session_id": 17, "pid": 4242 })
    );
}

#[test]
fn a_path_becomes_a_marker() {
    let out = redacted(&json!({
        "cwd": "/home/user/project",
        "transcript_path": "/home/user/.claude/projects/x/y.jsonl",
        "agent_transcript_path": "/tmp/agent.jsonl",
        "file_path": "/etc/passwd",
        "filePath": "/etc/hosts",
        "project_dir": "/work",
        "workdir": "/work",
        "directory": "/work",
        "path": "relative/path",
        "extra_paths": ["/a", "/b"],
        "transcript_path_missing": "stays a placeholder",
    }));
    let map = out.as_object().unwrap();
    for key in [
        "cwd",
        "transcript_path",
        "agent_transcript_path",
        "file_path",
        "filePath",
        "project_dir",
        "workdir",
        "directory",
        "path",
    ] {
        assert_eq!(map[key], "<path>", "{key}");
    }
    assert_eq!(map["extra_paths"], json!(["<path>", "<path>"]));
    assert_eq!(map["transcript_path_missing"], "<str:19>");
    assert_eq!(
        redacted(&json!({ "transcript_path": null }))["transcript_path"],
        json!(null)
    );
}

#[test]
fn the_value_of_a_payload_key_is_replaced_by_its_size() {
    let out = redacted(&json!({
        "prompt": "write the parser",
        "tool_input": { "command": "rm -rf target", "description": "clean" },
        "tool_response": { "stdout": "ok", "stderr": "" },
        "message": "heads up",
        "last_assistant_message": "Done: the parser is split.",
        "error_details": "boom",
        "compact_summary": "the user asked",
        "custom_instructions": "be brief",
        "title": "Parser work",
        "session_title": "Parser work",
        "permission_suggestions": [{ "type": "addRules", "rules": [] }, { "type": "setMode" }],
        "session_crons": [{ "id": "c1", "prompt": "check CI" }],
        "background_tasks": [{ "id": "t1", "type": "shell", "status": "running", "description": "Watch", "command": "cargo watch" }],
        "questions": [{ "question": "which?", "options": ["a", "b"] }],
        "answers": { "Which parser should change": "legacy" },
        "systemPrompt": "you are",
        "toolInput": { "x": 1 },
    }));
    assert_eq!(out["prompt"], "<str:16>");
    assert_eq!(out["tool_input"], "<object:2 keys>");
    assert_eq!(out["tool_response"], "<object:2 keys>");
    assert_eq!(out["message"], "<str:8>");
    assert_eq!(out["last_assistant_message"], "<str:26>");
    assert_eq!(out["error_details"], "<str:4>");
    assert_eq!(out["compact_summary"], "<str:14>");
    assert_eq!(out["custom_instructions"], "<str:8>");
    assert_eq!(out["title"], "<str:11>");
    assert_eq!(out["session_title"], "<str:11>");
    assert_eq!(out["permission_suggestions"], "<array:2>");
    assert_eq!(out["session_crons"], "<array:1>");
    assert_eq!(out["questions"], "<array:1>");
    assert_eq!(out["answers"], "<object:1 keys>");
    assert_eq!(out["systemPrompt"], "<str:7>");
    assert_eq!(out["toolInput"], "<object:1 keys>");
    // A background task keeps its id, type, and status, and nothing else.
    let task = &out["background_tasks"][0];
    assert_eq!(task["id"], pseudonym(SALT, "t1"));
    assert_eq!(task["type"], "shell");
    assert_eq!(task["status"], "running");
    assert_eq!(task["description"], "<str:5>");
    assert_eq!(task["command"], "<str:11>");
}

#[test]
fn a_number_or_flag_under_a_payload_key_is_kept() {
    let out = redacted(&json!({ "prompt": 3, "message": true, "content": null }));
    assert_eq!(
        out,
        json!({ "prompt": 3, "message": true, "content": null })
    );
}

#[test]
fn the_agent_tools_result_keeps_its_status_and_agent_id() {
    for tool in ["Agent", "Task"] {
        let out = redacted(&json!({
            "tool_name": tool,
            "tool_response": {
                "status": "completed",
                "agentId": "a3f9c27e1b4d5068",
                "content": [{ "type": "text", "text": "The parser has four modules." }],
                "totalTokens": 18342,
            },
        }));
        assert_eq!(
            out["tool_response"],
            json!({
                "status": "completed",
                "agentId": pseudonym(SALT, "a3f9c27e1b4d5068"),
                "<other>": "<object:2 keys>",
            }),
            "{tool}"
        );
        assert_eq!(out["tool_name"], tool);
    }
}

#[test]
fn the_agent_tools_result_redacts_a_status_that_is_text() {
    let out = redacted(&json!({
        "tool_name": "Agent",
        "tool_response": { "status": "this is a sentence", "agentId": 5 },
    }));
    assert_eq!(
        out["tool_response"],
        json!({ "status": "<str:18>", "agentId": 5 })
    );
    // A result that is not an object has only its size.
    let out = redacted(&json!({ "tool_name": "Agent", "tool_response": "text of the result" }));
    assert_eq!(out["tool_response"], "<str:18>");
    // With nothing else in it there is no `<other>`.
    let out =
        redacted(&json!({ "tool_name": "Agent", "tool_response": { "status": "async_launched" } }));
    assert_eq!(out["tool_response"], json!({ "status": "async_launched" }));
}

#[test]
fn another_tools_result_keeps_nothing() {
    for tool in ["Bash", "Read", "mcp__tracker__get", "agent", "AgentX"] {
        let out = redacted(&json!({
            "tool_name": tool,
            "tool_response": { "status": "completed", "agentId": "a1", "stdout": "x" },
        }));
        assert_eq!(out["tool_response"], "<object:3 keys>", "{tool}");
    }
    // The tool's name must sit beside the result, not above it.
    let out = redacted(&json!({
        "event": { "tool_name": "Agent" },
        "tool_response": { "status": "completed", "agentId": "a1" },
    }));
    assert_eq!(out["tool_response"], "<object:2 keys>");
}

#[test]
fn a_field_no_one_has_seen_leaks_no_text() {
    let out = redacted(&json!({
        "session_id": "s-1",
        "future_field": "SENTINEL-future-text",
        "future_object": {
            "inner": "SENTINEL-inner-text",
            "list": ["SENTINEL-in-list", { "deep": "SENTINEL-deep-text" }],
            "SENTINEL-odd key": "SENTINEL-odd-value",
        },
        "SENTINEL future key with spaces": "SENTINEL-value",
        "a": { "b": { "c": { "d": "SENTINEL-too-deep" } } },
    }));
    let mut found = Vec::new();
    strings(&out, &mut found);
    assert!(
        found.iter().all(|text| !text.contains("SENTINEL")),
        "text leaked: {out}"
    );
    // The shape is there.
    assert_eq!(out["future_field"], "<str:20>");
    assert_eq!(out["future_object"]["inner"], "<str:19>");
    assert_eq!(out["future_object"]["list"][1]["deep"], "<str:18>");
}

#[test]
fn an_unknown_key_in_the_right_shape_survives_and_its_text_does_not() {
    // The one channel left: a key shaped like an identifier.
    let out = redacted(&json!({ "Which_parser_should_change": "legacy" }));
    assert_eq!(out, json!({ "Which_parser_should_change": "<str:6>" }));
}

#[test]
fn only_vocabulary_ids_numbers_and_structure_reach_the_output() {
    let payload = json!({
        "session_id": "3f2a9c1e-7b4d-4e8a-9c6f-1a2b3c4d5e6f",
        "transcript_path": "/home/user/.claude/projects/x/3f2a.jsonl",
        "cwd": "/home/user/projects/my-project",
        "permission_mode": "default",
        "hook_event_name": "PostToolUse",
        "tool_name": "Agent",
        "tool_input": { "description": "Survey the parser", "prompt": "List every module" },
        "tool_response": { "status": "completed", "agentId": "a3f9", "content": [{ "text": "four modules" }] },
        "tool_use_id": "toolu_01Agent",
        "duration_ms": 8123,
    });
    let out = redacted(&payload);
    let mut found = Vec::new();
    strings(&out, &mut found);
    let allowed = |text: &str| {
        text.starts_with('<')
            || text.starts_with("id:")
            || matches!(
                text,
                "session_id"
                    | "transcript_path"
                    | "cwd"
                    | "permission_mode"
                    | "default"
                    | "hook_event_name"
                    | "PostToolUse"
                    | "tool_name"
                    | "Agent"
                    | "tool_input"
                    | "tool_response"
                    | "status"
                    | "completed"
                    | "agentId"
                    | "tool_use_id"
                    | "duration_ms"
            )
    };
    let stray: Vec<&String> = found.iter().filter(|text| !allowed(text)).collect();
    assert!(stray.is_empty(), "{stray:?} in {out}");
}
