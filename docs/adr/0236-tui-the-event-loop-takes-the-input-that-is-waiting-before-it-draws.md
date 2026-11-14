# ADR 0236: The Event Loop Takes The Input That Is Waiting Before It Draws

## Status

Accepted. Adds to the loop of [ADR 0024](0024-tui-runtime-and-architecture.md)
(decision 2, the event loop in `runtime.rs`) and
[ADR 0085](0085-tui-mvu-architecture.md) (contract 5, one loop for every
mode): one pass of the loop handles a batch of input, not one event. Adds
no dependency, process, effect, key, or write (ADR 0143), and changes no
rendering.

## Context

Scrolling the TUI quickly over mosh stalled it for seconds, and it kept
moving after the operator stopped. The TUI asks for no mouse mode, so a
terminal turns each wheel event into cursor keys: Ghostty sends three per
wheel tick by default, more as macOS speeds up a fast spin, and a trackpad
sends one per cell height of pixels. mosh delivers every key, in order.
Conspectus moves one row per cursor key, so the rows per notch are the
terminal's setting.

The loop read one event per pass and drew a full frame at the top of the
next, so a backlog of N cursor moves cost N frames. At 184 by 85 a frame
is 3.5 to 6 KB of output and takes 5 to 7 ms, so the loop tops out near
140 to 200 keys per second, below what a fast scroll produces. The excess
queued up and the screen worked through it after the scroll ended. On the
operator's data in a release build, a sweep of 900 cursor keys at 300 per
second, typed into a pty at 184 by 85, kept drawing for 1.0 to 2.6 s after
the last key (the same stream over mosh with 80 ms of round trip and a
2 Mbps cap left the screen changing for 1.6 to 2.4 s).

Only the last frame of a backlog is ever seen: the frames in between are
overwritten by the next one before the operator could read them.

## Decision

1. **A pass draws once per batch.** After the wait for input, the loop
   handles the event that arrived and every event already queued behind
   it, and only then goes back to drain, plan the preview, and draw. The
   loop asks the terminal whether another event is queued with a zero
   timeout; it never waits for one inside a batch.
2. **Every event is handled as if it were alone.** Each one resolves
   through `action_for_event` against the app as the event before it left
   it, and dispatches in arrival order. A key that opens a modal is
   followed by keys the modal handles; a bracketed paste types into the
   focused field, character by character, as it does when it arrives
   alone; an `r` or a hard refresh anywhere in the batch asks for the
   forced preview reload of the pass after it. A batch leaves the app in
   the state the same keys leave when they arrive one at a time; only the
   frames between them are skipped.
3. **A batch is bounded.** It ends when the queue is empty, when the app
   quits (the keys after the quit are never read), or when `FRAME_BUDGET`
   (50 ms) has passed since its first event. A stream that never pauses
   therefore still draws every 50 ms plus the cost of a frame, well inside
   100 ms, so a long scroll shows progress instead of freezing, and an
   action that takes long (an attach, a launch) ends the batch the moment
   it returns.
4. **The loop reaches the terminal through `Console`.** `Console` has
   four methods: `poll`, `read`, `draw`, and `now`. The real terminal
   implements it, and a mode names the console it runs on
   (`LoopMode::Terminal`). Tests run the real `drive_loop` over a scripted
   console whose events arrive at times a script gives, whose clock moves
   only when the loop waits or a test spends time, and which keeps every
   frame, so the counts and the 100 ms bound are asserted without
   sleeping (`src/tui/runtime/loop_tests.rs`).

## Consequences

- A backlog of cursor keys costs one frame when the loop finds it queued.
  On the operator's data (release build, 184 by 85, the same 900-key
  stream), the screen settles 6 to 14 ms after the last key, down from
  1.0 to 2.6 s; a burst of 180 keys written at once settles at once, down
  from 0.7 to 1.0 s.
- A key that arrives alone still draws at once: nothing waits to gather a
  batch, so typing latency is unchanged.
- The reducer still runs once per key. The saving is the frame, which is
  the cost that grows with the screen's area; the reducer's per-key work
  is small next to it.
- Work the loop does once per pass (draining discovery results, the live
  overlay's tick, planning the preview) now runs once per batch, up to
  50 ms apart in a sustained stream instead of after every key.
- A test of the loop needs no terminal, so later changes to its timing
  (ADR 0237) are tested with the same console.

## Alternatives Considered

- **Draw at most every N ms.** A fixed frame rate delays the first key
  of a quiet stream by up to a frame interval and keeps frames the
  operator cannot see when the loop is idle. Batching draws exactly when
  there is input to show and only skips frames when input is waiting.
- **Fold consecutive cursor keys into one move with a count.** It changes
  what a key means (a modal, a paste, or `r` between them would need a
  rule) and puts terminal-specific knowledge in the keymap. Handling every
  key keeps one meaning per key.
- **Draw on another thread.** `Terminal` and the app's state are not
  shared between threads, and the reducer is single-threaded by design
  (ADR 0024 decision 3); the frame is also the thing to skip, not to
  overlap.
- **Turn on mouse capture and read wheel events.** Whether the TUI asks
  for a mouse mode is the operator's open decision; the cursor keys the
  terminal sends today need this fix with or without it.
- **No budget (take everything queued).** A stream that outruns the
  reducer would never draw.

## Open Questions Answered

- *Why 50 ms?* A frame at 184 by 85 costs about 7 ms, so a 50 ms budget
  leaves the loop drawing more than ten times a second under a stream it
  cannot keep up with, and keeps the longest gap between frames, with the
  frame itself, under the 100 ms the poll timeout already allows.
- *Does a batch change which key does what?* No. The keys are resolved
  one at a time against the state the previous one left (decision 2).
