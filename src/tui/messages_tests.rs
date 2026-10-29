use super::*;

fn pin(id: &str) -> LogTarget {
    LogTarget::Pin(id.to_string())
}

#[test]
fn the_log_keeps_the_messages_capacity() {
    let mut log = message_log();
    for n in 0..MESSAGE_LOG_CAPACITY + 5 {
        log.push(LogEntry::success(format!("entry {n}")));
    }

    assert_eq!(log.len(), MESSAGE_LOG_CAPACITY);
    let oldest = log.newest_first().last().expect("entries");
    assert_eq!(oldest.summary, "entry 5");
}

#[test]
fn latest_failure_is_hidden_by_a_later_success_on_the_same_target() {
    let mut log = message_log();
    log.push(LogEntry::error("launch failed").with_target(pin("w1")));
    log.push(LogEntry::success("other pin launched").with_target(pin("w2")));
    assert_eq!(
        latest_failure_for(&log, &[pin("w1")]).map(|e| e.summary.as_str()),
        Some("launch failed")
    );
    assert!(latest_failure_for(&log, &[pin("w2")]).is_none());

    log.push(LogEntry::success("launched").with_target(pin("w1")));
    assert!(latest_failure_for(&log, &[pin("w1")]).is_none());
}

#[test]
fn entries_about_different_targets_do_not_collapse() {
    let mut log = message_log();
    log.push(LogEntry::error("launch failed").with_target(pin("w1")));
    log.push(LogEntry::error("launch failed").with_target(pin("w2")));
    log.push(LogEntry::error("launch failed").with_target(pin("w2")));
    assert_eq!(log.len(), 2, "the target is part of what repeats");
    assert_eq!(log.unseen(), 2);
}

#[test]
fn full_text_carries_argv_exit_status_and_both_streams() {
    let entry = LogEntry::error("pin `w1` launch failed")
        .with_target(pin("w1"))
        .with_command(CommandRecord {
            argv: vec!["conspectus".into(), "pin".into(), "launch".into()],
            exit_code: Some(1),
            stdout: "spawned".into(),
            stderr: "No conversation found\nResume this session with:".into(),
        });

    let text = entry.full_text();

    assert!(text.contains("× pin `w1` launch failed"), "{text}");
    assert!(text.contains("target: pin w1"), "{text}");
    assert!(text.contains("argv: conspectus pin launch"), "{text}");
    assert!(text.contains("exit: 1"), "{text}");
    assert!(
        text.contains("--- stderr ---\nNo conversation found"),
        "{text}"
    );
    assert!(text.contains("--- stdout ---\nspawned"), "{text}");
}

#[test]
fn full_text_leads_with_the_severity_mark() {
    for (entry, mark) in [
        (LogEntry::info("noted"), "•"),
        (LogEntry::success("attached"), "√"),
        (LogEntry::warning("fell back"), "▲"),
        (LogEntry::error("failed"), "×"),
    ] {
        let text = entry.full_text();
        assert!(
            text.contains(&format!(" {mark} {}", entry.summary)),
            "{text}"
        );
    }
}

#[test]
fn output_tail_prefers_detail_then_stderr() {
    let command = CommandRecord {
        stderr: "one\ntwo\n\nthree".into(),
        stdout: "out".into(),
        ..CommandRecord::default()
    };
    let from_stderr = LogEntry::error("x").with_command(command.clone());
    assert_eq!(from_stderr.output_tail(2), ["two", "three"]);

    let from_detail = LogEntry::error("x")
        .with_command(command)
        .with_detail("pane line");
    assert_eq!(from_detail.output_tail(5), ["pane line"]);
}

#[test]
fn blank_detail_is_dropped() {
    assert!(
        LogEntry::success("x")
            .with_detail("  \n")
            .item
            .detail
            .is_none()
    );
}

#[test]
fn clock_renders_utc_time_of_day() {
    assert_eq!(clock(0), "00:00:00 UTC");
    assert_eq!(clock(1_790_874_052), "17:00:52 UTC");
}

#[test]
fn repeats_show_as_a_count_after_the_summary() {
    let mut log = message_log();
    for _ in 0..3 {
        log.push(LogEntry::error("refresh failed: daemon gone"));
    }

    assert_eq!(log.len(), 1);
    let entry = log.newest_first().next().expect("entry");
    assert_eq!(
        entry.summary_with_repeats(),
        "refresh failed: daemon gone (×3)"
    );
}
