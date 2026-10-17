use super::*;

fn fixtures(dir: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pi")
        .join(dir)
}

fn locator(root: PathBuf, session_key: &str) -> SessionLocator {
    SessionLocator {
        harness_key: "pi".to_string(),
        session_key: session_key.to_string(),
        state_root: root,
    }
}

const BRANCHED: &str = "0199c0de-0000-7000-8000-0000000000a1";

fn branched() -> TranscriptDocument {
    PiParser
        .read(&locator(fixtures("viewer"), BRANCHED))
        .expect("parse branched fixture")
}

/// `(kind, hidden, body)` for every turn, for compact assertions.
fn shape(document: &TranscriptDocument) -> Vec<(TurnKind, bool, &str)> {
    document
        .turns
        .iter()
        .map(|turn| (turn.kind, turn.hidden, turn.body.as_str()))
        .collect()
}

#[test]
fn renders_the_active_branch_from_root_to_leaf() {
    let document = branched();
    assert_eq!(document.meta.cwd.as_deref(), Some("/work/tree"));

    let visible: Vec<&str> = document
        .turns
        .iter()
        .filter(|turn| !turn.hidden && turn.kind == TurnKind::Message)
        .map(|turn| turn.body.as_str())
        .collect();
    assert_eq!(
        visible,
        [
            "Plan the refactor",
            "Here is the plan.",
            "Try approach B",
            "Two todos remain.",
            "Approach B is done.",
        ],
    );
}

#[test]
fn abandoned_tree_branches_are_absent() {
    let document = branched();
    for abandoned in ["Try approach A", "Approach A is done."] {
        assert!(
            document.turns.iter().all(|turn| turn.body != abandoned),
            "`{abandoned}` is on an abandoned branch: {:#?}",
            shape(&document),
        );
    }
}

#[test]
fn tools_thinking_and_summaries_render() {
    let document = branched();
    let turns = shape(&document);
    assert!(turns.contains(&(TurnKind::Thinking, false, "Read the module first.")));
    assert!(turns.contains(&(TurnKind::ToolUse, false, r#"read: {"path":"src/lib.rs"}"#)));
    assert!(turns.contains(&(TurnKind::ToolResult, false, "pub fn lib() {}")));
    assert!(turns.contains(&(
        TurnKind::CompactionSummary,
        false,
        "Planned the refactor; approach B chosen."
    )));
    assert!(turns.contains(&(
        TurnKind::CompactionSummary,
        false,
        "**Branch summary**\n\nApproach A worked but was too slow."
    )));
}

#[test]
fn records_pi_hides_are_hidden_turns() {
    let document = branched();
    let hidden: Vec<&str> = document
        .turns
        .iter()
        .filter(|turn| turn.hidden)
        .map(|turn| turn.body.as_str())
        .collect();
    assert_eq!(hidden.len(), 6, "{hidden:#?}");
    assert!(
        hidden[0].contains("You are Pi, a coding agent."),
        "system prompt"
    );
    assert!(
        hidden[0].contains("<tools>read, bash</tools>"),
        "system tools"
    );
    assert_eq!(hidden[1], "model: anthropic/claude-sonnet-5");
    assert_eq!(hidden[2], r#"extension state `todo`: {"open":2}"#);
    assert_eq!(hidden[3], "Injected todo context");
    assert_eq!(hidden[4], "label `done` on e14");
    assert_eq!(hidden[5], "session name: Refactor");
    assert!(
        document
            .turns
            .iter()
            .filter(|turn| turn.hidden)
            .all(|turn| turn.role == TurnRole::System),
    );
    // A `custom_message` with `display: true` is visible.
    assert!(
        document
            .turns
            .iter()
            .any(|turn| turn.body == "Two todos remain." && !turn.hidden)
    );
}

#[test]
fn turns_carry_entry_timestamps() {
    let document = branched();
    let first_user = document
        .turns
        .iter()
        .find(|turn| turn.body == "Plan the refactor")
        .expect("first user turn");
    assert_eq!(
        first_user.timestamp.map(|time| time.to_rfc3339()),
        Some("2026-10-16T09:00:05+00:00".to_string()),
    );
}

#[test]
fn legacy_linear_file_renders_in_order() {
    let document = PiParser
        .read(&locator(
            fixtures("grouped"),
            "0196f0b1-1111-7222-8333-944455566677",
        ))
        .expect("parse v1 fixture");
    assert_eq!(document.meta.cwd.as_deref(), Some("/work/my/repo"));
    assert_eq!(
        shape(&document),
        [
            (
                TurnKind::Message,
                false,
                "Legacy linear session in a directory whose name collides"
            ),
            (TurnKind::Message, false, "Legacy reply"),
        ],
    );
}

#[test]
fn flat_layout_and_header_id_lookup() {
    let flat = PiParser
        .read(&locator(fixtures("flat"), "my-custom.id"))
        .expect("flat layout");
    assert_eq!(flat.meta.cwd.as_deref(), Some("/work/flat"));

    // The header has no id, so the file name carries the key.
    let grouped = PiParser
        .read(&locator(
            fixtures("grouped"),
            "0199f0a4-0000-7000-8000-000000000004",
        ))
        .expect("file-name lookup");
    assert_eq!(grouped.meta.cwd.as_deref(), Some("/work/other"));
}

#[test]
fn header_id_finds_a_file_whose_name_disagrees() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("2026-10-16T09-00-00-000Z_renamed.jsonl"),
        concat!(
            r#"{"type":"session","version":3,"id":"real-id","cwd":"/work/x"}"#,
            "\n",
            r#"{"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"hi"}}"#,
            "\n",
        ),
    )
    .expect("write");
    let document = PiParser
        .read(&locator(dir.path().to_path_buf(), "real-id"))
        .expect("header lookup");
    assert_eq!(shape(&document), [(TurnKind::Message, false, "hi")]);
}

#[test]
fn missing_file_is_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(matches!(
        PiParser.read(&locator(dir.path().to_path_buf(), "absent")),
        Err(ParseError::NotFound)
    ));
}

#[test]
fn dangling_and_cyclic_parents_end_the_walk() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("2026-10-16T09-00-00-000Z_s.jsonl"),
        concat!(
            r#"{"type":"session","version":3,"id":"s","cwd":"/w"}"#,
            "\n",
            r#"{"type":"message","id":"a","parentId":"b","message":{"role":"user","content":"one"}}"#,
            "\n",
            r#"{"type":"message","id":"b","parentId":"a","message":{"role":"assistant","content":"two"}}"#,
            "\n",
            r#"{"type":"message","id":"c","parentId":"missing","message":{"role":"user","content":"three"}}"#,
            "\n",
        ),
    )
    .expect("write");
    let document = PiParser
        .read(&locator(dir.path().to_path_buf(), "s"))
        .expect("parse");
    // The leaf `c` points at a missing parent, so only it renders.
    assert_eq!(shape(&document), [(TurnKind::Message, false, "three")]);
}

#[test]
fn a_parent_cycle_stops_instead_of_looping() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("2026-10-16T09-00-00-000Z_s.jsonl"),
        concat!(
            r#"{"type":"session","version":3,"id":"s","cwd":"/w"}"#,
            "\n",
            r#"{"type":"message","id":"a","parentId":"b","message":{"role":"user","content":"one"}}"#,
            "\n",
            r#"{"type":"message","id":"b","parentId":"a","message":{"role":"assistant","content":"two"}}"#,
            "\n",
        ),
    )
    .expect("write");
    let document = PiParser
        .read(&locator(dir.path().to_path_buf(), "s"))
        .expect("parse");
    assert_eq!(
        shape(&document),
        [
            (TurnKind::Message, false, "one"),
            (TurnKind::Message, false, "two"),
        ]
    );
}
