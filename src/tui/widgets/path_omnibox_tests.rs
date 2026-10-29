use super::*;
use ratatui::crossterm::event::KeyCode;
use tuikit::testkit::keys::press;

#[test]
fn ranks_known_candidates_before_filesystem_matches() {
    let mut state = PathOmniboxState::new("/tmp");
    state.set_known_candidates(vec![
        PathCandidate::new("/tmp/low", "agent", 20),
        PathCandidate::new("/tmp/high", "selected", 100),
    ]);

    let suggestions = state.suggestions();
    assert_eq!(suggestions[0].path, "/tmp/high");
    assert_eq!(suggestions[0].source, "selected");
    assert!(
        suggestions
            .iter()
            .any(|suggestion| suggestion.path == "/tmp/low")
    );
}

#[test]
fn tab_completes_highest_ranked_candidate() {
    let mut state = PathOmniboxState::new("/work");
    state.set_known_candidates(vec![
        PathCandidate::new("/workspace/beta", "agent", 20),
        PathCandidate::new("/workspace/alpha", "selected", 100),
    ]);

    assert_eq!(
        state.handle_key(press(KeyCode::Tab)),
        PathOmniboxOutcome::Completed
    );
    assert_eq!(state.value(), "/workspace/alpha");
}

#[test]
fn reports_missing_paths_without_rejecting_typed_value() {
    let state = PathOmniboxState::new("/definitely/not/here");
    assert_eq!(state.validation(), PathValidation::Missing);
    assert_eq!(state.value(), "/definitely/not/here");
}

#[test]
fn filesystem_suggestions_match_typed_prefix() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let alpha = tmp.path().join("alpha");
    std::fs::create_dir(&alpha).expect("mkdir");
    let mut state = PathOmniboxState::new(tmp.path().join("a").to_string_lossy().to_string());
    state.set_known_candidates(Vec::new());

    assert!(
        state
            .suggestions()
            .iter()
            .any(|suggestion| suggestion.path == alpha.to_string_lossy())
    );
}

#[test]
fn tilde_prefix_expands_for_validation() {
    let home = tempfile::TempDir::new().expect("home tempdir");
    assert_eq!(
        expand_home_prefix_with("~/src", Some(home.path())),
        home.path().join("src").to_string_lossy()
    );
    assert_eq!(
        collapse_home_prefix_with(
            &home.path().join("src").to_string_lossy(),
            Some(home.path())
        ),
        "~/src"
    );
}

#[test]
fn tilde_query_matches_absolute_known_candidate_and_completes_tilde_path() {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("HOME must be set for tilde completion test");
    let target = home.join("conspectus-tilde-candidate");
    let mut state = PathOmniboxState::new("~/cons");
    state.set_known_candidates(vec![PathCandidate::new(
        target.to_string_lossy(),
        "selected",
        100,
    )]);

    assert_eq!(state.suggestions()[0].path, "~/conspectus-tilde-candidate");
    assert_eq!(
        state.handle_key(press(KeyCode::Tab)),
        PathOmniboxOutcome::Completed
    );
    assert_eq!(state.value(), "~/conspectus-tilde-candidate");
    assert_eq!(state.expanded_value(), target.to_string_lossy().to_string());
}
