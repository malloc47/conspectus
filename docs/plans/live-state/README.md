# Live Agent State: Plan

Status: drafted 2026-11-12 on branch `feat/live-state` (milestone m-26).
Evidence: the six research notes in [`research/`](research/). Decisions: ADRs 0203 to 0207.

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
  daemonless TUI must work).
- Reducers for stateless hooks (Claude Code, Codex) run in Rust inside
  `hook write`, folding each event into the stored per-session entry. Plugins
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
  dropped on write; the file is capped at a few hundred entries.
- **Privacy:** entries hold enums, counts, ids, paths, and times. The writer
  parses payloads in memory against a typed whitelist and drops the rest
  ([01](research/01-claude-code.md) §6.2 has the Claude Code list). Tests plant
  sentinel strings in every payload-bearing field and assert none reaches the
  store. Even `tool_name` is not stored: it is used to classify
  (`AskUserQuestion` becomes `question`) and dropped.

### 3.4 Graph representation (ADR 0205)

A snapshot side table, `node_live: BTreeMap<NodeId, LiveObservation>`, next to
`node_presence` and `node_provenance`:

- Keyed by `AgentSession` id (exact match on harness and native session id) or
  by `MuxSession` id for entries that have a pane but no known session
  ("pane-only").
- Produced by a new discovery mutator, `live_state`, that runs after the
  harness providers and the hook sidecar, like the hook sidecar does today,
  and by the TUI overlay re-bind. Both call one pure function.
- Not on node structs: that churns the identity nodes, the process-tree
  fingerprint (ADR 0091), and 31 struct literals in the library API.
- Not graph links: a state is not a relationship between two nodes, and
  `ResolvedRelationship.target` is a `NodeId`.
- Bumps the rkyv `FORMAT_VERSION`, skipped from JSON only if empty.

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
| 2 | Vocabulary and rules in the model | CSP-807 |
| 3 | `node_live` snapshot side table | CSP-808 |
| 3 | The live-state store | CSP-809 |
| 4 | `hook write` as a fast, silent writer | CSP-810 |
| 5 | Claude Code: reducer, fixtures, install | CSP-811 |
| 6 | Bind live state at discovery | CSP-812 |
| 7 | CLI output | CSP-813 |
| 7 | TUI plumbing (data on row models, nothing rendered) | CSP-814 |
| 8 | Codex, OpenCode, Pi | CSP-815, CSP-816, CSP-817 |
| any | Spool defects, key order, Pi mtime | CSP-818, CSP-819, CSP-820 |
| 9 | Operator guide | CSP-821 |
| 10 | Real-payload verification (needs the operator) | CSP-822 |
| parked | TUI rendering (cell, rollup, header, order, jump, seen, detail, notify) | CSP-823 to CSP-830 |
| parked | Hook-free sources (pane title, `claude agents --json`, tails, other orchestrators) | CSP-831 to CSP-834 |

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

## 7. Parked for the operator

These change what the operator sees or widen what Conspectus is allowed to
read, so they wait for an answer. Recommendations are marked.

**TUI**

1. State names and the glyph/column (the `●◐○` set collides with session
   kind, PR, and refresh marks).
2. Mux rollup: which state dominates, and does the mux row show it.
3. Header chip: what it counts.
4. Ordering: attention-first sort, state grouping, a dedicated view.
5. A jump-to-next-attention key.
6. Seen/unseen for `ready`.
7. Pinned and History words for a live session.
8. Subagent display (count chip, nesting, detail only).
9. Notifications.
10. Animation for `working`.

**Detection policy**

11. Hook-free fallbacks (pane title, `claude agents --json`, transcript tails,
    ingesting another orchestrator's state).
12. Staleness display (quiet vs dim vs unknown).

The mockups and a recommended default for each are in
[`tui-options.md`](tui-options.md).

## 8. Existing bugs the research surfaced

Filed as their own stories; none is required by the first slice except where
noted.

- `hook init claude-code` matcher misses `source: "fork"`; `CLAUDE_CODE_VERSION`
  is not a real variable; two fixtures use an invalid `source`.
- `hook init` re-sorts the keys of the operator's `settings.json`.
- `hook init codex` replaces an operator's `[[hooks.SessionStart]]` tables and
  sets no timeout; hooks may be inert until trusted.
- The attribution spool key ignores the pane; `ACTIVE_TTL_SECONDS` is dead;
  an ingest leaves two active links for one pane until the next cycle.
- Socket clients have no timeouts.
- The OpenCode 1.x plugin forwards child-session events (a pane briefly names
  the child); the 2.x plugin listens for an event that no longer exists.
- Pi's idle cache warming moves the session file's mtime, so
  `last_active_epoch` can look recent on an idle session.
- Doc drift: ADR 0028 and `docs/operations.md` still describe a TTL and
  `hooks.sqlite3`; `docs/design.md` omits `hook_ingest`.
