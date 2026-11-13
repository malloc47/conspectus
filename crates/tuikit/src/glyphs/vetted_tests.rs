use super::*;

const NOTE: &str = "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous";

#[test]
fn the_framework_table_has_no_risk_duplicate_or_missing_note() {
    let table = VettedGlyphs::default();
    assert!(!table.is_empty());
    let mut seen = std::collections::HashSet::new();
    for (glyph, note) in table.iter() {
        assert!(seen.insert(glyph), "`{glyph}` is listed twice");
        assert!(!note.trim().is_empty(), "`{glyph}` needs a note");
        assert_eq!(glyph_risk(glyph), None, "`{glyph}` is risky");
    }
    assert_eq!(seen.len(), table.len());
}

#[test]
fn the_framework_table_holds_every_mark() {
    let table = VettedGlyphs::default();
    for glyph in marks::ALL {
        assert!(table.contains(glyph), "mark `{glyph}` is not vetted");
        assert!(table.note(glyph).is_some(), "mark `{glyph}` has no note");
    }
}

#[test]
fn the_two_operator_preferences_say_so_in_their_notes() {
    let table = VettedGlyphs::default();
    let collapsed = table.note(marks::COLLAPSED).unwrap();
    assert!(collapsed.contains("operator preference"), "{collapsed}");
    for frame in marks::SPINNER {
        let note = table.note(frame).unwrap();
        assert!(note.contains("operator preference"), "{note}");
    }
}

#[test]
fn registering_adds_a_glyph_with_its_note() {
    let mut table = VettedGlyphs::empty();
    assert!(!table.contains("◆"));
    table.register("◆", NOTE).unwrap();
    assert!(table.contains("◆"));
    assert_eq!(table.note("◆"), Some(NOTE));
    assert_eq!(table.len(), 1);
    assert_eq!(table.iter().collect::<Vec<_>>(), [("◆", NOTE)]);
}

#[test]
fn a_duplicate_is_refused_and_changes_nothing() {
    let mut table = VettedGlyphs::empty();
    table.register("◆", NOTE).unwrap();
    assert_eq!(
        table.register("◆", "another note"),
        Err(RegisterError::Duplicate { glyph: "◆" })
    );
    assert_eq!(table.note("◆"), Some(NOTE));
    assert_eq!(table.len(), 1);
}

#[test]
fn a_registration_needs_a_note() {
    let mut table = VettedGlyphs::empty();
    assert_eq!(
        table.register("◆", "  "),
        Err(RegisterError::NoNote { glyph: "◆" })
    );
    assert!(table.is_empty());
}

#[test]
fn a_risky_glyph_is_refused_with_the_reason() {
    let mut table = VettedGlyphs::empty();
    for glyph in ["東", "ab", "⚠", "😀"] {
        let error = table.register(glyph, NOTE).unwrap_err();
        assert!(matches!(error, RegisterError::Risky { .. }), "{error:?}");
    }
    assert!(table.is_empty());
    let message = table.register("⚠", NOTE).unwrap_err().to_string();
    assert!(message.contains("can take emoji presentation"), "{message}");
}

#[test]
fn register_all_stops_at_the_first_error_and_keeps_what_came_before() {
    let mut table = VettedGlyphs::empty();
    let result = table.register_all(&[("◆", NOTE), ("東", NOTE), ("◇", NOTE)]);
    assert!(matches!(
        result,
        Err(RegisterError::Risky { glyph: "東", .. })
    ));
    assert!(table.contains("◆"));
    assert!(!table.contains("◇"));
}

#[test]
fn printable_ascii_is_always_vetted_and_other_glyphs_are_not_until_registered() {
    let table = VettedGlyphs::empty();
    for glyph in ["?", "a", " ", "~"] {
        assert!(table.contains(glyph), "`{glyph}`");
    }
    for glyph in ["◆", "é", "", "ab", "\u{7f}", "\n"] {
        assert!(!table.contains(glyph), "`{glyph:?}`");
    }
    assert_eq!(table.note("?"), None, "ASCII needs no entry");
}

#[test]
fn the_error_messages_name_the_glyph_and_the_fix() {
    let duplicate = RegisterError::Duplicate { glyph: "◆" }.to_string();
    assert!(
        duplicate.contains("`◆`") && duplicate.contains("twice"),
        "{duplicate}"
    );
    let missing = RegisterError::NoNote { glyph: "◆" }.to_string();
    assert!(missing.contains("glyph-widths.py"), "{missing}");
}

#[test]
fn unvetted_reports_only_the_glyphs_no_entry_covers() {
    let mut table = VettedGlyphs::default();
    table.register("◆", NOTE).unwrap();
    let source = "const A: &str = \"◆ ► ☃\";\nconst B: &str = \"é\";\n// ☃ in a comment\n";
    let found: Vec<(usize, char)> = table
        .unvetted(source)
        .into_iter()
        .map(|drawn| (drawn.line, drawn.glyph))
        .collect();
    assert_eq!(found, [(1, '☃'), (2, 'é')]);
}
