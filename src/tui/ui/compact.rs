//! Compact labels for the panels: a path shortened to its last segment and
//! a long mux id kept to its head and tail. Width-aware cutting and padding
//! come from `tuikit::text`.

use super::*;

pub(super) fn compact_path_label(path: &str) -> String {
    if path == "Ungrouped" {
        return path.to_string();
    }
    // Workspace group rows render via `format_workspace_display`,
    // which joins `<label>  <members>  (<provider>)` with double-
    // space separators. Treat the first segment as the bold label
    // so workspace headers don't bold the member list or provider
    // chip (which the eye reads as metadata, parallel to a repo's
    // CWD path).
    if let Some((label, _)) = path.split_once("  ") {
        return label.to_string();
    }
    let trimmed = path.trim_end_matches('/');
    if trimmed == "~" {
        return "~".to_string();
    }
    trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

pub(super) fn compact_path_secondary(path: &str) -> String {
    if path == "Ungrouped" {
        return String::new();
    }
    // Workspace-shape: everything after the first `  ` is the
    // non-bold member-list / provider segment. Render it as-is so
    // the eye still picks out the segment separators
    // `format_workspace_display` inserted.
    if let Some((_, rest)) = path.split_once("  ") {
        return rest.to_string();
    }
    let label = compact_path_label(path);
    if label == path {
        return String::new();
    }
    // A checkout's `(linked)`-style marker already ends its label, so the
    // path after it doesn't repeat it.
    if let Some(index) = path.rfind(" (")
        && path.ends_with(')')
        && label.ends_with(&path[index..])
    {
        return path[..index].to_string();
    }
    path.to_string()
}

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
