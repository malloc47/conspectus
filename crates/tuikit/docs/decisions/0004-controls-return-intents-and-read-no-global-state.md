# 0004: A control performs no effect and reads no global state

| | |
| --- | --- |
| Status | Accepted |

## Decision

- Handling a key returns an outcome that says what the user meant, and the
  host interprets it in its own runtime. A commit carries the host's message
  `M`, built by a closure or a mapping the host supplies.
- A control is generic over what the host needs to say and know, never over
  a concrete host type: the message `M`, a context `Cx` borrowed into
  `handle` and `render`, a row identity `Id`, a row or result type `R`.
  Host meaning enters in five ways only: a type parameter, a closure, a
  trait the host implements, a named style, or a sink or source the host
  supplies.
- No control starts a process, opens a socket, installs a signal handler,
  or writes, renames, or deletes a file. What a control needs from outside
  comes from the host: the clipboard takes a `Write` sink, path completion a
  read-only source.
- No control reads global state. Time comes from the host (`now`
  arguments); a control that needs a directory takes it as an input and
  never reads the working directory; the host takes the terminal over and
  hands the controls frames to draw and key and paste events to handle. The
  one environment variable the crate reads is the test kit's switch for
  accepting goldens, which production builds leave out.

## Rationale

A control whose inputs are a key, a context, and a theme, and whose outputs
are an outcome and a buffer, is testable with nothing else. The effect
lifecycle (queuing, undo, retries, handing the terminal to a child process)
then lives in one place, the host's runtime, and two instances of a control
never share hidden state. Hosts differ in how they run effects; the controls
need not know.

## Consequences

- A control with something to say (a refusal, a confirmation) returns it as
  an outcome or a `Feedback` rather than showing or doing it.
- Tests need no terminal, no sleep, and no file system.
- A host's own scans (working directory, terminal ownership, its manifest of
  programs and write paths) find nothing in the crate to exempt.

## Alternatives considered

- **Controls that do their own effects** (copy, save). Shorter call sites,
  but untestable without the world and a second executor beside the host's.
- **A framework clock.** Fewer parameters, but tests would sleep.
- **An operating-system clipboard crate.** Rejected in record
  [0039](0039-the-clipboard-is-osc-52-to-a-sink.md).

## Origin

- host ADR 0150, "The interface framework is a workspace member crate":
  decision 4 (controls generic over `M` and `Cx`, no processes, no writes,
  no global state, sinks and sources supplied by the host).
- host ADR 0085, "The interface's Elm/MVU architecture": contract 2
  (effects as data, run by one executor in the host), which the controls'
  outcomes feed.
- host ADR 0024, "The interface's runtime, architecture, and dependency
  policy": the runtime as the only code that touches the terminal.
