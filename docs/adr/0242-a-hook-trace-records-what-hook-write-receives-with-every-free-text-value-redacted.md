# ADR 0242: A Hook Trace Records What `hook write` Receives, With Every Free-Text Value Redacted

## Status

Accepted. Extends [ADR 0206](0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md)
(what a writer may keep) and [ADR 0204](0204-the-live-state-store-folds-agent-events-per-session.md)
(the writer is silent and best-effort). Adds one more category 2
observation to [ADR 0143](0143-mutation-envelope-with-history-pins-and-clone.md)
and one more Tier 3 record to [ADR 0086](0086-payload-privacy-tenet.md),
and a direct dependency on a crate the lock file already holds.

## Context

The live-state reducers and the fixtures that test them were built from
each harness's documentation. Every research note lists claims it could
not verify (whether `PermissionRequest` fires for questions, what a
subagent's `PostToolUse` carries, how `session.status` behaves), and
comparing real event sequences with the reducers needs real payloads.

A raw payload carries a prompt, a tool's input and output, paths, and
session ids. ADR 0086 keeps those off disk, and a recording that is
redacted later, from a raw capture, still puts the raw payload on disk
first. So the recording has to be redacted in memory, before anything is
written, by something that sees what the installed hooks really receive.

All four harnesses already reach one place: `conspectus hook write
<harness>`. Claude Code and Codex run it once per event with the payload
on stdin. The Pi extension and the opencode plugin run it with a flat
payload their own code built. The writer already parses the payload,
maps it to neutral events, applies them to the store, and is bound to be
silent and to exit 0 whatever happens (ADR 0204 decision 6).

## Decision

1. **A trace inside the writer, switched on by the environment.** When
   `CONSPECTUS_HOOK_TRACE` holds a name matching
   `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, every `conspectus hook write
   <harness>` appends one JSON line to
   `$XDG_STATE_HOME/conspectus/hook-trace/<name>.jsonl`. The state
   directory is resolved as the live-state store resolves it
   (`$XDG_STATE_HOME`, else `$HOME/.local/state`, and `--state-root`
   when given). The directory is created with mode 0700 and the file
   with mode 0600. The line goes out in one `write` on a file opened
   with `O_APPEND`, with no lock: concurrent writers append whole lines
   in the order the kernel serves them. The variable unset, or holding a
   name that does not match, touches no file; the whole cost is one
   environment lookup. Only `hook write` reads the variable, so the
   daemon never traces and nothing starts a trace on the operator's
   behalf. The operator sets it for a session they want recorded, and
   deletes the files when done (nothing rotates or collects them).

2. **Tracing never changes the hook.** The trace is recorded last, after
   the store write and the attribution record, so it cannot delay or
   reorder either. It prints nothing, adds no exit status, starts no
   process, opens no socket, takes no lock, and swallows every error,
   an unwritable path and a panic included (the writer's own
   `catch_unwind` already covers the last). A trace that cannot be written
   is silently not written.

3. **A line is a record of what the writer saw and did.**

   ```json
   {"v":1,"t_ms":1794400904859,"harness":"claude-code",
    "payload":{...redacted...},
    "env":{"TMUX_PANE":true,"TMUX":true,"CLAUDE_PID":true},
    "events":["tool_finished"],"applied":"written",
    "after":{"state":"working","detail":"generating","subagents":0,
             "via_subagent":false,"since_ms":1794400900000,"rev":41}}
   ```

   - `v` is the format version (1) and `t_ms` the writer's start time.
   - `harness` is the command-line argument when it has the shape of a
     name, else `"<invalid>"`.
   - `payload` is the redacted payload (decision 4), or a placeholder
     when stdin was not one JSON value.
   - `env` holds booleans for whether `TMUX_PANE`, `TMUX`, and
     `CLAUDE_PID` were set, and nothing from their values.
   - `events` lists what the adapter produced, in order: the neutral
     event's name (`LiveEventKind::label`) with a qualifier from a
     closed enum where one exists (`wait_started:permission`,
     `turn_ended:interrupted`, `turn_failed:auth`, `retrying:rate_limited`,
     `session_started:after_compaction`, `background_work:2`) and `@child`
     when the event names a child agent. A plugin's snapshot is
     `["snapshot"]`. A payload that maps to nothing gives `[]`.
   - `applied` is the store's answer as a word: `written`, `unchanged`,
     `dropped`, `rejected`, or `newer_schema`; or the writer's own:
     `none` when nothing mapped, `no_store` without a state home, `error`
     when the write failed.
   - `after` is the session's observation read back from the store right
     after the apply, without the lock: `state`, `detail` (absent when the
     state has none), `subagents`, `via_subagent`, `since_ms`, and the
     entry's `rev`. It is absent when nothing was stored for the session.
     It is what ties each real event to the state the reducer computed.
     Another writer may have changed the entry in between; the `rev` says
     which write it is.

4. **Redaction is default-deny and keeps structure.** The payload is read
   whole in memory, only when tracing, and walked once. What survives:

   | Value | Kept as |
   | --- | --- |
   | number, boolean, null | itself |
   | object key | itself when it matches `^[A-Za-z_][A-Za-z0-9_]{0,39}$` and sits at depth 3 or less (the top-level keys are depth 1, and arrays add none); else `"<key>"` (a second one in the same object `"<key:2>"`, since an object cannot repeat a key) |
   | array | its length, with the first 8 elements redacted and then `"<+N more>"`; an object keeps its first 64 entries and `"<+N more>": N` |
   | string under an enum key | itself, when it matches `^[A-Za-z0-9_.:-]{1,48}$` |
   | `tool_name` | itself when it has that shape and does not start with `mcp__`; else `"mcp__<redacted>"` for an MCP tool, `"<str:N>"` for any other |
   | string under an identifier key | `"id:"` and 8 hex digits of SHA-256 of the trace name, a NUL byte, and the value |
   | string under a path key | `"<path>"` |
   | any other string | `"<str:N>"`, N its length in characters |
   | the value of a payload-bearing key | `"<str:N>"`, `"<object:K keys>"`, or `"<array:K>"`, by its size alone |

   - *Enum keys* are the vocabulary every documented payload uses:
     `hook_event_name`, `source`, `trigger`, `reason`, `notification_type`,
     `permission_mode`, `error` (the Claude Code failure type), `status`,
     `state`, `detail`, `kind`, `type`, `stop_reason`, `error_class`,
     `mode`, `level`, `behavior`, and any key ending `_version`.
   - *Identifier keys* are `id`, `instance`, `uuid`, and any key ending
     `_id`, `_ids`, `Id`, `ID`, `Ids`, or `IDs` (`session_id`, `agent_id`,
     `tool_use_id`, `prompt_id`, `turn_id`, `request_id`, `agentId`,
     `sessionID`). The trace name salts the hash, so one id keeps one
     pseudonym within a trace and has another in the next trace.
   - *Path keys* are `cwd`, `path`, `dir`, `directory`, and any key ending
     `_path`, `_dir`, `Path`, or `Dir`.
   - *Payload-bearing keys* are `prompt`, `tool_input`, `tool_response`,
     `message`, `messages`, `last_assistant_message`, `error_details`,
     `compact_summary`, `custom_instructions`, `title`, `session_title`,
     `permission_suggestions`, `session_crons`, `description`, `command`,
     `content`, `text`, `body`, `summary`, `query`, `question`,
     `questions`, `options`, `answer`, `answers`, `input`, `args`,
     `output`, `stdout`, `stderr`, `result`, `plan`, `instructions`, and
     `system_prompt`, however a key spells its case or underscores. The
     fixtures' text-bearing keys are all on it, and any other key's
     strings are placeholders anyway.
   - The one exception is the Claude Code Agent tool's result: when the
     payload's `tool_name` is `Agent` or `Task`, `tool_response` keeps
     `status` (an enum) and `agentId` (an id), the two fields the mapping
     reads, and `"<other>": "<object:K keys>"` stands for the rest.
   - ADR 0206 decision 12 keeps `tool_name` out of the store, since a
     state needs none. The trace keeps a built-in tool's name, because a
     recording has to show which tool fired each event, and a built-in
     name is public vocabulary; an MCP tool's name is what reveals server
     names, so it is redacted.
   - A key at depth 4 or deeper, or one that does not match the key shape,
     is hidden, but its value is still redacted by the key's real name.
   - A line over 64 KiB keeps its other fields and replaces `payload` by
     its size placeholder.

   A field no one has seen cannot leak text: an unknown string is a
   placeholder, and an unknown object keeps only its shape.

5. **The trace is a Tier 3 record.** It holds enums, counts, numbers,
   pseudonymous ids, and sizes, as the live-state store does, and never
   payload (ADR 0086). It is a category 2 observation under ADR 0143: it
   lives under `$XDG_STATE_HOME/conspectus/`, the manifest's
   `$STATE/conspectus/**` already allows it, and deleting it loses no
   operator intent. It is not rebuildable, but nothing reads it, and it
   is disposable. ADR 0117's manifest cites this ADR for it.

6. **`sha2` becomes a direct dependency.** The pseudonym needs a stable,
   non-reversible hash. `sha2` 0.10 is already in the lock through two
   crates the TUI uses, so declaring it adds no crate to the build. It
   opens no socket and is not on the manifest's list of networking crates.

## Consequences

- The coordinator can run real sessions of all four harnesses with one
  variable set and no edit to any harness's configuration, and read what
  each sent, the events it became, and the state it produced. The first
  use is CSP-822: its recordings come from traces, and a trace line,
  unlike a raw capture, can be committed as a fixture.
- Raw payloads never touch a file. The only copy of one is in the
  writer's memory for the length of the call.
- Tracing costs one more parse of the payload (whole, not whitelisted),
  the walk, a read of the store, and one `write`, only when it is on.
  With it off the writer is as before. The task's final summary records
  the medians.
- Several residual channels remain, each bounded. A key of up to 40
  identifier characters at depth 3 or less is kept, so a payload that
  keyed an unknown object by text would show it, cut to that shape; the
  known text-keyed objects (`tool_input`, `tool_response`, `answers`) sit
  under payload-bearing keys and show no keys. A value under an enum key
  that happens to look like an identifier is kept, and the enum keys are
  those whose values are vocabulary in every documented payload. Numbers
  are kept, so durations and token counts show. A pseudonym is 32 bits of
  a salted hash, and the salt is the file's own name, so it hides an id
  with real entropy (a UUID, a tool-use id) and not a small integer.
- The trace grows by about a kilobyte per event and nothing trims it.
  The operator deletes the files; `docs/operations.md` says how.
- A new payload-bearing key that nobody listed is still safe (its
  strings are placeholders); only its object keys and nested numbers
  show.
- The format is versioned (`v`), so a later reader can tell a line that
  gained fields from one that lost them.

## Alternatives Considered

- **A recorder hook for Claude Code only**, added through a `--settings`
  file. It sees the payload before Conspectus does, but covers one
  harness, needs a second hook entry and a second program, and its
  redaction would be separate code that could drift from what the writer
  reads. The trace sees the same bytes the writer parses, for all four
  harnesses, and needs no settings.
- **Wrapping the binary**, a `conspectus` script that tees stdin to a
  file. It keeps raw payloads on disk, which is what this decision
  avoids, and it changes the command the harness runs.
- **Recording raw and redacting later.** The redaction would be easier
  to change, but the raw payloads would exist on disk for as long as it
  takes someone to run the second step, and a crash or an interrupted
  cleanup would leave them there. Rejected on ADR 0086's terms.
- **Allow-listing keys instead of default-deny.** Naming every field a
  harness may send would hide a new one's text, but also its structure,
  and the point of a recording is to find the fields no one listed. A
  default-deny walk keeps the shape of the unknown and drops its text.
- **A per-key closed set of enum values** (`notification_type` is one of
  these seven). Stricter, but a recording exists to find the values the
  documentation missed, and a closed set would blank them. The shape rule
  (`^[A-Za-z0-9_.:-]{1,48}$`) keeps a new word and drops a sentence.
- **Hashing without a salt**, or with a fixed one. The same session id
  would have one pseudonym in every trace, and a trace could be matched
  with another. The trace name as salt keeps pseudonyms stable within one
  recording and unlinkable across recordings.
- **A hand-written SHA-256, or `std`'s hasher.** The first is code to
  maintain and test for a function the lock file already provides. The
  second has no stability guarantee across Rust releases and is not a
  cryptographic hash.
- **A socket to the daemon**, which would write the trace. The writer
  takes no socket for a state event (ADR 0204), and the daemon would then
  be involved in a write the operator started from a hook.
- **Environment variable naming a path.** More flexible, but a path in
  the environment sends the file anywhere the process can write, including
  outside the state directory the manifest covers. A name keeps it in one
  place.

## Open Questions Answered

- *Where are the deviations from the first design?* Four, each tighter or
  clearer than it was. The salt is joined to the value by a NUL byte, so
  `ab` and `c` do not hash as `a` and `bc`. A tool name that is not
  name-shaped is a placeholder, like any other string. An object keeps
  its first 64 entries, as an array its first 8, and hidden keys within
  one object are numbered, since one cannot repeat a key. An event name
  carries the closed-enum qualifier so a trace shows a permission wait
  and a question wait as different events.
- *How do I tie a pseudonym back to a session in `live-state.json`?*
  Compute it: `printf '%s\0%s' NAME SESSION_ID | sha256sum | cut -c1-8`
  gives the digits after `id:`.
- *Can the trace be left on?* It is meant for a recording session: it
  adds a parse and an append to every hook call, and nothing prunes it.
- *Does the daemon or the TUI read a trace?* No. The format is for a
  person or a script to read; `v` is there for the day one does.
