use super::*;
use std::path::PathBuf;
use tempfile::TempDir;

fn cache_in(dir: &TempDir) -> TuiStateCache {
    TuiStateCache::default().with_xdg_state_home(dir.path())
}

#[test]
fn read_returns_none_when_file_absent() {
    let dir = TempDir::new().expect("tempdir");
    assert!(read_last_view(&cache_in(&dir)).is_none());
}

#[test]
fn round_trip_persists_the_view() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    write_last_view(&cache, View::Mux).expect("write");
    assert_eq!(read_last_view(&cache), Some(View::Mux));
}

#[test]
fn write_skips_when_payload_matches() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    write_last_view(&cache, View::Prs).expect("first write");
    let mtime_before = mtime(&cache);
    // Sleep so a real overwrite would show up as a different
    // mtime on coarse-grained filesystems.
    std::thread::sleep(std::time::Duration::from_millis(50));
    write_last_view(&cache, View::Prs).expect("second write");
    let mtime_after = mtime(&cache);
    assert_eq!(
        mtime_before, mtime_after,
        "skip-on-unchanged must not touch the file"
    );
}

#[test]
fn malformed_json_collapses_to_none_on_read() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let path = cache.path().expect("path");
    fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
    fs::write(&path, "not json").expect("write garbage");
    assert!(read_last_view(&cache).is_none());
}

#[test]
fn unknown_view_value_collapses_to_none() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let path = cache.path().expect("path");
    fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
    fs::write(&path, r#"{"schema_version": 1, "last_view": "settings"}"#)
        .expect("write future view");
    assert!(read_last_view(&cache).is_none());
}

#[test]
fn state_saved_in_a_removed_view_costs_only_that_view() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let path = cache.path().expect("path");
    fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
    fs::write(
        &path,
        r#"{
            "schema_version": 1,
            "last_view": "union",
            "sort": "recency",
            "view_states": {
                "union": {"grouping": {"Union": "kind"}},
                "forks": {"grouping": {"Forks": "provider"}},
                "mux": {"grouping": {"Mux": "repo"}}
            }
        }"#,
    )
    .expect("write state from a build that had the views");

    assert!(read_last_view(&cache).is_none());
    let state = read_tui_state(&cache).expect("the rest of the state still loads");
    assert_eq!(state.last_view, None);
    assert_eq!(state.sort, Some(Sort::Recency));
    assert_eq!(
        state.view_states.keys().copied().collect::<Vec<_>>(),
        vec![View::Mux]
    );
}

#[test]
fn unknown_fields_round_trip_through_write() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let path = cache.path().expect("path");
    fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
    fs::write(
        &path,
        r#"{"schema_version": 2, "last_view": "mux", "extra_field": "preserved"}"#,
    )
    .expect("seed");
    write_last_view(&cache, View::Sessions).expect("write");
    let after = fs::read_to_string(&path).expect("read");
    assert!(
        after.contains("\"extra_field\""),
        "unknown field must round-trip: {after}",
    );
    assert!(
        after.contains("\"sessions\""),
        "new view must persist: {after}",
    );
}

#[test]
fn full_state_round_trips_sort_filter_and_view_slots() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let mut state = PersistedState {
        last_view: Some(View::Mux),
        sort: Some(Sort::Recency),
        mux_recency: None,
        preview_wrap: None,
        session_preview: None,
        view_states: BTreeMap::new(),
    };
    state.view_states.insert(
        View::Sessions,
        PersistedViewSlot {
            filter: RowFilter {
                harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
                max_age: Some(std::time::Duration::from_secs(60)),
                mux_state: None,
                ..RowFilter::default()
            },
            grouping: Some(Grouping::Sessions(crate::tui::SessionsGrouping::Workspace)),
            tree: PersistedTree::default(),
        },
    );

    write_tui_state(&cache, &state).expect("write full state");
    let read = read_tui_state(&cache).expect("read full state");

    assert_eq!(read.last_view, Some(View::Mux));
    assert_eq!(read.sort, Some(Sort::Recency));
    let sessions = read
        .view_states
        .get(&View::Sessions)
        .expect("sessions slot");
    assert_eq!(
        sessions.grouping,
        Some(Grouping::Sessions(crate::tui::SessionsGrouping::Workspace))
    );
    assert_eq!(
        sessions.filter.max_age,
        Some(std::time::Duration::from_secs(60))
    );
    assert_eq!(
        sessions.filter.harness,
        Some(crate::filter::HarnessFilter::from_values(["codex"]))
    );
}

#[test]
fn each_view_keeps_its_own_agent_state_filter_and_waiting_float() {
    use crate::filter::{LiveStateFilter, LiveStateKey};
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let mut state = PersistedState {
        last_view: Some(View::Sessions),
        sort: None,
        mux_recency: None,
        preview_wrap: None,
        session_preview: None,
        view_states: BTreeMap::new(),
    };
    let slot = |filter| PersistedViewSlot {
        filter,
        grouping: None,
        tree: PersistedTree::default(),
    };
    state.view_states.insert(
        View::Sessions,
        slot(RowFilter {
            float_waiting_top: true,
            ..RowFilter::default()
        }),
    );
    state.view_states.insert(
        View::Mux,
        slot(RowFilter {
            live_state: Some(LiveStateFilter::from_values([LiveStateKey::Waiting])),
            ..RowFilter::default()
        }),
    );

    write_tui_state(&cache, &state).expect("write");
    let text = fs::read_to_string(cache.path().expect("path")).expect("read text");
    assert!(text.contains("\"float_waiting_top\": true"), "{text}");
    assert!(text.contains("\"live_state\""), "{text}");
    let read = read_tui_state(&cache).expect("read");
    let filter = |view| &read.view_states.get(&view).expect("slot").filter;
    assert!(filter(View::Sessions).float_waiting_top);
    assert_eq!(filter(View::Sessions).live_state, None);
    assert!(!filter(View::Mux).float_waiting_top, "per view");
    assert_eq!(
        filter(View::Mux).live_state,
        Some(LiveStateFilter::from_values([LiveStateKey::Waiting]))
    );
}

#[test]
fn mux_recency_basis_round_trips() {
    // The mux recency basis survives a write/read
    // cycle so the operator's choice persists across restarts.
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let state = PersistedState {
        last_view: Some(View::Mux),
        sort: Some(Sort::Recency),
        mux_recency: Some(crate::tui::MuxRecency::LastAttached),
        preview_wrap: None,
        session_preview: None,
        view_states: BTreeMap::new(),
    };

    write_tui_state(&cache, &state).expect("write");
    let read = read_tui_state(&cache).expect("read");

    assert_eq!(read.mux_recency, Some(crate::tui::MuxRecency::LastAttached));
}

#[test]
fn preview_wrap_round_trips_and_ignores_unknown_tokens() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let state = PersistedState {
        last_view: None,
        sort: None,
        mux_recency: None,
        preview_wrap: Some(crate::tui::WrapMode::None),
        session_preview: None,
        view_states: BTreeMap::new(),
    };
    write_tui_state(&cache, &state).expect("write");
    let read = read_tui_state(&cache).expect("read");
    assert_eq!(read.preview_wrap, Some(crate::tui::WrapMode::None));

    let path = cache.path().expect("path");
    fs::write(&path, r#"{"schema_version": 1, "preview_wrap": "fancy"}"#).expect("write");
    let read = read_tui_state(&cache).expect("read");
    assert_eq!(read.preview_wrap, None);
}

#[test]
fn read_tui_state_accepts_compact_filter_objects() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let path = cache.path().expect("path");
    fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
    fs::write(
        &path,
        r#"{
              "schema_version": 1,
              "last_view": "sessions",
              "sort": "recency",
              "view_states": {
                "sessions": {
                  "filter": { "harness": { "any": ["codex"] } },
                  "grouping": { "Sessions": "workspace" }
                }
              }
            }"#,
    )
    .expect("seed compact state");

    let read = read_tui_state(&cache).expect("read compact state");
    let sessions = read
        .view_states
        .get(&View::Sessions)
        .expect("sessions slot");

    assert_eq!(read.sort, Some(Sort::Recency));
    assert_eq!(
        sessions.filter.harness,
        Some(crate::filter::HarnessFilter::from_values(["codex"]))
    );
    assert!(sessions.filter.max_age.is_none());
    assert!(!sessions.filter.float_muxed_sessions_top);
    assert_eq!(
        sessions.grouping,
        Some(Grouping::Sessions(crate::tui::SessionsGrouping::Workspace))
    );
}

#[test]
fn directory_prefers_xdg_state_home_then_falls_back_to_home() {
    let dir = TempDir::new().expect("tempdir");
    let with_xdg = TuiStateCache::default()
        .with_xdg_state_home(dir.path())
        .with_home(dir.path().join("home"));
    assert_eq!(
        with_xdg.directory(),
        Some(dir.path().join("conspectus")),
        "xdg wins",
    );
    let xdg_absent = TuiStateCache::default().with_home(dir.path().join("home"));
    assert_eq!(
        xdg_absent.directory(),
        Some(
            dir.path()
                .join("home")
                .join(".local")
                .join("state")
                .join("conspectus"),
        ),
    );
}

fn mtime(cache: &TuiStateCache) -> std::time::SystemTime {
    fs::metadata(cache.path().expect("path"))
        .expect("metadata")
        .modified()
        .expect("mtime")
}

#[test]
fn session_preview_round_trips_and_ignores_unknown_tokens() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let state = PersistedState {
        session_preview: Some(crate::tui::SessionPreview::Pane),
        ..PersistedState::default()
    };
    write_tui_state(&cache, &state).expect("write");
    let read = read_tui_state(&cache).expect("read");
    assert_eq!(read.session_preview, Some(crate::tui::SessionPreview::Pane));

    let path = cache.path().expect("path");
    fs::write(&path, r#"{"schema_version": 1, "session_preview": "both"}"#).expect("write");
    let read = read_tui_state(&cache).expect("read");
    assert_eq!(read.session_preview, None);
}

#[test]
fn one_unknown_nested_value_costs_only_that_setting() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let path = cache.path().expect("path");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{
          "schema_version": 1,
          "last_view": "mux",
          "sort": "recency",
          "future_key": {"x": 1},
          "view_states": {
            "pins": {"grouping": {"Pins": "x"}, "filter": "not a filter"},
            "sessions": {"grouping": {"Pins": "x"}}
          }
        }"#,
    )
    .unwrap();

    let state = read_tui_state(&cache).expect("the file still reads");
    assert_eq!(state.last_view, Some(View::Mux));
    assert_eq!(state.sort, Some(Sort::Recency));
    let sessions = state
        .view_states
        .get(&View::Sessions)
        .expect("sessions slot");
    assert_eq!(
        sessions.grouping,
        Some(Grouping::default_for(View::Sessions))
    );

    write_last_view(&cache, View::Prs).expect("write");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("future_key"),
        "unknown fields round-trip: {text}"
    );
    assert!(text.contains("\"sort\""), "{text}");
}

fn seed(cache: &TuiStateCache, text: &str) -> PathBuf {
    let path = cache.path().expect("path");
    fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
    fs::write(&path, text).expect("seed state");
    path
}

fn keys(keys: &[&str]) -> BTreeSet<String> {
    keys.iter().map(|key| (*key).to_string()).collect()
}

fn selection(row: &str, path: &[&str]) -> PersistedSelection {
    PersistedSelection {
        row: row.to_string(),
        path: path.iter().map(|key| (*key).to_string()).collect(),
    }
}

#[test]
fn expanded_rows_and_the_selection_round_trip_per_view() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let sessions = PersistedTree {
        expanded: Some(keys(&["group:workspace:/src", "group:repo:/src/app/.git"])),
        selected: Some(selection(
            "session:agent_session:codex:/state:s1",
            &["group:workspace:/src", "group:repo:/src/app/.git"],
        )),
    };
    let mux = PersistedTree {
        expanded: Some(BTreeSet::new()),
        selected: Some(selection("mux:mux_session:tmux:$3", &[])),
    };
    let mut state = PersistedState::default();
    for (view, tree) in [(View::Sessions, &sessions), (View::Mux, &mux)] {
        state.view_states.insert(
            view,
            PersistedViewSlot {
                tree: tree.clone(),
                ..PersistedViewSlot::default()
            },
        );
    }

    write_tui_state(&cache, &state).expect("write");
    let text = fs::read_to_string(cache.path().unwrap()).unwrap();
    assert!(text.contains("\"row_keys\": 1"), "{text}");
    let read = read_tui_state(&cache).expect("read");
    assert_eq!(read.view_states[&View::Sessions].tree, sessions);
    assert_eq!(
        read.view_states[&View::Mux].tree,
        mux,
        "an empty expanded list stays a list: every row collapsed"
    );
}

#[test]
fn a_file_from_an_earlier_build_remembers_no_rows() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    seed(
        &cache,
        r#"{
          "schema_version": 1,
          "last_view": "sessions",
          "view_states": {"sessions": {"grouping": {"Sessions": "workspace"}}}
        }"#,
    );
    let read = read_tui_state(&cache).expect("read");
    let sessions = &read.view_states[&View::Sessions];
    assert!(sessions.tree.is_empty());
    assert_eq!(
        sessions.grouping,
        Some(Grouping::Sessions(crate::tui::SessionsGrouping::Workspace))
    );

    let path = cache.path().unwrap();
    let mtime_before = mtime(&cache);
    std::thread::sleep(std::time::Duration::from_millis(50));
    write_tui_state(&cache, &read).expect("write back");
    assert_eq!(
        mtime(&cache),
        mtime_before,
        "nothing to remember adds no field, so the file is not rewritten"
    );
    assert!(!fs::read_to_string(path).unwrap().contains("row_keys"));
}

#[test]
fn a_row_key_that_is_not_text_costs_only_that_entry() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    seed(
        &cache,
        r#"{
          "schema_version": 1,
          "row_keys": 1,
          "view_states": {
            "sessions": {
              "expanded": ["group:workspace:/src", 3, null, {"x": 1}, "widget:unknown kind"],
              "selected": {"row": "group:workspace:/src", "path": ["bucket:a", 7]}
            },
            "mux": {"selected": {"path": ["group:workspace:/src"]}},
            "prs": {"selected": {"row": 12}}
          }
        }"#,
    );
    let read = read_tui_state(&cache).expect("read");
    let sessions = &read.view_states[&View::Sessions].tree;
    assert_eq!(
        sessions.expanded,
        Some(keys(&["group:workspace:/src", "widget:unknown kind"])),
        "an unknown kind is still a key; it just names no row"
    );
    assert_eq!(
        sessions.selected,
        Some(selection("group:workspace:/src", &["bucket:a"]))
    );
    assert_eq!(
        read.view_states[&View::Mux].tree.selected,
        None,
        "a selection without its row is no selection"
    );
    assert_eq!(read.view_states[&View::Prs].tree.selected, None);
}

#[test]
fn a_tree_field_of_the_wrong_shape_costs_only_that_views_field() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    seed(
        &cache,
        r#"{
          "schema_version": 1,
          "row_keys": 1,
          "sort": "recency",
          "view_states": {
            "sessions": {
              "grouping": {"Sessions": "workspace"},
              "expanded": "group:workspace:/src",
              "selected": ["group:workspace:/src"]
            },
            "mux": {
              "expanded": ["group:workspace:/src"],
              "selected": {"row": "mux:mux_session:tmux:$3", "path": "not a list"}
            }
          }
        }"#,
    );
    let read = read_tui_state(&cache).expect("the file still reads");
    assert_eq!(read.sort, Some(Sort::Recency));
    let sessions = &read.view_states[&View::Sessions];
    assert!(sessions.tree.is_empty());
    assert_eq!(
        sessions.grouping,
        Some(Grouping::Sessions(crate::tui::SessionsGrouping::Workspace))
    );
    let mux = &read.view_states[&View::Mux].tree;
    assert_eq!(mux.expanded, Some(keys(&["group:workspace:/src"])));
    assert_eq!(
        mux.selected,
        Some(selection("mux:mux_session:tmux:$3", &[])),
        "a path that is not a list leaves the row with nothing to fall back to"
    );
}

#[test]
fn another_row_key_encoding_drops_every_views_rows_and_keeps_the_rest() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    for row_keys in [r#""row_keys": 2,"#, r#""row_keys": "one","#, ""] {
        seed(
            &cache,
            &format!(
                r#"{{
                  "schema_version": 1,
                  {row_keys}
                  "view_states": {{
                    "sessions": {{
                      "grouping": {{"Sessions": "workspace"}},
                      "expanded": ["group:workspace:/src"],
                      "selected": {{"row": "group:workspace:/src", "path": []}}
                    }}
                  }}
                }}"#
            ),
        );
        let read = read_tui_state(&cache).expect("read");
        let sessions = &read.view_states[&View::Sessions];
        assert!(sessions.tree.is_empty(), "row_keys {row_keys:?}");
        assert_eq!(
            sessions.grouping,
            Some(Grouping::Sessions(crate::tui::SessionsGrouping::Workspace))
        );
    }
}

#[test]
fn unknown_fields_of_a_view_round_trip_through_a_write() {
    let dir = TempDir::new().expect("tempdir");
    let cache = cache_in(&dir);
    let path = seed(
        &cache,
        r#"{
          "schema_version": 1,
          "view_states": {
            "sessions": {"grouping": {"Sessions": "workspace"}, "future_field": {"kept": true}}
          }
        }"#,
    );
    let mut state = read_tui_state(&cache).expect("read");
    state
        .view_states
        .get_mut(&View::Sessions)
        .expect("sessions")
        .tree
        .selected = Some(selection("group:workspace:/src", &[]));
    write_tui_state(&cache, &state).expect("write");
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains("future_field"), "{text}");
    assert!(text.contains("\"selected\""), "{text}");
}

#[test]
fn every_kind_a_restored_view_lists_has_its_own_key() {
    use crate::model::{AgentSessionId, CheckoutId, ForgePrId, MuxSessionId, RepoId, WorkspaceId};
    let session = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "s1"));
    let mux = NodeId::MuxSession(MuxSessionId::new("tmux:$3"));
    let checkout = NodeId::Checkout(CheckoutId::new(RepoId::new("/src/app/.git"), "/src/app"));
    let pr = NodeId::ForgePr(ForgePrId::new("github", "github.com", "me", "app", 7));
    let cases = [
        (
            RowId::Group(NodeId::Workspace(WorkspaceId::new("/src"))),
            "group:workspace:/src",
        ),
        (
            RowId::Group(checkout),
            "group:checkout:repo:/src/app/.git@/src/app",
        ),
        (
            RowId::AgentSession(session.clone()),
            "session:agent_session:codex:/state:s1",
        ),
        (RowId::MuxSession(mux.clone()), "mux:mux_session:tmux:$3"),
        (RowId::Pr(pr), "pr:forge_pr:github:github.com/me/app#7"),
        (
            RowId::History {
                record_id: "h_1".into(),
            },
            "history:h_1",
        ),
        (
            RowId::Pin {
                pin_id: "work: main".into(),
            },
            "pin:work: main",
        ),
        (
            RowId::Nested {
                pin_id: "work".into(),
                node: mux.clone(),
            },
            "nested:work\nmux_session:tmux:$3",
        ),
        (RowId::Bucket("app".into()), "bucket:app"),
        (RowId::Remote("me/app".into()), "remote:me/app"),
        (RowId::Synthetic("ungrouped"), "synthetic:ungrouped"),
    ];
    for (id, key) in &cases {
        assert_eq!(row_key(id).as_deref(), Some(*key), "{id:?}");
    }
    let distinct: BTreeSet<_> = cases.iter().map(|(id, _)| row_key(id)).collect();
    assert_eq!(distinct.len(), cases.len());

    for search_only in [
        RowId::Hit { session, turn: 2 },
        RowId::Pane {
            mux: mux.clone(),
            pane: "%1".into(),
        },
        RowId::PaneHit {
            mux,
            pane: "%1".into(),
            line: 4,
        },
    ] {
        assert_eq!(row_key(&search_only), None, "{search_only:?}");
    }
}

#[test]
fn an_id_whose_text_reads_back_as_another_gets_no_key() {
    use crate::model::{AgentSessionId, CheckoutId, MuxSessionId, RepoId};
    // A scope with a colon reads back split in another place.
    let session = NodeId::AgentSession(AgentSessionId::new("codex", "a:b", "c"));
    assert_eq!(row_key(&RowId::AgentSession(session)), None);
    // A root with an `@` reads back with part of it in the repo.
    let checkout = NodeId::Checkout(CheckoutId::new(RepoId::new("/r"), "/x@y"));
    assert_eq!(row_key(&RowId::Group(checkout)), None);
    // Pin ids are one line, so a line break cannot be part of one.
    let nested = RowId::Nested {
        pin_id: "a\nb".into(),
        node: NodeId::MuxSession(MuxSessionId::new("tmux:$3")),
    };
    assert_eq!(row_key(&nested), None);
    // A colon in a pin id is fine: the line break ends it.
    let colon = |pin: &str, mux: &str| {
        row_key(&RowId::Nested {
            pin_id: pin.into(),
            node: NodeId::MuxSession(MuxSessionId::new(mux)),
        })
    };
    assert_ne!(colon("a:mux_session:b", "c"), colon("a", "b:mux_session:c"));
}
