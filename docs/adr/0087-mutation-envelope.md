# ADR 0087: Mutation Envelope

## Status

Accepted

## Context

The `CLAUDE.md` implementation guardrail says "start read-only
unless a task explicitly calls for persistence or link CRUD."
`docs/design.md`'s Initial Scope section says "start read-only,
but design the data model and storage layout for quick follow-up
commands that let users add or remove links manually." Both
predate a set of sanctioned mutation surfaces that Conspectus
now ships with:

- **User-authored TOML stores** — `[declared]` (ADR 0014),
  `[aliases]` (ADR 0029), `[[pins.entries]]` (ADR 0057),
  `.conspectus.toml` in project trees, and the user-scope
  equivalents in `$XDG_CONFIG_HOME`.
- **Lockstep tmux mux renames** — the rename overlay (ADR 0029
  / ADR 0030) writes an alias overlay and, when the mux is
  live, renames the mux native name so the display stays in
  sync.
- **Pin lifecycle management** — `pin launch` (ADR 0057)
  creates a new tmux session (`new-session`) or wakes a stale
  one (`send-keys` into the pane) and then attaches. Adopt
  renames an operator-selected existing mux to the pin's
  declared name.
- **Resume splicing** — the `s` accelerator (ADR 0058) writes
  a pin-binding sidecar so the resumed session gets its old
  identity when the operator's harness comes back.
- **Hook sidecar writes** — the `conspectus hook write` CLI
  (ADR 0028) writes rebuildable observations Conspectus itself
  will read back later.

The read-only-first phrasing implies that any of the above is a
special-case exception the reader has to hunt for. It also
leaves ambiguous what Conspectus is _not_ allowed to do — a
question new contributors and external readers ask early. The
answers are not new (ADR 0028 forbids terminal injection into
agents; every mutation surface enumerated above was ADR-scoped
before it landed), but they aren't stated in one place.

`H-ADR-002` records the drift and asks for a tenet ADR that
defines the envelope explicitly. This is that ADR.

## Decision

Conspectus operates under a **mutation envelope**: a bounded
set of write surfaces, each ADR-scoped, and a strict list of
mutations Conspectus never performs. Every current and future
write path fits inside the envelope; every prohibition below
is absolute.

Read-only remains the default for discovery and orchestration.
The envelope describes what write access exists at the edges
of that default, not a shift in the default.

### Sanctioned mutations

Conspectus may perform the following writes. Each is
operator-initiated (either by a direct CLI or TUI gesture, or
by a persistent operator-authored preference) and each has an
ADR entry describing the schema and semantics.

1. **User-intent TOML stores under Conspectus's ownership.**
   `[declared]`, `[aliases]`, `[[pins.entries]]`, and any
   future first-class user-intent table Conspectus adds to the
   `.conspectus.toml` project store or the user-scope store at
   `$XDG_CONFIG_HOME/conspectus/config.toml`. Writes are
   idempotent, unchanged-in unchanged-out, and preserve
   operator-authored TOML comments and formatting (`toml_edit`
   round-trip). See ADR 0014, ADR 0029, ADR 0057, ADR 0084.

2. **Rebuildable observation sidecars under
   `$XDG_STATE_HOME/conspectus/`.** Hook sidecar records
   (ADR 0028), pin binding sidecars (ADR 0058), and cached
   snapshots (`graph.bin`, ADR 0083). These are rebuildable —
   deleting them is safe, they never carry operator intent
   Conspectus authored, and they never carry payload (see
   ADR 0086 Tier 3).

3. **Mux backend lifecycle when operator-initiated.**
   - `tmux rename-session` under the rename overlay and the
     pin-adopt path (ADR 0029, ADR 0057).
   - `tmux new-session -d -c <cwd> <argv>` under `pin launch`
     (ADR 0057).
   - `tmux send-keys` into an existing pane under `pin
     launch`'s "wake stale mux" path (ADR 0057) — bounded to
     the harness's own launch argv, never to arbitrary
     operator-typed input.
   - `tmux attach-session` / `switch-client` under the `a` /
     `Enter` / `pin launch` attach paths.

   Every mux operation is triggered by a direct operator
   gesture and targets a mux the operator has selected or
   declared. Conspectus never enumerates every mux and
   proactively renames or spawns; the enumeration surface is
   the operator's mental model, not Conspectus's.

4. **Subprocess launches Conspectus owns end-to-end.** Resume
   command spawn (ADR 0058), transcript viewer external
   fallback (ADR 0052, `claude-history` for aider), pin launch
   re-exec through `conspectus pin launch <id>`. These are all
   `spawn` / `exec` calls with argv Conspectus constructs from
   ADR-scoped inputs; they never adopt operator-typed argv
   verbatim.

### Prohibitions

Conspectus **never**:

1. **Mutates harness-native state.** No writes to
   `~/.config/opencode/opencode.db`, `~/.codex/state_*.sqlite`,
   `~/.codex/logs_*.sqlite`, `~/.claude/`, `~/.aider/`, or
   any harness-owned file or database. Readers are read-only
   with `PRAGMA query_only = ON` where SQLite is involved
   (ADR 0048). Even where the harness ships a documented
   mutation API, Conspectus does not call it — the harness's
   own UI is the authoritative surface for harness state.

2. **Injects terminal input into agents (ADR 0028, absolute).**
   `tmux send-keys` into a live agent pane is off-charter, and
   the pin-launch `send-keys` exception is scoped to
   Conspectus-constructed harness launch argv, not
   operator-typed input. Bypasses to this rule are not
   negotiable — they would violate the operator's trust
   contract with their agent (the operator has to be certain
   Conspectus never types _for_ them into a live session).

3. **Persists payload content of any kind.** Sidecar and
   snapshot writes are Tier 3 (ADR 0086) — attribution and
   observation, not conversation. Payload content only
   reaches the operator through Tier 2 read surfaces
   (preview, viewer).

4. **Writes to shared or system-owned locations.** No
   `/etc/`, no `/usr/local/`, no cross-user files. Every
   Conspectus-authored write targets `$HOME` /
   `$XDG_CONFIG_HOME` / `$XDG_STATE_HOME` / a project tree the
   operator points at explicitly.

5. **Performs background mutation.** The daemon reads and
   caches; it never writes to user-intent stores, never
   renames mux sessions, never spawns harness processes. All
   sanctioned mutations flow through operator-initiated CLI
   or TUI dispatch.

6. **Mutates git state.** Conspectus reads `git` common dirs,
   `.gitconfig`, and worktree metadata; it never runs `git
   commit`, `git checkout`, `git push`, or any command that
   changes the repo state. Fork / workspace operations that
   need git mutation belong to Atelier or another dedicated
   tool.

7. **Bypasses hooks or signing.** When a write path is guarded
   by a hook (pre-commit, pre-push, filesystem watcher), the
   hook runs. There is no `--no-verify` equivalent for
   Conspectus writes; if a hook fails, the write fails.

### Envelope rules for new features

A new feature that requires a write path Conspectus doesn't
already have must:

1. Enumerate the exact operator gesture (CLI subcommand, TUI
   accelerator, or persistent preference) that triggers the
   write.
2. Describe the write's scope (which file, which table, which
   mux, which argv) and its idempotence.
3. Confirm it fits into one of the sanctioned mutation
   categories above, or record a new category via a
   supersession or extension of this ADR.
4. Confirm it does not touch any prohibition.
5. Land as an ADR that this ADR's Related ADRs section
   references, and that references this ADR from its Related
   ADRs section.

Features that would require a new prohibition removal — for
example, opening a write path against harness state, or
allowing background mutation — need this ADR to be superseded,
not just extended.

## Consequences

**For CLAUDE.md.**

The "start read-only unless a task explicitly calls for
persistence or link CRUD" bullet is replaced with a citation
of this ADR. The intent (default to read-only when in doubt)
is preserved, but the specific list of "persistence or link
CRUD" is subsumed by the sanctioned-mutations enumeration
here.

**For docs/design.md.**

The Initial Scope "start read-only, but design the data model
and storage layout for quick follow-up commands" is preserved
as a historical framing — that's how the project actually
grew — but the current envelope is what the code embodies,
so an inline pointer to this ADR is added there.

**For future ADRs.**

Every ADR that introduces a new write path must cite this ADR
and land its write inside one of the sanctioned categories.
Reviewers push back when a new ADR proposes a write that
touches a prohibition or introduces a new category without
extending this one.

**For contributor onboarding.**

A new contributor reading CLAUDE.md sees the citation, reads
this ADR once, and has the complete list of what Conspectus
does and doesn't write. The recurring "wait, we do rename tmux
sessions, isn't that terminal injection?" question that came
up repeatedly on the H-ADR-* audit gets a definitive
answer: no, it's a sanctioned mutation under category 3, and
the terminal-injection prohibition is specifically about
`send-keys` into a live agent pane (category 3's `send-keys`
exception is scoped to Conspectus-constructed harness launch
argv, not operator-typed input).

**For H-EXT provider work.**

New mux backends (H-EXT-008: zellij, ADR 0087 Related), new
forge adapters (H-EXT-013), and new orchestrator surfaces
(H-EXT-014) inherit the envelope. A zellij backend gets the
same rename / new-session / send-keys / attach surface as
tmux — no less, no more. A hypothetical GitLab forge adapter
would still not write to `.gitlab-ci.yml` or push branches,
because forge mutation is not in the envelope.

**For code review.**

Reviewers gate new write paths on the "cite this ADR, place
in a category" contract. `grep -rn "std::fs::write\|
File::create\|OpenOptions" src/` is a reasonable review
sanity check: every hit should trace back to a category above.

## Alternatives Considered

**Delete the read-only guardrail entirely.** Rejected. The
default-to-read-only stance is still correct — it makes the
first N commits of any new discovery / observation feature
zero-risk. Deleting it would license new features to grow
write paths without ADR review.

**Restate the guardrail as "operator-initiated writes only."**
Rejected as insufficient. The prohibitions above (harness
state, terminal injection, payload persistence, git state,
background mutation) are all "operator-initiated" in some
sense — the operator did type the key. What matters is where
the write lands and what invariants it preserves, and that
needs the full enumeration.

**Enumerate write paths in CLAUDE.md directly.** Rejected as
too fragile. CLAUDE.md is read on every task; every
enumeration change would ripple through cache and prompt
context. An ADR with a stable slug (`ADR 0087`) that CLAUDE.md
cites is the right level of indirection.

**Runtime enforcement (a `WriteCategory` type-level check).**
Rejected as overengineering. Write paths are already few
enough that ADR-scoped review is the right grain. Runtime
enforcement would be trivially bypassed via any `impl` of the
category enum and would push work onto every write site for
zero net gain.

**Split into per-category ADRs.** Rejected. The envelope is
useful as one document — the sanctioned categories and the
prohibitions are read together, and splitting would force
every new-feature ADR to cite five ADRs instead of one.

## Open Questions Answered

- **Is Conspectus read-only?** Not entirely. It has an
  envelope of sanctioned writes (categories 1–4 above); every
  other write is off-charter.
- **Does the daemon write?** No. Category 5 of the
  prohibitions makes background mutation off-charter. The
  daemon reads and caches only.
- **Can Conspectus rename mux sessions? Isn't that terminal
  injection?** Yes it can rename (category 3), no it isn't
  terminal injection. Terminal injection is `send-keys` of
  arbitrary content into a live agent pane; renames don't
  touch the pane's stdin. The `send-keys` exception in
  category 3 is scoped to Conspectus-constructed harness
  launch argv, not operator-typed input.
- **Does the mutation envelope license writing to
  harness-native state?** No. Prohibition 1 is absolute.
- **What about payload writes?** Prohibition 3, referencing
  ADR 0086. No write path Conspectus takes carries payload.

## Open Questions Deferred

- **Whether Atelier command modules should ever be Conspectus
  dependencies.** The current CLAUDE.md rule ("avoid making
  Conspectus depend on Atelier command modules directly") is
  adjacent to this envelope but distinct — it's a shape rule,
  not a mutation rule. Deferred to a separate story.
- **Config file write behavior when the file is under version
  control and dirty.** Should a user-intent write refuse when
  `.conspectus.toml` has uncommitted changes? Currently no;
  defer until the operator concern surfaces.
- **Batch write throttling.** Fast repeated pin CRUD (`Delete`
  → confirm → next pin → `Delete` → confirm) currently writes
  synchronously. Whether to debounce is a UX question, not an
  envelope decision. Defer.

## Related ADRs

- ADR 0014 (declared links) — user-intent TOML store, category 1.
- ADR 0028 (hook sidecar records) — Tier 3 rebuildable
  observations; carries the terminal-injection prohibition
  this ADR restates.
- ADR 0029 (lockstep session aliases) — mux rename surface,
  category 3.
- ADR 0030 (tui-input primitive) — the rename overlay's shared
  text input.
- ADR 0048 (Codex state and log readers) — read-only surface
  covered by prohibition 1.
- ADR 0052 (native transcript viewer) — read-only content
  surface; the external-launch fallback path is category 4.
- ADR 0057 (session pins) — pin launch / send-keys /
  new-session; category 1 + 3.
- ADR 0058 (resume splicing) — pin-binding sidecar; category 2.
- ADR 0083 (zero-copy snapshot format) — `graph.bin`;
  category 2.
- ADR 0086 (payload privacy tenet) — prohibition 3 references
  this for the never-persist-payload rule.
