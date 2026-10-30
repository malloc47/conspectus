//! Process-tree previews (ADR 0112 §1–2): a process and its
//! descendants, each with its command and working directory, read from
//! `/proc` only.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use tuikit::glyphs::marks;

use super::PreviewContent;

/// Processes a tree shows before summarizing the rest.
const MAX_PROCESSES: usize = 200;
/// Levels below the root a tree descends.
const MAX_DEPTH: usize = 16;
/// Cells of a command line a tree shows, the ellipsis included, so one
/// long command doesn't push its siblings out of view.
const MAX_COMMAND_CELLS: usize = 201;

/// The process tree rooted at `pid`, read from `proc_root` (`/proc` in
/// the TUI). Working directories and notes take `muted`, the theme's
/// muted modifier.
pub fn load_process_tree(proc_root: &Path, pid: i64, muted: Modifier) -> PreviewContent {
    if !proc_root.is_dir() {
        return PreviewContent::Unavailable("process trees need /proc".to_string());
    }
    if !proc_root.join(pid.to_string()).is_dir() {
        return PreviewContent::Unavailable("no longer running".to_string());
    }
    let children = children_by_parent(proc_root);
    let mut lines = Vec::new();
    let mut shown = 0;
    let mut stack = vec![(pid, 0usize)];
    while let Some((pid, depth)) = stack.pop() {
        if shown == MAX_PROCESSES {
            lines.push(dim(
                format!("{} more processes past {MAX_PROCESSES}", marks::ELLIPSIS),
                muted,
            ));
            break;
        }
        shown += 1;
        lines.extend(process_lines(proc_root, pid, depth, muted));
        if depth < MAX_DEPTH
            && let Some(kids) = children.get(&pid)
        {
            // Pushed in reverse so the lowest pid prints first.
            stack.extend(kids.iter().rev().map(|kid| (*kid, depth + 1)));
        }
    }
    PreviewContent::Lines(lines)
}

/// Every live process's children, from each `/proc/<pid>/stat`'s parent
/// pid.
fn children_by_parent(proc_root: &Path) -> BTreeMap<i64, Vec<i64>> {
    let mut children: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    let Ok(entries) = fs::read_dir(proc_root) else {
        return children;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<i64>().ok())
        else {
            continue;
        };
        if let Some(parent) = read_stat(proc_root, pid).map(|stat| stat.parent) {
            children.entry(parent).or_default().push(pid);
        }
    }
    for kids in children.values_mut() {
        kids.sort_unstable();
    }
    children
}

struct Stat {
    comm: String,
    parent: i64,
}

/// `comm` and the parent pid from `/proc/<pid>/stat`. `comm` sits in
/// parentheses and may hold spaces or parentheses itself, so the fields
/// after it are found from the last `)`.
fn read_stat(proc_root: &Path, pid: i64) -> Option<Stat> {
    let stat = fs::read_to_string(proc_root.join(pid.to_string()).join("stat")).ok()?;
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let comm = stat.get(open + 1..close)?.to_string();
    let mut rest = stat.get(close + 1..)?.split_whitespace();
    let _state = rest.next()?;
    let parent = rest.next()?.parse().ok()?;
    Some(Stat { comm, parent })
}

fn process_lines(proc_root: &Path, pid: i64, depth: usize, muted: Modifier) -> [Line<'static>; 2] {
    let dir = proc_root.join(pid.to_string());
    let command = fs::read(dir.join("cmdline"))
        .ok()
        .map(|raw| {
            raw.split(|byte| *byte == 0)
                .filter(|arg| !arg.is_empty())
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|command| !command.is_empty())
        // Kernel threads and zombies have no command line.
        .or_else(|| read_stat(proc_root, pid).map(|stat| format!("[{}]", stat.comm)))
        .unwrap_or_else(|| "(exited)".to_string());
    let command = tuikit::text::truncate_end(&command, MAX_COMMAND_CELLS);
    let cwd = fs::read_link(dir.join("cwd")).map_or_else(
        |_| "(cwd unreadable)".to_string(),
        |path| path.display().to_string(),
    );
    let indent = "  ".repeat(depth);
    [
        Line::from(vec![
            Span::raw(indent.clone()),
            Span::styled(pid.to_string(), Style::new().add_modifier(Modifier::BOLD)),
            Span::raw(format!("  {command}")),
        ]),
        dim(format!("{indent}  in {cwd}"), muted),
    ]
}

fn dim(text: String, muted: Modifier) -> Line<'static> {
    Line::styled(text, Style::new().add_modifier(muted))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake `/proc` entry for `pid` under `parent`.
    fn process(root: &Path, pid: i64, parent: i64, argv: &[&str], cwd: &str) {
        let dir = root.join(pid.to_string());
        fs::create_dir_all(&dir).expect("proc dir");
        fs::write(
            dir.join("stat"),
            format!("{pid} (a (b) c) S {parent} 1 1 0 -1"),
        )
        .expect("stat");
        let mut cmdline = argv.join("\0");
        cmdline.push('\0');
        fs::write(dir.join("cmdline"), cmdline).expect("cmdline");
        #[cfg(unix)]
        std::os::unix::fs::symlink(cwd, dir.join("cwd")).expect("cwd");
    }

    fn rows(content: PreviewContent) -> Vec<String> {
        match content {
            PreviewContent::Lines(lines) => lines.iter().map(ToString::to_string).collect(),
            other => panic!("expected lines, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_process_shows_its_descendants_with_commands_and_cwds() {
        let root = tempfile::tempdir().expect("proc root");
        process(root.path(), 10, 1, &["tmux"], "/");
        process(root.path(), 20, 10, &["claude", "--resume", "abc"], "/work");
        process(root.path(), 30, 20, &["node", "server.js"], "/work/web");
        process(root.path(), 21, 10, &["zsh"], "/home/op");
        process(root.path(), 99, 1, &["unrelated"], "/");
        assert_eq!(
            rows(load_process_tree(
                root.path(),
                10,
                crate::tui::Theme::default().muted
            )),
            [
                "10  tmux",
                "  in /",
                "  20  claude --resume abc",
                "    in /work",
                "    30  node server.js",
                "      in /work/web",
                "  21  zsh",
                "    in /home/op",
            ]
        );
    }

    #[test]
    fn an_exited_process_is_no_longer_running() {
        let root = tempfile::tempdir().expect("proc root");
        assert_eq!(
            load_process_tree(root.path(), 42, crate::tui::Theme::default().muted),
            PreviewContent::Unavailable("no longer running".to_string())
        );
    }

    #[test]
    fn no_proc_means_no_process_trees() {
        let root = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            load_process_tree(
                &root.path().join("proc"),
                42,
                crate::tui::Theme::default().muted
            ),
            PreviewContent::Unavailable("process trees need /proc".to_string())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_real_proc_shows_this_test_process() {
        let pid = i64::from(std::process::id());
        let lines = rows(load_process_tree(
            Path::new("/proc"),
            pid,
            crate::tui::Theme::default().muted,
        ));
        assert!(lines[0].starts_with(&pid.to_string()), "{lines:?}");
    }
}
