# Live Agent State: Plan

Status: drafted 2026-11-12 on branch `feat/live-state` (milestone m-26);
phase 1 built on the same branch (CSP-806 to CSP-821). Section 9 records
where the build differs from this draft. Evidence: the six research notes
in [`research/`](research/). Decisions: ADRs 0203 to 0207, and 0208 for the
Claude Code settings edit. Operator guide: "Live agent state" in
[`docs/operations.md`](../../operations.md#live-agent-state).

## 1. Goal

Show, at a glance, what each running agent session and each mux is doing
right now, in the operator's three buckets:

- **(a) Waiting on me**: a permission prompt, a question, a plan to approve,
  an elicitation.
- **(b) Ready for a new prompt**: the turn is over.
- **(c) Being worked on**: the model is generating, a tool is running, a
  subagent is active, context is compacting.

Plus the states around them: error and rate limit, starting, exited, and
"we can't tell".

Non-goals:

- Acting on a waiting agent. Conspectus observes. The action is attach, the
  existing gesture (ADR 0028, ADR 0143 prohibition 2).
- Persisting any prompt, message, tool input, or tool output (ADR 0086).
- Owning the agent loop. Conspectus surveys sessions it did not start.
- A bespoke terminal-scraping engine. Every tool surveyed that scrapes the
  screen has an issue tracker full of regressions after harness UI changes
  ([05](research/05-orchestrators-and-ux.md) §4).

## 2. What the research found

Per harness, in one table (detail in the notes):

| | Claude Code | Codex | OpenCode | Pi |
|---|---|---|---|---|
| Push channel | 33 hook events | 14 hook events | plugin events | extension events |
| WAITING | `PermissionRequest` (immediate), `PreToolUse` on `AskUserQuestion`/`ExitPlanMode` | `PermissionRequest` | `permission.asked`/`question.asked` (1.x), `form.*` (2.x) | `ui_prompt_start`/`end` (Pi >= 0.84.4); only extension dialogs, Pi has no built-in approvals |
| READY | `Stop` | `Stop`, `Interrupt` | `session.status idle` | `agent_settled` |
| Subagents | `SubagentStart`/`Stop`, `agent_id` | `SubagentStart`/`Stop` | child sessions with `parentID` | none built in; a `subagent` tool convention |
| Error / rate limit | `StopFailure` (typed) | rollout/`PostModelRequest` | `session.error`, retry status | `message_end` stopReason, classified in the extension |
| Compacting | `PreCompact`/`PostCompact` | `PreCompact`/`PostCompact` | compaction events | `session_before_compact`/`session_compact` |
| Gap: interrupt (Esc) | no `Stop` | `Interrupt` event | `execution.interrupted` (2.x) | `agent_settled{aborted}` |
| Gap: approval granted | no event until the tool ends | none | `permission.replied` | `ui_prompt_end` |
| Gap: killed process | no `SessionEnd`; use pid | no exit event; use pid | `dispose` only on clean exit | SIGKILL sends nothing; use pid |
| Hook-free fallback | `claude agents --json`; transcript tail; pane title | pane title (`[ ! ] Action Required`, spinner); rollout tail | storage polling (2.x columns) | session-file tail (medium confidence) |
| Install today | `SessionStart` only | `SessionStart` only; likely inert on current Codex until trusted in `/hooks`; can clobber operator hooks | plugin forwards the event name only | extension forwards `session_start` only |

Cross-cutting findings that shape the design
([06](research/06-internal-audit.md) §0):

1. The hook pipeline records **attribution** (which session is in which
   mux), not **state**. Nothing installed today reports permission waits,
   stops, or tool activity.
2. The spool keeps **one record per tmux session**: split-pane agents
   overwrite each other, timestamps have one-second resolution, no TTL is
   applied (`ACTIVE_TTL_SECONDS` is dead code), and it cannot hold history.
   Live state needs its own store.
3. Every hook ingest to the daemon costs a full resolve, rkyv
   re-serialization, and `graph.bin` rewrite, and the socket client has no
   timeout. State events (potentially two per tool call) must not use it.
4. There is **no push channel to the TUI**: it polls the whole snapshot every
   30 s. A hook event would take up to 30 s to show.
5. The graph has no pane node and no per-node attribute bag. The newest
   precedent for sparse time-varying facts is the `node_presence` side table
   (ADR 0140).
6. The session row's state cell and the braille spinner are taken; live state
   needs its own column and its own marks (ADR 0145).
7. Harnesses compute these states themselves and tell terminals via a status
   escape sequence (OSC 7501), but tmux does not forward it, so Conspectus
   has to be told by hook or plugin.

## 3. Architecture, inside-out

```
 harness process                      Conspectus
 ───────────────                      ──────────────────────────────────────────────
 hooks / plugin ──► hook write ──► [L2] live-state store ──► [L3] bind ──► node_live ──► [L5] CLI / JSON
   (L1 emitters)   (fast, silent)     $STATE/…/live-state.json   mutator       side table     [L6] TUI (parked)
                     │                  per-session folded           ▲  also: TUI overlay re-binds
                     │                  entries, pid, ordering       │  from the store ≤1 s
                     └── attribution (SessionStart) ─► existing spool + daemon ingest, unchanged
 [L0] vocabulary: model::live (state, detail, effective(), counts) used by every layer
```

| Layer | What | Decision |
|---|---|---|
| L0 | Neutral vocabulary and rules in `src/model` | ADR 0203 |
| L1 | Emitters: hook entries and plugins, one reducer per harness | ADR 0206 |
| L2 | The live-state store: a per-session folded state file, outside the attribution spool | ADR 0204 |
| L3 | Binding the store to nodes at discovery, and again in the TUI | ADR 0205 |
| L4 | The `node_live` snapshot side table | ADR 0205 |
| L5 | CLI and JSON surfaces | stories |
| L6 | TUI rendering | parked, section 7 |

### 3.1 Why a new store, and why not the daemon

The store is the integration point, not the daemon.

- `hook write` appends the event's effect to the store under a short advisory
  lock and exits. It never waits on a daemon, never spawns tmux after the
  first sight of a session, never prints, and always exits 0. Cost target:
  under 15 ms per event, measured ([01](research/01-claude-code.md) §0 saw
  about 5 to 10 ms).
- The daemon's class cycle (5 s) re-binds the store into `node_live`, so
  `graph.bin` and the socket snapshot carry state at most 5 s old, with no
  per-event resolve or `graph.bin` write.
- The TUI, with or without a daemon, stats the store each tick, and when it
  changes (at most once a second) re-reads it and re-binds against the
  snapshot it already holds. An event is visible in about a second, and the
  daemon stays optional (design requirement: the one-shot CLI and the
  daemonless TUI must work). Between binds it also checks the bound
  agents' processes once a second, so an agent killed without a goodbye
  reads `exited/killed` within a second rather than at the next snapshot.
- Reducers for stateless hooks (Claude Code, Codex) run in Rust inside
  `hook write`, folding each event into the stored per-session entry (as
  built, one shared reducer over neutral events; section 9). Plugins
  with in-process state (OpenCode, Pi) send a complete snapshot of their own
  state with an instance id and a sequence number, and the store replaces the
  entry when it is newer. Both end in the same entry shape.

### 3.2 State model (ADR 0203)

Closed set, harness-neutral:

| State | Details | Meaning |
|---|---|---|
| `working` | `generating`, `tool`, `retrying`, `rate_limited` | a turn is running |
| `waiting` | `permission`, `question`, `plan`, `input` | blocked on the operator |
| `ready` | `finished`, `interrupted` | the turn is over; accepts a prompt |
| `compacting` | | context compaction is running |
| `error` | `rate_limited`, `auth`, `overloaded`, `failed` | the turn ended on an error |
| `starting` | | the process is up, no prompt yet |
| `exited` | `ended`, `killed` | the session is over; `killed` means the process vanished without a goodbye |
| `unknown` | `stale` | no evidence, or too old |

Modifiers on the observation: `subagents` (active child count), `via_subagent`
(the wait or work comes from a child, so it is raised to the root session),
`since` (when this state began), `observed` (last evidence), `source`
(`hook`, ...), and the process `pid` plus its start time.

"Seen vs unseen" (`ready` that the operator has not looked at yet) is **not**
stored. It is derived by the viewer from `since` and the mux's
`last_attached_epoch`, if the operator wants it (section 7).

Effective state is computed at read time from `now` and process liveness,
never stored:

1. The pid is dead, or its start time changed: `exited/killed`.
2. `working`, `compacting`, and `starting` past the quiet window (10 min for
   event-only sources, 3 heartbeats for sources that heartbeat) with a live
   pid stay what they are but are marked **quiet** (a 40-minute build is
   still working). They never silently become idle.
3. `waiting`, `ready`, `error` never decay while the pid lives. An operator
   leaves a prompt for hours.
4. No pid and no other liveness evidence: past 15 minutes (today's hook TTL),
   everything but `exited` becomes `unknown/stale`.
5. `starting` older than 2 minutes without a follow-up is `unknown/stale`.

Absence of an entry means "no evidence", not "idle" (ADR 0140's rule for
presence applies).

A mux has no state of its own. It has **counts** over the sessions it hosts
(`waiting`, `error`, `ready`, `working`, ...), plus any pane-only entries.
Which state dominates is a display decision (section 7); the data layer
provides the counts and a default precedence function.

### 3.3 The store (ADR 0204)

`$XDG_STATE_HOME/conspectus/live-state.json`, category 2 (rebuildable
observation, already inside the manifest's `$STATE/conspectus/**` write
path), plus a lock file.

- Key: `harness:session_id`. Entries are pane-aware: each stores the tmux
  socket, session name, and pane id when known, so two agents in one tmux
  session never overwrite each other.
- **Ordering:** a store-wide revision counter assigned under the lock, plus
  milliseconds. Plugin snapshots also carry `(instance, seq)`.
- **Supersede:** one live entry per (harness, pid). A new session id from the
  same process (`/clear`, `/new`, resume) ends the old entry.
- **GC:** exited entries past an hour and dead-pid entries past a day are
  dropped on write; the file is capped at 512 entries.
- **Privacy:** entries hold enums, counts, ids, paths, and times. The writer
  parses payloads in memory against a typed whitelist and drops the rest
  ([01](research/01-claude-code.md) §6.2 has the Claude Code list). Tests plant
  sentinel strings in every payload-bearing field and assert none reaches the
  store. Even `tool_name` is not stored: it is used to classify
  (`AskUserQuestion` becomes `question`) and dropped.

### 3.4 Graph representation (ADR 0205)

A snapshot side table, `node_live: BTreeMap<NodeId, LiveObservation>`, next to
`node_presence` and `node_provenance` (as built, its values are
`LiveBinding`s, the observation with the liveness the binder found;
section 9):

- Keyed by `AgentSession` id (exact match on harness and native session id).
  Entries that have a pane but no known session ("pane-only") go to a second
  sparse table, `pane_live`, keyed by the tmux `MuxSession` id and the pane, so
  two such agents in one mux each count (CSP-841).
- Produced by a new discovery mutator, `live_state`, that runs after the
  harness providers and the hook sidecar, like the hook sidecar does today,
  and by the TUI overlay re-bind. Both call one pure function.
- Not on node structs: that churns the identity nodes, the process-tree
  fingerprint (ADR 0091), and 31 struct literals in the library API.
- Not graph links: a state is not a relationship between two nodes, and
  `ResolvedRelationship.target` is a `NodeId`.
- Bumps the rkyv `FORMAT_VERSION` (to 12, with both tables), skipped from
  JSON only if empty.

### 3.5 Install and emission (ADR 0206)

- `hook init <harness>` is idempotent and installs the full set for that
  harness; re-running upgrades an older install. Nothing changes without the
  operator running it. `hook status` reports per-event coverage and a health
  line ("the agent is active but no events arrived": hooks untrusted or
  disabled), `hook remove` removes every entry it added.
- **Claude Code:** 14 events: `SessionStart` (all sources, including `fork`),
  `UserPromptSubmit`, `PermissionRequest`, `PreToolUse` limited to
  `AskUserQuestion|ExitPlanMode`, `PostToolUse`, `PostToolUseFailure`,
  `Notification` (typed matcher), `SubagentStart`, `SubagentStop`, `Stop`,
  `StopFailure`, `PreCompact`, `PostCompact`, `SessionEnd`. Every entry has a
  5 s timeout. The writer never prints and never exits 2.
- **Codex:** `SessionStart`, `UserPromptSubmit`, `PermissionRequest`,
  `PostToolUse`, `Stop`, `Interrupt`, `SessionEnd`, `SubagentStart`,
  `SubagentStop`, `PreCompact`, `PostCompact`, each its own matcher group next
  to the operator's. `notify` is never touched. The installer defects (it
  replaces an operator's `[[hooks.SessionStart]]` tables; no timeout) are
  fixed first, and the `/hooks` trust step is explained, never written.
- **OpenCode, Pi:** the plugin/extension keeps a small reducer and sends a
  snapshot only on change, plus a 60 s heartbeat while not idle. The
  writer is a spawned `conspectus hook write` (detached, not awaited), never
  one per streaming event.
- Hook-free sources are out of phase 1 (ADR 0207).

## 4. Cost and latency budget

| Path | Budget | Mechanism |
|---|---|---|
| Hook to store | under 15 ms, never blocks the harness | no daemon call, no tmux spawn after first sight, lock timeout 200 ms then drop the event |
| Store to TUI | about 1 s | mtime stat per tick, re-read at most 1 per second, re-bind in memory |
| Killed agent to TUI | about 1 s | between binds, at most once a second: one read of `/proc/<pid>/stat` per bound entry whose process may still run (not a recorded exit, not already found gone); no store read, no process started, nothing when no entry is bound; ticks in between (100 ms) check nothing (CSP-843) |
| Store to daemon snapshot | at most 5 s | class-cycle mutator |
| Idle cost | none | no timer in the daemon; plugin heartbeat 60 s only while not idle |

## 5. Phasing

Phase 1 (this plan's implementation, no TUI decisions needed):

1. Vocabulary and rules (L0), the side table (L4), the store (L2).
2. The writer fast path and the Claude Code vertical slice (L1): reducer,
   fixtures, install, status.
3. Binding and the mutator (L3), CLI surfaces (L5), TUI plumbing that
   populates row data without rendering it.
4. Codex, then OpenCode, then Pi.
5. Attribution-path fixes the research found, as bugs in their own right.
6. Operator docs.

Phase 2 (parked on operator decisions, section 7): TUI rendering, and any
hook-free source.

Manual verification (needs a real session of each harness and is the
operator's call): the UNVERIFIED items in each note's appendix, and
recording the fixtures with a redacting recorder. Phase 1 fixtures are
synthetic, built from the documented payload shapes.

### 5.1 Stories (milestone m-26)

| Order | Story | Task |
|---|---|---|
| 1 | Record the decisions and plan | CSP-806 (done) |
| 2 | Vocabulary and rules in the model | CSP-807 (done) |
| 3 | `node_live` snapshot side table | CSP-808 (done) |
| 3 | The live-state store | CSP-809 (done) |
| 4 | `hook write` as a fast, silent writer | CSP-810 (done) |
| 5 | Claude Code: reducer, fixtures, install | CSP-811 (done) |
| 6 | Bind live state at discovery | CSP-812 (done) |
| 7 | CLI output | CSP-813 (done) |
| 7 | TUI plumbing (data on row models, nothing rendered) | CSP-814 (done) |
| 8 | Codex, OpenCode, Pi | CSP-815, CSP-816, CSP-817 (done) |
| any | Spool defects, key order, Pi mtime | CSP-818, CSP-819, CSP-820 (done) |
| 9 | Operator guide | CSP-821 (done) |
| 10 | Real-payload verification (needs the operator) | CSP-822 |
| 11 | TUI live cell and Pinned words (ADR 0209) | CSP-823 (done) |
| 12 | TUI group headers count waiting agents (ADR 0210) | CSP-824 (done) |
| 13 | TUI header chip for waiting agents (ADR 0211) | CSP-825 (done) |
| 14 | TUI subagent chip on session rows (ADR 0212) | CSP-854 (done) |
| 15 | TUI detail pane fields, Pinned and History words (ADR 0213) | CSP-829 (done) |
| 16 | TUI seen and unseen ready turns (ADR 0214) | CSP-828 (done) |
| 17 | TUI float waiting first, agent-state filter, `--state` (ADR 0215) | CSP-826 (done) |
| 18 | TUI `W` jumps to the agent waiting longest (ADR 0216) | CSP-827 (done) |
| decided | TUI notify (deferred) | CSP-830 |
| parked | Hook-free sources (pane title, `claude agents --json`, tails, other orchestrators) | CSP-831 to CSP-834 |

Follow-ups the stories filed (labels `live-state`, milestone m-26):

| Task | Follow-up | Status |
|---|---|---|
| CSP-836 | Bound the daemon socket calls the TUI and CLI make | To Do |
| CSP-837 | Stop reading the undocumented `CLAUDE_CODE_VERSION` in the hook writer | To Do |
| CSP-838 | Honor `CLAUDE_CONFIG_DIR` for Claude Code discovery and hook install | To Do |
| CSP-839 | Keep a stable `conspectus` path in installed hook commands | To Do |
| CSP-840 | Parse only the whitelisted fields of a hook payload in `hook write` | To Do |
| CSP-841 | Count every pane-only agent on its mux (`pane_live`) | Done |
| CSP-842 | Name running agents without live state in `hook status` | To Do |
| CSP-843 | Re-check bound agents' processes between snapshots in the TUI | Done |
| CSP-844 | Honor `CODEX_HOME` for Codex discovery | To Do |
| CSP-845 | Ask tmux about the hook's own pane in `hook write` | Done |
| CSP-846 | OpenCode 2.x: live state from a TUI-only plugin install | To Do |
| CSP-847 | OpenCode 2.x: re-read pending permission asks when the server plugin starts | To Do |
| CSP-848 | Report opencode's live-state health in `hook status` | To Do |
| CSP-849 | Decide whether CI builds and tests the harness plugins | To Do |
| CSP-850 | Pi: let the operator name the tools that count as subagents | To Do |
| CSP-851 | Pi: send the running Pi version with live state | To Do |
| CSP-852 | Gate a split pane's hook record against its own pane's harness | To Do |
| CSP-853 | Match a pane-only live entry to its tmux server by full socket path | To Do |

### 5.2 Effect on existing backlog

| Task | Effect |
|---|---|
| CSP-268 (session live status) | Superseded by this milestone; closes with CSP-823 |
| CSP-185 (throttle pane captures) | No longer a prerequisite for status; still wanted for previews |
| CSP-191, CSP-270 (density modes) | The live cell (CSP-823) must fit `compact` |
| CSP-193, CSP-256 (visible filter, chips) | A state filter and chip counts join them (CSP-826) |
| CSP-295 to CSP-298 (subagent metadata) | `session_kind` already marks subagent sessions; the subagent count in live state reuses it |
| CSP-546 (consolidate hook docs) | The operator guide (CSP-821) should land with or after it |
| CSP-630 (live Pi checks) | CSP-822 extends it |

## 6. Decisions taken without asking (non-TUI), and how to reverse them

| # | Decision | Why | To reverse |
|---|---|---|---|
| D1 | Live state is a side table, not a node field or link | precedent (0140), keeps identity and fingerprint stable, small socket payload, library API intact | ADR 0205 names the alternative; the table is read through one accessor |
| D2 | The store, not the daemon, is the integration point | daemon optional, no resolve per event, fast TUI | add a daemon `live_state` command later; the store stays the source |
| D3 | `tool_name`, `agent_type`, and error text are not persisted | MCP names leak server names; error text is payload | extend the whitelist in the ADR 0206 table |
| D4 | `hook init` installs the full event set and upgrades on re-run | operator-initiated; nothing silent | add a `--minimal` flag |
| D5 | Two writer spawns per tool call are acceptable (`PostToolUse` for all tools) | needed to leave WAITING; about 10 ms against tool latency | drop the two events; WAITING lingers until the next turn event |
| D6 | Quiet instead of silently stale: long WORKING stays WORKING, flagged | a 40-minute test run is real work | change the window or add a hard cutoff |
| D7 | OpenCode 1.x and 2.x plugins both get a reducer; Pi needs >= 0.84.4 for WAITING and degrades below | the operator runs 1.18.32; 2.x is current | drop one |
| D8 | Phase 1 reads no screen, no transcript, and runs no new subprocess | ADR 0028 and 0086; capability manifest | ADR 0207 lists the options |
| D9 | `hook init claude-code` edits `settings.json` in place with a scanner of the crate's own, and `serde_json`'s `preserve_order` stays off | the feature reorders every JSON map in the crate (10 goldens changed when measured) and still re-indents the file; the splice keeps every untouched byte | ADR 0208 alternative A: enable the feature and update the goldens |

## 7. Decided by the operator, and still parked

### 7.1 Decisions (2026-11-12)

The operator answered the options in [`tui-options.md`](tui-options.md). The
TUI stories (CSP-823 to CSP-830, CSP-854) and the two approved sources carry
these as their contract.

| # | Decision | Story |
|---|---|---|
| 1 | **Option A**: a new always-filled one-cell column right after the age on session rows, mux rows (rolled up), nested agent rows, and the agent rows of the PRs view. Glyphs: `!` waiting, `»` working (also compacting and starting), `›` ready, `×` error (the shared failure mark), `?` unknown or stale (the existing "can't tell" mark), `·` no evidence or exited. A quiet state dims the same glyph; shape never depends on color. Colors are theme roles. No animation. | CSP-823 |
| 2 | Mux rollup precedence **waiting, then error, then ready, then working** (compacting, starting, unknown, exited after). Nested agent rows keep their own mark. Group headers show the waiting count next to `(N)` when it is above zero. | CSP-824 |
| 3 | **Header chip**: `! N` waiting count, whole graph, hidden at zero, ranked after freshness and counts and before `▲`. | CSP-825 |
| 4 | **A float-waiting-first control** (off by default, same mechanism as "float sessions with a mux"), a **State filter** row in Controls and `--state` on `conspectus table`. Rows never re-sort under the cursor between operator actions. No state grouping and no Attention view yet. | CSP-826 |
| 5 | **Jump to the next waiting session** from any view: longest-waiting first, waiting only, wraps; the key comes from the free keys of the scoped keymap. | CSP-827 |
| 6 | **Seen/unseen for ready**, derived read-only: a ready turn is unseen until a tmux client has been attached to its mux after the turn ended, or, for a session with no mux, until the operator selects it in this TUI instance (memory only). Unseen ready is bold, seen is plain; the shape is the same. Nothing is persisted. | CSP-828 |
| 7 | Pinned and History **words** `waiting`, `working`, `ready`, `error`, replacing `in mux` when live state exists (the link fact stays in the detail pane). | CSP-829 |
| 8 | Subagents: a dim `+N` chip after the row label while any subagent is active, and the detail pane lists them; no nested rows. | CSP-854 |
| 9 | **No notifications** in v1 (the harnesses already notify). | CSP-830 (deferred) |
| 10 | **No animation.** | CSP-823 |
| 11 | Hook-free sources approved: the **Codex pane title**, and **`claude agents --json`** after a check that it does not start Claude's background supervisor. Not approved for now: transcript and rollout tails, ingesting agent-deck's `state.db`. | CSP-831, CSP-832 (CSP-833, CSP-834 declined) |
| 12 | Quiet states show dimmed; stale or unknown shows `?`. | CSP-829 |

Items 9, 10, and 12 are the defaults the options document recommended; the
operator did not change them.

### 7.2 Still parked

Notifications (item 9) are deferred, not decided against; the options
document lists the later choices (a terminal bell while the TUI is open and
unfocused, per-state toggles). Transcript tails and agent-deck ingestion
stay declined until the operator asks. CSP-822 (recording real payloads) needs
real harness sessions and the operator's accounts. The remaining follow-ups
(CSP-836 to CSP-853) are low priority and independent.

The data every TUI option reads is already on the row models (CSP-814):
`AgentSessionRow.live` (effective state, detail, confidence, since,
subagents), `MuxSessionRow.live` (the counts), and `MixedRow.live` for
Pinned, History, and stopped-mux rows, filled by `tui::rows::live::annotate`
for every view and kept current between snapshots by the TUI's live
overlay. A rendering story reads those fields; no builder computes live
state. The float and the filter (CSP-826) read the states the build reads
(`RowFilter.live`) and nothing else: an overlay change still refills the
cells in place and never rebuilds the tree, so the order and the rows a
build placed hold until the next build (ADR 0215).

**TUI**

1. State names and the glyph/column (the `●◐○` set collides with session
   kind, PR, and refresh marks). CSP-823.
2. Mux rollup: which state dominates, and does the mux row show it.
   CSP-824.
3. Header chip: what it counts. CSP-825.
4. Ordering: attention-first sort, state grouping, a dedicated view.
   CSP-826.
5. A jump-to-next-attention key. CSP-827.
6. Seen/unseen for `ready`. CSP-828 (and CSP-823's mark).
7. Pinned and History words for a live session. CSP-829.
8. Subagent display (count chip, nesting, detail only). CSP-854.
9. Notifications. CSP-830.
10. Animation for `working`. CSP-823.

**Detection policy**

11. Hook-free fallbacks (pane title, `claude agents --json`, transcript tails,
    ingesting another orchestrator's state). CSP-831 to CSP-834.
12. Staleness display (quiet vs dim vs unknown). CSP-829.

The live cell on session and mux rows and the Pinned words landed with
CSP-823 (ADR 0209), the group headers' waiting counts with CSP-824
(ADR 0210), the header's with CSP-825 (ADR 0211), the session rows'
subagent chip with CSP-854 (ADR 0212), the detail pane's fields, the
subagents as a count among them, with CSP-829 (ADR 0213), seen and
unseen ready turns with CSP-828 (ADR 0214), the float-waiting-first
controls, the agent-state filter, and `--state` with CSP-826 (ADR
0215), and the `W` jump with CSP-827 (ADR 0216); the rest of the list
above is still to build, and the CLI and the graph JSON keep the fuller view
([`docs/operations.md`](../../operations.md#live-agent-state)).

The mockups and a recommended default for each are in
[`tui-options.md`](tui-options.md).

## 8. Existing bugs the research surfaced

Filed as their own stories; none is required by the first slice except where
noted.

- `hook init claude-code` matcher misses `source: "fork"`; `CLAUDE_CODE_VERSION`
  is not a real variable; two fixtures use an invalid `source`. The matcher
  and the fixtures are fixed (CSP-811), and the writer no longer reads the
  variable: Claude Code documents no version for hooks, so its record
  carries none (CSP-837).
- `hook init` re-sorts the keys of the operator's `settings.json`. Fixed
  (CSP-819, ADR 0208): the install edits the file's text in place, so
  order and layout survive and a remove gives the file back.
- `hook init codex` replaces an operator's `[[hooks.SessionStart]]` tables and
  sets no timeout; hooks may be inert until trusted. Fixed (CSP-815): the
  install keeps every form, sets timeouts, and prints the `/hooks` step.
- The attribution spool key ignores the pane; `ACTIVE_TTL_SECONDS` is dead;
  an ingest leaves two active links for one pane until the next cycle. Fixed
  (CSP-818): the spool keeps one record per pane, the constant is gone (no
  time limit applies), and an ingest supersedes the pane's older link.
- Socket clients have no timeouts. Fixed: `hook write`'s ingest call is
  bounded at 2 s (CSP-810), and every other request by what it can take
  (CSP-836, ADR 0219).
- The OpenCode 1.x plugin forwards child-session events (a pane briefly names
  the child); the 2.x plugin listens for an event that no longer exists. Both
  fixed (CSP-816): only root sessions' events are attributed, and the dead
  entry is gone.
- Pi's idle cache warming moves the session file's mtime, so
  `last_active_epoch` can look recent on an idle session. Fixed
  (CSP-820): the adapter takes the newest entry that is not `usage`.
- Doc drift: ADR 0028 and `docs/operations.md` still describe a TTL and
  `hooks.sqlite3`; `docs/design.md` omits `hook_ingest`. Fixed (CSP-818).

## 9. As built (phase 1)

Where the build differs from, or settles, the draft above:

- **One shared reducer.** `hook write` maps each event source's payload
  (Claude Code, Codex) to a closed set of neutral events (ADR 0204
  decision 3) and folds them with one reducer shared by every event
  source; a harness's reducer handles only the cases it must. Codex
  needed none of its own. Plugin sources (opencode, Pi) send whole
  snapshots with an instance and sequence number, and the store replaces
  the entry, so both kinds end in the same entry shape.
- **Bindings, not bare observations.** `node_live` holds `LiveBinding`s:
  the stored observation and the `Liveness` (alive, dead, unknown) the
  binder found for its process, so `graph --format json` tells a dead
  agent's last state from a live one without a process table. Agents no
  session matches yet go to a second table, `pane_live`, keyed by the
  tmux mux and pane (`LivePane`), so two such agents in one mux each
  count (CSP-841). Readers use `GraphSnapshot::live_state_of`,
  `live_counts_of_mux`, `live_panes_of`, and `live_tables`. `graph.bin`
  is at format version 14 (12 carried both tables; 13 adds the pane-title
  marks on the mux probes and the `pane_title` source; 14 adds the repair
  polls, `live_polls`, and the `harness_poll` source).
- **The TUI keeps the data current.** It binds the store again within
  about a second of a change and re-checks bound agents' processes once a
  second (CSP-843); the row models carry `live` fields (CSP-814), which
  the live cell and the Pinned words draw (CSP-823, ADR 0209), the
  group headers count (CSP-824, ADR 0210), the header line counts over
  the whole graph (CSP-825, ADR 0211), session rows show running
  subagents (CSP-854, ADR 0212), the detail pane spells out (CSP-829,
  ADR 0213), an unseen finished turn draws bold (CSP-828, ADR 0214),
  Controls floats and filters by when the tree is built (CSP-826,
  ADR 0215), and `W` jumps to the agent waiting longest (CSP-827, ADR
  0216); a Codex pane's title fills in where hooks are silent (CSP-831,
  ADR 0217), and Claude Code's agent list repairs a state its hooks left
  stuck (CSP-832, ADR 0218).
- **CLI surfaces.** An opt-in `live` column on `table sessions` and
  `table mux`, a `live state` section in `node show`, the two lists in
  the graph JSON, and a health line at the end of `hook status`
  (`receiving`, `silent`, `no-events`, `off`, `unsupported`). No separate
  `--fresh` read was built: `--refresh` binds the store as it is.
- **Installers.** Claude Code gets 14 entries, written into `settings.json`
  in place so the operator's bytes survive (ADR 0208); Codex gets 11,
  edited into `config.toml` through `toml_edit`, with the `/hooks` trust
  step printed and never written; Pi gets extension version 2, and
  `hook status pi` states the Pi versions waiting needs; opencode gets
  plugin 0.3.0 with a 2.x server entrypoint. There is no `hook init
  opencode` and no opencode health line yet (CSP-848).
- **Plugin reducers are verified with `node --test`.** The opencode reducer
  and the Pi state machine are TypeScript modules with no harness imports,
  tested by `just opencode-plugin-test` and `just pi-plugin-test` (Node
  22.6 or later, run by hand). The scripted event fixtures are shared: the
  TypeScript tests replay them through the reducer, and the Rust tests
  feed the expected snapshots through the payload parser and the store.
  Neither CI nor the dev shell runs the plugin tests yet (CSP-849).
- **Attribution fixes.** The hook spool keeps one record per pane
  (CSP-818), `hook write` asks tmux about its own pane (CSP-845), and Pi's
  idle cache warming no longer dates a session (CSP-820).
- **One hook-free source: the Codex pane title** (CSP-831, ADR 0217).
  The mux probe's `list-sessions` call also returns each session's active
  pane id and title; a pane whose program is `codex` has its title reduced
  at once to `waiting`, `working`, or neither, and the text is dropped.
  The binding folds the marks in after the store: pane-only state for a
  Codex with no hook entry, a fresh hook entry wins, a quiet or stale one
  gives way, and a hook's `waiting` turns `working` once two probes after
  it lose the marker (the answered-approval gap). `hook status codex`
  counts the agents titles show; `CONSPECTUS_DISABLE_PANE_TITLES` turns
  it off. The title strings come from Codex's source, not a recording,
  and Claude Code's titles are not read.
- **A second: `claude agents --json`, as a repair only** (CSP-832, ADR
  0218). The operator's check came first: in a scratch config it starts no
  supervisor or other process, fires no hook, writes nothing once the
  config is initialized, and costs 0.3 to 0.6 s, but opens a TLS
  connection per call unless `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`,
  which Conspectus sets on the child. A `LivePoller` per harness that
  offers one (Claude Code only) lives as long as the daemon or a
  daemonless TUI's refresh worker and runs at most every 30 s, only while
  a bound Claude Code hook entry recorded `working` or `waiting`, has
  been silent for 10 s, and no earlier poll answered it; one-shot commands
  never poll. The parser keeps each listed session's pid, a plain session
  id, and a mapped state; the snapshot carries the latest poll in
  `live_polls`, and the binding folds it in after the pane titles: a hook
  entry silent for the settle window before the poll reads the poll's
  state for the same process (source `harness_poll`). A newer hook event
  wins, hook `ready`, `error`, and `exited` never change, and a session
  with no hook entry gets nothing (CSP-871).
  `CONSPECTUS_DISABLE_LIVE_POLL` turns it off. The output shapes are from
  the documentation, not a recording.
- **Every fixture is synthetic** until CSP-822 records real payloads.
