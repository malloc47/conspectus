# ADR 0121: Pi Harness Support

## Status

Accepted.

## Context

`docs/design.md` limits the v1 harnesses to Atelier's set (`claude-code`,
`opencode`, `codex`, `aider`) and says not to add others "until needed."
The operator now runs Pi (`@earendil-works/pi-coding-agent`, evaluated at
1.3.0, upstream commit `8efd314`, on 2026-10-16), and Pi work is invisible
to Conspectus. Pi is a minimal coding agent that is extended through
TypeScript extensions rather than built-in features.

Pi's state fits the existing seams without new graph concepts. The facts
this ADR has to settle against the model:

- **Session store.** Each session is one JSONL file at
  `~/.pi/agent/sessions/--<cwd>--/<timestamp>_<session-id>.jsonl`. The
  directory name encodes cwd lossily: the leading separator is dropped and
  `/`, `\`, and `:` all become `-`, so `/src/a-b` and `/src/a/b` share a
  directory. The first line is a header,
  `{"type":"session","version":3,"id","timestamp","cwd","parentSession"?}`.
  Versions 1 and 2 still exist on disk because Pi migrates a file only
  when it loads it.
- **Store location.** `PI_CODING_AGENT_DIR` moves the agent directory
  (default `~/.pi/agent`). `--session-dir`, then
  `PI_CODING_AGENT_SESSION_DIR`, then the `sessionDir` setting replace the
  session directory. A `sessionDir` in project settings overrides the
  global one, and a relative value resolves against the cwd Pi was run in.
- **Writes.** Pi creates the file at the first exchange, not at startup,
  and appends without holding the file open. `--no-session` runs write
  nothing.
- **Ids.** UUIDv7 by default. `--session-id` accepts any
  `[A-Za-z0-9._-]` string.
- **Lineage.** `/fork`, `/clone`, and `pi --fork` write a new file whose
  header `parentSession` is the parent's *file path*. `/tree` branches live
  inside one file as an `id` / `parentId` entry tree.
- **Names.** `session_info` entries carry the `/name` display name; the
  latest one wins.
- **Runtime.** The process title is `pi`, or `pi-rpc` in RPC mode. Pi
  exports `AI_AGENT=pi` and `PI_CODING_AGENT=true` to children. Resume is
  `pi --session <path|id>`, which searches the current project first.
- **Extensibility.** Pi has no shell-command hook config. Extensions are
  TypeScript or JavaScript files that Pi loads through jiti with no build
  step, from `<agent-dir>/extensions/` (personal) or `.pi/extensions/`
  (project, which needs the operator's trust approval). A `session_start`
  event fires with `reason` `startup`, `reload`, `new`, `resume`, or `fork`,
  and `ctx.sessionManager` exposes the session id, file, and cwd.
  Extensions load in every mode, and `ctx.mode` says which one is running.

## Decision

### 1. Harness identity

Pi is a supported harness with harness key `pi` and display label `pi`.
`docs/design.md` lists it in the v1 harness set.

### 2. State root and layout

The adapter resolves one session root, in this order:

1. `CONSPECTUS_PI_STATE`, matching the other `CONSPECTUS_<HARNESS>_STATE`
   overrides.
2. `PI_CODING_AGENT_SESSION_DIR`.
3. `sessionDir` from the global `<agent-dir>/settings.json`, only when it
   is an absolute path.
4. `<agent-dir>/sessions`, where `<agent-dir>` is `PI_CODING_AGENT_DIR` or
   `~/.pi/agent`.

Step 3 follows Pi's own precedence below the environment variable.
Relative and project-level `sessionDir` values are not followed: they
resolve against each run's cwd, so finding them would mean searching
project trees, which discovery doesn't do. `--session-dir` is per-run and
equally out of reach. Operators who use either point
`CONSPECTUS_PI_STATE` at the directory.

Under the root, the adapter scans both layouts: `*.jsonl` directly in the
root (flat) and `--*--/*.jsonl` one level down (grouped). cwd always comes
from the header. The directory name is never decoded back into a path.
Each session's `state_scope` is the root, not its cwd directory, so a
parent in another cwd directory still resolves (decision 4).

The header `id` is the session key. When the header has no usable `id`,
the adapter falls back to the `<session-id>` part of a
`<timestamp>_<session-id>.jsonl` filename. A file with neither is skipped.

### 3. Payload tiers (ADR 0086)

- **Tier 1:** the header, `session_info` names, and entry timestamps. The
  discovery pass deserializes only `type`, `timestamp`, `name`, and the
  header fields from each line. It skips `message` bodies and retains no
  content. A `/name` title is operator-authored identity, like other
  harnesses' titles.
- **Tier 2:** the last-message preview (ADR 0023: the most recent user or
  assistant text, whitespace collapsed, capped at 200 characters) and the native
  transcript parser (ADR 0052). Both are sanctioned Tier 2 readers on the
  same terms as the opencode and codex previews and the existing parsers.
- The hook extension's payload (decision 6) carries ids, paths, and the
  event name only, which is Tier 3 and payload-free.

### 4. Lineage

A non-empty `parentSession` emits one ADR 0018 `parent_session`
candidate from the child to the parent, with `harness_key = "pi"` and
`lineage_kind = "fork"`, like the other harnesses' lineage. `/fork`,
`/clone`, and `--fork` all use `fork`: the header doesn't say which one
ran, and all three copy a parent's history into a new session, which is
what ADR 0018 means by `fork`. The candidate links two `AgentSession`
nodes directly. Pi records no fork entity with effects on other repos or
harnesses, so there is nothing for a `Fork` node to anchor.

The adapter recovers the parent's key from the parent file's header when
that file is readable, and otherwise from its filename. A parent outside
the scan or deleted becomes a `LinkEndpoint::Unresolved` target with the
recovered key and the path as a hint. A self-reference, an empty value,
or a filename that doesn't match the session-file pattern emits nothing.

In-file `/tree` branches are one session and emit no cross-session
lineage.

### 5. Runtime signature

- `process_command_basenames` is `pi` and `pi-rpc`, matched exactly.
- `command_substrings` is empty. A `pi` substring would match `pip`,
  `api`, `spin`, and many more. Pane-command detection instead matches
  whole whitespace-separated tokens whose basename is exactly one of the
  signature's basenames. This is a general option on `RuntimeSignature`,
  not a Pi special case.
- `pi-rpc` is a `Background` process. RPC mode is driven over stdio by
  another program, such as an editor integration or an orchestrator, so no
  person types into a pane for it. It must not claim pane attribution or
  appear as a live human session. Its session file is still discovered
  like any other.
- `fd_path_patterns` is empty. Pi holds the session file open only during
  an append, so an fd scan would catch it only by chance, and attribution
  would flicker between refreshes. Precise attribution comes from the hook
  extension.
- `extract_session_keys` is the generic UUID grammar. It finds UUIDv7 ids
  in `pi --session <id>`, `pi --fork <id>`, and in session-file paths
  passed to those flags. Custom `--session-id` values outside that grammar
  are not extracted from commands. This is a documented limitation.
- `launch_argv` is `pi`. `resume_argv` is `pi --session <id>`, run from
  the session's cwd, because Pi resolves ids in the current project first.

### 6. Hook extension and `hook init pi`

ADR 0049's contract carries over: the writer is
`conspectus hook write pi`, `CONSPECTUS_HOOK_BIN` overrides the binary,
a missing binary or failed spawn is a silent no-op, and the flat payload
`{session_id, cwd, transcript_path, hook_event_name}` is the boundary, so
the Rust writer never parses Pi's types. The extension handles
`session_start` only. Every reason Pi reports (`startup`, `reload`, `new`,
`resume`, `fork`) marks a change of the active session, which is all
attribution needs.

Distribution departs from ADR 0049 in three ways, because Pi loads a
single TypeScript file with no build or package step:

1. **One file, no package.** The source is
   `plugins/pi-hook/conspectus-hook.ts`, depending only on Node built-ins.
   There is no `package.json`, build step, or npm publish.
2. **Embedded in the binary.** `conspectus` embeds the file at compile
   time, so `hook init pi` works without a repo checkout and always
   installs the version that matches the writer.
3. **Installed by `hook init`.** `conspectus hook init pi` writes the file
   to `<agent-dir>/extensions/conspectus-hook.ts` (`--scope user`) or
   `<repo>/.pi/extensions/conspectus-hook.ts` (`--scope project`).

This is ADR 0087 category 5, harness hook installation. For Pi, the
"hook entry" is this one Conspectus-owned file. Nothing else in Pi's
directories is written. Ownership is checked, not assumed. The file
starts with a marker line naming Conspectus as its owner, and:

- `hook init` writes the file when it is absent and replaces it when it
  carries the marker. It refuses to overwrite a file at that path
  without the marker.
- `hook status` reports `installed`, `outdated` (marked but different from
  the embedded copy), `not-installed`, or `foreign` (unmarked).
- `hook remove` deletes the file only when it carries the marker. It
  never touches other files in the extensions directory.

The extension writes nothing in two cases:

- When `getSessionFile()` is undefined (a `--no-session` run). There is
  no session file for the record to point at.
- When `ctx.mode` is anything but the interactive TUI. Print, JSON, and
  RPC runs don't own a pane, matching the `pi-rpc` classification above.

Opting out is `hook remove pi` or Pi's own extension controls. There is
no Conspectus-side knob, as in ADR 0049.

## Consequences

- Pi sessions join the sessions graph, repo grouping, lineage, TUI, and
  viewer through the existing adapter seams. No model, resolver, or
  relation changes are needed.
- `RuntimeSignature` gains an exact-token pane-command match. Short
  basenames can use it instead of substrings, and existing harnesses are
  unaffected.
- `hook init` can now write a whole file in a harness directory, not only
  edit a settings entry. Category 5 stays narrow: one fixed path per
  scope, marker-checked ownership, and reversible by `hook remove`.
  `docs/capabilities.toml` lists both paths.
- A new session's hook record arrives before Pi writes its file. The
  sidecar reader already skips records whose `transcript_path` doesn't
  exist, so until the first exchange the pane keeps its previous
  attribution. The new record takes effect once the file appears. A
  session abandoned before any exchange leaves a record that never takes
  effect, which is correct, since there's no session to show.
- Hook pid resolution walks the parent chain comparing
  `/proc/<pid>/comm` with `pi`. That assumes Node's process title also
  sets the thread name on Linux. It has not been checked against a live
  Pi (backlog `CSP-630`). Until it is, a Pi hook record may carry no
  pid, and the sidecar reader then skips its liveness check. If the
  assumption fails, the extension passes its pid explicitly, which is a
  writer change and not a reversal of this decision.
- Documented limitations: custom non-UUID `--session-id` values aren't
  matched in pane commands, `--no-session` runs are invisible, and
  relative, project-level, or per-run session directories need
  `CONSPECTUS_PI_STATE`.

## Alternatives Considered

### A. Decode cwd from the directory name

Skip parsing the header for cwd. Rejected: the encoding is lossy, and a
repo path containing `-` would land in the wrong repo.

### B. Substring-match `pi` like other harnesses

Rejected: `pi` is a substring of too many commands (`pip`, `pipx`,
`api-server`, `spin`). The false positives would attach unrelated panes
to Pi sessions.

### C. Ship the extension as an npm package, per ADR 0049

Rejected for Pi. Pi loads a single TypeScript file directly, so a package
adds a build step and a manual install for no benefit. Embedding the file
also keeps the extension and the writer in step.

### D. Have `hook init pi` print the file for the operator to install

Rejected on the same grounds as ADR 0087's amendment: an owned file that
`hook remove` can undo is safer than asking the operator to copy code into
their extensions directory by hand.

### E. Use a `Fork` node for Pi forks and clones

Rejected under ADR 0018. Pi records no fork entity with effects on other
harnesses or repos, and the direct `parent_session` link answers the
lineage query with one edge.

### F. Watch the session file through fd scans

Rejected. Pi holds the file open only during an append, so fd evidence
would come and go between refreshes.

## Open Questions Answered

- Harness key and display label: `pi`.
- State root: `CONSPECTUS_PI_STATE`, then `PI_CODING_AGENT_SESSION_DIR`,
  then an absolute global `sessionDir`, then `<agent-dir>/sessions`. Both
  layouts are scanned and cwd always comes from the header.
- Lineage: `parent_session` with `lineage_kind = "fork"`. `/tree`
  branches emit none.
- Payload tiers: header, name, and timestamp reads are Tier 1. The
  preview and transcript parser are Tier 2 with the ADR 0023 cap.
- Runtime: exact basenames `pi` and `pi-rpc`, no substrings, `pi-rpc` as
  `Background`, no fd patterns.
- Hook: a single embedded TypeScript file, installed and removed by
  `hook init pi` / `hook remove pi` under ADR 0087 category 5 with marker
  ownership. It writes only for interactive sessions with a session file.
