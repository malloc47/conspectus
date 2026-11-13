//! A workspace test run reaches this crate.
//!
//! If a full run (`cargo test --workspace`, `cargo nextest run --workspace`)
//! does not list this test, the run skipped the framework's suite.

#[test]
fn workspace_test_run_executes_tuikit_tests() {
    assert_eq!(env!("CARGO_PKG_NAME"), "tuikit");
}
