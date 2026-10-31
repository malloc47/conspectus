# Evaluation: Extracting The Application Runtime (Layer 3)

Story CSP-765, measured at commit `1aca4148` (2026-10-31). ADR 0150
decision 2 kept the application runtime (the MVU loop, the effect lifecycle,
undo, and terminal hand-off) in Conspectus and asked for this evaluation once
layers 1 and 2 had moved. ADR 0200 records the outcome.

**Recommendation: LATER.** Do not extract the runtime now. Extract it when a
second program is committed to run on `tuikit` with an interactive loop of
its own and needs a terminal hand-off, background work that reports back, or
a headless mode. Start with the terminal guard.

- 643 of the runtime's 5,611 classified lines are generic (11.5%). The rest
  is Conspectus: discovery workers, daemon IPC, the effects and their
  executors, launches and attaches, preview loading, and fixture modes.
  Counting the reducer and the `App` state, 724 of 11,751 (6.2%).
- There is no second consumer. `examples/pantry.rs` runs on `tui-pantry`'s
  loop, and the one planned candidate, CSP-342's standalone transcript
  viewer, is unscheduled and would need about 30 lines of loop.
- An extraction is about nine stories (three large), changes the framework's
  founding boundary and its terminal-ownership guard, and its riskiest parts
  (terminal hand-offs, worker threads) are covered only by the PTY
  end-to-end tests.
- The largest duplication left in the runtime is three copies of the
  key-to-handler dispatch over the keymap's `Action` (257 lines). Removing
  them is host work that pays without any extraction; it is filed as
  CSP-802.

## 1. What Was Measured

Every line of the runtime's files is in one of three classes:

- **Generic (G):** would move to the framework, or be replaced by a framework
  piece, unchanged in meaning. It knows nothing of the graph, muxes, pins,
  harnesses, or the daemon.
- **Domain (D):** knows the graph model or Conspectus's processes, stores,
  and workers, and stays whatever happens.
- **Wiring (W):** module docs, imports, `mod` lines, and re-exports, which go
  with whichever side they serve. Left out of the percentages.

Mixed items were split by line, not rounded to the item: `run_loop` is 35
generic lines (the poll interval, the quit check, drain then draw then poll,
paste, dispatch) and 15 domain lines (the preview refresh, the content-search
tick, the shutdown persist). A domain-typed item whose *shape* is generic (the
lifecycle classifier) counts as domain; section 2.3 lists those. The
classification and its line ranges are a throwaway script outside the
repository; the files and items named below let anyone re-check it.

Scope: the runtime proper is `src/tui/runtime.rs` and `src/tui/runtime/`,
`effect.rs`, `undo.rs`, `snapshot.rs` (the headless driver), and `keymap.rs`
(the input path). The reducer and its state (`app.rs`, `app/`: `Msg`, `App`,
`App::update`) are measured separately: they are domain by definition, and
counting them with the runtime would hide its own proportions.

## 2. Generic Versus Domain

### 2.1 By file

| File | Lines | Generic | Domain | Wiring | Generic share |
| --- | ---: | ---: | ---: | ---: | ---: |
| `src/tui/runtime.rs` | 2,258 | 287 | 1,900 | 71 | 13.1% |
| `src/tui/runtime/term.rs` | 70 | 51 | 0 | 19 | 100% |
| `src/tui/runtime/launch.rs` | 722 | 46 | 670 | 6 | 6.4% |
| `src/tui/runtime/executor.rs` | 792 | 24 | 754 | 14 | 3.1% |
| `src/tui/runtime/overlay_keys.rs` | 126 | 31 | 81 | 14 | 27.7% |
| `src/tui/runtime/content.rs` | 413 | 0 | 380 | 33 | 0% |
| `src/tui/runtime/visibility.rs` | 92 | 0 | 75 | 17 | 0% |
| `src/tui/runtime/worktree_exec.rs` | 220 | 0 | 215 | 5 | 0% |
| `src/tui/effect.rs` | 390 | 9 | 367 | 14 | 2.4% |
| `src/tui/undo.rs` | 98 | 29 | 50 | 19 | 36.7% |
| `src/tui/snapshot.rs` | 504 | 145 | 307 | 52 | 32.1% |
| `src/tui/keymap.rs` | 214 | 21 | 169 | 24 | 11.1% |
| **Runtime proper** | **5,899** | **643** | **4,968** | **288** | **11.5%** |
| `src/tui/app.rs` | 1,836 | 5 | 1,793 | 38 | 0.3% |
| `src/tui/app/overlays.rs` | 968 | 76 | 881 | 11 | 7.9% |
| `src/tui/app/`, the other nine files | 3,521 | 0 | 3,385 | 136 | 0% |
| **Reducer and state** | **6,325** | **81** | **6,059** | **185** | **1.3%** |
| **Runtime plus reducer** | **12,224** | **724** | **11,027** | **473** | **6.2%** |
| `src/tui/runtime_tests.rs` | 2,174 | 156 | 2,012 | 6 | 7.2% |
| `src/tui/runtime/*_tests.rs` (four files) | 795 | 0 | 766 | 29 | 0% |
| `src/tui/snapshot_tests.rs` | 418 | 48 | 368 | 2 | 11.5% |
| `src/tui/consistency_tests/` `lifecycle.rs`, `undo.rs`, `freshness.rs` | 659 | 0 | 626 | 33 | 0% |
| `tests/terminal_hygiene.rs` | 95 | 78 | 0 | 17 | 100% |
| **Tests** | **4,141** | **282** | **3,772** | **87** | **7.0%** |

Not in the table: the reducer's own tests (`app_tests.rs`, 3,672 lines, and
`app/*_tests.rs`, 1,986), which call `App::update` directly and do not touch
the loop, and `tests/tui_tmux_e2e.rs` (1,455 lines, 32 tests), the only tests
that run the live loop, in a real terminal under private tmux servers.

### 2.2 The generic code, by concern

| Concern | Where | Lines |
| --- | --- | ---: |
| The loop: the 100 ms poll, the quit check, drain then draw then poll, and the per-mode trait (`init`, `drain`, `dispatch`) | `runtime.rs` `run_loop`, `LoopMode` | 70 |
| Input and drawing: a key press goes to an open modal first, then the focused pane's keymap; a paste is typed only into a text field, flattened to one line; the expired toast goes, the frame is drawn, the body's report is kept; the entry that takes and returns the terminal | `runtime.rs` `draw_frame`, `draw_screen`, `action_for_event`, `type_paste`, `overlay_key_from_event`, `run` | 82 |
| The press filter in front of the keymap | `keymap.rs` `translate_in` | 21 |
| The terminal guard: raw mode, the alternate screen, bracketed paste, a panic hook that turns paste off | `term.rs` | 51 |
| The hand-off frame: leave, run the child, take the terminal back, clear; the clear before an external viewer and the wait for Enter after it fails; the same guard around the fixture entries | `launch.rs` (three sites), `runtime.rs` `run_static`, `run_from_fixture` | 59 |
| Work behind a spinner: a worker thread while the screen redraws every 50 ms, its panic re-raised on the UI thread | `runtime.rs` `run_with_spinner`, `run_with_terminal_spinner` | 50 |
| Effects as data: reduce then execute; a pure and a live executor; the lifecycle looked up per effect; an executor that cannot run an effect says so; the toast, copy, and quit effects | `runtime.rs` `execute_effects*`, `ExecutorKind`, `dispatch*`, `live_send`; `executor.rs`; `effect.rs` | 105 |
| Modal keys: route to the top entry, settle, send what it committed | `overlay_keys.rs` `modal_key` | 31 |
| A bounded undo history | `undo.rs` `UndoStack`, `UNDO_DEPTH` | 29 |
| The headless driver: the size check, the key script, a frame drawn before each key, the notes, one fresh frame, the region, ANSI out, the toast and status flags | `snapshot.rs` | 145 |
| **Runtime proper** | | **643** |
| App plumbing: lending the modal stack out, a key to it, whether it fills the screen or takes text, the top modal's hints, the toasts, quitting | `app/overlays.rs`, `app.rs` | 81 |

Resize handling is generic and takes no lines: a resize event resolves to no
action, and the next `Terminal::draw` resizes its buffers itself.

Of the 643 lines, a second program with no child processes and no background
work would use about 200: the loop, input and drawing, and the terminal guard.

### 2.3 Generic in shape, domain in content

The framework can document these patterns but not absorb them: each is
written over Conspectus's own variants or its hand-off rules.

| Pattern | Where | Lines |
| --- | --- | ---: |
| An exhaustive lifecycle classifier: every effect declares whether it attaches, freezes its row, refreshes, and follows its entity | `effect.rs` `Lifecycle`, `lifecycle` | 100 |
| An exhaustive undo classifier with a reason for each irreversible write | `effect.rs` `UndoClass`, `undo_class` | 35 |
| Which effects the pure executor runs, and which reach outside a snapshot | `runtime.rs` `pure_executor_runs`, `snapshot.rs` `reaches_outside` | 36 |
| Worker results where the newest generation wins and the first after a hand-off settles the frozen rows (ADR 0108) | `runtime.rs` `WorkerLedger`, `WorkerVerdict` | 54 |
| Key-to-handler dispatch over the 23 `Action` variants, three times: live, fixture, and snapshot | `LiveMode::dispatch` (92), `StaticMode::dispatch_action` (111), `snapshot::apply_action` (54) | 257 |

The tests that iterate every variant through a classifier
(`consistency_tests/lifecycle.rs`, `undo.rs`, `freshness.rs`, 659 lines) are
the same: the iterate-every-variant pattern is generic, the variants are not.

### 2.4 The domain code, by concern

`runtime.rs`'s 1,900 domain lines:

| Concern | Lines |
| --- | ---: |
| Discovery, daemon IPC, and refresh: daemon nudges, settle probes after a hand-off, the local rebuild, provider status | 501 |
| Live subscriptions and workers: the live mode's state and drain (four channels and the content driver), requested refreshes, the background forge fetch, the worker ledger | 405 |
| Openers and form seeding: the worktree menu, the mux forms and their known directories, Controls, the live-session advisory | 333 |
| Key-to-handler dispatch over `Action` (live and fixture) | 207 |
| Fixture and scenario mode: its mode, entries, fixture reads, the test driver | 162 |
| Preview loading: planning, live, recorded, offline | 141 |
| The live executor's arms, refresh requests, the executor mapping, content scans for snapshots | 80 |
| `Enter`'s default action, the hand-off guard, copying an id | 71 |

The other files: `executor.rs` (754: pin, rename, and worktree store writes,
the mux op, the exec specs, opening a transcript), `launch.rs` (670: `mux
new`, `mux launch`, rebuild and its plan, the attach routes per backend, the
external viewer), `content.rs` (380: the content search scans),
`effect.rs` (367: `Effect`'s eight variants, `ExecSpec`'s eight, `StoreOp`'s
18, `MuxOp`), `worktree_exec.rs` (215), `visibility.rs` (75: noticing the
TUI's tmux pane come back into view), `snapshot.rs` (307: fixtures, the
clock, seeded messages, `apply_action`, the scenario world), `keymap.rs`
(169: `Action` and `Enter`'s default), `undo.rs` (50: the two inverses), and
the reducer (`Msg`'s 102 variants and `App`).

### 2.5 What the framework already supplies

1,527 lines of `tuikit` already serve a host's loop: the overlay contract,
its outcome type, and `ModalStack` (596), `AppFrame` and `Screen` (230),
`Toasts` (218), the clipboard encoder (68), `testkit::render::FrameDriver`
(103), the key-script parser (187), and the ANSI writer (125); plus
`text_edit::single_line_paste`. Conspectus's draw is already `AppFrame` over
a `Screen` adapter (`ui::draw`), and its snapshot driver already presses keys
through `FrameDriver::drive`.

## 3. The Seam

### 3.1 Signatures

What the host provides: its model and reducer, its view as a `Screen`, its
keys, and one executor per mode. What the framework would own: the loop, the
input rules, ticks, resizes, the terminal guard with panic-safe restore,
suspend and resume, and the headless driver.

```rust
/// The program the runtime drives: the host's model.
pub trait Program {
    /// The host's message (`Msg`).
    type Msg;
    /// The host's effect (`Effect`).
    type Effect;
    /// What the body measured (`Screen::Report`), kept for the next key.
    type Report: Default;

    /// The reducer: pure, no terminal, no I/O.
    fn update(&mut self, msg: Self::Msg) -> Vec<Self::Effect>;
    /// One key press, as messages: the host routes it to its modal stack
    /// (`ModalStack::route`, `settle`) and then to its keymap. Only presses
    /// reach it; repeats and releases are dropped.
    fn key(&mut self, key: KeyEvent) -> Vec<Self::Msg>;
    /// Whether a text field has the keys (`ModalStack::takes_text`), so a
    /// paste is typed.
    fn takes_text(&self) -> bool {
        false
    }
    /// One frame, usually `AppFrame::render` over the host's `Screen`.
    fn view(&mut self, frame: &mut Frame<'_>) -> Self::Report;
    /// Keeps what the last frame measured (a page key moves by it).
    fn keep(&mut self, report: Self::Report) {
        let _ = report;
    }
    /// Before each frame: toasts retire (`Toasts::tick`), debounces fire.
    fn tick(&mut self, now: Instant) -> Vec<Self::Msg> {
        let _ = now;
        Vec::new()
    }
    /// Effects the state asks for without a message: a load the selection
    /// needs, a refresh a write left requested.
    fn pending(&mut self) -> Vec<Self::Effect> {
        Vec::new()
    }
    fn should_quit(&self) -> bool;
    /// After the loop, before the terminal is handed back.
    fn finish(&mut self) {}
}

/// Runs a program's effects: one per mode (live, fixture, headless).
pub trait Executor<P: Program> {
    /// Once, before the first frame.
    fn start(&mut self, exec: &mut Exec<'_, P>) -> Vec<P::Msg> {
        let _ = exec;
        Vec::new()
    }
    /// Before each frame: drain the executor's own worker channels and timers.
    fn poll(&mut self, exec: &mut Exec<'_, P>) -> Vec<P::Msg> {
        let _ = exec;
        Vec::new()
    }
    /// Whether this executor runs `effect`. One it cannot run is refused out
    /// loud, never dropped.
    fn can_run(&self, effect: &P::Effect) -> bool {
        let _ = effect;
        true
    }
    /// What a refused effect says (a warning toast, a snapshot note).
    fn refused(&mut self, effect: P::Effect) -> Vec<P::Msg>;
    /// Run one effect. Long work goes to a thread whose result comes back
    /// through `poll` or `Exec::inbox`.
    fn run(&mut self, effect: P::Effect, exec: &mut Exec<'_, P>) -> Vec<P::Msg>;
}

/// What an executor may touch while it runs.
pub struct Exec<'a, P: Program> {
    /// The model, borrowed: today's executors write into the App.
    pub program: &'a mut P,
    /// Where a worker thread sends a message for the next frame.
    pub inbox: &'a Sender<P::Msg>,
    // The live terminal, or the headless driver's frame and output buffer.
}

impl<P: Program> Exec<'_, P> {
    /// Hands the terminal to `child`: leaves raw mode, the alternate screen,
    /// and bracketed paste, runs it, takes the terminal back, and clears.
    /// Headless, it refuses (`Unavailable`), so the executor says so.
    pub fn suspend<R>(&mut self, child: impl FnOnce(&mut Suspended) -> R) -> Result<R, Unavailable>;
    /// Runs `work` on a thread, redrawing the program until it ends.
    pub fn while_drawing<R: Send + 'static>(&mut self, work: impl FnOnce() -> R + Send + 'static) -> R;
    /// The terminal's writer: an OSC 52 copy goes here (`clipboard::copy`);
    /// headless, a buffer a test can read.
    pub fn output(&mut self) -> &mut dyn Write;
}

/// The screen while a child has it.
pub struct Suspended { /* ... */ }
impl Suspended {
    /// Clears the screen before the child draws (nested multiplexers and
    /// mosh can show the old frame through the first writes).
    pub fn clear(&mut self) -> io::Result<()>;
    /// Prints `prompt` and waits for Enter, so the child's error stays
    /// readable before the frame comes back.
    pub fn wait_for_enter(&mut self, prompt: &str) -> io::Result<()>;
}

/// The terminal taken over: raw mode, the alternate screen, bracketed paste,
/// and a panic hook (installed once) that hands it back. Dropping hands it
/// back too.
pub struct TerminalGuard { /* DefaultTerminal, options */ }
pub struct TerminalOptions {
    pub bracketed_paste: bool,
    /// Clear before the first frame.
    pub clear_on_enter: bool,
}
impl TerminalGuard {
    pub fn enter(options: TerminalOptions) -> io::Result<Self>;
}

/// Where events come from: the terminal, or a script in tests, so the loop
/// itself is testable without one.
pub trait EventSource {
    fn next(&mut self, timeout: Duration) -> io::Result<Option<Event>>;
}

/// The loop, over a program whose messages are `M` and effects `Fx`.
pub struct Runtime<M, Fx> { /* poll interval, round bound */ }
impl<M, Fx> Runtime<M, Fx> {
    pub fn new() -> Self; // a 100 ms poll
    pub fn poll_every(self, every: Duration) -> Self;
    /// Takes the terminal, loops until `should_quit`, hands it back on
    /// return, error, and panic.
    pub fn run<P, X>(&self, program: &mut P, executor: &mut X, options: TerminalOptions) -> io::Result<()>
    where
        P: Program<Msg = M, Effect = Fx>,
        X: Executor<P>;
    /// The same loop over any backend and event source.
    pub fn run_on<P, X, B, E>(&self, program: &mut P, executor: &mut X, terminal: &mut Terminal<B>, events: &mut E) -> io::Result<()>
    where
        P: Program<Msg = M, Effect = Fx>,
        X: Executor<P>,
        B: Backend,
        E: EventSource;
}

/// The loop's body without a terminal, over `testkit::render::FrameDriver`.
pub struct Headless<M, Fx> { /* FrameDriver, inbox, output buffer */ }
impl<M, Fx> Headless<M, Fx> {
    pub fn new(width: u16, height: u16) -> Self;
    /// Each key: a frame drawn, then the key handled and what it set off run.
    pub fn press<P, X>(&mut self, program: &mut P, executor: &mut X, keys: impl IntoIterator<Item = KeyEvent>)
    where
        P: Program<Msg = M, Effect = Fx>,
        X: Executor<P>;
    /// `poll` and `pending` until nothing is left, a bounded number of rounds.
    pub fn settle<P, X>(&mut self, program: &mut P, executor: &mut X)
    where
        P: Program<Msg = M, Effect = Fx>,
        X: Executor<P>;
    /// One fresh frame.
    pub fn frame<P: Program<Msg = M, Effect = Fx>>(&mut self, program: &mut P) -> &Buffer;
    /// What the executors wrote to the terminal's writer (OSC 52 copies).
    pub fn output(&self) -> &[u8];
}
```

One pass of the loop, live and headless alike:

```text
settle(executor.start())
until program.should_quit():
    settle(program.tick(now) + executor.poll())
    run program.pending()
    program.keep(draw(program.view))
    match events.next(poll):
        key press      -> settle(program.key(key))
        paste          -> for each char of single_line_paste(text), while program.takes_text():
                              settle(program.key(char))
        resize         -> nothing; the next draw resizes
        anything else  -> nothing
program.finish()

settle(messages): for each message (and each message an effect returns, bounded):
    for each effect of program.update(message):
        executor.can_run(effect) ? executor.run(effect) : executor.refused(effect)
```

### 3.2 Checked against the two loops and snapshot mode

Conspectus already has one loop over a `LoopMode` (ADR 0085 contract 5); the
modes differ in their subscriptions and in how they dispatch an `Action`.

| Today | Under the seam |
| --- | --- |
| `run`, `run_static`, `run_from_fixture`: `term::enter`, clear, a loop, `term::leave` (three copies) | `Runtime::run(program, executor, TerminalOptions { bracketed_paste: true, clear_on_enter: true })` |
| `LoopMode::init` (live: provider status, the first discovery worker) | `Executor::start` |
| `LoopMode::drain` (live: 105 lines over four channels and the content driver; fixture: scans run to completion) | `Executor::poll` |
| `refresh_preview_if_needed` every pass; the App's `take_refresh_request` and `take_handoff_refresh_request` flags | `Program::pending` returns the load and refresh effects |
| `app.search_overlay_tick`, `app.prepare_toast_for_render` | `Program::tick` |
| `action_for_event`, then `LoopMode::dispatch(Action)` | `Program::key` returns messages, once `Action` is folded into `Msg` |
| `type_paste` | the runtime's paste rule over `Program::takes_text` |
| `execute_effects_live` and `execute_effects`, with `pure_executor_runs` | the live and fixture executors' `run` and `can_run` |
| `run_with_terminal_spinner` | `Exec::while_drawing` |
| `term::leave()` ... `*terminal = term::enter(); terminal.clear()` at three hand-off sites | `Exec::suspend`, with `Suspended::clear` and `wait_for_enter` for the external viewer |
| `app.persist_state()` after the loop | `Program::finish` |
| `--snapshot`: `FrameDriver::drive`, `apply_action`, `run_effects_opening_viewers`, `reaches_outside`, skipped copies | `Headless::press` with a snapshot executor whose `can_run` is `reaches_outside`; copies run into `Headless::output` |

What stays host code around the seam: snapshot mode's flags (fixture load and
export, the frozen clock, seeded messages, the pane region), the visibility
watcher (a tmux source the live executor polls), the spinner's in-flight
messages, and the fixture reload on `r`.

The seam fits all three modes only after one host change: the keymap's
`Action` layer folded into `Msg` and `Effect` (ADR 0085 contract 1). Today
the three modes run three different dispatches over its 23 variants, and the
snapshot driver handles a subset; until they are messages, `Program::key`
cannot return messages and each mode keeps its own dispatch.

### 3.3 Checked against the layer-2 contracts

For each contract, what the seam needs from it:

| Contract | What the seam needs | Gap |
| --- | --- | --- |
| The overlay contract and `ModalStack` (`route`, `settle`, `takes_text`, `fills_screen`, `draw`) | Nothing new. The stack stays host-owned and lent out with the host as its context (`App::with_modals`); `Program::key` calls `route` then `settle`, as the guide's one-page program does; `Program::takes_text` forwards `ModalStack::takes_text`; drawing stays in `Screen::overlays`. | None. The host's bookkeeping when a modal closes (`App::modal_closed`) stays inside `Program::key`. |
| Keys (`Keymap::resolve` over `ActiveScopes`, key tables, footers, `KeyRole`) | The press filter moves into the runtime. Which scopes are active (focus) and what a binding means stay in `Program::key`; help and footers are untouched. | A typed character for the paste rule is built with `KeyEvent::new`; the test kit's `typed` is test-only. Trivial. |
| Feedback (`Feedback`, `Severity`, `Toasts`, `Log`, `clipboard::copy`) | `Toasts::tick` from `Program::tick`. `clipboard::copy(sink, text)` with the sink `Exec::output()`: the terminal's writer live, a buffer headless, so snapshot mode could run the copies it skips today because their escape would land in the frame. A refused effect becomes a `Feedback` through `Executor::refused`. | The rule that an executor says what it cannot run has no framework home today (a host toast); the seam gives it `can_run` and `refused`. |
| `AppFrame` and `Screen` (`Report`, overlays filling the screen, toasts last) | `Program::view` is `AppFrame::render` over the host's `Screen`; `Program::Report` is `Screen::Report`; `Program::keep` is `App::keep_viewports`. The mutable draw (warming the Preview's wrapped lines) fits. | None. |
| The test kit (key scripts, `FrameDriver`, buffer probes, the ANSI writer, goldens, the conformance `Rig`) | `Headless` is `FrameDriver::drive` (a frame before each key, as the loop draws) plus effects run on a test executor; the ANSI writer prints snapshot frames; a `Rig` for any `Program` could replace the host's hand-written one (`consistency_tests/conventions.rs`, about 50 lines). | `FrameDriver` has no effect step and no inbox; `Headless` adds them. |
| Text entry (`single_line_paste`, house rule 15) | The paste rule: flatten, then type one character at a time while a text field has the keys. | None. |

### 3.4 Choices the seam forces

1. **Executors borrow the model, or return messages only.** Runtime code
   writes into the App 138 times today (80 `app.update`, 36 `app.report`, 5
   `app.post_toast`, 17 `app.open_*`), and the effects of those nested
   updates are dropped. Returning messages only would make executors testable
   without the App but multiplies the port; the sketch borrows
   (`Exec::program`), and messages-only can come later, one executor at a
   time.
2. **One inbox, or executor-owned channels.** A discovery result passes
   through the worker ledger and has its row tree built from the App's
   current view before it becomes `Msg::SetData`, so it cannot travel as a
   message on a framework channel unless the ledger moves into the App. The
   sketch keeps the channels in the executor and drains them in `poll`;
   `Exec::inbox` is for simple workers.
3. **A feature of `tuikit`, or a sibling crate.** The runtime reads terminal
   events, switches the terminal's modes, and installs a panic hook: the
   first process-wide state the framework would touch. The architecture
   guide's "never" list, `tests/terminal_hygiene.rs` (which forbids taking
   the terminal anywhere in `crates/tuikit/src`), and framework record 0002
   all say the framework does none of that, and record 0002's reason is that
   a host with another loop (asynchronous, fed by a server) can still use
   every control. A sibling crate (`crates/tuikit-run`, say) keeps all three
   intact and the controls provably loop-agnostic; a feature keeps one crate
   to separate but needs carve-outs in each. The sketch prefers the sibling
   crate.
4. **No new dependency either way.** Everything above is in ratatui and its
   crossterm re-export, which the framework already reaches.

## 4. The Cost

### 4.1 Stories

If the trigger fires, in the style of the m-25 stories (S: one module and its
tests; M: several files or one cross-cutting rule; L: many call sites, tests
rewritten, and visible behavior to catalog):

| # | Story | Side | Size | Depends on |
| --- | --- | --- | --- | --- |
| 1 | Fold the keymap's `Action` layer into `Msg` and `Effect`: keys resolve to messages, `r` and `Ctrl-R` emit refresh effects, the openers become reducer arms, and the live, fixture, and snapshot loops route a key one way (filed now as CSP-802, since it pays alone) | host | L | none |
| 2 | Make state-driven work effects: the preview load, the refresh a write requests, and the refresh after a hand-off are effects the App asks for, not flags and a loop step | host | M | 1 |
| 3 | Record the runtime's boundary: an ADR superseding ADR 0200 (and amending ADR 0150 decision 2), a framework record superseding record 0002, and the guards' rules (the sibling crate, its allowlist, terminal hygiene) | both | S | none |
| 4 | `TerminalGuard` and suspend: enter and leave with bracketed paste, the panic hook, `Drop`, suspend with clear and wait-for-Enter; `term.rs` and the three hand-off sites move onto it | both | M | 3 |
| 5 | `Program`, `Executor`, `Exec`, `EventSource`, and the one loop: the press filter, the paste rule, refused effects, `while_drawing`; tested with a scripted event source and a test backend | framework | L | 3, 4 |
| 6 | `Headless` over `FrameDriver`: the loop body with a test executor, the output buffer, the snapshot writer | framework | M | 5 |
| 7 | Port the live loop: the live mode becomes the live executor; `run_loop`, `LoopMode`, and `draw_frame` go; `&mut DefaultTerminal` leaves 27 signatures | host | L | 1, 2, 4, 5 |
| 8 | Port fixture mode and `--snapshot` onto the runtime and `Headless`; their executors replace `StaticMode` and `apply_action` | host | M | 6, 7 |
| 9 | Document it: the crate's guide and pages, the guide's one-page program on the runtime, the layer table, the roadmap; the separation rehearsal exports both crates | framework | M | 5, 6 |

Nine stories: three L, five M, one S. Stories 1 and 2 are host-only and finish
ADR 0085 contracts 1 and 5 whatever happens to the rest.

### 4.2 Risks

- **Async discovery workers.** The live mode owns four channels (discovery
  results, preview loads, forge fetches, the visibility watcher) and the
  content driver's scans; six `std::thread::spawn` sites in the runtime
  start their workers and the spinner's. Results are
  ordered by the worker ledger, the first generation after a hand-off
  settles frozen rows, and the row tree is built on the UI thread from the
  App's current view. A framework inbox cannot carry that without moving the
  ledger into the App (choice 2); keeping the channels in the executor keeps
  the risk low but leaves the framework a hook, not a mechanism.
- **`&mut DefaultTerminal` threaded through handlers.** 27 function
  signatures take it (16 in `runtime.rs`, 9 in `launch.rs`, 2 in
  `executor.rs`); every hand-off replaces it in place (`*terminal =
  term::enter()`), and the spinner redraws through a closure that captures
  it. All of them become `Exec`. Mechanical, but wide, and an ordering
  mistake shows as a frozen or garbled screen rather than a failing test.
- **Subprocess hand-offs.** Three rituals: tmux attach (`switch-client` when
  nested, which returns at once and leaves the TUI running in a hidden
  window, hence the visibility watcher), the other backends' runner attach,
  and the external viewer (clear and home before, wait for Enter after a
  failure). The clears work around mosh and nested multiplexers. The
  framework cannot start the child (its capability guard), so `suspend`
  takes the host's closure. Only the 32 PTY tests cover these paths.
- **Tests that depend on the loop.** No unit test drives `run_loop` today
  (the seam's event source would fix that). 48 call sites in 15 test files
  use the runtime's helpers (`modal_key_pure`, `action_for_event`,
  `draw_screen`, `dispatch`, `press_in_fixture_mode`), and 20 `translate_*`
  tests assert `Action` values (45 `Action::` references in
  `runtime_tests.rs`); story 1 rewrites them. `tests/terminal_hygiene.rs`
  changes its rule.
- **Visible drift in fixture and snapshot modes.** One dispatch replaces
  three, so their refusals change: the fixture TUI's "scenario TUI is static;
  attach is disabled" becomes the pure executor's "that isn't available in
  this mode" (CSP-716 already made modal picks say that), and snapshot
  mode's "skipped unsupported action: Attach" becomes "skipped an attach".
  Each is a consistency win to catalog under ADR 0151, but each touches
  frames and docs.
- **A public surface to keep.** Six types and three traits the host depends
  on, their page, their doctests, and a second crate to rehearse separating.
  The crate promises no semver, which softens this but does not remove it.

## 5. The Benefit

### 5.1 What a second consumer gains

- **A runnable skeleton.** Today a program on the framework writes its own
  loop. Over the guide's one-page program (its `press`, `update`, and
  `Screen`), the loop is the four steps the guide names (read an event, hand
  a key to `press`, draw with `AppFrame::render`, tick the toasts) inside
  ratatui's `init` and `restore`, plus the paste rule and paste turned off on
  a panic: about 30 lines, by estimate. The runtime saves those, and the
  mistakes they invite (handling a key before the frame it acts on is drawn,
  typing a paste into a menu).
- **Terminal safety.** ratatui 0.30's `init`, `restore`, and `run` already
  restore raw mode and the alternate screen on a panic. What Conspectus adds
  is bracketed paste turned off on exit and on a panic, and the suspend and
  resume ritual around a child process: the 51 lines of `term.rs` and 59 of
  hand-off frames, written once and tested.
- **Headless testing.** `FrameDriver::drive` already draws before each key.
  `Headless` adds effects run on a test executor and a terminal writer a test
  can read, so a program's interactions are testable end to end without a
  terminal, and its live loop is testable with a scripted event source.

### 5.2 What it costs the framework

- No new crate, but new capabilities: terminal events, terminal modes, a panic
  hook, and stdin for the wait-for-Enter prompt. The independence rules and
  record 0002 change, or a sibling crate carries them.
- A surface to keep in step: the three traits, `Exec`, `Suspended`,
  `TerminalGuard` and its options, `Runtime`, `Headless`, their page and
  doctests, and the separation rehearsal over a second crate.
- Tests the framework does not need today: a scripted event source, a test
  backend for the live loop, and the hand-off ritual against a fake child.

### 5.3 Is there a second consumer?

No.

- `examples/pantry.rs` (756 lines), the widget gallery, runs on `tui-pantry`'s
  own loop.
- `src/bin/csp.rs` (7 lines) is Conspectus under its short name.
- CSP-342 (milestone m-11, "(later)", To Do) proposes lifting `src/viewer/`
  into its own crate with a thin binary that opens one transcript. It is
  unscheduled, and that binary's loop is a `Pager` and a key: the ~30-line
  case, with no hand-off and no background work.
- The framework is `publish = false`, for internal reuse; nothing outside
  this repository depends on it.

## 6. Recommendation: LATER

**Not now.** The generic part is small (11.5% of the runtime), no program
would use it, and the cost (nine stories, the framework's founding boundary,
its terminal-ownership guard) buys nothing until one does. The framework's
documentation already shows a host's loop, and ratatui covers most of the
terminal safety.

**Not never.** A second program that hands the terminal to child processes,
runs background work, or wants a headless snapshot mode would reuse most of
the 643 generic lines, including the parts that are hard to get right (the
paste mode on a panic, the clears around a child, drawing before each key).

**The trigger that flips it to GO:** a second program is committed (a
scheduled story, not an idea) to run on `tuikit` with an interactive loop of
its own, and it needs at least one of a terminal hand-off to a child process,
background work that reports back as messages, or a headless mode for tests
or snapshots. CSP-342 scheduling its standalone viewer, with any of those, is
the likeliest way it fires. When it does:

1. If the program needs only terminal safety, stories 3 and 4 alone (the
   boundary record and `TerminalGuard`), one S and one M.
2. Otherwise the outline in 4.1, in its order. Stories 1 and 2 may already
   be done by then.

**What to do now:** nothing in the framework. On the host side, CSP-802 folds
the `Action` layer into `Msg` (story 1 above), on its own merits: it removes
the three dispatch copies (257 lines), makes the fixture and snapshot modes
follow the live one by construction instead of by convention (ADR 0085
contracts 1 and 5), and is the precondition any later extraction needs.

## 7. Review

The write-up was checked against each layer-2 contract (section 3.3: the
overlay contract and modal stack, keys, feedback, `AppFrame` and `Screen`,
the test kit, and text entry's paste rule) and against the live loop, the
fixture loop, and snapshot mode (section 3.2). Every contract serves the seam
as it is; the gaps are a non-test typed-key constructor and a home for the
rule that an executor says what it cannot run. The one change the seam needs
first is in Conspectus, not in a contract: the `Action` layer.
