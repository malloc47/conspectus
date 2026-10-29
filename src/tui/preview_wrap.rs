//! The Preview's wrap mode as Conspectus spells it (ADR 0106).
//!
//! The modes and the layout of a captured pane are the framework's
//! (`tuikit::richtext::WrapMode` and `layout_capture`, ADR 0181). What is
//! Conspectus's is the setting: the token `[tui] preview_wrap` and the
//! TUI state file write, and the label the controls overlay shows.

use tuikit::richtext::WrapMode;

/// Stable token used for config and persistence.
pub fn token(mode: WrapMode) -> &'static str {
    match mode {
        WrapMode::Plain => "plain",
        WrapMode::Smart => "smart",
        WrapMode::None => "none",
    }
}

/// Human label for the controls overlay.
pub fn label(mode: WrapMode) -> &'static str {
    match mode {
        WrapMode::Plain => "plain wrap",
        WrapMode::Smart => "smart wrap",
        WrapMode::None => "no wrap",
    }
}

/// The mode `raw` names, ignoring surrounding whitespace; `None` for a
/// token no mode has.
pub fn parse(raw: &str) -> Option<WrapMode> {
    match raw.trim() {
        "plain" => Some(WrapMode::Plain),
        "smart" => Some(WrapMode::Smart),
        "none" => Some(WrapMode::None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_reads_back_from_its_token() {
        for mode in WrapMode::ALL {
            assert_eq!(parse(token(mode)), Some(mode));
            assert_eq!(parse(&format!(" {} ", token(mode))), Some(mode));
        }
        assert_eq!(parse("wrap"), None);
    }
}
