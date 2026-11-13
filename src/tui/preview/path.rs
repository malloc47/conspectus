//! Path previews (ADR 0112 §2): a directory's listing or a file's
//! contents. Read-only (ADR 0143): one `read_dir` or one capped read,
//! and no listing follows a symlink.

use std::fs;
use std::io::Read;
use std::path::Path;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use tuikit::glyphs::marks;

use super::PreviewContent;

/// Entries a listing shows before summarizing the rest.
const MAX_LISTED: usize = 500;
/// Entries a listing reads from a directory; past this it stops
/// reading and says so, so a huge directory can't stall the loader.
const MAX_READ: usize = 5_000;
/// Bytes of a file the preview shows.
const MAX_FILE_BYTES: u64 = 64 * 1024;

/// Read what `path` holds: a listing for a directory, the contents of
/// a file, or why neither (ADR 0111: a missing path is ordinary). Notes
/// and sizes take `muted`, the theme's muted modifier.
pub fn load_path_preview(path: &Path, muted: Modifier) -> PreviewContent {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return PreviewContent::Unavailable("no longer exists".to_string());
        }
        Err(err) => return PreviewContent::Failed(format!("cannot read: {err}")),
    };
    let lines = if metadata.is_dir() {
        list_directory(path, muted)
    } else {
        read_file(path, metadata.len(), muted)
    };
    match lines {
        Ok(lines) => PreviewContent::Lines(lines),
        Err(err) => PreviewContent::Failed(format!("cannot read: {err}")),
    }
}

struct Entry {
    name: String,
    kind: EntryKind,
}

enum EntryKind {
    Directory,
    File(u64),
    Symlink(String),
    Other,
}

fn list_directory(path: &Path, muted: Modifier) -> std::io::Result<Vec<Line<'static>>> {
    let mut entries = Vec::new();
    let mut unread = false;
    for entry in fs::read_dir(path)? {
        if entries.len() == MAX_READ {
            unread = true;
            break;
        }
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // `file_type` doesn't follow symlinks, so a link is listed as
        // a link and never entered.
        let kind = match entry.file_type() {
            Ok(kind) if kind.is_symlink() => EntryKind::Symlink(
                fs::read_link(entry.path())
                    .map(|target| target.display().to_string())
                    .unwrap_or_default(),
            ),
            Ok(kind) if kind.is_dir() => EntryKind::Directory,
            Ok(kind) if kind.is_file() => EntryKind::File(entry.metadata().map_or(0, |m| m.len())),
            _ => EntryKind::Other,
        };
        entries.push(Entry { name, kind });
    }
    if entries.is_empty() {
        return Ok(vec![Line::styled(
            "(empty directory)",
            Style::new().add_modifier(muted),
        )]);
    }
    entries.sort_by(|a, b| {
        let dir_first = |e: &Entry| !matches!(e.kind, EntryKind::Directory);
        dir_first(a)
            .cmp(&dir_first(b))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    let total = entries.len();
    let name_width = entries
        .iter()
        .take(MAX_LISTED)
        .map(|e| tuikit::text::width(&e.name) + 1)
        .max()
        .unwrap_or(0)
        .min(48);
    let dim = Style::new().add_modifier(muted);
    let mut lines: Vec<Line<'static>> = entries
        .into_iter()
        .take(MAX_LISTED)
        .map(|entry| match entry.kind {
            EntryKind::Directory => Line::styled(
                format!("{}/", entry.name),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            EntryKind::File(size) => Line::from(vec![
                Span::raw(tuikit::text::pad(&entry.name, name_width)),
                Span::styled(format!(" {:>6}", human_size(size)), dim),
            ]),
            EntryKind::Symlink(target) => Line::from(vec![
                Span::styled(entry.name, Style::new().add_modifier(Modifier::ITALIC)),
                Span::styled(format!(" -> {target}"), dim),
            ]),
            EntryKind::Other => Line::raw(entry.name),
        })
        .collect();
    if total > MAX_LISTED || unread {
        let more = if unread {
            format!(
                "{} more than {MAX_READ} entries; showing {MAX_LISTED}",
                marks::ELLIPSIS
            )
        } else {
            format!("{} {} more", marks::ELLIPSIS, total - MAX_LISTED)
        };
        lines.push(Line::styled(more, dim));
    }
    Ok(lines)
}

fn read_file(path: &Path, len: u64, muted: Modifier) -> std::io::Result<Vec<Line<'static>>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_FILE_BYTES)
        .read_to_end(&mut bytes)?;
    let dim = Style::new().add_modifier(muted);
    if bytes.contains(&0) {
        return Ok(vec![Line::styled(
            format!("binary file · {}", human_size(len)),
            dim,
        )]);
    }
    if bytes.is_empty() {
        return Ok(vec![Line::styled("(empty file)", dim)]);
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<Line<'static>> = text
        .lines()
        .map(|line| Line::raw(line.replace('\t', "    ")))
        .collect();
    if len > MAX_FILE_BYTES {
        lines.push(Line::styled(
            format!(
                "{} cut at {} of {}",
                marks::ELLIPSIS,
                human_size(MAX_FILE_BYTES),
                human_size(len)
            ),
            dim,
        ));
    }
    Ok(lines)
}

/// `812`, `4.1K`, `12M`: a size short enough for a listing column.
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["K", "M", "G", "T"];
    if bytes < 1024 {
        return bytes.to_string();
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1}{}", UNITS[unit])
    } else {
        format!("{value:.0}{}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(content: &PreviewContent) -> Vec<String> {
        match content {
            PreviewContent::Lines(lines) => lines.iter().map(ToString::to_string).collect(),
            other => panic!("expected lines, got {other:?}"),
        }
    }

    #[test]
    fn a_directory_lists_directories_first_then_files_with_sizes() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir(dir.path().join("src")).expect("dir");
        fs::write(dir.path().join("README.md"), "hello").expect("file");
        fs::write(dir.path().join("a.txt"), vec![b'x'; 2048]).expect("file");
        let listing = rows(&load_path_preview(
            dir.path(),
            crate::tui::Theme::default().muted,
        ));
        assert_eq!(listing[0], "src/");
        assert!(listing[1].starts_with("a.txt") && listing[1].ends_with("2.0K"));
        assert!(listing[2].starts_with("README.md") && listing[2].ends_with('5'));
    }

    #[cfg(unix)]
    #[test]
    fn a_listing_shows_a_symlink_without_following_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("secret"), "x").expect("file");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("elsewhere")).expect("link");
        let listing = rows(&load_path_preview(
            dir.path(),
            crate::tui::Theme::default().muted,
        ));
        assert_eq!(
            listing,
            [format!("elsewhere -> {}", outside.path().display())]
        );
    }

    #[test]
    fn a_missing_path_no_longer_exists() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            load_path_preview(&dir.path().join("gone"), crate::tui::Theme::default().muted),
            PreviewContent::Unavailable("no longer exists".to_string())
        );
    }

    #[test]
    fn a_file_shows_its_contents() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("pins.toml");
        fs::write(&file, "[[pin]]\nid = \"work\"\n").expect("file");
        assert_eq!(
            rows(&load_path_preview(
                &file,
                crate::tui::Theme::default().muted
            )),
            ["[[pin]]", "id = \"work\""]
        );
    }

    #[test]
    fn a_large_file_is_cut_with_a_note() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("big.log");
        fs::write(&file, "line\n".repeat(30_000)).expect("file");
        let lines = rows(&load_path_preview(
            &file,
            crate::tui::Theme::default().muted,
        ));
        assert_eq!(
            lines.last().map(String::as_str),
            Some("… cut at 64K of 146K")
        );
        assert!(lines.len() < 30_000);
    }

    #[test]
    fn a_binary_file_shows_its_size_not_its_bytes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("blob");
        fs::write(&file, [0u8, 1, 2, 3]).expect("file");
        assert_eq!(
            rows(&load_path_preview(
                &file,
                crate::tui::Theme::default().muted
            )),
            ["binary file · 4"]
        );
    }
}
