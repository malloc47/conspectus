use super::*;
use std::path::PathBuf;

#[test]
fn shorten_home_replaces_exact_home_prefix() {
    let home = PathBuf::from("/home/op");
    assert_eq!(shorten_home("/home/op", Some(&home)), "~");
    assert_eq!(shorten_home("/home/op/src/x", Some(&home)), "~/src/x");
}

#[test]
fn shorten_home_keeps_non_home_paths_intact() {
    let home = PathBuf::from("/home/op");
    assert_eq!(shorten_home("/var/log", Some(&home)), "/var/log");
    // Prefix match must respect path boundaries — `/home/operator`
    // is NOT `/home/op/erator`.
    assert_eq!(
        shorten_home("/home/operator", Some(&home)),
        "/home/operator"
    );
}

#[test]
fn shorten_home_with_trailing_slash_in_home_still_matches() {
    let home = PathBuf::from("/home/op/");
    assert_eq!(shorten_home("/home/op/src", Some(&home)), "~/src");
}

#[test]
fn shorten_home_no_home_is_identity() {
    assert_eq!(shorten_home("/home/op/x", None), "/home/op/x");
}

#[test]
fn harness_label_shortens_claude_code() {
    assert_eq!(harness_label("claude-code"), "claude");
    assert_eq!(harness_label("codex"), "codex");
    assert_eq!(harness_label("opencode"), "opencode");
}

#[test]
fn format_recency_buckets_seconds_minutes_hours_days() {
    let now = Some(1_000_000);
    assert_eq!(format_recency(now, Some(999_990)), Some("10s".to_string()));
    assert_eq!(
        format_recency(now, Some(1_000_000 - 120)),
        Some("2m".to_string())
    );
    assert_eq!(
        format_recency(now, Some(1_000_000 - 3 * 3600)),
        Some("3h".to_string())
    );
    assert_eq!(
        format_recency(now, Some(1_000_000 - 5 * 86400)),
        Some("5d".to_string())
    );
}

#[test]
fn format_recency_clamps_future_timestamps_to_zero() {
    let now = Some(1_000_000);
    assert_eq!(format_recency(now, Some(1_000_500)), Some("0s".to_string()));
}

#[test]
fn format_recency_is_none_when_either_side_missing() {
    assert_eq!(format_recency(None, Some(100)), None);
    assert_eq!(format_recency(Some(100), None), None);
}

#[test]
fn recency_bucket_picks_bucket_per_age() {
    let now = Some(1_000_000);
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 60)),
        Some(RecencyBucket::Fresh),
        "1m ago is Fresh",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 5 * 60)),
        Some(RecencyBucket::Active),
        "exactly 5m ago crosses Fresh→Active",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 30 * 60)),
        Some(RecencyBucket::Active),
        "30m ago is Active",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 60 * 60)),
        Some(RecencyBucket::Recent),
        "exactly 1h ago crosses Active→Recent",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 12 * 60 * 60)),
        Some(RecencyBucket::Recent),
        "12h ago is Recent",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 24 * 60 * 60)),
        Some(RecencyBucket::Cold),
        "exactly 1d ago crosses Recent→Cold",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 7 * 24 * 60 * 60)),
        Some(RecencyBucket::Cold),
        "7d ago is Cold",
    );
}

#[test]
fn recency_bucket_is_none_when_either_side_missing() {
    assert_eq!(recency_bucket(None, Some(100)), None);
    assert_eq!(recency_bucket(Some(100), None), None);
}

fn program_test_pin(harness: &str, launch_argv: Option<&[&str]>) -> crate::model::PinCandidate {
    use crate::model::{PinCandidate, PinMuxRef, Provenance};
    PinCandidate {
        id: "p".to_string(),
        display_name: "p".to_string(),
        harness: harness.to_string(),
        cwd: "/p".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "x".to_string(),
            socket_name: None,
        },
        launch_argv: launch_argv.map(|argv| argv.iter().map(ToString::to_string).collect()),
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/.conspectus.toml".to_string(),
        binding: None,
    }
}

#[test]
fn mux_program_prefers_the_current_command_then_start_command_then_pin() {
    use crate::model::{MuxSessionId, MuxSessionNode};
    let bare = MuxSessionNode::new(MuxSessionId::new("tmux:x"), "tmux", "x");
    let serve_pin = program_test_pin("conspectus", Some(&["conspectus", "serve"]));

    let current = bare.clone().with_active_pane_command("/usr/bin/npm");
    assert_eq!(
        mux_program(&current, Some(&serve_pin)).as_deref(),
        Some("npm")
    );

    let mut started = bare.clone().with_active_pane_command("  ");
    started.active_pane_start_command = Some("/nix/store/x/bin/node server.js".to_string());
    assert_eq!(
        mux_program(&started, Some(&serve_pin)).as_deref(),
        Some("node")
    );

    assert_eq!(
        mux_program(&bare, Some(&serve_pin)).as_deref(),
        Some("conspectus")
    );
    assert_eq!(mux_program(&bare, None), None);
}

#[test]
fn pin_program_falls_back_to_the_harness_default_argv() {
    assert_eq!(
        pin_program(&program_test_pin("claude-code", None)).as_deref(),
        Some("claude")
    );
    assert_eq!(
        pin_program(&program_test_pin("custom-launcher", None)).as_deref(),
        Some("custom-launcher")
    );
}
