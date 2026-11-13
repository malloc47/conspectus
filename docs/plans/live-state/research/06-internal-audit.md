# Live state research 06: internal audit

What Conspectus already has that live state builds on or has to respect,
and where it would plug in. Line numbers are against the `live-state`
worktree as of 2026-11-11. Paths are relative to the repository root.

## 0. Findings that shape the design

1. **The hook pipeline exists end to end but is wired for attribution,
   not state.** Claude Code and Codex hooks are installed for
   `SessionStart` only (`src/cli/hook.rs:799-816`, `:889-898`). Pi
   reports `session_start` only (`plugins/pi-hook/conspectus-hook.ts:23`).
   OpenCode already forwards `session.status`, `session.idle`, and
   `session.compacted`, but only as the event *name*. The status value
   is dropped (`plugins/opencode-hook/src/index.ts:22-43`, payload type
   `write.ts:61-65`).
2. **The spool keeps the latest record per mux, not per pane or
   session.** The key is `tmux:{socket}:{session_name}`
   (`src/hook.rs:242-258`), and the writer never sets `native_id`
   (`src/cli/hook.rs:595`). Two agents in split panes of one tmux
   session overwrite each other in `hooks-latest.json`. The unit test
   that "proves" per-pane independence
   (`src/discovery/hook_sidecar_tests.rs:739`) goes through the legacy
   per-event JSON reader, not the spool. A per-session event history
   can't be represented under this rule.
3. **No time-based TTL is applied anywhere.** `ACTIVE_TTL_SECONDS`
   (`src/hook.rs:14`) is dead code since CSP-249. Staleness today means
   only "pid dead", "mux created after the record", or "pane runs
   another harness" (`src/discovery/hook_sidecar.rs:60-90`). ADR 0028
   and `docs/operations.md` still say stale records are ignored.
4. **There is no push channel.** The socket does one request and one
   response per connection (ADR 0038; `src/server/mod.rs:564-583`). The
   TUI polls the whole snapshot every 30 s by default
   (`src/tui/mod.rs:187`, `src/tui/runtime.rs:522-528`). Even with a
   daemon, a hook event takes up to about 30 s to show, unless the
   operator presses `r` or the TUI pane comes back into view.
5. **Every hook ingest costs a full resolve, a full rkyv
   re-serialization, and a `graph.bin` rewrite, under the writer lock**
   (`src/server/mod.rs:677-716`). The client waits for all of it and
   has no socket timeout (`src/server/mod.rs:498-562`). If tool-level
   hooks fire this path synchronously, they would slow or hang the
   agent.
6. **The graph has no Pane node, and no per-node attribute bag.**
   `MuxSessionNode` describes only the *active* pane
   (`src/model/node.rs:367-374`). `Metadata` exists only on
   `SourceMetadata.fields` of links, stored in rkyv as a JSON string
   (`src/model/rkyv_adapters.rs:26`). The precedents for time-varying
   facts are typed node fields (`last_active_epoch`, `activity_epoch`,
   `client_attached`), the `RuntimeProcess` observation node (ADR
   0047), and the sparse snapshot side tables `node_presence` and
   `mux_probes` (ADR 0140; `src/model/graph.rs:50-65`).
7. **The session row's one state cell is taken.** `Col::State` (one
   cell that never gives way) holds the mux-link glyph `⊸`/`∘`
   (`src/tui/ui/left_panel/columns.rs:194`, ADR 0145). Mux rows use
   `Col::MuxState` for client attachment (`:218`). Live state needs its
   own column. The Braille spinner already means "refresh pending"
   (`docs/tui-columns.md:134`), so it can't be reused as a "working"
   animation.
8. **CSP-268 is the existing story, and parts of it are stale.** Its
   reserved `status_*` theme colors don't exist (see its own CSP-766
   note). Its pane-delta detection collides with ADR 0028's rejection
   of terminal scraping and with ADR 0086 Tier 1.

---

## 1. The hook pipeline, end to end

### 1.1 Emitters per harness (what each sends today)

| Harness | Installed by | Events wired | Payload forwarded to `hook write` | Dropped or not wired | Source |
|---|---|---|---|---|---|
| Claude Code | `hook init claude-code` adds one `SessionStart` entry, matcher `resume\|startup\|clear\|compact`, `type: command` | `SessionStart` only | Claude's full stdin JSON. The adapter keeps `session_id`, `cwd`, `transcript_path`, `hook_event_name` | `source`, `model`, `permission_mode`; no other hook events installed | `src/cli/hook.rs:784-818`; fixture `tests/fixtures/hooks/claude-sessionstart.json` |
| Codex | `hook init codex` adds one inline `[hooks] SessionStart` entry with `async = false` | `SessionStart` only | stdin JSON, same default parser | everything else | `src/cli/hook.rs:877-900`, `:942-954` |
| OpenCode 1.x | npm plugin (`opencode plugin <dir>`), ADR 0049 | `session.created`, `session.updated`, `session.status`, `session.idle`, `session.compacted` | `{session_id, cwd, hook_event_name}` | the `status` payload of `session.status`; `permission.ask`, `tool.execute.*`, `chat.message` (all named in the CSP-223 audit) | `plugins/opencode-hook/src/index.ts:17-45`, `write.ts:61-65` |
| OpenCode 2.x | TUI plugin listed in `cli.json`, ADR 0126 | `tui.route` (1 s route poll), then `session.created`, `session.renamed`, `session.status`, `session.idle`, `session.compacted` for the routed session only | same flat payload | same as 1.x | `plugins/opencode-hook/src/v2/tui.ts:22-81` |
| Pi | `hook init pi` writes a Conspectus-owned extension file (ADR 0121), embedded via `include_str!` | `session_start` in TUI mode only | `{session_id, cwd, transcript_path, hook_event_name: "session_start:<reason>"}` | everything else | `plugins/pi-hook/conspectus-hook.ts:22-53`; `src/cli/hook.rs:644-645` |
| Legacy shim | `scripts/conspectus-claude-hook-sidecar.py` | whatever invokes it | stdin piped through, stdout and stderr to `/dev/null` | — | `scripts/conspectus-claude-hook-sidecar.py:11-22` |

No adapter overrides `HarnessAdapter::hook_record_from_payload`. Every
harness goes through the default, which requires a non-empty string
`session_id` and keeps four fields
(`src/discovery/harness/mod.rs:336-375`).

### 1.2 What `conspectus hook write <harness>` does (`src/cli/hook.rs:63-91`)

1. Reads the stdin JSON and fails on empty or invalid input.
2. Walks the parent-pid chain, up to 8 hops, for the first ancestor
   whose `/proc/<pid>/comm` matches the harness's
   `process_command_basenames` (`:467-505`, `:516-521`). It reads that
   ancestor's cmdline to set `background` from
   `RuntimeSignature::is_background_process` (`:551-579`).
3. Gets mux context unless the harness is a background one. It asks
   `SystemTmux` and then `SystemZellij` for
   `current_session_context` (`:581-605`). Tmux runs
   `tmux display-message -p '#{session_name}'` and `'#{pane_id}'`, two
   subprocesses with no `-t` (`src/discovery/tmux/mod.rs:967-1007`).
   tmux resolves the calling pane from the inherited client
   environment (`TMUX_PANE`); the writer never reads `TMUX_PANE`
   itself. The socket path comes from `$TMUX`. `native_id` is always
   `None` (`src/cli/hook.rs:595`).
4. Reads the harness version from `CLAUDE_CODE_VERSION`,
   `CONSPECTUS_OPENCODE_HOOK_VERSION`, or `CONSPECTUS_PI_HOOK_VERSION`
   (`:527-534`).
5. Builds a `HookRecord` with `observed_epoch = now` in seconds.
6. `write_or_ingest_hook_record` (`:381-414`) sends the record to the
   daemon (`client_hook_ingest`). On `NoDaemon`, any daemon error, or a
   transport error, it writes the spool locally, printing a warning to
   stderr except for `snapshot_unavailable`. `--state-root` bypasses
   the daemon.
7. Prints nothing to stdout. That matters: for Claude Code,
   `SessionStart` and `UserPromptSubmit` hook stdout is added to the
   model's context.

Per-invocation cost: one `conspectus` process start, a short `/proc`
walk, two `tmux` subprocesses, and one socket round trip. On the daemon
side the request waits up to 200 ms for the accept poll
(`src/server/mod.rs:1074-1076`), waits for the writer lock (which a
class cycle can hold for seconds), then does a spool read-modify-write,
`apply_hook_records`, a full `resolve_snapshot`, an rkyv serialize, and
a `graph.bin` write before it replies.

### 1.3 Record schema (`src/hook.rs:17-57`, ADR 0028 v1)

| Field | Type | Notes |
|---|---|---|
| `schema_version` | `u16` = 1 | `write_record` refuses other values (`:88-93`) |
| `harness_key` | String | registered adapter key; unknown keys fail loudly (`:152-178`) |
| `session_key` | String | harness-native session id |
| `cwd` | `Option<String>` | |
| `pid`, `ppid` | `Option<i64>` | the harness process, not the writer |
| `tmux` | `Option<HookTmuxRecord>` | `session_name`, `native_id` (unused), `pane_id`, `socket_path` (also carries a zellij session name) |
| `transcript_path` | `Option<String>` | gates synthesizing a session node |
| `hook_event_name` | `Option<String>` | carried into link metadata only (§1.6) |
| `observed_epoch` | `i64` seconds | ordering key for the spool and for dedupe |
| `harness_version` | `Option<String>` | |
| `background_harness` | bool | such records never produce mux links (`:37-44`) |

There is no `deny_unknown_fields`, so new optional fields are
backward-readable. A `SCHEMA_VERSION` bump is not safe for mixed
versions. `LatestHookStore::read` swaps a store with another version for
an empty default (`:207-216`), and the next `write_record` then writes
that store back containing only its own record. An older `conspectus`
on `PATH`, which the plugins spawn, would wipe a newer binary's spool.
The history store (ADR 0140) does the opposite: it leaves a newer file
untouched.

### 1.4 The spool

- **Path.** `$CONSPECTUS_HOOK_SIDECAR_STATE`, else
  `$XDG_STATE_HOME/conspectus/hooks`, else
  `~/.local/state/conspectus/hooks` (`src/hook.rs:125-141`, ADR 0028).
- **File.** `hooks-latest.json`: `{schema_version, records: {key ->
  HookRecord}}`, pretty JSON, written to a pid-suffixed temp file and
  renamed, directory `0700` and file `0600` (`:200-240`, `:275-299`).
- **Write rule.** Keep the incoming record if
  `observed_epoch >= existing` for its key (`:98-110`). There is no
  lock, so concurrent daemonless writers race and the last rename wins.
  ADR 0146 notes the race remains without a daemon.
- **Key** (`mux_record_key`, `:242-258`): `tmux:{socket|default}:{native_id|session_name}`,
  else `cwd:{cwd}`, else `session:{harness}:{session}`. The pane is
  not part of the key.
- **Read.** `read_records` returns the spool plus every other `*.json`
  in the root, the legacy per-event records (`:118-122`, `:260-273`).
- **Who writes it** (ADR 0146): the daemon in `hook_ingest`, to the
  root its own discovery reads (`src/server/mod.rs:692-704`), or the
  hook writer when no daemon snapshot is available.
  `docs/design.md:1823-1832` restates this.

### 1.5 The latest-record-per-mux rule: why, and what it loses

**Why.** ADR 0028 started with SQLite (`hooks.sqlite3`) plus legacy
per-event JSON (`docs/adr/0028-hook-sidecar-mux-attribution.md:50-56`).
It explicitly deferred both an append-only JSONL log (locking, partial
lines) and a latest file per harness, session, or pane ("discards event
history", pane-reuse key design) (`:138-145`). CSP-448.05 retired the
SQLite store along with the rest of SQLite (ADR 0082), and its summary
says this "intentionally drops the append-only sidecar history; the
retained semantic is only last-seen session for this mux". ADR 0146
kept the size bound (`docs/adr/0146-...md:51-52`). The only consumer
was current-session-to-mux attribution, and for that, latest-per-mux is
enough.

**What it loses:**

| Lost | Cause | Effect on live state |
|---|---|---|
| Other panes of the same tmux session | the key ignores `pane_id` (`src/hook.rs:242-258`) | split-pane agents: after a replay only the most recent pane's state survives |
| Earlier events and transitions | one record per key | no "waiting since", durations, or counts (subagents in flight, tool depth) |
| Concurrent daemonless writes | unlocked read-modify-write | a lost `Stop` leaves a session "working" until another event or a TTL |
| Sub-second order | `observed_epoch` is in seconds, compared with `>=` | events within one second (Pre and PostToolUse) land in nondeterministic order across processes |
| Non-tmux sessions sharing a cwd | key `cwd:{cwd}` | collide |
| Event detail | adapters keep the event name only | notification type, tool, status, and compaction source are all gone |

**Is a per-session event history impossible under it?** Yes. The spool
can't represent one. The daemon keeps no second store: ADR 0146
rejected keeping ingested records in daemon memory
(`docs/adr/0146-...md:60-62`), and `graph.bin` "is not a hook store"
(`:39-40`). Live state needs at least the latest *state* per (pane,
session), with a since-timestamp and counters. A new or re-keyed
category 2 store under `$XDG_STATE_HOME/conspectus/` is already within
the manifest's `read` writes (`docs/capabilities.toml:151-157`), but it
needs an ADR refining 0028 and 0146.

### 1.6 Daemon ingest (`src/server/mod.rs:648-723`)

1. Parse `args.record` (`invalid_hook_record` / `missing_hook_record`).
2. Take the writer lock (`:677-680`), shared with every class cycle.
3. With no snapshot yet, answer `snapshot_unavailable`, and the client
   writes the spool (`:681-691`).
4. Write the spool first; a failed write answers
   `hook_spool_write_failed` and leaves the graph alone (ADR 0146
   decision 3; `:692-704`).
5. `apply_hook_records(&mut snapshot, vec![record], now)`, then
   `resolve_snapshot`, then `publish_snapshot_to_path`: serialize,
   update `SnapshotBytes` and `SnapshotState`, write `graph.bin`
   (`:705-716`, `:934-961`).
6. Ingest does not record history; only cycles do (`:1553-1563`).

**Transient double link.** `apply_hook_records` builds a fresh
`winners` map for each call (`src/discovery/hook_sidecar.rs:36`).
`demote_weaker_mux_links` only demotes cwd and command evidence
(`:142-171`), and link ids embed `observed_epoch` (`:322-326`). So an
ingest for session B in a pane that already has an Active hook link for
session A leaves both Active until the next cycle evicts and replays
the spool. Each ingest with a pid also adds a new `RuntimeProcess` node
whose id embeds the epoch (`:353-358`). Today that churn lasts at most
one 5 s cycle. Per-event state ingests would multiply it.

### 1.7 In-process replay

`apply_mutators` calls `hook_sidecar::apply_hook_sidecars(snapshot,
root, now)` on every discovery (`src/discovery/mod.rs:937-939`): daemon
class cycles, one-shot CLI cold rebuilds (`src/cli/mod.rs:189-212`),
and the daemonless TUI's background discovery
(`src/tui/runtime.rs:1390-1434`). `hook_sidecar` is a Mutator provider
(`src/discovery/providers.rs:119`; `cache.rs:151`), always evicted and
re-run, so the spool is the only durable input. Each replay reads the
whole spool plus the legacy JSON and filters by `SCHEMA_VERSION`
(`src/discovery/hook_sidecar.rs:19-26`).

### 1.8 Attribution (`src/discovery/hook_sidecar.rs`)

| Step | Rule | Lines |
|---|---|---|
| Skip a background harness | `background_harness` means no mux links | `:44-46` |
| Find the mux, live muxes only (ADR 0140) | tmux `native_id` or `session_name` == mux `native_id`; then `active_pane_pid` == `pid` or `ppid`; then cwd == mux `cwd` or `active_pane_current_path` | `:233-271` |
| Pane | `pane_id` only builds the dedupe key `(mux, pane)`; the graph has no pane entity | `:50-53` |
| Find the session | exact `harness_key` + `session_key` | `:173-181` |
| Synthesize a session | sparse `AgentSessionNode` when none exists, unless a named transcript file is missing; `last_active_epoch = observed_epoch` | `:183-231` |
| Gates (Ignored) | pid not in `/proc`; Claude `kind == "bg"` from `~/.claude/sessions/<pid>.json`; mux `created_epoch > observed_epoch`; the active pane's command is another harness | `:59-90`, `:408-443` |
| Winner per `(mux, pane)` | freshest first; later ones `Overridden` | `:33-36`, `:91-105` |
| Evidence | `LinkedToMux`, `StrongDiscovered`/`High`/`Fresh`, `match_kind` = `hook_session_match` or `hook_session_path_match`, fields `observed_epoch`, `transcript_path`, `hook_event_name`, `harness_version`, `tmux` | `:273-341` |
| Process observation | `RuntimeProcess` node, `MuxContainsProcess`, `ProcessIdentifiesSession` | `:343-406` |

`pane_running_harness` reads the *active* pane's command (`:135-140`).
A hook from a non-active pane is gated against whatever the active pane
runs.

### 1.9 Freshness and TTL today

- `ACTIVE_TTL_SECONDS = 15*60` (`src/hook.rs:14`) has no reader.
  CSP-249 dropped the emission gate and said the constant was kept for
  "higher-layer freshness signals (e.g. CSP-243)". Nothing uses it.
- Hook links get `Freshness::Fresh` and `freshness_epoch: None`
  (`src/discovery/hook_sidecar.rs:331-337`). The `Freshness` enum
  (`src/model/link.rs:246-250`) is not computed from time for hooks.
- The provider-level warm-start gate uses
  `NodeProvenance.freshness_epoch` per class
  (`src/model/resolution.rs:22-37`; `cache::compute_freshness_gate`).
- Drift: `docs/operations.md:80-81` says "Stale records are ignored for
  active mux attribution", and ADR 0028 (`:90-94`) still cites the
  15-minute TTL and SQLite.

### 1.10 Mutation envelope and privacy, as they apply to hooks

- **Category 2** (ADR 0143 `:68-77`): hook sidecars, history, the TUI
  state file, and `graph.bin` are rebuildable observations. Prohibition
  5 (`:155-161`) lets the daemon write category 2 and nothing else.
- **Category 5** (`:109-118`): `hook init` and `hook remove` touch
  "Conspectus's own hook entry". Prohibition 1 (`:130-133`) forbids
  every other harness-settings write. More events means more entries
  inside the same settings files, which fits category 5 if the ADR
  words it so. The manifest's `hook` write list
  (`docs/capabilities.toml:186-198`) already names the files.
- **ADR 0086 Tier 3** (`:134-147`): hook records stay payload-free. They
  may carry ids, never content. Tool input, prompts, notification
  message text, and assistant text must never reach the record. Tier 1
  (`:64-89`): any reader whose output feeds graph state or the resolver
  reads no payload.
- **ADR 0028** (`:101-104`, `:128-133`) forbids `send-keys`, slash
  commands, prompt injection, and generic terminal scraping. Scraping an
  already-visible status surface needs its own ADR and is allowed only
  after hooks, control planes, and state sources fail.

---

## 2. The graph model

### 2.1 What exists

- `GraphNode` (`src/model/node.rs:19-30`) is a tagged enum of typed
  structs: Repo, Checkout, Workspace, AgentSession, MuxSession, Pin,
  RuntimeProcess, Branch, Fork, ForgePr. **No node has a free-form
  attribute bag.** `Metadata = BTreeMap<String, Value>`
  (`src/model/mod.rs:30`) appears only as `SourceMetadata.fields` on
  links (`src/model/link.rs:388-408`). Its keys are `source_field`
  constants (`:434-465`), and in rkyv it is a JSON string
  (`MetadataAsJson`, `src/model/rkyv_adapters.rs:22-76`).
- `GraphLink` (`src/model/link.rs:336-346`) carries relation,
  provenance, confidence, freshness, source metadata, and state
  (Active, Ignored, Overridden). The resolver turns candidates into
  `ResolvedRelationship`s whose target is a `NodeId`
  (`src/model/resolution.rs:52-72`). `preferred_mux_for`
  (`src/model/graph.rs:356-378`) and `mux_host_of` (`:382-411`) are the
  session-to-mux read helpers.
- **Snapshot side tables** (`src/model/graph.rs:16-71`):
  - `node_provenance`: provider and freshness per node, serialized as a
    list.
  - `node_presence`: sparse Gone or Unknown entries; live has no entry
    (ADR 0140; `:50-58`, serde `:73-114`).
  - `mux_probes`: `#[serde(skip)]`, so it rides in `graph.bin` but not
    in JSON (`:59-65`).
  - `history` and `pins`.
- `evict_provider` (`:474-495`) drops a provider's nodes, links,
  provenance, presence, and probes, and clears resolved relationships.
  `merge_with_prior` keeps only the `GraphFragment` fields: nodes,
  links, diagnostics, provenance, and probes
  (`src/discovery/mod.rs:297-333`, `:1421-1428`).
- `SnapshotIndex` (`src/model/graph.rs:175-336`) indexes Active links
  by source and relation, links by id, and per-agent mux candidate
  counts. It has **no reverse (by target) index**.
- Format: rkyv with `FORMAT_VERSION = 9`. Any model field or variant
  change bumps it, and readers cold-rebuild (`src/snapshot.rs:50-53`,
  ADRs 0082 and 0083).

### 2.2 Time-varying facts the graph already holds

| Fact | Where | Producer | Notes |
|---|---|---|---|
| `AgentSessionNode.last_active_epoch` | `node.rs:278-282` | transcript mtimes and state DBs (`harness/claude_code.rs:762`, `codex.rs:285`, `opencode.rs:207`); hooks for synthesized sessions | zeroed in the process-tree fingerprint |
| `AgentSessionNode.last_message_preview`, `title` | `node.rs:270-277` | adapters, Tier 2 (ADR 0023, ADR 0086) | zeroed in the fingerprint |
| `AgentSessionNode.session_kind` (Human/Subagent) | `node.rs:283-289`, `:734-737` | opencode | an existing seed for "subagents working" |
| `MuxSessionNode.activity_epoch`, `client_attached`, `last_attached_epoch`, `active_pane_*` | `node.rs:365-388` | tmux list | active pane only |
| `RuntimeProcessNode.observed_epoch`, `role` | `node.rs:509-530`, `:757-763` | `cross_link`, hook sidecar | ephemeral observation node (ADR 0047) |
| `node_presence` | `graph.rs:50-58` | history projection | "absence from a scan never means gone" (ADR 0140) |
| Hook `hook_event_name` and `observed_epoch` | link fields, `hook_sidecar.rs:285-320` | hook sidecar | no reader anywhere today |

`fingerprint_normalized_node` (`src/discovery/mod.rs:836-880`) zeroes
volatile session and mux fields so that churn doesn't reopen the
expensive `/proc` walk gate (ADR 0091). **Any live-state field placed
on a node must be zeroed there too.**

### 2.3 How a timestamped, TTL-bound observation fits the guardrails

The guardrails in CLAUDE.md and `docs/design.md:30-92` say the graph is
sparse, evidence comes first and a resolver follows, and the core is
provider-neutral with provider detail in adapters. Under those rules:

- Store **facts**: a neutral state enum, `observed` (ms or a sequence
  number), `since` (when this state began), source kind (`hook`,
  `harness_state`, ...), and optionally a normalized event name.
  Compute "stale" and the effective state **at read time** from `now`,
  as ages are (`tuikit::age`). The snapshot a TUI renders is up to
  `refresh_interval` old, and the left pane already gets `now` each
  frame (`src/tui/ui/left_panel/columns.rs:280-287`).
- **Absence is "no evidence", not "idle"**, as ADR 0140 treats absence
  for presence.
- Native event names (`PreToolUse`, `session.idle`, `session_start:new`)
  become the neutral enum in the adapter, at
  `HarnessAdapter::hook_record_from_payload`
  (`src/discovery/harness/mod.rs:336`) or a sibling method. The model
  never sees harness names.

### 2.4 Candidate representations

| | A. Field on `AgentSessionNode` (plus `MuxSessionNode` for pane-level) | B. Observation node (extend `RuntimeProcessNode` or a new kind) | C. Evidence links (candidate state links) | D. Snapshot side table `node_activity: BTreeMap<NodeId, Activity>` | E. Outside the graph (TUI or CLI cache) |
|---|---|---|---|---|---|
| Precedent | `last_active_epoch`, `client_attached` | ADR 0047 `RuntimeProcess`; hooks already emit one (`hook_sidecar.rs:343-406`) | hook link fields (`hook_sidecar.rs:285-320`) | `node_presence`, `node_provenance`, `mux_probes` | `PreviewStore`, `ContentHitMap` (`filter.rs:28`) |
| rkyv / `graph.bin` (ADR 0083) | bump `FORMAT_VERSION`; small | bump; a new `NodeKind` touches `NodeKind::ALL` (`node.rs:786`), glyphs, catalog tests, DOT/HTML | if the state lives only in `fields`, no bump (JSON string), but every read parses it | bump once; tiny entries; can be JSON-skipped like `mux_probes` | none |
| Daemon wire | inside the snapshot bytes; every change means a full-snapshot poll | same | same | same, and can **also** be served alone by a small socket command for fast polling | none |
| CLI JSON (`graph --format json`) | inline per node; `skip_serializing_if` keeps stateless goldens stable | new nodes and links, noisy; ADR 0047 hides them from default views | appears in `candidate_links` | list serializer like `node_presence_serde`, or skipped | invisible, which conflicts with `design.md:84-87` (a machine-readable graph) |
| Resolver | not needed for sessions (exact key); mux state derives from the preferred mux, so link ambiguity leaks into state | `derive_process_mux_links` (`resolve/mod.rs:240`) already uses process identity; state rides along | needs a new `RelationKind` whose target is no entity, which mismatches `ResolvedRelationship.target: NodeId`; riding on `LinkedToMux` couples state to attribution and hides it when the link is Ignored or Overridden | optional pure merge step (several sources become one entry with `sources`); mux entry = aggregate of hosted sessions plus pane-only evidence | n/a |
| TUI read | direct, in three agent-row builders | needs a reverse index (process to session) the index lacks | parse strings per row | an index accessor such as `presence()` | direct |
| Library API (ADR 0015) | breaks struct literals: 31 `AgentSessionNode {` sites, all fields public, no `#[non_exhaustive]` | a new `GraphNode` variant breaks exhaustive matches | none | adds a `GraphSnapshot` field; about 31 full literals (those that list `mux_probes`) need an edit; `..empty()` users are fine | none |
| Eviction / merge | the harness provider owns the node while a mutator writes into it; works because mutators re-run after merge; daemon ingest must update in place | ids embed the epoch (`hook_sidecar.rs:353-358`), so ids churn per event | links accumulate between cycles (§1.6) | must be handled in `evict_provider`, `canonicalize`, `From<GraphSnapshot> for GraphFragment` (which drops unlisted fields); upsert by key in the ingest path | n/a |
| Fingerprint (ADR 0091) | **must** zero it (`discovery/mod.rs:864-880`) | process nodes affect the slice | links affect the slice | untouched | untouched |
| Mux or pane-only state (no session found) | needs a `MuxSessionNode` field too, and the active pane only | natural: the process is linked to the mux | possible through `MuxContainsProcess` | key by `MuxSession` id, optionally with a pane sub-key | yes |

Lean from the audit: **D**, a sparse side table produced by the
`hook_sidecar` mutator and by the daemon ingest upsert, with the
per-source evidence kept in the entry. It follows the newest precedent
(ADR 0140 presence), keeps node identity and the fingerprint stable,
gives a cheap socket payload for fast polling, and keeps "evidence, then
a pure merge" without forcing a state into `GraphLink`. **A** is the
smallest change, but it spreads churn into identity nodes, the
fingerprint, and the library API. **B** is the right shape if pane-level
state should become graph structure. **E** is acceptable only as a
cache in front of D.

---

## 3. Existing state vocabulary and TUI machinery

### 3.1 Glyphs and words already in use (collision list)

From `docs/tui-columns.md:103-134`, `:151-164`, and
`src/tui/icons.rs:86-131`:

| In use | Meaning | Implication for live state |
|---|---|---|
| `⊸` / `∘` | session linked to a definitive mux / mux has a client | taken by `Col::State` / `Col::MuxState` |
| `⊘`, `?` | mux stopped; can't be checked | `?` can't also mean "state unknown" in another sense |
| `○ ◐ ⊕ ⊗ ◌` | PR states, untracked | `○` and `◐` are taken |
| `◉ ◎` | pin marker | taken |
| `●` | agent-session kind glyph | not a state cell; recoloring it breaks principle 2 ("none depends on color alone", `tui-columns.md:31-34`) |
| `▲` | ambiguous link (group header) | taken |
| `√ × •` | forms and checklists | taken |
| Braille spinner | refresh pending for this row | **can't** stand for "working" |
| Words: `live, in mux, resumable, stopped, removed, missing, planned, unknown, open, draft, merged, closed` (`src/tui/rows/mod.rs:267-286`) | Pinned and History | "one word for one thing" (`tui-columns.md:35-37`): a harness that exited inside a running mux is not `stopped` |
| Detail field `status: active · last 4m ago` (`src/tui/explorer.rs:1216-1222`, `:1606-1611`); glossary `("status","active")` (`src/tui/preview/value.rs:150-152`) | right pane | the natural slot for the precise state, its age, and its source |

A new glyph needs a vetted single-cell entry (ADR 0154; tuikit `glyphs`
or `icons::state::ALL`) and a row in the doc tables.
`tests/tui_columns_doc.rs` fails on drift; regenerate the frames with
`just tui-columns`.

### 3.2 Row view-models and columns

- `AgentSessionRow` (`src/tui/rows/mod.rs:495-551`) has
  `mux_state: MuxIndicator` (`:617-627`), recency, preview, and
  `mux_host`.
- `MuxSessionRow` (`:631-672`) has `client_attached`, `attached_count`,
  `agent_labels`, `single_session_preview`.
- `MixedRow` (`:301-328`) serves Pinned and History.
- `EntityState` (`:395-404`) is the state-glyph enum.
- Session schema (`columns.rs:189-204`): Prefix, Key (drops 1), Chip
  (drops 2), Age (drops 3), **State (fixed 1, never drops)**, Label,
  WasIn, Project, Hits, Preview. There is **no free state column**. A
  new `Col::` (`columns.rs:31-111`) needs a width and a drop rank
  (ADR 0183).
- Mux schema (`:211-221`): MuxName, MuxChips, MuxAge,
  **MuxState (fixed 1)**, MuxHits, MuxContent. A mux-level aggregate
  (several agents) needs its own column too.
- Density modes (CSP-270) will choose a schema per mode, so a state
  column must survive `compact`.

### 3.3 Header

`header_line` (`src/tui/ui/header.rs:48-77`) reads:
`[updated Ns ago] · N/M sessions · M mux [· K stopped] [harness chips] [▲ N]`.
Ranks: FRESHNESS 5, MUX 4, AMBIGUITY 3, STOPPED 2, HARNESS_CHIPS 1
(`:15-21`). `HeaderCounts` walks the visible row tree (`:86-110`). The
ambiguity chip is `StatusChip::new(marks::WARNING, Tone::Warning)`
(`:145-150`). A "N waiting" triage chip goes in as a `Segment` with a
rank. ADR 0078, as amended by ADR 0190, governs the order.

### 3.4 Filter, sort, Controls

- `RowFilter` (`src/filter.rs:181-237`) with `has_narrowing_predicates`
  (`:259-265`), `matches_session` (`:269-292`), and
  `SessionMatchInputs` (`:547-560`).
- `MuxStateFilter` / `MuxStateKey` (`:473-543`) is the template for a
  `StateFilter`.
- Float toggles (`float_muxed_sessions_top`, ...), Controls rows
  (`src/tui/widgets/controls.rs:380-438`), CLI `--mux-state`
  (`src/cli/mod.rs:503-606`, `src/cli/search.rs:118`).
- Sorts are `Sort::{Hierarchy, Recency}` (`src/tui/mod.rs:233-236`)
  plus `MuxRecency` (`:243-254`). Session order is `compare_sessions`
  (`src/tui/rows/sessions.rs:1030`). `RowFilter` persists in TUI state,
  so new fields need `skip_serializing_if`.

### 3.5 Theme

- Framework tokens `muted`, `inert`, `success`, `warning`, `error`,
  `info`, and `accent` (`crates/tuikit/docs/controls/theme.md:122-129`).
- `StatusChip` tones `Muted`, `Inert`, `Accent` ("work in progress"),
  `Info`, `Success`, `Warning`, `Error`
  (`crates/tuikit/src/chip/status.rs:15-34`).
- Host roles live in `ROLES` (`src/tui/theme.rs:108-178`). New colors
  become roles there (ADR 0153), and raw `Color::` use is guarded
  against. **No `status_*` roles exist**, despite CSP-268's claim.

### 3.6 Existing near-live signals

| Signal | Where | Cadence | What it can say | Limits |
|---|---|---|---|---|
| Transcript mtime, giving `last_active_epoch` | adapters (§2.2) | daemon harness class 5 s, woken by the recursive watcher on `~/.claude`, `~/.codex`, and others (`src/server/mod.rs:1212-1241`), at most every 1.25 s (`:1390-1395`); daemonless TUI 30 s | "wrote recently", roughly working | can't tell waiting from ready |
| Mux `activity_epoch`, `client_attached` | `node.rs:376-378` | mux 5 s | pane output; someone looking | operator typing counts as activity |
| Process tree | `src/discovery/cross_link.rs:1219-1290` (pid, ppid, command, cwd; the `/proc` state char is parsed and dropped at `:1277-1280`) | when the mux or harness slice changed (ADR 0091) | harness alive or exited; opencode subagent processes | no CPU or run state |
| Hook pid liveness | `hook_sidecar.rs:60-65`, `:408-417` | every replay | "exited" | Linux `/proc` only |
| Pane capture | `src/discovery/tmux/mod.rs:558-575` (`capture-pane -p -J -e -S -100`) | on selection change only, synchronous on the UI thread (`src/tui/runtime.rs:644-648`); `mux_preview_interval` 2 s parsed but unused (`src/tui/preview/mod.rs:8-10`) | visible prompt text | Tier 2 and operator-gated; scraping rejected (ADR 0028); one subprocess per capture |
| Codex logs DB | `discovery/codex_log.rs`, ADR 0048 | process-tree cycles | current thread per pid | Tier 1, no payload |
| `~/.claude/sessions/<pid>.json` | `hook_sidecar.rs:432-443` (reads `kind` only) | every replay | bg or interactive | other fields unread |
| OpenCode `session.idle`, `session.status`, `session.compacted` events | §1.1 | per event | idle, compacted (name only) | status value dropped |

---

## 4. Other surfaces that would show state

- **Machine-readable output.** Only `conspectus graph --format
  json|dot|html` (`src/cli/graph.rs:16`, `:78-95`). JSON is serde of
  `GraphSnapshot` (`src/output/mod.rs:18-20`), with goldens in
  `tests/graph_snapshots.rs`, `dot_snapshots.rs`, and
  `html_snapshots.rs`.
- **`conspectus table`** is text only (columnar or card). Columns come
  from `SESSIONS_COLUMNS` (`src/output/render.rs:246`), `MUX_COLUMNS`
  (`:351`), and `UNION_COLUMNS` (`:402`), with cells in
  `src/output/agent.rs:155-197` and `src/output/mux.rs` (`"activity"`
  at `:188`). `conspectus columns` lists them.
- **`conspectus node show`** is text (`src/output/node_show.rs:137`).
- **There is no `query` command.** ADR 0082 retired SQLite and `query`,
  and ADR 0043 is partially superseded, so no SQLite consumers remain.
- **Library** (ADR 0015, `docs/library-api.md`): `model` is the stable
  pure module, so any model change is API. `hook` is `#[doc(hidden)]`
  (`src/lib.rs:6`, `:29`; `docs/library-api.md:81`).
- **Resolution chain for one-shot commands:** daemon snapshot, else a
  cold rebuild that writes `graph.bin` (`src/cli/mod.rs:171-212`). CLIs
  don't read `graph.bin`, except `refresh`'s prior
  (`src/cli/lifecycle.rs:176-200`) and the daemon's warm start.
- **Daemon commands:** `ping`, `status`, `refresh`, `snapshot` (base64
  rkyv in JSON), and `hook_ingest` (`src/server/mod.rs:614-636`).
  `docs/design.md:1603-1612` omits `hook_ingest`. One request and one
  response per connection, a thread per connection
  (`src/server/mod.rs:1063-1083`). **There is no push or subscribe.**
  ADR 0104 (`:62-66`) defers tmux subscription. A long-poll command
  (one request, a delayed response) would fit the framing without
  multiplexing.
- **Cadences:**
  - Server intervals: harness 5 s, mux 5 s, git 30 s, forge 3600 s
    (`src/config.rs:267-276`); forge poll 10 s (`src/server/mod.rs:361`).
  - TUI `refresh_interval` 30 s (`src/tui/mod.rs:187`), run loop and
    redraw every 100 ms (`src/tui/runtime.rs:129`), `r` refresh
    (`:567-575`), pane-visibility refresh at a 250 ms poll
    (`src/tui/runtime/visibility.rs:16`), daemonless forge 10 s
    (`src/tui/runtime.rs:232`).

**How fast a hook event becomes visible today:**

| Mode | Hook to graph | Graph to TUI | Worst case |
|---|---|---|---|
| Daemon up | spawn and tmux probes, ≤200 ms accept poll, writer lock (≤ a class cycle), resolve, serialize, `graph.bin` write; synchronous for the hook | next TUI poll ≤30 s (`runtime.rs:525`), or `r`, or pane-visibility regain | ~30 s |
| Daemon up but `snapshot_unavailable` or error | client writes the spool | daemon replays it on its next cycle (≤5 s), then the TUI poll | ~35 s |
| No daemon | spool written at once | the next 30 s timer runs a full cold discovery (seconds) that replays the spool (`runtime.rs:1404-1414`) | 30 s plus discovery time |
| One-shot CLI | either | next invocation | n/a |

---

## 5. CSP-268 and neighboring tasks: what they assumed

| Task | Status | What it assumed, and whether that still holds |
|---|---|---|
| CSP-268 "Detect session live status (running / waiting / idle / error)...", m-13 | To Do; depends on CSP-185 | pane deltas against the last capture, tied to capture throttling (scraping-adjacent; ADR 0028 and Tier 1 object); hook evidence "when the harness exposes waiting on permission or tool error" (no such events are installed); a `SessionStatus` enum on the row view-model; reserved `status_*` theme colors (**false**, per its CSP-766 note); header chips swapping a per-mux-state breakdown for per-status counts under `[tui.show_status]` (that breakdown was already collapsed to `▲ N` by ADRs 0072 and 0078; harness chips are opt-in via `show_harness_chips`); needs an ADR on whether status is a candidate link or a property on `AgentSessionNode`, and on cadence and cost |
| CSP-185 Throttle and freshen pane captures | To Do | interval refresh, a "captured Ns ago" label, background capture; capture is still synchronous on the UI thread |
| CSP-168 mux live-preview adapter | Done | `capture_pane` with typed outcomes; `PreviewStore` keyed by mux |
| CSP-243 Live-session UX advisory | Done | used "fresh hook evidence per `ACTIVE_TTL_SECONDS`"; the constant has no reader now |
| CSP-249 Pane dedupe; drop the 15-minute gate | Done | kept the constant "for higher-layer freshness signals"; dedupe is by `(mux, pane)` in memory but the spool collapses panes (§1.5) |
| CSP-448.05 Revisit hook SQLite | Done | dropped append-only history on purpose; its summary's "persists via `graph.bin`" was corrected by CSP-698 / ADR 0146 |
| CSP-698 Keep hook observations across refreshes | Done | ADR 0146 |
| CSP-223 Audit hooks as session-state emitters | Done | already listed opencode `permission.ask`, `tool.execute.before/after`, `chat.message`, `session.status`; Codex hook semantics unknown then |
| CSP-219 Control-plane audit | Done | Codex app-server and opencode server/ACP were gated; Claude went through hooks |
| CSP-191 / CSP-270 density modes | To Do | a schema per mode; a state column must fit `compact` |
| CSP-193 visible search and filter | To Do, maybe already delivered | filter chips with counts (CSP-256) are where a state filter would show |
| CSP-295 to CSP-298 subagent metadata | To Do | `SessionKind` already exists on the node; relevant to "subagents working" |
| CSP-098 activity column in tables | To Do | `activity` columns exist, off by default |
| CSP-546 consolidate hook docs | To Do | docs cover Claude, Codex, and Pi installs; the Python shim is still shipped |

---

## 6. Risks to weigh

- **Hook latency is agent latency.** Claude Code tool hooks run
  synchronously, and the Codex entry sets `async = false`. The client
  waits for the full daemon ingest and has no `set_read_timeout`
  (`src/server/mod.rs:498-562`), so a hung daemon would hang the agent.
  The OpenCode and Pi plugins spawn detached; the Pi one calls
  `unref()`.
- **Daemon CPU and disk per event.** A full resolve, serialize, and
  `graph.bin` rewrite per ingest (ADR 0091 names these as the dominant
  idle costs) plus a spool read-modify-write. Events per tool call
  multiply all of it.
- **Lost or reordered events give stuck states.** Daemonless races, and
  second-resolution timestamps. This needs a TTL and decay policy and
  sequence or ms ordering.
- **Mixed binary versions.** A spool `SCHEMA_VERSION` bump lets an older
  binary clobber the spool (§1.3). Plugins spawn whatever `conspectus`
  is on `PATH`.
- **Multi-pane muxes.** The spool collapses panes. The graph knows only
  the active pane, and `pane_running_harness` gates on the active pane.
- **Ambiguous attribution.** A mux's state derived through
  `LinkedToMux` inherits attribution ambiguity, and between cycles the
  ingest path leaves two Active hook links (§1.6).
- **Pane-capture cost and privacy.** Capturing every pane on a timer is
  N subprocesses per tick on the UI thread today, and a Tier 1 or
  Tier 3 privacy problem if used for inference.
- **Library API churn** if fields are added to public model structs
  (§2.4).

**Test strategy already in place:**

- Unit tests: `src/hook_tests.rs` (11 tests),
  `src/discovery/hook_sidecar_tests.rs` (26),
  `src/server/server_tests.rs`, including `hook_ingest_*` at `:57`,
  `:159`, `:186`.
- CLI smoke tests for `hook write` per harness
  (`tests/cli_smoke.rs:454-544`) and payload fixtures in
  `tests/fixtures/hooks/*.json` (`tests/fixture_corpus.rs:354`, `:392`).
- `ReplayWorld` with a hook root (`tests/support/replay.rs:20-48`) and
  the `hook-supersession` dev scenario plus showcase
  (`src/dev_scenarios.rs:67-69`, `:678`, `:1583-1610`).
- `tests/tui_columns_doc.rs` and the TUI consistency suite
  (`src/tui/consistency_tests/{glyphs,state,text,freshness}.rs`), and
  `--snapshot` for visual checks.
- The traced capability test runs `hook init` and `hook remove` but
  **not** `hook write` (`tests/capabilities.rs:1223-1275`).

---

## 7. Extension points

**(a) Emitted by a hook**

1. `ensure_claude_hook`, `has_claude_hook`, `remove_claude_hook`,
   `is_conspectus_hook_command` (`src/cli/hook.rs:784-875`). Today: one
   `SessionStart` entry. Live state adds entries for more events (the
   Claude researcher's list) and must keep install, status, and remove
   idempotent.
2. `ensure_codex_hook` and `codex_hook_entry_value`
   (`src/cli/hook.rs:877-954`): event list and `async`.
3. Pi extension `plugins/pi-hook/conspectus-hook.ts:22-53`: more `pi.on`
   events. The embedded copy (`src/cli/hook.rs:644-645`) plus the
   version marker drive `hook status pi` reporting `outdated`.
4. OpenCode `plugins/opencode-hook/src/index.ts:22-43`, `v2/tui.ts:22-73`,
   and `write.ts:61-65`: forward a normalized state or the status type,
   not only the event name.
5. `HarnessAdapter::hook_record_from_payload`
   (`src/discovery/harness/mod.rs:336-375`): the per-harness override
   that maps native events to a neutral state and keeps only non-payload
   fields (Tier 3).
6. `HookWriteArgs::run` (`src/cli/hook.rs:63-91`) and `tmux_context`
   (`:581-605`): read `TMUX_PANE` directly to save two tmux spawns, add
   ms or sequence timestamps, and decide whether to detach before the
   daemon round trip.

**(b) Recorded in the spool**

7. `HookRecord` (`src/hook.rs:17-45`): additive optional fields
   (state, ms timestamp or sequence, detail enum), **without** a
   `SCHEMA_VERSION` bump.
8. `mux_record_key` (`src/hook.rs:242-258`): key by pane and session,
   or add a sibling latest-state store beside `hooks-latest.json`
   written through `HookStore::write_record` (`:87-116`), with a lock
   if daemonless writes are frequent (the `history.lock` precedent,
   ADR 0140).
9. `HookStore::read_records` (`src/hook.rs:118-122`) and the
   legacy-JSON reader (`:260-273`).

**(c) Ingested by the daemon and by in-process discovery**

10. `handle_hook_ingest` (`src/server/mod.rs:648-723`): a fast path for
    state-only records (upsert the side table, debounce or skip the full
    resolve and `graph.bin` write, reply early). `call_command`
    (`:498-562`) needs timeouts. A new `dispatch` arm (`:614-636`) can
    serve state alone.
11. `apply_hook_records` (`src/discovery/hook_sidecar.rs:28-129`):
    build per-session and per-pane state entries after the
    winner-per-`(mux, pane)` pass, and supersede older hook links for
    the same pane during ingest.
12. `apply_mutators` (`src/discovery/mod.rs:937-939`), the
    `HOOK_SIDECAR` descriptor (`providers.rs:119`, `:271`), and
    `MUTATOR_PROVIDERS` (`cache.rs:151`): in-process replay.
13. `HarnessAdapter::apply_aux_attribution`
    (`src/discovery/harness/mod.rs:319-325`): the seam for non-hook
    native state sources (Codex logs, opencode DB status,
    `~/.claude/sessions/<pid>.json`).
14. The daemonless TUI timer (`src/tui/runtime.rs:522-528`): a cheap
    "re-read the state store and re-apply" step instead of a full
    discovery.

**(d) Attached to the graph**

15. A model type for the neutral state enum and observation struct
    (rkyv and serde) in `src/model/`, and a `FORMAT_VERSION` bump
    (`src/snapshot.rs:53`).
16. A side table on `GraphSnapshot` (`src/model/graph.rs:16-71`) with
    a serde helper like `node_presence_serde` (`:73-114`). Wire it into
    `evict_provider` (`:474-495`), `canonicalize` (`:442-449`),
    `GraphFragment` and `From` (`src/discovery/mod.rs:297-333`) if a
    provider emits it, and add an index accessor (`graph.rs:205-214`).
    Alternative: fields on `AgentSessionNode` and `MuxSessionNode`
    (`node.rs:265-290`, `:348-399`), plus `fingerprint_normalized_node`
    (`src/discovery/mod.rs:864-880`).
17. An optional pure merge step after `resolve_snapshot`
    (`src/resolve/mod.rs:12`) to combine sources, with mux aggregation
    through `preferred_mux_for` (`src/model/graph.rs:356-378`).

**(e) Exposed through the CLI and JSON**

18. `render_graph_json` (`src/output/mod.rs:18`) and the goldens in
    `tests/graph_snapshots.rs`.
19. `SESSIONS_COLUMNS` and `MUX_COLUMNS` (`src/output/render.rs:246`,
    `:351`) with their cells (`src/output/agent.rs:155-197`,
    `src/output/mux.rs`), and `conspectus columns`.
20. `node show` (`src/output/node_show.rs:137`); DOT and HTML
    (`src/output/dot.rs`, `src/output/html`).
21. A `--state` filter beside `--mux-state` (`src/cli/mod.rs:503-606`,
    `src/cli/search.rs:118`).
22. An optional socket command `live_state` (or a long-poll `wait`) in
    `dispatch` (`src/server/mod.rs:614-636`) and the client helpers
    (`:380-493`).

**(f) Read by the TUI row builders**

23. Row fields: `AgentSessionRow` (`src/tui/rows/mod.rs:495-551`),
    `MuxSessionRow` (`:631-672`), `MixedRow` (`:301-328`).
24. Builders: `emit_session_row` (`src/tui/rows/sessions.rs:896-935`);
    `rows::agent_row` (`src/tui/rows/mod.rs:1229-1260`, the PRs view);
    `rows/mux.rs` `agent_row` (`:856`), `collect_attached_agents`
    (`:775`), and the parent `MuxSessionRow` (`:152`, `:741`); the
    Pinned and Search builders.
25. Columns: the `Col` enum (`src/tui/ui/left_panel/columns.rs:31-111`),
    the session schema (`:189-204`), the mux schema (`:211-221`), and
    the cell producer in `LeftColumns` (`:280-300`), which has `now`
    for render-time staleness.
26. Glyphs and words: `icons::state` (`src/tui/icons.rs:86-131`),
    `rows::words` (`src/tui/rows/mod.rs:267-286`), and
    `docs/tui-columns.md` (`just tui-columns`).
27. Header: `header_rank`, `header_line`, `HeaderCounts`
    (`src/tui/ui/header.rs:15-110`).
28. Filter and sort: `SessionMatchInputs` and `RowFilter`
    (`src/filter.rs:181-292`, `:547-560`), Controls rows
    (`src/tui/widgets/controls.rs:380-438`), `compare_sessions`
    (`src/tui/rows/sessions.rs:1030`), and mux sorting
    (`src/tui/rows/mux.rs:501-540`).
29. Theme roles (`src/tui/theme.rs:108-178`).
30. Detail pane: the `status` field and `session_status`
    (`src/tui/explorer.rs:1216-1222`, `:1606-1611`) and the glossary
    (`src/tui/preview/value.rs:150-152`).
31. Refresh: the `LiveMode` timer and `try_daemon_snapshot`
    (`src/tui/runtime.rs:522-528`, `:1446-1455`).

---

## 8. Constraints the design must not violate

1. **No terminal input into agents** (ADR 0028, ADR 0143 prohibition 2,
   absolute). Live state is observation only. The TUI may attach to a
   waiting agent, but it never approves or types on the operator's
   behalf.
2. **No generic terminal scraping for state** without a separate ADR,
   and only after hooks, control planes, and harness state fail
   (ADR 0028 `:128-133`). Pane text is Tier 2: operator-gated, capped,
   and never fed to attribution or state (ADR 0086).
3. **Payload-free observations** (ADR 0086 Tier 3; ADR 0143
   prohibition 3). Hook records and state entries carry neutral enums,
   ids, and timestamps, never prompt, tool-input, notification, or
   assistant text. Any harness-DB or transcript reader used for state
   states its tier in its ADR (Tier 1: structural fields only).
4. **Mutation envelope** (ADR 0143). New stores go under
   `$XDG_STATE_HOME/conspectus/` (category 2). The daemon writes
   category 2 only. New hook entries come only through operator
   `hook init` (category 5), stay idempotent, and `hook remove` must
   remove them all. No other harness-settings writes.
5. **Capability manifest** (ADR 0117). New programs, subcommands,
   sockets, or write paths go in `docs/capabilities.toml` with an ADR.
   Spawning go through `exec::command`. The `tuikit` crate starts no
   process and writes no file.
6. **The daemon is optional** (`docs/design.md:87-91`, `:1596-1598`,
   ADR 0038). One-shot CLIs and a daemonless TUI must produce the same
   state from the spool or state store; absence of a server is not an
   error.
7. **The hook must never disturb the harness.** Nothing on stdout
   (Claude injects `SessionStart` and `UserPromptSubmit` stdout into
   context). Never exit with a blocking code (exit 2 blocks a Claude
   tool). Errors are tolerated. Latency must be bounded, so detach or
   time out, and a hung daemon must not hang the agent.
8. **The data model comes first and is provider-neutral**
   (CLAUDE.md). The neutral state vocabulary lives in `src/model`, and
   harness event names stay in adapters and plugins.
9. **The graph is sparse.** No entry means no evidence. Don't infer
   "idle" from silence without a documented TTL and decay, computed at
   read time.
10. **The spool stays compatible across versions.** Add fields; don't
    bump `SCHEMA_VERSION` without a non-clobbering read rule.
11. **Respect the ADR 0091 idle budget.** Don't reopen the process-tree
    gate on state churn (fingerprint). Don't rewrite `graph.bin` per
    event without debounce. Background pane capture is not free.
12. **TUI vocabulary rules** (ADR 0145, `docs/tui-columns.md`
    principles 1-5). Glyphs in Sessions, Mux, and PRs; words in Pinned
    and History. Each value is its own shape and never color alone. One
    meaning per glyph and word (the spinner, `?`, `○`, `◐`, `stopped`,
    and `live` are taken). Glyphs are vetted at one cell (ADR 0154).
    Colors are theme roles (ADR 0153). Rows come from column schemas
    (ADR 0183). Keys come from the scoped keymap (ADR 0192), and hints
    are generated.
13. **User-facing text never cites ADRs, backlog ids, or `docs/`
    plans** (CLAUDE.md; `src/tui/consistency_tests/text.rs`). "pin",
    not "slot".
14. **Docs in the same change.** `docs/tui-columns.md` and its frames,
    `docs/design.md` (and its stale socket list), `docs/operations.md`,
    a new ADR (next number 0203) refining 0028 and 0146, plus a
    framework record if `tuikit` gains a control or mark.

## 9. Drift and latent bugs found along the way

- ADR 0028 still describes `hooks.sqlite3` and a 15-minute TTL
  (`:50-56`, `:90-94`).
- `docs/operations.md:80-81` claims stale records are ignored.
- `docs/design.md:1603-1612` omits `hook_ingest`.
- `ACTIVE_TTL_SECONDS` is dead (`src/hook.rs:14`).
- The spool key ignores the pane; the per-pane test uses legacy JSON
  (§1.5).
- The ingest path leaves two Active hook links for one pane until the
  next cycle (§1.6).
- Socket clients have no timeouts (`src/server/mod.rs:498-562`).
- `MuxIndicator`'s doc comment still says `◉`/`◯`
  (`src/tui/rows/mod.rs:613-625`).
- The traced capability test doesn't cover `hook write`.
