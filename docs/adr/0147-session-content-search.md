# ADR 0147: Session Content Search

## Status

Accepted. Amends ADR 0031 (a content query is a text predicate, held
app-wide and never persisted) and ADR 0086 (Tier 2 gains this reader).
Builds on ADR 0112 (in-memory transcript previews, `--no-live-preview`).
The persistent index it defers is decided in ADR 0148, which is built only
when a threshold below is crossed or the operator asks.

## Context

`/` finds sessions and muxes by metadata: alias, title, working
directory, and the last-message preview. There is no way to find a
session by what was said in it. Conspectus already parses Claude Code,
Codex, opencode (including 2.x), and Pi transcripts for the native
viewer, reached through `HarnessAdapter::transcript_source` and
`transcript_parser`, so the reading side exists.

The operator decided on 2026-10-16 that the first version scans
transcripts on demand with no index, offers three surfaces (a contents
scope in `/`, find in the viewer, a `conspectus search` command), groups
results per session or per message, and searches user and assistant
messages by default with tool and thinking content opt-in. On
2026-10-23 they added that search must work both as a dedicated view
and as a filter on the session list.

Two existing rules need a deliberate exception. ADR 0031 excludes
free-text filtering ("structured filters narrow by structure, `/` ranks
within the filtered set"), keeps filters per view, and persists them
with view state. ADR 0086 places transcript payload in Tier 2 only under
an operator gesture, with a cap, and an ADR entry per reader.

Measured on 2026-10-16: about 320 MB of transcripts, a 50 ms raw byte
scan, and about 0.9 s to parse everything single-threaded. That is fast
enough to scan on demand.

## Decision

### Placement and lifetime (ADR 0086)

1. **A Tier 2 reader.** The query is the operator gesture. Hits and
   snippets exist in memory for the life of the TUI process (or one CLI
   invocation) and are never written to disk, never stored in the graph
   or `graph.bin`, never sent over the daemon socket, never part of
   saved TUI state. ADR 0112 already accepts an in-memory transcript
   cache on the same footing.
2. **Capped and normalized.** A snippet is a window around the match,
   length-capped, with whitespace collapsed and control sequences
   removed. Each session keeps its hit count, latest hit time, and one
   best snippet; the full hit list is materialized only for the Search
   view and for a viewer that opens at a hit.
3. **Where it runs.** In the TUI or CLI process. The session list comes
   from the snapshot; transcripts are read directly from harness state.
   The daemon never reads transcripts for search.
4. **`--no-live-preview`.** The contents scope, the Search view, and
   `--contains` are unavailable under that flag, with a message saying
   why, because they show conversation text. The `conspectus search`
   command is unaffected: typing it is the gesture.

### What is searched

5. **Corpus.** Agent-session nodes in the snapshot whose adapter
   provides a transcript parser: Claude Code, Codex, opencode (including
   2.x), and Pi. A session without a parser is skipped and counted.
   There is no new file discovery, so Claude Code subagent files that
   are not graph nodes are not searched, and opencode subagent sessions,
   which are nodes, are.
6. **Content kinds.** User and assistant messages and compaction
   summaries by default. Aborted turns and turns the viewer hides by
   default are excluded, so every default hit is visible when the
   transcript opens. Tool calls and output, and thinking, are opt-in.
   The `[search]` config section sets the default kinds and grouping;
   invalid values produce config diagnostics.
7. **Query semantics.** One case-insensitive substring of the trimmed
   query, matching `/`. Case folding is ASCII only: non-ASCII text
   matches exactly. Unicode folding would change byte lengths and break
   highlight ranges. Multiple terms, smart-case, and quoted phrases are
   deferred.
8. **The matcher is shared.** One pure matcher in a module the viewer
   may import (the viewer may not depend on `crate::tui`, `model`, or
   `discovery`), used by the `/` overlay, the scanner, the viewer's find,
   and the CLI, so every surface agrees on what is a hit. `/` delegates
   to it and its behavior is unchanged.
9. **Ranking.** Per-session results rank by hit count, then latest hit.
   The global Hierarchy sort means relevance in the Search view and
   Recency means latest hit; no new sort variant is added.

### How the scan runs

10. **Threads and cancellation.** `std::thread` with a channel, no new
    dependency. Every scan carries a generation; a new query or a
    dropped handle cancels the running scan, the scanner checks the token
    per file and per batch, and results from an older generation are
    discarded.
11. **Streaming.** Results stream while the scan runs. The TUI coalesces
    them to at most one tree rebuild per 150 ms, keeps the selection by
    row, and shows progress (sessions scanned, total, skipped). Static
    snapshot mode has a synchronous path so frames are deterministic.
12. **A locator seam.** The scanner must not re-walk disk for every
    session. A harness parser can map a session to its transcript source
    (a file or a database row) once, so a scan is linear in the corpus.
13. **The byte prefilter.** A file is skipped without parsing only when
    it cannot match. The prefilter takes the longest whitespace-free run
    of `[A-Za-z0-9_.-]` in the query and requires every such run to
    appear in the bytes: characters that JSON may escape on disk
    (quotes, backslashes, control characters, `/`, non-ASCII) break a
    run, and whitespace never anchors a run because a newline can sit
    where the query has a space. With no safe run, the file is fully
    parsed. opencode keeps its sessions in SQLite, so its candidates
    come from a query using `instr(lower(...), ?)` (SQLite `lower` is
    ASCII only, which agrees with the matcher), read through a
    read-only, `query_only` connection.
14. **Unreadable input.** A malformed, unreadable, or parserless
    transcript is skipped and counted; it never fails a search.

### The projections

15. **Three projections of one query.** The query is held in app state:
    - **`/` overlay.** `Tab` switches scope between rows and contents.
      Rows scope is unchanged. In contents scope the overlay shows a
      bounded live preview with progress; Enter opens the viewer at the
      hit, one key applies the query as the filter, another opens the
      Search view, and Esc cancels the scan.
    - **Search view.** A sixth view, key `6`, listing matching sessions
      ranked, with a hit chip and a highlighted snippet, and hit rows
      when grouped per message. Enter, `v`, and the `.` menu act on the
      session like in every other view. It is not restored at startup.
    - **Content filter.** The query narrows Sessions, Mux, and PRs to
      sessions whose transcripts match, through one content predicate on
      the shared row filter that the tree builder injects from the app
      state. It composes with the other filters, grouping, sort, and pin
      markers; empty groups disappear. Rows show a hit count and the best
      snippet; the header shows the query with progress. Pinned and
      History do not apply it in the first version and say so.
16. **One deliberate departure from ADR 0031.** Filters there are
    per view, persisted with view state, and structural. The content
    query is app-wide (one query, not one per view), never persisted
    (it derives from payload), and a text predicate. It lives in app
    state, not in the per-view filter slot, so Controls cannot overwrite
    it and view state save and restore cannot write it. `F` and Controls
    clear it with the other filters.
17. **Rescan policy.** A discovery refresh keeps the filter valid
    because hits are keyed by harness and session key, and scans only
    sessions not yet scanned; an explicit refresh rescans everything.
    The Search view shows how old the scan is.

### The command line

18. **`conspectus search <query>`.** Per-session results by default,
    per-message with `--group message`; flags opt into tool and thinking
    content and narrow by harness; defaults come from `[search]`. It
    defines `--format table|json` itself (`table` has no format flag),
    with a documented JSON shape. Exit status follows grep: 0 for a
    match, 1 for none, 2 for an error, so the command traps its own
    errors. `table` and `tui` gain `--contains TEXT`, the same content
    predicate, so the filter has CLI parity as ADR 0031 requires.

### Revisiting a persistent index

19. **Not now.** A persistent full-text index would store payload on
    disk, which ADR 0086 and ADR 0087 forbid. ADR 0148 decides the
    index, amending both, and builds it only when the on-demand scan
    misses its latency budget or the corpus passes a size threshold: results start within
    300 ms and finish within 2 s on the operator's corpus, and the index
    work starts when the corpus exceeds 2 GB of transcripts or the 95th
    percentile finish time exceeds 5 s. The result source stays behind
    one seam so an index can replace the scan without changing the
    surfaces.

## Consequences

- Session contents become findable from the TUI and the shell without
  persisting anything.
- ADR 0031's "no free-text filter" and "filters are per view and
  persisted" are amended for one predicate; the content filter is the
  only filter held app-wide and not saved.
- The harness parsers gain a locator seam, and the viewer gains a find
  prompt and a way to open at a turn.
- A sixth view changes the tab strip in every frame, snapshot, and
  screenshot.
- Memory use is bounded by per-session caps; a very large corpus makes
  the scan slower rather than the process larger.

## Alternatives Considered

- **A persistent index now.** Faster at scale but needs an ADR 0086 and
  0087 amendment, a freshness story, and consent; the measured corpus
  does not need it.
- **Search only in the `/` overlay.** No place to review results and
  no filter on the list; the operator asked for both.
- **A per-view content filter in the existing filter slot.** Fits ADR
  0031 but would save payload-derived state and be overwritten by
  Controls.
- **Search in the daemon.** Would send transcript text over the socket
  and write it to the snapshot, against ADR 0086.
- **Unicode case folding.** Better recall for non-ASCII text, but byte
  offsets for highlights and snippets stop matching the source.
- **Reusing the `/` matcher from `src/tui/`.** The viewer cannot depend
  on it, so the matcher moves to a neutral module.

## Open Questions Answered

- **Do subagent transcripts count?** Only those that are graph nodes
  (opencode subagents). Claude Code subagent files are not nodes.
- **Active filters or every session?** The content filter composes with
  the structural filters; the Search view lists every matching session
  and the `conspectus search` command spans all sessions unless the
  filter flags narrow it.
- **Smart-case, multiple terms, quotes?** Deferred.
- **Aborted turns?** Excluded by default.
- **Does a refresh invalidate hits?** No; new sessions are scanned, and
  an explicit refresh rescans.
- **Which ADR number if another ADR lands first?** The number is taken
  at merge; the later branch renumbers.

## Measured

The scanner (`src/content_search`) was timed on a synthetic corpus of 200
sessions and 315 MB (two thirds Claude Code, one third Codex, one session
in 40 at 24 MB, tool results mixed in) on an 8-CPU Linux machine, with the
files on tmpfs, so the page cache was warm and a cold disk would be slower.
Eight workers, release build, whole scan to completion:

| Query | Prefilter | Time | Sessions matched |
|---|---|---|---|
| a phrase in 2 sessions | on | 130 ms | 2 |
| the same phrase | off (every file parsed) | 509 ms | 2 |
| a phrase in none | on | 139 ms | 0 |
| two common words | on (every file is a candidate) | 416 ms | 200 |

The prefilter skips about three quarters of the work for a selective query
and costs nothing when every file is a candidate. Both ends are well inside
the latency budget (results start within 300 ms and finish within 2 s), so
a persistent index is not yet justified. Rerun with
`CONSPECTUS_SCAN_BENCH_MB=300 cargo test --release scan_benchmark -- --ignored --nocapture`.
