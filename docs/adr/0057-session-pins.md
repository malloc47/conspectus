# ADR 0057: Session Pins

## Status

Accepted

## Context

Conspectus today represents only *discovered* agent sessions: an
`AgentSession` node exists when its harness has written evidence to disk.
Phase 5 declared links (ADR 0014) and the alias overlay (ADR 0029) layer
user intent onto those discovered nodes but cannot precede them. There is
no way to say "I want a Codex session for project X in a tmux named
`ingest-refactor` at cwd `/work/repo`" and have Conspectus treat that
declaration as a first-class dashboard row whether or not a live session
currently realizes it.

agent-deck and similar agent-over-tmux orchestrators (see the
`H-AGENTMUX-*` workstream in `docs/backlog.md`) provide exactly this:
a "new card" surface that declares a logical work unit (project +
harness + desired tmux name + cwd), spawns the agent into a new tmux
session, and tracks that pair as one entity across the session's
lifetime — including `/compact` / `/resume` (intra-harness lineage per
ADR 0018), external tmux renames, and harness restarts. Conspectus
needs an equivalent surface to replace agent-deck for the core
dashboard workflow without inheriting its broader orchestration scope.

The supporting machinery already exists:

- ADR 0014: declared-link TOML storage at project-local and user-level
  config, with `select_store_for_declaration` nearest-store selection.
- ADR 0029: alias overlay with `display_name` precedence
  `alias > title > id-suffix`, and lockstep mux rename via
  `TmuxRunner::rename_session` (H-RENAME-003).
- ADR 0005: unresolved endpoint evidence — declarations that point at
  not-yet-discovered sessions are already a first-class graph concept.
- ADR 0018: intra-harness `parent_session` lineage covers compaction
  and resume, so the same logical session survives an
  `agent_session_id` change.
- ADR 0027: workspace detection precedence settled the "deeper cwd
  wins" pattern that pin binding can reuse.
- P8-010 / `src/tui/actions.rs`: exec-replace into `tmux attach-session`
  is the existing pattern for handing the terminal to tmux.

What is missing is the surface that combines them: a persisted intent,
a launch primitive, a resolver rule that binds the intent to a live
session, and a TUI/CLI surface that renders the intent as a dashboard
row regardless of binding state.

## Decision

Introduce a **Session Pin**: a Conspectus-owned, user-authored
declaration of a logical agent session. A pin is

- a desired `(harness, cwd, display_name, mux)` tuple,
- persisted as TOML at the same store locations as ADR 0014 / ADR 0029,
- rendered as a first-class row in the sessions and mux row trees whether
  or not a live session realizes it,
- bound 1:1 at resolve time to whichever live agent session matches its
  declaration; the binding follows ADR 0018 lineage so `/compact` and
  `/resume` do not break the binding,
- launchable via `conspectus pin launch <id>` and the TUI's existing
  row-action keys; launch spawns the configured harness inside a fresh
  tmux session via the `TmuxRunner` mutation surface.

The binding is **not persisted**. It is recomputed every discovery pass
from the pin declaration plus live evidence, the same way ADR 0005
unresolved-endpoint evidence resolves opportunistically.

### Storage Schema

A new top-level `pins` TOML table lives alongside `declared`
(ADR 0014) and `aliases` (ADR 0029) in `.conspectus.toml` and the
user-level config.

```toml
[pins]
schema_version = 1

[[pins.entries]]
id = "ingest-refactor"
display_name = "ingest-refactor"
harness = "codex"
cwd = "/home/me/work/repo"
mux = { backend = "tmux", name = "ingest-refactor" }
launch = { argv = ["codex"] }  # optional; defaults per harness
reason = "the codex session for the ingest refactor"  # optional

# Example using a non-default tmux socket (equivalent to `tmux -L scratch`):
[[pins.entries]]
id = "scratch-codex"
display_name = "scratch-codex"
harness = "codex"
cwd = "/home/me/work/scratch"
mux = { backend = "tmux", name = "scratch-codex", socket_name = "scratch" }
```

Each entry has:

- `id`: stable user-facing identifier, unique within the store. Used
  for CLI and TUI references and in diagnostics.
- `display_name`: required, non-empty. Doubles as the bound agent
  session's alias overlay (per ADR 0029 precedence) *and* as the
  initial tmux session name at launch.
- `harness`: required harness key matching `HarnessAdapter::harness_key`
  (`codex`, `claude-code`, `opencode`, `aider`).
- `cwd`: required absolute path. Anchors store selection and binding;
  also the `tmux new-session -c` argument at launch.
- `mux`: required typed table. `backend = "tmux"` is the only v1 value
  (mirror of ADR 0029's stance on future mux backends). `name` is the
  tmux session name used at launch and as the binding anchor.
  `socket_name` is optional and tmux-specific (see below); when absent the
  pin uses the default tmux socket exactly as today's discovery does.
- `launch`: optional. `argv` overrides the per-harness default
  spawn command; `env` (deferred to a follow-up) would carry
  environment overrides. Absent `launch.argv` means "use the harness
  adapter's default argv."
- `reason`: optional explanatory text. Mirror of ADR 0014 / ADR 0029;
  reserved for future audit surfaces.

#### Tmux Socket Support

The `mux.socket_name` field is the equivalent of the `tmux -L <name>`
argument: it selects the tmux server identified by the socket file
`$TMUX_TMPDIR/<name>` (or `/tmp/tmux-$UID/<name>` when `TMUX_TMPDIR`
is unset). When absent, pins use the default socket exactly as
today's tmux discovery. `socket_name = "default"` is accepted and
equivalent to omitting the field. Absolute socket paths (`tmux -S
<path>`) are deliberately not supported in v1 — operators who need
arbitrary socket paths can either symlink them into `$TMUX_TMPDIR`
or wait for a follow-up that adds a separate `socket_path` field.

Socket support has two facets:

- **Launch (in v1)**: `TmuxRunner`'s mutation methods take a
  `socket_name: Option<&str>` parameter and pass `-L <name>` when it is
  set. `new_session`, `attach_session`, `send_keys`, and
  `rename_session` all thread the socket through. A pin with a
  non-default socket can therefore be launched and attached
  immediately, regardless of whether discovery sees its socket yet.
- **Discovery (deferred to a follow-up story)**: today's tmux runner
  shells out to `tmux list-sessions` with no `-L`, so it sees only
  the default socket. The follow-up extends discovery to enumerate a
  socket set built from `{default} ∪ {pin.mux.socket_name | active pin}` —
  pins implicitly declare which sockets matter, so the discovery
  surface stays bounded and explicit. Until that lands, pins on
  non-default sockets render as `PinUnbound` (the mux is invisible to
  discovery) but launch and attach work correctly; the dashboard
  catches up automatically when the discovery story merges.

Mux identity encoding gains a socket dimension. To preserve backward
compatibility with every existing `MuxSessionId` in declared links,
aliases, snapshots, and tests, the encoding is:

- default socket: `tmux:<name>` (unchanged — byte-for-byte
  compatibility with today's encoding).
- non-default socket: `tmux:<socket>:<name>`.

This keeps every existing fixture and declared link valid without
migration and makes "default" the canonical zero-cost case.

`schema_version = 1` applies only to the `pins` table. The same
forward-compatibility rules as ADR 0014 / ADR 0029 hold: unknown
fields are ignored on read and preserved on write when practical,
malformed entries produce diagnostics and are skipped, and an unknown
`schema_version` suppresses pin loading from that file rather than
failing the whole config load.

### Store Selection and Provenance

- pins whose `cwd` is inside a discovered repo, checkout, or
  workspace write to the nearest project-local `.conspectus.toml` via
  the existing `select_store_for_declaration` helper.
- pins whose `cwd` lies outside any discovered repo/workspace write
  to the user-level config.
- `LocalPin` and `GlobalPin` are new provenance variants on the
  loader side; local beats global when both name the same `id`.

A pin with no `cwd`, or with a `cwd` that does not exist on the
filesystem at write time, is rejected. Unlike declared links and
aliases — both of which can reasonably describe orphan or yet-to-exist
endpoints — a pin without a cwd has nothing to launch and no anchor
for store selection.

### Resolver Binding Semantics

A naive `(harness, cwd)` binding rule is unstable at realistic
densities. Empirically a single repo cwd can host dozens of historical
harness session files: in the Conspectus dev tree, `cwd =
/home/user/src/conspectus` resolves to 47+ live `AgentSession`
nodes (31 claude-code with empty titles, ~16 codex, ~14 opencode)
across three concurrent `tmux:agentdeck_conspectus_*` muxes. A binding
rule that picks "the latest harness session at this cwd" would
oscillate every time a stale transcript file's mtime changed and would
attribute arbitrary historical sessions to a pin the operator only
just created. The cwd is necessary but not nearly sufficient.

The binding therefore anchors on the **mux**, not the cwd:

1. **Pin row evidence** (always): a new `Pin` candidate kind
   carrying `(id, harness, cwd, display_name, mux.backend, mux.name,
   store)`. Row builders render this as a dashboard row regardless of
   whether the pin is currently bound to a live session.

2. **Mux lookup**: find a live `MuxSession` whose backend matches
   `pin.mux.backend` and whose `native_id` matches the encoded
   `(socket, name)` pair — `tmux:<name>` for the default socket and
   `tmux:<socket>:<name>` for non-default sockets. The match is exact
   (no prefix or fuzzy logic) so the binding is stable across
   discovery passes and unaffected by muxes with the same name on
   other sockets or other muxes that happen to share the cwd. If no
   such mux exists, the pin is **unbound** — render the dashboard
   row, do not synthesize any `LinkedToMux` candidate, and let
   `launch` create the mux. Pins on non-default sockets are
   structurally `PinUnbound` until the discovery-side socket
   enumeration follow-up lands; launch and attach still work because
   they thread the socket through `TmuxRunner` directly.

3. **Harness attribution within the bound mux**: walk the existing
   mux-to-agent-session attribution pipeline (ADR 0006 candidate set,
   ADR 0046 / ADR 0047 process-tree linker, ADR 0028 hook sidecars,
   ADR 0048 Codex log linker) restricted to the bound mux. Filter the
   resulting agent-session candidates by `harness_key == pin.harness`.

   - **Exactly one candidate**: bind. Synthesize an in-memory alias
     overlay entry `(agent_session_id → display_name)` (no TOML write
     — the alias pipeline from ADR 0029 sees a transparent overlay)
     and a synthetic `LinkedToMux` candidate carrying `PinDerived`
     provenance between the bound `AgentSession` and the bound
     `MuxSession`. Intra-harness lineage (ADR 0018) continues to
     follow `/compact` / `/resume` because the underlying attribution
     pipeline already does — the pin tracks the mux, the mux's pane
     tracks the live process, and the live process is the
     post-compaction/resume successor.
   - **Multiple candidates** (e.g. two harness PIDs in the same
     mux's panes, or stale hook evidence competing with live process
     evidence): pick the candidate with the freshest current-session
     evidence per the existing resolver ranking (ADR 0048 demotion of
     stale launch-argv evidence applies). Emit a
     `Diagnostic::PinAmbiguous` carrying every competing
     `agent_session_id` so the operator can disambiguate with
     `conspectus pin bind <pin-id> --to <session-id>`.
   - **Zero candidates** (mux exists but no live harness session
     attributed to it — common after `Ctrl-d` exits the harness but
     leaves the shell pane alive): emit a `Diagnostic::PinStaleMux`.
     The dashboard row renders with a "harness exited" status so the
     operator knows to relaunch.

4. **Cwd is a sanity check, not the discriminator**. If the bound
   agent session's first-observed cwd diverges from `pin.cwd` (e.g.
   the operator `cd`'d the pane elsewhere mid-session and a fresh
   harness invocation took the new cwd), the binding still holds but
   the resolver emits a `Diagnostic::PinDrift` with both paths. Cwd
   continues to serve two real roles: store selection at write time
   and the `-c` argument at launch time.

5. **Operator override via declared link**: a `conspectus pin bind
   <pin-id> --to <session-id>` writes a `LocalDeclared`
   `linked_to_mux` link tagged with the pin id (per ADR 0014)
   between the chosen `AgentSession` and `pin.mux.name`. The
   resolver treats that declared link as the authoritative
   `LinkedToMux` for the pin, suppressing the attribution-pipeline
   walk. This makes the v1 escape hatch when ambiguity cannot be
   auto-resolved use the existing declared-link machinery rather
   than introducing a new persisted binding type.

6. **Pin-to-pin conflict**: two pins with the same `mux.backend` +
   `mux.name` in the same effective config set (after local-over-
   global merging) are a write-time validation error, rejected by
   `conspectus pin create` and `pin rename`. The first one in the
   file wins at load time with a `Diagnostic::PinDuplicate`; this
   keeps the resolver linear without inventing tiebreakers.

The pin-to-mux binding being non-persistent is deliberate. The pin
declaration (TOML) and the declared-link override (also TOML) are the
durable sources of truth; the mux lookup and harness attribution are
projections over current discovery evidence. Persisting `(pin_id →
agent_session_id)` would introduce a stale-state class with no
authoritative source. Persisting `(pin_id → mux_native_id)` is
already what `pin.mux.name` is — the schema stores it directly.

### Launch Semantics

Launch introduces the first process-spawning code path in Conspectus.
It rides on the existing `TmuxRunner` trait, which gains three new
methods (defaulted to `Unsupported` so existing implementations need
no change). Each method takes an `Option<&str>` socket so pins with
non-default sockets thread `-L <name>` through every tmux invocation:

- `new_session(socket_name: Option<&str>, name: &str, cwd: &Path, argv: &[OsString]) -> Result<TmuxNewSessionOutcome>`
- `attach_session(socket_name: Option<&str>, name: &str) -> Result<TmuxAttachOutcome>`
  — wraps the exec-replace pattern from P8-010 / `src/tui/actions.rs`
  so callers can choose between `tmux [-L <socket>] attach-session -t <name>`
  (outside tmux) and `tmux [-L <socket>] switch-client -t <name>`
  (when `$TMUX` is set, to avoid nested client errors).
- `send_keys(socket_name: Option<&str>, target: &str, literal: &str, press_enter: bool) -> Result<TmuxSendKeysOutcome>`
  — used by the `PinStaleMux` relaunch path to inject the harness
  command into an existing pane without recreating the mux.

The existing `rename_session(target, new_name)` and `capture_pane(target)`
methods grow the same `Option<&str>` socket_name parameter so lockstep
renames and live previews work for non-default-socket pins. The
default-trait implementations stay as `Unsupported`; `SystemTmux`
adds the `-L <name>` flag only when `socket_name` is `Some(s)` and
`s != "default"`, preserving today's default-socket invocation
shape byte-for-byte.

`conspectus pin launch <id>`:

1. Load and validate the pin from the local then user stores.
2. Run discovery and the resolver. If the pin is already bound (a
   live mux with `pin.mux.name` exists and a `pin.harness` session is
   attributed to it), surface "already running" and fall through to
   the attach path; do not re-spawn.
3. If a tmux session named `pin.mux.name` exists but the pin resolves
   as `PinStaleMux` (mux present, no live harness): the operator's
   intent is "relaunch the harness inside this existing mux."
   Send the harness command into the existing mux via
   `tmux send-keys -t <name> "<argv...>" Enter` (new `TmuxRunner`
   method, see below) and then attach. This preserves the operator's
   tmux window/pane layout.
4. If a tmux session named `pin.mux.name` exists but cannot be tied
   to this pin (e.g. an unrelated tmux with the same name predates
   the pin and has a different cwd / unrelated harness), fail with
   `PinLaunchError::NameTaken`. The operator resolves it by renaming
   the conflicting tmux, picking a different `pin.mux.name`, or
   adopting the existing tmux into this pin via
   `conspectus pin adopt`.
5. Otherwise call `TmuxRunner::new_session(pin.mux.name, pin.cwd,
   argv)`. argv defaults to the per-harness launch command (see
   below) unless `pin.launch.argv` overrides it.
6. Hand the terminal off via
   `TmuxRunner::attach_session(pin.mux.name)`.

A `conspectus pin adopt <pin-id> <mux-name> [--harness <key>] [--display <name>]`
companion command lets operators convert an existing tmux session
(e.g. one already named by agent-deck or workmux) into a pin without
creating a new mux. It writes a pin whose `mux.name` matches the
existing tmux and infers `harness` from the resolver's current
attribution unless `--harness` overrides it. This is the v1 migration
path for replacing agent-deck without dropping live sessions.

External tmux renames (`tmux rename-session foo bar`) made outside
Conspectus do not propagate to the pin: tmux exposes no stable id
beyond `native_id`. The pin becomes unbound on the next discovery
pass and surfaces as `PinUnbound`; the operator's recovery is
`conspectus pin rebind <pin-id> --mux <new-name>` (a thin wrapper
around editing `pin.mux.name` in the TOML store). The TUI exposes
the same action as a one-key rebind on the unbound pin row.

The CLI binary is Unix-only when invoking the exec-replace path,
matching the existing P8-010 constraint. On Windows or in
non-attaching contexts (CI, scripts, `--no-attach`) launch returns
after `new_session` succeeds and prints the attach command for the
operator to run.

Per-harness default argv lives on `HarnessAdapter` via a new trait
method:

```rust
fn launch_argv(&self) -> Vec<OsString> { /* per harness */ }
```

with v1 defaults:

- `codex`: `["codex"]`
- `claude-code`: `["claude"]`
- `opencode`: `["opencode"]`
- `aider`: `["aider"]` (subject to the H-AGENTMUX-001 audit; aider's
  per-repo state shape may need additional flags)

Default argv intentionally carries no model, prompt, or feature flags.
Operators who want bespoke launch commands either set
`pin.launch.argv` or wrap the harness in a shell alias.

### Rename Semantics

`conspectus pin rename <id> [<new-id>] [--display <name>]`:

- changes the `id` (storage rename within the same store; the file is
  re-sorted deterministically), and/or
- changes `display_name`. When `display_name` changes and the pin
  is currently bound, lockstep applies per ADR 0029: the resolver's
  bound mux is renamed via `TmuxRunner::rename_session`, and the
  pin's `mux.name` field is updated in TOML so subsequent
  discovery resolves the new native id.

TUI: the `R` keybinding on a pin row edits `display_name` and
runs the same lockstep flow.

Because the pin's `display_name` *is* the bound agent session's
effective alias (via the synthesized in-memory overlay), the rename
propagates to every projection automatically. No separate
`[[aliases.entries]]` row is written for pin-bound sessions — the
pin owns the name. Sessions without an owning pin continue to
use the explicit alias overlay from ADR 0029.

### TUI Surface

Sessions row tree renders one row per pin:

- **Unbound pin row** (no live session matches yet): dim glyph,
  label `<display_name>`, secondary text `(pin · <harness> · ~/...)`.
  `Enter` invokes launch. `R` renames. `Delete` removes the pin
  after confirmation. Status bar surfaces "pin not bound — Enter
  to launch."
- **Bound pin row**: identical to today's agent-session row but
  carries a small `★` glyph (reserved here; final glyph picked during
  implementation against `Theme`) indicating pin provenance.
  `Enter` / `a` attach via the existing P8-010 flow. `R` renames with
  lockstep.

Mux row tree renders the pin-derived mux as an ordinary mux row
once bound. Unbound pins do not synthesize a mux row — the mux
session does not exist yet.

The "Controls overlay" surface from ADR 0031 gains a `Pins` action
group exposing create / rename / remove / launch from the menu, so the
capability stays discoverable per the
`feedback_tui_discoverability` memory.

### CLI Surface

```
conspectus pin create <id> --harness <key> --cwd <path>
  [--display <name>] [--mux-name <name>] [--mux-socket <name>]
  [--store project|user]

conspectus pin list [--bound | --unbound | --stale]
conspectus pin show <id>
conspectus pin rename <id> [<new-id>] [--display <name>]
conspectus pin rm <id>
conspectus pin launch <id> [--no-attach]
conspectus pin attach <id>           # alias for launch when already bound
conspectus pin bind <id> --to <session-id>
                                     # operator override for PinAmbiguous;
                                     # writes a LocalDeclared linked_to_mux
                                     # link per ADR 0014
conspectus pin rebind <id> --mux <new-name> [--mux-socket <name>]
                                     # update pin.mux.name (and socket_name)
                                     # after an external tmux rename
                                     # or socket move
conspectus pin adopt <id> <mux-name> [--harness <key>] [--display <name>]
                                     [--mux-socket <name>]
                                     # convert an existing tmux session
                                     # (e.g. agent-deck-named) into a pin
```

- `--display` defaults to `<id>` when omitted.
- `--mux-name` defaults to `<display>` when omitted.
- `--mux-socket` defaults to absent (default tmux socket).
- `--store` defaults to `auto` (nearest store) per ADR 0014.
- `launch` and `attach` collapse semantically: `launch` will attach if
  the pin is already bound, and `attach` will spawn if the pin
  is unbound and a session can be created idempotently. Keeping both
  verbs gives scripts explicit feedback when the state diverges from
  expectations.
- `bind`, `rebind`, and `adopt` are the operator escape hatches when
  the resolver's auto-binding cannot or should not be trusted. They
  are deliberately structured commands rather than free-form TOML
  editing so the changes are auditable and snapshot-testable.

### Read-Only Invariants

`graph`, `node show`, `table`, and `tui` navigation must never create,
mtime-touch, or content-modify pin-bearing config files. Only the
explicit mutation commands above (and the TUI write paths they back)
may mutate `[pins]`. This mirrors the ADR 0014 / ADR 0029 contract
and the existing test patterns from `P5-004`.

## Consequences

- Conspectus gains the first user-authored entity that *precedes*
  discovery. The unresolved-endpoint evidence machinery from ADR 0005
  carries the rendering; no synthetic `AgentSession` nodes are
  invented.
- The first process-spawning surface lands. P8-010's
  exec-replace-into-tmux is the precedent; this extends it to
  `tmux new-session` and `tmux send-keys`. The `TmuxRunner` trait
  grows three new defaulted methods so existing implementations and
  tests do not break.
- `HarnessAdapter` grows a `launch_argv` method. The v1 defaults are
  trivial; future harness additions must answer this question at
  registration time.
- Renaming a pin becomes the canonical "rename a project's session
  in lockstep" operation. ADR 0029's alias overlay continues to serve
  sessions without an owning pin — the two surfaces compose without
  contradiction.
- Resolver gains one extra pass per resolve to bind pins. Cost is
  linear in pin count (operator-bounded, tens not thousands) and
  reuses the existing mux-to-agent-session attribution pipeline
  rather than introducing a parallel matcher.
- Pin binding stability is anchored on the mux native name plus the
  mux-to-agent-session attribution pipeline (ADR 0006 / ADR 0028 /
  ADR 0046 / ADR 0047 / ADR 0048). At realistic densities (the
  Conspectus dev tree carries 47+ historical sessions and 3 concurrent
  muxes at one cwd) this is the only attribution that converges; cwd
  is preserved as a launch parameter and drift sanity check.
- Pins gain four resolver-emitted diagnostics: `PinUnbound`,
  `PinStaleMux`, `PinAmbiguous`, and `PinDrift`. Each maps to a
  specific TUI affordance (relaunch, rebind, manual-bind, status
  hint) so operators have a clear next action when auto-binding
  cannot resolve.
- The operator-override path for `PinAmbiguous` reuses the ADR 0014
  declared-link surface (a `LocalDeclared linked_to_mux` tagged with
  the pin id) rather than inventing a new persisted binding type.
  Pins compose with the existing precedence pipeline instead of
  shadowing it.
- The v1 `launch.argv` field is intentionally the single hook surface
  in v1, but the schema is shaped so adding richer lifecycle hooks
  later is additive rather than breaking. The cross-tool survey
  (tmuxinator `on_project_*`, smug `before_start` / `attach_hook`,
  sesh `startup_command`) confirms hooks are a widespread pattern;
  v1 does not preclude growing a `launch.before` / `launch.after`
  (or `[[pins.hooks]]`) surface in a follow-up ADR when a concrete
  pattern emerges that `launch.argv` plus prefix tooling
  (`nix develop --command`, `direnv exec`, `op run`) cannot express
  cleanly.
- Conspectus now has three sibling TOML tables for user intent:
  `[declared]` (relationships, ADR 0014), `[aliases]` (per-node
  attributes, ADR 0029), and `[pins]` (declared session lifecycle).
  Future per-node-attribute concepts should follow this sibling pattern
  rather than overloading any of the three.
- An unbound pin is conceptually distinct from an unresolved
  declared link or an alias for a missing node. The new evidence kind
  keeps row builders, the resolver, and the TUI from conflating
  "intent to create a session" with "evidence of a relationship that
  could not be resolved."
- The H-AGENTMUX adapter workstream (especially `H-AGENTMUX-002`
  agent-deck multi-repo workspace detection) remains useful and
  complementary: pins declare the *next* logical session, while
  the agent-mux adapters extract *existing* state from other tools.
  Adopting pins does not block, replace, or require those
  adapters.

## Prior Art

Several open-source tools occupy adjacent ground: declarative tmux
session managers that persist per-session configuration and spawn
sessions from it. None solves the problem this ADR solves (pins are
about *binding a declared logical session to live discovery
evidence*, not about workspace layout), but the design conventions
they have converged on are worth checking before we lock the schema.
Evaluations below are based on each project's current README plus
spot inspection of representative config examples.

### tmuxinator (Ruby, `tmuxinator/tmuxinator`)

The reference implementation in this space. YAML configs in
`~/.tmuxinator/<name>.yml` (or `.tmuxinator.yml` in cwd) declare a
session as `{name, root, windows[{commands, panes, layout}],
pre_window, hooks}`. Supports `socket_name` (= `tmux -L`),
`on_project_start` / `on_project_exit` hooks, ERB templating, and a
local-project variant that lives next to a repo. Lifecycle is
fire-and-forget: `tmuxinator start <name>` creates a fresh session
and attaches; there is no persistent linkage between the config and
the running session beyond the shared session name.

**Fit vs pins.** Strong overlap on persistent declaration (TOML/YAML
sibling), per-project file (`.tmuxinator.yml` ~ `.conspectus.toml`),
and `socket_name` semantics — Conspectus's `mux.socket_name` field name
mirrors tmuxinator's. The mismatch is granularity: tmuxinator
declares the *entire pane tree* (windows, panes, layouts,
per-pane commands) while a pin declares only the session-level
launch. Tmuxinator has no notion of "bind to whichever harness is
currently running here" — it always creates fresh. Adopting
tmuxinator's window/pane shape into pins would invert the
single-pane-per-pin model that makes pin binding tractable.

### tmuxp (Python, `tmux-python/tmuxp`)

Spiritual port of tmuxinator with YAML/JSON config and explicit
import shims for tmuxinator and teamocil configs. Adds a notable
operation Conspectus's design echoes: **`tmuxp freeze`** captures the
current tmux session's layout into a config file — analogous to
`conspectus pin adopt` for an existing tmux. Does not document
explicit socket support. Same lifecycle limitation as tmuxinator:
workspace orchestration on launch, no live-state binding.

**Fit vs pins.** `freeze` validates the adopt-an-existing-session
direction. Beyond that, tmuxp shares tmuxinator's granularity
mismatch.

### smug (Go, `ivaaaan/smug`)

Minimal tmuxinator-alike in Go. YAML configs at
`~/.config/smug/<name>.yml` or `.smug.yml` in cwd. Session-level
`{name, root, env, windows[{commands, panes}], hooks}`. Hooks are
`before_start` / `stop` / `attach_hook` / `detach_hook`. **No
documented socket support.** Lifecycle is the same start/stop
imperative shape as tmuxinator.

**Fit vs pins.** Closest in spirit to a low-ceremony pin design but
adds nothing beyond tmuxinator that would change the pin schema.
The absence of socket support is a useful data point — even in this
space, `-L` is a less-emphasized feature, so Conspectus is not
inheriting an obvious community convention by adding it but is also
not contradicting one by encoding it differently.

### sesh (Go, `joshmedeski/sesh`)

The closest design neighbor to pins by philosophy. **TOML
configuration** at `$XDG_CONFIG_HOME/sesh/sesh.toml`. Per-session
entries declare `{name, path, startup_command, preview_command,
windows[]}` and the tool's headline behavior is **idempotent
attach-or-create**: invoking sesh on a session name attaches if the
tmux session is live and spawns it from config otherwise. Includes
zoxide/fzf integration for fuzzy switching and a `[[wildcard]]`
table that applies settings to glob-matched paths (e.g.
`~/projects/*`). Does not document custom socket support.

**Fit vs pins.** Independent convergence on three pin design
choices: TOML over YAML, idempotent attach-or-create, declarative
"desired state" framing rather than imperative "start/stop." Sesh's
`startup_command` ≈ pin's `launch.argv`. Differences: sesh is a
*session switcher* with a fuzzy picker, not a graph-aware dashboard
— it has no concept of harness identity, no alias overlay, no
resolver, no lineage following. The wildcard table is an interesting
deferred direction: a pin-via-glob would let operators declare
"every checkout under `~/work/` should pin its codex sessions"
without hand-listing each one.

### Adjacent but out of category

- **teamocil** (Ruby, legacy). Ancestor of tmuxinator; effectively
  superseded. Tmuxp still imports its configs. No active development.
- **tmux-resurrect / tmux-continuum** (`tmux-plugins/`). Different
  category: these *capture* live tmux state to disk and restore it
  after restart. They are imperative state snapshotters, not
  declarative session managers. The "restore exact session
  topology" goal is orthogonal to pins, but the conceptual line
  between "declared intent" (pins, tmuxinator) and "snapshotted
  state" (resurrect) is worth keeping crisp: pins are intent.

### Bottom Line

The four declarative managers above are workspace orchestrators with
no live-state binding — they spawn but do not track. None of them
addresses the graph-attribution problem pins solve (which of 31 live
codex sessions in this cwd belongs to which logical work unit). None
can be a backend for pins without losing the alias, lineage, and
resolver composition that motivates the rest of the design.

The pin schema is, however, **independently convergent with sesh's
TOML + idempotent attach-or-create + declarative-not-imperative
shape**, with `mux.socket_name` matching tmuxinator's `socket_name` field
name. That convergence is reassuring rather than concerning: the
ergonomic decisions pins make match what the rest of the ecosystem
has settled on. Three specific cross-tool patterns worth carrying
forward as deferred follow-ups:

- **Lifecycle hooks beyond `launch.argv`** (every tool above has
  them). Already a deferred question; the prior-art density
  reinforces it.
- **Adopt-from-existing-tool config import** (tmuxp `freeze`,
  tmuxinator local-project files). One-shot importers from
  tmuxinator / tmuxp / smug into pins could lower the cost of
  migration for operators with existing setups, granularity
  mismatch notwithstanding (a multi-pane tmuxinator project would
  import as one pin per declared session, dropping window/pane
  detail).
- **Glob/wildcard pin patterns** (sesh wildcard table). A
  `[[pins.patterns]]` future surface could synthesize ephemeral
  pins for paths matching a glob — useful when operators want
  "every checkout under `~/work/` runs a codex pin" without
  hand-listing.

These are added to Open Questions Deferred rather than v1 scope.

## Alternatives Considered

- **Fold pins into `[[declared.links]]` with a synthetic
  `realizes` relation to an unresolved `AgentSession` endpoint.**
  Rejected. Declared links describe relationships between two
  endpoints; a pin describes a desired session-to-be. Forcing it
  into `RelationKind` pollutes the relation vocabulary, forces the
  resolver to special-case "unresolved link with launch intent" — a
  category that does not exist today — and reuses a row shape that
  cannot carry `harness`, `cwd`, or `launch.argv` without contortion.

- **Fold pins into `[[aliases.entries]]` with an "unrealized"
  flag.** Rejected. Aliases are per-node attributes for *existing*
  nodes. An alias for a session that doesn't exist has no node to
  attach to, no `harness`, no `cwd`, and no launch surface.

- **Bind pins by `(harness, cwd)` with deepest-cwd-wins.** Initial
  proposal in this ADR; rejected after pressure-testing against the
  live Conspectus dev tree. A single repo cwd can host dozens of
  historical harness session files (47+ live `AgentSession` nodes
  share `/home/user/src/conspectus` across 3 concurrent muxes);
  any cwd-anchored rule would either pick essentially at random or
  flip every time a stale transcript's mtime changed. The mux native
  name plus the existing mux-to-agent-session attribution pipeline
  is the only anchor that converges at realistic densities. Cwd is
  retained as a launch parameter and a drift sanity check, not as
  the discriminator.

- **1:N template model** — a pin is a reusable template that
  spawns multiple named instances, each with its own binding.
  Rejected per operator question; the 1:1 lineage-following model
  matches the agent-deck card mental model, makes rename meaningful
  (rename what — the template or the instance?), and avoids inventing
  a parallel "instance" persistence type.

- **Persist `(pin_id → agent_session_id)` bindings to disk** so
  the resolver does not re-derive them every pass. Rejected. The
  binding is fully derivable from live evidence; persisting it
  creates a stale-state class with no authoritative source and
  duplicates work that ADR 0005 / ADR 0018 already perform.

- **Make pins launch-only — no continuous binding, no rendering
  while unbound.** Rejected. The operator-visible value is the
  dashboard row that persists across launches and survives
  `/compact` / `/resume`. A launch-only "pin" would be a shell
  alias.

- **Store pins in the SQLite hook-sidecar database** (ADR 0028).
  Rejected. Pins are durable user-authored intent, not
  rebuildable observations. They belong in reviewable TOML near the
  project, mirroring ADR 0014 / ADR 0029. Sharing physical backends
  later may be reasonable but the semantic split must be preserved.

- **Add a first-class `Pin` node kind to `GraphSnapshot`.**
  Considered. Pins behave row-like (one row per pin, bound
  or unbound), but the canonical graph stays free of UI concerns: an
  unbound pin has no real-world referent, only intent. Modeling
  it as evidence (a `Pin` candidate kind plus resolver-synthesized
  overlays) keeps the canonical graph clean, reuses ADR 0005
  machinery, and avoids snapshot-fixture churn. Promotion to a
  first-class node kind is left to a follow-up ADR if multiple
  consumers demand it.

- **Per-harness `launch_command(pin: &Pin)` instead of a
  static argv default.** Considered and partially adopted. v1 keeps
  `launch_argv()` static on `HarnessAdapter`; per-pin overrides
  flow through `pin.launch.argv`. A dynamic `launch_command`
  could fold model selection or prompt injection into the adapter,
  but that surface is not justified before operator demand exists.

- **Let atelier own the persistent-session concept via
  `atelier.toml`.** Rejected. Atelier remains a workspace
  materializer; the migration plan in `docs/design.md` explicitly
  avoids making Conspectus depend on atelier command modules.
  Pins must work for non-atelier workspaces and standalone repos.

- **Spawn the harness directly with `Command::spawn` instead of going
  through tmux.** Rejected. The harness-without-mux path loses every
  attach/detach/inspect affordance Conspectus already provides for
  mux-rooted sessions and conflicts with the agent-deck mental model
  the surface is replacing.

- **Support absolute socket paths (`tmux -S <path>`) in `mux.socket_name`
  in v1.** Rejected. The `-L <name>` form covers the standard use
  case (separating agent-deck-style scratch servers from a user's
  default tmux server) and stays inside `$TMUX_TMPDIR`, which keeps
  identity stable across machines and respects the operator's
  `TMUX_TMPDIR` override transparently. An absolute path field would
  fork the identity encoding (`tmux:<socket>:<name>` vs
  `tmux:@<path>:<name>` or similar) and complicate discovery
  enumeration without an established demand. Operators who need an
  absolute path can symlink it into `$TMUX_TMPDIR` for v1; a future
  ADR can add `socket_path` additively if patterns emerge.

- **Auto-enumerate every socket file under `$TMUX_TMPDIR` during
  discovery.** Considered for the deferred discovery-side follow-up
  and tentatively rejected in favor of "scan the union of `{default}`
  and `{pin.mux.socket_name | active pin}`". Auto-scan would expose
  unrelated tmux servers the operator did not declare to Conspectus
  (e.g. another tool's scratch socket) and surface them as orphan
  muxes, violating the "discovery should not require users to
  scatter override files everywhere" rule by inverting it into
  "discovery should not surface things the user hasn't opted into."
  The opt-in via pins keeps the enumeration bounded and explicit.
  Final decision belongs to the discovery follow-up story.

## Open Questions Answered

- Pins live in a sibling `[[pins.entries]]` TOML table; they
  do not extend `[declared]` or `[aliases]`.
- The pin ↔ live-session binding is 1:1 and computed at resolve time
  rather than persisted. The mux native name (`pin.mux.name`) is the
  primary anchor; harness attribution within the bound mux flows
  through the existing pipeline (ADR 0006 / ADR 0028 / ADR 0046 /
  ADR 0047 / ADR 0048) and intra-harness lineage (ADR 0018) carries
  `/compact` / `/resume` transparently. Cwd is a launch parameter
  and drift sanity check, not the discriminator.
- `display_name` doubles as the bound agent session's alias overlay
  and as the initial tmux session name; renames apply the ADR 0029
  lockstep contract.
- Launch spawns a tmux session via `TmuxRunner::new_session` and
  hands the terminal off via `TmuxRunner::attach_session`
  (exec-replace, mirror of P8-010). Inside an existing tmux client,
  `attach_session` translates to `tmux switch-client`. Stale-mux
  relaunch uses `TmuxRunner::send_keys` to inject the harness command
  into the existing pane without recreating the mux.
- Per-harness default argv lives on `HarnessAdapter::launch_argv`.
- Resolver emits four pin-specific diagnostics: `PinUnbound`,
  `PinStaleMux`, `PinAmbiguous`, `PinDrift`. Duplicate pins (same
  `mux.backend` + `mux.name`) are write-time validation errors with
  a load-time `PinDuplicate` diagnostic as a safety net.
- `PinAmbiguous` is resolved by `conspectus pin bind <id> --to
  <session-id>`, which writes a `LocalDeclared linked_to_mux` link
  (ADR 0014) — reusing the existing declared-link precedence pipeline
  rather than inventing a new persisted binding type.
- External tmux renames are recovered via `conspectus pin rebind <id>
  --mux <new-name>`; existing tmuxes (e.g. agent-deck-named) are
  migrated into pins via `conspectus pin adopt`.
- `mux.socket_name` is optional and tmux-specific (equivalent to
  `tmux -L <name>`). When absent, pins use the default socket
  exactly as today's discovery. Non-default sockets launch and
  attach correctly in v1; discovery-side enumeration of non-default
  sockets is a deferred follow-up. Default-socket muxes keep their
  `tmux:<name>` identity byte-for-byte; non-default sockets encode
  as `tmux:<socket>:<name>`.
- Read-only commands never mutate pin-bearing config files.
- Pin declarations and the H-AGENTMUX adapter workstream are
  complementary: declarations name future sessions, adapters extract
  evidence from existing tools.

## Open Questions Deferred

- **Lifecycle hooks beyond `launch.argv`.** Every comparable tmux
  session manager (tmuxinator, tmuxp, smug, sesh) exposes pre/post
  hooks (`before_start`, `on_project_start`, `startup_command`,
  `attach_hook`). v1 keeps `launch.argv` as the single
  hook point and treats prefix tooling (`nix develop --command`,
  `direnv exec`, `op run`) as the v1 escape hatch. Promote to a
  richer hook schema when concrete patterns emerge that `launch.argv`
  cannot express cleanly.
- **Importers from tmuxinator / tmuxp / smug configs.** Operators
  with existing setups in these tools may want one-shot conversion
  into pins. The granularity mismatch (multi-window/multi-pane
  workspace configs collapsing into single-pane pins) makes this
  lossy, so v1 omits importers; document the conversion rule (one
  pin per declared session, dropping per-window/per-pane detail)
  whenever an importer story is taken up.
- **Glob / wildcard pin patterns** modeled on sesh's `[[wildcard]]`
  table. A `[[pins.patterns]]` surface could synthesize ephemeral
  pins for paths matching a glob (`~/work/*` → one codex pin per
  matched checkout). Deferred; the single-pin-per-entry v1 model
  stays explicit until operator demand justifies a synthesis layer.
- **Tmux non-default socket discovery enumeration.** v1 lands the
  schema, launch, attach, rename, and send-keys for non-default
  sockets, but tmux discovery still scans only the default socket.
  A follow-up story extends the discovery runner to enumerate
  `{default} ∪ {pin.mux.socket_name | active pin}`. Until then, pins on
  non-default sockets render as `PinUnbound` even when their tmux
  session is live — launch/attach still work, the dashboard catches
  up automatically once the story merges. The follow-up also
  decides whether to expose a `[tmux] sockets = [...]` config knob
  for sockets the operator wants scanned without an owning pin.
- **Absolute tmux socket paths (`tmux -S <path>`).** Deferred to a
  separate ADR if patterns emerge; v1 supports only `-L <name>`
  under `$TMUX_TMPDIR`.
- **Windows / non-tmux launch.** v1 is tmux + Unix-only. A future ADR
  generalizes via a `Launcher` trait keyed on `mux.backend`.
- **Multi-harness pins** (one declaration spawning two harnesses
  sharing a workspace). Out of scope; two pins with the same cwd and
  distinct `mux.name` values are the v1 workaround. Promote to a
  model concept only when evidence demands it.
- **Pre-launch hooks** (run a shell command before the harness spawns
  — useful for `nix develop` shells or environment setup). The v1
  escape hatch is `launch.argv`; a richer hook surface waits for
  patterns to emerge.
- **`launch.env` and `launch.env_file`.** Reserved in the schema but
  intentionally undocumented in v1; populate when a concrete operator
  use case appears.
- **Atelier fork integration**: should creating an atelier fork
  auto-create a pin for the new context? Deferred to the
  atelier-delegation phase that touches fork creation.
- **Auto-suggest pins from observed sessions.** A `conspectus
  pin suggest` command that proposes pin declarations from
  recently-active mux-bound sessions could lower the cost of
  retrofitting pins onto an existing workflow. Deferred.
