# ADR 0019: Session Transcript Viewer Integration

## Status

Accepted (amended 2026-06-02 with the May 2026 candidate survey;
amended 2026-06-02 again to reflect supersession of the *default*
viewer path by ADR 0052). The inline transcript preview is out of
scope for this ADR and is handled by ADR 0051 and the
`H-TRANSCRIPT-*` workstream. The native in-tree viewer (ADR 0052)
is the **default** target of the `T` keybind; external launches
described below remain as an **opt-in escape hatch**, reached via
`[viewers.<harness>]` config (`H-TRANSCRIPT-013`).

Amended 2026-09-30 (`H-RUST-016`): the `SessionViewerAction` trait
described under "Integration shape" is retired. See the amendment at
the end of this ADR.

## Context

Conspectus discovers sessions from multiple agent harnesses and records
lineage between sessions, mux sessions, checkouts, forks, and repositories.
The current table and JSON views are useful for finding sessions, but they do
not answer a separate workflow: open a readable, full transcript for an
`AgentSession` without dropping into provider-specific JSON, SQLite, or
history file formats.

This ADR covers the *external full-transcript viewer launch* — the
keybind that hands control to a child process, the same way
`P8-010` hands control to `tmux attach-session`. The *inline preview*
that fills the right panel for un-muxed `AgentSession` rows is a
separate concern: it reads recent turns through the harness adapters
(`H-TRANSCRIPT-003` … `H-TRANSCRIPT-007`), renders them with
`tui-markdown` per ADR 0051, and lives inside the same Ratatui
process. The two surfaces complement each other and neither
replaces the other.

The desired external viewer should:

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

## Candidate Tools (May 2026 Survey)

The May 2026 survey refreshes the original landscape. The headline change
since the first draft of this ADR: a clearer split has emerged between
*single-harness viewers with strong machine interfaces* (best for the
external-launch story) and *broad multi-harness dashboards* (better as
references than as Conspectus backends).

- **`claude-history`** (`https://github.com/raine/claude-history`,
  Rust, MIT, v0.1.64 May 2026). Most mature transcript viewer for
  Claude Code. The decisive change since the prior survey is the
  documented **agent protocol**: `claude-history agent search`,
  `within`, and `read` subcommands return structured data;
  `--show-id`, `--plain`, and `--render <FILE>` give clean
  scripting hooks; `--pager` and the interactive viewer cover the
  human path. This is now the obvious external-launch target for
  Claude Code rows.
- **`recall`** (`https://github.com/zippoxer/recall`, Rust, MIT,
  v0.5.0 January 2026). Multi-harness search/resume across Claude,
  Codex, OpenCode, and Factory/Droid. `recall search` is the
  scriptable surface; `recall` (no args) is the interactive TUI.
  Resume commands are configured per harness via `RECALL_*_CMD`
  environment variables, which is a useful integration pattern but
  not a direct viewer hand-off. Best fit when the user wants a
  cross-harness picker, not a fixed-harness deep viewer.
- **`coding_agent_session_search`** (`cass`,
  `Dicklesworthstone/coding_agent_session_search`, Rust, license
  "MIT + OpenAI/Anthropic Rider"). Indexes 20+ providers (Codex,
  Claude Code, Gemini, Cline, OpenCode, Amp, Cursor, ChatGPT,
  Aider, Pi-Agent, GitHub Copilot variants, OpenClaw, Clawdbot,
  Vibe, Crush, Hermes, Kimi Code, Qwen Code, Factory/Droid).
  `cass search`, `cass sessions`, `cass view`, `cass expand`,
  `cass export-html`, `cass pack` accept `--json` / `--robot` and
  use a stdout-data / stderr-diagnostics protocol with structured
  error envelopes. The non-standard license rider needs review
  before Conspectus declares it the preferred backend, but its
  machine interface is the strongest in the survey.
- **`lazyagent`** (`https://github.com/illegalstudio/lazyagent`,
  Go, MIT, v0.12.0 May 2026). Monitoring dashboard for nine
  harnesses (Claude Code, Cursor, Codex, Grok CLI, Kilo, Kimi Code
  CLI, Amp, pi, OpenCode). The new HTTP API (`lazyagent --api`,
  Bearer-token protected) is a viable embedding surface but is a
  long-running service rather than the one-shot child-process
  hand-off the Conspectus action surface assumes. Useful as a
  reference for the multi-harness data model.
- **`ccview`** (`https://github.com/shivamstaq/ccview`, Go, MIT,
  v1.0.1 April 2026). Earlier drafts of this ADR recorded `ccview`
  as disappeared — that was incorrect. The project is still alive
  with a split-pane TUI, terminal Markdown rendering (Glamour),
  conversation search, subagent inspection, and HTML / Markdown /
  JSONL export across Claude Code and OpenCode. Still narrower
  than `claude-history` for Claude Code alone.
- **`ai-dash`** (`https://github.com/adinhodovic/ai-dash`, Go).
  Discovery and dashboard TUI for Claude Code, Codex, and
  OpenCode. Has a shared session model and parent/child
  metadata, but it remains a session-browser rather than a
  full transcript renderer. Useful reference for session model
  shaping, not a launch backend.
- **`ccboard`** (`https://github.com/FlorianBruniaux/ccboard`, Rust,
  MIT OR Apache-2.0, v0.22.0 April 2026). Monitoring dashboard
  with a conversation viewer and `ccboard export conversation
  <session-id>` for Markdown / JSON / HTML output. The split into
  `ccboard-core` (parsers, models, store, watcher; published on
  crates.io at 0.22.0) is the notable change — it makes `ccboard`
  a realistic *library* candidate as well as a binary, though
  Conspectus has not committed to consuming any external session
  model.
- **`claude-code-log`** (`https://github.com/daaain/claude-code-log`,
  Python). Unchanged from the prior survey: Claude-only, export
  oriented, still useful for HTML/Markdown conversions.

Tools such as native GUI session browsers, web-first dashboards, or tools
that require local knowledge stores may still be useful for users, but they
should not be the primary integration target for Conspectus's minimal CLI
workflow.

## Decision

### Boundary

ADR 0051 and the `H-TRANSCRIPT-*` workstream own the *inline*
recent-history preview: read N recent turns on selection change via
per-harness adapters, render with `tui-markdown` inside the existing
Ratatui process, no external viewer involved. Conspectus owns that
extraction end-to-end because it shares the H-PREVIEW grammar and
needs to stay within process for snapshot-testability,
`--color=never`, and `--no-live-preview`.

This ADR owns the *external full-transcript launch*: a separate
keybind (proposed `T`) that hands control to a child viewer the same
way `P8-010` hands control to `tmux attach-session`. The two
surfaces complement each other.

### Integration shape

The viewer backend selection lives behind a narrow trait inside
`src/tui/actions.rs` (already the home of attach/resume command
construction):

```rust
trait SessionViewerAction {
    fn key(&self) -> &str;
    fn supports(&self, harness_key: &str) -> bool;
    fn plan(&self, session: &AgentSessionRef) -> Result<LaunchPlan>;
}
```

The trait deliberately omits the `view` / `export_text` methods from
the original sketch. The May 2026 survey shows that the candidates
worth integrating with all expose either (a) a binary on `PATH` that
takes a session identifier and runs to completion, or (b) a `--json`
contract that the inline preview already covers. A
`SessionViewer::view` method that owns the TUI would require
Conspectus to re-render content the external tools already render
themselves, which duplicates work and limits the user's choice of
viewer. `plan` returning a `LaunchPlan` (binary path + argv) keeps
Conspectus in the action-resolver role and lets the runtime hand off
the TTY exactly as `P8-010` does for `tmux attach`.

### Backends in v1 scope

The v1 launch story ships two action-resolver implementations behind
the trait above:

- A **Claude Code** backend that prefers `claude-history` when found
  on `PATH`. Argument shape: `claude-history --show-id
  <session-uuid>` for the interactive viewer; the structured
  agent-protocol subcommands stay reserved for future automation
  paths.
- A **multi-harness** backend that prefers `recall` when found on
  `PATH`, used when the selected row's harness is Claude Code,
  Codex, OpenCode, or Factory/Droid and the operator wants
  cross-harness behavior. `recall` does not currently accept an
  "open this exact session" argument, so the integration drops the
  user into the search UI rather than asserting a deep link.

Selection prefers the harness-specific backend (`claude-history`)
over the multi-harness backend (`recall`) when both are present.
When neither binary is on `PATH`, the action surfaces a
disabled-action reason that names the binaries the user could
install. There is no built-in fallback renderer in v1 — the inline
preview already covers "I just want to glance at this session" and
no built-in path would meaningfully improve on the external viewers
for the full-transcript surface.

### Deferred backends

- `cass` (`coding_agent_session_search`) is the strongest machine
  interface in the survey, but its non-standard license rider
  needs review before Conspectus declares it a recommended
  backend. Re-evaluate once the rider is clarified or removed.
- `ccview` and `ccboard` are recorded as known viewers operators
  may already have installed, but neither is the preferred backend
  for Claude Code (`claude-history` is more mature) or for
  multi-harness use (`recall` covers the four harnesses the
  Conspectus product surface already targets).
- `lazyagent --api` is an HTTP service rather than a one-shot
  viewer; pursuing it would require a long-running-service
  integration model Conspectus does not have today. Reconsider if
  Phase 7's server mode (ADRs 0037 / 0038) opens a useful seam.

## Open Questions Answered

- The inline preview is *not* part of this ADR. ADR 0051 covers the
  rendering dependency; `H-TRANSCRIPT-003` … `H-TRANSCRIPT-010`
  cover the data path and the widget.
- The viewer trait lives inside Conspectus (`src/tui/actions.rs`),
  not in Atelier and not in a shared adapter layer. Atelier can
  consume the same action-resolver if needed.
- The preferred Claude Code backend is `claude-history` (Rust,
  MIT, agent protocol, active maintenance). `ccview` and
  `claude-code-log` remain useful references but are not the
  preferred external launch target.
- The preferred multi-harness backend is `recall` for Claude Code,
  Codex, OpenCode, and Factory/Droid. `cass` is held back pending
  license rider review.
- "Full transcript" is delegated to the external viewer's
  interpretation. Conspectus's responsibility is to identify the
  session and pick a viewer; the viewer renders.
- Subagents, fork records, and intra-harness lineage continue to
  be surfaced by the Conspectus graph (`AgentSession` lineage
  endpoints, ADRs 0005 / 0018). The external viewer is not asked
  to follow them.
- Interactive expand/collapse is a property of the chosen
  external viewer; Conspectus does not normalize detail levels
  across backends in v1.

## Consequences

- `AgentSession` views can offer a single-keybind hand-off to a
  full transcript viewer. The keybind is disabled with a clear
  reason when no supported viewer is installed.
- The action-resolver seam mirrors `P8-010`'s `tmux attach`
  hand-off; no in-process viewer surface is added.
- External viewer support remains optional and replaceable. New
  backends can be added by implementing `SessionViewerAction` and
  registering them, without touching the inline preview or the
  graph model.
- Conspectus does *not* ship a built-in transcript renderer in
  v1. The inline recent-history preview (ADR 0051) covers the
  glance-at-context surface; the external launch covers the
  full-transcript surface.
- Conspectus keeps "full transcript" semantics tied to whichever
  viewer the operator installed. If a future user reports that an
  external viewer mishandles compaction or lineage in a way that
  matters, this ADR's seam is the place to add a native fallback
  renderer rather than the inline preview path.

## Alternatives Considered

- **Adopt a single external viewer as the implementation.** Updated
  position: still rejected because no candidate covers every
  harness in the Conspectus product surface, but the May 2026
  survey narrows the action-resolver targets to `claude-history`
  (Claude Code) and `recall` (multi-harness) for v1.
- **Only expose raw transcript paths and let users choose tools
  manually.** Too weak; provides no integration contract and the
  inline preview already handles the lightweight case.
- **Build a full TUI viewer inside Conspectus immediately.**
  Rejected for v1. ADR 0051 establishes the inline preview as the
  in-process surface; replicating a full transcript renderer
  duplicates work `claude-history` and `recall` already do well.
- **Use a web or GUI dashboard as the main viewer.** Rejected for
  the primary integration; keep the CLI-first, single-binary
  posture (ADRs 0016 / 0024).
- **Adopt `cass` as the v1 preferred multi-harness backend.**
  Deferred pending license rider review. Reconsider when the
  rider is clarified.
- **Bake a `SessionViewer::view` method that owns the TUI.**
  Rejected: it duplicates rendering the external viewers already
  do well, locks Conspectus into one viewer's conventions, and
  fights the immediate-mode posture from ADR 0024. Launching the
  viewer as a child process matches the `P8-010` precedent.

## Amendment: Retire The `SessionViewerAction` Trait (2026-09-30)

The trait had one implementation (`ClaudeHistoryViewer`), reached
through a one-element array of trait objects, so every call went through
a preference-order loop with nothing to choose between. Once ADR 0052
made the native viewer the default, the external launch became a
narrow escape hatch with no second backend in sight.

`resolve_viewer_target` in `src/tui/viewer.rs` now checks the harness,
probes `PATH` for `claude-history`, and resolves the transcript file
directly. The `ViewerTarget` and `ViewerDisabled` outcomes, and the
`BinaryProbe` test seam, are unchanged.

When config-defined viewers land (`H-TRANSCRIPT-013`), they are data
(a program plus an argument template per harness), not trait
implementations. A configured list of those, checked before the
built-in `claude-history` fallback, is the extension point rather than a
trait.
