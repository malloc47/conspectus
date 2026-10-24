//! `conspectus search <QUERY>`.
//!
//! Finds sessions by what was said in them. The session list comes from
//! the same warm-start snapshot path as `table` and `graph`; transcripts
//! are read in this process by the content scanner and never pass
//! through the daemon. Results are in memory only for the life of the
//! command.
//!
//! Exit status follows grep: 0 when something matched, 1 when nothing
//! did, 2 on an error. The errors are trapped here because `main` exits
//! 1 on any error, which would be indistinguishable from "no match".

use std::collections::{BTreeSet, HashMap};
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use anstyle::{Effects, Reset, Style};
use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use clap::{Args, ValueEnum};
use serde_json::{Value, json};

use crate::config;
use crate::content_search::{
    CancelToken, Progress, ScanOptions, ScanSession, SessionKey, hits_for, plan_sessions,
    scan_blocking,
};
use crate::model::{GraphNode, GraphSnapshot, NodeId};
use crate::output::render::{display_width, format_relative_age, truncate_to_width};
use crate::viewer::model::{TurnKind, TurnRole};
use crate::viewer::search::{
    ContentKinds, ContentQuery, Grouping, Hit, SessionHits, cmp_hits, cmp_sessions,
};

use super::{ColorFlag, FilterArgs, resolve_color_from_env, warm_start_discover_and_resolve};

/// Gap between table columns.
const GAP: &str = "  ";
/// The snippet column never gets narrower than this when a width is set.
const MIN_SNIPPET_WIDTH: usize = 20;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum GroupFlag {
    /// One result per session, ranked by hit count.
    Session,
    /// One result per matching message.
    Message,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum FormatFlag {
    /// An aligned table for people.
    Table,
    /// A JSON document for scripts.
    Json,
}

/// Search the text of every session's transcript.
///
/// Matching is a case-insensitive substring of the query (ASCII letters
/// fold; other text must match exactly). By default only user and
/// assistant messages are searched.
///
/// Exit status: 0 when at least one session matched, 1 when none did,
/// 2 on an error.
#[derive(Debug, Args)]
pub(super) struct SearchArgs {
    /// Text to find in session transcripts.
    #[arg(value_name = "QUERY")]
    query: String,
    /// One result per session (the default from `[search]` config) or
    /// one per matching message.
    #[arg(long, value_enum)]
    group: Option<GroupFlag>,
    /// Also search tool calls and tool output.
    #[arg(long)]
    tools: bool,
    /// Also search model thinking blocks.
    #[arg(long)]
    thinking: bool,
    /// Narrow to one or more harness keys. Repeatable.
    #[arg(long = "harness", value_name = "HARNESS")]
    harness: Vec<String>,
    /// Skip sessions last active longer ago than this (e.g. `7d`).
    #[arg(long = "max-age", value_name = "DURATION")]
    max_age: Option<String>,
    /// Narrow by derived mux state: `attached`, `ambiguous`, `unmuxed`.
    /// Comma-separated or repeatable.
    #[arg(long = "mux-state", value_name = "STATE", value_delimiter = ',')]
    mux_state: Vec<String>,
    /// Print at most this many results.
    #[arg(long, value_name = "N")]
    limit: Option<usize>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = FormatFlag::Table)]
    format: FormatFlag,
    /// Render the table at exactly this many columns. By default the
    /// terminal width is used on a TTY and output is unbounded when
    /// piped.
    #[arg(long, value_name = "N")]
    width: Option<usize>,
    /// When to highlight the match in table snippets: `auto` (only on a
    /// TTY, honoring `NO_COLOR`), `always`, or `never`.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
    /// Report how many sessions were scanned and skipped on stderr
    /// even when stderr is not a TTY.
    #[arg(long)]
    verbose: bool,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Don't write the rebuilt graph to the `graph.bin` cache.
    #[arg(long = "no-cache")]
    no_cache: bool,
    /// Ignore a running `conspectus serve` daemon and rebuild the
    /// graph in-process from live providers.
    #[arg(long = "refresh")]
    refresh: bool,
}

/// What a finished search found.
struct Found {
    group: Grouping,
    kinds: ContentKinds,
    progress: Progress,
    parserless: usize,
    rows: Vec<Row>,
}

/// One printed result.
enum Row {
    Session {
        info: SessionInfo,
        hits: SessionHits,
    },
    Message {
        info: SessionInfo,
        hit: Hit,
    },
}

#[derive(Clone)]
struct SessionInfo {
    harness: String,
    session: String,
    /// Alias, title, or hosting mux name, when there is one.
    label: Option<String>,
}

impl SearchArgs {
    // The dispatcher expects `Result<()>` from every command; this one
    // reports through its exit status and never returns an error.
    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn run(self) -> Result<()> {
        match self.execute() {
            Ok(true) => Ok(()),
            Ok(false) => {
                let _ = io::stdout().flush();
                std::process::exit(1);
            }
            Err(err) => {
                eprintln!("conspectus: {err:#}");
                std::process::exit(2);
            }
        }
    }

    /// Run the search and print it. `Ok(true)` when anything matched.
    fn execute(&self) -> Result<bool> {
        let query_text = self.query.trim();
        if query_text.is_empty() {
            return Err(anyhow!("the query is empty"));
        }

        let cwd = crate::cwd::current();
        let outcome = config::ConfigLoader::from_env().load(cwd.as_deref());
        for diagnostic in &outcome.diagnostics {
            eprintln!(
                "conspectus: warning: {}: {}",
                diagnostic.path.display(),
                diagnostic.message
            );
        }
        let defaults = &outcome.config.search;
        let group = match self.group {
            Some(GroupFlag::Session) => Grouping::Session,
            Some(GroupFlag::Message) => Grouping::Message,
            None => defaults.group,
        };
        let mut kinds = defaults.kinds;
        kinds.tools |= self.tools;
        kinds.thinking |= self.thinking;
        let query = ContentQuery::new(query_text, kinds);

        let roots: Vec<PathBuf> = if self.scan_roots.is_empty() {
            cwd.into_iter().collect()
        } else {
            self.scan_roots.clone()
        };
        let snapshot =
            warm_start_discover_and_resolve(roots, self.refresh, self.no_cache, &outcome.config)
                .context("could not discover sessions")?;

        let filter = FilterArgs {
            harness: self.harness.clone(),
            max_age: self.max_age.clone(),
            mux_state: self.mux_state.clone(),
            ..FilterArgs::default()
        }
        .to_row_filter()?;
        let allowed = allowed_sessions(&snapshot, &filter);
        let infos = session_infos(&snapshot, allowed.as_ref());
        let plan = plan_sessions(infos.iter().map(|(id, _)| id));
        let found = self.search(&infos, &plan.sessions, &query, group);
        let found = Found {
            parserless: plan.parserless,
            ..found
        };

        if self.verbose || io::stderr().is_terminal() {
            eprintln!(
                "searched {} of {} sessions ({} skipped, {} without a transcript parser)",
                found.progress.scanned,
                found.progress.total,
                found.progress.skipped,
                found.parserless
            );
        }

        let matched = !found.rows.is_empty();
        let mut out = io::stdout().lock();
        let text = match self.format {
            FormatFlag::Json => render_json(&self.query, &found)?,
            FormatFlag::Table => {
                let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
                let width = table_width(self.width, io::stdout().is_terminal());
                render_table(&found, width, color, crate::discovery::current_epoch())
            }
        };
        match out.write_all(text.as_bytes()) {
            Err(err) if err.kind() == io::ErrorKind::BrokenPipe => {}
            other => other.context("could not write the results")?,
        }
        Ok(matched)
    }

    /// Scan `sessions` and turn the summaries into ranked rows.
    fn search(
        &self,
        infos: &[(crate::model::AgentSessionId, SessionInfo)],
        sessions: &[ScanSession],
        query: &ContentQuery,
        group: Grouping,
    ) -> Found {
        let summary = scan_blocking(sessions, query, ScanOptions::default(), &CancelToken::new());
        let info_for: HashMap<SessionKey, &SessionInfo> = infos
            .iter()
            .map(|(id, info)| {
                (
                    SessionKey {
                        harness_key: id.harness_key.clone(),
                        session_key: id.session_key.clone(),
                    },
                    info,
                )
            })
            .collect();
        let by_key: HashMap<&SessionKey, &ScanSession> =
            sessions.iter().map(|s| (&s.key, s)).collect();

        let mut ranked = summary.results;
        // `scan_blocking` orders by key; the stable sort keeps that as
        // the tie-break.
        ranked.sort_by(|a, b| cmp_sessions(&a.1, &b.1));

        let limit = self.limit.unwrap_or(usize::MAX);
        let mut rows = Vec::new();
        for (key, hits) in ranked {
            if rows.len() >= limit {
                break;
            }
            let Some(info) = info_for.get(&key).map(|info| (*info).clone()) else {
                continue;
            };
            match group {
                Grouping::Session => rows.push(Row::Session { info, hits }),
                Grouping::Message => {
                    let Some(session) = by_key.get(&key) else {
                        continue;
                    };
                    let mut all = hits_for(session, query);
                    all.sort_by(cmp_hits);
                    for hit in all {
                        if rows.len() >= limit {
                            break;
                        }
                        rows.push(Row::Message {
                            info: info.clone(),
                            hit,
                        });
                    }
                }
            }
        }
        Found {
            group,
            kinds: query.kinds,
            progress: summary.progress,
            parserless: 0,
            rows,
        }
    }
}

/// The sessions the table projection would show under `filter`, as
/// `harness:key` labels, or `None` when nothing narrows. Going through
/// the table keeps the filter flags identical to `conspectus table
/// sessions`.
fn allowed_sessions(
    snapshot: &GraphSnapshot,
    filter: &crate::filter::RowFilter,
) -> Option<BTreeSet<String>> {
    if !filter.has_narrowing_predicates() {
        return None;
    }
    let options = crate::output::render::RenderOptions::wide()
        .with_filter(filter.clone())
        .with_now_epoch(Some(crate::discovery::current_epoch()));
    let rows = crate::output::agent::build_agent_rows_from_snapshot(snapshot, &["agent"], &options);
    Some(
        rows.into_iter()
            .skip(1)
            .filter_map(|row| row.into_iter().next())
            .collect(),
    )
}

/// Every agent session in the snapshot that passes the filter, with its
/// display label.
fn session_infos(
    snapshot: &GraphSnapshot,
    allowed: Option<&BTreeSet<String>>,
) -> Vec<(crate::model::AgentSessionId, SessionInfo)> {
    let mut out = Vec::new();
    for node in &snapshot.nodes {
        let GraphNode::AgentSession(agent) = node else {
            continue;
        };
        let label = format!(
            "{}:{}",
            agent.harness_key,
            crate::output::table::agent_session_key_for_label(&agent.id.session_key)
        );
        if allowed.is_some_and(|set| !set.contains(&label)) {
            continue;
        }
        let node_id = NodeId::AgentSession(agent.id.clone());
        let mux = snapshot.mux_host_of(&node_id);
        let name = crate::aliases::session_display_name(
            snapshot.aliases.get(&node_id),
            agent.title.as_deref(),
            mux.as_ref().map(|host| host.name.as_str()),
        )
        .map(str::to_string);
        out.push((
            agent.id.clone(),
            SessionInfo {
                harness: agent.harness_key.clone(),
                session: agent.id.session_key.clone(),
                label: name,
            },
        ));
    }
    out
}

/// The table width: the flag, else the terminal on a TTY, else none.
fn table_width(flag: Option<usize>, stdout_is_tty: bool) -> Option<usize> {
    if flag.is_some() {
        return flag;
    }
    if !stdout_is_tty {
        return None;
    }
    terminal_size::terminal_size().map(|(w, _)| usize::from(w.0))
}

// ---------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------

fn kind_names(kinds: &ContentKinds) -> Vec<&'static str> {
    let mut names = Vec::new();
    if kinds.messages {
        names.push("messages");
    }
    if kinds.tools {
        names.push("tools");
    }
    if kinds.thinking {
        names.push("thinking");
    }
    names
}

fn role_name(role: TurnRole) -> &'static str {
    match role {
        TurnRole::User => "user",
        TurnRole::Assistant => "assistant",
        TurnRole::System => "system",
    }
}

fn turn_kind_name(kind: TurnKind) -> &'static str {
    match kind {
        TurnKind::Message => "message",
        TurnKind::CompactionSummary => "compaction_summary",
        TurnKind::ToolUse => "tool_use",
        TurnKind::ToolResult => "tool_result",
        TurnKind::Thinking => "thinking",
    }
}

fn rfc3339(time: Option<DateTime<Utc>>) -> Value {
    time.map_or(Value::Null, |t| {
        Value::String(t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    })
}

fn snippet_json(hit: &Hit) -> Value {
    json!({
        "text": hit.snippet,
        "highlight": [hit.highlight.start, hit.highlight.end],
    })
}

/// The documented JSON document (see the operations guide).
fn render_json(query: &str, found: &Found) -> Result<String> {
    let results: Vec<Value> = found
        .rows
        .iter()
        .map(|row| match row {
            Row::Session { info, hits } => json!({
                "harness": info.harness,
                "session": info.session,
                "label": info.label,
                "hits": hits.count,
                "latest": rfc3339(hits.latest),
                "snippet": snippet_json(&hits.best),
            }),
            Row::Message { info, hit } => json!({
                "harness": info.harness,
                "session": info.session,
                "label": info.label,
                "turn": hit.turn_index,
                "role": role_name(hit.role),
                "kind": turn_kind_name(hit.kind),
                "time": rfc3339(hit.timestamp),
                "snippet": snippet_json(hit),
            }),
        })
        .collect();
    let document = json!({
        "query": query.trim(),
        "group": found.group.as_str(),
        "kinds": kind_names(&found.kinds),
        "searched": {
            "sessions": found.progress.scanned,
            "total": found.progress.total,
            "skipped": found.progress.skipped,
            "without_parser": found.parserless,
        },
        "results": results,
    });
    let mut text = serde_json::to_string_pretty(&document)?;
    text.push('\n');
    Ok(text)
}

/// Header and body cells of the table, with the snippet kept apart so it
/// can be fitted to the remaining width.
struct Table {
    header: Vec<&'static str>,
    cells: Vec<Vec<String>>,
    snippets: Vec<(String, std::ops::Range<usize>)>,
}

fn build_table(found: &Found, now: i64) -> Table {
    let age = |time: Option<DateTime<Utc>>| {
        time.map_or_else(
            || "—".to_string(),
            |t| format_relative_age(t.timestamp(), now),
        )
    };
    let name = |info: &SessionInfo| {
        info.label
            .clone()
            .unwrap_or_else(|| crate::output::table::agent_session_key_for_label(&info.session))
    };
    match found.group {
        Grouping::Session => {
            let mut cells = Vec::new();
            let mut snippets = Vec::new();
            for row in &found.rows {
                let Row::Session { info, hits } = row else {
                    continue;
                };
                cells.push(vec![
                    name(info),
                    info.harness.clone(),
                    hits.count.to_string(),
                    age(hits.latest),
                ]);
                snippets.push((hits.best.snippet.clone(), hits.best.highlight.clone()));
            }
            Table {
                header: vec!["SESSION", "HARNESS", "HITS", "LATEST", "SNIPPET"],
                cells,
                snippets,
            }
        }
        Grouping::Message => {
            let mut cells = Vec::new();
            let mut snippets = Vec::new();
            for row in &found.rows {
                let Row::Message { info, hit } = row else {
                    continue;
                };
                cells.push(vec![
                    name(info),
                    info.harness.clone(),
                    format!("#{}", hit.turn_index),
                    role_name(hit.role).to_string(),
                    turn_kind_name(hit.kind).replace('_', " "),
                    age(hit.timestamp),
                ]);
                snippets.push((hit.snippet.clone(), hit.highlight.clone()));
            }
            Table {
                header: vec![
                    "SESSION", "HARNESS", "TURN", "ROLE", "KIND", "TIME", "SNIPPET",
                ],
                cells,
                snippets,
            }
        }
    }
}

/// An aligned table; `width` fits the snippet column, and `color` bolds
/// the match inside it.
fn render_table(found: &Found, width: Option<usize>, color: bool, now: i64) -> String {
    if found.rows.is_empty() {
        return String::new();
    }
    let table = build_table(found, now);
    let columns = table.header.len() - 1;
    let mut widths: Vec<usize> = table.header[..columns].iter().map(|h| h.len()).collect();
    for row in &table.cells {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(display_width(cell));
        }
    }
    let fixed: usize = widths.iter().sum::<usize>() + GAP.len() * columns;
    let snippet_budget = width.map(|w| w.saturating_sub(fixed).max(MIN_SNIPPET_WIDTH));

    let mut out = String::new();
    for (idx, head) in table.header[..columns].iter().enumerate() {
        out.push_str(&pad(head, widths[idx]));
        out.push_str(GAP);
    }
    out.push_str(table.header[columns]);
    out.push('\n');
    for (row, (snippet, range)) in table.cells.iter().zip(&table.snippets) {
        for (idx, cell) in row.iter().enumerate() {
            out.push_str(&pad(cell, widths[idx]));
            out.push_str(GAP);
        }
        out.push_str(&fit_snippet(snippet, range, snippet_budget, color));
        out.push('\n');
    }
    out
}

fn pad(text: &str, width: usize) -> String {
    let gap = width.saturating_sub(display_width(text));
    format!("{text}{}", " ".repeat(gap))
}

/// The snippet cut to `budget` columns (when set), with the match in
/// bold when `color` is on and the match survived the cut.
fn fit_snippet(
    snippet: &str,
    highlight: &std::ops::Range<usize>,
    budget: Option<usize>,
    color: bool,
) -> String {
    let fitted = budget.map_or_else(|| snippet.to_string(), |b| truncate_to_width(snippet, b));
    if !color || highlight.is_empty() {
        return fitted;
    }
    let kept = fitted.strip_suffix('…').unwrap_or(&fitted);
    let intact = kept.len() >= highlight.end
        && kept.is_char_boundary(highlight.start)
        && kept.is_char_boundary(highlight.end)
        && snippet.starts_with(kept);
    if !intact {
        return fitted;
    }
    let bold = Style::new().effects(Effects::BOLD);
    format!(
        "{}{bold}{}{Reset}{}{}",
        &kept[..highlight.start],
        &kept[highlight.start..highlight.end],
        &kept[highlight.end..],
        &fitted[kept.len()..],
    )
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
