//! The process working directory can vanish while Conspectus runs, so
//! only `src/cwd.rs` reads it; everything else goes through that
//! module's helpers, which degrade instead of failing (ADR 0111).
//! Setting a child's directory with `Command::current_dir` is fine.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// A read of the process working directory, as opposed to a
/// `.current_dir(..)` builder call on a `Command` or the module's own
/// `from_current_dir` constructor.
fn reads_process_cwd(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or(line);
    code.match_indices("current_dir(").any(|(index, _)| {
        code[..index]
            .chars()
            .next_back()
            .is_none_or(|prev| prev != '.' && prev != '_' && !prev.is_alphanumeric())
    })
}

#[test]
fn only_the_cwd_module_reads_the_working_directory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    files.sort();

    let allowed = [root.join("src/cwd.rs"), root.join("src/cwd_tests.rs")];
    let mut problems = Vec::new();
    for path in files.iter().filter(|path| !allowed.contains(path)) {
        let text = fs::read_to_string(path).expect("read source file");
        for (number, line) in text.lines().enumerate() {
            if reads_process_cwd(line) {
                let relative = path.strip_prefix(root).unwrap_or(path);
                problems.push(format!(
                    "{}:{}: {}",
                    relative.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "read the working directory through `crate::cwd` (`current()` or \
         `for_default_target(flag)`) so a deleted directory can't fail the \
         command:\n{}",
        problems.join("\n")
    );
}

#[test]
fn matcher_tells_reads_from_command_builders() {
    assert!(reads_process_cwd("let cwd = std::env::current_dir()?;"));
    assert!(reads_process_cwd("    env::current_dir().ok()"));
    assert!(!reads_process_cwd("Command::new(git).current_dir(root)"));
    assert!(!reads_process_cwd("        .current_dir(cwd)"));
    assert!(!reads_process_cwd("// std::env::current_dir() is banned"));
    assert!(!reads_process_cwd(
        "pub fn from_current_dir() -> Result<Self>"
    ));
    assert!(reads_process_cwd("let here = current_dir()?;"));
}
