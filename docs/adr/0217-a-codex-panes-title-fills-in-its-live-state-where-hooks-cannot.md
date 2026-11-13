# ADR 0217: A Codex Pane's Title Fills In Its Live State Where Hooks Cannot

## Status

Accepted. The first live-state source beyond hooks under
[ADR 0207](0207-live-state-sources-beyond-hooks-need-their-own-decision.md)
(decision 4), approved by the operator as decision 11 in section 7.1 of
the live-state plan (`docs/plans/live-state/README.md`). Uses the
carve-out of [ADR 0028](0028-hook-sidecar-mux-attribution.md) for an
already-visible status surface; places the reader under
[ADR 0086](0086-payload-privacy-tenet.md) Tier 2 and amends its list;
adds a source to the vocabulary of
[ADR 0203](0203-live-state-is-a-neutral-observation-with-a-closed-state-set.md)
and a step to the binding of
[ADR 0205](0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md);
bumps the snapshot format to 13 (ADR 0083). Adds no program, subcommand,
socket, signal, or write path (ADR 0117, ADR 0143). Amended by
[ADR 0221](0221-every-panes-title-is-read-from-the-pane-listing.md):
every pane's title is read from the pane listing of ADR 0220, and a
pane running one of a harness's launchers (`node` for Codex) is read as
that harness's where its hook entry names the pane (decisions 1, 2, 6,
7, and 9).

## Context

Codex's hooks (ADR 0206) leave three gaps
(`docs/plans/live-state/research/02-codex.md` §3):

- No hook fires when the operator answers an approval, so a session reads
  `waiting` through the whole run of the tool it approved.
- MCP elicitations and sign-in prompts fire no hook at all.
- Recent Codex versions skip a hook until the operator trusts it in
  `/hooks`, so a session whose hooks are untrusted, or that was started
  before they were installed, has no live state.

Codex writes its own run state into the terminal title (OSC 0). Its
source (`codex-rs/tui`, `main`, read 2026-11-12) builds the title from
the items `activity`, `thread-name`, and `project-name`, in that order by
default. The activity item is:

- a braille spinner frame (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`, one every 100 ms) while a task
  runs or MCP servers start, when animations are on;
- `[ ! ] Action Required` while the bottom pane needs the operator (an
  approval, a question, an elicitation, a plan prompt), alternating with
  `[ . ] Action Required` every second when title animations are on; the
  spinner is suppressed then;
- nothing while idle.

`● ` comes before it while the microphone listens. Codex drops control
characters, collapses whitespace, caps the title at 240 characters, and
writes ASCII fallbacks (`| / - \` for the spinner) only under GNU screen
(`STY` set; tmux's own `TERM=screen` does not count). tmux keeps the
last title a pane set as `#{pane_title}` (`allow-set-title`, on by
default). The rest of the title is the thread name, often the
operator's own words, and the project name: text that may carry payload.

The mux probe already runs `tmux list-sessions -F <format>` against each
server every mux cycle (5 s with a daemon), one row per session
describing its active pane (ADR 0006, ADR 0140). Claude Code prefixes
its title with a status glyph too, but which glyph means which state is
unverified. None of the strings above has been checked against a
running Codex yet; they come from its source and the research notes.

## Decision

1. **What is read.** The probe's existing `list-sessions` format gains
   `#{pane_id}` and `#{pane_title}` at its end: the id and the title of
   each session's active pane. No other pane's title, no pane content or
   scrollback, no further tmux call, process, or timer. Setting
   `CONSPECTUS_DISABLE_PANE_TITLES` leaves `#{pane_title}` out of the
   format, so the title is not read at all.

2. **Whose titles.** Only a pane whose program is a harness that
   classifies its titles, judged by the evidence the hook sidecar uses to
   tell which harness a pane runs: `#{pane_current_command}` matched
   against each harness's process names (today `codex` or `codex-rs`).
   A harness adapter opts in through one method; the default classifies
   nothing. Every other pane's title is dropped where the row is split,
   unclassified. Claude Code gets no classifier until its glyphs are
   recorded from a real session: this decision asserts no Claude state.

3. **Classification, in memory.** The parser hands the title to the
   adapter the moment it splits the row and keeps only the result, one
   of three marks:

   | Mark | Codex title (after an optional `● `) |
   |---|---|
   | `waiting` | begins with `[ ! ] Action Required` or `[ . ] Action Required`, then the end or a space |
   | `working` | begins with a braille spinner frame, then the end or a space |
   | `neither` | anything else |

   `neither` covers an idle title (`fix-auth | conspectus`), an empty
   one, tmux's default host name, a shell's or the operator's own title,
   the screen fallback frames (too like ordinary titles, and only under
   GNU screen), and a title that contains a spinner glyph or the words
   `Action Required` anywhere but at its start. The classifier reads
   only the leading activity segment.

4. **Tier.** The title is Tier 2 content (ADR 0086): read only for the
   panes of decision 2, under this decision and its switch; capped to
   the activity segment; never shown, persisted, logged, or put in an
   error, a diagnostic, the store, `graph.bin`, or JSON; never fed to
   attribution or resolver evidence. Only the mark leaves the parser,
   and the mark is an enum like any Tier 3 field. A sentinel test plants
   a unique string in titles and finds it nowhere.

5. **What it asserts.** `waiting` with detail `input` (the title cannot
   say permission, question, or elicitation) and `working` with no
   detail. Nothing else: never `ready`, `error`, `compacting`,
   `starting`, or `exited`. `neither` asserts nothing, because the same
   plain title shows while idle, while MCP servers start without
   animations, and while working with animations off. Its absence is no
   evidence from this source, never `ready`.

6. **What the probe records.** Per listed session with a mark, the
   probe (`MuxProbe.titles`, keyed by session name) records the pane id,
   the mark, when the probe ran (`observed_ms`), and the mark's run:
   when the first of the consecutive probes that read this mark in this
   pane ran (`since_ms`), and when the probe before this one ran, if it
   read the same (`previous_ms`). The run is carried from one probe to the next in the discovery caches of
   the process that probes (the daemon, a daemonless TUI), never in a
   file; a probe that fails or no longer lists the session, or a new
   pane or mark, starts a new run. The marks ride in `graph.bin` with the
   probe and stay out of JSON, as the probes do.

7. **How it combines with hook evidence.** The live-state binding (ADR
   0205 decision 3) reads the marks after it has bound the store, so the
   discovery mutator and the TUI's overlay apply the same rules to the
   same snapshot. For a live tmux mux whose probe recorded a mark, the
   pane's *hook entry* is the bound store entry of the pane's harness
   whose tmux names the mux's server and session and the pane (or no
   pane), that has not recorded `exited` and whose process was not found
   dead. Read at the probe's time:
   1. **No hook entry**: a `waiting` or `working` mark binds as a
      pane-only entry for the pane (`pane_live`), source `pane_title`,
      since the run began, observed at the probe, with no pid, so its
      liveness is unknown and the 15-minute rule of ADR 0203 ages it out
      if probes stop. It replaces an entry an exited agent left in that
      slot. `neither` binds nothing.
   2. **A fresh hook entry wins** (confidence `fresh`, in any state but
      `unknown`), apart from rule 4.
   3. **A quiet, stale, or `unknown` hook entry gives way** to a
      `waiting` or `working` mark in its own slot (session or pane),
      source `pane_title`, observed at the probe. It keeps the hook
      entry's process, liveness, and subagent count, and its start when
      the state is the same. `neither` leaves it as it is.
   4. **The one carve-out: an answered approval.** When the hook entry
      is a fresh `waiting` and the pane's title has read the same mark
      other than `waiting` on two consecutive probes, both after the
      hook entry's latest evidence, the binding shows `working` (source
      `pane_title`, since the first probe of that run after the hook
      entry's evidence) in its place. Two probes keep a probe that lands
      between the hook's write and Codex's redraw from clearing a fresh
      wait; counting only probes after the hook entry lets an approval
      answered before any probe saw the marker clear too. Both forms of
      the blinking marker read `waiting`, so the blink never counts. The store's entry is not rewritten, and the next hook event
      (a tool ending, the turn ending) takes over again.

   A hook entry's `ready`, `error`, `compacting`, `starting`, and
   `exited` are never changed by a title. A title never binds to a
   session the store did not name: it has no session id, and the
   binding does not guess one.

8. **Cadence and latency.** The source rides the probe: every mux class
   cycle (about 5 s) with `conspectus serve`, at each refresh (30 s) of a
   daemonless TUI, and once per one-shot command, which has no earlier
   probe, so the carve-out needs a daemon or a running TUI. The TUI's
   one-second overlay binds the store again against the marks its
   snapshot holds and never asks tmux.

9. **Where it shows.** Bindings from this source say `source:
   pane_title` and otherwise read like any other: the row cell, the
   header's and group headers' counts, `W`, the detail pane, `node
   show`, `table --columns +live`, and the graph JSON. The health line
   of `hook status` for a harness whose titles are read adds how many of
   its agents the saved graph shows from their pane titles, so a Codex
   whose hooks are untrusted reads `silent` and `1 agent shown from its
   pane title`.

10. **Capabilities.** `list-sessions ...` is already a sanctioned tmux
    read. The manifest's tmux entry cites this ADR; no entry is added.

## Consequences

- A Codex with no trusted hooks shows `!` while it waits on the operator
  and `»` while it works, on its mux row (pane-only, since the title
  names no session), in the header count, and for `W`.
- A wait the hooks report clears within about two probes of the operator
  answering it instead of when the approved tool ends.
- With hooks reporting a fresh `working`, a wait no hook reports (an MCP
  elicitation, a sign-in, a question) still reads `working` until the
  hook entry goes quiet after ten minutes, when the title's `waiting`
  takes over. Raising a fresh `working` to `waiting` on the title alone
  would need a second carve-out to ADR 0207 decision 4; it is left to
  the operator.
- Only a session's active pane is read: a Codex in another pane of a
  split window, or in a window that is not current, gives this source
  nothing.
- A Codex started through a wrapper whose process the pane reports
  (`node` for an npm install) is not recognized as Codex, as for the
  hook sidecar.
- The source goes silent with a custom `tui.terminal_title` without the
  activity item, a `null` title, title animations and the progress
  effect both off (`working` then reads `neither`), or `allow-set-title
  off`. A crashed Codex leaves its title behind, but the pane's program
  is then the shell, so the title is no longer read.
- A thread whose name starts with the marker or a braille glyph reads as
  that mark while idle. The title cannot tell them apart.
- `graph.bin` moves to format version 13; older files cold-rebuild.
- The strings come from Codex's source, not from a recorded session.
  If a recording shows otherwise, the classifier's table changes and
  nothing else does.

## Alternatives Considered

- **Every pane's title (`list-panes -a`).** Covers split panes and
  background windows, at the cost of a second tmux call each cycle and
  of reading the titles of panes that run no agent. Deferred until a
  split-pane Codex is common enough to pay for it.
- **A `display-message` per Codex pane.** One process per pane per
  cycle for what the existing call returns. Rejected.
- **A field on `MuxSessionNode`.** The node is identity; a mark that
  changes every few seconds would churn it and the process-tree
  fingerprint (ADR 0205 alternative A). Rejected for the probe.
- **Keeping the title, or a hash of it.** Tier 2 text in a rebuildable
  file, and a short title's hash is guessable. Rejected.
- **Binding a title-only state to the Codex session its mux hosts.** It
  needs the resolver's mux attribution, which the mutator and the
  overlay see at different points, and a guess at which session the
  active pane runs. Pane-only keeps the evidence where it was seen.
- **Demoting a wait on one probe, or a fixed time after the hook.** One
  probe races the redraw; a time needs a clock in the binding, which
  takes none. Two probes instead.
- **A plain title means ready.** The same title shows while starting and
  while working without animations. Rejected (ADR 0203: no evidence is
  not idle).
- **Opt-in only.** The operator approved the source; the environment
  switch turns it off without a config table.
- **Claude Code's glyph now.** Unverified vocabulary. Deferred to a
  recording.

## Open Questions Answered

- *Is this terminal scraping?* No. It reads one tmux format field the
  harness publishes as its status, the text the terminal's title bar
  already shows, not pane content.
- *Does the source write anything?* No. The store, the spool, and the
  history are untouched; the marks live in memory and in the snapshot.
- *What does `[ . ] Action Required` mean?* The hidden phase of the
  marker's blink: still `waiting`.
