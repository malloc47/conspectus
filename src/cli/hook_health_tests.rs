use super::*;
use crate::live_store::LiveEntry;
use crate::model::{RuntimeProcessId, RuntimeProcessNode};

const NOW_MS: i64 = 1_790_769_600_000;

fn file(entries: &[(&str, &str, i64)]) -> LiveFile {
    let mut file = LiveFile::default();
    for (harness, session, observed_ms) in entries {
        let entry = LiveEntry::new(harness, session, *observed_ms);
        file.entries.insert(entry.key(), entry);
    }
    file
}

fn never_asked() -> usize {
    panic!("the running agents are only needed for an installed, silent hook")
}

#[test]
fn entries_for_the_harness_mean_its_hooks_report_whatever_the_install_says() {
    let store = file(&[
        ("claude-code", "a", NOW_MS - 90_000),
        ("claude-code", "b", NOW_MS - 12_000),
        ("codex", "c", NOW_MS),
    ]);
    let receiving = Health::Receiving {
        sessions: 2,
        newest_ms: NOW_MS - 12_000,
    };
    for installed in [true, false] {
        assert_eq!(
            Health::assess("claude-code", true, installed, &store, never_asked),
            receiving
        );
    }
    assert_eq!(receiving.label(), "receiving");
    assert_eq!(
        receiving.message(NOW_MS, "conspectus hook init claude-code"),
        "live state: 2 sessions, newest event 12s ago"
    );
    let single = Health::Receiving {
        sessions: 1,
        newest_ms: NOW_MS + 5_000,
    };
    assert_eq!(
        single.message(NOW_MS, ""),
        "live state: 1 session, newest event just now"
    );
}

#[test]
fn an_installed_hook_with_a_running_agent_and_no_entries_is_silent() {
    let store = file(&[("codex", "c", NOW_MS)]);
    let silent = Health::assess("claude-code", true, true, &store, || 1);
    assert_eq!(silent, Health::Silent);
    assert_eq!(silent.label(), "silent");
    assert_eq!(
        silent.message(NOW_MS, ""),
        "live state: active but no events: hooks may be untrusted, disabled, or the agent \
         was started before they were installed"
    );

    let idle = Health::assess("claude-code", true, true, &store, || 0);
    assert_eq!(idle, Health::NoEvents);
    assert_eq!(idle.label(), "no-events");
}

#[test]
fn a_hook_not_installed_says_how_to_install_it() {
    let off = Health::assess(
        "claude-code",
        true,
        false,
        &LiveFile::default(),
        never_asked,
    );
    assert_eq!(off, Health::Off);
    assert_eq!(off.label(), "off");
    assert_eq!(
        off.message(NOW_MS, "csp hook init claude-code --scope project"),
        "live state: off; `csp hook init claude-code --scope project` installs the hooks \
         that report it"
    );
}

#[test]
fn a_hook_that_reports_no_live_state_is_never_called_silent() {
    for installed in [true, false] {
        let health = Health::assess("aider", false, installed, &LiveFile::default(), never_asked);
        assert_eq!(health, Health::Unsupported);
        assert_eq!(health.label(), "unsupported");
    }
}

#[test]
fn the_claude_code_codex_opencode_and_pi_hooks_report_live_state() {
    let reports = |harness: &str| {
        crate::discovery::harness::registered_adapters()
            .find(|adapter| adapter.harness_key() == harness)
            .is_some_and(crate::discovery::harness::HarnessAdapter::reports_live_state)
    };
    assert!(reports("claude-code"));
    assert!(reports("codex"));
    assert!(reports("opencode"));
    assert!(reports("pi"));
    assert!(!reports("aider"));
}

struct Table(&'static [i64]);

impl PidProbe for Table {
    fn status(&self, pid: i64) -> PidStatus {
        if self.0.contains(&pid) {
            PidStatus::Running { start: None }
        } else {
            PidStatus::Gone
        }
    }
}

fn process(key: &str, harness: &str, pid: i64, role: RuntimeProcessRole) -> GraphNode {
    GraphNode::RuntimeProcess(RuntimeProcessNode {
        id: RuntimeProcessId::new(key),
        observation_key: key.to_string(),
        pid: Some(pid),
        parent_pid: None,
        root_pane_pid: None,
        command: None,
        cwd: None,
        harness_key: Some(harness.to_string()),
        role: Some(role),
        depth: None,
        observed_epoch: None,
    })
}

#[test]
fn running_agents_counts_main_agents_of_the_harness_still_running() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            process("main", "claude-code", 10, RuntimeProcessRole::HumanAgent),
            // The same process seen through a second pane.
            process("again", "claude-code", 10, RuntimeProcessRole::HumanAgent),
            process("child", "claude-code", 11, RuntimeProcessRole::Subagent),
            process("other", "codex", 12, RuntimeProcessRole::HumanAgent),
            process("exited", "claude-code", 13, RuntimeProcessRole::HumanAgent),
        ],
        ..GraphSnapshot::empty()
    };
    let probe = Table(&[10, 11, 12]);
    assert_eq!(running_agents(&snapshot, "claude-code", &probe), 1);
    assert_eq!(running_agents(&snapshot, "codex", &probe), 1);
    assert_eq!(running_agents(&snapshot, "pi", &probe), 0);
    assert_eq!(running_agents(&snapshot, "claude-code", &Table(&[])), 0);
}

#[test]
fn only_codex_pane_titles_are_read_for_now() {
    let reads = |harness: &str| {
        crate::discovery::harness::registered_adapters()
            .find(|adapter| adapter.harness_key() == harness)
            .is_some_and(crate::discovery::harness::reads_pane_titles)
    };
    assert!(reads("codex"));
    for harness in ["claude-code", "opencode", "pi", "aider"] {
        assert!(!reads(harness), "{harness}");
    }
}

#[test]
fn title_agents_counts_the_harnesss_bindings_read_from_pane_titles() {
    use crate::model::live::{LiveBinding, LiveObservation, LiveSource, LiveState, Liveness};
    use crate::model::{AgentSessionId, LivePane, MuxSessionId, MuxSessionNode};

    let mux = |name: &str, command: &str| {
        let mut node = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", name);
        node.active_pane_command = Some(command.to_string());
        GraphNode::MuxSession(node)
    };
    let binding = |source| {
        LiveBinding::new(
            LiveObservation::new(LiveState::Waiting, source, NOW_MS),
            Liveness::Unknown,
        )
    };
    let pane = |name: &str| LivePane::new(MuxSessionId::new(format!("tmux:{name}")), None);
    let session = |harness: &str, key: &str| {
        NodeId::AgentSession(AgentSessionId::new(harness, "/scope", key))
    };
    let mut snapshot = GraphSnapshot {
        nodes: vec![mux("cx", "codex"), mux("hooked", "codex"), mux("sh", "zsh")],
        ..GraphSnapshot::empty()
    };
    snapshot
        .pane_live
        .insert(pane("cx"), binding(LiveSource::PaneTitle));
    snapshot
        .pane_live
        .insert(pane("hooked"), binding(LiveSource::Hook));
    snapshot
        .pane_live
        .insert(pane("sh"), binding(LiveSource::PaneTitle));
    snapshot
        .node_live
        .insert(session("codex", "a"), binding(LiveSource::PaneTitle));
    snapshot
        .node_live
        .insert(session("codex", "b"), binding(LiveSource::Hook));
    snapshot
        .node_live
        .insert(session("pi", "c"), binding(LiveSource::PaneTitle));

    assert_eq!(title_agents(&snapshot, "codex"), 2);
    assert_eq!(title_agents(&snapshot, "claude-code"), 0);
    assert_eq!(title_agents(&GraphSnapshot::empty(), "codex"), 0);
}

#[test]
fn the_line_says_how_many_agents_their_pane_titles_show() {
    assert_eq!(title_note(0), None);
    assert_eq!(
        title_note(1).as_deref(),
        Some("1 agent shown from its pane title")
    );
    assert_eq!(
        title_note(3).as_deref(),
        Some("3 agents shown from their pane titles")
    );
}
