// Extracted from fixtures.rs H-HYG-011 rolling wave via #[path = "fixtures_tests.rs"] mod tests;
use super::*;
use tempfile::TempDir;

#[test]
fn fixture_paths_live_under_supplied_root() {
    let temp = TempDir::new().expect("temp dir");
    let fixture = HarnessFixture::at(temp.path());

    assert!(fixture.codex_state_root().starts_with(temp.path()));
    assert!(fixture.claude_code_state_root().starts_with(temp.path()));
    assert!(fixture.opencode_state_root().starts_with(temp.path()));
}

#[test]
fn codex_session_writes_session_meta_with_cwd_and_timestamp() {
    let temp = TempDir::new().expect("temp dir");
    let fixture = HarnessFixture::at(temp.path());

    let path = fixture
        .write_codex_session(
            &CodexSessionRecord::new("11111111")
                .with_cwd("/work/repo")
                .with_timestamp("2026-01-02T03:04:05Z"),
        )
        .expect("write codex session");

    let body = fs::read_to_string(&path).expect("read codex session");
    let parsed: serde_json::Value =
        serde_json::from_str(body.lines().next().expect("first line")).expect("parse jsonl");

    assert_eq!(parsed["type"], "session_meta");
    assert_eq!(parsed["payload"]["id"], "11111111");
    assert_eq!(parsed["payload"]["cwd"], "/work/repo");
    assert_eq!(parsed["payload"]["timestamp"], "2026-01-02T03:04:05Z");
}

#[test]
fn codex_session_omits_optional_fields_when_absent() {
    let temp = TempDir::new().expect("temp dir");
    let fixture = HarnessFixture::at(temp.path());

    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("only-id"))
        .expect("write codex session");

    let body = fs::read_to_string(&path).expect("read codex session");
    let parsed: serde_json::Value =
        serde_json::from_str(body.lines().next().expect("first line")).expect("parse jsonl");

    assert_eq!(parsed["payload"]["id"], "only-id");
    assert!(parsed["payload"].get("cwd").is_none());
    assert!(parsed["payload"].get("timestamp").is_none());
}

#[test]
fn claude_code_session_encodes_cwd_in_project_dir() {
    let temp = TempDir::new().expect("temp dir");
    let fixture = HarnessFixture::at(temp.path());

    let path = fixture
        .write_claude_code_session(&ClaudeCodeSessionRecord::new("uuid-a", "/work/repo"))
        .expect("write claude session");

    assert_eq!(path.file_name().unwrap(), "uuid-a.jsonl");
    let project_dir = path.parent().expect("parent");
    assert_eq!(project_dir.file_name().unwrap(), "-work-repo");
}

#[test]
fn opencode_session_records_time_fields() {
    let temp = TempDir::new().expect("temp dir");
    let fixture = HarnessFixture::at(temp.path());

    let path = fixture
        .write_opencode_session(
            &OpenCodeSessionRecord::new("session-1")
                .with_directory("/work/repo")
                .with_created(1_700_000_000_000)
                .with_updated(1_700_000_500_000),
        )
        .expect("write opencode session");

    let info: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read info")).expect("parse");

    assert_eq!(info["directory"], "/work/repo");
    assert_eq!(info["time"]["created"], 1_700_000_000_000_i64);
    assert_eq!(info["time"]["updated"], 1_700_000_500_000_i64);
}

#[test]
fn aider_state_writes_history_marker_files() {
    let temp = TempDir::new().expect("temp dir");
    let repo = temp.path().join("repo");

    let history = write_aider_state(&repo).expect("write aider state");

    assert!(history.exists());
    assert!(repo.join(".aider.input.history").exists());
}

#[test]
fn malformed_record_is_unparseable() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("nested").join("bad.jsonl");

    write_malformed(&path).expect("write malformed");

    let body = fs::read_to_string(&path).expect("read bad");
    assert!(serde_json::from_str::<serde_json::Value>(&body).is_err());
}
