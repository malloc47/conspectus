//! The log: bounded, newest first, repeats collapsed, and the unseen
//! count of warnings and errors, over a payload of the caller's type.

use super::*;

/// A payload of the test's own: what an entry is about.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct About {
    subject: Option<&'static str>,
}

fn entry(at: i64, severity: Severity, summary: &str) -> Entry<About> {
    Entry::new(at, severity, summary, About::default())
}

fn about(subject: &'static str) -> About {
    About {
        subject: Some(subject),
    }
}

#[test]
fn the_log_keeps_its_newest_entries_up_to_its_capacity() {
    let mut log = Log::new(3);
    for n in 0..5 {
        log.push(entry(n, Severity::Success, &format!("entry {n}")));
    }
    assert_eq!(log.len(), 3);
    assert_eq!(log.capacity(), 3);
    let summaries: Vec<&str> = log.newest_first().map(|e| e.summary.as_str()).collect();
    assert_eq!(summaries, ["entry 4", "entry 3", "entry 2"]);
    assert_eq!(
        Log::<About>::default().capacity(),
        Log::<About>::DEFAULT_CAPACITY
    );
    assert_eq!(Log::<About>::new(0).capacity(), 1, "it keeps at least one");
}

#[test]
fn only_warnings_and_errors_count_as_unseen() {
    let mut log = Log::default();
    for severity in Severity::ALL {
        log.push(entry(0, severity, &format!("{severity:?}")));
    }
    assert_eq!(log.unseen(), 2);
    log.mark_seen();
    assert_eq!(log.unseen(), 0);
    assert_eq!(log.len(), 4, "seeing the log keeps its entries");
    assert!(!log.is_empty());
}

#[test]
fn back_to_back_duplicates_collapse_into_a_repeat_count() {
    let mut log = Log::default();
    log.push(entry(10, Severity::Error, "refresh failed"));
    log.push(entry(20, Severity::Error, "refresh failed"));
    log.push(entry(30, Severity::Error, "refresh failed"));
    assert_eq!(log.len(), 1);
    assert_eq!(log.unseen(), 1, "a repeat is not a new failure");
    let newest = log.newest_first().next().expect("an entry");
    assert_eq!(
        (newest.repeats, newest.at),
        (3, 30),
        "the time moves forward"
    );

    log.push(entry(40, Severity::Error, "refresh failed: other"));
    log.push(entry(50, Severity::Error, "refresh failed"));
    assert_eq!(log.len(), 3, "only consecutive duplicates collapse");
}

#[test]
fn a_repeated_entry_reads_with_its_count() {
    let mut log = Log::default();
    log.push(entry(10, Severity::Error, "refresh failed"));
    let once = log.newest_first().next().expect("an entry");
    assert_eq!(once.summary_with_repeats(), "refresh failed");
    log.push(entry(20, Severity::Error, "refresh failed"));
    log.push(entry(30, Severity::Error, "refresh failed"));
    let thrice = log.newest_first().next().expect("an entry");
    assert_eq!(
        thrice.summary_with_repeats(),
        format!("refresh failed ({}3)", crate::glyphs::marks::TIMES)
    );
}

#[test]
fn entries_that_differ_in_severity_or_payload_do_not_collapse() {
    let mut log = Log::default();
    log.push(entry(0, Severity::Warning, "launch"));
    log.push(entry(0, Severity::Error, "launch"));
    log.push(Entry::new(0, Severity::Error, "launch", about("a")));
    log.push(Entry::new(0, Severity::Error, "launch", about("b")));
    assert_eq!(log.len(), 4);
    log.push(Entry::new(5, Severity::Error, "launch", about("b")));
    assert_eq!(log.len(), 4, "the same payload again is a repeat");
}

#[test]
fn newest_where_finds_the_latest_entry_about_a_subject() {
    let mut log = Log::default();
    log.push(Entry::new(0, Severity::Error, "launch failed", about("a")));
    log.push(Entry::new(1, Severity::Success, "launched", about("b")));
    let latest_about = |log: &Log<About>, subject| {
        log.newest_where(|e| e.item.subject == Some(subject))
            .map(|e| e.summary.clone())
    };
    assert_eq!(latest_about(&log, "a").as_deref(), Some("launch failed"));

    log.push(Entry::new(2, Severity::Success, "launched", about("a")));
    assert_eq!(
        latest_about(&log, "a").as_deref(),
        Some("launched"),
        "a later success is the newest entry about it"
    );
    assert!(latest_about(&log, "c").is_none());
}

#[test]
fn an_entry_reads_as_the_feedback_it_summarizes() {
    let e = entry(0, Severity::Warning, "resume failed, launched fresh");
    let feedback = e.feedback();
    assert_eq!(feedback, Feedback::warning("resume failed, launched fresh"));
}
