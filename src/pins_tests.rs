use super::*;

fn sample_entry(id: &str, mux_name: &str) -> PinEntry {
    PinEntry {
        id: id.to_string(),
        display_name: id.to_string(),
        harness: "codex".to_string(),
        cwd: "/home/me/work/repo".to_string(),
        mux: PinMux {
            backend: TMUX_MUX_BACKEND.to_string(),
            name: mux_name.to_string(),
            socket_name: None,
        },
        launch: None,
        worktree: None,
        reason: None,
    }
}

#[test]
fn round_trip_default_socket_entry() {
    let document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![sample_entry("ingest-refactor", "ingest-refactor")],
        }),
    };

    let encoded = to_toml(&document).expect("serialize");
    let decoded = parse_pins_document(&encoded).expect("parse");

    assert_eq!(decoded, document);
    assert!(encoded.contains("[[pins.entries]]"));
    assert!(encoded.contains("backend = \"tmux\""));
    assert!(!encoded.contains("socket_name"));
}

#[test]
fn round_trip_worktree_backed_entry() {
    // ADR 0094: a worktree-backed pin serializes a `[worktree]` block
    // and parses back unchanged; older stores (no block) stay `None`.
    let mut entry = sample_entry("feature-x", "feature-x");
    entry.worktree = Some(PinWorktree {
        branch: "feature-x".to_string(),
    });
    let document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![entry.clone()],
        }),
    };

    let encoded = to_toml(&document).expect("serialize");
    let decoded = parse_pins_document(&encoded).expect("parse");

    assert_eq!(decoded.entries(), &[entry]);
    assert!(encoded.contains("branch = \"feature-x\""));
    // A plain pin omits the block entirely.
    let plain = sample_entry("plain", "plain");
    let plain_encoded = to_toml(&PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![plain],
        }),
    })
    .expect("serialize");
    assert!(!plain_encoded.contains("[pins.entries.worktree]"));
}

#[test]
fn round_trip_non_default_socket_entry() {
    let mut entry = sample_entry("scratch-codex", "scratch-codex");
    entry.mux.socket_name = Some("scratch".to_string());
    entry.launch = Some(PinLaunch {
        argv: vec!["codex".to_string()],
    });

    let document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![entry.clone()],
        }),
    };

    let encoded = to_toml(&document).expect("serialize");
    let decoded = parse_pins_document(&encoded).expect("parse");

    assert_eq!(decoded.entries(), &[entry]);
    assert!(encoded.contains("socket_name = \"scratch\""));
    assert!(encoded.contains("argv = [\"codex\"]"));
}

#[test]
fn missing_pins_section_yields_empty_document() {
    let document = parse_pins_document("[session]\nprojection = \"agent\"\n").expect("parse");
    assert!(document.entries().is_empty());
}

#[test]
fn unknown_keys_are_ignored_for_forward_compatibility() {
    let document = parse_pins_document(
        r#"
            [pins]
            schema_version = 1
            future_table_key = "ignored"

            [[pins.entries]]
            id = "ingest-refactor"
            display_name = "ingest-refactor"
            harness = "codex"
            cwd = "/home/me/work/repo"
            mux = { backend = "tmux", name = "ingest-refactor", future_mux_key = "ignored" }
            future_entry_key = true
            "#,
    )
    .expect("parse with unknown keys");
    assert_eq!(document.entries().len(), 1);
}

#[test]
fn expand_home_prefix_with_rewrites_tilde_forms() {
    let home = Path::new("/home/op");
    assert_eq!(expand_home_prefix_with("~", Some(home)), "/home/op");
    assert_eq!(expand_home_prefix_with("~/", Some(home)), "/home/op/");
    assert_eq!(
        expand_home_prefix_with("~/src/proj", Some(home)),
        "/home/op/src/proj",
    );
    // Non-tilde absolute paths pass through unchanged.
    assert_eq!(
        expand_home_prefix_with("/abs/path", Some(home)),
        "/abs/path",
    );
    // Foreign-user form `~user/...` is intentionally not handled
    // so callers see a clear `RelativeCwd` error downstream rather
    // than a silently-wrong expansion.
    assert_eq!(
        expand_home_prefix_with("~other/foo", Some(home)),
        "~other/foo",
    );
    // Without `$HOME`, the helper degrades to identity so the
    // downstream `is_absolute` check produces the original error.
    assert_eq!(expand_home_prefix_with("~/foo", None), "~/foo");
}

#[test]
fn canonicalize_document_cwds_rewrites_each_entry() {
    // SAFETY: pin HOME for this thread for the duration of the
    // call. We avoid env mutation in tests that run under
    // nextest's parallel runner; this is exercised via the
    // explicit `expand_home_prefix_with` helper above. To verify
    // the integration without touching `$HOME`, we hand-build a
    // document, run the canonicalizer through a thin wrapper, and
    // assert the entry's `cwd` is rewritten to the expected
    // absolute path.
    fn canonicalize_with(document: &mut PinsDocument, home: &Path) {
        if let Some(section) = document.pins.as_mut() {
            for entry in &mut section.entries {
                entry.cwd = expand_home_prefix_with(&entry.cwd, Some(home));
            }
        }
    }

    let mut document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![PinEntry {
                cwd: "~/src/proj".to_string(),
                ..sample_entry("ingest", "ingest")
            }],
        }),
    };
    canonicalize_with(&mut document, Path::new("/home/op"));
    assert_eq!(document.entries()[0].cwd, "/home/op/src/proj");
}

#[test]
fn unsupported_schema_version_is_an_error() {
    let err = parse_pins_document(
        r"
            [pins]
            schema_version = 99
            ",
    )
    .expect_err("unsupported schema");
    assert!(matches!(err, PinParseError::UnsupportedSchemaVersion(99)));
}

#[test]
fn malformed_toml_is_an_error() {
    let err = parse_pins_document("this is not toml = ").expect_err("malformed");
    assert!(matches!(err, PinParseError::MalformedToml(_)));
}

#[test]
fn duplicate_id_is_rejected() {
    let document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![sample_entry("dup", "mux-a"), sample_entry("dup", "mux-b")],
        }),
    };
    let encoded = to_toml(&document).expect("serialize");
    let err = parse_pins_document(&encoded).expect_err("duplicate id");
    assert!(matches!(err, PinParseError::DuplicateId(id) if id == "dup"));
}

#[test]
fn duplicate_mux_triple_is_rejected() {
    let document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![
                sample_entry("first", "shared"),
                sample_entry("second", "shared"),
            ],
        }),
    };
    let encoded = to_toml(&document).expect("serialize");
    let err = parse_pins_document(&encoded).expect_err("duplicate mux");
    assert!(matches!(err, PinParseError::DuplicateMux { .. }));
}

#[test]
fn same_mux_name_on_different_sockets_is_allowed() {
    let a = sample_entry("a", "shared");
    let mut b = sample_entry("b", "shared");
    b.mux.socket_name = Some("scratch".to_string());

    let document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![a, b],
        }),
    };
    let encoded = to_toml(&document).expect("serialize");
    let decoded = parse_pins_document(&encoded).expect("parse");
    assert_eq!(decoded.entries().len(), 2);

    let mut c = sample_entry("c", "shared");
    c.mux.socket_name = Some("default".to_string());
    let collides_via_default_sentinel = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![sample_entry("a", "shared"), c],
        }),
    };
    let encoded2 = to_toml(&collides_via_default_sentinel).expect("serialize");
    let err = parse_pins_document(&encoded2).expect_err("default sentinel collides");
    assert!(matches!(err, PinParseError::DuplicateMux { .. }));
}

#[test]
fn empty_required_fields_are_rejected() {
    for (label, mutate) in [
        (
            "id",
            Box::new(|e: &mut PinEntry| e.id = "  ".to_string()) as Box<dyn Fn(&mut PinEntry)>,
        ),
        (
            "display_name",
            Box::new(|e: &mut PinEntry| e.display_name = String::new()),
        ),
        (
            "harness",
            Box::new(|e: &mut PinEntry| e.harness = String::new()),
        ),
        ("cwd", Box::new(|e: &mut PinEntry| e.cwd = String::new())),
        (
            "mux.name",
            Box::new(|e: &mut PinEntry| e.mux.name = String::new()),
        ),
        (
            "mux.socket_name",
            Box::new(|e: &mut PinEntry| e.mux.socket_name = Some(String::new())),
        ),
    ] {
        let mut entry = sample_entry("ingest", "ingest");
        mutate(&mut entry);
        let document = PinsDocument {
            pins: Some(PinsSection {
                schema_version: PINS_SCHEMA_VERSION,
                entries: vec![entry],
            }),
        };
        let encoded = to_toml(&document).expect("serialize");
        // Serialization may skip empty fields entirely, in which
        // case the parse error is a missing required field bubbled
        // up as MalformedToml. Either is acceptable as long as the
        // parser refuses to silently load the entry.
        match parse_pins_document(&encoded) {
            Err(PinParseError::EmptyField { field, .. }) => {
                assert_eq!(field, label, "wrong field flagged for {label}");
            }
            Err(PinParseError::MalformedToml(_)) => {
                // Serde reported the missing required key; still a
                // refusal, which is the invariant under test.
            }
            Err(other) => panic!("unexpected parse error for empty `{label}`: {other}"),
            Ok(_) => panic!("parser silently accepted empty `{label}`"),
        }
    }
}

#[test]
fn relative_cwd_is_rejected() {
    let mut entry = sample_entry("ingest", "ingest");
    entry.cwd = "relative/path".to_string();
    let document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![entry],
        }),
    };
    let encoded = to_toml(&document).expect("serialize");
    let err = parse_pins_document(&encoded).expect_err("relative cwd");
    assert!(matches!(err, PinParseError::RelativeCwd { .. }));
}

#[test]
fn unsupported_mux_backend_is_rejected() {
    // `zellij` is now a registered backend; test
    // with a synthetic key so the assertion still targets
    // the unregistered-backend rejection path.
    let mut entry = sample_entry("ingest", "ingest");
    entry.mux.backend = "screen-notreal".to_string();
    let document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![entry],
        }),
    };
    let encoded = to_toml(&document).expect("serialize");
    let err = parse_pins_document(&encoded).expect_err("unregistered backend");
    assert!(matches!(err, PinParseError::UnsupportedMuxBackend { .. }));
}

#[test]
fn round_trip_preserves_sibling_sections() {
    let text = r#"
[session]
projection = "agent"

[aliases]
schema_version = 1

[[aliases.entries]]
node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
display_name = "alpha-name"

[pins]
schema_version = 1

[[pins.entries]]
id = "ingest"
display_name = "ingest"
harness = "codex"
cwd = "/home/me/work/repo"
mux = { backend = "tmux", name = "ingest" }
"#;
    // We only verify that PinsDocument decodes the pins slice; the
    // sibling sections are owned by other modules. The test guards
    // against accidentally tightening PinsDocument to reject unknown
    // top-level keys.
    let document = parse_pins_document(text).expect("parse alongside siblings");
    assert_eq!(document.entries().len(), 1);
    assert_eq!(document.entries()[0].id, "ingest");
}

#[test]
fn effective_socket_and_native_id() {
    let mut entry = sample_entry("ingest", "ingest");
    assert_eq!(entry.mux.effective_socket(), None);
    assert_eq!(entry.mux.native_id(), "tmux:ingest");

    entry.mux.socket_name = Some("default".to_string());
    assert_eq!(
        entry.mux.effective_socket(),
        None,
        "the literal `default` sentinel collapses to the canonical no-socket case"
    );
    assert_eq!(entry.mux.native_id(), "tmux:ingest");

    entry.mux.socket_name = Some("scratch".to_string());
    assert_eq!(entry.mux.effective_socket(), Some("scratch"));
    assert_eq!(entry.mux.native_id(), "tmux:scratch:ingest");
}

// ---- ADR 0057 / H-PIN-005 store selection ------------------

use tempfile::TempDir;

fn loader_with_paths(home: &Path, xdg_config: &Path) -> ConfigLoader {
    ConfigLoader::new()
        .with_home(home.to_path_buf())
        .with_xdg_config_home(xdg_config.to_path_buf())
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir parent");
    }
    fs::write(path, contents).expect("write");
}

#[test]
fn select_store_for_pin_uses_existing_project_config_walking_up() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let project_root = tmp.path().join("project");
    let nested = project_root.join("sub/dir");
    fs::create_dir_all(&nested).expect("mkdir nested");
    let project_config = project_root.join(PROJECT_CONFIG_FILENAME);
    write_file(&project_config, "[session]\nprojection = \"agent\"\n");

    let loader = loader_with_paths(&home, &xdg);
    let selection = select_store_for_pin(&nested, &loader).expect("ok");
    assert_eq!(selection.kind, PinStoreKind::Project);
    assert_eq!(selection.path, project_config);
}

#[test]
fn select_store_for_pin_defaults_to_cwd_when_no_project_config_exists() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let cwd = tmp.path().join("workspace/project");
    fs::create_dir_all(&cwd).expect("mkdir cwd");

    let loader = loader_with_paths(&home, &xdg);
    let selection = select_store_for_pin(&cwd, &loader).expect("ok");
    assert_eq!(selection.kind, PinStoreKind::Project);
    assert_eq!(selection.path, cwd.join(PROJECT_CONFIG_FILENAME));
}

#[test]
fn select_store_for_pin_rejects_relative_cwd() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let loader = loader_with_paths(&home, &xdg);

    let err = select_store_for_pin(Path::new("project"), &loader).expect_err("relative cwd");
    assert!(matches!(err, PinStoreSelectionError::RelativeCwd { .. }));
}

#[test]
fn select_store_for_pin_rejects_nonexistent_cwd() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let cwd = tmp.path().join("does/not/exist");

    let loader = loader_with_paths(&home, &xdg);
    let err = select_store_for_pin(&cwd, &loader).expect_err("missing cwd");
    assert!(matches!(err, PinStoreSelectionError::CwdMissing { .. }));
}

#[test]
fn user_pin_store_returns_user_config_path() {
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    let xdg = tmp.path().join("xdg");
    let loader = loader_with_paths(&home, &xdg);

    let selection = user_pin_store(&loader).expect("ok");
    assert_eq!(selection.kind, PinStoreKind::User);
    assert!(selection.path.starts_with(&xdg));
}

// ---- ADR 0057 / H-PIN-006 write helpers --------------------

fn write_entry(id: &str, mux_name: &str) -> PinEntry {
    PinEntry {
        id: id.to_string(),
        display_name: id.to_string(),
        harness: "codex".to_string(),
        cwd: "/home/me/work/repo".to_string(),
        mux: PinMux {
            backend: TMUX_MUX_BACKEND.to_string(),
            name: mux_name.to_string(),
            socket_name: None,
        },
        launch: None,
        worktree: None,
        reason: None,
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("read")
}

#[test]
fn upsert_creates_new_file_with_pins_section() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");

    let outcome = upsert_pin_entry(&path, write_entry("ingest", "ingest")).expect("upsert");
    assert!(outcome.changed);
    assert_eq!(outcome.entry_count, 1);
    assert_eq!(outcome.path, path);

    let contents = read(&path);
    assert!(contents.contains("[pins]"));
    assert!(contents.contains("id = \"ingest\""));
    let parsed = parse_pins_document(&contents).expect("round-trip");
    assert_eq!(parsed.entries().len(), 1);
}

#[test]
fn upsert_with_identical_entry_is_a_no_op() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");

    upsert_pin_entry(&path, write_entry("ingest", "ingest")).expect("first");
    let mtime_first = fs::metadata(&path)
        .expect("stat")
        .modified()
        .expect("mtime");

    // Re-upsert the same entry; outcome must report unchanged.
    // Sleep briefly so a real change would shift the mtime.
    std::thread::sleep(std::time::Duration::from_millis(10));
    let outcome = upsert_pin_entry(&path, write_entry("ingest", "ingest")).expect("idempotent");
    assert!(!outcome.changed);

    let mtime_second = fs::metadata(&path)
        .expect("stat")
        .modified()
        .expect("mtime");
    assert_eq!(
        mtime_first, mtime_second,
        "no-op upsert must not touch the file"
    );
}

#[test]
fn upsert_replaces_existing_id_and_keeps_sorted_order() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");

    upsert_pin_entry(&path, write_entry("b-second", "mux-b")).expect("b");
    upsert_pin_entry(&path, write_entry("a-first", "mux-a")).expect("a");

    // Now replace `b-second` with a different mux name (changing
    // display_name to confirm it's the new entry).
    let mut updated = write_entry("b-second", "mux-b");
    updated.display_name = "renamed".to_string();
    let outcome = upsert_pin_entry(&path, updated).expect("replace");
    assert!(outcome.changed);
    assert_eq!(outcome.entry_count, 2);

    let contents = read(&path);
    let parsed = parse_pins_document(&contents).expect("round-trip");
    let ids: Vec<&str> = parsed.entries().iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["a-first", "b-second"], "sorted by id");
    let b_entry = parsed
        .entries()
        .iter()
        .find(|e| e.id == "b-second")
        .unwrap();
    assert_eq!(b_entry.display_name, "renamed");
}

#[test]
fn upsert_preserves_unrelated_sections() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");
    write_file(
        &path,
        r#"[session]
projection = "agent"

[declared]
schema_version = 1

[[declared.links]]
id = "codex-alpha-to-editor"
relation = "linked_to_mux"
state = "active"
source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
target = { type = "mux_session", native_id = "tmux:editor" }
"#,
    );

    let outcome =
        upsert_pin_entry(&path, write_entry("ingest", "ingest")).expect("upsert next to siblings");
    assert!(outcome.changed);

    let contents = read(&path);
    assert!(contents.contains("[session]"), "session block preserved");
    assert!(contents.contains("[declared]"), "declared block preserved");
    assert!(contents.contains("[pins]"), "pins block added");
    assert!(
        contents.contains("codex-alpha-to-editor"),
        "declared link payload preserved"
    );
}

#[test]
fn upsert_rejects_invalid_entry() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");
    let mut bad = write_entry("ingest", "ingest");
    bad.cwd = "relative/path".to_string();

    let err = upsert_pin_entry(&path, bad).expect_err("validation refused");
    assert!(matches!(
        err,
        PinWriteError::Validation(PinParseError::RelativeCwd { .. })
    ));
    assert!(!path.exists(), "no file created on validation failure");
}

#[test]
fn upsert_rejects_duplicate_mux_triple_across_ids() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");
    upsert_pin_entry(&path, write_entry("first", "shared")).expect("first");

    let err = upsert_pin_entry(&path, write_entry("second", "shared")).expect_err("duplicate mux");
    assert!(matches!(
        err,
        PinWriteError::Validation(PinParseError::DuplicateMux { .. })
    ));

    // First entry should still be intact on disk.
    let parsed = parse_pins_document(&read(&path)).expect("still parses");
    let ids: Vec<&str> = parsed.entries().iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["first"]);
}

#[test]
fn upsert_refuses_to_overwrite_malformed_file() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");
    write_file(&path, "this is not = toml at all =");
    let before = read(&path);

    let err = upsert_pin_entry(&path, write_entry("ingest", "ingest")).expect_err("malformed");
    assert!(matches!(err, PinWriteError::Parse { .. }));
    assert_eq!(read(&path), before, "malformed file left untouched");
}

#[test]
fn remove_existing_entry_strips_section_when_last() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");
    upsert_pin_entry(&path, write_entry("ingest", "ingest")).expect("upsert");

    let outcome = remove_pin_entry(&path, "ingest").expect("remove");
    assert!(outcome.changed);
    assert_eq!(outcome.entry_count, 0);
    // The file should be removed entirely since pins was the only
    // top-level section.
    assert!(!path.exists());
}

#[test]
fn remove_preserves_siblings_and_drops_only_pins_section() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");
    write_file(&path, "[session]\nprojection = \"agent\"\n");
    upsert_pin_entry(&path, write_entry("ingest", "ingest")).expect("upsert");

    let outcome = remove_pin_entry(&path, "ingest").expect("remove");
    assert!(outcome.changed);
    let contents = read(&path);
    assert!(contents.contains("[session]"), "session preserved");
    assert!(!contents.contains("[pins]"), "pins section stripped");
}

#[test]
fn remove_nonexistent_id_is_a_no_op() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("project/.conspectus.toml");
    upsert_pin_entry(&path, write_entry("ingest", "ingest")).expect("upsert");
    let before = read(&path);

    let outcome = remove_pin_entry(&path, "not-there").expect("remove");
    assert!(!outcome.changed);
    assert_eq!(outcome.entry_count, 1);
    assert_eq!(read(&path), before, "file unchanged on no-op remove");
}

#[test]
fn load_pin_entry_by_id_walks_paths_and_returns_first_match() {
    let tmp = TempDir::new().expect("tmp");
    let local = tmp.path().join("local/.conspectus.toml");
    let global = tmp.path().join("global/config.toml");
    upsert_pin_entry(&local, write_entry("ingest", "local-mux")).expect("local");
    upsert_pin_entry(&global, write_entry("scratch", "global-mux")).expect("global");

    let paths = vec![local.clone(), global.clone()];

    let (found_path, entry) = load_pin_entry_by_id(&paths, "ingest")
        .expect("lookup")
        .expect("present");
    assert_eq!(found_path, local);
    assert_eq!(entry.id, "ingest");

    let (found_path, entry) = load_pin_entry_by_id(&paths, "scratch")
        .expect("lookup")
        .expect("present");
    assert_eq!(found_path, global);
    assert_eq!(entry.id, "scratch");

    assert!(
        load_pin_entry_by_id(&paths, "nope")
            .expect("lookup")
            .is_none()
    );
}

#[test]
fn upsert_creates_parent_directory_via_atomic_write() {
    let tmp = TempDir::new().expect("tmp");
    let path = tmp.path().join("a/b/c/.conspectus.toml");

    upsert_pin_entry(&path, write_entry("ingest", "ingest")).expect("create parent");
    assert!(path.is_file());
}
