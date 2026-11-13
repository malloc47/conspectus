//! Compact labels for the panels: a long mux id kept to its head and tail.
//! Width-aware cutting and padding come from `tuikit::text`; a group
//! header's name and path are the row's own (`rows::GroupRow`).

use super::*;

pub(super) fn compact_mux_label(label: &str) -> String {
    let Some((backend, native)) = label.split_once(':') else {
        return compact_mux_native_id(label);
    };
    format!("{backend}:{}", compact_mux_native_id(native))
}

/// Head…tail truncation for long mux native ids. Long pane/session
/// ids would otherwise dominate the row; the head/tail shape keeps
/// both ends recognizable at a glance.
pub(super) fn compact_mux_native_id(native: &str) -> String {
    if text::width(native) <= 36 {
        return native.to_string();
    }
    format!(
        "{}{}{}",
        text::clip(native, 28),
        marks::ELLIPSIS,
        text::clip_start(native, 6)
    )
}
