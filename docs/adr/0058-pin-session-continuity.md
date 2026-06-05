# ADR 0058: Pin Session Continuity

## Status

Accepted

## Context

ADR 0057 (Session Pins) binds a pin to a live `(MuxSession,
AgentSession)` pair via mux-anchored attribution. The binding is
recomputed from fresh discovery on every cycle: the pin entry itself
records only the *declaration* (`harness`, `cwd`, `mux.name`,
optional `launch.argv`), never an *observation* of which session was
last attached.

This works for the steady state. It breaks at one specific
transition: **when the bound mux dies and the operator wants to
resume the same agent session on relaunch.**

### The scenario

1. `conspectus pin create my-work --harness codex --cwd /p`
2. `conspectus pin launch my-work` → spawns `tmux new-session -d -s
   my-work -c /p codex`. Codex creates session A. Resolver binds
   (mux=`my-work`, session=A).
3. Inside codex, the operator starts a fresh session (`/new`,
   `--continue`-derived, or kills and re-launches codex). The pane
   now runs session B. Resolver re-attributes: bound to B.
4. The tmux session is closed (`tmux kill-session`, terminal exit,
   reboot).
5. `conspectus pin launch my-work` → unbound branch → `tmux
   new-session ... codex` with the default `["codex"]` argv. Codex
   starts session C. The operator's actual work history (A, B) is
   still on disk but disconnected.

Step 5 silently breaks continuity. The pin's `display_name` and mux
identity are preserved, but the agent session the operator was
actually inside is gone from the deck's primary launch path.

### What conspectus already has nearby

- **`HarnessAdapter::launch_argv`** (`src/discovery/harness/mod.rs:45`)
  returns a static per-harness default (`["codex"]`,
  `["claude"]`, etc.). Operators can override with
  `pin create --launch-arg`. No notion of resume.
- **Hook sidecar** (`src/discovery/hook_sidecar.rs`,
  ADR 0028 / 0048) writes per-pane JSON records that produce
  `StrongDiscovered + Fresh` `LinkedToMux` candidates. The records
  live outside project trees, survive conspectus restart while a
  mux is up, but are written by *harness hooks*, not by conspectus
  itself.
- **Resolver precedence pipeline** (ADR 0006 / 0046 / 0047 / 0048)
  handles `LinkedToMux` candidates with Provenance, Confidence,
  and Freshness fields. Stale + Lower-Confidence observations
  defer to fresh strong evidence automatically.
- **Declared `linked_to_mux` overrides** (ADR 0014) with
  `label = "pin:<id>"` are how `pin bind` resolves
  `PinAmbiguous`. They are *authoritative* — the resolver short-
  circuits to them when present.
- **Atelier resume command** (existing in some adapters) shells
  out the harness with its native resume flag. Resume argv is
  per-harness today but lives outside the pin schema.
- **`CLAUDE.md` cache rule**: rebuildable caches live outside
  project trees under `$XDG_CACHE_HOME/conspectus/`.

### What's missing

Three things, in order of necessity:

1. **Durable record of "the most recent `(pin_id, session_id,
   mux_name, epoch)` binding."** Conspectus already knows the
   binding during a cycle; nothing writes it to disk.
2. **Per-harness `resume_argv(session_id)`.** Sibling to
   `launch_argv()` — knows how to spell "resume session X" in
   that harness's CLI.
3. **A launch decision tree change.** Today: bound → attach;
   stale → send-keys; unbound → default-argv `new-session`. The
   unbound branch needs a sub-decision: "do we have a known
   prior session we should resume into instead?"

The first piece is the architectural question this ADR resolves.
The second and third are mechanical once it's decided.

## Decision

Adopt **Option E**: a rebuildable per-pin JSON sidecar under
`$XDG_CACHE_HOME/conspectus/pin-bindings/` records every fresh
binding the resolver produces; at `pin launch` time, when the
resolver returns `PinUnbound`, consult the sidecar and inject the
harness's `resume_argv(<session_id>, <cwd>)` into the launch
instead of the default argv when a usable prior session exists.

### Sidecar shape

One file per pin: `$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`.

```json
{
  "schema_version": 1,
  "pin_id": "<id>",
  "mux_name": "<name>",
  "mux_socket": "<name or null>",
  "session_id": "<harness-native session id>",
  "harness": "<harness key>",
  "observed_epoch": 1738742400
}
```

Schema versioning follows ADR 0057's pattern: `schema_version: u32`,
unknown-field tolerance on read, validation on write, malformed
files leave the sidecar alone and surface a discovery diagnostic.

Writes are atomic (tempfile + rename). Per-pin granularity avoids
read-modify-write contention when multiple pins update in the same
cycle.

### Write path

After resolver completes each discovery cycle, for each pin
resolution where the binding is `Bound` (or, per Q6 below, where
`pin bind` wrote a `LocalDeclared` override that the resolver
honored):

1. Construct the sidecar record from the resolved binding.
2. Compare against the current sidecar on disk for that pin (if
   any). Skip the write when the payload is unchanged (no churn
   on quiet cycles).
3. Atomically write the new payload.

The sidecar is never written for `PinUnbound`, `PinStaleMux`, or
`PinAmbiguous` resolutions. Stale entries are pruned at launch
time (see Q7 below), not on every cycle.

### Read path (launch only)

On `pin launch <id>`, when the resolver returns `PinUnbound`:

1. **Load the sidecar.** If absent → status hint, fall through to
   default argv.
2. **Resolve the recorded session to its current head.** Look up
   the recorded `session_id` in the current discovery snapshot's
   `AgentSession` nodes. If absent, fall back to a direct check
   against the harness's state root (per Q3 below). Then walk the
   ADR 0018 `parent_session` chain *forward* — following
   successors — until either:
   - A leaf is reached (no successor recorded): use that session id.
   - A fork is encountered (multiple successors share an
     ancestor): stop. Surface the ambiguity as a status hint
     ("session has multiple compacted successors; resume one
     manually") and fall through to default argv.
3. **Validate the chosen session exists on disk.** If the lookup in
   step 2 found nothing usable (no live snapshot entry *and* no
   harness-state-root file), **delete the sidecar file** (per Q7
   below), surface a status hint ("previous session no longer
   exists; launching fresh"), and fall through to default argv.
4. **Consult `HarnessAdapter::resume_argv(session_id, cwd)`.** If
   it returns `None` (the harness has no resume CLI flag), status
   hint + default argv. If `Some(argv)` → use that argv with
   `tmux new-session`.

The resolver does **not** consume the sidecar. The launch decision
is the only consumer in this ADR.

### Resolver integration

None. This ADR keeps the sidecar out of the resolver pipeline.
Whether to wire it back in as a `LinkedToMux` candidate kind for
the "long-running tmux + conspectus restart" case is deferred to
a follow-up ADR (see Alternatives Considered → Option A).

### TUI surface

- `pin show <id>` surfaces the recorded last-bound session id with
  an `observed at <epoch>` line when one exists.
- The `PinUnbound` diagnostic is extended (not replaced — per Q5
  below) with an optional `last_session: Option<{session_id,
  observed_epoch}>` field. When populated, the TUI status hint
  reads `Enter to resume <session_id>` rather than `Enter to
  launch`.
- The right detail pane shows the resumable session id alongside
  the unbound state when applicable.

### What this ADR does *not* lock in

- Whether the sidecar should *also* produce `LinkedToMux`
  candidates that feed the resolver (Option A). Deferred to a
  follow-up ADR once Option E's launch story is in production
  and the failure modes are observed.
- Whether tmux `set-option` should be a backup write target
  (Option C) for the live-mux + conspectus-restart case. Out of
  scope here — complementary tool with its own decision surface.

## Consequences

### Benefits

- **Cross-mux-death continuity.** The user's stated scenario
  resolves: relaunch picks up where the pin left off, modulo
  honest fallback when the session no longer exists.
- **Hooks-future-proof.** When the harness exposes session
  hooks (per ADR 0049-style plugin distribution), the sidecar
  writer can subscribe to "session changed in this pane"
  events for near-real-time fidelity, without changing the
  launch-time consumer.
- **Composes with adopt.** After `pin adopt`, the sidecar
  starts recording from the adopted session; no special case.

### Costs

- **One new sidecar format.** Versioned JSON, atomic
  rename-on-write. Per-pin file is cheaper than one global
  file (no read-modify-write contention).
- **Per-harness resume knowledge.** Each `HarnessAdapter`
  gains `resume_argv(&self, session_id: &str) ->
  Option<Vec<OsString>>` returning `None` for harnesses
  without a resume CLI flag. Aider may return `None`; codex
  and claude-code return `Some`.
- **One new "should we resume?" decision in launch.** Today's
  three-branch tree (bound/stale/unbound) becomes four
  (bound/stale/unbound-with-prior/unbound-fresh). The
  `PinUnbound` diagnostic gains an optional `last_session`
  field rather than spawning a new diagnostic (per Q5).
- **Failure mode: stale sidecar entry.** If the recorded
  session is deleted/expired, the launch path detects it,
  deletes the sidecar file (per Q7), surfaces a status hint,
  and continues with default argv. No silent staleness.
- **Failure mode: fork in `parent_session` chain.** When the
  recorded session has been compacted into multiple
  successors (rare but possible per ADR 0018), the walk
  stops at the fork and falls back to default argv with a
  status hint advising manual resume. Honest about
  ambiguity rather than picking arbitrarily.

### Non-consequences

- **No `.conspectus.toml` churn.** Pin entries are unchanged.
- **No resolver semantics change for already-handled
  cases.** Bound, stale, ambiguous, drift all behave
  identically to ADR 0057.

## Alternatives Considered

### Option A: Evidence-based, sidecar replays as resolver candidates

Conspectus writes the binding to a sidecar; on next discovery,
the sidecar pass emits a `LinkedToMux` candidate with provenance
`RecordedObservation` (new variant) and freshness `Stale` (or
similar). The resolver's existing precedence pipeline handles
the rest.

**Why it's appealing.** Reuses ADR 0006 / 0028 / 0046
machinery wholesale; no new launch-decision logic. Stale
observations defer to fresh strong evidence automatically.

**Why it's not quite right for the stated problem.** The user's
scenario is *unbound + launch* — when the mux is gone, no
`LinkedToMux` candidate (replayed or otherwise) attributes a
session to a live mux. The resolver still says `PinUnbound`.
Continuity comes from the launch decision, not the resolver's
attribution.

The evidence flavor *is* useful for a different scenario:
**conspectus restart while a tmux is still up.** Today, after
restart, the hook sidecar re-establishes attribution. If the
hook sidecar is absent (older harness, configuration error), a
conspectus-written observation sidecar could pick up the slack.
That's a real story but it's a separate ADR.

**Verdict.** Adopt the *write* side (Option E does this anyway)
but don't wire it into the resolver yet. Defer the read-side-
as-evidence story to a follow-up ADR.

### Option B: Canonical pin→session mapping file

Maintain a single
`~/.config/conspectus/pin-bindings.toml` (or per-pin file in the
config dir) that records the current binding. Treat it as
authoritative — the resolver short-circuits to it; the launch
path reads it directly.

**Why it's appealing.** Conceptually simple, debuggable, hand-
editable.

**Why it's wrong.**

- **Conflates declaration and observation.** ADR 0014's whole
  point is that declarations beat observations. A
  conspectus-maintained "canonical" file that's actually a
  recorded observation is a declaration in name only and a
  recording in behavior. It creates a class of bugs where the
  user's `pin bind` and the auto-updated mapping disagree.
- **No confidence/freshness representation.** When the
  recorded session is deleted or stale, the file says
  "canonical=X" with no graceful degradation path. Either we
  trust it or we don't, and the user can't tell which from
  the file's contents.
- **Write contention.** A single global file means
  read-modify-write on every cycle if any pin's binding
  changed. Per-pin files would fix that but add complexity.

**Verdict.** Reject. The semantics are wrong, and the
ergonomics promised by "canonical" (debuggable, hand-editable)
are also delivered by Option E's sidecar format, without the
ontological confusion.

### Option C: tmux `set-option` custom property

Each discovery cycle, write
`tmux set-option -t <mux> @conspectus_session_id <id>` for the
bound pin. Read it back via `tmux show-options -t <mux>` on
next conspectus start.

**Why it's appealing.** No new conspectus state. Tmux is
already the source of truth for mux liveness; piggybacking on
its option storage is symmetric.

**Why it doesn't solve the stated problem.**

- **Dies with the mux.** `tmux kill-session` destroys the
  options. The exact scenario the user asked about —
  close-and-relaunch — loses the recorded session id.

**Where it would help.**

- **Conspectus restart, tmux still running.** A long-running
  tmux server where the user briefly stops conspectus would
  benefit: on restart, conspectus could short-circuit the
  process-tree walk by reading the tmux option.
- **Bootstrapping after a hook outage.** If the hook sidecar
  records are missing or stale (a harness was updated, hooks
  were temporarily uninstalled), the tmux option provides a
  fallback evidence source.

**Verdict.** Reject as the *primary* mechanism for the user's
question (it doesn't solve it). Keep on file as a complementary
write target for Option E — write to both the sidecar *and* the
tmux option per cycle; the latter helps the "long-running mux,
conspectus restart" case for free. The decision to do that, and
how to namespace the option (`@conspectus_session_id`,
`@conspectus_pin_id`, both?), is a follow-up.

### Option D: Persist SQLite database (or a section of it)

Snapshot a chosen subset of the in-memory graph (e.g., active
`LinkedToMux` candidates) to a SQLite file under
`$XDG_CACHE_HOME/conspectus/`. Re-hydrate on next start, then
discovery replaces stale rows.

**Why it's appealing.** The graph already models the binding;
persistence is just lifetime extension. Could enable other
caches (PR fetch results, transcript indices, last-N-runs of
discovery) under the same machinery.

**Why it's wrong for this problem.**

- **Massive new surface for narrow benefit.** Continuity for
  pins is one user-visible feature. Standing up persistent
  SQLite implies schema migration policy, IO cost per cycle,
  state-drift mitigation strategy, and answers to "which
  rows survive across runs?" None of these are needed for
  Option E to work.
- **Replayed rows must be marked stale.** Otherwise fresh
  observation can't overrule them. That means adding a
  Stale-Persisted variant to Freshness — a resolver semantics
  change for a single use case.
- **CLAUDE.md tension.** The guidance is "rebuildable caches
  outside project trees." A persisted database *is* outside
  the project tree, but the line between "cache" and "state"
  blurs when discovery's only-source-of-truth becomes
  partially historical.

**Verdict.** Reject for this ADR. Revisit if (a) we want to
persist multiple kinds of cross-run data (cf. transcript
index, PR cache) and (b) the cumulative complexity of N
separate sidecars justifies a unified store.

### Option E: Sidecar of pin-bound observations + launch-time decision (Recommended)

Already described in the Decision section. Repeating the shape
here for completeness:

- **Write path.** After resolver completes each cycle, for each
  pin with a `Bound` resolution: append/replace
  `$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`
  with `{schema_version, pin_id, mux_name, mux_socket,
  session_id, harness, observed_epoch}`. Write is atomic
  (tempfile + rename). Per-pin file avoids contention.
- **Read path (launch only).** On `pin launch <id>`, if the
  resolver returns `PinUnbound`:
  1. Read the sidecar for `<id>`. If absent → fall through to
     default argv.
  2. Look up the recorded `session_id` in the current
     snapshot's `AgentSession` nodes (or check the harness
     state root directly). If missing → status hint
     "previous session no longer exists; launching fresh" +
     default argv.
  3. Otherwise → look up
     `HarnessAdapter::resume_argv(<session_id>)`. If `None`
     (harness has no resume CLI) → status hint + default
     argv. If `Some(argv)` → use that argv with `tmux
     new-session`.
- **Read path (resolver).** *Not connected.* The resolver does
  not consume the sidecar today; binding stays purely
  observation-driven within the current cycle.
- **TUI surface.** `pin show <id>` and the right detail pane
  surface the recorded last-bound session id with an
  "observed at" timestamp. A `PinResumable` diagnostic on
  unbound pins advertises "Enter to resume `<session_id>`."

**Why this option.**

- **Smallest viable change.** One new sidecar format, one new
  harness-adapter method, one new branch in the launch
  decision tree.
- **Honest about uncertainty.** When the recorded session
  disappears, the launch path detects it and degrades to
  fresh launch with a hint. No silent "we thought we had it
  but we don't" behavior.
- **Reversible.** If we later decide to wire the sidecar into
  the resolver (Option A), the sidecar format and write path
  are the same. We just add a `discovery::pin_bindings` pass.

**Costs.**

- New sidecar format = one more thing to version. Mitigation:
  follow ADR 0057's pattern (`schema_version: u32`,
  unknown-field tolerance).
- Per-harness `resume_argv` adds a method to the adapter
  trait. Aider returns `None`; codex and claude-code return
  `Some`. Opencode TBD per its CLI.
- The launch hint surface grows. The operator sees one more
  status line in the failure paths. Acceptable.

### Option F (additional, not in original list): Harness-native introspection at launch

No conspectus persistence. At launch time, query the harness's
own state root for "most recent session in this cwd matching
this harness," use `--resume <id>` (or harness equivalent).

**Why it's appealing.** Zero new state. The harness already
records sessions on disk; we just read them.

**Why it's not quite right.**

- **Per-cwd, not per-pin.** Two pins in the same cwd would
  both pick the same "most recent" session. Wrong for any
  scenario with parallel work in one repo.
- **Roughly equivalent to `--launch-arg --continue`.** Most
  harnesses already provide a `--continue` flag that does
  this. Operators can already opt into this today by setting
  `pin.launch.argv = ["codex", "--continue"]`.
- **No memory of the *pin's* prior binding.** Switching
  inside the pane to a session that wasn't most-recent is
  invisible.

**Verdict.** Reject as the primary mechanism. Recommend
documenting `--continue`-style flags in the operations guide
as a "good enough for single-cwd users" pre-Option-E
workaround.

### Comparison summary

| Option | Solves stated scenario? | New schema/cache? | Resolver changes? | Honest about uncertainty? |
|---|---|---|---|---|
| A: Evidence-replay | No (helps adjacent restart-while-up case) | Yes (sidecar) | Yes (new candidate kind) | Yes (Freshness::Stale) |
| B: Canonical mapping | Yes | Yes (mapping file) | Yes (short-circuit attribution) | No |
| C: tmux set-option | No (dies with mux) | No | No | N/A |
| D: Persist SQLite | Yes | Yes (database file) | Yes (replayed rows + Stale variant) | Yes but expensive |
| **E: Sidecar + launch decision** | **Yes** | **Yes (per-pin JSON)** | **No** | **Yes (explicit fallback)** |
| F: Harness-native at launch | Partially (per-cwd) | No | No | Implicit |

## Open Questions Answered

1. **Sidecar granularity.** Per-pin file (`<pin_id>.json`).
   Avoids read-modify-write contention; one file per pin is
   small enough to write atomically without coordination.
2. **Schema version field policy.** Match ADR 0057:
   `schema_version: u32`, unknown-field tolerance on read,
   validation on write, malformed files leave the sidecar
   alone and surface a diagnostic.
3. **Session existence check at launch.** Snapshot first, fall
   back to the harness's state root if the snapshot has no
   record. The snapshot is always populated by the launch path
   anyway (the resolver runs before the launch decision); the
   state-root fallback catches sessions discovery missed in
   this cycle.
4. **`resume_argv` signature.** Takes both `(session_id, cwd)`.
   Adapters ignore what they don't need.
5. **TUI diagnostic surface.** Extend `PinUnbound` with an
   optional `last_session: Option<{session_id,
   observed_epoch}>` field. No new diagnostic kind. The status
   hint text branches on whether the field is populated.
6. **Bind writes to the sidecar too.** When `pin bind` writes
   a `LocalDeclared linked_to_mux` override that the resolver
   honors, the resulting binding is recorded in the sidecar
   like any other. The sidecar records observations of *what
   was bound*, not *how the binding got chosen*.
7. **Stale TTL.** No TTL. Instead, the launch path's
   existence check is authoritative: if the recorded
   `session_id` cannot be found in either the discovery
   snapshot or the harness state root, the launch path
   **deletes the sidecar file** before falling back to
   default argv. Stale entries never accumulate; they're
   cleared the next time the operator tries to use them.
8. **Walk the `parent_session` chain (ADR 0018) to the
   current head.** When the recorded session has been
   compacted into a successor, the launch path follows
   successor links forward to find the leaf. **If the chain
   forks** (an ancestor has multiple successors), the walk
   stops at the fork — no automatic disambiguation — and the
   launch surfaces a status hint advising manual resume.
   Single-line chains walk transparently; forks are honest
   about ambiguity rather than picking arbitrarily.

## Prior Art

- **agent-deck.** Resumes the most recent "card" by name,
  relying on tmux being up. Does not handle close-and-relaunch
  itself; sessions stay alive in tmux until explicit teardown.
  Conspectus's pin model differs by allowing the mux to die and
  reform — which makes continuity harder, but also makes the
  pin a more honest declaration of intent.
- **smug / tmuxinator.** Restore window/pane layout from a YAML
  declaration; do not persist *which session was active* in each
  pane. Continuity is implicit ("you'll get a fresh shell").
  Conspectus has stronger requirements because the pin
  represents an *agent* session, not a generic shell.
- **VS Code's "restore terminal sessions."** Persists the
  terminal's PID + tty + working state; restores on reopen.
  Architecturally similar to Option E (durable observation +
  on-open consumption), but the *unit* it tracks is the
  terminal, not an agent session inside it.

## Decision Log

- 2026-06-05 — Drafted with five candidate options.
- 2026-06-05 — Open questions resolved (see Open Questions
  Answered); status promoted to `Accepted`. Option E (per-pin
  JSON sidecar + launch-time decision tree) is the v1
  mechanism.
