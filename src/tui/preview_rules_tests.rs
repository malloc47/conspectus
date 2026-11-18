use super::*;

/// `[tui.preview]` from `body`, a config file's text.
fn file(body: &str) -> PreviewFile {
    #[derive(Deserialize)]
    struct Document {
        tui: Tui,
    }
    #[derive(Deserialize)]
    struct Tui {
        preview: PreviewFile,
    }
    toml::from_str::<Document>(body)
        .expect("a [tui.preview] table")
        .tui
        .preview
}

/// `body` merged over the built-in settings and rules.
fn merged(body: &str) -> (PreviewConfig, Vec<String>) {
    let mut config = PreviewConfig::default();
    let problems = merge(&mut config, file(body));
    (config, problems)
}

fn names(config: &PreviewConfig) -> Vec<&str> {
    config.rules.iter().map(|rule| rule.name.as_str()).collect()
}

const CLEAR_HINT: &str = "claude-code-clear-hint";

#[test]
fn the_built_in_document_has_no_problems_and_states_the_framework_defaults() {
    // Merged over a base that differs in every setting, so a setting the
    // document left out would show.
    let blank = PreviewConfig {
        smart: SmartWrap {
            decoration: Vec::new(),
            min_rule_run: 0,
            shorten_rules: false,
        },
        rules: Vec::new(),
    };
    let (builtin, problems) = builtin_over(blank);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(builtin.smart, SmartWrap::default());
    assert_eq!(builtin, PreviewConfig::default());
}

#[test]
fn the_built_in_clear_hint_rule_right_aligns_the_hint_in_claude_code_panes() {
    let config = PreviewConfig::default();
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.name == CLEAR_HINT)
        .expect("the built-in rule");
    assert_eq!(rule.harness.as_deref(), Some("claude-code"));
    assert_eq!(rule.fit, LineFit::RightAlign);
    let hint = format!("{}new task? /clear to save 494.6k tokens", " ".repeat(143));
    assert!(rule.pattern.is_match(&hint));
    assert!(rule.pattern.is_match(&format!("{hint}   ")));
    // The same words as content, or without the right-aligning padding,
    // are not the hint.
    assert!(
        !rule
            .pattern
            .is_match("new task? /clear to save 494.6k tokens")
    );
    assert!(
        !rule
            .pattern
            .is_match("  > new task? /clear to save 1k tokens and more")
    );
}

#[test]
fn settings_merge_key_by_key() {
    let (config, problems) = merged("[tui.preview]\nmin_rule_run = 3\n");
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(config.smart.min_rule_run, 3);
    assert_eq!(config.smart.decoration, SmartWrap::default().decoration);
    assert!(config.smart.shorten_rules);

    let (config, problems) = merged(
        "[tui.preview]\ndecoration = [\"*\", \"\\u2500..\\u257F\"]\nshorten_rules = false\n",
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(config.smart.decoration, vec!['*'..='*', '─'..='╿']);
    assert!(!config.smart.shorten_rules);
}

#[test]
fn a_bad_setting_is_reported_and_keeps_the_earlier_value() {
    let (config, problems) = merged(
        "[tui.preview]\nmin_rule_run = 1\nshorten_rules = \"yes\"\n\
         decoration = [\"*\", \"ab\"]\ncolour = 3\n",
    );
    assert_eq!(config.smart, SmartWrap::default());
    assert_eq!(problems.len(), 4, "{problems:?}");
    for (needle, problem) in [
        ("unknown `[tui.preview]` key `colour`", &problems[0]),
        ("`[tui.preview] decoration` entry `ab`", &problems[1]),
        ("`[tui.preview] min_rule_run` value `1`", &problems[2]),
        ("`[tui.preview] shorten_rules` value `yes`", &problems[3]),
    ] {
        assert!(problem.contains(needle), "{problem}");
    }
}

#[test]
fn a_decoration_entry_is_one_character_or_an_ordered_range() {
    assert_eq!(parse_range("-"), Some('-'..='-'));
    assert_eq!(parse_range("a..z"), Some('a'..='z'));
    assert_eq!(parse_range("...."), Some('.'..='.'));
    for bad in ["", "ab", "z..a", "a.z", "a..", "..", "a...z"] {
        assert_eq!(parse_range(bad), None, "{bad:?}");
    }
    let (_, problems) = merged("[tui.preview]\ndecoration = \"-\"\n");
    assert!(problems[0].contains("expected a list"), "{problems:?}");
}

#[test]
fn a_rule_of_a_new_name_is_checked_before_the_earlier_rules() {
    let (config, problems) =
        merged("[[tui.preview.rules]]\nname = \"banner\"\nmatch = '^=+ .* =+$'\nfit = \"cut\"\n");
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(names(&config), ["banner", CLEAR_HINT]);
    assert_eq!(config.rules[0].harness, None);
    assert_eq!(config.rules[0].fit, LineFit::Cut);
}

#[test]
fn a_rule_of_an_earlier_name_replaces_it_and_moves_ahead() {
    let mut config = PreviewConfig::default();
    merge(
        &mut config,
        file("[[tui.preview.rules]]\nname = \"other\"\nmatch = 'x'\nfit = \"wrap\"\n"),
    );
    assert_eq!(names(&config), ["other", CLEAR_HINT]);
    let problems = merge(
        &mut config,
        file(&format!(
            "[[tui.preview.rules]]\nname = \"{CLEAR_HINT}\"\nmatch = 'clear'\nfit = \"drop\"\n"
        )),
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(names(&config), [CLEAR_HINT, "other"]);
    assert_eq!(config.rules[0].fit, LineFit::Drop);
    // The replacement names no harness, so it applies to every pane.
    assert_eq!(config.rules[0].harness, None);
}

#[test]
fn a_rule_turned_off_by_name_is_gone() {
    let (config, problems) = merged(&format!(
        "[[tui.preview.rules]]\nname = \"{CLEAR_HINT}\"\nenabled = false\n"
    ));
    assert!(problems.is_empty(), "{problems:?}");
    assert!(config.rules.is_empty());
}

#[test]
fn a_name_given_twice_in_one_file_keeps_the_later_rule() {
    let (config, problems) = merged(
        "[[tui.preview.rules]]\nname = \"a\"\nmatch = 'x'\nfit = \"cut\"\n\
         [[tui.preview.rules]]\nname = \"a\"\nmatch = 'y'\nfit = \"wrap\"\n",
    );
    assert_eq!(names(&config), ["a", CLEAR_HINT]);
    assert_eq!(config.rules[0].fit, LineFit::Wrap);
    assert!(problems[0].contains("names `a` twice"), "{problems:?}");
}

#[test]
fn a_bad_rule_is_reported_and_skipped() {
    let cases = [
        (
            "match = 'x'\nfit = \"cut\"",
            "entry 1 skipped: it needs a `name`",
        ),
        (
            "name = \"r\"\nfit = \"cut\"",
            "rule `r` skipped: it needs a `match`",
        ),
        (
            "name = \"r\"\nmatch = 'x'",
            "rule `r` skipped: it needs a `fit`",
        ),
        (
            "name = \"r\"\nmatch = '('\nfit = \"cut\"",
            "rule `r` skipped: its `match` unclosed group",
        ),
        (
            "name = \"r\"\nmatch = 'x'\nfit = \"chomp\"",
            "its `fit` `chomp` is not one of smart, cut, wrap, right-align, drop",
        ),
        (
            "name = \"r\"\nmatch = 'x'\nfit = \"cut\"\nharness = \"claude\"",
            "its `harness` `claude` is not one of",
        ),
        (
            "name = \"r\"\nmatch = 'x'\nfit = \"cut\"\nharnes = \"codex\"",
            "unknown key `harnes`",
        ),
        (
            "name = \"r\"\nmatch = 'x'\nfit = \"cut\"\nenabled = \"no\"",
            "its `enabled` must be true or false",
        ),
    ];
    for (entry, needle) in cases {
        let (config, problems) = merged(&format!("[[tui.preview.rules]]\n{entry}\n"));
        assert_eq!(names(&config), [CLEAR_HINT], "{entry}");
        assert_eq!(problems.len(), 1, "{entry}: {problems:?}");
        assert!(problems[0].contains(needle), "{entry}: {}", problems[0]);
    }
    let (_, problems) = merged("[tui.preview]\nrules = 3\n");
    assert!(
        problems[0].contains("expected `[[tui.preview.rules]]`"),
        "{problems:?}"
    );
}

#[test]
fn every_fit_reads_back_from_its_token() {
    for fit in LineFit::ALL {
        assert_eq!(parse_fit(fit_token(fit)), Some(fit));
    }
    assert_eq!(parse_fit("chomp"), None);
}

#[test]
fn a_rule_naming_a_harness_applies_only_to_panes_running_it() {
    let (config, _) = merged(
        "[[tui.preview.rules]]\nname = \"any\"\nmatch = 'x'\nfit = \"cut\"\n\
         [[tui.preview.rules]]\nname = \"codex\"\nmatch = 'y'\nfit = \"drop\"\nharness = \"codex\"\n",
    );
    let running =
        |keys: &[&str]| -> BTreeSet<String> { keys.iter().map(|key| (*key).to_string()).collect() };
    let rules = |keys: &[&str]| -> Vec<String> {
        config
            .rules_for(&running(keys))
            .iter()
            .map(|rule| rule.name.clone())
            .collect()
    };
    assert_eq!(rules(&[]), ["any"]);
    assert_eq!(rules(&["codex"]), ["any", "codex"]);
    assert_eq!(rules(&["claude-code"]), ["any", CLEAR_HINT]);
}

#[test]
fn the_first_matching_rule_decides_a_lines_fit() {
    let (config, _) = merged(
        "[[tui.preview.rules]]\nname = \"a\"\nmatch = 'x'\nfit = \"cut\"\n\
         [[tui.preview.rules]]\nname = \"b\"\nmatch = 'x|y'\nfit = \"drop\"\n",
    );
    let rules = config.rules_for(&BTreeSet::new());
    assert_eq!(line_fit(&rules, "a x"), LineFit::Cut);
    assert_eq!(line_fit(&rules, "a y"), LineFit::Drop);
    assert_eq!(line_fit(&rules, "neither"), LineFit::Smart);
}

#[test]
fn a_panes_harnesses_come_from_its_sessions_processes_and_commands() {
    use crate::model::{
        AgentSessionId, GraphLink, LinkEndpoint, MuxSessionNode, Provenance, RuntimeProcessId,
        RuntimeProcessNode,
    };
    let mux = MuxSessionId::new("work");
    let mux_node = NodeId::MuxSession(mux.clone());
    let snapshot = |pane: Option<&str>, start: Option<&str>, links: Vec<GraphLink>| {
        let mut snapshot = GraphSnapshot::empty();
        let mut node = MuxSessionNode::new(mux.clone(), "tmux".to_string(), "work".to_string());
        node.active_pane_command = pane.map(str::to_string);
        node.active_pane_start_command = start.map(str::to_string);
        snapshot.nodes.push(GraphNode::MuxSession(node));
        snapshot
            .nodes
            .push(GraphNode::RuntimeProcess(RuntimeProcessNode {
                id: RuntimeProcessId::new("proc:1"),
                observation_key: "proc:1".to_string(),
                pid: None,
                parent_pid: None,
                root_pane_pid: None,
                command: None,
                cwd: None,
                harness_key: Some("codex".to_string()),
                role: None,
                depth: None,
                observed_epoch: None,
            }));
        snapshot.candidate_links = links;
        snapshot
    };
    let keys = |set: BTreeSet<String>| set.into_iter().collect::<Vec<_>>();

    // A shell with nothing linked runs no harness.
    assert!(pane_harnesses(&snapshot(Some("zsh"), None, Vec::new()), &mux).is_empty());

    let session = GraphLink::new(
        "session-mux",
        NodeId::AgentSession(AgentSessionId::new("claude-code", "global", "s1")),
        LinkEndpoint::Node {
            id: mux_node.clone(),
        },
        RelationKind::LinkedToMux,
        Provenance::Discovered,
    );
    let process = GraphLink::new(
        "mux-process",
        mux_node,
        LinkEndpoint::Node {
            id: NodeId::RuntimeProcess(RuntimeProcessId::new("proc:1")),
        },
        RelationKind::MuxContainsProcess,
        Provenance::Discovered,
    );
    assert_eq!(
        keys(pane_harnesses(
            &snapshot(None, None, vec![session.clone()]),
            &mux
        )),
        ["claude-code"]
    );
    assert_eq!(
        keys(pane_harnesses(&snapshot(None, None, vec![process]), &mux)),
        ["codex"]
    );
    // A wrapper's launch command names the harness after its own words.
    assert_eq!(
        keys(pane_harnesses(
            &snapshot(
                Some("node"),
                Some("atelier exec claude --resume"),
                Vec::new()
            ),
            &mux
        )),
        ["claude-code"]
    );
    // A link that is not active says nothing about the pane.
    let mut ended = session;
    ended.state = LinkState::Ignored { reason: None };
    assert!(pane_harnesses(&snapshot(None, None, vec![ended]), &mux).is_empty());
}
