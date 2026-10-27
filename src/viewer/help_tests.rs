use super::*;

/// A position for every message; the exhaustive match makes a new
/// message fail to compile until it is placed in [`ViewerMsg::ALL`].
fn position(msg: ViewerMsg) -> usize {
    match msg {
        ViewerMsg::ScrollDown => 0,
        ViewerMsg::ScrollUp => 1,
        ViewerMsg::PageDown => 2,
        ViewerMsg::PageUp => 3,
        ViewerMsg::HalfPageDown => 4,
        ViewerMsg::HalfPageUp => 5,
        ViewerMsg::JumpToStart => 6,
        ViewerMsg::JumpToEnd => 7,
        ViewerMsg::CycleToolDetail => 8,
        ViewerMsg::ToggleThinking => 9,
        ViewerMsg::ToggleAborted => 10,
        ViewerMsg::ToggleHidden => 11,
        ViewerMsg::FindOpen => 12,
        ViewerMsg::FindNext => 13,
        ViewerMsg::FindPrev => 14,
        ViewerMsg::ToggleHelp => 15,
        ViewerMsg::Close => 16,
        ViewerMsg::Quit => 17,
    }
}

#[test]
fn every_message_is_listed_once_at_its_position() {
    for msg in ViewerMsg::ALL {
        assert_eq!(ViewerMsg::ALL[position(msg)], msg);
    }
    let mut seen = ViewerMsg::ALL.to_vec();
    seen.dedup();
    assert_eq!(seen.len(), ViewerMsg::ALL.len());
}

#[test]
fn every_message_has_a_sentence_and_a_section() {
    for msg in ViewerMsg::ALL {
        let entry = msg.help();
        assert!(!entry.section.is_empty(), "{msg:?}");
        assert!(entry.description.len() > 6, "{msg:?}");
        assert!(
            !entry.description.ends_with('.'),
            "{msg:?} reads as a label"
        );
    }
}

#[test]
fn sections_are_contiguous_in_the_listing() {
    let mut order: Vec<&str> = Vec::new();
    for msg in ViewerMsg::ALL {
        let section = msg.help().section;
        match order.last() {
            Some(last) if *last == section => {}
            _ => {
                assert!(!order.contains(&section), "`{section}` is split up");
                order.push(section);
            }
        }
    }
}
