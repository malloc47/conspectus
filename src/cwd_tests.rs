use super::*;

#[test]
fn explicit_roots_are_used_as_given_even_when_missing() {
    let missing = PathBuf::from("/definitely/not/a/conspectus/root");
    let roots = ScanRoots::explicit(vec![missing.clone()]);
    assert_eq!(roots.effective(), vec![missing]);
}

#[test]
fn launch_dir_root_drops_out_once_deleted_and_returns_when_recreated() {
    let parent = tempfile::tempdir().expect("temp dir");
    let dir = parent.path().join("launch");
    std::fs::create_dir(&dir).expect("create launch dir");
    let roots = ScanRoots::launch_dir(Some(dir.clone()));
    assert_eq!(roots.effective(), vec![dir.clone()]);

    std::fs::remove_dir(&dir).expect("remove launch dir");
    assert!(roots.effective().is_empty());

    std::fs::create_dir(&dir).expect("recreate launch dir");
    assert_eq!(roots.effective(), vec![dir]);
}

#[test]
fn resolve_prefers_flags_then_config_then_launch_dir() {
    let launch = tempfile::tempdir().expect("temp dir");
    let launch_dir = launch.path().to_path_buf();
    let flag = PathBuf::from("/flag");
    let configured = PathBuf::from("/configured");
    let resolve = |flags: Vec<PathBuf>, configured: &[PathBuf]| {
        ScanRoots::resolve(flags, configured, Some(launch_dir.clone())).effective()
    };
    assert_eq!(
        resolve(vec![flag.clone()], std::slice::from_ref(&configured)),
        vec![flag]
    );
    assert_eq!(
        resolve(Vec::new(), std::slice::from_ref(&configured)),
        vec![configured]
    );
    assert_eq!(resolve(Vec::new(), &[]), vec![launch_dir.clone()]);
    assert!(
        ScanRoots::resolve(Vec::new(), &[], None)
            .effective()
            .is_empty()
    );
}
