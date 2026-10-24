//! Turn a resolved snapshot into history observations (ADR 0140).
//!
//! Only fresh probes count: a mux is observed only when its server's
//! probe ran at or after `fresh_since` and listed it, and checkouts,
//! repos, and PRs only when the provider that produced them ran in
//! that window. Data reused from a warm start, a daemon snapshot, or
//! a discovery cache entry carries its older time and is left out, so
//! it can neither move `last_seen` nor bring back a reaped record.

use std::collections::BTreeMap;
use std::path::Path;

use crate::config::HistoryConfig;
use crate::discovery::harness::adapter_in_command;
use crate::discovery::{orchestrator, providers};
use crate::model::{
    ArgvSource, CheckoutRecipe, GraphNode, GraphSnapshot, HistoryKind, HostedSession,
    MuxIncarnation, MuxProbeStatus, MuxRecipe, MuxSessionNode, NodeId, PrRecipe, RecordDisplay,
    RecordOrigin, RecordedRemote, RelationKind, RepoRecipe, WorktreeKind, probe_key,
};

use super::merge::{
    CheckoutObservation, MuxObservation, Observations, PrObservation, RepoObservation,
};

/// The adapter that stamps links and nodes history projects, so the
/// recorder never mistakes history's own fallback for evidence.
pub const HISTORY_ADAPTER: &str = "history";

/// What a recording pass needs besides the snapshot.
pub struct RecordContext<'a> {
    /// Probes and providers older than this didn't run this cycle.
    pub fresh_since: i64,
    pub config: &'a HistoryConfig,
    /// The operator's home, for `~` in display context.
    pub home: Option<&'a Path>,
}

/// Every observation the snapshot's fresh data supports.
pub fn observe(snapshot: &GraphSnapshot, ctx: &RecordContext<'_>) -> Observations {
    let hosted = hosted_sessions(snapshot);
    let mut observations = Observations::default();
    for node in &snapshot.nodes {
        if snapshot.node_presence.contains_key(&node.id()) {
            continue;
        }
        match node {
            GraphNode::MuxSession(mux) => {
                if let Some(obs) = observe_mux(snapshot, mux, &hosted, ctx) {
                    observations.muxes.push(obs);
                }
            }
            GraphNode::Checkout(checkout) => {
                if let Some(obs) = observe_checkout(snapshot, checkout, ctx) {
                    observations.checkouts.push(obs);
                }
            }
            GraphNode::Repo(repo) => {
                if let Some(obs) = observe_repo(snapshot, repo, ctx) {
                    observations.repos.push(obs);
                }
            }
            GraphNode::ForgePr(pr) => {
                if let Some(obs) = observe_pr(snapshot, pr, ctx) {
                    observations.prs.push(obs);
                }
            }
            _ => {}
        }
    }
    observations
}

/// Sessions the resolver attributed to each mux, from evidence other
/// than history's own fallback.
fn hosted_sessions(snapshot: &GraphSnapshot) -> BTreeMap<NodeId, Vec<(String, String)>> {
    let adapters: BTreeMap<&str, &str> = snapshot
        .candidate_links
        .iter()
        .map(|link| (link.id.as_str(), link.source_metadata.adapter.as_str()))
        .collect();
    let mut hosted: BTreeMap<NodeId, Vec<(String, String)>> = BTreeMap::new();
    for rel in &snapshot.resolved_relationships {
        if rel.relation != RelationKind::LinkedToMux {
            continue;
        }
        let Some(link_id) = &rel.selected_link_id else {
            continue;
        };
        if adapters.get(link_id.as_str()) == Some(&HISTORY_ADAPTER) {
            continue;
        }
        if let NodeId::AgentSession(session) = &rel.source {
            hosted
                .entry(rel.target.clone())
                .or_default()
                .push((session.harness_key.clone(), session.session_key.clone()));
        }
    }
    hosted
}

fn observe_mux(
    snapshot: &GraphSnapshot,
    mux: &MuxSessionNode,
    hosted: &BTreeMap<NodeId, Vec<(String, String)>>,
    ctx: &RecordContext<'_>,
) -> Option<MuxObservation> {
    let probe = snapshot
        .mux_probes
        .get(&probe_key(&mux.backend, mux.socket.as_deref()))?;
    if probe.status != MuxProbeStatus::Listed || probe.epoch < ctx.fresh_since {
        return None;
    }
    let name = mux.session_name().to_string();
    if probe.exited.contains(&name) || ctx.config.ignores(HistoryKind::Mux, &name) {
        return None;
    }
    let seen_at = probe.epoch;
    let id = NodeId::MuxSession(mux.id.clone());
    let sessions: Vec<HostedSession> = hosted
        .get(&id)
        .into_iter()
        .flatten()
        .map(|(harness, session_id)| HostedSession {
            harness: harness.clone(),
            session_id: session_id.clone(),
            first_seen: seen_at,
            last_seen: seen_at,
        })
        .collect();
    let argv = mux
        .active_pane_start_command
        .as_deref()
        .map(crate::argv::split_command_line)
        .filter(|argv| !argv.is_empty());
    let harness = sessions
        .first()
        .map(|hosted| hosted.harness.clone())
        .or_else(|| {
            let tokens = argv
                .iter()
                .flatten()
                .map(String::as_str)
                .chain(mux.active_pane_command.as_deref());
            adapter_in_command(tokens).map(|adapter| adapter.harness_key().to_string())
        });
    let orchestrator =
        orchestrator::owner_of_mux(&name).map(|owner| owner.display_name.to_string());
    let context = mux_context(
        harness.as_deref(),
        argv.as_deref(),
        mux.cwd.as_deref(),
        ctx.home,
    );
    Some(MuxObservation {
        seen_at,
        key: id.to_string(),
        recipe: MuxRecipe {
            backend: mux.backend.clone(),
            socket: mux.socket.clone(),
            name: name.clone(),
            cwd: mux.cwd.clone(),
            incarnation: Some(MuxIncarnation {
                session_id: mux.backend_session_id.clone(),
                created: mux.created_epoch,
            }),
            argv_source: argv.as_ref().map(|_| ArgvSource::Observed),
            argv,
            harness,
            orchestrator,
            hosted: sessions,
            extra: Default::default(),
        },
        display: RecordDisplay {
            label: name,
            context,
        },
        origin: RecordOrigin::Observed,
    })
}

/// `harness · ~/cwd`, or the command when there is no harness.
pub fn mux_context(
    harness: Option<&str>,
    argv: Option<&[String]>,
    cwd: Option<&str>,
    home: Option<&Path>,
) -> Option<String> {
    let cwd = cwd.map(|cwd| tilde(cwd, home));
    match (harness, argv, cwd) {
        (Some(harness), _, Some(cwd)) => Some(format!("{harness} · {cwd}")),
        (Some(harness), _, None) => Some(harness.to_string()),
        (None, Some(argv), _) => Some(argv.join(" ")),
        (None, None, cwd) => cwd,
    }
}

/// `path` with the home directory spelled `~`.
pub fn tilde(path: &str, home: Option<&Path>) -> String {
    let Some(home) = home.and_then(Path::to_str).filter(|home| !home.is_empty()) else {
        return path.to_string();
    };
    match path.strip_prefix(home) {
        Some("") => "~".to_string(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// Whether a node came from a provider in `providers` that ran at or
/// after `fresh_since`, and when.
fn fresh_epoch(
    snapshot: &GraphSnapshot,
    id: &NodeId,
    providers: &[&str],
    fresh_since: i64,
) -> Option<i64> {
    let provenance = snapshot.node_provenance.get(id)?;
    if !providers.contains(&provenance.provider.as_str()) {
        return None;
    }
    provenance
        .freshness_epoch
        .filter(|epoch| *epoch >= fresh_since)
}

const GIT_PROVIDERS: &[&str] = &[providers::GIT, providers::GIT_CWD];

fn observe_checkout(
    snapshot: &GraphSnapshot,
    checkout: &crate::model::CheckoutNode,
    ctx: &RecordContext<'_>,
) -> Option<CheckoutObservation> {
    let id = NodeId::Checkout(checkout.id.clone());
    let seen_at = fresh_epoch(snapshot, &id, GIT_PROVIDERS, ctx.fresh_since)?;
    if ctx.config.ignores(HistoryKind::Checkout, &checkout.root) {
        return None;
    }
    let branch = checkout
        .current_branch
        .as_ref()
        .map(|branch| short_ref(&branch.refname).to_string());
    let upstream = checkout.current_branch.as_ref().and_then(|branch| {
        snapshot.nodes.iter().find_map(|node| match node {
            GraphNode::Branch(node) if node.id == *branch => node.upstream.clone(),
            _ => None,
        })
    });
    let repo_name = repo_name(&checkout.id.repo.common_dir);
    let label = last_component(&checkout.root);
    let context = match &branch {
        Some(branch) => format!("{repo_name} · {branch}"),
        None => repo_name,
    };
    Some(CheckoutObservation {
        seen_at,
        key: id.to_string(),
        recipe: CheckoutRecipe {
            repo: NodeId::Repo(checkout.id.repo.clone()).to_string(),
            root: checkout.root.clone(),
            linked: checkout
                .worktree
                .as_ref()
                .is_some_and(|meta| meta.kind == WorktreeKind::Linked),
            branch,
            head: checkout.head_commit.clone(),
            upstream,
            extra: Default::default(),
        },
        display: RecordDisplay {
            label,
            context: Some(context),
        },
        origin: RecordOrigin::Observed,
        rebuilt_from: None,
    })
}

fn observe_repo(
    snapshot: &GraphSnapshot,
    repo: &crate::model::RepoNode,
    ctx: &RecordContext<'_>,
) -> Option<RepoObservation> {
    let id = NodeId::Repo(repo.id.clone());
    let seen_at = fresh_epoch(snapshot, &id, GIT_PROVIDERS, ctx.fresh_since)?;
    if ctx.config.ignores(HistoryKind::Repo, &repo.common_dir) {
        return None;
    }
    let remotes: Vec<RecordedRemote> = repo
        .remotes
        .iter()
        .filter_map(|remote| {
            let (name, url) = remote.split_once('=')?;
            Some(RecordedRemote {
                name: name.to_string(),
                url: url.to_string(),
            })
        })
        .collect();
    let primary_root = snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::Checkout(checkout)
            if checkout.id.repo == repo.id
                && checkout
                    .worktree
                    .as_ref()
                    .is_some_and(|meta| meta.kind == WorktreeKind::Primary) =>
        {
            Some(checkout.root.clone())
        }
        _ => None,
    });
    let context = remotes
        .first()
        .map(|remote| format!("{} {}", remote.name, remote.url));
    Some(RepoObservation {
        seen_at,
        key: id.to_string(),
        recipe: RepoRecipe {
            common_dir: repo.common_dir.clone(),
            remotes,
            primary_root,
            default_branch: None,
            extra: Default::default(),
        },
        display: RecordDisplay {
            label: repo_name(&repo.common_dir),
            context,
        },
        origin: RecordOrigin::Observed,
    })
}

fn observe_pr(
    snapshot: &GraphSnapshot,
    pr: &crate::model::ForgePrNode,
    ctx: &RecordContext<'_>,
) -> Option<PrObservation> {
    let id = NodeId::ForgePr(pr.id.clone());
    let provenance = snapshot.node_provenance.get(&id)?;
    if provenance.provider == HISTORY_ADAPTER {
        return None;
    }
    let seen_at = provenance
        .freshness_epoch
        .filter(|epoch| *epoch >= ctx.fresh_since)?;
    let label = format!("{}/{}#{}", pr.owner, pr.repo, pr.number);
    if ctx.config.ignores(HistoryKind::Pr, &label) {
        return None;
    }
    Some(PrObservation {
        seen_at,
        key: id.to_string(),
        recipe: PrRecipe {
            provider: pr.provider.clone(),
            host: pr.host.clone(),
            owner: pr.owner.clone(),
            repo: pr.repo.clone(),
            number: pr.number,
            title: pr.title.clone(),
            state: pr.state.clone(),
            draft: pr.is_draft,
            url: pr.url.clone(),
            head_ref: pr.head_ref.clone(),
            extra: Default::default(),
        },
        display: RecordDisplay {
            label: format!("#{}", pr.number),
            context: Some(match &pr.title {
                Some(title) => format!("{}/{} · {title}", pr.owner, pr.repo),
                None => format!("{}/{}", pr.owner, pr.repo),
            }),
        },
    })
}

/// `refs/heads/feature/x` → `feature/x`.
pub fn short_ref(refname: &str) -> &str {
    refname.strip_prefix("refs/heads/").unwrap_or(refname)
}

/// A repo's name from its common dir: the directory holding `.git`,
/// or the bare repo's own name without `.git`.
pub fn repo_name(common_dir: &str) -> String {
    let path = Path::new(common_dir);
    let named = if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    let name = last_component(&named.to_string_lossy());
    name.strip_suffix(".git")
        .map_or_else(|| name.clone(), str::to_string)
}

fn last_component(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
