//! Splicing a changed value into a JSON document's text: what is edited,
//! how new text is laid out, and that nothing else moves.

use serde_json::{Map, Value, json};

use super::*;

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|err| panic!("{err}: {text:?}"))
}

/// `text` spliced to hold its value after `change`.
fn edited(text: &str, change: impl FnOnce(&mut Value)) -> String {
    let mut value = parse(text);
    change(&mut value);
    let out = splice(text, &value).unwrap_or_else(|err| panic!("{err:#}: {text:?}"));
    assert_eq!(parse(&out), value, "{out:?}");
    out
}

fn object(value: &mut Value) -> &mut Map<String, Value> {
    value.as_object_mut().expect("an object")
}

#[test]
fn an_unchanged_value_leaves_the_text_alone() {
    let text = "  {\"b\":1e1, \"a\" : [ \"caf\\u00e9\" ],\"c\":{}}\n\n";
    assert_eq!(splice(text, &parse(text)).expect("splice"), text);
}

#[test]
fn a_new_member_follows_the_layout_of_the_ones_there() {
    for (text, expected) in [
        (
            "{\n  \"b\": 1,\n  \"a\": 2\n}\n",
            "{\n  \"b\": 1,\n  \"a\": 2,\n  \"z\": {\n    \"k\": [\n      1\n    ]\n  }\n}\n",
        ),
        (
            "{\n    \"b\": 1\n}",
            "{\n    \"b\": 1,\n    \"z\": {\n        \"k\": [\n            1\n        ]\n    }\n}",
        ),
        (
            "{\n\t\"b\": 1\n}\n",
            "{\n\t\"b\": 1,\n\t\"z\": {\n\t\t\"k\": [\n\t\t\t1\n\t\t]\n\t}\n}\n",
        ),
        (
            "{\r\n  \"b\": 1\r\n}\r\n",
            "{\r\n  \"b\": 1,\r\n  \"z\": {\r\n    \"k\": [\r\n      1\r\n    ]\r\n  }\r\n}\r\n",
        ),
        ("{\"b\":1}", "{\"b\":1,\"z\":{\"k\":[1]}}"),
        ("{\"b\": 1}", "{\"b\": 1, \"z\": {\"k\": [1]}}"),
        (
            "{ \"b\": 1, \"a\": 2 }",
            "{ \"b\": 1, \"a\": 2, \"z\": {\"k\": [1]} }",
        ),
        ("{}", "{\n  \"z\": {\n    \"k\": [\n      1\n    ]\n  }\n}"),
        (
            // Items on lines of their own but each kept on one line.
            "{\n  \"b\": {\"x\": 1},\n  \"a\": {\"y\": 2}\n}",
            "{\n  \"b\": {\"x\": 1},\n  \"a\": {\"y\": 2},\n  \"z\": {\"k\": [1]}\n}",
        ),
        (
            // One of them kept on one line is not enough.
            "{\n  \"b\": {\"x\": 1},\n  \"a\": {\n    \"y\": 2\n  }\n}",
            "{\n  \"b\": {\"x\": 1},\n  \"a\": {\n    \"y\": 2\n  },\n  \"z\": {\n    \"k\": [\n      1\n    ]\n  }\n}",
        ),
    ] {
        let out = edited(text, |value| {
            object(value).insert("z".to_string(), json!({ "k": [1] }));
        });
        assert_eq!(out, expected, "{text:?}");
    }
}

#[test]
fn cutting_an_item_takes_one_separator_with_it() {
    let text = r#"{"a": 1, "b": 2, "c": 3}"#;
    for (gone, expected) in [
        (&["a"][..], r#"{"b": 2, "c": 3}"#),
        (&["b"], r#"{"a": 1, "c": 3}"#),
        (&["c"], r#"{"a": 1, "b": 2}"#),
        (&["a", "b"], r#"{"c": 3}"#),
        (&["b", "c"], r#"{"a": 1}"#),
        (&["a", "c"], r#"{"b": 2}"#),
        (&["a", "b", "c"], "{}"),
    ] {
        let out = edited(text, |value| {
            for key in gone {
                object(value).remove(*key);
            }
        });
        assert_eq!(out, expected, "{gone:?}");
    }

    let lines = "{\n  \"a\": [\n    1,\n    2,\n    3\n  ],\n  \"b\": 2\n}\n";
    assert_eq!(
        edited(lines, |value| {
            object(value).remove("b");
        }),
        "{\n  \"a\": [\n    1,\n    2,\n    3\n  ]\n}\n"
    );
    assert_eq!(
        edited(lines, |value| value["a"] = json!([1, 3])),
        "{\n  \"a\": [\n    1,\n    3\n  ],\n  \"b\": 2\n}\n"
    );
    assert_eq!(
        edited(lines, |value| value["a"] = json!([])),
        "{\n  \"a\": [],\n  \"b\": 2\n}\n"
    );
}

#[test]
fn appending_and_cutting_again_gives_the_bytes_back() {
    let hooks = json!({
        "Stop": [{ "hooks": [{ "type": "command", "command": "x", "timeout": 5 }] }],
        "SessionStart": [{ "matcher": "startup", "hooks": [] }]
    });
    for text in [
        "{}",
        "{}\n",
        "{\"theme\":\"dark\"}",
        "{\n    \"z\": \"caf\\u00e9 ☕\",\n    \"a\": [ 1e1, -0.50 ]\n}",
        "{\r\n\t\"theme\": \"dark\",\r\n\t\"env\": {}\r\n}\r\n",
        "\n  { \"only\" : true }  \n",
    ] {
        let mut installed = parse(text);
        object(&mut installed).insert("hooks".to_string(), hooks.clone());
        let with_hooks = splice(text, &installed).expect("append");
        assert!(with_hooks.contains("\"Stop\""), "{with_hooks:?}");
        let back = splice(&with_hooks, &parse(text)).expect("cut");
        assert_eq!(back, text);

        // The same holds one level down.
        let mut nested = parse(&with_hooks);
        nested["hooks"]["Stop"]
            .as_array_mut()
            .expect("list")
            .push(json!({ "hooks": [] }));
        object(&mut nested["hooks"]).insert("PostCompact".to_string(), json!([]));
        let deeper = splice(&with_hooks, &nested).expect("append inside");
        assert_eq!(splice(&deeper, &installed).expect("cut inside"), with_hooks);
    }
}

#[test]
fn array_elements_are_matched_by_value_then_by_likeness() {
    let mixed = r#"{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard"}, {"type": "command", "command": "csp"}]}"#;
    let own = r#"{"hooks": [{"type": "command", "command": "csp"}]}"#;
    let without =
        json!({ "matcher": "Bash", "hooks": [{ "type": "command", "command": "guard" }] });
    let kept = r#"{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard"}]}"#;
    for (elements, expected) in [
        (
            format!("{mixed},\n    {own}"),
            format!("{{\"Stop\": [\n    {kept}\n]}}"),
        ),
        (
            format!("{own},\n    {mixed}"),
            format!("{{\"Stop\": [\n    {kept}\n]}}"),
        ),
        (
            format!("{own},\n    {mixed},\n    {own}"),
            format!("{{\"Stop\": [\n    {kept}\n]}}"),
        ),
    ] {
        let text = format!("{{\"Stop\": [\n    {elements}\n]}}");
        let out = edited(&text, |value| value["Stop"] = json!([without]));
        assert_eq!(out, expected, "{text}");
    }
}

#[test]
fn a_changed_value_is_replaced_where_it_stands() {
    let text = "{\"m\": \"old\",  \"n\": 1e1, \"o\": {\"p\": [1,2]}}";
    assert_eq!(
        edited(text, |value| value["m"] = json!("new")),
        "{\"m\": \"new\",  \"n\": 1e1, \"o\": {\"p\": [1,2]}}"
    );
    // A type change rewrites the value in its old layout.
    assert_eq!(
        edited(text, |value| value["m"] = json!({ "a": [true] })),
        "{\"m\": {\"a\": [true]},  \"n\": 1e1, \"o\": {\"p\": [1,2]}}"
    );
    assert_eq!(
        edited(text, |value| value["o"] = json!("flat")),
        "{\"m\": \"old\",  \"n\": 1e1, \"o\": \"flat\"}"
    );
    // A one-line value that shows no spacing of its own takes its
    // container's.
    assert_eq!(
        edited(r#"{"a": [1], "b": 2}"#, |value| value["a"] =
            json!({ "k": [1, 2] })),
        r#"{"a": {"k": [1, 2]}, "b": 2}"#
    );
    assert_eq!(
        edited(r#"{"a":[1],"b":2}"#, |value| value["a"] =
            json!({ "k": [1, 2] })),
        r#"{"a":{"k":[1,2]},"b":2}"#
    );
}

#[test]
fn an_empty_container_opens_one_level_deeper_than_its_line() {
    let text = "{\n    \"hooks\": {},\n    \"b\": []\n}";
    assert_eq!(
        edited(text, |value| value["hooks"] = json!({ "Stop": [1] })),
        "{\n    \"hooks\": {\n        \"Stop\": [\n            1\n        ]\n    },\n    \"b\": []\n}"
    );
    assert_eq!(
        edited(text, |value| value["b"] = json!(["x"])),
        "{\n    \"hooks\": {},\n    \"b\": [\n        \"x\"\n    ]\n}"
    );
    assert_eq!(
        edited(r#"{"hooks": {}, "b": 1}"#, |value| value["hooks"] =
            json!({ "Stop": [1] })),
        r#"{"hooks": {"Stop": [1]}, "b": 1}"#
    );
    assert_eq!(
        edited(r#"{"hooks":{},"b":1}"#, |value| value["hooks"] =
            json!({ "Stop": [1] })),
        r#"{"hooks":{"Stop":[1]},"b":1}"#
    );
}

#[test]
fn a_new_element_before_one_that_stays_rewrites_the_array() {
    assert_eq!(
        edited(r#"{"a": [1, 2], "b": "x"}"#, |value| value["a"] =
            json!([0, 1, 2])),
        r#"{"a": [0, 1, 2], "b": "x"}"#
    );
    assert_eq!(
        edited("{\"a\": [\n  1,\n  2\n]}", |value| value["a"] =
            json!([1, 9, 2])),
        "{\"a\": [\n  1,\n  9,\n  2\n]}"
    );
}

#[test]
fn escapes_numbers_and_unicode_outside_the_edit_stay_as_written() {
    let text = "{\n  \"s\": \"caf\\u00e9 ☕ \\/\",\n  \"n\": 1.50,\n  \"e\": 1E+2,\n  \"\\u0068ooks\": {}\n}";
    let out = edited(text, |value| value["hooks"] = json!({ "Stop": [] }));
    assert_eq!(
        out,
        "{\n  \"s\": \"caf\\u00e9 ☕ \\/\",\n  \"n\": 1.50,\n  \"e\": 1E+2,\n  \"\\u0068ooks\": {\n    \"Stop\": []\n  }\n}"
    );
}

#[test]
fn text_that_is_not_strict_json_or_holds_a_key_twice_is_refused() {
    for text in [
        "{\"a\": 1,}",
        "// settings\n{\"a\": 1}",
        "\u{feff}{\"a\": 1}",
        "",
    ] {
        assert!(splice(text, &json!({ "a": 2 })).is_err(), "{text:?}");
    }
    let twice = r#"{"a": 1, "a": 2, "b": 3}"#;
    let err = splice(twice, &json!({ "a": 2 })).expect_err("a key twice");
    assert!(format!("{err:#}").contains("appears twice"), "{err:#}");
    // A key twice in an object the edit does not touch is no matter.
    let elsewhere = r#"{"x": {"k": 1, "k": 2}, "b": 3}"#;
    assert_eq!(
        splice(elsewhere, &json!({ "x": { "k": 2 }, "b": 4 })).expect("splice"),
        r#"{"x": {"k": 1, "k": 2}, "b": 4}"#
    );
}

/// A small deterministic generator, so a failure names its seed.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % bound as u64) as usize
    }
}

/// Keys as written, each decoding to a different key.
const KEYS: &[&str] = &[
    "\"a\"",
    "\"b\"",
    "\"hooks\"",
    "\"caf\\u00e9\"",
    "\"☕\"",
    "\"z z\"",
    "\"Stop\"",
    "\"m\\\"q\"",
];

const SCALARS: &[&str] = &[
    "1",
    "-2.50",
    "1e3",
    "true",
    "null",
    "\"x\"",
    "\"caf\\u00e9\"",
    "\"☕\"",
    "\"\"",
];

/// A random document written with a random layout per container, in one
/// indentation unit and line ending.
fn random_text(rng: &mut Lcg, unit: &str, newline: &str) -> String {
    fn write(rng: &mut Lcg, depth: usize, indent: &str, unit: &str, nl: &str, out: &mut String) {
        let kind = if depth == 0 { 0 } else { rng.below(4) };
        if depth >= 3 || kind >= 2 {
            out.push_str(SCALARS[rng.below(SCALARS.len())]);
            return;
        }
        let object = kind == 0;
        let mut keys: Vec<&str> = KEYS.to_vec();
        for i in (1..keys.len()).rev() {
            keys.swap(i, rng.below(i + 1));
        }
        let count = rng.below(4);
        let pretty = rng.below(3);
        let inner = format!("{indent}{unit}");
        out.push(if object { '{' } else { '[' });
        for (index, key) in keys.iter().take(count).enumerate() {
            if index > 0 {
                out.push(',');
                if pretty == 1 {
                    out.push(' ');
                }
            }
            if pretty == 0 {
                out.push_str(nl);
                out.push_str(&inner);
            }
            if object {
                out.push_str(key);
                out.push_str(if pretty == 2 { ":" } else { ": " });
            }
            write(rng, depth + 1, &inner, unit, nl, out);
        }
        if pretty == 0 && count > 0 {
            out.push_str(nl);
            out.push_str(indent);
        }
        out.push(if object { '}' } else { ']' });
    }
    let mut out = String::new();
    write(rng, 0, "", unit, newline, &mut out);
    if rng.below(2) == 0 {
        out.push_str(newline);
    }
    out
}

fn random_scalar(rng: &mut Lcg) -> Value {
    parse(SCALARS[rng.below(SCALARS.len())])
}

/// A random container inside `value`, the root included.
fn random_container<'a>(rng: &mut Lcg, value: &'a mut Value) -> &'a mut Value {
    let children = match &*value {
        Value::Object(members) => members.values().filter(|v| is_container(v)).count(),
        Value::Array(elements) => elements.iter().filter(|v| is_container(v)).count(),
        _ => 0,
    };
    if children == 0 || rng.below(2) == 0 {
        return value;
    }
    let pick = rng.below(children);
    let child = match value {
        Value::Object(members) => members.values_mut().filter(|v| is_container(v)).nth(pick),
        Value::Array(elements) => elements.iter_mut().filter(|v| is_container(v)).nth(pick),
        _ => None,
    };
    random_container(rng, child.expect("a container"))
}

fn is_container(value: &Value) -> bool {
    value.is_object() || value.is_array()
}

/// Add, remove, or change one item somewhere in `value`.
fn mutate(rng: &mut Lcg, value: &mut Value) {
    let target = random_container(rng, value);
    let fresh = if rng.below(3) == 0 {
        json!({ "hooks": [random_scalar(rng)] })
    } else {
        random_scalar(rng)
    };
    match target {
        Value::Object(members) => {
            let keys: Vec<String> = members.keys().cloned().collect();
            match rng.below(3) {
                0 if !keys.is_empty() => {
                    members.remove(&keys[rng.below(keys.len())]);
                }
                1 if !keys.is_empty() => {
                    members.insert(keys[rng.below(keys.len())].clone(), fresh);
                }
                _ => {
                    let key = parse(KEYS[rng.below(KEYS.len())]);
                    let key = key.as_str().expect("a key").to_string();
                    members.insert(key, fresh);
                }
            }
        }
        Value::Array(elements) => match rng.below(4) {
            0 if !elements.is_empty() => {
                elements.remove(rng.below(elements.len()));
            }
            1 if !elements.is_empty() => {
                let at = rng.below(elements.len());
                elements[at] = fresh;
            }
            2 => {
                let at = rng.below(elements.len() + 1);
                elements.insert(at, fresh);
            }
            _ => elements.push(fresh),
        },
        _ => unreachable!("a container"),
    }
}

#[test]
fn random_documents_and_edits_splice_to_exactly_the_new_value() {
    for seed in 0..600u64 {
        let mut rng = Lcg(seed);
        let unit = ["  ", "    ", "\t"][rng.below(3)];
        let newline = ["\n", "\r\n"][rng.below(2)];
        let text = random_text(&mut rng, unit, newline);
        let old = parse(&text);
        let mut new = old.clone();
        for _ in 0..=rng.below(3) {
            mutate(&mut rng, &mut new);
        }
        let out = splice(&text, &new)
            .unwrap_or_else(|err| panic!("seed {seed}: {err:#}\n{text:?}\n{new}"));
        assert_eq!(parse(&out), new, "seed {seed}");
        if old == new {
            assert_eq!(out, text, "seed {seed}");
        }
    }
}

#[test]
fn random_documents_get_their_bytes_back_after_an_append_and_a_cut() {
    for seed in 0..600u64 {
        let mut rng = Lcg(seed);
        let unit = ["  ", "    ", "\t"][rng.below(3)];
        let newline = ["\n", "\r\n"][rng.below(2)];
        let text = random_text(&mut rng, unit, newline);
        let old = parse(&text);
        let mut new = old.clone();
        match random_container(&mut rng, &mut new) {
            Value::Object(members) => {
                let free = KEYS
                    .iter()
                    .map(|key| parse(key).as_str().expect("a key").to_string())
                    .find(|key| !members.contains_key(key));
                let Some(key) = free else { continue };
                members.insert(key, json!({ "hooks": [{ "type": "command" }] }));
            }
            Value::Array(elements) => elements.push(json!([1, { "a": null }])),
            _ => unreachable!("a container"),
        }
        let appended =
            splice(&text, &new).unwrap_or_else(|err| panic!("seed {seed}: {err:#}\n{text:?}"));
        let back = splice(&appended, &old)
            .unwrap_or_else(|err| panic!("seed {seed}: {err:#}\n{appended:?}"));
        assert_eq!(back, text, "seed {seed}: {appended:?}");
    }
}
