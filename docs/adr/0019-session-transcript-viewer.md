# ADR 0019: Session Transcript Viewer Integration

## Status

Proposed. Unresolved.

## Context

Conspectus discovers sessions from multiple agent harnesses and records
lineage between sessions, mux sessions, checkouts, forks, and repositories.
The current table and JSON views are useful for finding sessions, but they do
not answer a separate workflow: open a readable, full transcript for an
`AgentSession` without dropping into provider-specific JSON, SQLite, or
history file formats.

The desired viewer should:

- be usable from a CLI or terminal UI, with or without a pager
- support more than one harness, or fit behind an interface where multiple
  provider-specific viewers can coexist
- pretty-print conversation content in a form similar to an agent CLI, not
  just dump JSON
- avoid heavy runtime dependencies such as GUI-only apps, local vector
  databases, or service processes
- ideally follow session lineage, forks, and subagent links
- ideally support interactive expand/collapse for tool calls, thinking, and
  long messages

Claude Code compaction makes "full transcript" a concrete requirement rather
than a display preference. Local session inspection showed that compaction is
in-place at the JSONL file level: earlier rows remain in the same file, a
`system` row with `subtype = "compact_boundary"` marks the boundary, and the
generated summary appears as a `type = "user"` row with
`isCompactSummary = true` and `isVisibleInTranscriptOnly = true`. The
`compact_boundary` row uses `parentUuid = null` and carries the previous
message in `logicalParentUuid`. A viewer that only follows the normal
`parentUuid` chain may accidentally show only the post-compaction tree, while
a full forensic transcript viewer should be able to render the complete file
linearly and distinguish compaction summaries from ordinary user messages.

## Candidate Tools

The current survey did not identify one tool that satisfies every requirement.
Several projects are still useful candidates or references:

- `ccview` (`https://github.com/shivamstaq/ccview`): Go terminal explorer and
  renderer for Claude Code and OpenCode histories. It supports a split-pane
  TUI, terminal markdown rendering, conversation search, subagent inspection,
  and export. It is close to the desired shape and has a small integration
  footprint, but it is young and does not currently cover Codex or aider.
- `recall` (`https://github.com/zippoxer/recall`): Rust TUI for search and
  resume across Claude, Codex, OpenCode, and Factory/Droid sessions. It has
  message previews, scrolling, and per-message expansion, making it a good
  reference for multi-harness search and interactive expansion. It is more
  search/resume oriented than a full transcript renderer.
- `claude-history` (`https://github.com/raine/claude-history`): Mature Rust
  companion CLI for Claude Code. It has a strong terminal transcript viewer
  with scrolling, search, message navigation, markdown rendering, export,
  resume, and fork support. It is Claude-only, but it is the best reference
  found for the desired transcript-viewing experience.
- `ai-dash` (`https://github.com/adinhodovic/ai-dash`): Go TUI for browsing
  Claude Code, Codex, and OpenCode sessions. It has a shared session model,
  parent/child metadata, subagent support, and resume/start operations. It is
  useful as a discovery and dashboard reference, but it does not currently
  appear to be a full transcript renderer.
- `lazyagent` (`https://github.com/illegalstudio/lazyagent`): Go TUI/API for
  observing many agent tools, including Claude Code, Cursor, Codex, Amp, pi,
  and OpenCode. It has broad multi-agent coverage and transcript search, but
  it is a broader monitoring and maintenance dashboard rather than a focused
  "view this transcript" component.
- `ccboard` (`https://github.com/FlorianBruniaux/ccboard`): Rust dashboard for
  Claude Code monitoring, cost/config/hooks/MCP views, and session analytics.
  It includes a conversation viewer, subagent tree, regex search, and some
  third-party session import. Its product scope is much larger than the
  minimal transcript viewer Conspectus needs.
- `claude-code-log` (`https://github.com/daaain/claude-code-log`): Mature
  Python CLI for converting Claude Code JSONL to HTML and Markdown. It has a
  TUI browser, detail levels, token accounting, and good Claude Code parsing,
  but it is Claude-only and export-oriented.

Tools such as native GUI session browsers, web-first dashboards, or tools that
require local knowledge stores may still be useful for users, but they should
not be the primary integration target for Conspectus's minimal CLI workflow.

## Candidate Direction

No final tool choice is made by this ADR. The likely integration shape is an
interface owned by Conspectus or Atelier, with external tools treated as
pluggable backends:

```rust
trait SessionViewer {
    fn key(&self) -> &str;
    fn supports(&self, harness_key: &str) -> bool;
    fn view(&self, session: &AgentSessionRef, options: ViewOptions) -> Result<()>;
    fn export_text(&self, session: &AgentSessionRef, detail: DetailLevel) -> Result<String>;
}
```

The first implementation pass should keep room for:

- an external `ccview` backend for Claude Code and OpenCode transcript viewing
- an external `recall` backend for multi-harness search/resume-oriented
  workflows
- an external `claude-history` backend or reference path for Claude-only
  terminal transcript UX
- a native fallback renderer that reads known harness state directly and can
  preserve full-fidelity transcript semantics when no external viewer is
  available

The native fallback is important even if an external viewer is used by
default. Conspectus already has provider-specific discovery adapters and can
hold the provider provenance needed to locate the raw state. A fallback
renderer can guarantee behavior that matters to Conspectus's graph model:
render pre-compaction Claude Code rows, mark compaction summaries explicitly,
and follow `parent_session` relationships using Conspectus's resolved graph
rather than relying on a viewer's provider-specific interpretation.

## Open Questions

- Should Conspectus ship a native minimal transcript renderer before adding
  external viewer backends, or should it initially delegate to existing tools?
- Which viewer should be the preferred external backend for Claude Code:
  `ccview`, `claude-history`, or `claude-code-log`?
- Should the viewer interface live in Conspectus, Atelier, or a thin adapter
  layer used by both?
- What is the minimum common transcript model needed for rendering Claude
  Code, OpenCode, Codex, and aider without losing provider-specific detail?
- Should "full transcript" mean physical raw-state order, resolved conversation
  graph order, or both as separate modes?
- How should subagents, fork records, and intra-harness lineage be exposed in
  the viewer UI?
- Should interactive expand/collapse be a backend capability flag, or should
  Conspectus normalize detail levels and let backends degrade gracefully?

## Consequences If Accepted Later

- `AgentSession` views can offer a `view` command that opens a readable
  transcript instead of forcing users to locate provider-native state files.
- External viewer support remains optional and replaceable.
- Conspectus can keep "full transcript" semantics tied to its own lineage and
  compaction understanding, even when a user chooses a richer third-party TUI.
- The interface boundary prevents broad dashboards from becoming hard
  dependencies of Conspectus or Atelier.

## Alternatives Considered

- **Adopt a single external viewer as the implementation.** Deferred because no
  candidate currently satisfies multi-harness coverage, full transcript
  fidelity, minimal dependencies, and interactive rendering at once.
- **Only expose raw transcript paths and let users choose tools manually.**
  Too weak for the intended workflow; it does not provide pretty printing or a
  stable integration contract.
- **Build a full TUI viewer inside Conspectus immediately.** Possible, but it
  risks duplicating mature Claude Code UX already present in `claude-history`
  and broader multi-harness search ideas already present in `recall`.
- **Use a web or GUI dashboard as the main viewer.** Rejected for the primary
  integration because Conspectus and Atelier should preserve a CLI-first,
  minimal-dependency workflow.
