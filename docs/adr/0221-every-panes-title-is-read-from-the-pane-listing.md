# ADR 0221: Every Pane's Title Is Read From The Pane Listing

## Status

Accepted. Amends decisions 1, 2, 6, 7, and 9 of
[ADR 0217](0217-a-codex-panes-title-fills-in-its-live-state-where-hooks-cannot.md)
and answers its alternative "Every pane's title (`list-panes -a`)",
deferred there until it was worth a second tmux call. Reuses the pane
listing of [ADR 0220](0220-a-hook-record-is-judged-by-the-harness-its-own-pane-runs.md)
and the server match of [ADR 0205](0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md)'s
amendment. Stays under [ADR 0086](0086-payload-privacy-tenet.md) Tier 2
as ADR 0217 placed it. Bumps the snapshot format to 17 (ADR 0083). Adds
no program, subcommand, socket, signal, or write path, and no tmux call
(ADR 0117, ADR 0143).

## Context

ADR 0217 read the title of each tmux session's active pane from the
session listing, and only for a pane whose program tmux reports as a
harness that classifies its titles (`codex`, `codex-rs`). Two kinds of
Codex got nothing:

- **A Codex in another pane** of a split window, or in a window that is
  not current. Covering it needed `tmux list-panes -a`, a second call per
  server each mux cycle, which ADR 0217 deferred.
- **A Codex started through a launcher**: the npm package runs a Node
  script that starts the native binary as its child, and tmux reports the
  pane's foreground process group leader, `node`. Recognizing it from the
  program alone would be a guess: most `node` panes are not Codex.

ADR 0220 has since made the pane listing part of every probe: one
`list-panes -a` per server with sessions, each cycle, for the pane id,
its session, and its program. Asking it for the title as well costs no
call.

## Decision

1. **Every pane's title, from the pane listing.** The pane listing's
   format gains `#{pane_title}` as its last field; the session listing
   no longer asks for the active pane's title. Every pane of every
   listed server is so read once per probe, at no further call. Setting
   `CONSPECTUS_DISABLE_PANE_TITLES` leaves `#{pane_title}` out of the
   pane listing, which still lists the panes for ADR 0220. A failed pane
   listing reads no title.

2. **Whose titles.** A pane's title is classified at the split, by the
   program the same row reports for that pane, and dropped:
   - a program that names a harness by its exact process names (the hook
     sidecar's evidence, as in ADR 0217) is read by that harness's
     classifier when it has one, and by nothing when it has none;
   - a program that names no harness but that a harness lists as one of
     its *launchers* (a new adapter method, empty by default; Codex lists
     `node`) is read by that harness's classifier too, as a *launched*
     pane;
   - every other pane's title is dropped unclassified.

3. **A launched pane counts only on its hook's word.** The binding uses
   a launched pane's mark only where that harness's hook entry names the
   pane (by server, session, and pane, ADR 0217 decision 7's *hook entry*
   for that harness, alive and not exited): the record the hook sidecar
   would attribute to that pane, since a launcher is no harness for its
   gate. ADR 0217's rules 2 to 4 then apply as for a `codex` pane (a fresh
   hook entry wins; a quiet, stale, or `unknown` one gives way; an
   answered approval clears). Rule 1, a pane-only binding with no hook
   entry, does not apply: with no hook entry the mark binds nothing.

4. **Marks by pane.** The probe keeps the marks by pane id
   (`MuxProbe.titles`), with each pane's session and program in
   `MuxProbe.panes` and whether it is its session's active pane. A mark's
   run is carried from one probe to the next per pane. Everything else
   about a mark (what it asserts, its tier, where it rides) is as in ADR
   0217.

5. **A hook entry that names no pane** came from a hook tmux answered for
   the session's active pane (ADR 0028's amendment), so it is the hook
   entry of the active pane's title only, as before; another pane's
   title binds beside it.

6. **Where it shows.** A non-active pane's binding sits on its own pane
   of its mux (`pane_live`) or replaces its own hook entry's slot, like
   the active pane's. `hook status` counts the bindings read from pane
   titles by each pane's own program, so a launched Codex counts for
   Codex.

## Consequences

- A Codex in any pane of a tmux session shows `!` while it waits and `»`
  while it works, counted on its mux, in the headers, and for `W`.
- An npm-installed Codex whose hooks report gets the title's repairs (an
  answered approval clears; a quiet `working` gives way to a waiting
  title). With no hooks reporting it shows nothing, as before.
- The probe reads the titles of every pane, not only the active ones, and
  classifies those of `node` panes as Codex's in memory; only marks are
  kept, and a `node` pane's mark is used only with a Codex hook entry for
  that pane. The sentinel test plants a marker in every pane's title.
- The pane listing returns a few more bytes per pane; the session
  listing a few fewer.
- `graph.bin` moves to format version 17; older files cold-rebuild.

## Alternatives Considered

- **Stay with active panes.** Free, but leaves every split-pane Codex
  dark, now that the pane listing already runs each cycle. Rejected.
- **Keep the session listing's title and add the others.** Two sources
  for the active pane's title that can disagree by a few milliseconds.
  Rejected for one source.
- **Recognize a wrapped Codex by its start command or its child
  process.** `#{pane_start_command}` is empty for a pane started from a
  shell, and walking each `node` pane's children is the process-tree walk
  ADR 0091 keeps off the probe's path. Rejected for the hook's word.
- **Treat every `node` pane as Codex.** A guess. Rejected.
- **Classify every unrecognized pane's title by every classifier.** Reads
  marks off panes no harness launches. Rejected for an explicit launcher
  list.

## Open Questions Answered

- *Does this read more than titles?* No: the pane listing's id, session,
  program, and title, the last reduced to a mark at the split.
- *Is a launcher list a guess?* No more than the process names are: it
  only says whose classifier reads the title, and the mark is used only
  with that harness's own hook entry for the pane.
