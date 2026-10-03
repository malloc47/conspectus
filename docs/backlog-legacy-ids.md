# Backlog Legacy IDs

Backlog stories were renumbered from per-workstream IDs (`P8-014`,
`H-PIN-TUI-011`) to Backlog.md IDs (`CSP-NNN`) by the ADR 0109 migration.
Commit messages, pull requests, and agent transcripts from before the
migration cite the legacy IDs; this table maps them. Numbers follow the
order in which stories first landed on `main`. Lettered sub-stories became
subtasks of their parent (`T8-043a` became the first subtask of `T8-043`).

Each task file also records its legacy ID on a `Legacy ID:` line, so
`git grep -w P8-014 -- backlog/` finds the task too.

| ID | Legacy ID | First landed (UTC) | Commit | Title at migration |
| --- | --- | --- | --- | --- |
| `CSP-001` | `PLAN-001` | 2026-05-14 02:04 | 5d84bd6 | Convert `docs/design.md` into implementation phases and milestone-level stories |
| `CSP-002` | `PLAN-002` | 2026-05-14 02:04 | 5d84bd6 | Identify the first vertical slice for the Rust crate and CLI |
| `CSP-003` | `PLAN-003` | 2026-05-14 02:04 | 5d84bd6 | Define the fixture strategy for sparse graph and resolver tests |
| `CSP-004` | `P0-001` | 2026-05-15 02:31 | 7138299 | Add the Rust package skeleton |
| `CSP-005` | `P0-002` | 2026-05-15 02:31 | 7138299 | Add the thin CLI surface |
| `CSP-006` | `P0-003` | 2026-05-15 02:31 | 7138299 | Add baseline module boundaries |
| `CSP-007` | `P0-004` | 2026-05-15 02:31 | 7138299 | Add runtime and test dependencies |
| `CSP-008` | `P0-005` | 2026-05-15 02:31 | 7138299 | Add local check automation |
| `CSP-009` | `P0-006` | 2026-05-15 02:31 | 7138299 | Add project-foundation smoke tests |
| `CSP-010` | `P0-007` | 2026-05-15 02:31 | 7138299 | Verify the foundation end state |
| `CSP-011` | `P1-001` | 2026-05-15 02:36 | ad719f4 | Define graph node identity types |
| `CSP-012` | `P1-002` | 2026-05-15 02:36 | ad719f4 | Define typed node models |
| `CSP-013` | `P1-003` | 2026-05-15 02:36 | ad719f4 | Define GraphLink evidence types |
| `CSP-014` | `P1-004` | 2026-05-15 02:36 | ad719f4 | Define graph snapshot JSON output |
| `CSP-015` | `P1-005` | 2026-05-15 02:36 | ad719f4 | Add fixture builders for sparse graph scenarios |
| `CSP-016` | `P1-006` | 2026-05-15 02:36 | ad719f4 | Implement the resolver skeleton |
| `CSP-017` | `P1-007` | 2026-05-15 02:36 | ad719f4 | Implement resolver precedence rules |
| `CSP-018` | `P1-008` | 2026-05-15 02:36 | ad719f4 | Add `conspectus graph --format json` |
| `CSP-019` | `P1-009` | 2026-05-15 02:36 | ad719f4 | Add representative graph JSON snapshots |
| `CSP-020` | `P1-010` | 2026-05-15 02:36 | ad719f4 | Verify the Phase 1 end state |
| `CSP-021` | `P2-001` | 2026-05-15 03:58 | 93a3f8b | Define local discovery orchestration boundaries |
| `CSP-022` | `P2-002` | 2026-05-15 03:58 | 93a3f8b | Add read-only git command probes |
| `CSP-023` | `P2-003` | 2026-05-15 03:58 | 93a3f8b | Map git probes into graph nodes and candidate links |
| `CSP-024` | `P2-004` | 2026-05-15 03:58 | 93a3f8b | Add cwd and configured scan-root discovery inputs |
| `CSP-025` | `P2-005` | 2026-05-15 03:58 | 93a3f8b | Add generic workspace inference |
| `CSP-026` | `P2-006` | 2026-05-15 03:58 | 93a3f8b | Read Atelier workspace metadata |
| `CSP-027` | `P2-007` | 2026-05-15 03:58 | 93a3f8b | Read Atelier fork index metadata |
| `CSP-028` | `P2-008` | 2026-05-15 03:58 | 93a3f8b | Map Atelier forks into graph nodes and context-effect links |
| `CSP-029` | `P2-009` | 2026-05-15 03:58 | 93a3f8b | Wire local discovery into `graph --format json` |
| `CSP-030` | `P2-010` | 2026-05-15 03:58 | 93a3f8b | Add representative local-discovery snapshots |
| `CSP-031` | `P2-011` | 2026-05-15 03:58 | 93a3f8b | Verify the Phase 2 end state |
| `CSP-032` | `P3-001` | 2026-05-15 20:50 | 165fcaa | Define agent harness discovery boundaries |
| `CSP-033` | `P3-002` | 2026-05-15 20:50 | 165fcaa | Add synthetic harness fixture support |
| `CSP-034` | `P3-003` | 2026-05-15 20:50 | 165fcaa | Discover supported agent sessions |
| `CSP-035` | `P3-004` | 2026-05-15 20:50 | 165fcaa | Preserve fork session lineage evidence |
| `CSP-036` | `P3-005` | 2026-05-15 20:50 | 165fcaa | Add injectable tmux command execution |
| `CSP-037` | `P3-006` | 2026-05-15 20:50 | 165fcaa | Discover tmux sessions |
| `CSP-038` | `P3-007` | 2026-05-15 20:50 | 165fcaa | Generate session, workspace, fork, and mux candidate links |
| `CSP-039` | `P3-008` | 2026-05-15 20:50 | 165fcaa | Implement session-to-mux resolver scoring |
| `CSP-040` | `P3-009` | 2026-05-15 20:50 | 165fcaa | Wire agent and tmux discovery into local graph discovery |
| `CSP-041` | `P3-010` | 2026-05-15 20:50 | 165fcaa | Add representative agent and mux JSON snapshots |
| `CSP-042` | `P3-011` | 2026-05-15 20:50 | 165fcaa | Verify the Phase 3 end state |
| `CSP-043` | `P3-FU-001` | 2026-05-15 21:27 | 12a6a01 | Align harness adapter parsers with real provider state |
| `CSP-044` | `P3-FU-002` | 2026-05-16 01:59 | a0f7a77 | Read opencode sessions from the SQLite store |
| `CSP-045` | `P4-001` | 2026-05-16 03:21 | e28ae69 | Define forge discovery boundaries and `gh` command runner |
| `CSP-046` | `P4-002` | 2026-05-16 03:21 | e28ae69 | Discover GitHub pull requests for known repos |
| `CSP-047` | `P4-003` | 2026-05-16 03:21 | e28ae69 | Map PR records into ForgePr nodes and branch candidate links |
| `CSP-048` | `P4-004` | 2026-05-16 03:21 | e28ae69 | Resolver scoring for branch ↔ pull request |
| `CSP-049` | `P4-005` | 2026-05-16 03:21 | e28ae69 | Add config loading for session projection defaults |
| `CSP-050` | `P4-006` | 2026-05-16 03:21 | e28ae69 | Define table output projection boundaries |
| `CSP-051` | `P4-007` | 2026-05-16 03:21 | e28ae69 | Implement the agent projection table renderer |
| `CSP-052` | `P4-008` | 2026-05-16 03:21 | e28ae69 | Implement the mux projection table renderer |
| `CSP-053` | `P4-009` | 2026-05-16 03:21 | e28ae69 | Implement the union projection table renderer |
| `CSP-054` | `P4-010` | 2026-05-16 03:21 | e28ae69 | Add the `conspectus session` CLI subcommand |
| `CSP-055` | `P4-011` | 2026-05-16 03:21 | e28ae69 | Wire forge discovery into local graph discovery |
| `CSP-056` | `P4-012` | 2026-05-16 03:21 | e28ae69 | Add representative JSON and table snapshots |
| `CSP-057` | `P4-013` | 2026-05-16 03:21 | e28ae69 | Verify the Phase 4 end state |
| `CSP-058` | `P4-FU-001` | 2026-05-16 03:21 | e28ae69 | Document the `CONSPECTUS_DISABLE_FORGE`, `CONSPECTUS_DISABLE_TMUX`… |
| `CSP-059` | `P4-FU-002` | 2026-05-16 03:21 | e28ae69 | Match PRs whose head ref is a non-current local branch by enumerating all local refs in the git probe |
| `CSP-060` | `P5-001` | 2026-05-16 04:29 | 4e91741 | Record the declared-link storage schema |
| `CSP-061` | `P5-002` | 2026-05-16 04:29 | 4e91741 | Define declared-link file models and TOML round trips |
| `CSP-062` | `P5-003` | 2026-05-16 04:29 | 4e91741 | Load local and global declared links into graph evidence |
| `CSP-063` | `P5-004` | 2026-05-16 04:29 | 4e91741 | Preserve read-only command invariants |
| `CSP-064` | `P5-005` | 2026-05-16 04:29 | 4e91741 | Implement nearest-store selection for writes |
| `CSP-065` | `P5-006` | 2026-05-16 04:29 | 4e91741 | Add atomic declared-link write helpers |
| `CSP-066` | `P5-007` | 2026-05-16 04:29 | 4e91741 | Define the declared-link CLI surface |
| `CSP-067` | `P5-008` | 2026-05-16 04:29 | 4e91741 | Implement list and inspect commands for declared state |
| `CSP-068` | `P5-009` | 2026-05-16 04:29 | 4e91741 | Implement link and unlink commands |
| `CSP-069` | `P5-010` | 2026-05-16 04:29 | 4e91741 | Implement confirm, ignore, and override flows |
| `CSP-070` | `P5-011` | 2026-05-16 04:29 | 4e91741 | Add declared-link graph and table snapshots |
| `CSP-071` | `P5-012` | 2026-05-16 04:29 | 4e91741 | Verify the Phase 5 end state |
| `CSP-072` | `P5-FU-001` | 2026-05-16 16:15 | 0ccb9e6 | Prune empty `[declared]` sections after the last declared link is removed |
| `CSP-073` | `P6-001` | 2026-05-16 16:30 | 0974ce9 | Record an ADR for the Conspectus library API surface |
| `CSP-074` | `P6-002` | 2026-05-16 16:30 | 0974ce9 | Record an ADR for Conspectus distribution |
| `CSP-075` | `P6-003` | 2026-05-16 16:30 | 0974ce9 | Audit pure vs impure modules and produce a library API inventory |
| `CSP-076` | `P6-004` | 2026-05-16 16:30 | 0974ce9 | Add a curated public re-export facade |
| `CSP-077` | `P6-005` | 2026-05-16 16:30 | 0974ce9 | Write the Atelier migration guide |
| `CSP-078` | `P6-006` | 2026-05-16 16:30 | 0974ce9 | Add a representative comparison fixture |
| `CSP-079` | `P6-007` | 2026-05-16 16:30 | 0974ce9 | File the Atelier-side delegation work in the Atelier repo |
| `CSP-080` | `P6-008` | 2026-05-16 16:30 | 0974ce9 | Refresh top-level docs to position Conspectus as the cross-workspace observability surface |
| `CSP-081` | `P6-009` | 2026-05-16 16:30 | 0974ce9 | Decide whether to extract Conspectus into its own repository |
| `CSP-082` | `P6-010` | 2026-05-16 16:30 | 0974ce9 | Verify the Phase 6 end state |
| `CSP-083` | `H-REF-001` | 2026-05-17 20:45 | 091b831 | Extract a shared `DeclaredEndpoint` codec |
| `CSP-084` | `H-REF-002` | 2026-05-17 20:45 | 091b831 | Share the relation-kind string codec |
| `CSP-085` | `H-REF-003` | 2026-05-17 20:45 | 091b831 | Generalize the resolver scoring tier helpers |
| `CSP-086` | `H-REF-004` | 2026-05-17 20:45 | 091b831 | Unify the external-tool runner seam |
| `CSP-087` | `H-REF-005` | 2026-05-17 20:45 | 091b831 | Split `src/declared.rs` by concern |
| `CSP-088` | `H-REF-006` | 2026-05-17 20:45 | 091b831 | Slim `src/cli.rs` into per-command modules |
| `CSP-089` | `H-REF-007` | 2026-05-17 20:45 | 091b831 | Factor harness adapter state-root scanning |
| `CSP-090` | `H-REF-008` | 2026-05-17 20:45 | 091b831 | Replace string field names in `SourceMetadata.fields` |
| `CSP-091` | `H-REF-009` | 2026-05-17 20:45 | 091b831 | Centralize provider identifier constants |
| `CSP-092` | `H-REF-010` | 2026-05-17 20:45 | 091b831 | Audit and shrink the curated `conspectus::api` surface |
| `CSP-093` | `H-OBS-001` | 2026-05-17 20:45 | 091b831 | Add a human-readable graph projection |
| `CSP-094` | `H-OBS-002` | 2026-05-17 20:45 | 091b831 | Add `conspectus node show <id>` |
| `CSP-095` | `H-OBS-003` | 2026-05-17 20:45 | 091b831 | Add filter flags for the graph and session commands |
| `CSP-096` | `H-OBS-004` | 2026-05-17 20:45 | 091b831 | Add a `--explain` mode for resolved relationships |
| `CSP-097` | `H-OBS-005` | 2026-05-17 20:45 | 091b831 | Improve discovery diagnostics for missing providers |
| `CSP-098` | `H-OBS-006` | 2026-05-17 20:45 | 091b831 | Surface activity/recency in the session tables |
| `CSP-099` | `H-PROD-001` | 2026-05-17 20:45 | 091b831 | Implement the bootstrap-roots flow described in the design |
| `CSP-100` | `H-PROD-002` | 2026-05-17 20:45 | 091b831 | Cache layer for forge metadata, tmux, and harness scans |
| `CSP-101` | `H-PROD-003` | 2026-05-17 20:45 | 091b831 | Batch `gh pr list` across repos sharing a host |
| `CSP-102` | `H-PROD-004` | 2026-05-17 20:45 | 091b831 | Add a graph-diff command |
| `CSP-103` | `H-DIST-001` | 2026-05-17 20:45 | 091b831 | Add a GitHub Actions CI workflow |
| `CSP-104` | `H-DIST-002` | 2026-05-17 20:45 | 091b831 | Complete `Cargo.toml` metadata for crates.io |
| `CSP-105` | `H-DIST-003` | 2026-05-17 20:45 | 091b831 | Pin and verify MSRV |
| `CSP-106` | `H-DIST-004` | 2026-05-17 20:45 | 091b831 | Define the release process |
| `CSP-107` | `H-DESIGN-001` | 2026-05-17 20:45 | 091b831 | Settle the workspace-detection threshold and provider precedence |
| `CSP-108` | `H-DESIGN-002` | 2026-05-17 20:45 | 091b831 | Settle `ForgePr` identity and branch-association keys |
| `CSP-109` | `H-DESIGN-003` | 2026-05-17 20:45 | 091b831 | Settle declared-link conflict and override semantics |
| `CSP-110` | `H-DESIGN-004` | 2026-05-17 20:45 | 091b831 | Document the graph invariants and snapshot canonicalization contract |
| `CSP-111` | `H-FUTURE-001` | 2026-05-17 20:45 | 091b831 | Add a mux backend for zellij (and stub screen) |
| `CSP-112` | `H-FUTURE-002` | 2026-05-17 20:45 | 091b831 | Add a forge adapter for GitLab or Gitea |
| `CSP-113` | `H-FUTURE-003` | 2026-05-17 20:45 | 091b831 | Add harness adapters for jujutsu and sapling sessions if and when a user uses them with a supported harness |
| `CSP-114` | `H-DOC-001` | 2026-05-17 20:45 | 091b831 | Add a first-run walkthrough |
| `CSP-115` | `H-DOC-002` | 2026-05-17 20:45 | 091b831 | Add a provider-adapter contributor guide |
| `CSP-116` | `H-DOC-003` | 2026-05-17 20:45 | 091b831 | Add library-integration examples beyond the api doctest |
| `CSP-117` | `H-LINEAGE-001` | 2026-05-17 20:45 | 091b831 | Settle the data-model shape for intra-harness lineage |
| `CSP-118` | `H-LINEAGE-002` | 2026-05-17 20:45 | 091b831 | Extract claude-code session lineage |
| `CSP-119` | `H-LINEAGE-003` | 2026-05-17 20:45 | 091b831 | Extract opencode session lineage from `session.parent_id` |
| `CSP-120` | `H-LINEAGE-004` | 2026-05-17 20:45 | 091b831 | Extract codex resume lineage |
| `CSP-121` | `H-LINEAGE-005` | 2026-05-17 20:45 | 091b831 | Surface session lineage in the session table |
| `CSP-122` | `H-AGENTMUX-002` | 2026-05-17 20:45 | 091b831 | Detect agent-deck multi-repo checkouts as a workspace provider |
| `CSP-123` | `H-AGENTMUX-003` | 2026-05-17 20:45 | 091b831 | Surface multi-repo participants in the session table |
| `CSP-124` | `H-AGENTMUX-004` | 2026-05-17 20:45 | 091b831 | Read agent-deck profile state from `state.db` |
| `CSP-125` | `H-LINEAGE-006` | 2026-05-17 22:08 | d8f94ea | Retarget claude-code lineage extraction — fork uses a `forkedFrom` envelope object, not `parentUuid`… |
| `CSP-126` | `H-TBL-001` | 2026-05-18 14:50 | 1ed9597 | ADR: width-aware table rendering library |
| `CSP-127` | `H-TBL-002` | 2026-05-18 14:50 | 1ed9597 | Surface short, stable row identifiers in session tables |
| `CSP-128` | `H-TBL-003` | 2026-05-18 14:50 | 1ed9597 | Width-aware truncation default for session tables |
| `CSP-129` | `H-TBL-004` | 2026-05-18 14:50 | 1ed9597 | Opt-in card / multi-line row layout |
| `CSP-130` | `H-TBL-005` | 2026-05-18 14:50 | 1ed9597 | Resolve table row identifiers in `conspectus node show` |
| `CSP-131` | `H-AGENTMUX-001` | 2026-05-18 14:50 | 1ed9597 | Audit each candidate orchestrator's evidence against MUXPROC and decide which adapters to build |
| `CSP-132` | `H-AGENTMUX-005` | 2026-05-18 14:50 | 1ed9597 | Add a dmux orchestrator adapter (audit-gated) |
| `CSP-133` | `H-AGENTMUX-006` | 2026-05-18 14:50 | 1ed9597 | Add a workmux orchestrator adapter (audit-gated, narrowed scope) |
| `CSP-134` | `H-AGENTMUX-007` | 2026-05-18 14:50 | 1ed9597 | Add an agent-of-empires orchestrator adapter (audit-gated) |
| `CSP-135` | `H-MUXPROC-001` | 2026-05-18 14:50 | 1ed9597 | ADR: process-tree linker design and dependency choice |
| `CSP-136` | `H-MUXPROC-002` | 2026-05-18 14:50 | 1ed9597 | Implement the process-tree linker as a discovery source |
| `CSP-137` | `P7-001` | 2026-05-18 14:50 | 1ed9597 | ADR: graph snapshot persistence format and lifecycle |
| `CSP-138` | `P7-002` | 2026-05-18 14:50 | 1ed9597 | Add provider provenance and freshness metadata to graph nodes and candidate links |
| `CSP-139` | `P7-003` | 2026-05-18 14:50 | 1ed9597 | Implement snapshot save/load for the one-shot CLI |
| `CSP-140` | `P7-004` | 2026-05-18 14:50 | 1ed9597 | ADR: continuous server mode architecture and transport |
| `CSP-141` | `P7-005` | 2026-05-18 14:50 | 1ed9597 | Implement partial graph eviction at provider granularity |
| `CSP-142` | `P7-006` | 2026-05-18 14:50 | 1ed9597 | Implement `conspectus serve` |
| `CSP-143` | `P7-007` | 2026-05-18 14:50 | 1ed9597 | Implement CLI ↔ server snapshot read path |
| `CSP-144` | `P7-008` | 2026-05-18 14:50 | 1ed9597 | Add server status and inspection subcommands |
| `CSP-145` | `P7-009` | 2026-05-18 14:50 | 1ed9597 | Event-driven refresh via filesystem watchers (stretch) |
| `CSP-146` | `H-TBL-006` | 2026-05-18 21:28 | dab02cb | Rename `conspectus session` to `conspectus table <ROWS>` |
| `CSP-147` | `H-TBL-007` | 2026-05-18 21:28 | dab02cb | Per-row-type column registry and `--columns` flag |
| `CSP-148` | `H-TBL-008` | 2026-05-18 21:28 | dab02cb | `conspectus table prs` row-type |
| `CSP-149` | `H-TBL-009` | 2026-05-18 21:28 | dab02cb | `conspectus table forks` row-type |
| `CSP-150` | `H-TBL-010` | 2026-05-18 21:28 | dab02cb | Expand the `sessions` column pool |
| `CSP-151` | `H-TBL-011` | 2026-05-18 21:28 | dab02cb | Expand the `mux` column pool |
| `CSP-152` | `H-TBL-012` | 2026-05-18 21:28 | dab02cb | `conspectus columns <ROWS>` discovery subcommand |
| `CSP-153` | `H-TBL-013` | 2026-05-18 22:51 | f48a24e | Pager auto-fit for table-style outputs |
| `CSP-154` | `H-TBL-014` | 2026-05-19 02:48 | 580a1e9 | Terminal color and styling for table output |
| `CSP-155` | `H-PREVIEW-001` | 2026-05-19 03:32 | a7bd43a | Model field + opt-in preview column registration |
| `CSP-156` | `H-PREVIEW-002` | 2026-05-19 03:32 | a7bd43a | Claude Code last-message extraction |
| `CSP-157` | `H-PREVIEW-003` | 2026-05-19 03:32 | a7bd43a | Codex last-message extraction |
| `CSP-158` | `H-PREVIEW-004` | 2026-05-19 03:32 | a7bd43a | Opencode last-message extraction |
| `CSP-159` | `H-PREVIEW-005` | 2026-05-19 03:32 | a7bd43a | Aider last-message extraction |
| `CSP-160` | `P8-001` | 2026-05-19 03:51 | 0d9e07d | Lock v1 TUI product decisions (operator-journey core) |
| `CSP-160.01` | `P8-001a` | 2026-05-19 12:36 | 78caa86 | Settle remaining v1-blocking product questions |
| `CSP-161` | `P8-002` | 2026-05-19 03:51 | 0d9e07d | ADR: TUI runtime, app architecture, and dependency policy |
| `CSP-162` | `P8-003` | 2026-05-19 03:51 | 0d9e07d | Add `conspectus tui` CLI shell and terminal lifecycle |
| `CSP-163` | `P8-004` | 2026-05-19 03:51 | 0d9e07d | Build TUI row tree view-models for every table row-type |
| `CSP-164` | `P8-005` | 2026-05-19 03:51 | 0d9e07d | Build selected-node detail view-models |
| `CSP-165` | `P8-006` | 2026-05-19 03:51 | 0d9e07d | Implement selection, focus, navigation, and filtering state |
| `CSP-166` | `P8-007` | 2026-05-19 03:51 | 0d9e07d | Render the two-panel Ratatui UI |
| `CSP-167` | `P8-008` | 2026-05-19 03:51 | 0d9e07d | Add non-blocking graph refresh data adapter |
| `CSP-168` | `P8-009` | 2026-05-19 03:51 | 0d9e07d | Add mux live-preview capture adapter |
| `CSP-169` | `P8-010` | 2026-05-19 03:51 | 0d9e07d | Implement attach-to-existing-mux action |
| `CSP-170` | `P8-011` | 2026-05-19 03:51 | 0d9e07d | Implement resume un-muxed agent session into mux |
| `CSP-171` | `P8-012` (restored parent) | 2026-05-19 03:51 | 0d9e07d | Add PR, fork, and transcript/history detail enrichments |
| `CSP-171.01` | `P8-012a` | 2026-05-19 12:36 | 78caa86 | PR right-panel enrichment with async `gh` fetch + cache |
| `CSP-171.02` | `P8-012b` | 2026-05-19 12:36 | 78caa86 | Fork right-panel enrichment (lineage, context, children) |
| `CSP-171.03` | `P8-012c` | 2026-05-19 12:36 | 78caa86 | Un-muxed agent transcript preview |
| `CSP-172` | `P8-013` | 2026-05-19 03:51 | 0d9e07d | Document and verify the v1 TUI workflow |
| `CSP-173` | `H-PREVIEW-006` | 2026-05-19 12:05 | 526aa0d | Filter codex channel markers from preview |
| `CSP-174` | `H-TBL-015` | 2026-05-19 12:06 | ab830ac | Move title out of AGENT label into its own column |
| `CSP-175` | `P8-014` | 2026-05-19 12:49 | bd1a797 | Inline mux-picker for ambiguous `LinkedToMux` candidates |
| `CSP-176` | `P8-015` | 2026-05-19 19:05 | e2e235f | Surface session `title` in the sessions row tree when it uniquely distinguishes siblings |
| `CSP-177` | `H-AGENT-EPOCH` | 2026-05-19 19:33 | 50f206c | Populate `AgentSessionNode.last_active_epoch` across harness adapters |
| `CSP-178` | `T8-001` | 2026-05-19 19:33 | 50f206c | Collapse duplicate repo group rows when a project appears across multiple checkout buckets in the TUI sessions row tree |
| `CSP-179` | `T8-002` | 2026-05-19 19:33 | 50f206c | Align `conspectus table sessions` columns with the TUI sessions row tree once view-models converge |
| `CSP-180` | `T8-003` | 2026-05-19 23:23 | bc7a1b0 | Fill out the TUI empty/loading/error frame matrix |
| `CSP-181` | `T8-004` | 2026-05-19 23:23 | bc7a1b0 | Same-line session preview switch |
| `CSP-182` | `T8-005` | 2026-05-19 23:23 | bc7a1b0 | Header `updated Ns ago` freshness indicator |
| `CSP-183` | `T8-006` | 2026-05-19 23:23 | bc7a1b0 | Expand TUI buffer-snapshot test coverage |
| `CSP-184` | `T8-007` | 2026-05-19 23:23 | bc7a1b0 | Move TUI discovery onto a background thread with timer-driven refresh |
| `CSP-185` | `T8-009` | 2026-05-20 01:56 | ea89b4e | Throttle and freshen mux pane-capture previews |
| `CSP-186` | `T8-010` | 2026-05-20 03:28 | 847f84b | Render ANSI color in tmux previews |
| `CSP-187` | `T8-011` | 2026-05-20 03:28 | 847f84b | Strengthen selected-row and focused-pane visual states |
| `CSP-188` | `T8-012` | 2026-05-20 03:28 | 847f84b | Compress project, path, and mux display labels |
| `CSP-189` | `T8-013` | 2026-05-20 03:28 | 847f84b | Default-expand and mark the launch-context project without filtering the world |
| `CSP-190` | `T8-014` | 2026-05-20 03:28 | 847f84b | Make the status bar contextual to the selected row |
| `CSP-191` | `T8-015` | 2026-05-20 03:28 | 847f84b | Add sessions-tree density modes |
| `CSP-192` | `T8-016` | 2026-05-20 03:28 | 847f84b | Crop and annotate tmux previews for recognition |
| `CSP-193` | `T8-017` | 2026-05-20 03:28 | 847f84b | Add visible search/filter workflow for large session worlds |
| `CSP-194` | `T8-018` | 2026-05-20 13:38 | be3667d | Round-trip attach: return to the TUI after the operator detaches from the mux client |
| `CSP-195` | `T8-019` | 2026-05-20 13:38 | be3667d | Auto-scroll the left tree to keep the selected row visible |
| `CSP-196` | `T8-020` | 2026-05-20 18:50 | b7660c9 | Auto-broaden TUI scan roots to the cwd's "code dir" ancestor when neither CLI nor config specifies one |
| `CSP-197` | `H-CHECKOUT-001` | 2026-05-21 00:10 | 24032d6 | Memorialize the checkout context model |
| `CSP-198` | `H-CHECKOUT-002` | 2026-05-21 00:10 | 24032d6 | Introduce checkout-facing model helpers ahead of the graph wire rename |
| `CSP-199` | `H-CHECKOUT-003` | 2026-05-21 00:10 | 24032d6 | Probe observed session and mux cwd paths for checkout context |
| `CSP-200` | `H-CHECKOUT-004` | 2026-05-21 00:10 | 24032d6 | Preserve logical and canonical paths for workspace members |
| `CSP-201` | `H-CHECKOUT-005` | 2026-05-21 00:10 | 24032d6 | Resolve multi-context session membership |
| `CSP-202` | `H-CHECKOUT-006` | 2026-05-21 00:10 | 24032d6 | Update table and TUI projections for checkout grouping |
| `CSP-203` | `H-CHECKOUT-007` | 2026-05-21 00:10 | 24032d6 | Retire legacy user-facing worktree terminology |
| `CSP-204` | `H-CHECKOUT-008` | 2026-05-22 02:30 | 34b554e | Hard-rename checkout graph wire/model names |
| `CSP-205` | `H-TRANSCRIPT-001` | 2026-05-22 23:53 | 9633fcf | ADR: terminal markdown rendering for the inline transcript preview |
| `CSP-206` | `H-TRANSCRIPT-002` | 2026-05-22 23:53 | 9633fcf | Resolve ADR 0019 with the May 2026 survey |
| `CSP-207` | `H-TRANSCRIPT-003` | 2026-05-22 23:53 | 9633fcf | Recent-history adapter API |
| `CSP-208` | `H-TRANSCRIPT-004` | 2026-05-22 23:53 | 9633fcf | Claude Code recent-turns extractor |
| `CSP-209` | `H-TRANSCRIPT-005` | 2026-05-22 23:53 | 9633fcf | Codex recent-turns extractor |
| `CSP-210` | `H-TRANSCRIPT-006` | 2026-05-22 23:53 | 9633fcf | OpenCode recent-turns extractor |
| `CSP-211` | `H-TRANSCRIPT-007` | 2026-05-22 23:53 | 9633fcf | Aider recent-turns extractor (deferred) |
| `CSP-212` | `H-TRANSCRIPT-008` | 2026-05-22 23:53 | 9633fcf | Add `tui-markdown` dependency |
| `CSP-213` | `H-TRANSCRIPT-009` | 2026-05-22 23:53 | 9633fcf | Inline transcript-preview widget |
| `CSP-214` | `H-TRANSCRIPT-010` | 2026-05-22 23:53 | 9633fcf | Wire the widget into the right panel for un-muxed agent rows |
| `CSP-215` | `H-TRANSCRIPT-011` | 2026-05-22 23:53 | 9633fcf | Document the inline transcript preview |
| `CSP-216` | `H-TRANSCRIPT-012` | 2026-05-22 23:53 | 9633fcf | External full-transcript viewer launch (moved ahead of the inline-widget track per the amended ADR 0019… |
| `CSP-217` | `H-MUXPROC-003` | 2026-05-22 23:53 | 9633fcf | Add read-only session-file activity correlation |
| `CSP-218` | `H-MUXPROC-004` | 2026-05-22 23:53 | 9633fcf | Read Codex state and log databases for live session attribution |
| `CSP-219` | `H-MUXPROC-005` | 2026-05-22 23:53 | 9633fcf | Audit harness control planes for non-mutating current-session queries |
| `CSP-220` | `H-MUXPROC-006` | 2026-05-22 23:53 | 9633fcf | Add Codex app-server attribution adapter if the audit proves a stable non-mutating query |
| `CSP-221` | `H-MUXPROC-007` | 2026-05-22 23:53 | 9633fcf | Add opencode server/ACP attribution adapter if the audit proves a stable non-mutating query |
| `CSP-222` | `H-MUXPROC-008` | 2026-05-22 23:53 | 9633fcf | Document terminal-injection attribution as a rejected strategy unless a harness guarantees non-mutating status commands |
| `CSP-223` | `H-MUXPROC-009` | 2026-05-22 23:53 | 9633fcf | Audit harness hooks/plugins as definitive session-state sidecar emitters |
| `CSP-224` | `H-MUXPROC-010` | 2026-05-22 23:53 | 9633fcf | Define Conspectus hook sidecar schema and trust/ranking rules |
| `CSP-225` | `H-MUXPROC-011` | 2026-05-22 23:53 | 9633fcf | Implement hook-sidecar discovery provider |
| `CSP-226` | `H-MUXPROC-012` | 2026-05-22 23:53 | 9633fcf | Add Claude Code hook sidecar emitter if audit proves non-mutating session identity |
| `CSP-227` | `H-MUXPROC-015` | 2026-05-22 23:53 | 9633fcf | Fix Claude Code mux attribution after in-process `/resume` switches |
| `CSP-228` | `H-MUXPROC-013` | 2026-05-22 23:53 | 9633fcf | Add Codex hook sidecar emitter if audit proves non-mutating session identity |
| `CSP-229` | `H-MUXPROC-014` | 2026-05-22 23:53 | 9633fcf | Add opencode plugin sidecar emitter |
| `CSP-230` | `H-MUXPROC-016` (the 2026-05-23 story) | 2026-05-23 02:59 | 4db6628 | Add `conspectus hook write` sidecar writer |
| `CSP-231` | `H-MUXPROC-017` (the 2026-05-23 story) | 2026-05-23 02:59 | 4db6628 | Add `conspectus hook init` installer UX |
| `CSP-232` | `H-RENAME-001` | 2026-05-23 02:59 | 4db6628 | ADR: alias overlay schema and storage |
| `CSP-233` | `H-RENAME-002` | 2026-05-23 02:59 | 4db6628 | ADR: TUI text-input primitive |
| `CSP-234` | `H-RENAME-003` | 2026-05-23 02:59 | 4db6628 | Extend `TmuxRunner` with `rename_session` mutation seam |
| `CSP-235` | `H-RENAME-004` | 2026-05-23 02:59 | 4db6628 | Alias storage layer |
| `CSP-236` | `H-RENAME-006` | 2026-05-23 02:59 | 4db6628 | Projection precedence |
| `CSP-237` | `H-RENAME-007` | 2026-05-23 02:59 | 4db6628 | CLI: `conspectus rename` command tree |
| `CSP-238` | `H-RENAME-008` | 2026-05-23 02:59 | 4db6628 | CLI: `conspectus alias list` (and `show`) |
| `CSP-239` | `H-RENAME-009` | 2026-05-23 02:59 | 4db6628 | Mux lockstep helper |
| `CSP-240` | `H-RENAME-010` | 2026-05-23 02:59 | 4db6628 | TUI text-input widget implementation |
| `CSP-241` | `H-RENAME-011` | 2026-05-23 02:59 | 4db6628 | TUI `R` keybinding wires rename flow |
| `CSP-242` | `H-RENAME-012` | 2026-05-23 02:59 | 4db6628 | Read-only invariant audit |
| `CSP-243` | `H-RENAME-013` | 2026-05-23 02:59 | 4db6628 | Live-session UX advisory |
| `CSP-244` | `H-RENAME-014` | 2026-05-23 02:59 | 4db6628 | Docs and snapshot coverage |
| `CSP-245` | `H-AI-NAMING-001` | 2026-05-23 02:59 | 4db6628 | ADR: provider, dependency, privacy, dispatch |
| `CSP-246` | `H-AI-NAMING-002` | 2026-05-23 02:59 | 4db6628 | Transcript context extractor |
| `CSP-247` | `H-AI-NAMING-003` | 2026-05-23 02:59 | 4db6628 | CLI + TUI suggest surface |
| `CSP-248` | `H-AI-NAMING-004` | 2026-05-23 02:59 | 4db6628 | Optional auto-suggest hook |
| `CSP-249` | `H-MUXPROC-018` | 2026-05-23 04:22 | 2d8ae4f | Dedupe hook records by pane and drop the 15-minute emission gate |
| `CSP-250` | `F8-001` | 2026-05-24 01:47 | e278639 | Define `RowFilter` predicate + dimension types in a crate-public module |
| `CSP-251` | `F8-002` | 2026-05-24 01:47 | e278639 | Per-view grouping enums for the four pending views |
| `CSP-252` | `F8-003` | 2026-05-24 01:47 | e278639 | Per-view state retention |
| `CSP-253` | `F8-004` | 2026-05-24 01:47 | e278639 | Controls overlay (modal) |
| `CSP-254` | `F8-005` | 2026-05-24 01:47 | e278639 | Accelerator keybindings + view-switching plumbing |
| `CSP-255` | `F8-006` | 2026-05-24 01:47 | e278639 | Multi-select list widget |
| `CSP-256` | `F8-007` | 2026-05-24 01:47 | e278639 | Status-bar filter chips + counts-with-totals |
| `CSP-257` | `F8-008` | 2026-05-24 01:47 | e278639 | `[tui.views.<name>]` config schema + legacy alias |
| `CSP-258` | `F8-009` | 2026-05-24 01:47 | e278639 | CLI flag parity: shared `FilterArgs` + per-view grouping |
| `CSP-259` | `F8-010` | 2026-05-24 01:47 | e278639 | `conspectus table <ROWS>` consumes `RowFilter` |
| `CSP-260` | `F8-011` | 2026-05-24 01:47 | e278639 | Help-overlay docs |
| `CSP-261` | `F8-012` | 2026-05-24 01:47 | e278639 | Filtered-zero empty frame |
| `CSP-262` | `TEST-001` | 2026-05-24 20:27 | 78f4369 | Add a MUXPROC scenario replay harness |
| `CSP-263` | `TEST-002` | 2026-05-24 20:27 | 78f4369 | Add a sanitized real-state fixture corpus |
| `CSP-264` | `TEST-003` | 2026-05-24 20:27 | 78f4369 | Replay recent MUXPROC drift and stale-evidence bugs |
| `CSP-265` | `TEST-004` | 2026-05-24 20:27 | 78f4369 | Add graph and row-projection invariant tests |
| `CSP-266` | `TEST-005` | 2026-05-24 20:27 | 78f4369 | Add TUI interaction regression tests for row expansion, scrolling, and attach resolution |
| `CSP-267` | `T8-021` | 2026-05-24 21:02 | a634b57 | Descend into scan roots when looking for atelier workspaces (companion to `CSP-196`) |
| `CSP-268` | `T8-022` | 2026-05-25 02:00 | 14dac93 | Detect session live status (running / waiting / idle / error) and surface it as a row glyph and per-status header chip |
| `CSP-269` | `T8-023` | 2026-05-25 02:00 | 14dac93 | Ship preset theme variants on top of ADR 0032 |
| `CSP-270` | `T8-024` | 2026-05-25 02:00 | 14dac93 | Add sessions-tree density modes — folds `CSP-191` into the theme-aware renderer landed by the styling overhaul |
| `CSP-271` | `P9-001` | 2026-05-25 21:41 | 6a08f2f | SQLite integration spike (bundled build, WAL, lifecycle) |
| `CSP-272` | `P9-002` | 2026-05-25 21:41 | 6a08f2f | Define the SQL schema for the resolved graph |
| `CSP-273` | `P9-003` | 2026-05-25 21:41 | 6a08f2f | Implement the GraphSnapshot → SQLite loader |
| `CSP-274` | `P9-004` | 2026-05-25 21:41 | 6a08f2f | Implement `conspectus query <sql>` MVP |
| `CSP-275` | `P9-005` | 2026-05-25 21:41 | 6a08f2f | Result formatters for query output |
| `CSP-276` | `P9-006` | 2026-05-25 21:41 | 6a08f2f | Saved views: a library of common queries |
| `CSP-277` | `P9-007` | 2026-05-25 21:41 | 6a08f2f | Fixture corpus and query regression suite |
| `CSP-278` | `P9-008` | 2026-05-25 21:41 | 6a08f2f | Vector search via `sqlite-vec` (deferred) |
| `CSP-279` | `P10-001` | 2026-05-26 23:06 | c318251 | Promote reader + add compile-time read exhaustiveness |
| `CSP-280` | `P10-002` | 2026-05-26 23:06 | c318251 | JSON-encoded NodeId foreign references (ADR 0044) |
| `CSP-281` | `P10-003` | 2026-05-26 23:06 | c318251 | Extract the shared rendering substrate |
| `CSP-282` | `P10-004` | 2026-05-26 23:06 | c318251 | Migrate the CLI agent projection to SQLite |
| `CSP-283` | `P10-005` | 2026-05-26 23:06 | c318251 | Migrate the CLI mux projection to SQLite |
| `CSP-284` | `P10-006` | 2026-05-26 23:06 | c318251 | Migrate the CLI union projection to SQLite |
| `CSP-285` | `P10-007` | 2026-05-26 23:06 | c318251 | Migrate the CLI PRs projection to SQLite |
| `CSP-286` | `P10-008` | 2026-05-26 23:06 | c318251 | Migrate the CLI forks projection to SQLite |
| `CSP-287` | `P10-009` | 2026-05-26 23:06 | c318251 | Migrate `node show` to SQLite |
| `CSP-288` | `P10-010` | 2026-05-26 23:06 | c318251 | Migrate the TUI detail pane to SQLite |
| `CSP-289` | `P10-011` | 2026-05-26 23:06 | c318251 | Migrate the TUI sessions row builder to SQLite |
| `CSP-290` | `P10-012` | 2026-05-26 23:06 | c318251 | Decide whether TUI Mux/Union/Prs/Forks builders need SQLite-specific migration work |
| `CSP-291` | `P10-013` | 2026-05-26 23:06 | c318251 | Retire `SnapshotIndex`, `SnapshotView`, `SessionsIndex` |
| `CSP-292` | `P10-014` | 2026-05-26 23:06 | c318251 | Demote `GraphSnapshot` to producer-only |
| `CSP-293` | `P9-FU-001` | 2026-05-28 02:52 | f0fc775 | Add an embedding import command |
| `CSP-294` | `P10-FU-001` | 2026-05-28 02:52 | f0fc775 | Retire snapshot bridge APIs from consumer modules |
| `CSP-295` | `H-SUBAGENT-001` | 2026-05-28 23:02 | cd651a6 | Determine how to detect subagent sessions from opencode state |
| `CSP-296` | `H-SUBAGENT-002` | 2026-05-28 23:02 | cd651a6 | Thread subagent metadata into the graph model |
| `CSP-297` | `H-SUBAGENT-003` | 2026-05-28 23:02 | cd651a6 | Filter and nest subagent sessions in the TUI |
| `CSP-298` | `H-SUBAGENT-004` | 2026-05-28 23:02 | cd651a6 | Suppress subagent sessions from mux attachment resolution |
| `CSP-299` | `T8-025` | 2026-05-30 04:47 | 094f759 | Show full session and mux IDs in the TUI |
| `CSP-300` | `T8-026` | 2026-05-30 04:47 | 094f759 | Expand linked entities from the TUI detail pane |
| `CSP-301` | `GV-001` | 2026-05-30 04:47 | 094f759 | Record graph visualization export decisions |
| `CSP-302` | `GV-002` | 2026-05-30 04:47 | 094f759 | Add `conspectus graph --format dot` |
| `CSP-303` | `GV-003` | 2026-05-30 04:47 | 094f759 | Add `conspectus graph --format html` |
| `CSP-303.01` | `GV-003a` | 2026-06-02 02:41 | c9bcf01 | HTML renderer scaffolding + minimal viewer |
| `CSP-303.02` | `GV-003b` | 2026-06-02 02:41 | c9bcf01 | HTML inspector, filter panel, and search |
| `CSP-303.03` | `GV-003c` | 2026-06-02 02:41 | c9bcf01 | HTML navigation primitives |
| `CSP-303.04` | `GV-003d` | 2026-06-02 02:41 | c9bcf01 | HTML layout improvements and selection |
| `CSP-304` | `GV-004` | 2026-05-30 04:47 | 094f759 | Document graph visualization workflows |
| `CSP-305` | `H-MUXPROC-FU-001` | 2026-05-30 23:01 | 848cf47 | Evaluate first-class runtime process nodes |
| `CSP-306` | `TEST-006` | 2026-05-31 03:39 | a8a8ed7 | Expose named replay scenarios to CLI and TUI runs |
| `CSP-307` | `TEST-007` | 2026-05-31 04:15 | e00a8b3 | Add filter, grouping, and sort controls to dev scenario exploration |
| `CSP-308` | `H-MUXPROC-FU-002` | 2026-05-31 19:22 | 599e538 | Add runtime process graph model and relation kinds |
| `CSP-309` | `H-MUXPROC-FU-003` | 2026-05-31 19:22 | 599e538 | Persist runtime process nodes in SQLite and graph JSON |
| `CSP-310` | `H-MUXPROC-FU-004` | 2026-05-31 19:22 | 599e538 | Emit runtime process nodes from MUXPROC discovery |
| `CSP-311` | `H-MUXPROC-FU-005` | 2026-05-31 19:22 | 599e538 | Move mux-cardinality and attribution resolver logic onto runtime process evidence |
| `CSP-312` | `H-MUXPROC-FU-006` | 2026-05-31 19:22 | 599e538 | Surface runtime process diagnostics in node detail and scenario fixtures |
| `CSP-313` | `T8-027` | 2026-06-01 00:28 | ac74978 | Model detail-pane relationship groups and previews |
| `CSP-314` | `T8-028` | 2026-06-01 00:28 | ac74978 | Replace inline expansion with relationship-group navigation |
| `CSP-315` | `T8-029` | 2026-06-01 00:28 | ac74978 | Render the focused inspector, relationship explorer, and preview layout |
| `CSP-316` | `T8-030` | 2026-06-01 00:28 | ac74978 | Add full-value inspection for long detail fields |
| `CSP-317` | `T8-031` | 2026-06-01 00:28 | ac74978 | Update docs and scenario coverage for detail graph navigation |
| `CSP-318` | `T8-032` | 2026-06-01 03:32 | cdea893 | First-class evidence inspector and link-promotion flow |
| `CSP-318.01` | `T8-032a` | 2026-06-24 20:11 | d623512 | Surface resolver explanations in the TUI detail explorer |
| `CSP-319` | `T8-033` | 2026-06-01 03:32 | cdea893 | TUI responsive-layout design and breakpoints |
| `CSP-320` | `T8-034` | 2026-06-01 03:32 | cdea893 | Expanded Node Detail toggle |
| `CSP-321` | `T8-035` | 2026-06-01 03:32 | cdea893 | Left-pane mirror sync (default) |
| `CSP-322` | `T8-036` | 2026-06-01 03:32 | cdea893 | Left-pane follow sync (opt-in view switching) |
| `CSP-323` | `T8-037` | 2026-06-01 15:17 | b6611f7 | Distinguish symmetric relations in the detail explorer |
| `CSP-324` | `T8-038` | 2026-06-01 15:17 | b6611f7 | Shorten breadcrumb hop labels and elide deep chains |
| `CSP-325` | `T8-039` | 2026-06-01 15:17 | b6611f7 | Surface node kind as a first-class field in the detail pane |
| `CSP-326` | `T8-040` | 2026-06-01 15:17 | b6611f7 | Enter-to-copy on Node-zone fields with a toast widget |
| `CSP-327` | `T8-041` | 2026-06-01 15:17 | b6611f7 | Flip the Upstream / Downstream header layout so zone labels anchor to the right |
| `CSP-328` | `T8-042` | 2026-06-01 16:11 | cc4e66d | Hide edge meta (`provenance · confidence · state`) from link rows by default with an opt-in toggle |
| `CSP-329` | `GV-EDGEREASON` | 2026-06-02 03:59 | 8d53480 | Emit explicit resolver decision rationale |
| `CSP-330` | `H-TRANSCRIPT-014` | 2026-06-02 18:59 | c89809b | Surface recall's harness coverage gap (or broaden it) |
| `CSP-331` | `H-TRANSCRIPT-015` | 2026-06-02 18:59 | c89809b | Recall focus on deep-link entry |
| `CSP-332` | `H-TRANSCRIPT-013` | 2026-06-02 18:59 | c89809b | Config-driven viewer override |
| `CSP-333` | `H-VIEWER-NATIVE-001` | 2026-06-02 18:59 | c89809b | Module scaffold + dep-surface enforcement |
| `CSP-334` | `H-VIEWER-NATIVE-002` | 2026-06-02 18:59 | c89809b | `TranscriptDocument` + `TranscriptTurn` model + `SessionLocator` types |
| `CSP-335` | `H-VIEWER-NATIVE-003` | 2026-06-02 18:59 | c89809b | Claude Code parser |
| `CSP-336` | `H-VIEWER-NATIVE-004` | 2026-06-02 18:59 | c89809b | Codex parser |
| `CSP-337` | `H-VIEWER-NATIVE-005` | 2026-06-02 18:59 | c89809b | OpenCode parser (SQLite-of-record) |
| `CSP-338` | `H-VIEWER-NATIVE-006` | 2026-06-02 18:59 | c89809b | Viewer widget: full-screen modal, scroll, jump-to-end-on-open |
| `CSP-339` | `H-VIEWER-NATIVE-007` | 2026-06-02 18:59 | c89809b | Substring search inside the viewer |
| `CSP-340` | `H-VIEWER-NATIVE-008` | 2026-06-02 18:59 | c89809b | Viewer-bridge integration: wire the `T` keybind into the native viewer |
| `CSP-341` | `H-VIEWER-NATIVE-009` | 2026-06-02 18:59 | c89809b | Retire patched recall from `pkgs/recall/` |
| `CSP-342` | `H-VIEWER-NATIVE-010` | 2026-06-02 18:59 | c89809b | (later) Extraction prep: lift `src/viewer/` into a workspace member crate |
| `CSP-343` | `H-VIEWER-NATIVE-011` | 2026-06-03 00:29 | c513ddd | Styling + spacing pass |
| `CSP-344` | `H-VIEWER-NATIVE-012` | 2026-06-03 00:58 | 53e2cd2 | Mouse bindings inside the viewer |
| `CSP-345` | `H-VIEWER-NATIVE-013` | 2026-06-03 00:58 | 53e2cd2 | In-viewer navigation into forks / child sessions |
| `CSP-346` | `H-VIEWER-NATIVE-014` | 2026-06-03 12:44 | 501fdce | Lazy / chunk-by-chunk transcript loading around compaction boundaries |
| `CSP-347` | `H-CONTINUE-001` | 2026-06-03 12:44 | 501fdce | ADR: usage-limit detection and scheduled continuation policy |
| `CSP-348` | `H-CONTINUE-002` | 2026-06-03 12:44 | 501fdce | Model blocked-session metadata |
| `CSP-349` | `H-CONTINUE-003` | 2026-06-03 12:44 | 501fdce | Detect usage-limit tails in supported transcript parsers |
| `CSP-350` | `H-CONTINUE-004` | 2026-06-03 12:44 | 501fdce | Surface blocked-until state in CLI and TUI |
| `CSP-351` | `H-CONTINUE-005` | 2026-06-03 12:44 | 501fdce | Implement explicit continue scheduling |
| `CSP-352` | `H-CONTINUE-006` | 2026-06-03 12:44 | 501fdce | Document blocked-session and continue workflows |
| `CSP-353` | `T8-043` | 2026-06-03 12:44 | 501fdce | Make Enter trigger the selected row's default action |
| `CSP-354` | `H-VIEWER-NATIVE-015` | 2026-06-03 13:09 | e4c7c33 | Per-message selection + clipboard copy |
| `CSP-355` | `H-VIEWER-NATIVE-016` | 2026-06-03 13:09 | e4c7c33 | Per-tool expand on click |
| `CSP-356` | `H-VIEWER-NATIVE-017` | 2026-06-03 13:09 | e4c7c33 | Markdown table rendering |
| `CSP-357` | `H-MUXPROC-019` | 2026-06-03 22:58 | dd273c7 | Investigate Claude Code Workflows process and session topology |
| `CSP-358` | `H-MUXPROC-016` (the 2026-06-04 story) | 2026-06-04 14:26 | 7be4499 | Treat harness session keys as opaque strings in runtime attribution |
| `CSP-359` | `H-MUXPROC-017` (the 2026-06-04 story) | 2026-06-04 14:26 | 7be4499 | Sweep remaining UUID-only extractor call sites |
| `CSP-360` | `H-OBS-007` | 2026-06-04 15:59 | baa1368 | Gate left-pane tree navigation keys on left-pane focus |
| `CSP-361` | `H-PIN-001` | 2026-06-04 21:32 | b41f363 | ADR: session pin schema, binding, and launch contract |
| `CSP-362` | `H-PIN-002` | 2026-06-04 21:32 | b41f363 | Pin schema + TOML round-trip |
| `CSP-363` | `H-PIN-003` | 2026-06-04 21:32 | b41f363 | Load pins into discovery as GraphLink candidates |
| `CSP-364` | `H-PIN-004` | 2026-06-04 21:32 | b41f363 | Resolver binding pass |
| `CSP-365` | `H-PIN-005` | 2026-06-04 21:32 | b41f363 | Extend store selection for pin writes |
| `CSP-366` | `H-PIN-006` | 2026-06-04 21:32 | b41f363 | Atomic write helpers for `[pins]` |
| `CSP-367` | `H-PIN-007` | 2026-06-04 21:32 | b41f363 | Pin CLI command tree skeleton |
| `CSP-368` | `H-PIN-008` | 2026-06-04 21:32 | b41f363 | CLI `pin list` and `pin show` |
| `CSP-369` | `H-PIN-009` | 2026-06-04 21:32 | b41f363 | CLI `pin create` / `rename` / `rm` |
| `CSP-370` | `H-PIN-010` | 2026-06-04 21:32 | b41f363 | Extend `TmuxRunner` with launch mutation seams |
| `CSP-371` | `H-PIN-011` | 2026-06-04 21:32 | b41f363 | `HarnessAdapter::launch_argv` defaults |
| `CSP-372` | `H-PIN-012` | 2026-06-04 21:32 | b41f363 | CLI `pin launch` and `pin attach` |
| `CSP-373` | `H-PIN-013` | 2026-06-04 21:32 | b41f363 | CLI `pin bind` (PinAmbiguous override) |
| `CSP-374` | `H-PIN-014` | 2026-06-04 21:32 | b41f363 | CLI `pin rebind` (external-rename recovery) |
| `CSP-375` | `H-PIN-015` | 2026-06-04 21:32 | b41f363 | CLI `pin adopt` |
| `CSP-376` | `H-PIN-016` | 2026-06-04 21:32 | b41f363 | TUI row tree integration |
| `CSP-377` | `H-PIN-017` | 2026-06-04 21:32 | b41f363 | TUI keybindings for pin actions |
| `CSP-378` | `H-PIN-018` | 2026-06-04 21:32 | b41f363 | Pin diagnostic surfaces in the TUI |
| `CSP-379` | `H-PIN-019` | 2026-06-04 21:32 | b41f363 | Read-only invariant audit |
| `CSP-380` | `H-PIN-020` | 2026-06-04 21:32 | b41f363 | Snapshot and JSON coverage |
| `CSP-381` | `H-PIN-021` | 2026-06-04 21:32 | b41f363 | Docs and operations guide |
| `CSP-382` | `H-PIN-F-001` | 2026-06-04 21:32 | b41f363 | Tmux non-default socket discovery enumeration |
| `CSP-383` | `H-PIN-F-002` | 2026-06-04 21:32 | b41f363 | Lifecycle hooks beyond `launch.argv` |
| `CSP-384` | `H-PIN-F-003` | 2026-06-04 21:32 | b41f363 | Importers from tmuxinator / tmuxp / smug configs |
| `CSP-385` | `H-PIN-F-004` | 2026-06-04 21:32 | b41f363 | Glob/wildcard pin patterns |
| `CSP-386` | `H-PIN-F-005` | 2026-06-04 21:32 | b41f363 | Absolute tmux socket paths (`tmux -S <path>`) |
| `CSP-387` | `H-PIN-022` | 2026-06-05 12:25 | d512f33 | TUI pin create flow |
| `CSP-388` | `H-PIN-023` | 2026-06-05 12:25 | d512f33 | TUI pin edit and remove flow |
| `CSP-389` | `H-PIN-024` | 2026-06-05 12:25 | d512f33 | TUI pin bind / rebind / adopt flows |
| `CSP-390` | `H-CMD-001` | 2026-06-05 18:40 | 24b7ccc | ADR: command palette surface and minibuffer scope |
| `CSP-391` | `H-CMD-002` | 2026-06-05 18:40 | 24b7ccc | Action registry: enumerate the command surface |
| `CSP-392` | `H-CMD-003` | 2026-06-05 18:40 | 24b7ccc | Command palette overlay (first deliverable) |
| `CSP-393` | `H-CMD-004` | 2026-06-05 18:40 | 24b7ccc | Minibuffer prompt (second deliverable, deferred) |
| `CSP-394` | `H-CMD-005` | 2026-06-05 18:40 | 24b7ccc | Migrate rename, search, and view-switch prompts into the minibuffer |
| `CSP-395` | `H-PIN-RESUME-001` | 2026-06-05 22:52 | eebe773 | Sidecar schema + atomic I/O helpers |
| `CSP-396` | `H-PIN-RESUME-002` | 2026-06-05 22:52 | eebe773 | `HarnessAdapter::resume_argv` method + defaults |
| `CSP-397` | `H-PIN-RESUME-003` | 2026-06-05 22:52 | eebe773 | Sidecar write pass post-resolve |
| `CSP-398` | `H-PIN-RESUME-004` | 2026-06-05 22:52 | eebe773 | Launch decision tree: sidecar consumer + lineage walk |
| `CSP-399` | `H-PIN-RESUME-005` | 2026-06-05 22:52 | eebe773 | `PinUnbound` diagnostic extension + UX surfaces |
| `CSP-400` | `H-PIN-RESUME-006` | 2026-06-05 22:52 | eebe773 | Invariants, snapshots, and closeout |
| `CSP-401` | `P10-FU-002` | 2026-06-07 02:38 | 732f914 | Audit GraphSnapshot round-trip and force coverage on future fields |
| `CSP-402` | `H-MUXPROC-020` | 2026-06-08 02:02 | 9912cd2 | Record the harness pid, not the hook writer's pid, in hook sidecar records |
| `CSP-403` | `H-MUXPROC-021` | 2026-06-08 02:02 | 9912cd2 | Demote `LinkedToMux` candidates whose source `AgentSession` is materially stale compared to a fresher candidate for the… |
| `CSP-404` | `H-ADR-0059-REVIEW` | 2026-06-08 21:23 | 540e589 | Review and resolve ADR 0059 (resolver rules-engine evaluation) |
| `CSP-405` | `H-WS-001` | 2026-06-09 14:15 | c4c8463 | Strict-only + chip in Sessions/Graph workspace nesting |
| `CSP-406` | `H-WS-002` | 2026-06-09 14:15 | c4c8463 | Dedicated Workspaces view (MVP) |
| `CSP-406.01` | `H-WS-002a` | 2026-06-09 18:04 | 71eec1a | Workspaces view polish: Provider/Activity/Repo groupings |
| `CSP-407` | `H-WS-003` | 2026-06-09 14:15 | c4c8463 | Audit Mux/Prs/Forks/Union workspace grouping for the same (A)/(B) conflation |
| `CSP-408` | `H-VIS-001` | 2026-06-09 14:32 | 1804cba | ADR: node-type visual identity system |
| `CSP-409` | `H-VIS-002` | 2026-06-09 14:32 | 1804cba | Define the per-node-type glyph and color assignments |
| `CSP-410` | `H-VIS-003` | 2026-06-09 14:32 | 1804cba | Apply node-kind glyphs and colors to the TUI row tree |
| `CSP-411` | `H-VIS-004` | 2026-06-09 14:32 | 1804cba | Apply node-kind glyphs and colors to the detail panel and graph explorer |
| `CSP-412` | `H-VIS-005` | 2026-06-09 14:32 | 1804cba | Surface node-kind identity in non-TUI outputs (cross-surface consistency) |
| `CSP-413` | `H-VIS-006` | 2026-06-09 14:32 | 1804cba | Docs and snapshot coverage |
| `CSP-414` | `H-WS-004` | 2026-06-14 01:24 | a256d0a | Hybrid workspace+repo grouping in Sessions / Graph |
| `CSP-415` | `H-UI-001` | 2026-06-17 15:15 | a3d7cd6 | Collapse per-session mux chip to an attachable-binary; let group rows own the ambiguity signal |
| `CSP-416` | `H-UI-002` | 2026-06-17 15:15 | a3d7cd6 | Weave per-node-kind glyph identity through every TUI surface (tree, detail, filter, help) |
| `CSP-417` | `H-UI-003` | 2026-06-17 15:15 | a3d7cd6 | Roll back the detail-pane upstream/downstream split; render a single related-entities list with descriptive edge labels |
| `CSP-418` | `H-UI-004` | 2026-06-17 15:15 | a3d7cd6 | Audit the sessions-pane header content holistically |
| `CSP-419` | `H-UI-005` | 2026-06-17 19:04 | c505fab | Resolved-vs-candidate visual separation in the detail-pane explorer |
| `CSP-420` | `H-UI-006` | 2026-06-17 19:04 | c505fab | Resolver-side preservation for suppressed ambiguous `LinkedToMux` resolutions |
| `CSP-421` | `H-UI-007` | 2026-06-17 19:04 | c505fab | Renderer-side fallback so candidate fan-out flags the explorer group as ambiguous even when no `ResolvedRelationship`… |
| `CSP-422` | `H-UI-008` | 2026-06-17 23:43 | 56d0456 | Left-pane tree views consume resolved relationships only |
| `CSP-423` | `F8-013` | 2026-06-18 23:17 | 6d42f4d | Persist last-active view across TUI restarts |
| `CSP-424` | `T8-044` | 2026-06-18 23:35 | d378797 | Spike: evaluate `tui-pantry` as a widget-iteration harness |
| `CSP-425` | `H-WIDG-001` | 2026-06-19 00:59 | 6b3079a | Adopt `ratatui-macros` for `Span` / `Line` / `Text` / layout boilerplate |
| `CSP-426` | `H-WIDG-002` | 2026-06-19 00:59 | 6b3079a | Swap `widgets/multi_select.rs` for `ratatui-cheese.multi_select` |
| `CSP-427` | `H-WIDG-003` | 2026-06-19 00:59 | 6b3079a | Swap `widgets/toast.rs` for `ratatui-toaster` |
| `CSP-428` | `H-WIDG-004` | 2026-06-19 00:59 | 6b3079a | Replace bordered-frame overlay code with `tui-popup` |
| `CSP-429` | `H-WIDG-005` | 2026-06-19 00:59 | 6b3079a | Adopt `ratatui-cheese.help` for the `?` help overlay |
| `CSP-430` | `H-WIDG-006` | 2026-06-19 00:59 | 6b3079a | Adopt `tui-textarea` when a multi-line input field lands on the backlog |
| `CSP-431` | `H-WIDG-007` | 2026-06-19 00:59 | 6b3079a | Adopt `tui-skeleton` for background-load placeholders |
| `CSP-432` | `H-WIDG-008` | 2026-06-19 00:59 | 6b3079a | Adopt `throbber-widgets-tui` for in-flight spinners |
| `CSP-433` | `H-WIDG-009` | 2026-06-19 00:59 | 6b3079a | Spike: evaluate `rat-widget` as a cohesive widget kit |
| `CSP-434` | `H-WIDG-010` | 2026-06-19 00:59 | 6b3079a | `ratatui-explorer` cwd picker for pin `create` / `adopt` |
| `CSP-435` | `H-WIDG-011` | 2026-06-19 00:59 | 6b3079a | `tui-tree-widget` extraction for the explorer |
| `CSP-436` | `H-WIDG-012` | 2026-06-19 14:45 | 0a962eb | Opportunistic `ratatui-macros` sweep across the remaining small widgets and the layout helpers |
| `CSP-437` | `H-WIDG-013` | 2026-06-20 04:05 | e8d8c73 | Port high-variant widgets into the `examples/pantry.rs` ingredient list (CSP-424 follow-up) |
| `CSP-438` | `P11-001` | 2026-06-22 21:57 | c3cd78f | ADR: retire SQLite persistence and query surface |
| `CSP-439` | `P11-002` | 2026-06-22 21:57 | c3cd78f | ADR: zero-copy snapshot format selection |
| `CSP-440` | `P11-003` | 2026-06-22 21:57 | c3cd78f | Add rkyv archive derives to the graph model |
| `CSP-441` | `P11-004` | 2026-06-22 21:57 | c3cd78f | Implement the snapshot format module |
| `CSP-442` | `P11-005` | 2026-06-22 21:57 | c3cd78f | Daemon dual-writes SQLite and the new artifact |
| `CSP-443` | `P11-006` | 2026-06-22 21:57 | c3cd78f | Add the `snapshot` socket command |
| `CSP-444` | `P11-007` | 2026-06-22 21:57 | c3cd78f | Cut the TUI refresh over to socket-served snapshots |
| `CSP-445` | `P11-008` | 2026-06-22 21:57 | c3cd78f | One-shot CLI daemon-or-rebuild path (mmap-fresh deferred) |
| `CSP-446` | `P11-009` | 2026-06-22 21:57 | c3cd78f | Daemon warm-start from the on-disk artifact |
| `CSP-447` | `P11-010` | 2026-06-22 21:57 | c3cd78f | Remove the `conspectus query` subcommand |
| `CSP-448` | `P11-011` (restored parent) | 2026-06-22 21:57 | c3cd78f | Delete `src/query/`, drop the `query` Cargo feature, retire the SQLite read surface |
| `CSP-448.01` | `P11-011a` | 2026-06-23 03:12 | cbfa6fe | Retire SQLite persistence layer; daemon + CLI use in-memory snapshot caches and `graph.bin` only |
| `CSP-448.02` | `P11-011b` | 2026-06-23 11:56 | 19db7d9 | Restore in-memory rendering for the output crate |
| `CSP-448.03` | `P11-011c` | 2026-06-23 12:06 | 5420b2b | Rewrite TUI row builders for forks / prs / union in-memory |
| `CSP-448.04` | `P11-011d` | 2026-06-23 12:06 | 5420b2b | Delete `src/query/`, drop the `query` Cargo feature, finish in-memory TUI rendering |
| `CSP-448.05` | `P11-011e` | 2026-06-24 02:06 | 0a5a04e | Revisit hook sidecar SQLite storage |
| `CSP-449` | `P11-012` | 2026-06-22 21:57 | c3cd78f | Supersede the SQLite ADR cluster and rewrite design.md |
| `CSP-450` | `P11-013` | 2026-06-22 21:57 | c3cd78f | Operator migration notes |
| `CSP-451` | `H-AGENTMUX-008` | 2026-06-24 02:52 | cf1761a | Route agent-deck mux renames through agent-deck |
| `CSP-452` | `H-PIN-TUI-001` | 2026-06-24 20:11 | d623512 | Pin create usability map and terminology pass |
| `CSP-453` | `H-PIN-TUI-002` | 2026-06-24 20:11 | d623512 | Name-driven defaults and override tracking for pin create |
| `CSP-454` | `H-PIN-TUI-003` | 2026-06-24 20:11 | d623512 | Make pin form fields editable at real-world lengths |
| `CSP-455` | `H-PIN-TUI-004` | 2026-06-24 20:11 | d623512 | Hybrid cwd omnibox for pin create/adopt |
| `CSP-455.01` | `H-PIN-TUI-004a` | 2026-06-25 14:06 | 63edb5a | Explicit row/edit focus for pin create navigation |
| `CSP-456` | `H-PIN-TUI-005` | 2026-06-24 20:11 | d623512 | Harness picker with free-form escape hatch |
| `CSP-457` | `H-PIN-TUI-006` | 2026-06-24 20:11 | d623512 | Launch argv editor with resolved command preview |
| `CSP-457.01` | `H-PIN-TUI-006a` | 2026-06-24 20:11 | d623512 | Harness launch option mappings for pin create |
| `CSP-458` | `H-PIN-TUI-007` | 2026-06-24 20:11 | d623512 | Post-create/adopt focus and toast behavior |
| `CSP-459` | `H-PIN-TUI-008` | 2026-06-25 19:23 | fdeef65 | Float pinned entities and keep Pins groups open |
| `CSP-460` | `H-PIN-TUI-009` | 2026-06-25 19:23 | fdeef65 | Promote pins to first-class graph entities |
| `CSP-461` | `H-PIN-TUI-010` | 2026-06-26 20:48 | de14a3d | Project pins as placeholder session and mux entities |
| `CSP-462` | `H-HYG-001` | 2026-07-01 22:56 | bfffbbd | Dedupe the copy-pasted micro-helpers |
| `CSP-463` | `H-HYG-002` | 2026-07-01 22:56 | bfffbbd | Extract shared TUI/output row-assembly helpers |
| `CSP-464` | `H-HYG-003` | 2026-07-01 22:56 | bfffbbd | Parameterize the centered-modal rect math |
| `CSP-465` | `H-HYG-004` | 2026-07-01 22:56 | bfffbbd | Remove the argv-sniffing fake-mtime test backdoor |
| `CSP-466` | `H-HYG-005` | 2026-07-01 22:56 | bfffbbd | Adopt a curated `[lints.clippy]` table and fix fallout |
| `CSP-467` | `H-HYG-006` | 2026-07-01 22:56 | bfffbbd | Introduce a `SnapshotIndex` for graph lookups |
| `CSP-468` | `H-HYG-007` | 2026-07-01 22:56 | bfffbbd | Declarative keybinding table for dispatch, overlays, and help |
| `CSP-469` | `H-HYG-008` | 2026-07-01 22:56 | bfffbbd | Unify the dual event loops, then split `runtime.rs` |
| `CSP-470` | `H-HYG-009` | 2026-07-01 22:56 | bfffbbd | Split the TUI monolith files by concern |
| `CSP-471` | `H-HYG-010` | 2026-07-01 22:56 | bfffbbd | Finish the `output::render` migration and settle `dev_scenarios` gating |
| `CSP-472` | `H-HYG-011` | 2026-07-01 22:56 | bfffbbd | Test builders and sibling-file test extraction (rolling) |
| `CSP-473` | `H-EXT-001` | 2026-07-01 22:56 | bfffbbd | Add a provider descriptor registry |
| `CSP-474` | `H-EXT-002` | 2026-07-01 22:56 | bfffbbd | Route harness pure-data lookups through the adapter registry |
| `CSP-475` | `H-EXT-003` | 2026-07-01 22:56 | bfffbbd | Key TUI harness colors by harness key |
| `CSP-476` | `H-EXT-004` | 2026-07-01 22:56 | bfffbbd | Move per-harness runtime signatures onto `HarnessAdapter` |
| `CSP-477` | `H-EXT-005` | 2026-07-01 22:56 | bfffbbd | Normalize hook payloads through the adapter |
| `CSP-478` | `H-EXT-006` | 2026-07-01 22:56 | bfffbbd | Provide transcript locator and parser via the adapter |
| `CSP-479` | `H-EXT-007` | 2026-07-01 22:56 | bfffbbd | Generalize the codex_log-style aux-reader wiring |
| `CSP-480` | `H-EXT-008` | 2026-07-01 22:56 | bfffbbd | Extract a `MuxBackend` trait from `TmuxRunner` |
| `CSP-481` | `H-EXT-009` | 2026-07-01 22:56 | bfffbbd | Capability-gate mux actions instead of naming tmux |
| `CSP-482` | `H-EXT-010` | 2026-07-01 22:56 | bfffbbd | Add a zellij mux backend (discovery + attach) |
| `CSP-483` | `H-EXT-011` | 2026-07-01 22:56 | bfffbbd | Capture hook mux context through the backend probe |
| `CSP-484` | `H-EXT-012` | 2026-07-01 22:56 | bfffbbd | Wire forges as an adapter list |
| `CSP-485` | `H-EXT-013` | 2026-07-01 22:56 | bfffbbd | Add a second forge adapter (GitLab or Gitea) |
| `CSP-486` | `H-EXT-014` | 2026-07-01 22:56 | bfffbbd | Add a generic orchestrator registration surface |
| `CSP-487` | `H-EXT-015` | 2026-07-01 22:56 | bfffbbd | Add an orchestrator mutation-capability seam (deferred) |
| `CSP-488` | `H-EXT-016` | 2026-07-01 22:56 | bfffbbd | Add adapter conformance suites per entity family |
| `CSP-489` | `H-EXT-017` | 2026-07-01 22:56 | bfffbbd | Write the provider-adapter contributor guide |
| `CSP-490` | `H-ADR-001` | 2026-07-01 22:56 | bfffbbd | Restate the payload-privacy tenet precisely |
| `CSP-491` | `H-ADR-002` | 2026-07-01 22:56 | bfffbbd | Replace "read-only first" with a defined mutation envelope |
| `CSP-492` | `H-ADR-003` | 2026-07-01 22:56 | bfffbbd | Retire or permanently bless the `[tui].sessions_grouping` legacy alias |
| `CSP-493` | `H-ADR-004` | 2026-07-01 22:56 | bfffbbd | Write the consolidated mux-attribution architecture note |
| `CSP-494` | `H-ADR-005` | 2026-07-01 22:56 | bfffbbd | Adopt a two-tier decision-record convention |
| `CSP-495` | `H-TUI-001` | 2026-07-02 02:16 | 556c102 | Make row trees derived view-models |
| `CSP-496` | `H-TUI-002` | 2026-07-02 02:16 | 556c102 | Adopt effects-as-data in the reducer |
| `CSP-497` | `H-TUI-003` | 2026-07-02 02:16 | 556c102 | Replace overlay Option slots with a modal stack and a shared Overlay contract |
| `CSP-498` | `H-TUI-004` | 2026-07-02 02:16 | 556c102 | Unify the event loops behind an event union and subscriptions |
| `CSP-499` | `H-TUI-005` | 2026-07-02 02:16 | 556c102 | Move scroll reconciliation into the reducer |
| `CSP-500` | `H-TUI-006` | 2026-07-02 21:01 | 6e0f0d7 | Unify overlay dispatch under the `Overlay` trait |
| `CSP-501` | `H-VIEW-001` | 2026-07-28 03:30 | 6fa9404 | Drop the Union view and hide the PRs and Forks views from the TUI |
| `CSP-502` | `H-LAYOUT-001` | 2026-07-28 03:30 | 6fa9404 | Make the column-reflow (narrow → stacked) threshold configurable |
| `CSP-503` | `H-PIN-ROOT-001` | 2026-07-28 03:30 | 6fa9404 | Surface pins registered in repos outside the active search root |
| `CSP-504` | `H-MUX-SORT-001` | 2026-07-28 03:30 | 6fa9404 | Add better recency sort options for the mux view |
| `CSP-505` | `H-SERVE-PERF-001` | 2026-07-28 03:30 | 6fa9404 | Debug `conspectus serve` resource usage |
| `CSP-505.01` | `H-SERVE-PERF-001a` | 2026-07-28 12:43 | 58e43c0 | Class-gate the process-tree mutator: `discover_local_warm_with` defers eviction of the `PROCESS_TREE_MUTATORS` slice… |
| `CSP-505.02` | `H-SERVE-PERF-001b` | 2026-07-28 12:43 | 58e43c0 | Dropped: `LocalDiscoveryConfig` is consumed per call (drains boxed backends) and `from_env()` is env-reads only… |
| `CSP-505.03` | `H-SERVE-PERF-001c` | 2026-07-28 14:13 | ac8ca1b | Cut the per-cycle resolve/publish cost (deferred; revisit only on the triggers below) |
| `CSP-506` | `H-WT-001` | 2026-07-28 03:30 | 6fa9404 | Integrate first-class worktree management with pluggable backends |
| `CSP-507` | `H-WT-002` | 2026-07-28 12:43 | 58e43c0 | `WorktreeBackend` trait + registry + thin `git` read/list backend… |
| `CSP-508` | `H-WT-003` | 2026-07-28 12:43 | 58e43c0 | `worktrunk` backend (create/remove) behind `PATH` autodetection + `[worktree] backend` config |
| `CSP-509` | `H-WT-004` | 2026-07-28 12:43 | 58e43c0 | `conspectus worktree new/rm` CLI + menu-first TUI actions gated on a mutation-capable backend |
| `CSP-509.02` | `H-WT-004b` | 2026-08-04 04:13 | 6369426 | TUI worktree **action menu** (`w`) wired to the already-built safe actions (create, remove)… |
| `CSP-510` | `H-SERVE-PERF-002` | 2026-07-28 15:27 | a80a7fc | Cache `codex_log` DB scan across cycles |
| `CSP-511` | `H-SERVE-PERF-003` | 2026-07-28 15:27 | a80a7fc | Fingerprint-gated `/proc` walk (001a follow-up) |
| `CSP-512` | `H-SERVE-PERF-004` | 2026-07-29 02:07 | 90e9dd4 | Cache `GitProbe::probe` results across cycles keyed on `.git/HEAD` / `config` / `refs/heads/` / `packed-refs` mtimes… |
| `CSP-513` | `H-SERVE-PERF-005` | 2026-07-29 02:07 | 90e9dd4 | TTL-cache `ForgeDiscovery` output (`9a691a3`) |
| `CSP-514` | `H-SERVE-PERF-006` | 2026-07-29 02:07 | 90e9dd4 | Fix `codex::read_session_meta` full-file slurp (`3495367`) |
| `CSP-515` | `H-SERVE-PERF-007` | 2026-07-29 02:07 | 90e9dd4 | Per-file `(mtime, size)` cache for claude + codex session header/tail scans (`9b9eec3`) |
| `CSP-516` | `H-SERVE-PERF-008` | 2026-07-29 02:07 | 90e9dd4 | Refine CSP-511's fingerprint to zero mux `activity_epoch`, `last_attached_epoch`… |
| `CSP-517` | `H-SERVE-PERF-009` | 2026-07-29 02:07 | 90e9dd4 | Extend 008 to also zero `last_message_preview`, `title`, `created_epoch` (`aac498c`) |
| `CSP-518` | `H-SERVE-PERF-010` | 2026-07-29 02:07 | 90e9dd4 | mtime-cache the opencode.db SQL scan (`148763d`) |
| `CSP-519` | `H-SERVE-PERF-011` | 2026-07-29 02:07 | 90e9dd4 | TTL-cache `TmuxDiscovery` + `ZellijDiscovery` output (`03ad86b`) |
| `CSP-520` | `H-WT-ENV` | 2026-08-05 02:51 | 0e4abe0 | ADR: sanction `tmux kill-session` as an operator-initiated teardown mutation (ADR 0087 category-3 extension) +… |
| `CSP-521` | `H-WT-005` | 2026-08-05 02:51 | 0e4abe0 | `merge` — `WorktrunkBackend::merge` (`wt -C <wt> merge [target]`) + `WorktreeCaps.can_merge`… |
| `CSP-522` | `H-WT-006` | 2026-08-05 02:51 | 0e4abe0 | `close-down` compound orchestrator (ADR 0093) |
| `CSP-523` | `H-WT-007` | 2026-08-05 02:51 | 0e4abe0 | new-stream: worktree-backed pins realized at launch (ADR 0094) |
| `CSP-524` | `H-WT-008` | 2026-08-05 02:51 | 0e4abe0 | prune + reveal/navigate |
| `CSP-525` | `H-PIN-EDIT-MUX-001` | 2026-08-08 03:23 | 6ffec82 | Pin edit / delete does not resolve a pin when a pinned mux row is selected in the mux view |
| `CSP-526` | `H-HARNESS-ATELIER-001` | 2026-08-08 03:23 | 6ffec82 | Atelier `exec claude` panes render as "No agent" in the TUI / CLI |
| `CSP-527` | `H-MUX-NEW-001` | 2026-08-08 03:23 | 6ffec82 | Create bare tmux sessions from within Conspectus (no pin, no agent) |
| `CSP-528` | `H-PIN-TUI-011` | 2026-09-28 15:11 | c6905bb | Numeric-suffix auto-increment for derived pin names |
| `CSP-529` | `H-MUX-LAUNCH-001` | 2026-09-30 16:57 | 457824e | Launch a harness in a fresh mux without persisting a pin (ADR 0096) |
| `CSP-530` | `H-MUX-LAUNCH-002` | 2026-09-30 16:57 | 1fc63c9 | Extract the shared launch-spec form primitive (ADR 0097) |
| `CSP-531` | `H-PIN-RESUME-ARGV-001` | 2026-09-30 19:09 | 12e2254 | Pin resume drops the pin's launch argv (ADR 0098) |
| `CSP-532` | `REL-001` | 2026-09-30 18:30 | 0847c01 | License and third-party notices |
| `CSP-532.01` | `REL-001a` | 2026-09-30 18:30 | 0847c01 | Add `LICENSE` |
| `CSP-532.02` | `REL-001b` | 2026-09-30 18:30 | 0847c01 | Correct the vendored-asset notice |
| `CSP-533` | `REL-002` | 2026-09-30 18:30 | 0847c01 | Pre-publication review, then flip the repository public |
| `CSP-533.01` | `REL-002a` | 2026-09-30 18:30 | 0847c01 | Pre-publication review |
| `CSP-533.02` | `REL-002b` | 2026-09-30 18:30 | 0847c01 | Flip the repository public |
| `CSP-534` | `REL-003` | 2026-09-30 18:30 | 0847c01 | Fix demo-visible TUI defects |
| `CSP-534.01` | `REL-003a` | 2026-09-30 18:30 | 0847c01 | Render relative ages in the detail pane |
| `CSP-534.02` | `REL-003b` | 2026-09-30 18:30 | 0847c01 | Stop advertising `m choose` on ambiguous-mux rows |
| `CSP-534.03` | `REL-003c` | 2026-09-30 18:30 | 0847c01 | Make the help overlay readable |
| `CSP-534.04` | `REL-003d` | 2026-09-30 18:30 | 0847c01 | Keep related-row labels on their row |
| `CSP-534.05` | `REL-003e` | 2026-09-30 18:30 | 0847c01 | Fix the Mux view's session count |
| `CSP-535` | `REL-004` | 2026-09-30 18:30 | 0847c01 | True up CLI `--help` |
| `CSP-535.01` | `REL-004a` | 2026-09-30 18:30 | 0847c01 | Describe `--refresh` and `--no-cache` as they behave today |
| `CSP-535.02` | `REL-004b` | 2026-09-30 18:30 | 0847c01 | Remove internal IDs and stale text from help, and add a regression test |
| `CSP-536` | `REL-005` | 2026-09-30 18:30 | 0847c01 | True up the docs |
| `CSP-536.01` | `REL-005a` | 2026-09-30 18:30 | 0847c01 | Remove references to retired commands |
| `CSP-536.02` | `REL-005b` | 2026-09-30 18:30 | 0847c01 | Bring `docs/design.md` in line with the accepted ADRs |
| `CSP-536.03` | `REL-005c` | 2026-09-30 18:30 | 0847c01 | Complete the `docs/operations.md` reference |
| `CSP-536.04` | `REL-005d` | 2026-09-30 18:30 | 0847c01 | Fix developer-facing claims |
| `CSP-537` | `REL-006` | 2026-09-30 18:30 | 0847c01 | Retire `docs/feature-summary.md` |
| `CSP-538` | `REL-007` | 2026-09-30 18:30 | 0847c01 | True up backlog checkboxes |
| `CSP-539` | `REL-008` | 2026-09-30 18:30 | 0847c01 | Write the 0.1.0 CHANGELOG entry |
| `CSP-540` | `REL-009` | 2026-09-30 18:30 | 0847c01 | Talk and demo assets |
| `CSP-540.01` | `REL-009a` | 2026-09-30 18:30 | 0847c01 | Refresh the showcase fixture |
| `CSP-540.02` | `REL-009b` | 2026-09-30 18:30 | 0847c01 | Capture screenshots and graph exports |
| `CSP-540.03` | `REL-009c` | 2026-09-30 18:30 | 0847c01 | Add an optional `just demo` recipe |
| `CSP-540.04` | `REL-009d` | 2026-09-30 18:30 | 0847c01 | Refresh the README for the talk |
| `CSP-541` | `REL-010` | 2026-09-30 18:30 | 0847c01 | Define the release process and cut `v0.1.0` |
| `CSP-542` | `REL-011` | 2026-09-30 18:30 | 0847c01 | Add installation paths beyond `--path` |
| `CSP-543` | `REL-012` | 2026-09-30 18:30 | 0847c01 | Settle platform posture and CI shape |
| `CSP-544` | `REL-013` | 2026-09-30 18:30 | 0847c01 | Clean up dependency and tooling leftovers |
| `CSP-545` | `REL-014` | 2026-09-30 18:30 | 0847c01 | Make the on-disk footprint consistent with ADR 0087 |
| `CSP-546` | `REL-015` | 2026-09-30 18:30 | 0847c01 | Consolidate harness-hook docs |
| `CSP-547` | `REL-016` | 2026-09-30 18:30 | 0847c01 | Complete the documentation index and add an ADR index |
| `CSP-548` | `REL-017` | 2026-09-30 18:30 | 0847c01 | Decide how public docs refer to Atelier |
| `CSP-549` | `REL-018` | 2026-09-30 18:30 | 0847c01 | Fix or document known functional gaps |
| `CSP-550` | `REL-019` | 2026-09-30 18:30 | 0847c01 | Close the ADR-alignment items that shape the public story |
| `CSP-551` | `REL-020` | 2026-09-30 18:30 | 0847c01 | Add contributor onboarding |
| `CSP-552` | `REL-021` | 2026-09-30 18:30 | 0847c01 | Decide the backlog's long-term shape |
| `CSP-553` | `REL-022` | 2026-09-30 18:30 | 0847c01 | Decide the library API posture |
| `CSP-554` | `H-RUST-001` | 2026-10-01 00:23 | bc8bf90 | Apply idiomatic clippy fixes and enforce them (`a4eaf2f`) |
| `CSP-555` | `H-RUST-002` | 2026-10-01 00:23 | bc8bf90 | Use sets where maps held `()` or placeholder values (`5c3e2f7`) |
| `CSP-556` | `H-RUST-003` | 2026-10-01 00:23 | bc8bf90 | Look up nodes without cloning their ids (`30f1f7d`) |
| `CSP-557` | `H-RUST-004` | 2026-10-01 00:23 | bc8bf90 | Remove tombstone comments ("X moved to Y in wave N") and five copies of a current-epoch helper (`a381856`) |
| `CSP-558` | `H-RUST-005` | 2026-10-01 00:23 | bc8bf90 | Stop deep-cloning `GraphSnapshot` to release a borrow in TUI pin/rename executors… |
| `CSP-559` | `H-RUST-006` | 2026-10-01 00:23 | bc8bf90 | Remove two speculative traits: `MultiSelectItem` became `AsRef<str>` (`48ef2f4`)… |
| `CSP-560` | `H-RUST-007` | 2026-10-01 00:23 | bc8bf90 | Fix all 44 rustdoc warnings and add a warnings-as-errors `cargo doc` step to `just check` and CI (`aaf4a05`) |
| `CSP-561` | `H-RUST-008` | 2026-10-01 00:23 | bc8bf90 | Remove dead code hidden by `#[allow(dead_code)]`… |
| `CSP-562` | `H-RUST-009` | 2026-10-01 00:23 | bc8bf90 | Recover poisoned locks in the daemon with `lock().unwrap_or_else(PoisonError::into_inner)` instead of ten hand-written… |
| `CSP-563` | `H-RUST-010` | 2026-10-01 00:23 | bc8bf90 | Replace the process-global discovery caches |
| `CSP-564` | `H-RUST-011` | 2026-10-01 00:23 | bc8bf90 | Give the library typed errors |
| `CSP-565` | `H-RUST-012` | 2026-10-01 00:23 | bc8bf90 | Narrow the public surface |
| `CSP-566` | `H-RUST-013` | 2026-10-01 00:23 | bc8bf90 | Strip backlog IDs from code comments |
| `CSP-567` | `H-RUST-014` | 2026-10-01 00:23 | bc8bf90 | Use typed kinds instead of strings |
| `CSP-568` | `H-RUST-015` | 2026-10-01 00:23 | bc8bf90 | Split the 3,000-line TUI modules |
| `CSP-569` | `H-RUST-016` | 2026-10-01 00:23 | bc8bf90 | Retire `SessionViewerAction` |
| `CSP-570` | `H-RUST-017` | 2026-10-01 00:23 | bc8bf90 | Rename `GraphDb` |
| `CSP-571` | `H-RUST-018` | 2026-10-01 00:23 | bc8bf90 | Smaller follow-ups, as files are touched |
| `CSP-572` | `H-RUST-019` | 2026-10-01 00:23 | bc8bf90 | Extend lint enforcement after the chunks land |
| `CSP-573` | `H-RUST-020` | 2026-10-01 03:27 | b2459c9 | Decide how Codex-log evidence ranks in mux resolution |
| `CSP-574` | `H-PIN-FIX-001` | 2026-10-01 18:04 | 1d6a83c | One-to-one pin bindings (ADR 0102) |
| `CSP-575` | `H-PIN-FIX-002` | 2026-10-01 18:04 | 1d6a83c | Launches keep failed panes and confirm the start (ADR 0103) |
| `CSP-576` | `H-PIN-FIX-003` | 2026-10-01 18:04 | 1d6a83c | Pin placeholder rows preview their live mux |
| `CSP-577` | `H-PIN-FIX-004` | 2026-10-01 18:04 | 1d6a83c | Refresh after tmux hand-offs shows current state (ADR 0104) |
| `CSP-578` | `H-PIN-FIX-005` | 2026-10-01 18:04 | 1d6a83c | TUI message log for operation outcomes (ADR 0105) |
| `CSP-579` | `H-RUST-021` | 2026-10-02 16:58 | 6817947 | Don't abort discovery on one unreadable scan-root child |
| `CSP-580` | `H-PREVIEW-WRAP-001` | 2026-10-02 19:04 | 4ebfc7d | Preview pane hides quiet panes' output and wraps agent UI decorations (ADR 0106) |
| `CSP-581` | `H-UI-009` | 2026-10-02 19:18 | 98ce651 | Related view at neighbor granularity; split corroborating from competing candidates (ADR 0107) |
| `CSP-582` | `TEST-008` | 2026-10-02 23:15 | 69b3850 | Make `tui --snapshot --snapshot-keys` honor pane focus so scripts can drive the right pane |
| `CSP-583` | `H-HANDOFF-LATENCY-001` | 2026-10-02 23:54 | 82792fb | Returning from tmux redraws the TUI without waiting for the refresh (ADR 0108) |
| `CSP-584` | `H-HANDOFF-LATENCY-002` | 2026-10-02 23:54 | 82792fb | Cheaper post-hand-off rescans |
