use super::*;

/// A position for every toggle; the exhaustive match makes a new key
/// fail to compile until it is placed in the listing.
fn position(key: TranscriptKey) -> usize {
    match key {
        TranscriptKey::Tools => 0,
        TranscriptKey::Thinking => 1,
        TranscriptKey::Aborted => 2,
        TranscriptKey::Hidden => 3,
    }
}

#[test]
fn every_toggle_is_listed_once_at_its_position() {
    for &key in TranscriptKey::LISTED {
        assert_eq!(TranscriptKey::LISTED[position(key)], key);
    }
    let mut seen = TranscriptKey::LISTED.to_vec();
    seen.dedup();
    assert_eq!(seen.len(), TranscriptKey::LISTED.len());
}

#[test]
fn every_toggle_has_a_sentence_in_the_one_section() {
    for &key in TranscriptKey::LISTED {
        let entry = key.help();
        assert_eq!(entry.section, SHOWING, "{key:?}");
        assert!(entry.description.len() > 6, "{key:?}");
        assert!(
            !entry.description.ends_with('.'),
            "{key:?} reads as a label"
        );
    }
}

#[test]
fn the_viewer_rewords_only_the_ends_and_the_find() {
    let reworded: Vec<PagerKey> = <PagerKey as HelpAction>::LISTED
        .iter()
        .copied()
        .filter(|&key| describe(key).is_some())
        .collect();
    assert_eq!(reworded, [PagerKey::Top, PagerKey::Bottom, PagerKey::Find]);
}
