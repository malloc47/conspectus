# Architecture Decision Records

Every significant Conspectus decision is recorded here: model changes, new
dependencies, new write paths, workflow tools, and UI conventions. Each ADR
follows the same template: Status, Context, Decision, Consequences,
Alternatives Considered, and, where it applies, Open Questions Answered.
Retired decisions stay in place with a forward link to what replaced them.

To add one, take the next number, follow the template, link related
ADRs, and update `docs/design.md` when the decision changes the intended
model. The [ADR corpus audit](../adr-audit.md) reviews the set as a whole.

## Graph model, evidence, and resolution

The provider-neutral node model, evidence-preserving `GraphLink` candidates, and the resolver that picks preferred relationships without discarding the rest.

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-node-identity-and-stable-ids.md) | Node Identity And Stable IDs | Accepted |
| [0002](0002-graphlink-and-typed-relationships.md) | GraphLink And Typed Relationships | Accepted |
| [0003](0003-polymorphic-fork-node.md) | Polymorphic Fork Node | Accepted |
| [0004](0004-fork-context-relation-semantics.md) | Fork Context Relation Semantics | Accepted |
| [0005](0005-session-lineage-endpoints.md) | Session Lineage Endpoints | Accepted |
| [0006](0006-session-mux-link-candidates.md) | Session And Mux Link Candidates | Accepted |
| [0018](0018-intra-harness-session-lineage.md) | Intra-Harness Session Lineage | Accepted |
| [0026](0026-checkout-context-model.md) | Checkout Context Model | Accepted |
| [0041](0041-resolver-stays-in-rust.md) | Resolver Stays in Rust | Accepted |
| [0047](0047-runtime-process-nodes-candidate.md) | Runtime Process Nodes | Accepted |
| [0059](0059-resolver-rules-engine-evaluation.md) | Resolver Rules-Engine Evaluation | Accepted |
| [0077](0077-resolver-ambiguous-slot-preservation.md) | Resolver-Side Preservation Of Ambiguous Resolution Slots | Accepted |
| [0084](0084-first-class-pin-nodes.md) | First-Class Pin Nodes | Accepted |

## Discovery and attribution

How providers read local state, and how agent sessions get attributed to tmux panes without touching the agents.

| ADR | Decision | Status |
| --- | --- | --- |
| [0011](0011-forge-discovery-via-gh.md) | Forge Discovery Via The `gh` CLI | Accepted |
| [0013](0013-opencode-sqlite-session-store.md) | Read opencode Sessions From SQLite | Accepted |
| [0027](0027-workspace-detection-precedence.md) | Workspace Detection And Provider Precedence | Accepted |
| [0028](0028-hook-sidecar-mux-attribution.md) | Hook Sidecar Mux Attribution | Accepted |
| [0046](0046-process-tree-pane-linker.md) | Process-Tree Pane Linker | Accepted |
| [0048](0048-codex-state-and-log-readers.md) | Codex State and Log Readers for Mux Attribution | Accepted (amended) |
| [0049](0049-opencode-hook-plugin-distribution.md) | OpenCode Hook Plugin Distribution | Accepted |
| [0060](0060-agent-deck-multi-repo-workspace-adapter.md) | Agent-Deck Multi-Repo Workspace Adapter | Accepted |
| [0066](0066-agent-deck-instance-titles-as-workspace-names.md) | Agent-Deck Instance Titles As Workspace Display Names | Accepted |
| [0088](0088-provider-descriptor-registry.md) | Provider Descriptor Registry | Accepted |
| [0089](0089-mux-backend-trait.md) | Mux Backend Trait | Accepted |

## User intent and the write envelope

Where user-authored intent lives, what Conspectus may write, and what it must never touch.

| ADR | Decision | Status |
| --- | --- | --- |
| [0012](0012-config-file-layout.md) | Conspectus Configuration File Layout | Accepted |
| [0014](0014-declared-link-storage-schema.md) | Declared Link Storage Schema | Accepted |
| [0029](0029-session-alias-overlay.md) | Session Alias Overlay | Accepted |
| [0086](0086-payload-privacy-tenet.md) | Payload Privacy Tenet | Accepted |
| [0087](0087-mutation-envelope.md) | Mutation Envelope | Accepted |
| [0090](0090-pin-store-registry-sidecar.md) | Pin-Store Registry Sidecar | Accepted |

## Pins, mux lifecycle, and worktrees

Declared logical sessions, tmux session creation and teardown, and worktree streams.

| ADR | Decision | Status |
| --- | --- | --- |
| [0057](0057-session-pins.md) | Session Pins | Accepted |
| [0058](0058-pin-session-continuity.md) | Pin Session Continuity | Accepted |
| [0092](0092-worktree-backend-seam.md) | Worktree Management Backend Seam | Accepted |
| [0093](0093-operator-initiated-mux-teardown.md) | Operator-Initiated Mux Teardown (`kill-session`) | Accepted |
| [0094](0094-worktree-backed-pins-realized-at-launch.md) | Worktree-Backed Pins Realized at Launch | Accepted |
| [0095](0095-bare-mux-session-creation.md) | Bare Mux Session Creation (No Pin, No Agent) | Accepted |
| [0096](0096-ephemeral-harness-mux-launch.md) | Mux Launch — Harness in a New Mux Without a Pin | Accepted |
| [0097](0097-launch-spec-form-primitive.md) | Shared Launch-Spec Form Primitive | Accepted |
| [0098](0098-pin-resume-preserves-launch-argv.md) | Pin Resume Preserves the Pin's Launch Argv | Accepted |
| [0102](0102-pin-bindings-are-one-to-one.md) | Pin Bindings Are One-To-One | Accepted |
| [0103](0103-launches-keep-failed-panes.md) | Launches Keep Failed Panes And Confirm The Harness Started | Accepted |

## Persistence, daemon, and performance

The continuous server, the snapshot format, and the SQLite arc that was built and then retired (ADRs 0036–0044, superseded by 0082).

| ADR | Decision | Status |
| --- | --- | --- |
| [0035](0035-graph-to-view-query-layer.md) | Graph-to-View Query Layer | Accepted (amended) |
| [0036](0036-embedded-query-engine-selection.md) | Embedded Query Engine Selection | Superseded by 0082 |
| [0037](0037-snapshot-persistence-sqlite.md) | Snapshot Persistence Under SQLite | Superseded by 0082 |
| [0038](0038-cli-server-transport-wal.md) | CLI / Server Transport Under WAL | Partially superseded by 0082 |
| [0039](0039-query-feature-gate.md) | Library API Query Feature Gate | Superseded by 0082 |
| [0040](0040-distribution-amendment-sqlite.md) | Distribution Policy Amendment for SQLite | Superseded by 0082 |
| [0042](0042-vector-search-sqlite-vec.md) | Vector Search Via `sqlite-vec` | Superseded by 0082 |
| [0043](0043-sqlite-as-consumer-surface.md) | SQLite As The Sole Consumption Surface For Views And Renderers | Partially superseded by 0082 |
| [0044](0044-nodeid-foreign-references-as-json.md) | NodeId Foreign References Persisted as JSON | Superseded by 0082 |
| [0079](0079-server-intervals-dual-role-as-warm-start-ttl.md) | `[server.intervals]` Dual Role As CLI Warm-Start TTL | Accepted |
| [0080](0080-daemon-signal-handling.md) | Daemon Signal Handling Via `signal-hook` | Accepted |
| [0081](0081-filesystem-watcher-dependency.md) | Filesystem Watcher Dependency For Event-Driven Refresh | Accepted |
| [0082](0082-retire-sqlite-persistence-and-query-surface.md) | Retire SQLite Persistence And Query Surface | Accepted |
| [0083](0083-zero-copy-snapshot-format.md) | Zero-Copy Snapshot Format Selection | Accepted |
| [0091](0091-serve-idle-cost-and-class-gated-mutators.md) | Serve Idle Cost And Class-Gated Mutators | Accepted |
| [0099](0099-caller-owned-discovery-caches.md) | Caller-Owned Discovery Caches | Accepted |
| [0104](0104-tui-rescans-live-classes-after-tmux-handoffs.md) | The TUI Rescans Live Classes After Tmux Hand-Offs | Accepted |
| [0108](0108-handoff-refreshes-run-in-the-background.md) | Hand-Off Refreshes Run In The Background | Accepted |

## CLI output and exports

Tables, color, previews, and graph visualization.

| ADR | Decision | Status |
| --- | --- | --- |
| [0020](0020-width-aware-table-rendering.md) | Width-Aware Table Rendering | Accepted |
| [0021](0021-table-command-and-config-schema.md) | `conspectus table <ROWS>` Command And Config Schema | Accepted |
| [0022](0022-terminal-color-and-styling.md) | Terminal Color And Styling For Table Output | Accepted |
| [0023](0023-agent-session-last-message-preview.md) | Agent Session Last-Message Preview | Accepted |
| [0050](0050-graph-visualization-exports.md) | Graph Visualization Exports | Accepted |

## Interactive TUI

Runtime architecture, controls, theming, detail pane, and visual language.

| ADR | Decision | Status |
| --- | --- | --- |
| [0024](0024-tui-runtime-and-architecture.md) | TUI Runtime, App Architecture, And Dependency Policy | Accepted |
| [0025](0025-tui-preview-ansi-rendering-dependency.md) | TUI Preview ANSI Rendering Dependency | Accepted |
| [0030](0030-tui-text-input-primitive.md) | TUI Text-Input Primitive | Accepted |
| [0031](0031-tui-filter-view-switching-and-per-view-state.md) | TUI Filter, View Switching, And Per-View State | Accepted |
| [0032](0032-tui-theme-config.md) | TUI Theme Configuration | Accepted |
| [0033](0033-tui-detail-pane-sections.md) | TUI Detail-Pane Section Model | Accepted |
| [0034](0034-repo-row-display-path-preference.md) | Repo Row Display Path Preference | Accepted |
| [0045](0045-sessions-graph-lineage-and-repo-default.md) | Sessions Graph Lineage And Repo Default | Accepted |
| [0055](0055-sessions-graph-default.md) | Sessions Graph Default | Accepted |
| [0056](0056-tui-clipboard-backend-osc52.md) | TUI Clipboard Backend via OSC 52 | Accepted |
| [0061](0061-drop-workspace-grouping-from-non-workspaces-views.md) | Drop Workspace Grouping From Non-Workspaces Views | Accepted |
| [0062](0062-workspaces-view-polish.md) | Workspaces View Polish — Inline Members, Drop Related | Superseded by 0065 |
| [0063](0063-drop-workspace-cross-reference-chip.md) | Drop The Workspace Cross-Reference Chip | Accepted |
| [0064](0064-hybrid-sessions-graph-grouping.md) | Hybrid Workspace+Repo Grouping In Sessions/Graph | Accepted |
| [0065](0065-sessions-workspace-grouping-replaces-workspaces-view.md) | Workspace Grouping In Sessions Replaces The Workspaces View | Accepted |
| [0071](0071-ambiguous-mux-scope-and-detail.md) | Ambiguous Mux Surface Moves From Session Row To Group Detail | Accepted |
| [0072](0072-mux-indicator-attachable-binary.md) | Mux Indicator Becomes Attachable-Binary; Group Rows Own The Ambiguity Glyph | Accepted |
| [0073](0073-node-kind-visual-identity.md) | Per-Node-Kind Visual Identity (Glyph + Color) | Accepted |
| [0074](0074-detail-pane-related-entities-flatten.md) | Detail-Pane Related-Entities Flatten | Accepted |
| [0075](0075-edge-state-visual-language.md) | Edge-State Visual Language In The Detail-Pane Other Zone | Accepted |
| [0076](0076-scrollbar-widget-choice.md) | Scrollbar Widget Choice For Scrolled Panes | Accepted |
| [0078](0078-tui-surface-division-of-labor.md) | TUI Surface Division Of Labor | Accepted |
| [0085](0085-tui-mvu-architecture.md) | TUI Elm/MVU Architecture | Accepted |
| [0105](0105-tui-message-log.md) | TUI Message Log For Operation Outcomes | Accepted |
| [0106](0106-preview-pane-wrap-modes.md) | Preview Pane Wrap Modes | Accepted |
| [0107](0107-related-view-neighbor-granularity.md) | Related View At Neighbor Granularity | Accepted |

## Transcript viewer

Opening and rendering harness transcripts.

| ADR | Decision | Status |
| --- | --- | --- |
| [0019](0019-session-transcript-viewer.md) | Session Transcript Viewer Integration | Accepted (amended) |
| [0051](0051-tui-transcript-preview-markdown-dependency.md) | TUI Transcript Preview Markdown Rendering Dependency | Accepted (amended) |
| [0052](0052-native-session-viewer.md) | Native In-Tree Multi-Harness Session Transcript Viewer | Accepted |
| [0053](0053-viewer-abort-filter-default-harness-ux.md) | Aborted-Message Filtering In The Native Viewer | Accepted |
| [0054](0054-viewer-table-rendering-comfy-table.md) | Markdown Table Rendering Via comfy-table | Accepted |

## Process, tooling, and distribution

How the project is built, tested, tracked, and shipped, including the agent-oriented snapshot loop.

| ADR | Decision | Status |
| --- | --- | --- |
| [0007](0007-rust-development-approach.md) | Rust Development Approach | Accepted (amended) |
| [0008](0008-beads-work-tracking.md) | Beads Work Tracking | Superseded by 0009 |
| [0009](0009-lightweight-backlog-tracking.md) | Lightweight Backlog Tracking | Superseded by 0109 |
| [0109](0109-backlog-md-work-tracking.md) | Backlog.md Work Tracking | Accepted |
| [0010](0010-task-runner-selection.md) | Task Runner Selection | Accepted |
| [0015](0015-library-api-surface.md) | Library API Surface | Accepted (amended) |
| [0101](0101-typed-errors-on-the-library-facade.md) | Typed Errors On The Library Facade | Accepted |
| [0016](0016-distribution-policy.md) | Distribution Policy | Accepted (amended) |
| [0110](0110-pinned-rust-toolchain.md) | Pinned Rust Toolchain | Accepted |
| [0017](0017-repository-placement.md) | Repository Placement | Accepted |
| [0067](0067-tui-snapshot-mode-for-agent-iteration.md) | Dev-Only TUI Snapshot Mode For Agentic Iteration | Accepted |
| [0068](0068-snapshot-fixture-mode.md) | Fixture Mode For The Snapshot Tool | Accepted |
| [0069](0069-interactive-fixture-mode-for-tui.md) | Interactive Fixture Mode For The TUI | Accepted |
| [0070](0070-showcase-scenario.md) | Showcase Scenario For Comprehensive Functionality Exercise | Accepted |
| [0100](0100-comments-carry-rationale-not-backlog-ids.md) | Comments Carry Rationale, Not Backlog IDs | Accepted |
