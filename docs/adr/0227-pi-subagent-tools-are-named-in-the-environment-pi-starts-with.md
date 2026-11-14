# ADR 0227: Pi's Subagent Tools Are Named In The Environment Pi Starts With

## Status

Accepted. Extends [ADR 0206](0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md)
decision 25 (which tools the Pi extension counts as subagents) and the
distribution of [ADR 0121](0121-pi-harness-support.md) decision 6 (the
embedded, byte-compared extension file). Adds no program, subcommand,
socket, signal, or write path (ADR 0117, ADR 0143), and no configuration
key.

## Context

Pi has no subagents of its own. A package adds them as a tool: the
reference `subagent` example runs child `pi` processes behind one long
call of a tool named `subagent`. The Pi extension counts a session's
subagents as the calls in flight of tools in a fixed set, `{"subagent"}`
(ADR 0206 decision 25). A third-party subagent package with other tool
names shows its children as ordinary tool calls (research note 04, open
question 3).

The names have to reach the extension, which runs inside Pi. The
extension is one file that `conspectus hook init pi` writes from a copy
embedded in the binary, and `hook status pi` says `installed` only when
the file on disk is byte for byte that copy (ADR 0121 decision 6). The
ways in:

- **A variable in Pi's environment**, read by the extension when Pi
  loads it.
- **A configuration key** in Conspectus's `config.toml` that `hook init
  pi` writes into the file it installs, so the installed file differs by
  configuration.
- **The extension reading Conspectus's configuration** itself.

## Decision

1. **`CONSPECTUS_PI_SUBAGENT_TOOLS`.** The extension reads this variable
   from its own environment once, when Pi loads it (a restart or
   `/reload` picks up a change). Its value is tool names separated by
   commas; each is trimmed, and an empty one, one longer than 256
   characters, or one holding a control character is skipped. The names
   are added to `subagent`, never replace it, and are compared with a
   call's tool name exactly. The extension already treats
   `CONSPECTUS_HOOK_BIN` this way: Conspectus settings that the harness
   process carries are read from its environment (ADR 0049 decision 4).

2. **Nothing else changes about a call.** A call another tool makes
   (`parentToolCallId`) is that tool's work whatever its name, as
   before. A tool name is read only to classify and never leaves the
   extension (ADR 0206 decision 12); the snapshot carries the count.

3. **The installed file does not depend on the names.** `hook init pi`
   writes the embedded copy, and `hook status pi` keeps comparing the
   file byte for byte: a copy used with the variable set is `installed`
   like any other. Status cannot see the names, since they live in Pi's
   environment and not in its own, and says nothing about them.

4. **The extension is version 3.** A version 2 copy reads no names, so
   it differs from the embedded copy and `hook status pi` reports it
   `outdated`, naming both versions; it still reports live state, so the
   health line is unaffected. Version 2 stays the first that reports
   live state.

## Consequences

- An operator with a third-party subagent package sets one variable
  where Pi starts (a shell profile, or tmux's global environment for
  agents Conspectus launches) and gets `+N` for its calls, without
  editing the installed file.
- `hook status pi` stays a pure comparison with the embedded copy: no
  configuration enters what "installed" means, and an install stays
  idempotent and its removal exact.
- The names are per environment, not per Conspectus configuration: two
  Pi processes started with different values count differently, and a
  project-level `.conspectus.toml` has no say. Nobody has asked for more.
- The shared event script `subagent-configured` (header
  `subagent_tools`) and the extension's node tests cover a configured
  name; the Rust tests replay its payloads, which carry no name.

## Alternatives Considered

- **A configuration key written into the installed file.** Keeps the
  setting with Conspectus's other configuration and could be scoped by
  project. But `hook status pi` would have to render the file from the
  current configuration to compare it, a changed key would make every
  installed copy `outdated` until `hook init pi` ran again, and the
  installed file would differ from the copy the binary carries, which
  the ownership check and the version note were built around. Rejected
  as the larger change for the same effect.
- **The extension reading Conspectus's configuration.** Would mean
  resolving and parsing `config.toml` (and project files) in TypeScript
  inside Pi, a second reader that has to track the first. Rejected.
- **Replacing the default set.** An operator who uses both the reference
  tool and another package would have to list both. Adding is simpler
  and loses nothing.
- **Matching names by pattern.** A pattern can match tools that are not
  subagents; exact names are what the research asked for. Rejected.

## Open Questions Answered

- *Configurable in Conspectus's configuration, through an environment
  variable or a regenerated extension file (research note 04, open
  question 3)?* An environment variable, read by the extension; no
  configuration key and no regenerated file.
- *What does `hook status pi` report for a configured copy?* The same as
  for any copy: `installed` when the file is the embedded copy. It does
  not report the names.
