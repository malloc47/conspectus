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
| [0084](0084-first-class-pin-nodes.md) | First-Class Pin Nodes | Accepted (amended by 0141) |

## Discovery and attribution

How providers read local state, and how agent sessions get attributed to tmux panes without touching the agents.

| ADR | Decision | Status |
| --- | --- | --- |
| [0011](0011-forge-discovery-via-gh.md) | Forge Discovery Via The `gh` CLI | Accepted |
| [0013](0013-opencode-sqlite-session-store.md) | Read opencode Sessions From SQLite | Accepted |
| [0027](0027-workspace-detection-precedence.md) | Workspace Detection And Provider Precedence | Accepted |
| [0028](0028-hook-sidecar-mux-attribution.md) | Hook Sidecar Mux Attribution | Accepted (refined by 0146) |
| [0046](0046-process-tree-pane-linker.md) | Process-Tree Pane Linker | Accepted |
| [0048](0048-codex-state-and-log-readers.md) | Codex State and Log Readers for Mux Attribution | Accepted (amended) |
| [0049](0049-opencode-hook-plugin-distribution.md) | OpenCode Hook Plugin Distribution | Accepted |
| [0121](0121-pi-harness-support.md) | Pi Harness Support | Accepted |
| [0060](0060-agent-deck-multi-repo-workspace-adapter.md) | Agent-Deck Multi-Repo Workspace Adapter | Accepted |
| [0066](0066-agent-deck-instance-titles-as-workspace-names.md) | Agent-Deck Instance Titles As Workspace Display Names | Accepted |
| [0088](0088-provider-descriptor-registry.md) | Provider Descriptor Registry | Accepted |
| [0089](0089-mux-backend-trait.md) | Mux Backend Trait | Accepted |
| [0111](0111-launch-directory-is-optional.md) | The Launch Directory Is An Optional Input | Accepted |
| [0138](0138-provider-workspaces-from-observed-cwds.md) | Provider Workspaces From Observed Cwds | Accepted |

## User intent and the write envelope

Where user-authored intent lives, what Conspectus may write, and what it must never touch.

| ADR | Decision | Status |
| --- | --- | --- |
| [0012](0012-config-file-layout.md) | Conspectus Configuration File Layout | Accepted |
| [0014](0014-declared-link-storage-schema.md) | Declared Link Storage Schema | Accepted |
| [0029](0029-session-alias-overlay.md) | Session Alias Overlay | Accepted |
| [0086](0086-payload-privacy-tenet.md) | Payload Privacy Tenet | Accepted (amended by 0140, 0147, 0148, 0149) |
| [0087](0087-mutation-envelope.md) | Mutation Envelope | Superseded by 0143 |
| [0090](0090-pin-store-registry-sidecar.md) | Pin-Store Registry Sidecar | Superseded by 0141 |
| [0143](0143-mutation-envelope-with-history-pins-and-clone.md) | Mutation Envelope With History, Pin Slots, And Clone | Accepted (amended by 0148) |

## History, pins, mux lifecycle, and worktrees

What Conspectus remembers about entities that are gone, the slots the operator pins, rebuilding, tmux session creation and teardown, and worktree streams.

| ADR | Decision | Status |
| --- | --- | --- |
| [0057](0057-session-pins.md) | Session Pins | Superseded by 0140 and 0141 |
| [0058](0058-pin-session-continuity.md) | Pin Session Continuity | Superseded by 0140 |
| [0092](0092-worktree-backend-seam.md) | Worktree Management Backend Seam | Accepted |
| [0093](0093-operator-initiated-mux-teardown.md) | Operator-Initiated Mux Teardown (`kill-session`) | Accepted (amended by 0142) |
| [0094](0094-worktree-backed-pins-realized-at-launch.md) | Worktree-Backed Pins Realized at Launch | Superseded by 0142 |
| [0095](0095-bare-mux-session-creation.md) | Bare Mux Session Creation (No Pin, No Agent) | Accepted (amended by 0142) |
| [0096](0096-ephemeral-harness-mux-launch.md) | Mux Launch — Harness in a New Mux Without a Pin | Accepted (amended by 0142 and 0171, narrowed by 0169) |
| [0097](0097-launch-spec-form-primitive.md) | Shared Launch-Spec Form Primitive | Superseded by 0169 |
| [0098](0098-pin-resume-preserves-launch-argv.md) | Pin Resume Preserves the Pin's Launch Argv | Accepted (amended by 0142) |
| [0102](0102-pin-bindings-are-one-to-one.md) | Pin Bindings Are One-To-One | Superseded by 0140 |
| [0103](0103-launches-keep-failed-panes.md) | Launches Keep Failed Panes And Confirm The Harness Started | Accepted |
| [0140](0140-history-and-presence.md) | History And Presence | Accepted |
| [0141](0141-pin-slots.md) | Pin Slots | Accepted |
| [0142](0142-rebuild-plans-and-executors.md) | Rebuild Plans And Executors | Accepted |

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
| [0146](0146-the-hook-spool-is-the-durable-hook-record.md) | The Hook Spool Is The Durable Hook Record | Accepted |
| [0147](0147-session-content-search.md) | Session Content Search | Accepted (amended by 0149) |
| [0148](0148-opt-in-transcript-search-index.md) | Opt-In Transcript Search Index | Accepted (decided, not built) |
| [0149](0149-two-level-search-current-and-wider-context.md) | Two-Level Search: Current Context And Wider Context | Accepted (amended by 0157, 0158) |

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
| [0030](0030-tui-text-input-primitive.md) | TUI Text-Input Primitive | Accepted (narrowed by 0165, 0166) |
| [0031](0031-tui-filter-view-switching-and-per-view-state.md) | TUI Filter, View Switching, And Per-View State | Accepted (amended by 0147, 0149, 0173, 0176) |
| [0032](0032-tui-theme-config.md) | TUI Theme Configuration | Accepted (amended by 0153, 0159) |
| [0033](0033-tui-detail-pane-sections.md) | TUI Detail-Pane Section Model | Superseded by 0074 |
| [0034](0034-repo-row-display-path-preference.md) | Repo Row Display Path Preference | Accepted (amended) |
| [0045](0045-sessions-graph-lineage-and-repo-default.md) | Sessions Graph Lineage And Repo Default | Accepted |
| [0055](0055-sessions-graph-default.md) | Sessions Graph Default | Accepted |
| [0056](0056-tui-clipboard-backend-osc52.md) | TUI Clipboard Backend via OSC 52 | Accepted (amended by 0163) |
| [0061](0061-drop-workspace-grouping-from-non-workspaces-views.md) | Drop Workspace Grouping From Non-Workspaces Views | Accepted (mux view refined by 0139) |
| [0062](0062-workspaces-view-polish.md) | Workspaces View Polish — Inline Members, Drop Related | Superseded by 0065 |
| [0063](0063-drop-workspace-cross-reference-chip.md) | Drop The Workspace Cross-Reference Chip | Accepted |
| [0064](0064-hybrid-sessions-graph-grouping.md) | Hybrid Workspace+Repo Grouping In Sessions/Graph | Accepted |
| [0065](0065-sessions-workspace-grouping-replaces-workspaces-view.md) | Workspace Grouping In Sessions Replaces The Workspaces View | Accepted |
| [0071](0071-ambiguous-mux-scope-and-detail.md) | Ambiguous Mux Surface Moves From Session Row To Group Detail | Accepted (detail-pane section superseded by 0074) |
| [0072](0072-mux-indicator-attachable-binary.md) | Mux Indicator Becomes Attachable-Binary; Group Rows Own The Ambiguity Glyph | Accepted (amended by 0145) |
| [0120](0120-mux-rows-drop-the-ambiguity-glyph.md) | Mux Rows Drop The Ambiguity Glyph | Accepted |
| [0073](0073-node-kind-visual-identity.md) | Per-Node-Kind Visual Identity (Glyph + Color) | Accepted (amended by 0144, 0145, 0154) |
| [0074](0074-detail-pane-related-entities-flatten.md) | Detail-Pane Related-Entities Flatten | Accepted |
| [0075](0075-edge-state-visual-language.md) | Edge-State Visual Language In The Detail-Pane Other Zone | Accepted |
| [0076](0076-scrollbar-widget-choice.md) | Scrollbar Widget Choice For Scrolled Panes | Accepted (amended by 0155, 0156) |
| [0078](0078-tui-surface-division-of-labor.md) | TUI Surface Division Of Labor | Accepted |
| [0085](0085-tui-mvu-architecture.md) | TUI Elm/MVU Architecture | Accepted (amended by 0156, 0161) |
| [0105](0105-tui-message-log.md) | TUI Message Log For Operation Outcomes | Accepted (amended by 0163, 0179) |
| [0106](0106-preview-pane-wrap-modes.md) | Preview Pane Wrap Modes | Accepted (amended by 0155) |
| [0107](0107-related-view-neighbor-granularity.md) | Related View At Neighbor Granularity | Accepted (amended) |
| [0112](0112-node-specific-previews.md) | Node-Specific Previews | Accepted (see 0147) |
| [0113](0113-readme-screenshots.md) | README Screenshots From The Showcase Fixture | Accepted |
| [0114](0114-prs-view-grouped-by-checkout.md) | A PRs View Grouped By Checkout | Accepted |
| [0115](0115-forge-discovery-over-discovered-repos.md) | Forge Discovery Over Every Discovered Repo | Accepted |
| [0116](0116-lazy-forge-cadence-and-hard-refresh.md) | Lazy Forge Cadence And A Hard Refresh | Accepted |
| [0117](0117-capability-manifest-and-traced-backstop.md) | A Capability Manifest With A Traced Backstop | Accepted |
| [0118](0118-csp-short-name.md) | `csp` Is The Short Name | Accepted |
| [0119](0119-compass-rose-logo.md) | The Compass-Rose Logo | Accepted |
| [0123](0123-overlay-keymap-tables.md) | Overlay Keymap Tables | Accepted (amended by 0160, 0161) |
| [0139](0139-mux-workspace-or-repo-grouping.md) | Workspace-Or-Repo Grouping In The Mux View | Accepted (amended) |
| [0144](0144-pinned-and-history-views.md) | Pinned And History Views | Accepted (amended by 0145) |
| [0145](0145-state-glyphs-and-words-in-the-left-panel.md) | State Glyphs And Words In The Left Panel | Accepted |
| [0150](0150-tui-framework-workspace-crate.md) | The TUI Framework Is A Workspace Member Crate | Accepted |
| [0151](0151-tui-framework-control-set-and-house-rules.md) | The Framework Control Set And Its House Rules | Accepted |
| [0152](0152-tui-framework-dependency-ownership.md) | Who Owns The Framework's Third-Party Crates | Accepted |
| [0153](0153-tui-theme-roles-aliases-and-named-styles.md) | TUI Theme Roles, Alias Keys, And Named Styles | Accepted (amended by 0163) |
| [0154](0154-tui-glyph-marks-vetted-table-and-scan.md) | Shared Glyph Marks, The Vetted Table, And The Source Scan | Accepted (amended by 0168, 0179) |
| [0155](0155-tui-text-measured-cut-and-wrapped-in-cells.md) | Text Is Measured, Cut, And Wrapped In Terminal Cells By One Module | Accepted |
| [0156](0156-tui-scroll-window-follow-rule-and-viewports-by-value.md) | One Scroll Window, One Follow Rule, And Viewports Returned By Value | Accepted (amended by 0161, 0177) |
| [0157](0157-tui-chips-one-width-rule-and-ages-one-ladder.md) | Chips Share One Width Rule, And Ages One Ladder | Accepted |
| [0158](0158-tui-modals-one-frame-and-named-sizes.md) | Modals Share One Frame And Five Named Sizes | Accepted (amended by 0166, 0174) |
| [0159](0159-tui-lists-show-their-cursor-one-way.md) | Lists Show Their Cursor One Way And Step By Their Kind | Accepted |
| [0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md) | Overlay Keys Share One Model, One Character Rule, And One Footer Grammar | Accepted (amended by 0168, 0172, 0173, 0174, 0176, 0177, 0179) |
| [0161](0161-tui-overlays-draw-themselves-on-one-host-owned-stack.md) | Overlays Draw Themselves On One Host-Owned Modal Stack | Accepted (amended by 0172, 0176, 0177) |
| [0162](0162-tui-help-pages-one-page-generated-from-key-tables.md) | Help Pages Are One Page, Generated From The Key Tables | Accepted (amended by 0177, 0178) |
| [0163](0163-tui-feedback-one-type-four-severities-in-tree-toasts.md) | Feedback Is One Type With Four Severities, And Toasts Are In-Tree | Accepted (amended by 0179) |
| [0164](0164-tui-test-kit-key-scripts-frame-driver-goldens-conformance.md) | The Test Kit Drives Frames, Keeps Colors In Goldens, And Holds Every Modal To The Conventions | Accepted (amended by 0176, 0177) |
| [0165](0165-tui-text-entry-one-line-editor-and-one-text-field.md) | Text Entry Has One Line Editor And One Text Field | Accepted |
| [0166](0166-tui-text-prompts-one-overlay-that-explains-its-refusals.md) | Text Prompts Are One Overlay That Explains Its Refusals | Accepted (amended by 0176) |
| [0167](0167-tui-path-fields-complete-behind-a-source-and-expand-tilde-on-commit.md) | Path Fields Complete Behind A Source, Ask For A Kind, And Expand `~` On Commit | Accepted |
| [0168](0168-tui-forms-one-ring-submit-from-any-field-fixed-footer.md) | Forms Move On One Ring, Submit From Any Field, And Keep Their Keys In View | Accepted (amended by 0170) |
| [0169](0169-tui-launch-form-is-a-form-and-one-grammar-splits-every-argv.md) | The Launch Form Is A `Form`, And One Grammar Splits Every Argv | Accepted |
| [0170](0170-tui-pin-edit-form-is-a-form-with-override-rows.md) | The Pin Edit Form Is A `Form` With Override Rows | Accepted |
| [0171](0171-tui-menus-one-control-entries-as-data.md) | Menus Are One Control Whose Entries Are Data | Accepted (amended by 0172) |
| [0172](0172-tui-dialogs-one-control-and-follow-up-questions-are-steps.md) | Dialogs Are One Control, And A Menu's Questions Are Steps On The Stack | Accepted (amended by 0176) |
| [0173](0173-tui-checklists-one-control-select-all-none-and-a-count.md) | Checklists Are One Control That Selects All Or None And Counts | Accepted |
| [0174](0174-tui-palettes-one-control-a-query-over-ranked-results.md) | Palettes Are One Control, A Query Over Results The Host Ranks | Accepted (amended by 0175) |
| [0175](0175-tui-search-is-two-palettes-and-a-hit-is-one-column-plan.md) | Search Is Two Palettes, And A Hit's Cells Are One Column Plan | Accepted |
| [0176](0176-tui-settings-panels-apply-as-they-change-and-editors-are-steps.md) | Settings Panels Are One Control That Applies Each Change As It Is Made, And Their Editors Are Steps | Accepted |
| [0177](0177-tui-readers-are-one-pager-over-content-the-host-lays-out.md) | Readers Are One Pager Over Content The Host Lays Out | Accepted (amended by 0178) |
| [0178](0178-tui-help-pages-and-long-values-are-pager-contents.md) | Help Pages And Long Values Are Pager Contents, And A Framed Footer Scrolls What Does Not Fit | Accepted |
| [0179](0179-tui-log-lists-read-the-hosts-log-over-an-embedded-pager.md) | Log Lists Read The Host's Log, And The Detail Is An Embedded Pager | Accepted |

## Transcript viewer

Opening and rendering harness transcripts.

| ADR | Decision | Status |
| --- | --- | --- |
| [0019](0019-session-transcript-viewer.md) | Session Transcript Viewer Integration | Accepted (amended) |
| [0051](0051-tui-transcript-preview-markdown-dependency.md) | TUI Transcript Preview Markdown Rendering Dependency | Accepted (amended) |
| [0052](0052-native-session-viewer.md) | Native In-Tree Multi-Harness Session Transcript Viewer | Accepted |
| [0053](0053-viewer-abort-filter-default-harness-ux.md) | Aborted-Message Filtering In The Native Viewer | Accepted |
| [0122](0122-viewer-harness-hidden-records-toggle.md) | Harness-Hidden Records Behind A Viewer Toggle | Accepted |
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
| [0067](0067-tui-snapshot-mode-for-agent-iteration.md) | Dev-Only TUI Snapshot Mode For Agentic Iteration | Accepted (amended by 0164) |
| [0068](0068-snapshot-fixture-mode.md) | Fixture Mode For The Snapshot Tool | Accepted |
| [0069](0069-interactive-fixture-mode-for-tui.md) | Interactive Fixture Mode For The TUI | Accepted |
| [0070](0070-showcase-scenario.md) | Showcase Scenario For Comprehensive Functionality Exercise | Accepted |
| [0100](0100-comments-carry-rationale-not-backlog-ids.md) | Comments Carry Rationale, Not Backlog IDs | Accepted |
| [0124](0124-sibling-test-files.md) | Large Unit-Test Modules Live In Sibling Files | Accepted |
| [0125](0125-opencode-2-reference-findings.md) | OpenCode 2.x Reference Findings | Accepted |
| [0126](0126-opencode-2-pane-attribution.md) | OpenCode 2.x Session-To-Pane Attribution | Accepted |
| [0127](0127-launches-keep-the-tui-drawn-behind-a-spinner.md) | Launches Keep The TUI Drawn Behind A Spinner | Accepted |
| [0128](0128-github-hosts-gh-is-signed-in-to.md) | Forge Discovery Queries The GitHub Hosts `gh` Is Signed In To | Accepted |
| [0129](0129-pr-local-branch-symbol-filter-and-ranking.md) | PRs Show Whether They Have A Local Branch, With A Filter And A Ranking | Accepted |
| [0130](0130-pr-config-keys-and-no-checkout-catch-all.md) | PR Tracking Config Keys, And No "No checkout" Catch-All | Accepted |
| [0131](0131-pr-titles-and-the-pr-row-layout.md) | PR Titles And The PR Row Layout | Accepted |
| [0132](0132-refuse-actions-on-rows-awaiting-a-handoff-refresh.md) | Refuse Actions On Rows Awaiting A Hand-Off Refresh | Accepted |
| [0133](0133-launches-attach-before-they-refresh.md) | Launches Attach Before They Refresh | Accepted |
| [0134](0134-pr-state-filter.md) | A PR State Filter | Accepted |
| [0135](0135-pin-commands-skip-forge-discovery.md) | Pin Commands Skip Forge Discovery | Accepted |
| [0136](0136-lazy-forge-fetches-outside-the-writer-lock.md) | Lazy Forge Fetches Outside The Writer Lock | Accepted |
| [0137](0137-forge-fetches-stop-on-shutdown.md) | Forge Fetches Stop On Daemon Shutdown | Accepted |
