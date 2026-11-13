//! Installing, reporting, and removing the Claude Code hooks in a
//! settings document. Nothing here touches a file; the command-line
//! tests in `tests/cli_smoke.rs` run the same paths in a scratch home.

use serde_json::{Value, json};

use super::*;

const COMMAND: &str = "/opt/conspectus/bin/conspectus hook write claude-code";
const DEFAULT: &str = "/nix/store/abc-conspectus/bin/conspectus hook write claude-code";

fn install(document: &mut Value, command: Option<&str>) -> ClaudeInstall {
    ensure_claude_hook(document, command, DEFAULT).expect("install")
}

fn coverage_of(document: &Value) -> Vec<EventCoverage> {
    claude_hook_coverage(document)
        .into_iter()
        .map(|(_, coverage)| coverage)
        .collect()
}

fn label(document: &Value) -> &'static str {
    coverage_label(document, &claude_hook_coverage(document))
}

fn groups<'a>(document: &'a Value, event: &str) -> &'a Vec<Value> {
    document["hooks"][event].as_array().expect("event list")
}

/// The settings an operator might have before installing anything.
fn operator_settings() -> Value {
    json!({
        "theme": "dark",
        "model": "opus",
        "permissions": { "allow": ["Bash(git status)"], "deny": [] },
        "env": { "FOO": "1" },
        "hooks": {
            "SessionStart": [
                { "matcher": "startup", "hooks": [{ "type": "command", "command": "echo hello" }] }
            ],
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [{ "type": "command", "command": "/home/me/bin/guard", "timeout": 30 }]
                }
            ],
            "Stop": [
                { "hooks": [{ "type": "prompt", "prompt": "Is the work done?" }] }
            ]
        },
        "statusLine": { "type": "command", "command": "~/.claude/statusline.sh" }
    })
}

/// What `hook init claude-code` wrote before it installed every event.
fn session_start_only_install() -> Value {
    json!({
        "theme": "dark",
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "startup",
                    "hooks": [{ "type": "command", "command": "echo existing" }]
                },
                {
                    "matcher": "resume|startup|clear|compact",
                    "hooks": [{ "type": "command", "command": COMMAND }]
                }
            ]
        }
    })
}

#[test]
fn a_fresh_install_adds_one_group_per_event_with_its_matcher_and_a_timeout() {
    let mut document = json!({});
    assert_eq!(label(&document), "not-installed");
    assert!(!has_claude_hook(&document));

    let report = install(&mut document, None);
    assert_eq!(
        report,
        ClaudeInstall {
            added: 14,
            updated: 0
        }
    );
    assert!(report.changed());
    for subscription in HOOK_SUBSCRIPTIONS {
        let mut group = json!({
            "hooks": [{ "type": "command", "command": DEFAULT, "timeout": HOOK_TIMEOUT_S }]
        });
        if let Some(matcher) = subscription.matcher {
            group["matcher"] = json!(matcher);
        }
        assert_eq!(
            groups(&document, subscription.event),
            &vec![group],
            "{}",
            subscription.event
        );
    }
    assert_eq!(
        document["hooks"]["SessionStart"][0]["matcher"],
        "startup|resume|clear|compact|fork"
    );
    assert_eq!(
        document["hooks"]["Notification"][0]["matcher"],
        "permission_prompt|idle_prompt|elicitation_dialog|elicitation_url_dialog|\
         agent_needs_input|quota_auto_resume_fired|quota_auto_resume_stale|\
         quota_auto_resume_disabled"
    );
    assert_eq!(
        document["hooks"]["PreToolUse"][0]["matcher"],
        "AskUserQuestion|ExitPlanMode"
    );
    assert!(
        coverage_of(&document)
            .iter()
            .all(|coverage| *coverage == EventCoverage::Installed)
    );
    assert_eq!(label(&document), "installed");
    assert!(has_claude_hook(&document));
}

#[test]
fn installing_again_changes_nothing() {
    for start in [json!({}), operator_settings(), session_start_only_install()] {
        let mut document = start;
        install(&mut document, None);
        let installed = document.clone();
        assert_eq!(install(&mut document, None), ClaudeInstall::default());
        assert_eq!(document, installed);
        // The same command given again is no change either.
        let command = groups(&installed, "Stop")
            .iter()
            .find(|group| is_own_group(group))
            .and_then(|group| group["hooks"][0]["command"].as_str())
            .expect("command")
            .to_string();
        assert_eq!(
            install(&mut document, Some(&command)),
            ClaudeInstall::default()
        );
        assert_eq!(document, installed);
    }
}

#[test]
fn a_session_start_only_install_is_upgraded_in_place_and_never_duplicated() {
    let mut document = session_start_only_install();
    let mut before = vec![EventCoverage::Missing; 14];
    before[0] = EventCoverage::Outdated;
    assert_eq!(coverage_of(&document), before);
    assert_eq!(label(&document), "partial");

    let report = install(&mut document, None);
    assert_eq!(
        report,
        ClaudeInstall {
            added: 13,
            updated: 1
        }
    );
    // The operator's group stays first and untouched; Conspectus's old
    // group gains every source and a timeout, and keeps its command.
    assert_eq!(
        groups(&document, "SessionStart"),
        &vec![
            json!({
                "matcher": "startup",
                "hooks": [{ "type": "command", "command": "echo existing" }]
            }),
            json!({
                "matcher": "startup|resume|clear|compact|fork",
                "hooks": [{ "type": "command", "command": COMMAND, "timeout": HOOK_TIMEOUT_S }]
            }),
        ]
    );
    // The new events run the command the old install ran, not this
    // binary's.
    for subscription in &HOOK_SUBSCRIPTIONS[1..] {
        assert_eq!(
            document["hooks"][subscription.event][0]["hooks"][0]["command"], COMMAND,
            "{}",
            subscription.event
        );
    }
    assert_eq!(document["theme"], "dark");
    assert_eq!(label(&document), "installed");
}

#[test]
fn a_command_given_again_moves_every_handler_to_it() {
    let mut document = session_start_only_install();
    install(&mut document, None);
    let moved = "conspectus hook write claude-code";
    let report = install(&mut document, Some(moved));
    assert_eq!(
        report,
        ClaudeInstall {
            added: 0,
            updated: 14
        }
    );
    for subscription in HOOK_SUBSCRIPTIONS {
        let own = groups(&document, subscription.event)
            .iter()
            .filter(|group| is_own_group(group))
            .collect::<Vec<_>>();
        assert_eq!(own.len(), 1, "{}", subscription.event);
        assert_eq!(own[0]["hooks"][0]["command"], moved);
    }
    // The operator's own handler is not Conspectus's and stays.
    assert_eq!(
        document["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        "echo existing"
    );
}

#[test]
fn the_operators_hooks_and_unrelated_keys_survive_install_and_remove() {
    let original = operator_settings();
    let mut document = original.clone();
    install(&mut document, Some(COMMAND));

    for (key, value) in original.as_object().expect("object") {
        if key != "hooks" {
            assert_eq!(&document[key], value, "{key}");
        }
    }
    for (event, operator_groups) in original["hooks"].as_object().expect("hooks") {
        let groups = groups(&document, event);
        let operator_groups = operator_groups.as_array().expect("groups");
        // The operator's groups come first, as they were; Conspectus's
        // follows.
        assert_eq!(
            &groups[..operator_groups.len()],
            &operator_groups[..],
            "{event}"
        );
        assert_eq!(groups.len(), operator_groups.len() + 1, "{event}");
    }

    assert!(remove_claude_hook(&mut document));
    assert_eq!(document, original);
    assert!(!remove_claude_hook(&mut document));
}

#[test]
fn install_and_remove_on_an_empty_file_leave_it_empty() {
    let mut document = json!({ "theme": "dark" });
    install(&mut document, None);
    assert!(remove_claude_hook(&mut document));
    assert_eq!(document, json!({ "theme": "dark" }));
}

#[test]
fn a_conspectus_handler_inside_an_operators_group_is_left_and_reported_outdated() {
    let mixed = json!({
        "matcher": "Bash",
        "hooks": [
            { "type": "command", "command": "/home/me/bin/guard" },
            { "type": "command", "command": COMMAND, "timeout": 5 }
        ]
    });
    let mut document = json!({ "hooks": { "PostToolUse": [mixed] } });
    assert_eq!(coverage_of(&document)[4], EventCoverage::Outdated);
    assert_eq!(label(&document), "partial");

    install(&mut document, Some(COMMAND));
    let groups = groups(&document, "PostToolUse");
    assert_eq!(groups[0], mixed);
    assert_eq!(
        groups[1],
        json!({ "hooks": [{ "type": "command", "command": COMMAND, "timeout": 5 }] })
    );
    assert_eq!(coverage_of(&document)[4], EventCoverage::Installed);

    // Remove takes the handler out of the operator's group too, and
    // leaves the group with the operator's own.
    assert!(remove_claude_hook(&mut document));
    assert_eq!(
        document["hooks"]["PostToolUse"],
        json!([{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "/home/me/bin/guard" }] }])
    );
}

#[test]
fn an_exec_form_entry_is_recognized_and_copied() {
    let exec = json!({
        "type": "command",
        "command": "/opt/conspectus/bin/conspectus",
        "args": ["hook", "write", "claude-code"],
        "timeout": 5
    });
    assert!(hook_is_conspectus_command(&exec));
    let mut document = json!({
        "hooks": {
            "SessionStart": [{ "matcher": "startup|resume|clear|compact|fork", "hooks": [exec] }]
        }
    });
    assert_eq!(coverage_of(&document)[0], EventCoverage::Installed);

    let report = install(&mut document, None);
    assert_eq!(
        report,
        ClaudeInstall {
            added: 13,
            updated: 0
        }
    );
    assert_eq!(document["hooks"]["Stop"][0]["hooks"][0], exec);

    // The same command line given with `--command` changes nothing; another
    // moves an exec-form handler to the shell form it names.
    assert_eq!(
        install(&mut document, Some(COMMAND)),
        ClaudeInstall::default()
    );
    install(&mut document, Some("conspectus hook write claude-code"));
    assert_eq!(
        document["hooks"]["SessionStart"][0]["hooks"][0],
        json!({ "type": "command", "command": "conspectus hook write claude-code", "timeout": 5 })
    );

    assert!(remove_claude_hook(&mut document));
    install(&mut document, None);
    document["hooks"]["Stop"][0]["hooks"][0] = exec;
    assert!(remove_claude_hook(&mut document));
    assert!(document.get("hooks").is_none(), "{document}");
}

#[test]
fn only_a_command_hook_running_the_claude_code_writer_is_conspectuss() {
    for (handler, ours) in [
        (json!({ "type": "command", "command": COMMAND }), true),
        (
            json!({ "type": "command", "command": "'/path with space/conspectus' hook write claude-code" }),
            true,
        ),
        (
            json!({ "type": "command", "command": "conspectus", "args": ["hook", "write", "claude-code"] }),
            true,
        ),
        (
            json!({ "type": "command", "command": "conspectus hook write codex" }),
            false,
        ),
        (
            json!({ "type": "command", "command": "conspectus", "args": ["hook", "write", "codex"] }),
            false,
        ),
        (json!({ "type": "prompt", "prompt": COMMAND }), false),
        (json!({ "type": "command" }), false),
        (json!("hook write claude-code"), false),
    ] {
        assert_eq!(hook_is_conspectus_command(&handler), ours, "{handler}");
    }
}

#[test]
fn a_matcher_that_already_covers_the_event_is_left_as_it_is() {
    let own = |matcher: Option<&str>| {
        let mut group =
            json!({ "hooks": [{ "type": "command", "command": COMMAND, "timeout": 5 }] });
        if let Some(matcher) = matcher {
            group["matcher"] = json!(matcher);
        }
        group
    };
    for (event, matcher, covers) in [
        // No matcher, an empty one, or `*` matches every occurrence.
        ("SessionStart", None, true),
        ("Notification", Some(""), true),
        ("PreToolUse", Some("*"), true),
        // An exact list in another order, or with more names, covers.
        (
            "SessionStart",
            Some("fork|compact|clear|resume|startup"),
            true,
        ),
        (
            "PreToolUse",
            Some("ExitPlanMode|AskUserQuestion|Bash"),
            true,
        ),
        // A list missing a name does not, nor does a regular expression
        // other than the expected one.
        ("SessionStart", Some("resume|startup|clear|compact"), false),
        ("Notification", Some("permission_.*"), false),
        // An event that needs every occurrence needs no matcher.
        ("Stop", Some("anything"), false),
    ] {
        let mut document = json!({ "hooks": { event: [own(matcher)] } });
        let index = HOOK_SUBSCRIPTIONS
            .iter()
            .position(|subscription| subscription.event == event)
            .expect("subscribed");
        let expected = if covers {
            EventCoverage::Installed
        } else {
            EventCoverage::Outdated
        };
        assert_eq!(
            coverage_of(&document)[index],
            expected,
            "{event} {matcher:?}"
        );
        install(&mut document, None);
        let groups = groups(&document, event);
        assert_eq!(groups.len(), 1, "{event} {matcher:?}");
        if covers {
            assert_eq!(groups[0], own(matcher));
        } else {
            assert_eq!(groups[0], own(HOOK_SUBSCRIPTIONS[index].matcher));
        }
    }
}

#[test]
fn a_handler_without_a_timeout_is_outdated_until_install_gives_it_one() {
    for timeout in [None, Some(json!(0)), Some(json!("5"))] {
        let mut handler = json!({ "type": "command", "command": COMMAND });
        if let Some(timeout) = &timeout {
            handler["timeout"] = timeout.clone();
        }
        let mut document = json!({ "hooks": { "Stop": [{ "hooks": [handler] }] } });
        assert_eq!(
            coverage_of(&document)[9],
            EventCoverage::Outdated,
            "{timeout:?}"
        );
        install(&mut document, None);
        assert_eq!(
            document["hooks"]["Stop"][0]["hooks"][0]["timeout"],
            HOOK_TIMEOUT_S
        );
    }
    // A timeout the operator chose is theirs to keep.
    let mut document = json!({
        "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": COMMAND, "timeout": 10 }] }] }
    });
    assert_eq!(coverage_of(&document)[9], EventCoverage::Installed);
    install(&mut document, None);
    assert_eq!(document["hooks"]["Stop"][0]["hooks"][0]["timeout"], 10);
}

#[test]
fn of_two_groups_of_conspectuss_the_current_one_is_kept_and_none_is_added() {
    let old = json!({ "matcher": "resume", "hooks": [{ "type": "command", "command": COMMAND }] });
    let current = json!({
        "matcher": "startup|resume|clear|compact|fork",
        "hooks": [{ "type": "command", "command": COMMAND, "timeout": 5 }]
    });
    let mut document = json!({ "hooks": { "SessionStart": [old, current] } });
    install(&mut document, None);
    assert_eq!(groups(&document, "SessionStart"), &vec![old, current]);
}

#[test]
fn install_refuses_a_hooks_value_it_would_have_to_replace() {
    for settings in [
        json!({ "hooks": "not an object" }),
        json!({ "hooks": { "Stop": { "hooks": [] } } }),
        json!({ "hooks": { "PreToolUse": "x" } }),
    ] {
        let mut document = settings.clone();
        assert!(
            ensure_claude_hook(&mut document, None, DEFAULT).is_err(),
            "{settings}"
        );
        assert_eq!(document, settings);
    }
    // An event Conspectus does not install may hold anything.
    let mut document = json!({ "hooks": { "Setup": "anything" } });
    install(&mut document, None);
    assert_eq!(document["hooks"]["Setup"], "anything");
}

#[test]
fn remove_takes_every_conspectus_handler_from_every_event_and_only_what_it_emptied() {
    let mut document = json!({
        "hooks": {
            "TeammateIdle": [{ "hooks": [{ "type": "command", "command": COMMAND }] }],
            "Stop": [
                { "matcher": "", "hooks": [] },
                { "hooks": [{ "type": "command", "command": COMMAND }] }
            ],
            "SubagentStop": [],
            "Weird": "not a list",
            "PostToolUse": [{ "matcher": "Bash", "hooks": "not a list" }]
        }
    });
    assert!(remove_claude_hook(&mut document));
    assert_eq!(
        document,
        json!({
            "hooks": {
                "Stop": [{ "matcher": "", "hooks": [] }],
                "SubagentStop": [],
                "Weird": "not a list",
                "PostToolUse": [{ "matcher": "Bash", "hooks": "not a list" }]
            }
        })
    );
    assert!(!has_claude_hook(&document));
    assert_eq!(label(&document), "not-installed");
}

#[test]
fn status_and_remove_tolerate_whatever_the_file_holds() {
    for document in [
        json!({}),
        json!({ "hooks": "x" }),
        json!({ "hooks": { "Stop": 7 } }),
        json!({ "hooks": { "Stop": [7, { "hooks": 7 }] } }),
    ] {
        assert_eq!(label(&document), "not-installed", "{document}");
        let mut copy = document.clone();
        assert!(!remove_claude_hook(&mut copy));
        assert_eq!(copy, document);
    }
}
