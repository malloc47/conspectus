# ADR 0112: Node-Specific Previews

## Status

Accepted. Amended by ADR 0144: a `Pin` slot has no binding, so its
preview is its target's preview (see that ADR); the Bound/Unbound row
below is historical. Amends ADR 0107 (a Related row's evidence becomes a header
above the neighbor's own preview). Supersedes the Preview rules in
`docs/tui-sessions-mockup.md` and the recent-turns transcript plan
(`CSP-207`–`CSP-210`, `CSP-213`–`CSP-215`), which predates the native
session viewer (ADR 0052).

## Context

The right pane's Preview zone has two modes. With the explorer cursor on
a Related row it shows the neighbor's core fields, edge, and evidence
(ADR 0107). Everywhere else it falls back to logic built around tmux:

- a session attached to tmux, a mux row, or a mux candidate shows the
  pane capture (ADR 0025, ADR 0106), cached per mux session;
- a session without tmux shows its one-line `last_message_preview`
  (ADR 0023);
- a pin shows its live pane or its diagnostics;
- every other row says "no preview for this row", and node field rows
  have no preview of their own.

For a session, what identifies it is the conversation, not the terminal
around it. The older plan for a transcript preview added a separate
widget fed by per-harness recent-turns extractors, for sessions without
tmux only. Since then the native viewer (ADR 0052) parses and renders
Claude Code, Codex, and opencode transcripts, the same harnesses those
extractors would have covered.

## Decision

### 1. Every node kind defines its preview

| Node kind | Preview |
| --- | --- |
| `AgentSession` | Its transcript, newest turns in view, rendered by the native viewer inline |
| `MuxSession` | The pane capture, as before |
| `Pin` | Bound: its session's transcript. Unbound with a live mux: the pane. Otherwise: its diagnostics |
| `Checkout` | `git status --short` above a listing of its root |
| `Repo` | Recent commits on the default branch, plus its remotes |
| `Workspace` | A listing of its root |
| `Branch` | Recent commits at the ref, plus ahead/behind its upstream |
| `ForgePr` | Title, body, checks, and review state, fetched with `gh` (`CSP-171.01`) |
| `RuntimeProcess` | Its process tree from `/proc`, with commands and working directories |
| `Fork` | No preview beyond its fields |

### 2. Every field row previews its value, by value type

A field that names the focused node shows the node's preview; a field
that names another node shows that node's preview. Every other field
previews by the type of its value, so a new field inherits a preview:

| Value type | Preview | Fields |
| --- | --- | --- |
| Directory path | A listing; "no longer exists" when missing (ADR 0111) | `cwd`, `root`, `common_dir`, `git_dir`, `state_scope`, `active_pane_current_path`, `source_paths` |
| File path | Its contents, size-capped | pin `store` |
| This node | The node's preview | `id`, `session_key`, and the PR's `pr`, `url`, `number`, `host`, `owner` |
| Another node | That node's preview | `repo`, `current_branch`, pin `mux` |
| Git ref | Recent commits at the ref; `upstream` adds ahead/behind | `refname`, `upstream` |
| Commit | `git show --stat` | `current_commit` |
| Process ID | The process tree rooted there | `pid`, `parent_pid`, `root_pane_pid`, `active_pane_pid` |
| Command line | The full command, one argument per line | `command`, `active_pane_command`, `active_pane_start_command`, `launch` |
| Timestamp | Absolute time, local and UTC, with its age | `last_active`, `created`, `updated`, `observed` |
| Free text | The full value, untruncated | `title`, `alias`, `name`, `reason`, `last_message_preview` |
| Value from a fixed set | What the value means, from a short glossary | `harness`, `backend`, `provider`, `status`, `state`, `draft`, `attached`, `scope`, `capabilities`, `source`, `depth` |
| List | The full list | `remotes` |

### 3. Related rows show evidence, then the neighbor's preview

ADR 0107's `edge` and `evidence` lines stay, as a compact header. The
neighbor's own preview renders below them, so following a link shows
what is on the other end without drilling in.

### 4. A session in tmux shows its transcript, with the pane one choice away

The transcript is the default for every session. A "Session preview"
section in the controls overlay switches sessions in tmux to the live
pane, which shows prompts and running tools the transcript may lag. It
is one global choice: `[tui] session_preview = "transcript" | "pane"`
sets the starting value and the last pick persists in `tui-state.json`,
as `preview_wrap` does (ADR 0106). No single-key binding is added.

### 5. One preview mechanism

A preview is a source resolved from a node or from a node and one of its
fields. Sources that touch the system load off the UI thread when the
selection lands on them, and their results sit in an in-memory cache
keyed by source, refreshed on selection change and `r`. The pane capture
becomes one source among the others rather than the cache's only kind.
Each source reports loading, content, unavailable (with a reason), or
failed, and the failure banner (ADR 0105) still leads the zone.

### 6. Boundaries

- **Read-only** (ADR 0087): directory reads, size-capped file reads,
  `git -C <path>` read commands, `/proc` reads, and `gh` reads. No
  preview writes anything.
- **Memory only** (ADR 0086): the cache is never written to disk.
- **`--no-live-preview`** keeps hiding conversation and terminal
  content: transcripts and pane captures. Listings, git output, process
  trees, and the other sources still show.
- **Transcripts through the viewer** (ADR 0052): an inline,
  bottom-anchored, scroll-only mode of the native viewer. A harness the
  viewer cannot parse (aider) shows "transcript unavailable".

### 7. Superseded and retargeted work

- `CSP-207`–`CSP-210` and `CSP-213`–`CSP-215` (recent-turns API,
  extractors, widget, wiring, docs) are archived.
- `CSP-171.03` (the v1 marker for a transcript preview) is satisfied by
  `CSP-585.02` and now depends on it.
- `CSP-211` becomes an aider transcript parser for the viewer, still
  deferred.
- `CSP-246` (AI naming context) takes transcripts from the viewer's
  parser instead of the archived recent-turns API.
- `CSP-171.01` (async `gh` enrichment) is the source for the `ForgePr`
  preview.

## Consequences

- Selecting anything shows something useful about it, and the Preview
  zone is no longer a tmux view with exceptions.
- Moving the explorer cursor down a node's fields becomes a way to look
  at each value's context: the directory, the commits, the process.
- Transcript rendering has one implementation, shared by the full-screen
  viewer and the preview.
- More previews do I/O. Loading off the UI thread and caching per source
  keep the TUI responsive, at the cost of a "loading" state on first
  selection.
- The glossary for fixed-set values is new text to maintain alongside
  the code that produces those values.
- Each node kind and value type becomes an implementation task under
  `CSP-585`; the session transcript (`CSP-585.02`) comes first.

## Alternatives Considered

- **Keep tmux-first, add transcripts for sessions without tmux.** The
  earlier plan. Leaves sessions in tmux showing the terminal rather than
  the conversation, and every other kind with nothing.
- **A separate transcript widget.** Duplicates the native viewer's
  parsing and rendering for a smaller view of the same content.
- **Field previews by field name.** Each new field would need its own
  rule; value types cover new fields for free.
- **Only path fields preview their value.** Smaller, but leaves refs,
  process IDs, and commands without the context the operator asked for.
- **Evidence only on Related rows.** Keeps ADR 0107 as is, but the
  operator has to drill in to see what a link points at.
- **A per-session pane toggle.** More precise, but harder to predict;
  one remembered choice matches how `preview_wrap` behaves.
