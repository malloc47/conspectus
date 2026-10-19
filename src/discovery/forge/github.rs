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
use std::time::Instant;

use anyhow::Result;
use serde_json::Value;

use crate::discovery::{DiscoveryContext, GraphFragment, merge_fragments};
use crate::model::{
    BranchId, Confidence, Diagnostic, ForgePrId, ForgePrNode, Freshness, GraphLink, GraphNode,
    LinkEndpoint, LinkState, Metadata, NodeId, Provenance, RelationKind, RepoId, SourceMetadata,
    UnresolvedEndpoint,
};

use super::{
    GH_PR_LIST_FIELDS, GITHUB_DEFAULT_HOST, GITHUB_PROVIDER, GhOutcome, GhRunner, RemoteListing,
};

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
/// `repo_remotes` holds the local repo's GitHub remotes as lower-cased
/// `(owner, repo)`: a PR links to a local branch only when its head
/// lives in one of them (ADR 0115).
pub fn fragment_for_repo(
    context: &RepoContext,
    discovered_branch_short_refs: &BTreeSet<String>,
    repo_remotes: &BTreeSet<(String, String)>,
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
            repo_remotes,
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
        head_ref: Some(record.head_ref.clone()).filter(|head| !head.is_empty()),
    }
}

fn branch_pr_link(
    pr_id: &ForgePrId,
    context: &RepoContext,
    record: &PullRequestRecord,
    discovered_branch_short_refs: &BTreeSet<String>,
    repo_remotes: &BTreeSet<(String, String)>,
) -> GraphLink {
    // `gh` omits the head repo only when it no longer exists; treat
    // that as the repo queried.
    let head = (
        record
            .head_owner
            .as_deref()
            .unwrap_or(&context.owner)
            .to_ascii_lowercase(),
        record
            .head_repo
            .as_deref()
            .unwrap_or(&context.repo)
            .to_ascii_lowercase(),
    );
    let local = repo_remotes.contains(&head);
    let target = if local && discovered_branch_short_refs.contains(&record.head_ref) {
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
    // The local repo the PR was queried for, so a PR with no local
    // branch can still be filed under its repo (ADR 0129).
    metadata.insert(
        "local_repo".to_string(),
        Value::String(context.repo_id.common_dir.clone()),
    );
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
    fields.insert(
        crate::model::source_field::IS_DRAFT.to_string(),
        Value::Bool(record.is_draft),
    );
    if let Some(updated) = record.updated_epoch {
        fields.insert(
            crate::model::source_field::UPDATED_EPOCH.to_string(),
            Value::Number(updated.into()),
        );
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
    let number = row.get("number").and_then(serde_json::Value::as_u64)?;
    let head_ref = string_field(row.get("headRefName"))?;

    let state = row.get("state").and_then(|v| v.as_str()).map_or_else(
        || PullRequestState::Other(String::new()),
        PullRequestState::parse,
    );

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
/// the URL points at a built-in GitHub host. The parser is tolerant: it
/// accepts `https://github.com/owner/repo[.git]`, `git@github.com:owner/repo[.git]`,
/// and `ssh://git@github.com/owner/repo[.git]`, plus GitHub Enterprise
/// hosts whose names start with `github.` or end in `.github.com`.
/// Hosts `gh` is signed in to are recognized through
/// [`parse_github_remote_with`] (ADR 0128).
pub fn parse_github_remote(url: &str) -> Option<(String, String, String)> {
    parse_github_remote_with(url, &GithubHosts::default())
}

/// [`parse_github_remote`] that also accepts the hosts in `hosts`.
pub fn parse_github_remote_with(
    url: &str,
    hosts: &GithubHosts,
) -> Option<(String, String, String)> {
    let remote = parse_remote(url)?;
    hosts.recognizes(&remote.0).then_some(remote)
}

/// The `(host, owner, repo)` of a git remote URL, whatever the host.
fn parse_remote(url: &str) -> Option<(String, String, String)> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (host, path) = if let Some(rest) = trimmed.strip_prefix("https://") {
        split_host_and_path(rest)?
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        split_host_and_path(rest)?
    } else if let Some(rest) = trimmed.strip_prefix("ssh://") {
        let after_user = rest.split_once('@').map_or(rest, |(_, after)| after);
        split_host_and_path(after_user)?
    } else {
        let rest = trimmed.strip_prefix("git@")?;
        let (host, path) = rest.split_once(':')?;
        (host.to_string(), path.to_string())
    };

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

/// The hosts whose remotes count as GitHub: the built-in names plus
/// the hosts `gh` is signed in to (ADR 0128).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GithubHosts {
    signed_in: BTreeSet<String>,
}

impl GithubHosts {
    /// The built-in names and `signed_in`, compared case-insensitively.
    pub fn with_signed_in<I, S>(signed_in: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            signed_in: signed_in
                .into_iter()
                .map(|host| host.as_ref().trim().to_ascii_lowercase())
                .filter(|host| !host.is_empty())
                .collect(),
        }
    }

    pub fn recognizes(&self, host: &str) -> bool {
        looks_like_github_host(host) || self.signed_in.contains(&host.to_ascii_lowercase())
    }
}

fn split_host_and_path(rest: &str) -> Option<(String, String)> {
    let (host, path) = rest.split_once('/')?;
    Some((host.to_string(), path.to_string()))
}

fn looks_like_github_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    lower == "github.com" || lower.ends_with(".github.com") || lower.starts_with("github.")
}

/// GitHub PR discovery over the repos the graph found (ADR 0115). For
/// each distinct GitHub remote of each repo it asks `gh` for open PRs
/// and for PRs updated recently, a few calls at a time. Each remote's
/// answer is reused for the forge interval (ADR 0116). Failures become
/// diagnostics and leave other repos' PRs intact.
pub struct GitHubForgeProvider<R: GhRunner> {
    runner: R,
}

impl<R: GhRunner> GitHubForgeProvider<R> {
    pub fn with_runner(runner: R) -> Self {
        Self { runner }
    }

    /// The hosts whose remotes count as GitHub for `repos`. `gh` is
    /// asked which hosts it is signed in to only when a repo has a
    /// remote on a host the built-in names don't cover, and the answer
    /// is cached with the listings (ADR 0128).
    fn hosts(&self, repos: &[super::ForgeRepo], context: &DiscoveryContext) -> GithubHosts {
        let builtin = GithubHosts::default();
        let has_unfamiliar_host = repos
            .iter()
            .flat_map(|repo| &repo.remotes)
            .filter_map(|remote| parse_remote(&remote.url))
            .any(|(host, _, _)| !builtin.recognizes(&host));
        if !has_unfamiliar_host {
            return builtin;
        }
        let cache = &context.caches().forge;
        let signed_in = cache
            .fresh_hosts(context.forge_max_age())
            .unwrap_or_else(|| {
                let hosts = self.runner.authenticated_hosts();
                cache.set_hosts(hosts.clone());
                hosts
            });
        GithubHosts::with_signed_in(signed_in)
    }

    /// The listing for every distinct GitHub remote of `repos`: cached
    /// ones while they are fresh (ADR 0116), and the rest fetched now,
    /// a few remotes at a time.
    fn listings(
        &self,
        repos: &[super::ForgeRepo],
        hosts: &GithubHosts,
        context: &DiscoveryContext,
    ) -> BTreeMap<RemoteSlug, RemoteListing> {
        let cache = &context.caches().forge;
        let max_age = context.forge_max_age();
        let remotes: BTreeSet<RemoteSlug> = repos
            .iter()
            .flat_map(|repo| github_remotes(repo, hosts))
            .collect();
        let mut listings = BTreeMap::new();
        let mut stale = Vec::new();
        for remote in remotes {
            match cache.fresh(&remote_key(&remote), max_age) {
                Some(listing) => {
                    listings.insert(remote, listing);
                }
                None => stale.push(remote),
            }
        }
        if stale.is_empty() {
            return listings;
        }

        let epoch = crate::discovery::current_epoch();
        let since = chrono::DateTime::from_timestamp(epoch - super::RECENT_PR_DAYS * 86_400, 0)
            .map(|at| at.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        let filters = [
            super::PrListFilter::Open {
                limit: super::OPEN_PR_LIMIT,
            },
            super::PrListFilter::UpdatedSince {
                since,
                limit: super::RECENT_PR_LIMIT,
            },
        ];
        let jobs: Vec<(&RemoteSlug, &super::PrListFilter)> = stale
            .iter()
            .flat_map(|remote| filters.iter().map(move |filter| (remote, filter)))
            .collect();
        let outcomes = run_bounded(&jobs, super::MAX_CONCURRENT_GH, |(remote, filter)| {
            self.runner
                .list_pull_requests(&remote_key(remote), filter, GH_PR_LIST_FIELDS)
        });

        let fetched = Instant::now();
        let mut answered: BTreeMap<&RemoteSlug, RemoteListing> = BTreeMap::new();
        for ((remote, _), outcome) in jobs.iter().zip(outcomes) {
            let listing = answered.entry(remote).or_insert_with(|| RemoteListing {
                fetched,
                epoch,
                bodies: Vec::new(),
                failures: Vec::new(),
            });
            let failed = |message: String| {
                format!("gh pr list --repo {} failed: {message}", remote_key(remote))
            };
            match outcome {
                Ok(GhOutcome::PullRequests(body)) => listing.bodies.push(body),
                Ok(GhOutcome::Unavailable(reason)) => {
                    listing.failures.push(reason.as_str().to_string());
                }
                Ok(GhOutcome::Failed { message, .. }) => listing.failures.push(failed(message)),
                Err(err) => listing.failures.push(failed(format!("{err:#}"))),
            }
        }
        for (remote, mut listing) in answered {
            let key = remote_key(remote);
            // A failed refetch keeps the PRs the remote last returned,
            // with their age, so going offline doesn't empty the view.
            if !listing.failures.is_empty()
                && let Some(previous) = cache.get(&key)
                && !previous.bodies.is_empty()
            {
                listing.bodies = previous.bodies;
                listing.epoch = previous.epoch;
            }
            cache.set(key, listing.clone());
            listings.insert(remote.clone(), listing);
        }
        listings
    }
}

/// A remote as `gh --repo` takes it, and as forge listings are keyed.
fn remote_key((host, owner, name): &RemoteSlug) -> String {
    format!("{host}/{owner}/{name}")
}

/// A remote as `(host, owner, repo)`.
type RemoteSlug = (String, String, String);

impl<R: GhRunner + 'static> super::ForgeAdapter for GitHubForgeProvider<R> {
    fn provider(&self) -> &str {
        FORGE_ADAPTER
    }

    fn discover(
        &self,
        repos: &[super::ForgeRepo],
        context: &DiscoveryContext,
    ) -> Result<GraphFragment> {
        let hosts = self.hosts(repos, context);
        let listings = self.listings(repos, &hosts, context);

        let mut fragments = Vec::new();
        let mut diagnostics: BTreeSet<Diagnostic> = BTreeSet::new();
        for repo in repos {
            let remotes: BTreeSet<(String, String)> = github_remotes(repo, &hosts)
                .into_iter()
                .map(|(_, owner, name)| (owner.to_ascii_lowercase(), name.to_ascii_lowercase()))
                .collect();
            for remote in github_remotes(repo, &hosts) {
                let Some(listing) = listings.get(&remote) else {
                    continue;
                };
                diagnostics.extend(listing.failures.iter().cloned().map(unavailable));
                // Merge the open and recent queries by PR number.
                let mut records: BTreeMap<u64, PullRequestRecord> = BTreeMap::new();
                for body in &listing.bodies {
                    for record in GhPullRequestParser::new().parse(body) {
                        records.entry(record.number).or_insert(record);
                    }
                }
                if records.is_empty() {
                    continue;
                }
                let (host, owner, name) = remote;
                let context = RepoContext::new(repo.repo_id.clone(), host, owner, name);
                let records: Vec<PullRequestRecord> = records.into_values().collect();
                let mut fragment =
                    fragment_for_repo(&context, &repo.local_branches, &remotes, &records);
                // Stamp when the forge answered, not when the listing
                // was reused, so the freshness gate sees its real age.
                crate::discovery::stamp_fragment(&mut fragment, FORGE_ADAPTER, listing.epoch);
                fragments.push(fragment);
            }
        }
        let merged = merge_fragments(fragments);
        Ok(GraphFragment {
            nodes: merged.nodes,
            candidate_links: merged.candidate_links,
            diagnostics: merged.diagnostics.into_iter().chain(diagnostics).collect(),
            node_provenance: merged.node_provenance,
        })
    }

    fn claims_remote_url(&self, remote_url: &str) -> bool {
        remote_url_is_github(remote_url)
    }
}

/// A forge-discovery failure, as a diagnostic the TUI's forge chip
/// reads (ADR 0115).
fn unavailable(reason: String) -> Diagnostic {
    Diagnostic::ProviderUnavailable {
        provider: FORGE_ADAPTER.to_string(),
        reason,
    }
}

/// A repo's distinct GitHub remotes.
fn github_remotes(repo: &super::ForgeRepo, hosts: &GithubHosts) -> Vec<RemoteSlug> {
    let mut remotes: Vec<RemoteSlug> = repo
        .remotes
        .iter()
        .filter_map(|remote| parse_github_remote_with(&remote.url, hosts))
        .collect();
    remotes.sort();
    remotes.dedup();
    remotes
}

/// Run `call` over `jobs`, at most `limit` at a time on scoped threads,
/// returning results in job order.
fn run_bounded<J: Sync, T: Send>(
    jobs: &[J],
    limit: usize,
    call: impl Fn(&J) -> T + Sync,
) -> Vec<T> {
    let mut results = Vec::with_capacity(jobs.len());
    for chunk in jobs.chunks(limit.max(1)) {
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk.iter().map(|job| scope.spawn(|| call(job))).collect();
            for handle in handles {
                results.push(handle.join().expect("gh query thread"));
            }
        });
    }
    results
}

/// True when `remote_url` names `github.com`. This is the adapter's
/// static claim; enterprise hosts are recognized at discovery time from
/// the hosts `gh` is signed in to (ADR 0128).
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
#[path = "github_tests.rs"]
mod tests;
