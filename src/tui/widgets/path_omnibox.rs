//! Inline path entry with ranked completion candidates.
//!
//! The widget is deliberately domain-neutral: callers provide known
//! path candidates and priorities, while this module owns text input,
//! filesystem prefix matching, completion, and validation.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::tui::widgets::input::TextInputState;

const MAX_CANDIDATES: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCandidate {
    pub path: String,
    pub source: String,
    pub rank: i32,
}

impl PathCandidate {
    pub fn new(path: impl Into<String>, source: impl Into<String>, rank: i32) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
            rank,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathValidation {
    Empty,
    Exists,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathSuggestion {
    pub path: String,
    pub source: String,
    pub exists: bool,
}

#[derive(Debug, Clone)]
pub struct PathOmniboxState {
    title: String,
    input: TextInputState,
    known: Vec<PathCandidate>,
    selected: usize,
}

impl PathOmniboxState {
    pub fn new(title: impl Into<String>, initial: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            input: TextInputState::new(title.clone(), initial),
            title,
            known: Vec::new(),
            selected: 0,
        }
    }

    pub fn value(&self) -> &str {
        self.input.value()
    }

    pub fn cursor(&self) -> usize {
        self.input.cursor()
    }

    pub fn set_known_candidates(&mut self, candidates: Vec<PathCandidate>) {
        self.known = normalize_known_candidates(candidates);
        self.selected = self
            .selected
            .min(self.suggestions().len().saturating_sub(1));
    }

    pub fn validation(&self) -> PathValidation {
        let value = self.value().trim();
        if value.is_empty() {
            PathValidation::Empty
        } else if Path::new(&expand_home_prefix(value)).exists() {
            PathValidation::Exists
        } else {
            PathValidation::Missing
        }
    }

    pub fn expanded_value(&self) -> String {
        expand_home_prefix(self.value().trim())
    }

    pub fn suggestions(&self) -> Vec<PathSuggestion> {
        ranked_path_suggestions(self.value(), &self.known)
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> PathOmniboxOutcome {
        match event.code {
            KeyCode::Tab => {
                if let Some(path) = self.selected_suggestion_path() {
                    self.input = TextInputState::new(self.title.clone(), path);
                    self.selected = 0;
                    PathOmniboxOutcome::Completed
                } else {
                    PathOmniboxOutcome::NoCompletion
                }
            }
            _ => {
                let before = self.input.value().to_string();
                let _ = self.input.handle_key(event);
                if self.input.value() != before {
                    self.selected = 0;
                    PathOmniboxOutcome::Changed
                } else {
                    PathOmniboxOutcome::Continue
                }
            }
        }
    }

    fn selected_suggestion_path(&self) -> Option<String> {
        let value = self.value().trim();
        self.suggestions()
            .into_iter()
            .filter(|suggestion| suggestion.path.as_str() != value)
            .nth(self.selected)
            .map(|suggestion| suggestion.path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOmniboxOutcome {
    Continue,
    Changed,
    Completed,
    NoCompletion,
}

fn normalize_known_candidates(candidates: Vec<PathCandidate>) -> Vec<PathCandidate> {
    let mut seen = BTreeSet::new();
    let mut out: Vec<PathCandidate> = candidates
        .into_iter()
        .filter_map(|candidate| {
            let path = candidate.path.trim();
            if path.is_empty() || !seen.insert(path.to_string()) {
                None
            } else {
                Some(PathCandidate {
                    path: path.to_string(),
                    source: candidate.source,
                    rank: candidate.rank,
                })
            }
        })
        .collect();
    out.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.path.cmp(&right.path))
    });
    out
}

fn ranked_path_suggestions(value: &str, known: &[PathCandidate]) -> Vec<PathSuggestion> {
    let query = value.trim();
    let expanded_query = expand_home_prefix(query);
    let tilde_query = query == "~" || query.starts_with("~/");
    let mut seen = BTreeSet::new();
    let mut ranked = Vec::new();

    for candidate in known {
        if !path_matches(&expanded_query, &candidate.path) {
            continue;
        }
        let path = if tilde_query {
            collapse_home_prefix(&candidate.path)
        } else {
            candidate.path.clone()
        };
        if seen.insert(path.clone()) {
            ranked.push((
                candidate.rank + match_score(&expanded_query, &candidate.path),
                PathSuggestion {
                    path,
                    source: candidate.source.clone(),
                    exists: Path::new(&candidate.path).exists(),
                },
            ));
        }
    }

    for suggestion in filesystem_suggestions(query) {
        if seen.insert(suggestion.path.clone()) {
            let expanded_path = expand_home_prefix(&suggestion.path);
            ranked.push((
                10 + match_score(&expanded_query, &expanded_path),
                suggestion,
            ));
        }
    }

    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.path.cmp(&right.1.path))
    });
    ranked
        .into_iter()
        .map(|(_, suggestion)| suggestion)
        .take(MAX_CANDIDATES)
        .collect()
}

fn path_matches(query: &str, path: &str) -> bool {
    query.is_empty() || path.starts_with(query) || path.contains(query)
}

fn match_score(query: &str, path: &str) -> i32 {
    if query.is_empty() {
        0
    } else if path == query {
        100
    } else if path.starts_with(query) {
        60
    } else if path.contains(query) {
        20
    } else {
        0
    }
}

fn filesystem_suggestions(query: &str) -> Vec<PathSuggestion> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let expanded_query = expand_home_prefix(query);
    let query_path = Path::new(&expanded_query);
    let tilde_query = query == "~" || query.starts_with("~/");
    let (parent, prefix) = if expanded_query.ends_with(std::path::MAIN_SEPARATOR) {
        (query_path, "")
    } else {
        (
            query_path.parent().unwrap_or_else(|| Path::new(".")),
            query_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
        )
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut suggestions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(prefix) {
            continue;
        }
        suggestions.push(PathSuggestion {
            path: display_path(path, tilde_query),
            source: "filesystem".to_string(),
            exists: true,
        });
    }
    suggestions
}

fn display_path(path: PathBuf, collapse_home: bool) -> String {
    let raw = path.to_string_lossy().into_owned();
    if collapse_home {
        collapse_home_prefix(&raw)
    } else {
        raw
    }
}

fn expand_home_prefix(value: &str) -> String {
    expand_home_prefix_with(
        value,
        std::env::var_os("HOME").map(PathBuf::from).as_deref(),
    )
}

fn expand_home_prefix_with(value: &str, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return value.to_string();
    };
    if value == "~" {
        return home.to_string_lossy().into_owned();
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home.join(rest).to_string_lossy().into_owned();
    }
    value.to_string()
}

fn collapse_home_prefix(value: &str) -> String {
    collapse_home_prefix_with(
        value,
        std::env::var_os("HOME").map(PathBuf::from).as_deref(),
    )
}

fn collapse_home_prefix_with(value: &str, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return value.to_string();
    };
    let home = home.to_string_lossy();
    if value == home {
        return "~".to_string();
    }
    let prefix = format!("{home}/");
    if let Some(rest) = value.strip_prefix(&prefix) {
        return format!("~/{rest}");
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn ranks_known_candidates_before_filesystem_matches() {
        let mut state = PathOmniboxState::new(" cwd ", "/tmp");
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
        let mut state = PathOmniboxState::new(" cwd ", "/work");
        state.set_known_candidates(vec![
            PathCandidate::new("/workspace/beta", "agent", 20),
            PathCandidate::new("/workspace/alpha", "selected", 100),
        ]);

        assert_eq!(
            state.handle_key(key(KeyCode::Tab)),
            PathOmniboxOutcome::Completed
        );
        assert_eq!(state.value(), "/workspace/alpha");
    }

    #[test]
    fn reports_missing_paths_without_rejecting_typed_value() {
        let state = PathOmniboxState::new(" cwd ", "/definitely/not/here");
        assert_eq!(state.validation(), PathValidation::Missing);
        assert_eq!(state.value(), "/definitely/not/here");
    }

    #[test]
    fn filesystem_suggestions_match_typed_prefix() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let alpha = tmp.path().join("alpha");
        std::fs::create_dir(&alpha).expect("mkdir");
        let mut state =
            PathOmniboxState::new(" cwd ", tmp.path().join("a").to_string_lossy().to_string());
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
        let mut state = PathOmniboxState::new(" cwd ", "~/cons");
        state.set_known_candidates(vec![PathCandidate::new(
            target.to_string_lossy(),
            "selected",
            100,
        )]);

        assert_eq!(state.suggestions()[0].path, "~/conspectus-tilde-candidate");
        assert_eq!(
            state.handle_key(key(KeyCode::Tab)),
            PathOmniboxOutcome::Completed
        );
        assert_eq!(state.value(), "~/conspectus-tilde-candidate");
        assert_eq!(state.expanded_value(), target.to_string_lossy().to_string());
    }
}
