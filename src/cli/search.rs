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
    CancelToken, Progress, ScanOptions, ScanSession, SessionKey, plan_sessions, scan_blocking,
    session_detail,
};
use crate::model::{GraphNode, GraphSnapshot, NodeId};
use crate::output::render::{display_width, format_relative_age, truncate_to_width};
use crate::viewer::model::{TurnKind, TurnRole};
use crate::viewer::search::{
    ContentKinds, ContentQuery, Grouping, Hit, SessionHits, cmp_hits_across_sessions, cmp_sessions,
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
    /// One result per matching message, in session order.
    Message,
    /// Every matching message across all sessions as one list, newest
    /// first.
    Flat,
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
    /// One result per session (the default from `[search]` config), one
    /// per matching message in session order, or every matching message
    /// as one list across sessions, newest first (`flat`).
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
    #[arg(long, value_name = "N", value_parser = parse_limit)]
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
    /// Sessions with more matching messages than were fetched.
    truncated: Vec<Truncated>,
    /// The flat list cut to `--limit`: how many rows are shown of how
    /// many matched.
    limited: Option<Limited>,
}

/// A flat list that `--limit` cut short.
struct Limited {
    shown: usize,
    total: usize,
}

/// A session whose per-message results were capped.
struct Truncated {
    info: SessionInfo,
    matches: usize,
    shown: usize,
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
            Some(GroupFlag::Flat) => Grouping::Flat,
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
                "searched {} of {} sessions; skipped {} whose transcript could not be read; \
                 {} more belong to a harness with no transcript parser and were not searched",
                found.progress.scanned,
                found.progress.total,
                found.progress.skipped,
                found.parserless
            );
        }

        let matched = !found.rows.is_empty();
        for capped in &found.truncated {
            eprintln!(
                "conspectus: note: {} has {} matching messages; showing the newest {}",
                capped.info.label.as_deref().unwrap_or(&capped.info.session),
                capped.matches,
                capped.shown
            );
        }
        if let Some(limit) = &found.limited {
            eprintln!(
                "conspectus: note: showing the newest {} of {} matching messages",
                limit.shown, limit.total
            );
        }
        if !matched && matches!(self.format, FormatFlag::Table) {
            eprintln!("No sessions match \"{}\"", self.query.trim());
        }
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
        let mut truncated = Vec::new();
        let mut limited = None;
        if group == Grouping::Flat {
            // Every matching message of every matching session, then one
            // order across all of them.
            let mut all: Vec<(SessionKey, SessionInfo, Hit)> = Vec::new();
            for (key, _) in ranked {
                let (Some(info), Some(session)) = (info_for.get(&key), by_key.get(&key)) else {
                    continue;
                };
                let detail = session_detail(session, query);
                if detail.truncated() {
                    truncated.push(Truncated {
                        info: (*info).clone(),
                        matches: detail.total,
                        shown: detail.hits.len(),
                    });
                }
                all.extend(
                    detail
                        .hits
                        .into_iter()
                        .map(|hit| (key.clone(), (*info).clone(), hit)),
                );
            }
            all.sort_by(|a, b| cmp_hits_across_sessions((&a.0, &a.2), (&b.0, &b.2)));
            let total = all.len();
            if total > limit {
                limited = Some(Limited {
                    shown: limit,
                    total,
                });
            }
            rows.extend(
                all.into_iter()
                    .take(limit)
                    .map(|(_, info, hit)| Row::Message { info, hit }),
            );
            return Found {
                group,
                kinds: query.kinds,
                progress: summary.progress,
                parserless: 0,
                rows,
                truncated,
                limited,
            };
        }
        for (key, hits) in ranked {
            if rows.len() >= limit {
                break;
            }
            let Some(info) = info_for.get(&key).map(|info| (*info).clone()) else {
                continue;
            };
            match group {
                Grouping::Session => rows.push(Row::Session { info, hits }),
                Grouping::Flat => {}
                Grouping::Message => {
                    let Some(session) = by_key.get(&key) else {
                        continue;
                    };
                    let detail = session_detail(session, query);
                    if detail.truncated() {
                        truncated.push(Truncated {
                            info: info.clone(),
                            matches: detail.total,
                            shown: detail.hits.len(),
                        });
                    }
                    // Turn order, as the viewer and the Search view list
                    // a session's messages.
                    for hit in detail.hits {
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
            truncated,
            limited,
        }
    }
}

/// The `(harness, session)` pairs that pass `filter`, or `None` when
/// nothing narrows. The sessions table applies the same predicate, so the
/// filter flags mean the same here as in `conspectus table sessions`.
fn allowed_sessions(
    snapshot: &GraphSnapshot,
    filter: &crate::filter::RowFilter,
) -> Option<BTreeSet<(String, String)>> {
    if !filter.has_narrowing_predicates() {
        return None;
    }
    let options = crate::output::render::RenderOptions::wide()
        .with_filter(filter.clone())
        .with_now_epoch(Some(crate::discovery::current_epoch()));
    Some(crate::output::agent::sessions_passing_filter(
        snapshot, &options,
    ))
}

/// Every agent session in the snapshot that passes the filter, with its
/// display label.
fn session_infos(
    snapshot: &GraphSnapshot,
    allowed: Option<&BTreeSet<(String, String)>>,
) -> Vec<(crate::model::AgentSessionId, SessionInfo)> {
    let mut out = Vec::new();
    for node in &snapshot.nodes {
        let GraphNode::AgentSession(agent) = node else {
            continue;
        };
        let passes = |set: &BTreeSet<(String, String)>| {
            set.contains(&(agent.harness_key.clone(), agent.id.session_key.clone()))
        };
        if allowed.is_some_and(|set| !passes(set)) {
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

/// `--limit` must be a positive count: zero would print nothing and exit
/// as if the query had no matches.
fn parse_limit(raw: &str) -> Result<usize, String> {
    match raw.parse::<usize>() {
        Ok(0) => Err("must be at least 1".to_string()),
        Ok(n) => Ok(n),
        Err(err) => Err(err.to_string()),
    }
}

/// The words the TUI uses for a role and a kind of turn.
fn role_word(role: TurnRole) -> &'static str {
    match role {
        TurnRole::User => "you",
        TurnRole::Assistant => "ai",
        TurnRole::System => "system",
    }
}

fn kind_word(kind: TurnKind) -> &'static str {
    match kind {
        TurnKind::Message => "message",
        TurnKind::CompactionSummary => "compaction summary",
        TurnKind::ToolUse => "tool call",
        TurnKind::ToolResult => "tool output",
        TurnKind::Thinking => "thinking",
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
                "turn": hit.turn_index + 1,
                "role": role_name(hit.role),
                "kind": turn_kind_name(hit.kind),
                "time": rfc3339(hit.timestamp),
                "snippet": snippet_json(hit),
            }),
        })
        .collect();
    let mut document = json!({
        "query": query.trim(),
        "group": found.group.as_str(),
        "kinds": kind_names(&found.kinds),
        "searched": {
            "sessions": found.progress.scanned,
            "total": found.progress.total,
            "skipped": found.progress.skipped,
            "without_parser": found.parserless,
        },
        "truncated": found
            .truncated
            .iter()
            .map(|capped| json!({
                "harness": capped.info.harness,
                "session": capped.info.session,
                "matches": capped.matches,
                "shown": capped.shown,
            }))
            .collect::<Vec<_>>(),
        "results": results,
    });
    if let (Some(limit), Some(object)) = (&found.limited, document.as_object_mut()) {
        object.insert(
            "limited".into(),
            json!({"shown": limit.shown, "total": limit.total}),
        );
    }
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
    // The session's name, or the first eight characters of its key, as
    // the TUI shows an unnamed session.
    let name = |info: &SessionInfo| {
        info.label
            .clone()
            .unwrap_or_else(|| info.session.chars().take(8).collect())
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
        Grouping::Message | Grouping::Flat => {
            let mut cells = Vec::new();
            let mut snippets = Vec::new();
            for row in &found.rows {
                let Row::Message { info, hit } = row else {
                    continue;
                };
                cells.push(vec![
                    name(info),
                    info.harness.clone(),
                    format!("#{}", hit.turn_index + 1),
                    role_word(hit.role).to_string(),
                    kind_word(hit.kind).to_string(),
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

/// The snippet cut to `budget` columns (when set) around the match, on
/// word boundaries where it can, with the match in bold when `color` is
/// on.
fn fit_snippet(
    snippet: &str,
    highlight: &std::ops::Range<usize>,
    budget: Option<usize>,
    color: bool,
) -> String {
    let (text, range) = match budget {
        Some(budget) => crop_around(snippet, highlight, budget),
        None => (snippet.to_string(), highlight.clone()),
    };
    if !color || range.is_empty() || range.end > text.len() {
        return text;
    }
    let bold = Style::new().effects(Effects::BOLD);
    format!(
        "{}{bold}{}{Reset}{}",
        &text[..range.start],
        &text[range.clone()],
        &text[range.end..],
    )
}

/// `snippet` cropped to at most `budget` display columns, keeping
/// `highlight` (a byte range) visible, with `…` on a cut side. Context is
/// spread to both sides of the match and each cut moves to the nearest
/// space that doesn't lose the match. Returns the text and where the
/// highlight is in it. A match wider than the budget is itself cut.
fn crop_around(
    snippet: &str,
    highlight: &std::ops::Range<usize>,
    budget: usize,
) -> (String, std::ops::Range<usize>) {
    if display_width(snippet) <= budget {
        return (snippet.to_string(), highlight.clone());
    }
    let chars: Vec<(usize, char)> = snippet.char_indices().collect();
    let width = |c: char| unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
    let valid = !highlight.is_empty()
        && highlight.end <= snippet.len()
        && snippet.is_char_boundary(highlight.start)
        && snippet.is_char_boundary(highlight.end);
    if !valid {
        return (truncate_to_width(snippet, budget), 0..0);
    }
    let first = chars.partition_point(|&(at, _)| at < highlight.start);
    let last = chars.partition_point(|&(at, _)| at < highlight.end);
    let marker = 1;
    let mut left = first;
    let mut right = last;
    let mut used: usize = chars[first..last].iter().map(|&(_, c)| width(c)).sum();
    // The cells a candidate window costs: its text plus a marker on each
    // side that is cut.
    let cost = |used: usize, left: usize, right: usize| {
        used + marker * (usize::from(left > 0) + usize::from(right < chars.len()))
    };
    if cost(used, left, right) > budget {
        // The match alone fills the line: cut it to fit.
        let room = budget.saturating_sub(marker * (usize::from(first > 0) + 1));
        let mut cut = first;
        let mut cells = 0;
        while cut < last && cells + width(chars[cut].1) <= room {
            cells += width(chars[cut].1);
            cut += 1;
        }
        let text: String = chars[first..cut].iter().map(|&(_, c)| c).collect();
        let head = if first > 0 { "…" } else { "" };
        let tail = if cut < chars.len() { "…" } else { "" };
        let end = head.len() + text.len();
        return (format!("{head}{text}{tail}"), head.len()..end);
    }
    // Grow outward one character at a time, alternating sides.
    let mut grow_left = true;
    loop {
        let can_left = left > 0 && cost(used + width(chars[left - 1].1), left - 1, right) <= budget;
        let can_right =
            right < chars.len() && cost(used + width(chars[right].1), left, right + 1) <= budget;
        if !can_left && !can_right {
            break;
        }
        if (grow_left && can_left) || !can_right {
            left -= 1;
            used += width(chars[left].1);
        } else {
            used += width(chars[right].1);
            right += 1;
        }
        grow_left = !grow_left;
    }
    // Move each cut to a word edge when that keeps the match.
    if left > 0 {
        let mut edge = left;
        while edge < first && chars[edge - 1].1 != ' ' {
            edge += 1;
        }
        if edge < first || chars[edge - 1].1 == ' ' {
            left = edge;
        }
    }
    if right < chars.len() {
        let mut edge = right;
        while edge > last && chars[edge].1 != ' ' {
            edge -= 1;
        }
        if edge > last || chars[edge].1 == ' ' {
            right = edge;
        }
    }
    let head = if left > 0 { "…" } else { "" };
    let tail = if right < chars.len() { "…" } else { "" };
    let body: String = chars[left..right].iter().map(|&(_, c)| c).collect();
    let start = head.len() + chars[first].0 - chars[left].0;
    let end = start + (highlight.end - highlight.start);
    (format!("{head}{body}{tail}"), start..end)
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
