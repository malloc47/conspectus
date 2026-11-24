use std::process::Stdio;

use super::*;

fn path(text: &str) -> PathBuf {
    PathBuf::from(text)
}

/// `exists` over a fixed set of files.
fn files<'a>(present: &'a [&'a str]) -> impl Fn(&Path) -> bool + 'a {
    move |candidate| present.iter().any(|file| Path::new(file) == candidate)
}

#[test]
fn a_binary_that_is_still_there_runs_as_it_is() {
    let found = locate(Ok(path("/opt/conspectus")), 42, files(&["/opt/conspectus"]));
    assert_eq!(
        found,
        RunningBinary {
            path: path("/opt/conspectus"),
            fallback: None
        }
    );
}

#[test]
fn a_replaced_binary_runs_the_file_now_at_its_path() {
    let found = locate(
        Ok(path("/opt/conspectus (deleted)")),
        42,
        files(&["/opt/conspectus"]),
    );
    assert_eq!(
        found,
        RunningBinary {
            path: path("/opt/conspectus"),
            fallback: Some(Fallback::Replaced)
        }
    );
}

#[test]
fn a_removed_binary_runs_the_image_that_is_running() {
    let found = locate(Ok(path("/opt/conspectus (deleted)")), 42, files(&[]));
    assert_eq!(
        found,
        RunningBinary {
            path: path("/proc/42/exe"),
            fallback: Some(Fallback::Removed)
        }
    );
}

#[test]
fn a_file_that_really_ends_in_the_marker_is_not_taken_for_a_deleted_one() {
    let found = locate(
        Ok(path("/opt/odd (deleted)")),
        42,
        files(&["/opt/odd (deleted)", "/opt/odd"]),
    );
    assert_eq!(found.path, path("/opt/odd (deleted)"));
    assert_eq!(found.fallback, None);
}

#[test]
fn a_path_without_the_marker_is_left_alone_even_when_missing() {
    // Nothing says it was replaced, so there is no better program to
    // offer and the spawn reports what it finds.
    let found = locate(Ok(path("/opt/conspectus")), 42, files(&[]));
    assert_eq!(found.path, path("/opt/conspectus"));
    assert_eq!(found.fallback, None);
}

#[test]
fn an_unknown_binary_runs_whatever_conspectus_is_on_the_path() {
    let found = locate(
        Err(io::Error::new(io::ErrorKind::NotFound, "no /proc")),
        42,
        files(&[]),
    );
    assert_eq!(
        found,
        RunningBinary {
            path: path("conspectus"),
            fallback: None
        }
    );
}

/// A copy of `sh` at `dir/name`, running `read`, so it stays alive until
/// its stdin closes.
fn live_copy_of_sh(dir: &Path, name: &str) -> (PathBuf, std::process::Child) {
    let copy = dir.join(name);
    std::fs::copy("/bin/sh", &copy).expect("copy sh");
    let child = command(Program::Conspectus, &copy)
        .args(["-c", "read line"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("start the copy");
    (copy, child)
}

/// What `sh -c 'echo alive'` prints, run as `program`.
fn runs_as_sh(program: &Path) -> bool {
    command(Program::Conspectus, program)
        .args(["-c", "echo alive"])
        .output()
        .is_ok_and(|out| out.status.success() && out.stdout == b"alive\n")
}

#[test]
fn the_running_image_of_a_deleted_binary_still_runs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (copy, mut child) = live_copy_of_sh(dir.path(), "conspectus");
    std::fs::remove_file(&copy).expect("remove the file a process runs from");

    let marked = PathBuf::from(format!("{} (deleted)", copy.display()));
    let found = locate(Ok(marked), child.id(), Path::is_file);
    assert_eq!(found.fallback, Some(Fallback::Removed));
    assert!(!copy.exists(), "nothing is at the old path");
    assert!(
        runs_as_sh(&found.path),
        "{} should run the image",
        found.path.display()
    );

    drop(child.stdin.take());
    child.wait().expect("the copy exits");
}

#[test]
fn a_replaced_binary_runs_the_new_file_at_its_place() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (copy, mut child) = live_copy_of_sh(dir.path(), "conspectus");
    // What a rebuild does: a new file where the old one was.
    std::fs::remove_file(&copy).expect("remove the old file");
    std::fs::copy("/bin/sh", &copy).expect("write the new file");

    let marked = PathBuf::from(format!("{} (deleted)", copy.display()));
    let found = locate(Ok(marked), child.id(), Path::is_file);
    assert_eq!(found.fallback, Some(Fallback::Replaced));
    assert_eq!(found.path, copy);
    assert!(runs_as_sh(&found.path));

    drop(child.stdin.take());
    child.wait().expect("the copy exits");
}
