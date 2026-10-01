//! Width-aware truncation, padding, and compact labels shared by the panels.

use super::*;

pub(super) fn truncate_to_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(width);
    let mut budget = width;
    for ch in text.chars() {
        let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if ch_w > budget {
            out.push('…');
            return out;
        }
        out.push(ch);
        budget -= ch_w;
    }
    out
}

pub(super) fn truncate_to_width_strict(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_string();
    }
    let mut out = String::with_capacity(width);
    let mut used = 0;
    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > width - 1 {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out.push('…');
    out
}

/// Truncate `text` to `width` cells by replacing a middle slice with
/// `…` when the natural width overflows. Keeps the leading and
/// trailing context visible so a path like
/// `/fixture/atelier-demo/repo-a` collapses to
/// `/fixture/…/repo-a` rather than dropping the basename. The left
/// half is preferred when the budget is odd.
pub(super) fn truncate_to_width_middle(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let text_width = UnicodeWidthStr::width(text);
    if text_width <= width {
        return text.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }
    let usable = width - 1;
    let left_budget = usable.div_ceil(2);
    let right_budget = usable - left_budget;
    let chars: Vec<char> = text.chars().collect();

    let mut left = String::new();
    let mut left_used = 0usize;
    for ch in &chars {
        let cw = unicode_width::UnicodeWidthChar::width(*ch).unwrap_or(0);
        if left_used + cw > left_budget {
            break;
        }
        left.push(*ch);
        left_used += cw;
    }

    let mut right_chars: Vec<char> = Vec::new();
    let mut right_used = 0usize;
    for ch in chars.iter().rev() {
        let cw = unicode_width::UnicodeWidthChar::width(*ch).unwrap_or(0);
        if right_used + cw > right_budget {
            break;
        }
        right_chars.push(*ch);
        right_used += cw;
    }
    let right: String = right_chars.into_iter().rev().collect();

    let mut out = String::with_capacity(width);
    out.push_str(&left);
    out.push('…');
    out.push_str(&right);
    out
}

pub(super) fn truncate_to_width_no_marker(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    let mut out = String::with_capacity(width);
    let mut used = 0;
    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > width {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out
}

pub(super) fn pad_to_width(mut text: String, width: usize) -> String {
    let used = UnicodeWidthStr::width(text.as_str());
    if used < width {
        text.push_str(&" ".repeat(width - used));
    }
    text
}

pub(super) fn fit_spans_to_width(spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let mut used = 0;
    let mut out = Vec::with_capacity(spans.len());
    for span in spans {
        let span_width = UnicodeWidthStr::width(span.content.as_ref());
        if used + span_width <= width {
            used += span_width;
            out.push(span);
            continue;
        }
        let remaining = width.saturating_sub(used);
        if remaining > 0 {
            let style = span.style;
            out.push(span!(
                style;
                "{}",
                truncate_to_width_strict(span.content.as_ref(), remaining)
            ));
        }
        break;
    }
    out
}

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
        String::new()
    } else {
        path.to_string()
    }
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
    if native.chars().count() <= 36 {
        return native.to_string();
    }
    let head: String = native.chars().take(28).collect();
    let tail: String = native
        .chars()
        .rev()
        .take(6)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{head}…{tail}")
}
