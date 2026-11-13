//! The probe's reading of the active panes' titles (ADR 0217). Every
//! title here is synthetic.

use std::time::Duration;

use super::*;
use crate::discovery::DiscoveryContext;
use crate::live_store::{LiveFile, PidProbe, PidStatus};
use crate::model::live::{LiveSource, LiveState};

/// Planted in titles: it must never leave the parser.
const SENTINEL: &str = "sentinel-title-7f3a91";

/// A listing row for session `name`, whose active pane `pane` runs
/// `command` under the title `title`.
fn row(name: &str, command: &str, pane: &str, title: &str) -> String {
    format!(
        "{name}\t/work\t1\t1\t{command}\t42\t/work\t{command}\t0\t\t\t$1\t{pane}\t/tmp/tmux-1000/default\t{title}\n"
    )
}

#[test]
fn the_parser_keeps_a_harness_panes_mark_and_drops_its_title() {
    let rows = parse_list_sessions(&format!(
        "{}{}{}{}",
        row("cx", "codex", "%0", "[ ! ] Action Required fix | proj"),
        row("rs", "codex-rs", "%4", "⠙ fix | proj"),
        row("idle", "codex", "%5", ""),
        row("sh", "zsh", "%1", "[ ! ] Action Required"),
    ));
    let marks: Vec<_> = rows
        .iter()
        .map(|row| {
            (
                row.name.as_str(),
                row.active_pane_id.as_deref(),
                row.active_pane_title,
            )
        })
        .collect();
    assert_eq!(
        marks,
        [
            ("cx", Some("%0"), Some(TitleMark::Waiting)),
            ("rs", Some("%4"), Some(TitleMark::Working)),
            ("idle", Some("%5"), Some(TitleMark::Neither)),
            ("sh", Some("%1"), None),
        ]
    );
    assert_eq!(rows[0].session_id.as_deref(), Some("$1"));
    assert_eq!(rows[0].active_pane_command.as_deref(), Some("codex"));
}

#[test]
fn a_tab_in_a_title_shifts_no_field() {
    let rows = parse_list_sessions(&row("cx", "codex", "%0", "⠋ a\tb\tc"));
    assert_eq!(rows[0].active_pane_title, Some(TitleMark::Working));
    assert_eq!(rows[0].active_pane_id.as_deref(), Some("%0"));
    assert_eq!(rows[0].session_id.as_deref(), Some("$1"));
}

#[test]
fn rows_without_the_pane_fields_still_parse() {
    let older = "cx\t/work\t1\t1\tcodex\t42\t/work\tcodex\t0\t\t\t$1";
    let rows = parse_list_sessions(older);
    assert_eq!(rows[0].active_pane_id, None);
    assert_eq!(rows[0].active_pane_title, None);
    // The format without titles ends at the socket path, and a row
    // from before it at the pane id.
    let untitled = format!("{older}\t%0\t/tmp/tmux-1000/default");
    let rows = parse_list_sessions(&untitled);
    assert_eq!(rows[0].active_pane_id.as_deref(), Some("%0"));
    assert_eq!(
        rows[0].socket_path.as_deref(),
        Some("/tmp/tmux-1000/default")
    );
    assert_eq!(rows[0].active_pane_title, None);
    let rows = parse_list_sessions(&format!("{older}\t%0"));
    assert_eq!(rows[0].active_pane_id.as_deref(), Some("%0"));
    assert_eq!(rows[0].socket_path, None);
}

#[test]
fn the_formats_differ_only_by_the_title() {
    assert_eq!(
        TMUX_LIST_FORMAT,
        format!("{TMUX_LIST_FORMAT_WITHOUT_TITLES}\t#{{pane_title}}")
    );
    assert!(
        TMUX_LIST_FORMAT_WITHOUT_TITLES.ends_with("\t#{session_id}\t#{pane_id}\t#{socket_path}")
    );
    assert_eq!(TMUX_LIST_FORMAT.split('\t').count(), TMUX_LIST_FIELDS);
}

/// A runner that records the formats it was asked for.
#[derive(Default)]
struct FormatRunner {
    asked: std::sync::Mutex<Vec<String>>,
}

impl MuxBackend for FormatRunner {
    fn backend_key(&self) -> &'static str {
        TMUX_BACKEND
    }

    fn list_sessions(&self, format: &str) -> Result<TmuxOutcome> {
        self.asked.lock().unwrap().push(format.to_string());
        Ok(TmuxOutcome::Sessions(String::new()))
    }
}

#[test]
fn a_probe_without_titles_does_not_ask_for_them() {
    let reading = TmuxDiscovery::with_runner(FormatRunner::default());
    reading.rows().unwrap();
    assert_eq!(*reading.runner.asked.lock().unwrap(), [TMUX_LIST_FORMAT]);

    let blind = TmuxDiscovery::with_runner(FormatRunner::default()).without_pane_titles();
    blind.rows().unwrap();
    assert_eq!(
        *blind.runner.asked.lock().unwrap(),
        [TMUX_LIST_FORMAT_WITHOUT_TITLES]
    );

    // A backend that answers with a title anyway is not read either.
    let listing = row("cx", "codex", "%0", "[ ! ] Action Required");
    let fragment = TmuxDiscovery::with_runner(FakeTmux::with_sessions(listing))
        .without_pane_titles()
        .discover(&DiscoveryContext::default())
        .unwrap();
    assert!(titles(&fragment).is_empty());
}

fn titles(fragment: &GraphFragment) -> BTreeMap<String, PaneTitle> {
    fragment.mux_probes["tmux"].titles.clone()
}

#[test]
fn the_probe_records_each_harness_panes_mark_and_nothing_of_the_title() {
    let listing = format!(
        "{}{}",
        row(
            "cx",
            "codex",
            "%0",
            &format!("[ ! ] Action Required {SENTINEL}")
        ),
        row("sh", "zsh", "%1", &format!("⠋ {SENTINEL}")),
    );
    let before = crate::live_store::now_ms();
    let fragment = TmuxDiscovery::with_runner(FakeTmux::with_sessions(listing))
        .discover(&DiscoveryContext::default())
        .unwrap();
    let titles = titles(&fragment);
    assert_eq!(titles.keys().collect::<Vec<_>>(), ["cx"]);
    let cx = &titles["cx"];
    assert_eq!(
        (cx.pane.as_deref(), cx.mark, cx.previous_ms),
        (Some("%0"), TitleMark::Waiting, None)
    );
    assert!(cx.observed_ms >= before);
    assert_eq!(cx.since_ms, cx.observed_ms);
    assert!(!format!("{fragment:?}").contains(SENTINEL));
}

/// One probe of `server`, past the listing cache, at least a millisecond
/// after the last.
fn probe(discovery: &TmuxDiscovery<ScriptedTmux>, context: &DiscoveryContext) -> PaneTitles {
    std::thread::sleep(Duration::from_millis(2));
    context.caches().clear_mux();
    titles(&discovery.discover(context).unwrap())
}

type PaneTitles = BTreeMap<String, PaneTitle>;

#[test]
fn a_mark_held_across_probes_keeps_its_start_and_the_probe_before() {
    let server = ScriptedTmux::default();
    let discovery = TmuxDiscovery::with_runner(server.clone());
    let context = DiscoveryContext::default();

    server.lists(&row("cx", "codex", "%0", "⠋ fix"));
    let first = probe(&discovery, &context)["cx"].clone();
    // A later frame of the spinner is the same mark.
    server.lists(&row("cx", "codex", "%0", "⠼ fix"));
    let second = probe(&discovery, &context)["cx"].clone();
    assert_eq!(second.since_ms, first.observed_ms);
    assert_eq!(second.previous_ms, Some(first.observed_ms));
    assert!(second.observed_ms > first.observed_ms);
    assert!(second.held_twice_after(first.observed_ms - 1));
    assert!(!second.held_twice_after(first.observed_ms));

    // A listing served from the cache is the same probe.
    let cached = titles(&discovery.discover(&context).unwrap())["cx"].clone();
    assert_eq!(cached, second);

    // Another mark starts a new run.
    server.lists(&row("cx", "codex", "%0", "[ . ] Action Required fix"));
    let third = probe(&discovery, &context)["cx"].clone();
    assert_eq!((third.mark, third.previous_ms), (TitleMark::Waiting, None));
    assert_eq!(third.since_ms, third.observed_ms);

    // So does another pane.
    server.lists(&row("cx", "codex", "%3", "[ ! ] Action Required fix"));
    let fourth = probe(&discovery, &context)["cx"].clone();
    assert_eq!(fourth.previous_ms, None);

    // And a probe that does not list the session breaks the run.
    server.lists(&row("other", "zsh", "%9", ""));
    assert!(probe(&discovery, &context).is_empty());
    server.lists(&row("cx", "codex", "%3", "[ ! ] Action Required fix"));
    assert_eq!(probe(&discovery, &context)["cx"].previous_ms, None);

    // As does a server that stopped.
    server.stops();
    assert!(probe(&discovery, &context).is_empty());
}

/// A runner whose every server lists the same sessions.
struct EveryServer(String);

impl MuxBackend for EveryServer {
    fn backend_key(&self) -> &'static str {
        TMUX_BACKEND
    }

    fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
        Ok(TmuxOutcome::Sessions(self.0.clone()))
    }

    fn list_sessions_on(&self, _socket: Option<&str>, _format: &str) -> Result<TmuxOutcome> {
        Ok(TmuxOutcome::Sessions(self.0.clone()))
    }
}

#[test]
fn each_server_keeps_its_own_runs() {
    let discovery = TmuxDiscovery::with_runner(EveryServer(row("cx", "codex", "%0", "⠋ fix")));
    let context = DiscoveryContext::default();
    discovery.discover(&context).unwrap();
    // A server a history names is probed too, and a session of the same
    // name on it starts a run of its own.
    let context = context.with_mux_sockets(["scratch".to_string()]);
    std::thread::sleep(Duration::from_millis(2));
    context.caches().clear_mux();
    let fragment = discovery.discover(&context).unwrap();
    assert!(
        fragment.mux_probes["tmux"].titles["cx"]
            .previous_ms
            .is_some()
    );
    assert_eq!(
        fragment.mux_probes["tmux:scratch"].titles["cx"].previous_ms,
        None
    );
}

/// A process table with no process in it.
struct NoProcesses;

impl PidProbe for NoProcesses {
    fn status(&self, _pid: i64) -> PidStatus {
        PidStatus::Gone
    }
}

#[test]
fn a_title_leaves_no_trace_in_the_graph_its_mark_reaches() {
    let listing = format!(
        "{}{}",
        row(
            "cx",
            "codex",
            "%0",
            &format!("[ ! ] Action Required {SENTINEL}")
        ),
        row(
            "sh",
            "zsh",
            "%1",
            &format!("[ ! ] Action Required {SENTINEL}")
        ),
    );
    let fragment = TmuxDiscovery::with_runner(FakeTmux::with_sessions(listing))
        .discover(&DiscoveryContext::default())
        .unwrap();
    let mut snapshot = crate::discovery::merge_fragments([fragment]);
    let bound =
        crate::discovery::live_state::bind_live(&snapshot, &LiveFile::default(), &NoProcesses);
    snapshot.node_live = bound.nodes;
    snapshot.pane_live = bound.panes;

    // The mark reached the live tables, and only the Codex pane's.
    let pane = crate::model::LivePane::new(MuxSessionId::new("tmux:cx"), Some("%0".to_string()));
    let binding = snapshot.pane_live.get(&pane).expect("bound");
    assert_eq!(
        (binding.observation.state, binding.observation.source),
        (LiveState::Waiting, LiveSource::PaneTitle)
    );
    assert_eq!(snapshot.pane_live.len(), 1);

    // The title did not.
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(!json.contains(SENTINEL), "JSON");
    assert!(!format!("{snapshot:?}").contains(SENTINEL), "Debug");
    let bytes = crate::snapshot::serialize_to_bytes(&snapshot).unwrap();
    assert!(
        !bytes
            .windows(SENTINEL.len())
            .any(|window| window == SENTINEL.as_bytes()),
        "graph.bin bytes"
    );
    assert!(snapshot.diagnostics.is_empty());
    // The marks ride in graph.bin with the probe and come back.
    let back = crate::snapshot::from_bytes(&bytes).unwrap();
    assert_eq!(back.mux_probes, snapshot.mux_probes);
    assert_eq!(back.pane_live, snapshot.pane_live);
}
