use super::*;

#[test]
fn every_mark_is_one_cell_and_free_of_width_risk() {
    for glyph in marks::ALL {
        assert_eq!(cell_width(glyph), 1, "`{glyph}` is not one cell wide");
        assert_eq!(glyph_risk(glyph), None, "`{glyph}` is risky");
    }
}

#[test]
fn the_mark_list_is_every_mark_once() {
    let mut seen = std::collections::BTreeSet::new();
    for glyph in marks::ALL {
        assert!(seen.insert(*glyph), "`{glyph}` is listed twice in ALL");
    }
    for frame in marks::SPINNER {
        assert!(seen.contains(frame), "spinner frame `{frame}` is missing");
    }
    // The list and the module's source agree: a mark added to the file
    // and left off the list fails here.
    let drawn: std::collections::BTreeSet<String> = drawn_glyphs(include_str!("glyphs/marks.rs"))
        .into_iter()
        .map(|drawn| drawn.glyph.to_string())
        .collect();
    let listed: std::collections::BTreeSet<String> = marks::ALL
        .iter()
        .map(|glyph| (*glyph).to_string())
        .collect();
    assert_eq!(drawn, listed);
}

#[test]
fn the_pointer_and_the_back_mark_are_the_plain_pointers_not_emoji_capable_ones() {
    assert_eq!(marks::POINTER, "\u{25BA}");
    assert_eq!(marks::BACK, "\u{25C4}");
    assert_ne!(marks::POINTER, marks::COLLAPSED, "look-alike codepoints");
}

#[test]
fn the_spinner_is_eight_braille_frames() {
    assert_eq!(marks::SPINNER.len(), 8);
    for frame in marks::SPINNER {
        let c = frame.chars().next().unwrap();
        assert!(('\u{2800}'..='\u{28FF}').contains(&c), "`{frame}`");
    }
}

#[test]
fn the_modules_own_sources_draw_only_vetted_glyphs() {
    let table = VettedGlyphs::default();
    for (name, source) in [
        ("glyphs.rs", include_str!("glyphs.rs")),
        ("glyphs/marks.rs", include_str!("glyphs/marks.rs")),
        ("glyphs/scan.rs", include_str!("glyphs/scan.rs")),
        ("glyphs/vetted.rs", include_str!("glyphs/vetted.rs")),
    ] {
        let found = table.unvetted(source);
        assert!(found.is_empty(), "{name} draws unvetted glyphs: {found:?}");
    }
}

#[test]
fn cell_width_counts_terminal_cells() {
    assert_eq!(cell_width(""), 0);
    assert_eq!(cell_width("a"), 1);
    assert_eq!(cell_width(marks::POINTER), 1);
    assert_eq!(cell_width("東"), 2);
    assert_eq!(cell_width("ab"), 2);
}

#[test]
fn risky_glyphs_are_rejected() {
    for glyph in [
        "◀",
        "⚠",
        "⚙",
        "✔",
        "😀",
        "東",
        "ab",
        "",
        "\u{200D}",
        "▶\u{FE0F}",
    ] {
        assert!(glyph_risk(glyph).is_some(), "`{glyph}` should be rejected");
    }
}

#[test]
fn safe_glyphs_are_allowed() {
    for glyph in ["▼", "▶", "◉", "⊸", "?", "►", "◄", "▲", "—"] {
        assert_eq!(glyph_risk(glyph), None, "`{glyph}` should be allowed");
    }
}

#[test]
fn the_collapsed_triangle_is_the_one_emoji_exemption() {
    assert_eq!(glyph_risk(marks::COLLAPSED), None);
    // The same codepoint with the emoji selector is still refused, and so is
    // its mirror image.
    assert!(glyph_risk("\u{25B6}\u{FE0F}").is_some());
    assert!(glyph_risk("\u{25C0}").is_some());
}

#[test]
fn the_reasons_say_what_is_wrong() {
    assert_eq!(
        glyph_risk("東"),
        Some("not exactly one cell wide, or more than one character")
    );
    assert_eq!(glyph_risk("⚠"), Some("can take emoji presentation"));
    assert_eq!(
        glyph_risk("😀").map(|why| why.contains("one cell")),
        Some(true)
    );
}
