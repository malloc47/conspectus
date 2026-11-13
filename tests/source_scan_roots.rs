//! The plumbing the source-scanning tests share (`support/source_tree.rs`)
//! resolves paths from the workspace root and fails a scan that visited too
//! few files. These tests check that plumbing itself; each scan has its own
//! test that a violation planted in every tree it covers is reported.

use std::fs;
use std::path::Path;

#[path = "support/source_tree.rs"]
mod source_tree;

use source_tree::{EVERY_RUST_TREE, FRAMEWORK_SRC, HOST_SRC, Tree};

#[test]
fn scans_start_from_the_workspace_root() {
    let root = source_tree::workspace_root();
    let manifest: toml::Table = fs::read_to_string(root.join("Cargo.toml"))
        .expect("read the root manifest")
        .parse()
        .expect("parse the root manifest");
    let members: Vec<&str> = manifest["workspace"]["members"]
        .as_array()
        .expect("workspace members")
        .iter()
        .filter_map(toml::Value::as_str)
        .collect();
    assert!(
        members.contains(&"crates/tuikit"),
        "the scan root {} is not the workspace root: members are {members:?}",
        root.display()
    );
    assert!(
        root.join("Cargo.lock").is_file(),
        "the lockfile sits at the workspace root"
    );
}

#[test]
fn every_named_tree_meets_its_minimum_from_the_workspace_root() {
    for tree in EVERY_RUST_TREE {
        source_tree::try_rust_files(source_tree::workspace_root(), &[*tree])
            .unwrap_or_else(|message| panic!("{message}"));
    }
}

#[test]
fn the_framework_tree_is_found_from_the_workspace_root() {
    let files = source_tree::rust_files(source_tree::workspace_root(), &[FRAMEWORK_SRC]);
    assert!(
        files
            .iter()
            .any(|path| path.ends_with("crates/tuikit/src/lib.rs")),
        "{files:?}"
    );
}

#[test]
fn a_wrong_root_trips_the_tripwire_for_every_tree() {
    let scratch = tempfile::tempdir().expect("scratch directory");
    let error = source_tree::try_rust_files(scratch.path(), &[HOST_SRC, FRAMEWORK_SRC])
        .expect_err("an empty root has no sources");
    for tree in [HOST_SRC, FRAMEWORK_SRC] {
        let expected = format!(
            "`{}`: 0 Rust files, at least {} expected",
            tree.dir, tree.min_files
        );
        assert!(error.contains(&expected), "{error}");
    }
}

#[test]
fn a_tree_below_its_minimum_trips_and_one_at_it_does_not() {
    let scratch = tempfile::tempdir().expect("scratch directory");
    fs::create_dir_all(scratch.path().join("pkg/src")).expect("create tree");
    fs::write(scratch.path().join("pkg/src/a.rs"), "").expect("write file");
    fs::write(scratch.path().join("pkg/src/b.txt"), "").expect("write file");

    let at_minimum = [Tree::new("pkg/src", 1)];
    assert_eq!(
        source_tree::rust_files(scratch.path(), &at_minimum).len(),
        1,
        "only `.rs` files count"
    );
    let below = [Tree::new("pkg/src", 2)];
    let error = source_tree::try_rust_files(scratch.path(), &below).expect_err("one file is short");
    assert!(
        error.contains("`pkg/src`: 1 Rust files, at least 2 expected"),
        "{error}"
    );
}

#[test]
fn a_missing_single_file_tree_trips_the_tripwire() {
    let scratch = tempfile::tempdir().expect("scratch directory");
    let single = [Tree::new("src/gone.rs", 1)];
    let error =
        source_tree::try_rust_files(scratch.path(), &single).expect_err("the file is missing");
    assert!(error.contains("`src/gone.rs`: 0 Rust files"), "{error}");

    fs::create_dir_all(scratch.path().join("src")).expect("create tree");
    fs::write(scratch.path().join("src/gone.rs"), "").expect("write file");
    assert_eq!(source_tree::rust_files(scratch.path(), &single).len(), 1);
}

#[test]
fn a_tree_with_a_minimum_of_zero_may_be_absent() {
    let scratch = tempfile::tempdir().expect("scratch directory");
    let optional = [Tree::new("crates/tuikit/examples", 0)];
    assert!(source_tree::rust_files(scratch.path(), &optional).is_empty());
}

#[test]
fn unit_test_files_are_recognized_by_name() {
    assert!(source_tree::is_test_file(Path::new("src/app_tests.rs")));
    assert!(source_tree::is_test_file(Path::new("src/tui/tests.rs")));
    assert!(!source_tree::is_test_file(Path::new("src/tui/mod.rs")));
    assert!(!source_tree::is_test_file(Path::new("src/contests.rs")));
}
