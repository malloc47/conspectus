//! Pull-request previews (ADR 0112 §1): a PR's title, state, checks,
//! reviews, and body, read with `gh pr view`. Held in memory only
//! (ADR 0086); the preview cache is the only place a fetch lands.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::PreviewContent;
use crate::discovery::forge::{GhOutcome, GhRunner, SystemGh};

/// The fields `gh pr view --json` returns for a preview.
const PR_FIELDS: &str = "title,body,state,isDraft,reviewDecision,statusCheckRollup,\
                         latestReviews,url,headRefName,baseRefName,updatedAt";

/// Which pull request to read.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrQuery {
    pub provider: String,
    pub host: String,
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

pub fn load_pr_preview(query: &PrQuery) -> PreviewContent {
    // Tests and offline setups turn forge reads off wholesale.
    if std::env::var_os("CONSPECTUS_DISABLE_FORGE").is_some() {
        return PreviewContent::Unavailable(
            "forge reads are off (CONSPECTUS_DISABLE_FORGE)".to_string(),
        );
    }
    load_pr_preview_with(&SystemGh::new(), query)
}

pub fn load_pr_preview_with(runner: &dyn GhRunner, query: &PrQuery) -> PreviewContent {
    if query.provider != crate::discovery::providers::GITHUB {
        return PreviewContent::Unavailable(format!(
            "previews read GitHub pull requests; this one is on {}",
            query.provider
        ));
    }
    let repo = format!("{}/{}/{}", query.host, query.owner, query.repo);
    match runner.view_pull_request(&repo, query.number, PR_FIELDS) {
        Ok(GhOutcome::PullRequests(json)) => pr_preview_from_json(&json),
        Ok(GhOutcome::Unavailable(reason)) => {
            PreviewContent::Unavailable(reason.as_str().to_string())
        }
        Ok(GhOutcome::Failed { message, .. }) => PreviewContent::Failed(format!("gh: {message}")),
        Err(err) => PreviewContent::Failed(format!("gh: {err:#}")),
    }
}

/// The preview for a `gh pr view --json` object, ages rendered against
/// the TUI's clock.
pub fn pr_preview_from_json(json: &str) -> PreviewContent {
    match render_pr(json, Some(crate::tui::rows::tree_current_unix_epoch())) {
        Ok(lines) => PreviewContent::Lines(lines),
        Err(err) => PreviewContent::Failed(format!("gh returned unreadable JSON: {err}")),
    }
}

fn bold(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::new().add_modifier(Modifier::BOLD))
}

fn dim(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::new().add_modifier(Modifier::DIM))
}

fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// Compose a `gh pr view --json` object: title, a state line, the
/// branches and update time, checks, reviews, then the body. `now`
/// renders the update time as an age.
pub fn render_pr(json: &str, now: Option<i64>) -> Result<Vec<Line<'static>>, serde_json::Error> {
    let pr: Value = serde_json::from_str(json)?;
    let mut lines = vec![Line::from(bold(str_field(&pr, "title").to_string()))];

    let mut state = vec![str_field(&pr, "state").to_ascii_lowercase()];
    if pr.get("isDraft").and_then(Value::as_bool) == Some(true) {
        state.push("draft".to_string());
    }
    let decision = match str_field(&pr, "reviewDecision") {
        "APPROVED" => "approved",
        "CHANGES_REQUESTED" => "changes requested",
        "REVIEW_REQUIRED" => "review required",
        _ => "",
    };
    if !decision.is_empty() {
        state.push(decision.to_string());
    }
    lines.push(Line::from(dim(state.join(" · "))));

    let mut branches = Vec::new();
    match (str_field(&pr, "headRefName"), str_field(&pr, "baseRefName")) {
        ("", _) => {}
        (head, "") => branches.push(head.to_string()),
        (head, base) => branches.push(format!("{head} → {base}")),
    }
    let updated = chrono::DateTime::parse_from_rfc3339(str_field(&pr, "updatedAt"))
        .ok()
        .and_then(|at| crate::tui::rows::format_recency(now, Some(at.timestamp())));
    if let Some(age) = updated {
        branches.push(format!("updated {age} ago"));
    }
    if !branches.is_empty() {
        lines.push(Line::from(dim(branches.join(" · "))));
    }

    let checks = pr
        .get("statusCheckRollup")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    lines.push(Line::raw(""));
    if checks.is_empty() {
        lines.push(Line::from(vec![bold("checks"), dim("  none")]));
    } else {
        let outcomes: Vec<(String, CheckOutcome)> = checks.iter().map(check_outcome).collect();
        let count = |kind: CheckOutcome| outcomes.iter().filter(|(_, o)| *o == kind).count();
        lines.push(Line::from(vec![
            bold("checks"),
            Span::raw(format!(
                "  {} passed · {} failed · {} pending",
                count(CheckOutcome::Passed),
                count(CheckOutcome::Failed),
                count(CheckOutcome::Pending)
            )),
        ]));
        for (name, outcome) in &outcomes {
            match outcome {
                CheckOutcome::Failed => lines.push(Line::raw(format!("  × {name}"))),
                CheckOutcome::Pending => lines.push(Line::from(dim(format!("  … {name}")))),
                CheckOutcome::Passed | CheckOutcome::Other => {}
            }
        }
    }

    let reviews = pr
        .get("latestReviews")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if reviews.is_empty() {
        lines.push(Line::from(vec![bold("reviews"), dim("  none")]));
    } else {
        lines.push(Line::from(bold("reviews")));
        for review in &reviews {
            let author = review
                .get("author")
                .map(|author| str_field(author, "login"))
                .unwrap_or_default();
            let verdict = str_field(review, "state")
                .to_ascii_lowercase()
                .replace('_', " ");
            lines.push(Line::raw(format!("  {author}  {verdict}")));
        }
    }

    let body = str_field(&pr, "body").trim();
    lines.push(Line::raw(""));
    if body.is_empty() {
        lines.push(Line::from(dim("(no description)")));
    } else {
        lines.extend(
            body.lines()
                .map(|line| Line::raw(line.replace('\t', "    "))),
        );
    }
    Ok(lines)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckOutcome {
    Passed,
    Failed,
    Pending,
    Other,
}

/// A check's name and outcome. Check runs report `status` and
/// `conclusion`; commit statuses report `context` and `state`.
fn check_outcome(check: &Value) -> (String, CheckOutcome) {
    let name = check
        .get("name")
        .or_else(|| check.get("context"))
        .and_then(Value::as_str)
        .unwrap_or("check")
        .to_string();
    let conclusion = str_field(check, "conclusion");
    let status = str_field(check, "status");
    let state = str_field(check, "state");
    let outcome = match (status, conclusion, state) {
        (_, "SUCCESS", _) | (_, _, "SUCCESS") => CheckOutcome::Passed,
        (_, "FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE", _)
        | (_, _, "FAILURE" | "ERROR") => CheckOutcome::Failed,
        ("QUEUED" | "IN_PROGRESS" | "PENDING" | "WAITING" | "REQUESTED", _, _)
        | (_, _, "PENDING" | "EXPECTED") => CheckOutcome::Pending,
        _ => CheckOutcome::Other,
    };
    (name, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::forge::{FakeGh, GhUnavailableReason};

    fn query() -> PrQuery {
        PrQuery {
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            number: 7,
        }
    }

    const PR_JSON: &str = r#"{
        "title": "Preview pull requests",
        "body": "Adds a PR preview.\n\nCloses #6.",
        "state": "OPEN",
        "isDraft": true,
        "reviewDecision": "CHANGES_REQUESTED",
        "statusCheckRollup": [
            {"__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "SUCCESS"},
            {"__typename": "CheckRun", "name": "clippy", "status": "COMPLETED", "conclusion": "FAILURE"},
            {"__typename": "CheckRun", "name": "docs", "status": "IN_PROGRESS", "conclusion": ""},
            {"__typename": "StatusContext", "context": "ci/legacy", "state": "SUCCESS"}
        ],
        "latestReviews": [{"author": {"login": "alice"}, "state": "CHANGES_REQUESTED"}],
        "url": "https://github.com/o/r/pull/7",
        "headRefName": "feature/preview",
        "baseRefName": "main",
        "updatedAt": "2023-11-14T22:13:20Z"
    }"#;

    #[test]
    fn a_pr_shows_its_title_state_checks_reviews_and_body() {
        let lines: Vec<String> = render_pr(PR_JSON, Some(1_700_003_600))
            .expect("renders")
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            lines,
            [
                "Preview pull requests",
                "open · draft · changes requested",
                "feature/preview → main · updated 1h ago",
                "",
                "checks  2 passed · 1 failed · 1 pending",
                "  × clippy",
                "  … docs",
                "reviews",
                "  alice  changes requested",
                "",
                "Adds a PR preview.",
                "",
                "Closes #6.",
            ]
        );
    }

    #[test]
    fn missing_unauthenticated_or_offline_gh_is_unavailable() {
        for reason in [
            GhUnavailableReason::BinaryNotFound,
            GhUnavailableReason::NotAuthenticated,
            GhUnavailableReason::Offline,
        ] {
            assert_eq!(
                load_pr_preview_with(&FakeGh::unavailable(reason), &query()),
                PreviewContent::Unavailable(reason.as_str().to_string())
            );
        }
    }

    #[test]
    fn a_non_github_pr_is_unavailable() {
        let query = PrQuery {
            provider: "gitlab".to_string(),
            ..query()
        };
        assert!(matches!(
            load_pr_preview_with(&FakeGh::with_pull_requests(PR_JSON), &query),
            PreviewContent::Unavailable(_)
        ));
    }
}
