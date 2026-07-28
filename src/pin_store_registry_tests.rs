use std::fs;

use tempfile::TempDir;

use super::*;

fn write_store(dir: &Path) -> PathBuf {
    let path = dir.join(PROJECT_CONFIG_FILENAME);
    fs::write(&path, "[pins]\nschema_version = 1\n").expect("write store");
    path
}

#[test]
fn path_prefers_xdg_state_home() {
    let registry = PinStoreRegistry::new()
        .with_home("/home/u")
        .with_xdg_state_home("/xdg/state");
    assert_eq!(
        registry.path().expect("path"),
        Path::new("/xdg/state/conspectus/pin-stores.json"),
    );
}

#[test]
fn path_falls_back_to_home_local_state() {
    let registry = PinStoreRegistry::new().with_home("/home/u");
    assert_eq!(
        registry.path().expect("path"),
        Path::new("/home/u/.local/state/conspectus/pin-stores.json"),
    );
}

#[test]
fn read_returns_empty_when_file_absent() {
    let temp = TempDir::new().expect("temp");
    let registry = PinStoreRegistry::new().with_xdg_state_home(temp.path());
    assert!(registry.read().is_empty());
}

#[test]
fn record_then_read_round_trips_a_project_store() {
    let temp = TempDir::new().expect("temp");
    let state = temp.path().join("state");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&repo).expect("mkdir repo");
    let store = write_store(&repo);

    let registry = PinStoreRegistry::new().with_xdg_state_home(&state);
    registry.record(&store).expect("record");

    assert_eq!(registry.read(), vec![store]);
}

#[test]
fn record_is_idempotent_and_skips_on_unchanged() {
    let temp = TempDir::new().expect("temp");
    let state = temp.path().join("state");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&repo).expect("mkdir repo");
    let store = write_store(&repo);

    let registry = PinStoreRegistry::new().with_xdg_state_home(&state);
    registry.record(&store).expect("first record");
    let path = registry.path().expect("path");
    let mtime_first = fs::metadata(&path)
        .and_then(|m| m.modified())
        .expect("mtime");

    registry.record(&store).expect("second record");
    let mtime_second = fs::metadata(&path)
        .and_then(|m| m.modified())
        .expect("mtime");

    assert_eq!(registry.read(), vec![store]);
    assert_eq!(mtime_first, mtime_second, "unchanged write should skip");
}

#[test]
fn record_ignores_user_scope_and_relative_paths() {
    let temp = TempDir::new().expect("temp");
    let state = temp.path().join("state");
    let registry = PinStoreRegistry::new().with_xdg_state_home(&state);

    // A user-scope store (config.toml, not .conspectus.toml) is never
    // recorded because discovery consults it directly.
    let user_store = temp.path().join("config.toml");
    fs::write(&user_store, "").expect("write user store");
    registry.record(&user_store).expect("record user");

    // A relative path is ignored so the on-disk list stays absolute.
    registry
        .record(Path::new(".conspectus.toml"))
        .expect("record relative");

    assert!(registry.read().is_empty());
}

#[test]
fn read_filters_out_stores_that_no_longer_exist() {
    let temp = TempDir::new().expect("temp");
    let state = temp.path().join("state");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&repo).expect("mkdir repo");
    let store = write_store(&repo);

    let registry = PinStoreRegistry::new().with_xdg_state_home(&state);
    registry.record(&store).expect("record");
    assert_eq!(registry.read(), vec![store.clone()]);

    fs::remove_file(&store).expect("remove store");
    assert!(
        registry.read().is_empty(),
        "a store whose file is gone should not be returned",
    );
}

#[test]
fn record_prunes_dead_entries_on_write() {
    let temp = TempDir::new().expect("temp");
    let state = temp.path().join("state");
    let repo_a = temp.path().join("repoA");
    let repo_b = temp.path().join("repoB");
    fs::create_dir_all(&repo_a).expect("mkdir repoA");
    fs::create_dir_all(&repo_b).expect("mkdir repoB");
    let store_a = write_store(&repo_a);
    let store_b = write_store(&repo_b);

    let registry = PinStoreRegistry::new().with_xdg_state_home(&state);
    registry.record(&store_a).expect("record A");

    // repoA disappears; recording repoB should drop the dead A entry.
    fs::remove_dir_all(&repo_a).expect("rm repoA");
    registry.record(&store_b).expect("record B");

    assert_eq!(registry.read(), vec![store_b]);
}

#[test]
fn read_ignores_malformed_json() {
    let temp = TempDir::new().expect("temp");
    let state = temp.path().join("state");
    let registry = PinStoreRegistry::new().with_xdg_state_home(&state);
    let path = registry.path().expect("path");
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir state");
    fs::write(&path, "{ not valid json").expect("write garbage");

    assert!(registry.read().is_empty());
}

#[test]
fn write_preserves_unknown_forward_compat_fields() {
    let temp = TempDir::new().expect("temp");
    let state = temp.path().join("state");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&repo).expect("mkdir repo");
    let store = write_store(&repo);

    let registry = PinStoreRegistry::new().with_xdg_state_home(&state);
    let path = registry.path().expect("path");
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir state");
    fs::write(
        &path,
        r#"{"schema_version":1,"stores":[],"future_field":{"k":"v"}}"#,
    )
    .expect("seed file");

    registry.record(&store).expect("record");

    let text = fs::read_to_string(&path).expect("read back");
    assert!(
        text.contains("future_field"),
        "unknown fields must round-trip: {text}",
    );
}
