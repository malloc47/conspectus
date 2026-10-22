use super::*;
use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME, USER_CONFIG_RELATIVE};
use crate::model::{FieldValue, PinKind, PinSlot, SlotFields};
use tempfile::TempDir;

fn legacy_toml(id: &str, mux_name: &str) -> String {
    format!(
        r#"
[pins]
schema_version = 1

[[pins.entries]]
id = "{id}"
display_name = "{id}"
harness = "codex"
cwd = "/home/me/work/repo"
mux = {{ backend = "tmux", name = "{mux_name}" }}
"#,
    )
}

struct Fixture {
    _tmp: TempDir,
    home: PathBuf,
    xdg: PathBuf,
    root: PathBuf,
}

fn fixture() -> Fixture {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("mkdir root");
    Fixture {
        home,
        xdg,
        root,
        _tmp: tmp,
    }
}

impl Fixture {
    fn loader(&self) -> ConfigLoader {
        ConfigLoader::new()
            .with_home(self.home.clone())
            .with_xdg_config_home(self.xdg.clone())
    }

    fn context(&self) -> DiscoveryContext {
        DiscoveryContext::from_roots([self.root.clone()]).expect("ctx")
    }

    fn write_project(&self, text: &str) {
        fs::write(self.root.join(PROJECT_CONFIG_FILENAME), text).expect("write project config");
    }

    fn write_user(&self, text: &str) {
        let path = self.xdg.join(USER_CONFIG_RELATIVE);
        fs::create_dir_all(path.parent().unwrap()).expect("mkdir user config");
        fs::write(path, text).expect("write user config");
    }
}

fn slot(id: &str, order: u32) -> PinSlot {
    PinSlot {
        id: id.into(),
        kind: PinKind::Mux,
        target: None,
        pinned_at: None,
        overrides: SlotFields::from([("name".to_string(), FieldValue::Text(id.into()))]),
        snapshot: SlotFields::new(),
        order,
        legacy: false,
    }
}

fn legacy_diagnostics(snapshot: &GraphSnapshot) -> Vec<(u32, Vec<String>)> {
    snapshot
        .diagnostics
        .iter()
        .filter_map(|d| match d {
            Diagnostic::LegacyPins { count, ids } => Some((*count, ids.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn nothing_loads_without_a_store_or_legacy_config() {
    let fx = fixture();
    let loader = fx.loader();
    let mut snapshot = GraphSnapshot::empty();

    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: None,
            legacy_loader: Some(&loader),
            legacy_registry_stores: &[],
        },
    );

    assert!(snapshot.pins.is_empty());
    assert!(snapshot.diagnostics.is_empty());
}

#[test]
fn slots_load_in_file_order() {
    let fx = fixture();
    let store = SlotStore::at(fx.root.join("pins.toml"));
    store.add(slot("second", 0)).unwrap();
    store.add(slot("first", 0)).unwrap();
    let mut snapshot = GraphSnapshot::empty();

    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: Some(&store),
            legacy_loader: None,
            legacy_registry_stores: &[],
        },
    );

    let ids: Vec<_> = snapshot.pins.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["second", "first"]);
    assert!(snapshot.pins.iter().all(|p| !p.legacy));
    assert!(legacy_diagnostics(&snapshot).is_empty());
}

#[test]
fn a_bad_slot_is_reported_and_the_rest_still_load() {
    let fx = fixture();
    let path = fx.root.join("pins.toml");
    fs::write(
        &path,
        r#"
[pins]
schema_version = 2

[[pins.slots]]
id = "good"
kind = "mux"

[[pins.slots]]
id = "bad"
kind = "nonsense"
"#,
    )
    .unwrap();
    let store = SlotStore::at(&path);
    let mut snapshot = GraphSnapshot::empty();

    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: Some(&store),
            legacy_loader: None,
            legacy_registry_stores: &[],
        },
    );

    assert_eq!(snapshot.pins.len(), 1);
    assert_eq!(snapshot.pins[0].id, "good");
    assert!(
        snapshot
            .diagnostics
            .iter()
            .any(|d| matches!(d, Diagnostic::Config { path: p, .. } if p.ends_with("pins.toml"))),
        "{:?}",
        snapshot.diagnostics
    );
}

#[test]
fn project_and_user_legacy_entries_load_after_the_real_slots_with_one_diagnostic() {
    let fx = fixture();
    fx.write_project(&legacy_toml("proj", "proj"));
    fx.write_user(&legacy_toml("usr", "usr"));
    let store = SlotStore::at(fx.root.join("pins.toml"));
    store.add(slot("real", 0)).unwrap();
    let loader = fx.loader();
    let mut snapshot = GraphSnapshot::empty();

    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: Some(&store),
            legacy_loader: Some(&loader),
            legacy_registry_stores: &[],
        },
    );

    let ids: Vec<_> = snapshot
        .pins
        .iter()
        .map(|p| (p.id.as_str(), p.legacy))
        .collect();
    assert_eq!(ids, [("real", false), ("proj", true), ("usr", true)]);
    assert_eq!(
        legacy_diagnostics(&snapshot),
        [(2, vec!["proj".to_string(), "usr".to_string()])]
    );
}

#[test]
fn a_legacy_entry_named_like_a_slot_is_skipped_with_a_note() {
    let fx = fixture();
    fx.write_project(&legacy_toml("dup", "dup"));
    let store = SlotStore::at(fx.root.join("pins.toml"));
    store.add(slot("dup", 0)).unwrap();
    let loader = fx.loader();
    let mut snapshot = GraphSnapshot::empty();

    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: Some(&store),
            legacy_loader: Some(&loader),
            legacy_registry_stores: &[],
        },
    );

    assert_eq!(snapshot.pins.len(), 1);
    assert!(!snapshot.pins[0].legacy);
    assert!(legacy_diagnostics(&snapshot).is_empty());
    assert!(
        snapshot.diagnostics.iter().any(|d| matches!(
            d,
            Diagnostic::Config { message, .. } if message.contains("already a slot")
        )),
        "{:?}",
        snapshot.diagnostics
    );
}

#[test]
fn a_project_entry_shadows_a_user_entry_with_the_same_id() {
    let fx = fixture();
    fx.write_project(&legacy_toml("same", "from-project"));
    fx.write_user(&legacy_toml("same", "from-user"));
    let loader = fx.loader();
    let mut snapshot = GraphSnapshot::empty();

    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: None,
            legacy_loader: Some(&loader),
            legacy_registry_stores: &[],
        },
    );

    assert_eq!(snapshot.pins.len(), 1);
    assert_eq!(
        snapshot.pins[0].overrides.get("name"),
        Some(&FieldValue::Text("from-project".into()))
    );
    assert!(
        snapshot.diagnostics.iter().any(|d| matches!(
            d,
            Diagnostic::Config { message, .. } if message.contains("shadowed")
        )),
        "{:?}",
        snapshot.diagnostics
    );
}

#[test]
fn a_registry_store_outside_every_scan_root_still_loads() {
    let fx = fixture();
    let outside = fx.root.parent().unwrap().join("elsewhere/repo");
    fs::create_dir_all(&outside).unwrap();
    let store_path = outside.join(PROJECT_CONFIG_FILENAME);
    fs::write(&store_path, legacy_toml("ingest", "ingest")).unwrap();
    let loader = fx.loader();

    let mut baseline = GraphSnapshot::empty();
    apply_pins(
        &mut baseline,
        &fx.context(),
        &PinSources {
            slots: None,
            legacy_loader: Some(&loader),
            legacy_registry_stores: &[],
        },
    );
    assert!(baseline.pins.is_empty(), "invisible without the registry");

    let mut snapshot = GraphSnapshot::empty();
    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: None,
            legacy_loader: Some(&loader),
            legacy_registry_stores: std::slice::from_ref(&store_path),
        },
    );
    assert_eq!(snapshot.pins.len(), 1);
    assert_eq!(snapshot.pins[0].id, "ingest");
}

#[test]
fn a_registry_store_that_no_longer_exists_is_skipped_quietly() {
    let fx = fixture();
    let gone = fx.root.join("deleted/.conspectus.toml");
    let loader = fx.loader();
    let mut snapshot = GraphSnapshot::empty();

    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: None,
            legacy_loader: Some(&loader),
            legacy_registry_stores: std::slice::from_ref(&gone),
        },
    );

    assert!(snapshot.pins.is_empty());
    assert!(snapshot.diagnostics.is_empty());
}

#[test]
fn a_malformed_legacy_file_is_a_diagnostic_not_a_failure() {
    let fx = fixture();
    fx.write_project("[pins\nbroken");
    let loader = fx.loader();
    let mut snapshot = GraphSnapshot::empty();

    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: None,
            legacy_loader: Some(&loader),
            legacy_registry_stores: &[],
        },
    );

    assert!(snapshot.pins.is_empty());
    assert!(
        snapshot.diagnostics.iter().any(|d| matches!(
            d,
            Diagnostic::Config { message, .. } if message.contains("failed to parse pins")
        )),
        "{:?}",
        snapshot.diagnostics
    );
}

#[test]
fn loading_never_writes_a_file() {
    let fx = fixture();
    fx.write_project(&legacy_toml("a", "a"));
    let before = fs::read_to_string(fx.root.join(PROJECT_CONFIG_FILENAME)).unwrap();
    let store = SlotStore::at(fx.root.join("data/pins.toml"));
    let loader = fx.loader();
    let mut snapshot = GraphSnapshot::empty();

    apply_pins(
        &mut snapshot,
        &fx.context(),
        &PinSources {
            slots: Some(&store),
            legacy_loader: Some(&loader),
            legacy_registry_stores: &[],
        },
    );

    assert_eq!(snapshot.pins.len(), 1);
    assert_eq!(
        fs::read_to_string(fx.root.join(PROJECT_CONFIG_FILENAME)).unwrap(),
        before
    );
    assert!(
        !fx.root.join("data").exists(),
        "the store file isn't created by a read"
    );
}
