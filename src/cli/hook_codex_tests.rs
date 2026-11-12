//! Installing, reporting, and removing the Codex hooks in a
//! `config.toml` document, over the install cases under
//! `tests/fixtures/hooks/codex/install/`. Nothing here writes a file
//! outside a scratch directory; the command-line tests in
//! `tests/cli_smoke.rs` run the same paths in a scratch home.

use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use super::*;

const COMMAND: &str = "/opt/conspectus/bin/conspectus hook write codex";
const DEFAULT: &str = "/nix/store/abc-conspectus/bin/conspectus hook write codex";

fn case_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hooks/codex/install")
        .join(name)
}

fn case(name: &str) -> String {
    let path = case_path(name);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// Every TOML install case, by file name.
const CASES: [&str; 6] = [
    "operator-array-of-tables.toml",
    "inline-hooks.toml",
    "inline-lists.toml",
    "dotted-keys.toml",
    "hooks-state.toml",
    "conspectus-between-operator-groups.toml",
];

fn parse(text: &str) -> DocumentMut {
    text.parse().expect("a TOML document")
}

fn install(document: &mut DocumentMut, command: Option<&str>) -> CodexInstall {
    ensure_codex_hook(document, command, DEFAULT).expect("install")
}

/// `text` after an install, as `hook init codex` would write it.
fn installed(text: &str, command: Option<&str>) -> (String, CodexInstall) {
    let mut document = parse(text);
    let report = install(&mut document, command);
    (checked_text(&document, true).expect("checked"), report)
}

/// `text` after a removal, as `hook remove codex` would write it.
fn removed(text: &str) -> (String, CodexRemoval) {
    let mut document = parse(text);
    let removal = remove_codex_hook(&mut document);
    let text = if removal.changed {
        checked_text(&document, false).expect("checked")
    } else {
        document.to_string()
    };
    (text, removal)
}

/// What a TOML reader sees in `text`, formatting aside.
fn meaning(text: &str) -> toml::Table {
    toml::from_str(text).expect("valid TOML")
}

fn coverage_of(document: &DocumentMut) -> Vec<EventCoverage> {
    codex_hook_coverage(document)
        .into_iter()
        .map(|(_, coverage)| coverage)
        .collect()
}

fn label(text: &str) -> &'static str {
    let document = parse(text);
    codex_coverage_label(&document, &codex_hook_coverage(&document))
}

/// The groups of `event` as a TOML reader sees them.
fn groups(text: &str, event: &str) -> Vec<toml::Value> {
    meaning(text)["hooks"][event]
        .as_array()
        .expect("an event list")
        .clone()
}

/// Conspectus's group as an install writes it.
fn own_group(command: &str) -> toml::Value {
    toml::Value::Table(meaning(&format!(
        "hooks = [{{ type = \"command\", command = \"{command}\", timeout = 5 }}]"
    )))
}

#[test]
fn a_fresh_install_writes_one_array_of_tables_group_per_event() {
    let (text, report) = installed("", None);
    assert_eq!(
        report,
        CodexInstall {
            added: HOOK_EVENTS.len(),
            updated: 0
        }
    );
    let expected: Vec<String> = HOOK_EVENTS
        .iter()
        .map(|event| {
            format!(
                "[[hooks.{event}]]\nhooks = [{{ type = \"command\", command = \"{DEFAULT}\", \
                 timeout = 5 }}]\n"
            )
        })
        .collect();
    assert_eq!(text, expected.join("\n"));
    assert_eq!(label(&text), "installed");
    assert!(has_codex_hook(&parse(&text)));
    assert_eq!(label(""), "not-installed");
}

#[test]
fn installing_into_a_file_with_other_settings_keeps_them_and_adds_no_empty_hooks_table() {
    let original = "model = \"gpt-5.4\"\n\n[tui]\nnotifications = true\n";
    let (text, _) = installed(original, None);
    assert!(text.starts_with(original), "{text}");
    assert!(!text.contains("[hooks]"), "{text}");
    let (back, removal) = removed(&text);
    assert!(removal.changed && !removal.moved);
    assert_eq!(back, original);
}

#[test]
fn install_then_remove_gives_back_every_case_unchanged() {
    for name in CASES {
        let original = case(name);
        let mut document = parse(&original);
        // Take out what an earlier Conspectus left, so the case is the
        // operator's own file.
        remove_codex_hook(&mut document);
        let operator = document.to_string();

        let (text, report) = installed(&operator, Some(COMMAND));
        assert!(report.changed(), "{name}");
        assert_eq!(label(&text), "installed", "{name}");
        let (back, removal) = removed(&text);
        assert!(removal.changed, "{name}");
        assert!(
            !removal.moved,
            "{name}: nothing of the operator's comes after"
        );
        assert_eq!(back, operator, "{name}");
        assert_eq!(label(&back), "not-installed", "{name}");
    }
}

#[test]
fn installing_again_changes_nothing() {
    for name in CASES {
        let (text, _) = installed(&case(name), None);
        let (again, report) = installed(&text, None);
        assert_eq!(report, CodexInstall::default(), "{name}");
        assert_eq!(again, text, "{name}");
        // The same command given again is no change either.
        let command = first_command(&parse(&text)).expect("a command");
        let (again, report) = installed(&text, Some(&command));
        assert_eq!(report, CodexInstall::default(), "{name}");
        assert_eq!(again, text, "{name}");
    }
}

#[test]
fn every_case_keeps_the_operators_values_and_puts_conspectuss_group_after_theirs() {
    for name in CASES {
        let original = case(name);
        let mut operator = parse(&original);
        remove_codex_hook(&mut operator);
        let operator = operator.to_string();
        let before = meaning(&operator);
        let (text, _) = installed(&operator, Some(COMMAND));
        let after = meaning(&text);

        // Every key outside the hooks table is as it was.
        for (key, value) in &before {
            if key != "hooks" {
                assert_eq!(&after[key], value, "{name}: {key}");
            }
        }
        // Each event list starts with the operator's groups, unchanged,
        // and ends with Conspectus's own; the other keys of the hooks
        // table are untouched.
        let empty = toml::Table::new();
        let before_hooks = before
            .get("hooks")
            .and_then(toml::Value::as_table)
            .unwrap_or(&empty);
        let after_hooks = after["hooks"].as_table().expect("hooks");
        for (key, value) in before_hooks {
            if HOOK_EVENTS.contains(&key.as_str()) {
                let operator_groups = value.as_array().expect("groups");
                let groups = after_hooks[key].as_array().expect("groups");
                assert_eq!(&groups[..operator_groups.len()], &operator_groups[..]);
                assert_eq!(groups[operator_groups.len()..], [own_group(COMMAND)]);
            } else {
                assert_eq!(&after_hooks[key], value, "{name}: hooks.{key}");
            }
        }
        for event in HOOK_EVENTS {
            if !before_hooks.contains_key(event) {
                assert_eq!(
                    after_hooks[event].as_array().expect("groups"),
                    &vec![own_group(COMMAND)]
                );
            }
        }
    }
}

#[test]
fn an_operators_array_of_tables_gains_a_group_and_keeps_its_text() {
    let original = case("operator-array-of-tables.toml");
    let (text, _) = installed(&original, Some(COMMAND));
    // The operator's text is all there, each part in its place, with
    // Conspectus's session start group right after theirs.
    let session_start_end = "statusMessage = \"Logging the session\"\n";
    let (head, tail) = original
        .split_once(session_start_end)
        .expect("the operator's session start group");
    assert!(
        text.starts_with(&format!("{head}{session_start_end}")),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "{session_start_end}\n[[hooks.SessionStart]]\nhooks = [{{ type = \"command\", \
             command = \"{COMMAND}\", timeout = 5 }}]\n"
        )),
        "{text}"
    );
    for line in tail.lines() {
        assert!(text.contains(line), "{line}");
    }
    assert_eq!(
        groups(&text, "SessionStart")[0]["hooks"][0]["statusMessage"].as_str(),
        Some("Logging the session")
    );
}

#[test]
fn an_inline_hooks_table_stays_inline() {
    let (text, _) = installed(&case("inline-hooks.toml"), Some(COMMAND));
    let document = parse(&text);
    assert!(
        matches!(
            document.get("hooks"),
            Some(Item::Value(Value::InlineTable(_)))
        ),
        "{text}"
    );
    assert!(!text.contains("[[hooks"), "{text}");
    assert_eq!(
        groups(&text, "SessionStart"),
        vec![
            toml::Value::Table(meaning(
                "matcher = \"startup\"\nhooks = [{ type = \"command\", command = \"echo started\" }]"
            )),
            own_group(COMMAND),
        ]
    );
}

#[test]
fn dotted_keys_stay_dotted_keys() {
    let (text, _) = installed(&case("dotted-keys.toml"), Some(COMMAND));
    assert!(
        !text.contains("[hooks]") && !text.contains("[[hooks"),
        "{text}"
    );
    assert!(text.contains(
        "hooks.SessionStart = [{ hooks = [{ type = \"command\", command = \"echo dotted\" }] }, "
    ));
    assert_eq!(groups(&text, "SessionStart").len(), 2);
}

#[test]
fn an_older_install_is_upgraded_in_place_and_new_entries_run_its_command() {
    let original = case("inline-lists.toml");
    let document = parse(&original);
    let mut before = vec![EventCoverage::Missing; HOOK_EVENTS.len()];
    before[0] = EventCoverage::Outdated;
    assert_eq!(coverage_of(&document), before);
    assert_eq!(label(&original), "partial");

    let (text, report) = installed(&original, None);
    assert_eq!(
        report,
        CodexInstall {
            added: HOOK_EVENTS.len() - 1,
            updated: 1
        }
    );
    // The operator's entry stays first and untouched; the old entry gains
    // a timeout and keeps its command and its `async`.
    let session_start = groups(&text, "SessionStart");
    assert_eq!(session_start.len(), 2, "never duplicated: {text}");
    assert_eq!(
        session_start[0],
        toml::Value::Table(meaning(
            "hooks = [{ type = \"command\", command = \"echo existing\", async = false }]"
        ))
    );
    assert_eq!(
        session_start[1],
        toml::Value::Table(meaning(
            "hooks = [{ type = \"command\", command = \"/old/bin/conspectus hook write codex\", \
             async = false, timeout = 5 }]"
        ))
    );
    assert!(
        text.contains(
            "      { type = \"command\", command = \"echo existing\", async = false },\n"
        )
    );
    assert!(
        text.contains(
            "  { hooks = [{ type = \"command\", command = \"/old/bin/conspectus hook write codex\", \
             async = false, timeout = 5 }] },\n"
        ),
        "{text}"
    );
    // New events run the command the old install ran, inline as the
    // file's lists are.
    for event in &HOOK_EVENTS[1..] {
        assert_eq!(
            groups(&text, event),
            vec![own_group("/old/bin/conspectus hook write codex")]
        );
    }
    assert!(!text.contains("[[hooks"), "{text}");
    assert_eq!(label(&text), "installed");
}

#[test]
fn a_command_given_again_moves_every_handler_to_it() {
    let (text, _) = installed(&case("inline-lists.toml"), None);
    let moved = "conspectus hook write codex";
    let (text, report) = installed(&text, Some(moved));
    assert_eq!(
        report,
        CodexInstall {
            added: 0,
            updated: HOOK_EVENTS.len()
        }
    );
    for event in HOOK_EVENTS {
        let own: Vec<toml::Value> = groups(&text, event)
            .into_iter()
            .filter(|group| {
                group["hooks"][0]["command"]
                    .as_str()
                    .is_some_and(is_conspectus_hook_command)
            })
            .collect();
        assert_eq!(own.len(), 1, "{event}");
        assert_eq!(own[0]["hooks"][0]["command"].as_str(), Some(moved));
    }
    // The operator's own handler is not Conspectus's and stays.
    assert_eq!(
        groups(&text, "SessionStart")[0]["hooks"][0]["command"].as_str(),
        Some("echo existing")
    );
}

#[test]
fn the_trust_codex_recorded_is_never_touched() {
    let original = case("hooks-state.toml");
    let state_text = original
        .split("[[hooks.SessionStart]]")
        .next()
        .expect("the state table comes first")
        .to_string();
    let state = meaning(&original)["hooks"]["state"].clone();

    let (text, _) = installed(&original, Some(COMMAND));
    assert!(text.starts_with(&state_text), "{text}");
    assert_eq!(meaning(&text)["hooks"]["state"], state);
    assert_eq!(text.matches("trusted_hash").count(), 1, "{text}");
    assert!(!text.contains("trusted_content_hash"));

    let (back, _) = removed(&text);
    assert_eq!(back, original);
    assert_eq!(meaning(&back)["hooks"]["state"], state);
}

#[test]
fn remove_takes_only_conspectuss_group_from_between_the_operators() {
    let original = case("conspectus-between-operator-groups.toml");
    assert_eq!(coverage_of(&parse(&original))[0], EventCoverage::Installed);
    let (text, removal) = removed(&original);
    assert_eq!(
        removal,
        CodexRemoval {
            changed: true,
            moved: true
        }
    );
    let middle = "\n[[hooks.SessionStart]]\nhooks = [{ type = \"command\", command = \
         \"/usr/local/bin/conspectus hook write codex\", timeout = 5 }]\n";
    assert!(original.contains(middle));
    assert_eq!(text, original.replace(middle, ""));
    let session_start = groups(&text, "SessionStart");
    assert_eq!(session_start.len(), 2);
    assert_eq!(session_start[0]["matcher"].as_str(), Some("startup"));
    assert_eq!(session_start[1]["matcher"].as_str(), Some("resume"));
    assert!(!has_codex_hook(&parse(&text)));
}

#[test]
fn a_conspectus_handler_inside_an_operators_group_is_left_and_reported_outdated() {
    let original = format!(
        "[[hooks.SessionStart]]\nmatcher = \"startup\"\nhooks = [\n  {{ type = \"command\", command = \
         \"{COMMAND}\", timeout = 5 }},\n  {{ type = \"command\", command = \"/home/me/bin/guard\" }},\n]\n"
    );
    assert_eq!(coverage_of(&parse(&original))[0], EventCoverage::Outdated);
    assert_eq!(label(&original), "partial");

    let (text, _) = installed(&original, Some(COMMAND));
    assert!(text.starts_with(&original), "{text}");
    assert_eq!(coverage_of(&parse(&text))[0], EventCoverage::Installed);

    // Remove takes the handler out of the operator's group too, which
    // moves the operator's handler up a place.
    let (back, removal) = removed(&text);
    assert!(removal.moved);
    assert_eq!(
        groups(&back, "SessionStart"),
        vec![toml::Value::Table(meaning(
            "matcher = \"startup\"\nhooks = [{ type = \"command\", command = \"/home/me/bin/guard\" }]"
        ))]
    );
}

#[test]
fn a_matcher_or_a_missing_timeout_makes_a_group_outdated_until_install_fixes_it() {
    for (group, current) in [
        (
            "hooks = [{ type = \"command\", command = \"C\", timeout = 5 }]",
            true,
        ),
        (
            "matcher = \"\"\nhooks = [{ type = \"command\", command = \"C\", timeout = 5 }]",
            true,
        ),
        (
            "hooks = [{ type = \"command\", command = \"C\", timeout = 2.5 }]",
            true,
        ),
        (
            "matcher = \"startup\"\nhooks = [{ type = \"command\", command = \"C\", timeout = 5 }]",
            false,
        ),
        (
            "matcher = \"*\"\nhooks = [{ type = \"command\", command = \"C\", timeout = 5 }]",
            false,
        ),
        ("hooks = [{ type = \"command\", command = \"C\" }]", false),
        (
            "hooks = [{ type = \"command\", command = \"C\", timeout = 0 }]",
            false,
        ),
        (
            "hooks = [{ type = \"command\", command = \"C\", timeout = \"5\" }]",
            false,
        ),
    ] {
        let text = format!("[[hooks.SessionStart]]\n{}\n", group.replace('C', COMMAND));
        let expected = if current {
            EventCoverage::Installed
        } else {
            EventCoverage::Outdated
        };
        assert_eq!(coverage_of(&parse(&text))[0], expected, "{group}");
        let (fixed, report) = installed(&text, None);
        assert_eq!(report.updated, usize::from(!current), "{group}");
        let session_start = groups(&fixed, "SessionStart");
        assert_eq!(session_start.len(), 1, "{group}");
        if !current {
            assert_eq!(session_start[0], own_group(COMMAND), "{group}");
        }
    }
    // A timeout the operator chose is theirs to keep.
    let text = format!(
        "[[hooks.SessionStart]]\nhooks = [{{ type = \"command\", command = \"{COMMAND}\", timeout = 30 }}]\n"
    );
    let (fixed, report) = installed(&text, None);
    assert_eq!(report.updated, 0);
    assert_eq!(
        groups(&fixed, "SessionStart")[0]["hooks"][0]["timeout"].as_integer(),
        Some(30)
    );
}

#[test]
fn handlers_in_an_array_of_tables_are_recognized_and_upgraded() {
    let text = format!(
        "[[hooks.SessionStart]]\n\n[[hooks.SessionStart.hooks]]\ntype = \"command\"\ncommand = \"{COMMAND}\"\n"
    );
    assert_eq!(coverage_of(&parse(&text))[0], EventCoverage::Outdated);
    let (fixed, report) = installed(&text, None);
    assert_eq!(report.updated, 1);
    assert!(
        fixed.contains(&format!("command = \"{COMMAND}\"\ntimeout = 5\n")),
        "{fixed}"
    );
    assert_eq!(label(&fixed), "installed");
}

#[test]
fn only_a_command_hook_running_the_codex_writer_is_conspectuss() {
    for (handler, ours) in [
        (
            format!("{{ type = \"command\", command = \"{COMMAND}\" }}"),
            true,
        ),
        (
            "{ type = \"command\", command = \"'/path with space/conspectus' hook write codex\" }"
                .to_string(),
            true,
        ),
        (
            "{ type = \"command\", command = \"conspectus hook write claude-code\" }".to_string(),
            false,
        ),
        (
            format!("{{ type = \"mcp_tool\", command = \"{COMMAND}\" }}"),
            false,
        ),
        ("{ type = \"command\" }".to_string(), false),
    ] {
        let text = format!("[[hooks.Stop]]\nhooks = [{handler}]\n");
        assert_eq!(has_codex_hook(&parse(&text)), ours, "{handler}");
    }
}

#[test]
fn install_refuses_a_hooks_value_it_would_have_to_replace() {
    for text in [
        "hooks = \"not a table\"\n",
        "hooks = [1, 2]\n",
        "[hooks.SessionStart]\nmatcher = \"startup\"\n",
        "hooks = { SessionStart = \"x\" }\n",
        "[hooks]\nSessionStart = 7\n",
    ] {
        let mut document = parse(text);
        assert!(
            ensure_codex_hook(&mut document, None, DEFAULT).is_err(),
            "{text}"
        );
        assert_eq!(document.to_string(), text);
    }
    // An event Conspectus does not install may hold anything.
    let (text, _) = installed("[hooks]\nPreModelRequest = \"anything\"\n", None);
    assert_eq!(
        meaning(&text)["hooks"]["PreModelRequest"].as_str(),
        Some("anything")
    );
}

#[test]
fn remove_takes_every_conspectus_handler_from_every_event_and_only_what_it_emptied() {
    let text = format!(
        "[hooks]\nPreModelRequest = [{{ hooks = [{{ type = \"command\", command = \"{COMMAND}\" }}] }}]\n\
         Stop = [{{ matcher = \"\", hooks = [] }}, {{ hooks = [{{ type = \"command\", command = \"{COMMAND}\" }}] }}]\n\
         SubagentStop = []\nWeird = \"not a list\"\nPostToolUse = [{{ matcher = \"Bash\", hooks = \"not a list\" }}, 7]\n"
    );
    let (back, removal) = removed(&text);
    assert!(removal.changed);
    assert!(!removal.moved);
    assert_eq!(
        meaning(&back),
        meaning(
            "[hooks]\nStop = [{ matcher = \"\", hooks = [] }]\nSubagentStop = []\n\
             Weird = \"not a list\"\nPostToolUse = [{ matcher = \"Bash\", hooks = \"not a list\" }, 7]\n"
        )
    );
    assert_eq!(label(&back), "not-installed");
}

#[test]
fn status_and_remove_tolerate_whatever_the_file_holds() {
    for text in [
        "",
        "hooks = \"x\"\n",
        "[hooks]\nStop = 7\n",
        "[hooks]\nStop = [7, { hooks = 7 }]\n",
        "[hooks.Stop]\nhooks = []\n",
    ] {
        assert_eq!(label(text), "not-installed", "{text}");
        let (back, removal) = removed(text);
        assert_eq!(removal, CodexRemoval::default(), "{text}");
        assert_eq!(back, text);
    }
}

#[test]
fn a_hooks_json_beside_the_config_gets_a_note() {
    let scratch = tempfile::tempdir().expect("scratch");
    let config = scratch.path().join("config.toml");
    for note_for in [NoteFor::Init, NoteFor::Status, NoteFor::Remove] {
        assert_eq!(hooks_json_note(&config, note_for), None);
    }

    let hooks_json = scratch.path().join("hooks.json");
    fs::copy(case_path("hooks.json"), &hooks_json).expect("hooks.json");
    assert_eq!(
        hooks_json_beside(&config),
        Some((hooks_json.clone(), false))
    );
    let init = hooks_json_note(&config, NoteFor::Init).expect("a note");
    assert!(init.contains(&hooks_json.display().to_string()), "{init}");
    assert!(init.contains("also holds Codex hooks"), "{init}");
    assert!(hooks_json_note(&config, NoteFor::Status).is_some());
    assert_eq!(hooks_json_note(&config, NoteFor::Remove), None);

    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks_json).expect("read")).expect("json");
    value["hooks"]["Stop"] =
        serde_json::json!([{ "hooks": [{ "type": "command", "command": COMMAND }] }]);
    fs::write(&hooks_json, value.to_string()).expect("write");
    assert_eq!(hooks_json_beside(&config), Some((hooks_json.clone(), true)));
    for note_for in [NoteFor::Init, NoteFor::Status, NoteFor::Remove] {
        let note = hooks_json_note(&config, note_for).expect("a note");
        assert!(note.contains("Conspectus hook"), "{note}");
    }

    // A file that does not parse is still a source Codex may read.
    fs::write(&hooks_json, "{ not json").expect("write");
    assert_eq!(hooks_json_beside(&config), Some((hooks_json, false)));
}

#[test]
fn the_review_hint_names_the_hooks_command_and_cites_nothing() {
    assert!(REVIEW_HINT.contains("/hooks"));
    assert!(MOVED_NOTE.contains("/hooks"));
    for text in [REVIEW_HINT, MOVED_NOTE] {
        assert!(!text.contains("ADR") && !text.contains("docs/"), "{text}");
    }
}
