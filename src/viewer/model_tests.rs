// Extracted from model.rs H-HYG-011 rolling wave via #[path = "model_tests.rs"] mod tests;
use super::*;
use chrono::TimeZone;

fn sample_locator_claude() -> SessionLocator {
    SessionLocator {
        harness_key: "claude-code".to_string(),
        session_key: "0b34e59c-14d0-4d04-be79-4dc1d4c120c2".to_string(),
        state_root: PathBuf::from("/home/u/.claude"),
    }
}

fn sample_locator_codex() -> SessionLocator {
    SessionLocator {
        harness_key: "codex".to_string(),
        session_key: "019df146-41e8-7fb0-8df0-dc326b4fdee8".to_string(),
        state_root: PathBuf::from("/home/u/.codex"),
    }
}

fn sample_locator_opencode() -> SessionLocator {
    SessionLocator {
        harness_key: "opencode".to_string(),
        session_key: "ses_17f328a8effeK52nvLaEV954yO".to_string(),
        state_root: PathBuf::from("/home/u/.local/share/opencode/opencode.db"),
    }
}

#[test]
fn locator_display_matches_harness_session_form() {
    assert_eq!(
        sample_locator_claude().to_string(),
        "claude-code:0b34e59c-14d0-4d04-be79-4dc1d4c120c2"
    );
    assert_eq!(
        sample_locator_codex().to_string(),
        "codex:019df146-41e8-7fb0-8df0-dc326b4fdee8"
    );
    assert_eq!(
        sample_locator_opencode().to_string(),
        "opencode:ses_17f328a8effeK52nvLaEV954yO"
    );
}

#[test]
fn locator_harness_key_round_trips() {
    assert_eq!(sample_locator_claude().harness_key(), "claude-code");
    assert_eq!(sample_locator_codex().harness_key(), "codex");
    assert_eq!(sample_locator_opencode().harness_key(), "opencode");
}

#[test]
fn locator_serde_round_trip_claude() {
    let original = sample_locator_claude();
    let json = serde_json::to_string(&original).expect("serialize");
    // H-EXT-006 flat shape: `harness_key` field replaces the
    // pre-H-EXT-006 `harness` tag; external bin / future
    // config files write `{"harness_key": "claude-code",
    // ...}`.
    assert!(
        json.contains("\"harness_key\":\"claude-code\""),
        "got {json}"
    );
    let decoded: SessionLocator = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, original);
}

#[test]
fn locator_serde_round_trip_codex() {
    let original = sample_locator_codex();
    let json = serde_json::to_string(&original).expect("serialize");
    let decoded: SessionLocator = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, original);
}

#[test]
fn locator_serde_round_trip_opencode() {
    let original = sample_locator_opencode();
    let json = serde_json::to_string(&original).expect("serialize");
    assert!(json.contains("\"harness_key\":\"opencode\""), "got {json}");
    let decoded: SessionLocator = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, original);
}

#[test]
fn turn_kind_shown_by_default_filters_noise() {
    assert!(TurnKind::Message.shown_by_default());
    assert!(TurnKind::CompactionSummary.shown_by_default());
    assert!(!TurnKind::ToolUse.shown_by_default());
    assert!(!TurnKind::ToolResult.shown_by_default());
    assert!(!TurnKind::Thinking.shown_by_default());
}

#[test]
fn turn_role_header_labels_match_inline_preview() {
    assert_eq!(TurnRole::User.header_label(), "you");
    assert_eq!(TurnRole::Assistant.header_label(), "assistant");
    assert_eq!(TurnRole::System.header_label(), "system");
}

#[test]
fn transcript_turn_serde_round_trip_with_timestamp() {
    let original = TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        body: "Hello **world**.".to_string(),
        timestamp: Some(
            Utc.with_ymd_and_hms(2026, 6, 1, 16, 52, 36)
                .single()
                .expect("valid timestamp"),
        ),
        aborted: false,
    };
    let json = serde_json::to_string(&original).expect("serialize");
    let decoded: TranscriptTurn = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, original);
}

#[test]
fn transcript_turn_serde_omits_absent_timestamp() {
    let turn = TranscriptTurn {
        role: TurnRole::User,
        kind: TurnKind::Message,
        body: "hi".to_string(),
        timestamp: None,
        aborted: false,
    };
    let json = serde_json::to_string(&turn).expect("serialize");
    assert!(!json.contains("timestamp"), "got {json}");
    let decoded: TranscriptTurn = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, turn);
}

#[test]
fn transcript_document_unavailable_carries_meta_and_no_turns() {
    let doc = TranscriptDocument::unavailable(&sample_locator_claude());
    assert!(doc.is_empty());
    assert_eq!(doc.meta.harness, "claude-code");
    assert_eq!(doc.meta.session_key, "0b34e59c-14d0-4d04-be79-4dc1d4c120c2");
    assert!(doc.meta.cwd.is_none());
}

#[test]
fn transcript_document_serde_round_trip() {
    let original = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "abc-123".to_string(),
            cwd: Some("/home/u/src/proj".to_string()),
        },
        turns: vec![
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "what's up?".to_string(),
                timestamp: None,
                aborted: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Message,
                body: "not much".to_string(),
                timestamp: Some(
                    Utc.with_ymd_and_hms(2026, 6, 1, 17, 0, 0)
                        .single()
                        .expect("valid timestamp"),
                ),
                aborted: false,
            },
        ],
    };
    let json = serde_json::to_string(&original).expect("serialize");
    let decoded: TranscriptDocument = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, original);
}
