use serde_json::json;

use super::*;

#[test]
fn jsonc_comments_and_trailing_commas_are_dropped_and_strings_kept() {
    let text = r#"{
  // a line comment with "quotes" and a , before }
  "plugins": [
    /* a block comment */ { "package": "/home/user/src/conspectus/plugins/opencode-hook/dist/v2" },
    "https://example.com/a//b", // after a value
  ],
  "theme": "a \"quoted\" // not a comment /* nor this */ ,]",
}"#;
    let document: Value = serde_json::from_str(&strip_jsonc(text)).expect("parses");
    assert_eq!(
        document,
        json!({
            "plugins": [
                {"package": "/home/user/src/conspectus/plugins/opencode-hook/dist/v2"},
                "https://example.com/a//b",
            ],
            "theme": "a \"quoted\" // not a comment /* nor this */ ,]",
        })
    );
    assert_eq!(strip_jsonc("[1, 2,\n]"), "[1, 2\n]");
    assert_eq!(strip_jsonc("\"a\\\\\" // c"), "\"a\\\\\" ");
}

#[test]
fn an_entry_is_the_plugins_when_it_names_its_directory_or_package() {
    for entry in [
        json!("/home/user/src/conspectus/plugins/opencode-hook"),
        json!("/home/user/src/conspectus/plugins/opencode-hook/"),
        json!("/home/user/src/conspectus/plugins/opencode-hook/dist/v2"),
        json!("file:///home/user/src/conspectus/plugins/opencode-hook"),
        json!({"package": "/srv/conspectus/plugins/opencode-hook/dist/v2"}),
        json!("@conspectus/opencode-hook"),
        json!("@conspectus/opencode-hook@0.4.0"),
        json!("C:\\src\\conspectus\\plugins\\opencode-hook"),
    ] {
        assert!(names_plugin(&entry), "{entry}");
    }
    for entry in [
        json!("/home/user/src/my-opencode-hook"),
        json!("opencode-hooks"),
        json!("opencode-hook"),
        json!("@someone/opencode-hook"),
        json!({"name": "/plugins/opencode-hook"}),
        json!(42),
        json!(null),
    ] {
        assert!(!names_plugin(&entry), "{entry}");
    }
}

#[test]
fn a_config_file_says_which_entrypoints_it_loads() {
    let v1 = r#"{"plugin": ["/src/conspectus/plugins/opencode-hook"]}"#;
    assert_eq!(
        status_of(v1, FileKind::Config),
        FileStatus::Lists(vec![Loads::V1])
    );
    let v2 = r#"{
      // the server package
      "plugins": [{"package": "/src/conspectus/plugins/opencode-hook/dist/v2"},],
    }"#;
    assert_eq!(
        status_of(v2, FileKind::Config),
        FileStatus::Lists(vec![Loads::V2])
    );
    let both = r#"{"plugin": ["/x/opencode-hook"], "plugins": ["/x/opencode-hook/dist/v2"]}"#;
    assert_eq!(
        status_of(both, FileKind::Config),
        FileStatus::Lists(vec![Loads::V1, Loads::V2])
    );
    let cli = r#"{"plugins": ["/src/conspectus/plugins/opencode-hook/dist/v2"]}"#;
    assert_eq!(
        status_of(cli, FileKind::Cli),
        FileStatus::Lists(vec![Loads::V2Client])
    );
    // cli.json has no 1.x key.
    assert_eq!(status_of(v1, FileKind::Cli), FileStatus::NotListed);
    for other in [
        "{}",
        r#"{"plugins": ["someone-else"]}"#,
        r#"{"plugin": "not a list"}"#,
    ] {
        assert_eq!(
            status_of(other, FileKind::Config),
            FileStatus::NotListed,
            "{other}"
        );
    }
    assert!(matches!(
        status_of("{", FileKind::Config),
        FileStatus::Unreadable(_)
    ));
    assert!(matches!(
        status_of("[]", FileKind::Cli),
        FileStatus::Unreadable(_)
    ));
}

#[test]
fn the_note_says_what_a_listing_in_cli_json_means() {
    assert_eq!(listing_note(&[]), None);
    assert_eq!(listing_note(&[Loads::V1]), None);
    assert_eq!(listing_note(&[Loads::V2]), None);
    let only = listing_note(&[Loads::V2Client]).expect("a note");
    assert!(only.contains("only in cli.json"), "{only}");
    assert!(only.contains("opencode.jsonc"), "{only}");
    let twice = listing_note(&[Loads::V2, Loads::V2Client]).expect("a note");
    assert!(twice.contains("twice"), "{twice}");
}
