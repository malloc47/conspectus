use std::fs;

use super::*;
use crate::model::{
    ArgvSource, CheckoutRecipe, HistoryKind, HistorySnapshot, HostedSession, MuxIncarnation,
    MuxRecipe, PrRecipe, RecordDisplay, RecordOrigin,
};

fn mux_obs(name: &str, session_id: &str, created: i64) -> MuxObservation {
    MuxObservation {
        key: format!("mux_session:tmux:{name}"),
        recipe: MuxRecipe {
            backend: "tmux".into(),
            name: name.into(),
            cwd: Some("/src/app".into()),
            incarnation: Some(MuxIncarnation {
                session_id: Some(session_id.into()),
                created: Some(created),
            }),
            ..Default::default()
        },
        display: RecordDisplay {
            label: name.into(),
            context: None,
        },
        origin: RecordOrigin::Observed,
    }
}

fn hosted(id: &str, first: i64, last: i64) -> HostedSession {
    HostedSession {
        harness: "claude-code".into(),
        session_id: id.into(),
        first_seen: first,
        last_seen: last,
    }
}

fn observations(seen_at: i64, muxes: Vec<MuxObservation>) -> Observations {
    Observations {
        seen_at,
        muxes,
        ..Default::default()
    }
}

fn checkout_obs(root: &str, branch: &str, head: &str) -> CheckoutObservation {
    CheckoutObservation {
        key: format!("checkout:repo:/src/app/.git@{root}"),
        recipe: CheckoutRecipe {
            repo: "repo:/src/app/.git".into(),
            root: root.into(),
            linked: true,
            branch: Some(branch.into()),
            head: Some(head.into()),
            ..Default::default()
        },
        display: RecordDisplay {
            label: root.into(),
            context: None,
        },
        ..Default::default()
    }
}

#[test]
fn a_new_observation_becomes_a_record_with_a_stable_id_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::at(dir.path());
    let (load, outcome) = store
        .record(&observations(1_000, vec![mux_obs("main", "$1", 900)]))
        .unwrap();
    assert_eq!(outcome.changed.len(), 1);
    let record = &load.history.muxes[0];
    assert!(record.meta.id.starts_with("h_") && record.meta.id.len() == 14);
    assert_eq!(record.meta.first_seen, 1_000);
    assert_eq!(outcome.mux_ids, vec![record.meta.id.clone()]);

    let text = fs::read_to_string(dir.path().join("muxes.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema_version"], HISTORY_SCHEMA_VERSION);
    assert_eq!(value["records"][0]["id"], record.meta.id.as_str());

    let again = store.load();
    assert_eq!(again.history.muxes, load.history.muxes);
}

#[test]
fn unknown_fields_survive_a_rewrite() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("muxes.json"),
        r#"{
          "schema_version": 1,
          "future_top": {"x": 1},
          "records": [{
            "id": "h_000000000001",
            "key": "mux_session:tmux:main",
            "first_seen": 100,
            "last_seen": 100,
            "future_record": "kept",
            "recipe": {"backend": "tmux", "name": "main", "future_recipe": [1, 2]}
          }]
        }"#,
    )
    .unwrap();
    let store = HistoryStore::at(dir.path());
    let (load, _) = store
        .record(&observations(5_000, vec![mux_obs("main", "$1", 4_000)]))
        .unwrap();
    assert_eq!(
        load.history.muxes.len(),
        1,
        "matched by key, not duplicated"
    );

    let text = fs::read_to_string(dir.path().join("muxes.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["future_top"]["x"], 1);
    assert_eq!(value["records"][0]["future_record"], "kept");
    assert_eq!(value["records"][0]["recipe"]["future_recipe"][1], 2);
    assert_eq!(value["records"][0]["last_seen"], 5_000);
}

/// Records compared without their random ids, with `last_seen`
/// allowed to differ within the rewrite cadence.
fn assert_same_history(a: &HistorySnapshot, b: &HistorySnapshot) {
    assert_eq!(a.muxes.len(), b.muxes.len());
    for (x, y) in a.muxes.iter().zip(&b.muxes) {
        assert_eq!(x.meta.key, y.meta.key);
        assert_eq!(x.meta.first_seen, y.meta.first_seen);
        assert!((x.meta.last_seen - y.meta.last_seen).abs() < LAST_SEEN_CADENCE_SECS);
        assert_eq!(x.recipe, y.recipe);
    }
    assert_eq!(a.checkouts.len(), b.checkouts.len());
    for (x, y) in a.checkouts.iter().zip(&b.checkouts) {
        assert_eq!(x.meta.key, y.meta.key);
        assert_eq!(x.meta.first_seen, y.meta.first_seen);
        assert_eq!(x.recipe, y.recipe);
    }
}

#[test]
fn merging_in_either_order_agrees() {
    let mut a = mux_obs("main", "$1", 900);
    a.recipe.hosted = vec![hosted("s1", 1_000, 1_000)];
    a.recipe.cwd = Some("/src/old".into());
    let mut b = mux_obs("main", "$1", 900);
    b.recipe.hosted = vec![hosted("s2", 2_000, 2_000), hosted("s1", 1_500, 1_900)];
    b.recipe.cwd = Some("/src/new".into());
    let first = Observations {
        seen_at: 1_000,
        muxes: vec![a, mux_obs("other", "$2", 950)],
        checkouts: vec![checkout_obs("/wt/a", "feature", "aaa")],
        ..Default::default()
    };
    let second = Observations {
        seen_at: 2_000,
        muxes: vec![b],
        checkouts: vec![checkout_obs("/wt/a", "feature", "bbb")],
        ..Default::default()
    };

    let mut forward = HistorySnapshot::default();
    merge::merge(&mut forward, &first);
    merge::merge(&mut forward, &second);
    let mut backward = HistorySnapshot::default();
    merge::merge(&mut backward, &second);
    merge::merge(&mut backward, &first);
    sort(&mut forward);
    sort(&mut backward);

    assert_same_history(&forward, &backward);
    let main = forward
        .muxes
        .iter()
        .find(|r| r.recipe.name == "main")
        .unwrap();
    assert_eq!(
        main.recipe.cwd.as_deref(),
        Some("/src/new"),
        "newest sighting wins"
    );
    assert_eq!(main.meta.first_seen, 1_000);
    assert_eq!(main.meta.last_seen, 2_000);
    let ids: Vec<_> = main
        .recipe
        .hosted
        .iter()
        .map(|h| h.session_id.as_str())
        .collect();
    assert_eq!(ids, ["s2", "s1"], "most recent first");
    assert_eq!(main.recipe.hosted[1].first_seen, 1_000);
    assert_eq!(forward.checkouts[0].recipe.head.as_deref(), Some("bbb"));
}

fn sort(history: &mut HistorySnapshot) {
    history.muxes.sort_by(|a, b| a.meta.key.cmp(&b.meta.key));
    history
        .checkouts
        .sort_by(|a, b| a.meta.key.cmp(&b.meta.key));
}

#[test]
fn a_writer_with_an_old_copy_cannot_undo_a_reap() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::at(dir.path());
    store
        .record(&observations(
            1_000,
            vec![mux_obs("old", "$1", 900), mux_obs("keep", "$2", 900)],
        ))
        .unwrap();
    let stale_copy = store.load();
    assert_eq!(stale_copy.history.muxes.len(), 2);

    store
        .update(|history| {
            history.muxes.retain(|r| r.recipe.name != "old");
            merge::MergeOutcome::changed(HistoryKind::Mux)
        })
        .unwrap();

    // The other writer only saw `keep` this cycle. It merges that
    // observation into what is on disk now, not into its old copy.
    let (load, _) = store
        .record(&observations(2_000, vec![mux_obs("keep", "$2", 900)]))
        .unwrap();
    let names: Vec<_> = load
        .history
        .muxes
        .iter()
        .map(|r| r.recipe.name.as_str())
        .collect();
    assert_eq!(names, ["keep"]);

    // A fresh sighting of the reaped mux brings it back, as a new record.
    let old_id = stale_copy
        .history
        .muxes
        .iter()
        .find(|r| r.recipe.name == "old")
        .unwrap()
        .meta
        .id
        .clone();
    let (load, _) = store
        .record(&observations(3_000, vec![mux_obs("old", "$1", 900)]))
        .unwrap();
    let revived = load
        .history
        .muxes
        .iter()
        .find(|r| r.recipe.name == "old")
        .unwrap();
    assert_ne!(revived.meta.id, old_id);
    assert_eq!(revived.meta.first_seen, 3_000);
}

#[test]
fn an_unchanged_live_record_is_rewritten_at_most_every_ten_minutes() {
    let mut history = HistorySnapshot::default();
    merge::merge(
        &mut history,
        &observations(1_000, vec![mux_obs("main", "$1", 900)]),
    );

    let outcome = merge::merge(
        &mut history,
        &observations(1_300, vec![mux_obs("main", "$1", 900)]),
    );
    assert!(
        outcome.changed.is_empty(),
        "five minutes later, nothing to write"
    );
    assert_eq!(history.muxes[0].meta.last_seen, 1_000);

    let outcome = merge::merge(
        &mut history,
        &observations(
            1_000 + LAST_SEEN_CADENCE_SECS,
            vec![mux_obs("main", "$1", 900)],
        ),
    );
    assert!(outcome.changed.contains(&HistoryKind::Mux));
    assert_eq!(
        history.muxes[0].meta.last_seen,
        1_000 + LAST_SEEN_CADENCE_SECS
    );

    let mut moved = mux_obs("main", "$1", 900);
    moved.recipe.cwd = Some("/elsewhere".into());
    let at = 1_000 + LAST_SEEN_CADENCE_SECS + 10;
    let outcome = merge::merge(&mut history, &observations(at, vec![moved]));
    assert!(
        outcome.changed.contains(&HistoryKind::Mux),
        "a changed field writes at once"
    );
    assert_eq!(history.muxes[0].meta.last_seen, at);
}

#[test]
fn an_outside_rename_follows_the_record_by_incarnation() {
    let mut history = HistorySnapshot::default();
    merge::merge(
        &mut history,
        &observations(1_000, vec![mux_obs("main", "$1", 900)]),
    );
    let id = history.muxes[0].meta.id.clone();

    merge::merge(
        &mut history,
        &observations(2_000, vec![mux_obs("renamed", "$1", 900)]),
    );
    assert_eq!(history.muxes.len(), 1);
    assert_eq!(history.muxes[0].meta.id, id);
    assert_eq!(history.muxes[0].recipe.name, "renamed");
    assert_eq!(history.muxes[0].meta.key, "mux_session:tmux:renamed");
}

#[test]
fn a_recreated_name_relives_its_record_without_its_old_sessions() {
    let mut history = HistorySnapshot::default();
    let mut first = mux_obs("main", "$1", 900);
    first.recipe.hosted = vec![hosted("old-session", 950, 1_000)];
    merge::merge(&mut history, &observations(1_000, vec![first]));
    let id = history.muxes[0].meta.id.clone();

    // The server restarted: same name, new incarnation.
    let mut second = mux_obs("main", "$0", 5_000);
    second.recipe.hosted = vec![hosted("new-session", 5_100, 5_200)];
    merge::merge(&mut history, &observations(5_200, vec![second]));
    assert_eq!(history.muxes.len(), 1);
    let record = &history.muxes[0];
    assert_eq!(record.meta.id, id);
    assert_eq!(
        record.recipe.incarnation.as_ref().unwrap().created,
        Some(5_000)
    );
    let ids: Vec<_> = record
        .recipe
        .hosted
        .iter()
        .map(|h| h.session_id.as_str())
        .collect();
    assert_eq!(ids, ["new-session"]);
}

#[test]
fn a_rename_and_a_new_session_under_the_old_name_land_on_the_right_records() {
    let mut history = HistorySnapshot::default();
    merge::merge(
        &mut history,
        &observations(1_000, vec![mux_obs("main", "$1", 900)]),
    );
    let id = history.muxes[0].meta.id.clone();

    // `$1` was renamed to `work`, and a new `main` (`$2`) was created.
    // The new `main` is listed first, which must not steal `$1`'s record.
    merge::merge(
        &mut history,
        &observations(
            2_000,
            vec![mux_obs("main", "$2", 1_900), mux_obs("work", "$1", 900)],
        ),
    );
    assert_eq!(history.muxes.len(), 2);
    let work = history
        .muxes
        .iter()
        .find(|r| r.recipe.name == "work")
        .unwrap();
    assert_eq!(work.meta.id, id);
    let main = history
        .muxes
        .iter()
        .find(|r| r.recipe.name == "main")
        .unwrap();
    assert_ne!(main.meta.id, id);
}

#[test]
fn hosted_sessions_keep_the_ten_most_recent() {
    let mut history = HistorySnapshot::default();
    let mut obs = mux_obs("main", "$1", 900);
    obs.recipe.hosted = (0..15).map(|i| hosted(&format!("s{i:02}"), i, i)).collect();
    merge::merge(&mut history, &observations(1_000, vec![obs]));
    let hosted = &history.muxes[0].recipe.hosted;
    assert_eq!(hosted.len(), crate::model::HOSTED_SESSIONS_KEPT);
    assert_eq!(hosted[0].session_id, "s14");
    assert_eq!(hosted.last().unwrap().session_id, "s05");
}

#[test]
fn an_observed_start_command_never_replaces_the_argv_conspectus_launched_with() {
    let mut history = HistorySnapshot::default();
    let mut launched = mux_obs("main", "$1", 900);
    launched.origin = RecordOrigin::Launched;
    launched.recipe.argv = Some(vec!["atelier".into(), "exec".into(), "claude".into()]);
    launched.recipe.argv_source = Some(ArgvSource::Conspectus);
    merge::merge(&mut history, &observations(1_000, vec![launched]));

    let mut observed = mux_obs("main", "$1", 900);
    observed.recipe.argv = Some(vec!["atelier exec claude".into()]);
    observed.recipe.argv_source = Some(ArgvSource::Observed);
    merge::merge(&mut history, &observations(2_000, vec![observed]));

    let recipe = &history.muxes[0].recipe;
    assert_eq!(recipe.argv_source, Some(ArgvSource::Conspectus));
    assert_eq!(recipe.argv.as_ref().unwrap().len(), 3);
    assert_eq!(history.muxes[0].meta.origin, RecordOrigin::Launched);
}

#[test]
fn concurrent_writers_never_lose_each_others_observations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let writers: Vec<_> = (0..8)
        .map(|writer| {
            let path = path.clone();
            std::thread::spawn(move || {
                let store = HistoryStore::at(&path);
                for round in 0..10 {
                    let name = format!("w{writer}-r{round}");
                    store
                        .record(&observations(
                            1_000 + round,
                            vec![mux_obs(&name, &format!("${writer}{round}"), 900)],
                        ))
                        .unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let load = HistoryStore::at(&path).load();
    assert!(load.problems.is_empty(), "{:?}", load.problems);
    assert_eq!(load.history.muxes.len(), 80);
    let ids: std::collections::BTreeSet<_> = load
        .history
        .muxes
        .iter()
        .map(|r| r.meta.id.clone())
        .collect();
    assert_eq!(ids.len(), 80, "ids stay unique across writers");
}

#[test]
fn a_writer_waits_for_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    store::with_lock(&path, || {
        let path = path.clone();
        let writer = std::thread::spawn(move || {
            HistoryStore::at(&path)
                .record(&observations(1_000, vec![mux_obs("main", "$1", 900)]))
                .unwrap();
            tx.send(()).unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(
            rx.try_recv().is_err(),
            "the writer must block while the lock is held"
        );
        writer
    })
    .unwrap()
    .join()
    .unwrap();
    rx.recv().unwrap();
    assert_eq!(HistoryStore::at(&path).load().history.muxes.len(), 1);
}

#[test]
fn a_file_from_a_newer_binary_is_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let newer = r#"{"schema_version": 2, "records": [{"anything": true}]}"#;
    fs::write(dir.path().join("muxes.json"), newer).unwrap();
    let store = HistoryStore::at(dir.path());
    let (load, _) = store
        .record(&observations(1_000, vec![mux_obs("main", "$1", 900)]))
        .unwrap();
    assert!(!load.is_writable(HistoryKind::Mux));
    assert_eq!(load.problems.len(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("muxes.json")).unwrap(),
        newer
    );
}

#[test]
fn a_malformed_record_makes_its_file_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let broken = r#"{"schema_version": 1, "records": [{"id": 5}]}"#;
    fs::write(dir.path().join("prs.json"), broken).unwrap();
    let store = HistoryStore::at(dir.path());
    let pr = PrObservation {
        key: "forge_pr:github:github.com/o/r#1".into(),
        recipe: PrRecipe {
            provider: "github".into(),
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
            number: 1,
            ..Default::default()
        },
        display: RecordDisplay::default(),
    };
    let (load, _) = store
        .record(&Observations {
            seen_at: 1_000,
            prs: vec![pr],
            ..Default::default()
        })
        .unwrap();
    assert!(!load.is_writable(HistoryKind::Pr));
    assert_eq!(
        fs::read_to_string(dir.path().join("prs.json")).unwrap(),
        broken
    );
}

#[test]
fn reading_a_missing_directory_is_empty_history() {
    let dir = tempfile::tempdir().unwrap();
    let load = HistoryStore::at(dir.path().join("nope")).load();
    assert!(load.history.is_empty());
    assert!(load.problems.is_empty());
    assert!(!dir.path().join("nope").exists(), "reading creates nothing");
}

#[test]
fn globs_match_stars_and_question_marks() {
    assert!(glob_matches("scratch*", "scratch-1"));
    assert!(glob_matches("*", ""));
    assert!(glob_matches("a?c", "abc"));
    assert!(glob_matches("*/tmp/*", "/home/me/tmp/x"));
    assert!(!glob_matches("scratch*", "my-scratch"));
    assert!(!glob_matches("a?c", "ac"));
}
