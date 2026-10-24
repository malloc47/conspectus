use super::*;

const TWO_PINS: &str = r#"# my config
[session]
poll_ms = 250

[pins]
schema_version = 1

[[pins.entries]]
id = "api"
display_name = "API work"
harness = "claude-code"
cwd = "/src/api"
[pins.entries.mux]
backend = "tmux"
name = "api"
socket_name = "work"
[pins.entries.launch]
argv = ["claude", "--model", "opus"]
[pins.entries.worktree]
branch = "feature/x"

[[pins.entries]]
id = "docs"
display_name = "docs"
harness = "codex"
cwd = "/src/docs"
[pins.entries.mux]
backend = "tmux"
name = "docs"
"#;

fn entry(id: &str, mux: &str) -> PinEntry {
    PinEntry {
        id: id.into(),
        display_name: id.into(),
        harness: "claude-code".into(),
        cwd: "/src/x".into(),
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

fn ids(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| (*n).to_string()).collect()
}

#[test]
fn a_config_file_with_other_tables_still_yields_its_entries() {
    let document = parse_pins_document(TWO_PINS).unwrap();
    let entries = document.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "api");
    assert_eq!(entries[0].mux.native_id(), "tmux:work:api");
    assert_eq!(entries[1].mux.native_id(), "tmux:docs");
}

#[test]
fn a_file_without_pins_has_no_entries() {
    let document = parse_pins_document("[session]\npoll_ms = 5\n").unwrap();
    assert!(document.entries().is_empty());
}

#[test]
fn invalid_documents_are_rejected_with_the_old_messages() {
    let bad = |body: &str| parse_pins_document(&format!("[pins]\nschema_version = 1\n{body}"));
    let one = |extra: &str| {
        format!(
            "[[pins.entries]]\nid = \"a\"\ndisplay_name = \"a\"\nharness = \"codex\"\ncwd = \"{extra}\"\n[pins.entries.mux]\nbackend = \"tmux\"\nname = \"a\"\n"
        )
    };
    assert!(matches!(
        bad(&one("relative/dir")),
        Err(PinParseError::RelativeCwd { .. })
    ));
    assert!(matches!(
        bad(&one("")),
        Err(PinParseError::EmptyField { field: "cwd", .. })
    ));
    assert!(matches!(
        parse_pins_document("[pins]\nschema_version = 9\n"),
        Err(PinParseError::UnsupportedSchemaVersion(9))
    ));
    assert!(matches!(
        parse_pins_document("[pins\n"),
        Err(PinParseError::MalformedToml(_))
    ));
    let twice = format!("{}{}", one("/a"), one("/b"));
    assert!(matches!(bad(&twice), Err(PinParseError::DuplicateId(_))));
}

#[test]
fn a_leading_tilde_expands_to_home() {
    let home = Path::new("/home/me");
    assert_eq!(expand_home_prefix_with("~", Some(home)), "/home/me");
    assert_eq!(expand_home_prefix_with("~/src", Some(home)), "/home/me/src");
    assert_eq!(
        expand_home_prefix_with("~other/src", Some(home)),
        "~other/src"
    );
    assert_eq!(expand_home_prefix_with("~/src", None), "~/src");
}

#[test]
fn an_entry_becomes_a_legacy_mux_slot_with_overrides_and_no_target() {
    let document = parse_pins_document(TWO_PINS).unwrap();
    let slot = entry_to_slot(&document.entries()[0], 3);

    assert_eq!(slot.id, "api");
    assert_eq!(slot.kind, PinKind::Mux);
    assert_eq!(slot.target, None);
    assert!(slot.legacy);
    assert_eq!(slot.order, 3);
    let text = |field: &str| slot.overrides.get(field).and_then(FieldValue::as_text);
    assert_eq!(text("name"), Some("api"));
    assert_eq!(text("socket"), Some("work"));
    assert_eq!(text("cwd"), Some("/src/api"));
    assert_eq!(text("harness"), Some("claude-code"));
    assert_eq!(text("worktree_branch"), Some("feature/x"));
    assert_eq!(
        slot.overrides.get("argv"),
        Some(&FieldValue::List(vec![
            "claude".into(),
            "--model".into(),
            "opus".into()
        ]))
    );
    assert_eq!(slot.label(), "API work");
}

#[test]
fn the_default_socket_and_empty_argv_leave_no_overrides() {
    let document = parse_pins_document(TWO_PINS).unwrap();
    let slot = entry_to_slot(&document.entries()[1], 0);
    assert!(!slot.overrides.contains_key("socket"));
    assert!(!slot.overrides.contains_key("argv"));
    assert!(!slot.overrides.contains_key("worktree_branch"));

    let mut named = entry("x", "x");
    named.mux.socket_name = Some("default".into());
    assert!(!entry_to_slot(&named, 0).overrides.contains_key("socket"));
}

#[test]
fn removing_entries_keeps_comments_other_tables_and_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".conspectus.toml");
    fs::write(&path, TWO_PINS).unwrap();

    let removed = remove_entries(&path, &ids(&["api"])).unwrap();

    assert_eq!(removed, 1);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("# my config"), "{text}");
    assert!(text.contains("poll_ms = 250"), "{text}");
    assert!(!text.contains("id = \"api\""), "{text}");
    let document = parse_pins_document(&text).unwrap();
    assert_eq!(document.entries().len(), 1);
    assert_eq!(document.entries()[0].id, "docs");
}

#[test]
fn removing_the_last_entry_drops_the_pins_table_but_not_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".conspectus.toml");
    fs::write(&path, TWO_PINS).unwrap();

    assert_eq!(remove_entries(&path, &ids(&["api", "docs"])).unwrap(), 2);

    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains("pins"), "{text}");
    assert!(text.contains("poll_ms = 250"), "{text}");
}

#[test]
fn a_file_left_empty_is_removed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".conspectus.toml");
    let only_pins = TWO_PINS
        .split_once("[session]")
        .map(|(head, rest)| {
            let pins = rest.split_once("[pins]").unwrap().1;
            format!("{}[pins]{pins}", head.replace("# my config\n", ""))
        })
        .unwrap();
    fs::write(&path, only_pins).unwrap();

    assert_eq!(remove_entries(&path, &ids(&["api", "docs"])).unwrap(), 2);

    assert!(!path.exists());
}

#[test]
fn removing_unknown_ids_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".conspectus.toml");
    fs::write(&path, TWO_PINS).unwrap();

    assert_eq!(remove_entries(&path, &ids(&["missing"])).unwrap(), 0);

    assert_eq!(fs::read_to_string(&path).unwrap(), TWO_PINS);
}

#[test]
fn removing_from_a_file_with_no_pins_is_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".conspectus.toml");
    fs::write(&path, "[session]\npoll_ms = 1\n").unwrap();

    assert_eq!(remove_entries(&path, &ids(&["api"])).unwrap(), 0);
    assert!(remove_entries(&dir.path().join("absent.toml"), &ids(&["api"])).is_err());
}

#[test]
fn round_tripping_through_toml_keeps_entries() {
    let document = parse_pins_document(TWO_PINS).unwrap();
    let again = parse_pins_document(&to_toml(&document).unwrap()).unwrap();
    assert_eq!(document, again);
}

#[cfg(unix)]
#[test]
fn a_symlinked_file_left_empty_keeps_its_link() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("dotfiles.toml");
    let link = dir.path().join(".conspectus.toml");
    let only_pins = TWO_PINS
        .split_once("[session]")
        .map(|(_, rest)| format!("[pins]{}", rest.split_once("[pins]").unwrap().1));
    fs::write(&real, only_pins.unwrap()).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    assert_eq!(remove_entries(&link, &ids(&["api", "docs"])).unwrap(), 2);

    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(!fs::read_to_string(&real).unwrap().contains("api"));
}
