//! The styles the viewer draws with.
//!
//! The viewer draws from the framework theme (`tuikit::theme::Theme`) it
//! is given, as every control does. Roles any program has take the
//! framework's tokens: the rules and separators `inert` with `muted`,
//! bodies that recede `muted`, a line-number prefix `inert`, an assistant
//! turn's chip `success`, a user turn's `accent`, a tool turn's `warning`,
//! thinking and system turns' `inert`, the cwd in the header `accent` with
//! `muted`, matches the theme's match style and the current one
//! `selection_active`.
//!
//! Roles that are the transcript's own are named styles the host
//! registers in that theme (`Theme::register`), under the names here, so
//! its configuration can set them; a host that registers none gets the
//! fallback each names. The header's title takes the named style the host
//! hands the content (a harness's identity color, say): its name is the
//! host's, not the viewer's.

use ratatui::style::Color;
use tuikit::theme::Theme;

/// The named style of a compaction summary's chip and thread.
pub const COMPACTION: &str = "transcript_compaction";

/// The color of a compaction summary's chip: the [`COMPACTION`] style the
/// host registered, or the `link` token.
pub fn compaction(theme: &Theme) -> Color {
    theme.color(COMPACTION).unwrap_or(theme.link)
}

/// The color of the header's title: the named style `role` the host
/// handed the content, or the `accent` token when it registered none.
pub fn title(theme: &Theme, role: &str) -> Color {
    theme.color(role).unwrap_or(theme.accent)
}
