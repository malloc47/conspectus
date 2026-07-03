//! GitHub pull-request discovery via the `gh` CLI.
//!
//! [`GhPullRequestParser`] turns the JSON body produced by
//! `gh pr list --json <fields>` (see
//! [`GH_PR_LIST_FIELDS`]) into
//! provider-neutral [`PullRequestRecord`]s. The parser is deliberately
//! tolerant: rows missing the small set of required fields are skipped
//! rather than failing the whole run, which keeps forge discovery
//! best-effort when `gh` changes its output for some rows.
//!
//! [`fragment_for_repo`] turns those records into a
//! [`GraphFragment`] of `ForgePr` nodes plus `BranchHasForgePr`
//! candidate links. When the head ref does not name a discovered
//! branch the link carries an `UnresolvedEndpoint` so the evidence
//! survives until later discovery can resolve it.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use anyhow::Result;
use serde_json::Value;

use crate::discovery::git::GitProbe;
use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};
use crate::model::{
    BranchId, Confidence, ForgePrId, ForgePrNode, Freshness, GraphLink, GraphNode, LinkEndpoint,
    LinkState, Metadata, NodeId, Provenance, RelationKind, RepoId, SourceMetadata,
    UnresolvedEndpoint,
};

use super::{GH_PR_LIST_FIELDS, GITHUB_DEFAULT_HOST, GITHUB_PROVIDER, GhOutcome, GhRunner};

const FORGE_ADAPTER: &str = crate::discovery::providers::GITHUB;

/// Provider-neutral pull-request record emitted by forge adapters.
///
/// All fields except the required identity columns
/// (`number` and `head_ref`) are optional so adapters can preserve
/// whatever the upstream tool returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestRecord {
    pub number: u64,
    pub state: PullRequestState,
    pub url: Option<String>,
    pub head_ref: String,
    pub head_owner: Option<String>,
    pub head_repo: Option<String>,
    pub base_ref: Option<String>,
    pub is_draft: bool,
    pub updated_at: Option<String>,
    pub updated_epoch: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
    Other(String),
}

impl PullRequestState {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Merged => "merged",
            Self::Other(value) => value.as_str(),
        }
    }

    fn parse(raw: &str) -> Self {
        match raw.to_ascii_uppercase().as_str() {
            "OPEN" => Self::Open,
            "CLOSED" => Self::Closed,
            "MERGED" => Self::Merged,
            _ => Self::Other(raw.to_string()),
        }
    }
}

/// Identity of the local repo that a batch of PR records belongs to.
///
/// `repo_id` is the discovered [`RepoId`] for the local clone (used to
/// build `BranchId`s) and `host`/`owner`/`repo` are the GitHub-side
/// identifiers used to form the `ForgePrId`. Adapters that probe `gh`
/// per repo populate this from the matched remote.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepoContext {
    pub repo_id: RepoId,
    pub host: String,
    pub owner: String,
    pub repo: String,
}

impl RepoContext {
    pub fn new(
        repo_id: RepoId,
        host: impl Into<String>,
        owner: impl Into<String>,
        repo: impl Into<String>,
    ) -> Self {
        Self {
            repo_id,
            host: host.into(),
            owner: owner.into(),
            repo: repo.into(),
        }
    }

    /// Convenience for the common case of `host = github.com`.
    pub fn github(repo_id: RepoId, owner: impl Into<String>, repo: impl Into<String>) -> Self {
        Self::new(repo_id, GITHUB_DEFAULT_HOST, owner, repo)
    }
}

/// Build a [`GraphFragment`] that holds one `ForgePr` node per
/// `record` plus a `BranchHasForgePr` candidate link per record. The
/// link target is the discovered `Branch` node when its short ref is in
/// `discovered_branch_short_refs`; otherwise it carries an unresolved
/// endpoint so the evidence stays visible to later passes.
///
/// `discovered_branch_short_refs` should hold the short ref names
/// (e.g. `"feature/login"`) of branches already in the graph. The
/// short form is what `gh pr list` returns in `headRefName`.
pub fn fragment_for_repo(
    context: &RepoContext,
    discovered_branch_short_refs: &BTreeSet<String>,
    records: &[PullRequestRecord],
) -> GraphFragment {
    let mut nodes = Vec::with_capacity(records.len());
    let mut candidate_links = Vec::with_capacity(records.len());

    for record in records {
        let pr_id = ForgePrId::new(
            GITHUB_PROVIDER,
            context.host.clone(),
            context.owner.clone(),
            context.repo.clone(),
            record.number,
        );
        nodes.push(GraphNode::ForgePr(forge_pr_node(&pr_id, context, record)));
        candidate_links.push(branch_pr_link(
            &pr_id,
            context,
            record,
            discovered_branch_short_refs,
        ));
    }

    GraphFragment {
        nodes,
        candidate_links,
        diagnostics: Vec::new(),
        node_provenance: BTreeMap::new(),
    }
}

fn forge_pr_node(id: &ForgePrId, context: &RepoContext, record: &PullRequestRecord) -> ForgePrNode {
    ForgePrNode {
        id: id.clone(),
        provider: GITHUB_PROVIDER.to_string(),
        host: context.host.clone(),
        owner: context.owner.clone(),
        repo: context.repo.clone(),
        number: record.number,
        state: Some(record.state.as_str().to_string()),
        url: record.url.clone(),
        updated_epoch: record.updated_epoch,
        is_draft: record.is_draft,
    }
}

fn branch_pr_link(
    pr_id: &ForgePrId,
    context: &RepoContext,
    record: &PullRequestRecord,
    discovered_branch_short_refs: &BTreeSet<String>,
) -> GraphLink {
    let target = if discovered_branch_short_refs.contains(&record.head_ref) {
        let branch_id = BranchId::new(
            context.repo_id.clone(),
            format!("refs/heads/{}", record.head_ref),
        );
        LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        }
    } else {
        LinkEndpoint::Unresolved {
            evidence: unresolved_branch_evidence(context, record),
        }
    };

    let id = format!(
        "github:{}:branch_has_forge_pr:{}#{}",
        context.repo_id, context.repo, record.number
    );

    GraphLink {
        id,
        source: NodeId::ForgePr(pr_id.clone()),
        target,
        relation: RelationKind::BranchHasForgePr,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: FORGE_ADAPTER.to_string(),
            evidence: Some("gh pr list head ref".to_string()),
            fields: branch_link_fields(record),
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn unresolved_branch_evidence(
    context: &RepoContext,
    record: &PullRequestRecord,
) -> UnresolvedEndpoint {
    let mut metadata = Metadata::new();
    metadata.insert("host".to_string(), Value::String(context.host.clone()));
    metadata.insert("owner".to_string(), Value::String(context.owner.clone()));
    metadata.insert("repo".to_string(), Value::String(context.repo.clone()));
    if let Some(head_owner) = &record.head_owner {
        metadata.insert("head_owner".to_string(), Value::String(head_owner.clone()));
    }
    if let Some(head_repo) = &record.head_repo {
        metadata.insert("head_repo".to_string(), Value::String(head_repo.clone()));
    }

    UnresolvedEndpoint {
        node_type: "branch".to_string(),
        harness_key: None,
        native_id: Some(record.head_ref.clone()),
        state_scope: None,
        path: None,
        metadata,
    }
}

fn branch_link_fields(record: &PullRequestRecord) -> Metadata {
    let mut fields = Metadata::new();
    fields.insert(
        "head_ref".to_string(),
        Value::String(record.head_ref.clone()),
    );
    fields.insert(
        "state".to_string(),
        Value::String(record.state.as_str().to_string()),
    );
    fields.insert("is_draft".to_string(), Value::Bool(record.is_draft));
    if let Some(updated) = record.updated_epoch {
        fields.insert("updated_epoch".to_string(), Value::Number(updated.into()));
    }
    fields
}

/// Parser for `gh pr list --json ...` output. Stateless — kept as a
/// type so the [`crate::discovery::forge::ForgeAdapter`] can mock or
/// replace it later without changing call sites.
#[derive(Clone, Copy, Debug, Default)]
pub struct GhPullRequestParser;

impl GhPullRequestParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a `gh pr list --json` JSON body. Rows that are not
    /// objects, are missing `number` / `headRefName`, or carry an
    /// unparseable `number` are skipped.
    pub fn parse(&self, body: &str) -> Vec<PullRequestRecord> {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let value: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => return Vec::new(),
        };

        let Some(rows) = value.as_array() else {
            return Vec::new();
        };

        rows.iter().filter_map(parse_row).collect()
    }
}

fn parse_row(value: &Value) -> Option<PullRequestRecord> {
    let row = value.as_object()?;
    let number = row.get("number").and_then(|v| v.as_u64())?;
    let head_ref = string_field(row.get("headRefName"))?;

    let state = row
        .get("state")
        .and_then(|v| v.as_str())
        .map(PullRequestState::parse)
        .unwrap_or_else(|| PullRequestState::Other(String::new()));

    let url = string_field(row.get("url"));
    let base_ref = string_field(row.get("baseRefName"));
    let updated_at = string_field(row.get("updatedAt"));
    let updated_epoch = updated_at.as_deref().and_then(parse_rfc3339_epoch);
    let is_draft = row.get("isDraft").and_then(Value::as_bool).unwrap_or(false);

    let head_owner = row
        .get("headRepositoryOwner")
        .and_then(Value::as_object)
        .and_then(|owner| string_field(owner.get("login")));
    let head_repo = row
        .get("headRepository")
        .and_then(Value::as_object)
        .and_then(|repo| string_field(repo.get("name")));

    Some(PullRequestRecord {
        number,
        state,
        url,
        head_ref,
        head_owner,
        head_repo,
        base_ref,
        is_draft,
        updated_at,
        updated_epoch,
    })
}

fn string_field(value: Option<&Value>) -> Option<String> {
    let raw = value?.as_str()?.trim();
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

/// Minimal RFC 3339 / ISO 8601 parser that converts the
/// `updatedAt` strings `gh` emits (`YYYY-MM-DDTHH:MM:SSZ` plus an
/// optional fractional component and numeric offset) into a Unix
/// epoch. Returns `None` for anything it does not understand so the
/// caller can fall back to the raw string.
fn parse_rfc3339_epoch(raw: &str) -> Option<i64> {
    let (date_part, time_part) = raw.split_once('T')?;
    let mut date_iter = date_part.split('-');
    let year: i64 = date_iter.next()?.parse().ok()?;
    let month: i64 = date_iter.next()?.parse().ok()?;
    let day: i64 = date_iter.next()?.parse().ok()?;
    if date_iter.next().is_some() {
        return None;
    }

    let (clock_part, offset_seconds) = split_offset(time_part)?;
    let mut clock_iter = clock_part.split(':');
    let hour: i64 = clock_iter.next()?.parse().ok()?;
    let minute: i64 = clock_iter.next()?.parse().ok()?;
    let raw_seconds = clock_iter.next()?;
    if clock_iter.next().is_some() {
        return None;
    }
    let seconds_whole = raw_seconds.split('.').next()?.parse::<i64>().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) || !(0..=60).contains(&seconds_whole)
    {
        return None;
    }

    let days = days_from_civil(year, month as u32, day as u32);
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + seconds_whole - offset_seconds;
    Some(seconds)
}

/// Returns `(clock_string, offset_in_seconds)` for a time component
/// like `"12:34:56Z"`, `"12:34:56+02:00"`, or `"12:34:56.789-05:00"`.
fn split_offset(time_part: &str) -> Option<(&str, i64)> {
    if let Some(stripped) = time_part.strip_suffix('Z') {
        return Some((stripped, 0));
    }
    let bytes = time_part.as_bytes();
    let mut idx = None;
    for (i, byte) in bytes.iter().enumerate().rev() {
        match byte {
            b'+' | b'-' => {
                idx = Some(i);
                break;
            }
            b':' | b'0'..=b'9' | b'.' => continue,
            _ => return None,
        }
    }
    let split_at = idx?;
    let clock = &time_part[..split_at];
    let offset = &time_part[split_at..];
    let sign = if offset.as_bytes().first() == Some(&b'-') {
        -1
    } else {
        1
    };
    let mut offset_iter = offset[1..].split(':');
    let hours: i64 = offset_iter.next()?.parse().ok()?;
    let minutes: i64 = offset_iter.next().map(str::parse).transpose().ok()??;
    Some((clock, sign * (hours * 3600 + minutes * 60)))
}

/// `days_from_civil` (Howard Hinnant). Returns the number of days
/// since the Unix epoch (1970-01-01) for `(year, month, day)` in the
/// proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let m = i64::from(month);
    let d = i64::from(day);
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Extracted `(host, owner, repo)` triple from a git remote URL, if
/// the URL points at a GitHub-shaped host. The parser is tolerant: it
/// accepts `https://github.com/owner/repo[.git]`, `git@github.com:owner/repo[.git]`,
/// and `ssh://git@github.com/owner/repo[.git]`, plus GitHub Enterprise
/// hosts whose names start with `github.` or end in `.github.com`.
pub fn parse_github_remote(url: &str) -> Option<(String, String, String)> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (host, path) = if let Some(rest) = trimmed.strip_prefix("https://") {
        split_host_and_path(rest)?
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        split_host_and_path(rest)?
    } else if let Some(rest) = trimmed.strip_prefix("ssh://") {
        let after_user = rest.split_once('@').map(|(_, after)| after).unwrap_or(rest);
        split_host_and_path(after_user)?
    } else if let Some(rest) = trimmed.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        (host.to_string(), path.to_string())
    } else {
        return None;
    };

    if !looks_like_github_host(&host) {
        return None;
    }

    let path = path.trim_start_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.splitn(2, '/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }
    Some((host, owner.to_string(), repo.to_string()))
}

fn split_host_and_path(rest: &str) -> Option<(String, String)> {
    let (host, path) = rest.split_once('/')?;
    Some((host.to_string(), path.to_string()))
}

fn looks_like_github_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    lower == "github.com" || lower.ends_with(".github.com") || lower.starts_with("github.")
}

/// Discovery provider that probes each scan root for a git repo with
/// a GitHub remote, then asks `GhRunner` for that repo's pull
/// requests. Missing `gh`, unauthenticated runs, and per-repo
/// failures degrade silently to no rows for that repo so the rest of
/// the graph still loads.
pub struct GitHubForgeProvider<R: GhRunner> {
    runner: R,
    git: GitProbe,
}

impl<R: GhRunner> GitHubForgeProvider<R> {
    pub fn with_runner(runner: R) -> Self {
        Self {
            runner,
            git: GitProbe::new(),
        }
    }

    fn discover_repo(&self, root: &std::path::Path) -> Result<GraphFragment> {
        let Some(probe) = self.git.probe(root)? else {
            return Ok(GraphFragment::empty());
        };
        let Some((host, owner, repo)) = probe
            .remotes
            .iter()
            .find_map(|remote| parse_github_remote(&remote.url))
        else {
            return Ok(GraphFragment::empty());
        };

        let outcome = self
            .runner
            .list_pull_requests(&probe.worktree_root, GH_PR_LIST_FIELDS)?;
        let body = match outcome {
            GhOutcome::PullRequests(body) => body,
            GhOutcome::Unavailable(_) | GhOutcome::Failed { .. } => {
                return Ok(GraphFragment::empty());
            }
        };

        let records = GhPullRequestParser::new().parse(&body);
        if records.is_empty() {
            return Ok(GraphFragment::empty());
        }

        let context = RepoContext::new(
            RepoId::new(probe.common_dir.to_string_lossy().to_string()),
            host,
            owner,
            repo,
        );

        let short_refs = probe.local_branches.iter().cloned().collect();

        Ok(fragment_for_repo(&context, &short_refs, &records))
    }
}

impl<R: GhRunner + 'static> DiscoveryProvider for GitHubForgeProvider<R> {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let epoch = crate::discovery::current_epoch();
        let mut fragments = Vec::with_capacity(context.roots().len());
        for root in context.roots() {
            fragments.push(self.discover_repo(root)?);
        }
        let merged = merge_fragments(fragments);
        let mut fragment = GraphFragment {
            nodes: merged.nodes,
            candidate_links: merged.candidate_links,
            diagnostics: merged.diagnostics,
            node_provenance: merged.node_provenance,
        };
        crate::discovery::stamp_fragment(&mut fragment, FORGE_ADAPTER, epoch);
        Ok(fragment)
    }
}

/// H-EXT-012: `GitHubForgeProvider` also satisfies the
/// `ForgeAdapter` shape so the `LocalDiscoveryConfig.forge_adapters`
/// registry can carry it. Every method here delegates to the
/// existing `DiscoveryProvider` impl (for `discover`) or to
/// a URL-host matcher (for `claims_remote_url`).
impl<R: GhRunner + 'static> super::ForgeAdapter for GitHubForgeProvider<R> {
    fn provider(&self) -> &str {
        FORGE_ADAPTER
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        <Self as DiscoveryProvider>::discover(self, context)
    }

    fn claims_remote_url(&self, remote_url: &str) -> bool {
        remote_url_is_github(remote_url)
    }
}

/// True when `remote_url` names a GitHub host — either the
/// canonical `github.com` or a Conspectus operator's declared
/// enterprise host. H-EXT-012 keeps the initial impl focused on
/// `github.com`; enterprise-host routing is a follow-up.
fn remote_url_is_github(remote_url: &str) -> bool {
    let lower = remote_url.to_ascii_lowercase();
    // Accept both HTTPS (`https://github.com/o/r`,
    // `https://github.com:443/o/r`) and SSH
    // (`git@github.com:o/r.git`, `ssh://git@github.com/o/r`)
    // shapes. `contains` is fine here because the token is
    // both specific enough not to collide with unrelated hosts
    // and short enough that false-positives on user paths are
    // negligible.
    lower.contains("github.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_body_parses_to_zero_records() {
        assert!(GhPullRequestParser::new().parse("").is_empty());
        assert!(GhPullRequestParser::new().parse("   \n  ").is_empty());
        assert!(GhPullRequestParser::new().parse("[]").is_empty());
    }

    #[test]
    fn malformed_body_returns_no_records() {
        assert!(GhPullRequestParser::new().parse("not json").is_empty());
        assert!(GhPullRequestParser::new().parse("{}").is_empty());
    }

    #[test]
    fn single_open_pull_request_parses_with_all_fields() {
        let body = r#"[{
            "number": 7,
            "state": "OPEN",
            "url": "https://github.com/octo/repo/pull/7",
            "headRefName": "feature/login",
            "baseRefName": "main",
            "updatedAt": "2026-03-05T12:34:56Z",
            "headRepositoryOwner": {"login": "octo"},
            "headRepository": {"name": "repo"},
            "isDraft": false
        }]"#;

        let records = GhPullRequestParser::new().parse(body);

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.number, 7);
        assert_eq!(record.state, PullRequestState::Open);
        assert_eq!(record.head_ref, "feature/login");
        assert_eq!(record.base_ref.as_deref(), Some("main"));
        assert_eq!(record.head_owner.as_deref(), Some("octo"));
        assert_eq!(record.head_repo.as_deref(), Some("repo"));
        assert!(!record.is_draft);
        assert_eq!(
            record.url.as_deref(),
            Some("https://github.com/octo/repo/pull/7")
        );
        assert_eq!(record.updated_at.as_deref(), Some("2026-03-05T12:34:56Z"));
        assert_eq!(record.updated_epoch, Some(1_772_714_096));
    }

    #[test]
    fn multiple_pull_requests_preserve_input_order() {
        let body = r#"[
            {"number": 2, "state": "OPEN", "headRefName": "b"},
            {"number": 1, "state": "MERGED", "headRefName": "a"}
        ]"#;

        let records = GhPullRequestParser::new().parse(body);

        assert_eq!(
            records.iter().map(|r| r.number).collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert_eq!(records[0].state, PullRequestState::Open);
        assert_eq!(records[1].state, PullRequestState::Merged);
    }

    #[test]
    fn rows_missing_required_fields_are_skipped() {
        let body = r#"[
            {"state": "OPEN", "headRefName": "no-number"},
            {"number": 5, "state": "OPEN"},
            {"number": 6, "state": "OPEN", "headRefName": "ok"}
        ]"#;

        let records = GhPullRequestParser::new().parse(body);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].number, 6);
    }

    #[test]
    fn draft_flag_is_preserved() {
        let body = r#"[{"number": 1, "state": "OPEN", "headRefName": "wip", "isDraft": true}]"#;

        let records = GhPullRequestParser::new().parse(body);

        assert!(records[0].is_draft);
    }

    #[test]
    fn unknown_state_falls_through_to_other() {
        let body = r#"[{"number": 1, "state": "CONVERTED_TO_DISCUSSION", "headRefName": "x"}]"#;

        let records = GhPullRequestParser::new().parse(body);

        match &records[0].state {
            PullRequestState::Other(raw) => assert_eq!(raw, "CONVERTED_TO_DISCUSSION"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn missing_optional_fields_yield_none() {
        let body = r#"[{"number": 9, "state": "OPEN", "headRefName": "minimal"}]"#;

        let records = GhPullRequestParser::new().parse(body);

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert!(record.url.is_none());
        assert!(record.base_ref.is_none());
        assert!(record.head_owner.is_none());
        assert!(record.head_repo.is_none());
        assert!(record.updated_at.is_none());
        assert!(record.updated_epoch.is_none());
        assert!(!record.is_draft);
    }

    #[test]
    fn updated_at_with_offset_is_normalized_to_utc_epoch() {
        let utc = r#"[{"number": 1, "state": "OPEN", "headRefName": "a",
                       "updatedAt": "2026-03-05T12:00:00Z"}]"#;
        let offset = r#"[{"number": 2, "state": "OPEN", "headRefName": "b",
                          "updatedAt": "2026-03-05T14:00:00+02:00"}]"#;

        let utc_records = GhPullRequestParser::new().parse(utc);
        let offset_records = GhPullRequestParser::new().parse(offset);

        assert_eq!(
            utc_records[0].updated_epoch,
            offset_records[0].updated_epoch
        );
    }

    #[test]
    fn updated_at_with_unsupported_shape_falls_back_to_raw_string() {
        let body = r#"[{"number": 1, "state": "OPEN", "headRefName": "a",
                        "updatedAt": "not a timestamp"}]"#;

        let records = GhPullRequestParser::new().parse(body);

        assert_eq!(records[0].updated_at.as_deref(), Some("not a timestamp"));
        assert!(records[0].updated_epoch.is_none());
    }

    #[test]
    fn pull_request_state_round_trips_through_as_str() {
        assert_eq!(PullRequestState::Open.as_str(), "open");
        assert_eq!(PullRequestState::Closed.as_str(), "closed");
        assert_eq!(PullRequestState::Merged.as_str(), "merged");
        assert_eq!(PullRequestState::Other("DRAFT".into()).as_str(), "DRAFT");
    }

    fn context() -> RepoContext {
        RepoContext::github(RepoId::new("/workspace/repo/.git"), "octo", "repo")
    }

    fn record(number: u64, head_ref: &str, state: PullRequestState) -> PullRequestRecord {
        PullRequestRecord {
            number,
            state,
            url: Some(format!("https://github.com/octo/repo/pull/{number}")),
            head_ref: head_ref.to_string(),
            head_owner: Some("octo".to_string()),
            head_repo: Some("repo".to_string()),
            base_ref: Some("main".to_string()),
            is_draft: false,
            updated_at: None,
            updated_epoch: None,
        }
    }

    #[test]
    fn fragment_emits_forge_pr_node_per_record() {
        let context = context();
        let branches = BTreeSet::new();
        let records = vec![
            record(1, "feature/a", PullRequestState::Open),
            record(2, "feature/b", PullRequestState::Merged),
        ];

        let fragment = fragment_for_repo(&context, &branches, &records);

        let pr_nodes: Vec<_> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::ForgePr(pr) => Some(pr),
                _ => None,
            })
            .collect();
        assert_eq!(pr_nodes.len(), 2);
        assert_eq!(pr_nodes[0].id.number, 1);
        assert_eq!(pr_nodes[0].owner, "octo");
        assert_eq!(pr_nodes[0].repo, "repo");
        assert_eq!(pr_nodes[0].state.as_deref(), Some("open"));
        assert_eq!(pr_nodes[1].state.as_deref(), Some("merged"));
    }

    #[test]
    fn matched_head_ref_emits_branch_node_endpoint() {
        let context = context();
        let mut branches = BTreeSet::new();
        branches.insert("feature/login".to_string());
        let records = vec![record(7, "feature/login", PullRequestState::Open)];

        let fragment = fragment_for_repo(&context, &branches, &records);

        assert_eq!(fragment.candidate_links.len(), 1);
        let link = &fragment.candidate_links[0];
        assert_eq!(link.relation, RelationKind::BranchHasForgePr);
        match &link.target {
            LinkEndpoint::Node {
                id: NodeId::Branch(branch),
            } => {
                assert_eq!(branch.refname, "refs/heads/feature/login");
                assert_eq!(branch.repo, context.repo_id);
            }
            other => panic!("expected branch node target, got {other:?}"),
        }
    }

    #[test]
    fn unknown_head_ref_emits_unresolved_endpoint() {
        let context = context();
        let branches = BTreeSet::new();
        let records = vec![record(9, "feature/missing", PullRequestState::Open)];

        let fragment = fragment_for_repo(&context, &branches, &records);

        let link = &fragment.candidate_links[0];
        match &link.target {
            LinkEndpoint::Unresolved { evidence } => {
                assert_eq!(evidence.node_type, "branch");
                assert_eq!(evidence.native_id.as_deref(), Some("feature/missing"));
                assert_eq!(
                    evidence.metadata.get("host"),
                    Some(&Value::String("github.com".to_string()))
                );
            }
            other => panic!("expected unresolved branch evidence, got {other:?}"),
        }
    }

    #[test]
    fn draft_flag_is_carried_onto_node_and_link_metadata() {
        let context = context();
        let branches = BTreeSet::new();
        let mut draft = record(3, "wip", PullRequestState::Open);
        draft.is_draft = true;

        let fragment = fragment_for_repo(&context, &branches, &[draft]);

        let pr = match &fragment.nodes[0] {
            GraphNode::ForgePr(pr) => pr,
            other => panic!("expected ForgePr node, got {other:?}"),
        };
        assert!(pr.is_draft);
        let link = &fragment.candidate_links[0];
        assert_eq!(
            link.source_metadata.fields.get("is_draft"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn state_is_preserved_for_open_closed_merged() {
        let context = context();
        let branches = BTreeSet::new();
        let records = vec![
            record(1, "open-branch", PullRequestState::Open),
            record(2, "closed-branch", PullRequestState::Closed),
            record(3, "merged-branch", PullRequestState::Merged),
        ];

        let fragment = fragment_for_repo(&context, &branches, &records);

        let states: Vec<_> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::ForgePr(pr) => pr.state.clone(),
                _ => None,
            })
            .collect();
        assert_eq!(states, vec!["open", "closed", "merged"]);
    }

    #[test]
    fn node_and_link_ids_are_stable_across_repeated_runs() {
        let context = context();
        let mut branches = BTreeSet::new();
        branches.insert("feature/login".to_string());
        let records = vec![record(11, "feature/login", PullRequestState::Open)];

        let first = fragment_for_repo(&context, &branches, &records);
        let second = fragment_for_repo(&context, &branches, &records);

        let extract_ids = |fragment: &GraphFragment| -> (Vec<NodeId>, Vec<String>) {
            (
                fragment.nodes.iter().map(GraphNode::id).collect(),
                fragment
                    .candidate_links
                    .iter()
                    .map(|link| link.id.clone())
                    .collect(),
            )
        };
        assert_eq!(extract_ids(&first), extract_ids(&second));
    }

    #[test]
    fn updated_epoch_is_preserved_on_the_node() {
        let context = context();
        let branches = BTreeSet::new();
        let mut rec = record(5, "head", PullRequestState::Open);
        rec.updated_epoch = Some(1_772_714_096);

        let fragment = fragment_for_repo(&context, &branches, &[rec]);

        let pr = match &fragment.nodes[0] {
            GraphNode::ForgePr(pr) => pr,
            other => panic!("expected ForgePr, got {other:?}"),
        };
        assert_eq!(pr.updated_epoch, Some(1_772_714_096));
    }

    #[test]
    fn empty_records_yield_empty_fragment() {
        let fragment = fragment_for_repo(&context(), &BTreeSet::new(), &[]);

        assert!(fragment.nodes.is_empty());
        assert!(fragment.candidate_links.is_empty());
    }

    #[test]
    fn parses_https_github_remotes() {
        assert_eq!(
            parse_github_remote("https://github.com/octo/repo.git"),
            Some((
                "github.com".to_string(),
                "octo".to_string(),
                "repo".to_string()
            ))
        );
        assert_eq!(
            parse_github_remote("https://github.com/octo/repo"),
            Some((
                "github.com".to_string(),
                "octo".to_string(),
                "repo".to_string()
            ))
        );
    }

    #[test]
    fn parses_ssh_and_git_at_github_remotes() {
        assert_eq!(
            parse_github_remote("git@github.com:octo/repo.git"),
            Some((
                "github.com".to_string(),
                "octo".to_string(),
                "repo".to_string()
            ))
        );
        assert_eq!(
            parse_github_remote("ssh://git@github.com/octo/repo.git"),
            Some((
                "github.com".to_string(),
                "octo".to_string(),
                "repo".to_string()
            ))
        );
    }

    #[test]
    fn parses_github_enterprise_hosts() {
        assert_eq!(
            parse_github_remote("https://github.acme.com/octo/repo.git"),
            Some((
                "github.acme.com".to_string(),
                "octo".to_string(),
                "repo".to_string()
            ))
        );
        assert_eq!(
            parse_github_remote("git@code.example.github.com:octo/repo.git"),
            Some((
                "code.example.github.com".to_string(),
                "octo".to_string(),
                "repo".to_string()
            ))
        );
    }

    #[test]
    fn rejects_non_github_remotes() {
        assert!(parse_github_remote("https://gitlab.com/octo/repo.git").is_none());
        assert!(parse_github_remote("git@bitbucket.org:octo/repo.git").is_none());
        assert!(parse_github_remote("file:///tmp/repo.git").is_none());
        assert!(parse_github_remote("").is_none());
    }

    #[test]
    fn rejects_malformed_remote_paths() {
        assert!(parse_github_remote("https://github.com/octo").is_none());
        assert!(parse_github_remote("https://github.com//repo.git").is_none());
        assert!(parse_github_remote("https://github.com/octo/repo/extra").is_none());
    }

    use crate::discovery::forge::{FakeGh, GhUnavailableReason};
    use std::process::Command as ProcessCommand;
    use tempfile::TempDir;

    fn make_git_repo(temp: &TempDir, remote: &str) -> std::path::PathBuf {
        let root = temp.path().join("repo");
        std::fs::create_dir(&root).expect("create repo dir");
        run_git(&root, &["init", "--initial-branch", "main"]);
        run_git(&root, &["config", "user.name", "Conspectus Test"]);
        run_git(&root, &["config", "user.email", "test@example.invalid"]);
        run_git(&root, &["remote", "add", "origin", remote]);
        std::fs::write(root.join("README.md"), "fixture\n").expect("write fixture");
        run_git(&root, &["add", "README.md"]);
        run_git(&root, &["commit", "-m", "initial"]);
        root
    }

    fn run_git(root: &std::path::Path, args: &[&str]) {
        let output = ProcessCommand::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn provider_emits_no_rows_when_repo_has_no_github_remote() {
        let temp = TempDir::new().expect("temp dir");
        let root = make_git_repo(&temp, "git@gitlab.com:octo/repo.git");
        let provider = GitHubForgeProvider::with_runner(FakeGh::with_pull_requests("[]"));

        let context = DiscoveryContext::from_roots([root]).expect("context");
        let fragment = provider.discover(&context).expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn provider_emits_no_rows_when_gh_unavailable() {
        let temp = TempDir::new().expect("temp dir");
        let root = make_git_repo(&temp, "git@github.com:octo/repo.git");
        let provider = GitHubForgeProvider::with_runner(FakeGh::unavailable(
            GhUnavailableReason::NotAuthenticated,
        ));

        let context = DiscoveryContext::from_roots([root]).expect("context");
        let fragment = provider.discover(&context).expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn provider_emits_forge_pr_nodes_for_github_repo() {
        let temp = TempDir::new().expect("temp dir");
        let root = make_git_repo(&temp, "git@github.com:octo/repo.git");
        let body = r#"[
            {"number": 1, "state": "OPEN", "headRefName": "main", "isDraft": false},
            {"number": 2, "state": "MERGED", "headRefName": "feature", "isDraft": false}
        ]"#;
        let provider = GitHubForgeProvider::with_runner(FakeGh::with_pull_requests(body));

        let context = DiscoveryContext::from_roots([root]).expect("context");
        let fragment = provider.discover(&context).expect("discover");

        let pr_nodes: Vec<_> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::ForgePr(pr) => Some(pr),
                _ => None,
            })
            .collect();
        assert_eq!(pr_nodes.len(), 2);
        assert_eq!(fragment.candidate_links.len(), 2);
    }

    #[test]
    fn provider_emits_branch_node_endpoint_when_head_ref_matches_current_branch() {
        let temp = TempDir::new().expect("temp dir");
        let root = make_git_repo(&temp, "git@github.com:octo/repo.git");
        let body = r#"[{"number": 1, "state": "OPEN", "headRefName": "main"}]"#;
        let provider = GitHubForgeProvider::with_runner(FakeGh::with_pull_requests(body));

        let context = DiscoveryContext::from_roots([root]).expect("context");
        let fragment = provider.discover(&context).expect("discover");

        let link = &fragment.candidate_links[0];
        match &link.target {
            LinkEndpoint::Node {
                id: NodeId::Branch(_),
            } => {}
            other => panic!("expected branch node endpoint, got {other:?}"),
        }
    }

    #[test]
    fn provider_emits_branch_node_endpoint_when_head_ref_matches_sibling_local_branch() {
        let temp = TempDir::new().expect("temp dir");
        let root = make_git_repo(&temp, "git@github.com:octo/repo.git");
        run_git(&root, &["branch", "feature/sibling"]);
        let body = r#"[{"number": 2, "state": "OPEN", "headRefName": "feature/sibling"}]"#;
        let provider = GitHubForgeProvider::with_runner(FakeGh::with_pull_requests(body));

        let context = DiscoveryContext::from_roots([root]).expect("context");
        let fragment = provider.discover(&context).expect("discover");

        let link = &fragment.candidate_links[0];
        match &link.target {
            LinkEndpoint::Node {
                id: NodeId::Branch(branch),
            } => assert_eq!(branch.refname, "refs/heads/feature/sibling"),
            other => panic!("expected sibling branch node endpoint, got {other:?}"),
        }
    }
}
