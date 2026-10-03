---
id: m-11
title: "Hardening Backlog"
---

## Description

Deferred investments and opportunistic cleanups identified during the
post-Phase-6 codebase review. Items here are not gated by a phase plan; pull
them into a future phase or land them opportunistically when the surrounding
area is already being touched. Group prefixes:

- `H-REF-*` internal refactors and dedup
- `H-OBS-*` observability, diagnostics, and CLI UX
- `H-PROD-*` user-facing product surface gaps
- `H-DIST-*` distribution, CI, and release plumbing
- `H-DESIGN-*` open design questions to settle before they constrain
  implementation
- `H-FUTURE-*` provider/feature expansions deliberately deferred until needed

### Refactors And Dedup

- **CSP-083** Extract a shared `DeclaredEndpoint` codec
- **CSP-084** Share the relation-kind string codec
- **CSP-085** Generalize the resolver scoring tier helpers
- **CSP-086** Unify the external-tool runner seam
- **CSP-087** Split `src/declared.rs` by concern
- **CSP-088** Slim `src/cli.rs` into per-command modules
- **CSP-089** Factor harness adapter state-root scanning
- **CSP-090** Replace string field names in `SourceMetadata.fields`
- **CSP-091** Centralize provider identifier constants
- **CSP-092** Audit and shrink the curated `conspectus::api` surface

### Code Hygiene And Simplification (H-HYG-*)

Source plan: `docs/code-hygiene-audit.md` (2026-07-01 audit; companion to
`docs/extensibility-assessment.md`). Findings were verified by diff/hash and
a pedantic+nursery clippy inventory, not name-matching. All chunks are
behavior-preserving; existing snapshot suites are the regression net. The
audit's "What Not To Change" section bounds the scope — resolver semantics,
runner seams, doc culture, and test volume are explicitly out of bounds.

- **CSP-462** Dedupe the copy-pasted micro-helpers
- **CSP-463** Extract shared TUI/output row-assembly helpers
- **CSP-464** Parameterize the centered-modal rect math
- **CSP-465** Remove the argv-sniffing fake-mtime test backdoor
- **CSP-466** Adopt a curated `[lints.clippy]` table and fix fallout
- **CSP-467** Introduce a `SnapshotIndex` for graph lookups
- **CSP-468** Declarative keybinding table for dispatch, overlays, and help
- **CSP-469** Unify the dual event loops, then split `runtime.rs`
- **CSP-470** Split the TUI monolith files by concern
- **CSP-471** Finish the `output::render` migration and settle `dev_scenarios` gating
- **CSP-472** Test builders and sibling-file test extraction (rolling)

### TUI Architecture Convergence (H-TUI-*)

Source plan: `docs/tui-architecture-review.md` (2026-07-01 fresh-eyes
review). Governing decision: **ADR 0085** — memorializes the target
model (single-store Elm/MVU loop with effects-as-data, modal stack,
and derived view-models) as the guardrail every H-TUI-* story lands
against. Verdict from the review: the TUI is ~70% of an Elm/MVU
architecture already — single state value, `Msg` reducer,
immediate-mode render, async discovery as messages — and these
stories finish that shape instead of adopting a framework. Explicit
non-goals recorded in the ADR and review: no tui-realm or component
framework, no retained-mode rewrite, no async reducer. These refine
the overlapping `H-HYG` stories rather than duplicating them;
cross-references below.

- **CSP-495** Make row trees derived view-models
- **CSP-496** Adopt effects-as-data in the reducer
- **CSP-497** Replace overlay Option slots with a modal stack and a shared Overlay contract
- **CSP-498** Unify the event loops behind an event union and subscriptions
- **CSP-499** Move scroll reconciliation into the reducer
- **CSP-500** Unify overlay dispatch under the `Overlay` trait

### Observability And CLI UX

- **CSP-093** Add a human-readable graph projection
- **CSP-094** Add `conspectus node show <id>`
- **CSP-095** Add filter flags for the graph and session commands
- **CSP-096** Add a `--explain` mode for resolved relationships
- **CSP-404** Review and resolve ADR 0059 (resolver rules-engine evaluation)
- **CSP-097** Improve discovery diagnostics for missing providers
- **CSP-098** Surface activity/recency in the session tables
- **CSP-360** Gate left-pane tree navigation keys on left-pane focus

### Table Output Modernization

The current `conspectus session` tables (`src/output/table.rs`) hand-roll
column alignment via a fixed 2-space padder. Cells like cwd, agent label,
mux session, and PR identifier routinely blow past any reasonable terminal
width, making the default output unusable in narrow CLIs. JSON / graph
output is not affected by this stream; the work is scoped to the text-table
projection layer.

CSP-126 through CSP-130 modernized the renderer itself (width-aware
truncation, short row ids, card layout, `node show` integration).
CSP-146 onward shifts the surface from `conspectus session
[--projection ...]` to `conspectus table <ROWS>` so that growing row-types
(PRs, forks, …) and per-row-type column customization stay first-class.
The columns themselves stop being session-specific, since cells like PR,
fork lineage, and checkout apply to any row whose node touches them.

- **CSP-126** ADR: width-aware table rendering library

- **CSP-127** Surface short, stable row identifiers in session tables

- **CSP-128** Width-aware truncation default for session tables

- **CSP-129** Opt-in card / multi-line row layout

- **CSP-130** Resolve table row identifiers in `conspectus node show`

- **CSP-146** Rename `conspectus session` to `conspectus table <ROWS>`

- **CSP-147** Per-row-type column registry and `--columns` flag

- **CSP-148** `conspectus table prs` row-type

- **CSP-149** `conspectus table forks` row-type

- **CSP-150** Expand the `sessions` column pool

- **CSP-151** Expand the `mux` column pool

- **CSP-152** `conspectus columns <ROWS>` discovery subcommand

- **CSP-153** Pager auto-fit for table-style outputs

- **CSP-154** Terminal color and styling for table output

Deferred under this cluster (no story yet, file when needed):

- `conspectus table repos` / `conspectus table checkouts`. Both node
  kinds already appear as related-context columns under
  `CSP-150`. Promote to their own row-type only when a user
  workflow requires a repos-first or checkouts-first table.

### Product Surface Gaps

- **CSP-099** Implement the bootstrap-roots flow described in the design
- **CSP-100** Cache layer for forge metadata, tmux, and harness scans
- **CSP-101** Batch `gh pr list` across repos sharing a host
- **CSP-102** Add a graph-diff command

### Distribution And CI

- **CSP-103** Add a GitHub Actions CI workflow
- **CSP-104** Complete `Cargo.toml` metadata for crates.io
- **CSP-105** Pin and verify MSRV
- **CSP-106** Define the release process

### Design Closure

- **CSP-107** Settle the workspace-detection threshold and provider precedence
- **CSP-108** Settle `ForgePr` identity and branch-association keys
- **CSP-109** Settle declared-link conflict and override semantics
- **CSP-110** Document the graph invariants and snapshot canonicalization contract

### Checkout Context Model

ADR 0026 replaces "worktree" as the product-level concept with
`Checkout`: the concrete editable working tree for a repo, whether it is
an ordinary clone checkout, a linked git worktree, a bare-repo-derived
linked worktree, or a workspace member reached through a symlink.

- **CSP-197** Memorialize the checkout context model
- **CSP-198** Introduce checkout-facing model helpers ahead of the graph wire rename
- **CSP-199** Probe observed session and mux cwd paths for checkout context
- **CSP-200** Preserve logical and canonical paths for workspace members
- **CSP-201** Resolve multi-context session membership
- **CSP-202** Update table and TUI projections for checkout grouping
- **CSP-203** Retire legacy user-facing worktree terminology
- **CSP-204** Hard-rename checkout graph wire/model names

### Deferred Provider And Workflow Expansions

These items match the design guidance to *design for* additional providers
without *implementing* them until needed. File them so the next consumer
need does not surprise the project.

- **CSP-111** Add a mux backend for zellij (and stub screen)
- **CSP-112** Add a forge adapter for GitLab or Gitea
- **CSP-113** Add harness adapters for jujutsu and sapling sessions if and when a user uses them with a supported harness

### Provider Extensibility (H-EXT-*)

Source plan: `docs/extensibility-assessment.md` (2026-07-01 audit). Goal:
implementing a single interface per entity family — harness, mux backend,
forge, orchestrator — is sufficient for a new provider to flow through
discovery, CLI, TUI, graph exports, and continuous mode. The acceptance
criterion is the assessment's: anything a new provider needs outside its own
module plus one registry entry is a regression. Phases A (registry backbone)
and B–D land independently; ordering below follows the assessment's
dependency graph.

Phase A — registry backbone (no behavior change):

- **CSP-473** Add a provider descriptor registry
- **CSP-474** Route harness pure-data lookups through the adapter registry
- **CSP-475** Key TUI harness colors by harness key

Phase B — harness experience parity:

- **CSP-476** Move per-harness runtime signatures onto `HarnessAdapter`
- **CSP-477** Normalize hook payloads through the adapter
- **CSP-478** Provide transcript locator and parser via the adapter
- **CSP-479** Generalize the codex_log-style aux-reader wiring

Phase C — mux backend abstraction:

- **CSP-480** Extract a `MuxBackend` trait from `TmuxRunner`
- **CSP-481** Capability-gate mux actions instead of naming tmux
- **CSP-482** Add a zellij mux backend (discovery + attach)
- **CSP-483** Capture hook mux context through the backend probe

Phase D — forge and orchestrator registries:

- **CSP-484** Wire forges as an adapter list
- **CSP-485** Add a second forge adapter (GitLab or Gitea)
- **CSP-486** Add a generic orchestrator registration surface
- **CSP-487** Add an orchestrator mutation-capability seam (deferred)

Phase E — conformance and docs:

- **CSP-488** Add adapter conformance suites per entity family
- **CSP-489** Write the provider-adapter contributor guide

### Documentation

- **CSP-114** Add a first-run walkthrough
- **CSP-115** Add a provider-adapter contributor guide
- **CSP-116** Add library-integration examples beyond the api doctest

### ADR And Tenet Alignment (H-ADR-*)

Source plan: `docs/adr-audit.md` (2026-07-01 corpus audit). The audit's
bookkeeping amendments (ADR 0035 status, ADR 0062/0063 supersession notes,
ADR 0082 lessons addendum) were applied directly on 2026-07-01; the items
below need a real decision or real writing and are tracked here. All are
documentation/tenet work — none block feature stories, but `CSP-490` and
`CSP-491` should land before external contributors read the guardrails.

- **CSP-490** Restate the payload-privacy tenet precisely
- **CSP-491** Replace "read-only first" with a defined mutation envelope
- **CSP-492** Retire or permanently bless the `[tui].sessions_grouping` legacy alias
- **CSP-493** Write the consolidated mux-attribution architecture note
- **CSP-494** Adopt a two-tier decision-record convention

### Intra-Harness Session Lineage

`RelationKind::ParentSession` / `ChildSession` are wired through the model
(ADR 0005) and the resolver, but today they are only emitted from
atelier's fork-index metadata (`src/discovery/atelier.rs:423-440`). None
of the harness adapters extract lineage from the harness's own state, so
post-compaction, post-resume, and post-fork-by-the-harness sessions show
up as independent rows even when one is a direct successor of another.
With long-lived users, this means a large fraction of the
`conspectus session` rows are legacy sessions whose link to a currently
active session is silently dropped.

ADR 0005 currently frames `ParentSession` / `ChildSession` as edges
anchored at a `Fork` node. Intra-harness compaction/resume is *not* a
fork (no context effect, no provider-recorded fork metadata). Decide
during `CSP-117` whether to (a) extend ADR 0005 to allow
session→session edges without a Fork middle node, or (b) require a
synthetic `Fork` node with provider `<harness>` and an explicit
`lineage_kind` such as `compaction` or `resume`. The former is simpler
for queries; the latter keeps lineage uniform with the fork-anchored
shape.

- **CSP-117** Settle the data-model shape for intra-harness lineage
- **CSP-118** Extract claude-code session lineage
- **CSP-119** Extract opencode session lineage from `session.parent_id`
- **CSP-120** Extract codex resume lineage
- **CSP-121** Surface session lineage in the session table
- **CSP-125** Retarget claude-code lineage extraction — fork uses a `forkedFrom` envelope object, not `parentUuid`…

### Agent Session Last-Message Preview

Claude Code's `/resume` view shows each historical session with a short
snippet of its most recent message. Conspectus's table output benefits
from the same: scanning a column of `claude-code:alpha`,
`codex:beta`, `opencode:gamma` rows is far more useful when each
carries a one-line "what was it doing?" preview alongside the harness
label, cwd, mux, and PR cells. Per ADR 0023 the preview lives on
`AgentSessionNode` as `last_message_preview: Option<String>`,
populated best-effort by each harness adapter; the table renderer
sources it like any other cell rather than re-reading transcripts at
render time.

- **CSP-155** Model field + opt-in preview column registration

- **CSP-156** Claude Code last-message extraction

- **CSP-157** Codex last-message extraction

- **CSP-158** Opencode last-message extraction

- **CSP-159** Aider last-message extraction

- **CSP-173** Filter codex channel markers from preview

- **CSP-174** Move title out of AGENT label into its own column

### Agent Session Transcript Preview And Viewer

The `H-PREVIEW-*` stories established a single-line
`last_message_preview` populated at discovery time. The next step is
the TUI right-panel surface: when an un-muxed agent session is
selected, the panel currently renders only that one normalized line
even though there is plenty of vertical space and useful prior context
on disk. This workstream extends the un-muxed preview into a styled,
multi-message recent-history rendering, and decides how an optional
external "full transcript" viewer integrates per ADR 0019.

The design constraints from the May 2026 candidate survey (see also
ADR 0019):

- The inline preview lives inside the existing two-panel Ratatui UI;
  it should not require a child-process viewer just to populate the
  right pane. Embedding a Rust markdown renderer is the lowest-risk
  path.
- `tui-markdown` (joshka/tui-markdown, v0.3.7) returns
  `ratatui::text::Text` directly and pairs cleanly with the existing
  `ansi-to-tui` dep and Ratatui 0.30. It is the leading candidate for
  the inline styling concern. Per ADR 0024 a new TUI crate dependency
  needs a follow-on ADR before adoption.
- Recent-history extraction should be done on demand when the
  selection changes, not eagerly populated for every session at
  discovery time the way `last_message_preview` is. A per-selection
  read is bounded (one transcript, last N turns) and avoids paying
  the cost for sessions the user never opens.
- Compaction (Claude Code), tool-only tails, reasoning blocks, and
  channel markers (codex) are already handled in the H-PREVIEW
  extractors; the recent-history readers should share that grammar
  rather than re-parsing from scratch.
- External viewers (`claude-history`, `recall`, etc.) remain the
  right surface for a separate "open full transcript" action. The
  inline preview is not a replacement for them, and they are not a
  replacement for the inline preview.

This workstream supersedes the single-bullet `CSP-171.03` story; that
entry stays in Phase 8 as the v1 release-boundary marker that is
satisfied when this workstream's TUI integration stories land.

- **CSP-205** ADR: terminal markdown rendering for the inline transcript preview

- **CSP-206** Resolve ADR 0019 with the May 2026 survey

- **CSP-207** Recent-history adapter API

- **CSP-208** Claude Code recent-turns extractor

- **CSP-209** Codex recent-turns extractor

- **CSP-210** OpenCode recent-turns extractor

- **CSP-211** Aider recent-turns extractor (deferred)

- **CSP-212** Add `tui-markdown` dependency

- **CSP-213** Inline transcript-preview widget

- **CSP-214** Wire the widget into the right panel for un-muxed agent rows

- **CSP-215** Document the inline transcript preview

- **CSP-216** External full-transcript viewer launch (moved ahead of the inline-widget track per the amended ADR 0019…

- **CSP-330** Surface recall's harness coverage gap (or broaden it)

- **CSP-331** Recall focus on deep-link entry

- **CSP-332** Config-driven viewer override

### Native In-Tree Transcript Viewer (H-VIEWER-NATIVE-*)

Per ADR 0052. Builds a Ratatui full-screen modal that renders a
single session's transcript inside the conspectus process.
Replaces the external-launch path (`CSP-216`) as the
default `T` action. Designed for later extraction to a standalone
crate per `docs/transcript-viewer-deps.md`.

Stories below depend on `CSP-207` (recent-history adapter
API) but extend its return shape from "last N turns" to "full
transcript with a cursor at the last turn".

- **CSP-333** Module scaffold + dep-surface enforcement

- **CSP-334** `TranscriptDocument` + `TranscriptTurn` model + `SessionLocator` types

- **CSP-335** Claude Code parser

- **CSP-336** Codex parser

- **CSP-337** OpenCode parser (SQLite-of-record)

- **CSP-338** Viewer widget: full-screen modal, scroll, jump-to-end-on-open

- **CSP-339** Substring search inside the viewer

- **CSP-340** Viewer-bridge integration: wire the `T` keybind into the native viewer

- **CSP-343** Styling + spacing pass

- **CSP-341** Retire patched recall from `pkgs/recall/`

- **CSP-344** Mouse bindings inside the viewer

- **CSP-345** In-viewer navigation into forks / child sessions

- **CSP-354** Per-message selection + clipboard copy

- **CSP-355** Per-tool expand on click

- **CSP-356** Markdown table rendering

- **CSP-346** Lazy / chunk-by-chunk transcript loading around compaction boundaries

- **CSP-342** (later) Extraction prep: lift `src/viewer/` into a workspace member crate

### Agent Session Continue Scheduling

Some harnesses end a transcript with a usage-limit or rate-limit
message that includes the time when work can resume. Conspectus
already reads the last message for previews and can open native
transcripts on demand; this workstream adds a higher-level
"blocked until" signal and an explicit way for the operator to
schedule a `Continue` prompt for that session at the time identified
by the transcript.

Detection stays read-only: a session whose final meaningful turn is a
usage-limit message should surface as paused/blocked metadata and table
or TUI affordances. Scheduling is separate, explicit user intent. It
must not silently send prompts or create scheduler state from ordinary
`graph`, `table`, or `tui` discovery.

- **CSP-347** ADR: usage-limit detection and scheduled continuation policy

- **CSP-348** Model blocked-session metadata

- **CSP-349** Detect usage-limit tails in supported transcript parsers

- **CSP-350** Surface blocked-until state in CLI and TUI

- **CSP-351** Implement explicit continue scheduling

- **CSP-352** Document blocked-session and continue workflows

### Agent-Mux Orchestrator Integrations

A growing class of "agent-over-tmux" orchestrators — agent-deck, dmux,
workmux, agent-of-empires, and others — maintain on-disk state that
maps agent sessions to tmux sessions, checkouts, branches, and
sometimes forks. Most of that state, however, overlaps with what the
process-tree linker (`H-MUXPROC-*`) can derive directly from running
processes inside each tmux pane: pane ↔ harness binary, pane PID,
process cwd, and therefore pane ↔ `AgentSession` for live sessions.
Conspectus should treat MUXPROC as the primary, tool-agnostic source
of agent ↔ tmux evidence and only build per-orchestrator adapters
when they expose evidence MUXPROC cannot — concretely: workspace
composition, container-isolated agents, exited / paused sessions, or
orchestrator-specific labels and lineage.

In-scope candidates (gated on the audit in `CSP-131`):
agent-deck (~/.agent-deck/, SQLite), dmux (`standardagents/dmux`,
~1.6k stars), workmux (`raine/workmux`, ~1.5k stars, per-worktree
`.workmux/` plus `~/.local/state/workmux/`), agent-of-empires
(`njbrake/agent-of-empires`, ~2.3k stars). Explicitly deferred:
`cdknorow/coral` (~21 stars), `honeymux/honeymux` (~71 stars, runtime
overlay rather than persistent state).

- **CSP-131** Audit each candidate orchestrator's evidence against MUXPROC and decide which adapters to build

- **CSP-122** Detect agent-deck multi-repo checkouts as a workspace provider

- **CSP-123** Surface multi-repo participants in the session table

- **CSP-124** Read agent-deck profile state from `state.db`

- **CSP-451** Route agent-deck mux renames through agent-deck

- **CSP-132** Add a dmux orchestrator adapter (audit-gated)

- **CSP-133** Add a workmux orchestrator adapter (audit-gated, narrowed scope)

- **CSP-134** Add an agent-of-empires orchestrator adapter (audit-gated)

### Workspace UX Redesign (H-WS-*)

Operator-noted UX confusion in the Sessions view's `graph` grouping:
sessions whose cwd is in a repo that happens to be a workspace member
get nested under the workspace even when the session itself has no
`AssociatedWith` workspace edge. Heavily-used member repos
(`conspectus`, `config`, `atelier`) make the workspace level
actively misleading. See `docs/plans/workspace-view-redesign.md` for
the full diagnosis, the three axes (Sessions/Graph fix, dedicated
Workspaces view, other-view audit), and the recommended sequence.

The conflation is between two semantically distinct relationships:
(A) the session is workspace-rooted via `AssociatedWith Workspace`,
and (B) the session merely touches a repo that is *also* a workspace
member with no session-level workspace edge. Today's grouping
promotes (B) to look like (A); the fix surfaces them as different
concepts everywhere they appear.

- **CSP-405** Strict-only + chip in Sessions/Graph workspace nesting

- **CSP-406** Dedicated Workspaces view (MVP)

- **CSP-406.01** Workspaces view polish: Provider/Activity/Repo groupings

- **CSP-407** Audit Mux/Prs/Forks/Union workspace grouping for the same (A)/(B) conflation

- **CSP-414** Hybrid workspace+repo grouping in Sessions / Graph

### Process-Tree Agent↔Pane Linking

Independent of any orchestrator's state file, an agent process running
inside a tmux pane can be identified by walking the pane's process
tree and matching descendant command names against known agent
harness binaries (claude / codex / opencode / aider). `tmux-agent`
(`trentdavies/tmux-agent`) demonstrates this stateless approach: it
takes a single `sysinfo` snapshot, then for each pane walks up to ~3
levels of descendants from the shell PID and matches against a known
binary-name set, with regex-over-pane-content and title heuristics as
fallbacks. For Conspectus this would be a tool-agnostic, definitive
session ↔ pane evidence source that works even when no orchestrator
is installed — and a useful cross-check against agent-mux adapter
output when one is.

Drift-reduction sequence after the `CSP-227` Claude Code
failure:

1. Finish the `CSP-136` process-linking slice by making the
   evidence taxonomy explicit in tests and resolver ranking. In
   particular, treat start-command / argv session ids as launch
   evidence, below active open-fd, hook, control-plane, and fresh
   state evidence. This immediately reduces the chance that stale
   `--resume` arguments become preferred links.
2. Take `CSP-227` as the first regression story, even before a
   definitive Claude-current-session source exists. Add fixtures for
   launch session A plus stronger current-session evidence B, and for
   the fallback case where launch evidence remains the best available
   signal. This locks in the intended resolver behavior while later
   sources are still being researched.
3. Do `CSP-219` and `CSP-223` as short audits in parallel
   if possible. They have no blockers and decide whether Claude Code,
   Codex, or opencode can expose current session identity through a
   non-mutating control plane or hook/plugin path. The Claude Code
   `/resume` drift should be the primary audit scenario.
4. If hooks are viable, do `CSP-224`, `CSP-225`, then
   `CSP-226`. This is the highest-confidence durable path for
   Claude Code drift if hook payloads include the post-`/resume`
   session id or transcript path. Keep `CSP-228` and
   `CSP-229` behind the same schema, but do not let them delay
   the Claude fix.
5. If a non-mutating Claude control plane exists, add the corresponding
   control-plane adapter before or instead of the hook emitter. If only
   Codex or opencode surfaces survive the audit, keep `CSP-220`
   and `CSP-221` scoped to those harnesses and continue the
   Claude path through hooks or read-only file/state evidence.
6. Do `CSP-217` next for one-shot and future continuous-mode
   activity correlation. This improves fresh-session and post-switch
   attribution without requiring opt-in hooks, and gives the resolver a
   middle-strength signal above cwd-only matching.
7. Do `CSP-218` per ADR 0048. The May 2026 audit closed the
   opencode portion as a no-op and scoped the work to a Codex
   state-reader slice plus a Codex log-derived live-attribution
   linker. The log linker is also the Codex-side fix for the same
   stale-`--resume` drift class as `CSP-227`.
8. Land `CSP-222` as soon as the ADR path is open, or fold it
   into `CSP-135` if that ADR is still being written. This keeps
   terminal injection and slash-command probing out of the attribution
   design while the tempting `/usage` workaround is fresh.
9. Leave `CSP-220`, `CSP-221`, `CSP-228`, and
   `CSP-229` behind their audits and schema decisions. They
   improve cross-harness correctness, but they are not the shortest
   path to fixing the Claude Code mapping drift seen in
   `CSP-227`.

- **CSP-135** ADR: process-tree linker design and dependency choice

- **CSP-136** Implement the process-tree linker as a discovery source

- **CSP-217** Add read-only session-file activity correlation

- **CSP-358** Treat harness session keys as opaque strings in runtime attribution

- **CSP-359** Sweep remaining UUID-only extractor call sites

- **CSP-305** Evaluate first-class runtime process nodes

- **CSP-308** Add runtime process graph model and relation kinds

- **CSP-309** Persist runtime process nodes in SQLite and graph JSON

- **CSP-310** Emit runtime process nodes from MUXPROC discovery

- **CSP-311** Move mux-cardinality and attribution resolver logic onto runtime process evidence

- **CSP-312** Surface runtime process diagnostics in node detail and scenario fixtures

- **CSP-218** Read Codex state and log databases for live session attribution

- **CSP-219** Audit harness control planes for non-mutating current-session queries

- **CSP-220** Add Codex app-server attribution adapter if the audit proves a stable non-mutating query

- **CSP-221** Add opencode server/ACP attribution adapter if the audit proves a stable non-mutating query

- **CSP-222** Document terminal-injection attribution as a rejected strategy unless a harness guarantees non-mutating status commands

- **CSP-223** Audit harness hooks/plugins as definitive session-state sidecar emitters

- **CSP-224** Define Conspectus hook sidecar schema and trust/ranking rules

- **CSP-225** Implement hook-sidecar discovery provider

- **CSP-226** Add Claude Code hook sidecar emitter if audit proves non-mutating session identity

- **CSP-230** Add `conspectus hook write` sidecar writer

- **CSP-231** Add `conspectus hook init` installer UX

- **CSP-249** Dedupe hook records by pane and drop the 15-minute emission gate

- **CSP-357** Investigate Claude Code Workflows process and session topology

- **CSP-402** Record the harness pid, not the hook writer's pid, in hook sidecar records

- **CSP-403** Demote `LinkedToMux` candidates whose source `AgentSession` is materially stale compared to a fresher candidate for the…

- **CSP-227** Fix Claude Code mux attribution after in-process `/resume` switches

- **CSP-228** Add Codex hook sidecar emitter if audit proves non-mutating session identity

- **CSP-229** Add opencode plugin sidecar emitter

### Testing Improvements And Regression Replay (TEST-*)

Recent bugfixes around hook sidecars, active-pane evidence, TUI
attachment, and provider parser drift show that individual unit tests
are not enough. The missing coverage is a higher-level, fixture-backed
way to replay whole operator scenarios across harness state, mux state,
hook records, resolver output, and TUI row projection. This workstream
adds that layer without replacing the existing unit, CLI smoke, and
snapshot tests.

Dependency shape inside the workstream:

```
CSP-262 ──→ CSP-263 ──→ CSP-264 ──→ CSP-266
              │             │
              └────────────→ CSP-265
```

`CSP-262` establishes the harness. `CSP-263` adds sanitized
real-world fixture material so regressions can be captured quickly.
`CSP-264` turns recent MUXPROC escapes into replayed scenarios.
`CSP-265` adds broad invariants that should hold across any graph
fixture. `CSP-266` covers TUI interaction regressions that only show
up after row expansion, scrolling, or attach resolution. `CSP-306`
turns the same replay worlds into named operator scenarios that can be
launched through CLI/TUI surfaces for manual inspection.

- **CSP-262** Add a MUXPROC scenario replay harness

- **CSP-263** Add a sanitized real-state fixture corpus

- **CSP-264** Replay recent MUXPROC drift and stale-evidence bugs

- **CSP-265** Add graph and row-projection invariant tests

- **CSP-266** Add TUI interaction regression tests for row expansion, scrolling, and attach resolution

- **CSP-306** Expose named replay scenarios to CLI and TUI runs

- **CSP-307** Add filter, grouping, and sort controls to dev scenario exploration

- **CSP-582** Make `tui --snapshot --snapshot-keys` honor pane focus so scripts can drive the right pane

### Session Naming

Conspectus today is read-only outside Phase 5 declared-link CRUD. Session
identifiers feel anonymous in the TUI: harness-native titles are sparse
(opencode only, claude-code carries a compaction summary, codex/aider
populate nothing) and tmux session names are operator-chosen but not
coordinated with the agent context. This workstream introduces operator-
controlled session naming as the first non-relationship write surface,
deliberately scoped conservatively:

- Names are stored as a Conspectus-owned alias overlay (ADR 0029), not
  written back to harness stores. Works uniformly across every harness
  including read-only ones.
- Renaming a muxed agent session also renames the tmux session in
  lockstep by default; `--no-mux` decouples.
- AI-driven name suggestions are deferred to the sibling
  `H-AI-NAMING-*` workstream so this one stays free of new dependencies,
  network IO, and an async runtime.
- Incidentally builds the first reusable TUI text-input primitive
  (ADR 0030), which unblocks `CSP-193` (`/` search overlay) and `CSP-175`
  (inline mux-picker).

Dependency shape inside the workstream:

```
CSP-232 ───────┬─→ CSP-235 ───────┐
CSP-233 ───────┤                  ├─→ CSP-236 ───────┬─→ CSP-237 ──────→ CSP-238
               └─→ CSP-240        │                  │
                                  │                  ├─→ CSP-239 ──────→ CSP-241
CSP-234 ──────────────────────────┤                  │                       │
                                  │                  │                       ├─→ CSP-242
                                  │                  │                       ├─→ CSP-243
                                  │                  │                       └─→ CSP-244
```

The two ADRs (`CSP-232`, `CSP-233`) and the `TmuxRunner::rename_session` seam
(`CSP-234`) are unblocked from day one and can land in parallel. `CSP-235` is the
spine; once it lands, projection (`CSP-236`), CLI (`CSP-237`/`CSP-238`), and lockstep
(`CSP-239`) follow. Input widget (`CSP-240`) is parallel to the CLI track but
blocks TUI wire-up (`CSP-241`).

- **CSP-232** ADR: alias overlay schema and storage
- **CSP-233** ADR: TUI text-input primitive
- **CSP-234** Extend `TmuxRunner` with `rename_session` mutation seam
- **CSP-235** Alias storage layer
- **CSP-236** Projection precedence
- **CSP-237** CLI: `conspectus rename` command tree
- **CSP-238** CLI: `conspectus alias list` (and `show`)
- **CSP-239** Mux lockstep helper
- **CSP-240** TUI text-input widget implementation
- **CSP-241** TUI `R` keybinding wires rename flow
- **CSP-242** Read-only invariant audit
- **CSP-243** Live-session UX advisory
- **CSP-244** Docs and snapshot coverage

### Session Pins

Settled by ADR 0057 (Accepted). Pins are user-authored declarations of
a logical agent session — a `(harness, cwd, display_name, mux)` tuple
persisted in a sibling `[[pins.entries]]` TOML table that renders as a
first-class dashboard row whether or not a live session realizes it,
binds 1:1 on the mux native name through the existing mux-to-agent-
session attribution pipeline (ADR 0006 / ADR 0028 / ADR 0046 / ADR
0047 / ADR 0048), and launches via new `TmuxRunner` mutation methods
plus the existing CSP-169 exec-replace attach.

Pins replace the agent-deck "new card" workflow without inheriting the
broader orchestrator scope. They compose with — rather than replace —
ADR 0014 declared links and ADR 0029 aliases: `display_name` is the
overlay alias for the bound session, ambiguity overrides write a
`LocalDeclared linked_to_mux` link tagged with the pin id, and the
sibling TOML tables share store-selection rules.

Dependency shape inside the workstream:

```
CSP-361 (ADR) ────┬─→ CSP-362 ────┬─→ CSP-363 ────→ CSP-364 ────┬─→ CSP-376 ────→ CSP-377
                  │               │                              │
                  │               └─→ CSP-365 ────→ CSP-366 ────→│
                  │                                              ├─→ CSP-369 ────→ CSP-373
                  └─→ CSP-370 ────→ CSP-371 ────→ CSP-372 ───────┤              ──→ CSP-374
                                                                 │              ──→ CSP-375
                                                                 └─→ CSP-378
                                          CSP-387  CSP-388  CSP-389 (TUI CRUD parity)
                                          CSP-367 ────→ CSP-368 (CLI read path)
                                          CSP-379  CSP-380  CSP-381 (closeout)
```

`CSP-361` (the ADR) is unblocked; `CSP-362` (schema + TOML) and
`CSP-370` (TmuxRunner extensions) can land in parallel after it.
`CSP-364` (resolver binding) is the integration spine that the TUI
and launch stories converge on. `CSP-377` provides the immediate
row-level actions; `CSP-387..389` bring the Controls overlay to
CLI-parity for create / edit / remove / bind / rebind / adopt. The
closeout stories (`CSP-379..381`) document and lock in the surface
once everything else has landed.

- **CSP-361** ADR: session pin schema, binding, and launch contract

- **CSP-362** Pin schema + TOML round-trip

- **CSP-363** Load pins into discovery as GraphLink candidates

- **CSP-364** Resolver binding pass

- **CSP-365** Extend store selection for pin writes

- **CSP-366** Atomic write helpers for `[pins]`

- **CSP-367** Pin CLI command tree skeleton

- **CSP-368** CLI `pin list` and `pin show`

- **CSP-369** CLI `pin create` / `rename` / `rm`

- **CSP-370** Extend `TmuxRunner` with launch mutation seams

- **CSP-371** `HarnessAdapter::launch_argv` defaults

- **CSP-372** CLI `pin launch` and `pin attach`

- **CSP-373** CLI `pin bind` (PinAmbiguous override)

- **CSP-374** CLI `pin rebind` (external-rename recovery)

- **CSP-375** CLI `pin adopt`

- **CSP-376** TUI row tree integration

- **CSP-377** TUI keybindings for pin actions

- **CSP-378** Pin diagnostic surfaces in the TUI

- **CSP-387** TUI pin create flow

- **CSP-388** TUI pin edit and remove flow

- **CSP-389** TUI pin bind / rebind / adopt flows

- **CSP-379** Read-only invariant audit

- **CSP-380** Snapshot and JSON coverage

- **CSP-381** Docs and operations guide

#### Pinning TUI improvements (H-PIN-TUI-*)

The v1 pin surface reached CLI parity, but operator feedback from
using the TUI create form shows the modal is still too raw to be a
good daily workflow. The common creation cases are:

1. pin the selected live entry exactly, which may be better framed as
   `adopt`;
2. create a minor variation of the selected entry, usually same
   harness + cwd/workspace but a fresh mux/session;
3. start from a mostly blank slate, which is the least common case.

The first improvement pass should keep the implementation grounded in
those workflows: selected-row context drives defaults, one user-facing
"name" drives derived ids/mux names until the operator overrides them,
long fields remain inspectable while editing, path and harness fields
offer live choices without forbidding free-form input, launch argv is
previewed as the command that will actually run, and successful create
flows move focus to the resulting pin row with a toast that
clarifies whether anything was launched. Pinning has not seen active
operator adoption yet, so this workstream should optimize for the
right TUI workflow rather than preserving the old schema-shaped form
layout or awkward key semantics.

- **CSP-452** Pin create usability map and terminology pass

- **CSP-453** Name-driven defaults and override tracking for pin create

- **CSP-454** Make pin form fields editable at real-world lengths

- **CSP-455** Hybrid cwd omnibox for pin create/adopt

- **CSP-455.01** Explicit row/edit focus for pin create navigation

- **CSP-456** Harness picker with free-form escape hatch

- **CSP-457** Launch argv editor with resolved command preview

- **CSP-457.01** Harness launch option mappings for pin create

- **CSP-458** Post-create/adopt focus and toast behavior

- **CSP-459** Float pinned entities and keep Pins groups open

- **CSP-460** Promote pins to first-class graph entities

- **CSP-461** Project pins as placeholder session and mux entities

- **CSP-528** Numeric-suffix auto-increment for derived pin names

#### Deferred follow-ups (post-v1)

These are explicitly out of v1 scope but recorded so the design
surface stays coherent. Each is documented in ADR 0057's Open
Questions Deferred section.

- **CSP-382** Tmux non-default socket discovery enumeration

- **CSP-383** Lifecycle hooks beyond `launch.argv`

- **CSP-384** Importers from tmuxinator / tmuxp / smug configs

- **CSP-385** Glob/wildcard pin patterns

- **CSP-386** Absolute tmux socket paths (`tmux -S <path>`)

#### Session Continuity (H-PIN-RESUME-*)

Settled by ADR 0058 (Accepted). Each discovery cycle writes the
most recent fresh `(pin_id, mux_name, session_id, harness,
observed_epoch)` binding to a per-pin sidecar under
`$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`. On `pin
launch`, when the resolver returns `PinUnbound`, the launch path
consults the sidecar, walks the ADR 0018 `parent_session` chain
forward to the current head (stopping at any fork), validates the
session still exists on disk, and splices
`HarnessAdapter::resume_argv(session_id, cwd)` into the tmux
new-session call. The sidecar is a rebuildable cache — the resolver
never reads it; stale entries are deleted at launch time when their
recorded session can no longer be found.

Dependency shape:

```
CSP-395 (sidecar I/O) ───────────┬─→ CSP-397 (write pass)
                                 │
CSP-396 (resume_argv) ───────────┴─→ CSP-398 (launch consumer + lineage walk)
                                              │
                                              ├─→ CSP-399 (PinUnbound extension + TUI/CLI surfaces)
                                              │
                                              └─→ CSP-400 (invariants + snapshots + closeout)
```

- **CSP-395** Sidecar schema + atomic I/O helpers

- **CSP-396** `HarnessAdapter::resume_argv` method + defaults

- **CSP-397** Sidecar write pass post-resolve

- **CSP-398** Launch decision tree: sidecar consumer + lineage walk

- **CSP-399** `PinUnbound` diagnostic extension + UX surfaces

- **CSP-400** Invariants, snapshots, and closeout

#### Launch And Binding Reliability (H-PIN-FIX-*)

Bugs found running the deck day to day: one session bound to two
pins, a pin launch that failed with no visible reason, previews that
ignored a pin's live mux, stale state after returning from tmux, and
errors that only flashed on the status bar.

- **CSP-574** One-to-one pin bindings (ADR 0102)

- **CSP-575** Launches keep failed panes and confirm the start (ADR 0103)

- **CSP-576** Pin placeholder rows preview their live mux

- **CSP-577** Refresh after tmux hand-offs shows current state (ADR 0104)

- **CSP-578** TUI message log for operation outcomes (ADR 0105)

### AI Session Naming

Sibling workstream to `H-RENAME-*`. Layers AI-driven name suggestions on
top of the alias write surface. Filed lightly so the follow-up is tracked
without diluting the rename workstream — the AI track touches dependencies,
network IO, privacy posture, and possibly an async runtime, all of which
deserve their own ADR before any story lands.

- **CSP-245** ADR: provider, dependency, privacy, dispatch
- **CSP-246** Transcript context extractor
- **CSP-247** CLI + TUI suggest surface
- **CSP-248** Optional auto-suggest hook

### Subagent Session Filtering

OpenCode subagent invocations (`@explore`, `@general`) create persistent
`AgentSession` rows in the SQLite store. These sessions appear alongside
human-driven sessions in the TUI session list, cluttering the view and
diluting the signal of the operator's actual work. Every subagent session
carries a `parent_id` pointing back to the invoking human session, and the
openCode convention names them with the subagent type in the title
(e.g. `Find exact_cwd_match code (@explore subagent)`).

The desired behavior is to either nest subagent sessions under their
human session in the row tree, or hide them behind a toggle that defaults
to collapsed/filtered. The resolver/mux pipeline should treat a subagent
session's cwd/activity as owned by the parent when resolving mux
attachments (a subagent running in a tmux pane should map to the human
session that invoked it, not pollute mux resolution for that pane).

Dependency shape inside the workstream:

```
CSP-295 ─────────→ CSP-296 ─────────→ CSP-297
                                       └──→ CSP-298
```

- **CSP-295** Determine how to detect subagent sessions from opencode state
- **CSP-296** Thread subagent metadata into the graph model
- **CSP-297** Filter and nest subagent sessions in the TUI
- **CSP-298** Suppress subagent sessions from mux attachment resolution

### TUI Pass-2 Revisions (H-UI-*)

A fresh pass over the rendered showcase (ADR 0070) surfaced three
revisions to existing TUI work. Tracking them here so the followups
do not get lost inside their originating workstreams.

- **CSP-415** Collapse per-session mux chip to an attachable-binary; let group rows own the ambiguity signal

- **CSP-416** Weave per-node-kind glyph identity through every TUI surface (tree, detail, filter, help)

- **CSP-417** Roll back the detail-pane upstream/downstream split; render a single related-entities list with descriptive edge labels

- **CSP-418** Audit the sessions-pane header content holistically

- **CSP-419** Resolved-vs-candidate visual separation in the detail-pane explorer

- **CSP-420** Resolver-side preservation for suppressed ambiguous `LinkedToMux` resolutions

- **CSP-421** Renderer-side fallback so candidate fan-out flags the explorer group as ambiguous even when no `ResolvedRelationship`…

- **CSP-422** Left-pane tree views consume resolved relationships only

- **CSP-581** Related view at neighbor granularity; split corroborating from competing candidates (ADR 0107)

### TUI Widget Ecosystem Adoption (H-WIDG-*)

Posture shift: the TUI carries ~5.2k LOC of in-house widget code
across `src/tui/widgets/` (badge, controls, help, input,
multi_select, pins, search, toast, value_modal). Recent hardening
cycles (CSP-415..422, fix(tui) commits on narrow-pane truncation,
wrap-attached chips, scrollbar columns) have repeatedly traced
defects back to bespoke primitives. This workstream commits to a
more dep-friendly posture: prefer well-maintained ratatui-ecosystem
crates over in-tree reimplementations where the seam is clean, with
ADR 0067 snapshot tests as the regression net per swap.

Adoption tiers (cribbed from the dep landscape audit):

- **Tier A** — high-confidence drop-ins that retire substantial
  in-tree LOC. Each is conceptually 1:1 with an existing widget;
  effort dominated by snapshot-fixture regeneration and ADR 0032
  theme glue.
- **Tier B** — capability-add adoptions for surfaces Conspectus
  does not have yet. Gated on a concrete trigger story.
- **Tier C** — strategic kit evaluation. `rat-widget` is the one
  upstream that could plausibly absorb several in-tree forms under
  one design system. Spike before commit.
- **Tier D** — explicit pass list (frameworks, gated pickers,
  tree-widget extraction). Recorded so future audits don't
  re-relitigate.

Dependency shape:

```
CSP-425 (macros cleanup) ─────┬─→ CSP-426 (multi_select → ratatui-cheese)
                              ├─→ CSP-427 (toast → ratatui-toaster)
                              ├─→ CSP-428 (overlay framing → tui-popup)
                              ├─→ CSP-429 (help → ratatui-cheese.help)
                              ├─→ CSP-430 (tui-textarea, gated)
                              ├─→ CSP-431 (tui-skeleton, gated on CSP-184)
                              ├─→ CSP-432 (throbber-widgets-tui, gated)
                              └─→ CSP-433 (rat-widget kit spike)
                                          │
                                          ├─→ CSP-434 (ratatui-explorer cwd picker, gated)
                                          └─→ CSP-435 (tui-tree-widget explorer extract, deferred)
```

Cross-cutting expectations across every Tier A swap:

- One focused PR per swap; snapshot fixtures regenerate in the
  same commit so the review reads the visual delta directly.
- **ADR 0032 `[tui.theme]` glue lands in-scope of the swap, not as
  a follow-up.** Estimate 50–100 LOC of bridge code per swap
  (typically a `*_styles_from_theme(&Theme)` helper plus a
  `.theme(&Theme)` builder on the widget surface, threaded to the
  call site). CSP-426's two-commit sequence (functional swap →
  theme glue) was the calibration; subsequent swaps land both in
  one PR with the visual-verification snapshot in the commit.
- License posture preserved (MIT or MIT/Apache-2.0 only — no
  copyleft adoptions without an ADR).
- Snapshot-replayable reducer (ADR 0067) stays the source of truth;
  no widget that owns the event loop is adopted into the runtime.

- **CSP-425** Adopt `ratatui-macros` for `Span` / `Line` / `Text` / layout boilerplate

- **CSP-426** Swap `widgets/multi_select.rs` for `ratatui-cheese.multi_select`

- **CSP-427** Swap `widgets/toast.rs` for `ratatui-toaster`

- **CSP-428** Replace bordered-frame overlay code with `tui-popup`

- **CSP-429** Adopt `ratatui-cheese.help` for the `?` help overlay

- **CSP-430** Adopt `tui-textarea` when a multi-line input field lands on the backlog

- **CSP-431** Adopt `tui-skeleton` for background-load placeholders

- **CSP-432** Adopt `throbber-widgets-tui` for in-flight spinners

- **CSP-433** Spike: evaluate `rat-widget` as a cohesive widget kit

- **CSP-434** `ratatui-explorer` cwd picker for pin `create` / `adopt`

- **CSP-435** `tui-tree-widget` extraction for the explorer

- **CSP-436** Opportunistic `ratatui-macros` sweep across the remaining small widgets and the layout helpers

- **CSP-437** Port high-variant widgets into the `examples/pantry.rs` ingredient list (CSP-424 follow-up)

#### TUI Widget Ecosystem — explicit pass list (Tier D)

Recorded so future audits do not re-relitigate.

- `tuirealm`, `widgetui`, `tui-react` — framework-tier;
  React/Elm/Bevy-style component models. Conflict with
  ADR 0067's snapshot-replayable reducer. Pass.
- `rat-salsa` (framework, distinct from `rat-widget` above) —
  event queue + tasks + timers; owns the loop. Same conflict.
  Pass.
- `rat-widget` — closed in `CSP-433` after structural
  analysis. The kit-shaped posture forces the
  rat-event / rat-focus / rat-scrolled trifecta into widget
  state types as mandatory fields; partial adoption is
  structurally infeasible. Re-evaluate a single primitive (not
  the form widget) only if a future surface genuinely needs a
  kit-shaped one.
- `ratatui-interact` — pure-compose, mouse hit-testing, focus
  manager. Steal the mouse hit-testing *pattern* if mouse support
  ever becomes a goal; do not take the framework — ~70% of its
  widget catalog duplicates the in-tree surface.
- `tui-overlay` — interesting "drawer / modal / popover / toast
  from a single primitive" abstraction, but pre-1.0 (v0.1.2) and
  `tui-popup` is the more conservative pick. Re-evaluate once
  `tui-overlay` reaches 0.4+.
