//! The probe's reading of every pane's title from its pane listing (ADRs
//! 0217, 0221). Every title here is synthetic.

use std::time::Duration;

use super::*;
use crate::discovery::DiscoveryContext;
use crate::live_store::{LiveEntry, LiveFile, LiveTmux, PidProbe, PidStatus};
use crate::model::live::{LiveObservation, LiveSource, LiveState};

/// Planted in titles: it must never leave the parser.
const SENTINEL: &str = "sentinel-title-7f3a91";

/// One pane of a server: its session, its id, its program, and its
/// title. A session's first pane here is its active pane.
#[derive(Clone, Copy)]
struct Pane<'a>(&'a str, &'a str, &'a str, &'a str);

/// The session listing of `panes`: one row per session, describing its
/// active pane.
fn sessions(panes: &[Pane<'_>]) -> String {
    let mut seen = Vec::new();
    let mut out = String::new();
    for Pane(session, pane, command, _) in panes {
        if seen.contains(session) {
            continue;
        }
        seen.push(*session);
        out.push_str(&format!(
            "{session}\t/work\t1\t1\t{command}\t42\t/work\t{command}\t0\t\t\t$1\t{pane}\t/tmp/tmux-1000/default\n"
        ));
    }
    out
}

/// The pane listing of `panes`, titles included.
fn pane_rows(panes: &[Pane<'_>]) -> String {
    panes
        .iter()
        .map(|Pane(session, pane, command, title)| {
            format!("{pane}\t{session}\t{command}\t{title}\n")
        })
        .collect()
}

/// A server that lists `panes`.
fn server(panes: &[Pane<'_>]) -> FakeTmux {
    FakeTmux::with_sessions(sessions(panes))
        .with_panes(None, TmuxOutcome::Sessions(pane_rows(panes)))
}

#[test]
fn the_parser_keeps_a_title_reading_panes_mark_and_drops_its_title() {
    let rows = parse_list_panes(&pane_rows(&[
        Pane("cx", "%0", "codex", "[ ! ] Action Required fix | proj"),
        Pane("cx", "%4", "codex-rs", "⠙ fix | proj"),
        Pane("cx", "%5", "codex", ""),
        Pane("cx", "%6", "node", "⠙ fix | proj"),
        Pane("cx", "%1", "zsh", "[ ! ] Action Required"),
        Pane("cx", "%2", "claude", "⠙ fix"),
    ]));
    let marks: Vec<_> = rows
        .iter()
        .map(|row| (row.pane_id.as_str(), row.title))
        .collect();
    assert_eq!(
        marks,
        [
            ("%0", Some(TitleMark::Waiting)),
            ("%4", Some(TitleMark::Working)),
            ("%5", Some(TitleMark::Neither)),
            // A Codex launcher's title is read as Codex's.
            ("%6", Some(TitleMark::Working)),
            ("%1", None),
            // Claude Code's titles are not read.
            ("%2", None),
        ]
    );
    assert_eq!(rows[0].command.as_deref(), Some("codex"));
}

#[test]
fn a_tab_in_a_title_shifts_no_field() {
    let rows = parse_list_panes("%0\tcx\tcodex\t⠋ a\tb\tc\n");
    assert_eq!(rows[0].title, Some(TitleMark::Working));
    assert_eq!(rows[0].command.as_deref(), Some("codex"));
    assert_eq!(rows[0].session, "cx");
}

#[test]
fn rows_without_the_title_still_parse() {
    let rows = parse_list_panes("%0\tcx\tcodex\n");
    assert_eq!(rows[0].command.as_deref(), Some("codex"));
    assert_eq!(rows[0].title, None);
}

#[test]
fn the_formats_differ_only_by_the_title_and_the_session_listing_reads_none() {
    assert_eq!(
        TMUX_PANES_FORMAT,
        format!("{TMUX_PANES_FORMAT_WITHOUT_TITLES}\t#{{pane_title}}")
    );
    assert_eq!(TMUX_PANES_FORMAT.split('\t').count(), TMUX_PANES_FIELDS);
    assert!(!TMUX_LIST_FORMAT.contains("pane_title"));
    assert!(TMUX_LIST_FORMAT.ends_with("\t#{session_id}\t#{pane_id}\t#{socket_path}"));
    assert_eq!(TMUX_LIST_FORMAT.split('\t').count(), TMUX_LIST_FIELDS);
}

/// A runner with one session that records the pane formats it was asked
/// for.
#[derive(Default)]
struct FormatRunner {
    asked: std::sync::Mutex<Vec<String>>,
}

impl MuxBackend for FormatRunner {
    fn backend_key(&self) -> &'static str {
        TMUX_BACKEND
    }

    fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
        Ok(TmuxOutcome::Sessions("cx\t/work\t1\t1\n".to_string()))
    }

    fn list_panes_on(&self, _socket: Option<&str>, format: &str) -> Result<TmuxOutcome> {
        self.asked.lock().unwrap().push(format.to_string());
        Ok(TmuxOutcome::Sessions(String::new()))
    }
}

#[test]
fn a_probe_without_titles_does_not_ask_for_them() {
    let reading = TmuxDiscovery::with_runner(FormatRunner::default());
    reading.discover(&DiscoveryContext::default()).unwrap();
    assert_eq!(*reading.runner.asked.lock().unwrap(), [TMUX_PANES_FORMAT]);

    let blind = TmuxDiscovery::with_runner(FormatRunner::default()).without_pane_titles();
    blind.discover(&DiscoveryContext::default()).unwrap();
    assert_eq!(
        *blind.runner.asked.lock().unwrap(),
        [TMUX_PANES_FORMAT_WITHOUT_TITLES]
    );

    // A backend that answers with a title anyway is not read either, and
    // the panes are still listed.
    let fragment = TmuxDiscovery::with_runner(server(&[Pane(
        "cx",
        "%0",
        "codex",
        "[ ! ] Action Required",
    )]))
    .without_pane_titles()
    .discover(&DiscoveryContext::default())
    .unwrap();
    assert!(titles(&fragment).is_empty());
    assert_eq!(
        fragment.mux_probes["tmux"].pane_command("cx", "%0"),
        Some(Some("codex"))
    );
}

fn titles(fragment: &GraphFragment) -> BTreeMap<String, PaneTitle> {
    fragment.mux_probes["tmux"].titles.clone()
}

#[test]
fn the_probe_records_every_title_reading_panes_mark_and_nothing_of_the_title() {
    let waiting = format!("[ ! ] Action Required {SENTINEL}");
    let spinning = format!("⠋ {SENTINEL}");
    // Codex runs in the second, non-active pane of `cx`; `np` runs a
    // launcher Codex may run under.
    let panes = [
        Pane("cx", "%0", "zsh", &spinning),
        Pane("cx", "%1", "codex", &waiting),
        Pane("np", "%2", "node", &spinning),
        Pane("sh", "%3", "zsh", &waiting),
    ];
    let before = crate::live_store::now_ms();
    let fragment = TmuxDiscovery::with_runner(server(&panes))
        .discover(&DiscoveryContext::default())
        .unwrap();
    let titles = titles(&fragment);
    assert_eq!(titles.keys().collect::<Vec<_>>(), ["%1", "%2"]);
    let cx = &titles["%1"];
    assert_eq!(
        (cx.pane.as_deref(), cx.mark, cx.previous_ms),
        (Some("%1"), TitleMark::Waiting, None)
    );
    assert_eq!(titles["%2"].mark, TitleMark::Working);
    assert!(cx.observed_ms >= before);
    assert_eq!(cx.since_ms, cx.observed_ms);
    let probe = &fragment.mux_probes["tmux"];
    assert!(probe.panes["%0"].active && !probe.panes["%1"].active);
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

/// `server` lists `panes`.
fn lists(server: &ScriptedTmux, panes: &[Pane<'_>]) {
    server.lists_with_panes(&sessions(panes), &pane_rows(panes));
}

#[test]
fn a_mark_held_across_probes_keeps_its_start_and_the_probe_before() {
    let server = ScriptedTmux::default();
    let discovery = TmuxDiscovery::with_runner(server.clone());
    let context = DiscoveryContext::default();
    let shell = Pane("cx", "%0", "zsh", "");

    lists(&server, &[shell, Pane("cx", "%2", "codex", "⠋ fix")]);
    let first = probe(&discovery, &context)["%2"].clone();
    // A later frame of the spinner is the same mark.
    lists(&server, &[shell, Pane("cx", "%2", "codex", "⠼ fix")]);
    let second = probe(&discovery, &context)["%2"].clone();
    assert_eq!(second.since_ms, first.observed_ms);
    assert_eq!(second.previous_ms, Some(first.observed_ms));
    assert!(second.observed_ms > first.observed_ms);
    assert!(second.held_twice_after(first.observed_ms - 1));
    assert!(!second.held_twice_after(first.observed_ms));

    // A listing served from the cache is the same probe.
    let cached = titles(&discovery.discover(&context).unwrap())["%2"].clone();
    assert_eq!(cached, second);

    // Another mark starts a new run.
    lists(
        &server,
        &[
            shell,
            Pane("cx", "%2", "codex", "[ . ] Action Required fix"),
        ],
    );
    let third = probe(&discovery, &context)["%2"].clone();
    assert_eq!((third.mark, third.previous_ms), (TitleMark::Waiting, None));
    assert_eq!(third.since_ms, third.observed_ms);

    // Another pane has a run of its own.
    lists(
        &server,
        &[
            shell,
            Pane("cx", "%3", "codex", "[ ! ] Action Required fix"),
        ],
    );
    assert_eq!(probe(&discovery, &context)["%3"].previous_ms, None);

    // And a probe that does not list the pane breaks the run.
    lists(&server, &[Pane("other", "%9", "zsh", "")]);
    assert!(probe(&discovery, &context).is_empty());
    lists(
        &server,
        &[
            shell,
            Pane("cx", "%3", "codex", "[ ! ] Action Required fix"),
        ],
    );
    assert_eq!(probe(&discovery, &context)["%3"].previous_ms, None);

    // As does a server that stopped.
    server.stops();
    assert!(probe(&discovery, &context).is_empty());
}

/// A runner whose every server lists the same panes.
struct EveryServer(Vec<Pane<'static>>);

impl MuxBackend for EveryServer {
    fn backend_key(&self) -> &'static str {
        TMUX_BACKEND
    }

    fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
        Ok(TmuxOutcome::Sessions(sessions(&self.0)))
    }

    fn list_sessions_on(&self, _socket: Option<&str>, _format: &str) -> Result<TmuxOutcome> {
        Ok(TmuxOutcome::Sessions(sessions(&self.0)))
    }

    fn list_panes_on(&self, _socket: Option<&str>, _format: &str) -> Result<TmuxOutcome> {
        Ok(TmuxOutcome::Sessions(pane_rows(&self.0)))
    }
}

#[test]
fn each_server_keeps_its_own_runs() {
    let discovery =
        TmuxDiscovery::with_runner(EveryServer(vec![Pane("cx", "%0", "codex", "⠋ fix")]));
    let context = DiscoveryContext::default();
    discovery.discover(&context).unwrap();
    // A server a history names is probed too, and a pane of the same id
    // on it starts a run of its own.
    let context = context.with_mux_sockets(["scratch".to_string()]);
    std::thread::sleep(Duration::from_millis(2));
    context.caches().clear_mux();
    let fragment = discovery.discover(&context).unwrap();
    assert!(
        fragment.mux_probes["tmux"].titles["%0"]
            .previous_ms
            .is_some()
    );
    assert_eq!(
        fragment.mux_probes["tmux:scratch"].titles["%0"].previous_ms,
        None
    );
}

/// A process table where only pid 4242 runs.
struct OnlyCodex;

impl PidProbe for OnlyCodex {
    fn status(&self, pid: i64) -> PidStatus {
        if pid == 4242 {
            PidStatus::Running { start: Some(77) }
        } else {
            PidStatus::Gone
        }
    }
}

#[test]
fn a_title_leaves_no_trace_in_the_graph_its_mark_reaches() {
    let waiting = format!("[ ! ] Action Required {SENTINEL}");
    let spinning = format!("⠋ {SENTINEL}");
    let panes = [
        Pane("cx", "%0", "zsh", &waiting),
        Pane("cx", "%1", "codex", &waiting),
        Pane("np", "%2", "node", &spinning),
        Pane("hk", "%3", "node", &waiting),
        Pane("sh", "%4", "zsh", &waiting),
    ];
    let fragment = TmuxDiscovery::with_runner(server(&panes))
        .discover(&DiscoveryContext::default())
        .unwrap();
    let mut snapshot = crate::discovery::merge_fragments([fragment]);
    // A live Codex hook entry, gone quiet, names the launcher pane of
    // `hk`; nothing names the one of `np`.
    let mut entry = LiveEntry::new("codex", "s-hk", 1_000);
    entry.observation =
        LiveObservation::new(LiveState::Working, LiveSource::Hook, 1_000).with_pid(4242, Some(77));
    entry.tmux = Some(LiveTmux {
        socket: Some("/tmp/tmux-1000/default".to_string()),
        session: Some("hk".to_string()),
        pane: Some("%3".to_string()),
    });
    entry.rev = 1;
    let mut store = LiveFile::default();
    store.rev = 1;
    store.entries.insert(entry.key(), entry);
    let bound = crate::discovery::live_state::bind_live(&snapshot, &store, &OnlyCodex);
    snapshot.node_live = bound.nodes;
    snapshot.pane_live = bound.panes;

    // The marks reached the live tables: the Codex pane that is not
    // active, and the launcher pane its hook entry names.
    let pane = |mux: &str, pane: &str| {
        crate::model::LivePane::new(
            MuxSessionId::new(format!("tmux:{mux}")),
            Some(pane.to_string()),
        )
    };
    let read = |mux: &str, id: &str| {
        let binding = snapshot.pane_live.get(&pane(mux, id)).expect("bound");
        (binding.observation.state, binding.observation.source)
    };
    assert_eq!(
        read("cx", "%1"),
        (LiveState::Waiting, LiveSource::PaneTitle)
    );
    assert_eq!(
        read("hk", "%3"),
        (LiveState::Waiting, LiveSource::PaneTitle)
    );
    assert_eq!(snapshot.pane_live.len(), 2, "{:?}", snapshot.pane_live);

    // The titles did not.
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
