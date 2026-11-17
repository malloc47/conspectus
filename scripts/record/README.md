# Recording real sessions

The live-state reducers (ADRs 0204 to 0206, 0217, 0218) rest on what each
harness sends its hooks, and the documentation of a harness that ships every
week is only a guess about it. This driver runs a harness for real, in a
throwaway sandbox, with the hook trace on (ADR 0242), so the sequences it
sends can be read against the reducers again after a harness update. It
produced the recordings in `tests/fixtures/hooks/recorded/` (ADR 0243).

**It is manual.** It spends the model quota of the account the harness is
signed in to, so it is never run by CI, by `just check`, or by any test, and
it refuses to start without an explicit yes: it prints what it will use and
asks you to type `record` (or take `--yes`), and without a terminal it stops.

```sh
just record claude-code --dry-run     # what it would use and create; starts nothing
just record claude-code               # asks first
just record codex --yes               # no question, for a script you wrote
RECORD_FAST=1 just record pi          # skip the waits of a minute or more
```

`just record HARNESS [options]` is `scripts/record/run.sh [options] HARNESS`.
HARNESS is `claude-code`, `codex`, or `pi`. OpenCode is not covered.

## What a run does

| Harness | Prompts | Steps |
| :- | -: | :- |
| `claude-code` | about 24 | a turn and the idle notification; a permission prompt denied; a long approved tool; an Esc interrupt; a question; a subagent and a background subagent; `/compact`; `/clear`; a plan rejected and approved; a closed pane; `kill -9`; `StopFailure` for a 401, a 429, and a 500 against a local stand-in for the model gateway |
| `codex` | about 14 | an approval declined and granted; an Esc interrupt; `/clear`; `/compact`; plan mode; a question; a subagent whose command needs approval; a terminal UI killed and one whose pane is closed (the shared app server ends them a minute later); `--no-daemon` and `kill -9` |
| `pi` | about 8 | a turn; a tool run; an Esc abort; `/compact` of a session too small to compact; `/new`; `/reload`; `/quit`; a closed pane; `kill -9` |

At each step it samples the stored live state and the pane title (Claude Code
also its own `claude agents --json` list, Codex a log of every title change), and
snapshots the screen at the moments that matter. A run takes 10 to 25 minutes;
`RECORD_FAST=1` drops the waits that exist only to see a notification arrive.

It writes to `record-out/<time>-<harness>/` (set with `--out` or `RECORD_OUT`;
the directory is ignored by git):

- `traces/<session>.jsonl`: the hook traces, one JSON line per hook call, as
  `conspectus hook write` redacted them (every free-text value is a size, every
  id a pseudonym, every path `<path>`);
- `summary.txt`: a table of each trace: the event, what the adapter made of it,
  and the state the reducer reached;
- `<session>/NNN-label.txt`: screen snapshots, and `timeline.tsv`, the steps and
  samples with their times. **These are not redacted.** They hold the prompts
  and replies of the run, which are a few words each, and paths of the scratch
  directory. Read them; do not commit them.

To keep a recording as a fixture, check a trace against `summary.txt` and the
screens, then copy it under `tests/fixtures/hooks/recorded/<harness>/`
(its README says how they are named and what each holds).

## What it touches

- **Reads**, never writes back: the harness's configuration (`~/.claude`,
  `~/.codex`, `~/.pi/agent`, or where `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, and
  `PI_CODING_AGENT_DIR` move them), copied into the scratch directory. The
  hooks in the copies are pointed at the `conspectus` the run uses, so the
  installed one, and its link, are not changed. Claude Code's plugin list is
  dropped from the copy; every Pi extension but Conspectus's is linked in
  (`RECORD_PI_SKIP="name ..."` leaves some out), and Conspectus's is this
  checkout's `plugins/pi-hook/conspectus-hook.ts`.
- **Creates**: `$RECORD_TMP/conspectus-record.XXXXXX` (default
  `$XDG_RUNTIME_DIR`, else `/tmp`): a private tmux server with its own socket, the
  configuration copies, a throwaway project directory, and a scratch
  `XDG_STATE_HOME`, so **your own live-state store is not written**. It is
  removed when the run ends (`--keep` or `RECORD_KEEP=1` keeps it).
- **Runs** the harness, with the model and account its configuration names. The
  `StopFailure` steps point a copy of Claude Code's configuration at a local
  server (`stub-gateway.py`) that answers every request with an error; the
  harness puts its credential in the headers it sends there, and the stand-in
  ignores them and logs nothing.
- **The `conspectus` used**: `RECORD_CONSPECTUS`, else `target/release/conspectus`
  of this checkout, else the one on `PATH`. It must be a build that has the hook
  trace; the run checks first and stops if it does not.

## Why it is bounded, and why the environment is clean

Two hazards found while building it are part of the tool, not of anyone's memory.

**Memory.** A pty tool fed from an endless source (`script` reading
`/dev/zero`) grew to 7 to 9 GB in seconds on a machine whose swap was full, was
killed by the OOM killer, and, because tmux pane scopes run with
`OOMPolicy=stop`, systemd then stopped the whole scope: the session that had
started it, and everything in it, went. So `run.sh` starts every run in its own
`systemd-run --user --scope` with a memory cap (`RECORD_MEM`, default 3G), no
swap, `OOMPolicy=continue`, and a wall-clock limit (`RECORD_SECS`, default 900), and
refuses to run without `systemd-run`. Nothing here gives a pty tool, or anything
else, an unbounded input: no `script`, no `/dev/zero`, no `yes`. A test checks
that of every script in this directory.

**Environment.** A harness started from inside another harness inherits its
session variables (`CLAUDE_PID`, `CLAUDECODE`, `TMUX`, `TMUX_PANE`), and the hooks
then report the wrong process and pane. Each harness here starts under `env -i`
with only `HOME`, `PATH`, `TERM`, `LANG`, `USER`, `XDG_RUNTIME_DIR`, the scratch
`XDG_STATE_HOME`, the trace's name, and the private tmux server's own `TMUX` and
`TMUX_PANE`. Every process of a run carries `RECORD_SCRATCH=<scratch directory>`,
so the cleanup can find and end all of them, a background supervisor included.

## Writing or changing a scenario

The scenarios are `claude-code.sh`, `codex.sh`, and `pi.sh`; `lib.sh` has the
pieces (`rec_start`, `rec_ask`, `rec_wait`, `rec_snap`, `rec_sample`,
`rec_settle`, `rec_find_pid`, and the `rec_init_*` copies). What cost the most
time while this was written:

- **Wait for text the harness draws, never for text you typed.** A pattern
  such as `red|Red` matches your own prompt at once, and the next keys land in
  a dialog that is not open yet. `rec_ask` clears the input line first (an Esc
  gives the interrupted prompt back), but it cannot know a dialog is open.
- **A step that drifts still records.** The trace shows what the harness did
  anyway. Say so in the fixtures' README rather than hiding it.
- **Codex runs every session's hooks in one app server**, in the environment of
  the first terminal UI, so the first session (`cx`) stays open and names the
  trace for all of them. `--no-daemon` sessions have their own.
- **Waits on the model can time out.** A plan or a question a prompt asks for
  is the model's to produce, and a run saw it take two minutes or answer in
  words instead. The step prints `no plan prompt` (or the like) and the run
  goes on; the trace shows what happened.
- **Keep prompts to a few words.** The quota is not yours alone.
- `claude --bg` (background sessions, which start a supervisor) is not part of the
  scenarios; `claude agents --json` of one was recorded by hand for
  `tests/fixtures/claude-agents/`.

`summarize.py` prints a trace as a table (`summarize.py TRACE.jsonl`) and reads
the store, Claude Code's session list, and the process tree for the scenarios.
