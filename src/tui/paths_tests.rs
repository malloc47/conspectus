use super::*;
use tuikit::path_field::{Completion, Validity};

#[test]
fn the_system_source_lists_directories_and_files_and_follows_links() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    std::fs::create_dir(tmp.path().join("alpha")).expect("mkdir");
    std::fs::write(tmp.path().join("notes.txt"), "x").expect("write");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(tmp.path().join("alpha"), tmp.path().join("linked"))
            .expect("link");
        std::os::unix::fs::symlink(tmp.path().join("gone"), tmp.path().join("broken"))
            .expect("link");
    }
    let mut entries = SystemPaths.entries(tmp.path());
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let mut expected = vec![
        Entry::new("alpha", PathKind::Dir),
        Entry::new("notes.txt", PathKind::File),
    ];
    if cfg!(unix) {
        expected.insert(1, Entry::new("linked", PathKind::Dir));
    }
    assert_eq!(entries, expected, "a broken link is left out");
    assert_eq!(SystemPaths.kind(tmp.path()), Some(PathKind::Dir));
    assert_eq!(
        SystemPaths.kind(&tmp.path().join("notes.txt")),
        Some(PathKind::File)
    );
    assert_eq!(SystemPaths.kind(&tmp.path().join("missing")), None);
    assert!(
        SystemPaths
            .entries(&tmp.path().join("notes.txt"))
            .is_empty()
    );
}

#[test]
fn the_system_home_is_home() {
    assert_eq!(
        SystemPaths.home(),
        std::env::var_os("HOME").map(PathBuf::from)
    );
}

#[test]
fn a_directory_field_completes_from_the_disk_and_takes_no_file() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    std::fs::create_dir(tmp.path().join("alpha")).expect("mkdir");
    std::fs::write(tmp.path().join("apple.txt"), "x").expect("write");
    let typed = tmp.path().join("a").to_string_lossy().into_owned();
    let mut field = directory_field(typed, Vec::new());
    let values: Vec<&str> = field.candidates().iter().map(Completion::value).collect();
    assert_eq!(
        values,
        [tmp.path().join("alpha").to_string_lossy().as_ref()],
        "the file is not offered for a directory"
    );
    field.set_value(tmp.path().join("apple.txt").to_string_lossy());
    assert_eq!(field.validity(), Validity::Advisory);
    field.set_value(tmp.path().join("alpha").to_string_lossy());
    assert_eq!(field.validity(), Validity::Exists);
}

#[test]
fn a_directory_field_expands_tilde_to_home() {
    let home = std::env::var("HOME").expect("HOME must be set for the `~` check");
    let field = directory_field("~/", Vec::new());
    assert_eq!(field.expanded_value(), format!("{home}/"));
    assert_eq!(field.validity(), Validity::Exists);
}

#[test]
fn every_field_shares_one_source() {
    assert!(Arc::ptr_eq(&system(), &system()));
}
