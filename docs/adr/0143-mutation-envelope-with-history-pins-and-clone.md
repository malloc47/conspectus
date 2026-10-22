# ADR 0143: Mutation Envelope With History, Pin Slots, And Clone

## Status

Accepted. Supersedes ADR 0087 and its 2026-10-12 amendment. Keeps every
category and prohibition ADR 0087 numbered, under the same numbers, so
"ADR 0087 category 3" in an older ADR reads the same against this one.

## Context

ADR 0087 bounds what Conspectus may write: five sanctioned categories
and seven prohibitions. It says that removing a prohibition takes a
supersession, not an amendment. The history, rebuild, and pin slots
plan (`docs/plans/history-rebuild-and-pin-slots.md`, ADRs 0140, 0141,
and 0142) needs four changes to the envelope:

- **A history store the daemon writes.** History (ADR 0140) records
  every rebuildable entity Conspectus observes under
  `$XDG_STATE_HOME/conspectus/history/`. Both the daemon and
  in-process discovery write it. ADR 0087 already lists `graph.bin`
  as a category 2 sidecar, and the daemon already writes that, but
  prohibition 5 says "the daemon reads and caches; it never writes to
  user-intent stores". ADR 0090 read the prohibition more strictly
  than that, as "the daemon writes nothing". The rule needs stating
  outright.
- **A pin store outside the config directory.** Pin slots (ADR 0141)
  move from `[[pins.entries]]` in project and user config files to one
  user-scope file, `$XDG_DATA_HOME/conspectus/pins.toml`. Category 1
  lists `.conspectus.toml` and `$XDG_CONFIG_HOME/conspectus/config.toml`
  only.
- **Cloning a repo during a rebuild.** Rebuilding a gone repo or
  primary checkout (ADR 0142) clones it from its recorded remote.
  Prohibition 6 says Conspectus never mutates git state, and cloning
  is a git command that creates a repository.
- **Rebuild steps.** Each rebuild step has to land in a category.

The operator chose to supersede ADR 0087 rather than amend it again, so
the whole envelope reads from one document.

## Decision

Conspectus operates under a **mutation envelope**: a bounded set of
write surfaces, each ADR-scoped, and a list of mutations Conspectus
never performs. Read-only remains the default for discovery and
orchestration. Every write path fits inside the envelope, and every
prohibition is absolute except where this ADR names a carve-out.
`docs/capabilities.toml` is the enforced form (ADR 0117).

### Sanctioned mutations

Each write is operator-initiated, by a CLI or TUI gesture or by a
persistent operator-authored preference, except category 2, which is
observation. Each has an ADR describing its schema and semantics.

1. **User-intent TOML stores under Conspectus's ownership.**
   `[declared]` (ADR 0014), `[aliases]` (ADR 0029), and any future
   user-intent table in a project's `.conspectus.toml` or in
   `$XDG_CONFIG_HOME/conspectus/config.toml`, plus the pin slot store
   `$XDG_DATA_HOME/conspectus/pins.toml` (ADR 0141). Writes are
   idempotent, unchanged-in unchanged-out, preserve operator-authored
   comments and formatting (`toml_edit`), and go through a sibling
   temporary file renamed into place. Legacy `[[pins.entries]]` tables
   (ADR 0057) are still read until `conspectus pin migrate` removes
   them; nothing else writes them.

2. **Rebuildable observations under `$XDG_STATE_HOME/conspectus/`,
   plus the snapshot cache.** Hook sidecar records (ADR 0028), the
   history store (ADR 0140), the TUI state file, and `graph.bin` under
   `$XDG_DATA_HOME/conspectus/` (ADR 0083). Deleting any of them loses
   no operator intent. History is the one whose loss is felt, since it
   holds gone entities and their recorded commands, but pin slots keep
   their own copy of what they need (ADR 0141). None of them carries
   payload, with the one exception ADR 0140 makes to ADR 0086 Tier 3:
   a mux's launch argv is recorded verbatim.

3. **Mux backend lifecycle when operator-initiated.**
   - `rename-session` under the rename overlay and `pin rename` on a
     live mux slot (ADR 0029, ADR 0141).
   - `new-session -d -c <cwd> <argv>`, then `set-option remain-on-exit
     failed`, under `mux new`, `mux launch`, and the mux rebuild
     executor (ADR 0095, ADR 0096, ADR 0103, ADR 0142).
   - `send-keys` of the Conspectus-built launch argv into a live mux's
     pane, only when the mux's harness has exited and pane-process
     evidence shows no harness running there (ADR 0102, ADR 0142). It
     never sends operator-typed input.
   - `kill-session`, graceful first, under worktree close-down and for
     a dead pane during a launch (ADR 0093, ADR 0103).
   - `attach-session` / `switch-client`, and `zellij attach`, which
     also resurrects an exited zellij session (ADR 0142).

   Every mux operation targets a mux the operator selected, pinned, or
   asked to rebuild. Conspectus never enumerates muxes and proactively
   renames, spawns, or kills them. A bulk rebuild (backlog `CSP-686`)
   is one operator gesture over slots the operator pinned, shown as a
   plan first.

4. **Subprocess launches Conspectus owns end to end.** Harness resume
   (ADR 0058's resume command, now the session rebuild executor in ADR
   0142), the transcript viewer's external fallback (ADR 0052), the
   TUI re-running `conspectus rebuild`, `mux new`, and `mux launch` as
   a child process, and worktree changes delegated to worktrunk
   (`wt switch`, `wt remove`, `wt merge`, `wt step prune`; ADR 0092).
   The argv comes from ADR-scoped inputs: a slot's override, a
   recorded launch argv, a launch profile, or a harness default (ADR
   0142).

5. **Harness hook installation when operator-initiated.**
   `conspectus hook init <harness>` adds, and `hook remove` removes,
   Conspectus's own hook entry in the harness's settings file:
   `~/.claude/settings.json`, `$CODEX_HOME/config.toml` (default
   `~/.codex/config.toml`), Pi's Conspectus-owned extension file, or
   their project-scope equivalents under `--scope project` (ADR 0028,
   ADR 0121). The write is idempotent, keeps every other setting, goes
   through a sibling temporary file, and `--dry-run` prints it
   instead. Conspectus never installs or removes a hook on its own.

6. **Cloning a repository during a rebuild.** `git clone <remote>
   <path>` when the operator rebuilds a gone repo or primary checkout
   (ADR 0142). It runs only in the act context, only into a path that
   doesn't exist, and with the remote and path from the history record
   or the operator's slot override. No other git command that changes
   anything runs (prohibition 6).

### Prohibitions

Conspectus **never**:

1. **Mutates harness-native state.** No writes to harness databases,
   session files, transcripts, or settings, except the category 5 hook
   entry. Readers open SQLite with `PRAGMA query_only = ON` (ADR 0048).
   Conspectus doesn't call a harness's own mutation API either.

2. **Injects terminal input into agents (ADR 0028, absolute).**
   `send-keys` into a live agent's pane is off-charter. The category 3
   `send-keys` path types only Conspectus-built launch argv into a
   pane whose harness has exited, and only after pane-process evidence
   says no harness is running there.

3. **Persists payload content.** Sidecars, history, and snapshots stay
   Tier 3 (ADR 0086): attribution and observation, not conversation.
   ADR 0140 records a mux's launch argv verbatim, including any prompt
   a harness argv carries; that is the only payload-adjacent field, and
   it is the command the operator ran, not anything a harness wrote.

4. **Writes to shared or system-owned locations.** Every write targets
   `$HOME`, the XDG base directories, a project tree the operator
   points at, or the destination of a category 6 clone.

5. **Performs background mutation.** Only operator gestures mutate.
   The daemon and every other background pass may write category 2
   observations (history, hook sidecars, `graph.bin`) and nothing in
   categories 1, 3, 4, 5, or 6: no user-intent store, including
   `pins.toml`; no mux rename, creation, or kill; no subprocess launch
   beyond the read-only probes discovery runs; no hook installation;
   no clone.

6. **Mutates git state.** Conspectus reads common dirs, config, and
   worktree metadata. It never commits, checks out, fetches, pushes,
   or runs any command that changes a repository. The one carve-out is
   category 6: `git clone` into a path that doesn't exist, which
   creates a repository without changing any existing one. Worktree
   changes go through worktrunk (ADR 0092).

7. **Bypasses hooks or signing.** There is no `--no-verify`
   equivalent. If a hook guards a write and fails, the write fails.

### History and pin slot placement

| Write | Category | Writers | Context |
|---|---|---|---|
| `$XDG_STATE_HOME/conspectus/history/*.json`, `history.lock` | 2 | daemon cycle, in-process discovery, rebuild and launch paths, `history reap` / `forget` | read and act |
| `$XDG_DATA_HOME/conspectus/pins.toml` | 1 | `pin` commands, `mux new` / `mux launch --pin`, `pin migrate`, TUI pin gestures | act only |
| Legacy `[[pins.entries]]` removal | 1 | `pin migrate` | act only |

`history reap` and `history forget` remove history records only. They
never touch tmux, git, or the filesystem outside the history store.

### Rebuild step placement

| Step | Category |
|---|---|
| Mux `new-session`, `set-option`, attach | 3 |
| Mux `send-keys` of the built argv into an agent-exited pane | 3 |
| Zellij `attach` to resurrect an exited session | 3 |
| Harness resume inside a new mux | 3 (it is the mux's initial command) |
| Worktree `wt switch [--create --base <head>] <branch>` | 4 |
| Repo or primary checkout `git clone <remote> <path>` | 6 |
| History record written at launch | 2 |
| Slot retargeted to a rebuilt worktree at a new path | 1 (the operator's rebuild gesture) |

### Rules for new features

A new write path must name the operator gesture that triggers it,
describe its scope and idempotence, place it in a category above,
confirm it touches no prohibition, add its entry to
`docs/capabilities.toml` citing its ADR, and land as an ADR that cites
this one. Removing a prohibition, or adding a background mutation,
needs this ADR superseded.

## Consequences

- `CLAUDE.md`, `docs/design.md`, `docs/capabilities.toml`, and code
  comments cite this ADR for the envelope. Older ADRs that place their
  writes "under ADR 0087 category N" read the same against this ADR,
  because the numbers are unchanged.
- The capability manifest narrows the read-context write allowance in
  `$XDG_DATA_HOME/conspectus/` to `graph.bin` and its temporary file,
  and allows `pins.toml` in the act context only, so a traced daemon
  or read command that touched `pins.toml` fails the test.
- `git` gains one act-context pattern, `clone * *`.
- The daemon's category 2 writes are now an explicit rule rather than
  an inference from `graph.bin`.

## Alternatives Considered

- **Amend ADR 0087 again.** Rejected by the operator. ADR 0087 says a
  removed prohibition needs supersession, and the git carve-out is
  one. A third amendment would also leave the envelope spread across
  one decision and three patches.
- **`gh repo clone`.** Covers GitHub remotes only. Recorded remotes
  include other hosts, and `gh` would be a second network path where
  `git` already is one.
- **Leave clone to the operator.** The rebuild plan would stop at "run
  `git clone …` yourself" for a gone repo, which is most of the work
  of rebuilding it. The carve-out is narrow enough (an absent
  destination, the act context, nothing else) that the prohibition's
  purpose, never changing a repository the operator has, holds.
- **Put `pins.toml` in `$XDG_CONFIG_HOME`.** That directory holds
  hand-edited configuration, often version-controlled in dotfiles.
  Slots are operator intent written mostly by gestures, with
  snapshots that change as the operator works. `$XDG_DATA_HOME` is
  where user data an application owns belongs (ADR 0141).
- **Let only the daemon write history.** Without a daemon, history
  would never be recorded. ADR 0140 merges writers under a lock
  instead.

## Open Questions Answered

- **Does the daemon write?** Yes: category 2 observations only.
- **Is cloning "mutating git state"?** It creates a repository; it
  doesn't change one. This ADR sanctions exactly that, into a path
  that doesn't exist, and nothing else.
- **Is recording a launch argv persisting payload?** ADR 0140 decides
  it isn't, for the reason given under prohibition 3.

## Related ADRs

- ADR 0087 (mutation envelope) — superseded by this ADR.
- ADR 0014, ADR 0029 — category 1 stores.
- ADR 0028 — hook sidecars (category 2), hook installation (category
  5), and the terminal-injection prohibition.
- ADR 0048 — read-only harness readers.
- ADR 0052 — transcript viewer fallback, category 4.
- ADR 0083 — `graph.bin`, category 2.
- ADR 0086 — payload tiers; ADR 0140 amends Tier 3 for launch argv.
- ADR 0092 — worktree changes through worktrunk, category 4.
- ADR 0093 — `kill-session`, category 3.
- ADR 0095, ADR 0096 — `mux new` and `mux launch`, category 3.
- ADR 0103 — `remain-on-exit failed`, category 3.
- ADR 0117 — the capability manifest that enforces this envelope.
- ADR 0140 — history, category 2.
- ADR 0141 — pin slots and `pins.toml`, category 1.
- ADR 0142 — rebuild plans; every step's category.
