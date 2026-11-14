# ADR 0237: A Preview Loads Once The Cursor Rests On Its Row

## Status

Accepted. Amends [ADR 0112](0112-node-specific-previews.md) section 5
(a source loads "when the selection lands on it"): it loads once the
cursor has rested there. Adds to the cancellation of
[ADR 0024](0024-tui-runtime-and-architecture.md) decision 3 (a stale
result is dropped when it arrives): a load also stops starting
subprocesses once the cursor has left its row. Adds no dependency,
process, effect, key, or write (ADR 0143), and changes no rendering.

## Context

Every cursor move started a preview load for the row it landed on, even
when the cursor was only passing through, and the load kept going after
the cursor moved on. A repo row runs four git commands (`symbolic-ref`,
`remote -v`, `rev-parse`, and `log -n 30`), a checkout row runs `git
status`, a session row reads its transcript, and a pull request runs
`gh`. A held cursor key (about 25 a second) or a mouse wheel turned into
cursor keys by the terminal therefore started loads for every row it
crossed. On the operator's data, in the sessions view, a sweep of 90
cursor keys at 25 a second started 174 git commands (60 to 72 at 200 a
second, where keys share a pass), all for rows the operator only scrolled
past. Those processes compete for CPU with the UI thread just when it is
working through a scroll backlog (ADR 0236), and the stall is worst on a
loaded machine.

The content search already waits for its query to rest before it scans
(`search_overlay_tick`).

## Decision

1. **A load starts once the cursor has rested for 100 ms.** Each pass
   of the loop resolves the source the Preview shows
   (`preview_source`). When input has just changed it, the cursor is
   passing through and nothing loads; once it has stayed for `REST`
   (100 ms) the usual rules of `plan_preview_load` decide what loads
   (nothing cached, the selection moved onto the source, stale content,
   `r`). The loop's wait for input shortens to the time left until the
   rest, so the row the cursor stops on starts loading about 100 ms after
   the last key.
2. **Only a cursor moved by input waits.** A source that changes with no
   key behind it (the first snapshot landing, a refresh that changes
   what the row stands for) loads at once. The rest is measured from the
   pass that handled the input, which is after the whole batch of keys
   (ADR 0236).
3. **`r` and a hard refresh load the selection at once**, rested or
   not: the operator asked for it.
4. **What shows meanwhile is unchanged.** A row passed shows its cached
   preview if it has one and the loading placeholder otherwise. What the
   selection "last showed" (the `shown` source ADR 0112 compares against)
   advances only when a pass was allowed to load, so a cursor that
   returns to the row it left before resting does not reload it, and one
   that rests on another row loads that row, as before.
5. **A load stops when the cursor leaves its row.** Each load runs with
   a stop flag. The loop tells the live mode where the cursor is every
   pass, and every load running for another source is flagged. The git
   loaders ask the flag before each command and end `Stopped` without
   starting it. A stopped load stores nothing: it sends
   `Msg::PreviewLoadAbandoned`, whose reducer arm ends the source's
   loading mark and marks a cached entry stale, so the source loads again
   the next time the cursor rests on it. A command already running, a
   transcript read, and a `gh` call are not interrupted: they run to the
   end and their result is stored, since it is complete.
6. **Unchanged.** A load in flight for a source is not started again, a
   loaded pull request is reused until the forge interval or a hard
   refresh (ADR 0116), `--no-live-preview` still loads nothing, and a
   mux pane is captured as before (its own story moves that off the UI
   thread).

## Consequences

- On the operator's data (release build, sessions view, 184 by 85), a
  sweep of 90 cursor keys that returns to its row starts no git command
  (174 before at 25 keys a second; 60 to 72 before at 200 a second), and
  one that ends on another repo row starts that row's four.
- The Preview of a row the cursor stops on appears about 100 ms later
  than it did when the cursor stopped, and a key that moves the cursor
  one row and waits shows its preview after the same 100 ms. A held key or
  a wheel scroll shows the placeholder or the last cached preview for the
  rows it crosses, which it never read before either.
- A load that was half done when the cursor left leaves the row's cached
  preview (if any) marked stale and shows no partial result.
- The loop wakes at the rest deadline as well as at its 100 ms timeout;
  an idle loop is unchanged.

## Alternatives Considered

- **A longer rest, like the content search's 200 ms.** The Preview would
  lag a stopped cursor by a visible fifth of a second, and the story
  asked for a load within 150 ms of the last key. A held key repeats at
  about 30 ms, well inside 100 ms.
- **Only stop loads, still start one per row.** Each row would still
  start its first command; the processes are the cost.
- **Kill the running subprocess.** Signalling a child needs a handle on
  it and a new capability entry (ADR 0117); asking before each command
  needs neither, and a command lasts milliseconds.
- **Rest measured from every source change, input or not.** The first
  preview after startup, and after a snapshot, would wait for nothing.
- **Drop the result only.** The status quo: the processes still run.

## Open Questions Answered

- *Why 100 ms?* It is longer than the gap between a held key's repeats
  and a wheel's keys, and short enough to leave room (the frame and the
  loop's wake) inside the 150 ms the preview should take to start.
- *Does a snapshot that lands while the cursor moves load early?* Only if
  it changes the source under the cursor, and then once, for the row the
  cursor is on.
