use std::fs;

use super::*;
use crate::config::HistoryConfig;
use crate::history::HistoryStore;
use crate::legacy_bindings::{PinBindingRecord, to_json};
use crate::legacy_pins::{PinLaunch, PinMux, PinWorktree, parse_pins_document};
use crate::model::AgentSessionNode;

struct World {
    dir: tempfile::TempDir,
    slots: SlotStore,
    history: HistoryWriter,
    sidecars: PinBindingsCache,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    World {
        slots: SlotStore::at(dir.path().join("data/pins.toml")),
        history: HistoryWriter {
            store: HistoryStore::at(dir.path().join("state/history")),
            config: HistoryConfig::default(),
            home: None,
        },
        sidecars: PinBindingsCache::new().with_xdg_cache_home(dir.path().join("cache")),
        dir,
    }
}

impl World {
    fn targets(&self) -> Targets<'_> {
        Targets {
            slots: &self.slots,
            history: &self.history,
            user_config: None,
            fallback_epoch: 5_000,
        }
    }

    fn write_sidecar(&self, pin: &str, session: &str, epoch: i64) {
        let record = PinBindingRecord::new(pin, "mux", None, session, "claude-code", epoch);
        let path = self.sidecars.path_for(pin).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, to_json(&record).unwrap()).unwrap();
    }

    fn config(&self, name: &str, text: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        fs::write(&path, text).unwrap();
        path
    }

    fn entries(&self, path: &Path) -> Vec<LegacyEntry> {
        parse_pins_document(&fs::read_to_string(path).unwrap())
            .unwrap()
            .entries()
            .iter()
            .map(|entry| LegacyEntry {
                path: path.to_path_buf(),
                entry: entry.clone(),
            })
            .collect()
    }
}

fn entry(id: &str, display: &str, mux: &str) -> PinEntry {
    PinEntry {
        id: id.into(),
        display_name: display.into(),
        harness: "claude-code".into(),
        cwd: "/src/app".into(),
        mux: PinMux {
            backend: "tmux".into(),
            name: mux.into(),
            socket_name: None,
        },
        launch: None,
        worktree: None,
        reason: None,
    }
}

fn config_text(entries: &[PinEntry]) -> String {
    let mut text = String::from("[session]\npoll_ms = 250\n\n[pins]\nschema_version = 1\n");
    for e in entries {
        text.push_str(&format!(
            "\n[[pins.entries]]\nid = \"{}\"\ndisplay_name = \"{}\"\nharness = \"{}\"\ncwd = \"{}\"\n\
             [pins.entries.mux]\nbackend = \"tmux\"\nname = \"{}\"\n",
            e.id, e.display_name, e.harness, e.cwd, e.mux.name
        ));
        if let Some(PinLaunch { argv }) = &e.launch {
            let argv: Vec<String> = argv.iter().map(|a| format!("\"{a}\"")).collect();
            text.push_str(&format!(
                "[pins.entries.launch]\nargv = [{}]\n",
                argv.join(", ")
            ));
        }
        if let Some(PinWorktree { branch }) = &e.worktree {
            text.push_str(&format!("[pins.entries.worktree]\nbranch = \"{branch}\"\n"));
        }
    }
    text
}

fn empty_snapshot() -> GraphSnapshot {
    GraphSnapshot::empty()
}

#[test]
fn each_entry_becomes_one_mux_slot_that_keeps_its_id_and_points_at_a_record() {
    let w = world();
    let mut api = entry("api", "API work", "api");
    api.launch = Some(PinLaunch {
        argv: vec!["claude".into(), "--model".into(), "opus".into()],
    });
    let path = w.config(
        ".conspectus.toml",
        &config_text(&[api, entry("docs", "docs", "docs")]),
    );
    let plan = plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars);
    assert_eq!(plan.items.len(), 2);

    let report = run(&plan, &w.targets());

    assert_eq!(report.migrated, ["api", "docs"]);
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    let slots = w.slots.load().slots;
    let ids: Vec<_> = slots.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["api", "docs"]);
    let history = w.history.store.load().history;
    assert_eq!(history.muxes.len(), 2);
    for slot in &slots {
        assert_eq!(slot.kind, PinKind::Mux);
        let record = slot
            .target
            .as_deref()
            .and_then(|t| t.strip_prefix("history:"))
            .and_then(|id| history.mux(id))
            .expect("the slot points at a seeded record");
        assert_eq!(record.meta.origin, RecordOrigin::Migrated);
    }
    let api = &slots[0];
    assert_eq!(
        api.snapshot.get("label").and_then(FieldValue::as_text),
        Some("API work")
    );
    let record = history
        .muxes
        .iter()
        .find(|r| r.recipe.name == "api")
        .unwrap();
    assert_eq!(
        record.recipe.argv,
        Some(vec!["claude".into(), "--model".into(), "opus".into()])
    );
    assert_eq!(record.recipe.argv_source, Some(ArgvSource::Conspectus));
}

#[test]
fn a_slot_carries_overrides_only_for_what_the_record_does_not_say() {
    let w = world();
    let mut api = entry("api", "api", "api");
    api.worktree = Some(PinWorktree {
        branch: "feature/x".into(),
    });
    let path = w.config(".conspectus.toml", &config_text(&[api]));
    let plan = plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars);

    run(&plan, &w.targets());

    let slot = w.slots.load().slots.remove(0);
    assert_eq!(
        slot.overrides
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["worktree_branch"],
        "the record already says name, cwd, and harness"
    );
}

#[test]
fn a_sidecar_dates_the_record_and_keeps_the_last_session() {
    let w = world();
    w.write_sidecar("api", "sess-1", 1_234);
    let path = w.config(
        ".conspectus.toml",
        &config_text(&[entry("api", "api", "api")]),
    );
    let plan = plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars);
    assert_eq!(
        plan.items[0].last_session.as_ref().unwrap().session_id,
        "sess-1"
    );

    run(&plan, &w.targets());

    let history = w.history.store.load().history;
    let record = &history.muxes[0];
    assert_eq!(record.meta.first_seen, 1_234);
    let hosted = &record.recipe.hosted;
    assert_eq!(hosted.len(), 1);
    assert_eq!(
        (hosted[0].harness.as_str(), hosted[0].session_id.as_str()),
        ("claude-code", "sess-1")
    );
}

#[test]
fn without_a_sidecar_the_record_is_dated_by_the_fallback_epoch() {
    let w = world();
    let path = w.config(
        ".conspectus.toml",
        &config_text(&[entry("api", "api", "api")]),
    );
    let plan = plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars);

    run(&plan, &w.targets());

    let history = w.history.store.load().history;
    assert_eq!(history.muxes[0].meta.first_seen, 5_000);
    assert!(history.muxes[0].recipe.hosted.is_empty());
}

#[test]
fn converted_entries_leave_their_file_but_other_tables_stay() {
    let w = world();
    let path = w.config(
        ".conspectus.toml",
        &config_text(&[entry("a", "a", "a"), entry("b", "b", "b")]),
    );
    let plan = plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars);

    let report = run(&plan, &w.targets());

    assert_eq!(report.removed_entries, 2);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("poll_ms = 250"), "{text}");
    assert!(!text.contains("[pins"), "{text}");
}

#[test]
fn a_second_run_finds_nothing_to_do() {
    let w = world();
    let path = w.config(".conspectus.toml", &config_text(&[entry("a", "a", "a")]));
    run(
        &plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars),
        &w.targets(),
    );

    let again = plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars);

    assert!(again.is_empty(), "the entries are gone from the file");
    assert_eq!(w.slots.load().slots.len(), 1);
}

#[test]
fn an_entry_named_like_an_existing_slot_is_skipped_and_stays_put() {
    let w = world();
    let path = w.config(
        ".conspectus.toml",
        &config_text(&[entry("taken", "x", "x")]),
    );
    let existing = vec![PinSlot {
        id: "taken".into(),
        kind: PinKind::Mux,
        target: None,
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    }];

    let plan = plan(&empty_snapshot(), w.entries(&path), &existing, &w.sidecars);

    assert!(plan.items.is_empty());
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(plan.skipped[0].0, "taken");
    assert!(plan.render().contains("skipped"), "{}", plan.render());
    assert!(
        fs::read_to_string(&path).unwrap().contains("taken"),
        "nothing was touched"
    );
}

#[test]
fn a_write_failure_leaves_the_entry_in_its_file() {
    let w = world();
    let path = w.config(".conspectus.toml", &config_text(&[entry("a", "a", "a")]));
    let plan = plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars);
    // A directory where the slot file should be makes the write fail.
    fs::create_dir_all(w.dir.path().join("data/pins.toml")).unwrap();

    let report = run(&plan, &w.targets());

    assert!(report.migrated.is_empty());
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].0, "a");
    assert_eq!(report.removed_entries, 0);
    assert!(fs::read_to_string(&path).unwrap().contains("id = \"a\""));
}

#[test]
fn a_display_name_that_would_vanish_is_kept_as_a_session_alias() {
    let w = world();
    w.write_sidecar("api", "sess-1", 1_234);
    let mut session = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "scope", "sess-1"),
        "claude-code",
    );
    session.title = Some("Something else".into());
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::AgentSession(session)],
        ..GraphSnapshot::empty()
    };
    let path = w.config(
        ".conspectus.toml",
        &config_text(&[entry("api", "My API", "api")]),
    );
    let plan = plan(&snapshot, w.entries(&path), &[], &w.sidecars);
    assert_eq!(
        plan.items[0].alias.as_ref().map(|(_, n)| n.as_str()),
        Some("My API")
    );
    let user_config = w.dir.path().join("user.toml");

    let report = run(
        &plan,
        &Targets {
            user_config: Some(&user_config),
            ..w.targets()
        },
    );

    assert_eq!(report.aliases, ["My API"]);
    let aliases = fs::read_to_string(&user_config).unwrap();
    assert!(aliases.contains("My API"), "{aliases}");
}

#[test]
fn a_display_name_matching_the_mux_or_the_title_needs_no_alias() {
    let w = world();
    w.write_sidecar("same", "sess-1", 1_234);
    w.write_sidecar("titled", "sess-1", 1_234);
    let mut session = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "scope", "sess-1"),
        "claude-code",
    );
    session.title = Some("Fix parser".into());
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::AgentSession(session)],
        ..GraphSnapshot::empty()
    };
    let path = w.config(
        ".conspectus.toml",
        &config_text(&[
            entry("same", "same", "same"),
            entry("titled", "Fix parser", "other"),
        ]),
    );

    let plan = plan(&snapshot, w.entries(&path), &[], &w.sidecars);

    assert!(plan.items.iter().all(|item| item.alias.is_none()));
}

#[test]
fn the_dry_run_text_names_each_pin_and_its_file() {
    let w = world();
    w.write_sidecar("api", "sess-1", 1_234);
    let path = w.config(
        ".conspectus.toml",
        &config_text(&[entry("api", "api", "api")]),
    );
    let rendered = plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars).render();

    assert!(rendered.contains("pin `api` -> mux pin"), "{rendered}");
    assert!(rendered.contains("sess-1"), "{rendered}");
    assert!(rendered.contains(&path.display().to_string()), "{rendered}");
}

#[test]
fn a_run_interrupted_before_removal_is_finished_by_the_next_one() {
    let w = world();
    let text = config_text(&[entry("a", "a", "a"), entry("b", "b", "b")]);
    let path = w.config(".conspectus.toml", &text);
    run(
        &plan(&empty_snapshot(), w.entries(&path), &[], &w.sidecars),
        &w.targets(),
    );
    // The removal never happened: the entries are back.
    fs::write(&path, &text).unwrap();
    let snapshot = GraphSnapshot {
        history: w.history.store.load().history,
        ..GraphSnapshot::empty()
    };
    let existing = w.slots.load().slots;

    let again = plan(&snapshot, w.entries(&path), &existing, &w.sidecars);
    assert!(again.skipped.is_empty(), "{:?}", again.skipped);
    assert!(again.items.iter().all(|item| item.already_migrated));
    assert!(again.render().contains("already migrated"));
    let report = run(&again, &w.targets());

    assert_eq!(report.removed_entries, 2);
    assert_eq!(w.slots.load().slots.len(), 2, "no duplicate slots");
    assert_eq!(w.history.store.load().history.muxes.len(), 2);
    assert!(!fs::read_to_string(&path).unwrap().contains("[pins"));
}

#[test]
fn a_same_named_slot_that_is_not_ours_still_blocks() {
    let w = world();
    let path = w.config(".conspectus.toml", &config_text(&[entry("a", "a", "a")]));
    let existing = vec![PinSlot {
        id: "a".into(),
        kind: PinKind::Mux,
        target: Some("history:h_other".into()),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    }];

    let plan = plan(&empty_snapshot(), w.entries(&path), &existing, &w.sidecars);

    assert_eq!(plan.skipped.len(), 1);
    assert!(plan.items.is_empty());
}
