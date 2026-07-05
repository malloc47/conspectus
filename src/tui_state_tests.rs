// Extracted from tui_state.rs H-HYG-011 rolling wave via #[path = "tui_state_tests.rs"] mod tests;
use super::*;
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
