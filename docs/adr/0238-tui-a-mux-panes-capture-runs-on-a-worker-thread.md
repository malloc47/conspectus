# ADR 0238: A Mux Pane's Capture Runs On A Worker Thread

## Status

Accepted. Carries out [ADR 0112](0112-node-specific-previews.md) section 5
("sources that touch the system load off the UI thread") for the one source
that did not, and applies the concurrency model of
[ADR 0024](0024-tui-runtime-and-architecture.md) decision 3 (threads over
`mpsc`, no async runtime) to it. Adds no dependency, program, capability
entry, effect, key, or write (ADR 0117, ADR 0143), and changes no
rendering.

## Context

The live loop loaded every preview on a worker thread except a mux pane's.
A capture ran `tmux display-message` and `capture-pane -p -J -e -S -100`
on the UI thread, before the frame drew. On the operator's tmux server one
capture takes 5 to 10 ms, so each cursor move onto a live mux row held up
input for that long, and scrolling the mux view stacked those waits on top
of the frame cost. The call also has no timeout, so a tmux server that
stops answering froze the whole TUI, not only the preview: with a `tmux`
that took six seconds to capture, a key pressed with a capture under way
produced no output for the rest of the six seconds (3.2 s measured).

## Decision

1. **A capture runs on a worker thread.** The live loop holds its mux
   backend as an `Arc<dyn MuxBackend>` (the trait is `Send + Sync`, and
   the content search already shares one the same way), and
   `PreviewLoads::start` captures through `capture_via` on a thread, as it
   reads the other sources. The result comes back over the same channel
   and reaches the reducer as `Msg::SetPreview` when the loop drains it:
   preview state changes only through the reducer, and the capture still
   runs through the mux backend.
2. **The pane's target is found on the UI thread, the capture is not.**
   The native id comes from the snapshot, which needs no call. A mux with
   no pane to capture is answered `NoTarget` at once, with no `tmux` call
   and no thread.
3. **At most one capture per pane is in flight.** `PreviewLoads` records
   the panes it is capturing and ignores a request for one of them, beside
   the loading mark the planner already honors, so a request that
   arrives while a capture runs (`r`, the cursor returning to the row)
   waits for it.
4. **A capture is never stopped, and a late one is kept.** It is a single
   `tmux` call, not a sequence (ADR 0237 stops a load between commands). A
   capture that lands after the cursor has left its row is stored under
   its own pane, and the Preview shows the pane the cursor is on, so it
   replaces nothing on screen. A pane captured before keeps showing its
   last capture until the new one lands, and a pane never captured shows
   the loading placeholder, as every source does.
5. **No timeout is added.** A `tmux` that never answers leaves its pane on
   its last capture, or the placeholder, and its worker thread waiting;
   every other key and frame is unaffected, and `r` cannot start a second
   capture for that pane while one waits. A timeout belongs to the process
   layer (`exec::command`), where it would bound every call at once; it is
   left to a decision of its own.

## Consequences

- Selecting a mux row never runs `tmux` on the UI thread. With a `tmux`
  whose capture takes six seconds, a key pressed during it produced output
  3.2 s later before and 1 ms later now (release build, private `tmux`
  server, numbers only).
- A pane's preview appears when its capture lands, a few milliseconds
  after the cursor rests on it, and the row shows the placeholder or its
  last capture until then.
- A worker thread per capture, one per pane at most, which is the same
  cost the other sources already carry.
- The executor's `MuxOp::CapturePreview` is still what the scenario TUI and
  the default loop mode run, on their own thread; only the live loop
  moves the capture.

## Alternatives Considered

- **One long-lived capture thread fed by a queue.** Its queue would hold a
  stuck capture's successors behind it, so one pane that stops answering
  would stall every other pane's preview. A thread per pane isolates them.
- **A timeout on the call.** Worth having for every `tmux` call, and
  larger than this story: it changes the process layer and its manifest
  (ADR 0117).
- **Capture on the UI thread but only on the rested row** (ADR 0237).
  That cuts the count, not the stall: the capture would still hold up the
  frame, and a stuck server would still freeze the TUI.
- **An async runtime.** ADR 0024 decision 3 rejects it for work this
  coarse.
